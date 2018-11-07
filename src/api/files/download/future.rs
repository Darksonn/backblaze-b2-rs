use hyper::{client::ResponseFuture, Body};
use futures::{Poll, Future, Async, Stream};
use http::response::Parts;
use http::StatusCode;

use std::mem;

use crate::B2Error;

use crate::api::files::download::DownloadStream;

/// A future waiting for a backblaze download to start.
///
/// This future resolves to the headers of the response together with a stream of the
/// bytes in the file.
///
/// Resolves to [the headers][1] of the response together with a [`DownloadStream`] with
/// the contents of the file.
///
/// [1]: https://docs.rs/http/0.1/http/response/struct.Parts.html
/// [`DownloadStream`]: struct.DownloadStream.html
pub struct DownloadFuture {
    state: State,
}


enum State {
    Connecting(ResponseFuture),
    CollectingError(Parts, Body, Vec<u8>),
    FailImmediately(B2Error),
    Done(),
}
// Body does not impl Sync, but since all access to the body happens through the poll
// method on B2Future which is a &mut method, only one thread can access the Body at
// a time.
unsafe impl Sync for State {}

impl DownloadFuture {
    /// Create a new `DownloadFuture`.
    pub(crate) fn new(resp: ResponseFuture) -> Self {
        DownloadFuture {
            state: State::Connecting(resp),
        }
    }
    /// Create a `DownloadFuture` that immediately fails with the specified error.
    pub(crate) fn err<E: Into<B2Error>>(err: E) -> Self {
        DownloadFuture {
            state: State::FailImmediately(err.into()),
        }
    }
}

impl Future for DownloadFuture {
    type Item = (Parts, DownloadStream);
    type Error = B2Error;
    fn poll(&mut self) -> Poll<(Parts, DownloadStream), B2Error> {
        let mut state = mem::replace(&mut self.state, State::Done());
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

enum Action {
    Done(Poll<(Parts, DownloadStream), B2Error>),
    Again(),
}

impl State {
    #[inline]
    fn poll(self) -> (State, Action) {
        match self {
            State::Connecting(mut fut) => {
                match fut.poll() {
                    Ok(Async::NotReady) => {
                        (State::Connecting(fut),
                        Action::Done(Ok(Async::NotReady)))
                    },
                    Ok(Async::Ready(resp)) => {
                        let (parts, body) = resp.into_parts();
                        if parts.status == StatusCode::OK
                        || parts.status == StatusCode::PARTIAL_CONTENT {
                            let stream = DownloadStream::new(body, &parts);
                            (State::Done(), Action::Done(
                                    Ok(Async::Ready((parts, stream)))))
                        } else {
                            let size = crate::get_content_length(&parts);
                            (State::CollectingError(parts, body, Vec::with_capacity(size)),
                            Action::Again())
                        }
                    },
                    Err(e) => {
                        (State::Done(), Action::Done(Err(e.into())))
                    },
                }
            },
            State::CollectingError(parts, mut body, mut bytes) => {
                match body.poll() {
                    Ok(Async::NotReady) => {
                        (State::CollectingError(parts, body, bytes),
                        Action::Done(Ok(Async::NotReady)))
                    },
                    Ok(Async::Ready(Some(chunk))) => {
                        bytes.extend(&chunk[..]);
                        (State::CollectingError(parts, body, bytes),
                        Action::Again())
                    },
                    Ok(Async::Ready(None)) => {
                        match ::serde_json::from_slice(&bytes) {
                            Ok(err_msg) => {
                                let err = B2Error::B2Error (
                                    parts.status, err_msg
                                );
                                (State::Done(),
                                Action::Done(Err(err)))
                            },
                            Err(e) => {
                                (State::Done(),
                                Action::Done(Err(e.into())))
                            },
                        }
                    },
                    Err(e) => {
                        (State::Done(), Action::Done(Err(e.into())))
                    },
                }
            }
            State::FailImmediately(err) => {
                (State::Done(), Action::Done(Err(err)))
            }
            State::Done() => {
                panic!("poll on finished backblaze_b2::files::download::DownloadFuture");
            }
        }
    }
}

