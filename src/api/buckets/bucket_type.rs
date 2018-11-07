use serde::de::{self, Deserialize, Deserializer, Visitor};
use serde::ser::{Serialize, Serializer};
use std::fmt;

/// Specifies the type of a bucket on backblaze.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BucketType {
    Public,
    Private,
    Snapshot,
}
impl BucketType {
    /// This function returns the string needed to specify the bucket type to the
    /// backblaze api.
    pub fn as_str(self) -> &'static str {
        match self {
            BucketType::Public => "allPublic",
            BucketType::Private => "allPrivate",
            BucketType::Snapshot => "snapshot",
        }
    }
}
impl std::str::FromStr for BucketType {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, &'static str> {
        match s {
            "allPublic" => Ok(BucketType::Public),
            "allPrivate" => Ok(BucketType::Private),
            "snapshot" => Ok(BucketType::Snapshot),
            _ => Err("Not allPublic, allPrivate or snapshot."),
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
    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        match v.parse::<BucketType>() {
            Err(_) => Err(de::Error::unknown_variant(v, &BUCKET_TYPES)),
            Ok(v) => Ok(v),
        }
    }
}
impl<'de> Deserialize<'de> for BucketType {
    fn deserialize<D>(deserializer: D) -> Result<BucketType, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(BucketTypeVisitor)
    }
}
impl Serialize for BucketType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}
