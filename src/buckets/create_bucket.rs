use crate::BytesString;
use crate::auth::B2Authorization;
use crate::buckets::{Bucket, BucketType, CorsRule, LifecycleRule, NoBucketInfo};

use serde::Serialize;

use crate::B2Error;
use crate::b2_future::B2Future;
use crate::client::{ApiCall, serde_body};
use http::header::HeaderMap;
use http::method::Method;
use http::uri::Uri;
use hyper::Body;
use hyper::client::ResponseFuture;
use std::convert::TryFrom;

/// The [`b2_create_bucket`] api call.
///
/// You can execute this api call using a [`B2Client`], which will result in a
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
/// [`b2_create_bucket`]: https://www.backblaze.com/b2/docs/b2_create_bucket.html
/// [`B2Client`]: ../client/struct.B2Client.html
/// [`Bucket`]: struct.Bucket.html
#[derive(Clone, Debug)]
pub struct CreateBucket<'a, Info: Serialize> {
    auth: &'a B2Authorization,
    bucket_name: &'a str,
    bucket_type: BucketType,
    bucket_info: Info,
    cors_rules: &'a [CorsRule],
    lifecycle_rules: &'a [LifecycleRule],
}
impl<'a> CreateBucket<'a, NoBucketInfo> {
    /// Create a new api call with the specified name and type.
    pub fn new(
        auth: &'a B2Authorization,
        bucket_name: &'a str,
        bucket_type: BucketType,
    ) -> Self {
        CreateBucket {
            auth,
            bucket_name,
            bucket_type,
            bucket_info: NoBucketInfo,
            cors_rules: &[],
            lifecycle_rules: &[],
        }
    }
}
impl<'a, I: Serialize> CreateBucket<'a, I> {
    /// Set the info assigned to this bucket. This value must serialize into a map with
    /// at most 10 keys. You can use a [`HashMap`] as the type in this argument.
    ///
    /// See the [official documentation][1] for more details on the restrictions on
    /// bucket info.
    ///
    /// [`HashMap`]: https://doc.rust-lang.org/std/collections/struct.HashMap.html
    /// [1]: https://www.backblaze.com/b2/docs/buckets.html#bucketInfo
    pub fn bucket_info<Info: Serialize>(self, info: Info) -> CreateBucket<'a, Info> {
        CreateBucket {
            auth: self.auth,
            bucket_name: self.bucket_name,
            bucket_type: self.bucket_type,
            bucket_info: info,
            cors_rules: self.cors_rules,
            lifecycle_rules: self.lifecycle_rules,
        }
    }
    /// Set the cors rules assigned to this bucket.
    ///
    /// See the [official documentation][1] for more information.
    ///
    /// [1]: https://www.backblaze.com/b2/docs/cors_rules.html
    pub fn cors_rules(self, cors_rules: &'a [CorsRule]) -> Self {
        CreateBucket {
            cors_rules,
            ..self
        }
    }
    /// Set the lifetime rules assigned to this bucket.
    ///
    /// See the [official documentation][1] for more information.
    ///
    /// [1]: https://www.backblaze.com/b2/docs/lifecycle_rules.html
    pub fn lifecycle_rules(self, rules: &'a [LifecycleRule]) -> Self {
        CreateBucket {
            lifecycle_rules: rules,
            ..self
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateBucketRequest<'a, Info> {
    account_id: &'a BytesString,
    bucket_name: &'a str,
    bucket_type: &'a BucketType,
    bucket_info: &'a Info,
    cors_rules: &'a [CorsRule],
    lifecycle_rules: &'a [LifecycleRule],
}

impl<'a, Info: Serialize> ApiCall for CreateBucket<'a, Info> {
    type Future = B2Future<Bucket>;
    const METHOD: Method = Method::POST;
    fn url(&self) -> Result<Uri, B2Error> {
        Uri::try_from(format!("{}/b2api/v2/b2_create_bucket", self.auth.api_url))
            .map_err(B2Error::from)
    }
    fn headers(&self) -> Result<HeaderMap, B2Error> {
        let mut map = HeaderMap::new();
        map.append("Authorization", self.auth.auth_token());
        Ok(map)
    }
    fn body(&self) -> Result<Body, B2Error> {
        serde_body(&CreateBucketRequest {
            account_id: &self.auth.account_id,
            bucket_name: &self.bucket_name,
            bucket_type: &self.bucket_type,
            bucket_info: &self.bucket_info,
            cors_rules: &self.cors_rules,
            lifecycle_rules: &self.lifecycle_rules,
        })
    }
    fn finalize(self, fut: ResponseFuture) -> B2Future<Bucket> {
        B2Future::new(fut)
    }
    fn error(self, err: B2Error) -> B2Future<Bucket> {
        B2Future::err(err)
    }
}

