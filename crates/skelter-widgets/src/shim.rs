//! The interface the built-in kinds are written against, bridged to skelter-core and skelter-text by
//! [`crate::host`] (`plans/cloud/presto-b2/BRIEF.md`). It keeps what skelter-core's `Kind` leaves to the
//! document: typed props held by the kind, events in the shape the egui-semantics tracker reads, and
//! contexts that carry the computed style the UA tables layer under. `plans/milestones/SKELTER-widgets-basic.md`
//! lists what skelter-core would need for the kinds to implement its `Kind` directly.
//!
//! - [`core`]: the kind interface ([`core::Kind`], [`core::KindSpec`], [`core::Value`], the contexts, the
//!   [`core::Event`] a kind receives).
//! - [`text`]: rich text in, placed runs out ([`text::TextShaper`]); implemented for
//!   `skelter_text::TextSystem` in [`crate::host`].
//! - [`testing`]: a deterministic monospace shaper and in-memory resources, for this crate's tests (a
//!   test double, never a production path).

pub mod core {
    //! Stand-in for skelter-core's kind API (architecture §4.2, b1 brief "skelter-core").

    use crate::interact::{FocusReason, Policy, StateBits};
    use crate::shim::text::TextShaper;
    use skelter_base::accesskit;
    use skelter_base::change::ChangeClass;
    use skelter_base::color::Color;
    use skelter_base::cursor::CursorIcon;
    use skelter_base::display::DisplayList;
    use skelter_base::event::{KeyEvent, PointerEvent, WheelEvent};
    use skelter_base::geom::{Point, Rect, Size};
    use skelter_base::ids::{ImageId, RunId};
    use skelter_base::commit::ImageData;
    use skelter_base::text::GlyphRun;
    use skelter_base::time::Time;
    use std::sync::Arc;
    use svelters_style::ComputedStyle;

    /// Stands in for `skelter_core::PropId`: here the index of the prop in its kind's [`KindSpec::props`].
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct PropId(pub u32);
    /// Stands in for `skelter_core::SignalId`: the index in [`KindSpec::signals`].
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct SignalId(pub u32);

    /// Stands in for `skelter_core::Value`.
    #[derive(Clone, Debug, PartialEq)]
    pub enum Value {
        Bool(bool),
        I64(i64),
        F64(f64),
        Str(Arc<str>),
        /// One of the prop's [`ValueType::Enum`] names.
        Enum(&'static str),
        Color(Color),
        List(Vec<Value>),
        Record(Vec<(Arc<str>, Value)>),
    }

    impl Value {
        pub fn as_bool(&self) -> Option<bool> {
            match self {
                Value::Bool(b) => Some(*b),
                _ => None,
            }
        }
        pub fn as_f64(&self) -> Option<f64> {
            match self {
                Value::F64(v) => Some(*v),
                Value::I64(v) => Some(*v as f64),
                _ => None,
            }
        }
        pub fn as_i64(&self) -> Option<i64> {
            match self {
                Value::I64(v) => Some(*v),
                Value::F64(v) if v.fract() == 0.0 => Some(*v as i64),
                _ => None,
            }
        }
        pub fn as_str(&self) -> Option<&str> {
            match self {
                Value::Str(s) => Some(s),
                Value::Enum(s) => Some(s),
                _ => None,
            }
        }
        pub fn str(s: &str) -> Value {
            Value::Str(Arc::from(s))
        }
    }

    /// A prop's or signal argument's type, as the spec carries it.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum ValueType {
        Bool,
        I64,
        F64,
        Str,
        Enum(&'static [&'static str]),
        Color,
        List,
        Record,
    }

    /// A prop's default in the spec.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum Default {
        Bool(bool),
        I64(i64),
        F64(f64),
        Str(&'static str),
        Enum(&'static str),
        None,
    }

    impl Default {
        pub fn value(self) -> Option<Value> {
            Some(match self {
                Default::Bool(b) => Value::Bool(b),
                Default::I64(v) => Value::I64(v),
                Default::F64(v) => Value::F64(v),
                Default::Str(s) => Value::str(s),
                Default::Enum(s) => Value::Enum(s),
                Default::None => return None,
            })
        }
    }

    /// One prop of a kind: name (kebab-case, as Svelte writes it), type, change class (architecture §4.3).
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct PropSpec {
        pub name: &'static str,
        pub ty: ValueType,
        pub change: ChangeClass,
        pub default: Default,
        /// Native code changes it too (a checkbox's `checked`): JS observes it through `bind:` (§4.5).
        pub bindable: bool,
        pub doc: &'static str,
    }

    impl PropSpec {
        pub const fn new(name: &'static str, ty: ValueType, change: ChangeClass, default: Default, doc: &'static str) -> PropSpec {
            PropSpec { name, ty, change, default, bindable: false, doc }
        }
        pub const fn bindable(mut self) -> PropSpec {
            self.bindable = true;
            self
        }
    }

    /// One signal (a JS-facing event) of a kind (architecture §11.5).
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct SignalSpec {
        pub name: &'static str,
        /// Its listeners never call `preventDefault` (scroll and drag paths default to passive).
        pub passive: bool,
        /// A listener can cancel the native default that follows it.
        pub cancelable: bool,
        /// Delivered synchronously at event time (the default action waits for it).
        pub sync: bool,
        pub args: &'static [(&'static str, ValueType)],
        pub doc: &'static str,
    }

    impl SignalSpec {
        pub const fn new(name: &'static str, args: &'static [(&'static str, ValueType)], doc: &'static str) -> SignalSpec {
            SignalSpec { name, passive: true, cancelable: false, sync: false, args, doc }
        }
        pub const fn cancelable(mut self) -> SignalSpec {
            self.cancelable = true;
            self.passive = false;
            self.sync = true;
            self
        }
    }

    /// Where a node's accessible name comes from.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum NameFrom {
        /// A prop's string value (`text`, `label`).
        Prop(&'static str),
        /// The text of its descendants.
        Contents,
        /// No name of its own (`aria-label` still applies).
        None,
    }

    /// Stands in for `skelter_core::KindSpec`: the static description of a kind (architecture §4.2).
    #[derive(Debug)]
    pub struct KindSpec {
        /// The element tag (`button`, `checkbox`).
        pub tag: &'static str,
        pub props: &'static [PropSpec],
        pub signals: &'static [SignalSpec],
        /// Named child slots; empty = children go in the default slot (or none, for leaves).
        pub slots: &'static [&'static str],
        /// The kind takes element children.
        pub children: bool,
        pub role: accesskit::Role,
        pub name_from: NameFrom,
        /// The prop that holds its text content (`<label>text</label>`), if any.
        pub text_prop: Option<&'static str>,
        /// It takes keyboard focus by default (in the Tab order).
        pub focusable: bool,
    }

    impl KindSpec {
        pub fn prop(&self, name: &str) -> Option<PropId> {
            self.props.iter().position(|p| p.name == name).map(|i| PropId(i as u32))
        }
        pub fn signal(&self, name: &str) -> Option<SignalId> {
            self.signals.iter().position(|s| s.name == name).map(|i| SignalId(i as u32))
        }
        pub fn prop_spec(&self, id: PropId) -> Option<&'static PropSpec> {
            self.props.get(id.0 as usize)
        }
    }

    /// Stands in for `skelter_core::PropError`.
    #[derive(Clone, Debug, PartialEq)]
    pub enum PropError {
        UnknownProp,
        /// The value has the wrong type (expected type in the spec).
        Type { prop: &'static str, expected: ValueType },
        /// Native code changed this state since the event JS saw (architecture §4.5).
        Stale,
        /// The value has the right type but the kind rejects it (a `grid-template-columns` that is
        /// not a track list); `message` says why.
        Invalid { prop: &'static str, message: String },
    }

    /// Stands in for `skelter_core::Handled`.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Handled {
        No,
        Yes,
    }

    /// Stands in for `skelter_core::HitResult`.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum HitResult {
        Miss,
        Hit,
    }

    /// Stands in for the event a kind receives from skelter-core's dispatch (architecture §11). Pointer
    /// positions are node-local (points); core has done the hit test, capture and hover.
    #[derive(Clone, Debug, PartialEq)]
    pub enum Event {
        /// A pointer event on this node or captured by it (it pressed here and the button is held).
        Pointer(PointerEvent),
        /// The pointer is over this node and nothing covers it (`true`), or no longer (`false`).
        Hover(bool),
        Wheel(WheelEvent),
        /// A key while this node has keyboard focus.
        Key(KeyEvent),
        Focus { gained: bool, reason: FocusReason },
        /// An assistive-technology action addressed to this node.
        A11y(accesskit::ActionRequest),
        /// The time the kind asked for with [`EventCx::wake_at`] came (press repeat, a press held long
        /// enough to become a drag).
        Timer,
        /// The pointer capture ended without a release (another window took it, the node was hidden).
        CaptureLost,
        /// A pointer press in the window outside this node and its subtree (node-local position),
        /// for a node that asks for them while open ([`Kind::light_dismiss`]): a popup closes on it.
        OutsidePress(Point),
    }

    /// Stands in for `skelter_core::StyleCx`: the node's computed style and state, for `layout_style`.
    pub struct StyleCx<'a> {
        pub computed: Option<&'a ComputedStyle>,
        pub state: StateBits,
        pub dark: bool,
        /// The taffy style skelter-core's cascade computed (`None` in tests: taffy's default).
        pub base: Option<taffy::Style>,
    }

    impl StyleCx<'_> {
        /// The layout style the cascade gives (CSS over the kind's defaults).
        pub fn default_layout(&self) -> taffy::Style {
            self.base.clone().unwrap_or_default()
        }
    }

    /// The style a kind reads, shared by the contexts: the node's computed style, its parts' computed
    /// styles (`::part(name)`), state and scheme.
    pub struct Styles<'a> {
        pub computed: Option<&'a ComputedStyle>,
        pub parts: &'a [(&'static str, &'a ComputedStyle)],
        /// The inline run children a `label` folds into its paragraph, as `runs` records
        /// (`controls::run`; empty for other kinds).
        pub children: &'a [Value],
        pub state: StateBits,
        pub dark: bool,
    }

    impl<'a> Styles<'a> {
        pub fn part_css(&self, name: &str) -> Option<&'a ComputedStyle> {
            self.parts.iter().find(|(n, _)| *n == name).map(|(_, c)| *c)
        }
    }

    /// What a container that places or hides its children itself reads about one child in
    /// [`Kind::child_layout`]: its index, the `anchor` of every child (empty when it has none) and
    /// the name of the slot each child is in (`""`: the default slot; `slots` may be empty then).
    pub struct ChildCx<'a> {
        pub index: usize,
        pub anchors: &'a [&'a str],
        pub slots: &'a [&'a str],
    }

    impl ChildCx<'_> {
        pub fn count(&self) -> usize {
            self.anchors.len()
        }
        /// The slot child `k` is in (`""`: the default slot).
        pub fn slot_of(&self, k: usize) -> &str {
            self.slots.get(k).copied().unwrap_or("")
        }
    }

    /// Stands in for `skelter_core::MeasureCx`.
    pub struct MeasureCx<'a> {
        pub style: Styles<'a>,
        pub text: &'a mut dyn TextShaper,
        pub scale: f32,
    }

    /// Stands in for `skelter_core::PrepareCx`: after layout, before recording.
    pub struct PrepareCx<'a> {
        pub size: Size,
        pub style: Styles<'a>,
        pub text: &'a mut dyn TextShaper,
        /// Signals emitted while preparing ([`PrepareCx::emit`]); the host queues them after the
        /// prepare (`skelter_core::PrepareCx::emit`).
        pub emitted: Vec<Emitted>,
    }

    impl PrepareCx<'_> {
        pub fn emit(&mut self, signal: SignalId, args: Vec<Value>) {
            self.emitted.push(Emitted { signal, args });
        }
    }

    /// The resources a recording registers (runs and images), the stand-in for skelter-core's commit
    /// builder: a run or image registered once keeps its id while the node holds it.
    pub trait Resources {
        /// The id of `run` in the commit (registered on first use).
        fn run(&mut self, run: &Arc<GlyphRun>) -> RunId;
        /// The id of the image under `key` (a content hash or an icon key with its size and colour);
        /// `make` rasterises it when the key is new. `None` when `make` fails.
        fn image(&mut self, key: &str, make: &mut dyn FnMut() -> Option<ImageData>) -> Option<ImageId>;
    }

    /// Stands in for `skelter_core::RecordCx`: writes the node's display list in node-local coordinates.
    pub struct RecordCx<'a> {
        /// The node's border box size (its local rect is `(0,0)..size`).
        pub size: Size,
        pub style: Styles<'a>,
        pub list: &'a mut DisplayList,
        pub text: &'a mut dyn TextShaper,
        pub res: &'a mut dyn Resources,
        /// Device pixels per point, for snapping edges to the pixel grid.
        pub scale: f32,
    }

    impl RecordCx<'_> {
        pub fn rect(&self) -> Rect {
            Rect::from_origin_size(Point::ZERO, self.size)
        }
    }

    /// Stands in for `skelter_core::HitCx`.
    pub struct HitCx {
        pub size: Size,
    }

    impl HitCx {
        pub fn box_hit(&self, p: Point) -> HitResult {
            if Rect::from_origin_size(Point::ZERO, self.size).contains(p) { HitResult::Hit } else { HitResult::Miss }
        }
    }

    /// A signal a kind emitted, for skelter-core to deliver to JS at event time.
    #[derive(Clone, Debug, PartialEq)]
    pub struct Emitted {
        pub signal: SignalId,
        pub args: Vec<Value>,
    }

    /// Delivers a signal now; returns whether a listener prevented its default.
    pub type SyncEmit<'a> = dyn FnMut(SignalId, &[Value]) -> bool + 'a;

    /// Stands in for `skelter_core::EventCx`: what a kind's native default reads and writes.
    pub struct EventCx<'a> {
        pub now: Time,
        pub size: Size,
        /// The node's state bits; units write the ones they own.
        pub state: &'a mut StateBits,
        /// The node takes input (`enabled` and not inert).
        pub enabled: bool,
        /// The node has keyboard focus.
        pub focused: bool,
        pub policy: &'a Policy,
        /// Signals to deliver, in order.
        pub emitted: Vec<Emitted>,
        /// Props the kind changed natively (`checked` after a click): core journals them with their
        /// change class and tells `bind:` observers.
        pub changed: Vec<PropId>,
        /// The kind asked for focus (`Some(true)`) or gave it up (`Some(false)`).
        pub focus_request: Option<bool>,
        /// The kind wants an [`Event::Timer`] at this time.
        pub wake: Option<Time>,
        /// The kind's content changed (re-record).
        pub repaint: bool,
        pub cursor: Option<CursorIcon>,
        /// The kind asked for pointer capture (`true`) or released it.
        pub capture: Option<bool>,
        /// Requests to the node's peer group ([`PeerRequest`]), applied by core after the event, in order.
        pub peers: Vec<PeerRequest>,
        /// Move keyboard focus to the node with this anchor (a `label`'s `for`); core resolves it.
        pub focus_anchor: Option<String>,
        /// The kind moved its scroll container's offset to this (content points scrolled out at the top
        /// left): the host writes it to the document as a fast commit (`skelter_core::EventCx::scroll_fast`)
        /// and emits the common `scroll` event.
        pub scroll_to: Option<skelter_base::geom::Vec2>,
        /// Delivers a signal to JS now and reports whether a listener prevented its default
        /// ([`EventCx::emit_sync`]); `None`: signals are queued and nothing is prevented.
        pub sync: Option<&'a mut SyncEmit<'a>>,
        /// Signals for the node's children, by child index, with skelter-core's signal ids (the
        /// child's kind declares them): the host emits them on the child
        /// (`skelter_core::EventCx::emit_on`).
        pub emitted_on: Vec<(usize, skelter_core::SignalId, Option<skelter_core::Value>)>,
    }

    impl<'a> EventCx<'a> {
        pub fn new(now: Time, size: Size, state: &'a mut StateBits, policy: &'a Policy) -> EventCx<'a> {
            let enabled = !state.contains(StateBits::DISABLED);
            EventCx { now, size, state, enabled, focused: false, policy, emitted: Vec::new(), changed: Vec::new(), focus_request: None, wake: None, repaint: false, cursor: None, capture: None, peers: Vec::new(), focus_anchor: None, scroll_to: None, sync: None, emitted_on: Vec::new() }
        }
        pub fn emit(&mut self, signal: SignalId, args: Vec<Value>) {
            self.emitted.push(Emitted { signal, args });
        }
        /// Emits `signal` (a signal of the child's kind) on the node's `child`-th child.
        pub fn emit_on(&mut self, child: usize, signal: skelter_core::SignalId, arg: Option<skelter_core::Value>) {
            self.emitted_on.push((child, signal, arg));
        }
        /// Emits `signal` at event time when the host can deliver it now (a cancelable signal whose
        /// default action follows); returns whether a listener prevented that default.
        pub fn emit_sync(&mut self, signal: SignalId, args: Vec<Value>) -> bool {
            match self.sync.as_mut() {
                Some(f) => f(signal, &args),
                None => {
                    self.emit(signal, args);
                    false
                }
            }
        }
        pub fn prop_changed(&mut self, prop: PropId) {
            if !self.changed.contains(&prop) {
                self.changed.push(prop);
            }
            self.repaint = true;
        }
        /// Keeps the earliest of the wake-ups asked for.
        pub fn wake_at(&mut self, t: Time) {
            self.wake = Some(self.wake.map_or(t, |w| w.min(t)));
        }
        /// Asks core to deliver `msg` to the members of the node's peer group that `to` picks.
        pub fn peer_request(&mut self, to: PeerTarget, msg: PeerMsg) {
            self.peers.push(PeerRequest { to, msg });
        }
    }

    /// Stands in for skelter-core's peer groups: sibling nodes of one kind that act as one control (the
    /// radio buttons of a parent, the toggle buttons of a `group`), what svelters-egui reached through
    /// `WidgetCx::sibling_ids`/`with_peer`. A kind declares its group with [`Kind::peer_group`]; what
    /// core must do with it:
    /// - **members**: the node's siblings (same parent, tree order, the node included) whose kind tag
    ///   is [`PeerGroup::tag`] and, when [`PeerGroup::key`] is `(prop, value)`, whose `prop` equals
    ///   `value` (read with [`Kind::get_prop`]);
    /// - **Tab order**: a group of two or more members is one Tab stop, the first member whose bool
    ///   prop [`PeerGroup::stop`] is true, else the first member (WIDGET-PARITY §2.3, Qt and GTK);
    /// - **requests** ([`EventCx::peer_request`]), after the event that made them, in order: for
    ///   [`PeerTarget::Others`] every other member gets [`Kind::peer`] with the message; for
    ///   [`PeerTarget::Rove`] the member the move lands on (from the node's index among the members,
    ///   [`Rove::target`](crate::interact::Rove::target)), unless that is the node itself, first takes
    ///   keyboard focus (an [`Event::Focus`] with [`FocusReason::Keyboard`], the node losing it) and
    ///   then gets the message. A member's own `peer` handling is applied as an event's would be
    ///   (signals, native prop changes, focus, its own peer requests, queued after the current ones).
    #[derive(Clone, Debug, PartialEq)]
    pub struct PeerGroup {
        pub tag: &'static str,
        pub key: Option<(&'static str, Value)>,
        pub stop: &'static str,
    }

    /// Which members of the peer group a [`PeerRequest`] goes to.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum PeerTarget {
        /// Every member but the node.
        Others,
        /// The member a roving move lands on (it takes focus first).
        Rove(crate::interact::Rove),
    }

    /// What a peer is told ([`Kind::peer`]).
    #[derive(Clone, Debug, PartialEq)]
    pub enum PeerMsg {
        /// A roving move landed on it: it has focus and checks itself as a click would (no `clicked`).
        Roved,
        /// Another member was checked; the arguments are the kind's (a radio button's group value).
        Checked(Vec<Value>),
    }

    /// A request to a node's peer group, made during an event.
    #[derive(Clone, Debug, PartialEq)]
    pub struct PeerRequest {
        pub to: PeerTarget,
        pub msg: PeerMsg,
    }

    /// Stands in for `skelter_core::A11yCx`.
    pub struct A11yCx<'a> {
        pub node: &'a mut accesskit::Node,
        pub state: StateBits,
        pub size: Size,
    }

    /// Stands in for `skelter_core::Kind` (architecture §4.2). Prop storage is the kind's own: core calls
    /// [`Kind::set_prop`]/[`Kind::get_prop`] by spec index (skelter-core's API for this is not fixed yet).
    pub trait Kind: 'static {
        fn spec() -> &'static KindSpec
        where
            Self: Sized;
        fn kind_spec(&self) -> &'static KindSpec;
        fn set_prop(&mut self, prop: PropId, v: &Value) -> Result<(), PropError>;
        fn get_prop(&self, prop: PropId) -> Option<Value>;
        fn layout_style(&self, cx: &StyleCx) -> taffy::Style {
            cx.default_layout()
        }
        fn measure(&self, _cx: &mut MeasureCx, _known: taffy::Size<Option<f32>>, _avail: taffy::Size<taffy::AvailableSpace>) -> taffy::Size<f32> {
            taffy::Size::ZERO
        }
        /// Stands in for skelter-core's padded layout (svelters-egui `LayoutCx::padded_layout`): room
        /// `[left, right, top, bottom]` in points for chrome the kind paints itself (a frame's
        /// border and caption row), which core adds to the length parts of the padding of
        /// [`Kind::layout_style`] before laying out the children. `None`: nothing to add. It reads
        /// the measure context because the chrome can hold text.
        fn chrome_padding(&self, _cx: &mut MeasureCx) -> Option<[f32; 4]> {
            None
        }
        /// Stands in for skelter-core's per-child layout hook: a container that places or hides its
        /// children itself (`stack`) adjusts child `cx.index`'s taffy style, after core mapped the
        /// child's computed style. Returns whether the child is shown (painted, hit and in the
        /// accessibility tree); a child that is not shown but keeps its layout still sizes the
        /// container.
        fn child_layout(&self, _cx: &ChildCx, _style: &mut taffy::Style) -> bool {
            true
        }
        /// Stands in for skelter-core's subtree enablement: `true` makes every descendant inert (no
        /// input, `:disabled`), as an unchecked `checkable` frame does to its content.
        fn children_inert(&self) -> bool {
            false
        }
        fn prepare(&mut self, _cx: &mut PrepareCx) {}
        fn record(&self, cx: &mut RecordCx);
        fn hit_test(&self, cx: &HitCx, p: Point) -> HitResult {
            cx.box_hit(p)
        }
        fn event(&mut self, _cx: &mut EventCx, _ev: &Event) -> Handled {
            Handled::No
        }
        /// Methods the app (or JS) calls on the node through `skelter_core::Document::call`, as
        /// skelter-core declares them.
        fn methods() -> &'static [skelter_core::MethodSpec]
        where
            Self: Sized,
        {
            &[]
        }
        /// Method `method` (an index into [`Kind::methods`]) with its checked arguments.
        fn call(&mut self, _cx: &mut EventCx, _method: usize, _args: &[skelter_core::Value]) -> Handled {
            Handled::No
        }
        fn a11y(&self, cx: &mut A11yCx);
        /// EV3: while focused, whether the kind takes `key` with `m` itself (keymaps and Tab skip it).
        fn claims_key(&self, _key: skelter_base::key::Key, _m: skelter_base::event::Modifiers) -> bool {
            false
        }
        /// The peer group the node belongs to ([`PeerGroup`]), if any.
        fn peer_group(&self) -> Option<PeerGroup> {
            None
        }
        /// A member of its peer group sent it `msg` (core applies [`PeerRequest`]s through this).
        fn peer(&mut self, _cx: &mut EventCx, _msg: &PeerMsg) -> Handled {
            Handled::No
        }
        /// Stands in for core telling the kind that JS now listens to `signal` (`listened`) or no
        /// longer does (`Document::listen`). A kind whose behaviour depends on a listener keeps it:
        /// an `icon` or `image` takes clicks only while one of its click signals is listened.
        fn listen(&mut self, _signal: SignalId, _listened: bool) {}
        /// It wants [`Event::OutsidePress`] now (an open floating surface: light dismiss).
        fn light_dismiss(&self) -> bool {
            false
        }
        /// Records the kind's open floating surface (a picker's popup, a select's list) into
        /// `cx.list`, in the node's local points, and returns what it is: skelter-core puts it in a
        /// layer of the window's overlay stack (above the content) and routes pointer input inside
        /// `part.rect` to this node first (`skelter_core::RecordCx::overlay`). `None`: no surface
        /// is open (the default).
        fn record_overlay(&self, _cx: &mut RecordCx) -> Option<skelter_core::overlay::OverlayPart> {
            None
        }
        /// The window's overlay stack closed the surface of [`Kind::record_overlay`] (a press
        /// outside, Escape, its parent closing).
        fn overlay_closed(&mut self, _cx: &mut EventCx, _reason: skelter_core::overlay::DismissReason) -> Handled {
            Handled::No
        }
        /// The state bits the kind derives from its props alone (`:checked`, `:busy`); skelter-core sets
        /// them after every prop write (bits outside [`Kind::prop_states`] are ignored).
        fn prop_state(&self) -> StateBits {
            StateBits::NONE
        }
        /// The paint parts it draws (`::part(name)`): skelter-core computes their styles, which reach
        /// the kind as [`Styles::parts`].
        fn parts() -> &'static [&'static str]
        where
            Self: Sized,
        {
            &[]
        }
        /// It places or hides its own children ([`Kind::child_layout`] is called for each child).
        fn places_children() -> bool
        where
            Self: Sized,
        {
            false
        }
        /// Its children are content it folds into its own (a label's runs, [`Styles::children`]): skelter-core
        /// measures it as a leaf and does not lay out, record or hit-test the children.
        fn folds_children() -> bool
        where
            Self: Sized,
        {
            false
        }
        /// With [`Kind::folds_children`]: the children arrive in [`Styles::children`] as element records
        /// (`{tag, anchor, props, children}`: every prop of the child's kind by name, its `anchor`, its own
        /// children the same way) instead of text runs. A menu bar builds its menus from them.
        fn folds_elements() -> bool
        where
            Self: Sized,
        {
            false
        }
        /// The bits [`Kind::prop_state`] owns.
        fn prop_states() -> StateBits
        where
            Self: Sized,
        {
            StateBits::NONE
        }
    }

    /// Checks a value against a prop's spec type (what `set_prop` implementations call first).
    pub fn check(spec: &PropSpec, v: &Value) -> Result<(), PropError> {
        let ok = matches!(
            (spec.ty, v),
            (ValueType::Bool, Value::Bool(_))
                | (ValueType::I64, Value::I64(_))
                | (ValueType::F64, Value::F64(_) | Value::I64(_))
                | (ValueType::Str, Value::Str(_))
                | (ValueType::Color, Value::Color(_))
                | (ValueType::List, Value::List(_))
                | (ValueType::Record, Value::Record(_))
        ) || matches!((spec.ty, v), (ValueType::Enum(names), Value::Enum(s) ) if names.contains(s))
            || matches!((spec.ty, v), (ValueType::Enum(names), Value::Str(s)) if names.contains(&&**s));
        if ok { Ok(()) } else { Err(PropError::Type { prop: spec.name, expected: spec.ty }) }
    }

    /// An enum prop's value as one of its spec names (a `Str` with a listed name counts).
    pub fn enum_name(spec: &PropSpec, v: &Value) -> Option<&'static str> {
        let ValueType::Enum(names) = spec.ty else { return None };
        let s = v.as_str()?;
        names.iter().copied().find(|n| *n == s)
    }
}

pub mod text {
    //! Stand-in for skelter-text's layout API (b1 brief "skelter-text"): `RichText` in, `TextLayout` with
    //! placed glyph runs out. Colour is not part of shaping: each placed run carries a style index, and
    //! the recorder colours it.

    use skelter_base::geom::{Point, Size};
    use skelter_base::text::GlyphRun;
    use std::ops::Range;
    use std::sync::Arc;

    /// Stands in for `skelter_text::TextStyle`.
    #[derive(Clone, Debug, PartialEq)]
    pub struct TextStyle {
        /// CSS `font-family` list.
        pub family: Arc<str>,
        pub size: f32,
        pub weight: u16,
        pub italic: bool,
        /// Line height in points (`None`: the font's normal).
        pub line_height: Option<f32>,
        pub letter_spacing: f32,
        pub underline: bool,
        pub strikethrough: bool,
    }

    impl std::default::Default for TextStyle {
        fn default() -> Self {
            TextStyle { family: Arc::from("system-ui, sans-serif"), size: 13.0, weight: 400, italic: false, line_height: None, letter_spacing: 0.0, underline: false, strikethrough: false }
        }
    }

    /// Stands in for `skelter_text::Span`: a byte range of the text in another style.
    #[derive(Clone, Debug, PartialEq)]
    pub struct Span {
        pub range: Range<usize>,
        pub style: TextStyle,
    }

    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum TextAlign {
        #[default]
        Start,
        Center,
        End,
    }

    /// Stands in for `skelter_text::RichText`.
    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct RichText {
        pub text: String,
        /// Styled ranges, in order, not overlapping; style index `i + 1` in [`PlacedRun::style`].
        pub spans: Vec<Span>,
        /// Style index 0.
        pub base: TextStyle,
        pub align: TextAlign,
        /// Cut the last line with `…` when it does not fit.
        pub ellipsis: bool,
        pub max_lines: Option<u32>,
    }

    impl RichText {
        pub fn plain(text: &str, base: TextStyle) -> RichText {
            RichText { text: text.to_owned(), base, ..RichText::default() }
        }
    }

    /// Stands in for `skelter_text::PlacedRun`: a shaped run placed in the layout (`origin` = baseline
    /// start), with the index of its style (0 = base, `i + 1` = `spans[i]`).
    #[derive(Clone, Debug, PartialEq)]
    pub struct PlacedRun {
        pub run: Arc<GlyphRun>,
        pub origin: Point,
        pub style: u32,
    }

    /// Caret and selection geometry of a laid-out text (skelter-text's `TextLayout::hit_test` and
    /// `selection_rects`), in the layout's coordinates; offsets are UTF-8 bytes of the text.
    pub trait TextGeometry: Send + Sync {
        /// The caret offset nearest `p`.
        fn hit_byte(&self, p: Point) -> usize;
        /// The rects that highlight `range`, one or more per line.
        fn selection_rects(&self, range: Range<usize>) -> Vec<skelter_base::geom::Rect>;
    }

    /// A layout's geometry, shared (compared by identity).
    #[derive(Clone)]
    pub struct Geometry(pub Arc<dyn TextGeometry>);

    impl std::fmt::Debug for Geometry {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("Geometry")
        }
    }

    impl PartialEq for Geometry {
        fn eq(&self, o: &Geometry) -> bool {
            Arc::ptr_eq(&self.0, &o.0)
        }
    }

    /// Stands in for `skelter_text::TextLayout`.
    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct TextLayout {
        pub size: Size,
        pub runs: Vec<PlacedRun>,
        /// Number of laid-out lines.
        pub lines: u32,
        /// Caret and selection geometry (`None`: the shaper gives none).
        pub geometry: Option<Geometry>,
    }

    impl TextLayout {
        pub fn size(&self) -> Size {
            self.size
        }
        pub fn runs(&self) -> &[PlacedRun] {
            &self.runs
        }
    }

    /// Stands in for `skelter_text::TextSystem::layout`.
    pub trait TextShaper {
        fn layout(&mut self, text: &RichText, max_width: Option<f32>) -> Arc<TextLayout>;
    }
}

pub mod testing {
    //! Test doubles: a deterministic monospace shaper (every char advances `0.6 × size`, glyph id = the
    //! char's code point, one run per line) and an in-memory [`Resources`]. For tests only.

    use super::core::Resources;
    use super::text::{Geometry, PlacedRun, RichText, TextGeometry, TextLayout, TextShaper};
    use skelter_base::commit::ImageData;
    use skelter_base::geom::{Point, Rect, Size};
    use skelter_base::ids::{FontId, ImageId, RunId};
    use skelter_base::text::{AaMode, Glyph, GlyphRun, HintMode};
    use std::collections::HashMap;
    use std::sync::Arc;

    #[derive(Default)]
    pub struct MonoShaper;

    impl MonoShaper {
        pub fn advance(size: f32) -> f32 {
            (size * 0.6 * 4.0).round() / 4.0
        }
    }

    impl TextShaper for MonoShaper {
        fn layout(&mut self, t: &RichText, max_width: Option<f32>) -> Arc<TextLayout> {
            let size = t.base.size;
            let adv = Self::advance(size);
            let line_h = t.base.line_height.unwrap_or((size * 1.25).round());
            let per_line = max_width.map(|w| ((w / adv).floor() as usize).max(1));
            // each line with the byte offset it starts at
            let mut lines: Vec<String> = Vec::new();
            let mut starts: Vec<usize> = Vec::new();
            let mut at = 0;
            for para in t.text.split('\n') {
                let chars: Vec<char> = para.chars().collect();
                match per_line {
                    Some(n) if chars.len() > n => {
                        let mut b = at;
                        for c in chars.chunks(n) {
                            let s: String = c.iter().collect();
                            starts.push(b);
                            b += s.len();
                            lines.push(s);
                        }
                    }
                    _ => {
                        starts.push(at);
                        lines.push(para.to_owned());
                    }
                }
                at += para.len() + 1;
            }
            if let Some(m) = t.max_lines {
                lines.truncate(m.max(1) as usize);
                starts.truncate(lines.len());
            }
            let mut runs = Vec::new();
            let mut width: f32 = 0.0;
            for (i, line) in lines.iter().enumerate() {
                let glyphs: Vec<Glyph> = line.chars().enumerate().map(|(k, c)| Glyph { id: c as u32, x: k as f32 * adv, y: 0.0 }).collect();
                let advance = glyphs.len() as f32 * adv;
                width = width.max(advance);
                let run = GlyphRun { font: FontId(0), size, variations: 0, glyphs, ascent: size * 0.8, descent: size * 0.2, advance, hint: HintMode::Vertical, aa: AaMode::Grayscale, embolden: 0.0, skew_deg: 0.0 };
                let baseline = i as f32 * line_h + (line_h - size) / 2.0 + size * 0.8;
                runs.push(PlacedRun { run: Arc::new(run), origin: Point::new(0.0, baseline), style: 0 });
            }
            let geometry = MonoGeometry { lines: lines.iter().zip(&starts).map(|(l, s)| (*s, l.clone())).collect(), adv, line_h };
            Arc::new(TextLayout { size: Size::new(width, line_h * lines.len() as f32), runs, lines: lines.len() as u32, geometry: Some(Geometry(Arc::new(geometry))) })
        }
    }

    /// [`MonoShaper`]'s geometry: lines of fixed height, chars of fixed advance.
    struct MonoGeometry {
        /// (start byte, text) per line.
        lines: Vec<(usize, String)>,
        adv: f32,
        line_h: f32,
    }

    impl TextGeometry for MonoGeometry {
        fn hit_byte(&self, p: Point) -> usize {
            if self.lines.is_empty() {
                return 0;
            }
            let i = ((p.y / self.line_h).floor().max(0.0) as usize).min(self.lines.len() - 1);
            let (start, text) = &self.lines[i];
            let col = (p.x / self.adv).round().max(0.0) as usize;
            start + text.char_indices().nth(col).map_or(text.len(), |(b, _)| b)
        }
        fn selection_rects(&self, range: std::ops::Range<usize>) -> Vec<Rect> {
            let mut out = Vec::new();
            for (i, (start, text)) in self.lines.iter().enumerate() {
                let (a, b) = (range.start.max(*start), range.end.min(start + text.len()));
                if a >= b {
                    continue;
                }
                let col = |byte: usize| text[..byte - start].chars().count() as f32;
                let y = i as f32 * self.line_h;
                out.push(Rect::new(col(a) * self.adv, y, col(b) * self.adv, y + self.line_h));
            }
            out
        }
    }

    /// Resources kept in memory: run ids count up from 1, image ids from 1 per new key.
    #[derive(Default)]
    pub struct MemResources {
        pub runs: Vec<Arc<GlyphRun>>,
        pub images: HashMap<String, (ImageId, ImageData)>,
    }

    impl Resources for MemResources {
        fn run(&mut self, run: &Arc<GlyphRun>) -> RunId {
            if let Some(i) = self.runs.iter().position(|r| Arc::ptr_eq(r, run)) {
                return RunId(i as u64 + 1);
            }
            self.runs.push(run.clone());
            RunId(self.runs.len() as u64)
        }
        fn image(&mut self, key: &str, make: &mut dyn FnMut() -> Option<ImageData>) -> Option<ImageId> {
            if let Some((id, _)) = self.images.get(key) {
                return Some(*id);
            }
            let data = make()?;
            let id = ImageId(self.images.len() as u64 + 1);
            self.images.insert(key.to_owned(), (id, data));
            Some(id)
        }
    }
}
