use serde::de::{self, Deserialize, Deserializer, Visitor};
use serde::ser::{Serialize, Serializer};
use std::convert::Infallible;
use std::fmt;

/// Specifies the type of a bucket on backblaze.
#[derive(Debug, Clone, Eq, PartialEq)]
#[non_exhaustive]
pub enum Action {
    /// Refers to a complete file uploaded to backblaze.
    Upload,
    /// Refers to an incomplete large file.
    Start,
    /// Refers to a file on backblaze which has been hidden.
    Hide,
    /// Folder is used to indicate a virtual folder when listing files.
    Folder,
    /// The b2 api may add new types in the future.
    Other(String),
}
impl Action {
    /// This function returns the string needed to specify the action to the
    /// backblaze api.
    pub fn as_str(&self) -> &str {
        match self {
            Action::Upload => "upload",
            Action::Start => "start",
            Action::Hide => "hide",
            Action::Folder => "folder",
            Action::Other(s) => s.as_str(),
        }
    }
}
impl From<String> for Action {
    fn from(s: String) -> Action {
        match s.as_str() {
            "upload" => Action::Upload,
            "start" => Action::Start,
            "hide" => Action::Hide,
            "folder" => Action::Folder,
            _ => Action::Other(s),
        }
    }
}
impl std::str::FromStr for Action {
    type Err = Infallible;
    /// Try to convert a string into a `Action`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "upload" => Ok(Action::Upload),
            "start" => Ok(Action::Start),
            "hide" => Ok(Action::Hide),
            "folder" => Ok(Action::Folder),
            _ => Ok(Action::Other(s.to_string())),
        }
    }
}

struct ActionVisitor;
impl<'de> Visitor<'de> for ActionVisitor {
    type Value = Action;
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("upload, start, hide, or folder")
    }
    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(Action::from(v))
    }
    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        match v.parse::<Action>() {
            Err(i) => match i {},
            Ok(v) => Ok(v),
        }
    }
}
impl<'de> Deserialize<'de> for Action {
    fn deserialize<D>(deserializer: D) -> Result<Action, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(ActionVisitor)
    }
}
impl Serialize for Action {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}
