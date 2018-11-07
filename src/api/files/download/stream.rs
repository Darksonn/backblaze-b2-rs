use hyper::Body;
use http::response::Parts;

use futures::{Poll, Async, Stream};
use bytes::Bytes;
use crate::stream_util::{self, Collect};

use B2Error;

/// A stream of chunks of bytes from backblaze.
///
/// Usually this type is obtained from a [`DownloadFuture`]. The functions in the module
/// [`stream_utils`] can be useful together with this type.
///
/// [`DownloadFuture`]: struct.DownloadFuture.html
/// [`stream_utils`]: ../../stream_util/index.html
pub struct DownloadStream {
    inner: Inner,
    size: Option<usize>,
}

impl DownloadStream {
    pub(crate) fn new(body: Body, parts: &Parts) -> DownloadStream {
        use http::header::CONTENT_LENGTH;
        if let Some(size_str) = parts.headers.get(CONTENT_LENGTH) {
            match size_str.to_str().map(str::parse) {
                Ok(Ok(size)) => {
                    return DownloadStream {
                        inner: Inner(body),
                        size: Some(size),
                    };
                },
                _ => {},
            }
        }
        DownloadStream {
            inner: Inner(body),
            size: None,
        }
    }
    /// Returns the remaining number of bytes in the stream if it's known.
    pub fn content_length(&self) -> Option<usize> {
        self.size
    }
    /// Returns a future resolving to a `Vec<u8>` containing the contents of the stream.
    ///
    /// Internally this method just calls [`collect_stream`] using [`content_length`] as
    /// the size hint.
    ///
    /// [`collect_stream`]: ../../stream_util/fn.collect_stream.html
    /// [`content_length`]: struct.DownloadStream.html#method.content_length
    pub fn collect_vec(self) -> Collect<Self> {
        let size = self.size.unwrap_or(1024);
        stream_util::collect_stream(self, size)
    }
}

// The purpose of this inner is to control the location of Sync in the documentation.
struct Inner(Body);
// Body does not impl Sync, but since all access to the body happens through the poll
// method on DownloadStream which is a &mut method, only one thread can access the Body at
// a time.
unsafe impl Sync for Inner {}

impl Stream for DownloadStream {
    type Item = Bytes;
    type Error = B2Error;
    fn poll(&mut self) -> Poll<Option<Bytes>, B2Error> {
        match self.inner.0.poll() {
            Ok(Async::Ready(Some(chunk))) => {
                if let Some(size) = self.size {
                    self.size = Some(size - chunk.len());
                }
                Ok(Async::Ready(Some(Bytes::from(chunk))))
            },
            Ok(Async::Ready(None)) => Ok(Async::Ready(None)),
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(err) => Err(err.into())
        }
    }
}
impl From<DownloadStream> for Body {
    fn from(stream: DownloadStream) -> Self {
        stream.inner.0
    }
}
