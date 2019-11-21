use std::collections::VecDeque;
use std::marker::PhantomData;
use serde::de::DeserializeOwned;

use serde_json::{from_reader, from_slice};
use bytes::Bytes;
use std::io::{Read, Cursor};

use crate::B2Error;

pub struct PartialJson<T> {
    buffer: VecDeque<u8>,
    parens: u32,
    level: u32,
    in_string: bool,
    last_was_escape: bool,
    last_was_start: bool,
    i: usize,
    phantom: PhantomData<T>,
}
impl<T: DeserializeOwned> PartialJson<T> {
    pub fn new(size: usize, level: u32) -> Self {
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
    pub fn push(&mut self, bytes: &Bytes) {
        self.buffer.extend(&bytes[..]);
    }
    fn next_value(&mut self) -> Result<T, ::serde_json::Error> {
        let i = self.i - 1;
        let res = {
            let (first, second) = self.buffer.as_slices();
            if first.len() < i {
                from_reader(
                    Cursor::new(first).chain(Cursor::new(&second[0..i - first.len()])),
                )
            } else {
                from_slice(&first[0..i])
            }
        };
        for _ in self.buffer.drain(0..self.i) {}
        self.i = 0;
        res
    }
    pub fn next(&mut self) -> Result<Option<T>, B2Error> {
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
                } else if next_char == '"' {
                    self.in_string = false;
                } else if next_char == '\\' {
                    self.last_was_escape = true;
                }
            } else {
                match next_char {
                    '[' => {
                        self.parens += 1;
                        self.last_was_start = self.parens == self.level;
                    }
                    '{' => {
                        self.parens += 1;
                        self.last_was_start = self.parens == self.level;
                    }
                    ',' => {
                        self.last_was_start = false;
                        if self.parens == self.level {
                            return Ok(Some(self.next_value()?));
                        }
                    }
                    '"' => {
                        self.last_was_start = false;
                        self.in_string = true;
                    }
                    ']' => {
                        if self.parens == 0 {
                            return Err(B2Error::api("Invalid json"));
                        }
                        self.parens -= 1;
                        if self.parens == self.level - 1 && !self.last_was_start {
                            return Ok(Some(self.next_value()?));
                        }
                        self.last_was_start = false;
                    }
                    '}' => {
                        if self.parens == 0 {
                            return Err(B2Error::api("Invalid json"));
                        }
                        self.parens -= 1;
                        if self.parens == self.level - 1 && !self.last_was_start {
                            return Ok(Some(self.next_value()?));
                        }
                        self.last_was_start = false;
                    }
                    other => {
                        if !other.is_whitespace() {
                            self.last_was_start = false;
                        }
                    }
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
        json.push(&Bytes::from_static(JSON.as_bytes()));
        let mut res = Vec::new();
        while let Some(next) = json.next().unwrap() {
            res.push(next);
        }
        assert_eq!(res, [1, 2, 3, 4, 5]);
    }
    #[test]
    fn partial_json_test_object() {
        const JSON: &'static str = "{list: [1, 2, 3, 4, 5]}";
        let mut json: PartialJson<u32> = PartialJson::new(100, 2);
        json.push(&Bytes::from_static(JSON.as_bytes()));
        let mut res = Vec::new();
        while let Some(next) = json.next().unwrap() {
            res.push(next);
        }
        assert_eq!(res, [1, 2, 3, 4, 5]);
    }
    #[test]
    fn partial_json_test_big_item() {
        #[derive(Deserialize, Eq, PartialEq, Debug)]
        struct Item {
            a: String,
            b: Vec<u32>,
        }

        const JSON: &'static str = r#"{list: [
                { "a": "test", "b": [1, 2]},
                { "a": "test2", "b": [3, 4]}
            ]}"#;
        let mut json: PartialJson<Item> = PartialJson::new(100, 2);
        json.push(&Bytes::from_static(JSON.as_bytes()));
        let mut res = Vec::new();
        while let Some(next) = json.next().unwrap() {
            res.push(next);
        }
        assert_eq!(res, [
            Item {
                a: "test".into(),
                b: vec![1,2],
            },
            Item {
                a: "test2".into(),
                b: vec![3,4],
            }]);
    }
    #[test]
    fn partial_json_test_list() {
        const JSON: &'static str = "[[1,2,3],[1,2,3],[3,2,1]]";
        for i in 1..JSON.len() {
            let mut json: PartialJson<Vec<u32>> = PartialJson::new(0, 1);
            let mut res = Vec::new();

            json.push(&Bytes::from_static(&JSON.as_bytes()[..i]));
            while let Some(next) = json.next().unwrap() {
                res.push(next);
            }
            json.push(&Bytes::from_static(&JSON.as_bytes()[i..]));
            while let Some(next) = json.next().unwrap() {
                res.push(next);
            }
            assert_eq!(res, [vec![1, 2, 3], vec![1, 2, 3], vec![3, 2, 1]]);
        }
    }
    #[test]
    fn empty_json() {
        const JSON: &'static str = "{[ \n]}";
        for i in 1..JSON.len() {
            let mut json: PartialJson<u8> = PartialJson::new(0, 2);
            let mut res: Vec<u8> = Vec::new();

            json.push(&Bytes::from_static(&JSON.as_bytes()[..i]));
            while let Some(next) = json.next().unwrap() {
                res.push(next);
            }
            json.push(&Bytes::from_static(&JSON.as_bytes()[i..]));
            while let Some(next) = json.next().unwrap() {
                res.push(next);
            }
            assert_eq!(res.len(), 0);
        }
    }
}
