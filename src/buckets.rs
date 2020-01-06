//! Bucket manipulation.
//!
//! The official documentation on buckets can be found [here][1]. This module
//! defines five api calls:
//!
//! 1. [`CreateBucket`]
//! 2. [`DeleteBucket`]
//! 5. [`UpdateBucket`]
//! 3. [`GetBucket`]
//! 4. [`ListBuckets`]
//!
//! See the documentation for each api call for examples on how to use them.
//!
//! [1]: https://www.backblaze.com/b2/docs/buckets.html
//! [`CreateBucket`]: struct.CreateBucket.html
//! [`DeleteBucket`]: struct.DeleteBucket.html
//! [`UpdateBucket`]: struct.UpdateBucket.html
//! [`GetBucket`]: struct.GetBucket.html
//! [`ListBuckets`]: struct.ListBuckets.html

use serde::{Serialize, Deserialize};
use std::collections::HashMap;

mod bucket_type;
mod create_bucket;
mod delete_bucket;
mod list_buckets;
mod get_bucket;
mod update_bucket;
pub use self::bucket_type::BucketType;
pub use self::create_bucket::CreateBucket;
pub use self::delete_bucket::DeleteBucket;
pub use self::list_buckets::{ListBuckets, ListBucketsFuture};
pub use self::get_bucket::{GetBucket, GetBucketFuture};
pub use self::update_bucket::UpdateBucket;

/// This struct contains a lifecycle rule as specified in the [backblaze b2
/// documentation][1].
///
/// [1]: https://www.backblaze.com/b2/docs/lifecycle_rules.html
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
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
#[non_exhaustive]
pub struct CorsRule {
    /// A name for humans to recognize the rule in a user interface. Names must
    /// be unique within a bucket. Names can consist of upper-case and
    /// lower-case English letters, numbers, and `-`. No other characters are
    /// allowed. A name must be at least 6 characters long, and can be at most
    /// 50 characters long. These are all allowed names: `myPhotosSite`,
    /// `allowAnyHttps`, `backblaze-images`. Names that start with `b2-` are
    /// reserved for Backblaze use.
    pub cors_rule_name: String,
    /// A non-empty list specifying which origins the rule covers. Each value
    /// may have one of many formats:
    ///
    ///  * The origin can be fully specified, such as
    ///    `http://www.example.com:8180` or `https://www.example.com:4433`.
    ///  * The origin can omit a default port, such as `https://www.example.com`.
    ///  * The origin may have a single `*` as part of the domain name, such as
    ///    `https://*.example.com`, `https://*:8443` or `https://*`.
    ///  * The origin may be `https` to match any origin that uses HTTPS. (This
    ///    is broader than `https://*` because it matches any port.)
    ///  * Finally, the origin can be a single `*` to match any origin.
    ///
    /// If any entry is `*`, it must be the only entry. There can be at most one
    /// `https` entry and no entry after it may start with `https:`.
    pub allowed_origins: Vec<String>,
    /// A list specifying which operations the rule allows. At least one value
    /// must be specified. All values must be from the following list. More
    /// values may be added to this list at any time.
    ///
    ///  * `b2_download_file_by_name`
    ///  * `b2_download_file_by_id`
    ///  * `b2_upload_file`
    ///  * `b2_upload_part`
    pub allowed_operations: Vec<String>,
    /// If present, this is a list of headers that are allowed in a pre-flight
    /// OPTIONS's request's Access-Control-Request-Headers header value. Each
    /// value may have one of many formats:
    ///
    ///  * It may be a complete header name, such as x-bz-content-sha1.
    ///  * It may end with an asterisk, such as x-bz-info-*.
    ///  * Finally, it may be a single `*` to match any header.
    ///
    /// If any entry is `*`, it must be the only entry in the list.
    pub allowed_headers: Vec<String>,
    /// If present, this is a list of headers that may be exposed to an
    /// application inside the client (eg. exposed to Javascript in a browser).
    /// Each entry in the list must be a complete header name (eg.
    /// `x-bz-content-sha1`). If this list is empty, no headers will be exposed.
    pub expose_headers: Vec<String>,
    /// This specifies the maximum number of seconds that a browser may cache
    /// the response to a preflight request. The value must not be negative and
    /// it must not be more than 86,400 seconds (one day).
    pub max_age_seconds: u32,
}

/// This function contains various information about a backblaze bucket.
///
/// The `Eq` implementation considers two bucket objects equal, even if their revision
/// number is different.
///
/// See [the official documentation][1] for more information.
///
/// [1]: https://www.backblaze.com/b2/docs/buckets.html
#[derive(Serialize, Deserialize, Debug, Clone, Eq)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
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
    /// Compare two buckets, ignoring the revision.
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
///
/// This type can be used together with the [`CreateBucket`] and [`UpdateBucket`]
/// api calls.
///
/// [`CreateBucket`]: struct.CreateBucket.html
/// [`UpdateBucket`]: struct.UpdateBucket.html
pub struct NoBucketInfo;
impl Serialize for NoBucketInfo {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        // This does not allocate as the map is empty.
        let map: HashMap<&str, &str> = HashMap::new();
        Serialize::serialize(&map, serializer)
    }
}
