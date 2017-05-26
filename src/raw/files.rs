use std::fmt;

use hyper::{self, Client};
use hyper::client::Body;

use serde::{Serialize, Deserialize};
use serde::ser::Serializer;
use serde::de::{self, Visitor, Deserializer};
use serde_json::{self, Value as JsonValue};

use B2Error;
use raw::authorize::B2Authorization;

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
#[derive(Serialize,Deserialize,Debug,Clone)]
#[serde(rename_all = "camelCase")]
pub struct FolderInfo {
    pub file_name: String,
}
#[derive(Serialize,Deserialize,Debug,Clone)]
#[serde(rename_all = "camelCase")]
pub struct HideMarkerInfo {
    pub file_id: String,
    pub file_name: String,
    pub upload_timestamp: u64,
}
#[derive(Serialize,Deserialize,Debug,Clone)]
#[serde(rename_all = "camelCase")]
pub struct UnfinishedLargeFileInfo<InfoType=JsonValue> {
    pub file_id: String,
    pub file_name: String,
    pub content_type: String,
    pub file_info: InfoType,
    pub upload_timestamp: u64,
}
#[derive(Serialize,Deserialize,Debug,Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileNameListing<InfoType=JsonValue> {
    pub files: Vec<FileInfo<InfoType>>,
    pub folders: Vec<FolderInfo>,
}
#[derive(Serialize,Deserialize,Debug,Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileVersionListing<InfoType=JsonValue> {
    pub files: Vec<FileInfo<InfoType>>,
    pub folders: Vec<FolderInfo>,
    pub hide_markers: Vec<HideMarkerInfo>,
    pub unfinished_large_files: Vec<UnfinishedLargeFileInfo<InfoType>>,
}

impl<'a> B2Authorization<'a> {
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
            use std::io::Read;
            println!("{}", resp.chars().map(|r| r.unwrap()).collect::<String>());
            Ok(())
        }
    }
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


#[derive(Debug,Clone,Copy,Eq,PartialEq)]
pub enum FileType {
    File, HideMarker
}
impl FileType {
    pub fn from_str(s: &str) -> Option<FileType> {
        match s {
            "upload" => Some(FileType::File),
            "hide" => Some(FileType::HideMarker),
            _ => None
        }
    }
    pub fn as_str(&self) -> &'static str {
        match *self {
            FileType::File => "upload",
            FileType::HideMarker => "hide"
        }
    }
    pub fn as_file_folder_type(&self) -> FileFolderType {
        match *self {
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
#[derive(Debug,Clone,Copy)]
pub enum FileFolderType {
    File, HideMarker, Folder
}
impl FileFolderType {
    pub fn from_str(s: &str) -> Option<FileFolderType> {
        match s {
            "upload" => Some(FileFolderType::File),
            "hide" => Some(FileFolderType::HideMarker),
            "folder" => Some(FileFolderType::Folder),
            _ => None
        }
    }
    pub fn as_str(&self) -> &'static str {
        match *self {
            FileFolderType::File => "upload",
            FileFolderType::HideMarker => "hide",
            FileFolderType::Folder => "folder"
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



