use bytes::Bytes;
use std::fmt;
use std::str::{from_utf8, from_utf8_unchecked, Utf8Error};
use http::header::{HeaderValue, InvalidHeaderValue};

use serde::de::{Deserialize, Deserializer};
use serde::ser::{Serialize, Serializer};

/// A wrapper containing a [`Bytes`]. This type is guaranteed to contain valid utf-8.
///
/// [`Bytes`]: https://carllerche.github.io/bytes/bytes/struct.Bytes.html
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BytesString {
    inner: Bytes,
}
impl BytesString {
    /// Creates a `BytesString` from the provided bytes.
    pub fn new(inner: Bytes) -> Result<BytesString, Utf8Error> {
        match from_utf8(&inner[..]) {
            Ok(_) => { /* valid utf-8 */ }
            Err(err) => return Err(err),
        }
        Ok(BytesString { inner })
    }
    /// Get a reference to the inner string.
    pub fn as_str(&self) -> &str {
        unsafe { from_utf8_unchecked(&self.inner[..]) }
    }
    /// This method returns the length of the string.
    pub fn len(&self) -> usize {
        self.inner.len()
    }
    pub(crate) fn as_header(&self) -> Result<HeaderValue, InvalidHeaderValue> {
        HeaderValue::from_maybe_shared(self.inner.clone())
    }
}
impl From<BytesString> for Bytes {
    fn from(v: BytesString) -> Bytes {
        v.inner
    }
}
impl From<String> for BytesString {
    fn from(v: String) -> BytesString {
        // since self is a String, it is guaranteed to contain valid utf-8.
        BytesString {
            inner: Bytes::from(v),
        }
    }
}
impl<'a> From<&'a str> for BytesString {
    fn from(v: &'a str) -> BytesString {
        // since self is a string, it is guaranteed to contain valid utf-8.
        BytesString {
            inner: Bytes::from(v.to_string()),
        }
    }
}
impl fmt::Display for BytesString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), f)
    }
}
impl fmt::Debug for BytesString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}
impl std::ops::Deref for BytesString {
    type Target = str;
    fn deref(&self) -> &str {
        self.as_str()
    }
}
impl Serialize for BytesString {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for BytesString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)
            .map(BytesString::from)
    }
}

