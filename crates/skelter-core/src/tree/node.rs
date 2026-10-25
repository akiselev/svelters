//! Per-node records: flags, dirty bits, the core column, the layout box and the prop store.

use crate::ids::{KindId, PropId, SignalId, SlotId};
use crate::value::Value;
use skelter_base::geom::Size;
use skelter_base::ids::{NodeId, Revision, WindowId};
use smallvec::SmallVec;

macro_rules! bits {
    ($(#[$m:meta])* pub struct $name:ident($t:ty) { $($(#[$fm:meta])* const $f:ident = $v:expr;)* }) => {
        $(#[$m])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
        pub struct $name(pub $t);

        impl $name {
            $($(#[$fm])* pub const $f: $name = $name($v);)*
            pub const EMPTY: $name = $name(0);
            #[inline]
            pub const fn contains(self, o: $name) -> bool {
                self.0 & o.0 == o.0
            }
            #[inline]
            pub const fn intersects(self, o: $name) -> bool {
                self.0 & o.0 != 0
            }
            #[inline]
            pub fn insert(&mut self, o: $name) {
                self.0 |= o.0;
            }
            #[inline]
            pub fn remove(&mut self, o: $name) {
                self.0 &= !o.0;
            }
            #[inline]
            pub const fn union(self, o: $name) -> $name {
                $name(self.0 | o.0)
            }
            #[inline]
            pub const fn is_empty(self) -> bool {
                self.0 == 0
            }
        }

        impl std::ops::BitOr for $name {
            type Output = $name;
            fn bitor(self, o: $name) -> $name {
                $name(self.0 | o.0)
            }
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let mut first = true;
                write!(f, "{}(", stringify!($name))?;
                $(if self.contains($name::$f) && $v != 0 {
                    if !first { write!(f, " | ")?; }
                    first = false;
                    write!(f, stringify!($f))?;
                })*
                let _ = first;
                write!(f, ")")
            }
        }
    };
}


bits! {
    /// Structural facts about a node.
    pub struct NodeFlags(u32) {
        /// A window's root.
        const ROOT = 1 << 0;
        /// Its size cannot depend on its content (set by a kind or by `contain: size`).
        const LAYOUT_BOUNDARY = 1 << 1;
        /// Queued in its window's relayout queue.
        const IN_RELAYOUT = 1 << 2;
        /// Queued in its window's remeasure queue.
        const IN_REMEASURE = 1 << 3;
        /// Has listeners whose signal is not passive on a scroll or wheel path.
        const NONPASSIVE = 1 << 4;
        /// A descendant has [`NodeFlags::NONPASSIVE`]: wheel scrolling of this node (a scroll
        /// container) waits for JS too, since the listener may cancel it (§11.5).
        const DESC_NONPASSIVE = 1 << 7;
        /// Its kind folds its children into its own content ([`KindSpec::folds_children`](crate::KindSpec::folds_children)).
        const FOLDS = 1 << 8;
        /// Its parent's [`Kind::child_layout`](crate::Kind::child_layout) does not show it: laid out,
        /// but not painted, hit or in the accessibility tree.
        const CONCEALED = 1 << 9;
        /// An ancestor's kind makes its descendants inert ([`Kind::children_inert`](crate::Kind::children_inert)):
        /// `:disabled`, no input.
        const INERT = 1 << 10;
        /// Its kind makes its descendants inert.
        const INERT_CHILDREN = 1 << 11;
        /// Has a kind with virtual children (rows bound on demand).
        const VIRTUAL = 1 << 5;
        /// A virtual row realised by its host (not a light child JS created).
        const ROW = 1 << 6;
        /// `scroll-into-view` was asked; the frame scrolls its scroll ancestors to reveal it.
        const WANT_SCROLL = 1 << 18;
        /// A descendant has `WANT_SCROLL`.
        const SCROLL_BELOW = 1 << 19;
        /// `scroll-into-view("center")`.
        const SCROLL_CENTER = 1 << 20;
        /// The subtree paints outside the node's box; its ink rect is kept by the recorder.
        /// (svelters-egui had this at `1 << 21` together with `SCROLL_CENTER`; they are apart here.)
        const INK_OVERFLOW = 1 << 21;
    }
}

bits! {
    /// The work a node needs. Set by the write paths, cleared by the frame phase that serves it.
    pub struct Dirty(u16) {
        /// Computed style (cascade).
        const STYLE = 1;
        /// The leaf's content changed: measure again; a changed size sets LAYOUT.
        const MEASURE = 2;
        /// The node's layout cache was cleared.
        const LAYOUT = 4;
        /// Re-record the display list.
        const PAINT = 8;
        /// Property-tree entries (transform, clip, effect, scroll) changed.
        const PROPERTY = 16;
        /// Accessibility data changed.
        const A11Y = 32;
        /// The children changed (layer items, hit list, a11y children).
        const CHILDREN = 64;
        /// Some descendant has STYLE.
        const DESC_STYLE = 128;
        /// Some descendant has LAYOUT (set up to the nearest layout boundary).
        const DESC_LAYOUT = 256;
        /// Some descendant has PAINT, PROPERTY, CHILDREN or A11Y.
        const DESC_RECORD = 512;
        /// Restyle every descendant (an inherited property changed, or a structural selector).
        const STYLE_SUBTREE = 1024;
    }
}

impl Dirty {
    /// What the record walk serves.
    pub const RECORD: Dirty = Dirty(Dirty::PAINT.0 | Dirty::PROPERTY.0 | Dirty::A11Y.0 | Dirty::CHILDREN.0);
}

bits! {
    /// Listener options (`addEventListener`'s, plus `sync`).
    pub struct ListenFlags(u8) {
        const CAPTURE = 1;
        /// Cannot call `preventDefault`: never holds up scrolling.
        const PASSIVE = 2;
        const ONCE = 4;
        /// Deliver at event time, even for a signal the kind marks async.
        const SYNC = 8;
    }
}

/// One listener of a node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Listener {
    pub signal: SignalId,
    pub flags: ListenFlags,
}

/// The hot per-node record.
#[derive(Clone, Debug)]
pub(crate) struct NodeCore {
    pub kind: KindId,
    pub parent: Option<NodeId>,
    pub slot: Option<SlotId>,
    /// The window whose tree the node is in (`None`: detached).
    pub window: Option<WindowId>,
    pub flags: NodeFlags,
    pub dirty: Dirty,
    /// The revision of the node's last change (any class).
    pub changed: Revision,
}

/// What layout keeps per node.
#[derive(Clone, Debug, Default)]
pub(crate) struct LayoutBox {
    /// Shared with every node whose style is equal ([`crate::compact::Interner`]): most nodes share
    /// one of a few dozen styles, so the column holds 8 bytes per node, not a whole `taffy::Style`.
    pub style: std::sync::Arc<taffy::Style>,
    pub cache: taffy::Cache,
    pub unrounded: taffy::Layout,
    pub layout: taffy::Layout,
    /// A leaf's last measured max-content size (the measure guard compares against it).
    pub intrinsic: Option<Size>,
    /// The out-of-flow boxes this node is the containing block of (taffy's hoisted children).
    pub hoisted: Vec<NodeId>,
}

/// The props written to a node (defaults are not stored).
#[derive(Clone, Debug, Default)]
pub(crate) struct PropStore {
    pub values: SmallVec<[(PropId, Value); 4]>,
}

impl PropStore {
    pub fn get(&self, p: PropId) -> Option<&Value> {
        self.values.iter().find(|(k, _)| *k == p).map(|(_, v)| v)
    }
    /// Stores `v` (or removes the prop for `None`); returns the previous value.
    pub fn set(&mut self, p: PropId, v: Option<Value>) -> Option<Value> {
        let at = self.values.iter().position(|(k, _)| *k == p);
        match (at, v) {
            (Some(i), Some(v)) => Some(std::mem::replace(&mut self.values[i].1, v)),
            (Some(i), None) => Some(self.values.remove(i).1),
            (None, Some(v)) => {
                self.values.push((p, v));
                None
            }
            (None, None) => None,
        }
    }
}
