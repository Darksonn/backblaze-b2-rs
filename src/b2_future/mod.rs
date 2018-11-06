//! Futures that parse the `ResponseFuture` returned from hyper.

use hyper::{client::ResponseFuture, Body};
use futures::{Poll, Future, Async, Stream};
use http::response::Parts;
use http::StatusCode;
use serde::Deserialize;

use std::mem;
use std::marker::PhantomData;

use crate::B2Error;

mod b2_stream;
pub use self::b2_stream::B2Stream;

/// A future that reads all data from a hyper future and parses it with `serde_json`.
pub struct B2Future<T> {
    state: State<T>,
}
enum State<T> {
    Connecting(ResponseFuture),
    Collecting(Parts, Body, Vec<u8>),
    FailImmediately(B2Error),
    Done(PhantomData<T>),
}
// Body does not impl Sync, but since all access to the body happens through the poll
// method on B2Future which is a &mut method, only one thread can access the Body at
// a time.
unsafe impl<T> Sync for State<T> {}
// We don't actually contain any values of T, so sending this value doesn't send a value
// of T. T is only used for return values.
unsafe impl<T> Send for State<T> {}

impl<T> B2Future<T>
    where for<'de> T: Deserialize<'de>
{
    /// Create a new `B2Future`.
    pub fn new(resp: ResponseFuture) -> Self {
        B2Future {
            state: State::Connecting(resp),
        }
    }
    /// Create a `B2Future` that immediately fails with the specified error.
    pub fn err<E: Into<B2Error>>(err: E) -> Self {
        B2Future {
            state: State::FailImmediately(err.into()),
        }
    }
}
impl<T> Future for B2Future<T>
    where for<'de> T: Deserialize<'de>
{
    type Item = T;
    type Error = B2Error;
    fn poll(&mut self) -> Poll<T, B2Error> {
        let mut state = mem::replace(&mut self.state, State::Done(PhantomData));
        loop {
            let (new_state, action) = state.poll();
            state = new_state;
            match action {
                Action::Done(poll) => {
                    self.state = state;
                    return poll;
                },
                Action::Again() => { },
            }
        }
    }
}

enum Action<T> {
    Done(Poll<T, B2Error>),
    Again(),
}

impl<T> State<T>
    where for<'de> T: Deserialize<'de>
{
    #[inline]
    fn done() -> Self {
        State::Done(PhantomData)
    }
    #[inline]
    fn poll(self) -> (State<T>, Action<T>) {
        match self {
            State::Connecting(mut fut) => {
                match fut.poll() {
                    Ok(Async::NotReady) => {
                        (State::Connecting(fut),
                        Action::Done(Ok(Async::NotReady)))
                    },
                    Ok(Async::Ready(resp)) => {
                        let (parts, body) = resp.into_parts();
                        let size = crate::get_content_length(&parts);
                        (State::Collecting(parts, body, Vec::with_capacity(size)),
                        Action::Again())
                    },
                    Err(e) => {
                        (State::done(),
                        Action::Done(Err(e.into())))
                    },
                }
            }
            State::Collecting(parts, mut body, mut bytes) => {
                match body.poll() {
                    Ok(Async::NotReady) => {
                        (State::Collecting(parts, body, bytes),
                        Action::Done(Ok(Async::NotReady)))
                    },
                    Ok(Async::Ready(Some(chunk))) => {
                        bytes.extend(&chunk[..]);
                        (State::Collecting(parts, body, bytes),
                        Action::Again())
                    },
                    Ok(Async::Ready(None)) => {
                        if parts.status == StatusCode::OK {
                            match ::serde_json::from_slice(&bytes) {
                                Ok(t) => {
                                    (State::done(),
                                    Action::Done(Ok(Async::Ready(t))))
                                },
                                Err(e) => {
                                    (State::done(),
                                    Action::Done(Err(e.into())))
                                },
                            }
                        } else {
                            match ::serde_json::from_slice(&bytes) {
                                Ok(err_msg) => {
                                    let err = B2Error::B2Error (
                                        parts.status, err_msg
                                    );
                                    (State::done(),
                                    Action::Done(Err(err)))
                                },
                                Err(e) => {
                                    (State::done(),
                                    Action::Done(Err(e.into())))
                                },
                            }
                        }
                    },
                    Err(e) => {
                        (State::done(), Action::Done(Err(e.into())))
                    },
                }
            }
            State::FailImmediately(err) => {
                (State::done(), Action::Done(Err(err)))
            }
            State::Done(_) => {
                panic!("poll on finished backblaze_b2::b2_future::B2Future");
            }
        }
    }
}

