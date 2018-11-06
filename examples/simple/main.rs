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
use std::collections::HashMap;
use std::env;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::channel;

use tokio::runtime::Runtime;
use tokio::fs::{File as TokioFile};

use backblaze_b2::B2Error;
use backblaze_b2::stream_util;
use backblaze_b2::authorize::{B2Credentials, B2Authorization};
use backblaze_b2::buckets::{self, Bucket, BucketType};
use backblaze_b2::files::{self, upload, download, File as B2File};

// When using this library you probably want to use this Client.
type Client = hyper::client::Client<HttpsConnector<HttpConnector>, Body>;

// Read the credentials from the file credentials.txt.
// The contents should look like this:
//
// { "id": "your backblaze id", "key": "your backblaze key" }
fn read_creds() -> B2Credentials {
    let creds_file = match std::fs::File::open("credentials.txt") {
        Ok(file) => file,
        Err(_err) => panic!("credentials.txt not found"),
    };
    serde_json::from_reader(creds_file).unwrap()
}

/// Finds the path of `file1.txt`, which is to be uploaded.
fn get_text_file() -> Result<PathBuf, std::io::Error> {
    let mut file = env::current_dir()?;
    file.push("examples/simple/file1.txt");
    Ok(file)
}
/// Finds the path of `file2.png`, which is to be uploaded.
fn get_image_file() -> Result<PathBuf, std::io::Error> {
    let mut file = env::current_dir()?;
    file.push("examples/simple/file2.png");
    Ok(file)
}

// This method will create a future that uploads the specified file.
//
// Note that this method takes the client and authorization by value.
// This is okay, because both types are internally reference counted. This is done because
// otherwise one easily runs into lifetime problems when working with futures.
fn upload_file(
    auth: B2Authorization,
    client: Client,
    file_location: PathBuf,
    bucket_id: &str,
) -> impl Future<Item = B2File, Error = B2Error> {
    // In order to upload the file, we need three things:
    // 1. A file opened for writing.
    // 2. The length of the file.
    // 3. The url to upload it to.

    // Create a task that will open the file for reading.
    let open_future = TokioFile::open(file_location.clone());

    // Create a task that will fetch the metadata of the file.
    // We need this because we must know the size of the file in advance in order to
    // upload it to backblaze.
    let metadata_future = tokio::fs::metadata(file_location.clone());

    // Let's join these two futures. Joining means to create a future that does both
    // things at the same time, and finishes when they are both done.
    //
    // Note that we call map_err here. This is because tokio will return an io error if it
    // fails, but the backblaze functions return a B2Error. The problem is that to join
    // the futures they need the same error type, and we will later join file_future with
    // something where the error type is B2Error.
    let file_future = open_future.join(metadata_future).map_err(|err| B2Error::from(err));

    // We also need the url that we're uploading to.
    //
    // Note that when uploading files in parallel, the same url should not be reused.
    // That's why every call to upload_file creates a new url.
    let url_future = upload::get_upload_url(&auth, &client, bucket_id);

    // Let's join the futures.
    let ready_future = file_future.join(url_future);

    // When we are ready for the download, we want to actually perform the download.
    ready_future.and_then(move |((open_file, metadata), upload_url)| {
        // Notice the `move` on the closure. This is so we can use client inside the
        // closure. The need for doing this is the reason for it to be internally
        // reference counted.

        // Extract the file name from the path.
        let file_name = file_location.file_name().unwrap().to_str().unwrap();

        println!("Starting upload of {} containing {} bytes.", file_name, metadata.len());

        // We need to prepare the file for the upload function first.
        // This can be done like this:
        let body = Body::wrap_stream(stream_util::chunked_stream(open_file));

        upload::upload_file(
            &upload_url,
            &client,
            file_name,
            body,
            "b2/x-auto", // let backblaze figure out the content type.
            metadata.len() as usize,
            "do_not_verify",
            None,
            None,
        ).map(|file| {
            println!("Upload of {} done.", &file.file_name);
            file
        })
    })
}


fn main() -> Result<(), Box<std::error::Error>> {

    // Here we use a multi threaded runtime.
    // Often using a single threaded runtime instead would be fine.
    let mut runtime = Runtime::new().unwrap();

    let https = HttpsConnector::new(1).unwrap();
    let client = hyper::client::Client::builder().build(https);

    // Let's authenticate our account.
    let creds = read_creds();
    let auth = runtime.block_on(creds.authorize(&client))?;

    println!("Authenticated!");

    // Now that we have authenticated, we can create a bucket to run our example in.
    let bucket_name = format!("rust-b2test-{}", auth.account_id);
    let bucket_future = buckets::create_bucket(
        &auth,
        &client,
        &bucket_name,
        BucketType::Private,
        &buckets::NoBucketInfo,
        &[],
        &[],
    );
    // Wait for the bucket to finish.
    let bucket = runtime.block_on(bucket_future)?;
    println!("Bucket created");



    // Let's upload the two files in this example.
    let file1_path = get_text_file()?;
    let file2_path = get_image_file()?;

    // We use the upload function defined earlier, which creates a future that completes
    // when the file has been uploaded.
    let file1_upload_future = upload_file(
        auth.clone(),
        client.clone(),
        file1_path,
        &bucket.bucket_id
    );
    let file2_upload_future = upload_file(
        auth.clone(),
        client.clone(),
        file2_path,
        &bucket.bucket_id
    );
    // Perform both futures at the same time.
    let (file1, file2) = runtime.block_on(file1_upload_future.join(file2_upload_future))?;



    // Let's delete the files again.
    let delete1_future = files::delete_file(
        &auth,
        &client,
        &file1.file_id,
        &file1.file_name,
    ).map(|file| println!("Deleted {}.", file.file_name));
    let delete2_future = files::delete_file(
        &auth,
        &client,
        &file2.file_id,
        &file2.file_name,
    ).map(|file| println!("Deleted {}.", file.file_name));
    // Delete them at the same time.
    runtime.block_on(delete1_future.join(delete2_future))?;



    // Delete the bucket again.
    let delete_future = buckets::delete_bucket(&auth, &client, &bucket.bucket_id);
    runtime.block_on(delete_future)?;
    println!("Bucket deleted.");

    // Finish any tasks still waiting on the executor. Then exit.
    //
    // Important! The client must be dropped before shutting down the runtime.
    // Otherwise the shutdown call will hang forever.
    drop(client);
    runtime.shutdown_on_idle().wait().unwrap();
    println!("Done!");
    Ok(())
}

