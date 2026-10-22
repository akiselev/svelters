//! The data the non-GUI type core reads (N4; pure, tested on every platform).
//!
//! `generated/types.rs` is written by `tools/metadata/appkit/gen_types.py` from the type facts
//! (`metadata/appkit/types/*.json`). It is `const` data with no `unsafe` (NON-GUI-TYPES §2.4): the bound
//! classes with their category, and the async members with the shape of their completion block. Every
//! other member is reached reflectively (KVC, `NSInvocation`) by its selector, so it needs no table row.

use crate::generated::types::{ASYNC, BOOL_SELS, TYPE_CLASSES};

/// Kind ids of non-GUI types (ObjC classes) start here; element kinds are small table indexes.
pub const TYPE_KIND_BASE: u32 = 1 << 16;

/// The taxonomy category of a bound class (NON-GUI-TYPES §3.1). Every native object crosses as a
/// SHARED handle; the category only shapes the JS class (value: no identity contract, mutators hidden).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cat {
    Value,
    Object,
    Statics,
}

/// A class the generated modules bind.
#[derive(Debug)]
pub struct TypeClass {
    pub name: &'static str,
    pub cat: Cat,
    pub framework: &'static str,
}

/// A method whose last argument is a completion block (an async member: JS gets a Promise).
#[derive(Debug)]
pub struct AsyncMember {
    pub class: &'static str,
    pub sel: &'static str,
    pub is_static: bool,
    /// The block's shape (`svelters_apple::blocks::SHAPES`).
    pub shape: &'static str,
    /// JS arguments before the block.
    pub args: u8,
}

/// The bound class named `name` (`TYPE_CLASSES` is sorted by name).
pub fn type_class(name: &str) -> Option<&'static TypeClass> {
    TYPE_CLASSES.binary_search_by(|c| c.name.cmp(name)).ok().map(|i| &TYPE_CLASSES[i])
}

/// The async member `sel` of `class` (static or instance), if the facts have one.
pub fn async_member(class: &str, sel: &str, is_static: bool) -> Option<&'static AsyncMember> {
    ASYNC.iter().find(|a| a.class == class && a.sel == sel && a.is_static == is_static)
}

/// Whether selector or KVC key `sel` returns BOOL per the facts. On x86-64 the runtime encodes BOOL
/// as `c` (signed char), so the reflective paths would otherwise read it back as a number.
pub fn returns_bool(sel: &str) -> bool {
    BOOL_SELS.binary_search(&sel).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_are_sorted_and_shapes_supported() {
        assert!(TYPE_CLASSES.windows(2).all(|w| w[0].name < w[1].name), "TYPE_CLASSES must be sorted by name");
        for a in ASYNC {
            assert!(svelters_apple::blocks::supported(a.shape), "{}.{}: block shape ({}) unsupported by the core", a.class, a.sel, a.shape);
            assert!(type_class(a.class).is_some(), "{} is not a bound class", a.class);
            assert_eq!(a.sel.matches(':').count(), a.args as usize + 1, "{}: the block is the last argument", a.sel);
        }
        assert_eq!(type_class("NSURL").map(|c| c.cat), Some(Cat::Value));
    }
}
