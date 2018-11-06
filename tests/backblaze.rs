#![type_length_limit="4194304"]
extern crate backblaze_b2;
extern crate hyper;
extern crate serde_json;
extern crate futures;
extern crate hyper_tls;
extern crate tokio_io;
extern crate tokio;
extern crate rand;

use hyper::body::Body;
use hyper::client::HttpConnector;
use hyper_tls::HttpsConnector;
use futures::stream::{self, Stream};
use futures::future::{self, Future, IntoFuture};
use std::sync::mpsc::channel;
use std::rc::Rc;
use std::collections::HashMap;
use std::io::Cursor;

use tokio_io::io::AllowStdIo;

use backblaze_b2::B2Error;
use backblaze_b2::stream_util;
use backblaze_b2::authorize::{B2Credentials, B2Authorization};
use backblaze_b2::buckets::{self, Bucket, BucketType};
use backblaze_b2::files::{self, upload, download, File};

type Client = hyper::client::Client<HttpsConnector<HttpConnector>, Body>;

fn read_creds() -> B2Credentials {
    use std::fs::File;
    let creds_file = match File::open("credentials.txt") {
        Ok(file) => file,
        Err(_err) => panic!("credentials.txt not found"),
    };
    serde_json::from_reader(creds_file).unwrap()
}
fn new_client() -> Client {
    let https = HttpsConnector::new(2).unwrap();
    hyper::client::Client::builder().build(https)
}
fn authorize(client: &Client) -> backblaze_b2::authorize::B2AuthFuture {
    let creds = read_creds();
    creds.authorize(&client)
}

fn run_future<F,Fut,T,E>(f: F) -> Result<T,E>
where
    F: FnOnce() -> Fut + 'static,
    Fut: Future<Item = T, Error = E> + 'static,
    T: 'static,
    E: 'static,
{
    use tokio::runtime::current_thread::Runtime;
    let mut exec = Runtime::new().unwrap();
    let (send, recv) = channel();
    exec.spawn(future::lazy(move || {
        let send1 = send;
        let send2 = send1.clone();
        f()
            .map(move |v| send1.send(Ok(v)).unwrap())
            .map_err(move |e| send2.send(Err(e)).unwrap())
    }));
    exec.run().unwrap();
    recv.try_recv().unwrap()
}

fn assert_ok<F,Fut,T,E>(f: F)
where
    F: Fn() -> Fut + Sync + Send + 'static,
    Fut: Future<Item = T, Error = E> + Sync + Send + 'static,
    T: std::fmt::Debug + 'static,
    E: std::fmt::Display + 'static,
{
    match run_future(f) {
        Ok(res) => {
            println!("{:#?}", res);
        },
        Err(err) => {
            panic!("{}", err);
        }
    }
}

#[test]
fn authorize_test() {
    assert_ok(|| {
        let client = new_client();
        authorize(&client)
    });
}
#[test]
fn authorize_fail_test() {
    match run_future(|| {
        let client = new_client();
        let creds = B2Credentials::new_ref("invalid", "invalid");
        creds.authorize(&client)
    }) {
        Ok(auth) => {
            panic!("Got auth from invalid credentials. {:?}", auth);
        },
        Err(err) => {
            if !err.is_wrong_credentials() {
                panic!("Error not credentials issue: {}", err);
            }
        }
    }
}
#[test]
fn list_buckets_test() {
    let buckets = run_future(|| {
        let client = new_client();
        authorize(&client)
            .and_then(move |auth| {
                buckets::list_buckets(&auth, &client, None, None, None).collect()
            })
    }).unwrap();
    println!("{:#?}", buckets);
}
fn create_test_bucket<Del, DelF>(
    auth: Rc<B2Authorization>,
    client: Rc<Client>,
    before_delete: Del,
) -> impl Future<Item = (), Error = B2Error>
where
    Del: FnOnce(Rc<B2Authorization>, Rc<Client>, Bucket) -> DelF,
    DelF: Future<Item = Bucket, Error = B2Error>,
{
    let auth2 = auth.clone();
    let client2 = client.clone();
    let auth3 = auth.clone();
    let client3 = client.clone();
    let empty: HashMap<String,String> = HashMap::new();
    buckets::create_bucket(
        auth.clone().as_ref(),
        client.clone().as_ref(),
        &format!("rust-b2test-{}", auth.as_ref().account_id),
        BucketType::Public,
        &empty,
        &[], &[])
        .or_else(move |_err| {
            // fetch the bucket if it already exists.
            buckets::list_buckets(
                auth3.clone().as_ref(),
                client3.clone().as_ref(),
                None,
                Some(&format!("rust-b2test-{}", auth3.as_ref().account_id)),
                None)
                .collect()
                .map(|vec| vec.into_iter().next().expect("create failed but can't list"))
                .and_then(move |bucket| {
                    files::list_file_versions(
                        auth3.as_ref(),
                        client3.as_ref(),
                        &bucket.bucket_id,
                        1000, None, None, None, None
                    ).map(move |resp| {
                        assert_eq!(resp.next_file_id, None);
                        (auth3, client3, bucket, resp.files)
                    })
                })
                .and_then(move |(auth, client, bucket, files)| {
                    stream::iter_ok(files.into_iter())
                        .for_each(move |file| {
                            files::delete_file(
                                auth.as_ref(),
                                client.as_ref(),
                                &file.file_id,
                                &file.file_name,
                            ).map(move |deleted| {
                                println!("Deleted: {}", deleted.file_name);
                                assert_eq!(deleted.file_id, file.file_id);
                                assert_eq!(deleted.file_name, file.file_name);
                            })
                        })
                        .map(move |()| bucket)
                })
        })
        .and_then(move |bucket| before_delete(auth, client, bucket))
        .and_then(move |bucket| {
            buckets::delete_bucket(auth2.as_ref(), client2.as_ref(), &bucket.bucket_id)
                .map(move |del_bucket| {
                    assert_eq!(bucket, del_bucket);
                })
        })
}
fn test_update_bucket<Mean, MeanF>(
    auth: Rc<B2Authorization>,
    client: Rc<Client>,
    bucket: Bucket,
    meanwhile: Mean,
) -> impl Future<Item = Bucket, Error = B2Error>
where
    Mean: FnOnce(Rc<B2Authorization>, Rc<Client>, Bucket) -> MeanF,
    MeanF: IntoFuture<Error = B2Error>,
{
    let mut info = HashMap::new();
    info.insert("info-value".into(), "hello world".into());
    buckets::update_bucket(
        auth.clone().as_ref(),
        client.clone().as_ref(),
        &bucket.bucket_id,
        Some(BucketType::Private),
        Some(&info),
        None,
        None,
        Some(bucket.revision))
        .join(meanwhile(auth, client, bucket))
        .map(move |(bucket, _)| {
            assert_eq!(bucket.bucket_type, BucketType::Private);
            assert_eq!(bucket.bucket_info, info);
            bucket
        })
}

fn download_test(
    auth: Rc<B2Authorization>,
    client: Rc<Client>,
    bucket: Bucket,
    file: File,
    content: &'static [u8],
) -> impl Future<Item = (), Error = B2Error> {
    let without_dl_auth = {
        let id = download::download_by_id(
            auth.as_ref(),
            client.as_ref(),
            &file.file_id,
            None);
        let id = id.and_then(|(_parts, stream)| {
            let buf = Vec::with_capacity(10);
            let cursor = Cursor::new(buf);
            let async_cursor = AllowStdIo::new(cursor);
            stream_util::pipe(stream, async_cursor).map_err(|e| e.into())
        }).map(move |cursor| assert_eq!(cursor.into_inner().into_inner(), content));
        let name = download::download_by_name(
            auth.as_ref(),
            client.as_ref(),
            &bucket.bucket_name,
            &file.file_name,
            None,
            ).map(|(_parts, stream)| stream.collect_vec())
            .flatten()
            .map(move |vec| assert_eq!(vec, content));
        id.join(name)
    };
    let with_dl_auth = download::get_download_authorization(
        auth.clone().as_ref(),
        client.clone().as_ref(),
        &bucket.bucket_id, "", 100, None)
        .map(|auth| { println!("{:?}", auth); auth })
        .and_then(move |dlauth| {
            download::download_by_name(
                &dlauth,
                client.as_ref(),
                &bucket.bucket_name,
                &file.file_name,
                None,
            ).map(|(_parts, stream)| stream.collect_vec())
            .flatten()
            .map(move |vec| assert_eq!(vec, content))
        });
    without_dl_auth.join(with_dl_auth).map(|_| ())
}

fn upload_test(
    auth: Rc<B2Authorization>,
    client: Rc<Client>,
    bucket: Bucket,
) -> impl Future<Item = (), Error = B2Error> {
    upload::get_upload_url(
        auth.clone().as_ref(),
        client.clone().as_ref(),
        &bucket.bucket_id)
        .and_then(move |url| {
            let body = vec!['a' as u8,'b' as u8,'c' as u8];
            upload::upload_file(
                &url,
                client.clone().as_ref(),
                "temp_file.dat",
                body,
                "application/octet-stream",
                3,
                "a9993e364706816aba3e25717850c26c9cd0d89d",
                None,
                None)
                .map(move |file| (file, auth, client))
        })
        .and_then(move |(file, auth, client)| {
            let dl_test = download_test(
                auth.clone(), client.clone(), bucket.clone(), file.clone(), b"abc"
            );
            let file_clone = file.clone();
            let list_test = files::list_file_names(
                auth.as_ref(),
                client.as_ref(),
                &file.bucket_id,
                100,
                None,
                None,
                None,
            ).map(|list| {
                assert_eq!(list.files, &[file_clone]);
                assert_eq!(list.next_file_name, None);
            });
            files::get_file_info(
                auth.as_ref(),
                client.as_ref(),
                &file.file_id)
                .map(move |info| {
                    assert_eq!(info, file);
                    (info, auth, client)
                })
            .join(dl_test).map(|(a, ())| a)
            .join(list_test).map(|(a, ())| a)
        })
        .and_then(move |(file, auth, client)| {
            files::hide_file(
                auth.clone().as_ref(),
                client.clone().as_ref(),
                &file.bucket_id,
                "temp_file.dat")
                .map(move |hide_marker| {
                    (hide_marker, file, auth, client)
                })
        })
        .and_then(move |(hide_marker, file, auth, client)| {
            // delete the hide marker
            files::delete_file(
                auth.clone().as_ref(),
                client.clone().as_ref(),
                &hide_marker.file_id,
                "temp_file.dat")
                .map(move |deleted| {
                    assert_eq!(&deleted.file_id, &hide_marker.file_id);
                    (file, auth, client)
                })
        })
        .and_then(move |(file, auth, client)| {
            // delete the actual file
            files::delete_file(
                auth.clone().as_ref(),
                client.clone().as_ref(),
                &file.file_id,
                "temp_file.dat")
                .map(move |deleted| {
                    assert_eq!(&deleted.file_id, &file.file_id);
                    ()
                })
        })
}

#[test]
fn big_test() {
    run_future(|| {
        let client = new_client();
        authorize(&client)
            .and_then(move |auth| {
                let auth = Rc::new(auth);
                let client = Rc::new(client);
                let test = |auth, client, bucket| {
                    test_update_bucket(auth, client, bucket, upload_test)
                };
                create_test_bucket(auth, client, test)
            })
    }).unwrap();
}
