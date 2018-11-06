//! Access to files on backblaze.

use std::collections::HashMap;

use hyper::{Client, Request};
use serde_json::to_vec;

use hyper::body::Body;
use hyper::client::connect::Connect;

use crate::BytesString;
use crate::authorize::B2Authorization;
use crate::b2_future::B2Future;

pub mod upload;
pub mod download;
mod action;
pub use self::action::Action;

/// Describes a file on backblaze.
#[derive(Deserialize,Serialize,Debug,PartialEq,Eq,Clone)]
#[serde(rename_all = "camelCase")]
pub struct File {
    pub account_id: BytesString,
    pub action: Action,
    pub bucket_id: String,
    pub content_length: usize,
    pub content_sha1: String,
    pub content_type: String,
    pub file_id: String,
    pub file_info: HashMap<String, String>,
    pub file_name: String,
    pub upload_timestamp: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GetFileInfoRequest<'a> {
    file_id: &'a str,
}

/// Get metadata regarding a file on backblaze. This requires the `readFiles` capability.
///
/// This is done using the [b2_get_file_info][1] api call.
///
/// [1]: https://www.backblaze.com/b2/docs/b2_get_file_info.html
pub fn get_file_info<C>(
    auth: &B2Authorization,
    client: &Client<C, Body>,
    file_id: &str,
) -> B2Future<File>
where
    C: Connect + Sync + 'static,
    C::Transport: 'static,
    C::Future: 'static,
{
    let url_string: String = format!("{}/b2api/v2/b2_get_file_info", auth.api_url);
    let mut request = Request::post(url_string);
    request.header("Authorization", auth.auth_token());

    let body = match to_vec(&GetFileInfoRequest {
        file_id,
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



#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HideFileRequest<'a> {
    bucket_id: &'a str,
    file_name: &'a str,
}

/// Hide a file on backblaze. This requires the `writeFiles` capability.
///
/// This is done using the [b2_hide_file][1] api call. The future resolves to the file
/// representing the hide marker. Note that this is a different file from the one that
/// was hidden. [Read more][2] about file versions.
///
/// [1]: https://www.backblaze.com/b2/docs/b2_hide_file.html
/// [2]: https://www.backblaze.com/b2/docs/file_versions.html
pub fn hide_file<C>(
    auth: &B2Authorization,
    client: &Client<C, Body>,
    bucket_id: &str,
    file_name: &str,
) -> B2Future<File>
where
    C: Connect + Sync + 'static,
    C::Transport: 'static,
    C::Future: 'static,
{
    let url_string: String = format!("{}/b2api/v2/b2_hide_file", auth.api_url);
    let mut request = Request::post(url_string);
    request.header("Authorization", auth.auth_token());

    let body = match to_vec(&HideFileRequest {
        bucket_id,
        file_name,
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



/// The response of the [`delete_file`] api call.
///
/// [`delete_file`]: fn.delete_file.html
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeletedFile {
    pub file_id: String,
    pub file_name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DeleteFileRequest<'a> {
    file_id: &'a str,
    file_name: &'a str,
}

/// Delete a file on backblaze. This requires the `deleteFiles` capability.
///
/// This is done using the [b2_delete_file_version][1] api call. The future resolves to
/// the deleted file.
///
/// [1]: https://www.backblaze.com/b2/docs/b2_delete_file_version.html
pub fn delete_file<C>(
    auth: &B2Authorization,
    client: &Client<C, Body>,
    file_id: &str,
    file_name: &str,
) -> B2Future<DeletedFile>
where
    C: Connect + Sync + 'static,
    C::Transport: 'static,
    C::Future: 'static,
{
    let url_string: String = format!("{}/b2api/v2/b2_delete_file_version", auth.api_url);
    let mut request = Request::post(url_string);
    request.header("Authorization", auth.auth_token());

    let body = match to_vec(&DeleteFileRequest {
        file_id,
        file_name,
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

/// The return value of [`list_file_versions`].
///
/// [`list_file_versions`]: fn.list_file_versions.html
#[derive(Deserialize)]
pub struct ListFileVersionsResponse {
    pub files: Vec<File>,
    pub next_file_name: Option<String>,
    pub next_file_id: Option<String>,
}
impl IntoIterator for ListFileVersionsResponse {
    type Item = File;
    type IntoIter = std::vec::IntoIter<File>;
    fn into_iter(self) -> Self::IntoIter {
        self.files.into_iter()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ListFileVersionsRequest<'a> {
    bucket_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    start_file_name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    start_file_id: Option<&'a str>,
    max_file_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    prefix: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    delimeter: Option<&'a str>,
}


/// Lists the file versions in a bucket. Requires the `listFiles` capability.
///
/// This is done using the [b2_list_file_versions][1] api call.
///
/// [1]: https://www.backblaze.com/b2/docs/b2_list_file_versions.html
pub fn list_file_versions<C>(
    auth: &B2Authorization,
    client: &Client<C, Body>,
    bucket_id: &str,
    max_file_count: usize,
    start_file_name: Option<&str>,
    start_file_id: Option<&str>,
    prefix: Option<&str>,
    delimeter: Option<&str>,
) -> B2Future<ListFileVersionsResponse>
where
    C: Connect + Sync + 'static,
    C::Transport: 'static,
    C::Future: 'static,
{
    let url_string: String = format!("{}/b2api/v2/b2_list_file_versions", auth.api_url);
    let mut request = Request::post(url_string);
    request.header("Authorization", auth.auth_token());

    let body = match to_vec(&ListFileVersionsRequest {
        bucket_id,
        start_file_name,
        start_file_id,
        max_file_count,
        prefix,
        delimeter,
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



/// The return value of [`list_file_names`].
///
/// [`list_file_names`]: fn.list_file_names.html
#[derive(Deserialize)]
pub struct ListFileNamesResponse {
    pub files: Vec<File>,
    pub next_file_name: Option<String>,
}
impl IntoIterator for ListFileNamesResponse {
    type Item = File;
    type IntoIter = std::vec::IntoIter<File>;
    fn into_iter(self) -> Self::IntoIter {
        self.files.into_iter()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ListFileNamesRequest<'a> {
    bucket_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    start_file_name: Option<&'a str>,
    max_file_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    prefix: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    delimeter: Option<&'a str>,
}


/// Lists the file names in a bucket. Requires the `listFiles` capability.
///
/// This is done using the [b2_list_file_names][1] api call.
///
/// [1]: https://www.backblaze.com/b2/docs/b2_list_file_names.html
pub fn list_file_names<C>(
    auth: &B2Authorization,
    client: &Client<C, Body>,
    bucket_id: &str,
    max_file_count: usize,
    start_file_name: Option<&str>,
    prefix: Option<&str>,
    delimeter: Option<&str>,
) -> B2Future<ListFileNamesResponse>
where
    C: Connect + Sync + 'static,
    C::Transport: 'static,
    C::Future: 'static,
{
    let url_string: String = format!("{}/b2api/v2/b2_list_file_names", auth.api_url);
    let mut request = Request::post(url_string);
    request.header("Authorization", auth.auth_token());

    let body = match to_vec(&ListFileNamesRequest {
        bucket_id,
        start_file_name,
        max_file_count,
        prefix,
        delimeter,
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
