use std::fmt;
use serde::de::{self, Visitor, Deserialize, Deserializer};
use serde::ser::{Serialize, Serializer};

/// Specifies the type of a file on backblaze.
#[derive(Debug,Clone,Copy,Eq,PartialEq)]
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
    /// Creates an `Action` from a string. The strings are the ones used by the backblaze
    /// api.
    ///
    /// ```
    /// use backblaze_b2::files::Action;
    ///
    /// assert_eq!(Action::from_str("upload"), Some(Action::Upload));
    /// assert_eq!(Action::from_str("start"), Some(Action::Start));
    /// assert_eq!(Action::from_str("hide"), Some(Action::Hide));
    /// assert_eq!(Action::from_str("folder"), Some(Action::Folder));
    /// assert_eq!(Action::from_str("invalid"), None);
    /// ```
    pub fn from_str(s: &str) -> Option<Action> {
        match s {
            "upload" => Some(Action::Upload),
            "start" => Some(Action::Start),
            "hide" => Some(Action::Hide),
            "folder" => Some(Action::Folder),
            _ => None
        }
    }
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
static ACTIONS: [&'static str; 4] = ["upload", "start", "hide", "folder"];
struct ActionVisitor;
impl<'de> Visitor<'de> for ActionVisitor {
    type Value = Action;
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("upload, start, hide or folder")
    }
    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E> where E: de::Error {
        match Action::from_str(v) {
            None => Err(de::Error::unknown_variant(v, &ACTIONS)),
            Some(v) => Ok(v)
        }
    }
}
impl<'de> Deserialize<'de> for Action {
    fn deserialize<D>(deserializer: D) -> Result<Action, D::Error>
        where D: Deserializer<'de>
    {
        deserializer.deserialize_str(ActionVisitor)
    }
}
impl Serialize for Action {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        serializer.serialize_str(self.as_str())
    }
}
