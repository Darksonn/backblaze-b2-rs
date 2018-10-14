//! Like the parent module, but allows global throttling of several streams.
//!
//! The main type in this module is [`Throttle`].
//!
//! [`Throttle`]: struct.Throttle.html

use futures::stream::Stream;
use futures::{Poll, Async, Future};

use tokio::timer::Delay;
use tokio::prelude::task;
use tokio_codec::{FramedRead, BytesCodec};
use tokio_io::AsyncRead;

use bytes::{Bytes, BytesMut, BufMut};

use std::cmp::min;
use std::mem;
use std::time::{Instant, Duration};
use std::io::Read;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::{Release, Acquire};

use throttle::saturating_u64_to_usize;

/// Builder of throttled streams that are globally throttled.
///
/// Every stream will have it's own [token bucket][1], but the rate of each token bucket
/// is automatically adjusted as the number of streams changes.
///
/// This type contains an internal counter of the number of streams currently using
/// bandwidth. The rate of the `Throttle` is divided evenly between the token buckets in
/// the streams. This means that if a stream is registered as using bandwidth, but isn't
/// actually using it, the other streams won't use all of the available bandwidth.
///
/// This type can be cloned as the inner count is reference counted. Each clone will share
/// the same resources.
///
/// [1]: https://en.wikipedia.org/wiki/Token_bucket
#[derive(Clone)]
pub struct Throttle {
    stream_count: Arc<AtomicUsize>,
    default_rate: u64,
    default_bucket_size: usize,
}
impl Throttle {
    /// Create a new `Throttle`.
    pub fn new(rate: u64, bucket_size: usize) -> Throttle {
        if bucket_size < 1024 {
            panic!("Bucket size must be at least 1024.");
        }
        Throttle {
            stream_count: Arc::new(AtomicUsize::new(0)),
            default_rate: rate,
            default_bucket_size: bucket_size,
        }
    }
    /// Set the rate that every stream will have when it is created. Note that this does
    /// not change the rate of streams that have already been created. A rate of zero
    /// indicates that no throttling should be done.
    pub fn set_default_rate(&mut self, rate: u64) {
        self.default_rate = rate;
    }
    /// Set the bucket size that every stream will have when it is created. Note that this
    /// does not change the bucket size of streams that have already been created. Panics
    /// if `bucket_size` is less than 1024.
    pub fn set_default_bucket_size(&mut self, bucket_size: usize) {
        if bucket_size < 1024 {
            panic!("Bucket size must be at least 1024.");
        }
        self.default_bucket_size = bucket_size;
    }
    /// Wrap the provided stream in a stream that is throttled globally together with all
    /// other streams created on this `Throttle`.
    pub fn throttle_stream<S: Stream>(&self, stream: S) -> ThrottledStream<S>
    where
        S::Item: Into<Bytes>
    {
        ThrottledStream {
            tokens: self.default_bucket_size,
            last_read: Instant::now(),
            next: None,
            timeout: None,

            stream_count: self.stream_count.clone(),
            registered: false,

            bucket_size: self.default_bucket_size,
            rate: self.default_rate,

            inner: stream,
        }
    }
    /// Wrap the provided read in a stream that is throttled globally together with all
    /// other streams created on this `Throttle`.
    ///
    /// A common usage of this type would be to throttle the upload of a file. This would
    /// be done by wrapping a [`tokio::fs::File`] in `ThrottledRead`.
    ///
    /// [`tokio::fs::File`]: https://docs.rs/tokio/0.1/tokio/fs/struct.File.html
    /// [`AsyncRead`]: https://docs.rs/tokio-io/0.1/tokio_io/trait.AsyncRead.html
    pub fn throttle_read<R: AsyncRead>(&self, read: R) -> ThrottledRead<R> {
        let framed = FramedRead::new(read, BytesCodec::new());
        ThrottledRead {
            inner: self.throttle_stream(framed)
        }
    }
}

/// Throttles the underlying [`AsyncRead`] using a [token bucket][1].
///
/// This works internally by wrapping the [`AsyncRead`] in a [`FramedRead`] with a
/// [`BytesCodec`] and wrapping that in a [`ThrottledStream`].
///
/// [1]: https://en.wikipedia.org/wiki/Token_bucket
/// [`AsyncRead`]: https://docs.rs/tokio-io/0.1/tokio_io/trait.AsyncRead.html
/// [`FramedRead`]: https://docs.rs/tokio-codec/0.1/tokio_codec/struct.FramedRead.html
/// [`BytesCodec`]: https://docs.rs/tokio-codec/0.1/tokio_codec/struct.BytesCodec.html
/// [`ThrottledStream`]: struct.ThrottledStream.html
pub struct ThrottledRead<R> {
    inner: ThrottledStream<FramedRead<R, BytesCodec>>,
}
impl<R: AsyncRead> ThrottledRead<R> {
    /// Set the rate that new tokens are gained at.
    pub fn set_rate(&mut self, rate: u64) {
        self.inner.set_rate(rate);
    }
    /// Set the bucket size of the `ThrottledRead`. Panics if `bucket_size` is less
    /// than 1024.
    pub fn set_bucket_size(&mut self, bucket_size: usize) {
        self.inner.set_bucket_size(bucket_size);
    }
}
impl<R: AsyncRead> Stream for ThrottledRead<R> {
    type Item = Bytes;
    type Error = ::std::io::Error;
    fn poll(&mut self) -> Poll<Option<Bytes>, Self::Error> {
        self.inner.poll()
    }
}

/// Throttles the underlying [`Stream`] using a [token bucket][1].
///
/// The rate used in the token bucket is the rate of the stream divided by the number of
/// registered streams in this [`Throttle`]. A rate of zero indicates that no throttling
/// should be done.
///
/// [1]: https://en.wikipedia.org/wiki/Token_bucket
/// [`Throttle`]: struct.Throttle.html
/// [`Stream`]: https://docs.rs/tokio/0.1/tokio/fs/struct.File.html
pub struct ThrottledStream<S> {
    tokens: usize,
    last_read: Instant,
    next: Option<Bytes>,
    timeout: Option<Delay>,

    stream_count: Arc<AtomicUsize>,
    registered: bool,

    bucket_size: usize,
    rate: u64,

    inner: S,
}
impl<S: Stream> ThrottledStream<S>
where
    S::Item: Into<Bytes>
{
    /// Set the rate that new tokens are gained at. Note that increasing this beyond the
    /// initial value will increase the overall bandwidth usage of the streams. A rate of
    /// zero indicates that no throttling should be done.
    #[inline]
    pub fn set_rate(&mut self, rate: u64) {
        self.rate = rate;
    }
    /// Set the bucket size of the `ThrottledStream`. Panics if `bucket_size` is less
    /// than 1024.
    #[inline]
    pub fn set_bucket_size(&mut self, bucket_size: usize) {
        if bucket_size < 1024 {
            panic!("The bucket size of a ThrottledStream must be at least 1024.");
        }
        self.bucket_size = bucket_size;
    }
    #[inline]
    fn fill_tokens(&mut self, now: Instant) {
        let dur = now.duration_since(self.last_read);
        let nanos = dur.as_secs().saturating_mul(1000000000u64)
            .saturating_add(dur.subsec_nanos() as u64);
        let tokens_x_1000000000 = nanos.saturating_mul(self.rate);
        let tokens_all = tokens_x_1000000000 / 1000000000u64;
        let tokens = saturating_u64_to_usize(tokens_all) / self.stream_count.load(Acquire);
        let new_tokens = self.tokens.saturating_add(tokens);
        self.tokens = min(self.bucket_size, new_tokens);
        self.last_read = now;
    }
    #[inline]
    fn cut_chunk(&mut self, mut bytes: Bytes) -> (Bytes, Option<Bytes>) {
        if self.tokens < bytes.len() {
            let first = bytes.split_to(self.tokens);
            self.tokens = 0;
            (first, Some(bytes))
        } else {
            self.tokens -= bytes.len();
            (bytes, None)
        }
    }
}
impl<S> ThrottledStream<S> {
    /// Register this stream as using bandwidth. Normally this method is not needed, as
    /// this is automatically done when you start using the stream.
    #[inline]
    pub fn register(&mut self) {
        if !self.registered {
            self.stream_count.fetch_add(1, Release);
            self.registered = true;
        }
    }
    /// Register this stream as not using bandwidth. This is automatically called when
    /// this type is dropped.
    #[inline]
    pub fn unregister(&mut self) {
        if self.registered {
            let prev = self.stream_count.fetch_sub(1, Release);
            self.registered = false;
            debug_assert!(prev > 0);
        }
    }
}
impl<S> Drop for ThrottledStream<S> {
    fn drop(&mut self) {
        self.unregister();
    }
}

impl<S: Stream> Stream for ThrottledStream<S>
where
    S::Item: Into<Bytes>
{
    type Item = Bytes;
    type Error = S::Error;
    fn poll(&mut self) -> Poll<Option<Bytes>, S::Error> {
        let next = match mem::replace(&mut self.next, None) {
            Some(bytes) => bytes,
            None => match self.inner.poll() {
                Err(err) => return Err(err),
                Ok(Async::NotReady) => return Ok(Async::NotReady),
                Ok(Async::Ready(None)) => return Ok(Async::Ready(None)),
                Ok(Async::Ready(Some(bytes))) => bytes.into(),
            },
        };
        if self.rate == 0 {
            // No throttling is done.
            return Ok(Async::Ready(Some(bytes)));
        }
        self.fill_tokens(Instant::now());
        if self.tokens < next.len() && self.tokens < 1024 {
            let needed_tokens = min(self.bucket_size, next.len() - self.tokens);
            // Here we divide round up, preferring to wait a millisecond more than one too
            // few. Notice that if the numerator is zero this returns one. This is good as
            // we want to make sure the timeout isn't zero.
            let millis = 1 + ((needed_tokens as u64).saturating_mul(1000u64) - 1)
                             / self.rate;
            let duration = Duration::from_millis(millis);
            let mut timeout = Delay::new(self.last_read + duration);
            match timeout.poll() {
                Ok(Async::Ready(())) => {
                    // Timeout completed immediately?!
                    // Maybe the computer went into suspend since the last read.
                    // Or maybe the rate is very very high.
                    // Refill the tokens and proceed as normal.
                    self.timeout = None;
                    self.fill_tokens(Instant::now());
                },
                Ok(Async::NotReady) => {
                    // Timeouts will notify the executor, but if it's dropped, the
                    // notification is cancelled.
                    // We store the timeout in the struct so it isn't dropped, but if we
                    // are polled before the timeout completes, we won't poll the timeout
                    // again.
                    self.timeout = Some(timeout);
                    return Ok(Async::NotReady);
                },
                Err(err) => {
                    if err.is_shutdown() {
                        panic!("ThrottledStream requires a timer to be available.");
                    } else if err.is_at_capacity() {
                        task::current().notify();
                        return Ok(Async::NotReady);
                    } else {
                        panic!("Unknown timer error: {}", err);
                    }
                },
            }
        }
        let (send, store) = self.cut_chunk(next);
        self.next = store;
        self.timeout = None;
        self.register();
        Ok(Async::Ready(Some(send)))
    }
}
