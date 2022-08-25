use std::collections::{BTreeMap, HashMap};

/// A type that can be used as file info when uploading.
///
/// Typically you would use either [`SimpleFileInfo`] or a slice of pairs as
/// your file info.
///
/// [`SimpleFileInfo`]: struct.SimpleFileInfo.html
pub trait UploadFileInfo<'a> {
    /// The kind of iterator over key-value pairs.
    type Iter: Iterator<Item = (&'a str, &'a str)>;
    /// Returns an iterator over the key-value pairs.
    fn as_iter(&'a self) -> Self::Iter;
}

/// A simple info type that allows specifying the two infos that have meaning
/// supplied by backblaze.
///
/// These are:
///
/// 1. src_last_modified_millis
/// 2. b2-content-disposition
#[derive(Debug, Clone)]
pub struct SimpleFileInfo {
    last_modified: Option<String>,
    content_disposition: Option<String>,
}
impl SimpleFileInfo {
    /// Create a new simple file info.
    pub const fn new() -> SimpleFileInfo {
        SimpleFileInfo {
            last_modified: None,
            content_disposition: None,
        }
    }
    /// Milliseconds since January 1, 1970 UTC.
    pub fn last_modified(self, val: u64) -> Self {
        SimpleFileInfo {
            last_modified: Some(format!("{}", val)),
            ..self
        }
    }
    /// If this is present, B2 will use it as the value of the
    /// `Content-Disposition` header when the file is downloaded (unless it's
    /// overridden by a value given in the download request).  The value must
    /// match the grammar specified in RFC 6266. Parameter continuations are not
    /// supported. 'Extended-value's are supported for charset 'UTF-8'
    /// (case-insensitive) when the language is empty.
    pub fn content_disposition(self, value: String) -> Self {
        SimpleFileInfo {
            content_disposition: Some(value),
            ..self
        }
    }
}
impl<'a> UploadFileInfo<'a> for SimpleFileInfo {
    type Iter = SimpleFileInfoIter<'a>;
    fn as_iter(&'a self) -> Self::Iter {
        SimpleFileInfoIter {
            last_modified: self.last_modified.as_deref(),
            content_disposition: self.content_disposition.as_deref(),
        }
    }
}
pub struct SimpleFileInfoIter<'a> {
    last_modified: Option<&'a str>,
    content_disposition: Option<&'a str>,
}
impl<'a> Iterator for SimpleFileInfoIter<'a> {
    type Item = (&'a str, &'a str);
    fn next(&mut self) -> Option<Self::Item> {
        self.last_modified
            .take()
            .map(|lm| ("src_last_modified_millis", lm))
            .or_else(|| {
                self.content_disposition
                    .take()
                    .map(|cd| ("b2-content-disposition", cd))
            })
    }
}

impl<'a, K, V> UploadFileInfo<'a> for [(K, V)]
where
    K: AsRef<str> + 'a,
    V: AsRef<str> + 'a,
{
    type Iter = SliceIter<'a, K, V>;
    /// Create an iterator over the slice.
    fn as_iter(&'a self) -> Self::Iter {
        SliceIter { real: self.iter() }
    }
}
pub struct SliceIter<'a, K, V> {
    real: std::slice::Iter<'a, (K, V)>,
}
impl<'a, K, V> Iterator for SliceIter<'a, K, V>
where
    K: AsRef<str>,
    V: AsRef<str>,
{
    type Item = (&'a str, &'a str);
    #[inline]
    fn next(&mut self) -> Option<(&'a str, &'a str)> {
        self.real.next().map(|(k, v)| (k.as_ref(), v.as_ref()))
    }
}

impl<'a, K, V> UploadFileInfo<'a> for HashMap<K, V>
where
    K: AsRef<str> + 'a,
    V: AsRef<str> + 'a,
{
    type Iter = HashMapIter<'a, K, V>;
    /// Create an iterator over the `HashMap`.
    fn as_iter(&'a self) -> Self::Iter {
        HashMapIter { real: self.iter() }
    }
}

pub struct HashMapIter<'a, K, V> {
    real: std::collections::hash_map::Iter<'a, K, V>,
}
impl<'a, K, V> Iterator for HashMapIter<'a, K, V>
where
    K: AsRef<str>,
    V: AsRef<str>,
{
    type Item = (&'a str, &'a str);
    #[inline]
    fn next(&mut self) -> Option<(&'a str, &'a str)> {
        self.real.next().map(|(k, v)| (k.as_ref(), v.as_ref()))
    }
}

impl<'a, K, V> UploadFileInfo<'a> for BTreeMap<K, V>
where
    K: AsRef<str> + 'a,
    V: AsRef<str> + 'a,
{
    type Iter = BTreeIter<'a, K, V>;
    /// Create an iterator over the `BTreeMap`.
    fn as_iter(&'a self) -> Self::Iter {
        BTreeIter { real: self.iter() }
    }
}

pub struct BTreeIter<'a, K, V> {
    real: std::collections::btree_map::Iter<'a, K, V>,
}
impl<'a, K, V> Iterator for BTreeIter<'a, K, V>
where
    K: AsRef<str>,
    V: AsRef<str>,
{
    type Item = (&'a str, &'a str);
    #[inline]
    fn next(&mut self) -> Option<(&'a str, &'a str)> {
        self.real.next().map(|(k, v)| (k.as_ref(), v.as_ref()))
    }
}
