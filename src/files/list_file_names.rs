use crate::auth::B2Authorization;
use crate::files::File;

use serde::{Serialize, Deserialize};

use crate::B2Error;
use crate::b2_future::B2Future;
use crate::client::{ApiCall, serde_body};
use http::header::HeaderMap;
use http::method::Method;
use http::uri::Uri;
use hyper::Body;
use hyper::client::ResponseFuture;
use std::convert::TryFrom;

/// A list of files.
///
/// This is the return value of the [`ListFileNames`] api call, and the `next_file` field
/// contains the value you need to pass to [`start_file_name`] to get more files.
///
/// This type can be iterated directly, which is equivalent to iterating the `files`
/// field.
///
/// [`ListFileNames`]: struct.ListFileNames.html
/// [`start_file_name`]: struct.ListFileNames.html#method.start_file_name
#[derive(Serialize, Deserialize, Debug, Clone)]
#[non_exhaustive]
pub struct ListFileNamesResponse {
    pub files: Vec<File>,
    #[serde(rename = "startFileName")]
    pub next_file: Option<String>,
}
impl IntoIterator for ListFileNamesResponse {
    type Item = File;
    type IntoIter = std::vec::IntoIter<File>;
    /// Create an iterator over the `files` field.
    fn into_iter(self) -> Self::IntoIter {
        self.files.into_iter()
    }
}
impl<'a> IntoIterator for &'a ListFileNamesResponse {
    type Item = &'a File;
    type IntoIter = std::slice::Iter<'a, File>;
    /// Create an iterator over the `files` field.
    fn into_iter(self) -> Self::IntoIter {
        self.files.iter()
    }
}
impl ListFileNamesResponse {
    /// Iterate over the `files` field.
    pub fn iter(&self) -> std::slice::Iter<'_, File> {
        IntoIterator::into_iter(self)
    }
}

/// The [`b2_list_file_names`] api call.
///
/// You can execute this api call using a [`B2Client`], which will return a
/// [`ListFileNamesResponse`].
///
/// [`b2_list_file_names`]: https://www.backblaze.com/b2/docs/b2_list_file_names.html
/// [`B2Client`]: ../client/struct.B2Client.html
/// [`ListFileNamesResponse`]: struct.ListFileNamesResponse.html
#[derive(Clone, Debug)]
pub struct ListFileNames<'a> {
    auth: &'a B2Authorization,
    bucket_id: &'a str,
    start_file_name: Option<&'a str>,
    max_file_count: Option<usize>,
    prefix: Option<&'a str>,
    delimiter: Option<&'a str>,
}
impl<'a> ListFileNames<'a> {
    /// Create a new `b2_list_file_names` api call.
    pub fn new(auth: &'a B2Authorization, bucket_id: &'a str) -> Self {
        ListFileNames {
            auth,
            bucket_id,
            start_file_name: None,
            max_file_count: None,
            prefix: None,
            delimiter: None,
        }
    }
    /// Set the maximum number of files to return. Defaults to 100, and the maximum is
    /// 10000.
    ///
    /// This is a class C transaction, and if you request more than 1000 files, this
    /// will be billed as if you had requested 1000 files at a time.
    ///
    /// See [the official documentation on transaction types][1] for more information.
    ///
    /// [1]: https://www.backblaze.com/b2/b2-transactions-price.html
    pub fn max_file_count(mut self, count: usize) -> Self {
        self.max_file_count = Some(count);
        self
    }
    /// Since not every file can be retrieved in one api call, you can keep going from
    /// the end of a previous api call by passing the `next_file` field of the
    /// [`ListFileNamesResponse`] to this method.
    ///
    /// [`ListFileNamesResponse`]: struct.ListFileNamesResponse.html
    pub fn start_file_name(mut self, file_name: &'a str) -> Self {
        self.start_file_name = Some(file_name);
        self
    }
    /// Files returned will be limited to those with the given prefix. Defaults to
    /// the empty string, which matches all files.
    pub fn prefix(mut self, prefix: &'a str) -> Self {
        self.prefix = Some(prefix);
        self
    }
    /// Please see [the official documentation][1] for details on the use of this
    /// argument.
    ///
    /// [1]: https://www.backblaze.com/b2/docs/b2_list_file_names.html
    pub fn delimiter(mut self, prefix: &'a str) -> Self {
        self.prefix = Some(prefix);
        self
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ListFileNamesRequest<'a> {
    bucket_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    start_file_name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_file_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prefix: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    delimiter: Option<&'a str>,
}

impl<'a> ApiCall for ListFileNames<'a> {
    type Future = B2Future<ListFileNamesResponse>;
    const METHOD: Method = Method::POST;
    fn url(&self) -> Result<Uri, B2Error> {
        Uri::try_from(format!("{}/b2api/v2/b2_list_file_names", self.auth.api_url))
            .map_err(B2Error::from)
    }
    fn headers(&self) -> Result<HeaderMap, B2Error> {
        let mut map = HeaderMap::new();
        map.append("Authorization", self.auth.auth_token());
        Ok(map)
    }
    fn body(&mut self) -> Result<Body, B2Error> {
        serde_body(&ListFileNamesRequest {
            bucket_id: &self.bucket_id,
            start_file_name: self.start_file_name,
            max_file_count: self.max_file_count,
            prefix: self.prefix,
            delimiter: self.delimiter,
        })
    }
    fn finalize(self, fut: ResponseFuture) -> B2Future<ListFileNamesResponse> {
        B2Future::new(fut)
    }
    fn error(self, err: B2Error) -> B2Future<ListFileNamesResponse> {
        B2Future::err(err)
    }
}
