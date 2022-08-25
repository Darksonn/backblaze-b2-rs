use crate::auth::B2Authorization;
use crate::buckets::Bucket;
use crate::BytesString;

use serde::de::{Deserializer, Error, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};

use futures::future::FusedFuture;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::b2_future::B2Future;
use crate::client::{serde_body, ApiCall};
use crate::B2Error;
use http::header::HeaderMap;
use http::method::Method;
use http::uri::Uri;
use hyper::client::ResponseFuture;
use hyper::Body;
use std::convert::TryFrom;

/// A filtered [`b2_list_buckets`] api call.
///
/// You can execute this api call using a [`B2Client`], which will return an
/// option that contains the specified [`Bucket`] if it exists.
///
/// # Example
///
/// ```
/// use backblaze_b2::B2Error;
/// use backblaze_b2::auth::B2Credentials;
/// use backblaze_b2::client::B2Client;
/// use backblaze_b2::buckets::{Bucket, BucketType, CreateBucket, GetBucket, DeleteBucket};
///
/// #[tokio::main]
/// async fn main() -> Result<(), B2Error> {
///     let mut client = B2Client::new();
///     let creds = B2Credentials::from_file("credentials.txt")?;
///     let auth = client.send(creds.authorize()).await?;
///
///     // Buckets on b2 are globally unique, so we create a random name.
///     use rand::Rng;
///     let bucket_name: String = rand::thread_rng()
///         .sample_iter(&rand::distributions::Alphanumeric)
///         .take(50)
///         .collect();
///
///     let bucket: Bucket = client.send(
///         CreateBucket::new(&auth, &bucket_name, BucketType::Private)
///     ).await?;
///
///     let by_name: Option<Bucket> = client.send(
///         GetBucket::by_name(&auth, &bucket_name)
///     ).await?;
///     assert_eq!(bucket, by_name.unwrap());
///
///     let by_id: Option<Bucket> = client.send(
///         GetBucket::by_id(&auth, &bucket.bucket_id)
///     ).await?;
///     assert_eq!(bucket, by_id.unwrap());
///
///     // Delete it again.
///     client.send(DeleteBucket::new(&auth, &bucket.bucket_id)).await?;
///
///     Ok(())
/// }
/// ```
///
/// [`b2_list_buckets`]: https://www.backblaze.com/b2/docs/b2_list_buckets.html
/// [`B2Client`]: ../client/struct.B2Client.html
/// [`Bucket`]: struct.Bucket.html
#[derive(Clone, Debug)]
pub struct GetBucket<'a> {
    auth: &'a B2Authorization,
    bucket_name: Option<&'a str>,
    bucket_id: Option<&'a str>,
}
impl<'a> GetBucket<'a> {
    /// Create a new api call that fetches the specified bucket.
    pub fn by_name(auth: &'a B2Authorization, name: &'a str) -> GetBucket<'a> {
        GetBucket {
            auth,
            bucket_name: Some(name),
            bucket_id: None,
        }
    }
    /// Create a new api call that fetches the specified bucket.
    pub fn by_id(auth: &'a B2Authorization, id: &'a str) -> GetBucket<'a> {
        GetBucket {
            auth,
            bucket_name: None,
            bucket_id: Some(id),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GetBucketRequest<'a> {
    account_id: &'a BytesString,
    #[serde(skip_serializing_if = "Option::is_none")]
    bucket_name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bucket_id: Option<&'a str>,
}

impl<'a> ApiCall for GetBucket<'a> {
    type Future = GetBucketFuture;
    const METHOD: Method = Method::POST;
    fn url(&self) -> Result<Uri, B2Error> {
        Uri::try_from(format!("{}/b2api/v2/b2_list_buckets", self.auth.api_url))
            .map_err(B2Error::from)
    }
    fn headers(&self) -> Result<HeaderMap, B2Error> {
        let mut map = HeaderMap::new();
        map.append("Authorization", self.auth.auth_token());
        Ok(map)
    }
    fn body(&mut self) -> Result<Body, B2Error> {
        serde_body(&GetBucketRequest {
            account_id: &self.auth.account_id,
            bucket_name: self.bucket_name,
            bucket_id: self.bucket_id,
        })
    }
    fn finalize(self, fut: ResponseFuture) -> GetBucketFuture {
        GetBucketFuture {
            future: B2Future::new(fut),
        }
    }
    fn error(self, err: B2Error) -> GetBucketFuture {
        GetBucketFuture {
            future: B2Future::err(err),
        }
    }
}

#[derive(Deserialize)]
struct GetBucketResponse {
    #[serde(deserialize_with = "list_as_option")]
    buckets: Option<Bucket>,
}

/// A future that resolves to an optional [`Bucket`].
///
/// This future is created by the [`GetBucket`] api call.
///
/// [`Bucket`]: struct.Bucket.html
/// [`GetBucket`]: struct.GetBucket.html
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct GetBucketFuture {
    future: B2Future<GetBucketResponse>,
}
impl Future for GetBucketFuture {
    type Output = Result<Option<Bucket>, B2Error>;
    /// Attempt to resolve the future to a final value.
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.future).poll(cx) {
            Poll::Ready(Ok(response)) => Poll::Ready(Ok(response.buckets)),
            Poll::Ready(Err(err)) => Poll::Ready(Err(err)),
            Poll::Pending => Poll::Pending,
        }
    }
}
impl FusedFuture for GetBucketFuture {
    /// Returns `true` if this future has completed.
    fn is_terminated(&self) -> bool {
        self.future.is_terminated()
    }
}

fn list_as_option<'de, D>(deserializer: D) -> Result<Option<Bucket>, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_seq(ListAsOptionVisitor)
}

struct ListAsOptionVisitor;

impl<'de> Visitor<'de> for ListAsOptionVisitor {
    type Value = Option<Bucket>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a list with zero or one elements")
    }

    fn visit_seq<V>(self, mut seq: V) -> Result<Self::Value, V::Error>
    where
        V: SeqAccess<'de>,
    {
        let result = seq.next_element()?;
        if seq.next_element::<Bucket>()?.is_some() {
            let len = match seq.size_hint() {
                Some(len) => len + 2,
                None => {
                    let mut len = 2;
                    while seq.next_element::<Bucket>()?.is_some() {
                        len += 1;
                    }
                    len
                }
            };
            Err(Error::invalid_length(
                len,
                &"a list with zero or one elements",
            ))
        } else {
            Ok(result)
        }
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Some(Bucket::deserialize(deserializer)?))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use crate::auth::B2Credentials;
    use crate::buckets::GetBucket;
    use crate::client::B2Client;
    use crate::B2Error;

    use futures::future::try_join;

    #[tokio::test]
    async fn missing_bucket_none() -> Result<(), B2Error> {
        let mut client = B2Client::new();
        let creds = B2Credentials::from_file("credentials.txt")?;
        let auth = client.send(creds.authorize()).await?;

        use rand::Rng;
        let random: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(50)
            .collect();

        let by_name = client.send(GetBucket::by_name(&auth, &random));

        let by_id = client.send(GetBucket::by_id(&auth, &"4a48fe7875c6214145260818"));

        let res = try_join(by_name, by_id).await?;
        assert_eq!(res, (None, None));

        Ok(())
    }
}
