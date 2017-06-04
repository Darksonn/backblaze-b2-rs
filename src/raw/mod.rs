
//! This module contains five different modules, each with different functions for accessing the
//! b2 api directly.
//!
//! The various methods for accessing the backblaze api are implemented on an Authorization struct.
//! There are 3 different authorization structs: B2Authorization, UploadAuthorization and
//! DownloadAuthorization.
//!
//! All access to the library starts with somehow obtaining the appropriate authorization struct.
//! In order to obtain an B2Authorization struct, one must first obtain a B2Credentials struct,
//! which contains a b2 user id and api key. Using this struct an authorization token can be
//! obtained in the form of the B2Authorization struct:
//!
//! ```rust,no_run
//!extern crate hyper;
//!extern crate hyper_native_tls;
//!use hyper::Client;
//!use hyper::net::HttpsConnector;
//!use hyper_native_tls::NativeTlsClient;
//!# extern crate backblaze_b2;
//!use backblaze_b2::raw::authorize::B2Credentials;
//!
//!# fn main() {
//!let ssl = NativeTlsClient::new().unwrap();
//!let connector = HttpsConnector::new(ssl);
//!let client = Client::with_connector(connector);
//!
//!let cred = B2Credentials {
//!    id: "user id".to_owned(), key: "user key".to_owned()
//!};
//!let auth = cred.authorize(&client).unwrap();
//!# }
//! ```
//!
//! This B2Authorization struct can be used to perform various requests to the b2 api, see the
//! documentation on the B2Authorization for documentation regarding each of the possible
//! functions.

pub mod authorize;
pub mod buckets;
pub mod files;
pub mod upload;
pub mod download;

