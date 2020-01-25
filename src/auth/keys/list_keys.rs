use crate::BytesString;
use crate::auth::B2Authorization;
use crate::auth::keys::Key;

use serde::{Serialize, Deserialize};

use crate::B2Error;
use crate::b2_future::B2Future;
use crate::client::{ApiCall, serde_body};
use http::header::HeaderMap;
use http::method::Method;
use http::uri::Uri;
use hyper::Body;
use hyper::client::ResponseFuture;
use std::convert::TryFrom;

/// A list of keys.
///
/// This is the return value of the [`ListKeys`] api call, and the `next_key` field
/// contains the value you need to pass to [`start_key_id`] to get more keys.
///
/// This type can be iterated directly, which is equivalent to iterating the `keys`
/// field.
///
/// # Example
///
/// ```
/// use backblaze_b2::B2Error;
/// use backblaze_b2::auth::B2Credentials;
/// use backblaze_b2::auth::keys::{ListKeys, ListKeysResponse};
/// use backblaze_b2::client::B2Client;
///
/// #[tokio::main]
/// async fn main() -> Result<(), B2Error> {
///     let mut client = B2Client::new();
///     let creds = B2Credentials::from_file("credentials.txt")?;
///     let auth = client.send(creds.authorize()).await?;
///
///     // Since we may not get every key from this api call, we need to loop.
///     let mut keys = Vec::new();
///     // This variable contains the next api call to perform.
///     let mut job = Some(client.send(ListKeys::new(&auth)));
///
///     // Using Option::take, the job variable is replaced with None.
///     while let Some(this_job) = job.take() {
///         // Fetch the next batch of keys.
///         let key_batch = this_job.await?;
///
///         // If there are more keys, create a new job.
///         if let Some(next_key) = key_batch.next_key {
///             job = Some(client.send(
///                 ListKeys::new(&auth).start_key_id(&next_key)
///             ));
///         }
///
///         // Add the batch to keys.
///         keys.extend(key_batch.keys);
///     }
///     println!("{:#?}", keys);
///
///     Ok(())
/// }
/// ```
///
/// [`ListKeys`]: struct.ListKeys.html
/// [`start_key_id`]: struct.ListKeys.html#method.start_key_id
#[derive(Serialize, Deserialize, Debug, Clone)]
#[non_exhaustive]
pub struct ListKeysResponse {
    pub keys: Vec<Key>,
    #[serde(rename = "nextApplicationKeyId")]
    pub next_key: Option<String>,
}
impl IntoIterator for ListKeysResponse {
    type Item = Key;
    type IntoIter = std::vec::IntoIter<Key>;
    /// Create an iterator over the `keys` field.
    fn into_iter(self) -> Self::IntoIter {
        self.keys.into_iter()
    }
}
impl<'a> IntoIterator for &'a ListKeysResponse {
    type Item = &'a Key;
    type IntoIter = std::slice::Iter<'a, Key>;
    /// Create an iterator over the `keys` field.
    fn into_iter(self) -> Self::IntoIter {
        self.keys.iter()
    }
}
impl ListKeysResponse {
    /// Iterate over the `keys` field.
    pub fn iter(&self) -> std::slice::Iter<'_, Key> {
        IntoIterator::into_iter(self)
    }
}

/// The [`b2_list_keys`] api call.
///
/// You can execute this api call using a [`B2Client`], which will return a
/// [`ListKeysResponse`].
///
/// # Example
///
/// ```
/// use backblaze_b2::B2Error;
/// use backblaze_b2::auth::B2Credentials;
/// use backblaze_b2::auth::keys::{ListKeys, ListKeysResponse};
/// use backblaze_b2::client::B2Client;
///
/// #[tokio::main]
/// async fn main() -> Result<(), B2Error> {
///     let mut client = B2Client::new();
///     let creds = B2Credentials::from_file("credentials.txt")?;
///     let auth = client.send(creds.authorize()).await?;
///
///     // Since we may not get every key from this api call, we need to loop.
///     let mut keys = Vec::new();
///     // This variable contains the next api call to perform.
///     let mut job = Some(client.send(ListKeys::new(&auth)));
///
///     // Using Option::take, the job variable is replaced with None.
///     while let Some(this_job) = job.take() {
///         // Fetch the next batch of keys.
///         let key_batch = this_job.await?;
///
///         // If there are more keys, create a new job.
///         if let Some(next_key) = key_batch.next_key {
///             job = Some(client.send(
///                 ListKeys::new(&auth).start_key_id(&next_key)
///             ));
///         }
///
///         // Add the batch to keys.
///         keys.extend(key_batch.keys);
///     }
///     println!("{:#?}", keys);
///
///     Ok(())
/// }
/// ```
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
    fn body(&mut self) -> Result<Body, B2Error> {
        serde_body(&ListKeysRequest {
            account_id: &self.auth.account_id,
            max_key_count: self.max_key_count,
            start_application_key_id: self.start_key_id,
        })
    }
    fn finalize(self, fut: ResponseFuture) -> B2Future<ListKeysResponse> {
        B2Future::new(fut)
    }
    fn error(self, err: B2Error) -> B2Future<ListKeysResponse> {
        B2Future::err(err)
    }
}
