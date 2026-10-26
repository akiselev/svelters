//! Kinds (architecture §4.2): the retained behaviour of a node. A kind is a static [`KindSpec`] (its
//! props with their change classes, its signals, slots, accessibility role and text-content prop)
//! plus a [`Kind`] value per node that lays out, records, hit-tests, handles input and projects
//! accessibility through the contexts in [`crate::cx`].

use crate::cx::{A11yCx, ChildLayoutCx, EventCx, HitCx, MeasureCx, NodeView, PrepareCx, PropCx, RecordCx, StyleCx};
use crate::ids::{KindId, MethodId, PropId, SignalId, SlotId};
use crate::value::{PropDefault, Value, ValueType};
use skelter_base::change::ChangeClass;
use skelter_base::event::{ImeEvent, KeyEvent, Modifiers, MouseButton, PointerType, ScrollDelta};
use skelter_base::geom::{Point, Rect, Vec2};
use skelter_base::ids::Revision;
use skelter_base::input::TextInputClient;
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
    /// The prop may be unset (`null`/`undefined` in JS) after it was set.
    pub nullable: bool,
    pub doc: &'static str,
}

impl PropSpec {
    pub const fn new(name: &'static str, ty: ValueType, change: ChangeClass) -> Self {
        PropSpec { name, ty, change, default: PropDefault::Unset, enum_values: &[], native_owned: false, nullable: false, doc: "" }
    }
    pub const fn nullable(mut self) -> Self {
        self.nullable = true;
        self
    }
    pub const fn doc(mut self, d: &'static str) -> Self {
        self.doc = d;
        self
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

/// A named, typed argument of a signal or method.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArgSpec {
    pub name: &'static str,
    pub ty: ValueType,
    pub nullable: bool,
}

impl ArgSpec {
    pub const fn new(name: &'static str, ty: ValueType) -> Self {
        ArgSpec { name, ty, nullable: false }
    }
    pub const fn nullable(mut self) -> Self {
        self.nullable = true;
        self
    }
}

/// One signal of a kind (an event the kind emits: `onchange`, `onactivate`).
#[derive(Clone, Copy, Debug)]
pub struct SignalSpec {
    pub name: &'static str,
    /// The type of the argument as it travels (`None`: no argument; several named args travel as a
    /// `Record` of `args`). Its id hashes this type.
    pub arg: Option<ValueType>,
    pub flags: SignalFlags,
    /// The named arguments, for the spec and typings (empty: `arg` alone, unnamed).
    pub args: &'static [ArgSpec],
    pub doc: &'static str,
}

impl SignalSpec {
    pub const fn new(name: &'static str, arg: Option<ValueType>, flags: SignalFlags) -> Self {
        SignalSpec { name, arg, flags, args: &[], doc: "" }
    }
    pub const fn args(mut self, a: &'static [ArgSpec]) -> Self {
        self.args = a;
        self
    }
    pub const fn doc(mut self, d: &'static str) -> Self {
        self.doc = d;
        self
    }
}

/// One method a kind exposes to JS (`focusAt(index)`, `scrollToRow(row)`), called through
/// [`Document::call`](crate::Document::call).
#[derive(Clone, Copy, Debug)]
pub struct MethodSpec {
    pub name: &'static str,
    pub params: &'static [ArgSpec],
    /// `None`: returns nothing.
    pub ret: Option<ArgSpec>,
    pub doc: &'static str,
}

impl MethodSpec {
    pub const fn new(name: &'static str, params: &'static [ArgSpec], ret: Option<ArgSpec>) -> Self {
        MethodSpec { name, params, ret, doc: "" }
    }
    pub const fn doc(mut self, d: &'static str) -> Self {
        self.doc = d;
        self
    }
}

/// Where a node's accessible name comes from, tried in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameSource {
    /// The common `aria-label` prop.
    AriaLabel,
    /// One of the kind's props.
    Prop(&'static str),
    /// The text-content prop.
    Text,
    /// The concatenated labels of the node's children (a button with a text child).
    Children,
}

/// The default name sources: `aria-label`, then the text content.
pub const DEFAULT_NAME_FROM: &[NameSource] = &[NameSource::AriaLabel, NameSource::Text];

/// One named slot of a container kind.
#[derive(Clone, Copy, Debug)]
pub struct SlotSpec {
    pub name: &'static str,
    /// At most one child in the slot.
    pub single: bool,
}

/// The static description of a kind. Generated into the svelters spec (`skelter.json`), so the
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
    /// Its children are content it lays out, records, hit-tests and names itself (a label's inline
    /// runs): the node is a layout leaf (its `measure` is called), its children are not laid out,
    /// recorded or hit, and a child's prop, style or structure change remeasures and repaints it.
    pub folds_children: bool,
    pub methods: &'static [MethodSpec],
    /// The accessible name's sources, in order.
    pub name_from: &'static [NameSource],
    /// The paint parts it draws (`::part(name)`): the document computes each part's style with the
    /// node's (inheriting from it), and kinds read it with [`NodeView::part`](crate::NodeView::part).
    pub parts: &'static [&'static str],
    /// Its [`Kind::child_layout`] adjusts its children (a stack shows one): the document calls it for
    /// every child when the child is restyled, and again for all children after a `Layout`-class
    /// write to this node or a restyle of it.
    pub places_children: bool,
    /// The state bits [`Kind::prop_state`] derives from props (`:checked`, `:indeterminate`): after
    /// every prop write the document sets these bits as the hook says.
    pub prop_states: svelters_style::StateMask,
    pub doc: &'static str,
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
            folds_children: false,
            methods: &[],
            name_from: DEFAULT_NAME_FROM,
            parts: &[],
            places_children: false,
            prop_states: svelters_style::StateMask::NONE,
            doc: "",
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
    pub fn method_id(&self, i: usize) -> MethodId {
        let m = &self.methods[i];
        MethodId::of(self.name, m.name, m.ret.map_or("void", |r| r.ty.name()))
    }
    pub fn method_named(&self, name: &str) -> Option<MethodId> {
        self.methods.iter().position(|m| m.name == name).map(|i| self.method_id(i))
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
    /// A deadline the kind set with [`EventCx::wake_at`](crate::EventCx::wake_at) passed (press
    /// repeat, press-held → drag).
    Timer,
    /// An assistive technology asked for an action on this node.
    A11y(&'a accesskit::ActionRequest),
    /// An assistive technology asked for an action on the node's virtual child with this index
    /// ([`A11yCx::add_virtual_child`](crate::A11yCx::add_virtual_child)).
    A11yChild(&'a accesskit::ActionRequest, u32),
}

/// A node's peer group: the siblings (same parent, tree order, the node included) of the node's own
/// kind that act as one control, as the radio buttons of a parent or the toggle buttons of a group
/// do. The document gives a group of two or more focusable members one Tab stop (the first member
/// whose [`PeerGroup::stop`] prop is true, else the first member) and applies the members'
/// [`EventCx::peer_request`](crate::EventCx::peer_request)s through [`Kind::peer`].
#[derive(Clone, Debug, PartialEq)]
pub struct PeerGroup {
    /// Only the siblings whose prop `key.0` equals `key.1` are members (a radio button's `group`
    /// name); `None`: every sibling of the kind.
    pub key: Option<(PropId, Value)>,
    /// The bool prop that makes a member the group's Tab stop (`checked`).
    pub stop: Option<PropId>,
}

/// A roving focus move inside a peer group, over its focusable members.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rove {
    /// By `n` members (−1 previous, +1 next), wrapping.
    By(i32),
    First,
    Last,
}

impl Rove {
    /// The member index this move lands on, from `at` in a group of `len` (`len > 0`).
    pub fn target(self, at: usize, len: usize) -> usize {
        match self {
            Rove::First => 0,
            Rove::Last => len - 1,
            Rove::By(n) => (at as i64 + n as i64).rem_euclid(len as i64) as usize,
        }
    }
}

/// Which members of the node's peer group a request goes to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeerTarget {
    /// Every member but the node.
    Others,
    /// The focusable member a roving move lands on: unless that is the node itself, it takes
    /// keyboard focus first, then gets the message.
    Rove(Rove),
}

/// What a peer is told ([`Kind::peer`]).
#[derive(Clone, Debug, PartialEq)]
pub enum PeerMsg {
    /// A roving move landed on it: it has focus and acts as a click would (a radio checks itself).
    Roved,
    /// The kind's own message (another member was checked: its group value).
    Notify(Option<Value>),
}

/// A kind that scrolls its own content ([`Kind::self_scroll`]): the node is a scroll container of
/// `content` (points), scrolling on the axes set, whatever its CSS `overflow`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelfScroll {
    pub content: skelter_base::geom::Size,
    pub x: bool,
    pub y: bool,
}

/// What JS listeners did with a signal delivered at event time
/// ([`EventCx::emit_sync`](crate::EventCx::emit_sync)).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    /// A listener called `preventDefault` (cancelable signals only): skip the default action.
    pub prevented: bool,
    /// A listener stopped propagation.
    pub stopped: bool,
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
    /// Binds `rows` (the range [`VirtualChildren::visible`] gave, clamped to the row count) to child
    /// nodes of [`RealizeCx::node`](crate::cx::RealizeCx::node): recycled through a
    /// [`RowPool`](crate::virt::RowPool), or created and inserted with
    /// [`RealizeCx::doc`](crate::cx::RealizeCx::doc), their props written for the row. Returns them as
    /// `(row, node)`. The document marks them [`NodeFlags::ROW`](crate::NodeFlags::ROW), places each
    /// absolutely at [`VirtualChildren::row_rect`] in the node's content space, and hides (`display:
    /// none`) the rows it bound before that are not returned (pooled rows stay alive). Called before
    /// layout when the range, the row count or the extent changed, and after
    /// [`Document::refresh_virtual`](crate::Document::refresh_virtual).
    fn realize(&mut self, cx: &mut crate::cx::RealizeCx, rows: Range<usize>) -> Vec<(usize, skelter_base::ids::NodeId)> {
        let _ = (cx, rows);
        Vec::new()
    }
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

    /// A method call from JS ([`KindSpec::methods`]); `args` are checked against the spec's params.
    fn call(&mut self, cx: &mut EventCx, method: MethodId, args: &[Value]) -> Result<Option<Value>, crate::PropError> {
        Err(crate::PropError::Rejected(format!("{method:?} is not implemented by this kind")))
    }

    /// The kind as `Any`, for an app that reads a known kind's state ([`Document::with_kind`]; the
    /// runner reads the menu bar's model this way). `None` by default.
    ///
    /// [`Document::with_kind`]: crate::Document::with_kind
    fn as_any(&self) -> Option<&dyn std::any::Any> {
        None
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

    /// Room in points for chrome the kind paints itself around its children (a frame's border and
    /// caption row): added to the padding of [`Kind::layout_style`]'s style before the children are
    /// laid out. It may measure text. A kind whose chrome follows a prop gets its new padding after
    /// a `Layout`-class write of it.
    fn chrome_padding(&self, cx: &mut MeasureCx) -> Option<skelter_base::geom::Insets> {
        None
    }

    /// For a kind with [`KindSpec::places_children`]: adjusts child `cx.index`'s taffy style (after
    /// the child's own cascade and kind) and returns whether the child is shown. A child that is not
    /// shown keeps its layout (it still sizes the container) but is not painted, hit or in the
    /// accessibility tree.
    fn child_layout(&self, cx: &ChildLayoutCx, style: &mut taffy::Style) -> bool {
        true
    }

    /// The node's descendants are inert (no input, `:disabled`), as an unchecked checkable frame's
    /// content. Asked after every prop write.
    fn children_inert(&self) -> bool {
        false
    }

    /// The state bits of [`KindSpec::prop_states`] that the node's props imply now (`:checked` for
    /// `checked`), asked after every prop write and when the node is created.
    fn prop_state(&self, node: &NodeView) -> svelters_style::StateMask {
        svelters_style::StateMask::NONE
    }

    /// The node's peer group ([`PeerGroup`]), if any.
    fn peer_group(&self, node: &NodeView) -> Option<PeerGroup> {
        None
    }

    /// A member of the node's peer group (`from`) sent it `msg`; the context is an event's (signals,
    /// native props, focus, its own peer requests).
    fn peer(&mut self, cx: &mut EventCx, from: skelter_base::ids::NodeId, msg: &PeerMsg) -> Handled {
        Handled::No
    }

    /// The kind scrolls its own content (a code editor): its node gets a scroll node, a content
    /// space and a scroll layer as an `overflow: auto` box does, and its own display list is
    /// recorded in content space and placed in that layer, so a scroll is a compositor offset
    /// ([`EventCx::scroll_fast`](crate::EventCx::scroll_fast)) and never a re-record. What it records
    /// in [`RecordCx::fixed`](crate::RecordCx::fixed) (its box) stays in the border box, below the
    /// scroll layer. Pointer positions stay in the border box: add [`Document::scroll_offset`](crate::Document::scroll_offset).
    /// Asked after each `prepare`.
    fn self_scroll(&self, node: &NodeView) -> Option<SelfScroll> {
        None
    }

    /// Another node claimed the one selection ([`EventCx::claim_selection`](crate::EventCx::claim_selection)
    /// with [`Document::set_single_selection`](crate::Document::set_single_selection) on, GTK's rule):
    /// collapse this node's selection (to its start).
    fn selection_lost(&mut self, cx: &mut EventCx) {}

    /// An open floating surface (popup, menu, select list, tooltip) that a press outside it closes:
    /// while true, a primary-or-other pointer press in the window outside the node's subtree first
    /// reaches [`Kind::outside_press`]. Asked after every prop write and every call into the kind.
    fn light_dismiss(&self, node: &NodeView) -> bool {
        false
    }

    /// A press outside the node's subtree while [`Kind::light_dismiss`] is true (`p.local` in the
    /// node's space), before the press is dispatched as usual (it is not consumed).
    fn outside_press(&mut self, cx: &mut EventCx, p: &PointerInput) -> Handled {
        Handled::No
    }

    /// JS started (`listened`) or stopped listening to `signal` on this node (an icon takes clicks
    /// only while its click signal is listened). [`NodeView::listened`](crate::NodeView::listened)
    /// answers the same at any time.
    fn listen_changed(&mut self, cx: &mut PropCx, signal: SignalId, listened: bool) {}
}

/// Constructs a node's kind value.
pub type KindFactory = fn() -> Box<dyn Kind>;
