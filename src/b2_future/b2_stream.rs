use std::future::Future;
use std::task::{Context, Poll};
use std::pin::Pin;
use futures::stream::{Stream, FusedStream};
use http::response::Parts;
use http::StatusCode;
use hyper::{client::ResponseFuture, Body};
use serde::de::DeserializeOwned;
use serde_json::from_slice;

use std::cmp::min;
use std::mem;
use std::fmt;

use crate::B2Error;

#[path = "partial_json.rs"]
mod partial_json;

use self::partial_json::PartialJson;

/// A stream that reads a json list from a `ResponseFuture` and parses each element with
/// `serde_json`
#[must_use = "streams do nothing unless you poll them"]
pub struct B2Stream<T> {
    state: State<T>,
    capacity: usize,
}
enum State<T> {
    Connecting(ResponseFuture),
    Collecting(Body, PartialJson<T>),
    CollectingError(Parts, Body, Vec<u8>),
    FailImmediately(B2Error),
    Done(),
}
// The ResponseFuture does not implement Sync, but since it can only be accessed through
// &mut methods, it is not possible to synchronously access it.
unsafe impl<T> Sync for State<T> {}
// The compiler adds a T: Send bound, but it is not needed as we don't store any Ts.
unsafe impl<T> Send for State<T> {}
// The compiler adds a T: Unpin bound, but it is not needed as we don't store any Ts.
impl<T> Unpin for State<T> {}

impl<T> fmt::Debug for B2Stream<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.state {
            State::Connecting(_) => f.pad("B2Stream(connecting)"),
            State::Collecting(_, _) => f.pad("B2Stream(receiving)"),
            State::CollectingError(_, _, _) => f.pad("B2Stream(api error)"),
            State::FailImmediately(_) => f.pad("B2Stream(failed)"),
            State::Done() => f.pad("B2Stream(done)"),
        }
    }
}

impl<T: DeserializeOwned> B2Stream<T> {
    /// Create a new `B2Stream`. The `capacity` is the initial size of the allocation
    /// meant to hold the body of the response.
    pub fn new(resp: ResponseFuture, capacity: usize) -> Self {
        B2Stream {
            state: State::Connecting(resp),
            capacity,
        }
    }
    /// Create a `B2Stream` that immediately fails with the specified error.
    pub fn err<E: Into<B2Error>>(err: E) -> Self {
        B2Stream {
            state: State::FailImmediately(err.into()),
            capacity: 0,
        }
    }
    /// Turn the provided `B2Future` into a `B2Stream`. This function arbitrarily
    /// changes which type the data is parsed into, so it is up to you to ensure that the
    /// new type is correct, or serde will fail deserialization.
    pub fn from_b2_future<U>(fut: super::B2Future<U>, cap: usize) -> Self {
        use super::State as FutState;
        match fut.state {
            FutState::Connecting(fut) => B2Stream {
                state: State::Connecting(fut),
                capacity: cap,
            },
            FutState::Collecting(parts, body, vec) =>
                if parts.status == StatusCode::OK {
                    let partial = PartialJson::from_vec(vec, 2);
                    B2Stream {
                        state: State::Collecting(body, partial),
                        capacity: cap,
                    }
                } else {
                    B2Stream {
                        state: State::CollectingError(parts, body, vec),
                        capacity: cap,
                    }
                },
            FutState::FailImmediately(err) => B2Stream {
                state: State::FailImmediately(err),
                capacity: cap,
            },
            FutState::Done(_) => B2Stream {
                state: State::Done(),
                capacity: cap,
            },
        }
    }
}
impl<T: DeserializeOwned> FusedStream for B2Stream<T> {
    /// Returns `true` if this stream has completed.
    fn is_terminated(&self) -> bool {
        match self.state {
            State::Done() => true,
            _ => false,
        }
    }
}
impl<T: DeserializeOwned> Stream for B2Stream<T> {
    type Item = Result<T, B2Error>;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>)
        -> Poll<Option<Result<T, B2Error>>>
    {
        let this = self.get_mut();
        let cap = this.capacity;
        let state_ref = &mut this.state;
        loop {
            if let Some(poll) = state_ref.poll(cx, cap) {
                return poll;
            }
        }
    }
}

impl<T: DeserializeOwned> State<T> {
    #[inline]
    fn poll(&mut self, cx: &mut Context<'_>, cap: usize)
        -> Option<Poll<Option<Result<T, B2Error>>>>
    {
        match self {
            State::Connecting(ref mut fut) => {
                match Pin::new(fut).poll(cx) {
                    Poll::Pending => {
                        Some(Poll::Pending)
                    }
                    Poll::Ready(Ok(resp)) => {
                        let (parts, body) = resp.into_parts();
                        if parts.status == StatusCode::OK {
                            let json = PartialJson::new(cap, 2);
                            *self = State::Collecting(body, json);
                        } else {
                            let size = min(crate::get_content_length(&parts), 0x1000);
                            *self = State::CollectingError(parts, body,
                                                           Vec::with_capacity(size));
                        }
                        None
                    }
                    Poll::Ready(Err(e)) => {
                        *self = State::Done();
                        Some(Poll::Ready(Some(Err(e.into()))))
                    }
                }
            }
            State::Collecting(ref mut body, ref mut json) => match json.next() {
                Ok(Some(value)) => {
                    Some(Poll::Ready(Some(Ok(value))))
                }
                Ok(None) => {
                    match Pin::new(body).poll_next(cx) {
                        Poll::Pending => Some(Poll::Pending),
                        Poll::Ready(Some(Ok(chunk))) => {
                            json.push(&chunk[..]);
                            None
                        }
                        Poll::Ready(None) => {
                            Some(Poll::Ready(None))
                        }
                        Poll::Ready(Some(Err(e))) => {
                            *self = State::Done();
                            Some(Poll::Ready(Some(Err(e.into()))))
                        }
                    }
                }
                Err(err) => {
                    *self = State::Done();
                    Some(Poll::Ready(Some(Err(err.into()))))
                }
            },
            State::CollectingError(ref parts, ref mut body, ref mut bytes) => {
                match Pin::new(body).poll_next(cx) {
                    Poll::Pending => Some(Poll::Pending),
                    Poll::Ready(Some(Ok(chunk))) => {
                        bytes.extend(chunk.as_ref());
                        None
                    }
                    Poll::Ready(None) => match from_slice(&bytes) {
                        Ok(err_msg) => {
                            let err = B2Error::B2Error(parts.status, err_msg);
                            *self = State::Done();
                            Some(Poll::Ready(Some(Err(err.into()))))
                        }
                        Err(err) => {
                            *self = State::Done();
                            Some(Poll::Ready(Some(Err(err.into()))))
                        }
                    },
                    Poll::Ready(Some(Err(err))) => {
                        *self = State::Done();
                        Some(Poll::Ready(Some(Err(err.into()))))
                    }
                }
            }
            State::FailImmediately(err) => {
                // Put in a dummy error
                let err = mem::replace(err, B2Error::ApiInconsistency(String::new()));
                *self = State::Done();
                Some(Poll::Ready(Some(Err(err))))
            }
            State::Done() => {
                Some(Poll::Ready(None))
            }
        }
    }
}

