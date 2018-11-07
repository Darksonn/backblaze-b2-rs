use serde::de::{self, Deserialize, Error, Unexpected, Visitor};
use serde::ser::{Serialize, SerializeSeq, Serializer};
use std::fmt;

/// The capabilities of a backblaze authorization.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
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
        }
    }
    /// Returns the number of capabilities set to `true`.
    pub fn len(self) -> usize {
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
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
}

impl Serialize for Capabilities {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(self.len()))?;
        if self.list_keys {
            seq.serialize_element("listKeys")?;
        }
        if self.write_keys {
            seq.serialize_element("writeKeys")?;
        }
        if self.delete_keys {
            seq.serialize_element("deleteKeys")?;
        }
        if self.list_buckets {
            seq.serialize_element("listBuckets")?;
        }
        if self.write_buckets {
            seq.serialize_element("writeBuckets")?;
        }
        if self.delete_buckets {
            seq.serialize_element("deleteBuckets")?;
        }
        if self.list_files {
            seq.serialize_element("listFiles")?;
        }
        if self.read_files {
            seq.serialize_element("readFiles")?;
        }
        if self.share_files {
            seq.serialize_element("shareFiles")?;
        }
        if self.write_files {
            seq.serialize_element("writeFiles")?;
        }
        if self.delete_files {
            seq.serialize_element("deleteFiles")?;
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
    fn expecting(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
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
                _ => return Err(A::Error::invalid_value(
                    Unexpected::Str(next),
                    &"one of listKeys, writeKeys, deleteKeys, listBuckets, writeBuckets, deleteBuckets, listFiles, readFiles, shareFiles, writeFiles, or deleteFiles."
                )),
            }
        }
        Ok(res)
    }
}
