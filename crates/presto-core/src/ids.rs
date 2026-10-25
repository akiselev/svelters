//! The ids of kinds and their members (props, signals, slots).
//!
//! Each id is the 32-bit FNV-1a hash of the kind's name, the member's name and the member's type, so
//! it does not depend on the position of a row in a table: adding a prop renumbers nothing, and the
//! compiler, the generated spec and the host compute the same id from the same names. The registry
//! rejects a collision when a kind is registered ([`crate::KindRegistry::register`]). The hash is
//! also the member's fingerprint: the host compares the spec's ids with the registry's at load.

/// FNV-1a over `parts`, with a 0xff separator between parts (so `("ab", "c")` ≠ `("a", "bc")`).
pub const fn fingerprint(parts: &[&str]) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    let mut p = 0;
    while p < parts.len() {
        let bytes = parts[p].as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            h ^= bytes[i] as u32;
            h = h.wrapping_mul(0x0100_0193);
            i += 1;
        }
        h ^= 0xff;
        h = h.wrapping_mul(0x0100_0193);
        p += 1;
    }
    // 0 is reserved for "none" in the ABI.
    if h == 0 { 1 } else { h }
}

macro_rules! member_id {
    ($(#[$m:meta])* $name:ident, $tag:literal) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub u32);

        impl $name {
            /// The id of member `member` (of type `ty`) of kind `kind`.
            pub const fn of(kind: &str, member: &str, ty: &str) -> Self {
                $name(fingerprint(&[$tag, kind, member, ty]))
            }
        }
    };
}

/// A registered kind: the hash of its name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KindId(pub u32);

impl KindId {
    pub const fn of(name: &str) -> Self {
        KindId(fingerprint(&["kind", name]))
    }
}

member_id!(
    /// A prop of a kind. Common props (those every kind has) use the kind name `"*"`.
    PropId, "prop"
);
member_id!(
    /// A signal (an event a node emits: DOM-style events use the kind name `"*"`).
    SignalId, "signal"
);
member_id!(
    /// A named slot of a container kind (`header`, `footer`); `None` in the tree is the default slot.
    SlotId, "slot"
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_depend_on_names_not_positions() {
        assert_eq!(PropId::of("button", "label", "string"), PropId::of("button", "label", "string"));
        assert_ne!(PropId::of("button", "label", "string"), PropId::of("button", "label", "number"));
        assert_ne!(PropId::of("ab", "c", "t"), PropId::of("a", "bc", "t"));
        assert_ne!(PropId::of("k", "m", "t").0, SignalId::of("k", "m", "t").0);
        assert_ne!(KindId::of("box").0, 0);
    }
}
