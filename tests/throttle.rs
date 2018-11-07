extern crate backblaze_b2;
extern crate futures;
extern crate tokio;
extern crate tokio_io;

use futures::stream::Stream;
use futures::{future, Future};
use std::sync::mpsc::channel;
use tokio_io::io::AllowStdIo;

use std::io::Cursor;
use std::time::Instant;

fn run_future<Fut, T, E>(future: Fut) -> Result<T, E>
where
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
        future
            .map(move |v| send1.send(Ok(v)).unwrap())
            .map_err(move |e| send2.send(Err(e)).unwrap())
    }));
    exec.run().unwrap();
    recv.try_recv().unwrap()
}

#[test]
fn test_throttled_read() {
    use backblaze_b2::throttle::*;

    // create 20 megabytes
    let mut data = Vec::with_capacity(1024 * 1024 * 20);
    for i in 0..data.capacity() {
        data.push(i as u8);
    }

    let len = data.len();
    let cursor = AllowStdIo::new(Cursor::new(data));
    // The rate is the size of the data divided by four.
    // This means it will take at least four seconds to complete.
    let throttled = ThrottledRead::new(cursor, 8192, (len / 4) as u64);

    let now = Instant::now();
    let sum = run_future(
        throttled
            .map_err(|_| ())
            .fold(0, |sum, buf| future::ok(sum + buf.len())),
    )
    .unwrap();
    assert_eq!(sum, len);
    let elapsed = now.elapsed();
    println!("Elapsed: {}", elapsed.as_secs());
    assert!(elapsed.as_secs() >= 4);
}
#[test]
fn test_throttled_async_read() {
    use backblaze_b2::throttle::async::*;

    // create 20 megabytes
    let mut data1 = Vec::with_capacity(1024 * 1024 * 20);
    for i in 0..data1.capacity() {
        data1.push(i as u8);
    }
    let data2 = data1.clone();

    let len = data1.len();

    let cursor1 = AllowStdIo::new(Cursor::new(data1));
    let cursor2 = AllowStdIo::new(Cursor::new(data2));

    // The rate is the size of the data divided by two.
    // This means it will take at least four seconds to complete both.
    let throttle = Throttle::new((len / 2) as u64, 8192);

    let read1 = throttle
        .throttle_read(cursor1)
        .map_err(|_| ())
        .fold(0, |sum, buf| future::ok(sum + buf.len()));
    let read2 = throttle
        .throttle_read(cursor2)
        .map_err(|_| ())
        .fold(0, |sum, buf| future::ok(sum + buf.len()));

    let now = Instant::now();
    let (sum1, sum2) = run_future(read1.join(read2)).unwrap();
    assert_eq!(sum1, len);
    assert_eq!(sum2, len);
    let elapsed = now.elapsed();
    println!("Elapsed: {}", elapsed.as_secs());
    assert!(elapsed.as_secs() >= 4);
}
