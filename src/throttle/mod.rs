//! Throttle a [`Stream`] or [`AsyncRead`].
//!
//! [`Stream`]: https://docs.rs/tokio/0.1/tokio/fs/struct.File.html
//! [`AsyncRead`]: https://docs.rs/tokio-io/0.1/tokio_io/trait.AsyncRead.html

use futures::stream::Stream;
use futures::{Async, Future, Poll};

use tokio::prelude::task;
use tokio::timer::Delay;
use tokio_codec::{BytesCodec, FramedRead};
use tokio_io::AsyncRead;

use bytes::Bytes;

use std::cmp::min;
use std::time::{Duration, Instant};

pub mod async;

/// Throttles the underlying [`AsyncRead`] using a [token bucket][1].
///
/// Every read consumes one token from the bucket for each byte, and tokens are regained
/// at a rate of `rate` tokens per second. A rate of zero indicates that no throttling
/// should be done.
///
/// A common usage of this type would be to throttle the upload of a file. This would be
/// done by wrapping a [`tokio::fs::File`] in `ThrottledRead`.
///
/// This works internally by wrapping the `AsyncRead` in a [`FramedRead`] with a
/// [`BytesCodec`] and wrapping that in a [`ThrottledStream`].
///
/// [1]: https://en.wikipedia.org/wiki/Token_bucket
/// [`tokio::fs::File`]: https://docs.rs/tokio/0.1/tokio/fs/struct.File.html
/// [`AsyncRead`]: https://docs.rs/tokio-io/0.1/tokio_io/trait.AsyncRead.html
/// [`FramedRead`]: https://docs.rs/tokio-codec/0.1/tokio_codec/struct.FramedRead.html
/// [`BytesCodec`]: https://docs.rs/tokio-codec/0.1/tokio_codec/struct.BytesCodec.html
/// [`ThrottledStream`]: struct.ThrottledStream.html
pub struct ThrottledRead<R> {
    inner: ThrottledStream<FramedRead<R, BytesCodec>>,
}
impl<R: AsyncRead> ThrottledRead<R> {
    /// Create a new `ThrottledRead`. This method requires that `bucket_size` is at
    /// least 1024.
    pub fn new(read: R, bucket_size: usize, rate: u64) -> Self {
        let framed = FramedRead::new(read, BytesCodec::new());
        ThrottledRead {
            inner: ThrottledStream::new(framed, bucket_size, rate),
        }
    }
    /// Set the rate that new tokens are gained at. A rate of zero indicates that no
    /// throttling should be done.
    pub fn set_rate(&mut self, rate: u64) {
        self.inner.set_rate(rate);
    }
    /// Set the bucket size of the `ThrottledRead`. Panics if `bucket_size` is less
    /// than 1024.
    pub fn set_bucket_size(&mut self, bucket_size: usize) {
        self.inner.set_bucket_size(bucket_size);
    }
    /// Unwrap the `ThrottledRead`. This method returns the underlying stream together
    /// with any bytes not yet polled.
    pub fn into_inner(self) -> (Option<Bytes>, R) {
        let (bytes, framed_read) = self.inner.into_inner();
        (bytes, framed_read.into_inner())
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
/// Every read consumes one token from the bucket for each byte, and tokens are regained
/// at a rate of `rate` tokens per second. A rate of zero indicates that no throttling
/// should be done.
///
/// If your byte stream is not framed into chunks, consider using a [`ThrottledRead`].
///
/// [1]: https://en.wikipedia.org/wiki/Token_bucket
/// [`Stream`]: https://docs.rs/tokio/0.1/tokio/fs/struct.File.html
/// [`ThrottledRead`]: struct.ThrottledRead.html
pub struct ThrottledStream<S> {
    tokens: usize,
    last_read: Instant,
    next: Option<Bytes>,
    timeout: Option<Delay>,

    bucket_size: usize,
    rate: u64,

    inner: S,
}
impl<S: Stream> ThrottledStream<S>
where
    S::Item: Into<Bytes>,
{
    /// Create a new `ThrottledStream`. This method requires that `bucket_size` is at
    /// least 1024.
    pub fn new(stream: S, bucket_size: usize, rate: u64) -> Self {
        if bucket_size < 1024 {
            panic!("The bucket size of a ThrottledStream must be at least 1024.");
        }
        ThrottledStream {
            tokens: bucket_size,
            last_read: Instant::now(),
            next: None,
            timeout: None,

            bucket_size,
            rate,

            inner: stream,
        }
    }
    /// Set the rate that new tokens are gained at. A rate of zero indicates that no
    /// throttling should be done.
    pub fn set_rate(&mut self, rate: u64) {
        self.rate = rate;
    }
    /// Set the bucket size of the `ThrottledStream`. Panics if `bucket_size` is less
    /// than 1024.
    pub fn set_bucket_size(&mut self, bucket_size: usize) {
        if bucket_size < 1024 {
            panic!("The bucket size of a ThrottledStream must be at least 1024.");
        }
        self.bucket_size = bucket_size;
    }
    /// Unwrap the `ThrottledStream`. This method returns the underlying stream together
    /// with any bytes not yet polled.
    pub fn into_inner(self) -> (Option<Bytes>, S) {
        (self.next, self.inner)
    }
    #[inline]
    fn fill_tokens(&mut self, now: Instant) {
        let dur = now.duration_since(self.last_read);
        let nanos = dur
            .as_secs()
            .saturating_mul(1_000_000_000u64)
            .saturating_add(u64::from(dur.subsec_nanos()));
        let tokens_x_1000000000 = nanos.saturating_mul(self.rate);
        let tokens = tokens_x_1000000000 / 1_000_000_000u64;
        let new_tokens = self.tokens.saturating_add(saturating_u64_to_usize(tokens));
        self.tokens = min(self.bucket_size, new_tokens);
        self.last_read = now;
    }
    #[inline]
    fn cut_chunk(&mut self, mut bytes: Bytes) -> (Bytes, Option<Bytes>) {
        if self.tokens < bytes.len() {
            let remaining = bytes.split_off(self.tokens);
            assert_eq!(bytes.len(), self.tokens);
            self.tokens = 0;
            (bytes, Some(remaining))
        } else {
            self.tokens -= bytes.len();
            (bytes, None)
        }
    }
}

#[inline]
fn saturating_u64_to_usize(i: u64) -> usize {
    if i as usize as u64 == i {
        i as usize
    } else {
        usize::max_value()
    }
}

impl<S: Stream> Stream for ThrottledStream<S>
where
    S::Item: Into<Bytes>,
{
    type Item = Bytes;
    type Error = S::Error;
    fn poll(&mut self) -> Poll<Option<Bytes>, S::Error> {
        let next = match self.next.take() {
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
            return Ok(Async::Ready(Some(next)));
        }
        self.fill_tokens(Instant::now());
        if self.tokens < next.len() && self.tokens < 1024 {
            let needed_tokens = min(self.bucket_size, next.len() - self.tokens);
            // Here we divide round up, preferring to wait a millisecond more than one too
            // few. Notice that if the numerator is zero this returns one. This is good as
            // we want to make sure the timeout isn't zero.
            let millis =
                1 + ((needed_tokens as u64).saturating_mul(1000u64) - 1) / self.rate;
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
                }
                Ok(Async::NotReady) => {
                    // Timeouts will notify the executor, but if it's dropped, the
                    // notification is cancelled.
                    // We store the timeout in the struct so it isn't dropped, but if we
                    // are polled before the timeout completes, we won't poll the timeout
                    // again.
                    self.timeout = Some(timeout);
                    self.next = Some(next);
                    return Ok(Async::NotReady);
                }
                Err(err) => {
                    self.next = Some(next);
                    if err.is_shutdown() {
                        panic!("ThrottledStream requires a timer to be available.");
                    } else if err.is_at_capacity() {
                        task::current().notify();
                        return Ok(Async::NotReady);
                    } else {
                        panic!("Unknown timer error: {}", err);
                    }
                }
            }
        }
        let (send, store) = self.cut_chunk(next);
        self.next = store;
        self.timeout = None;
        Ok(Async::Ready(Some(send)))
    }
}
