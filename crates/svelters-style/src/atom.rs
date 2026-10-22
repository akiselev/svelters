//! [`Atom`]: the shared string type of selectors (identifiers, tags, attribute names and values).

use cssparser::ToCss;
use precomputed_hash::PrecomputedHash;
use std::borrow::Borrow;
use std::fmt;
use std::ops::Deref;
use std::sync::Arc;

/// An immutable, cheaply cloned string. Serialises as a CSS identifier.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Atom(Arc<str>);

impl Atom {
    pub fn new(s: &str) -> Atom {
        Atom(Arc::from(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Deref for Atom {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for Atom {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for Atom {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<&str> for Atom {
    fn from(s: &str) -> Atom {
        Atom::new(s)
    }
}

impl fmt::Debug for Atom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&*self.0, f)
    }
}

impl fmt::Display for Atom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl ToCss for Atom {
    fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
        cssparser::serialize_identifier(&self.0, dest)
    }
}

impl PrecomputedHash for Atom {
    fn precomputed_hash(&self) -> u32 {
        hash_str(&self.0)
    }
}

/// An attribute selector's value: an [`Atom`] that serialises as a CSS string.
#[derive(Clone, PartialEq, Eq, Hash, Default, Debug)]
pub struct AttrValue(pub Atom);

impl From<&str> for AttrValue {
    fn from(s: &str) -> AttrValue {
        AttrValue(Atom::new(s))
    }
}

impl AsRef<str> for AttrValue {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl ToCss for AttrValue {
    fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
        cssparser::serialize_string(&self.0, dest)
    }
}

/// FNV-1a over the bytes (the bloom filter and rule buckets only need a stable spread).
pub(crate) fn hash_str(s: &str) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for b in s.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}
