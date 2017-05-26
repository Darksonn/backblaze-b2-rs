use std::fmt;

use base64::{encode as b64encode};

use hyper;
use hyper::{Client};
use hyper::header::{Header, HeaderFormat};

use serde_json;

use B2Error;
use B2AuthHeader;

#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct B2Credentials {
    pub id: String,
    pub key: String
}
impl B2Credentials {
    pub fn id_key(&self) -> String {
        format!("{}:{}", self.id, self.key)
    }
    pub fn auth_string(&self) -> String {
        format!("Basic {}", b64encode(&self.id_key()))
    }
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
    pub fn auth_header(&self) -> B2AuthHeader {
        B2AuthHeader(self.authorization_token.clone())
    }
}

