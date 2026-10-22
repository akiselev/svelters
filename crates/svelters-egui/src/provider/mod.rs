//! Rust-provided kinds (ARCH-EMBED §8.1–§8.4, RA1): an app crate defines its own native egui widget
//! kinds in Rust and uses them from Svelte as elements, fully typed.
//!
//! ```ignore
//! use svelters_egui::prelude::*;
//!
//! /// A unit-aware numeric field.
//! #[svelters::widget(namespace = "cad", tag = "quantity-field", caps = FOCUSABLE | EDITABLE)]
//! pub struct QuantityField {
//!     #[prop(bind)] value: Option<f64>,
//!     #[prop] label: String,
//!     #[event] accepted: Event<f64>,
//!     #[state] text: String,
//! }
//! impl Widget for QuantityField {
//!     fn measure(&mut self, cx: &mut MeasureCx, avail: AvailableSpace) -> egui::Vec2 { … }
//!     fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx) -> egui::Response { … }
//! }
//! ```
//!
//! - `#[svelters::widget]` (svelters-macros) registers a [`KindEntry`] with `inventory` and the JS
//!   types of the props and event payloads with the export schema (`KindTypesDef`), so kind props and
//!   exports share one type system (`JsType`/`FromJs`/`IntoJs`, ARCH-EMBED §8.6).
//! - [`crate::kinds::KindRegistry`] collects the entries at startup (pack-62 ids, sorted by spec id;
//!   duplicates fail), and [`crate::spec`] emits the compiler's spec fragment and the
//!   `IntrinsicElements` typings from it ([`crate::spec::dump`]).
//! - The scene stores the widget struct in the node (`W::Ext`); the walk calls [`Widget::ui`] at the
//!   rect taffy computed, layout calls [`Widget::measure`].
//!
//! - `#[svelters::behavior]` registers a behaviour the same way (EV2b, ARCH-EMBED §8.5): an attached
//!   kind ([`KindEntry::attaches_to`]) whose struct implements [`Behavior`] instead of [`Widget`],
//!   used from Svelte as `use:name` (`behavior.rs`).
//! - SG4 (ARCH-SCENE §7.3): the walk takes a widget's payload out of the scene while its code runs
//!   ([`with_taken`]), so [`WidgetCx::scene`] reads the rest of the scene, [`WidgetCx::children`]
//!   draws its shadow children, and a panic drops the widget (a fresh one replaces it) and faults
//!   its root only. Opt-in hooks by capability: [`Widget::layout`] (`CUSTOM_LAYOUT`),
//!   [`Widget::prepare`] (`PREPARE`), [`Widget::hit_test`] (`HIT_TEST`, SG3).
//!
//! Files: this one holds the trait, the contexts and the registration types; `state.rs` the state
//! bits; `behavior.rs` the behaviours (EVENTS, EV2b); `pass.rs` (SG4: `StateProvider`, `ScenePass`,
//! `PrepareCx`) is SCENE's. Writer lane: RUSTAPI (RA1, RA3).

pub(crate) mod behavior;
pub(crate) mod pass;
mod state;

pub use behavior::{__new_behavior, Behavior, BehaviorBox, BehaviorCx};

pub use crate::scene::ext::{ExtBlock, ExtEntry, ExtId, ExtKey, ExtTicks, Storage};
pub use crate::scene::{Hit, HitCx};
pub use state::{IntoStateBits, StateBit, StateBitDef, StateBits, by_name as state_by_name, registry as state_registry};

use crate::scene::{Handle, Node, W};
use crate::walk::Out;
use std::any::Any;
use std::cell::RefCell;
use std::marker::PhantomData;
use svelters_host::exports::{CallCx, IntoJs, NativeError, Value};

#[doc(hidden)]
pub mod __private {
    //! Used by the code `#[svelters::widget]` generates.
    pub use inventory;
    pub use svelters_host::exports::__private::{KindTypesDef, TypeBuilder, TypeRef};
    pub use svelters_host::exports::{CallCx, FromJs, IntoJs, JsType, NativeError, Value};

    /// A prop value that did not convert, with the prop's name in the message.
    pub fn at(e: NativeError, prop: &str) -> NativeError {
        svelters_host::exports::__private::at(e, prop)
    }

    /// An index the generated tables do not have (an internal error).
    pub fn no_member(what: &str, i: usize) -> NativeError {
        svelters_host::exports::__private::host_error("internal", format!("no {what} #{i} on this kind"))
    }
}

// ---- capabilities ----

/// What a kind does, as the macro's `caps = A | B` declares it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Caps(pub u32);

impl Caps {
    pub const NONE: Caps = Caps(0);
    /// Takes keyboard focus (tab order, `request-focus`).
    pub const FOCUSABLE: Caps = Caps(1 << 0);
    /// Edits a value from user input (its bound props change during the pass).
    pub const EDITABLE: Caps = Caps(1 << 1);
    /// Height depends on width: layout measures it again at the width it gets.
    pub const HEIGHT_FOR_WIDTH: Caps = Caps(1 << 2);
    // Reserved for RA3/SG4 (containers, custom layout, prepare, virtual, hit-test).
    pub const CONTAINER: Caps = Caps(1 << 8);
    pub const CUSTOM_LAYOUT: Caps = Caps(1 << 9);
    pub const PREPARE: Caps = Caps(1 << 10);
    pub const VIRTUAL: Caps = Caps(1 << 11);
    pub const HIT_TEST: Caps = Caps(1 << 12);

    pub const fn union(self, o: Caps) -> Caps {
        Caps(self.0 | o.0)
    }
    pub const fn contains(self, o: Caps) -> bool {
        self.0 & o.0 == o.0
    }

    /// The names of the set capabilities (spec and docs).
    pub fn names(self) -> Vec<&'static str> {
        [
            (Self::FOCUSABLE, "focusable"),
            (Self::EDITABLE, "editable"),
            (Self::HEIGHT_FOR_WIDTH, "height-for-width"),
            (Self::CONTAINER, "container"),
            (Self::CUSTOM_LAYOUT, "custom-layout"),
            (Self::PREPARE, "prepare"),
            (Self::VIRTUAL, "virtual"),
            (Self::HIT_TEST, "hit-test"),
        ]
        .into_iter()
        .filter(|(c, _)| self.contains(*c))
        .map(|(_, n)| n)
        .collect()
    }
}

impl std::ops::BitOr for Caps {
    type Output = Caps;
    fn bitor(self, o: Caps) -> Caps {
        self.union(o)
    }
}

// ---- registration (what the macro emits) ----

/// One `#[prop]` of a kind.
pub struct PropEntry {
    /// The JS name: the field name in kebab-case (`units_shown` → `units-shown`).
    pub name: &'static str,
    pub docs: &'static str,
    /// `#[prop(bind)]`: the widget writes it through [`WidgetCx::set`]; `bind:` observes it (`exact`).
    pub bind: bool,
    /// No `default =` and not an `Option`: the typings make it required.
    pub required: bool,
    /// `#[prop(measure)]`: a change measures the node again.
    pub measure: bool,
    /// `offset_of!` the field (how [`WidgetCx::set`] finds the prop a write is for).
    pub offset: usize,
}

/// One `#[event]` of a kind (`Event<T>`, `on<name>` in Svelte).
pub struct EventEntry {
    /// The JS event name: the field name in kebab-case.
    pub name: &'static str,
    pub docs: &'static str,
}

/// A provider kind, as `#[svelters::widget]` registers it.
pub struct KindEntry {
    /// `cad`
    pub namespace: &'static str,
    /// The struct name, `QuantityField`; the spec id is `<namespace>.<name>`.
    pub name: &'static str,
    /// `cad.QuantityField`
    pub id: &'static str,
    /// The element tag, `quantity-field`.
    pub tag: &'static str,
    /// The Rust path (`module_path!()::Name`).
    pub rust: &'static str,
    pub docs: &'static str,
    pub caps: Caps,
    pub props: &'static [PropEntry],
    pub events: &'static [EventEntry],
    /// A widget with every prop at its default.
    pub new: fn() -> Box<dyn ErasedWidget>,
    /// `None` for an element kind (`#[svelters::widget]`). A behaviour (`#[svelters::behavior]`,
    /// EV2b) attaches to an element of this kind (`"Widget"`): its `tag` is `<namespace>.<name>`,
    /// which no template can write, and Svelte uses it through `use:<name>` from
    /// `svelters:behaviors/<namespace>`.
    pub attaches_to: Option<&'static str>,
}

impl KindEntry {
    /// A behaviour's JS name (the export of `svelters:behaviors/<namespace>`), `None` for an element.
    pub fn behavior_name(&self) -> Option<&'static str> {
        self.attaches_to.map(|_| {
            self.tag
                .strip_prefix(self.namespace)
                .and_then(|t| t.strip_prefix('.'))
                .unwrap_or(self.tag)
        })
    }
}

/// What `inventory` collects (a reference to the kind's static entry).
pub struct KindRef(pub &'static KindEntry);

inventory::collect!(KindRef);

/// Implemented by `#[svelters::widget]`: the struct's entry and its typed prop table.
pub trait KindDecl: Sized + 'static {
    const KIND: &'static KindEntry;
    #[doc(hidden)]
    fn __svelters_new() -> Self;
    #[doc(hidden)]
    fn __svelters_set(&mut self, prop: usize, v: Value, cx: &CallCx<'_>) -> Result<(), NativeError>;
    #[doc(hidden)]
    fn __svelters_reset(&mut self, prop: usize);
    #[doc(hidden)]
    fn __svelters_get(&self, prop: usize, cx: &CallCx<'_>) -> Result<Value, NativeError>;
}

// ---- the widget trait ----

/// Space offered to [`Widget::measure`] along the width.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AvailableSpace {
    /// This width (the widget may wrap to fit it).
    Definite(f32),
    /// As narrow as it can be.
    MinContent,
    /// As wide as it likes (the no-wrap size).
    MaxContent,
}

impl AvailableSpace {
    /// The definite width, else `default`.
    pub fn width_or(self, default: f32) -> f32 {
        match self {
            AvailableSpace::Definite(w) => w,
            _ => default,
        }
    }
}

/// The behaviour of a provider kind (ARCH-EMBED §8.2, D3(a)).
pub trait Widget: 'static {
    /// Intrinsic size: width down, height up. Called before the pass when the node needs measuring
    /// (first layout, a `#[prop(measure)]` change, [`WidgetCx::request_measure`]).
    fn measure(&mut self, cx: &mut MeasureCx<'_>, avail: AvailableSpace) -> egui::Vec2;
    /// Paints and interacts inside `cx.rect()`. Never runs JS; writes go through `cx`.
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response;
    /// Fills role, name, value and states on the AccessKit node of the response's id (after `ui`).
    fn a11y(&self, _cx: &A11yCx<'_>, _node: &mut egui::accesskit::Node) {}
    /// True while the widget has work in flight (ARCH-EMBED R8: a provider bit in `Busy`).
    fn busy(&self) -> bool {
        false
    }
    /// What is at `local` (relative to the drawn rect's top left), for `SceneRef::hit_test`
    /// (ARCH-SCENE §7.4, R13). Asked only for kinds with `Caps::HIT_TEST`, from last pass's rect:
    /// `Hit::Part(key)` names a part only this kind interprets (a row key, a pick id), `Hit::Miss`
    /// lets the point through to what is below.
    fn hit_test(&self, _cx: &HitCx, _local: egui::Pos2) -> Hit {
        Hit::Node
    }
    /// SG4, `Caps::CUSTOM_LAYOUT` (ARCH-SCENE §7.3): lays out this node's children (its shadow
    /// children) itself; taffy lays out the rest of the tree. Called with taffy's inputs: in a
    /// `RunMode::ComputeSize` run only the size is wanted, in `PerformLayout` place each child with
    /// [`LayoutCx::place`]. Without the cap (or by default) the node's children get flexbox from
    /// its style. Unstable (§16): taffy's types.
    fn layout(&mut self, cx: &mut LayoutCx<'_>, inputs: crate::taffy::LayoutInput) -> crate::taffy::LayoutOutput {
        cx.default_layout(inputs)
    }
    /// SG4, `Caps::PREPARE` (ARCH-SCENE §7.2 step b, §8.2): runs before its root's layout every
    /// frame the root is drawn, with last frame's visible rect: build or rebind shadow children
    /// here (a virtualiser's rows) through [`PrepareCx::scene`].
    fn prepare(&mut self, _cx: &mut PrepareCx<'_>) {}
    /// ARCH-EMBED R11 (EV3): while focused, whether it takes `key` with `modifiers` itself. A
    /// claimed key is taken out of the pass's input after `ui` ran, a host keymap sees it as
    /// consumed (`shortcut_override`), and Enter or Escape then do not reach the scope's default
    /// or cancel button.
    fn consume_key(&self, _key: egui::Key, _modifiers: egui::Modifiers) -> bool {
        false
    }
    /// R11: an input method is composing in it (every key is its own, Enter included).
    fn composing(&self) -> bool {
        false
    }
}

pub use crate::layout::LayoutCx;
pub use pass::{PassCx, PassHandle, PassScope, PrepareCx, ProviderCx, ProviderHandle, ScenePass, StateProvider};

/// An `on<name>` event of a kind. The macro gives each `#[event]` field its index; [`WidgetCx::emit`]
/// delivers a payload to the JS listeners after the pass.
pub struct Event<T> {
    index: u16,
    _t: PhantomData<fn(T)>,
}

impl<T> Event<T> {
    #[doc(hidden)]
    pub const fn __new(index: u16) -> Self {
        Event { index, _t: PhantomData }
    }
    pub fn index(&self) -> usize {
        self.index as usize
    }
}

impl<T> std::fmt::Debug for Event<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Event#{}", self.index)
    }
}

// ---- contexts ----

/// What [`Widget::measure`] gets: egui's fonts and style.
pub struct MeasureCx<'a> {
    pub ctx: &'a egui::Context,
    pub style: &'a egui::Style,
}

impl MeasureCx<'_> {
    fn font(&self) -> egui::FontId {
        egui::TextStyle::Body.resolve(self.style)
    }

    /// The size of `text` in the body font, wrapped at `wrap` if given.
    pub fn text_size(&self, text: &str, wrap: Option<f32>) -> egui::Vec2 {
        let font = self.font();
        self.ctx.fonts_mut(|f| match wrap {
            Some(w) => f.layout(text.to_string(), font, egui::Color32::WHITE, w.max(0.0)).size(),
            None => f.layout_no_wrap(text.to_string(), font, egui::Color32::WHITE).size(),
        })
    }

    /// One line of body text.
    pub fn row_height(&self) -> f32 {
        let font = self.font();
        self.ctx.fonts_mut(|f| f.row_height(&font))
    }

    /// `ui.spacing().interact_size`.
    pub fn interact_size(&self) -> egui::Vec2 {
        self.style.spacing.interact_size
    }

    /// A single-line text field `width` wide (`TextEdit`'s margin is (4, 2)).
    pub fn text_field_size(&self, _text: &str, width: f32) -> egui::Vec2 {
        egui::vec2(width, (self.row_height() + 4.0).max(self.interact_size().y))
    }
}

/// What [`Widget::ui`] gets: the node's id, rect and state, read access to the whole scene, the
/// node's children to draw, and the queued writes back to the scene and JS.
///
/// SG4 (ARCH-SCENE §7.3): the walk takes the widget's payload out of the scene while `ui` runs,
/// so the cx can hand out the rest of the scene ([`WidgetCx::scene`]) while the widget has
/// `&mut self`, with no `unsafe`. Writes are limited to the node itself (state bits, ext data,
/// `set`, events): everything else goes through `SceneHandle::defer` (S9).
pub struct WidgetCx<'a> {
    s: &'a mut crate::scene::Scene,
    i: usize,
    id: egui::Id,
    rect: egui::Rect,
    enabled: bool,
    state: StateBits,
    node: crate::scene::NodeId,
    entry: &'static KindEntry,
    base: usize,
    /// The walk's origin (root-relative rects + origin = screen) and outbox, for `children`.
    origin: egui::Vec2,
    out: &'a mut Vec<Out>,
    changed: Vec<usize>,
    emits: Vec<(usize, Value)>,
    remeasure: bool,
    errors: Vec<String>,
}

impl WidgetCx<'_> {
    /// The node's egui id (use it for the widget's own id: focus and memory follow the node).
    pub fn id(&self) -> egui::Id {
        self.id
    }
    /// The rect layout gave the node, in screen coordinates.
    pub fn rect(&self) -> egui::Rect {
        self.rect
    }
    pub fn enabled(&self) -> bool {
        self.enabled
    }
    /// Whether every bit of `bits` is set on the node.
    pub fn state(&self, bits: impl IntoStateBits) -> bool {
        self.state.contains(bits.state_bits())
    }
    pub fn set_state(&mut self, bits: impl IntoStateBits, on: bool) {
        self.state.set(bits.state_bits(), on);
    }

    /// SG4: the whole scene, read-only, while this widget is borrowed. This node's own payload is
    /// out while `ui` runs: through the scene it reads as an empty box (`NodeRef::widget` is `None`);
    /// its props, state (as of the start of the pass), ext data and children read normally.
    pub fn scene(&self) -> crate::scene::SceneRef<'_> {
        crate::scene::SceneRef::new(self.s)
    }
    /// SG4: this node (see [`WidgetCx::scene`]).
    pub fn node(&self) -> crate::scene::NodeRef<'_> {
        crate::scene::NodeRef::at(self.s, self.node)
    }
    /// SG4: the node's children (its shadow children, ARCH-SCENE §4.4), in layout order.
    pub fn child_ids(&self) -> &[crate::scene::NodeId] {
        &self.s.children[self.i]
    }
    /// SG4: draws every child of this node at the rect layout gave it, in order (a container
    /// provider's content). Their responses go to the outbox and the hit list like any node's.
    pub fn children(&mut self, ui: &mut egui::Ui) {
        for k in 0..self.s.children[self.i].len() {
            let c = self.s.children[self.i][k].index() as usize;
            crate::walk::paint_child(self.s, c, self.origin, ui, self.out);
        }
    }
    /// SG4: draws one child of this node (a row of a pool); false if `id` is not one of them.
    pub fn child(&mut self, ui: &mut egui::Ui, id: crate::scene::NodeId) -> bool {
        match self.s.idx(id) {
            Some(c) if self.s.core[c].parent == Some(self.node) => {
                crate::walk::paint_child(self.s, c, self.origin, ui, self.out);
                true
            }
            _ => false,
        }
    }

    /// The exact write of a prop field (ARCH-EMBED §8.2, SV10): stores `v` and, if it differs,
    /// announces `prop_changed` after the pass to an observer (`bind:`). Only `#[prop]` fields of the
    /// widget being drawn are props; a write to any other field is stored and not announced.
    pub fn set<T: PartialEq>(&mut self, field: &mut T, v: T) {
        if *field == v {
            return;
        }
        *field = v;
        if let Some(i) = written_prop(self.entry, self.base, field, &mut self.changed, &mut self.errors)
            && self.entry.props[i].measure
        {
            self.remeasure = true;
        }
    }

    /// Queues `payload` for the event's JS listeners (delivered after the pass, in order).
    pub fn emit<T: IntoJs>(&mut self, event: &Event<T>, payload: T) {
        queue_emit(self.entry, event, payload, &mut self.emits, &mut self.errors);
    }

    /// The node's provider data for `key` (ARCH-EMBED §8.4).
    pub fn ext<T: 'static>(&self, key: &ExtKey<T>) -> Option<&T> {
        self.s.ext.get(key, self.node.index())
    }
    /// The node's entry for `key`; writes are journaled with `Origin::Widget`.
    pub fn ext_mut<'b, T: 'static>(&'b mut self, key: &'b ExtKey<T>) -> ExtEntry<'b, T> {
        ExtEntry::new(self.s.ext_cx(), key, self.node, crate::scene::Origin::Widget)
    }

    /// Measures the node again before the next layout (its content size changed).
    pub fn request_measure(&mut self) {
        self.remeasure = true;
    }
}

/// The prop a `cx.set` wrote (by the field's offset in the struct at `base`), recorded once in
/// `changed`; an error for a field that is not a `#[prop]`.
fn written_prop<T>(
    entry: &KindEntry,
    base: usize,
    field: &mut T,
    changed: &mut Vec<usize>,
    errors: &mut Vec<String>,
) -> Option<usize> {
    let off = (field as *mut T as usize).wrapping_sub(base);
    match entry.props.iter().position(|p| p.offset == off) {
        Some(i) => {
            if !changed.contains(&i) {
                changed.push(i);
            }
            Some(i)
        }
        None => {
            errors.push(format!(
                "<{}>: cx.set on a field that is not a #[prop]",
                entry.tag
            ));
            None
        }
    }
}

/// Converts an event payload and queues it.
fn queue_emit<T: IntoJs>(
    entry: &KindEntry,
    event: &Event<T>,
    payload: T,
    emits: &mut Vec<(usize, Value)>,
    errors: &mut Vec<String>,
) {
    match svelters_host::exports::__private::with_cx(entry.namespace, |cx| payload.into_js(cx)) {
        Ok(v) => emits.push((event.index(), v)),
        Err(e) => errors.push(format!(
            "<{}> {}: payload: {}",
            entry.tag,
            entry.events.get(event.index()).map_or("?", |e| e.name),
            e.message
        )),
    }
}

/// What [`Widget::a11y`] gets.
pub struct A11yCx<'a> {
    pub id: egui::Id,
    pub state: StateBits,
    /// The node's `accessible-name`, if set.
    pub name: Option<&'a str>,
}

// ---- the erased widget the scene stores ----

/// A provider widget behind a pointer (`W::Ext`): the [`Widget`] plus its generated prop table.
pub trait ErasedWidget: Any {
    fn entry(&self) -> &'static KindEntry;
    fn set_prop(&mut self, prop: usize, v: Value, cx: &CallCx<'_>) -> Result<(), NativeError>;
    fn reset_prop(&mut self, prop: usize);
    fn get_prop(&self, prop: usize, cx: &CallCx<'_>) -> Result<Value, NativeError>;
    fn measure(&mut self, cx: &mut MeasureCx<'_>, avail: AvailableSpace) -> egui::Vec2;
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response;
    fn a11y(&self, cx: &A11yCx<'_>, node: &mut egui::accesskit::Node);
    fn busy(&self) -> bool;
    fn hit_test(&self, cx: &HitCx, local: egui::Pos2) -> Hit;
    fn layout(&mut self, cx: &mut LayoutCx<'_>, inputs: crate::taffy::LayoutInput) -> crate::taffy::LayoutOutput;
    fn prepare(&mut self, cx: &mut PrepareCx<'_>);
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    /// The struct's address (field offsets are relative to it).
    fn base(&self) -> usize;
    /// The behaviour, for an attached kind (EV2b; `None` for a widget).
    fn behavior(&mut self) -> Option<&mut dyn Behavior> {
        None
    }
    fn behavior_ref(&self) -> Option<&dyn Behavior> {
        None
    }
    /// EV3: [`Widget::consume_key`].
    fn consume_key(&self, _key: egui::Key, _modifiers: egui::Modifiers) -> bool {
        false
    }
    /// EV3: [`Widget::composing`].
    fn composing(&self) -> bool {
        false
    }
}

impl<T: Widget + KindDecl> ErasedWidget for T {
    fn entry(&self) -> &'static KindEntry {
        T::KIND
    }
    fn set_prop(&mut self, prop: usize, v: Value, cx: &CallCx<'_>) -> Result<(), NativeError> {
        self.__svelters_set(prop, v, cx)
    }
    fn reset_prop(&mut self, prop: usize) {
        self.__svelters_reset(prop)
    }
    fn get_prop(&self, prop: usize, cx: &CallCx<'_>) -> Result<Value, NativeError> {
        self.__svelters_get(prop, cx)
    }
    fn measure(&mut self, cx: &mut MeasureCx<'_>, avail: AvailableSpace) -> egui::Vec2 {
        Widget::measure(self, cx, avail)
    }
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        Widget::ui(self, ui, cx)
    }
    fn a11y(&self, cx: &A11yCx<'_>, node: &mut egui::accesskit::Node) {
        Widget::a11y(self, cx, node)
    }
    fn busy(&self) -> bool {
        Widget::busy(self)
    }
    fn hit_test(&self, cx: &HitCx, local: egui::Pos2) -> Hit {
        Widget::hit_test(self, cx, local)
    }
    fn layout(&mut self, cx: &mut LayoutCx<'_>, inputs: crate::taffy::LayoutInput) -> crate::taffy::LayoutOutput {
        Widget::layout(self, cx, inputs)
    }
    fn prepare(&mut self, cx: &mut PrepareCx<'_>) {
        Widget::prepare(self, cx)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn base(&self) -> usize {
        self as *const T as usize
    }
    fn consume_key(&self, key: egui::Key, modifiers: egui::Modifiers) -> bool {
        Widget::consume_key(self, key, modifiers)
    }
    fn composing(&self) -> bool {
        Widget::composing(self)
    }
}

/// The payload of a provider node (`W::Ext`). A `RefCell` so layout can measure through `&W`.
pub struct ExtWidget(pub RefCell<Box<dyn ErasedWidget>>);

impl ExtWidget {
    pub fn new(w: Box<dyn ErasedWidget>) -> ExtWidget {
        ExtWidget(RefCell::new(w))
    }
    pub fn entry(&self) -> &'static KindEntry {
        self.0.borrow().entry()
    }
    /// The widget, typed (full access to a kind's own state; ARCH-SCENE §6.1 `NodeRef::widget`).
    /// `None` also while the widget is being drawn (or a behaviour's hook runs on it).
    pub fn with<W: 'static, R>(&self, f: impl FnOnce(&W) -> R) -> Option<R> {
        let b = self.0.try_borrow().ok()?;
        b.as_any().downcast_ref::<W>().map(f)
    }
    pub fn with_mut<W: 'static, R>(&self, f: impl FnOnce(&mut W) -> R) -> Option<R> {
        let mut b = self.0.try_borrow_mut().ok()?;
        b.as_any_mut().downcast_mut::<W>().map(f)
    }
    pub(crate) fn measure(&self, m: &crate::layout::Metrics, avail: AvailableSpace) -> egui::Vec2 {
        let mut cx = MeasureCx { ctx: &m.ctx, style: &m.style };
        self.0.borrow_mut().measure(&mut cx, avail)
    }
}

impl std::fmt::Debug for ExtWidget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ExtWidget(<{}>)", self.entry().tag)
    }
}

impl Clone for ExtWidget {
    /// A fresh widget of the same kind with default props (provider state is not cloneable).
    fn clone(&self) -> ExtWidget {
        ExtWidget::new((self.entry().new)())
    }
}

impl PartialEq for ExtWidget {
    /// Identity: two payloads are equal only if they are the same widget.
    fn eq(&self, o: &ExtWidget) -> bool {
        std::ptr::eq(self, o)
    }
}

impl Node<'_> {
    /// The provider widget of this node, typed, if it is one of kind `W`.
    pub fn widget<Wd: 'static, R>(&self, f: impl FnOnce(&Wd) -> R) -> Option<R> {
        match self.w {
            W::Ext(x) => x.with(f),
            _ => None,
        }
    }
}

// ---- the scene's calls (props, paint) ----

/// `set_prop` of prop `i` of a provider node's widget `w` (the host already validated `v` against
/// the prop's slot).
pub(crate) fn set_prop(w: &W, i: usize, v: &Value) -> Result<crate::scene::Dirty, String> {
    let W::Ext(x) = w else { return Err("not a provider node".into()) };
    let mut w = x.0.borrow_mut();
    let entry = w.entry();
    let p = entry.props.get(i).ok_or_else(|| format!("no prop #{i}"))?;
    if matches!(v, Value::Null) && !nullable(entry, i) {
        w.reset_prop(i);
    } else {
        svelters_host::exports::__private::with_cx(entry.namespace, |cx| w.set_prop(i, v.clone(), cx)).map_err(|e| e.message)?;
    }
    Ok(if p.measure { crate::scene::Dirty::MEASURE } else { crate::scene::Dirty::EMPTY })
}

fn nullable(entry: &KindEntry, i: usize) -> bool {
    crate::kinds::KindRegistry::global().provider_type(entry.id, i).is_some_and(|t| t.slot.has(svelters_abi::SVL_D_NULLABLE))
}

pub(crate) fn get_prop(n: &Node, i: usize) -> Result<Value, String> {
    let W::Ext(x) = n.w else { return Err("not a provider node".into()) };
    let w = x.0.borrow();
    svelters_host::exports::__private::with_cx(w.entry().namespace, |cx| w.get_prop(i, cx)).map_err(|e| e.message)
}

/// Runs `f` on the provider widget of node `i` with its payload taken out of the scene (SG4,
/// ARCH-SCENE §7.3), so `f` may also have the scene. The payload goes back afterwards. If `f`
/// panics, the widget is dropped and a fresh one of the same kind (props at their defaults) takes
/// its place, so the scene stays whole while the panic unwinds to the walk's `catch_unwind`, which
/// faults the root (ARCH-EMBED §6.2). `None` if node `i` holds no provider widget (or it is out).
pub(crate) fn with_taken<R>(s: &mut crate::scene::Scene, i: usize, f: impl FnOnce(&mut dyn ErasedWidget, &mut crate::scene::Scene) -> R) -> Option<R> {
    use crate::scene::node::Payload;
    if !matches!(s.payload[i], Payload::Builtin(W::Ext(_))) {
        return None;
    }
    let payload = std::mem::replace(&mut s.payload[i], Payload::Vacant);
    let Payload::Builtin(W::Ext(x)) = &payload else { unreachable!("checked above") };
    let entry = x.entry();
    let in_batch = s.in_batch;
    let r = {
        let mut wb = x.0.borrow_mut();
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&mut **wb, s)))
    };
    match r {
        Ok(r) => {
            s.payload[i] = payload;
            Some(r)
        }
        Err(p) => {
            if !in_batch {
                s.abort_batch();
            }
            s.payload[i] = Payload::Builtin(W::Ext(ExtWidget::new((entry.new)())));
            s.counters.provider_panics += 1;
            drop(payload);
            std::panic::resume_unwind(p)
        }
    }
}

/// Draws the provider node in slot `i` (the walk's index into the scene's columns): its `ui` in `ui`
/// (a child `Ui` at the node's rect) with the payload taken out (SG4), then the a11y hook, then its
/// writes as outbox entries (`prop_changed` for observed props, events for listeners). `origin` is
/// the walk's (for `WidgetCx::children`). Returns the widget's response (the host response of its
/// behaviours, EV2b).
pub(crate) fn paint(arena: &mut crate::scene::Arena, i: usize, ui: &mut egui::Ui, origin: egui::Vec2, out: &mut Vec<Out>) -> Option<egui::Response> {
    use crate::scene::NodeFlags;
    let node = arena.slots.id_at(i).expect("live provider node");
    let h: Handle = node.to_raw();
    let rect = ui.max_rect();
    let enabled = arena.common(i).enabled;
    let id = arena.core[i].egui_id;
    let state0 = arena.core[i].state;
    let ran = with_taken(arena, i, |w, s| {
        let mut cx = WidgetCx {
            s,
            i,
            id,
            rect,
            enabled,
            state: state0,
            node,
            entry: w.entry(),
            base: w.base(),
            origin,
            out: &mut *out,
            changed: Vec::new(),
            emits: Vec::new(),
            remeasure: false,
            errors: Vec::new(),
        };
        let resp = w.ui(ui, &mut cx);
        let WidgetCx { state, changed, emits, remeasure, errors, .. } = cx;
        (resp, state, changed, emits, remeasure, errors)
    });
    let Some((mut resp, state, changed, emits, remeasure, errors)) = ran else { return None };
    // EM1: the accessible name and, for an anchored node, the anchor as AccessKit author id
    resp = crate::a11y::decorate(resp, &arena.n(h));
    let c = arena.common(i);
    if !c.tooltip.is_empty() {
        resp = resp.on_hover_text(c.tooltip.clone());
    }
    {
        let name = (!c.accessible_name.is_empty()).then_some(c.accessible_name.as_str());
        let acx = A11yCx { id: resp.id, state, name };
        if let crate::scene::node::Payload::Builtin(W::Ext(x)) = &arena.payload[i] {
            let w = x.0.borrow();
            ui.ctx().accesskit_node_builder(resp.id, |n| w.a11y(&acx, n));
        }
    }
    if state != arena.core[i].state {
        let before = arena.core[i].state;
        arena.core[i].state = state;
        let t = arena.stamp();
        arena.core[i].changed = t;
        arena.record_state(i, before, crate::scene::Origin::Widget);
    }
    let core = &mut arena.core[i];
    if core.flags.contains(NodeFlags::WANT_FOCUS) {
        core.flags.remove(NodeFlags::WANT_FOCUS);
        resp.request_focus();
    }
    core.drawn = resp.rect;
    flush_writes(arena, i, changed, emits, errors, out);
    if remeasure {
        arena.layout[i].intrinsic = None;
        arena.invalidate_content(node);
        ui.ctx().request_repaint();
    }
    Some(resp)
}

/// The writes a provider node (widget or behaviour) in slot `i` made during the pass: `cx.set`
/// props are journaled and announced to observers (`prop_changed`), events go to listeners.
fn flush_writes(
    arena: &mut crate::scene::Arena,
    i: usize,
    changed: Vec<usize>,
    emits: Vec<(usize, Value)>,
    errors: Vec<String>,
    out: &mut Vec<Out>,
) {
    let node = arena.slots.id_at(i).expect("live provider node");
    let h: Handle = node.to_raw();
    let kind = arena.core[i].kind as usize;
    let reg = crate::kinds::KindRegistry::global();
    if !changed.is_empty() {
        let t = arena.stamp();
        arena.core[i].changed = t;
    }
    for p in changed {
        let prop = reg.provider_prop_id(kind, p);
        arena.record(crate::scene::Change::Prop { node, prop: crate::scene::PropId(prop), origin: crate::scene::Origin::Widget }, crate::scene::Origin::Widget);
        if arena.observes(i, prop) {
            out.push(Out::Changed(h, prop));
        }
    }
    for (e, v) in emits {
        let sig = reg.provider_signal_id(kind, e);
        if arena.listens(i, sig) {
            out.push(Out::Event(h, sig, vec![v]));
        }
    }
    for e in errors {
        eprintln!("svelters-egui: {e}");
    }
}
