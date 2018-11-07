//! Download files from backblaze.
//!
//! The module [`stream_util`] has useful methods for working with the streams provided
//! by the methods in this module
//!
//! [`stream_util`]: ../../stream_util/index.html
use serde_json::to_vec;

use hyper::{Client, Request};
use hyper::body::Body;
use hyper::client::connect::Connect;
use percent_encoding::*;

use bytes::Bytes;
use futures::{Poll, Future, Async};

use crate::{BytesString, B2Error};
use crate::authorize::B2Authorization;
use crate::b2_future::B2Future;

//pub mod large;
mod future;
mod stream;
pub use self::future::DownloadFuture;
pub use self::stream::DownloadStream;

#[inline]
fn encode_bucket(bucket: &str) -> PercentEncode<PATH_SEGMENT_ENCODE_SET> {
    utf8_percent_encode(bucket, PATH_SEGMENT_ENCODE_SET)
}
#[inline]
fn encode_file(filename: &str) -> PercentEncode<DEFAULT_ENCODE_SET> {
    utf8_percent_encode(filename, DEFAULT_ENCODE_SET)
}
#[inline]
fn encode_query(query: &[u8]) -> PercentEncode<QUERY_ENCODE_SET> {
    percent_encode(query, QUERY_ENCODE_SET)
}

/// An authorization for downloads.
///
/// Created by [`get_download_authorization`].
///
/// [`get_download_authorization`]: fn.get_download_authorization.html
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadAuthorization {
    pub bucket_id: String,
    pub file_name_prefix: String,
    pub authorization_token: BytesString,
    pub download_url: BytesString,
}

/// An authorization for downloading backblaze files in public buckets.
#[derive(Clone, Serialize, Deserialize)]
pub struct PublicDownloadAuthorization {
    pub download_url: BytesString,
}
impl PublicDownloadAuthorization {
    /// Create a download authorization for public buckets. Please note that the download
    /// url varies from backblaze account to account.
    pub fn new<I>(download_url: I) -> PublicDownloadAuthorization
    where I: Into<BytesString>
    {
        PublicDownloadAuthorization {
            download_url: download_url.into(),
        }
    }
    /// Create an authorization allowing access to all public buckets owned by this
    /// account.
    pub fn from_auth(auth: &B2Authorization) -> PublicDownloadAuthorization {
        PublicDownloadAuthorization {
            download_url: auth.download_url.clone(),
        }
    }
    /// Create an authorization allowing access to all public buckets owned by the same
    /// account as this authorization.
    pub fn from_dl_auth(auth: &DownloadAuthorization) -> PublicDownloadAuthorization {
        PublicDownloadAuthorization {
            download_url: auth.download_url.clone(),
        }
    }
}

/// A trait implemented on types that can authorize a download on backblaze using the
/// filename.
///
/// The distinction between this type and [`CanAuthorizeIdDownload`] is required since
/// [b2_download_file_by_id][1] doesn't accept a [`DownloadAuthorization`].
///
/// [`CanAuthorizeIdDownload`]: trait.CanAuthorizeIdDownload.html
/// [`DownloadAuthorization`]: struct.DownloadAuthorization.html
/// [1]: https://www.backblaze.com/b2/docs/b2_download_file_by_id.html
pub trait CanAuthorizeNameDownload {
    /// Returns the value of the `Authorization` header, if needed.
    fn authorization_header(&self) -> Option<Bytes>;
    /// Returns the url to download from.
    fn download_url(&self) -> &str;
}
impl CanAuthorizeNameDownload for DownloadAuthorization {
    fn authorization_header(&self) -> Option<Bytes> {
        Some(self.authorization_token.clone().into())
    }
    fn download_url(&self) -> &str {
        self.download_url.as_str()
    }
}
impl CanAuthorizeNameDownload for B2Authorization {
    fn authorization_header(&self) -> Option<Bytes> {
        Some(self.authorization_token.clone().into())
    }
    fn download_url(&self) -> &str {
        self.download_url.as_str()
    }
}
impl CanAuthorizeNameDownload for PublicDownloadAuthorization {
    fn authorization_header(&self) -> Option<Bytes> {
        None
    }
    fn download_url(&self) -> &str {
        self.download_url.as_str()
    }
}

/// A trait implemented on types that can authorize a download on backblaze using the
/// file id.
///
/// The distinction between this type and [`CanAuthorizeNameDownload`] is required since
/// [b2_download_file_by_id][1] doesn't accept a [`DownloadAuthorization`].
///
/// [`CanAuthorizeNameDownload`]: trait.CanAuthorizeNameDownload.html
/// [`DownloadAuthorization`]: struct.DownloadAuthorization.html
/// [1]: https://www.backblaze.com/b2/docs/b2_download_file_by_id.html
pub trait CanAuthorizeIdDownload {
    /// Returns the value of the `Authorization` header, if needed.
    fn authorization_header(&self) -> Option<Bytes>;
    /// Returns the url to download from.
    fn download_url(&self) -> &str;
}
impl CanAuthorizeIdDownload for B2Authorization {
    fn authorization_header(&self) -> Option<Bytes> {
        Some(self.authorization_token.clone().into())
    }
    fn download_url(&self) -> &str {
        self.download_url.as_str()
    }
}
impl CanAuthorizeIdDownload for PublicDownloadAuthorization {
    fn authorization_header(&self) -> Option<Bytes> {
        None
    }
    fn download_url(&self) -> &str {
        self.download_url.as_str()
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DlAuthResponse {
    bucket_id: String,
    file_name_prefix: String,
    authorization_token: BytesString,
}
impl DlAuthResponse {
    fn merge(self, url: BytesString) -> DownloadAuthorization {
        DownloadAuthorization {
            bucket_id: self.bucket_id,
            file_name_prefix: self.file_name_prefix,
            authorization_token: self.authorization_token,
            download_url: url,
        }
    }
}
/// A future that resolves to a [`DownloadAuthorization`].
///
/// This future is typically created by the [`get_download_authorization`] function.
///
/// [`get_download_authorization`]: fn.get_download_authorization.html
/// [`DownloadAuthorization`]: struct.DownloadAuthorization.html
pub struct DownloadAuthFuture {
    future: B2Future<DlAuthResponse>,
    url: BytesString,
}
impl Future for DownloadAuthFuture {
    type Item = DownloadAuthorization;
    type Error = B2Error;
    fn poll(&mut self) -> Poll<DownloadAuthorization, B2Error> {
        match self.future.poll() {
            Ok(Async::Ready(response)) => {
                Ok(Async::Ready(response.merge(self.url.clone())))
            },
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(err) => Err(err),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GetDownloadAuthRequest<'a> {
    bucket_id: &'a str,
    file_name_prefix: &'a str,
    valid_duration_in_seconds: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    b2_content_disposition: Option<&'a str>,
}

/// Get the authorization for downloading files. This requires the `shareFiles`
/// capability.
///
/// This is done using the [b2_get_download_authorization][1] api call. The maximum
/// duration is 604800 seconds (one week).
///
/// [1]: https://www.backblaze.com/b2/docs/b2_get_download_authorization.html
pub fn get_download_authorization<C>(
    auth: &B2Authorization,
    client: &Client<C, Body>,
    bucket_id: &str,
    file_name_prefix: &str,
    valid_duration_in_seconds: u32,
    b2_content_disposition: Option<&str>,
) -> DownloadAuthFuture
where
    C: Connect + Sync + 'static,
    C::Transport: 'static,
    C::Future: 'static,
{
    let url_string: String =
        format!("{}/b2api/v2/b2_get_download_authorization", auth.api_url);
    let mut request = Request::post(url_string);
    request.header("Authorization", auth.auth_token());

    let body = match to_vec(&GetDownloadAuthRequest {
        bucket_id,
        file_name_prefix,
        valid_duration_in_seconds,
        b2_content_disposition,
    }) {
        Ok(body) => body,
        Err(err) => return DownloadAuthFuture {
            future: B2Future::err(err),
            url: auth.download_url.clone(),
        },
    };
    let body = Body::from(body);

    let request = match request.body(body) {
        Ok(req) => req,
        Err(err) => return DownloadAuthFuture {
            future: B2Future::err(err),
            url: auth.download_url.clone(),
        },
    };

    let future = client.request(request);

    DownloadAuthFuture {
        future: B2Future::new(future),
        url: auth.download_url.clone(),
    }
}

/// Downloads a file from backblaze by id.
///
/// If range is specified, that part of the file is downloaded. Both ends of the range
/// are inclusive.
///
/// This is done using the [b2_download_file_by_id][1] api call.
///
/// [1]: https://www.backblaze.com/b2/docs/b2_download_file_by_id.html
pub fn download_by_id<C, Auth>(
    auth: &Auth,
    client: &Client<C, Body>,
    file_id: &str,
    range: Option<(u64, u64)>,
) -> DownloadFuture
where
    Auth: CanAuthorizeIdDownload,
    C: Connect + Sync + 'static,
    C::Transport: 'static,
    C::Future: 'static,
{
    let url_string: String =
        format!("{}/b2api/v2/b2_download_file_by_id?fileId={}",
                auth.download_url(),
                encode_file(file_id));
    let mut request = Request::get(url_string);
    if let Some(token) = auth.authorization_header() {
        request.header("Authorization", token);
    }
    if let Some((start, end)) = range {
        request.header("Range", format!("{}-{}", start, end));
    }

    let request = match request.body(Body::empty()) {
        Ok(req) => req,
        Err(err) => return DownloadFuture::err(err),
    };

    let future = client.request(request);

    DownloadFuture::new(future)
}
/// Downloads a file from backblaze by name.
///
/// If range is specified, that part of the file is downloaded. Both ends of the range
/// are inclusive.
///
/// This is done using the [b2_download_file_by_name][1] api call.
///
/// [1]: https://www.backblaze.com/b2/docs/b2_download_file_by_name.html
pub fn download_by_name<C, Auth>(
    auth: &Auth,
    client: &Client<C, Body>,
    bucket_name: &str,
    file_name: &str,
    range: Option<(u64, u64)>,
) -> DownloadFuture
where
    Auth: CanAuthorizeNameDownload,
    C: Connect + Sync + 'static,
    C::Transport: 'static,
    C::Future: 'static,
{
    let url_string: String =
        format!("{}/file/{}/{}", auth.download_url(),
        encode_bucket(bucket_name), encode_file(file_name));
    let mut request = Request::get(url_string);
    if let Some(token) = auth.authorization_header() {
        request.header("Authorization", token);
    }
    if let Some((start, end)) = range {
        request.header("Range", format!("{}-{}", start, end));
    }

    let request = match request.body(Body::empty()) {
        Ok(req) => req,
        Err(err) => return DownloadFuture::err(err),
    };

    let future = client.request(request);

    DownloadFuture::new(future)
}

/// Create the url needed to download the specified file.
///
/// This url will work in a GET request without any special headers, and could therefore
/// be used in links on a webpage. However you should be aware that the authorization
/// token is included in the url, which means that anyone given this url has the equal
/// access to the backblaze account as you.
///
/// For public buckets, prefer to use a [`PublicDownloadAuthorization`], and for private
/// buckets prefer to use a [`DownloadAuthorization`] with an appropriate prefix on the
/// file name.
///
/// [`PublicDownloadAuthorization`]: struct.PublicDownloadAuthorization.html
/// [`DownloadAuthorization`]: struct.DownloadAuthorization.html
pub fn download_by_name_url<Auth>(
    auth: &Auth,
    bucket_name: &str,
    file_name: &str
) -> String where Auth: CanAuthorizeNameDownload {
    let url = auth.download_url();
    match auth.authorization_header() {
        None => {
            format!("{}/file/{}/{}",
                    url,
                    encode_bucket(bucket_name),
                    encode_file(file_name))
        },
        Some(auth) => {
            format!("{}/file/{}/{}?Authorization={}",
                    url,
                    encode_bucket(bucket_name),
                    encode_file(file_name),
                    encode_query(&auth[..]))
        },
    }
}


/// Create the url needed to download the specified file.
///
/// This url will work in a GET request without any special headers, and could therefore
/// be used in links on a webpage. However you should be aware that the authorization
/// token is included in the url, which means that anyone given this url has the equal
/// access to the backblaze account as you.
///
/// If the url is distributed, prefer to use this only with a public bucket and a
/// [`PublicDownloadAuthorization`].
///
/// [`PublicDownloadAuthorization`]: struct.PublicDownloadAuthorization.html
pub fn download_by_id_url<Auth>(
    auth: &Auth,
    file_id: &str
) -> String where Auth: CanAuthorizeIdDownload {
    let url = auth.download_url();
    match auth.authorization_header() {
        None => {
            format!("{}/b2api/v2/b2_download_file_by_id?fileId={}",
                    url,
                    utf8_percent_encode(file_id, QUERY_ENCODE_SET))
        },
        Some(auth) => {
            format!("{}/b2api/v2/b2_download_file_by_id?fileId={}&Authorization={}",
                    url,
                    utf8_percent_encode(file_id, QUERY_ENCODE_SET),
                    encode_query(&auth[..]))
        },
    }
}
