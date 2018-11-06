//! Utilities for handling streams of chunks.

use bytes::Bytes;
use futures::{Stream, Future, Poll, Async};
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_codec::{FramedRead, BytesCodec};

use std::mem;
use crate::B2Error;

/// Turn an [`AsyncRead`] into a [`Stream`] of [`Bytes`].
///
/// [`AsyncRead`]: https://docs.rs/tokio-io/0.1/tokio_io/trait.AsyncRead.html
/// [`Stream`]: https://docs.rs/tokio/0.1/tokio/fs/struct.File.html
/// [`Bytes`]: https://carllerche.github.io/bytes/bytes/struct.Bytes.html
pub fn chunked_stream<R: AsyncRead>(read: R) -> Chunked<R> {
    Chunked {
        inner: FramedRead::new(read, BytesCodec::new()),
    }
}

/// A stream of chunks of bytes, reading from an [`AsyncRead`]. Created by
/// [`chunked_stream`].
///
/// [`AsyncRead`]: https://docs.rs/tokio-io/0.1/tokio_io/trait.AsyncRead.html
/// [`chunked_stream`]: fn.chunked_stream.html
pub struct Chunked<R> {
    inner: FramedRead<R, BytesCodec>,
}
impl<R: AsyncRead> Stream for Chunked<R> {
    type Item = Bytes;
    type Error = B2Error;
    fn poll(&mut self) -> Poll<Option<Bytes>, B2Error> {
        match self.inner.poll() {
            Ok(Async::Ready(Some(bytes))) => Ok(Async::Ready(Some(bytes.freeze()))),
            Ok(Async::Ready(None)) => Ok(Async::Ready(None)),
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(err) => Err(err.into()),
        }
    }
}

/// Collect a chunked stream to a `Vec<u8>`.
///
/// The internal vector will initially have a capacity of `size_hint`.
pub fn collect_stream<S>(stream: S, size_hint: usize) -> Collect<S>
where
    S: Stream<Item = Bytes>
{
    Collect {
        stream,
        buf: Vec::with_capacity(size_hint),
    }
}

/// Collects a stream of chunks of bytes to a single `Vec<u8>`.
///
/// This type is usually constructed with the function [`collect_stream`].
///
/// [`collect_stream`]: fn.collect_stream.html
pub struct Collect<S> {
    stream: S,
    buf: Vec<u8>,
}

impl<S: Stream<Item = Bytes>> Future for Collect<S> {
    type Item = Vec<u8>;
    type Error = S::Error;
    fn poll(&mut self) -> Poll<Vec<u8>, Self::Error> {
        loop {
            match self.stream.poll() {
                Ok(Async::Ready(Some(chunk))) => {
                    self.buf.extend_from_slice(&chunk[..])
                },
                Ok(Async::Ready(None)) => {
                    let buf = mem::replace(&mut self.buf, Vec::new());
                    return Ok(Async::Ready(buf));
                },
                Ok(Async::NotReady) => return Ok(Async::NotReady),
                Err(err) => return Err(err),
            }
        }
    }
}

/// Pipe a stream of chunks to an [`AsyncWrite`].
///
/// This future resolves to the sink.
///
/// [`AsyncWrite`]: https://docs.rs/tokio-io/0.1/tokio_io/trait.AsyncWrite.html
pub fn pipe<S, W>(stream: S, sink: W) -> StreamPipe<S, W>
where
    S: Stream<Item = Bytes, Error = B2Error>,
    W: AsyncWrite
{
    StreamPipe {
        from: stream,
        to: Some(sink),
        chunk: None,
    }
}

/// A future that completes when everything in a [`Stream`] has been piped to an
/// [`AsyncWrite`].
///
/// Created by [`pipe`]. This future resolves to the [`AsyncWrite`] that the data will be
/// written to.
///
/// [`Stream`]: https://docs.rs/tokio/0.1/tokio/fs/struct.File.html
/// [`AsyncWrite`]: https://docs.rs/tokio-io/0.1/tokio_io/trait.AsyncWrite.html
/// [`pipe`]: fn.pipe.html
pub struct StreamPipe<S, W> {
    from: S,
    to: Option<W>,
    chunk: Option<Bytes>,
}
impl<S, W> StreamPipe<S, W>
where
    S: Stream<Item = Bytes, Error = B2Error>,
    W: AsyncWrite
{
    #[inline]
    fn push_chunk(&mut self, chunk: Bytes)
    -> Result<Option<Bytes>, Poll<W, B2Error>> {
        match self.to.as_mut().unwrap().poll_write(&chunk[..]) {
            Ok(Async::Ready(len)) => {
                if len < chunk.len() {
                    Ok(Some(chunk.slice_from(len)))
                } else {
                    Ok(None)
                }
            },
            Ok(Async::NotReady) => {
                self.chunk = Some(chunk);
                Err(Ok(Async::NotReady))
            },
            Err(err) => {
                self.chunk = Some(chunk);
                Err(Err(err.into()))
            },
        }
    }
    #[inline]
    fn pull_chunk(&mut self) -> Result<Option<Bytes>, Poll<W, B2Error>> {
        match self.from.poll() {
            Ok(Async::Ready(Some(chunk))) => {
                self.push_chunk(chunk)
            },
            Ok(Async::Ready(None)) => {
                Err(Ok(Async::Ready(self.to.take().unwrap())))
            },
            Ok(Async::NotReady) => {
                Err(Ok(Async::NotReady))
            },
            Err(err) => {
                Err(Err(err))
            },
        }
    }
}
impl<S, W> Future for StreamPipe<S, W>
where
    S: Stream<Item = Bytes, Error = B2Error>,
    W: AsyncWrite
{
    type Item = W;
    type Error = B2Error;
    fn poll(&mut self) -> Poll<W, B2Error> {
        let mut mchunk = self.chunk.take();
        loop {
            match mchunk {
                Some(chunk) => {
                    mchunk = match self.push_chunk(chunk) {
                        Ok(a) => a,
                        Err(a) => return a,
                    }
                },
                None => {
                    mchunk = match self.pull_chunk() {
                        Ok(a) => a,
                        Err(a) => return a,
                    }
                },
            }
        }
    }
}
