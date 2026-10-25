//! Kinds (architecture §4.2): the retained behaviour of a node. A kind is a static [`KindSpec`] (its
//! props with their change classes, its signals, slots, accessibility role and text-content prop)
//! plus a [`Kind`] value per node that lays out, records, hit-tests, handles input and projects
//! accessibility through the contexts in [`crate::cx`].

use crate::cx::{A11yCx, EventCx, HitCx, MeasureCx, PrepareCx, PropCx, RecordCx, StyleCx};
use crate::ids::{KindId, PropId, SignalId, SlotId};
use crate::value::{PropDefault, Value, ValueType};
use presto_base::change::ChangeClass;
use presto_base::event::{ImeEvent, KeyEvent, Modifiers, MouseButton, PointerType, ScrollDelta};
use presto_base::geom::{Point, Rect, Vec2};
use presto_base::ids::Revision;
use presto_base::input::TextInputClient;
use std::ops::Range;

/// One prop of a kind.
#[derive(Clone, Copy, Debug)]
pub struct PropSpec {
    pub name: &'static str,
    pub ty: ValueType,
    /// The work a write causes (architecture §4.3).
    pub change: ChangeClass,
    pub default: PropDefault,
    /// The names of an `Enum` prop's values, in index order.
    pub enum_values: &'static [&'static str],
    /// The kind owns this prop's live value (scroll offset, selection, a slider during a drag): a JS
    /// write older than the kind's last change of it is rejected (architecture §4.5).
    pub native_owned: bool,
}

impl PropSpec {
    pub const fn new(name: &'static str, ty: ValueType, change: ChangeClass) -> Self {
        PropSpec { name, ty, change, default: PropDefault::Unset, enum_values: &[], native_owned: false }
    }
    pub const fn default(mut self, d: PropDefault) -> Self {
        self.default = d;
        self
    }
    pub const fn enum_values(mut self, v: &'static [&'static str]) -> Self {
        self.enum_values = v;
        self
    }
    pub const fn native(mut self) -> Self {
        self.native_owned = true;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct SignalFlags(pub u8);

impl SignalFlags {
    pub const NONE: SignalFlags = SignalFlags(0);
    /// Listeners can never cancel the native default, so they never hold up scrolling (§11.5).
    pub const PASSIVE: SignalFlags = SignalFlags(1);
    /// A listener may call `preventDefault`.
    pub const CANCELABLE: SignalFlags = SignalFlags(2);
    /// Delivered to JS at event time (dispatch and drain), not in the next frame's input flush.
    pub const SYNC: SignalFlags = SignalFlags(4);
    /// Propagates through ancestors (capture and bubble); otherwise target only.
    pub const BUBBLES: SignalFlags = SignalFlags(8);
    pub const fn contains(self, o: SignalFlags) -> bool {
        self.0 & o.0 == o.0
    }
    pub const fn union(self, o: SignalFlags) -> SignalFlags {
        SignalFlags(self.0 | o.0)
    }
}

/// One signal of a kind (an event the kind emits: `onchange`, `onactivate`).
#[derive(Clone, Copy, Debug)]
pub struct SignalSpec {
    pub name: &'static str,
    /// The type of the argument (`None`: no argument).
    pub arg: Option<ValueType>,
    pub flags: SignalFlags,
}

impl SignalSpec {
    pub const fn new(name: &'static str, arg: Option<ValueType>, flags: SignalFlags) -> Self {
        SignalSpec { name, arg, flags }
    }
}

/// One named slot of a container kind.
#[derive(Clone, Copy, Debug)]
pub struct SlotSpec {
    pub name: &'static str,
    /// At most one child in the slot.
    pub single: bool,
}

/// The static description of a kind. Generated into the svelters spec (`presto.json`), so the
/// compiler can type props and tag writes with their change class.
#[derive(Clone, Copy, Debug)]
pub struct KindSpec {
    /// The tag (`button`, `text`), matched by CSS type selectors.
    pub name: &'static str,
    pub props: &'static [PropSpec],
    pub signals: &'static [SignalSpec],
    pub slots: &'static [SlotSpec],
    pub role: accesskit::Role,
    /// The prop that holds the node's text content (`<text>hello</text>` writes it).
    pub text_prop: Option<&'static str>,
    /// Takes keyboard focus (in tab order unless `tabindex` says otherwise).
    pub focusable: bool,
    /// Has children (a leaf kind rejects inserts).
    pub container: bool,
}

impl KindSpec {
    pub const fn new(name: &'static str) -> Self {
        KindSpec {
            name,
            props: &[],
            signals: &[],
            slots: &[],
            role: accesskit::Role::GenericContainer,
            text_prop: None,
            focusable: false,
            container: false,
        }
    }
    pub const fn id(&self) -> KindId {
        KindId::of(self.name)
    }
    pub fn prop_id(&self, i: usize) -> PropId {
        let p = &self.props[i];
        PropId::of(self.name, p.name, p.ty.name())
    }
    pub fn signal_id(&self, i: usize) -> SignalId {
        let s = &self.signals[i];
        SignalId::of(self.name, s.name, s.arg.map_or("void", ValueType::name))
    }
    pub fn slot_id(&self, i: usize) -> SlotId {
        SlotId::of(self.name, self.slots[i].name, "slot")
    }
    /// The id of the prop named `name`.
    pub fn prop_named(&self, name: &str) -> Option<PropId> {
        self.props.iter().position(|p| p.name == name).map(|i| self.prop_id(i))
    }
    pub fn signal_named(&self, name: &str) -> Option<SignalId> {
        self.signals.iter().position(|s| s.name == name).map(|i| self.signal_id(i))
    }
    pub fn slot_named(&self, name: &str) -> Option<SlotId> {
        self.slots.iter().position(|s| s.name == name).map(|i| self.slot_id(i))
    }
}

/// The result of a native default handler.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Handled {
    #[default]
    No,
    /// The kind acted on the event; propagation of native handling stops here (JS listeners still run).
    Yes,
}

/// A kind's precise hit test inside its box.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitResult {
    /// The point is outside the node's shape: test what is below.
    Miss,
    /// The node is the target and blocks what is below.
    Hit,
    /// The node is a target for pointer events but lets scroll/wheel through to what is below
    /// (GPUI's `BlockMouseExceptScroll`).
    HitExceptScroll,
}

/// Pointer input as a kind sees it.
#[derive(Clone, Debug, PartialEq)]
pub struct PointerInput {
    pub pointer: u32,
    pub kind: PointerType,
    pub button: Option<MouseButton>,
    pub buttons: u32,
    /// Window points.
    pub pos: Point,
    /// In the node's local space (its box's top-left is the origin).
    pub local: Point,
    pub modifiers: Modifiers,
    pub pressure: f32,
    /// 1 for a single click, 2 for a double click, … (pointer down and click only).
    pub count: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WheelInput {
    pub pos: Point,
    pub local: Point,
    pub delta: ScrollDelta,
    pub modifiers: Modifiers,
}

/// Why focus moved (it decides `:focus-visible`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusReason {
    Pointer,
    Keyboard,
    Programmatic,
}

/// An input event delivered to a kind's native default handler.
#[derive(Debug)]
pub enum Event<'a> {
    PointerDown(&'a PointerInput),
    PointerUp(&'a PointerInput),
    PointerMove(&'a PointerInput),
    /// The press that started on this node was released on it (egui's click semantics).
    Click(&'a PointerInput),
    PointerCancel,
    HoverEnter,
    HoverLeave,
    Wheel(&'a WheelInput),
    Key(&'a KeyEvent),
    Ime(&'a ImeEvent),
    FocusIn(FocusReason),
    FocusOut,
}

/// State a kind owns natively (architecture §4.5). The document checks writes of `native_owned` props
/// against [`NativeState::changed_at`].
pub trait NativeState {
    /// The revision of the kind's last native change of `prop` (`None`: never changed natively).
    fn changed_at(&self, prop: PropId) -> Option<Revision>;
    /// The live value of `prop` (what `bind:` reads).
    fn get(&self, prop: PropId) -> Option<Value>;
}

/// Children supplied on demand (virtual list and tree rows). The document realises the rows in
/// `visible`, asks the kind to bind them, and lays them out where [`VirtualChildren::row_rect`] says.
pub trait VirtualChildren {
    /// The number of rows of the model.
    fn row_count(&self) -> usize;
    /// The rows to realise for a viewport (content-space rect).
    fn visible(&self, viewport: Rect) -> Range<usize>;
    /// Row `i`'s rect in content space.
    fn row_rect(&self, i: usize) -> Rect;
    /// The content size (scroll extent).
    fn content_extent(&self) -> Vec2;
}

/// The retained behaviour of one node.
#[allow(unused_variables)]
pub trait Kind: 'static {
    /// Static description: props (with change class), signals, slots, a11y role, the text-content prop.
    fn spec() -> &'static KindSpec
    where
        Self: Sized;

    /// A prop was written (after the document stored it). Kinds that cache derived data refresh it;
    /// a kind may reject a value it cannot take (the write is undone).
    fn prop_changed(&mut self, cx: &mut PropCx, prop: PropId, value: Option<&Value>) -> Result<(), crate::PropError> {
        Ok(())
    }

    /// Layout participation: taffy style from the computed style.
    fn layout_style(&self, cx: &StyleCx) -> taffy::Style {
        cx.default_layout()
    }

    /// Intrinsic size of a leaf.
    fn measure(&self, cx: &mut MeasureCx, known: taffy::Size<Option<f32>>, avail: taffy::Size<taffy::AvailableSpace>) -> taffy::Size<f32> {
        taffy::Size::ZERO
    }

    /// After layout, before recording: allocate text layouts, ask for images, place owned children.
    fn prepare(&mut self, cx: &mut PrepareCx) {}

    /// Write this node's display list (node-local coordinates). Called only when content-dirty.
    fn record(&self, cx: &mut RecordCx);

    /// Precise hit test inside the node's bounds (default: the rounded rect of the box).
    fn hit_test(&self, cx: &HitCx, p: Point) -> HitResult {
        cx.box_hit(p)
    }

    /// Native defaults for input reaching this node.
    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        Handled::No
    }

    /// Text input client, for kinds that edit text (§11.3).
    fn text_input(&mut self) -> Option<&mut dyn TextInputClient> {
        None
    }

    /// Native-owned state the kind exposes (§4.5).
    fn native_state(&self) -> Option<&dyn NativeState> {
        None
    }

    /// Children supplied on demand.
    fn virtual_children(&mut self) -> Option<&mut dyn VirtualChildren> {
        None
    }

    /// AccessKit node from props and state.
    fn a11y(&self, cx: &mut A11yCx) {
        cx.default_a11y();
    }
}

/// Constructs a node's kind value.
pub type KindFactory = fn() -> Box<dyn Kind>;
