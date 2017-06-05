//! This module defines the struct [DownloadAuthorization][1], which has various methods for
//! downlaoding files from backblaze b2. This struct is usually obtained from a
//! [B2Authorization][2] using the methods [to_download_authorization][3] and
//! [get_download_authorization][4].
//!
//! This module also defines two functions, which allow downloading from public backblaze buckets
//! without authentication.
//!
//!  [1]: struct.DownloadAuthorization.html
//!  [2]: ../authorize/struct.B2Authorization.html
//!  [3]: ../authorize/struct.B2Authorization.html#method.to_download_authorization
//!  [4]: ../authorize/struct.B2Authorization.html#method.get_download_authorization

use std::io::Read;

use hyper::{self, Client};
use hyper::client::Body;
use hyper::client::response::Response;
use hyper::header::{ContentLength,ContentType,CacheControl};

use serde::Deserialize;
use serde_json;
use serde_json::value::{Value as JsonValue};
use serde_json::map::Map;

use B2Error;
use B2AuthHeader;
use raw::authorize::B2Authorization;
use raw::files::FileInfo;

header! { (XBzFileId, "X-Bz-File-Id") => [String] }
header! { (XBzUploadTimestamp, "X-Bz-Upload-Timestamp") => [String] }
header! { (XBzFileName, "X-Bz-File-Name") => [String] }
header! { (XBzContentSha1, "X-Bz-Content-Sha1") => [String] }

/// Contains the authorization and access data concerning a download authorization on backblaze.
///
/// This struct is usually obtained from a [B2Authorization][1] using the methods
/// [to_download_authorization][2] and [get_download_authorization][3].
///
///  [1]: ../authorize/struct.B2Authorization.html
///  [2]: ../authorize/struct.B2Authorization.html#method.to_download_authorization
///  [3]: ../authorize/struct.B2Authorization.html#method.get_download_authorization
#[derive(Serialize,Deserialize,Clone,Debug)]
#[serde(rename_all = "camelCase")]
pub struct DownloadAuthorization<'a> {
    pub authorization_token: String,
    pub bucket_id: Option<String>,
    pub file_name_prefix: String,
    pub download_url: &'a str
}
impl<'a> DownloadAuthorization<'a> {
    /// Returns a hyper header that can be added to download requests on the backblaze api.
    pub fn auth_header(&self) -> B2AuthHeader {
        B2AuthHeader(self.authorization_token.clone())
    }
    /// Tests whether this download authorization allows access to the given bucket
    pub fn allows_bucket(&self, bucket: &str) -> bool {
        match self.bucket_id {
            Some(ref s) => s == bucket,
            None => true
        }
    }
}

fn handle_download_response<InfoType>(resp: Response)
    -> Result<(impl Read, Option<FileInfo<InfoType>>), B2Error>
    where for<'de> InfoType: Deserialize<'de>
{
    loop { // never actually loops, but allows break
           // I break so I can return response even though the match borrows it
        let file_id = match resp.headers.get::<XBzFileId>() {
            Some(header) => format!("{}", header),
            None => break
        };
        let file_name = match resp.headers.get::<XBzFileName>() {
            Some(header) => format!("{}", header),
            None => break
        };
        let content_length = match resp.headers.get::<ContentLength>() {
            Some(header) => header.0,
            None => break
        };
        let content_type = match resp.headers.get::<ContentType>() {
            Some(header) => format!("{}", header),
            None => break
        };
        let content_sha1 = match resp.headers.get::<XBzContentSha1>() {
            Some(header) => format!("{}", header),
            None => break
        };
        let upload_timestamp = match resp.headers.get::<XBzUploadTimestamp>() {
            Some(header) => format!("{}", header),
            None => break
        };
        let mut info = Map::new();
        // maybe add ContentRange check here?
        let check_headers = if resp.headers.has::<CacheControl>() {
            resp.headers.len() > 7
        } else {
            resp.headers.len() > 6
        };
        if check_headers {
            for header in resp.headers.iter() {
                if header.name().starts_with("X-Bz-Info-") {
                    info.insert(header.name()[10..].to_owned(),
                    JsonValue::String(header.value_string()));
                }
            }
        }
        return Ok((resp, Some(FileInfo {
            file_id: file_id,
            file_name: file_name,
            content_length: content_length,
            content_type: content_type,
            content_sha1: content_sha1,
            file_info: serde_json::from_value(JsonValue::Object(info))?,
            upload_timestamp: match upload_timestamp.parse() {
                Ok(v) => v,
                Err(_) => return Err(B2Error::LibraryError("upload timestamp not integer".to_owned()))
            },
        })));
    }
    Ok((resp, None))
}

impl<'a> DownloadAuthorization<'a> {

    /// Performs a [b2_download_file_by_id][1] api call.
    ///
    ///  [1]: https://www.backblaze.com/b2/docs/b2_download_file_by_id.html
    pub fn download_file_by_id<InfoType>(&self, file_id: &str, client: &Client)
        -> Result<(impl Read, Option<FileInfo<InfoType>>), B2Error>
        where for<'de> InfoType: Deserialize<'de>
    {
        let url_string: String = format!("{}/b2api/v1/b2_download_file_by_id", self.download_url);
        let url: &str = &url_string;

        let body: String = format!("{{\"fileId\":\"{}\"}}", file_id);

        let resp = try!(client.post(url)
            .body(Body::BufBody(body.as_bytes(), body.len()))
            .header(self.auth_header())
            .send());
        if resp.status != hyper::status::StatusCode::Ok {
            Err(B2Error::from_response(resp))
        } else {
            handle_download_response(resp)
        }
    }
    /// Performs a [b2_download_file_by_id][1] api call. This function specifies the range of the
    /// file to download, and the range_max parameter is inclusive.
    ///
    ///  [1]: https://www.backblaze.com/b2/docs/b2_download_file_by_id.html
    pub fn download_range_by_id<InfoType>(&self, file_id: &str, range_min: u64, range_max: u64, client: &Client)
        -> Result<(impl Read, Option<FileInfo<InfoType>>), B2Error>
        where for<'de> InfoType: Deserialize<'de>
    {
        let url_string: String = format!("{}/b2api/v1/b2_download_file_by_id", self.download_url);
        let url: &str = &url_string;

        let body: String = format!("{{\"fileId\":\"{}\"}}", file_id);

        let resp = try!(client.post(url)
            .body(Body::BufBody(body.as_bytes(), body.len()))
            .header(self.auth_header())
            .header(B2Range(format!("bytes={}-{}", range_min, range_max)))
            .send());
        if resp.status != hyper::status::StatusCode::PartialContent {
            Err(B2Error::from_response(resp))
        } else {
            handle_download_response(resp)
        }
    }
    /// Performs a [b2_download_file_by_name][1] api call.
    ///
    ///  [1]: https://www.backblaze.com/b2/docs/b2_download_file_by_name.html
    pub fn download_file_by_name<InfoType>(&self, bucket_name: &str, file_name: &str, client: &Client)
        -> Result<(impl Read, Option<FileInfo<InfoType>>), B2Error>
        where for<'de> InfoType: Deserialize<'de>
    {
        let url_string: String = format!("{}/file/{}/{}", self.download_url, bucket_name, file_name);
        let url: &str = &url_string;

        let resp = try!(client.get(url)
            .header(self.auth_header())
            .send());
        if resp.status != hyper::status::StatusCode::Ok {
            Err(B2Error::from_response(resp))
        } else {
            handle_download_response(resp)
        }
    }
    /// Performs a [b2_download_file_by_name][1] api call. This function specifies the range of the
    /// file to download, and the range_max parameter is inclusive.
    ///
    ///  [1]: https://www.backblaze.com/b2/docs/b2_download_file_by_name.html
    pub fn download_range_by_name<InfoType>(&self, bucket_name: &str, file_name: &str,
                                            range_min: u64, range_max: u64, client: &Client)
        -> Result<(impl Read, Option<FileInfo<InfoType>>), B2Error>
        where for<'de> InfoType: Deserialize<'de>
    {
        let url_string: String = format!("{}/file/{}/{}", self.download_url, bucket_name, file_name);
        let url: &str = &url_string;

        let resp = try!(client.get(url)
            .header(self.auth_header())
            .header(B2Range(format!("bytes={}-{}", range_min, range_max)))
            .send());
        if resp.status != hyper::status::StatusCode::PartialContent {
            Err(B2Error::from_response(resp))
        } else {
            handle_download_response(resp)
        }
    }
}
header! { (B2Range, "Range") => [String] }

impl<'a> B2Authorization<'a> {
    /// Use the authorization token in this B2Authorization as a download authorization. The
    /// DownloadAuthorization returned by this function can download any file on any bucket owned
    /// by this user.
    pub fn to_download_authorization(&self) -> DownloadAuthorization {
        DownloadAuthorization {
            authorization_token: self.authorization_token.clone(),
            bucket_id: None,
            file_name_prefix: "".to_owned(),
            download_url: &self.download_url
        }
    }
    /// Performs a [b2_get_download_authorization][1] api call. The DownloadAuthorization returned
    /// by this method can only download files from the specified bucket and with the specified
    /// prefix.
    ///
    ///  [1]: https://www.backblaze.com/b2/docs/b2_get_download_authorization.html
    pub fn get_download_authorization<'s>(&'s self, bucket_id: &str, file_name_prefix: Option<&str>,
                                      expires_in_seconds: u32, client: &Client)
        -> Result<DownloadAuthorization<'s>, B2Error>
    {
        let url_string: String = format!("{}/b2api/v1/b2_get_download_authorization", self.api_url);
        let url: &str = &url_string;

        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Request<'a> {
            bucket_id: &'a str,
            file_name_prefix: &'a str,
            valid_duration_in_seconds: u32
        }
        let request = Request {
            bucket_id: bucket_id,
            file_name_prefix: match file_name_prefix {
                Some(v) => v,
                None => ""
            },
            valid_duration_in_seconds: expires_in_seconds
        };
        #[derive(Serialize,Deserialize,Clone,Debug)]
        #[serde(rename_all = "camelCase")]
        pub struct Response {
            authorization_token: String,
            bucket_id: String,
            file_name_prefix: String
        }
        let body: String = serde_json::to_string(&request)?;

        let resp = client.post(url)
            .body(Body::BufBody(body.as_bytes(), body.len()))
            .header(self.auth_header())
            .send()?;
        if resp.status != hyper::status::StatusCode::Ok {
            Err(B2Error::from_response(resp))
        } else {
            let Response {
                authorization_token, bucket_id, file_name_prefix
            } = serde_json::from_reader(resp)?;
            Ok(DownloadAuthorization {
                authorization_token: authorization_token,
                bucket_id: Some(bucket_id),
                file_name_prefix: file_name_prefix,
                download_url: &self.download_url
            })
        }
    }
}

/// Performs a [b2_download_file_by_name][1] api call.
///
/// This function does not include any authorization in the request, so it can only be used to
/// access public buckets.
///
///  [1]: https://www.backblaze.com/b2/docs/b2_download_file_by_name.html
pub fn download_file_by_name<InfoType>(download_url: &str, bucket_name: &str, file_name: &str, client: &Client)
    -> Result<(impl Read, Option<FileInfo<InfoType>>), B2Error>
    where for<'de> InfoType: Deserialize<'de>
{
    let url_string: String = format!("{}/file/{}/{}", download_url, bucket_name, file_name);
    let url: &str = &url_string;

    let resp = try!(client.post(url)
                    .send());
    if resp.status != hyper::status::StatusCode::Ok {
        Err(B2Error::from_response(resp))
    } else {
        handle_download_response(resp)
    }
}
/// Performs a [b2_download_file_by_name][1] api call. This function specifies the range of the
/// file to download, and the range_max parameter is inclusive.
///
/// This function does not include any authorization in the request, so it can only be used to
/// access public buckets.
///
///  [1]: https://www.backblaze.com/b2/docs/b2_download_file_by_name.html
pub fn download_range_by_name<InfoType>(download_url: &str, bucket_name: &str, file_name: &str,
                                        range_min: u64, range_max: u64, client: &Client)
    -> Result<(impl Read, Option<FileInfo<InfoType>>), B2Error>
    where for<'de> InfoType: Deserialize<'de>
{
    let url_string: String = format!("{}/file/{}/{}", download_url, bucket_name, file_name);
    let url: &str = &url_string;

    let resp = try!(client.get(url)
                    .header(B2Range(format!("bytes={}-{}", range_min, range_max)))
                    .send());
    if resp.status != hyper::status::StatusCode::PartialContent {
        Err(B2Error::from_response(resp))
    } else {
        handle_download_response(resp)
    }
}


