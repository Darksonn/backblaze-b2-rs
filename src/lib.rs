//! A library for interacting with the [backblaze b2 api][1].
//!
//! # Backwards compatibility
//!
//! Since the b2 api does not consider adding fields to any response type a breaking
//! change, neither does this library meaning that most types are marked non
//! exhaustive.
//!
//! That said, this library does provide one guarantee regarding new fields: adding
//! new fields without a default value on deserialization is considered breaking.
//! This means that serializing a value and deserializing it later will never fail
//! unless a breaking change has been introduced since it was serialized.
//!
//! [1]: https://www.backblaze.com/b2/docs/

#![warn(rust_2018_idioms)]

use hyper::StatusCode;
use serde::Deserialize;
use std::fmt;

// pub mod api;
// pub mod source;
pub mod auth;
pub mod b2_future;
pub mod buckets;
mod bytes_string;
pub mod client;
pub mod files;
// pub mod prelude;
// pub mod stream_util;
// pub mod throttle;
pub use bytes_string::BytesString;

/// Parse the content length header.
fn get_content_length(parts: &http::response::Parts) -> usize {
    parts
        .headers
        .get(http::header::CONTENT_LENGTH)
        .and_then(|size_str| size_str.to_str().ok())
        .and_then(|size_str| size_str.parse().ok())
        .unwrap_or(0)
}

mod header_serde {
    use crate::BytesString;
    use http::header::HeaderValue;
    use serde::de::Deserializer;
    use serde::ser::Serializer;
    use serde::{Deserialize, Serialize};

    pub fn serialize<S>(header: &HeaderValue, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let v = header
            .to_str()
            .map_err(<S::Error as serde::ser::Error>::custom)?;
        v.serialize(s)
    }

    pub fn deserialize<'de, D>(d: D) -> Result<HeaderValue, D::Error>
    where
        D: Deserializer<'de>,
    {
        BytesString::deserialize(d)?
            .as_header()
            .map_err(<D::Error as serde::de::Error>::custom)
    }
}

/// The b2 api returns errors in a json-object, that can be deserialized into this struct.
///
/// This struct is usually contained in a [`B2Error`].
///
///  [`B2Error`]: enum.B2Error.html
#[derive(Deserialize, Debug, Clone)]
pub struct B2ErrorMessage {
    pub code: String,
    pub message: String,
    pub status: u32,
}

/// An error caused while using any of the B2 apis. Errors returned by the b2 api are
/// stored exactly as received from backblaze and for ease of use several methods are
/// provided on this type in order to check the kind of error.
///
/// The following methods are relevant for any backblaze api call:
/// [`is_service_unavilable`], [`is_too_many_requests`], [`should_back_off`].
///
/// The following methods are relevant for any backblaze api call beside authentication:
/// [`is_expired_authentication`], [`is_authorization_issue`],
/// [`should_obtain_new_authentication`].
///
/// Since these errors are so common, they are not mentioned directly in the documentation
/// for the api-call. Also take care with snapshot buckets, they might cause the error
/// [`is_snapshot_interaction_failure`], but the B2 documentation is inconsistent
/// regarding when this error can be returned.
///
///  [`is_service_unavilable`]: #method.is_service_unavilable
///  [`is_too_many_requests`]: #method.is_too_many_requests
///  [`should_obtain_new_authentication`]: #method.should_obtain_new_authentication
///  [`should_back_off`]: #method.should_back_off
///  [`is_expired_authentication`]: #method.is_expired_authentication
///  [`is_authorization_issue`]: #method.is_authorization_issue
///  [`is_snapshot_interaction_failure`]: #method.is_snapshot_interaction_failure
#[derive(Debug)]
#[non_exhaustive]
pub enum B2Error {
    HyperError(hyper::error::Error),
    HttpError(http::Error),
    IOError(std::io::Error),
    JsonError(serde_json::Error),
    /// When the b2 website returns an error, it is stored in this variant.
    B2Error(StatusCode, B2ErrorMessage),
    /// This type is only returned if the b2 website is not following the api spec.
    ApiInconsistency(String),
}
impl B2Error {
    /// Turn this error into an io error.
    pub fn into_io_error(self) -> std::io::Error {
        std::io::Error::new(self.get_io_kind(), self)
    }
}
/// Load errors
impl B2Error {
    pub(crate) fn api(s: &str) -> B2Error {
        B2Error::ApiInconsistency(String::from(s))
    }
    /// Returns true if the B2 server returned any status code in the 5xx range. According
    /// to the B2 specification, one should obtain new authentication in this case, so the
    /// method [`should_obtain_new_authentication`] always returns true if this method
    /// returns true.
    ///
    ///  [`should_obtain_new_authentication`]: #method.should_obtain_new_authentication
    pub fn is_service_unavilable(&self) -> bool {
        match self {
            B2Error::B2Error(_, B2ErrorMessage { status, .. }) => {
                *status >= 500 && *status <= 599
            }
            _ => false,
        }
    }
    /// Returns true if we are making too many requests.
    pub fn is_too_many_requests(&self) -> bool {
        match self {
            B2Error::B2Error(_, B2ErrorMessage { status, .. }) => *status == 429,
            _ => false,
        }
    }
    fn get_io_kind(&self) -> std::io::ErrorKind {
        use std::error::Error;
        use std::io::ErrorKind;
        match self {
            B2Error::HyperError(ref err) => err
                .source()
                .and_then(|err| err.downcast_ref::<std::io::Error>())
                .map(|err| err.kind())
                .unwrap_or(ErrorKind::Other),
            B2Error::HttpError(_) => ErrorKind::InvalidData,
            B2Error::IOError(ref ioe) => ioe.kind(),
            B2Error::JsonError(ref err) => err
                .source()
                .and_then(|err| err.downcast_ref::<std::io::Error>())
                .map(|err| err.kind())
                .unwrap_or(ErrorKind::InvalidData),
            B2Error::B2Error(_, _) => ErrorKind::Other,
            B2Error::ApiInconsistency(_) => ErrorKind::InvalidData,
        }
    }
    /// Returns true if any of the situtations described on the [B2 documentation][1] has
    /// occurred.  When this function returns true, you should obtain a new
    /// [`B2Authorization`].
    ///
    ///  [1]: https://www.backblaze.com/b2/docs/uploading.html
    ///  [`B2Authorization`]: authorize/struct.B2Authorization.html
    pub fn should_obtain_new_authentication(&self) -> bool {
        match self.get_io_kind() {
            std::io::ErrorKind::BrokenPipe => true,
            std::io::ErrorKind::ConnectionRefused => true,
            std::io::ErrorKind::ConnectionReset => true,
            std::io::ErrorKind::ConnectionAborted => true,
            std::io::ErrorKind::NotConnected => true,
            std::io::ErrorKind::TimedOut => true,
            _ => self.is_authorization_issue() || self.is_service_unavilable(),
        }
    }
    /// Returns true if you should be using some sort of exponential back off for future
    /// requests.
    pub fn should_back_off(&self) -> bool {
        if let B2Error::B2Error(_, B2ErrorMessage { status, .. }) = self {
            matches!(status, 408 | 429 | 503)
        } else {
            false
        }
    }
}
/// Authorization errors
impl B2Error {
    /// Returns true if the error is related to invalid credentials during authentication.
    pub fn is_credentials_issue(&self) -> bool {
        if let B2Error::B2Error(_, B2ErrorMessage { ref message, .. }) = self {
            matches!(message.as_str(),
                "B2 has not been enabled for this account" |
                "User is in B2 suspend" |
                "Cannot authorize domain site license account" |
                "Invalid authorization" |
                "Account is missing a mobile phone number. Please update account settings.")
        } else {
            false
        }
    }
    pub fn is_wrong_credentials(&self) -> bool {
        if let B2Error::B2Error(_, B2ErrorMessage { ref code, .. }) = self {
            matches!(code.as_str(), "bad_auth_token")
        } else {
            false
        }
    }
    /// Returns true if the error is caused by the authentication being expired. Consider
    /// using the method [`should_obtain_new_authentication`] instead.
    ///
    ///  [`should_obtain_new_authentication`]: #method.should_obtain_new_authentication
    pub fn is_expired_authentication(&self) -> bool {
        if let B2Error::B2Error(
            _,
            B2ErrorMessage {
                ref code, status, ..
            },
        ) = self
        {
            if *status == 401 && code == "expired_auth_token" {
                return true;
            }
        }
        false
    }
    /// Returns true if the error is caused by any issue related to the authorization
    /// token, including expired authentication tokens and invalid authorization tokens.
    pub fn is_authorization_issue(&self) -> bool {
        if self.is_expired_authentication() {
            return true;
        }
        if let B2Error::B2Error(_, B2ErrorMessage { ref message, .. }) = self {
            if message.starts_with("Account ") && message.ends_with(" does not exist") {
                return true;
            }
            if message.starts_with("Bucket is not authorized: ") {
                return true;
            }
            matches!(
                message.as_str(),
                "Invalid authorization token" |
                "Authorization token for wrong cluster" |
                "Not authorized" |
                //"No Authorization header" |
                //"Authorization token is missing" |
                "AccountId bad"
            )
        } else {
            false
        }
    }
}
/// File errors
impl B2Error {
    /// Returns true if the error is caused by a file name which is not allowed on the b2
    /// server.
    pub fn is_invalid_file_name(&self) -> bool {
        if let B2Error::B2Error(_, B2ErrorMessage { ref message, .. }) = self {
            matches!(
                message.as_str(),
                "File names must contain at least one character"
                    | "File names in UTF8 must be no more than 1000 bytes"
                    | "File names must not start with '/'"
                    | "File names must not end with '/'"
                    | "File names must not contain '\\'"
                    | "File names must not contain DELETE"
                    | "File names must not contain '//'"
                    | "File names segment must not be more than 250 bytes"
            )
        } else {
            false
        }
    }
    /// Returns true if the error is related to a file that was not found.
    pub fn is_file_not_found(&self) -> bool {
        if let B2Error::B2Error(
            _,
            B2ErrorMessage {
                ref code,
                ref message,
                ..
            },
        ) = self
        {
            if code == "no_such_file" {
                return true;
            }
            if message.starts_with("Invalid fileId: ") {
                return true;
            }
            if message.starts_with("Not a valid file id: ") {
                return true;
            }
            if message.starts_with("File not present: ") {
                return true;
            }
            if message.starts_with("Bucket ") && message.contains("does not have file:") {
                return true;
            }
            matches!(
                message.as_str(),
                "file_state_deleted" | "file_state_none" | "file_state_unknown"
            )
        } else {
            false
        }
    }
    /// Returns true if the error is caused by an attempt to hide a hidden file.
    pub fn is_file_already_hidden(&self) -> bool {
        if let B2Error::B2Error(_, B2ErrorMessage { ref code, .. }) = self {
            code == "already_hidden"
        } else {
            false
        }
    }
    /// Returns true if the error is caused by a request to download an interval of a file
    /// that is out of bounds.
    pub fn is_range_out_of_bounds(&self) -> bool {
        if let B2Error::B2Error(_, B2ErrorMessage { ref code, .. }) = self {
            code == "range_not_satisfiable"
        } else {
            false
        }
    }
    /// Returns true if the error is caused by the sha1 of the uploaded file not matching.
    pub fn is_invalid_sha1(&self) -> bool {
        if let B2Error::B2Error(_, B2ErrorMessage { ref message, .. }) = self {
            message == "Sha1 did not match data received"
        } else {
            false
        }
    }
}
/// Bucket errors
impl B2Error {
    /// Returns true if the error is caused by the account having reached the maximum
    /// bucket count.
    pub fn is_maximum_bucket_limit(&self) -> bool {
        if let B2Error::B2Error(
            _,
            B2ErrorMessage {
                ref code, status, ..
            },
        ) = self
        {
            if *status == 400 && code == "too_many_buckets" {
                return true;
            }
        }
        false
    }
    /// Returns true if the error is caused by an attempt to create a bucket with a name
    /// of a pre-existing bucket.
    pub fn is_duplicate_bucket_name(&self) -> bool {
        if let B2Error::B2Error(
            _,
            B2ErrorMessage {
                ref code, status, ..
            },
        ) = self
        {
            if *status == 400 && code == "duplicate_bucket_name" {
                return true;
            }
        }
        false
    }
    /// Returns true if the error is caused by an attempt to create a bucket with a name
    /// which is not allowed.
    pub fn is_invalid_bucket_name(&self) -> bool {
        if let B2Error::B2Error(
            _,
            B2ErrorMessage {
                ref message,
                status,
                ..
            },
        ) = self
        {
            if *status == 400 {
                match message.as_str() {
                    "bucketName must be at least 6 characters long" => true,
                    "bucketName can be at most 50 characters long" => true,
                    "Invalid characters in bucketName: must be alphanumeric or '-'" => {
                        true
                    }
                    _ => false,
                }
            } else {
                false
            }
        } else {
            false
        }
    }
    /// Returns true if the error is caused by requests to interact with buckets that do
    /// not exist.
    pub fn is_bucket_not_found(&self) -> bool {
        if let B2Error::B2Error(_, B2ErrorMessage { ref message, .. }) = self {
            if message.starts_with("Bucket does not exist: ") {
                return true;
            }
            if message.starts_with("Invalid bucket id: ") {
                return true;
            }
            if message.starts_with("Invalid bucketId: ") {
                return true;
            }
            if message == "bad bucketId" {
                return true;
            }
            if message == "invalid_bucket_id" {
                return true;
            }
            if message == "BucketId not valid for account" {
                return true;
            }
            if message.starts_with("Bucket ") || message.starts_with("bucket ") {
                message.ends_with(" does not exist")
                    || message.ends_with(" is not a B2 bucket")
            } else {
                false
            }
        } else {
            false
        }
    }
}
/// Various errors
impl B2Error {
    /// Returns true if a request used a ifRevisionIs header and the test failed.
    pub fn is_conflict(&self) -> bool {
        if let B2Error::B2Error(_, B2ErrorMessage { status, .. }) = self {
            *status == 409
        } else {
            false
        }
    }
    /// Returns true if the usage cap on backblaze b2 has been exceeded.
    pub fn is_cap_exceeded(&self) -> bool {
        if let B2Error::B2Error(_, B2ErrorMessage { ref code, .. }) = self {
            code == "cap_exceeded"
        } else {
            false
        }
    }
    /// Returns true if the error is caused by interacting with snapshot buckets in ways
    /// not allowed.
    pub fn is_snapshot_interaction_failure(&self) -> bool {
        if let B2Error::B2Error(_, B2ErrorMessage { ref message, .. }) = self {
            matches!(message.as_str(), "Snapshot buckets are reserved for Backblaze use" | "Allow snapshot header must be specified when deleting a file from a snapshot bucket" | "Cannot change a bucket to a snapshot bucket")
        } else {
            false
        }
    }
    /// Returns true if the issue is regarding an invalid file prefix.
    pub fn is_prefix_issue(&self) -> bool {
        if let B2Error::B2Error(_, B2ErrorMessage { ref message, .. }) = self {
            matches!(
                message.as_str(),
                "Prefix must not start with delimiter"
                    | "Prefix must be 1 or more characters long"
            )
        } else {
            false
        }
    }
    /// Returns true if the issue is an invalid path delimiter.
    pub fn is_invalid_delimiter(&self) -> bool {
        if let B2Error::B2Error(_, B2ErrorMessage { ref message, .. }) = self {
            message == "Delimiter must be within acceptable list"
        } else {
            false
        }
    }
}

impl From<serde_json::Error> for B2Error {
    fn from(err: serde_json::Error) -> B2Error {
        B2Error::JsonError(err)
    }
}
impl From<hyper::error::Error> for B2Error {
    fn from(err: hyper::error::Error) -> B2Error {
        B2Error::HyperError(err)
    }
}
impl From<http::Error> for B2Error {
    fn from(err: http::Error) -> B2Error {
        B2Error::HttpError(err)
    }
}
impl From<http::header::InvalidHeaderName> for B2Error {
    fn from(err: http::header::InvalidHeaderName) -> B2Error {
        B2Error::HttpError(http::Error::from(err))
    }
}
impl From<http::header::InvalidHeaderValue> for B2Error {
    fn from(err: http::header::InvalidHeaderValue) -> B2Error {
        B2Error::HttpError(http::Error::from(err))
    }
}
impl From<http::uri::InvalidUri> for B2Error {
    fn from(err: http::uri::InvalidUri) -> B2Error {
        B2Error::HttpError(http::Error::from(err))
    }
}
impl From<std::io::Error> for B2Error {
    fn from(err: std::io::Error) -> B2Error {
        B2Error::IOError(err)
    }
}
impl fmt::Display for B2Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            B2Error::HyperError(err) => err.fmt(f),
            B2Error::HttpError(err) => err.fmt(f),
            B2Error::IOError(err) => err.fmt(f),
            B2Error::JsonError(err) => err.fmt(f),
            B2Error::B2Error(_, err) => {
                write!(f, "{} ({}): {}", err.status, err.code, err.message)
            }
            B2Error::ApiInconsistency(ref msg) => msg.fmt(f),
        }
    }
}
impl std::error::Error for B2Error {
    fn cause(&self) -> Option<&dyn std::error::Error> {
        match self {
            B2Error::HyperError(err) => Some(err),
            B2Error::HttpError(err) => Some(err),
            B2Error::IOError(err) => Some(err),
            B2Error::JsonError(err) => Some(err),
            B2Error::B2Error(_, _) => None,
            B2Error::ApiInconsistency(_) => None,
        }
    }
}
