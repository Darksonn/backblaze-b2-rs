extern crate backblaze_b2;
extern crate hyper;
extern crate hyper_native_tls;
extern crate serde;
#[macro_use]
extern crate serde_json;
extern crate rand;
extern crate sha1;

use std::io::{Read, Write};
use std::fs::File;

use hyper::Client;
use hyper::net::HttpsConnector;
use hyper_native_tls::NativeTlsClient;

use rand::Rng;

use backblaze_b2::raw::authorize::*;
use backblaze_b2::raw::buckets::*;
use backblaze_b2::raw::files::*;

use serde_json::value::Value;

fn make_connector() -> HttpsConnector<NativeTlsClient> {
    let ssl = NativeTlsClient::new().unwrap();
    HttpsConnector::new(ssl)
}
fn make_client() -> Client {
    Client::with_connector(make_connector())
}

fn rand_string(len: usize) -> String {
    let mut rng = rand::thread_rng();
    rng.gen_ascii_chars().take(len).collect()
}

#[test]
fn list_all_files() {
    let client = make_client();
    let connector = make_connector();
    let cred_file = match File::open("credentials.txt") {
        Ok(f) => f,
        Err(_) =>
            panic!("The test requires the credentials for b2 to be placed in the file \'credentials.txt\' which contains a json object with the properties \"id\" and \"key\".")
    };
    let cred: B2Credentials = serde_json::from_reader(cred_file).unwrap();
    let auth: B2Authorization = cred.authorize(&client).unwrap();
    let new_bucket_name = format!("rust-b2-test-{}", rand_string(16));
    let bucket = auth.create_bucket_no_info(&new_bucket_name, BucketType::Private,
                                            Vec::new(), &client).unwrap();
    let mut files = Vec::new();
    let upload_auth = auth.get_upload_url(&bucket.bucket_id, &client).unwrap();
    for i in 0..30 {
        let mut file_data: [u8; 9] = [0,0,0,0,0,0,0,0,0];
        let mut rng = rand::thread_rng();
        for j in 0..9 {
            file_data[j] = rng.gen();
        }

        let mut m = sha1::Sha1::new();
        m.update(&file_data);
        let sha1 = m.digest().to_string();
        let file: MoreFileInfo = {
            if i % 3 == 0 {
                upload_auth.upload_file(
                    &mut&file_data[..],
                    format!("{}", i),
                    None, 9,
                    sha1.clone(), &connector
                ).unwrap()
            } else if i % 3 == 1 {
                let mut request = upload_auth.create_upload_file_request(
                    format!("{}", i),
                    None, 9, sha1.clone(),
                    &connector
                ).unwrap();
                request.write_all(&file_data[..]).unwrap();
                request.finish().unwrap()
            } else {
                let mut request = upload_auth.create_upload_file_request_sha1_at_end(
                    format!("{}", i),
                    None, 9,
                    &connector
                ).unwrap();
                request.write_all(&file_data[..]).unwrap();
                request.finish(&sha1).unwrap()
            }
        };
        files.push(file);
    }
    let listing = auth.list_all_file_names::<Value>(&bucket.bucket_id, 12, None, None, &client).unwrap();
    assert_eq!(listing.folders.len(), 0);
    for file in listing.files {
        let fi: usize = file.file_name.parse().unwrap();
        assert_eq!(files[fi].content_sha1, file.content_sha1);
        auth.delete_file_version(&file.file_name, &file.file_id, &client).unwrap();
    }
    auth.delete_bucket(&bucket, &client).unwrap();
}
#[test]
#[allow(unused_variables)]
fn main_test() {
    let client = make_client();
    let cred_file = match File::open("credentials.txt") {
        Ok(f) => f,
        Err(_) =>
            panic!("The test requires the credentials for b2 to be placed in the file \'credentials.txt\' which contains a json object with the properties \"id\" and \"key\".")
    };
    let cred: B2Credentials = serde_json::from_reader(cred_file).unwrap();
    let auth: B2Authorization = cred.authorize(&client).unwrap();

    let buckets_before: Vec<Bucket> = auth.list_buckets(&client).unwrap();

    let new_bucket_name = format!("rust-b2-test-{}", rand_string(16));

    {
        let bucket_info = json!({"abc": "test", "json": "data"});
        let bucket = auth.create_bucket(&new_bucket_name, BucketType::Private,
                                        bucket_info.clone(), Vec::new(), &client).unwrap();
        assert_eq!(bucket.bucket_name, new_bucket_name);
        assert_eq!(bucket.bucket_type, BucketType::Private);
        assert_eq!(bucket.bucket_info, bucket_info);
        assert_eq!(bucket.account_id, auth.credentials.id);
        auth.delete_bucket_id::<Value>(&bucket.bucket_id, &client).unwrap();
    }
    let bucket = auth.create_bucket_no_info(&new_bucket_name, BucketType::Private,
                                            Vec::new(), &client).unwrap();
    assert_eq!(bucket.bucket_name, new_bucket_name);
    assert_eq!(bucket.bucket_type, BucketType::Private);
    assert_eq!(bucket.bucket_info, json!({}));
    assert_eq!(bucket.account_id, auth.credentials.id);

    let buckets_after: Vec<Bucket> = auth.list_buckets(&client).unwrap();
    //assert_eq!(buckets_after.len() - buckets_before.len(), 1);
    //other tests intefere with this

    if let Ok((fnl, None)) =
        auth.list_file_names::<Value>(&bucket.bucket_id, None, 10, None, None, &client) {
        assert_eq!(fnl.files.len(), 0);
        assert_eq!(fnl.folders.len(), 0);
    } else {
        panic!();
    }

    let file_data: [u8; 9] = [23, 10, 43, 94, 23, 0, 18, 34, 244];
    let mut m = sha1::Sha1::new();
    m.update(&file_data);
    let sha1 = m.digest().to_string();
    let file = {
        let upload_auth = auth.get_upload_url(&bucket.bucket_id, &client).unwrap();

        let file: MoreFileInfo = upload_auth.upload_file(&mut&file_data[..], "test_file.png".to_owned(), None, 9,
        sha1.clone(), &make_connector()).unwrap();
        assert_eq!(&file.file_name, "test_file.png");
        assert_eq!(file.account_id, auth.credentials.id);
        assert_eq!(file.content_sha1, sha1);
        assert_eq!(file.bucket_id, bucket.bucket_id);
        assert_eq!(file.content_length, 9);
        assert_eq!(file.content_type, "image/png");
        assert_eq!(file.action, FileType::File);
        file
    };

    {
        let file2: MoreFileInfo = auth.get_file_info(&file.file_id, &client).unwrap();
        assert_eq!(&file2.file_name, "test_file.png");
        assert_eq!(file2.account_id, auth.credentials.id);
        assert_eq!(file2.content_sha1, sha1);
        assert_eq!(file2.bucket_id, bucket.bucket_id);
        assert_eq!(file2.content_length, 9);
        assert_eq!(file2.content_type, "image/png");
        assert_eq!(file2.action, FileType::File);
    }
    if let Ok((fnl, None)) =
        auth.list_file_names::<Value>(&bucket.bucket_id, None, 10, None, None, &client) {
        assert_eq!(fnl.files.len(), 1);
        assert_eq!(fnl.folders.len(), 0);
        let file2 = &fnl.files[0];
        assert_eq!(&file2.file_name, "test_file.png");
        assert_eq!(file2.content_sha1, sha1);
        assert_eq!(file2.content_length, 9);
        assert_eq!(file2.content_type, "image/png");
    } else {
        panic!();
    }
    if let Ok((fvl, None, None)) =
        auth.list_file_versions::<Value>(&bucket.bucket_id, None, None, 10, None, None, &client) {
        assert_eq!(fvl.files.len(), 1);
        assert_eq!(fvl.folders.len(), 0);
        assert_eq!(fvl.hide_markers.len(), 0);
        assert_eq!(fvl.unfinished_large_files.len(), 0);
        let file2 = &fvl.files[0];
        assert_eq!(&file2.file_name, "test_file.png");
        assert_eq!(file2.content_sha1, sha1);
        assert_eq!(file2.content_length, 9);
        assert_eq!(file2.content_type, "image/png");
    } else {
        panic!();
    }

    {
        let (mut data, file2): (_, Option<FileInfo>) = auth.to_download_authorization()
                            .download_file_by_id(&file.file_id, &client).unwrap();
        let mut buf = Vec::new();
        data.read_to_end(&mut buf).unwrap();
        assert_eq!(buf, Vec::from(&file_data[..]));
        let file2 = file2.unwrap();
        assert_eq!(&file2.file_name, "test_file.png");
        assert_eq!(file2.content_sha1, sha1);
        assert_eq!(file2.content_length, 9);
        assert_eq!(file2.content_type, "image/png");
    }
    {
        let (mut data, file2): (_, Option<FileInfo>) = auth.to_download_authorization()
                            .download_file_by_name(&bucket.bucket_name, &file.file_name, &client).unwrap();
        let mut buf = Vec::new();
        data.read_to_end(&mut buf).unwrap();
        assert_eq!(buf, Vec::from(&file_data[..]));
        let file2 = file2.unwrap();
        assert_eq!(&file2.file_name, "test_file.png");
        assert_eq!(file2.content_sha1, sha1);
        assert_eq!(file2.content_length, 9);
        assert_eq!(file2.content_type, "image/png");
    }
    {
        let (mut data, file2): (_, Option<FileInfo>) = auth.to_download_authorization()
                            .download_range_by_id(&file.file_id, 1, 3, &client).unwrap();
        let mut buf = Vec::new();
        data.read_to_end(&mut buf).unwrap();
        assert_eq!(buf, Vec::from(&file_data[1..4]));
        let file2 = file2.unwrap();
        assert_eq!(&file2.file_name, "test_file.png");
        assert_eq!(file2.content_sha1, sha1);
        assert_eq!(file2.content_length, 3);
        assert_eq!(file2.content_type, "image/png");
    }
    {
        let (mut data, file2): (_, Option<FileInfo>) = auth.to_download_authorization()
                            .download_range_by_name(&bucket.bucket_name, &file.file_name, 1, 3, &client).unwrap();
        let mut buf = Vec::new();
        data.read_to_end(&mut buf).unwrap();
        assert_eq!(buf, Vec::from(&file_data[1..4]));
        let file2 = file2.unwrap();
        assert_eq!(&file2.file_name, "test_file.png");
        assert_eq!(file2.content_sha1, sha1);
        assert_eq!(file2.content_length, 3);
        assert_eq!(file2.content_type, "image/png");
    }

    auth.hide_file(&file.file_name, &bucket.bucket_id, &client).unwrap();
    if let Ok((fvl, None, None)) =
        auth.list_file_versions::<Value>(&bucket.bucket_id, None, None, 10, None, None, &client) {
        for file in fvl.files {
            auth.delete_file_version(&file.file_name, &file.file_id, &client).unwrap();
        }
        for file in fvl.hide_markers {
            auth.delete_file_version(&file.file_name, &file.file_id, &client).unwrap();
        }
        for file in fvl.unfinished_large_files {
            auth.delete_file_version(&file.file_name, &file.file_id, &client).unwrap();
        }
    } else {
        panic!();
    }
    auth.delete_bucket(&bucket, &client).unwrap();

    /*  comment in to clean up buckets
    for buck in buckets_before {
        if buck.bucket_name.starts_with("rust-b2-test-") {
            match auth.list_file_versions::<Value>(&buck.bucket_id, None, None, 10, None, None, &client) {
                Ok((fvl, None, None)) => {
                    for f in fvl.files {
                        auth.delete_file_version(&f.file_name, &f.file_id, &client).unwrap();
                    }
                    for f in fvl.hide_markers {
                        auth.delete_file_version(&f.file_name, &f.file_id, &client).unwrap();
                    }
                    for f in fvl.unfinished_large_files {
                        auth.delete_file_version(&f.file_name, &f.file_id, &client).unwrap();
                    }
                },
                Ok(x) => panic!("{:?}", x),
                Err(e) => panic!("{:?}", e)
            }
            auth.delete_bucket(&buck, &client).unwrap();
        }
    }
    // */


}


