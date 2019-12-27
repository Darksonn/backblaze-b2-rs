use serde::de::{self, Deserialize, Visitor};
use serde::ser::{Serialize, SerializeSeq, Serializer};
use std::fmt;

/// The capabilities of a backblaze authorization.
///
/// This type is serialized as a list of strings.
#[derive(Clone, PartialEq, Eq)]
pub struct Capabilities {
    pub list_keys: bool,
    pub write_keys: bool,
    pub delete_keys: bool,
    pub list_buckets: bool,
    pub write_buckets: bool,
    pub delete_buckets: bool,
    pub list_files: bool,
    pub read_files: bool,
    pub share_files: bool,
    pub write_files: bool,
    pub delete_files: bool,
    _non_exhaustive: (),
}
impl Capabilities {
    /// Create a new `Capabilities` with everything set to `false`.
    pub fn empty() -> Self {
        Capabilities {
            list_keys: false,
            write_keys: false,
            delete_keys: false,
            list_buckets: false,
            write_buckets: false,
            delete_buckets: false,
            list_files: false,
            read_files: false,
            share_files: false,
            write_files: false,
            delete_files: false,
            _non_exhaustive: (),
        }
    }
    /// Create a new `Capabilities` with everything set to `true`.
    pub fn all() -> Self {
        Capabilities {
            list_keys: true,
            write_keys: true,
            delete_keys: true,
            list_buckets: true,
            write_buckets: true,
            delete_buckets: true,
            list_files: true,
            read_files: true,
            share_files: true,
            write_files: true,
            delete_files: true,
            _non_exhaustive: (),
        }
    }
    /// Returns the number of capabilities set to `true`.
    ///
    /// # Example
    ///
    /// ```
    /// use backblaze_b2::auth::Capabilities;
    ///
    /// let mut cap = Capabilities::empty();
    /// cap.read_files = true;
    ///
    /// assert_eq!(cap.len(), 1);
    ///
    /// cap.write_files = true;
    ///
    /// assert_eq!(cap.len(), 2);
    /// ```
    pub fn len(&self) -> usize {
        self.list_keys as usize
            + self.write_keys as usize
            + self.delete_keys as usize
            + self.list_buckets as usize
            + self.write_buckets as usize
            + self.delete_buckets as usize
            + self.list_files as usize
            + self.read_files as usize
            + self.share_files as usize
            + self.write_files as usize
            + self.delete_files as usize
    }
    /// Returns true if this key has no capabilities.
    ///
    /// # Example
    ///
    /// ```
    /// use backblaze_b2::auth::Capabilities;
    ///
    /// let mut cap = Capabilities::empty();
    ///
    /// assert!(cap.is_empty());
    ///
    /// cap.read_files = true;
    ///
    /// assert!(!cap.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Iterate over the capabilities in this `Capabilities`.
    ///
    /// # Example
    ///
    /// ```
    /// use backblaze_b2::auth::Capabilities;
    ///
    /// // Create our capabilities value.
    /// let mut cap = Capabilities::empty();
    /// cap.read_files = true;
    ///
    /// // Create a list from the iterator.
    /// let list: Vec<&'static str> = cap.iter().collect();
    /// assert_eq!(list, vec!["readFiles"]);
    /// ```
    pub fn iter(&self) -> CapabilitiesIter {
        CapabilitiesIter { c: self.clone(), i: 0 }
    }
}

impl IntoIterator for Capabilities {
    type Item = &'static str;
    type IntoIter = CapabilitiesIter;
    fn into_iter(self) -> CapabilitiesIter {
        self.iter()
    }
}
impl<'a> IntoIterator for &'a Capabilities {
    type Item = &'static str;
    type IntoIter = CapabilitiesIter;
    fn into_iter(self) -> CapabilitiesIter {
        self.iter()
    }
}

/// An iterator over a [`Capabilities`].
///
/// # Example
///
/// ```
/// use backblaze_b2::auth::Capabilities;
///
/// // Create our capabilities value.
/// let mut cap = Capabilities::empty();
/// cap.read_files = true;
///
/// // Create a list from the iterator.
/// let list: Vec<&'static str> = cap.iter().collect();
/// assert_eq!(list, vec!["readFiles"]);
/// ```
///
/// [`Capabilities`]: struct.Capabilities.html
#[derive(Clone, Debug)]
pub struct CapabilitiesIter {
    c: Capabilities,
    i: u8,
}
impl Iterator for CapabilitiesIter {
    type Item = &'static str;
    /// Returns the next capability.
    #[inline]
    fn next(&mut self) -> Option<&'static str> {
        loop {
            self.i = self.i.wrapping_add(1);
            match self.i {
                1 => if self.c.list_keys { return Some("listKeys"); },
                2 => if self.c.write_keys { return Some("writeKeys"); },
                3 => if self.c.delete_keys { return Some("deleteKeys"); },
                4 => if self.c.list_buckets { return Some("listBuckets"); },
                5 => if self.c.write_buckets { return Some("writeBuckets"); },
                6 => if self.c.delete_buckets { return Some("deleteBuckets"); },
                7 => if self.c.list_files { return Some("listFiles"); },
                8 => if self.c.read_files { return Some("readFiles"); },
                9 => if self.c.share_files { return Some("shareFiles"); },
                10 => if self.c.write_files { return Some("writeFiles"); },
                11 => if self.c.delete_files { return Some("deleteFiles"); },
                _ => return None,
            }
        }
    }
}

impl fmt::Debug for Capabilities {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut list = f.debug_list();
        for cap in self.iter() {
            list.entry(&cap);
        }
        list.finish()
    }
}

impl Default for Capabilities {
    /// Create a new `Capabilities` with everything set to `false`.
    fn default() -> Capabilities {
        Capabilities::empty()
    }
}

impl Serialize for Capabilities {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(self.len()))?;
        for cap in self.iter() {
            seq.serialize_element(cap)?;
        }
        seq.end()
    }
}

impl<'de> Deserialize<'de> for Capabilities {
    fn deserialize<D>(deserializer: D) -> Result<Capabilities, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_seq(CapabilityVisitor)
    }
}

struct CapabilityVisitor;

impl<'de> Visitor<'de> for CapabilityVisitor {
    type Value = Capabilities;
    fn expecting(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "A list of capabilties.")
    }
    fn visit_seq<A>(self, mut seq: A) -> Result<Capabilities, A::Error>
    where
        A: de::SeqAccess<'de>,
    {
        let mut res = Capabilities::empty();
        while let Some(next) = seq.next_element::<&'de str>()? {
            match next {
                "listKeys" => res.list_keys = true,
                "writeKeys" => res.write_keys = true,
                "deleteKeys" => res.delete_keys = true,
                "listBuckets" => res.list_buckets = true,
                "writeBuckets" => res.write_buckets = true,
                "deleteBuckets" => res.delete_buckets = true,
                "listFiles" => res.list_files = true,
                "readFiles" => res.read_files = true,
                "shareFiles" => res.share_files = true,
                "writeFiles" => res.write_files = true,
                "deleteFiles" => res.delete_files = true,
                _ => { /* Ignore unknown to be forward compatible with b2 api. */ },
            }
        }
        Ok(res)
    }
}
