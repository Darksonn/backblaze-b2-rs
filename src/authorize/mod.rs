//! This module defines various methods and structs used for authenticating on the B2
//! server.
//!
//! Authentication is usually performed by calling the [`authorize`] method on the
//! [`B2Credentials`] struct, which returns a [`B2Authorization`].
//!
//!  [`authorize`]: struct.B2Credentials.html#method.authorize
//!  [`B2Credentials`]: struct.B2Credentials.html
//!  [`B2Authorization`]: struct.B2Authorization.html

use std::fmt;

use base64::{encode as b64encode};
use hyper::{Client, Request};
use futures::{Poll, Future, Async};

use hyper::body::Body;
use hyper::client::connect::Connect;

use bytes::Bytes;
use serde::de::{Visitor, Deserialize, Deserializer, Error, SeqAccess, MapAccess};

use crate::B2Error;
use crate::b2_future::B2Future;
use crate::bytes_string::BytesString;

pub mod keys;
mod capabilities;

pub use self::capabilities::Capabilities;

/// The credentials needed to create a [`B2Authorization`].
///
/// [`B2Authorization`]: struct.B2Authorization.html
#[derive(Debug,Clone,Serialize)]
pub struct B2Credentials {
    pub id: BytesString,
    pub key: BytesString,
    #[serde(skip_serializing)]
    auth_string: Bytes,
}
impl B2Credentials {
    pub fn new_ref(id: &str, key: &str) -> B2Credentials {
        let buffer = Bytes::from(format!("{}:{}", id, key));
        let auth_string = Bytes::from(format!("Basic {}", b64encode(&buffer[..])));
        let id = buffer.slice(0, id.len());
        let key = buffer.slice_from(id.len()+1);
        B2Credentials {
            id: BytesString::new(id).unwrap(),
            key: BytesString::new(key).unwrap(),
            auth_string,
        }
    }
    pub fn new(id: BytesString, key: BytesString) -> B2Credentials {
        let buffer = format!("{}:{}", id, key);
        let auth_string = Bytes::from(format!("Basic {}", b64encode(&buffer[..])));
        B2Credentials {
            id,
            key,
            auth_string,
        }
    }
    /// This method performs a [b2_authorize_account][1] api call to the backblaze api and
    /// returns a future resolving to an authorization token.
    ///
    /// # Errors
    /// The future resolves to a [`B2Error`] in case something goes wrong. Besides the
    /// standard non-authorization errors, this can fail with [`is_credentials_issue`].
    ///
    ///  [1]: https://www.backblaze.com/b2/docs/b2_authorize_account.html
    ///  [`is_credentials_issue`]: ../../enum.B2Error.html#method.is_credentials_issue
    ///  [`B2Error`]: ../../enum.B2Error.html
    pub fn authorize<C>(&self, client: &Client<C, Body>) -> B2AuthFuture
    where
        C: Connect + Sync + 'static,
        C::Transport: 'static,
        C::Future: 'static,
    {
        let mut request = Request::get(
            "https://api.backblazeb2.com/b2api/v2/b2_authorize_account");
        request.header("Authorization", self.auth_string.clone());

        let request = match request.body(Body::empty()) {
            Ok(req) => req,
            Err(err) => return B2AuthFuture {
                future: B2Future::err(err),
                id: self.id.clone(),
            },
        };

        let future = client.request(request);

        B2AuthFuture {
            future: B2Future::new(future),
            id: self.id.clone(),
        }
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct B2AuthResponse {
    account_id: BytesString,
    authorization_token: BytesString,
    allowed: Allowed,
    api_url: BytesString,
    download_url: BytesString,
    recommended_part_size: usize,
    absolute_minimum_part_size: usize,
}
/// Describes what a certain authorization is allowed to do.
///
/// If the bucket is set, the authorization can only access that bucket, and if
/// `name_prefix` is set, it can only access files with that prefix.
#[derive(Deserialize,Clone,Debug)]
#[serde(rename_all = "camelCase")]
pub struct Allowed {
    pub capabilities: Capabilities,
    pub bucket_id: Option<BytesString>,
    pub bucket_name: Option<BytesString>,
    pub name_prefix: Option<BytesString>,
}

/// A future that resolves to a [`B2Authorization`].
///
/// This future is typically created by the [`authorize`] method.
///
/// [`authorize`]: struct.B2Credentials.html#method.authorize
/// [`B2Authorization`]: struct.B2Authorization.html
pub struct B2AuthFuture {
    future: B2Future<B2AuthResponse>,
    id: BytesString,
}
impl Future for B2AuthFuture {
    type Item = B2Authorization;
    type Error = B2Error;
    fn poll(&mut self) -> Poll<B2Authorization, B2Error> {
        match self.future.poll() {
            Ok(Async::Ready(response)) => {
                if self.id == response.account_id {
                    Ok(Async::Ready(B2Authorization::from(self.id.clone(), response)))
                } else {
                    Ok(Async::Ready(B2Authorization::from(response.account_id.clone(),
                    response)))
                }
            },
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(err) => Err(err),
        }
    }
}

/// This struct contains the needed authorization to perform any b2 api call.
///
/// It is typically created using the [`authorize`] method on [`B2Credentials`]. This type
/// is internally reference counted, so cloning is cheap.
///
///  [`authorize`]: struct.B2Credentials.html#method.authorize
///  [`B2Credentials`]: struct.B2Credentials.html
#[derive(Clone,Debug)]
pub struct B2Authorization {
    pub account_id: BytesString,
    pub authorization_token: BytesString,
    pub api_url: BytesString,
    pub download_url: BytesString,
    pub recommended_part_size: usize,
    pub absolute_minimum_part_size: usize,
    pub allowed: Allowed,
}
impl B2Authorization {
    fn from(id: BytesString, resp: B2AuthResponse) -> B2Authorization {
        B2Authorization {
            account_id: id,
            authorization_token: resp.authorization_token,
            api_url: resp.api_url,
            download_url: resp.download_url,
            recommended_part_size: resp.recommended_part_size,
            absolute_minimum_part_size: resp.absolute_minimum_part_size,
            allowed: resp.allowed,
        }
    }
    pub(crate) fn auth_token(&self) -> Bytes {
        self.authorization_token.clone().into()
    }
}

struct B2CredentialsVisitor;
#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "lowercase")]
enum B2CredentialsField { Id, Key }

impl<'de> Visitor<'de> for B2CredentialsVisitor {
    type Value = B2Credentials;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an object with id and key")
    }

    fn visit_seq<V>(self, mut seq: V) -> Result<B2Credentials, V::Error>
    where
        V: SeqAccess<'de>,
    {
        let id = seq.next_element()?
            .ok_or_else(|| Error::invalid_length(0, &self))?;
        let key = seq.next_element()?
            .ok_or_else(|| Error::invalid_length(1, &self))?;
        Ok(B2Credentials::new(id, key))
    }

    fn visit_map<V>(self, mut map: V) -> Result<B2Credentials, V::Error>
    where
        V: MapAccess<'de>,
    {
        let mut id = None;
        let mut key = None;
        while let Some(field) = map.next_key()? {
            match field {
                B2CredentialsField::Id => {
                    if id.is_some() {
                        return Err(Error::duplicate_field("id"));
                    }
                    id = Some(map.next_value()?);
                }
                B2CredentialsField::Key => {
                    if key.is_some() {
                        return Err(Error::duplicate_field("key"));
                    }
                    key = Some(map.next_value()?);
                }
            }
        }
        let id = id.ok_or_else(|| Error::missing_field("id"))?;
        let key = key.ok_or_else(|| Error::missing_field("keys"))?;
        Ok(B2Credentials::new(id, key))
    }
}
impl<'de> Deserialize<'de> for B2Credentials {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        const FIELDS: &'static [&'static str] = &["id", "key"];
        deserializer.deserialize_struct("B2Credentials", FIELDS, B2CredentialsVisitor)
    }
}

