# backblaze-b2-rs
Rust library for using the backblaze b2 api.

The backblaze api requires https, so you need to provide a Client with a https
connector.  Such a client can be created with the api call below:

    extern crate hyper;
    extern crate hyper_native_tls;
    use hyper::Client;
    use hyper::net::HttpsConnector;
    use hyper_native_tls::NativeTlsClient;
    
    let ssl = NativeTlsClient::new().unwrap();
    let connector = HttpsConnector::new(ssl);
    let client = Client::with_connector(connector);

Unfortunately because of the hyper api design, the upload functionality in this
library requires the connector instead of the client, and since the client
consumes the connector, you'll have to make two of them.
