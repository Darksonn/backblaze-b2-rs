//! The backblaze api requires https, so you need to provide a Client with a https connector.
//!
//! Such a client can be created with the api call below:
//!
//! ```rust
//!extern crate hyper;
//!extern crate hyper_native_tls;
//!use hyper::Client;
//!use hyper::net::HttpsConnector;
//!use hyper_native_tls::NativeTlsClient;
//!
//!# fn main() {
//!let ssl = NativeTlsClient::new().unwrap();
//!let connector = HttpsConnector::new(ssl);
//!let client = Client::with_connector(connector);
//!# }
//! ```
//!
//! Unfortunately because of the hyper api design, the upload functionality in this library
//! requires the connector instead of the client, and since the client consumes the connector,
//! you'll have to make two of them.
//!
//! See the [raw module documentation][1] for more information on using this crate.
//!
//! Currently this library is used through the raw module. This module simply supplies a function
//! for each api call. Another module for easier usage is planned.
//!
//!  [1]: raw/index.html

extern crate base64;
extern crate serde;
extern crate serde_json;
extern crate core;

#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate hyper;

pub mod raw;

use std::fmt;
use hyper::client::Response;

header! { (B2AuthHeader, "Authorization") => [String] }

/// The b2 api returns errors in a json-object, that can be deserialized into this struct.
#[derive(Deserialize, Debug)]
pub struct B2ErrorMessage {
    code: String,
    message: String,
    status: u32
}

/// An error caused while using any of the B2 apis. Errors returned by the b2 api are stored
/// exactly as received from backblaze and for ease of use several methods are provided on this
/// type in order to check the kind of error.
#[derive(Debug)]
pub enum B2Error {
    HyperError(hyper::error::Error),
    IOError(std::io::Error),
    JsonError(serde_json::Error),
    /// When the b2 website returns an error, it is stored in this variant.
    B2Error(hyper::status::StatusCode, B2ErrorMessage),
    /// This type is only returned if the b2 website is not following the api spec.
    LibraryError(String)
}

/// Load errors
impl B2Error {
    /// Returns true if the issue is related to the backblaze server being under too much load.
    pub fn is_service_unavilable(&self) -> bool {
        if let &B2Error::B2Error(_, B2ErrorMessage { ref code, ref message, status }) = self {
            status == 503
        } else { false }
    }
    /// Returns true if we are making too many requests.
    pub fn is_too_many_requests(&self) -> bool {
        if let &B2Error::B2Error(_, B2ErrorMessage { ref code, ref message, status }) = self {
            status == 429
        } else { false }
    }
    fn get_io_kind(&self) -> Option<::std::io::ErrorKind> {
        match self {
            &B2Error::IOError(ref ioe) => Some(ioe),
            &B2Error::HyperError(::hyper::error::Error::Io(ref ioe)) => Some(ioe),
            _ => None
        }.map(|io| io.kind())
    }
    /// Returns true if we should obtain new authentication. This returns true if either the
    /// connection failed or the authentication has expired. The reason for obtaining new
    /// authentication is that this allows you to connect to some other backblaze pod, which might
    /// be better connected.
    pub fn should_obtain_new_authentication(&self) -> bool {
        if let Some(ref ioe) = self.get_io_kind() {
            match ioe {
                &::std::io::ErrorKind::BrokenPipe => true,
                &::std::io::ErrorKind::ConnectionRefused => true,
                &::std::io::ErrorKind::ConnectionReset => true,
                &::std::io::ErrorKind::ConnectionAborted => true,
                &::std::io::ErrorKind::NotConnected => true,
                _ => false
            }
        } else { self.is_expired_authentication() }
    }
    /// Returns true if you should be using some sort of exponential back off for future requests.
    pub fn should_back_off(&self) -> bool {
        if let &B2Error::B2Error(_, B2ErrorMessage { ref code, ref message, status }) = self {
            match status {
                408 => true, 429 => true, 503 => true,
                _ => false
            }
        } else { false }
    }
}
/// Authorization errors
impl B2Error {
    /// Returns true if the error is related to invalid credentials during authentication.
    pub fn is_credentials_issue(&self) -> bool {
        if let &B2Error::B2Error(_, B2ErrorMessage { ref code, ref message, status }) = self {
            match message.as_str() {
                "B2 has not been enabled for this account" => true,
                "User is in B2 suspend" => true,
                "Cannot authorize domain site license account" => true,
                "Invalid authorization" => true,
                "Account is missing a mobile phone number. Please update account settings." => true,
                _ => false
            }
        } else { false }
    }
    /// Returns true if the error is caused by the authentication to have expired.
    pub fn is_expired_authentication(&self) -> bool {
        if let &B2Error::B2Error(_, B2ErrorMessage { ref code, ref message, status }) = self {
            if status == 401 && code == "expired_auth_token" {
                return true;
            }
        }
        false
    }
    /// Returns true if the error is caused by any issue related to the authorization token,
    /// including expired authentication tokens.
    pub fn is_authorization_issue(&self) -> bool {
        if self.is_expired_authentication() { return true; }
        if let &B2Error::B2Error(_, B2ErrorMessage { ref code, ref message, status }) = self {
            if message.starts_with("Account ") && message.ends_with(" does not exist") {
                return true;
            }
            if message.starts_with("Bucket is not authorized: ") {
                return true;
            }
            match message.as_str() {
                "Invalid authorization token" => true,
                "Authorization token for wrong cluster" => true,
                "Not authorized" => true,
                //"No Authorization header" => true,
                //"Authorization token is missing" => true,
                "AccountId bad" => true,
                _ => false
            }
        } else { false }
    }
}
/// File errors
impl B2Error {
    /// Returns true if the error is related to referring to file names that are invalid.
    pub fn is_invalid_file_name(&self) -> bool {
        if let &B2Error::B2Error(_, B2ErrorMessage { ref code, ref message, status }) = self {
            match message.as_str() {
                "File names must contain at least one character" => true,
                "File names in UTF8 must be no more than 1000 bytes" => true,
                "File names must not start with '/'" => true,
                "File names must not end with '/'" => true,
                "File names must not contain '\\'" => true,
                "File names must not contain DELETE" => true,
                "File names must not contain '//'" => true,
                "File names segment must not be more than 250 bytes" => true,
                _ => false
            }
        } else { false }
    }
    /// Returns true if the error is related to a file that was not found.
    pub fn is_file_not_found(&self) -> bool {
        if let &B2Error::B2Error(_, B2ErrorMessage { ref code, ref message, status }) = self {
            if code == "no_such_file" { return true; }
            if message.starts_with("Invalid fileId: ") { return true; }
            if message.starts_with("Not a valid file id: ") { return true; }
            if message.starts_with("File not present: ") { return true; }
            if message.starts_with("Bucket ") &&
               message.contains("does not have file:") { return true; }
            match message.as_str() {
                "file_state_deleted" => true,
                "file_state_none" => true,
                "file_state_unknown" => true,
                _ => false
            }
        } else { false }
    }
    /// Returns true if the error is caused by an attempt to hide a hidden file.
    pub fn is_file_already_hidden(&self) -> bool {
        if let &B2Error::B2Error(_, B2ErrorMessage { ref code, ref message, status }) = self {
            code == "already_hidden"
        } else { false }
    }
    /// Returns true if the error is caused by a request to download an interval of a file that is
    /// out of bounds.
    pub fn is_range_out_of_bounds(&self) -> bool {
        if let &B2Error::B2Error(_, B2ErrorMessage { ref code, ref message, status }) = self {
            if code == "range_not_satisfiable" { true } else { false }
        } else { false }
    }
}
/// Bucket errors
impl B2Error {
    /// Returns true if the error is caused by the account having reached the maximum bucket count.
    pub fn is_maximum_bucket_limit(&self) -> bool {
        if let &B2Error::B2Error(_, B2ErrorMessage { ref code, ref message, status }) = self {
            if status == 400 && code == "too_many_buckets" {
                return true;
            }
        }
        false
    }
    /// Returns true if the error is caused by duplicate bucket names.
    pub fn is_duplicate_bucket_name(&self) -> bool {
        if let &B2Error::B2Error(_, B2ErrorMessage { ref code, ref message, status }) = self {
            if status == 400 && code == "duplicate_bucket_name" {
                return true;
            }
        }
        false
    }
    /// Returns true if the error is caused by an attempt to create a bucket with a name which is
    /// not allowed.
    pub fn is_invalid_bucket_name(&self) -> bool {
        if let &B2Error::B2Error(_, B2ErrorMessage { ref code, ref message, status }) = self {
            if status == 400 {
                match message.as_str() {
                    "bucketName must be at least 6 characters long" => true,
                    "bucketName can be at most 50 characters long" => true,
                    "Invalid characters in bucketName: must be alphanumeric or '-'" => true,
                    _ => false
                }
            } else { false }
        } else { false }
    }
    /// Returns true if the error is caused by requests to interact with buckets that do not exist.
    pub fn is_bucket_not_found(&self) -> bool {
        if let &B2Error::B2Error(_, B2ErrorMessage { ref code, ref message, status }) = self {
            if message.starts_with("Bucket does not exist: ") { return true; }
            if message.starts_with("Invalid bucket id: ") { return true; }
            if message.starts_with("Invalid bucketId: ") { return true; }
            if message == "bad bucketId" { return true; }
            if message == "invalid_bucket_id" { return true; }
            if message == "BucketId not valid for account" { return true; }
            if message.starts_with("Bucket ") || message.starts_with("bucket ") {
                if message.ends_with(" does not exist") {
                    true
                } else if message.ends_with(" is not a B2 bucket") {
                    true
                } else { false }
            } else { false }
        } else { false }
    }
}
/// Various errors
impl B2Error {
    /// Returns true if a request used a ifRevisionIs header and the test failed.
    pub fn is_conflict(&self) -> bool {
        if let &B2Error::B2Error(_, B2ErrorMessage { ref code, ref message, status }) = self {
            status == 409
        } else { false }
    }
    /// Returns true if the usage cap on backblaze b2 has been exceeded.
    pub fn is_cap_exceeded(&self) -> bool {
        if let &B2Error::B2Error(_, B2ErrorMessage { ref code, ref message, status }) = self {
            code == "cap_exceeded"
        } else { false }
    }
    /// Returns true if the error is caused by interacting with snapshot buckets in ways not
    /// allowed.
    pub fn is_snapshot_interaction_failure(&self) -> bool {
        if let &B2Error::B2Error(_, B2ErrorMessage { ref code, ref message, status }) = self {
            match message.as_str() {
                "Snapshot buckets are reserved for Backblaze use" => true,
                "Allow snapshot header must be specified when deleting a file from a snapshot bucket" => true,
                "Cannot change a bucket to a snapshot bucket" => true,
                _ => false
            }
        } else { false }
    }
    /// Returns true if the issue is regarding an invalid file prefix.
    pub fn is_prefix_issue(&self) -> bool {
        if let &B2Error::B2Error(_, B2ErrorMessage { ref code, ref message, status }) = self {
            match message.as_str() {
                "Prefix must not start with delimiter" => true,
                "Prefix must be 1 or more characters long" => true,
                _ => false
            }
        } else { false }
    }
    /// Returns true if the issue is an invalid path delimiter.
    pub fn is_invalid_delimiter(&self) -> bool {
        if let &B2Error::B2Error(_, B2ErrorMessage { ref code, ref message, status }) = self {
            message == "Delimiter must be within acceptable list"
        } else { false }
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
impl From<hyper::error::ParseError> for B2Error {
    fn from(err: hyper::error::ParseError) -> B2Error {
        B2Error::HyperError(hyper::error::Error::Uri(err))
    }
}
impl From<std::io::Error> for B2Error {
    fn from(err: std::io::Error) -> B2Error {
        B2Error::IOError(err)
    }
}
impl B2Error {
    fn from_response(response: Response) -> B2Error {
        let status = response.status;
        let b2err = serde_json::from_reader(response);
        match b2err {
            Ok(errm) =>
                B2Error::B2Error(status, errm),
            Err(json) => B2Error::from(json)
        }
    }
}
impl fmt::Display for B2Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            B2Error::HyperError(ref he) => he.fmt(f),
            B2Error::IOError(ref ioe) => ioe.fmt(f),
            B2Error::JsonError(ref jsonerr) => jsonerr.fmt(f),
            B2Error::B2Error(_, ref b2err) => write!(f, "{} ({}): {}", b2err.status, b2err.code, b2err.message),
            B2Error::LibraryError(ref msg) => write!(f, "{}", msg)
        }
    }
}
