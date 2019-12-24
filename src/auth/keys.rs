//! This module defines various methods for interacting with authorization keys on
//! backblaze.

use serde::{Deserialize, Serialize};

use crate::auth::{B2Authorization, B2Credentials, Capabilities};
use crate::BytesString;

use crate::B2Error;
use crate::b2_future::B2Future;
use crate::client::{ApiCall, serde_body};
use http::header::HeaderMap;
use http::method::Method;
use http::uri::Uri;
use hyper::Body;
use hyper::client::ResponseFuture;
use std::convert::TryFrom;

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
    pub fn as_credentials(&self) -> B2Credentials {
        B2Credentials::new_shared(self.key_id.clone(), self.application_key.clone())
    }
    /// Split this key into the key without the secret and the secret.
    pub fn without_secret(self) -> (Key, BytesString) {
        (
            Key {
                account_id: self.account_id,
                key_name: self.key_name,
                key_id: self.key_id,
                capabilities: self.capabilities,
                expiration_timestamp: self.expiration_timestamp,
                bucket_id: self.bucket_id,
                name_prefix: self.name_prefix,
            },
            self.application_key,
        )
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

/// The [`b2_create_key`] api call.
///
/// You can execute this api call using a [`B2Client`], which will result in a
/// [`KeyWithSecret`] if successful.
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
    /// Set the duration of the validity of this api call in seconds.
    pub fn duration(self, duration_in_seconds: u32) -> Self {
        CreateKey {
            auth: self.auth,
            capabilities: self.capabilities,
            key_name: self.key_name,
            duration: Some(duration_in_seconds),
            bucket_id: self.bucket_id,
            name_prefix: self.name_prefix,
        }
    }
    /// Set the bucket this key is restriced to.
    pub fn bucket_id(self, bucket_id: &'a str) -> Self {
        CreateKey {
            auth: self.auth,
            capabilities: self.capabilities,
            key_name: self.key_name,
            duration: self.duration,
            bucket_id: Some(bucket_id),
            name_prefix: self.name_prefix,
        }
    }
    /// Set the name prefix that this key is restricted to.
    pub fn name_prefix(self, name_prefix: &'a str) -> Self {
        CreateKey {
            auth: self.auth,
            capabilities: self.capabilities,
            key_name: self.key_name,
            duration: self.duration,
            bucket_id: self.bucket_id,
            name_prefix: Some(name_prefix),
        }
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

impl<'a> ApiCall for CreateKey<'a> {
    type Future = B2Future<KeyWithSecret>;
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
    fn body(&self) -> Result<Body, B2Error> {
        serde_body(&CreateKeyRequest {
            account_id: &self.auth.account_id,
            capabilities: self.capabilities,
            key_name: self.key_name,
            valid_duration_in_seconds: self.duration,
            bucket_id: self.bucket_id,
            name_prefix: self.name_prefix,
        })
    }
    fn finalize(&self, fut: ResponseFuture) -> B2Future<KeyWithSecret> {
        B2Future::new(fut)
    }
    fn error(&self, err: B2Error) -> B2Future<KeyWithSecret> {
        B2Future::err(err)
    }
}

/// The [`b2_delete_key`] api call.
///
/// You can execute this api call using a [`B2Client`], which will return the deleted
/// [`Key`].
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
        Uri::try_from(format!("{}/b2api/v2/b2_create_key", self.auth.api_url))
            .map_err(B2Error::from)
    }
    fn headers(&self) -> Result<HeaderMap, B2Error> {
        let mut map = HeaderMap::new();
        map.append("Authorization", self.auth.auth_token());
        Ok(map)
    }
    fn body(&self) -> Result<Body, B2Error> {
        serde_body(&DeleteKeyRequest {
            key_id: self.key_id,
        })
    }
    fn finalize(&self, fut: ResponseFuture) -> B2Future<Key> {
        B2Future::new(fut)
    }
    fn error(&self, err: B2Error) -> B2Future<Key> {
        B2Future::err(err)
    }
}

/// A list of keys.
///
/// This is the return value of the [`ListKeys`] api call, and the `next_key` field
/// contains the value you need to pass to [`start_key_id`] to get more keys.
///
/// [`ListKeys`]: struct.ListKeys.html
/// [`start_key_id`]: struct.ListKeys.html#method.start_key_id
#[derive(Serialize, Deserialize)]
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

/// The [`b2_list_keys`] api call.
///
/// You can execute this api call using a [`B2Client`], which will return a
/// [`ListKeysResponse`].
///
/// [`b2_list_keys`]: https://www.backblaze.com/b2/docs/b2_list_keys.html
/// [`B2Client`]: ../../client/struct.B2Client.html
/// [`ListKeysResponse`]: struct.ListKeysResponse.html
#[derive(Clone, Debug)]
pub struct ListKeys<'a> {
    auth: &'a B2Authorization,
    max_key_count: Option<usize>,
    start_key_id: Option<&'a str>,
}
impl<'a> ListKeys<'a> {
    /// Create a new `b2_list_keys` api call.
    pub fn new(auth: &'a B2Authorization) -> Self {
        ListKeys {
            auth,
            max_key_count: None,
            start_key_id: None
        }
    }
    /// Set the maximum number of keys to return. Defaults to 1000, and the maximum is
    /// 10000.
    ///
    /// If you request more than 1000 keys, this will be billed as if you had requested
    /// 1000 keys at a time.
    pub fn max_key_count(mut self, count: usize) -> Self {
        self.max_key_count = Some(count);
        self
    }
    /// Since not every key can be retrieved in one api call, you can keep going from the
    /// end of a previous api call by passing the `next_key` field of the
    /// [`ListKeysResponse`] to this method.
    ///
    /// [`ListKeysResponse`]: struct.ListKeysResponse.html
    pub fn start_key_id(mut self, key_id: &'a str) -> Self {
        self.start_key_id = Some(key_id);
        self
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ListKeysRequest<'a> {
    account_id: &'a BytesString,

    #[serde(skip_serializing_if = "Option::is_none")]
    max_key_count: Option<usize>,

    #[serde(skip_serializing_if = "Option::is_none")]
    start_application_key_id: Option<&'a str>,
}

impl<'a> ApiCall for ListKeys<'a> {
    type Future = B2Future<ListKeysResponse>;
    const METHOD: Method = Method::POST;
    fn url(&self) -> Result<Uri, B2Error> {
        Uri::try_from(format!("{}/b2api/v2/b2_list_keys", self.auth.api_url))
            .map_err(B2Error::from)
    }
    fn headers(&self) -> Result<HeaderMap, B2Error> {
        let mut map = HeaderMap::new();
        map.append("Authorization", self.auth.auth_token());
        Ok(map)
    }
    fn body(&self) -> Result<Body, B2Error> {
        serde_body(&ListKeysRequest {
            account_id: &self.auth.account_id,
            max_key_count: self.max_key_count,
            start_application_key_id: self.start_key_id,
        })
    }
    fn finalize(&self, fut: ResponseFuture) -> B2Future<ListKeysResponse> {
        B2Future::new(fut)
    }
    fn error(&self, err: B2Error) -> B2Future<ListKeysResponse> {
        B2Future::err(err)
    }
}
