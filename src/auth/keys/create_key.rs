use crate::BytesString;
use crate::auth::{B2Authorization, Capabilities};
use crate::auth::keys::KeyWithSecret;

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

/// The [`b2_create_key`] api call.
///
/// You can execute this api call using a [`B2Client`], which will result in a
/// [`KeyWithSecret`] if successful.
///
/// # Example
///
/// ```
/// use backblaze_b2::B2Error;
/// use backblaze_b2::auth::{B2Credentials, Capabilities};
/// use backblaze_b2::auth::keys::{KeyWithSecret, CreateKey};
/// use backblaze_b2::client::B2Client;
///
/// #[tokio::main]
/// async fn main() -> Result<(), B2Error> {
///     let mut client = B2Client::new();
///     let creds = B2Credentials::from_file("credentials.txt")?;
///     let auth = client.send(creds.authorize()).await?;
///
///     // Create a new key that expires in a minute.
///     let key: KeyWithSecret = client.send(
///         CreateKey::new(&auth, Capabilities::all(), "rust-test-key")
///             .duration(60)
///     ).await?;
///
///     println!("{:#?}", key);
///
///     Ok(())
/// }
/// ```
///
/// [`b2_create_key`]: https://www.backblaze.com/b2/docs/b2_create_key.html
/// [`B2Client`]: ../../client/struct.B2Client.html
/// [`KeyWithSecret`]: struct.KeyWithSecret.html
#[derive(Clone, Debug)]
pub struct CreateKey<'a> {
    auth: &'a B2Authorization,
    capabilities: Capabilities,
    key_name: &'a str,
    duration: Option<u32>,
    bucket_id: Option<&'a str>,
    name_prefix: Option<&'a str>,
}
impl<'a> CreateKey<'a> {
    /// Create a new api call with the specified capabilities and name.
    ///
    /// There is no requirement that the name be unique, and the name cannot be used to
    /// look up the key. Names can contain letters, numbers, and "-", and are limited to
    /// 100 characters.
    pub fn new(
        auth: &'a B2Authorization,
        capabilities: Capabilities,
        name: &'a str
    ) -> Self {
        CreateKey {
            auth,
            capabilities,
            key_name: name,
            duration: None,
            bucket_id: None,
            name_prefix: None,
        }
    }
    /// When provided, the key will expire after the given number of seconds, and will
    /// have [`expiration_timestamp`] set. Value must be a positive integer, and must be
    /// less than 1000 days (in seconds).
    ///
    /// [`expiration_timestamp`]: struct.KeyWithSecret.html#structfield.expiration_timestamp
    pub fn duration(self, duration_in_seconds: u32) -> Self {
        CreateKey {
            duration: Some(duration_in_seconds),
            ..self
        }
    }
    /// When present, the new key can only access this bucket. When set, only these
    /// capabilities can be specified: `listBuckets`, `listFiles`, `readFiles`,
    /// `shareFiles`, `writeFiles`, and `deleteFiles`.
    pub fn bucket_id(self, bucket_id: &'a str) -> Self {
        CreateKey {
            bucket_id: Some(bucket_id),
            ..self
        }
    }
    /// When present, restricts access to files whose names start with the prefix. You
    /// must set `bucket_id` when setting this.
    pub fn name_prefix(self, name_prefix: &'a str) -> Self {
        CreateKey {
            name_prefix: Some(name_prefix),
            ..self
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateKeyRequest<'a> {
    account_id: &'a BytesString,
    capabilities: &'a Capabilities,
    key_name: &'a str,

    #[serde(skip_serializing_if = "Option::is_none")]
    valid_duration_in_seconds: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    bucket_id: Option<&'a str>,

    #[serde(skip_serializing_if = "Option::is_none")]
    name_prefix: Option<&'a str>,
}

impl<'a> ApiCall for CreateKey<'a> {
    type Future = B2Future<KeyWithSecret>;
    const METHOD: Method = Method::POST;
    fn url(&self) -> Result<Uri, B2Error> {
        Uri::try_from(format!("{}/b2api/v2/b2_create_key", self.auth.api_url))
            .map_err(B2Error::from)
    }
    fn headers(&self) -> Result<HeaderMap, B2Error> {
        let mut map = HeaderMap::new();
        map.append("Authorization", self.auth.auth_token());
        Ok(map)
    }
    fn body(&mut self) -> Result<Body, B2Error> {
        serde_body(&CreateKeyRequest {
            account_id: &self.auth.account_id,
            capabilities: &self.capabilities,
            key_name: self.key_name,
            valid_duration_in_seconds: self.duration,
            bucket_id: self.bucket_id,
            name_prefix: self.name_prefix,
        })
    }
    fn finalize(self, fut: ResponseFuture) -> B2Future<KeyWithSecret> {
        B2Future::new(fut)
    }
    fn error(self, err: B2Error) -> B2Future<KeyWithSecret> {
        B2Future::err(err)
    }
}
