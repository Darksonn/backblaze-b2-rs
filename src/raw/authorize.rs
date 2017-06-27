//! This module defines various methods and structs used for authenticating on the B2 server.
//!
//! Authentication is usually performed by calling the [`authorize`] method on the
//! [`B2Credentials`] struct, which returns a [`B2Authorization`].
//!
//!  [`authorize`]: struct.B2Credentials.html#method.authorize
//!  [`B2Credentials`]: struct.B2Credentials.html
//!  [`B2Authorization`]: struct.B2Authorization.html

use std::fmt;

use base64::{encode as b64encode};

use hyper;
use hyper::{Client};
use hyper::header::{Header, HeaderFormat};

use serde_json;

use B2Error;
use B2AuthHeader;

/// Contains the backblaze id and key needed to authorize access to the backblaze b2 api.
/// This struct derives [Deserialize][1], so a simple way to read this from a file would be:
///
/// ```rust,no_run
///extern crate serde;
///extern crate serde_json;
///use std::fs::File;
///
///# extern crate backblaze_b2;
///# use backblaze_b2::raw::authorize::B2Credentials;
///#
///# fn main() {
///serde_json::from_reader::<_,B2Credentials>(File::open("credentials.txt").unwrap()).unwrap();
///# }
/// ```
///
///  [1]: ../../../serde/trait.Deserialize.html
#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct B2Credentials {
    pub id: String,
    pub key: String
}
impl B2Credentials {
    fn id_key(&self) -> String {
        format!("{}:{}", self.id, self.key)
    }
    /// This function returns the value of the Authorization header needed to perform a
    /// b2_authorize_account api call.
    pub fn auth_string(&self) -> String {
        format!("Basic {}", b64encode(&self.id_key()))
    }
    /// This function performs a [b2_authorize_account][1] api call to the backblaze api and returns an
    /// authorization token.
    ///
    /// # Errors
    /// This function returns a [`B2Error`] in case something goes wrong. Besides the standard
    /// non-authorization errors, this function can fail with [`is_credentials_issue`].
    ///
    ///  [1]: https://www.backblaze.com/b2/docs/b2_authorize_account.html
    ///  [`is_credentials_issue`]: ../../enum.B2Error.html#method.is_credentials_issue
    ///  [`B2Error`]: ../../enum.B2Error.html
    pub fn authorize<'a>(&'a self, client: &Client) -> Result<B2Authorization<'a>,B2Error> {
        let resp = try!(client.get("https://api.backblazeb2.com/b2api/v1/b2_authorize_account")
            .header(self.clone())
            .send());
        if resp.status != hyper::status::StatusCode::Ok {
            Err(B2Error::from_response(resp))
        } else {
            Ok(B2Authorization::from(self, try!(serde_json::from_reader(resp))))
        }
    }
}
impl HeaderFormat for B2Credentials {
    fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.auth_string().as_str())
    }
}
impl Header for B2Credentials {
    fn header_name() -> &'static str {
        "Authorization"
    }
#[allow(unused_variables)]
    fn parse_header(raw: &[Vec<u8>]) -> hyper::Result<B2Credentials> {
        panic!("we are not the b2 server");
    }
}
#[derive(Serialize,Deserialize)]
#[serde(rename_all = "camelCase")]
struct B2AuthResponse {
    authorization_token: String,
    api_url: String,
    download_url: String,
    recommended_part_size: usize,
    absolute_minimum_part_size: usize
}
/// This struct contains the needed authorization to perform any b2 api call. It is typically
/// created using the [`authorize`] method on [`B2Credentials`].
///
///  [`authorize`]: struct.B2Credentials.html#method.authorize
///  [`B2Credentials`]: struct.B2Credentials.html
#[derive(Debug)]
pub struct B2Authorization<'a> {
    pub credentials: &'a B2Credentials,
    pub authorization_token: String,
    pub api_url: String,
    pub download_url: String,
    pub recommended_part_size: usize,
    pub absolute_minimum_part_size: usize
}
impl<'a> B2Authorization<'a> {
    fn from(credentials: &'a B2Credentials, resp: B2AuthResponse) -> B2Authorization<'a> {
        B2Authorization {
            credentials: credentials,
            authorization_token: resp.authorization_token,
            api_url: resp.api_url,
            download_url: resp.download_url,
            recommended_part_size: resp.recommended_part_size,
            absolute_minimum_part_size: resp.absolute_minimum_part_size
        }
    }
    /// Returns a hyper header that correctly authorizes an api call to backblaze.
    pub fn auth_header(&self) -> B2AuthHeader {
        B2AuthHeader(self.authorization_token.clone())
    }
}

