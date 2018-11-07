use hyper::{client::ResponseFuture, Body};
use futures::{Poll, Future, Async, Stream};
use http::response::Parts;
use http::StatusCode;
use bytes::Bytes;
use serde::Deserialize;
use serde_json::{from_slice, from_reader};

use std::mem;
use std::marker::PhantomData;
use std::collections::VecDeque;
use std::io::{Read, Cursor};

use B2Error;

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
// Body does not impl Sync, but since all access to the body happens through the poll
// method on B2Stream which is a &mut method, only one thread can access the Body at
// a time.
unsafe impl<T> Sync for State<T> {}
// We don't actually contain any values of T, so sending this value doesn't send a value
// of T. T is only used for return values.
unsafe impl<T> Send for State<T> {}

impl<T> B2Stream<T>
    where for<'de> T: Deserialize<'de>
{
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
impl<T> Stream for B2Stream<T>
    where for<'de> T: Deserialize<'de>
{
    type Item = T;
    type Error = B2Error;
    fn poll(&mut self) -> Poll<Option<T>, B2Error> {
        let mut state = mem::replace(&mut self.state, State::Done());
        loop {
            let (new_state, action) = state.poll(self.capacity, self.level);
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
    Done(Poll<Option<T>, B2Error>),
    Again(),
}

impl<T> State<T>
    where for<'de> T: Deserialize<'de>
{
    #[inline]
    fn poll(self, cap: usize, level: u32) -> (State<T>, Action<T>) {
        match self {
            State::Connecting(mut fut) => {
                match fut.poll() {
                    Ok(Async::NotReady) => {
                        (State::Connecting(fut),
                        Action::Done(Ok(Async::NotReady)))
                    },
                    Ok(Async::Ready(resp)) => {
                        let (parts, body) = resp.into_parts();
                        if parts.status == StatusCode::OK {
                            (State::Collecting(body, PartialJson::new(cap, level)),
                            Action::Again())
                        } else {
                            let size = crate::get_content_length(&parts);
                            (State::CollectingError(parts, body, Vec::with_capacity(size)),
                            Action::Again())
                        }
                    },
                    Err(e) => {
                        (State::Done(),
                        Action::Done(Err(e.into())))
                    },
                }
            }
            State::Collecting(mut body, mut partial) => {
                match partial.next() {
                    Ok(Some(value)) => {
                        (State::Collecting(body, partial),
                        Action::Done(Ok(Async::Ready(Some(value)))))
                    },
                    Ok(None) => match body.poll() {
                        Ok(Async::NotReady) => {
                            (State::Collecting(body, partial),
                            Action::Done(Ok(Async::NotReady)))
                        },
                        Ok(Async::Ready(Some(chunk))) => {
                            partial.push(chunk.into());
                            (State::Collecting(body, partial),
                            Action::Again())
                        },
                        Ok(Async::Ready(None)) => {
                            (State::Done(), Action::Done(Ok(Async::Ready(None))))
                        },
                        Err(e) => {
                            (State::Done(), Action::Done(Err(e.into())))
                        },
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
                        match from_slice(&bytes) {
                            Ok(err_msg) => {
                                let err = B2Error::B2Error(parts.status, err_msg);
                                (State::Done(), Action::Done(Err(err)))
                            },
                            Err(e) => {
                                (State::Done(), Action::Done(Err(e.into())))
                            },
                        }
                    },
                    Err(e) => {
                        (State::Done(), Action::Done(Err(e.into())))
                    },
                }
            },
            State::FailImmediately(err) => {
                (State::Done(), Action::Done(Err(err)))
            },
            State::Done() => {
                panic!("poll on finished backblaze_b2::b2_stream::B2Stream");
            },
        }
    }
}




struct PartialJson<T> {
    buffer: VecDeque<u8>,
    parens: u32,
    level: u32,
    in_string: bool,
    last_was_escape: bool,
    last_was_start: bool,
    i: usize,
    phantom: PhantomData<T>,
}
impl<T> PartialJson<T>
where
    for<'de> T: Deserialize<'de>
{
    fn new(size: usize, level: u32) -> Self {
        PartialJson {
            buffer: VecDeque::with_capacity(size),
            parens: 0,
            level,
            in_string: false,
            last_was_escape: false,
            last_was_start: false,
            i: 0,
            phantom: PhantomData,
        }
    }
    fn push(&mut self, bytes: Bytes) {
        self.buffer.extend(&bytes[..]);
    }
    fn next_value(&mut self) -> Result<T, ::serde_json::Error> {
        let i = self.i - 1;
        let res = {
            let (first, second) = self.buffer.as_slices();
            if first.len() < i {
                from_reader(
                    Cursor::new(first)
                    .chain(Cursor::new(&second[0..i-first.len()])))
            } else {
                from_slice(&first[0..i])
            }
        };
        for _ in self.buffer.drain(0..self.i) {}
        self.i = 0;
        res
    }
    fn next(&mut self) -> Result<Option<T>, B2Error> {
        loop {
            if self.i == self.buffer.len() {
                return Ok(None);
            }
            let next_char = self.buffer[self.i] as char;
            if self.parens < self.level {
                self.buffer.pop_front();
            } else {
                self.i += 1;
            }
            if self.in_string {
                if self.last_was_escape {
                    self.last_was_escape = false;
                } else {
                    if next_char == '"' {
                        self.in_string = false;
                    } else if next_char == '\\' {
                        self.last_was_escape = true;
                    }
                }
            } else {
                match next_char {
                    '[' => {
                        self.parens += 1;
                        self.last_was_start = self.parens == self.level;
                    },
                    '{' => {
                        self.parens += 1;
                        self.last_was_start = self.parens == self.level;
                    },
                    ',' => {
                        self.last_was_start = false;
                        if self.parens == self.level {
                            return Ok(Some(self.next_value()?));
                        }
                    },
                    '"' => {
                        self.last_was_start = false;
                        self.in_string = true;
                    },
                    ']' => {
                        if self.parens == 0 {
                            return Err(B2Error::api("Invalid json"))
                        }
                        self.parens -= 1;
                        if self.parens == self.level - 1 && !self.last_was_start {
                            return Ok(Some(self.next_value()?));
                        }
                        self.last_was_start = false;
                    },
                    '}' => {
                        if self.parens == 0 {
                            return Err(B2Error::api("Invalid json"))
                        }
                        self.parens -= 1;
                        if self.parens == self.level - 1 && !self.last_was_start {
                            return Ok(Some(self.next_value()?));
                        }
                        self.last_was_start = false;
                    },
                    other => {
                        if !other.is_whitespace() {
                            self.last_was_start = false;
                        }
                    },
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PartialJson;
    use bytes::Bytes;
    #[test]
    fn partial_json_test() {
        const JSON: &'static str = "[1, 2, 3, 4, 5]";
        let mut json: PartialJson<u32> = PartialJson::new(100, 1);
        json.push(Bytes::from_static(JSON.as_bytes()));
        let mut res = Vec::new();
        while let Some(next) = json.next().unwrap() {
            res.push(next);
        }
        assert_eq!(res, [1, 2, 3, 4, 5]);
    }
    #[test]
    fn partial_json_test_2() {
        const JSON: &'static str = "[[1,2,3],[1,2,3],[3,2,1]]";
        for i in 1..JSON.len() {
            let mut json: PartialJson<Vec<u32>> = PartialJson::new(0, 1);
            let mut res = Vec::new();

            json.push(Bytes::from_static(&JSON.as_bytes()[..i]));
            while let Some(next) = json.next().unwrap() {
                res.push(next);
            }
            json.push(Bytes::from_static(&JSON.as_bytes()[i..]));
            while let Some(next) = json.next().unwrap() {
                res.push(next);
            }
            assert_eq!(res, [vec![1,2,3],vec![1,2,3],vec![3,2,1]]);
        }
    }
    #[test]
    fn empty_json() {
        const JSON: &'static str = "{[ \n]}";
        for i in 1..JSON.len() {
            let mut json: PartialJson<u8> = PartialJson::new(0, 2);
            let mut res: Vec<u8> = Vec::new();

            json.push(Bytes::from_static(&JSON.as_bytes()[..i]));
            while let Some(next) = json.next().unwrap() {
                res.push(next);
            }
            json.push(Bytes::from_static(&JSON.as_bytes()[i..]));
            while let Some(next) = json.next().unwrap() {
                res.push(next);
            }
            assert_eq!(res.len(), 0);
        }
    }
}
