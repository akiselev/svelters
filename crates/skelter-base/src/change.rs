//! Change classes (architecture §4.3): what work a write causes. Every prop and every CSS property is
//! declared with one; the svelters spec carries it to the compiler.

/// Ordered by cost: a `Structure` change implies the work of every class below it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ChangeClass {
    /// Compositor only: transform, opacity, clip, scroll offset, z-raise, canvas view, `visibility`.
    Property = 0,
    /// Re-record this node's display list: colours, borders, icons, selection visuals.
    Paint = 1,
    /// Style, then layout if the intrinsic size changed, then record: text, font, padding, width, gap.
    Layout = 2,
    /// Relayout to the boundary, record, accessibility, hit list: create, insert, remove, reorder, slot.
    Structure = 3,
}

impl ChangeClass {
    /// The spec's spelling (`tessel.json`/`skelter.json` `change` field).
    pub const fn name(self) -> &'static str {
        match self {
            ChangeClass::Property => "property",
            ChangeClass::Paint => "paint",
            ChangeClass::Layout => "layout",
            ChangeClass::Structure => "structure",
        }
    }
    pub fn from_name(s: &str) -> Option<Self> {
        Some(match s {
            "property" => ChangeClass::Property,
            "paint" => ChangeClass::Paint,
            "layout" => ChangeClass::Layout,
            "structure" => ChangeClass::Structure,
            _ => return None,
        })
    }
}
