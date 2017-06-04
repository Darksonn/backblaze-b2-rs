use std::fmt;

use hyper::{self, Client};
use hyper::client::Body;

use serde::{Serialize, Deserialize};
use serde::ser::Serializer;
use serde::de::{self, Visitor, Deserializer};
use serde_json::{self, Value as JsonValue};

use B2Error;
use raw::authorize::B2Authorization;

/// Specifies the type of a bucket on backblaze
#[derive(Debug,Clone,Copy,Eq,PartialEq)]
pub enum BucketType {
    Public, Private, Snapshot
}
impl BucketType {
    /// Creates a BucketType from a string. The strings are the ones used by the backblaze api
    ///
    /// ```rust
    ///use backblaze_b2::raw::buckets::BucketType;
    ///
    ///assert_eq!(BucketType::from_str("allPublic"), Some(BucketType::Public));
    ///assert_eq!(BucketType::from_str("allPrivate"), Some(BucketType::Private));
    ///assert_eq!(BucketType::from_str("snapshot"), Some(BucketType::Snapshot));
    /// ```
    pub fn from_str(s: &str) -> Option<BucketType> {
        match s {
            "allPublic" => Some(BucketType::Public),
            "allPrivate" => Some(BucketType::Private),
            "snapshot" => Some(BucketType::Snapshot),
            _ => None
        }
    }
    /// This function returns the string needed to specify the bucket type to the backblaze api
    pub fn as_str(&self) -> &'static str {
        match *self {
            BucketType::Public => "allPublic",
            BucketType::Private => "allPrivate",
            BucketType::Snapshot => "snapshot"
        }
    }
}
static BUCKET_TYPES: [&str; 3] = ["allPublic", "allPrivate", "snapshot"];
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
    fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E> where E: de::Error {
        match BucketType::from_str(v) {
            None => Err(de::Error::unknown_variant(v, &BUCKET_TYPES)),
            Some(v) => Ok(v)
        }
    }
    fn visit_string<E>(self, v: String) -> Result<Self::Value, E> where E: de::Error {
        match BucketType::from_str(&v) {
            None => Err(de::Error::unknown_variant(&v, &BUCKET_TYPES)),
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

/// This struct contains a lifecycle rule as specified in the [backblaze b2
/// documentation](https://www.backblaze.com/b2/docs/lifecycle_rules.html).
#[derive(Serialize,Deserialize,Debug,Clone)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleRule {
    days_from_uploading_to_hiding: Option<u32>,
    days_from_hiding_to_deleting: Option<u32>,
    file_name_prefix: String
}

/// This function contains various information about a backblaze bucket
#[derive(Serialize,Deserialize,Debug,Clone)]
#[serde(rename_all = "camelCase")]
pub struct Bucket<InfoType=JsonValue> {
    pub account_id: String,
    pub bucket_id: String,
    pub bucket_name: String,
    pub bucket_type: BucketType,
    pub bucket_info: InfoType,
    pub lifecycle_rules: Vec<LifecycleRule>,
    pub revision: u32
}

#[derive(Deserialize)]
struct ListBucketsResponse<InfoType> {
    buckets: Vec<Bucket<InfoType>>
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateBucketRequest<'a, InfoType> {
    account_id: &'a str,
    bucket_name: &'a str,
    bucket_type: BucketType,
    bucket_info: InfoType,
    lifecycle_rules: Vec<LifecycleRule>
}
impl<'a> B2Authorization<'a> {
    /// Performs a [b2_list_buckets][1] api call.
    ///
    ///  [1]: https://www.backblaze.com/b2/docs/b2_list_buckets.html
    pub fn list_buckets<InfoType>(&self, client: &Client)
        -> Result<Vec<Bucket<InfoType>>,B2Error>
        where for<'de> InfoType: Deserialize<'de>
    {
        let url_string: String = format!("{}/b2api/v1/b2_list_buckets?accountId={}",
                                               self.api_url, self.credentials.id);
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
    ///  [1]: https://www.backblaze.com/b2/docs/b2_create_bucket.html
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
            account_id: &self.credentials.id,
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
    ///  [1]: https://www.backblaze.com/b2/docs/b2_create_bucket.html
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
    ///  [1]: https://www.backblaze.com/b2/docs/b2_delete_bucket.html
    pub fn delete_bucket_id<InfoType>(&self, bucket_id: &str, client: &Client)
        -> Result<Bucket<InfoType>, B2Error>
        where for <'de> InfoType: Deserialize<'de>
    {
        let url_string: String = format!("{}/b2api/v1/b2_delete_bucket", self.api_url);
        let url: &str = &url_string;

        let body: String =
            format!("{{\"accountId\":\"{}\", \"bucketId\":\"{}\"}}", self.credentials.id, bucket_id);

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
    ///  [1]: https://www.backblaze.com/b2/docs/b2_delete_bucket.html
    pub fn delete_bucket<InfoType>(&self, bucket: &Bucket<InfoType>, client: &Client)
        -> Result<Bucket<InfoType>, B2Error>
        where for <'de> InfoType: Deserialize<'de>
    {
        self.delete_bucket_id(&bucket.bucket_id, client)
    }

}



