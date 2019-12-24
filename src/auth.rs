//! Module for authorization.
//!
//! The main types in this module are [`B2Credentials`] and [`B2Authorization`], and the
//! first is used to obtain the latter using the [`AuthorizeAccount`] api call.
//!
//! [`B2Credentials`]: struct.B2Credentials.html
//! [`B2Authorization`]: struct.B2Authorization.html
//! [`AuthorizeAccount`]: struct.AuthorizeAccount.html

use crate::BytesString;
use bytes::Bytes;

use serde::{Deserialize, Serialize};
use base64::encode as b64encode;

use futures::future::FusedFuture;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::B2Error;
use crate::b2_future::B2Future;
use crate::client::ApiCall;
use http::header::{HeaderMap, HeaderValue};
use http::method::Method;
use http::uri::Uri;
use hyper::Body;
use hyper::client::ResponseFuture;

mod capabilities;
mod credentials_deserialize;

pub mod keys;
pub use self::capabilities::Capabilities;

/// The credentials needed to create a [`B2Authorization`].
///
/// [`B2Authorization`]: struct.B2Authorization.html
#[derive(Debug, Clone, Serialize)]
pub struct B2Credentials {
    pub id: BytesString,
    pub key: BytesString,
    #[serde(skip_serializing)]
    auth_string: Bytes,
}
impl B2Credentials {
    pub fn new(id: &str, key: &str) -> B2Credentials {
        let buffer = Bytes::from(format!("{}:{}", id, key));
        let auth_string = Bytes::from(format!("Basic {}", b64encode(&buffer[..])));
        let id = buffer.slice(0..id.len());
        let key = buffer.slice(id.len() + 1 ..);
        B2Credentials {
            id: BytesString::new(id).unwrap(),
            key: BytesString::new(key).unwrap(),
            auth_string,
        }
    }
    pub fn new_shared(id: BytesString, key: BytesString) -> B2Credentials {
        let buffer = format!("{}:{}", id, key);
        let auth_string = Bytes::from(format!("Basic {}", b64encode(&buffer[..])));
        B2Credentials {
            id,
            key,
            auth_string,
        }
    }
    /// Create an api call that tries to authorize using these credentials.
    pub fn authorize(&self) -> AuthorizeAccount<'_> {
        AuthorizeAccount::new(self)
    }
}

/// The [`b2_authorize_account`] api call.
///
/// You can execute this api call using a [`B2Client`], which will result in an
/// [`B2Authorization`] if successful.
///
/// [`b2_authorize_account`]: https://www.backblaze.com/b2/docs/b2_authorize_account.html
/// [`B2Client`]: ../client/struct.B2Client.html
/// [`B2Authorization`]: struct.B2Authorization.html
#[derive(Copy, Clone, Debug)]
pub struct AuthorizeAccount<'a> {
    creds: &'a B2Credentials,
}
impl<'a> AuthorizeAccount<'a> {
    pub fn new(credentials: &'a B2Credentials) -> Self {
        AuthorizeAccount { creds: credentials }
    }
}
impl<'a> ApiCall for AuthorizeAccount<'a> {
    type Future = AuthFuture;
    const METHOD: Method = Method::GET;
    fn url(&self) -> Result<Uri, B2Error> {
        Ok(Uri::from_static(
                "https://api.backblazeb2.com/b2api/v2/b2_authorize_account"))
    }
    fn headers(&self) -> Result<HeaderMap, B2Error> {
        let mut map = HeaderMap::new();
        let header = HeaderValue::from_maybe_shared(self.creds.auth_string.clone())?;
        map.append("Authorization", header);
        Ok(map)
    }
    fn body(&self) -> Result<Body, B2Error> {
        Ok(Body::empty())
    }
    fn finalize(&self, fut: ResponseFuture) -> AuthFuture {
        AuthFuture {
            future: B2Future::new(fut),
            id: self.creds.id.clone(),
        }
    }
    fn error(&self, err: B2Error) -> AuthFuture {
        AuthFuture {
            future: B2Future::err(err),
            id: self.creds.id.clone(),
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
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Allowed {
    /// The list of capabilities of this authorization.
    pub capabilities: Capabilities,
    /// If set, this authorization is limited to the specified bucket.
    pub bucket_id: Option<BytesString>,
    /// If set, this authorization is limited to the specified bucket.
    pub bucket_name: Option<BytesString>,
    /// If set, this authorization is limited to files within this prefix.
    pub name_prefix: Option<BytesString>,
}

/// A future that resolves to a [`B2Authorization`].
///
/// This future is created by the [`AuthorizeAccount`] api call.
///
/// [`AuthorizeAccount`]: struct.AuthorizeAccount.html
/// [`B2Authorization`]: struct.B2Authorization.html
#[derive(Debug)]
pub struct AuthFuture {
    future: B2Future<B2AuthResponse>,
    id: BytesString,
}
impl Future for AuthFuture {
    type Output = Result<B2Authorization, B2Error>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.future).poll(cx) {
            Poll::Ready(Ok(response)) => {
                let id = if self.id == response.account_id {
                    self.id.clone()
                } else {
                    response.account_id.clone()
                };
                Poll::Ready(B2Authorization::from(id, response))
            }
            Poll::Ready(Err(err)) => Poll::Ready(Err(err)),
            Poll::Pending => Poll::Pending,
        }
    }
}
impl FusedFuture for AuthFuture {
    fn is_terminated(&self) -> bool {
        self.future.is_terminated()
    }
}

/// An authorization for the backblaze b2 api.
///
/// It is typically created using the [`AuthorizeAccount`] api call with a
/// [`B2Credentials`].  This type is internally reference counted, so cloning is cheap.
///
/// [`AuthorizeAccount`]: struct.AuthorizeAccount.html
/// [`B2Credentials`]: struct.B2Credentials.html
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct B2Authorization {
    pub account_id: BytesString,
    pub authorization_token: HeaderValue,
    pub api_url: BytesString,
    pub download_url: BytesString,
    pub recommended_part_size: usize,
    pub absolute_minimum_part_size: usize,
    pub allowed: Allowed,
}
impl B2Authorization {
    fn from(id: BytesString, resp: B2AuthResponse) -> Result<B2Authorization, B2Error> {
        Ok(B2Authorization {
            account_id: id,
            authorization_token: resp.authorization_token.as_header()?,
            api_url: resp.api_url,
            download_url: resp.download_url,
            recommended_part_size: resp.recommended_part_size,
            absolute_minimum_part_size: resp.absolute_minimum_part_size,
            allowed: resp.allowed,
        })
    }
    pub(crate) fn auth_token(&self) -> HeaderValue {
        self.authorization_token.clone()
    }
}
