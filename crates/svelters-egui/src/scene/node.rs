//! Per-node data, split by temperature into columns (ARCH-SCENE §4.3): the hot [`NodeCore`] (walked
//! every frame), [`LayoutBox`] (taffy data), [`Payload`] (the kind's behaviour: built-in [`W`] today,
//! provider widgets from RA1) and the rare, lazily boxed [`NodeCold`] (common props, visual channels,
//! observation counts). Plus the bit sets [`NodeFlags`] and [`Dirty`], and [`Owner`]. State bits are
//! RA1's [`StateBits`] (`provider/state.rs`), stored in `NodeCore`. Provider data ([`ExtKey`]) lives
//! in one column per key (`ext.rs`, SG2), not here.
//!
//! [`ExtKey`]: crate::provider::ExtKey
//!
//! [`Node`] is a borrowed read view over the columns of one node, kept for the pre-SG1 read paths
//! (`with_scene(|s| s.n(h).children)` in tests and the eframe runner).
//! Writer lane: SCENE (RA1 adds payload variants, EM1 a few core fields).

use super::id::{NodeId, ProviderId, RootIx, Tick};
use crate::provider::{ExtBlock, StateBits};

/// Which egui widget a node shows, with the props the widget borrows mutably.
#[derive(Clone, Debug, PartialEq)]
pub enum W {
    Window { title: String, default_w: f64, default_h: f64 },
    Box,
    /// `rich`: RT2's runs when it has run children (`crate::rich`), refreshed when measured.
    Label { text: String, wrap: &'static str, selectable: bool, weight: i64, rich: Option<Box<crate::rich::Rich>> },
    Button { text: String, selected: bool, frame: bool, small: bool, weight: i64 },
    Checkbox { text: String, checked: bool },
    TextEdit { text: String, hint: String, multiline: bool, password: bool, char_limit: i64 },
    /// RA1: a provider kind's widget struct (`#[svelters::widget]`, `crate::provider`).
    Ext(crate::provider::ExtWidget),
    /// LS1: a `virtual-list` (`kinds/virtuallist.rs`): its rows are its children.
    VirtualList(Box<crate::kinds::VList>),
    // ---- WG1 (plans/milestones/WG1-sc1-widgets.md) ----
    ToggleButton { text: String, checked: bool, radio: bool, frame: bool, weight: i64 },
    ComboBox { options: Vec<String>, selected: i64, hint: String },
    Separator { vertical: bool },
    Icon { name: String, glyph: String, family: String, size: Option<f64>, weight: i64, frame: bool, selected: bool, source: String, tint: bool },
    /// A CSS grid container (`columns` is `grid-template-columns`, validated on write).
    Grid { columns: String, justify_items: &'static str },
    CollapsingHeader { text: String, open: bool },
    /// `seen`: the offsets egui reported last frame (a differing `x`/`y` is a host write to apply).
    // ---- WG2a (plans/milestones/WG2a-menus-popups.md) ----
    /// The menu family (`crate::menus`).
    Menu(crate::menus::MenuW),
    /// `popup`, `modal`, `inner-window`: a flex box shown out of flow (`crate::overlays`).
    Overlay(Box<crate::overlays::OverlayW>),
    // ---- WG2b (plans/milestones/WG2b-tier0-controls.md; `crate::elements`) ----
    Hyperlink { text: String, url: String, new_tab: bool },
    /// `value`: what the button stands for; with one, `checked` follows `group == value`.
    RadioButton { text: String, checked: bool, value: String, group: String },
    Switch { text: String, active: bool },
    Sides,
    Frame { label: String, style: &'static str },
    Stack { name: String, index: i64, homogeneous: bool },
    Image { source: String, fit: &'static str },
    ProgressBar { fraction: f64, text: String, show_percentage: bool, animate: bool },
    Spinner { spinning: bool, size: Option<f64> },
    Canvas(Box<crate::elements::canvas::Canvas>),
    // ---- WG4a (plans/milestones/WG4a-tier1-views.md; state and paint in `crate::views`) ----
    /// `table`: props, the JS data window and what the last frame asked for.
    Table(std::boxed::Box<crate::views::table::TableState>),
    /// OW5b: boxed (the column's header and cell props).
    TableColumn(std::boxed::Box<crate::views::table::ColumnState>),
    TabView { selected: i64 },
    Tab { title: String, closable: bool },
    /// `vertical`: `orientation="vertical"` (panes stacked).
    /// `toast-host`: props and the toasts shown.
    ToastHost(std::boxed::Box<crate::views::toast::Toasts>),
    /// RT2: an inline run (`span`, `b`, `i`, `code`, `a`: `kinds/rich.rs`), drawn by its label.
    /// `weight` 0 inherits; `anchor`: an `a` run (a link).
    Run { text: String, weight: i64, href: String, anchor: bool },
}

impl W {
    /// Containers lay out children with taffy (flexbox, or grid for `Grid`); `Box` and `Window`
    /// paint nothing themselves, the WG1 containers paint their chrome (`leaf::container`).
    pub fn is_container(&self) -> bool {
        matches!(self, W::Box | W::Window { .. } | W::Grid { .. } | W::CollapsingHeader { .. } | W::Overlay(_))
            || crate::elements::is_container(self) // WG2b
            || crate::views::is_container(self) // WG4a
    }
}

/// Props every in-window node has (`Widget`), plus the box/window container props.
#[derive(Clone, Debug, PartialEq)]
pub struct Common {
    pub enabled: bool,
    pub visible: bool,
    pub tooltip: String,
    pub accessible_name: String,
    /// EM1 (ARCH-EMBED §4.4): the `anchor` prop.
    pub anchor: String,
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
    /// WG1: the `invalid` prop.
    pub invalid: bool,
    /// WG4b: `drag-type`, `drag-data`, `drop-types` (`kinds/dnd.rs`); `None` while all are empty.
    pub dnd: Option<Box<crate::kinds::dnd::Dnd>>,
    /// SG4: `position` (`static`, `relative`, `absolute`): taffy's out-of-flow boxes, hoisted to
    /// their containing block (`layout.rs`).
    pub position: &'static str,
    pub left: Option<f64>,
    pub top: Option<f64>,
    pub right: Option<f64>,
    pub bottom: Option<f64>,
    /// EV3 (`crate::focus`): focus this node when it first appears.
    pub autofocus: bool,
    /// EV3: a box's focus scope (`none`, `contain`, `dialog`).
    pub focus_scope: &'static str,
    /// EV3: a button activated by Enter (`default`) or Escape (`cancel`) in its scope.
    pub default_button: bool,
    pub cancel_button: bool,
}

impl Common {
    pub const DEFAULT: Common = Common {
        enabled: true,
        visible: true,
        tooltip: String::new(),
        accessible_name: String::new(),
        anchor: String::new(),
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
        invalid: false,
        dnd: None,
        position: "static",
        left: None,
        top: None,
        right: None,
        bottom: None,
        autofocus: false,
        focus_scope: "none",
        default_button: false,
        cancel_button: false,
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
pub(crate) use bits;

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
        /// Attached to `parent` (a behaviour, EV2b): listed in the host's `NodeCold::attached`, not
        /// among its light children; not laid out or painted (ARCH-SCENE §4.4).
        const ATTACHED = 1 << 15;
        /// Has attached nodes (`NodeCold::attached` is not empty): the walk runs their hooks.
        const HAS_ATTACHED = 1 << 16;
        /// CX3: a text field the user edited since its last `commit` (blur then commits).
        const EDITED = 1 << 17;
        /// WG6/OW2 (QA13): `scroll-into-view` was called; the walk asks the scrollables around it
        /// to reveal it when it reaches it (`widgets::scroll_area::reveal_flagged`).
        const WANT_SCROLL = 1 << 18;
        /// WG6/OW2: a descendant has `WANT_SCROLL`: the walk does not cull this subtree.
        const SCROLL_BELOW = 1 << 19;
        /// WG6/OW2: `scroll-into-view("center")`.
        const SCROLL_CENTER = 1 << 21;
        /// SG4: the top of a provider's shadow subtree (ARCH-SCENE §4.4): its parent is the host
        /// provider node, which lays it out and draws it (`WidgetCx::children`); JS never sees it.
        const SHADOW = 1 << 20;
        /// SG6: the subtree paints outside the node's own paint bounds; its ink rect (root-relative)
        /// is in `Scene::ink` and the walk culls by it (`walk::culled`).
        const INK_OVERFLOW = 1 << 21;
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
            state: StateBits::NONE,
            listen: 0,
            explicit: 0,
            egui_id,
            rect: egui::Rect::NOTHING,
            drawn: egui::Rect::NOTHING,
            changed: tick,
        }
    }
}

/// Taffy's data for one node (the adapter is `layout.rs`). SG6: 64 B, not 1.2 KB (the style is
/// interned, the cache boxed for containers only, the layouts only their rects: `compact.rs`).
#[derive(Clone, Debug)]
pub(crate) struct LayoutBox {
    /// Shared with every node of the same style (`Scene::intern_style`); write a new one rather
    /// than `Arc::make_mut` where the write is not one-off.
    pub style: std::sync::Arc<taffy::Style>,
    pub cache: super::compact::LayoutCache,
    pub unrounded: super::compact::BoxRect,
    pub layout: super::compact::BoxRect,
    /// No-wrap size of the leaf's content, measured with egui's fonts; `None` = measure again.
    pub intrinsic: Option<egui::Vec2>,
}

impl LayoutBox {
    pub fn new(style: std::sync::Arc<taffy::Style>) -> LayoutBox {
        LayoutBox {
            style,
            cache: super::compact::LayoutCache::new(),
            unrounded: Default::default(),
            layout: Default::default(),
            intrinsic: None,
        }
    }
}

/// The kind's behaviour and its typed props. RA1 adds `Widget(Box<dyn ErasedWidget>)`, EV2b
/// `Behavior`, EM1 `Slot`.
#[derive(Clone, Debug)]
pub(crate) enum Payload {
    /// A freed slot, or a payload taken out while its own code runs (SG4: a provider's `ui`,
    /// `layout`, `prepare`); a taken-out payload reads as an empty box (`Scene::w`).
    Vacant,
    Builtin(W),
}

/// Rare data, boxed only when a node has some (most nodes have none). SG6: the common props moved
/// to their own interned column (`Scene::common`).
#[derive(Clone, Debug)]
pub(crate) struct NodeCold {
    /// TR1 visual channels.
    pub vis: Visual,
    /// R1 observation counts per prop id.
    pub observe: Vec<(u32, u32)>,
    /// Listened signals whose ordinal is ≥ 64.
    pub listen_overflow: Vec<u32>,
    /// Attached nodes (behaviours, EV2b) in attach order; `NodeFlags::HAS_ATTACHED` when not empty.
    pub attached: Vec<NodeId>,
    /// SG4: the out-of-flow (`position: absolute`) boxes this node is the containing block of, as
    /// taffy's last layout recorded them (`layout.rs`, `LayoutContainingBlock`).
    pub hoisted: Vec<NodeId>,
    /// EV1: signals listened in the capture phase (`listen_ex` with `SVL_LISTEN_CAPTURE`).
    pub listen_capture: Vec<u32>,
}

impl NodeCold {
    pub const DEFAULT: NodeCold = NodeCold { vis: Visual::IDENTITY, observe: Vec::new(), listen_overflow: Vec::new(), attached: Vec::new(), hoisted: Vec::new(), listen_capture: Vec::new() };

    pub fn observes(&self, prop: u32) -> bool {
        prop != 0 && self.observe.iter().any(|&(p, c)| p == prop && c > 0)
    }
}

pub(crate) static DEFAULT_COLD: NodeCold = NodeCold::DEFAULT;
/// SG6: what a node without common props reads (`Scene::common`).
pub(crate) static DEFAULT_COMMON: Common = Common::DEFAULT;

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
    /// State bits (RA1).
    pub state: StateBits,
    /// Provider data (read with `ext`, `ext_block`): a view of this node's entries in the scene's
    /// extension columns (SG2).
    pub ext: ExtBlock<'a>,
    pub rect: egui::Rect,
    pub drawn: egui::Rect,
    pub presented: bool,
    pub intrinsic: Option<egui::Vec2>,
}

impl Node<'_> {
    pub fn tag(&self) -> &'static str {
        crate::kinds::kind(self.kind).tag.unwrap_or("?")
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
        W::Label { rich: Some(r), .. } => &r.plain,
        W::Run { text, .. } => text,
        W::Label { text, .. }
        | W::Button { text, .. }
        | W::Checkbox { text, .. }
        | W::TextEdit { text, .. }
        | W::ToggleButton { text, .. }
        | W::CollapsingHeader { text, .. } => text,
        W::Window { title, .. } | W::Tab { title, .. } => title,
        W::TableColumn(c) => &c.title,
        W::ComboBox { options, selected, .. } => usize::try_from(*selected).ok().and_then(|i| options.get(i)).map_or("", |s| s.as_str()),
        W::Icon { name, .. } => name,
        W::Menu(m) => m.label(),
        W::Overlay(o) => &o.title,
        // OW0: an own widget replacing a built-in kind shows its text prop (a mirror kept by `set_prop`)
        W::Ext(x) => x.text(),
        W::Box | W::Separator { .. } | W::Grid { .. } | W::VirtualList(_) => "",
        // WG2b
        W::Hyperlink { text, .. } | W::RadioButton { text, .. } | W::Switch { text, .. } | W::ProgressBar { text, .. } => text,
        W::Frame { label, .. } => label,
        W::Sides | W::Stack { .. } | W::Image { .. } | W::Spinner { .. } | W::Canvas(_) => "",
        W::Table(_) | W::TabView { .. } | W::ToastHost(_) => "",
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

    /// SG6: the per-node columns' element sizes (ARCH-SCENE §8.5), printed for the milestone.
    #[test]
    fn column_sizes() {
        use std::mem::size_of;
        println!(
            "core {} children {} layout {} (taffy style {} cache {} rect {}) payload {} W {} cold {} (NodeCold {} Common {} Visual {})",
            size_of::<NodeCore>(),
            size_of::<Vec<NodeId>>(),
            size_of::<LayoutBox>(),
            size_of::<taffy::Style>(),
            size_of::<super::super::compact::LayoutCache>(),
            size_of::<super::super::compact::BoxRect>(),
            size_of::<Payload>(),
            size_of::<W>(),
            size_of::<Option<Box<NodeCold>>>(),
            size_of::<NodeCold>(),
            size_of::<Common>(),
            size_of::<Visual>(),
        );
    }
}
