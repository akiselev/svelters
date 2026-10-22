//! Per-node data, split by temperature into columns (ARCH-SCENE §4.3): the hot [`NodeCore`] (walked
//! every frame), [`LayoutBox`] (taffy data), [`Payload`] (the kind's behaviour: built-in [`W`] today,
//! provider widgets from RA1) and the rare, lazily boxed [`NodeCold`] (common props, visual channels,
//! observation counts). Plus the bit sets [`NodeFlags`], [`Dirty`], [`StateBits`] and [`Owner`].
//!
//! [`Node`] is a borrowed read view over the columns of one node, kept for the pre-SG1 read paths
//! (`with_scene(|s| s.n(h).children)` in tests and the eframe runner).
//! Writer lane: SCENE (RA1 adds payload variants, EM1 a few core fields).

use super::id::{NodeId, ProviderId, RootIx, Tick};
use crate::kinds::KINDS;

/// Which egui widget a node shows, with the props the widget borrows mutably.
#[derive(Clone, Debug, PartialEq)]
pub enum W {
    Window { title: String, default_w: f64, default_h: f64 },
    Box,
    Label { text: String, wrap: &'static str, selectable: bool },
    Button { text: String, selected: bool, frame: bool, small: bool },
    Checkbox { text: String, checked: bool },
    TextEdit { text: String, hint: String, multiline: bool, password: bool, char_limit: i64 },
    Slider { value: f64, min: f64, max: f64, step: f64, text: String, show_value: bool },
}

impl W {
    /// Containers lay out children with flexbox and paint nothing themselves.
    pub fn is_container(&self) -> bool {
        matches!(self, W::Box | W::Window { .. })
    }
}

/// Props every in-window node has (`Widget`), plus the box/window container props.
#[derive(Clone, Debug, PartialEq)]
pub struct Common {
    pub enabled: bool,
    pub visible: bool,
    pub tooltip: String,
    pub accessible_name: String,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub min_width: Option<f64>,
    pub min_height: Option<f64>,
    pub max_width: Option<f64>,
    pub max_height: Option<f64>,
    pub flex_grow: f64,
    pub flex_shrink: f64,
    pub flex_basis: Option<f64>,
    pub align_self: &'static str,
    pub margin: f64,
    pub flex_direction: &'static str,
    pub flex_wrap: &'static str,
    pub gap: f64,
    pub row_gap: Option<f64>,
    pub column_gap: Option<f64>,
    pub padding: f64,
    pub align_items: &'static str,
    pub justify_content: &'static str,
}

impl Common {
    pub const DEFAULT: Common = Common {
        enabled: true,
        visible: true,
        tooltip: String::new(),
        accessible_name: String::new(),
        width: None,
        height: None,
        min_width: None,
        min_height: None,
        max_width: None,
        max_height: None,
        flex_grow: 0.0,
        flex_shrink: 1.0,
        flex_basis: None,
        align_self: "auto",
        margin: 0.0,
        flex_direction: "column",
        flex_wrap: "nowrap",
        gap: 0.0,
        row_gap: None,
        column_gap: None,
        padding: 0.0,
        align_items: "stretch",
        justify_content: "start",
    };
}

impl Default for Common {
    fn default() -> Self {
        Common::DEFAULT
    }
}

/// The visual channels of a node (plans/milestones/TR1-transitions.md section 3.2). Paint-only
/// except `clip`, which also caps the node's layout size.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Visual {
    /// `SVL_STYLE_OPACITY`: multiplies the subtree's opacity.
    pub opacity: Option<f32>,
    /// `SVL_STYLE_TRANSFORM`: tx, ty (points), sx, sy, origin x, y (fractions of the size).
    pub transform: Option<[f32; 6]>,
    /// `SVL_STYLE_CLIP`: width, height the node occupies and is cut at (< 0: that axis free).
    pub clip: Option<[f32; 2]>,
    /// `SVL_STYLE_INERT`: the subtree reports no events.
    pub inert: bool,
}

impl Visual {
    pub const IDENTITY: Visual = Visual { opacity: None, transform: None, clip: None, inert: false };
    pub fn is_identity(&self) -> bool {
        *self == Visual::IDENTITY
    }
}

macro_rules! bits {
    ($(#[$m:meta])* $vis:vis struct $T:ident($R:ty) { $($(#[$fm:meta])* const $F:ident = $v:expr;)* }) => {
        $(#[$m])*
        #[derive(Copy, Clone, PartialEq, Eq, Hash, Default)]
        $vis struct $T(pub $R);
        impl $T {
            $($(#[$fm])* pub const $F: $T = $T($v);)*
            pub const EMPTY: $T = $T(0);
            pub const fn bits(self) -> $R { self.0 }
            pub const fn contains(self, o: $T) -> bool { self.0 & o.0 == o.0 }
            pub const fn intersects(self, o: $T) -> bool { self.0 & o.0 != 0 }
            pub const fn is_empty(self) -> bool { self.0 == 0 }
            pub fn insert(&mut self, o: $T) { self.0 |= o.0 }
            pub fn remove(&mut self, o: $T) { self.0 &= !o.0 }
            pub fn set(&mut self, o: $T, on: bool) { if on { self.insert(o) } else { self.remove(o) } }
        }
        impl std::ops::BitOr for $T { type Output = $T; fn bitor(self, o: $T) -> $T { $T(self.0 | o.0) } }
        impl std::ops::BitOrAssign for $T { fn bitor_assign(&mut self, o: $T) { self.0 |= o.0 } }
        impl std::ops::BitAnd for $T { type Output = $T; fn bitand(self, o: $T) -> $T { $T(self.0 & o.0) } }
        impl std::ops::Not for $T { type Output = $T; fn not(self) -> $T { $T(!self.0) } }
        impl std::fmt::Debug for $T {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let mut first = true;
                write!(f, "{}(", stringify!($T))?;
                $(if self.0 & $v != 0 && $v != 0 { if !first { write!(f, "|")?; } first = false; write!(f, stringify!($F))?; })*
                let _ = first;
                write!(f, ")")
            }
        }
    };
}

bits! {
    /// Per-node flags. The public ones may be set from Rust ([`crate::scene::NodeMut::set_flags`]);
    /// the others are the scene's own bookkeeping.
    pub struct NodeFlags(u32) {
        /// Relayout of content changes stops here: the node's size never depends on its content
        /// (ARCH-SCENE §7.1). Also implied by a definite width and height (see `scene/dirty.rs`).
        const LAYOUT_BOUNDARY = 1 << 0;
        /// Hit-tests pass through (SG3).
        const HIT_TRANSPARENT = 1 << 1;
        /// Skip the subtree when it is off screen (SG6).
        const SKIP_OFFSCREEN = 1 << 2;
        /// A window that was presented (`present`).
        const PRESENTED = 1 << 8;
        /// A text field asked for focus (`request-focus`), applied next pass.
        const WANT_FOCUS = 1 << 9;
        /// Last frame's focus, for gained/lost-focus.
        const HAD_FOCUS = 1 << 10;
        /// Has non-identity visual channels (TR1; the values are in `NodeCold::vis`).
        const HAS_VISUAL = 1 << 11;
        /// Queued in its root's relayout queue (a layout boundary with dirty content).
        const IN_RELAYOUT = 1 << 12;
        /// Queued in its root's remeasure queue.
        const IN_REMEASURE = 1 << 13;
        /// A root's top node (window, slot root).
        const ROOT = 1 << 14;
    }
}

impl NodeFlags {
    /// The flags Rust may set through `NodeMut::set_flags`.
    pub const PUBLIC: NodeFlags = NodeFlags(0xff);
}

bits! {
    /// What a node needs (ARCH-SCENE §7.1). Set by the write paths, cleared by the pass that serves
    /// it. Unstable: the values and their exact meaning may change in any sha (§16).
    pub struct Dirty(u16) {
        /// Computed style (CSS, CS5e).
        const STYLE = 1;
        /// The leaf's content changed: measure again; a changed size sets LAYOUT.
        const MEASURE = 2;
        /// The node's layout cache was cleared.
        const LAYOUT = 4;
        /// Repaint.
        const PAINT = 8;
        /// Accessibility data changed.
        const A11Y = 16;
        /// The light children changed.
        const CHILDREN = 32;
        const DESC_STYLE = 64;
        /// Some descendant has LAYOUT (set up to the nearest layout boundary).
        const DESC_LAYOUT = 128;
    }
}

bits! {
    /// State bits: 0–31 built-in (hover, pressed, focus, …, set by the walk from SG4/EV on), 32–63
    /// registered by providers (ARCH-EMBED §8.4; RA1's `state_bits!`).
    pub struct StateBits(u64) {
        const HOVERED = 1 << 0;
        const PRESSED = 1 << 1;
        const FOCUSED = 1 << 2;
        const DISABLED = 1 << 3;
        const CHECKED = 1 << 4;
        const SELECTED = 1 << 5;
    }
}

/// Who created a node, and so who may change its structure (ARCH-SCENE §5.1).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Owner {
    /// A runtime, through host ops. Its light children are the runtime's.
    Js,
    /// Rust code, through `SceneMut`.
    Rust(ProviderId),
    /// The embed itself: windows' and slots' roots.
    Host,
}

/// The hot column: what the walk and the tree edits read for every node.
#[derive(Clone, Debug)]
pub(crate) struct NodeCore {
    /// Light parent; for an attached or shadow node, its host (SG4).
    pub parent: Option<NodeId>,
    /// Index into the kind table (`kinds::KINDS`).
    pub kind: u32,
    /// The root this node is under (cached at attach), or `RootIx::DETACHED`.
    pub root: RootIx,
    pub flags: NodeFlags,
    pub dirty: Dirty,
    pub owner: Owner,
    pub state: StateBits,
    /// Listened signals: a bit per signal ordinal of the kind (`scene/mod.rs`, `member_bit`).
    pub listen: u64,
    /// Props a runtime set (CSS precedence, CS5e): a bit per prop ordinal of the kind.
    pub explicit: u64,
    pub egui_id: egui::Id,
    /// Root-relative rect from the last layout.
    pub rect: egui::Rect,
    /// What the widget actually took last frame (screen coordinates; `NOTHING` if not drawn).
    pub drawn: egui::Rect,
    /// The tick of the last change of a prop, state bit or structure of this node.
    pub changed: Tick,
}

impl NodeCore {
    pub fn new(kind: u32, owner: Owner, egui_id: egui::Id, tick: Tick) -> NodeCore {
        NodeCore {
            parent: None,
            kind,
            root: RootIx::DETACHED,
            flags: NodeFlags::EMPTY,
            dirty: Dirty::EMPTY,
            owner,
            state: StateBits::EMPTY,
            listen: 0,
            explicit: 0,
            egui_id,
            rect: egui::Rect::NOTHING,
            drawn: egui::Rect::NOTHING,
            changed: tick,
        }
    }
}

/// Taffy's data for one node (the adapter is `layout.rs`).
#[derive(Clone, Debug)]
pub(crate) struct LayoutBox {
    pub style: taffy::Style,
    pub cache: taffy::Cache,
    pub unrounded: taffy::Layout,
    pub layout: taffy::Layout,
    /// No-wrap size of the leaf's content, measured with egui's fonts; `None` = measure again.
    pub intrinsic: Option<egui::Vec2>,
}

impl LayoutBox {
    pub fn new() -> LayoutBox {
        LayoutBox {
            style: taffy::Style::default(),
            cache: taffy::Cache::new(),
            unrounded: taffy::Layout::with_order(0),
            layout: taffy::Layout::with_order(0),
            intrinsic: None,
        }
    }
}

/// The kind's behaviour and its typed props. RA1 adds `Widget(Box<dyn ErasedWidget>)`, EV2b
/// `Behavior`, EM1 `Slot`.
#[derive(Clone, Debug)]
pub(crate) enum Payload {
    /// A freed slot, or a payload taken out while its own code runs (SG4).
    Vacant,
    Builtin(W),
}

/// Rare data, boxed only when a node has some (most leaves have none).
#[derive(Clone, Debug)]
pub(crate) struct NodeCold {
    /// Common props (layout fields move to the computed style with CS5e).
    pub common: Common,
    /// TR1 visual channels.
    pub vis: Visual,
    /// R1 observation counts per prop id.
    pub observe: Vec<(u32, u32)>,
    /// Listened signals whose ordinal is ≥ 64.
    pub listen_overflow: Vec<u32>,
}

impl NodeCold {
    pub const DEFAULT: NodeCold = NodeCold { common: Common::DEFAULT, vis: Visual::IDENTITY, observe: Vec::new(), listen_overflow: Vec::new() };

    pub fn observes(&self, prop: u32) -> bool {
        prop != 0 && self.observe.iter().any(|&(p, c)| p == prop && c > 0)
    }
}

pub(crate) static DEFAULT_COLD: NodeCold = NodeCold::DEFAULT;

/// A borrowed view of one node (the pre-SG1 `Node`), for read paths that predate [`NodeRef`].
///
/// [`NodeRef`]: crate::scene::NodeRef
#[derive(Clone, Copy)]
pub struct Node<'a> {
    pub kind: usize,
    pub handle: NodeId,
    pub parent: Option<NodeId>,
    pub children: &'a [NodeId],
    pub id: egui::Id,
    pub c: &'a Common,
    pub w: &'a W,
    pub vis: Visual,
    pub rect: egui::Rect,
    pub drawn: egui::Rect,
    pub presented: bool,
    pub intrinsic: Option<egui::Vec2>,
}

impl Node<'_> {
    pub fn tag(&self) -> &'static str {
        KINDS[self.kind].tag.unwrap_or("?")
    }
    pub fn is_window(&self) -> bool {
        matches!(self.w, W::Window { .. })
    }
    pub fn text(&self) -> &str {
        text_of(self.w)
    }
}

/// The text a node shows (window: its title).
pub fn text_of(w: &W) -> &str {
    match w {
        W::Label { text, .. } | W::Button { text, .. } | W::Checkbox { text, .. } | W::TextEdit { text, .. } | W::Slider { text, .. } => text,
        W::Window { title, .. } => title,
        W::Box => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ARCH-SCENE §14 SG1 acceptance: the hot column stays within 112 bytes per node.
    #[test]
    fn node_core_fits_112_bytes() {
        let n = std::mem::size_of::<NodeCore>();
        assert!(n <= 112, "size_of::<NodeCore>() = {n}");
        assert_eq!(std::mem::size_of::<Option<NodeId>>(), 8);
        assert_eq!(std::mem::size_of::<Owner>(), 4);
    }
}
