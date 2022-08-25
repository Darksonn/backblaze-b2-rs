use crate::auth::B2Credentials;
use crate::BytesString;

use serde::de::Deserializer;
use serde::Deserialize;

#[derive(Deserialize)]
struct Helper {
    id: String,
    key: String,
}

impl<'de> Deserialize<'de> for B2Credentials {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Helper::deserialize(deserializer)?;
        let id = BytesString::from(value.id);
        let key = BytesString::from(value.key);
        Ok(B2Credentials::new_shared(id, key))
    }
}
