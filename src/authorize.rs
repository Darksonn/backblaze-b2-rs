//! This module defines various methods and structs used for authenticating on the B2 server.
//!
//! Authentication is usually performed by calling the [`authorize`] method on the
//! [`B2Credentials`] struct, which returns a [`B2Authorization`].
//!
//!  [`authorize`]: struct.B2Credentials.html#method.authorize
//!  [`B2Credentials`]: struct.B2Credentials.html
//!  [`B2Authorization`]: struct.B2Authorization.html

use std::fmt;
use std::mem;

use base64::{encode as b64encode};
use hyper::{Client, Request, Error as HyperError, StatusCode};
use futures::{Poll, Future, Async, Stream};

use hyper::body::Body;
use hyper::client::connect::Connect;

use B2Error;
use capabilities::Capabilities;
use b2_future::B2Future;

#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct B2Credentials {
    pub id: String,
    pub key: String
}
impl B2Credentials {
    fn id_key(&self) -> String {
        format!("{}:{}", self.id, self.key)
    }
    /// This function returns the value of the Authorization header needed to perform a
    /// b2_authorize_account api call.
    pub fn auth_string(&self) -> String {
        format!("Basic {}", b64encode(&self.id_key()))
    }
    /// This function performs a [b2_authorize_account][1] api call to the backblaze api
    /// and returns an authorization token.
    ///
    /// # Errors
    /// This function returns a [`B2Error`] in case something goes wrong. Besides the
    /// standard non-authorization errors, this function can fail with
    /// [`is_credentials_issue`].
    ///
    ///  [1]: https://www.backblaze.com/b2/docs/b2_authorize_account.html
    ///  [`is_credentials_issue`]: ../../enum.B2Error.html#method.is_credentials_issue
    ///  [`B2Error`]: ../../enum.B2Error.html
    pub fn authorize<C>(self, client: &Client<C, Body>) -> B2AuthFuture
    where
        C: Connect + Sync + 'static,
        C::Transport: 'static,
        C::Future: 'static,
    {
        let mut request = Request::get(
            "https://api.backblazeb2.com/b2api/v1/b2_authorize_account");
        request.header("Authorization", self.auth_string());

        let request = match request.body(Body::empty()) {
            Ok(req) => req,
            Err(err) => return B2AuthFuture {
                future: B2Future::err(err),
                id: self.id,
            },
        };

        let future = client.request(request);

        B2AuthFuture {
            future: B2Future::new(future, 64),
            id: self.id,
        }
    }
    pub fn authorize_blocking<C>(self, client: &Client<C, Body>)
        -> Result<B2Authorization, B2Error>
    where
        C: Connect + Sync + 'static,
        C::Transport: 'static,
        C::Future: 'static,
    {
        use futures::future;
        use tokio::runtime::current_thread::Runtime;
        use std::sync::mpsc::channel;
        let mut exec = Runtime::new()?;
        let (send, recv) = channel();
        let send1 = send;
        let send2 = send1.clone();

        let future = self.authorize(client);

        exec.spawn(
            future
            .map(move |v| {
                let _ = send1.send(Ok(v));
            })
            .map_err(move |e| {
                let _ = send2.send(Err(e));
            })
        );
        exec.run().unwrap();
        match recv.try_recv() {
            Ok(ok) => ok,
            Err(_) => {
                panic!("panic in inner authorize call.");
            }
        }
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct B2AuthResponse {
    authorization_token: String,
    api_url: String,
    download_url: String,
    recommended_part_size: usize,
    absolute_minimum_part_size: usize,
    allowed: Allowed,
}
#[derive(Deserialize,Clone,Debug)]
#[serde(rename_all = "camelCase")]
pub struct Allowed {
    pub capabilities: Capabilities,
    pub bucket_id: Option<String>,
    pub bucket_name: Option<String>,
    pub name_prefix: Option<String>,
}

pub struct B2AuthFuture {
    future: B2Future<B2AuthResponse>,
    id: String,
}
impl Future for B2AuthFuture {
    type Item = B2Authorization;
    type Error = B2Error;
    fn poll(&mut self) -> Poll<B2Authorization, B2Error> {
        match self.future.poll() {
            Ok(Async::Ready(response)) => {
                let id = mem::replace(&mut self.id, String::new());
                Ok(Async::Ready(B2Authorization::from(id, response)))
            },
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(err) => Err(err),
        }
    }
}

/// This struct contains the needed authorization to perform any b2 api call. It is typically
/// created using the [`authorize`] method on [`B2Credentials`].
///
///  [`authorize`]: struct.B2Credentials.html#method.authorize
///  [`B2Credentials`]: struct.B2Credentials.html
#[derive(Debug)]
pub struct B2Authorization {
    pub account_id: String,
    pub authorization_token: String,
    pub api_url: String,
    pub download_url: String,
    pub recommended_part_size: usize,
    pub absolute_minimum_part_size: usize,
    pub allowed: Allowed,
}
impl B2Authorization {
    fn from(id: String, resp: B2AuthResponse) -> B2Authorization {
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
}
