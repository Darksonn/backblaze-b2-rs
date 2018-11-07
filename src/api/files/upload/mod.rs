//! Upload files to backblaze.
use serde_json::to_vec;

use hyper::{Client, Request};
use hyper::body::{Body, Payload};
use hyper::client::connect::Connect;

use bytes::Bytes;

use crate::BytesString;
use crate::b2_future::B2Future;
use crate::api::authorize::B2Authorization;
use crate::api::files::File;

pub mod large;

/// The url to upload files to.
///
/// Created by [`get_upload_url`]. Backblaze recomends not using the same upload url
/// simultaneously. If you wish to perform simultaneous uploads, prefer to use one upload
/// url for each thread.
///
/// [`get_upload_url`]: fn.get_upload_url.html
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadUrl {
    pub bucket_id: String,
    pub upload_url: BytesString,
    pub authorization_token: BytesString,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GetUploadUrlRequest<'a> {
    bucket_id: &'a str,
}

/// Get the url for uploading files. This requires the `writeFiles` capability.
///
/// This is done using the [b2_get_upload_url][1] api call.
///
/// [1]: https://www.backblaze.com/b2/docs/b2_get_upload_url.html
pub fn get_upload_url<C>(
    auth: &B2Authorization,
    client: &Client<C, Body>,
    bucket_id: &str,
) -> B2Future<UploadUrl>
where
    C: Connect + Sync + 'static,
    C::Transport: 'static,
    C::Future: 'static,
{
    let url_string: String = format!("{}/b2api/v2/b2_get_upload_url", auth.api_url);
    let mut request = Request::post(url_string);
    request.header("Authorization", auth.auth_token());

    let body = match to_vec(&GetUploadUrlRequest {
        bucket_id,
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

/// Upload a file. This requires the `writeFiles` capability.
///
/// If you are working with a chunked stream such as [`ThrottledRead`] you can use
/// [`wrap_stream`] to create the body needed for this method.
///
/// To upload an [`AsyncRead`] such as a [file][2], turn it into a chunked stream using
/// the [`chunked_stream`] function and use the [`wrap_stream`] function to turn it into
/// something this function accepts.
///
/// This is done using the [b2_upload_file][1] api call.
///
/// [1]: https://www.backblaze.com/b2/docs/b2_upload_file.html
/// [2]: https://docs.rs/tokio/0.1/tokio/fs/struct.File.html
/// [`wrap_stream`]: https://hyper.rs/hyper/master/hyper/struct.Body.html#method.wrap_stream
/// [`ThrottledRead`]: ../../throttle/struct.ThrottledRead.html
/// [`AsyncRead`]: https://docs.rs/tokio-io/0.1/tokio_io/trait.AsyncRead.html
/// [`chunked_stream`]: ../../stream_util/fn.chunked_stream.html
pub fn upload_file<C, B>(
    url: &UploadUrl,
    client: &Client<C, B>,
    file_name: &str,
    body: impl Into<B>,
    content_type: &str,
    content_length: u64,
    content_sha1: &str,
    last_modified_millis: Option<u64>,
    content_disposition: Option<&str>,
) -> B2Future<File>
where
    C: Connect + Sync + 'static,
    C::Transport: 'static,
    C::Future: 'static,
    B: Payload + Send + 'static,
    B::Data: Send,
{
    let mut request = Request::post(Bytes::from(url.upload_url.clone()));
    request.header("Authorization", Bytes::from(url.authorization_token.clone()));
    request.header("X-Bz-File-Name", file_name);
    request.header("Content-Type", content_type);
    request.header("Content-Length", content_length);
    request.header("X-Bz-Content-Sha1", content_sha1);
    if let Some(last_mod) = last_modified_millis {
        request.header("X-Bz-Info-src_last_modified_millis", last_mod);
    }
    if let Some(dispo) = content_disposition {
        request.header("X-Bz-Info-b2-content-disposition", dispo);
    }

    let request = match request.body(body.into()) {
        Ok(req) => req,
        Err(err) => return B2Future::err(err),
    };

    let future = client.request(request);

    B2Future::new(future)
}


/// Upload a file. This requires the `writeFiles` capability.
///
/// If you are working with a chunked stream such as [`ThrottledRead`] you can use
/// [`wrap_stream`] to create the body needed for this method.
///
/// To upload an [`AsyncRead`] such as a [file][2], turn it into a chunked stream using
/// the [`chunked_stream`] function and use the [`wrap_stream`] function to turn it into
/// something this function accepts.
///
/// This is done using the [b2_upload_file][1] api call.
///
/// [1]: https://www.backblaze.com/b2/docs/b2_upload_file.html
/// [2]: https://docs.rs/tokio/0.1/tokio/fs/struct.File.html
/// [`wrap_stream`]: https://hyper.rs/hyper/master/hyper/struct.Body.html#method.wrap_stream
/// [`ThrottledRead`]: ../../throttle/struct.ThrottledRead.html
/// [`AsyncRead`]: https://docs.rs/tokio-io/0.1/tokio_io/trait.AsyncRead.html
/// [`chunked_stream`]: ../../stream_util/fn.chunked_stream.html
pub fn upload_file_info<C, B, InfoName, InfoValue>(
    url: &UploadUrl,
    client: &Client<C, B>,
    file_name: &str,
    body: impl Into<B>,
    content_type: &str,
    content_length: u64,
    content_sha1: &str,
    info: impl IntoIterator<Item = (InfoName, InfoValue)>,
) -> B2Future<File>
where
    C: Connect + Sync + 'static,
    C::Transport: 'static,
    C::Future: 'static,
    B: Payload + Send + 'static,
    B::Data: Send,
    InfoName: std::fmt::Display,
    http::header::HeaderValue: http::HttpTryFrom<InfoValue>,
{
    let mut request = Request::post(Bytes::from(url.upload_url.clone()));
    request.header::<_, Bytes>("Authorization",
                               Bytes::from(url.authorization_token.clone()));
    request.header::<_, &str>("X-Bz-File-Name", file_name);
    request.header::<_, &str>("Content-Type", content_type);
    request.header::<_, u64>("Content-Length", content_length);
    request.header::<_, &str>("X-Bz-Content-Sha1", content_sha1);
    for (key, value) in info {
        request.header::<_, InfoValue>(Bytes::from(format!("X-Bz-Info-{}", key)), value);
    }

    let request = match request.body(body.into()) {
        Ok(req) => req,
        Err(err) => return B2Future::err(err),
    };

    let future = client.request(request);

    B2Future::new(future)
}
