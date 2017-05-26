use hyper::{self, Client};
use hyper::client::Body;

use serde::Deserialize;
use serde_json::{self, Value as JsonValue};

use B2Error;
use raw::authorize::B2Authorization;
use raw::buckets::Bucket;

pub struct B2File<'a, InfoType=JsonValue> {
    pub file_id: String,
    pub file_name: String,
    pub content_length: u64, // max file size is 10 TB
    pub content_type: String,
    pub content_sha1: String,
    pub file_info: InfoType,
    pub upload_timestamp: u64,
    pub bucket: &'a Bucket
}

pub struct B2UnfinishedLargeFile<'a, InfoType=JsonValue> {
    pub file_id: String,
    pub file_name: String,
    pub content_type: String,
    pub content_sha1: String,
    pub file_info: InfoType,
    pub upload_timestamp: u64,
    pub bucket: &'a Bucket
}
pub struct B2HideMarker<'a> {
    pub file_id: String,
    pub file_name: String,
    pub upload_timestamp: u64,
    pub bucket: &'a Bucket
}
pub enum B2FileVersion<'a, InfoType=JsonValue> {
    File(B2File<'a, InfoType>), Large(B2UnfinishedLargeFile<'a, InfoType>), Hide(B2HideMarker<'a>)
}
impl<'a, IT> B2FileVersion<'a, IT> {
    pub fn file_id<'s>(&'s self) -> &'s str {
        match *self {
            B2FileVersion::File(ref file) => &file.file_id,
            B2FileVersion::Large(ref file) => &file.file_id,
            B2FileVersion::Hide(ref file) => &file.file_id
        }
    }
    pub fn file_name<'s>(&'s self) -> &'s str {
        match *self {
            B2FileVersion::File(ref file) => &file.file_name,
            B2FileVersion::Large(ref file) => &file.file_name,
            B2FileVersion::Hide(ref file) => &file.file_name
        }
    }
    pub fn upload_timestamp<'s>(&'s self) -> u64 {
        match *self {
            B2FileVersion::File(ref file) => file.upload_timestamp,
            B2FileVersion::Large(ref file) => file.upload_timestamp,
            B2FileVersion::Hide(ref file) => file.upload_timestamp
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ListFileNamesRequest<'a> {
    bucket_id: &'a str,
    start_file_name: Option<&'a str>,
    max_file_count: u16,
    prefix: Option<&'a str>,
    delimiter: Option<char>
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListFileNamesResponse<InfoType> {
    files: Vec<LFN<InfoType>>,
    next_file_name: Option<String>
}
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


#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ListFileVersionsRequest<'a> {
    bucket_id: &'a str,
    start_file_name: Option<&'a str>,
    start_file_id: Option<&'a str>,
    max_file_count: u16,
    prefix: Option<&'a str>,
    delimiter: Option<char>
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListFileVersionsResponse<InfoType> {
    files: Vec<LFV<InfoType>>,
    next_file_name: Option<String>,
#[allow(dead_code)]
    next_file_id: Option<String>
}
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
        bucket_id: Option<String>
    },
#[serde(rename_all = "camelCase")]
    start {
        file_id: String,
        file_name: String,
        content_type: String,
        content_sha1: String,
        file_info: InfoType,
        upload_timestamp: u64,
        bucket_id: Option<String>
    },
#[serde(rename_all = "camelCase")]
    hide {
        file_id: String,
        file_name: String,
        upload_timestamp: u64,
        bucket_id: Option<String>
    },
#[serde(rename_all = "camelCase")]
    folder {
#[allow(dead_code)]
        file_name: String
    }
}
impl<IT> LFV<IT> {
    fn bucket_id<'a>(&'a self) -> Option<&'a String> {
        match *self {
            LFV::upload { ref bucket_id, .. } => bucket_id.as_ref(),
            LFV::start { ref bucket_id, .. } => bucket_id.as_ref(),
            LFV::hide { ref bucket_id, .. } => bucket_id.as_ref(),
            LFV::folder { .. } => None
        }
    }
    fn to_file_version<'a>(self, bucket: &'a Bucket) -> Option<B2FileVersion<'a, IT>> {
        Some(match self {
            LFV::upload {
                file_id,
                file_name,
                content_length,
                content_type,
                content_sha1,
                file_info,
                upload_timestamp,
                bucket_id: _
            } => {
                B2FileVersion::File(B2File {
                    file_id: file_id,
                    file_name: file_name,
                    content_length: content_length,
                    content_type: content_type,
                    content_sha1: content_sha1,
                    file_info: file_info,
                    upload_timestamp: upload_timestamp,
                    bucket: bucket
                })
            },
            LFV::start {
                file_id,
                file_name,
                content_type,
                content_sha1,
                file_info,
                upload_timestamp,
                bucket_id: _
            } => {
                B2FileVersion::Large(B2UnfinishedLargeFile {
                    file_id: file_id,
                    file_name: file_name,
                    content_type: content_type,
                    content_sha1: content_sha1,
                    file_info: file_info,
                    upload_timestamp: upload_timestamp,
                    bucket: bucket
                })
            },
            LFV::hide {
                file_id,
                file_name,
                upload_timestamp,
                bucket_id: _
            } => {
                B2FileVersion::Hide(B2HideMarker {
                    file_id: file_id,
                    file_name: file_name,
                    upload_timestamp: upload_timestamp,
                    bucket: bucket
                })
            },
            LFV::folder { file_name: _ } => return None
        })
    }
}



impl<'a> B2Authorization<'a> {
    pub fn list_file_names<'b, InfoType>(&self,
        bucket: &'b Bucket,
        start_file_name: Option<&str>,
        max_file_count: u16,
        prefix: Option<&str>,
        delimiter: Option<char>,
        client: &Client
    ) -> Result<(Vec<B2File<'b, InfoType>>, Option<String>), B2Error>
        where for<'de> InfoType: Deserialize<'de>
    {
        let url_string: String = format!("{}/b2api/v1/b2_list_file_names", self.api_url);
        let url: &str = &url_string;

        let body = try!(serde_json::to_string(&ListFileNamesRequest {
            bucket_id: &bucket.bucket_id,
            start_file_name: start_file_name,
            max_file_count: max_file_count,
            prefix: prefix,
            delimiter: delimiter
        }));

        let resp = try!(client.post(url)
            .body(Body::BufBody(body.as_bytes(), body.len()))
            .header(self.auth_header())
            .send());
        if resp.status != hyper::status::StatusCode::Ok {
            Err(B2Error::from_response(resp))
        } else {
            let response: ListFileNamesResponse<InfoType> = try!(serde_json::from_reader(resp));
            let mut vec = Vec::new();
            for lfn in response.files {
                match lfn {
                    LFN::upload {
                        file_id,
                        file_name,
                        content_length,
                        content_type,
                        content_sha1,
                        file_info,
                        upload_timestamp
                    } => {
                        vec.push(B2File {
                            file_id: file_id,
                            file_name: file_name,
                            content_length: content_length,
                            content_type: content_type,
                            content_sha1: content_sha1,
                            file_info: file_info,
                            upload_timestamp: upload_timestamp,
                            bucket: bucket
                        });
                    },
                    LFN::folder { file_name: _ } => {}
                }
            }
            Ok((vec, response.next_file_name))
        }
    }
    pub fn list_all_file_names<'b, InfoType>(&self,
        bucket: &'b Bucket,
        files_per_request: u16,
        prefix: Option<&str>,
        delimiter: Option<char>,
        client: &Client
    ) -> Result<Vec<B2File<'b, InfoType>>, B2Error>
        where for<'de> InfoType: Deserialize<'de>
    {
        let (mut vec, mut next) =
            try!(self.list_file_names(bucket, None, files_per_request, prefix, delimiter, client));
        let mut again = next.is_some();
        while again {
            let next_string = next.take().unwrap(); // we know it's there
            let (morevec, nnext) =
                try!(self.list_file_names(bucket, Some(&next_string),
                files_per_request, prefix, delimiter, client));
            vec.extend(morevec);
            next = nnext;
            again = next.is_some();
        }
        Ok(vec)
    }


    pub fn list_file_versions<'b, InfoType>(&self,
        bucket: &'b Bucket,
        start_file_name: Option<&str>,
        start_file_id: Option<&str>,
        max_file_count: u16,
        prefix: Option<&str>,
        delimiter: Option<char>,
        client: &Client
    ) -> Result<(Vec<B2FileVersion<'b, InfoType>>, Option<String>, Option<String>), B2Error>
        where for<'de> InfoType: Deserialize<'de>
    {
        let url_string: String = format!("{}/b2api/v1/b2_list_file_versions", self.api_url);
        let url: &str = &url_string;

        let body = try!(serde_json::to_string(&ListFileVersionsRequest {
            bucket_id: &bucket.bucket_id,
            start_file_name: start_file_name,
            start_file_id: start_file_id,
            max_file_count: max_file_count,
            prefix: prefix,
            delimiter: delimiter
        }));

        let resp = try!(client.post(url)
            .body(Body::BufBody(body.as_bytes(), body.len()))
            .header(self.auth_header())
            .send());
        if resp.status != hyper::status::StatusCode::Ok {
            Err(B2Error::from_response(resp))
        } else {
            let response: ListFileVersionsResponse<InfoType> = try!(serde_json::from_reader(resp));
            let mut vec = Vec::new();
            for lfv in response.files {
                if let Some(file) = lfv.to_file_version(bucket) {
                    vec.push(file);
                }
            }
            Ok((vec, response.next_file_name, response.next_file_id))
        }
    }
    pub fn list_all_file_versions<'b, InfoType>(&self,
        bucket: &'b Bucket,
        files_per_request: u16,
        prefix: Option<&str>,
        delimiter: Option<char>,
        client: &Client
    ) -> Result<Vec<B2FileVersion<'b, InfoType>>, B2Error>
        where for<'de> InfoType: Deserialize<'de>
    {
        let (mut vec, mut next_name, mut next_id) =
            try!(self.list_file_versions(bucket, None, None, files_per_request, prefix, delimiter, client));
        let mut again = next_name.is_some() && next_id.is_some();
        while again {
            let next1_string = next_name.take().unwrap(); // we know it's there
            let next2_string = next_id.take().unwrap(); // we know it's there
            let (morevec, nnext1, nnext2) =
                try!(self.list_file_versions(bucket, Some(&next1_string),
                Some(&next2_string), files_per_request, prefix,
                delimiter, client));
            vec.extend(morevec);
            next_name = nnext1;
            next_id = nnext2;
            again = next_name.is_some() && next_id.is_some();
        }
        Ok(vec)
    }
    pub fn delete_file_version<IT>(&self, file: &B2File<'a, IT>, client: &Client)
        -> Result<(),B2Error>
    {
        let url_string: String = format!("{}/b2api/v1/b2_delete_file_version", self.api_url);
        let url: &str = &url_string;

        let body: String =
            format!("{{\"fileName\":\"{}\", \"fileId\":\"{}\"}}", file.file_name, file.file_id);

        let resp = try!(client.post(url)
            .body(Body::BufBody(body.as_bytes(), body.len()))
            .header(self.auth_header())
            .send());
        if resp.status != hyper::status::StatusCode::Ok {
            Err(B2Error::from_response(resp))
        } else {
            Ok(())
        }
    }
    pub fn get_file_info<'b, IT>(&self, file_id: String, bucket: &'b Bucket, client: &Client)
        -> Result<B2FileVersion<'b,IT>,B2Error>
        where for<'de> IT: Deserialize<'de>
    {
        let url_string: String = format!("{}/b2api/v1/b2_get_file_info", self.api_url);
        let url: &str = &url_string;

        let body: String = format!("{{\"fileId\":\"{}\"}}", file_id);

        let resp = try!(client.post(url)
            .body(Body::BufBody(body.as_bytes(), body.len()))
            .header(self.auth_header())
            .send());
        if resp.status != hyper::status::StatusCode::Ok {
            Err(B2Error::from_response(resp))
        } else {
            let lfv: LFV<IT> = try!(serde_json::from_reader(resp));
            match lfv.bucket_id() {
                None => {},
                Some(bid) => if bid != &bucket.bucket_id {
                    return Err(B2Error::LibraryError("Provided bucket does not match actual bucket".to_owned()));
                }
            }
            match lfv.to_file_version(bucket) {
                Some(fv) => Ok(fv),
                // I'm pretty sure you can't point to a folder but just to be sure
                None => Err(B2Error::LibraryError("file id points to a folder".to_owned()))
            }
        }
    }
    pub fn hide_file<'b, IT>(&self, file: B2File<'b, IT>, client: &Client)
        -> Result<B2HideMarker<'b>,B2Error>
        where for<'de> IT: Deserialize<'de>
    {
        let url_string: String = format!("{}/b2api/v1/b2_hide_file", self.api_url);
        let url: &str = &url_string;

        let body: String =
            format!("{{\"fileId\":\"{}\", \"bucketId\":\"{}\"}}", file.file_id, file.bucket.bucket_id);

        let resp = try!(client.post(url)
            .body(Body::BufBody(body.as_bytes(), body.len()))
            .header(self.auth_header())
            .send());
        if resp.status != hyper::status::StatusCode::Ok {
            Err(B2Error::from_response(resp))
        } else {
            let lfv: LFV<IT> = try!(serde_json::from_reader(resp));
            match lfv.to_file_version(file.bucket) {
                Some(B2FileVersion::Hide(hide)) => Ok(hide),
                _ => Err(B2Error::LibraryError("hide_file did not return a hide marker".to_owned()))
            }
        }
    }

}

