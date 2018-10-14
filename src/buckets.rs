//! This module defines various methods for interacting with buckets on backblaze.

use std::fmt;
use std::mem;

use base64::{encode as b64encode};
use hyper::{Client, Request, Error as HyperError, StatusCode};
use futures::{Poll, Future, Async, Stream};
use serde_json::to_vec;

use hyper::body::Body;
use hyper::client::connect::Connect;

use B2Error;
use capabilities::Capabilities;
use b2_future::B2Future;
use authorize::B2Authorization;

use serde::de::{self, Visitor, Deserialize, Deserializer, Error, Unexpected, Expected};
use serde::ser::{Serialize, Serializer, SerializeSeq};

/// Specifies the type of a bucket on backblaze.
#[derive(Debug,Clone,Copy,Eq,PartialEq)]
pub enum BucketType {
    Public, Private, Snapshot
}
impl BucketType {
    /// Creates a BucketType from a string. The strings are the ones used by the backblaze
    /// api.
    ///
    /// ```
    /// use backblaze_b2::buckets::BucketType;
    ///
    /// assert_eq!(BucketType::from_str("allPublic"), Some(BucketType::Public));
    /// assert_eq!(BucketType::from_str("allPrivate"), Some(BucketType::Private));
    /// assert_eq!(BucketType::from_str("snapshot"), Some(BucketType::Snapshot));
    /// assert_eq!(BucketType::from_str("invalid"), None);
    /// ```
    pub fn from_str(s: &str) -> Option<BucketType> {
        match s {
            "allPublic" => Some(BucketType::Public),
            "allPrivate" => Some(BucketType::Private),
            "snapshot" => Some(BucketType::Snapshot),
            _ => None
        }
    }
    /// This function returns the string needed to specify the bucket type to the
    /// backblaze api.
    pub fn as_str(self) -> &'static str {
        match self {
            BucketType::Public => "allPublic",
            BucketType::Private => "allPrivate",
            BucketType::Snapshot => "snapshot"
        }
    }
}
static BUCKET_TYPES: [&'static str; 3] = ["allPublic", "allPrivate", "snapshot"];
struct BucketTypeVisitor;
impl<'de> Visitor<'de> for BucketTypeVisitor {
    type Value = BucketType;
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("allPublic, allPrivate or snapshot")
    }
    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E> where E: de::Error {
        match BucketType::from_str(v) {
            None => Err(de::Error::unknown_variant(v, &BUCKET_TYPES)),
            Some(v) => Ok(v)
        }
    }
}
impl<'de> Deserialize<'de> for BucketType {
    fn deserialize<D>(deserializer: D) -> Result<BucketType, D::Error>
        where D: Deserializer<'de>
    {
        deserializer.deserialize_str(BucketTypeVisitor)
    }
}
impl Serialize for BucketType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        serializer.serialize_str(self.as_str())
    }
}

type BucketInfo = ::std::collections::HashMap<String, String>;

/// This struct contains a lifecycle rule as specified in the [backblaze b2
/// documentation][1].
///
/// [1]: https://www.backblaze.com/b2/docs/lifecycle_rules.html
#[derive(Serialize,Deserialize,Debug,Clone)]
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
#[derive(Serialize,Deserialize,Debug,Clone)]
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
#[derive(Serialize,Deserialize,Debug,Clone)]
#[serde(rename_all = "camelCase")]
pub struct Bucket {
    pub account_id: String,
    pub bucket_id: String,
    pub bucket_name: String,
    pub bucket_type: BucketType,
    pub bucket_info: BucketInfo,
    pub lifecycle_rules: Vec<LifecycleRule>,
    pub cors_rules: Vec<CorsRule>,
    pub revision: u32,
}



/// A future resolving to a `Vec` of [`Bucket`].
///
/// [`Bucket`]: struct.Bucket.html
pub struct ListBucketsFuture(B2Future<ListBucketsResponse>);
impl Future for ListBucketsFuture {
    type Item = Vec<Bucket>;
    type Error = B2Error;
    fn poll(&mut self) -> Poll<Vec<Bucket>, B2Error> {
        match self.0.poll() {
            Ok(Async::Ready(res)) => {
                Ok(Async::Ready(res.buckets))
            },
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(err) => Err(err),
        }
    }
}

#[derive(Deserialize)]
struct ListBucketsResponse {
    buckets: Vec<Bucket>
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ListBucketsRequest<'a> {
    account_id: &'a str,
    bucket_id: Option<&'a str>,
    bucket_name: Option<&'a str>,
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
) -> ListBucketsFuture
where
    C: Connect + Sync + 'static,
    C::Transport: 'static,
    C::Future: 'static,
{
    let url_string: String = format!("{}/b2api/v1/b2_list_buckets", auth.api_url);
    let mut request = Request::post(url_string);
    request.header("Authorization", &auth.authorization_token[..]);

    let body = match to_vec(&ListBucketsRequest {
        account_id: &auth.account_id,
        bucket_id,
        bucket_name,
        bucket_types,
    }) {
        Ok(body) => body,
        Err(err) => return ListBucketsFuture(B2Future::err(err)),
    };
    let body = Body::from(body);

    let request = match request.body(body) {
        Ok(req) => req,
        Err(err) => return ListBucketsFuture(B2Future::err(err)),
    };

    let future = client.request(request);

    ListBucketsFuture(B2Future::new(future, 64))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateBucketRequest<'a> {
    account_id: &'a str,
    bucket_name: &'a str,
    bucket_type: BucketType,
    bucket_info: BucketInfo,
    cors_rules: &'a [CorsRule],
    lifecycle_rules: &'a [LifecycleRule],
}
/// Create a new bucket on backblaze. This requires the `writeBuckets` capability.
///
/// This is done using the [b2_create_bucket][1] api call.
///
/// [1]: https://www.backblaze.com/b2/docs/b2_create_bucket.html
pub fn create_bucket<C>(
    auth: &B2Authorization,
    client: &Client<C, Body>,
    bucket_name: &str,
    bucket_type: BucketType,
    bucket_info: BucketInfo,
    cors_rules: &[CorsRule],
    lifecycle_rules: &[LifecycleRule],
) -> B2Future<Bucket>
where
    C: Connect + Sync + 'static,
    C::Transport: 'static,
    C::Future: 'static,
{
    let url_string: String = format!("{}/b2api/v1/b2_create_bucket", auth.api_url);
    let mut request = Request::post(url_string);
    request.header("Authorization", &auth.authorization_token[..]);

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

    B2Future::new(future, 64)
}

/*
impl B2Authorization {
    /// Performs a [b2_list_buckets][1] api call.
    ///
    /// # Errors
    /// This function returns a [`B2Error`] in case something goes wrong. This function is only
    /// going to fail with the standard errors.
    ///
    ///  [1]: https://www.backblaze.com/b2/docs/b2_list_buckets.html
    ///  [`B2Error`]: ../../enum.B2Error.html
    pub fn list_buckets<InfoType>(&self, client: &Client)
        -> Result<Vec<Bucket<InfoType>>,B2Error>
        where for<'de> InfoType: Deserialize<'de>
    {
        let url_string: String = format!("{}/b2api/v1/b2_list_buckets?accountId={}",
                                               self.api_url, self.account_id);
        let url: &str = &url_string;
        let resp = try!(client.get(url)
            .header(self.auth_header())
            .send());
        if resp.status != hyper::status::StatusCode::Ok {
            Err(B2Error::from_response(resp))
        } else {
            let buckets: ListBucketsResponse<InfoType> = try!(serde_json::from_reader(resp));
            Ok(buckets.buckets)
        }
    }
    /// Performs a [b2_create_bucket][1] api call.
    ///
    /// # Errors
    /// This function returns a [`B2Error`] in case something goes wrong. Besides the standard
    /// errors, this function can fail with [`is_maximum_bucket_limit`],
    /// [`is_duplicate_bucket_name`] and [`is_invalid_bucket_name`].
    ///
    ///  [1]: https://www.backblaze.com/b2/docs/b2_create_bucket.html
    ///  [`B2Error`]: ../../enum.B2Error.html
    ///  [`is_maximum_bucket_limit`]: ../../enum.B2Error.html#method.is_maximum_bucket_limit
    ///  [`is_duplicate_bucket_name`]: ../../enum.B2Error.html#method.is_duplicate_bucket_name
    ///  [`is_invalid_bucket_name`]: ../../enum.B2Error.html#method.is_invalid_bucket_name
    pub fn create_bucket<InfoType>(&self,
                                   bucket_name: &str,
                                   bucket_type: BucketType,
                                   bucket_info: InfoType,
                                   lifecycle_rules: Vec<LifecycleRule>,
                                   client: &Client)
        -> Result<Bucket<InfoType>, B2Error>
        where for <'de> InfoType: Serialize + Deserialize<'de>
    {
        let url_string: String = format!("{}/b2api/v1/b2_create_bucket", self.api_url);
        let url: &str = &url_string;

        let body = try!(serde_json::to_string(&CreateBucketRequest {
            account_id: &self.account_id,
            bucket_name: bucket_name,
            bucket_type: bucket_type,
            bucket_info: bucket_info,
            lifecycle_rules: lifecycle_rules
        }));

        let resp = try!(client.post(url)
            .body(Body::BufBody(body.as_bytes(), body.len()))
            .header(self.auth_header())
            .send());
        if resp.status != hyper::status::StatusCode::Ok {
            Err(B2Error::from_response(resp))
        } else {
            let bucket: Bucket<InfoType> = try!(serde_json::from_reader(resp));
            Ok(bucket)
        }
    }
    /// Performs a [b2_create_bucket][1] api call. This function initializes the bucket with no
    /// info.
    ///
    /// # Errors
    /// This function returns a [`B2Error`] in case something goes wrong. Besides the standard
    /// errors, this function can fail with [`is_maximum_bucket_limit`],
    /// [`is_duplicate_bucket_name`] and [`is_invalid_bucket_name`].
    ///
    ///  [1]: https://www.backblaze.com/b2/docs/b2_create_bucket.html
    ///  [`B2Error`]: ../../enum.B2Error.html
    ///  [`is_maximum_bucket_limit`]: ../../enum.B2Error.html#method.is_maximum_bucket_limit
    ///  [`is_duplicate_bucket_name`]: ../../enum.B2Error.html#method.is_duplicate_bucket_name
    ///  [`is_invalid_bucket_name`]: ../../enum.B2Error.html#method.is_invalid_bucket_name
    pub fn create_bucket_no_info(&self,
                                   bucket_name: &str,
                                   bucket_type: BucketType,
                                   lifecycle_rules: Vec<LifecycleRule>,
                                   client: &Client)
        -> Result<Bucket<JsonValue>, B2Error>
    {
        self.create_bucket(bucket_name, bucket_type, JsonValue::Object(serde_json::map::Map::new()),
            lifecycle_rules, client)
    }
    /// Performs a [b2_delete_bucket][1] api call.
    ///
    /// # Errors
    /// This function returns a [`B2Error`] in case something goes wrong. Besides the standard
    /// errors, this function can fail with [`is_bucket_not_found`].
    ///
    ///  [1]: https://www.backblaze.com/b2/docs/b2_delete_bucket.html
    ///  [`B2Error`]: ../../enum.B2Error.html
    ///  [`is_bucket_not_found`]: ../../enum.B2Error.html#method.is_bucket_not_found
    pub fn delete_bucket_id<InfoType>(&self, bucket_id: &str, client: &Client)
        -> Result<Bucket<InfoType>, B2Error>
        where for <'de> InfoType: Deserialize<'de>
    {
        let url_string: String = format!("{}/b2api/v1/b2_delete_bucket", self.api_url);
        let url: &str = &url_string;

        let body: String =
            format!("{{\"accountId\":\"{}\", \"bucketId\":\"{}\"}}", self.account_id, bucket_id);

        let resp = try!(client.post(url)
            .body(Body::BufBody(body.as_bytes(), body.len()))
            .header(self.auth_header())
            .send());
        if resp.status != hyper::status::StatusCode::Ok {
            Err(B2Error::from_response(resp))
        } else {
            let bucket: Bucket<InfoType> = try!(serde_json::from_reader(resp));
            Ok(bucket)
        }
    }
    /// Performs a [b2_delete_bucket][1] api call.
    ///
    /// # Errors
    /// This function returns a [`B2Error`] in case something goes wrong. Besides the standard
    /// errors, this function can fail with [`is_bucket_not_found`].
    ///
    ///  [1]: https://www.backblaze.com/b2/docs/b2_delete_bucket.html
    ///  [`B2Error`]: ../../enum.B2Error.html
    ///  [`is_bucket_not_found`]: ../../enum.B2Error.html#method.is_bucket_not_found
    pub fn delete_bucket<InfoType>(&self, bucket: &Bucket<InfoType>, client: &Client)
        -> Result<Bucket<InfoType>, B2Error>
        where for <'de> InfoType: Deserialize<'de>
    {
        self.delete_bucket_id(&bucket.bucket_id, client)
    }

}
*/



