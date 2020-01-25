use crate::auth::B2Authorization;
use crate::auth::keys::Key;

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

/// The [`b2_delete_key`] api call.
///
/// You can execute this api call using a [`B2Client`], which will return the deleted
/// [`Key`].
///
/// # Example
///
/// ```
/// use backblaze_b2::B2Error;
/// use backblaze_b2::auth::{B2Credentials, Capabilities};
/// use backblaze_b2::auth::keys::{Key, KeyWithSecret, CreateKey, DeleteKey};
/// use backblaze_b2::client::B2Client;
///
/// #[tokio::main]
/// async fn main() -> Result<(), B2Error> {
///     let mut client = B2Client::new();
///     let creds = B2Credentials::from_file("credentials.txt")?;
///     let auth = client.send(creds.authorize()).await?;
///
///     // Create a new key.
///     let key: KeyWithSecret = client.send(
///         CreateKey::new(&auth, Capabilities::all(), "rust-test-key")
///             .duration(60)
///     ).await?;
///
///     println!("{:#?}", key);
///
///     // Delete it again.
///     let deleted: Key = client.send(DeleteKey::new(&auth, &key.key_id)).await?;
///
///     assert_eq!(deleted, Key::from(key));
///
///     Ok(())
/// }
/// ```
///
/// [`b2_delete_key`]: https://www.backblaze.com/b2/docs/b2_delete_key.html
/// [`B2Client`]: ../../client/struct.B2Client.html
/// [`Key`]: struct.Key.html
#[derive(Clone, Debug)]
pub struct DeleteKey<'a> {
    auth: &'a B2Authorization,
    key_id: &'a str,
}
impl<'a> DeleteKey<'a> {
    /// Create a new api call with the specified capabilities and name.
    pub fn new(auth: &'a B2Authorization, key_id: &'a str) -> Self {
        DeleteKey {
            auth,
            key_id,
        }
    }
}

#[derive(Serialize)]
struct DeleteKeyRequest<'a> {
    #[serde(rename = "applicationKeyId")]
    key_id: &'a str,
}

impl<'a> ApiCall for DeleteKey<'a> {
    type Future = B2Future<Key>;
    const METHOD: Method = Method::POST;
    fn url(&self) -> Result<Uri, B2Error> {
        Uri::try_from(format!("{}/b2api/v2/b2_delete_key", self.auth.api_url))
            .map_err(B2Error::from)
    }
    fn headers(&self) -> Result<HeaderMap, B2Error> {
        let mut map = HeaderMap::new();
        map.append("Authorization", self.auth.auth_token());
        Ok(map)
    }
    fn body(&mut self) -> Result<Body, B2Error> {
        serde_body(&DeleteKeyRequest {
            key_id: self.key_id,
        })
    }
    fn finalize(self, fut: ResponseFuture) -> B2Future<Key> {
        B2Future::new(fut)
    }
    fn error(self, err: B2Error) -> B2Future<Key> {
        B2Future::err(err)
    }
}

