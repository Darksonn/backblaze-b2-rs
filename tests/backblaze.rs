extern crate backblaze_b2;
extern crate hyper;
extern crate serde_json;
extern crate futures;
extern crate hyper_tls;
extern crate tokio;

use hyper::body::Body;
use hyper::client::HttpConnector;
use hyper_tls::HttpsConnector;
use futures::{future, Future};
use std::sync::mpsc::channel;

use backblaze_b2::authorize::B2Credentials;
use backblaze_b2::buckets::list_buckets;

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
    F: Fn() -> Fut + 'static,
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
        let creds = B2Credentials {
            id: String::from("invalid"),
            key: String::from("invalid"),
        };
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
    assert_ok(|| {
        let client = new_client();
        authorize(&client)
            .and_then(move |auth| {
                list_buckets(&auth, &client, None, None, None)
            })
    });
}
