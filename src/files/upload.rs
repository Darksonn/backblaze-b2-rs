//! Upload files to backblaze.

use serde::{Serialize, Deserialize};
use http::header::HeaderValue;

mod get_upload_url;
mod upload_file;
mod upload_info;

pub use self::get_upload_url::GetUploadUrl;
pub use self::upload_info::UploadFileInfo;
pub use self::upload_info::SimpleFileInfo;
pub use self::upload_file::UploadFile;

/// An url that can be used to upload files to backblaze.
#[derive(Serialize, Deserialize)]
#[non_exhaustive]
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct UploadUrl {
    pub bucket_id: String,
    pub upload_url: String,
    #[serde(with = "crate::header_serde")]
    pub authorization_token: HeaderValue,
}

impl UploadUrl {
    fn auth_token(&self) -> HeaderValue {
        self.authorization_token.clone()
    }
}
