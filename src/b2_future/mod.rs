//! Futures that parse the `ResponseFuture` returned from hyper.

use futures::future::FusedFuture;
use futures::stream::Stream;
use http::response::Parts;
use http::StatusCode;
use hyper::{client::ResponseFuture, Body};
use serde::de::DeserializeOwned;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use std::cmp::min;
use std::fmt;
use std::marker::PhantomData;
use std::mem;

use crate::B2Error;

mod b2_stream;
pub use self::b2_stream::B2Stream;

/// A future that reads all data from a hyper future and parses it with `serde_json`.
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct B2Future<T> {
    state: State<T>,
}
enum State<T> {
    Connecting(ResponseFuture),
    Collecting(Parts, Body, Vec<u8>),
    FailImmediately(B2Error),
    Done(PhantomData<T>),
}
// The ResponseFuture does not implement Sync, but since it can only be accessed through
// &mut methods, it is not possible to synchronously access it.
unsafe impl<T> Sync for State<T> {}
// The compiler adds a T: Send bound, but it is not needed as we don't store any Ts.
unsafe impl<T> Send for State<T> {}
// The compiler adds a T: Unpin bound, but it is not needed as we don't store any Ts.
impl<T> Unpin for State<T> {}

impl<T> fmt::Debug for B2Future<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.state {
            State::Connecting(_) => f.pad("B2Future(connecting)"),
            State::Collecting(_, _, _) => f.pad("B2Future(receiving)"),
            State::FailImmediately(_) => f.pad("B2Future(failed)"),
            State::Done(_) => f.pad("B2Future(done)"),
        }
    }
}

impl<T: DeserializeOwned> B2Future<T> {
    /// Create a new `B2Future`.
    pub fn new(inner: ResponseFuture) -> Self {
        B2Future {
            state: State::Connecting(inner),
        }
    }
    /// Create a `B2Future` that immediately fails with the specified error.
    pub fn err<E: Into<B2Error>>(err: E) -> Self {
        B2Future {
            state: State::FailImmediately(err.into()),
        }
    }
}
impl<T: DeserializeOwned> Future for B2Future<T> {
    type Output = Result<T, B2Error>;
    /// Attempt to resolve the future to a final value.
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<T, B2Error>> {
        let state_ref = &mut self.get_mut().state;
        loop {
            if let Some(poll) = state_ref.poll(cx) {
                return poll;
            }
        }
    }
}
impl<T: DeserializeOwned> FusedFuture for B2Future<T> {
    /// Returns `true` if this future has completed.
    fn is_terminated(&self) -> bool {
        matches!(self.state, State::Done(_))
    }
}

impl<T: DeserializeOwned> State<T> {
    #[inline]
    fn done() -> Self {
        State::Done(PhantomData)
    }
    // Poll the state. This will advance the state machine at most once, so repeatedly
    // call it until it returns Some.
    #[inline]
    fn poll(&mut self, cx: &mut Context<'_>) -> Option<Poll<Result<T, B2Error>>> {
        match self {
            State::Connecting(ref mut fut) => match Pin::new(fut).poll(cx) {
                Poll::Pending => Some(Poll::Pending),
                Poll::Ready(Ok(resp)) => {
                    let (parts, body) = resp.into_parts();
                    let size = min(crate::get_content_length(&parts), 0x1000000);
                    *self = State::Collecting(parts, body, Vec::with_capacity(size));
                    None
                }
                Poll::Ready(Err(e)) => {
                    *self = State::done();
                    Some(Poll::Ready(Err(e.into())))
                }
            },
            State::Collecting(ref parts, ref mut body, ref mut bytes) => {
                match Pin::new(body).poll_next(cx) {
                    Poll::Pending => Some(Poll::Pending),
                    Poll::Ready(Some(Ok(chunk))) => {
                        bytes.extend(chunk.as_ref());
                        None
                    }
                    Poll::Ready(None) => {
                        let result = if parts.status == StatusCode::OK {
                            match ::serde_json::from_slice(bytes) {
                                Ok(t) => Some(Poll::Ready(Ok(t))),
                                Err(e) => Some(Poll::Ready(Err(e.into()))),
                            }
                        } else {
                            match ::serde_json::from_slice(bytes) {
                                Ok(err_msg) => {
                                    let err = B2Error::B2Error(parts.status, err_msg);
                                    Some(Poll::Ready(Err(err)))
                                }
                                Err(e) => Some(Poll::Ready(Err(e.into()))),
                            }
                        };
                        *self = State::done();
                        result
                    }
                    Poll::Ready(Some(Err(e))) => {
                        *self = State::done();
                        Some(Poll::Ready(Err(e.into())))
                    }
                }
            }
            State::FailImmediately(err) => {
                // Put in a dummy error
                let err = mem::replace(err, B2Error::ApiInconsistency(String::new()));
                *self = State::done();
                Some(Poll::Ready(Err(err)))
            }
            State::Done(_) => {
                panic!("poll on finished backblaze_b2::b2_future::B2Future");
            }
        }
    }
}
