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

/// The [`b2_update_bucket`] api call.
///
/// You can execute this api call using a [`B2Client`], which will result in the
/// new [`Bucket`] if successful.
///
/// # Example
///
/// ```
/// use backblaze_b2::B2Error;
/// use backblaze_b2::auth::B2Credentials;
/// use backblaze_b2::client::B2Client;
/// use backblaze_b2::buckets::{Bucket, BucketType, CreateBucket, UpdateBucket, DeleteBucket};
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
///     // Create a private bucket.
///     let mut bucket: Bucket = client.send(
///         CreateBucket::new(&auth, &bucket_name, BucketType::Private)
///     ).await?;
///
///     // Make the bucket public
///     let updated_bucket: Bucket = client.send(
///         UpdateBucket::new(&auth, &bucket.bucket_id)
///             .bucket_type(BucketType::Public)
///     ).await?;
///
///     assert_eq!(bucket.bucket_type, BucketType::Private);
///     assert_eq!(updated_bucket.bucket_type, BucketType::Public);
///
///#    bucket.bucket_type = BucketType::Public;
///#    assert_eq!(bucket, updated_bucket);
///#
///     // Delete it again.
///     client.send(DeleteBucket::new(&auth, &bucket.bucket_id)).await?;
///
///     Ok(())
/// }
/// ```
///
/// [`b2_update_bucket`]: https://www.backblaze.com/b2/docs/b2_update_bucket.html
/// [`B2Client`]: ../client/struct.B2Client.html
/// [`Bucket`]: struct.Bucket.html
#[derive(Clone, Debug)]
pub struct UpdateBucket<'a, Info: Serialize> {
    auth: &'a B2Authorization,
    bucket_id: &'a str,
    bucket_type: Option<BucketType>,
    bucket_info: Option<Info>,
    cors_rules: Option<&'a [CorsRule]>,
    lifecycle_rules: Option<&'a [LifecycleRule]>,
    if_revision_is: Option<u32>,
}
impl<'a> UpdateBucket<'a, NoBucketInfo> {
    /// Create a new api call for the specified bucket id. By default, this call does not
    /// change the bucket.
    ///
    /// Note that in this case, `NoBucketInfo` is used as a dummy type for
    /// update calls that do not change the bucket, but if `NoBucketInfo` is
    /// passed to the `bucket_info` method, the bucket info would be cleared.
    pub fn new(
        auth: &'a B2Authorization,
        bucket_id: &'a str,
    ) -> UpdateBucket<'a, NoBucketInfo> {
        UpdateBucket {
            auth,
            bucket_id,
            bucket_type: None,
            bucket_info: None,
            cors_rules: None,
            lifecycle_rules: None,
            if_revision_is: None,
        }
    }
}
impl<'a, I: Serialize> UpdateBucket<'a, I> {
    /// Only perform this update if the bucket revision is as specified.
    pub fn if_revision_is(self, revision: u32) -> Self {
        UpdateBucket {
            if_revision_is: Some(revision),
            ..self
        }
    }
    /// Set the type of the bucket.
    pub fn bucket_type(self, bucket_type: BucketType) -> Self {
        UpdateBucket {
            bucket_type: Some(bucket_type),
            ..self
        }
    }
    /// Set the info assigned to this bucket. This value must serialize into a map with
    /// at most 10 keys. You can use a [`HashMap`] as the type in this argument.
    ///
    /// See the [official documentation][1] for more details on the restrictions on
    /// bucket info.
    ///
    /// [`HashMap`]: https://doc.rust-lang.org/std/collections/struct.HashMap.html
    /// [1]: https://www.backblaze.com/b2/docs/buckets.html#bucketInfo
    pub fn bucket_info<Info: Serialize>(self, info: Info) -> UpdateBucket<'a, Info> {
        UpdateBucket {
            auth: self.auth,
            bucket_id: self.bucket_id,
            bucket_type: self.bucket_type,
            bucket_info: Some(info),
            cors_rules: self.cors_rules,
            lifecycle_rules: self.lifecycle_rules,
            if_revision_is: self.if_revision_is,
        }
    }
    /// Set the cors rules assigned to this bucket.
    ///
    /// See the [official documentation][1] for more information.
    ///
    /// [1]: https://www.backblaze.com/b2/docs/cors_rules.html
    pub fn cors_rules(self, cors_rules: &'a [CorsRule]) -> Self {
        UpdateBucket {
            cors_rules: Some(cors_rules),
            ..self
        }
    }
    /// Set the lifetime rules assigned to this bucket.
    ///
    /// See the [official documentation][1] for more information.
    ///
    /// [1]: https://www.backblaze.com/b2/docs/lifecycle_rules.html
    pub fn lifecycle_rules(self, rules: &'a [LifecycleRule]) -> Self {
        UpdateBucket {
            lifecycle_rules: Some(rules),
            ..self
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateBucketRequest<'a, Info> {
    account_id: &'a BytesString,
    bucket_id: &'a str,
    bucket_type: Option<BucketType>,
    bucket_info: Option<&'a Info>,
    cors_rules: Option<&'a [CorsRule]>,
    lifecycle_rules: Option<&'a [LifecycleRule]>,
    if_revision_is: Option<u32>,
}

impl<'a, Info: Serialize> ApiCall for UpdateBucket<'a, Info> {
    type Future = B2Future<Bucket>;
    const METHOD: Method = Method::POST;
    fn url(&self) -> Result<Uri, B2Error> {
        Uri::try_from(format!("{}/b2api/v2/b2_update_bucket", self.auth.api_url))
            .map_err(B2Error::from)
    }
    fn headers(&self) -> Result<HeaderMap, B2Error> {
        let mut map = HeaderMap::new();
        map.append("Authorization", self.auth.auth_token());
        Ok(map)
    }
    fn body(&self) -> Result<Body, B2Error> {
        serde_body(&UpdateBucketRequest {
            account_id: &self.auth.account_id,
            bucket_id: &self.bucket_id,
            bucket_type: self.bucket_type,
            bucket_info: self.bucket_info.as_ref(),
            cors_rules: self.cors_rules,
            lifecycle_rules: self.lifecycle_rules,
            if_revision_is: self.if_revision_is,
        })
    }
    fn finalize(self, fut: ResponseFuture) -> B2Future<Bucket> {
        B2Future::new(fut)
    }
    fn error(self, err: B2Error) -> B2Future<Bucket> {
        B2Future::err(err)
    }
}

