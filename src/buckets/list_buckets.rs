use crate::auth::B2Authorization;
use crate::buckets::{Bucket, BucketType};
use crate::BytesString;

use serde::{Deserialize, Serialize};

use futures::future::FusedFuture;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::b2_future::{B2Future, B2Stream};
use crate::client::{serde_body, ApiCall};
use crate::B2Error;
use http::header::HeaderMap;
use http::method::Method;
use http::uri::Uri;
use hyper::client::ResponseFuture;
use hyper::Body;
use std::convert::TryFrom;

/// The [`b2_list_buckets`] api call.
///
/// You can execute this api call using a [`B2Client`], which will return a list of
/// [`Buckets`] if successful.
///
/// This api call will by default create a futur that resolves to `Vec<Bucket>`, but
/// by calling the [`into_stream`] method on the future, you can obtain a stream of
/// buckets instead.
///
/// # Example
///
/// ```
/// use backblaze_b2::B2Error;
/// use backblaze_b2::auth::B2Credentials;
/// use backblaze_b2::client::B2Client;
/// use backblaze_b2::buckets::{Bucket, ListBuckets};
///
/// #[tokio::main]
/// async fn main() -> Result<(), B2Error> {
///     let mut client = B2Client::new();
///     let creds = B2Credentials::from_file("credentials.txt")?;
///     let auth = client.send(creds.authorize()).await?;
///
///     let buckets: Vec<Bucket> = client.send(ListBuckets::new(&auth)).await?;
///     println!("{:#?}", buckets);
///     Ok(())
/// }
/// ```
///
/// [`b2_list_buckets`]: https://www.backblaze.com/b2/docs/b2_list_buckets.html
/// [`B2Client`]: ../client/struct.B2Client.html
/// [`Buckets`]: struct.Bucket.html
/// [`into_stream`]: struct.ListBucketsFuture.html#method.into_stream
#[derive(Clone, Debug)]
pub struct ListBuckets<'a> {
    auth: &'a B2Authorization,
    bucket_types: Option<&'a [BucketType]>,
}
impl<'a> ListBuckets<'a> {
    /// Create a new api call that fetches the list of buckets.
    pub fn new(auth: &'a B2Authorization) -> ListBuckets<'a> {
        ListBuckets {
            auth,
            bucket_types: None,
        }
    }
    /// Filter the buckets by type.
    ///
    /// # Example
    ///
    /// To list only public buckets:
    ///
    /// ```
    /// use backblaze_b2::B2Error;
    /// use backblaze_b2::auth::B2Credentials;
    /// use backblaze_b2::client::B2Client;
    /// use backblaze_b2::buckets::{Bucket, BucketType, ListBuckets};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), B2Error> {
    ///     let mut client = B2Client::new();
    ///     let creds = B2Credentials::from_file("credentials.txt")?;
    ///     let auth = client.send(creds.authorize()).await?;
    ///
    ///     let buckets: Vec<Bucket> = client.send(
    ///         ListBuckets::new(&auth).bucket_types(&[BucketType::Public])
    ///     ).await?;
    ///     println!("{:#?}", buckets);
    ///     Ok(())
    /// }
    /// ```
    pub fn bucket_types(self, bucket_types: &'a [BucketType]) -> Self {
        ListBuckets {
            bucket_types: Some(bucket_types),
            ..self
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ListBucketsRequest<'a> {
    account_id: &'a BytesString,
    #[serde(skip_serializing_if = "Option::is_none")]
    bucket_types: Option<&'a [BucketType]>,
}

impl<'a> ApiCall for ListBuckets<'a> {
    type Future = ListBucketsFuture;
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
        serde_body(&ListBucketsRequest {
            account_id: &self.auth.account_id,
            bucket_types: self.bucket_types,
        })
    }
    fn finalize(self, fut: ResponseFuture) -> ListBucketsFuture {
        ListBucketsFuture {
            future: B2Future::new(fut),
        }
    }
    fn error(self, err: B2Error) -> ListBucketsFuture {
        ListBucketsFuture {
            future: B2Future::err(err),
        }
    }
}

#[derive(Deserialize)]
struct ListBucketsResponse {
    buckets: Vec<Bucket>,
}

/// A future that resolves to a list of [`Buckets`].
///
/// This future is created by the [`ListBuckets`] api call.
///
/// [`Buckets`]: struct.Bucket.html
/// [`ListBuckets`]: struct.ListBuckets.html
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct ListBucketsFuture {
    future: B2Future<ListBucketsResponse>,
}
impl ListBucketsFuture {
    /// Retrieve the buckets from a stream instead of resolving to a vector at the end.
    ///
    /// The stream will start parsing the received json before the full request has
    /// been received, so buckets become available on the stream incrementally.
    ///
    /// # Example
    ///
    /// ```
    /// use backblaze_b2::B2Error;
    /// use backblaze_b2::auth::B2Credentials;
    /// use backblaze_b2::client::B2Client;
    /// use backblaze_b2::buckets::{Bucket, ListBuckets};
    /// use futures::stream::StreamExt;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), B2Error> {
    ///     let mut client = B2Client::new();
    ///     let creds = B2Credentials::from_file("credentials.txt")?;
    ///     let auth = client.send(creds.authorize()).await?;
    ///
    ///     let mut stream = client.send(ListBuckets::new(&auth)).into_stream();
    ///     while let Some(result) = stream.next().await {
    ///         // extract the bucket from the result
    ///         let bucket = result?;
    ///         println!("{:#?}", bucket);
    ///     }
    ///     Ok(())
    /// }
    /// ```
    pub fn into_stream(self) -> B2Stream<Bucket> {
        B2Stream::from_b2_future(self.future, 256)
    }
}
impl Future for ListBucketsFuture {
    type Output = Result<Vec<Bucket>, B2Error>;
    /// Attempt to resolve the future to a final value.
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.future).poll(cx) {
            Poll::Ready(Ok(response)) => Poll::Ready(Ok(response.buckets)),
            Poll::Ready(Err(err)) => Poll::Ready(Err(err)),
            Poll::Pending => Poll::Pending,
        }
    }
}
impl FusedFuture for ListBucketsFuture {
    /// Returns `true` if this future has completed.
    fn is_terminated(&self) -> bool {
        self.future.is_terminated()
    }
}
