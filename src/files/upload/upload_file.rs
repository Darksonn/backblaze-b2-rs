use crate::files::upload::{UploadUrl, UploadFileInfo, SimpleFileInfo};

use serde::Serialize;

use crate::B2Error;
use crate::b2_future::B2Future;
use crate::client::ApiCall;
use http::header::{HeaderMap, HeaderName, HeaderValue};
use http::method::Method;
use http::uri::Uri;
use hyper::Body;
use hyper::client::ResponseFuture;
use std::convert::{TryFrom, TryInto};

/// The [`b2_upload_file`] api call.
///
/// You can execute this api call using a [`B2Client`], which will result in a
/// [`File`] if successful.
///
/// [`b2_upload_file`]: https://www.backblaze.com/b2/docs/b2_upload_file.html
/// [`B2Client`]: ../../client/struct.B2Client.html
/// [`File`]: ../struct.File.html
#[derive(Debug)]
pub struct UploadFile<'a, Info: UploadFileInfo<'a>> {
    url: &'a UploadUrl,
    file_name: &'a str,
    content_type: &'a str,
    content_length: u64,
    content_sha1: &'a str,
    info: &'a Info,
    body: Option<Body>,
}

static DEFAULT_INFO: SimpleFileInfo = SimpleFileInfo::new();

impl<'a> UploadFile<'a, SimpleFileInfo> {
    /// Create an api call to request an upload url for the specified bucket.
    pub fn new(
        url: &'a UploadUrl,
        file_name: &'a str,
        content_type: &'a str,
        content_length: u64,
        content_sha1: &'a str,
        body: Body,
    ) -> Self {
        UploadFile {
            url,
            file_name,
            content_type,
            content_length,
            content_sha1,
            body: Some(body),
            info: &DEFAULT_INFO,
        }
    }
}
impl<'a, Info: UploadFileInfo<'a>> UploadFile<'a, Info> {
    /// Create an api call to request an upload url for the specified bucket.
    pub fn with_info<NewInfo: UploadFileInfo<'a>>(
        self,
        info: &'a NewInfo,
    ) -> UploadFile<'a, NewInfo> {
        UploadFile {
            url: self.url,
            file_name: self.file_name,
            content_type: self.content_type,
            content_length: self.content_length,
            content_sha1: self.content_sha1,
            body: self.body,
            info,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GetUploadUrlRequest<'a> {
    bucket_id: &'a str,
}

impl<'a, Info: UploadFileInfo<'a>> ApiCall for UploadFile<'a, Info> {
    type Future = B2Future<UploadUrl>;
    const METHOD: Method = Method::POST;
    fn url(&self) -> Result<Uri, B2Error> {
        Uri::try_from(self.url.upload_url.as_str()).map_err(B2Error::from)
    }
    fn headers(&self) -> Result<HeaderMap, B2Error> {
        let mut map = HeaderMap::new();
        let mut buf = self.content_length.to_string();
        map.append("Authorization", self.url.auth_token());
        map.append("X-Bz-File-Name", self.file_name.try_into()?);
        map.append("Content-Type", self.content_type.try_into()?);
        map.append("Content-Length", buf.as_str().try_into()?);
        map.append("X-Bz-Content-Sha1", self.content_sha1.try_into()?);

        for (key, val) in self.info.as_iter() {
            buf.clear();
            buf.reserve("X-Bz-Info-".len() + key.len());
            buf.push_str("X-Bz-Info-");
            buf.push_str(key);
            map.append(
                HeaderName::from_bytes(buf.as_bytes())?,
                HeaderValue::from_str(val)?,
            );
        }
        Ok(map)
    }
    fn body(&mut self) -> Result<Body, B2Error> {
         Ok(self.body.take().expect("body() called twice on UploadFile"))
    }
    fn finalize(self, fut: ResponseFuture) -> B2Future<UploadUrl> {
        B2Future::new(fut)
    }
    fn error(self, err: B2Error) -> B2Future<UploadUrl> {
        B2Future::err(err)
    }
}
