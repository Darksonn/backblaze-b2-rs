//! This module defines various methods for interacting with authorization keys on
//! backblaze.

use hyper::{Client, Request};
use serde_json::to_vec;

use hyper::body::Body;
use hyper::client::connect::Connect;

use crate::BytesString;
use crate::api::authorize::{Capabilities, B2Credentials, B2Authorization};
use crate::b2_future::B2Future;

/// An authorization key with its secret application key.
///
/// This value can be created by [`create_key`].
///
/// [`create_key`]: fn.create_key.html
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct KeyWithSecret {
    pub account_id: BytesString,
    pub key_name: String,
    #[serde(rename = "applicationKeyId")]
    pub key_id: BytesString,
    /// this is the secret of the key
    pub application_key: BytesString,
    pub capabilities: Capabilities,
    pub expiration_timestamp: Option<u32>,
    pub bucket_id: Option<String>,
    pub name_prefix: Option<String>,
}
/// An authorization key for which the secret application key isn't known.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Key {
    pub account_id: BytesString,
    pub key_name: String,
    #[serde(rename = "applicationKeyId")]
    pub key_id: BytesString,
    pub capabilities: Capabilities,
    pub expiration_timestamp: Option<u32>,
    pub bucket_id: Option<String>,
    pub name_prefix: Option<String>,
}
impl KeyWithSecret {
    /// Create the credentials needed to authorize with this key.
    ///
    /// This doesn't allocate as the relevant values are stored in a [`BytesString`].
    ///
    /// [`BytesString`]: ../struct.BytesString.html
    pub fn as_credentials(&self) -> B2Credentials {
        B2Credentials::new(self.key_id.clone(), self.application_key.clone())
    }
    /// Split this key into the key without the secret and the secret.
    pub fn without_secret(self) -> (Key, BytesString) {
        (Key {
            account_id: self.account_id,
            key_name: self.key_name,
            key_id: self.key_id,
            capabilities: self.capabilities,
            expiration_timestamp: self.expiration_timestamp,
            bucket_id: self.bucket_id,
            name_prefix: self.name_prefix,
        }, self.application_key)
    }
}
impl Key {
    /// Add the secret to the key.
    pub fn with_secret<I: Into<BytesString>>(self, application_key: I) -> KeyWithSecret {
        KeyWithSecret {
            account_id: self.account_id,
            key_name: self.key_name,
            key_id: self.key_id,
            application_key: application_key.into(),
            capabilities: self.capabilities,
            expiration_timestamp: self.expiration_timestamp,
            bucket_id: self.bucket_id,
            name_prefix: self.name_prefix,
        }
    }
}
impl From<KeyWithSecret> for Key {
    fn from(key: KeyWithSecret) -> Key {
        key.without_secret().0
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateKeyRequest<'a> {
    account_id: &'a BytesString,
    capabilities: Capabilities,
    key_name: &'a str,

    #[serde(skip_serializing_if = "Option::is_none")]
    valid_duration_in_seconds: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    bucket_id: Option<&'a str>,

    #[serde(skip_serializing_if = "Option::is_none")]
    name_prefix: Option<&'a str>,
}

/// Create a new authorization key on backblaze. This requires the `writeKeys` capability.
///
/// This is done using the [b2_create_key][1] api call.
///
/// [1]: https://www.backblaze.com/b2/docs/b2_create_key.html
pub fn create_key<C>(
    auth: &B2Authorization,
    client: &Client<C, Body>,
    capabilities: Capabilities,
    key_name: &str,
    valid_duration_in_seconds: Option<u32>,
    bucket_id: Option<&str>,
    name_prefix: Option<&str>,
) -> B2Future<KeyWithSecret>
where
    C: Connect + Sync + 'static,
    C::Transport: 'static,
    C::Future: 'static,
{
    let url_string: String = format!("{}/b2api/v2/b2_create_key", auth.api_url);
    let mut request = Request::post(url_string);
    request.header("Authorization", auth.auth_token());

    let body = match to_vec(&CreateKeyRequest {
        account_id: &auth.account_id,
        capabilities,
        key_name,
        valid_duration_in_seconds,
        bucket_id,
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



#[derive(Serialize)]
struct DeleteKeyRequest<'a> {
    #[serde(rename = "applicationKeyId")]
    key_id: &'a str,
}

/// Delete a authorization key on backblaze. This requires the `deleteKeys` capability.
///
/// This is done using the [b2_delete_key][1] api call.
///
/// [1]: https://www.backblaze.com/b2/docs/b2_delete_key.html
pub fn delete_key<C>(
    auth: &B2Authorization,
    client: &Client<C, Body>,
    key_id: &str,
) -> B2Future<Key>
where
    C: Connect + Sync + 'static,
    C::Transport: 'static,
    C::Future: 'static,
{
    let url_string: String = format!("{}/b2api/v2/b2_delete_key", auth.api_url);
    let mut request = Request::post(url_string);
    request.header("Authorization", auth.auth_token());

    let body = match to_vec(&DeleteKeyRequest {
        key_id,
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



/// The return value of [`list_keys`].
///
/// If `next_key` is `Some` then there are more keys available, and they can be fetched
/// with another call to [`list_keys`].
///
/// [`list_keys`]: fn.list_keys.html
#[derive(Deserialize)]
pub struct ListKeysResponse {
    pub keys: Vec<Key>,
    #[serde(rename = "nextApplicationKeyId")]
    pub next_key: Option<String>,
}
impl IntoIterator for ListKeysResponse {
    type Item = Key;
    type IntoIter = std::vec::IntoIter<Key>;
    fn into_iter(self) -> Self::IntoIter {
        self.keys.into_iter()
    }
}
impl<'a> IntoIterator for &'a ListKeysResponse {
    type Item = &'a Key;
    type IntoIter = std::slice::Iter<'a, Key>;
    fn into_iter(self) -> Self::IntoIter {
        self.keys.iter()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ListKeysRequest<'a> {
    account_id: &'a BytesString,

    #[serde(skip_serializing_if = "Option::is_none")]
    max_key_count: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    start_application_key_id: Option<&'a str>,
}

/// List the available authorization keys. This requires the `listKeys` capability.
///
/// In order to list all keys, first pass `None` to `next_key`, and the return value will
/// contain the value for the next page of keys.
///
/// This is done using the [b2_list_keys][1] api call.
///
/// [1]: https://www.backblaze.com/b2/docs/b2_list_keys.html
pub fn list_keys<C>(
    auth: &B2Authorization,
    client: &Client<C, Body>,
    max_key_count: Option<u32>,
    next_key: Option<&str>,
) -> B2Future<ListKeysResponse>
where
    C: Connect + Sync + 'static,
    C::Transport: 'static,
    C::Future: 'static,
{
    let url_string: String = format!("{}/b2api/v2/b2_list_keys", auth.api_url);
    let mut request = Request::post(url_string);
    request.header("Authorization", auth.auth_token());

    let body = match to_vec(&ListKeysRequest {
        account_id: &auth.account_id,
        max_key_count,
        start_application_key_id: next_key,
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
