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
//! - `#[svelters::widget] impl T { pub fn … }` (RA3) gives a kind methods JS calls on the element
//!   (`methods.rs`, [`MethodCx`]).
//!
//! Files: this one holds the trait, the contexts and the registration types; `state.rs` the state
//! bits; `methods.rs` the kind methods (RA3); `behavior.rs` the behaviours (EVENTS, EV2b); `pass.rs` (SG4: `StateProvider`, `ScenePass`,
//! `PrepareCx`) is SCENE's. Writer lane: RUSTAPI (RA1, RA3).

pub(crate) mod behavior;
pub(crate) mod methods;
pub(crate) mod pass;
mod state;

pub use behavior::{__new_behavior, Behavior, BehaviorBox, BehaviorCx};
pub use methods::{__kind_of, __this, KindMethods, MethodCx, MethodEntry, types_key as methods_types_key};

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
    /// OW0 (ARCH-WIDGETS §1): draws itself entirely with the paint primitives (`crate::paint`):
    /// the walk paints no box behind it, and its behaviour units own `hovered`, `pressed`,
    /// `focused` and `focus-visible` (the walk sets only the prop-derived bits).
    pub const PAINTS_BOX: Caps = Caps(1 << 3);
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
            (Self::PAINTS_BOX, "paints-box"),
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

/// OW0: the kind-table type of a prop or event payload (from the Rust type; `#[prop(enumeration =
/// "a|b")]` for an enum). An own widget's prop that its built-in row lacks is added to the built-in
/// kind with this type (`KindRegistry::replacement`); provider kinds describe theirs through the
/// export schema instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowTy {
    Bool,
    I64,
    F64,
    Str,
    Enum(&'static [&'static str]),
    /// Not a kind-table type (records, lists): not allowed as an extra prop of a built-in kind.
    Other,
}

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
    /// OW0: its kind-table type and nullability (an `Option`).
    pub row_ty: RowTy,
    pub nullable: bool,
}

/// One `#[event]` of a kind (`Event<T>`, `on<name>` in Svelte).
pub struct EventEntry {
    /// The JS event name: the field name in kebab-case.
    pub name: &'static str,
    pub docs: &'static str,
    /// OW0: the payload's kind-table type (`None` for `Event<()>`).
    pub arg: Option<RowTy>,
}

/// One `#[part]` of a kind (ARCH-WIDGETS §1.3): `::part(name)` in CSS.
#[derive(Debug)]
pub struct PartEntry {
    /// The part name, the field name in kebab-case.
    pub name: &'static str,
    pub docs: &'static str,
    /// `#[part(node)]`: a shadow child with its own rect, states and hit-list entry; otherwise a
    /// paint part (styled as a pseudo-element of the host, no node).
    pub node: bool,
}

/// A `#[part]` field of an own widget (ARCH-WIDGETS §1.3). A paint part is only a name (the
/// widget paints it with the part's style); a node part holds its shadow child once created
/// (OW1b and later create them through `insert_shadow` in `prepare`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Part {
    pub node: Option<crate::scene::NodeId>,
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
    /// OW0 (ARCH-WIDGETS §6.2): the built-in tag this own widget implements (crate-private to
    /// svelters-egui). Not an element of its own: the built-in row (tag, prop and signal ids)
    /// stays, and the own-widget switch (`crate::widgets`) picks this struct for new nodes.
    pub replaces: Option<&'static str>,
    /// OW0 (WIDGET-PARITY §4.3): the built-in tag whose type selectors and part names also style
    /// this kind (`style_as = "slider"` on `cad-slider`): the theme styles it as that built-in and
    /// the app's CSS adds only its differences.
    pub style_as: Option<&'static str>,
    /// Its `#[part]`s, in declaration order.
    pub parts: &'static [PartEntry],
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

/// CX3 (CADATOMIC.md SV3): a draft bind source a provider crate declares, as data. It goes into the
/// spec fragment (`bind_sources`), and the compiler lowers `bind:prop={export(...args)}` (with
/// `export` imported from `module`) to a binding whose value is `get(...args)` and whose changes, or
/// commits with `|commit`, call `set(...args, value)`; `get` and `set` are exports of `module`
/// (Rust `#[svelters::export]`s or a JS module). For the node to follow the source, `get` must read
/// reactive state (a store getter); a plain function export is read when the binding's effect runs.
///
/// ```ignore
/// inventory::submit! { svelters_egui::provider::BindSourceDef::draft("svelters:app/task", "slot", "draftValue", "setDraft") }
/// ```
pub struct BindSourceDef {
    pub module: &'static str,
    pub export: &'static str,
    pub get: &'static str,
    pub set: &'static str,
    pub doc: &'static str,
}

impl BindSourceDef {
    pub const fn draft(module: &'static str, export: &'static str, get: &'static str, set: &'static str) -> BindSourceDef {
        BindSourceDef { module, export, get, set, doc: "" }
    }

    pub const fn doc(self, doc: &'static str) -> BindSourceDef {
        BindSourceDef { doc, ..self }
    }
}

inventory::collect!(BindSourceDef);

/// Implemented by `#[svelters::widget]`: the struct's entry and its typed prop table.
pub trait KindDecl: Sized + 'static {
    const KIND: &'static KindEntry;
    /// The kind's namespace (`KindEntry::namespace`), as a constant for generated statics.
    #[doc(hidden)]
    const __NAMESPACE: &'static str;
    /// RA3: the key the types of the kind's methods are registered under (`methods_types_key`).
    #[doc(hidden)]
    const __METHODS_KEY: &'static str;
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
    /// `Caps::VIRTUAL` (ARCH-SCENE §8.2, SG5): painted rows exposed to a11y, anchors
    /// (`SceneRef::target_by_path`), queries (`Query::include_virtual`) and `target_rect`. Not asked
    /// while the widget draws itself.
    fn virtual_children(&self) -> Option<&dyn crate::scene::VirtualChildren> {
        None
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
    /// RA3 (ARCH-WIDGETS TextInput unit): `Some(multiline)` when the widget is a text input whose
    /// egui text field has the node's id (`cx.id()`). EV3's edit commands (the Edit menu's Cut,
    /// Copy, Paste, Undo, Select All: `Embed::text_command`) then reach it as the input events they
    /// stand for, fed just before its `ui`, and it gets the text-field key rules (R11). `None` (the
    /// default): not a text input.
    fn text_input(&self) -> Option<bool> {
        None
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
    /// RA3: an id for the widget's measuring (stable per node, apart from its drawing ids).
    pub id: egui::Id,
    /// OW0: the node's computed style (CSS), if the cascade reaches it.
    pub(crate) css: Option<&'a svelters_style::ComputedStyle>,
    /// OW0: its parts' computed styles (`::part(name)`).
    pub(crate) parts: &'a [(Box<str>, std::sync::Arc<svelters_style::ComputedStyle>)],
}

impl MeasureCx<'_> {
    /// OW0: the node's style over the widget's UA table `ua`, at rest (what own widgets measure
    /// with: font, padding, border).
    pub fn style(&self, ua: crate::paint::Ua) -> crate::paint::StyleView<'_> {
        crate::paint::StyleView::new(self.css, ua, StateBits::NONE, self.ctx.theme() == egui::Theme::Dark)
    }

    /// OW0: the computed style of part `name` (`::part(name)`), if a rule reaches it.
    pub fn part_css(&self, name: &str) -> Option<&svelters_style::ComputedStyle> {
        self.parts.iter().find(|(n, _)| &**n == name).map(|(_, c)| &**c)
    }

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

    /// RA3: the size `add` takes when it runs in an invisible, disabled sizing-pass `Ui` `avail`
    /// wide (egui's own way to size a widget before showing it): what the fn form and
    /// `wrap_widget!` measure with, so the measured size is the drawn one. Nothing is painted and
    /// no input is sensed; the `Ui`'s ids derive from [`MeasureCx::id`].
    pub fn sizing_pass(&self, avail: AvailableSpace, add: impl FnOnce(&mut egui::Ui)) -> egui::Vec2 {
        let width = match avail {
            AvailableSpace::Definite(w) => w,
            AvailableSpace::MinContent => 0.0,
            AvailableSpace::MaxContent => 16384.0,
        };
        let max = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 16384.0));
        let builder = egui::UiBuilder::new().max_rect(max).sizing_pass().invisible().layout(egui::Layout::left_to_right(egui::Align::Min));
        let mut ui = egui::Ui::new(self.ctx.clone(), self.id, builder);
        ui.set_style(std::sync::Arc::new(self.style.clone()));
        add(&mut ui);
        ui.min_rect().size()
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
    /// OW0: UA tokens resolve in the dark scheme.
    dark: bool,
    /// EV1: the pass's context (repaints asked by behaviours' filters during `dispatch`).
    ctx: egui::Context,
}

/// What [`WidgetCx::dispatch`] reports (OW0's name for EV1's [`Outcome`](crate::events::Outcome)):
/// `prevented` skips the widget's default action, `stopped` means a filter stopped propagation.
pub type Dispatched = crate::events::Outcome;

impl WidgetCx<'_> {
    // ---- OW0: own widgets (ARCH-WIDGETS §1.3, §2.3) ----

    /// `ui.interact` at the node's rect with its id and `sense` (the units' merged sense): egui's
    /// picking, capture and focus memory under the units (Q2).
    pub fn interact(&self, ui: &mut egui::Ui, sense: egui::Sense) -> egui::Response {
        ui.interact(self.rect, self.id, sense)
    }

    /// Runs behaviour units on the node's response (`#[svelters::widget]`'s generated
    /// `run_units` calls it): `f` runs each unit's `input` with the node's state bits, focus
    /// requests are applied after, then [`WidgetCx::commit_state`] restyles in the same frame.
    pub fn run_units(&mut self, ui: &egui::Ui, resp: &egui::Response, f: impl FnOnce(&mut crate::interact::InputCx<'_>)) -> crate::interact::FocusCx {
        let parts = crate::interact::PartResponses::default();
        // the prop-mirror bit the units' rules depend on, also when no sheet runs the cascade
        self.state.set(StateBits::DISABLED, !self.enabled);
        let fc = crate::interact::run(ui, resp, &mut self.state, &parts, f);
        self.commit_state();
        fc
    }

    /// Writes the bits set so far to the node now (journaled) and restyles what the change
    /// reaches, so this pass paints the new state (`:hover`, `:active`, `:focus-visible` never
    /// lag; §2.3). Also moves `:focus-within` with `focused`.
    pub fn commit_state(&mut self) {
        crate::style::set_own_state(self.s, self.i, self.state);
    }

    /// OW0: the computed style of the node's part `name` (`::part(name)`), if a rule reaches it;
    /// build its view with `cx.style(UA).part(cx.part_css("label"), LABEL_UA)`.
    pub fn part_css(&self, name: &str) -> Option<&svelters_style::ComputedStyle> {
        self.s.style.part_computed(self.node, name).map(|c| &**c)
    }

    /// The node's style over `ua` in its current state, with its running CSS transitions: what
    /// the paint primitives read.
    pub fn style(&self, ua: crate::paint::Ua) -> crate::paint::StyleView<'_> {
        let css = self.s.style.computed(self.node).map(|c| &**c);
        crate::paint::StyleView::new(css, ua, self.state, self.dark).with_overlay(self.s.style.overlay(self.node))
    }

    /// OW-CSS: part `name`'s view under `host` (this node's [`WidgetCx::style`]): its
    /// `::part(name)` style over `ua`, with the part's running transitions, inheriting from the host.
    pub fn part<'v>(&'v self, host: &'v crate::paint::StyleView<'v>, name: &str, ua: crate::paint::Ua) -> crate::paint::StyleView<'v> {
        host.part(self.part_css(name), ua).with_overlay(self.s.style.part_overlay(self.node, name))
    }

    /// A paint context on `ui`'s painter for this node (traced when the pass traces).
    pub fn paint_cx<'p>(&self, ui: &'p egui::Ui) -> crate::paint::PaintCx<'p> {
        let mut p = crate::paint::PaintCx::for_node(ui.painter(), self.node.to_raw());
        p.tag = self.entry.replaces.unwrap_or(self.entry.tag);
        p
    }

    /// Paints the node's box (shadows, background, border) at its rect; returns its geometry.
    pub fn paint_box(&self, ui: &egui::Ui, ua: crate::paint::Ua) -> crate::paint::BoxGeom {
        let s = self.style(ua);
        crate::paint::paint_box(&mut self.paint_cx(ui), &s, self.rect)
    }

    /// Paints the node's outline (the focus ring from `:focus-visible` rules), last.
    pub fn paint_outline(&self, ui: &egui::Ui, ua: crate::paint::Ua) {
        let s = self.style(ua);
        crate::paint::paint_outline(&mut self.paint_cx(ui), &s, self.rect);
    }

    /// Paints `text` in `content` under the node's style (`weight`: a `font-weight` prop).
    pub fn paint_text(&self, ui: &egui::Ui, ua: crate::paint::Ua, text: &str, content: egui::Rect, weight: Option<i64>, align: crate::paint::Align) {
        let s = self.style(ua).with_font_weight(weight);
        let g = crate::paint::text_galley(ui.ctx(), &s, text, None);
        crate::paint::paint_text(&mut self.paint_cx(ui), &s, &g, content, crate::paint::Align::of(&s, align));
    }

    /// OW1a: whether a runtime listens to `event` on this node (a label senses clicks only then).
    pub fn listens<T>(&self, event: &Event<T>) -> bool {
        let kind = self.s.core[self.i].kind as usize;
        let sig = crate::kinds::KindRegistry::global().ext_signal_id(kind, event.index());
        self.s.listens(self.i, sig)
    }

    /// The node's egui id (use it for the widget's own id: focus and memory follow the node).
    pub fn id(&self) -> egui::Id {
        self.id
    }
    /// The rect layout gave the node, in screen coordinates.
    pub fn rect(&self) -> egui::Rect {
        self.rect
    }
    /// SG6 (ARCH-EMBED §8.2 D3, ARCH-SCENE §8.3): the opt-in paint cache. `paint` draws the
    /// widget's shapes into `shapes` for a rect of the node's size at the origin (it may use the
    /// painter for text layout); they are kept while `version`, the size, `pixels_per_point` and
    /// the theme stay the same, and replayed at the node's rect with `painter`. Bump `version`
    /// when what the widget draws changes; a prop write drops the entry. Saves shape building and
    /// text layout, not egui's tessellation. True if the cached shapes were used.
    pub fn cached_shapes(&mut self, painter: &egui::Painter, version: u64, paint: impl FnOnce(&egui::Painter, egui::Rect, &mut Vec<egui::Shape>)) -> bool {
        self.s.paint_cache.paint(self.i as u32, version, painter, self.rect, paint)
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

    /// EV1 (ARCH-WIDGETS §1.2, §2.3): dispatches a user-visible event of this widget (a unit's
    /// intent: a press, a toggle, a selection change) along the node's path through the scene's
    /// Rust filters (SV6: registered [`EventFilter`](crate::events::EventFilter)s and the
    /// behaviours' [`Behavior::filter`], capture, target, bubble) **before** the default action,
    /// then queues `payload` for the event's JS listener (as [`WidgetCx::emit`], delivered after
    /// the pass) unless a filter stopped it. Apply the default action only when the outcome is not
    /// [`prevented`](crate::events::Outcome::prevented):
    ///
    /// ```ignore
    /// if !cx.dispatch(&self.clicked, ()).prevented() { self.activate(cx) }
    /// ```
    pub fn dispatch<T: IntoJs>(&mut self, event: &Event<T>, payload: T) -> Dispatched {
        let name = self.entry.events.get(event.index()).map_or("?", |e| e.name);
        let (o, runs) = crate::events::dispatch_signal(self.s, self.node, name);
        if !runs.is_empty() {
            // behaviours on this node may change its bits: keep them in this cx's copy
            let before = self.s.core[self.i].state;
            for r in runs {
                behavior::apply(self.s, r, Some(&self.ctx), self.out);
            }
            let after = self.s.core[self.i].state;
            let changed = before.0 ^ after.0;
            self.state = StateBits((self.state.0 & !changed) | (after.0 & changed));
        }
        if !o.stopped {
            self.emit(event, payload);
        }
        o
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
    fn virtual_children(&self) -> Option<&dyn crate::scene::VirtualChildren>;
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
    /// RA3: [`Widget::text_input`].
    fn text_input(&self) -> Option<bool> {
        None
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
    fn virtual_children(&self) -> Option<&dyn crate::scene::VirtualChildren> {
        Widget::virtual_children(self)
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
    fn text_input(&self) -> Option<bool> {
        Widget::text_input(self)
    }
}

/// The payload of a provider node (`W::Ext`). A `RefCell` so layout can measure through `&W`.
pub struct ExtWidget(pub RefCell<Box<dyn ErasedWidget>>, String);

impl ExtWidget {
    pub fn new(w: Box<dyn ErasedWidget>) -> ExtWidget {
        ExtWidget(RefCell::new(w), String::new())
    }
    /// OW0: the text an own widget replacing a built-in kind shows (its `text` prop, mirrored when
    /// set so `Node::text`, queries and a11y read it without borrowing the widget); empty for
    /// provider kinds.
    pub fn text(&self) -> &str {
        &self.1
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
    pub(crate) fn measure(&self, m: &crate::layout::Metrics, avail: AvailableSpace, css: Option<&svelters_style::ComputedStyle>, parts: &[(Box<str>, std::sync::Arc<svelters_style::ComputedStyle>)]) -> egui::Vec2 {
        let mut w = self.0.borrow_mut();
        let id = egui::Id::unique(("svelters-egui.measure", w.base()));
        let mut cx = MeasureCx { ctx: &m.ctx, style: &m.style, id, css, parts };
        w.measure(&mut cx, avail)
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

/// OW0: a write of built-in prop `def` to an own widget that replaces the kind (§6.2): routed by
/// name to the struct's prop (`Null` resets to the row's default), `text` mirrored for
/// `Node::text`.
pub(crate) fn set_replaced_prop(w: &mut W, def: &crate::kinds::PropDef, v: &Value) -> Result<crate::scene::Dirty, String> {
    let W::Ext(x) = w else { return Err("not an own widget".into()) };
    let v = if matches!(v, Value::Null) && !def.nullable { crate::scene::props::default_value(def) } else { v.clone() };
    let mut d = {
        let mut b = x.0.borrow_mut();
        let entry = b.entry();
        let i = entry.props.iter().position(|p| p.name == def.name).ok_or_else(|| format!("<{}> has no prop {}", entry.tag, def.name))?;
        svelters_host::exports::__private::with_cx(entry.namespace, |cx| b.set_prop(i, v.clone(), cx)).map_err(|e| e.message)?;
        if entry.props[i].measure { crate::scene::Dirty::MEASURE } else { crate::scene::Dirty::EMPTY }
    };
    if def.key == crate::kinds::P::Text {
        x.1 = match v {
            Value::Str(t) => t,
            _ => String::new(),
        };
        d.insert(crate::scene::Dirty::MEASURE);
    }
    if def.layout {
        d.insert(crate::scene::Dirty::LAYOUT);
    }
    Ok(d)
}

/// OW0: a read of built-in prop `def` from an own widget that replaces the kind.
pub(crate) fn get_replaced_prop(x: &ExtWidget, def: &crate::kinds::PropDef) -> Result<Value, String> {
    let w = x.0.borrow();
    let entry = w.entry();
    let i = entry.props.iter().position(|p| p.name == def.name).ok_or_else(|| format!("<{}> has no prop {}", entry.tag, def.name))?;
    svelters_host::exports::__private::with_cx(entry.namespace, |cx| w.get_prop(i, cx)).map_err(|e| e.message)
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
    let dark = ui.ctx().theme() == egui::Theme::Dark;
    // RA3: a provider text input gets EV3's pending edit command before its `ui`
    let text_input = matches!(&arena.payload[i], crate::scene::node::Payload::Builtin(W::Ext(x)) if x.0.try_borrow().is_ok_and(|w| w.text_input().is_some()));
    if text_input {
        crate::focus::before_text(ui, id);
    }
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
            dark,
            ctx: ui.ctx().clone(),
        };
        let resp = w.ui(ui, &mut cx);
        let WidgetCx { state, changed, emits, remeasure, errors, .. } = cx;
        (resp, state, changed, emits, remeasure, errors)
    });
    let Some((mut resp, state, changed, emits, remeasure, errors)) = ran else { return None };
    if text_input {
        crate::focus::after_text(ui, id);
    }
    // EM1: the accessible name and, for an anchored node, the anchor as AccessKit author id
    resp = crate::a11y::decorate(resp, &arena.n(h));
    let c = arena.common(i);
    if !c.tooltip.is_empty() {
        // OW0 (DESIGN R9): an own widget shows its tooltip while disabled too (the reason)
        resp = if !resp.enabled() && crate::style::is_own(arena.w(i)) { resp.on_disabled_hover_text(c.tooltip.clone()) } else { resp.on_hover_text(c.tooltip.clone()) };
    }
    let c = arena.common(i);
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
        let prop = reg.ext_prop_id(kind, p);
        arena.record(crate::scene::Change::Prop { node, prop: crate::scene::PropId(prop), origin: crate::scene::Origin::Widget }, crate::scene::Origin::Widget);
        if arena.observes(i, prop) {
            out.push(Out::Changed(h, prop));
        }
    }
    for (e, v) in emits {
        let sig = reg.ext_signal_id(kind, e);
        if arena.listens(i, sig) {
            // OW0: an own widget's event keeps the built-in signal's arity (no args: no payload)
            let args = if reg.provider(kind).is_none() && reg.signal_def(kind, sig).is_some_and(|d| d.args.is_empty()) { vec![] } else { vec![v] };
            out.push(Out::Event(h, sig, args));
        }
    }
    for e in errors {
        eprintln!("svelters-egui: {e}");
    }
}
