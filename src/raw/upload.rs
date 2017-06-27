//! This module defines various methods and structs for interacting with uplaods on backblaze.
//!
//! The primary struct in this module is [UploadAuthorization][1], which has various methods for
//! uploading files to backblaze b2. This struct is usually obtained from a [B2Authorization][2]
//! using the method [get_upload_url][3].
//!
//!  [1]: struct.UploadAuthorization.html
//!  [2]: ../authorize/struct.B2Authorization.html
//!  [3]: ../authorize/struct.B2Authorization.html#method.get_upload_url

use std::io::{Write, Read, copy};

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
/// Contains the information needed to authorize an upload to b2. This struct is usually obtained
/// from a [B2Authorization][1] using the method [get_upload_url][2].
///
/// The b2 website specifies that you may not upload to the same url in parallel.
///
///  [1]: ../authorize/struct.B2Authorization.html
///  [2]: ../authorize/struct.B2Authorization.html#method.get_upload_url
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

/// Methods related to the [upload module][1].
///
///  [1]: ../upload/index.html
impl<'a> B2Authorization<'a> {
    /// Performs a [b2_get_upload_url][1] api call and returns the upload url wrapped in an
    /// [`UploadAuthorization`].
    ///
    /// # Errors
    /// This function returns a [`B2Error`] in case something goes wrong. Besides the standard
    /// errors, this function can fail with [`is_bucket_not_found`].
    ///
    ///  [1]: https://www.backblaze.com/b2/docs/b2_get_upload_url.html
    ///  [`B2Error`]: ../authorize/enum.B2Error.html
    ///  [`is_bucket_not_found`]: ../../enum.B2Error.html#method.is_bucket_not_found
    ///  [`UploadAuthorization`]: struct.UploadAuthorization.html
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
    /// Equivalent to calling [create_upload_file_request][1], writing everything in the Read to
    /// the Writer and calling finish.
    ///
    ///  [1]: struct.UploadAuthorization.html#method.create_upload_file_request
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
    /// [UploadFileRequest][1], which implements [Write][2]. When writing to this object, the
    /// data is sent to backblaze b2. This method of uploading can be used to
    /// implement things such as rate limiting of the request.
    ///
    /// After the file has been sent, you need to call the [finish method][3] on the
    /// [UploadFileRequest][1], in order to close the connection.
    ///
    /// The [upload_file method][4] can be used to upload any Reader easily. The backblaze api
    /// supports not specifying the sha1 checksum. This is not recommended, but if you wish to do
    /// this, simply pass the string `do_not_verify` as the sha1 checksum.
    ///
    /// The function [create_upload_file_request_sha1_at_end][5] might be of interest. This
    /// function behaves identically to this function, except the sha1 is passed when calling
    /// finish instead of before initiating the request.
    ///
    /// Read the [backblaze api documentation][6] for more information.
    ///
    /// # Errors
    /// This function returns a [`B2Error`] in case something goes wrong. Besides the standard
    /// errors, this function can fail with [`is_invalid_file_name`] and [`is_cap_exceeded`].
    ///
    ///  [1]: struct.UploadFileRequest.html
    ///  [2]: https://doc.rust-lang.org/stable/std/io/trait.Write.html
    ///  [3]: struct.UploadFileRequest.html#method.finish
    ///  [4]: struct.UploadAuthorization.html#method.upload_file
    ///  [5]: struct.UploadFileRequest.html#method.create_upload_file_request_sha1_at_end
    ///  [6]: https://www.backblaze.com/b2/docs/uploading.html
    ///  [`B2Error`]: ../authorize/enum.B2Error.html
    ///  [`is_invalid_file_name`]: ../../enum.B2Error.html#method.is_invalid_file_name
    ///  [`is_cap_exceeded`]: ../../enum.B2Error.html#method.is_cap_exceeded
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
    /// Starts a request to upload a file to backblaze b2. This function returns an
    /// [UploadFileRequestSha1End][1], which implements [Write][2]. When writing to this object,
    /// the data is sent to backblaze b2. This method of uploading can be used to implement things
    /// such as rate limiting of the request.
    ///
    /// The value of the `content_length` parameter must be exactly the amount of bytes you are
    /// going to write, not including the 40 byte sha1 appended by the [finish method][3].
    ///
    /// After the file has been sent, you need to call the [finish method][3] on the
    /// [UploadFileRequestSha1End][1], in order to close the connection.
    ///
    /// Read the [backblaze api documentation][4] for more information.
    ///
    /// # Errors
    /// This function returns a [`B2Error`] in case something goes wrong. Besides the standard
    /// errors, this function can fail with [`is_invalid_file_name`] and [`is_cap_exceeded`].
    ///
    ///  [1]: struct.UploadFileRequestSha1End.html
    ///  [2]: https://doc.rust-lang.org/stable/std/io/trait.Write.html
    ///  [3]: struct.UploadFileRequestSha1End.html#method.finish
    ///  [4]: https://www.backblaze.com/b2/docs/uploading.html
    ///  [`B2Error`]: ../authorize/enum.B2Error.html
    ///  [`is_invalid_file_name`]: ../../enum.B2Error.html#method.is_invalid_file_name
    ///  [`is_cap_exceeded`]: ../../enum.B2Error.html#method.is_cap_exceeded
    pub fn create_upload_file_request_sha1_at_end<C,S>(&self, file_name: String,
                                                       content_type: Option<Mime>,
                                                       content_length: u64,
                                                       connector: &C)
        -> Result<UploadFileRequestSha1End, B2Error>
        where C: NetworkConnector<Stream=S>, S: Into<Box<NetworkStream + Send>>
    {
        let url: Url = Url::parse(&self.upload_url)?;
        let mut request = Request::with_connector(Method::Post, url, connector)?;
        {
            let headers: &mut Headers = request.headers_mut();
            headers.set(self.auth_header());
            headers.set(XBzFileName(file_name));
            headers.set(XBzContentSha1("hex_digits_at_end".to_owned()));
            headers.set(ContentLength(content_length + 40));
            headers.set(ContentType(match content_type {
                Some(v) => v,
                None => "b2/x-auto".parse().unwrap()
            }));
        }
        Ok(UploadFileRequestSha1End { request: request.start()? })
    }
}
header! { (XBzFileName, "X-Bz-File-Name") => [String] }
header! { (XBzContentSha1, "X-Bz-Content-Sha1") => [String] }

/// Contains an ongoing upload to the backblaze b2 api. This struct is created by the
/// [`create_upload_file_request`] method.
///
///  [`create_upload_file_request`]: struct.UploadAuthorization.html#method.create_upload_file_request
pub struct UploadFileRequest {
    request: Request<Streaming>
}
impl Write for UploadFileRequest {
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
    ///
    /// # Errors
    /// This function returns a [`B2Error`] in case something goes wrong. Besides the standard
    /// errors, this function can fail with [`is_cap_exceeded`], [`is_invalid_sha1`].
    ///
    ///  [`B2Error`]: ../authorize/enum.B2Error.html
    ///  [`is_cap_exceeded`]: ../../enum.B2Error.html#method.is_cap_exceeded
    ///  [`is_invalid_sha1`]: ../../enum.B2Error.html#method.is_invalid_sha1
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
/// Contains an ongoing upload to the backblaze b2 api. This struct is created by the
/// [create_upload_file_request_sha1_at_end][1] method.
///
///  [1]: struct.UploadAuthorization.html#method.create_upload_file_request_sha1_at_end
pub struct UploadFileRequestSha1End {
    request: Request<Streaming>
}
impl Write for UploadFileRequestSha1End {
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
impl UploadFileRequestSha1End {
    /// Finishes the upload of the file and returns information about the uploaded file. The `sha1`
    /// argument must be the ascii encoding of the sha1 of the file.
    ///
    /// The sha1 should be 40 bytes long, but this is not checked at runtime.
    ///
    /// # Errors
    /// This function returns a [`B2Error`] in case something goes wrong. Besides the standard
    /// errors, this function can fail with [`is_cap_exceeded`], [`is_invalid_sha1`].
    ///
    ///  [`B2Error`]: ../authorize/enum.B2Error.html
    ///  [`is_cap_exceeded`]: ../../enum.B2Error.html#method.is_cap_exceeded
    ///  [`is_invalid_sha1`]: ../../enum.B2Error.html#method.is_invalid_sha1
    pub fn finish<InfoType>(mut self, sha1: &str) -> Result<MoreFileInfo<InfoType>, B2Error>
        where for<'de> InfoType: Deserialize<'de>
    {
        self.request.write_all(sha1.as_bytes())?;
        let resp = self.request.send()?;
        if resp.status != hyper::status::StatusCode::Ok {
            Err(B2Error::from_response(resp))
        } else {
            Ok(serde_json::from_reader(resp)?)
        }
    }
}

