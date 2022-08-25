use crate::auth::B2Authorization;
use crate::buckets::Bucket;
use crate::BytesString;

use serde::Serialize;

use crate::b2_future::B2Future;
use crate::client::{serde_body, ApiCall};
use crate::B2Error;
use http::header::HeaderMap;
use http::method::Method;
use http::uri::Uri;
use hyper::client::ResponseFuture;
use hyper::Body;
use std::convert::TryFrom;

/// The [`b2_delete_bucket`] api call.
///
/// You can execute this api call using a [`B2Client`], which will return the deleted
/// [`Bucket`] if successful.
///
/// # Example
///
/// ```
/// use backblaze_b2::B2Error;
/// use backblaze_b2::auth::B2Credentials;
/// use backblaze_b2::client::B2Client;
/// use backblaze_b2::buckets::{Bucket, BucketType, CreateBucket, DeleteBucket};
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
///     println!("{:#?}", bucket);
///
///     // Delete it again.
///     client.send(DeleteBucket::new(&auth, &bucket.bucket_id)).await?;
///
///     Ok(())
/// }
/// ```
///
/// [`b2_delete_bucket`]: https://www.backblaze.com/b2/docs/b2_delete_bucket.html
/// [`B2Client`]: ../client/struct.B2Client.html
/// [`Bucket`]: struct.Bucket.html
#[derive(Clone, Debug)]
pub struct DeleteBucket<'a> {
    auth: &'a B2Authorization,
    bucket_id: &'a str,
}
impl<'a> DeleteBucket<'a> {
    /// Create a new api call that deletes the specified bucket.
    pub fn new(auth: &'a B2Authorization, bucket_id: &'a str) -> DeleteBucket<'a> {
        DeleteBucket { auth, bucket_id }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DeleteBucketRequest<'a> {
    account_id: &'a BytesString,
    bucket_id: &'a str,
}

impl<'a> ApiCall for DeleteBucket<'a> {
    type Future = B2Future<Bucket>;
    const METHOD: Method = Method::POST;
    fn url(&self) -> Result<Uri, B2Error> {
        Uri::try_from(format!("{}/b2api/v2/b2_delete_bucket", self.auth.api_url))
            .map_err(B2Error::from)
    }
    fn headers(&self) -> Result<HeaderMap, B2Error> {
        let mut map = HeaderMap::new();
        map.append("Authorization", self.auth.auth_token());
        Ok(map)
    }
    fn body(&mut self) -> Result<Body, B2Error> {
        serde_body(&DeleteBucketRequest {
            account_id: &self.auth.account_id,
            bucket_id: self.bucket_id,
        })
    }
    fn finalize(self, fut: ResponseFuture) -> B2Future<Bucket> {
        B2Future::new(fut)
    }
    fn error(self, err: B2Error) -> B2Future<Bucket> {
        B2Future::err(err)
    }
}
