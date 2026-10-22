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
//!
//! Precedence (CSS.md §2.8): a prop the runtime set (`NodeCore::explicit`) wins over every rule,
//! then inline, then rules, then the native default.
//!
//! Nothing runs until the first style input: an app without styles pays nothing.
//!
//! Writer lane: STYLE.

use crate::backend::EguiBackend;
use crate::kinds;
use crate::scene::{Change, ChangeFilter, ChangeMask, Dirty, Handle, NodeId, NodeRef, Observer, Origin as SceneOrigin, Scene, StateBits, W};
use std::borrow::Cow;
use std::num::NonZeroU64;
use std::sync::{Arc, Mutex, OnceLock};
use svelters_abi::*;
use svelters_style::{
    Atom, ColorScheme, ComputeContext, ComputedStyle, ComputedValue, DeclBlock, Diagnostic, Element, Length, MatchCaches, MediaEnv, Origin, ParseOptions, Reach, SheetId,
    StateMask, StateNames, Stylesheet, Stylist,
};

/// `SVL_STYLE_TARGET` answer of a backend that runs the Rust engine itself: the runtime sends raw
/// CSS (sheets, inline text) and the user classes. Runtimes that do not know the code treat the
/// backend as unstyled, as with `SVL_STYLE_TARGET_NONE`.
pub const SVL_STYLE_TARGET_ENGINE: i32 = 4;

/// Properties the egui cascade computes with no kind prop behind them: `(name, type, keywords)`.
/// They go into the egui style table as `via: css` entries (`spec.rs`), so the compiler leaves
/// them to the engine.
pub(crate) const ENGINE_PROPS: &[(&str, &str, &[&str])] = &[
    ("background-color", "color", &[]),
    ("border-color", "color", &[]),
    ("border-radius", "length", &[]),
    ("border-width", "length", &[]),
    ("color", "color", &[]),
    ("display", "enum", &["flex", "grid", "none"]),
    // computed and inherited (`em`); measuring and painting with it is deferred
    ("font-size", "length", &[]),
    ("grid-column", "raw", &[]),
    ("grid-row", "raw", &[]),
    ("grid-template-columns", "raw", &[]),
    ("grid-template-rows", "raw", &[]),
    ("margin-bottom", "length", &[]),
    ("margin-left", "length", &[]),
    ("margin-right", "length", &[]),
    ("margin-top", "length", &[]),
    ("opacity", "number", &[]),
    ("padding-bottom", "length", &[]),
    ("padding-left", "length", &[]),
    ("padding-right", "length", &[]),
    ("padding-top", "length", &[]),
];

/// The bits the walk owns: set from the pass's interaction and from props, never by providers.
pub(crate) const WALK_BITS: StateBits = StateBits(
    StateBits::HOVERED.0 | StateBits::PRESSED.0 | StateBits::FOCUSED.0 | StateBits::FOCUS_WITHIN.0 | StateBits::DISABLED.0 | StateBits::CHECKED.0 | StateBits::SELECTED.0 | StateBits::EXITING.0,
);

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
    /// The scheme set with `SVL_STYLE_SCHEME` or `set_color_scheme` (else egui's dark mode).
    scheme: Option<ColorScheme>,
    /// The focused node at the end of the last walk (for `:focus-within`).
    focus: Option<NodeId>,
    /// A walk-time restyle that needs another frame (an ancestor or a layout change).
    pub(crate) repaint: bool,
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
            focus: None,
            repaint: false,
        }
    }
}

impl SceneStyle {
    pub(crate) fn is_active(&self) -> bool {
        self.active
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
}

// ---- restyle ----

/// Stock widgets: egui draws them with per-state `WidgetVisuals`.
fn is_stock(w: &W) -> bool {
    matches!(w, W::Button { .. } | W::Checkbox { .. } | W::Slider { .. } | W::TextEdit { .. })
}

/// Whether two styles differ in what children inherit (inherited properties, custom properties).
fn inherited_differs(a: Option<&ComputedStyle>, b: &ComputedStyle) -> bool {
    let Some(a) = a else { return true };
    let t = b.table();
    (0..t.props.len()).any(|k| t.props[k].is_inherited() && a.get_index(k) != b.get_index(k)) || !a.customs().eq(b.customs())
}

impl Scene {
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

    /// Turns the cascade on: every node gets a style, and the journal observer starts.
    fn activate_style(&mut self) {
        if !self.style.active {
            self.style.active = true;
            self.style.all = true;
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
        caches.clear();
        self.style.caches = caches;
        let ns = self.style.node_mut(id);
        let changed = ns.computed.as_deref() != Some(&c) || ns.states.as_deref().map(|s| &s[..]) != states.as_deref().map(|s| &s[..]);
        let inherit = inherited_differs(ns.computed.as_deref(), &c);
        if !changed {
            return false;
        }
        ns.computed = Some(Arc::new(c));
        ns.states = states;
        self.core[i].dirty.insert(Dirty::PAINT);
        let t = layout_style(self, i);
        if t != self.layout[i].style {
            self.layout[i].style = t;
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
    fn set_walk_state(&mut self, i: usize, bits: StateBits) {
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
    let mut b = s.prop_bits(i) | StateBits(s.core[i].state.0 & (StateBits::FOCUSED.0 | StateBits::FOCUS_WITHIN.0));
    b.set(StateBits::HOVERED, hovered);
    b.set(StateBits::PRESSED, pressed);
    s.set_walk_state(i, b);
    let id = s.slots.id_at(i)?;
    let c = s.style.computed(id)?.clone();
    paint_box(&c, r, ui.painter());
    match c.get("opacity") {
        Some(ComputedValue::Number(o)) if *o < 1.0 => Some(o.max(0.0)),
        _ => None,
    }
}

/// Before a leaf or provider node paints into `child`: its state from this pass's response, the
/// restyle that reaches, then its visuals on `child`.
pub(crate) fn before_leaf(s: &mut Scene, i: usize, r: egui::Rect, child: &mut egui::Ui) {
    if !s.style.active {
        return;
    }
    let Some(id) = s.slots.id_at(i) else { return };
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
    } else {
        paint_box(&c, r, child.painter());
        if let Some(col) = color(&c, "color") {
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

/// A box's background and border, behind its content.
fn paint_box(c: &ComputedStyle, r: egui::Rect, p: &egui::Painter) {
    let fill = color(c, "background-color");
    let bw = px(c, "border-width").unwrap_or(0.0);
    let stroke = match color(c, "border-color") {
        Some(col) if bw > 0.0 => Some(egui::Stroke::new(bw, col)),
        _ => None,
    };
    if fill.is_none() && stroke.is_none() {
        return;
    }
    let radius = px(c, "border-radius").unwrap_or(0.0);
    p.add(egui::epaint::RectShape::new(r, radius, fill.unwrap_or(egui::Color32::TRANSPARENT), stroke.unwrap_or(egui::Stroke::NONE), egui::StrokeKind::Inside));
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
    let kind = s.core[i].kind as usize;
    let explicit = |name: &str| {
        kinds::find_prop(kind, name)
            .and_then(|(id, _)| crate::scene::member_bit(kind, id, crate::scene::Member::Prop))
            .is_some_and(|b| b < 64 && s.core[i].explicit & (1 << b) != 0)
    };
    let container = w.is_container();
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
                if let Some(m) = lpa(v) {
                    st.margin = taffy::Rect { left: m, right: m, top: m, bottom: m };
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
            "padding" => {
                if let Some(g) = len_of(v) {
                    st.padding = taffy::Rect { left: g, right: g, top: g, bottom: g };
                }
            }
            "padding-top" => st.padding.top = len_of(v).unwrap_or(st.padding.top),
            "padding-right" => st.padding.right = len_of(v).unwrap_or(st.padding.right),
            "padding-bottom" => st.padding.bottom = len_of(v).unwrap_or(st.padding.bottom),
            "padding-left" => st.padding.left = len_of(v).unwrap_or(st.padding.left),
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

    /// Installs (or with `None` removes) sheet `name`; `@tokens…` sheets are the theme origin.
    fn apply_sheet(&mut self, name: &str, css: Option<&str>) -> Vec<Diagnostic> {
        self.activate_style();
        if let Some(k) = self.style.sheets.iter().position(|(n, _)| n == name) {
            let (_, id) = self.style.sheets.remove(k);
            self.style.stylist.remove_sheet(id);
        }
        let mut diags = Vec::new();
        if let Some(css) = css {
            let sheet = Stylesheet::parse(css, &ParseOptions { states: Some(&ProviderStates) });
            diags.extend(sheet.diagnostics.iter().cloned());
            let origin = if name.starts_with("@tokens") { Origin::Theme } else { Origin::Author };
            let (id, d) = self.style.stylist.add_sheet(&sheet, origin);
            diags.extend(d);
            self.style.sheets.push((name.to_string(), id));
        }
        self.observe_for_style();
        self.style.all = true;
        diags
    }

    fn clear_sheets(&mut self) {
        for (_, id) in std::mem::take(&mut self.style.sheets) {
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
        let env = MediaEnv { width: r.width(), height: r.height(), color_scheme: scheme, ..*self.style.stylist.media_env() };
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
                self.0.scene.borrow_mut().style.scheme = scheme;
                self.touch_all();
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
    fn touch_all(&self) {
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

    /// `prefers-color-scheme` (`None`: follow egui's `Visuals::dark_mode`).
    pub fn set_color_scheme(&self, scheme: Option<ColorScheme>) {
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
