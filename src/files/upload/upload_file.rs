use crate::auth::B2Authorization;
use crate::files::File;
use crate::files::upload::{UploadUrl, UploadFileInfo, SimpleFileInfo};

use serde::Serialize;

use crate::B2Error;
use crate::b2_future::B2Future;
use crate::client::{ApiCall, serde_body};
use http::header::HeaderMap;
use http::method::Method;
use http::uri::Uri;
use hyper::Body;
use hyper::client::ResponseFuture;
use std::convert::TryFrom;

/// The [`b2_upload_file`] api call.
///
/// You can execute this api call using a [`B2Client`], which will result in a
/// [`File`] if successful.
///
/// [`b2_get_upload_url`]: https://www.backblaze.com/b2/docs/b2_get_upload_url.html
/// [`B2Client`]: ../../client/struct.B2Client.html
/// [`File`]: ../struct.File.html
#[derive(Clone, Debug)]
pub struct UploadFile<'a, Info: UploadFileInfo> {
    url: &'a UploadUrl,
    file_name: &'a str,
    content_type: &'a str,
    content_length: u64,
    content_sha1: &'a str,
    info: Info,
}
impl<'a> UploadFile<'a, SimpleFileInfo> {
    /// Create an api call to request an upload url for the specified bucket.
    pub fn new(
        url: &'a UploadUrl,
        file_name: &'a str,
        content_type: &'a str,
        content_length: u64,
        content_sha1: &'a str,
    ) -> Self {
        UploadFile {
            url,
            file_name,
            content_type,
            content_length,
            content_sha1,
            info: SimpleFileInfo::new(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GetUploadUrlRequest<'a> {
    bucket_id: &'a str,
}

impl<'a> ApiCall for GetUploadUrl<'a> {
    type Future = B2Future<UploadUrl>;
    const METHOD: Method = Method::POST;
    fn url(&self) -> Result<Uri, B2Error> {
        Uri::try_from(format!("{}/b2api/v2/b2_get_upload_url", self.auth.api_url))
            .map_err(B2Error::from)
    }
    fn headers(&self) -> Result<HeaderMap, B2Error> {
        let mut map = HeaderMap::new();
        map.append("Authorization", self.auth.auth_token());
        Ok(map)
    }
    fn body(&self) -> Result<Body, B2Error> {
        serde_body(&GetUploadUrlRequest {
            bucket_id: self.bucket_id,
        })
    }
    fn finalize(self, fut: ResponseFuture) -> B2Future<UploadUrl> {
        B2Future::new(fut)
    }
    fn error(self, err: B2Error) -> B2Future<UploadUrl> {
        B2Future::err(err)
    }
}
