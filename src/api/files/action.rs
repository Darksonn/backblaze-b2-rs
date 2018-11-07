use serde::de::{self, Deserialize, Deserializer, Visitor};
use serde::ser::{Serialize, Serializer};
use std::fmt;

/// Specifies the type of a file on backblaze.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Action {
    /// Refers to a complete file uploaded to backblaze.
    Upload,
    /// Refers to an incomplete large file.
    Start,
    /// Refers to a file on backblaze which has been hidden.
    Hide,
    /// Folder is used to indicate a virtual folder when listing files.
    Folder,
}
impl Action {
    /// This function returns the string needed to specify the action to the backblaze api.
    pub fn as_str(self) -> &'static str {
        match self {
            Action::Upload => "upload",
            Action::Start => "start",
            Action::Hide => "hide",
            Action::Folder => "folder",
        }
    }
}
impl std::str::FromStr for Action {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, &'static str> {
        match s {
            "upload" => Ok(Action::Upload),
            "start" => Ok(Action::Start),
            "hide" => Ok(Action::Hide),
            "folder" => Ok(Action::Folder),
            _ => Err("Not upload, start, hide or folder."),
        }
    }
}

static ACTIONS: [&'static str; 4] = ["upload", "start", "hide", "folder"];
struct ActionVisitor;
impl<'de> Visitor<'de> for ActionVisitor {
    type Value = Action;
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("upload, start, hide or folder")
    }
    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        match v.parse::<Action>() {
            Err(_) => Err(de::Error::unknown_variant(v, &ACTIONS)),
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
