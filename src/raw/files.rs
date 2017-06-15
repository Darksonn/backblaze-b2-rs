//! This module defines various functions on the [B2Authorization][1] type for interacting with the
//! files on backblaze. There are also various structs defined for storing information about files
//! on backblaze.
//!
//!  [1]: ../authorize/struct.B2Authorization.html

use std::fmt;

use hyper::{self, Client};
use hyper::client::Body;

use serde::{Serialize, Deserialize};
use serde::ser::Serializer;
use serde::de::{self, Visitor, Deserializer};
use serde_json::{self, Value as JsonValue};

use B2Error;
use raw::authorize::B2Authorization;

/// Contains information for a b2 file.
/// This struct is returned by the function get_file_info and the functions for uploading files.
/// This struct contains more information about the file compared to the FileInfo struct.
#[derive(Serialize,Deserialize,Debug,Clone)]
#[serde(rename_all = "camelCase")]
pub struct MoreFileInfo<InfoType=JsonValue> {
    pub file_id: String,
    pub file_name: String,
    pub account_id: String,
    pub content_sha1: String,
    pub bucket_id: String,
    pub content_length: u64,
    pub content_type: String,
    pub file_info: InfoType,
    pub action: FileType,
    pub upload_timestamp: u64,
}
impl<IT> Into<FileInfo<IT>> for MoreFileInfo<IT> {
    fn into(self) -> FileInfo<IT> {
        FileInfo {
            file_id: self.file_id,
            file_name: self.file_name,
            content_length: self.content_length,
            content_type: self.content_type,
            content_sha1: self.content_sha1,
            file_info: self.file_info,
            upload_timestamp: self.upload_timestamp,
        }
    }
}
/// Contains information for a b2 file.
/// This struct is returned by the file listing functions and the functions for downloading files.
/// Some other functions return additional information about the file than this struct, and they
/// use the struct MoreFileInfo.
#[derive(Serialize,Deserialize,Debug,Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileInfo<InfoType=JsonValue> {
    pub file_id: String,
    pub file_name: String,
    pub content_length: u64,
    pub content_type: String,
    pub content_sha1: String,
    pub file_info: InfoType,
    pub upload_timestamp: u64,
}
/// Folders are not real objects stored on backblaze b2, but derived from the names of the stored
/// files. This struct is returned by the file listing functions.
#[derive(Serialize,Deserialize,Debug,Clone)]
#[serde(rename_all = "camelCase")]
pub struct FolderInfo {
    pub file_name: String,
}
/// Contains information about a hide marker. Hide markers are used to mark a filename as not used
/// without deleting the old versions.
#[derive(Serialize,Deserialize,Debug,Clone)]
#[serde(rename_all = "camelCase")]
pub struct HideMarkerInfo {
    pub file_id: String,
    pub file_name: String,
    pub upload_timestamp: u64,
}
/// Contains information about unfinished large files.
#[derive(Serialize,Deserialize,Debug,Clone)]
#[serde(rename_all = "camelCase")]
pub struct UnfinishedLargeFileInfo<InfoType=JsonValue> {
    pub file_id: String,
    pub file_name: String,
    pub content_type: String,
    pub file_info: InfoType,
    pub upload_timestamp: u64,
}
/// Contains the files and folders returned by the file name listing api.
#[derive(Serialize,Deserialize,Debug,Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileNameListing<InfoType=JsonValue> {
    pub files: Vec<FileInfo<InfoType>>,
    pub folders: Vec<FolderInfo>,
}
/// Contains the files, folders, hide markers and unfinished large files returned by the file
/// version listing api.
#[derive(Serialize,Deserialize,Debug,Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileVersionListing<InfoType=JsonValue> {
    pub files: Vec<FileInfo<InfoType>>,
    pub folders: Vec<FolderInfo>,
    pub hide_markers: Vec<HideMarkerInfo>,
    pub unfinished_large_files: Vec<UnfinishedLargeFileInfo<InfoType>>,
}

/// Methods related to the [files module][1].
///
///  [1]: ../files/index.html
impl<'a> B2Authorization<'a> {
    /// Performs a [b2_get_file_info][1] api call.
    ///
    /// # Errors
    /// This function returns a [`B2Error`] in case something goes wrong. Besides the standard
    /// errors, this function can fail with [`is_file_not_found`].
    ///
    ///  [1]: https://www.backblaze.com/b2/docs/b2_get_file_info.html
    ///  [`B2Error`]: ../authorize/enum.B2Error.html
    ///  [`is_file_not_found`]: ../../enum.B2Error.html#method.is_file_not_found
    pub fn get_file_info<IT>(&self, file_id: &str, client: &Client)
        -> Result<MoreFileInfo<IT>,B2Error>
        where for<'de> IT: Deserialize<'de>
    {
        let url_string: String = format!("{}/b2api/v1/b2_get_file_info", self.api_url);
        let url: &str = &url_string;

        let body: String = format!("{{\"fileId\":\"{}\"}}", file_id);

        let resp = client.post(url)
            .body(Body::BufBody(body.as_bytes(), body.len()))
            .header(self.auth_header())
            .send()?;
        if resp.status != hyper::status::StatusCode::Ok {
            Err(B2Error::from_response(resp))
        } else {
            Ok(serde_json::from_reader(resp)?)
        }
    }
    /// Performs a [b2_list_file_names][1] api call. This function returns at most max_file_count
    /// files.
    ///
    /// In order to list all the files on b2, pass None as start_file_name on the first call to
    /// this function and to subsequent calls pass the Option returned by this function to the next
    /// call of this function, until that Option is None. This is also done by the convenience
    /// function list_all_file_names.
    ///
    /// Filenames hidden by a hide marker are not returned.
    ///
    /// # Errors
    /// This function returns a [`B2Error`] in case something goes wrong. Besides the standard
    /// errors, this function can fail with [`is_bucket_not_found`], [`is_invalid_file_name`],
    /// [`is_prefix_issue`], [`is_invalid_delimiter`].
    ///
    ///  [1]: https://www.backblaze.com/b2/docs/b2_list_file_names.html
    ///  [`B2Error`]: ../authorize/enum.B2Error.html
    ///  [`is_invalid_file_name`]: ../../enum.B2Error.html#method.is_invalid_file_name
    ///  [`is_bucket_not_found`]: ../../enum.B2Error.html#method.is_bucket_not_found
    ///  [`is_prefix_issue`]: ../../enum.B2Error.html#method.is_prefix_issue
    ///  [`is_invalid_delimiter`]: ../../enum.B2Error.html#method.is_invalid_delimiter
    pub fn list_file_names<IT>(&self, bucket_id: &str, start_file_name: Option<&str>, max_file_count: u32,
                               prefix: Option<&str>, delimiter: Option<char>, client: &Client)
        -> Result<(FileNameListing<IT>, Option<String>), B2Error>
        where for<'de> IT: Deserialize<'de>
    {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Request<'a> {
            bucket_id: &'a str,
            start_file_name: Option<&'a str>,
            max_file_count: u32,
            prefix: Option<&'a str>,
            delimiter: Option<char>
        }
        let request = Request {
            bucket_id: bucket_id,
            start_file_name: start_file_name,
            max_file_count: max_file_count,
            prefix: prefix,
            delimiter: delimiter
        };
        let body: String = serde_json::to_string(&request)?;
        let url_string: String = format!("{}/b2api/v1/b2_list_file_names", self.api_url);
        let url: &str = &url_string;
        let resp = client.post(url)
            .body(Body::BufBody(body.as_bytes(), body.len()))
            .header(self.auth_header())
            .send()?;
        if resp.status != hyper::status::StatusCode::Ok {
            Err(B2Error::from_response(resp))
        } else {
            #[derive(Deserialize)]
            #[serde(tag = "action")]
            #[allow(non_camel_case_types)]
            enum LFN<InfoType> {
                #[serde(rename_all = "camelCase")]
                upload {
                    file_id: String,
                    file_name: String,
                    content_length: u64,
                    content_type: String,
                    content_sha1: String,
                    file_info: InfoType,
                    upload_timestamp: u64
                },
                #[serde(rename_all = "camelCase")]
                folder {
                    #[allow(dead_code)]
                    file_name: String,
                }
            }
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct Response<InfoType> {
                files: Vec<LFN<InfoType>>,
                next_file_name: Option<String>,
            }
            let lfns: Response<IT> = serde_json::from_reader(resp)?;
            let mut files = Vec::new();
            let mut folders = Vec::new();
            for lfn in lfns.files {
                match lfn {
                    LFN::folder { file_name } => folders.push(FolderInfo { file_name: file_name }),
                    LFN::upload {
                        file_id,
                        file_name,
                        content_length,
                        content_type,
                        content_sha1,
                        file_info,
                        upload_timestamp
                    } => files.push(FileInfo {
                        file_id: file_id,
                        file_name: file_name,
                        content_length: content_length,
                        content_type: content_type,
                        content_sha1: content_sha1,
                        file_info: file_info,
                        upload_timestamp: upload_timestamp
                    })
                }
            }
            Ok((FileNameListing { files: files, folders: folders }, lfns.next_file_name))
        }
    }
    /// Uses the function [`list_file_names`] several times in order to download a list of all file
    /// names on b2.
    ///
    /// Filenames hidden by a hide marker are not returned.
    ///
    /// # Errors
    /// This function returns a [`B2Error`] in case something goes wrong. Besides the standard
    /// errors, this function can fail with [`is_bucket_not_found`], [`is_prefix_issue`],
    /// [`is_invalid_delimiter`].
    ///
    ///  [`list_file_names`]: #method.list_file_names
    ///  [`B2Error`]: ../authorize/enum.B2Error.html
    ///  [`is_invalid_file_name`]: ../../enum.B2Error.html#method.is_invalid_file_name
    ///  [`is_bucket_not_found`]: ../../enum.B2Error.html#method.is_bucket_not_found
    ///  [`is_prefix_issue`]: ../../enum.B2Error.html#method.is_prefix_issue
    ///  [`is_invalid_delimiter`]: ../../enum.B2Error.html#method.is_invalid_delimiter
    pub fn list_all_file_names<IT>(&self, bucket_id: &str, files_per_request: u32, prefix: Option<&str>,
                                  delimiter: Option<char>, client: &Client)
        -> Result<FileNameListing<IT>, B2Error>
        where for<'de> IT: Deserialize<'de>
    {
        let (mut fnl, mut name) = self.list_file_names(bucket_id, None, files_per_request, prefix,
                                                  delimiter, client)?;
        while name != None {
            let (list, n) = self.list_file_names(bucket_id, name.as_ref().map(|s| s.as_str()),
                files_per_request, prefix, delimiter, client)?;

            fnl.files.extend(list.files);
            fnl.folders.extend(list.folders);
            name = n;
        }
        Ok(fnl)
    }
    /// Performs a [b2_list_file_versions][1] api call. This function returns at most max_file_count
    /// files.
    ///
    /// In order to list all the files on b2, pass None as start_file_name on the first call to
    /// this function and to subsequent calls pass the Option returned by this function to the next
    /// call of this function, until that Option is None. This is also done by the convenience
    /// function list_all_file_names.
    ///
    /// # Errors
    /// This function returns a [`B2Error`] in case something goes wrong. Besides the standard
    /// errors, this function can fail with [`is_bucket_not_found`], [`is_invalid_file_name`],
    /// [`is_prefix_issue`] and [`is_invalid_delimiter`].
    ///
    ///  [1]: https://www.backblaze.com/b2/docs/b2_list_file_versions.html
    ///  [`B2Error`]: ../authorize/enum.B2Error.html
    ///  [`is_invalid_file_name`]: ../../enum.B2Error.html#method.is_invalid_file_name
    ///  [`is_bucket_not_found`]: ../../enum.B2Error.html#method.is_bucket_not_found
    ///  [`is_prefix_issue`]: ../../enum.B2Error.html#method.is_prefix_issue
    ///  [`is_invalid_delimiter`]: ../../enum.B2Error.html#method.is_invalid_delimiter
    pub fn list_file_versions<IT>(&self, bucket_id: &str, start_file_name: Option<&str>,
                                  start_file_id: Option<&str>, max_file_count: u32, prefix: Option<&str>,
                                  delimiter: Option<char>, client: &Client)
        -> Result<(FileVersionListing<IT>, Option<String>, Option<String>), B2Error>
        where for<'de> IT: Deserialize<'de>
    {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Request<'a> {
            bucket_id: &'a str,
            start_file_name: Option<&'a str>,
            start_file_id: Option<&'a str>,
            max_file_count: u32,
            prefix: Option<&'a str>,
            delimiter: Option<char>
        }
        let request = Request {
            bucket_id: bucket_id,
            start_file_name: start_file_name,
            start_file_id: start_file_id,
            max_file_count: max_file_count,
            prefix: prefix,
            delimiter: delimiter
        };
        let body: String = serde_json::to_string(&request)?;
        let url_string: String = format!("{}/b2api/v1/b2_list_file_versions", self.api_url);
        let url: &str = &url_string;
        let resp = client.post(url)
            .body(Body::BufBody(body.as_bytes(), body.len()))
            .header(self.auth_header())
            .send()?;
        if resp.status != hyper::status::StatusCode::Ok {
            Err(B2Error::from_response(resp))
        } else {
            #[derive(Deserialize)]
            #[serde(tag = "action")]
            #[allow(non_camel_case_types)]
            enum LFV<InfoType> {
                #[serde(rename_all = "camelCase")]
                upload {
                    file_id: String,
                    file_name: String,
                    content_length: u64,
                    content_type: String,
                    content_sha1: String,
                    file_info: InfoType,
                    upload_timestamp: u64,
                },
                #[serde(rename_all = "camelCase")]
                start {
                    file_id: String,
                    file_name: String,
                    content_type: String,
                    file_info: InfoType,
                    upload_timestamp: u64,
                },
                #[serde(rename_all = "camelCase")]
                hide {
                    file_id: String,
                    file_name: String,
                    upload_timestamp: u64,
                },
                #[serde(rename_all = "camelCase")]
                folder {
                    file_name: String
                }
            }
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct Response<InfoType> {
                files: Vec<LFV<InfoType>>,
                next_file_name: Option<String>,
                next_file_id: Option<String>,
            }
            let lfns: Response<IT> = serde_json::from_reader(resp)?;
            let mut files = Vec::new();
            let mut folders = Vec::new();
            let mut hides = Vec::new();
            let mut larges = Vec::new();
            for lfn in lfns.files {
                match lfn {
                    LFV::folder { file_name } => folders.push(FolderInfo { file_name: file_name }),
                    LFV::upload {
                        file_id,
                        file_name,
                        content_length,
                        content_type,
                        content_sha1,
                        file_info,
                        upload_timestamp
                    } => files.push(FileInfo {
                        file_id: file_id,
                        file_name: file_name,
                        content_length: content_length,
                        content_type: content_type,
                        content_sha1: content_sha1,
                        file_info: file_info,
                        upload_timestamp: upload_timestamp
                    }),
                    LFV::start {
                        file_id,
                        file_name,
                        content_type,
                        file_info,
                        upload_timestamp,
                    } => larges.push(UnfinishedLargeFileInfo {
                        file_id: file_id,
                        file_name: file_name,
                        content_type: content_type,
                        file_info: file_info,
                        upload_timestamp: upload_timestamp,
                    }),
                    LFV::hide {
                        file_id,
                        file_name,
                        upload_timestamp,
                    } => hides.push(HideMarkerInfo {
                        file_id: file_id,
                        file_name: file_name,
                        upload_timestamp: upload_timestamp,
                    }),
                }
            }
            Ok((FileVersionListing {
                files: files,
                hide_markers: hides,
                unfinished_large_files: larges,
                folders: folders
            }, lfns.next_file_name, lfns.next_file_id))
        }
    }
    /// Uses the function [`list_file_versions`] several times in order to download a list of all file
    /// versions on b2.
    ///
    /// # Errors
    /// This function returns a [`B2Error`] in case something goes wrong. Besides the standard
    /// errors, this function can fail with [`is_bucket_not_found`], [`is_prefix_issue`] and
    /// [`is_invalid_delimiter`].
    ///
    ///  [`list_file_versions`]: #method.list_file_versions
    ///  [`B2Error`]: ../authorize/enum.B2Error.html
    ///  [`is_bucket_not_found`]: ../../enum.B2Error.html#method.is_bucket_not_found
    ///  [`is_prefix_issue`]: ../../enum.B2Error.html#method.is_prefix_issue
    ///  [`is_invalid_delimiter`]: ../../enum.B2Error.html#method.is_invalid_delimiter
    pub fn list_all_file_versions<IT>(&self, bucket_id: &str, files_per_request: u32, prefix: Option<&str>,
                                  delimiter: Option<char>, client: &Client)
        -> Result<FileVersionListing<IT>, B2Error>
        where for<'de> IT: Deserialize<'de>
    {
        let (mut fvl, mut name, mut id) = self.list_file_versions(bucket_id, None, None, files_per_request, prefix,
                                                     delimiter, client)?;
        while name != None || id != None {

            let (list, n, i) = self.list_file_versions(bucket_id, name.as_ref().map(|s| s.as_str()),
                id.as_ref().map(|s| s.as_str()), files_per_request, prefix, delimiter, client)?;

            fvl.files.extend(list.files);
            fvl.folders.extend(list.folders);
            fvl.hide_markers.extend(list.hide_markers);
            fvl.unfinished_large_files.extend(list.unfinished_large_files);
            name = n;
            id = i;
        }
        Ok(fvl)
    }
    /// Performs a [b2_delete_file_version][1] api call.
    ///
    /// This function also works on unfinished large files and hide markers.
    ///
    /// # Errors
    /// This function returns a [`B2Error`] in case something goes wrong. Besides the standard
    /// errors, this function can fail with [`is_file_not_found`].
    ///
    ///  [1]: https://www.backblaze.com/b2/docs/b2_delete_file_version.html
    ///  [`B2Error`]: ../authorize/enum.B2Error.html
    ///  [`is_file_not_found`]: ../../enum.B2Error.html#method.is_file_not_found
    pub fn delete_file_version(&self, file_name: &str, file_id: &str, client: &Client)
        -> Result<(),B2Error>
    {
        let url_string: String = format!("{}/b2api/v1/b2_delete_file_version", self.api_url);
        let url: &str = &url_string;

        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Request<'a> {
            file_name: &'a str,
            file_id: &'a str
        }
        let request = Request {
            file_name: file_name,
            file_id: file_id
        };
        let body: String = serde_json::to_string(&request)?;

        let resp = client.post(url)
            .body(Body::BufBody(body.as_bytes(), body.len()))
            .header(self.auth_header())
            .send()?;
        if resp.status != hyper::status::StatusCode::Ok {
            Err(B2Error::from_response(resp))
        } else {
            Ok(())
        }
    }
    /// Performs a [b2_hide_file][1] api call.
    ///
    /// This function creates a hide marker with the given name.
    ///
    /// # Errors
    /// This function returns a [`B2Error`] in case something goes wrong. Besides the standard
    /// errors, this function can fail with [`is_file_not_found`], [`is_bucket_not_found`],
    /// [`is_file_already_hidden`] and [`is_invalid_file_name`].
    ///
    ///  [1]: https://www.backblaze.com/b2/docs/b2_hide_file.html
    ///  [`B2Error`]: ../authorize/enum.B2Error.html
    ///  [`is_file_not_found`]: ../../enum.B2Error.html#method.is_file_not_found
    ///  [`is_bucket_not_found`]: ../../enum.B2Error.html#method.is_bucket_not_found
    ///  [`is_file_already_hidden`]: ../../enum.B2Error.html#method.is_file_already_hidden
    ///  [`is_invalid_file_name`]: ../../enum.B2Error.html#method.is_invalid_file_name
    pub fn hide_file(&self, file_name: &str, bucket_id: &str, client: &Client)
        -> Result<HideMarkerInfo,B2Error>
    {
        let url_string: String = format!("{}/b2api/v1/b2_hide_file", self.api_url);
        let url: &str = &url_string;

        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Request<'a> {
            file_name: &'a str,
            bucket_id: &'a str
        }
        let request = Request {
            file_name: file_name,
            bucket_id: bucket_id
        };
        let body: String = serde_json::to_string(&request)?;

        let resp = client.post(url)
            .body(Body::BufBody(body.as_bytes(), body.len()))
            .header(self.auth_header())
            .send()?;
        if resp.status != hyper::status::StatusCode::Ok {
            Err(B2Error::from_response(resp))
        } else {
            Ok(serde_json::from_reader(resp)?)
        }
    }
}

/// Specifies if something is a file or a hide marker.
#[derive(Debug,Clone,Copy,Eq,PartialEq)]
pub enum FileType {
    File, HideMarker
}
impl FileType {
    /// Converts the strings "upload" and "hide" into the appropriate enum values.
    pub fn from_str(s: &str) -> Option<FileType> {
        match s {
            "upload" => Some(FileType::File),
            "hide" => Some(FileType::HideMarker),
            _ => None
        }
    }
    /// Converts the enum into the strings "upload" or "hide".
    pub fn as_str(&self) -> &'static str {
        match *self {
            FileType::File => "upload",
            FileType::HideMarker => "hide"
        }
    }
}
impl Into<FileFolderType> for FileType {
    fn into(self) -> FileFolderType {
        match self {
            FileType::File => FileFolderType::File,
            FileType::HideMarker => FileFolderType::HideMarker
        }
    }
}

static FILE_TYPES: [&str; 2] = ["upload", "hide"];
struct FileTypeVisitor;
impl<'de> Visitor<'de> for FileTypeVisitor {
    type Value = FileType;
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("upload or hide")
    }
    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E> where E: de::Error {
        match FileType::from_str(v) {
            None => Err(de::Error::unknown_variant(v, &FILE_TYPES)),
            Some(v) => Ok(v)
        }
    }
    fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E> where E: de::Error {
        match FileType::from_str(v) {
            None => Err(de::Error::unknown_variant(v, &FILE_TYPES)),
            Some(v) => Ok(v)
        }
    }
    fn visit_string<E>(self, v: String) -> Result<Self::Value, E> where E: de::Error {
        match FileType::from_str(&v) {
            None => Err(de::Error::unknown_variant(&v, &FILE_TYPES)),
            Some(v) => Ok(v)
        }
    }
}
impl<'de> Deserialize<'de> for FileType {
    fn deserialize<D>(deserializer: D) -> Result<FileType, D::Error>
        where D: Deserializer<'de>
    {
        deserializer.deserialize_str(FileTypeVisitor)
    }
}
impl Serialize for FileType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        serializer.serialize_str(self.as_str())
    }
}
/// Specifies if something is a file, a hide marker og a folder.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum FileFolderType {
    File, HideMarker, Folder
}
impl FileFolderType {
    /// Converts the strings "upload", "hide" and "folder" into the appropriate enum values.
    pub fn from_str(s: &str) -> Option<FileFolderType> {
        match s {
            "upload" => Some(FileFolderType::File),
            "hide" => Some(FileFolderType::HideMarker),
            "folder" => Some(FileFolderType::Folder),
            _ => None
        }
    }
    /// Converts the enum into the strings "upload", "hide" or "folder".
    pub fn as_str(&self) -> &'static str {
        match *self {
            FileFolderType::File => "upload",
            FileFolderType::HideMarker => "hide",
            FileFolderType::Folder => "folder"
        }
    }
    /// Converts the FileFolderType into a FileType if possible, otherwise returns None.
    pub fn into_file_type(self) -> Option<FileType> {
        match self {
            FileFolderType::File => Some(FileType::File),
            FileFolderType::HideMarker => Some(FileType::HideMarker),
            FileFolderType::Folder => None,
       }
    }
}
static FILE_FOLDER_TYPES: [&str; 3] = ["upload", "hide", "folder"];
struct FileFolderTypeVisitor;
impl<'de> Visitor<'de> for FileFolderTypeVisitor {
    type Value = FileFolderType;
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("upload, hide or folder")
    }
    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E> where E: de::Error {
        match FileFolderType::from_str(v) {
            None => Err(de::Error::unknown_variant(v, &FILE_FOLDER_TYPES)),
            Some(v) => Ok(v)
        }
    }
    fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E> where E: de::Error {
        match FileFolderType::from_str(v) {
            None => Err(de::Error::unknown_variant(v, &FILE_FOLDER_TYPES)),
            Some(v) => Ok(v)
        }
    }
    fn visit_string<E>(self, v: String) -> Result<Self::Value, E> where E: de::Error {
        match FileFolderType::from_str(&v) {
            None => Err(de::Error::unknown_variant(&v, &FILE_FOLDER_TYPES)),
            Some(v) => Ok(v)
        }
    }
}
impl<'de> Deserialize<'de> for FileFolderType {
    fn deserialize<D>(deserializer: D) -> Result<FileFolderType, D::Error>
        where D: Deserializer<'de>
    {
        deserializer.deserialize_str(FileFolderTypeVisitor)
    }
}
impl Serialize for FileFolderType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        serializer.serialize_str(self.as_str())
    }
}

