//! This module contains five different modules, each with different functions for accessing the
//! b2 api directly.
//!
//! The various methods for accessing the backblaze api are implemented on an Authorization struct.
//! There are 3 different authorization structs: [B2Authorization][1], [UploadAuthorization][4] and
//! [DownloadAuthorization][3].
//!
//! All access to the library starts with somehow obtaining the appropriate authorization struct.
//! In order to obtain an [B2Authorization][1], one must first obtain a [B2Credentials][2], which
//! contains a b2 user id and api key. Using this struct an authorization token can be obtained in
//! the form of the [B2Authorization][1] struct:
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
//! This [B2Authorization][1] struct can be used to perform various requests to the b2 api, see the
//! documentation on the [B2Authorization][1] for documentation regarding each of the possible
//! functions.
//!
//!  [1]: authorize/struct.B2Authorization.html
//!  [2]: authorize/struct.B2Credentials.html
//!  [3]: download/struct.DownloadAuthorization.html
//!  [4]: upload/struct.UploadAuthorization.html

pub mod authorize;
pub mod buckets;
pub mod files;
pub mod upload;
pub mod download;

