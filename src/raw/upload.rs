use std::io::Read;

use hyper::{self, Client};
use hyper::client::Body;
use hyper::header::{Headers,ContentLength,ContentType};
use hyper::mime::Mime;

use serde::Deserialize;
use serde_json;

use B2Error;
use B2AuthHeader;
use raw::authorize::B2Authorization;
use raw::files::MoreFileInfo;

/// The b2 website specifies that you may not upload to the same url in parallel.
/// Therefore this type is not Sync
#[derive(Deserialize,Serialize,Clone,Debug)]
#[serde(rename_all = "camelCase")]
pub struct UploadAuthorization {
    pub bucket_id: String,
    pub upload_url: String,
    pub authorization_token: String
}
//impl !Sync for UploadAuthorization {}
impl UploadAuthorization {
    pub fn auth_header(&self) -> B2AuthHeader {
        B2AuthHeader(self.authorization_token.clone())
    }
}

impl<'a> B2Authorization<'a> {
    pub fn get_upload_url(&self, bucket_id: &str, client: &Client)
        -> Result<UploadAuthorization,B2Error>
    {
        let url_string: String = format!("{}/b2api/v1/b2_get_upload_url", self.api_url);
        let url: &str = &url_string;

        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Request<'a> {
            bucket_id: &'a str
        }
        let request = Request {
            bucket_id: bucket_id
        };
        let body: String = serde_json::to_string(&request)?;

        let resp = client.post(url)
            .body(Body::BufBody(body.as_bytes(), body.len()))
            .header(self.auth_header())
            .send()?;
        if resp.status != hyper::status::StatusCode::Ok {
            Err(B2Error::from_response(resp))
        } else {
            Ok(serde_json::from_reader(resp)?)
        }
    }
}
impl UploadAuthorization {
    /// Some arguments are String, since the hyper headers require Strings
    pub fn upload_file<InfoType, R: Read>(&self, mut file: R, file_name: String, content_type: Option<Mime>,
                                 content_length: u64, content_sha1: String, client: &Client)
        -> Result<MoreFileInfo<InfoType>, B2Error>
        where for<'de> InfoType: Deserialize<'de>, R: Sized
    {
        let mut headers = Headers::new();
        headers.set(self.auth_header());
        headers.set(XBzFileName(file_name));
        headers.set(XBzContentSha1(content_sha1));
        headers.set(ContentLength(content_length));
        headers.set(ContentType(match content_type {
            Some(v) => v,
            None => "b2/x-auto".parse().unwrap()
        }));
        let resp = client.post(&self.upload_url)
            .body(Body::SizedBody(&mut file, content_length))
            .headers(headers)
            .send()?;
        if resp.status != hyper::status::StatusCode::Ok {
            Err(B2Error::from_response(resp))
        } else {
            Ok(serde_json::from_reader(resp)?)
        }
    }
}
header! { (XBzFileName, "X-Bz-File-Name") => [String] }
header! { (XBzContentSha1, "X-Bz-Content-Sha1") => [String] }


