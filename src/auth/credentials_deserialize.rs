use super::B2Credentials;
use serde::Deserialize;
use serde::de::{Deserializer, Error, MapAccess, SeqAccess, Visitor};
use std::fmt;

struct B2CredentialsVisitor;
#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "lowercase")]
enum B2CredentialsField {
    Id,
    Key,
}

impl<'de> Visitor<'de> for B2CredentialsVisitor {
    type Value = B2Credentials;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an object with id and key")
    }

    fn visit_seq<V>(self, mut seq: V) -> Result<B2Credentials, V::Error>
    where
        V: SeqAccess<'de>,
    {
        let id = seq
            .next_element()?
            .ok_or_else(|| Error::invalid_length(0, &self))?;
        let key = seq
            .next_element()?
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
        const FIELDS: &[&str] = &["id", "key"];
        deserializer.deserialize_struct("B2Credentials", FIELDS, B2CredentialsVisitor)
    }
}
