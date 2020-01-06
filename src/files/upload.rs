//! Upload files to backblaze.

use serde::{Serialize, Deserialize};

mod get_upload_url;
mod upload_info;

pub use self::get_upload_url::GetUploadUrl;
pub use self::upload_info::UploadFileInfo;
pub use self::upload_info::SimpleFileInfo;

/// An url that can be used to upload files to backblaze.
#[derive(Serialize, Deserialize)]
#[non_exhaustive]
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct UploadUrl {
    pub bucket_id: String,
    pub upload_url: String,
    pub authorization_token: String,
}
