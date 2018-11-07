//! Buckets from backblaze.

use std::collections::HashMap;

use hyper::{Client, Request};
use serde_json::to_vec;

use hyper::body::Body;
use hyper::client::connect::Connect;

use crate::api::authorize::B2Authorization;
use crate::b2_future::{B2Future, B2Stream};
use crate::BytesString;

use serde::ser::Serialize;

mod bucket_type;
pub use self::bucket_type::BucketType;

/// This struct contains a lifecycle rule as specified in the [backblaze b2
/// documentation][1].
///
/// [1]: https://www.backblaze.com/b2/docs/lifecycle_rules.html
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleRule {
    pub days_from_uploading_to_hiding: Option<u32>,
    pub days_from_hiding_to_deleting: Option<u32>,
    pub file_name_prefix: String,
}

/// This struct contains a cors rule as specified in the [backblaze b2
/// documentation][1].
///
/// [1]: https://www.backblaze.com/b2/docs/cors_rules.html
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CorsRule {
    pub cors_rule_name: String,
    pub allowed_origins: Vec<String>,
    pub allowed_operations: Vec<String>,
    pub allowed_headers: Vec<String>,
    pub expose_headers: Vec<String>,
    pub max_age_seconds: u32,
}

/// This function contains various information about a backblaze bucket.
///
/// The `Eq` implementation considers two bucket objects equal, even if their revision
/// number is different.
#[derive(Serialize, Deserialize, Debug, Clone, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Bucket {
    pub account_id: String,
    pub bucket_id: String,
    pub bucket_name: String,
    pub bucket_type: BucketType,
    pub bucket_info: HashMap<String, String>,
    pub lifecycle_rules: Vec<LifecycleRule>,
    pub cors_rules: Vec<CorsRule>,
    pub revision: u32,
}
impl PartialEq<Bucket> for Bucket {
    fn eq(&self, other: &Bucket) -> bool {
        self.account_id == other.account_id
            && self.bucket_id == other.bucket_id
            && self.bucket_name == other.bucket_name
            && self.bucket_type == other.bucket_type
            && self.bucket_info == other.bucket_info
            && self.lifecycle_rules == other.lifecycle_rules
            && self.cors_rules == other.cors_rules
    }
}

/// Used for creating buckets without any info.
pub struct NoBucketInfo;
impl Serialize for NoBucketInfo {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        let map: HashMap<&str, &str> = HashMap::new();
        Serialize::serialize(&map, serializer)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ListBucketsRequest<'a> {
    account_id: &'a BytesString,
    #[serde(skip_serializing_if = "Option::is_none")]
    bucket_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bucket_name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bucket_types: Option<&'a [BucketType]>,
}

/// List the buckets on backblaze. This requires the `listBuckets` capability.
///
/// If `bucket_id` or `bucket_name` is specified, this call will return just that specific
/// bucket.  If `bucket_types` is specified, the list will only contain buckets with those
/// types.
///
/// This is done using the [b2_list_buckets][1] api call.
///
/// [1]: https://www.backblaze.com/b2/docs/b2_list_buckets.html
pub fn list_buckets<C>(
    auth: &B2Authorization,
    client: &Client<C, Body>,
    bucket_id: Option<&str>,
    bucket_name: Option<&str>,
    bucket_types: Option<&[BucketType]>,
) -> B2Stream<Bucket>
where
    C: Connect + Sync + 'static,
    C::Transport: 'static,
    C::Future: 'static,
{
    let url_string: String = format!("{}/b2api/v2/b2_list_buckets", auth.api_url);
    let mut request = Request::post(url_string);
    request.header("Authorization", auth.auth_token());

    let body = match to_vec(&ListBucketsRequest {
        account_id: &auth.account_id,
        bucket_id,
        bucket_name,
        bucket_types,
    }) {
        Ok(body) => body,
        Err(err) => return B2Stream::err(err),
    };
    let body = Body::from(body);

    let request = match request.body(body) {
        Ok(req) => req,
        Err(err) => return B2Stream::err(err),
    };

    let future = client.request(request);

    B2Stream::new(future, 64, 2)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateBucketRequest<'a, T: 'a> {
    account_id: &'a BytesString,
    bucket_name: &'a str,
    bucket_type: BucketType,
    bucket_info: &'a T,
    cors_rules: &'a [CorsRule],
    lifecycle_rules: &'a [LifecycleRule],
}
/// Create a new bucket on backblaze. This requires the `writeBuckets` capability.
///
/// This is done using the [b2_create_bucket][1] api call. The `bucket_info` should be
/// some type that serializes into a json object that follows the spec specified on the
/// [backblaze documentation][2].
///
/// [1]: https://www.backblaze.com/b2/docs/b2_create_bucket.html
/// [2]: https://www.backblaze.com/b2/docs/buckets.html#bucketInfo
pub fn create_bucket<C, Info>(
    auth: &B2Authorization,
    client: &Client<C, Body>,
    bucket_name: &str,
    bucket_type: BucketType,
    bucket_info: &Info,
    cors_rules: &[CorsRule],
    lifecycle_rules: &[LifecycleRule],
) -> B2Future<Bucket>
where
    C: Connect + Sync + 'static,
    C::Transport: 'static,
    C::Future: 'static,
    Info: Serialize,
{
    let url_string: String = format!("{}/b2api/v2/b2_create_bucket", auth.api_url);
    let mut request = Request::post(url_string);
    request.header("Authorization", auth.auth_token());

    let body = match to_vec(&CreateBucketRequest {
        account_id: &auth.account_id,
        bucket_name,
        bucket_type,
        bucket_info,
        cors_rules,
        lifecycle_rules,
    }) {
        Ok(body) => body,
        Err(err) => return B2Future::err(err),
    };
    let body = Body::from(body);

    let request = match request.body(body) {
        Ok(req) => req,
        Err(err) => return B2Future::err(err),
    };

    let future = client.request(request);

    B2Future::new(future)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DeleteBucketRequest<'a> {
    account_id: &'a BytesString,
    bucket_id: &'a str,
}
/// Delete a bucket on backblaze. This requires the `deleteBuckets` capability.
///
/// The future resolves to the deleted bucket.
///
/// This is done using the [b2_delete_bucket][1] api call.
///
/// [1]: https://www.backblaze.com/b2/docs/b2_delete_bucket.html
pub fn delete_bucket<C>(
    auth: &B2Authorization,
    client: &Client<C, Body>,
    bucket_id: &str,
) -> B2Future<Bucket>
where
    C: Connect + Sync + 'static,
    C::Transport: 'static,
    C::Future: 'static,
{
    let url_string: String = format!("{}/b2api/v2/b2_delete_bucket", auth.api_url);
    let mut request = Request::post(url_string);
    request.header("Authorization", auth.auth_token());

    let body = match to_vec(&DeleteBucketRequest {
        account_id: &auth.account_id,
        bucket_id,
    }) {
        Ok(body) => body,
        Err(err) => return B2Future::err(err),
    };
    let body = Body::from(body);

    let request = match request.body(body) {
        Ok(req) => req,
        Err(err) => return B2Future::err(err),
    };

    let future = client.request(request);

    B2Future::new(future)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateBucketRequest<'a, T: 'a> {
    account_id: &'a BytesString,
    bucket_id: &'a str,

    #[serde(skip_serializing_if = "Option::is_none")]
    bucket_type: Option<BucketType>,

    #[serde(skip_serializing_if = "Option::is_none")]
    bucket_info: Option<&'a T>,

    #[serde(skip_serializing_if = "Option::is_none")]
    cors_rules: Option<&'a [CorsRule]>,

    #[serde(skip_serializing_if = "Option::is_none")]
    lifecycle_rules: Option<&'a [LifecycleRule]>,

    #[serde(skip_serializing_if = "Option::is_none")]
    if_revision_is: Option<u32>,
}
/// Update a bucket on backblaze. This requires the `writeBuckets` capability.
///
/// The future resolves to the updated bucket. Settings specified as `None` are left
/// unchanged. The `bucket_info` should be some type that serializes into a json object
/// that follows the spec specified on the [backblaze documentation][2].
///
/// This is done using the [b2_update_bucket][1] api call.
///
/// [1]: https://www.backblaze.com/b2/docs/b2_update_bucket.html
/// [2]: https://www.backblaze.com/b2/docs/buckets.html#bucketInfo
pub fn update_bucket<C, Info>(
    auth: &B2Authorization,
    client: &Client<C, Body>,
    bucket_id: &str,
    bucket_type: Option<BucketType>,
    bucket_info: Option<&Info>,
    cors_rules: Option<&[CorsRule]>,
    lifecycle_rules: Option<&[LifecycleRule]>,
    if_revision_is: Option<u32>,
) -> B2Future<Bucket>
where
    C: Connect + Sync + 'static,
    C::Transport: 'static,
    C::Future: 'static,
    Info: Serialize,
{
    let url_string: String = format!("{}/b2api/v2/b2_update_bucket", auth.api_url);
    let mut request = Request::post(url_string);
    request.header("Authorization", auth.auth_token());

    let body = match to_vec(&UpdateBucketRequest {
        account_id: &auth.account_id,
        bucket_id,
        bucket_type,
        bucket_info,
        cors_rules,
        lifecycle_rules,
        if_revision_is,
    }) {
        Ok(body) => body,
        Err(err) => return B2Future::err(err),
    };
    let body = Body::from(body);

    let request = match request.body(body) {
        Ok(req) => req,
        Err(err) => return B2Future::err(err),
    };

    let future = client.request(request);

    B2Future::new(future)
}
