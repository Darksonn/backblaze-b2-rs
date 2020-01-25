use serde::de::{self, Deserialize, Deserializer, Visitor};
use serde::ser::{Serialize, Serializer};
use std::fmt;
use std::convert::Infallible;

/// Specifies the type of a bucket on backblaze.
#[derive(Debug, Clone, Eq, PartialEq)]
#[non_exhaustive]
pub enum BucketType {
    Public,
    Private,
    Snapshot,
    Other(String)
}
impl BucketType {
    /// This function returns the string needed to specify the bucket type to the
    /// backblaze api.
    pub fn as_str(&self) -> &str {
        match self {
            BucketType::Public => "allPublic",
            BucketType::Private => "allPrivate",
            BucketType::Snapshot => "snapshot",
            BucketType::Other(s) => s.as_str(),
        }
    }
}
impl From<String> for BucketType {
    fn from(s: String) -> BucketType {
        match s.as_str() {
            "allPublic" => BucketType::Public,
            "allPrivate" => BucketType::Private,
            "snapshot" => BucketType::Snapshot,
            _ => BucketType::Other(s),
        }
    }
}
impl std::str::FromStr for BucketType {
    type Err = Infallible;
    /// Try to convert a string into a `BucketType`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "allPublic" => Ok(BucketType::Public),
            "allPrivate" => Ok(BucketType::Private),
            "snapshot" => Ok(BucketType::Snapshot),
            _ => Ok(BucketType::Other(s.to_string())),
        }
    }
}

struct BucketTypeVisitor;
impl<'de> Visitor<'de> for BucketTypeVisitor {
    type Value = BucketType;
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("allPublic, allPrivate or snapshot")
    }
    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(BucketType::from(v))
    }
    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        match v.parse::<BucketType>() {
            Err(i) => match i {},
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
