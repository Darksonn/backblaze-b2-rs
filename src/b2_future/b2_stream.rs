use std::future::Future;
use std::task::{Context, Poll};
use std::pin::Pin;
use futures::stream::Stream;
use http::response::Parts;
use http::StatusCode;
use hyper::{client::ResponseFuture, Body};
use serde::de::DeserializeOwned;
use serde_json::from_slice;

use std::cmp::min;
use std::mem;

use crate::B2Error;

#[path = "partial_json.rs"]
mod partial_json;

use self::partial_json::PartialJson;

/// A stream that reads a json list from a `ResponseFuture` and parses each element with
/// `serde_json`
pub struct B2Stream<T> {
    state: State<T>,
    capacity: usize,
    level: u32,
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

impl<T: DeserializeOwned> B2Stream<T> {
    /// Create a new `B2Stream`. The `capacity` is the initial size of the allocation
    /// meant to hold the body of the response.
    ///
    /// The `level` is how far in the list starts. If the json just contains a list like
    /// `[1,2,3]`, then the level should be 1, but if the json is wrapped in an object
    /// with a single value, like in `{"buckets": [1,2,3]}` then a level of 2 would be
    /// appropriate.
    pub fn new(resp: ResponseFuture, capacity: usize, level: u32) -> Self {
        B2Stream {
            state: State::Connecting(resp),
            capacity,
            level,
        }
    }
    /// Create a `B2Stream` that immediately fails with the specified error.
    pub fn err<E: Into<B2Error>>(err: E) -> Self {
        B2Stream {
            state: State::FailImmediately(err.into()),
            capacity: 0,
            level: 0,
        }
    }
}
impl<T: DeserializeOwned> Stream for B2Stream<T> {
    type Item = Result<T, B2Error>;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context)
        -> Poll<Option<Result<T, B2Error>>>
    {
        let this = self.get_mut();
        let cap = this.capacity;
        let level = this.level;
        let state_ref = &mut this.state;
        loop {
            if let Some(poll) = state_ref.poll(cx, cap, level) {
                return poll;
            }
        }
    }
}

impl<T: DeserializeOwned> State<T> {
    #[inline]
    fn poll(&mut self, cx: &mut Context, cap: usize, level: u32)
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
                            let json = PartialJson::new(cap, level);
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
                            json.push(&chunk.into_bytes());
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

