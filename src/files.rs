//! File manipulation.

use serde::{Serialize, Deserialize};
use serde::de::Deserializer;
use std::collections::HashMap;

pub mod upload;
pub mod download;

mod action;
pub use self::action::Action;

mod get_file_info;
mod list_file_names;
pub use self::get_file_info::GetFileInfo;
pub use self::list_file_names::{ListFileNames, ListFileNamesResponse};

/// A file stored on backblaze.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct File {
    /// The account that owns the file.
    pub account_id: String,
    /// The kind of file. See the documentation on [`Action`] for more details.
    ///
    /// [`Action`]: enum.Action.html
    pub action: Action,
    /// The bucket that the file is in.
    pub bucket_id: String,
    /// The unique identifier for this version of this file. This is an empty string if
    /// the action is `Folder`.
    #[serde(deserialize_with = "default_if_null")]
    pub file_id: String,
    /// The name of this file which can be used with [`b2_download_file_by_name`][1].
    ///
    /// [1]: https://www.backblaze.com/b2/docs/b2_download_file_by_name.html
    pub file_name: String,
    /// The number of bytes stored in the file. This is zero if the action is not
    /// `File`.
    #[serde(deserialize_with = "default_if_null")]
    pub content_length: u64,
    /// The sha1 of the file. This is `None` if this is not a file or if the file has no
    /// checksum, which happens for e.g. large files.
    #[serde(deserialize_with = "sha1_deserialize")]
    pub content_sha1: Option<String>,
    /// When the action is `Upload` or `Start`, the MIME type of the file, as
    /// specified when the file was uploaded. For `Hide` action, always
    /// `application/x-bz-hide-marker`. For `Folder` action, always empty string.
    #[serde(deserialize_with = "default_if_null")]
    pub content_type: String,
    /// The custom information that was uploaded with the file.
    pub file_info: HashMap<String, String>,
    /// The UTC timestamp when this file was uploaded.
    pub upload_timestamp: u64,
}

impl File {
    /// Returns `true` if this is an ordinary completed file, and `false` otherwise.
    pub fn is_ordinary_file(&self) -> bool {
        match &self.action {
            Action::Upload => true,
            _ => false,
        }
    }
    /// Returns `true` if this is a hide marker, and `false` otherwise.
    pub fn is_hide_marker(&self) -> bool {
        match &self.action {
            Action::Hide => true,
            _ => false,
        }
    }
    /// Returns `true` if this is an unfinished large file, and `false` otherwise.
    pub fn is_unfinished_large_file(&self) -> bool {
        match &self.action {
            Action::Start => true,
            _ => false,
        }
    }
    /// Returns `true` if this is a virtual folder, and `false` otherwise.
    pub fn is_folder(&self) -> bool {
        match &self.action {
            Action::Folder => true,
            _ => false,
        }
    }
    /// Convenience method for borrowing the sha1.
    pub fn sha1(&self) -> Option<&str> {
        self.content_sha1.as_ref().map(String::as_str)
    }
}

fn default_if_null<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default,
    Option<T>: Deserialize<'de>,
{
    let v: Option<T> = Deserialize::deserialize(d)?;
    Ok(v.unwrap_or_else(Default::default))
}

fn sha1_deserialize<'de, D>(d: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let v: Option<String> = Deserialize::deserialize(d)?;
    Ok(v.and_then(|v| if v == "none" { None } else { Some(v) }))
}
