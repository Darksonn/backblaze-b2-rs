//! Application keys are an alternative to using the master application key for
//! authorization.
//!
//! The official documentation on keys can be found [here][1]. This module
//! defines three api calls:
//!
//! 1. [`CreateKey`]
//! 2. [`DeleteKey`]
//! 3. [`ListKeys`]
//!
//! See the documentation for each api call for examples on how to use them.
//!
//! # Example
//!
//! This example shows how to create a new key, authorize with it and use it.
//!
//! ```
//! use backblaze_b2::B2Error;
//! use backblaze_b2::auth::{B2Credentials, Capabilities};
//! use backblaze_b2::auth::keys::{Key, KeyWithSecret, CreateKey, DeleteKey};
//! use backblaze_b2::client::B2Client;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), B2Error> {
//!     let mut client = B2Client::new();
//!     let creds = B2Credentials::from_file("credentials.txt")?;
//!     let auth = client.send(creds.authorize()).await?;
//!
//!     let mut capabilities = Capabilities::empty();
//!     capabilities.delete_keys = true;
//!
//!     // Create the new key.
//!     let key: KeyWithSecret = client.send(
//!         CreateKey::new(&auth, capabilities, "rust-test-key")
//!             .duration(60)
//!     ).await?;
//!
//!     println!("{:#?}", key);
//!
//!     // Authorize using the key.
//!     let key_creds = key.as_credentials();
//!     let key_auth = client.send(key_creds.authorize()).await?;
//!
//!     // Use the new authorization to delete the key.
//!     let deleted: Key = client.send(DeleteKey::new(&key_auth, &key.key_id)).await?;
//!
//!     // Check that we deleted the right key.
//!     assert_eq!(deleted, Key::from(key));
//!
//!     Ok(())
//! }
//! ```
//!
//! [1]: https://www.backblaze.com/b2/docs/application_keys.html
//! [`CreateKey`]: struct.CreateKey.html
//! [`DeleteKey`]: struct.DeleteKey.html
//! [`ListKeys`]: struct.ListKeys.html

use serde::{Deserialize, Serialize};

use crate::auth::{B2Credentials, Capabilities};
use crate::BytesString;

use std::fmt;

mod create_key;
mod delete_key;
mod list_keys;
pub use self::create_key::CreateKey;
pub use self::delete_key::DeleteKey;
pub use self::list_keys::{ListKeys, ListKeysResponse};

/// The secret for an authorization key.
///
/// This type is usually used together with a [`Key`] to create a [`KeyWithSecret`].
///
/// See the module level documentation for examples.
///
/// [`Key`]: struct.Key.html
/// [`KeyWithSecret`]: struct.KeyWithSecret.html
#[derive(Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct Secret(pub BytesString);

impl Secret {
    /// Create a new secret from the provided string.
    pub fn new(secret: String) -> Secret {
        Secret(BytesString::from(secret))
    }
    /// View the secret as a string slice.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl<'de> Deserialize<'de> for Secret {
    /// Deserialize a string into a `Secret`.
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        BytesString::deserialize(deserializer).map(Secret)
    }
}
impl Serialize for Secret {
    /// Serialize this secret as a string.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        BytesString::serialize(&self.0, serializer)
    }
}
impl fmt::Display for Secret {
    /// This is equivalent to just printing the underlying string.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}
impl fmt::Debug for Secret {
    /// This is equivalent to just debug-printing the underlying string.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}
impl From<Secret> for BytesString {
    /// Obtain the underlying `BytesString` from the `Secret`.
    fn from(secret: Secret) -> BytesString {
        secret.0
    }
}
impl From<BytesString> for Secret {
    /// Turn this string into a `Secret`.
    fn from(secret: BytesString) -> Secret {
        Secret(secret)
    }
}

/// An authorization key with its secret application key.
///
/// This value can be created by [`CreateKey`]. It is not possible to retrieve the
/// secret after creation, so you must store it somewhere. If lost, you should create a
/// new application key.
///
/// See the module level documentation for examples.
///
/// [`CreateKey`]: struct.CreateKey.html
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct KeyWithSecret {
    pub account_id: BytesString,
    pub key_name: String,
    #[serde(rename = "applicationKeyId")]
    pub key_id: BytesString,
    pub capabilities: Capabilities,
    pub expiration_timestamp: Option<u64>,
    pub bucket_id: Option<String>,
    pub name_prefix: Option<String>,
    #[serde(rename = "applicationKey")]
    pub secret: Secret,
}
/// An authorization key for which the secret application key isn't known.
///
/// See the module level documentation for examples.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Key {
    pub account_id: BytesString,
    pub key_name: String,
    #[serde(rename = "applicationKeyId")]
    pub key_id: BytesString,
    pub capabilities: Capabilities,
    pub expiration_timestamp: Option<u64>,
    pub bucket_id: Option<String>,
    pub name_prefix: Option<String>,
}
impl KeyWithSecret {
    /// Create the credentials needed to authorize with this key.
    pub fn as_credentials(&self) -> B2Credentials {
        B2Credentials::new_shared(self.key_id.clone(), self.secret.0.clone())
    }
    /// Split this key into the key without the secret and the secret.
    pub fn split(self) -> (Key, Secret) {
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
            self.secret,
        )
    }
}
impl Key {
    /// Add the secret to the key.
    pub fn with_secret(self, secret: Secret) -> KeyWithSecret {
        KeyWithSecret {
            account_id: self.account_id,
            key_name: self.key_name,
            key_id: self.key_id,
            capabilities: self.capabilities,
            expiration_timestamp: self.expiration_timestamp,
            bucket_id: self.bucket_id,
            name_prefix: self.name_prefix,
            secret,
        }
    }
}
impl From<KeyWithSecret> for Key {
    fn from(key: KeyWithSecret) -> Key {
        key.split().0
    }
}

