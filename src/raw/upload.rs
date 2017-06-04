use std::io::{Read, copy};

use hyper::{self, Client, Url};
use hyper::client::Body;
use hyper::client::request::Request;
use hyper::header::{Headers,ContentLength,ContentType};
use hyper::mime::Mime;
use hyper::method::Method;
use hyper::net::{Streaming, NetworkConnector, NetworkStream};

use serde::Deserialize;
use serde_json;

use B2Error;
use B2AuthHeader;
use raw::authorize::B2Authorization;
use raw::files::MoreFileInfo;

/// Contains the information needed to authorize an upload to b2.
/// The b2 website specifies that you may not upload to the same url in parallel.
#[derive(Deserialize,Serialize,Clone,Debug)]
#[serde(rename_all = "camelCase")]
pub struct UploadAuthorization {
    pub bucket_id: String,
    pub upload_url: String,
    pub authorization_token: String
}
impl UploadAuthorization {
    /// Returns a hyper header that authorizes an upload request.
    pub fn auth_header(&self) -> B2AuthHeader {
        B2AuthHeader(self.authorization_token.clone())
    }
}

impl<'a> B2Authorization<'a> {
    /// Performs a [b2_get_upload_url][1] api call.
    ///
    ///  [1]: https://www.backblaze.com/b2/docs/b2_get_upload_url.html
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
    pub fn upload_file<InfoType, R: Read, C, S>(&self, file: &mut R, file_name: String, content_type: Option<Mime>,
                                 content_length: u64, content_sha1: String, connector: &C)
        -> Result<MoreFileInfo<InfoType>, B2Error>
        where for<'de> InfoType: Deserialize<'de>, R: Sized, C: NetworkConnector<Stream=S>,
              S: Into<Box<NetworkStream + Send>>
    {
        let mut ufr = self.create_upload_file_request(
            file_name, content_type, content_length, content_sha1, connector)?;
        copy(file, &mut ufr)?;
        ufr.finish()
    }
    /// Starts a request to upload a file to backblaze b2. This function returns an
    /// UploadFileRequest, which implements Write. When writing to this object, the
    /// files are sent to backblaze b2. This method of uploading can be used to
    /// implement things such as rate limiting of the request.
    ///
    /// After the file has been written, you need to call the finish function on the
    /// UploadFileRequest, in order to close the connection.
    ///
    /// The upload_file function can be used to do all of this.
    pub fn create_upload_file_request<C,S>(&self, file_name: String,
                                           content_type: Option<Mime>,
                                           content_length: u64, content_sha1: String,
                                           connector: &C)
        -> Result<UploadFileRequest, B2Error>
        where C: NetworkConnector<Stream=S>, S: Into<Box<NetworkStream + Send>>
    {
        let url: Url = Url::parse(&self.upload_url)?;
        let mut request = Request::with_connector(Method::Post, url, connector)?;
        {
            let headers: &mut Headers = request.headers_mut();
            headers.set(self.auth_header());
            headers.set(XBzFileName(file_name));
            headers.set(XBzContentSha1(content_sha1));
            headers.set(ContentLength(content_length));
            headers.set(ContentType(match content_type {
                Some(v) => v,
                None => "b2/x-auto".parse().unwrap()
            }));
        }
        Ok(UploadFileRequest { request: request.start()? })
    }
}
header! { (XBzFileName, "X-Bz-File-Name") => [String] }
header! { (XBzContentSha1, "X-Bz-Content-Sha1") => [String] }

/// Contains an ongoing upload to the backblaze b2 api.
pub struct UploadFileRequest {
    request: Request<Streaming>
}
impl ::std::io::Write for UploadFileRequest {
    fn write(&mut self, msg: &[u8]) -> ::std::io::Result<usize> {
        self.request.write(msg)
    }
    fn flush(&mut self) -> ::std::io::Result<()> {
        self.request.flush()
    }
    fn write_all(&mut self, buf: &[u8]) -> Result<(), ::std::io::Error> {
        self.request.write_all(buf)
    }
    fn write_fmt(&mut self, fmt: ::core::fmt::Arguments) -> Result<(), ::std::io::Error> {
        self.request.write_fmt(fmt)
    }
}
impl UploadFileRequest {
    /// Finishes the upload of the file and returns information about the uploaded file.
    pub fn finish<InfoType>(self) -> Result<MoreFileInfo<InfoType>, B2Error>
        where for<'de> InfoType: Deserialize<'de>
    {
        let resp = self.request.send()?;
        if resp.status != hyper::status::StatusCode::Ok {
            Err(B2Error::from_response(resp))
        } else {
            Ok(serde_json::from_reader(resp)?)
        }
    }
}

