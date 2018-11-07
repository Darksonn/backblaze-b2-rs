use std::fmt;
use serde::de::{self, Visitor, Deserialize, Deserializer};
use serde::ser::{Serialize, Serializer};

/// Specifies the type of a bucket on backblaze.
#[derive(Debug,Clone,Copy,Eq,PartialEq)]
pub enum BucketType {
    Public, Private, Snapshot
}
impl BucketType {
    /// Creates a `BucketType` from a string. The strings are the ones used by the
    /// backblaze api.
    ///
    /// ```
    /// use backblaze_b2::api::buckets::BucketType;
    ///
    /// assert_eq!(BucketType::from_str("allPublic"), Some(BucketType::Public));
    /// assert_eq!(BucketType::from_str("allPrivate"), Some(BucketType::Private));
    /// assert_eq!(BucketType::from_str("snapshot"), Some(BucketType::Snapshot));
    /// assert_eq!(BucketType::from_str("invalid"), None);
    /// ```
    pub fn from_str(s: &str) -> Option<BucketType> {
        match s {
            "allPublic" => Some(BucketType::Public),
            "allPrivate" => Some(BucketType::Private),
            "snapshot" => Some(BucketType::Snapshot),
            _ => None
        }
    }
    /// This function returns the string needed to specify the bucket type to the
    /// backblaze api.
    pub fn as_str(self) -> &'static str {
        match self {
            BucketType::Public => "allPublic",
            BucketType::Private => "allPrivate",
            BucketType::Snapshot => "snapshot"
        }
    }
}
static BUCKET_TYPES: [&'static str; 3] = ["allPublic", "allPrivate", "snapshot"];
struct BucketTypeVisitor;
impl<'de> Visitor<'de> for BucketTypeVisitor {
    type Value = BucketType;
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("allPublic, allPrivate or snapshot")
    }
    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E> where E: de::Error {
        match BucketType::from_str(v) {
            None => Err(de::Error::unknown_variant(v, &BUCKET_TYPES)),
            Some(v) => Ok(v)
        }
    }
}
impl<'de> Deserialize<'de> for BucketType {
    fn deserialize<D>(deserializer: D) -> Result<BucketType, D::Error>
        where D: Deserializer<'de>
    {
        deserializer.deserialize_str(BucketTypeVisitor)
    }
}
impl Serialize for BucketType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        serializer.serialize_str(self.as_str())
    }
}
