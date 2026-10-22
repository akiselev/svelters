//! The egui cascade (CS5e, plans/milestones/CS5e-egui-cascade.md; CSS.md §2.2, §2.5, §2.7 "egui:
//! in-process"): the Rust CSS engine (`svelters-style`) runs on the scene itself.
//!
//! - **The scene is the tree.** [`NodeRef`] implements [`svelters_style::Element`]: the key is
//!   `NodeId::to_raw()`, navigation is the light tree, the state is the node's [`StateBits`] passed
//!   through unchanged (the two bit tables are pinned together by a test), classes, `#id` and the
//!   inline block live in this module's per-slot column ([`SceneStyle`]), and attribute selectors
//!   read props by name. Provider states match as `:state(ns.name)` ([`ProviderStates`]).
//! - **Inputs.** The `style` host op (`SVL_STYLE_TARGET` answers [`SVL_STYLE_TARGET_ENGINE`]):
//!   `CLASSES`, `NAME`, `INLINE` (raw declarations), `SHEET` (raw CSS, `@tokens` at theme origin)
//!   and `SCHEME`; and Rust: [`EguiBackend::add_sheet`], [`EguiBackend::set_tokens`] (SV5),
//!   [`EguiBackend::set_classes`], [`EguiBackend::set_inline`].
//! - **Invalidation.** The engine's dependency maps decide what a change reaches. Class, id and
//!   inline changes are marked when they are written; structure, attribute (prop) and provider
//!   state changes come from an internal journal observer (SG2), drained at the logic step
//!   ([`EguiBackend::restyle_pending`], ARCH-SCENE §7.2 step 7), which restyles top-down.
//! - **Same-frame state.** The walk sets hover, pressed and focus from this pass's interaction
//!   (`ctx.read_response` of the widget's id; the pointer rect for containers), and disabled,
//!   checked, selected and exiting from props, then restyles what the change reaches *before* the
//!   node paints ([`before_leaf`], [`before_container`]). Stock widgets get one computed style per
//!   egui interaction state in their `WidgetVisuals`, so egui switches them itself.
//! - **Outputs.** Layout properties overlay the node's taffy `Style` ([`layout_style`]; `%`,
//!   `calc()` through taffy's calc handles, per-side insets, `display: grid` and grid
//!   placement). Paint properties go to a `RectShape` behind boxes and labels and to the child
//!   `Ui`'s `Visuals` for stock widgets. Unset properties keep egui's `Visuals` (the defaults).
//! - **Text and leaf padding** (CS5f). Leaves measure and paint with one egui style
//!   ([`leaf_style`]), which holds the computed `font-size`, `font-family` and `font-weight` (through
//!   `fonts::register_weight`) and, for buttons, the padding as `button_padding`. Other leaves are
//!   padded around the widget ([`content_rect`]). A change of any of these remeasures the leaf.
//!
//! Precedence (CSS.md §2.8): a prop the runtime set (`NodeCore::explicit`) wins over every rule,
//! then inline, then rules, then the native default.
//!
//! Nothing runs until the first style input: an app without styles pays nothing.
//!
//! Writer lane: STYLE.

pub(crate) mod shadow;
pub(crate) use shadow::paint_bounds;

use crate::backend::EguiBackend;
use crate::kinds;
use crate::scene::{Change, ChangeFilter, ChangeMask, Dirty, Handle, NodeId, NodeRef, Observer, Origin as SceneOrigin, Scene, StateBits, W};
use std::borrow::Cow;
use std::num::NonZeroU64;
use std::sync::{Arc, Mutex, OnceLock};
use svelters_abi::*;
use svelters_style::{
    Atom, ColorScheme, ComputeContext, Contrast, ComputedStyle, ComputedValue, DeclBlock, Diagnostic, Element, Length, MatchCaches, MediaEnv, Origin, ParseOptions, Reach, SheetId,
    Overlay, PseudoElement, StateMask, StateNames, Stylesheet, Stylist, Transitions,
};

/// `SVL_STYLE_TARGET` answer of a backend that runs the Rust engine itself: the runtime sends raw
/// CSS (sheets, inline text) and the user classes. Runtimes that do not know the code treat the
/// backend as unstyled, as with `SVL_STYLE_TARGET_NONE`.
pub const SVL_STYLE_TARGET_ENGINE: i32 = 4;

/// Host-owned sheets (DS1's theme): theme origin, kept by the runtime's clear.
pub(crate) const THEME_SHEETS: &str = "@theme";
/// The theme's token sheet (`@layer sv.tokens`); the default theme's until `set_theme`.
pub(crate) const THEME_TOKENS: &str = "@theme:tokens";
/// The theme's base sheet (`@layer sv.base`, [`crate::theme::BASE_CSS`]).
pub(crate) const THEME_BASE: &str = "@theme:base";

/// Properties the egui cascade computes with no kind prop behind them: `(name, type, keywords)`.
/// They go into the egui style table as `via: css` entries (`spec.rs`), so the compiler leaves
/// them to the engine.
pub(crate) const ENGINE_PROPS: &[(&str, &str, &[&str])] = &[
    ("background-color", "color", &[]),
    ("border-color", "color", &[]),
    ("border-radius", "length", &[]),
    ("border-width", "length", &[]),
    // CS5g: painted around boxes and stock widgets (`style/shadow.rs`)
    ("box-shadow", "shadow", &[]),
    ("color", "color", &[]),
    ("display", "enum", &["flex", "grid", "none"]),
    // CS5f: computed and inherited; leaves measure and paint with them (`leaf_style`)
    ("font-family", "raw", &[]),
    ("font-size", "length", &[]),
    ("font-weight", "font-weight", &[]),
    ("grid-column", "raw", &[]),
    ("grid-row", "raw", &[]),
    ("grid-template-columns", "raw", &[]),
    ("grid-template-rows", "raw", &[]),
    // CS5f: the shorthands take 1 to 4 values. Engine properties, not setters: inline `style=`
    // reaches the engine as the inline block; a `margin`/`padding` attribute stays an explicit prop.
    ("margin", "insets", &[]),
    ("margin-bottom", "length", &[]),
    ("margin-left", "length", &[]),
    ("margin-right", "length", &[]),
    ("margin-top", "length", &[]),
    ("opacity", "number", &[]),
    // OW2: scrolling (`widgets::scroll_area`; `overflow-*` also set taffy's overflow, so a scrolled
    // box keeps its size while its content overflows)
    ("overflow-x", "enum", &["visible", "hidden", "clip", "scroll", "auto"]),
    ("overflow-y", "enum", &["visible", "hidden", "clip", "scroll", "auto"]),
    ("overscroll-behavior", "enum", &["auto", "contain", "none"]),
    ("padding", "insets", &[]),
    ("padding-bottom", "length", &[]),
    ("padding-left", "length", &[]),
    ("padding-right", "length", &[]),
    ("padding-top", "length", &[]),
    // OW2: the gap a revealed node keeps from the scroll area's edge; a classic bar's gutter
    ("scroll-margin", "length", &[]),
    ("scrollbar-gutter", "enum", &["auto", "stable"]),
    // OW-CSS (ARCH-WIDGETS §1.1): read by the paint primitives through `StyleView`
    ("background-image", "image", &[]),
    ("border-top-color", "color", &[]),
    ("border-top-style", "enum", &["none", "hidden", "solid", "dashed", "dotted", "double", "groove", "ridge", "inset", "outset"]),
    ("border-top-width", "length", &[]),
    ("border-right-color", "color", &[]),
    ("border-right-style", "enum", &["none", "hidden", "solid", "dashed", "dotted", "double", "groove", "ridge", "inset", "outset"]),
    ("border-right-width", "length", &[]),
    ("border-bottom-color", "color", &[]),
    ("border-bottom-style", "enum", &["none", "hidden", "solid", "dashed", "dotted", "double", "groove", "ridge", "inset", "outset"]),
    ("border-bottom-width", "length", &[]),
    ("border-left-color", "color", &[]),
    ("border-left-style", "enum", &["none", "hidden", "solid", "dashed", "dotted", "double", "groove", "ridge", "inset", "outset"]),
    ("border-left-width", "length", &[]),
    ("border-top-left-radius", "length", &[]),
    ("border-top-right-radius", "length", &[]),
    ("border-bottom-right-radius", "length", &[]),
    ("border-bottom-left-radius", "length", &[]),
    // WIDGET-PARITY P2: what a glyph part draws
    ("content", "content", &[]),
    ("cursor", "enum", &["auto", "default", "none", "context-menu", "help", "pointer", "progress", "wait", "cell", "crosshair", "text", "vertical-text", "alias", "copy", "move", "no-drop", "not-allowed", "grab", "grabbing", "all-scroll", "col-resize", "row-resize", "n-resize", "e-resize", "s-resize", "w-resize", "ne-resize", "nw-resize", "se-resize", "sw-resize", "ew-resize", "ns-resize", "nesw-resize", "nwse-resize", "zoom-in", "zoom-out"]),
    ("letter-spacing", "length", &[]),
    ("outline-color", "color", &[]),
    ("outline-offset", "length", &[]),
    ("outline-style", "enum", &["none", "auto", "solid", "dashed", "dotted", "double", "groove", "ridge", "inset", "outset"]),
    ("outline-width", "length", &[]),
    // the text primitive's inputs (ARCH-WIDGETS §1.1); `font-family` and `line-height` stay text
    // (a family list; a number or a length) for the primitive to read
    ("font-family", "raw", &[]),
    ("font-style", "enum", &["normal", "italic", "oblique"]),
    ("font-weight", "font-weight", &[]),
    ("line-height", "raw", &[]),
    ("text-align", "enum", &["start", "end", "left", "right", "center", "justify"]),
    // R18: links underline on hover
    ("text-decoration-line", "enum", &["none", "underline", "overline", "line-through"]),
    ("text-overflow", "enum", &["clip", "ellipsis"]),
    ("text-transform", "enum", &["none", "uppercase", "lowercase", "capitalize"]),
    ("transition", "transition", &[]),
    ("white-space", "enum", &["normal", "nowrap", "pre", "pre-wrap", "pre-line", "break-spaces"]),
];

/// The bits the walk owns: set from the pass's interaction and from props, never by providers.
pub(crate) const WALK_BITS: StateBits = StateBits(
    StateBits::HOVERED.0
        | StateBits::PRESSED.0
        | StateBits::FOCUSED.0
        | StateBits::FOCUS_WITHIN.0
        | StateBits::DISABLED.0
        | StateBits::CHECKED.0
        | StateBits::SELECTED.0
        | StateBits::EXITING.0
        | StateBits::FOCUS_VISIBLE.0,
);

/// OW0: the walk-owned bits an own widget (`Caps::PAINTS_BOX`) keeps itself: its units' and its
/// prop mirror's. The walk sets only `disabled` and `exiting` on it.
const OWN_KEEPS: StateBits = StateBits(WALK_BITS.0 & !(StateBits::DISABLED.0 | StateBits::EXITING.0));

/// OW0: an own widget draws itself (`crate::paint`) and owns its interaction bits.
pub(crate) fn is_own(w: &W) -> bool {
    matches!(w, W::Ext(x) if x.entry().caps.contains(crate::provider::Caps::PAINTS_BOX))
}

/// One node's style inputs and outputs.
#[derive(Debug, Default)]
pub(crate) struct NodeStyle {
    /// The node these belong to (a reused slot has another id: the entry is stale).
    id: Option<NodeId>,
    classes: Vec<Atom>,
    name: Option<Box<str>>,
    inline: Option<Arc<DeclBlock>>,
    computed: Option<Arc<ComputedStyle>>,
    /// Stock widgets: the style in egui's `inactive`, `hovered` and `active` states.
    states: Option<Arc<[Arc<ComputedStyle>; 3]>>,
    /// OW-CSS: an own widget's `#[part]`s (its `KindEntry::parts`), kept from the last restyle
    /// that saw the payload (it is taken out while the widget paints).
    part_decl: &'static [crate::provider::PartEntry],
    /// OW-CSS: the computed styles of its paint parts that a `::part()` rule can reach, as
    /// (declaration index, name, style), restyled with the host.
    parts: Vec<(u16, &'static str, Arc<ComputedStyle>)>,
}

/// The cascade's state in one scene.
pub(crate) struct SceneStyle {
    /// Set by the first style input; until then nothing runs.
    active: bool,
    stylist: Stylist,
    /// Registered sheets by id, in registration order.
    sheets: Vec<(String, SheetId)>,
    /// By slot index.
    nodes: Vec<NodeStyle>,
    /// Nodes to restyle at the next logic step.
    dirty: Vec<NodeId>,
    /// Restyle every node at the next logic step.
    all: bool,
    observer: Option<Observer>,
    caches: MatchCaches,
    cx: ComputeContext,
    /// The scheme set with `SVL_STYLE_SCHEME`, `set_color_scheme` or `set_theme` (else egui's
    /// dark mode).
    pub(crate) scheme: Option<ColorScheme>,
    /// `prefers-contrast`, set with `set_theme` (DS1; else no preference).
    pub(crate) contrast: Option<Contrast>,
    /// The focused node at the end of the last walk (for `:focus-within`).
    focus: Option<NodeId>,
    /// A walk-time restyle that needs another frame (an ancestor or a layout change).
    pub(crate) repaint: bool,
    /// OW-CSS: running CSS transitions of nodes, and of own widgets' parts (by declaration index).
    transitions: Transitions<NodeId>,
    part_transitions: Transitions<(NodeId, u16)>,
    /// Their samples at `now`: what paint reads over the computed style.
    sampled: std::collections::HashMap<NodeId, Overlay>,
    part_sampled: std::collections::HashMap<(NodeId, u16), Overlay>,
    /// egui's input time at the last logic step (kittest's virtual clock in tests): transitions
    /// start and are sampled at it.
    pub(crate) now: f64,
    /// Reduced motion (`EguiBackend::set_motion`): `prefers-reduced-motion: reduce` matches and
    /// no transition runs.
    pub(crate) reduced_motion: bool,
}

impl SceneStyle {
    /// SG6: heap bytes of the per-node data (`SceneStats::bytes_by`): the entries, their class
    /// lists and the computed styles they own alone (a shared `Arc` counts once per owner's share).
    pub(crate) fn bytes(&self) -> usize {
        use std::mem::size_of;
        let mut b = self.nodes.capacity() * size_of::<NodeStyle>() + self.dirty.capacity() * size_of::<NodeId>();
        for n in &self.nodes {
            b += n.classes.capacity() * size_of::<Atom>();
            if let Some(c) = &n.computed {
                b += size_of::<ComputedStyle>() / Arc::strong_count(c);
            }
            if let Some(s) = &n.states {
                b += (3 * size_of::<ComputedStyle>()) / Arc::strong_count(s);
            }
        }
        b
    }
}

impl Default for SceneStyle {
    fn default() -> Self {
        SceneStyle {
            active: false,
            stylist: Stylist::new(&svelters_style::tables::EGUI),
            sheets: Vec::new(),
            nodes: Vec::new(),
            dirty: Vec::new(),
            all: false,
            observer: None,
            caches: MatchCaches::new(),
            cx: ComputeContext { root_font_size: 14.0, default_font_size: 14.0 },
            scheme: None,
            contrast: None,
            focus: None,
            repaint: false,
            transitions: Transitions::new(),
            part_transitions: Transitions::new(),
            sampled: Default::default(),
            part_sampled: Default::default(),
            now: 0.0,
            reduced_motion: false,
        }
    }
}

/// OW0: the computed styles of one node's paint parts (`::part(name)`), by part name.
pub(crate) type PartStyles = Vec<(Box<str>, Arc<ComputedStyle>)>;

impl SceneStyle {
    pub(crate) fn is_active(&self) -> bool {
        self.active
    }

    /// The computed style of node `id`'s paint part `part` (`::part(name)`), as a pseudo-element
    /// of the host (`Stylist::compute_pseudo`). `None` when no sheet has a rule ending in that
    /// part: own widgets then paint the part from its UA table, inheriting from the host.
    pub(crate) fn part_computed(&self, id: NodeId, part: &str) -> Option<&Arc<ComputedStyle>> {
        self.node(id)?.parts.iter().find(|(_, n, _)| *n == part).map(|(_, _, c)| c)
    }

    /// Every part style of node `id` (what `measure` sees).
    pub(crate) fn part_styles(&self, id: NodeId) -> PartStyles {
        self.node(id).map(|n| n.parts.iter().map(|(_, name, c)| (Box::from(*name), c.clone())).collect()).unwrap_or_default()
    }

    /// Node `id`'s running transitions at [`SceneStyle::now`] (empty: none), sorted by property.
    pub(crate) fn overlay(&self, id: NodeId) -> &[(usize, ComputedValue)] {
        self.sampled.get(&id).map_or(&[], |v| &v[..])
    }

    /// Part `part` of node `id`: its running transitions at [`SceneStyle::now`].
    pub(crate) fn part_overlay(&self, id: NodeId, part: &str) -> &[(usize, ComputedValue)] {
        let Some((k, _, _)) = self.node(id).and_then(|n| n.parts.iter().find(|(_, n, _)| *n == part)) else { return &[] };
        self.part_sampled.get(&(id, *k)).map_or(&[], |v| &v[..])
    }

    /// Whether a CSS transition of a node or a part is running.
    pub(crate) fn transitions_running(&self) -> bool {
        !self.sampled.is_empty() || !self.part_sampled.is_empty()
    }

    /// The nodes with a running transition (theirs or a part's), possibly repeated.
    pub(crate) fn transitioning_nodes(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.sampled.keys().copied().chain(self.part_sampled.keys().map(|k| k.0))
    }

    /// The logic step's clock: drops ended transitions and samples the running ones at `now`.
    /// Returns whether any still run (their roots need frames).
    pub(crate) fn step_transitions(&mut self, now: f64) -> bool {
        self.now = now;
        if !self.transitions_running() {
            return false;
        }
        self.transitions.tick(now);
        self.part_transitions.tick(now);
        let SceneStyle { transitions, part_transitions, sampled, part_sampled, .. } = self;
        sampled.clear();
        sampled.extend(transitions.running_keys().filter_map(|k| Some((k, transitions.sample(k, now)?))));
        part_sampled.clear();
        part_sampled.extend(part_transitions.running_keys().filter_map(|k| Some((k, part_transitions.sample(k, now)?))));
        self.transitions_running()
    }

    /// Records node `id`'s restyle for transitions: the host from `old` to its new style, and each
    /// part from `old_parts` to its new one; what starts is sampled at once (paint this pass).
    fn note_transitions(&mut self, id: NodeId, old: Option<&ComputedStyle>, old_parts: &[(u16, &'static str, Arc<ComputedStyle>)]) {
        let now = self.now;
        let instant = self.reduced_motion;
        let SceneStyle { nodes, transitions, part_transitions, sampled, part_sampled, .. } = self;
        let Some(ns) = nodes.get(id.index() as usize).filter(|n| n.id == Some(id)) else { return };
        if let (Some(a), Some(b)) = (old, ns.computed.as_deref())
            && !std::ptr::eq(a, b)
        {
            transitions.restyled(id, a, b, now, instant);
            match transitions.sample(id, now) {
                Some(o) => sampled.insert(id, o),
                None => sampled.remove(&id),
            };
        }
        for (k, _, b) in &ns.parts {
            let Some((_, _, a)) = old_parts.iter().find(|(j, _, _)| j == k) else { continue };
            if Arc::ptr_eq(a, b) {
                continue;
            }
            let key = (id, *k);
            part_transitions.restyled(key, a, b, now, instant);
            match part_transitions.sample(key, now) {
                Some(o) => part_sampled.insert(key, o),
                None => part_sampled.remove(&key),
            };
        }
    }

    /// Ends every running transition (reduced motion switched on).
    pub(crate) fn end_transitions(&mut self) {
        self.transitions.clear();
        self.part_transitions.clear();
        self.sampled.clear();
        self.part_sampled.clear();
    }

    /// Node `id` went away: forgets its transitions.
    pub(crate) fn forget(&mut self, id: NodeId) {
        if self.sampled.remove(&id).is_some() {
            self.transitions.remove(id);
        }
        let parts = self.node(id).map_or(0, |n| n.part_decl.len());
        for k in 0..parts as u16 {
            if self.part_sampled.remove(&(id, k)).is_some() {
                self.part_transitions.remove((id, k));
            }
        }
    }

    fn node(&self, id: NodeId) -> Option<&NodeStyle> {
        self.nodes.get(id.index() as usize).filter(|n| n.id == Some(id))
    }

    fn node_mut(&mut self, id: NodeId) -> &mut NodeStyle {
        let i = id.index() as usize;
        if self.nodes.len() <= i {
            self.nodes.resize_with(i + 1, NodeStyle::default);
        }
        let n = &mut self.nodes[i];
        if n.id != Some(id) {
            *n = NodeStyle { id: Some(id), ..NodeStyle::default() };
        }
        n
    }

    /// The computed style of a live node.
    pub(crate) fn computed(&self, id: NodeId) -> Option<&Arc<ComputedStyle>> {
        self.node(id)?.computed.as_ref()
    }
}

// ---- the scene as the engine's tree ----

/// Provider states by full name (`cad.pending`), for `:state()` (RA1's registry).
pub struct ProviderStates;

impl StateNames for ProviderStates {
    fn lookup(&self, name: &str) -> Option<StateMask> {
        let i = crate::provider::state_registry().iter().position(|d| d.full == name)?;
        Some(StateMask(1u64 << (32 + i)))
    }
}

/// The text of a prop value for attribute selectors (`[value="3"]`).
fn value_text(v: &Value) -> Option<Cow<'static, str>> {
    Some(match v {
        Value::Null => return None,
        Value::Bool(b) => Cow::Borrowed(if *b { "true" } else { "false" }),
        Value::I64(n) => Cow::Owned(n.to_string()),
        Value::U64(n) => Cow::Owned(n.to_string()),
        Value::F64(x) | Value::DateTime(x) => Cow::Owned(if x.fract() == 0.0 && x.abs() < 1e15 { format!("{}", *x as i64) } else { x.to_string() }),
        Value::Str(s) => Cow::Owned(s.clone()),
        _ => return None,
    })
}

fn key_of(id: NodeId) -> NonZeroU64 {
    NonZeroU64::new(id.to_raw()).unwrap_or(NonZeroU64::MAX)
}

fn sibling<'a>(n: NodeRef<'a>, delta: isize) -> Option<NodeRef<'a>> {
    let p = n.parent()?;
    let kids = &n.s.children[p.i];
    let k = kids.iter().position(|&c| c == n.id())? as isize + delta;
    let c = *kids.get(usize::try_from(k).ok()?)?;
    n.s.idx(c).map(|_| NodeRef::at(n.s, c))
}

impl Element for NodeRef<'_> {
    fn key(&self) -> NonZeroU64 {
        key_of(NodeRef::id(*self))
    }
    fn parent(&self) -> Option<Self> {
        NodeRef::parent(*self)
    }
    fn prev_sibling(&self) -> Option<Self> {
        sibling(*self, -1)
    }
    fn next_sibling(&self) -> Option<Self> {
        sibling(*self, 1)
    }
    fn first_child(&self) -> Option<Self> {
        let c = *self.s.children[self.i].first()?;
        Some(NodeRef::at(self.s, c))
    }
    fn tag(&self) -> &str {
        NodeRef::tag(*self)
    }
    /// OW0: a provider kind declared with `style_as = "<tag>"`.
    fn style_as(&self) -> Option<&str> {
        match self.s.w(self.i) {
            W::Ext(x) => x.entry().style_as,
            _ => None,
        }
    }
    fn id(&self) -> Option<&str> {
        self.s.style.node(NodeRef::id(*self))?.name.as_deref()
    }
    fn has_class(&self, name: &str) -> bool {
        self.s.style.node(NodeRef::id(*self)).is_some_and(|n| n.classes.iter().any(|c| c.as_str() == name))
    }
    fn for_each_class(&self, f: &mut dyn FnMut(&str)) {
        if let Some(n) = self.s.style.node(NodeRef::id(*self)) {
            for c in &n.classes {
                f(c.as_str());
            }
        }
    }
    fn attr(&self, name: &str) -> Option<Cow<'_, str>> {
        self.value_of(name).ok().as_ref().and_then(value_text)
    }
    fn state(&self) -> StateMask {
        StateMask(NodeRef::state(*self).0)
    }
    /// OW0: the `focus-visible` bit (own widgets: Focusable's rule; stock leaves: the walk sets it
    /// with `focused`, as before).
    fn is_focus_visible(&self) -> bool {
        NodeRef::state(*self).contains(StateBits::FOCUS_VISIBLE)
    }
}

/// A node with its state replaced (the per-state styles of a stock widget); other nodes as is.
#[derive(Clone, Copy, Debug)]
struct Forced<'a> {
    n: NodeRef<'a>,
    subject: NodeId,
    state: StateBits,
}

impl<'a> Forced<'a> {
    fn wrap(&self, n: Option<NodeRef<'a>>) -> Option<Self> {
        n.map(|n| Forced { n, ..*self })
    }
}

impl Element for Forced<'_> {
    fn key(&self) -> NonZeroU64 {
        self.n.key()
    }
    fn parent(&self) -> Option<Self> {
        self.wrap(Element::parent(&self.n))
    }
    fn prev_sibling(&self) -> Option<Self> {
        self.wrap(self.n.prev_sibling())
    }
    fn next_sibling(&self) -> Option<Self> {
        self.wrap(self.n.next_sibling())
    }
    fn first_child(&self) -> Option<Self> {
        self.wrap(self.n.first_child())
    }
    fn tag(&self) -> &str {
        Element::tag(&self.n)
    }
    fn style_as(&self) -> Option<&str> {
        Element::style_as(&self.n)
    }
    fn id(&self) -> Option<&str> {
        Element::id(&self.n)
    }
    fn has_class(&self, name: &str) -> bool {
        self.n.has_class(name)
    }
    fn for_each_class(&self, f: &mut dyn FnMut(&str)) {
        self.n.for_each_class(f)
    }
    fn attr(&self, name: &str) -> Option<Cow<'_, str>> {
        self.n.attr(name)
    }
    fn state(&self) -> StateMask {
        if self.n.id() == self.subject { StateMask(self.state.0) } else { Element::state(&self.n) }
    }
    fn is_focus_visible(&self) -> bool {
        self.state().contains(StateMask(StateBits::FOCUS_VISIBLE.0))
    }
}

// ---- restyle ----

/// Stock widgets: egui draws them with per-state `WidgetVisuals`.
fn is_stock(w: &W) -> bool {
    matches!(w, W::Button { .. } | W::Checkbox { .. } | W::TextEdit { .. })
}

/// Whether two part lists hold the same parts with equal styles.
fn parts_eq(a: &[(u16, &'static str, Arc<ComputedStyle>)], b: &[(u16, &'static str, Arc<ComputedStyle>)]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.0 == y.0 && x.2 == y.2)
}

/// The part properties `measure` reads (box and text metrics): a change relayouts the host.
const PART_LAYOUT_PROPS: &[&str] = &[
    "padding-left", "padding-right", "padding-top", "padding-bottom", "padding", "border-width", "border-left-width", "border-right-width", "border-top-width", "border-bottom-width",
    "font-size", "font-weight", "font-family", "font-style", "line-height", "letter-spacing", "text-transform", "width", "height", "min-width", "min-height", "display", "gap", "content",
];

/// Whether `b`'s parts differ from `a`'s in what layout reads (a part added or removed, or a
/// [`PART_LAYOUT_PROPS`] value).
fn parts_layout_differs(a: &[(u16, &'static str, Arc<ComputedStyle>)], b: &[(u16, &'static str, Arc<ComputedStyle>)]) -> bool {
    a.len() != b.len() || a.iter().zip(b).any(|(x, y)| x.0 != y.0 || PART_LAYOUT_PROPS.iter().any(|p| x.2.get(p) != y.2.get(p)))
}

/// Whether two styles differ in what children inherit (inherited properties, custom properties).
fn inherited_differs(a: Option<&ComputedStyle>, b: &ComputedStyle) -> bool {
    let Some(a) = a else { return true };
    let t = b.table();
    (0..t.props.len()).any(|k| t.props[k].is_inherited() && a.get_index(k) != b.get_index(k)) || !a.customs().eq(b.customs())
}

impl Scene {
    /// OW-CSS: the logic step's transition clock ([`SceneStyle::step_transitions`]). Every node
    /// whose transitions ran this frame, including those that just ended, drops its cached shapes
    /// (SG6's `WidgetCx::cached_shapes`): its paint changes every frame while it animates.
    pub(crate) fn step_style_transitions(&mut self, now: f64) {
        let ran: Vec<NodeId> = self.style.transitioning_nodes().collect();
        self.style.step_transitions(now);
        for id in ran {
            if let Some(i) = self.idx(id) {
                self.paint_cache.remove(i as u32);
            }
        }
    }

    /// OW-CSS: whether a CSS transition (of a node or an own widget's part) is running.
    pub fn style_transitions_running(&self) -> bool {
        self.style.transitions_running()
    }

    /// Marks node `id` for the next logic restyle.
    pub(crate) fn mark_style(&mut self, id: NodeId) {
        if let Some(i) = self.idx(id)
            && !self.core[i].dirty.contains(Dirty::STYLE)
        {
            self.core[i].dirty.insert(Dirty::STYLE);
            self.style.dirty.push(id);
        }
    }

    /// Marks what `reach` names from `id`.
    fn mark_reach(&mut self, id: NodeId, reach: Reach) {
        if reach.is_empty() || self.idx(id).is_none() {
            return;
        }
        let mut ids = Vec::new();
        svelters_style::for_each_reached(NodeRef::at(self, id), reach, &mut |n| ids.push(n.id()));
        for n in ids {
            self.mark_style(n);
        }
    }

    /// Turns the cascade on: every node gets a style, the default theme tokens are installed
    /// unless a theme was (DS1: the backend owns the `--sv-*` defaults), and the journal observer
    /// starts.
    fn activate_style(&mut self) {
        if !self.style.active {
            self.style.active = true;
            self.style.all = true;
            if !self.style.sheets.iter().any(|(n, _)| n == THEME_TOKENS) {
                self.install_sheet(THEME_TOKENS, crate::theme::default_tokens_css());
            }
            self.observe_for_style();
        }
    }

    /// (Re)creates the internal observer for the current sheets' dependencies.
    fn observe_for_style(&mut self) {
        let states = StateBits(self.style.stylist.invalidation().states_used().0 & !WALK_BITS.0);
        let mut f = ChangeFilter::only(ChangeMask::CREATED | ChangeMask::STRUCTURE | ChangeMask::PROPS | ChangeMask::STATE);
        f.states = states;
        self.style.observer = Some(self.journal.observe(f));
    }

    /// Computes node `i`'s style now from its parent's stored style; applies it (taffy style,
    /// paint). Returns true if what its children inherit changed.
    pub(crate) fn restyle_node(&mut self, i: usize) -> bool {
        let Some(id) = self.slots.id_at(i) else { return false };
        self.core[i].dirty.remove(Dirty::STYLE);
        let parent = self.core[i].parent.and_then(|p| self.style.computed(p).cloned());
        let inline = self.style.node(id).and_then(|n| n.inline.clone());
        let mut caches = std::mem::take(&mut self.style.caches);
        let n = NodeRef::at(self, id);
        let st = &self.style;
        let c = st.stylist.compute(n, parent.as_deref(), inline.as_deref(), &st.cx, &mut caches);
        let states = if is_stock(self.w(i)) && st.stylist.invalidation().states_used().intersects(StateMask(StateBits::HOVERED.0 | StateBits::PRESSED.0)) {
            let s0 = self.core[i].state;
            let base = StateBits(s0.0 & !(StateBits::HOVERED.0 | StateBits::PRESSED.0));
            let one = |state: StateBits, caches: &mut MatchCaches| {
                caches.clear();
                Arc::new(st.stylist.compute(Forced { n, subject: id, state }, parent.as_deref(), inline.as_deref(), &st.cx, caches))
            };
            let a = one(base, &mut caches);
            let h = one(base | StateBits::HOVERED, &mut caches);
            let p = one(base | StateBits::HOVERED | StateBits::PRESSED, &mut caches);
            Some(Arc::new([a, h, p]))
        } else {
            None
        };
        // OW-CSS: an own widget's paint parts, as pseudo-elements of the host (W3)
        let decl: &'static [crate::provider::PartEntry] = match self.w(i) {
            W::Ext(x) => x.entry().parts,
            _ => st.node(id).map_or(&[], |n| n.part_decl),
        };
        let mut parts = Vec::new();
        for (k, p) in decl.iter().enumerate() {
            let pe = PseudoElement::part(p.name);
            if !p.node && st.stylist.has_pseudo_rules(&pe) {
                parts.push((k as u16, p.name, Arc::new(st.stylist.compute_pseudo(n, &pe, &c, &st.cx, &mut caches))));
            }
        }
        caches.clear();
        self.style.caches = caches;
        let ns = self.style.node_mut(id);
        ns.part_decl = decl;
        let changed = ns.computed.as_deref() != Some(&c) || ns.states.as_deref().map(|s| &s[..]) != states.as_deref().map(|s| &s[..]);
        let parts_changed = !parts_eq(&ns.parts, &parts);
        let inherit = inherited_differs(ns.computed.as_deref(), &c);
        if !changed && !parts_changed {
            return false;
        }
        let relayout_parts = parts_changed && parts_layout_differs(&ns.parts, &parts);
        // keep unchanged styles (their identity tells transitions what changed)
        let old = if ns.computed.as_deref() != Some(&c) { ns.computed.replace(Arc::new(c)) } else { None };
        ns.states = states;
        let old_parts = std::mem::take(&mut ns.parts);
        for p in parts.iter_mut() {
            if let Some((_, _, o)) = old_parts.iter().find(|(k, _, o)| *k == p.0 && **o == *p.2) {
                p.2 = o.clone();
            }
        }
        ns.parts = parts;
        self.style.note_transitions(id, old.as_deref(), &old_parts);
        self.core[i].dirty.insert(Dirty::PAINT);
        // SG6: a new style draws differently, whatever version the widget caches its shapes under
        self.paint_cache.remove(i as u32);
        let w = self.w(i);
        if matches!(w, W::Run { .. }) {
            crate::rich::touched(self, i); // RT2: a run's font may have changed
        } else if !w.is_container() && !matches!(w, W::Ext(_)) {
            // CS5f: a leaf's font or padding changed: it measures again (a new size relays out).
            // `old` is None when only its parts changed (OW-CSS): nothing of its own to compare.
            if let Some(o) = old.as_deref() {
                let new = self.style.computed(id).cloned();
                if MEASURED_PROPS.iter().any(|p| o.get(p) != new.as_ref().and_then(|n| n.get(p))) {
                    self.mark_measure(id);
                    self.style.repaint = true;
                }
            }
        }
        let t = layout_style(self, i);
        if t != *self.layout[i].style || relayout_parts {
            if t != *self.layout[i].style {
                self.layout[i].style = self.intern_style(t);
            }
            self.invalidate_style(id);
            self.style.repaint = true;
        }
        inherit
    }

    /// The logic step: drains the observer, then restyles the marked nodes, parents first.
    pub(crate) fn restyle_marked(&mut self) -> usize {
        if !self.style.active {
            return 0;
        }
        if let Some(obs) = &self.style.observer {
            let set = obs.take();
            if set.overflowed {
                self.style.all = true;
            }
            let inv = self.style.stylist.invalidation().clone();
            for r in set.records {
                match r {
                    Change::Created { node, .. } => self.mark_style(node),
                    Change::Inserted { parent, node, .. } => {
                        self.mark_reach(node, Reach::SELF | Reach::DESCENDANTS);
                        self.mark_reach(parent, inv.structure_changed());
                    }
                    Change::Removed { parent, .. } => self.mark_reach(parent, inv.structure_changed()),
                    Change::Prop { node, prop, .. } => {
                        if let Some(i) = self.idx(node)
                            && let Some(def) = kinds::prop_def(self.core[i].kind as usize, prop.0)
                            && inv.uses_attr(def.name)
                        {
                            self.mark_reach(node, inv.attr_changed(def.name));
                        }
                    }
                    Change::State { node, before, after } => self.mark_reach(node, inv.state_changed(StateMask(before.0), StateMask(after.0))),
                    _ => {}
                }
            }
        }
        if std::mem::take(&mut self.style.all) {
            self.style.dirty.clear();
            let ids: Vec<NodeId> = (0..self.core.len()).filter_map(|i| self.slots.id_at(i)).collect();
            for id in ids {
                let i = self.ix(id);
                self.core[i].dirty.remove(Dirty::STYLE);
                self.mark_style(id);
            }
        }
        if self.style.dirty.is_empty() {
            return 0;
        }
        // parents first: by depth
        let depth = |s: &Scene, id: NodeId| {
            let mut d = 0u32;
            let mut a = s.core[s.ix(id)].parent;
            while let Some(p) = a {
                d += 1;
                a = s.core[s.ix(p)].parent;
            }
            d
        };
        let mut heap: std::collections::BinaryHeap<std::cmp::Reverse<(u32, u64)>> = std::collections::BinaryHeap::new();
        for id in std::mem::take(&mut self.style.dirty) {
            if self.idx(id).is_some() {
                heap.push(std::cmp::Reverse((depth(self, id), id.to_raw())));
            }
        }
        let mut n = 0;
        while let Some(std::cmp::Reverse((d, raw))) = heap.pop() {
            let Some(id) = NodeId::from_raw(raw).filter(|&id| self.idx(id).is_some()) else { continue };
            let i = self.ix(id);
            if !self.core[i].dirty.contains(Dirty::STYLE) {
                continue;
            }
            n += 1;
            if self.restyle_node(i) {
                for k in 0..self.children[i].len() {
                    let c = self.children[i][k];
                    let ci = self.ix(c);
                    if !self.core[ci].dirty.contains(Dirty::STYLE) {
                        self.core[ci].dirty.insert(Dirty::STYLE);
                        heap.push(std::cmp::Reverse((d + 1, c.to_raw())));
                    }
                }
            }
        }
        n
    }

    /// Restyles what a state change of `id` reaches, now (the walk). Nodes painted later this pass
    /// show it this frame; earlier ones next frame.
    fn restyle_reached_now(&mut self, id: NodeId, reach: Reach) {
        if reach.is_empty() {
            return;
        }
        let mut ids = Vec::new();
        svelters_style::for_each_reached(NodeRef::at(self, id), reach, &mut |n| ids.push(n.id()));
        if reach.contains(Reach::ANCESTORS) || reach.contains(Reach::TREE) {
            self.style.repaint = true;
        }
        let mut seen = std::collections::HashSet::new();
        for n in ids {
            if !seen.insert(n) {
                continue;
            }
            let Some(i) = self.idx(n) else { continue };
            if self.restyle_node(i) {
                // what its children inherit changed: those not reached yet restyle at the next logic
                for k in 0..self.children[i].len() {
                    let c = self.children[i][k];
                    if !seen.contains(&c) {
                        self.mark_style(c);
                        self.style.repaint = true;
                    }
                }
            }
        }
    }

    /// Sets the walk-owned state bits of node `i` to `bits` (journaled; restyles what it reaches).
    pub(crate) fn set_walk_state(&mut self, i: usize, bits: StateBits) {
        let before = self.core[i].state;
        let after = StateBits((before.0 & !WALK_BITS.0) | (bits.0 & WALK_BITS.0));
        if after == before {
            return;
        }
        self.core[i].state = after;
        let t = self.stamp();
        self.core[i].changed = t;
        self.core[i].dirty.insert(Dirty::PAINT);
        self.record_state(i, before, SceneOrigin::Style);
        let Some(id) = self.slots.id_at(i) else { return };
        let reach = self.style.stylist.invalidation().state_changed(StateMask(before.0), StateMask(after.0));
        self.restyle_reached_now(id, reach);
    }

    /// The prop-derived walk bits of node `i`.
    fn prop_bits(&self, i: usize) -> StateBits {
        let mut b = StateBits::NONE;
        if !self.common(i).enabled {
            b = b | StateBits::DISABLED;
        }
        if self.vis(i).inert {
            b = b | StateBits::EXITING;
        }
        match self.w(i) {
            W::Checkbox { checked: true, .. } => b = b | StateBits::CHECKED,
            W::Button { selected: true, .. } => b = b | StateBits::SELECTED,
            _ => {}
        }
        b
    }

    /// Moves `:focus-within` from the old focus chain to the new one.
    fn set_focus(&mut self, focus: Option<NodeId>) {
        if self.style.focus == focus {
            return;
        }
        let chain = |s: &Scene, f: Option<NodeId>| -> Vec<NodeId> {
            let mut v = Vec::new();
            let mut a = f.filter(|&f| s.idx(f).is_some());
            while let Some(x) = a {
                v.push(x);
                a = s.core[s.ix(x)].parent;
            }
            v
        };
        let old = chain(self, self.style.focus);
        let new = chain(self, focus);
        self.style.focus = focus;
        for (ids, on) in [(old, false), (new, true)] {
            for id in ids {
                let i = self.ix(id);
                let mut b = self.core[i].state;
                b.set(StateBits::FOCUS_WITHIN, on);
                self.set_walk_state(i, b);
            }
        }
        self.style.repaint = true;
    }
}

// ---- the walk's hooks ----

/// Before a container paints: hover from the pointer, prop states, then its box (background,
/// border). Returns the opacity its subtree is painted with, if not 1.
pub(crate) fn before_container(s: &mut Scene, i: usize, r: egui::Rect, ui: &egui::Ui) -> Option<f32> {
    if !s.style.active {
        return None;
    }
    let hovered = ui.rect_contains_pointer(r);
    let pressed = hovered && ui.ctx().input(|inp| inp.pointer.primary_down());
    let mut b = s.prop_bits(i) | StateBits(s.core[i].state.0 & (StateBits::FOCUSED.0 | StateBits::FOCUS_WITHIN.0 | StateBits::FOCUS_VISIBLE.0));
    b.set(StateBits::HOVERED, hovered);
    b.set(StateBits::PRESSED, pressed);
    s.set_walk_state(i, b);
    let id = s.slots.id_at(i)?;
    let c = s.style.computed(id)?.clone();
    let dark = ui.ctx().theme() == egui::Theme::Dark;
    let v = crate::paint::StyleView::new(Some(&c), crate::paint::Ua::NONE, s.core[i].state, dark).with_overlay(s.style.overlay(id));
    crate::paint::paint_box(&mut crate::paint::PaintCx::for_node(ui.painter(), id.to_raw()), &v, r);
    let o = v.opacity();
    (o < 1.0).then_some(o)
}

/// OW0: an own widget's bits after its units ran (`WidgetCx::commit_state`): stored now
/// (journaled), the restyle they reach run now, and `:focus-within` moved with `focused`, so the
/// widget paints this pass's state.
pub(crate) fn set_own_state(s: &mut Scene, i: usize, bits: StateBits) {
    let before = s.core[i].state;
    if !s.style.active {
        // no cascade: the bits are kept for the widget (and queries), but nothing restyles, and
        // interaction bits are not journaled, as for the stock leaves (no extra frames)
        s.core[i].state = bits;
        return;
    }
    if bits != before {
        s.core[i].state = bits;
        let t = s.stamp();
        s.core[i].changed = t;
        s.core[i].dirty.insert(Dirty::PAINT);
        s.record_state(i, before, SceneOrigin::Style); // as the walk's bits: restyled here, not at the logic step
    }
    if !s.style.active {
        return;
    }
    let Some(id) = s.slots.id_at(i) else { return };
    if bits != before {
        let reach = s.style.stylist.invalidation().state_changed(StateMask(before.0), StateMask(bits.0));
        s.restyle_reached_now(id, reach);
    }
    if bits.contains(StateBits::FOCUSED) {
        s.set_focus(Some(id));
    } else if s.style.focus == Some(id) {
        s.set_focus(None);
    }
}

/// Before a leaf or provider node paints into `child`: its state from this pass's response, the
/// restyle that reaches, then its visuals on `child`.
pub(crate) fn before_leaf(s: &mut Scene, i: usize, r: egui::Rect, child: &mut egui::Ui) {
    if !s.style.active {
        return;
    }
    let Some(id) = s.slots.id_at(i) else { return };
    if is_own(s.w(i)) {
        // OW0: its units set hover, press and focus during its `ui` (`WidgetCx::run_units`), and
        // it paints its own box: only the prop bits here
        // (and `opacity`: `StyleView::opacity`, CSS over its UA table)
        let b = s.prop_bits(i) | StateBits(s.core[i].state.0 & OWN_KEEPS.0);
        s.set_walk_state(i, b);
        let _ = child;
        return;
    }
    // the id the widget takes: TextEdit's is the node's (leaf.rs), the others the child's first
    // auto id; providers draw with the node's id
    let wid = match s.w(i) {
        W::TextEdit { .. } | W::Ext(_) => s.core[i].egui_id,
        _ => child.next_auto_id(),
    };
    let resp = child.ctx().read_response(wid);
    let (hovered, pressed, focused) = match &resp {
        Some(r) => (r.hovered(), r.is_pointer_button_down_on(), r.has_focus()),
        None => {
            let h = child.rect_contains_pointer(r);
            (h, h && child.ctx().input(|inp| inp.pointer.primary_down()), false)
        }
    };
    let mut b = s.prop_bits(i) | StateBits(s.core[i].state.0 & StateBits::FOCUS_WITHIN.0);
    b.set(StateBits::HOVERED, hovered);
    b.set(StateBits::PRESSED, pressed);
    b.set(StateBits::FOCUSED, focused);
    b.set(StateBits::FOCUS_VISIBLE, focused);
    s.set_walk_state(i, b);
    if focused {
        s.set_focus(Some(id));
    } else if s.style.focus == Some(id) {
        s.set_focus(None);
    }
    let Some(c) = s.style.computed(id).cloned() else { return };
    if let Some(ComputedValue::Number(o)) = c.get("opacity")
        && *o < 1.0
    {
        child.multiply_opacity(o.max(0.0));
    }
    // CS5f: its font and button padding, as measured (`leaf_style`)
    if let Some(st) = leaf_style(s, i, &child.ctx().clone(), child.style()) {
        child.set_style(st);
    }
    let states = s.style.node(id).and_then(|n| n.states.clone());
    if is_stock(s.w(i)) {
        let v = &mut child.visuals_mut().widgets;
        match &states {
            Some(st) => {
                widget_visuals(&st[0], &mut v.noninteractive);
                widget_visuals(&st[0], &mut v.inactive);
                widget_visuals(&st[1], &mut v.hovered);
                widget_visuals(&st[2], &mut v.active);
            }
            None => {
                for w in [&mut v.noninteractive, &mut v.inactive, &mut v.hovered, &mut v.active] {
                    widget_visuals(&c, w);
                }
            }
        }
        // CS5g: the shadows of this pass's state, behind the widget
        let now = match &states {
            Some(st) => &st[if pressed { 2 } else if hovered { 1 } else { 0 }],
            None => &c,
        };
        stock_shadows(now, r, child);
    } else {
        let dark = child.ctx().theme() == egui::Theme::Dark;
        let v = crate::paint::StyleView::new(Some(&c), crate::paint::Ua::NONE, s.core[i].state, dark).with_overlay(s.style.overlay(id));
        crate::paint::paint_box(&mut crate::paint::PaintCx::for_node(child.painter(), id.to_raw()), &v, r);
        if let Some(col) = v.color("color").filter(|_| c.get("color").is_some()) {
            child.visuals_mut().override_text_color = Some(col);
        }
    }
}

fn color(c: &ComputedStyle, name: &str) -> Option<egui::Color32> {
    match c.get(name) {
        Some(ComputedValue::Color(k)) => {
            let [r, g, b, a] = k.rgba8();
            Some(egui::Color32::from_rgba_unmultiplied(r, g, b, a))
        }
        _ => None,
    }
}

fn px(c: &ComputedStyle, name: &str) -> Option<f32> {
    match c.get(name) {
        Some(ComputedValue::Length(l)) => Some(l.px),
        _ => None,
    }
}

/// A stock widget's outer shadows (CS5g) around its rect `r`, with its corner radius as egui draws
/// it. Inset shadows would paint under the widget's own background, so stock widgets skip them.
fn stock_shadows(c: &ComputedStyle, r: egui::Rect, child: &egui::Ui) {
    if shadow::shadows(c).is_none() {
        return;
    }
    let radius = px(c, "border-radius").unwrap_or_else(|| child.visuals().widgets.inactive.corner_radius.average());
    shadow::paint(c, r, radius, false, child.painter());
}

/// A stock widget's visuals in one interaction state from that state's computed style.
fn widget_visuals(c: &ComputedStyle, v: &mut egui::style::WidgetVisuals) {
    if let Some(col) = color(c, "background-color") {
        v.bg_fill = col;
        v.weak_bg_fill = col;
    }
    if let Some(col) = color(c, "border-color") {
        v.bg_stroke.color = col;
    }
    if let Some(w) = px(c, "border-width") {
        v.bg_stroke.width = w;
    }
    if let Some(rad) = px(c, "border-radius") {
        v.corner_radius = egui::CornerRadius::same(rad.round().clamp(0.0, 255.0) as u8);
    }
    if let Some(col) = color(c, "color") {
        v.fg_stroke.color = col;
    }
}

// ---- typography and leaf padding (CS5f) ----

/// The computed properties a leaf is measured with: a change remeasures it.
const MEASURED_PROPS: &[&str] = &["font-family", "font-size", "font-weight", "padding", "padding-bottom", "padding-left", "padding-right", "padding-top"];

/// Leaves whose CSS padding goes inside egui's frame, as `Spacing::button_padding` (the button is
/// the box). Other leaves are padded around the widget (taffy padding, drawn in the content box).
fn pads_inside(w: &W) -> bool {
    matches!(w, W::Button { .. } | W::ToggleButton { .. } | W::ComboBox { .. } | W::Icon { frame: true, .. })
}

/// Built-in leaves padded around the widget ([`content_rect`]).
fn pads_outside(w: &W) -> bool {
    !w.is_container() && !pads_inside(w) && !matches!(w, W::Ext(_))
}

/// Whether the runtime set prop `name` of node `i` explicitly (it wins over every rule).
fn explicit_prop(s: &Scene, i: usize, name: &str) -> bool {
    let kind = s.core[i].kind as usize;
    kinds::find_prop(kind, name)
        .and_then(|(id, _)| crate::scene::member_bit(kind, id, crate::scene::Member::Prop))
        .is_some_and(|b| b < 64 && s.core[i].explicit & (1 << b) != 0)
}

/// Padding (top, right, bottom, left) in points: the `padding` shorthand, then the per-side
/// longhands over it. `%` has no basis in a leaf's measure and counts as 0.
fn padding_sides(c: &ComputedStyle) -> Option<[f32; 4]> {
    let mut sides = match c.get("padding") {
        Some(ComputedValue::Insets(v)) => Some(v.map(|l| l.px)),
        _ => None,
    };
    for (k, name) in ["padding-top", "padding-right", "padding-bottom", "padding-left"].into_iter().enumerate() {
        if let Some(ComputedValue::Length(l)) = c.get(name) {
            sides.get_or_insert([0.0; 4])[k] = l.px;
        }
    }
    sides
}

/// The first family of a `font-family` list the context can draw: the generic names map to egui's
/// `Proportional` and `Monospace`, any other name to `FontFamily::Name` when the embedder bound it
/// (`Context::set_fonts`). An unbound name is skipped, never drawn (egui would panic).
fn resolve_family(ctx: &egui::Context, list: &str) -> Option<egui::FontFamily> {
    for f in list.split(',') {
        let name = f.trim().trim_matches(|c| c == '"' || c == '\'');
        let fam = match name.to_ascii_lowercase().as_str() {
            "" => continue,
            "sans-serif" | "serif" | "system-ui" | "ui-sans-serif" | "ui-serif" | "proportional" => egui::FontFamily::Proportional,
            "monospace" | "ui-monospace" => egui::FontFamily::Monospace,
            _ => egui::FontFamily::Name(name.into()),
        };
        if crate::fonts::has_family(ctx, &fam) {
            return Some(fam);
        }
    }
    None
}

/// The text a leaf draws, for the weight family's glyph check.
fn leaf_text(w: &W) -> &str {
    match w {
        W::Label { text, .. } | W::Button { text, .. } | W::ToggleButton { text, .. } | W::Checkbox { text, .. } | W::TextEdit { text, .. } | W::Slider { text, .. } => text,
        _ => "",
    }
}

/// The egui style leaf `i` measures (`layout.rs`) and paints ([`before_leaf`]) with under its
/// computed style: every text style at the computed `font-size`, `font-family` (not Monospace) and
/// `font-weight` (the family `fonts::register_weight` registered for the nearest weight; an explicit
/// `font-weight` prop wins and is applied by the leaf), and for [`pads_inside`] kinds the CSS
/// padding as `button_padding` (egui pads both sides alike: the mean of left/right and top/bottom).
/// `None`: the base style as is.
pub(crate) fn leaf_style(s: &Scene, i: usize, ctx: &egui::Context, base: &egui::Style) -> Option<egui::Style> {
    if !s.style.active {
        return None;
    }
    let w = s.w(i);
    if w.is_container() || matches!(w, W::Ext(_)) {
        return None;
    }
    let c = s.style.computed(s.slots.id_at(i)?)?;
    let size = px(c, "font-size").filter(|v| *v > 0.0);
    let family = match c.get("font-family") {
        Some(ComputedValue::Raw(t)) => resolve_family(ctx, t),
        _ => None,
    };
    let weight = match c.get("font-weight") {
        Some(ComputedValue::FontWeight(v)) if *v != 400 && !explicit_prop(s, i, "font-weight") => Some(*v as i64),
        _ => None,
    };
    let pad = if pads_inside(w) { padding_sides(c) } else { None };
    if size.is_none() && family.is_none() && weight.is_none() && pad.is_none() {
        return None;
    }
    let mut st = base.clone();
    let text = leaf_text(w);
    for (ts, f) in st.text_styles.iter_mut() {
        if let Some(v) = size {
            f.size = v;
        }
        if let Some(fam) = &family
            && *ts != egui::TextStyle::Monospace
        {
            f.family = fam.clone();
        }
        if let Some(v) = weight {
            f.family = crate::fonts::weighted_family(ctx, &f.family, v, text);
        }
    }
    if let Some([t, r, b, l]) = pad {
        st.spacing.button_padding = egui::vec2((l + r) / 2.0, (t + b) / 2.0);
    }
    Some(st)
}

/// The rect leaf `i`'s widget draws in: `r` less its CSS padding ([`pads_outside`] leaves; the
/// padding is in its taffy style). SG6 keeps no resolved padding per node, so it is resolved here
/// as taffy does for a flex child: `%` and `calc()` against the parent's content width.
pub(crate) fn content_rect(s: &Scene, i: usize, r: egui::Rect) -> egui::Rect {
    use taffy::ResolveOrZero;
    if !s.style.active || !pads_outside(s.w(i)) {
        return r;
    }
    let zero = taffy::LengthPercentage::length(0.0);
    let pad = s.layout[i].style.padding;
    if pad.left == zero && pad.right == zero && pad.top == zero && pad.bottom == zero {
        return r;
    }
    let basis = s.core[i].parent.and_then(|q| s.idx(q)).map(|q| {
        let w = s.core[q].rect.width();
        let ps = &s.layout[q].style;
        let side = |v: taffy::LengthPercentage| v.resolve_or_zero(Some(w), resolve_calc);
        (w - side(ps.padding.left) - side(ps.padding.right) - side(ps.border.left) - side(ps.border.right)).max(0.0)
    });
    let side = |v: taffy::LengthPercentage| v.resolve_or_zero(basis, resolve_calc);
    let p = taffy::Rect { left: side(pad.left), right: side(pad.right), top: side(pad.top), bottom: side(pad.bottom) };
    let min = r.min + egui::vec2(p.left, p.top);
    let max = (r.max - egui::vec2(p.right, p.bottom)).max(min);
    egui::Rect::from_min_max(min, max)
}

// ---- layout ----

/// A `calc()` value (`px + percent% of the basis`) behind a taffy calc handle: an index into a
/// process-wide interner, shifted past taffy's 3 tag bits.
fn calc_handle(l: Length) -> *const () {
    static TABLE: OnceLock<Mutex<Vec<(u32, u32)>>> = OnceLock::new();
    let key = (l.px.to_bits(), l.percent.to_bits());
    let mut t = TABLE.get_or_init(Mutex::default).lock().unwrap();
    let k = match t.iter().position(|e| *e == key) {
        Some(k) => k,
        None => {
            t.push(key);
            t.len() - 1
        }
    };
    CALCS.get_or_init(Mutex::default).lock().unwrap().resize(t.len(), (0.0, 0.0));
    CALCS.get().unwrap().lock().unwrap()[k] = (l.px, l.percent);
    std::ptr::without_provenance((k + 1) << 3)
}

static CALCS: OnceLock<Mutex<Vec<(f32, f32)>>> = OnceLock::new();

/// Resolves a handle from [`calc_handle`] against `basis` (taffy's `resolve_calc_value`).
pub(crate) fn resolve_calc(ptr: *const (), basis: f32) -> f32 {
    let k = (ptr.addr() >> 3).wrapping_sub(1);
    match CALCS.get().and_then(|c| c.lock().unwrap().get(k).copied()) {
        Some((px, pct)) => px + pct / 100.0 * basis,
        None => 0.0,
    }
}

fn lp(l: &Length) -> taffy::LengthPercentage {
    match (l.px != 0.0, l.percent != 0.0) {
        (_, false) => taffy::LengthPercentage::length(l.px),
        (false, true) => taffy::LengthPercentage::percent(l.percent / 100.0),
        (true, true) => taffy::LengthPercentage::calc(calc_handle(*l)),
    }
}

fn lpa(v: &ComputedValue) -> Option<taffy::LengthPercentageAuto> {
    Some(match v {
        ComputedValue::Auto => taffy::LengthPercentageAuto::auto(),
        ComputedValue::Length(l) => match (l.px != 0.0, l.percent != 0.0) {
            (_, false) => taffy::LengthPercentageAuto::length(l.px),
            (false, true) => taffy::LengthPercentageAuto::percent(l.percent / 100.0),
            (true, true) => taffy::LengthPercentageAuto::calc(calc_handle(*l)),
        },
        _ => return None,
    })
}

fn dim(v: &ComputedValue) -> Option<taffy::Dimension> {
    Some(match v {
        ComputedValue::Auto => taffy::Dimension::auto(),
        ComputedValue::Length(l) => match (l.px != 0.0, l.percent != 0.0) {
            (_, false) => taffy::Dimension::length(l.px),
            (false, true) => taffy::Dimension::percent(l.percent / 100.0),
            (true, true) => taffy::Dimension::calc(calc_handle(*l)),
        },
        _ => return None,
    })
}

fn len_of(v: &ComputedValue) -> Option<taffy::LengthPercentage> {
    match v {
        ComputedValue::Length(l) => Some(lp(l)),
        _ => None,
    }
}

fn align(k: &str) -> Option<taffy::AlignItems> {
    Some(match k {
        "start" => taffy::AlignItems::START,
        "end" => taffy::AlignItems::END,
        "center" => taffy::AlignItems::CENTER,
        "stretch" => taffy::AlignItems::STRETCH,
        "baseline" => taffy::AlignItems::BASELINE,
        _ => return None,
    })
}

/// One grid track: `<n>fr`, a length or percentage, `auto`, `min-content`, `max-content`.
fn track(t: &str) -> Option<taffy::TrackSizingFunction> {
    use taffy::style_helpers::{auto, fr, length, max_content, min_content, percent};
    let t = t.trim();
    Some(match t {
        "auto" => auto(),
        "min-content" => min_content(),
        "max-content" => max_content(),
        _ if t.ends_with("fr") => fr(t.trim_end_matches("fr").parse::<f32>().ok()?),
        _ if t.ends_with('%') => percent(t.trim_end_matches('%').parse::<f32>().ok()? / 100.0),
        _ => length(t.trim_end_matches("px").parse::<f32>().ok()?),
    })
}

/// `grid-template-columns`/`-rows`: tracks separated by spaces, with `repeat(<n>, <tracks>)`.
fn template(text: &str) -> Option<Vec<taffy::GridTemplateComponent<String>>> {
    let mut out = Vec::new();
    let mut rest = text.trim();
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix("repeat(") {
            let end = r.find(')')?;
            let (n, tracks) = r[..end].split_once(',')?;
            let n: u16 = n.trim().parse().ok()?;
            let tracks: Vec<_> = tracks.split_whitespace().map(track).collect::<Option<_>>()?;
            for _ in 0..n {
                out.extend(tracks.iter().map(|t| taffy::GridTemplateComponent::Single(*t)));
            }
            rest = r[end + 1..].trim_start();
        } else {
            let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            out.push(taffy::GridTemplateComponent::Single(track(&rest[..end])?));
            rest = rest[end..].trim_start();
        }
    }
    Some(out)
}

/// `grid-column`/`grid-row`: `<n>`, `span <n>`, `auto`, each side of an optional `/`.
fn placement(text: &str) -> Option<taffy::Line<taffy::GridPlacement<String>>> {
    use taffy::style_helpers::{line, span};
    let one = |t: &str| -> Option<taffy::GridPlacement<String>> {
        let t = t.trim();
        Some(if t == "auto" {
            taffy::GridPlacement::Auto
        } else if let Some(n) = t.strip_prefix("span") {
            span(n.trim().parse().ok()?)
        } else {
            line(t.parse().ok()?)
        })
    };
    let (a, b) = match text.split_once('/') {
        Some((a, b)) => (one(a)?, one(b)?),
        None => (one(text)?, taffy::GridPlacement::Auto),
    };
    Some(taffy::Line { start: a, end: b })
}

/// Node `i`'s taffy style: its props (`layout::style_of`), overlaid by its computed style for
/// every property the runtime did not set explicitly, then the TR1 clip.
pub(crate) fn layout_style(s: &Scene, i: usize) -> taffy::Style {
    let w = s.w(i);
    let vis = s.vis(i);
    let mut st = crate::layout::style_of(s.common(i), w, &vis);
    let Some(c) = s.slots.id_at(i).and_then(|id| s.style.computed(id)) else { return st };
    let explicit = |name: &str| explicit_prop(s, i, name);
    // (OW2: an own widget with children takes the box properties too)
    let container = w.is_container()
        || matches!(w, W::Ext(x) if x.entry().caps.contains(crate::provider::Caps::CONTAINER) || x.entry().caps.contains(crate::provider::Caps::CUSTOM_LAYOUT));
    // CS5f: padding is taffy's for containers and for leaves padded around the widget
    let pad_ok = container || pads_outside(w);
    for (p, v) in c.iter() {
        // per-side insets yield to an explicit shorthand prop
        let owner = match p.name {
            n if n.starts_with("margin-") => "margin",
            n if n.starts_with("padding-") => "padding",
            n => n,
        };
        if explicit(owner) {
            continue;
        }
        let kw = match v {
            ComputedValue::Keyword(k) => Some(*k),
            _ => None,
        };
        match p.name {
            "width" => st.size.width = dim(v).unwrap_or(st.size.width),
            "height" => st.size.height = dim(v).unwrap_or(st.size.height),
            "min-width" => st.min_size.width = lpa(v).unwrap_or(st.min_size.width),
            "min-height" => st.min_size.height = lpa(v).unwrap_or(st.min_size.height),
            "max-width" => st.max_size.width = lpa(v).unwrap_or(st.max_size.width),
            "max-height" => st.max_size.height = lpa(v).unwrap_or(st.max_size.height),
            "flex-basis" => st.flex_basis = dim(v).unwrap_or(st.flex_basis),
            "flex-grow" | "flex-shrink" => {
                if let ComputedValue::Number(n) = v {
                    if p.name == "flex-grow" { st.flex_grow = *n } else { st.flex_shrink = *n }
                }
            }
            "align-self" => st.align_self = kw.and_then(align),
            "margin" => {
                if let ComputedValue::Insets([t, r, b, l]) = v {
                    let side = |x: &Length| lpa(&ComputedValue::Length(*x)).unwrap_or(taffy::LengthPercentageAuto::length(0.0));
                    st.margin = taffy::Rect { top: side(t), right: side(r), bottom: side(b), left: side(l) };
                }
            }
            "margin-top" => st.margin.top = lpa(v).unwrap_or(st.margin.top),
            "margin-right" => st.margin.right = lpa(v).unwrap_or(st.margin.right),
            "margin-bottom" => st.margin.bottom = lpa(v).unwrap_or(st.margin.bottom),
            "margin-left" => st.margin.left = lpa(v).unwrap_or(st.margin.left),
            // SG4: out-of-flow positioning (`layout.rs` hoists absolute boxes)
            "position" => {
                st.position = match kw {
                    Some("absolute") => taffy::Position::Absolute,
                    Some("relative") => taffy::Position::Relative,
                    Some("static") => taffy::Position::Static,
                    _ => st.position,
                }
            }
            "left" => st.inset.left = lpa(v).unwrap_or(st.inset.left),
            "top" => st.inset.top = lpa(v).unwrap_or(st.inset.top),
            "right" => st.inset.right = lpa(v).unwrap_or(st.inset.right),
            "bottom" => st.inset.bottom = lpa(v).unwrap_or(st.inset.bottom),
            // OW2: a scrolled or clipped axis keeps the box from growing to its content
            "overflow-x" | "overflow-y" => {
                let o = match kw {
                    Some("scroll" | "auto") => Some(taffy::Overflow::Scroll),
                    Some("hidden") => Some(taffy::Overflow::Hidden),
                    Some("clip") => Some(taffy::Overflow::Clip),
                    // `visible` (the initial value) keeps what the props gave (a scroll area's axes)
                    _ => None,
                };
                if let Some(o) = o {
                    if p.name == "overflow-x" { st.overflow.x = o } else { st.overflow.y = o }
                }
            }
            // OW2: classic scroll bars take a gutter beside the content (`scrollbar-w`)
            "scrollbar-gutter" if kw == Some("stable") => {
                st.scrollbar_width = crate::paint::StyleView::ua_only(crate::widgets::scroll_area::TRACK_UA, crate::provider::StateBits::NONE, true).px("width").unwrap_or(10.0);
            }
            "visible" => {
                if matches!(v, ComputedValue::Bool(false)) {
                    st.display = taffy::Display::None;
                }
            }
            "display" => match kw {
                Some("none") => st.display = taffy::Display::None,
                Some("grid") if container && st.display != taffy::Display::None => st.display = taffy::Display::Grid,
                Some("flex") if st.display != taffy::Display::None => st.display = taffy::Display::Flex,
                _ => {}
            },
            "grid-column" | "grid-row" => {
                if let ComputedValue::Raw(t) = v
                    && let Some(l) = placement(t)
                {
                    if p.name == "grid-column" { st.grid_column = l } else { st.grid_row = l }
                }
            }
            "padding" if pad_ok => {
                if let ComputedValue::Insets([t, r, b, l]) = v {
                    st.padding = taffy::Rect { top: lp(t), right: lp(r), bottom: lp(b), left: lp(l) };
                }
            }
            "padding-top" if pad_ok => st.padding.top = len_of(v).unwrap_or(st.padding.top),
            "padding-right" if pad_ok => st.padding.right = len_of(v).unwrap_or(st.padding.right),
            "padding-bottom" if pad_ok => st.padding.bottom = len_of(v).unwrap_or(st.padding.bottom),
            "padding-left" if pad_ok => st.padding.left = len_of(v).unwrap_or(st.padding.left),
            _ if !container => {}
            "flex-direction" => {
                st.flex_direction = match kw {
                    Some("row") => taffy::FlexDirection::Row,
                    Some("row-reverse") => taffy::FlexDirection::RowReverse,
                    Some("column-reverse") => taffy::FlexDirection::ColumnReverse,
                    _ => taffy::FlexDirection::Column,
                }
            }
            "flex-wrap" => {
                st.flex_wrap = match kw {
                    Some("wrap") => taffy::FlexWrap::Wrap,
                    Some("wrap-reverse") => taffy::FlexWrap::WrapReverse,
                    _ => taffy::FlexWrap::NoWrap,
                }
            }
            "align-items" => st.align_items = kw.and_then(align).unwrap_or(taffy::AlignItems::STRETCH),
            "justify-content" => {
                st.justify_content = match kw {
                    Some("end") => taffy::AlignContent::END,
                    Some("center") => taffy::AlignContent::CENTER,
                    Some("space-between") => taffy::AlignContent::SPACE_BETWEEN,
                    Some("space-around") => taffy::AlignContent::SPACE_AROUND,
                    Some("space-evenly") => taffy::AlignContent::SPACE_EVENLY,
                    _ => taffy::AlignContent::START,
                }
            }
            "gap" => {
                if let Some(g) = len_of(v) {
                    st.gap = taffy::Size { width: g, height: g };
                }
            }
            "row-gap" => st.gap.height = len_of(v).unwrap_or(st.gap.height),
            "column-gap" => st.gap.width = len_of(v).unwrap_or(st.gap.width),
            "grid-template-columns" | "grid-template-rows" => {
                if let ComputedValue::Raw(t) = v
                    && let Some(tracks) = template(t)
                {
                    if p.name == "grid-template-columns" { st.grid_template_columns = tracks } else { st.grid_template_rows = tracks }
                }
            }
            _ => {}
        }
    }
    // TR1's clip channel caps what CSS asked for
    if let Some([cw, ch]) = vis.clip {
        if cw >= 0.0 {
            st.max_size.width = taffy::LengthPercentageAuto::length(cw);
            st.min_size.width = taffy::LengthPercentageAuto::length(0.0);
            st.size.width = taffy::Dimension::auto();
        }
        if ch >= 0.0 {
            st.max_size.height = taffy::LengthPercentageAuto::length(ch);
            st.min_size.height = taffy::LengthPercentageAuto::length(0.0);
            st.size.height = taffy::Dimension::auto();
        }
    }
    st
}

// ---- inputs ----

/// Splits `id\x01text`.
fn sheet_payload(text: &str) -> (&str, Option<&str>) {
    match text.split_once(SVL_STYLE_SEP) {
        Some((id, t)) => (id, Some(t)),
        None => (text, None),
    }
}

impl Scene {
    fn apply_classes(&mut self, id: NodeId, classes: Vec<Atom>) {
        self.activate_style();
        let old = std::mem::take(&mut self.style.node_mut(id).classes);
        let inv = self.style.stylist.invalidation();
        let mut reach = Reach::NONE;
        for c in old.iter().filter(|c| !classes.contains(c)).chain(classes.iter().filter(|c| !old.contains(c))) {
            reach |= inv.class_changed(c.as_str());
        }
        self.style.node_mut(id).classes = classes;
        self.mark_reach(id, reach);
    }

    fn apply_name(&mut self, id: NodeId, name: Option<&str>) {
        self.activate_style();
        let old = self.style.node_mut(id).name.take();
        let inv = self.style.stylist.invalidation();
        let mut reach = Reach::NONE;
        for n in old.as_deref().into_iter().chain(name) {
            reach |= inv.id_changed(n);
        }
        self.style.node_mut(id).name = name.map(Into::into);
        self.mark_reach(id, reach);
    }

    fn apply_inline(&mut self, id: NodeId, text: Option<&str>) -> Vec<Diagnostic> {
        self.activate_style();
        let (block, mut diags) = match text {
            Some(t) if !t.trim().is_empty() => {
                let (b, d) = DeclBlock::parse_inline(t);
                (Some(Arc::new(b)), d)
            }
            _ => (None, Vec::new()),
        };
        if let Some(b) = &block {
            diags.extend(self.style.stylist.check_inline(b));
        }
        self.style.node_mut(id).inline = block;
        self.mark_style(id);
        diags
    }

    /// Installs (or with `None` removes) sheet `name`; `@tokens…` and `@theme…` sheets are the
    /// theme origin.
    pub(crate) fn apply_sheet(&mut self, name: &str, css: Option<&str>) -> Vec<Diagnostic> {
        self.activate_style();
        let diags = match css {
            Some(css) => self.install_sheet(name, css),
            None => {
                self.uninstall_sheet(name);
                Vec::new()
            }
        };
        self.observe_for_style();
        self.style.all = true;
        diags
    }

    fn uninstall_sheet(&mut self, name: &str) {
        if let Some(k) = self.style.sheets.iter().position(|(n, _)| n == name) {
            let (_, id) = self.style.sheets.remove(k);
            self.style.stylist.remove_sheet(id);
        }
    }

    /// Replaces sheet `name` (no observer or restyle bookkeeping: [`Scene::apply_sheet`]'s).
    fn install_sheet(&mut self, name: &str, css: &str) -> Vec<Diagnostic> {
        self.uninstall_sheet(name);
        let sheet = Stylesheet::parse(css, &ParseOptions { states: Some(&ProviderStates) });
        let mut diags = sheet.diagnostics.clone();
        let origin = if name.starts_with("@tokens") || name.starts_with(THEME_SHEETS) { Origin::Theme } else { Origin::Author };
        let (id, d) = self.style.stylist.add_sheet(&sheet, origin);
        diags.extend(d);
        self.style.sheets.push((name.to_string(), id));
        diags
    }

    /// The runtime's clear (`SHEET` with no text, at start and hot reload): every sheet but the
    /// host's theme sheets (`@theme…`, DS1).
    fn clear_sheets(&mut self) {
        let (keep, gone): (Vec<_>, Vec<_>) = std::mem::take(&mut self.style.sheets).into_iter().partition(|(n, _)| n.starts_with(THEME_SHEETS));
        self.style.sheets = keep;
        for (_, id) in gone {
            self.style.stylist.remove_sheet(id);
        }
        if self.style.active {
            self.observe_for_style();
            self.style.all = true;
        }
    }

    /// The media environment of this frame: the viewport size and the scheme.
    fn update_media(&mut self, ctx: &egui::Context) {
        let r = ctx.content_rect();
        let scheme = self.style.scheme.unwrap_or(if ctx.global_style().visuals.dark_mode { ColorScheme::Dark } else { ColorScheme::Light });
        let mut env = MediaEnv { width: r.width(), height: r.height(), color_scheme: scheme, ..*self.style.stylist.media_env() };
        if let Some(c) = self.style.contrast {
            env.contrast = c;
        }
        env.reduced_motion = self.style.reduced_motion;
        if env.reduced_motion && !self.style.stylist.media_env().reduced_motion {
            self.style.end_transitions();
        }
        if env != *self.style.stylist.media_env() && !self.style.stylist.set_media(env).is_empty() {
            self.style.all = true;
        }
        let body = egui::TextStyle::Body.resolve(&ctx.global_style()).size;
        if self.style.cx.default_font_size != body {
            self.style.cx = ComputeContext { root_font_size: body, default_font_size: body };
            self.style.all = true;
        }
    }
}

fn parse_classes(text: &str) -> Vec<Atom> {
    let mut v: Vec<Atom> = Vec::new();
    for c in text.split_whitespace() {
        if !v.iter().any(|x| x.as_str() == c) {
            v.push(Atom::new(c));
        }
    }
    v
}

/// The text of a token sheet: `:root { --name: value; }`, under `@media (prefers-color-scheme)`
/// when it is for one scheme.
fn tokens_css(scheme: Option<ColorScheme>, map: &[(&str, &str)]) -> Result<String, String> {
    let mut body = String::new();
    for (k, v) in map {
        if !k.starts_with("--") {
            return Err(format!("token names start with '--': {k}"));
        }
        body.push_str(&format!("  {k}: {v};\n"));
    }
    let root = format!(":root {{\n{body}}}\n");
    Ok(match scheme {
        None => root,
        Some(ColorScheme::Light) => format!("@media (prefers-color-scheme: light) {{\n{root}}}\n"),
        Some(ColorScheme::Dark) => format!("@media (prefers-color-scheme: dark) {{\n{root}}}\n"),
    })
}

impl EguiBackend {
    /// The `style` host op (CS5e): the engine's inputs and the TR1 visual channels.
    pub(crate) fn style_op(&self, op: u32, h: svl_handle, text: Option<&str>) -> StrResult<i32> {
        match op {
            SVL_STYLE_TARGET => return Ok(SVL_STYLE_TARGET_ENGINE),
            SVL_STYLE_OPACITY | SVL_STYLE_TRANSFORM | SVL_STYLE_CLIP | SVL_STYLE_INERT => {
                self.visual_op(op, h, text)?;
                if op == SVL_STYLE_INERT {
                    // `:exiting` follows the inert channel at the next walk
                    let mut s = self.0.scene.borrow_mut();
                    if s.style.is_active()
                        && let Ok(id) = s.live(h)
                    {
                        s.mark_style(id);
                    }
                }
                return Ok(0);
            }
            // TR2: answers 1 = the runtime hands this backend the animation (any other answer: it plays it)
            SVL_STYLE_ANIMATE => {
                self.animate_op(h, text)?;
                return Ok(1);
            }
            SVL_STYLE_SHEET => {
                let mut s = self.0.scene.borrow_mut();
                match text {
                    None => s.clear_sheets(),
                    Some(t) => {
                        let (name, css) = sheet_payload(t);
                        for d in s.apply_sheet(name, css) {
                            self.warn_style(name, &d);
                        }
                    }
                }
                drop(s);
                self.touch_all();
                return Ok(0);
            }
            SVL_STYLE_SCHEME => {
                let scheme = text.and_then(|t| t.split(SVL_STYLE_SEP).next()).map(|s| if s == "dark" { ColorScheme::Dark } else { ColorScheme::Light });
                // the runtime's `setColorScheme` switches an installed theme's scheme (DS1)
                self.set_color_scheme(scheme);
                return Ok(0);
            }
            _ => {}
        }
        let r = {
            let mut s = self.0.scene.borrow_mut();
            let id = s.live(h)?;
            match op {
                SVL_STYLE_CLASSES => s.apply_classes(id, parse_classes(text.unwrap_or(""))),
                SVL_STYLE_NAME => s.apply_name(id, text.filter(|t| !t.is_empty())),
                SVL_STYLE_INLINE => {
                    for d in s.apply_inline(id, text) {
                        self.warn_style("style=", &d);
                    }
                }
                _ => {}
            }
            s.core[s.ix(id)].root
        };
        self.touched(Some(r));
        Ok(0)
    }

    fn warn_style(&self, what: &str, d: &Diagnostic) {
        if d.severity == svelters_style::Severity::Error || cfg!(debug_assertions) {
            eprintln!("[svelters-egui/style] {what}:{}:{}: {}", d.line, d.column, d.message);
        }
    }

    /// Asks every presented root for a frame (a sheet or scheme change can restyle anything).
    pub(crate) fn touch_all(&self) {
        let roots: Vec<_> = self.0.scene.borrow().roots.iter().map(|(r, _)| r).collect();
        for r in roots {
            self.touched(Some(r));
        }
    }

    /// The logic step (ARCH-SCENE §7.2 step 7): media, then the marked nodes, parents first.
    pub(crate) fn restyle_pending(&self, ctx: &egui::Context) {
        let mut s = self.0.scene.borrow_mut();
        if !s.style.is_active() {
            return;
        }
        s.update_media(ctx);
        let n = s.restyle_marked();
        drop(s);
        if n > 0 {
            self.touch_all();
        }
    }

    // ---- the Rust API ----

    /// Installs or replaces the stylesheet `name` (raw CSS, author origin; a name starting with
    /// `@tokens` is the theme origin). Returns the parse and table diagnostics.
    pub fn add_sheet(&self, name: &str, css: &str) -> Vec<Diagnostic> {
        let d = self.0.scene.borrow_mut().apply_sheet(name, Some(css));
        self.touch_all();
        d
    }

    /// Removes the stylesheet `name`.
    pub fn remove_sheet(&self, name: &str) {
        self.0.scene.borrow_mut().apply_sheet(name, None);
        self.touch_all();
    }

    /// SV5: a provider's resolved tokens (`--cad-panel: #202020`, any namespace) for one scheme,
    /// or for both with `None`, at theme origin below every component sheet. Replaces the tokens
    /// set before for that scheme.
    pub fn set_tokens(&self, scheme: Option<ColorScheme>, map: &[(&str, &str)]) -> Result<(), String> {
        let css = tokens_css(scheme, map)?;
        let name = match scheme {
            None => "@tokens",
            Some(ColorScheme::Light) => "@tokens:light",
            Some(ColorScheme::Dark) => "@tokens:dark",
        };
        self.add_sheet(name, &css);
        Ok(())
    }

    /// `prefers-color-scheme` (`None`: follow egui's `Visuals::dark_mode`). With a theme installed
    /// ([`EguiBackend::set_theme`]), a scheme switches the theme to that scheme (its tokens and
    /// the context's Style follow).
    pub fn set_color_scheme(&self, scheme: Option<ColorScheme>) {
        let themed = self.0.theme.borrow().as_ref().zip(scheme).filter(|(t, s)| t.scheme() != *s).map(|(t, s)| t.with_scheme(s));
        if let Some(t) = themed {
            self.set_theme(&t);
            return;
        }
        self.0.scene.borrow_mut().style.scheme = scheme;
        self.touch_all();
    }

    /// The node's user classes (as the `class` attribute).
    pub fn set_classes(&self, h: Handle, classes: &str) -> Result<(), String> {
        self.style_op(SVL_STYLE_CLASSES, h, Some(classes)).map(|_| ()).map_err(|e| e.to_string())
    }

    /// The node's inline declarations (as `style=`).
    pub fn set_inline(&self, h: Handle, decls: Option<&str>) -> Result<(), String> {
        self.style_op(SVL_STYLE_INLINE, h, decls).map(|_| ()).map_err(|e| e.to_string())
    }

    /// The node's computed style (after the last restyle), `None` before any style input.
    pub fn computed_style(&self, h: Handle) -> Option<Arc<ComputedStyle>> {
        let s = self.0.scene.borrow();
        let id = s.live(h).ok()?;
        s.style.computed(id).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The engine's state bits are the scene's (CSS.md §2.3): same names, same bits.
    #[test]
    fn state_bit_tables_are_pinned_together() {
        assert_eq!(StateMask::BUILTIN.len(), StateBits::BUILTIN.len());
        for ((a, ma), (b, mb)) in StateMask::BUILTIN.iter().zip(StateBits::BUILTIN) {
            assert_eq!((a, ma.0), (b, mb.0));
        }
    }

    #[test]
    fn grid_templates_and_placements_parse() {
        let t = template("100px repeat(2, 1fr) auto").unwrap();
        assert_eq!(t.len(), 4);
        assert!(template("1fr nope").is_none());
        let p = placement("2 / span 3").unwrap();
        assert_eq!(p.start, taffy::style_helpers::line::<taffy::GridPlacement<String>>(2));
        assert_eq!(p.end, taffy::style_helpers::span::<taffy::GridPlacement<String>>(3));
    }

    #[test]
    fn calc_handles_resolve() {
        let h = calc_handle(Length { px: -8.0, percent: 50.0 });
        assert_eq!(h.addr() & 7, 0, "taffy's tag bits stay clear");
        assert_eq!(resolve_calc(h, 200.0), 92.0);
        assert_eq!(calc_handle(Length { px: -8.0, percent: 50.0 }), h, "interned");
    }

    #[test]
    fn token_sheets() {
        assert_eq!(tokens_css(None, &[("--cad-bg", "#101010")]).unwrap(), ":root {\n  --cad-bg: #101010;\n}\n");
        assert!(tokens_css(Some(ColorScheme::Dark), &[("--a", "1")]).unwrap().starts_with("@media (prefers-color-scheme: dark)"));
        assert!(tokens_css(None, &[("bg", "1")]).is_err());
    }
}
