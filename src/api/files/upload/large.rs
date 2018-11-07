//! Upload large files to backblaze.
use serde_json::to_vec;

use hyper::body::{Body, Payload};
use hyper::client::connect::Connect;
use hyper::{Client, Request};

use bytes::Bytes;

use crate::api::authorize::B2Authorization;
use crate::api::files::File;
use crate::b2_future::B2Future;
use crate::BytesString;

use serde::ser::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StartLargeFileRequest<'a, T: Serialize + 'a> {
    bucket_id: &'a str,
    file_name: &'a str,
    content_type: &'a str,
    file_info: &'a T,
}

/// Create a new large file. This requires the `writeFiles` capability.
///
/// This is done using the [b2_start_large_file][1] api call.
///
/// [1]: https://www.backblaze.com/b2/docs/b2_start_large_file.html
pub fn start_large_file_info<C, T>(
    auth: &B2Authorization,
    client: &Client<C, Body>,
    bucket_id: &str,
    file_name: &str,
    content_type: &str,
    file_info: &T,
) -> B2Future<File>
where
    C: Connect + Sync + 'static,
    C::Transport: 'static,
    C::Future: 'static,
    T: Serialize,
{
    let url_string: String = format!("{}/b2api/v2/b2_start_large_file", auth.api_url);
    let mut request = Request::post(url_string);
    request.header("Authorization", auth.auth_token());

    let body = match to_vec(&StartLargeFileRequest {
        bucket_id,
        file_name,
        content_type,
        file_info,
    }) {
        Ok(body) => body,
        Err(err) => return B2Future::err(err),
    };
    let body = Body::from(body);

    let request = match request.body(body) {
        Ok(req) => req,
        Err(err) => return B2Future::err(err),
    };

    let future = client.request(request);

    B2Future::new(future)
}

/// Create a new large file. This requires the `writeFiles` capability.
///
/// This is done using the [b2_start_large_file][1] api call.
///
/// [1]: https://www.backblaze.com/b2/docs/b2_start_large_file.html
pub fn start_large_file<C>(
    auth: &B2Authorization,
    client: &Client<C, Body>,
    bucket_id: &str,
    file_name: &str,
    content_type: &str,
    content_sha1: Option<&str>,
    last_modified_millis: Option<u64>,
    content_disposition: Option<&str>,
) -> B2Future<File>
where
    C: Connect + Sync + 'static,
    C::Transport: 'static,
    C::Future: 'static,
{
    #[derive(Serialize)]
    struct Info<'a> {
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "large_file_sha1")]
        content_sha1: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "src_last_modified_millis")]
        last_modified_millis: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "b2-content-disposition")]
        content_disposition: Option<&'a str>,
    }

    start_large_file_info(
        auth,
        client,
        bucket_id,
        file_name,
        content_type,
        &Info {
            content_sha1,
            last_modified_millis,
            content_disposition,
        },
    )
}

/// The response of [`cancel_large_file`].
///
/// [`cancel_large_file`]: fn.cancel_large_file.html
#[derive(Deserialize, Serialize, Debug, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CancelLargeFileResponse {
    pub file_id: String,
    pub file_name: String,
    pub bucket_id: String,
    pub account_id: String,
}

/// Cancel the upload of a large file.
///
/// This is done using the [b2_cancel_large_file][1] api call.
///
/// [1]: https://www.backblaze.com/b2/docs/b2_cancel_large_file.html
pub fn cancel_large_file<C>(
    auth: &B2Authorization,
    client: &Client<C, Body>,
    file_id: &str,
) -> B2Future<CancelLargeFileResponse>
where
    C: Connect + Sync + 'static,
    C::Transport: 'static,
    C::Future: 'static,
{
    let url_string: String = format!(
        "{}/b2api/v2/b2_cancel_large_file?fileId={}",
        auth.api_url, file_id
    );
    let mut request = Request::get(url_string);
    request.header("Authorization", auth.auth_token());

    let request = match request.body(Body::empty()) {
        Ok(req) => req,
        Err(err) => return B2Future::err(err),
    };

    let future = client.request(request);

    B2Future::new(future)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FinishLargeFileRequest<'a, T: Serialize + 'a> {
    file_id: &'a str,
    part_sha1_array: &'a [T],
}

/// Finish the upload of a large file.
///
/// The argument `part_sha1_array` must be the list of sha1's of the individual parts of
/// the upload in the correct order.
///
/// This is done using the [b2_finish_large_file][1] api call.
///
/// [1]: https://www.backblaze.com/b2/docs/b2_finish_large_file.html
pub fn finish_large_file<C, Sha1>(
    auth: &B2Authorization,
    client: &Client<C, Body>,
    file_id: &str,
    part_sha1_array: &[Sha1],
) -> B2Future<File>
where
    C: Connect + Sync + 'static,
    C::Transport: 'static,
    C::Future: 'static,
    Sha1: Serialize,
{
    let url_string: String = format!("{}/b2api/v2/b2_finish_large_file", auth.api_url);
    let mut request = Request::post(url_string);
    request.header("Authorization", auth.auth_token());

    let body = match to_vec(&FinishLargeFileRequest {
        file_id,
        part_sha1_array,
    }) {
        Ok(body) => body,
        Err(err) => return B2Future::err(err),
    };

    let request = match request.body(Body::from(body)) {
        Ok(req) => req,
        Err(err) => return B2Future::err(err),
    };

    let future = client.request(request);

    B2Future::new(future)
}

/// The url to upload file parts to.
///
/// Created by [`get_upload_part_url`]. Backblaze recomends not using the same upload url
/// simultaneously. If you wish to perform simultaneous uploads, prefer to use one upload
/// url for each thread.
///
/// [`get_upload_part_url`]: fn.get_upload_part_url.html
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadPartUrl {
    pub file_id: String,
    pub upload_url: BytesString,
    pub authorization_token: BytesString,
}

/// Get the url for uploading parts of the specified large file. This requires the
/// `writeFiles` capability.
///
/// This is done using the [b2_get_upload_part_url][1] api call.
///
/// [1]: https://www.backblaze.com/b2/docs/b2_get_upload_part_url.html
pub fn get_upload_part_url<C>(
    auth: &B2Authorization,
    client: &Client<C, Body>,
    file_id: &str,
) -> B2Future<UploadPartUrl>
where
    C: Connect + Sync + 'static,
    C::Transport: 'static,
    C::Future: 'static,
{
    let url_string: String = format!(
        "{}/b2api/v2/b2_get_upload_part_url?fileId={}",
        auth.api_url, file_id
    );
    let mut request = Request::get(url_string);
    request.header("Authorization", auth.auth_token());

    let request = match request.body(Body::empty()) {
        Ok(req) => req,
        Err(err) => return B2Future::err(err),
    };

    let future = client.request(request);

    B2Future::new(future)
}

/// The response of [`upload_part`].
///
/// [`upload_part`]: fn.upload_part.html
#[derive(Deserialize, Serialize, Debug, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UploadPartResponse {
    pub file_id: String,
    pub part_number: usize,
    pub content_length: u64,
    pub content_sha1: String,
    pub upload_timestamp: i64,
}

/// Upload a part of a large file. This requires the `writeFiles` capability.
///
/// The `content_sha` must be the sha1 of this part of the file. Note that the first part
/// has a `part_number` of 1.
///
/// If you are working with a chunked stream such as [`ThrottledRead`] you can use
/// [`wrap_stream`] to create the body needed for this method.
///
/// To upload an [`AsyncRead`] such as a [file][2], turn it into a chunked stream using
/// the [`chunked_stream`] function and use the [`wrap_stream`] function to turn it into
/// something this function accepts.
///
/// This is done using the [b2_upload_part][1] api call.
///
/// [1]: https://www.backblaze.com/b2/docs/b2_upload_part.html
/// [2]: https://docs.rs/tokio/0.1/tokio/fs/struct.File.html
/// [`wrap_stream`]: https://hyper.rs/hyper/master/hyper/struct.Body.html#method.wrap_stream
/// [`ThrottledRead`]: ../../throttle/struct.ThrottledRead.html
/// [`AsyncRead`]: https://docs.rs/tokio-io/0.1/tokio_io/trait.AsyncRead.html
/// [`chunked_stream`]: ../../stream_util/fn.chunked_stream.html
pub fn upload_part<C, B>(
    url: &UploadPartUrl,
    client: &Client<C, B>,
    part_number: usize,
    body: impl Into<B>,
    content_length: u64,
    content_sha1: &str,
) -> B2Future<UploadPartResponse>
where
    C: Connect + Sync + 'static,
    C::Transport: 'static,
    C::Future: 'static,
    B: Payload + Send + 'static,
    B::Data: Send,
{
    let mut request = Request::post(Bytes::from(url.upload_url.clone()));
    request.header(
        "Authorization",
        Bytes::from(url.authorization_token.clone()),
    );
    request.header("X-Bz-Part-Number", part_number);
    request.header("Content-Length", content_length);
    request.header("X-Bz-Content-Sha1", content_sha1);

    let request = match request.body(body.into()) {
        Ok(req) => req,
        Err(err) => return B2Future::err(err),
    };

    let future = client.request(request);

    B2Future::new(future)
}

/// The return value of [`list_unfinished_large_files`].
///
/// [`list_unfinished_large_files`]: fn.list_unfinished_large_files.html
#[derive(Deserialize)]
pub struct ListUnfinishedLargeFilesResponse {
    pub files: Vec<File>,
    pub next_file_id: Option<String>,
}

impl IntoIterator for ListUnfinishedLargeFilesResponse {
    type Item = File;
    type IntoIter = std::vec::IntoIter<File>;
    fn into_iter(self) -> Self::IntoIter {
        self.files.into_iter()
    }
}
impl<'a> IntoIterator for &'a ListUnfinishedLargeFilesResponse {
    type Item = &'a File;
    type IntoIter = std::slice::Iter<'a, File>;
    fn into_iter(self) -> Self::IntoIter {
        self.files.iter()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ListUnfinishedLargeFilesRequest<'a> {
    bucket_id: &'a str,

    #[serde(skip_serializing_if = "Option::is_none")]
    name_prefix: Option<&'a str>,

    #[serde(skip_serializing_if = "Option::is_none")]
    start_file_id: Option<&'a str>,

    max_file_count: usize,
}

/// Lists the unfinished large files in a bucket. Requires the `listFiles` capability.
///
/// This is done using the [b2_list_unfinished_large_files][1] api call.
///
/// [1]: https://www.backblaze.com/b2/docs/b2_list_unfinished_large_files.html
pub fn list_unfinished_large_files<C>(
    auth: &B2Authorization,
    client: &Client<C, Body>,
    bucket_id: &str,
    max_file_count: usize,
    start_file_id: Option<&str>,
    name_prefix: Option<&str>,
) -> B2Future<ListUnfinishedLargeFilesResponse>
where
    C: Connect + Sync + 'static,
    C::Transport: 'static,
    C::Future: 'static,
{
    let url_string: String = format!("{}/b2api/v2/b2_list_file_versions", auth.api_url);
    let mut request = Request::post(url_string);
    request.header("Authorization", auth.auth_token());

    let body = match to_vec(&ListUnfinishedLargeFilesRequest {
        bucket_id,
        max_file_count,
        start_file_id,
        name_prefix,
    }) {
        Ok(body) => body,
        Err(err) => return B2Future::err(err),
    };
    let body = Body::from(body);

    let request = match request.body(body) {
        Ok(req) => req,
        Err(err) => return B2Future::err(err),
    };

    let future = client.request(request);

    B2Future::new(future)
}
