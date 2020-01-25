use crate::auth::B2Authorization;
use crate::files::File;

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

/// The [`b2_get_file_info`] api call.
///
/// You can execute this api call using a [`B2Client`], which will result in a
/// [`File`] if successful.
///
/// [`b2_get_file_info`]: https://www.backblaze.com/b2/docs/b2_get_file_info.html
/// [`B2Client`]: ../client/struct.B2Client.html
/// [`File`]: struct.File.html
#[derive(Clone, Debug)]
pub struct GetFileInfo<'a> {
    auth: &'a B2Authorization,
    file_id: &'a str,
}
impl<'a> GetFileInfo<'a> {
    /// Create a new api call for the specified file.
    pub fn new(auth: &'a B2Authorization, file_id: &'a str) -> Self {
        GetFileInfo {
            auth,
            file_id,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GetFileInfoRequest<'a> {
    file_id: &'a str,
}

impl<'a> ApiCall for GetFileInfo<'a> {
    type Future = B2Future<File>;
    const METHOD: Method = Method::POST;
    fn url(&self) -> Result<Uri, B2Error> {
        Uri::try_from(format!("{}/b2api/v2/b2_get_file_info", self.auth.api_url))
            .map_err(B2Error::from)
    }
    fn headers(&self) -> Result<HeaderMap, B2Error> {
        let mut map = HeaderMap::new();
        map.append("Authorization", self.auth.auth_token());
        Ok(map)
    }
    fn body(&mut self) -> Result<Body, B2Error> {
        serde_body(&GetFileInfoRequest {
            file_id: self.file_id,
        })
    }
    fn finalize(self, fut: ResponseFuture) -> B2Future<File> {
        B2Future::new(fut)
    }
    fn error(self, err: B2Error) -> B2Future<File> {
        B2Future::err(err)
    }
}
