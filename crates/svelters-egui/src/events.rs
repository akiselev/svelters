//! Event propagation on the scene (EV1: SV9's capture and bubbling, SV6's Rust event filters, SV7's
//! overlay trait; plans/milestones/EV1-events.md, ARCH-SCENE §7.3-§7.5, CADATOMIC.md §2.1-§2.2).
//!
//! **DOM-style events.** Every widget and window has the signals `pointerdown`, `pointerup`,
//! `click`, `dblclick`, `contextmenu`, `wheel`, `keydown`, `keyup`, `focusin` and `focusout`
//! (`kinds::dom_signals!`). They are dispatched along the scene in DOM order: Rust filters first
//! (see below), then the capture listeners from the root down to the target's parent, the target's
//! own capture and plain listeners, and the plain listeners from the parent back up to the root.
//! Svelte's `onclick` / `onclickcapture` (and `on:click|capture`) listen to them; a listener gets
//! the event object (`NativeEvent` in `@svelters/native-runtime`) with `target`, `currentTarget`,
//! `eventPhase`, the point, key or button, and `stopPropagation()`, `stopImmediatePropagation()`,
//! `preventDefault()`. Pointer events and `wheel` carry the point twice: `x`/`y` in the viewport's
//! points and `offsetX`/`offsetY` relative to the target's drawn rect (CV1: [`SceneEvent::local`];
//! a provider part's own rect where the provider has one, else its host's).
//!
//! **When.** egui decides clicks, drags and focus when its pass begins, from the raw input. The
//! dispatch therefore runs in an input hook (an `egui::Plugin`'s `input_hook`), before the pass and
//! before `logic`, against the hit list of the viewport's last pass (what the user saw): a listener
//! runs synchronously through `call_sync` (never during the walk), and `preventDefault` (or a
//! filter's [`Flow::Consume`]) takes the input out before egui sees it:
//!
//! | Event | Target | Default prevented |
//! |---|---|---|
//! | `pointerdown` | the topmost node under the point ([`SceneRef::event_target`]: leaves, provider parts, else the innermost box) | the press (and its release) never reach egui: no click, drag or focus |
//! | `contextmenu` | as `pointerdown`, after it, for the secondary button | as `pointerdown` (no context menu) |
//! | `pointerup` | the node under the point | the click egui would see is cancelled |
//! | `click`, `dblclick` | the innermost node holding the press and the release (primary button) | as `pointerup` |
//! | `wheel` | the node under the pointer | the scroll never reaches egui |
//! | `pointermove` | the node under the pointer (SV50): one per pass, at the last point (moves between button changes are coalesced) | not cancelable: egui always sees the move (hover) |
//! | `keydown`, `keyup` | the focused node, else the viewport's window | the key (and the text it types) never reaches the focused widget |
//! | `focusin`, `focusout` | the node that took or lost focus, after the pass that moved it | not cancelable |
//!
//! Input of a viewport whose pass runs inside another's walk (the legacy runner's immediate
//! viewports for extra windows) is dispatched at the next pass, not cancelable.
//!
//! **Filters (SV6).** Rust sees an event before Svelte does: [`EventFilter`]s registered with
//! [`SceneHandle::register_filter`] for the whole scene, a root or a subtree ([`FilterScope`]), and
//! the [`Behavior::filter`](crate::provider::Behavior::filter) hook of the behaviours (`use:`) on
//! every node of the path, outermost first. A filter returning [`Flow::Consume`] ends the event: no
//! Svelte listener runs and the input does not reach egui. [`EventFilter::after`] sees the outcome
//! after bubbling (observers, tutorials).
//!
//! **Overlays (SV7).** [`Overlay`] paints after the walk of every viewport (or one), on its own
//! layer, with the rects of nodes, anchors and identities (`"<root key>/<anchor>"`) drawn last pass
//! ([`OverlayCx::rect`]); [`SceneHandle::register_overlay`]. Built on SG4's `ScenePass`: it runs
//! from `Embed::show_viewport_passes` (the host's end of each viewport's ui) and, for presented
//! windows, after the window's walk.
//!
//! Writer lane: EVENTS (EV1).

use crate::backend::{EguiBackend, Inner};
use crate::kinds;
use crate::provider::{PassCx, PassHandle, PassScope, ScenePass};
use crate::scene::{NodeFlags, NodeId, RootIx, Scene, SceneHandle, SceneRef, Target};
use crate::walk::Out;
use egui::{Context, Event, Id, Modifiers, PointerButton, Pos2, RawInput, Vec2, ViewportId};
use std::cell::RefCell;
use std::rc::{Rc, Weak};
use svelters_abi::{SVL_LISTEN_CAPTURE, SVL_SIGNAL_FLAGS_SHIFT, Value};

// ---- the event ----

/// Which DOM-style event (the signal of the same name).
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EventKind {
    PointerDown,
    PointerUp,
    /// CV1 (SV50): the pointer moved over the node (one per pass).
    PointerMove,
    Click,
    DoubleClick,
    ContextMenu,
    Wheel,
    KeyDown,
    KeyUp,
    FocusIn,
    FocusOut,
    /// A widget's or behaviour's own user-visible event, by its signal name (`clicked`, `toggled`,
    /// `picked`): a unit's intent dispatched with `WidgetCx::dispatch` / `BehaviorCx::dispatch`
    /// before the widget's default action (ARCH-WIDGETS §1.2). Rust filters only; its JS listener
    /// gets it after the pass, as an emitted event.
    Signal(&'static str),
}

impl EventKind {
    /// The signal name (`pointerdown`, `click`, `keydown`, ...).
    pub fn name(self) -> &'static str {
        match self {
            EventKind::PointerDown => "pointerdown",
            EventKind::PointerUp => "pointerup",
            EventKind::PointerMove => "pointermove",
            EventKind::Click => "click",
            EventKind::DoubleClick => "dblclick",
            EventKind::ContextMenu => "contextmenu",
            EventKind::Wheel => "wheel",
            EventKind::KeyDown => "keydown",
            EventKind::KeyUp => "keyup",
            EventKind::FocusIn => "focusin",
            EventKind::FocusOut => "focusout",
            EventKind::Signal(name) => name,
        }
    }

    fn pointer(self) -> bool {
        matches!(self, EventKind::PointerDown | EventKind::PointerUp | EventKind::PointerMove | EventKind::Click | EventKind::DoubleClick | EventKind::ContextMenu)
    }
}

/// Where along its path an event is (DOM `eventPhase`).
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventPhase {
    /// At an ancestor of the target, on the way down (filters, capture listeners).
    Capture = 1,
    /// At the target.
    Target = 2,
    /// At an ancestor, on the way back up (plain listeners).
    Bubble = 3,
}

/// One DOM-style event as Rust filters see it.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub struct SceneEvent {
    pub kind: EventKind,
    /// The node it is for, or a part a `Caps::HIT_TEST` provider reported under the point.
    pub target: Target,
    /// The target as Svelte sees it: `target.node()`, or for a node inside a shadow tree (a
    /// widget's node part: a slider's thumb, a scrollbar, a select's popup) the host element of
    /// the outermost shadow tree (the DOM's shadow-tree retargeting). Listeners never see parts.
    pub light_target: NodeId,
    /// The node the filter runs at (`target.node()` or an ancestor).
    pub current: NodeId,
    pub phase: EventPhase,
    pub viewport: ViewportId,
    /// Pointer events and `wheel`: the point (screen points of the viewport).
    pub pos: Option<Pos2>,
    /// CV1: pointer events and `wheel`: the point relative to the top left of the target's rect
    /// drawn last pass (a provider part's rect where it has one, else its host's): DOM `offsetX`,
    /// `offsetY`. Filled at dispatch.
    pub local: Option<Pos2>,
    /// Pointer events: the button (`None` for `pointermove`).
    pub button: Option<PointerButton>,
    /// CV1: pointer events: the buttons held, DOM `buttons` bits (1 primary, 2 secondary, 4
    /// middle, 8 back, 16 forward).
    pub buttons: u8,
    /// `wheel`: the scroll in points, DOM direction (positive `y`: scrolling down).
    pub delta: Vec2,
    /// Key events: the key, and whether it is an auto-repeat.
    pub key: Option<egui::Key>,
    pub repeat: bool,
    pub modifiers: Modifiers,
    /// Focus events: the node focus came from (`focusin`) or went to (`focusout`).
    pub related: Option<NodeId>,
    /// Whether the default can be prevented (input taken out before egui sees it).
    pub cancelable: bool,
}

impl SceneEvent {
    fn new(kind: EventKind, target: Target, viewport: ViewportId) -> SceneEvent {
        SceneEvent {
            kind,
            target,
            light_target: target.node(),
            current: target.node(),
            phase: EventPhase::Target,
            viewport,
            pos: None,
            local: None,
            button: None,
            buttons: 0,
            delta: Vec2::ZERO,
            key: None,
            repeat: false,
            modifiers: Modifiers::NONE,
            related: None,
            cancelable: !matches!(kind, EventKind::FocusIn | EventKind::FocusOut),
        }
    }

    /// The signal name.
    pub fn name(&self) -> &'static str {
        self.kind.name()
    }

    /// The target node (the host of a provider part).
    pub fn target_node(&self) -> NodeId {
        self.target.node()
    }
}

/// What a filter decides.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Flow {
    /// Let the event go on (to the next filter, then Svelte's listeners and egui).
    #[default]
    Pass,
    /// Stop the propagation: no further filter or Svelte listener sees it; the default still
    /// happens (egui gets the input, the widget applies its action).
    Stop,
    /// Prevent the default (the input is taken out before egui sees it; a widget skips its
    /// action); propagation goes on, listeners see `defaultPrevented`.
    Prevent,
    /// Both: the event ends here and, when cancelable, has no default.
    Consume,
}

impl Flow {
    /// `Stop` or `Consume`.
    pub fn stops(self) -> bool {
        matches!(self, Flow::Stop | Flow::Consume)
    }
    /// `Prevent` or `Consume`.
    pub fn prevents(self) -> bool {
        matches!(self, Flow::Prevent | Flow::Consume)
    }
}

/// How an event ended: what `WidgetCx::dispatch` returns, what [`EventFilter::after`] sees.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    /// A Rust filter consumed it ([`Flow::Consume`]).
    pub consumed: bool,
    /// Propagation stopped: a filter's `Stop` or `Consume`, or a listener's `stopPropagation()`.
    pub stopped: bool,
    /// The default was prevented (a filter's `Prevent` or `Consume`, a listener's
    /// `preventDefault()`); only for cancelable events.
    pub prevented: bool,
    /// How many Svelte listener sets ran (one `call_sync` each).
    pub listeners: u32,
}

impl Outcome {
    /// Skip the default action (a widget's activation, egui's handling of the input).
    pub fn prevented(&self) -> bool {
        self.prevented
    }

    /// Records a filter's answer; true when propagation ends.
    fn note(&mut self, f: Flow, cancelable: bool) -> bool {
        if f.prevents() && cancelable {
            self.prevented = true;
        }
        if f == Flow::Consume {
            self.consumed = true;
        }
        if f.stops() {
            self.stopped = true;
        }
        f.stops()
    }
}

// ---- filters (SV6) ----

/// Which events a filter sees.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterScope {
    /// Every event of the scene, first.
    All,
    /// Events whose target is in root `r` (an embed slot's root, a window's), after `All`.
    Root(RootIx),
    /// Events whose path passes `node` (it or a descendant is the target), when the dispatch
    /// reaches it on the way down.
    Subtree(NodeId),
}

/// A Rust event filter (Qt's `eventFilter` shape, SV6): sees each event before Svelte's listeners
/// and may stop, prevent or consume it. A `Subtree` filter is called on the way down (`ev.phase`
/// `Capture`, or `Target` at its node) and, unless the event stopped, on the way back up
/// (`Bubble`): check `ev.phase` to act once. Runs on the UI thread, before the frame's pass for
/// input events, during the walk for a widget's own events ([`EventKind::Signal`]); reads the
/// scene only. Closures `FnMut(&SceneEvent, SceneRef) -> Flow` are filters.
pub trait EventFilter: 'static {
    fn filter(&mut self, ev: &SceneEvent, scene: SceneRef<'_>) -> Flow;
    /// After the dispatch (and bubbling) ended, with its outcome: every filter that saw the event.
    fn after(&mut self, _ev: &SceneEvent, _outcome: &Outcome, _scene: SceneRef<'_>) {}
}

impl<F: FnMut(&SceneEvent, SceneRef<'_>) -> Flow + 'static> EventFilter for F {
    fn filter(&mut self, ev: &SceneEvent, scene: SceneRef<'_>) -> Flow {
        self(ev, scene)
    }
}

struct FilterEntry {
    id: u64,
    scope: FilterScope,
    f: Rc<RefCell<dyn EventFilter>>,
}

/// The filters of one scene (in its cell).
#[derive(Default)]
pub(crate) struct Registry {
    next: u64,
    filters: Vec<FilterEntry>,
}

/// A registered filter; dropping it keeps the filter registered.
#[derive(Clone)]
pub struct FilterHandle {
    id: u64,
    reg: Weak<RefCell<Registry>>,
}

impl FilterHandle {
    /// Removes the filter: it sees no further event.
    pub fn unregister(&self) {
        if let Some(r) = self.reg.upgrade() {
            r.borrow_mut().filters.retain(|e| e.id != self.id);
        }
    }
}

impl SceneHandle {
    /// SV6: registers a Rust event filter for `scope` (see [`EventFilter`]). Filters of one scope
    /// run in registration order.
    pub fn register_filter(&self, scope: FilterScope, f: impl EventFilter) -> FilterHandle {
        let mut r = self.cell().events.borrow_mut();
        r.next += 1;
        let id = r.next;
        r.filters.push(FilterEntry { id, scope, f: Rc::new(RefCell::new(f)) });
        FilterHandle { id, reg: Rc::downgrade(&self.cell().events) }
    }

    /// Registered event filters.
    pub fn filter_count(&self) -> usize {
        self.cell().events.borrow().filters.len()
    }

    /// SV7: registers an [`Overlay`] (a `ScenePass` over every viewport, or the one it names).
    pub fn register_overlay<O: Overlay>(&self, o: O) -> OverlayHandle<O> {
        OverlayHandle(self.register_pass(OverlayPass(o)))
    }
}

// ---- overlays (SV7) ----

/// What an overlay asks the rect of (`OverlayCx::rect`): a node, a target (a provider's part), an
/// anchor in a root, or an identity path `"<root key>/<anchor>"` (`SceneRef::target_by_path`).
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum At {
    Node(NodeId),
    Target(Target),
    Anchor(RootIx, String),
    Path(String),
}

impl From<NodeId> for At {
    fn from(n: NodeId) -> At {
        At::Node(n)
    }
}

impl From<Target> for At {
    fn from(t: Target) -> At {
        At::Target(t)
    }
}

impl From<&str> for At {
    fn from(p: &str) -> At {
        At::Path(p.to_string())
    }
}

impl From<String> for At {
    fn from(p: String) -> At {
        At::Path(p)
    }
}

/// SV7: paints over the scene after the walk (tutorial highlights, pick previews, badges), on its
/// own click-through layer. Frames come from the usual paths (`OverlayCx::request_repaint`).
pub trait Overlay: 'static {
    /// The viewport it paints in; `None` for every viewport.
    fn viewport(&self) -> Option<ViewportId> {
        None
    }
    /// The layer it paints on.
    fn order(&self) -> egui::Order {
        egui::Order::Foreground
    }
    /// Paints this viewport's overlay; `cx` resolves rects from the last pass.
    fn paint(&mut self, cx: &mut OverlayCx<'_>);
}

/// What [`Overlay::paint`] gets.
pub struct OverlayCx<'a> {
    ui: &'a mut egui::Ui,
    scene: SceneRef<'a>,
    viewport: ViewportId,
}

impl<'a> OverlayCx<'a> {
    /// The overlay's `Ui` (its layer, the viewport's whole rect).
    pub fn ui(&mut self) -> &mut egui::Ui {
        self.ui
    }
    pub fn painter(&self) -> &egui::Painter {
        self.ui.painter()
    }
    /// The scene, read-only.
    pub fn scene(&self) -> SceneRef<'a> {
        self.scene
    }
    pub fn viewport(&self) -> ViewportId {
        self.viewport
    }
    /// The target `at` names, if it exists.
    pub fn resolve(&self, at: impl Into<At>) -> Option<Target> {
        match at.into() {
            At::Node(n) => self.scene.get(n).map(|_| Target::Node(n)),
            At::Target(t) => Some(t),
            At::Anchor(r, a) => self.scene.target_by_anchor(r, &a),
            At::Path(p) => self.scene.target_by_path(&p),
        }
    }
    /// Where `at` was drawn last pass in this viewport (screen points): a widget's rect, a box's
    /// layout rect, a provider part's row rect. `None` when it is not drawn here.
    pub fn rect(&self, at: impl Into<At>) -> Option<egui::Rect> {
        let t = self.resolve(at)?;
        if let Some(r) = self.scene.box_rect(self.viewport, t.node()).filter(|_| matches!(t, Target::Node(_))) {
            return Some(r);
        }
        let (vp, r) = self.scene.target_rect(t)?;
        (vp == self.viewport).then_some(r)
    }
    /// What is under the pointer (the target a click there would have), if anything.
    pub fn hovered(&self) -> Option<Target> {
        let pos = self.ui.ctx().pointer_hover_pos()?;
        self.scene.event_target(self.viewport, pos)
    }
    /// Another pass of this viewport.
    pub fn request_repaint(&self) {
        self.ui.ctx().request_repaint_of(self.viewport);
    }
    /// A pass of this viewport after `d` (an animated highlight).
    pub fn request_repaint_after(&self, d: std::time::Duration) {
        self.ui.ctx().request_repaint_after_for(d, self.viewport);
    }
}

/// An [`Overlay`] as SG4's `ScenePass`.
pub struct OverlayPass<O>(O);

impl<O: Overlay> ScenePass for OverlayPass<O> {
    fn scope(&self) -> PassScope {
        self.0.viewport().map_or(PassScope::AllViewports, PassScope::Viewport)
    }
    fn order(&self) -> egui::Order {
        self.0.order()
    }
    fn overlay(&mut self, ui: &mut egui::Ui, scene: SceneRef<'_>, cx: &mut PassCx<'_>) {
        let mut ocx = OverlayCx { ui, scene, viewport: cx.viewport() };
        self.0.paint(&mut ocx);
    }
}

/// A registered [`Overlay`]. Cloning shares it.
pub struct OverlayHandle<O>(PassHandle<OverlayPass<O>>);

impl<O> Clone for OverlayHandle<O> {
    fn clone(&self) -> Self {
        OverlayHandle(self.0.clone())
    }
}

impl<O: Overlay> OverlayHandle<O> {
    pub fn with<R>(&self, f: impl FnOnce(&O) -> R) -> R {
        self.0.with(|p| f(&p.0))
    }
    /// Changes the overlay (its next paint sees it; ask for a repaint if it must show now).
    pub fn with_mut<R>(&self, f: impl FnOnce(&mut O) -> R) -> R {
        self.0.with_mut(|p| f(&mut p.0))
    }
    pub fn unregister(&self) {
        self.0.unregister();
    }
}

// ---- the dispatch state of a backend ----

const BUTTONS: usize = 5;
/// The bits a listener set answers through `call_sync` (the runtime's `NativeEvent` of a
/// propagating event): stopPropagation, preventDefault, stopImmediatePropagation.
const STOP: i64 = 1;
const PREVENT: i64 = 2;
const IMMEDIATE: i64 = 4;
/// Added to the `phase` argument: a listener before prevented the default / not cancelable.
const PHASE_PREVENTED: i64 = 8;
const PHASE_NOT_CANCELABLE: i64 = 16;

#[derive(Clone, Copy, Debug)]
struct Press {
    target: Option<NodeId>,
    pos: Pos2,
    /// A filter consumed the press: the gesture is the filter's (its release is dropped too).
    consumed: bool,
    /// A listener prevented its default: egui never saw the press, so the release goes too.
    prevented: bool,
}

#[derive(Debug)]
struct Vp {
    vp: ViewportId,
    press: [Option<Press>; BUTTONS],
    last_click: Option<(NodeId, f64, Pos2)>,
    pointer: Option<Pos2>,
    /// egui's focus at the end of the last pass, and its node in this scene.
    focus: Option<Id>,
    focused: Option<NodeId>,
    /// A keydown was prevented: the text it types (the next `Text` event, in this input or a
    /// later one) is taken out too.
    eat_text: bool,
    /// SV50: the buttons held (DOM `buttons` bits), from the presses and releases seen.
    buttons: u8,
}

impl Vp {
    fn new(vp: ViewportId) -> Vp {
        Vp { vp, press: [None; BUTTONS], last_click: None, pointer: None, focus: None, focused: None, eat_text: false, buttons: 0 }
    }
}

/// Per backend (`Inner::ev`).
#[derive(Default)]
pub(crate) struct BackendState {
    vps: Vec<Vp>,
    /// Events to dispatch at the next input hook that may run JS (not cancelable): focus changes,
    /// input of a viewport whose pass ran inside another's walk.
    late: Vec<SceneEvent>,
}

impl BackendState {
    fn vp(&mut self, vp: ViewportId) -> &mut Vp {
        match self.vps.iter().position(|v| v.vp == vp) {
            Some(k) => &mut self.vps[k],
            None => {
                self.vps.push(Vp::new(vp));
                self.vps.last_mut().unwrap()
            }
        }
    }
}

// ---- the input hook ----

thread_local! {
    /// The backends of this thread by serial (their `Inner`'s address), for the plugin.
    static BACKENDS: RefCell<Vec<(usize, Weak<Inner>)>> = const { RefCell::new(Vec::new()) };
}

fn list_id() -> Id {
    Id::unique("svelters-egui/events/backends")
}

/// Makes the input hook dispatch `b`'s events in `ctx` (once per context and backend; cheap after).
pub(crate) fn install(b: &EguiBackend, ctx: &Context) {
    let serial = Rc::as_ptr(&b.0) as usize;
    let mut list: Vec<usize> = ctx.data(|d| d.get_temp::<Vec<usize>>(list_id())).unwrap_or_default();
    if list.contains(&serial) {
        return;
    }
    list.push(serial);
    ctx.data_mut(|d| d.insert_temp(list_id(), list));
    BACKENDS.with(|v| {
        let mut v = v.borrow_mut();
        v.retain(|(_, w)| w.strong_count() > 0);
        if !v.iter().any(|(s, _)| *s == serial) {
            v.push((serial, Rc::downgrade(&b.0)));
        }
    });
    ctx.add_plugin(EventsPlugin); // once per context (egui ignores a second one)
}

fn backends(ctx: &Context) -> Vec<EguiBackend> {
    let Some(list) = ctx.data(|d| d.get_temp::<Vec<usize>>(list_id())) else { return Vec::new() };
    BACKENDS.with(|v| v.borrow().iter().filter(|(s, _)| list.contains(s)).filter_map(|(_, w)| w.upgrade().map(EguiBackend)).collect())
}

/// The egui plugin: the input hook (dispatch before the pass) and the end of each pass (focus).
struct EventsPlugin;

impl egui::Plugin for EventsPlugin {
    fn debug_name(&self) -> &'static str {
        "svelters-events"
    }

    fn input_hook(&mut self, ctx: &Context, input: &mut RawInput) {
        for b in backends(ctx) {
            b.dispatch_input(ctx, input);
        }
        if crate::frame::exp_instant_wheel() {
            instant_wheel(ctx, input);
        }
    }

    fn on_end_pass(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        for b in backends(&ctx) {
            b.track_focus(&ctx);
        }
    }
}

/// Latency experiment (`SVELTERS_EXP_INSTANT_WHEEL`): egui eases a mouse wheel's line steps in over
/// about 100 ms (`WheelState::after_events`) but applies touch-phase deltas at once. Each line or
/// page step becomes a `Start` and a point `Move`, so every scrollable (egui's and ours) moves in
/// the frame that got the notch.
fn instant_wheel(ctx: &Context, input: &mut RawInput) {
    use egui::{MouseWheelSource, MouseWheelUnit, TouchPhase};
    let speed = ctx.options(|o| o.input_options.line_scroll_speed);
    let page = input.screen_rect.map_or(0.0, |r| r.height());
    let events = std::mem::take(&mut input.events);
    for e in events {
        match e {
            Event::MouseWheel { unit, delta, phase: TouchPhase::Move, source: MouseWheelSource::Wheel, modifiers } if unit != MouseWheelUnit::Point => {
                let d = if unit == MouseWheelUnit::Line { delta * speed } else { delta * page };
                let wheel = |phase, delta| Event::MouseWheel { unit: MouseWheelUnit::Point, delta, phase, source: MouseWheelSource::Wheel, modifiers };
                input.events.push(wheel(TouchPhase::Start, Vec2::ZERO));
                input.events.push(wheel(TouchPhase::Move, d));
            }
            e => input.events.push(e),
        }
    }
}

fn dom_button(b: PointerButton) -> i64 {
    match b {
        PointerButton::Primary => 0,
        PointerButton::Middle => 1,
        PointerButton::Secondary => 2,
        PointerButton::Extra1 => 3,
        PointerButton::Extra2 => 4,
    }
}

/// DOM `buttons` bit of `b`.
fn dom_buttons_bit(b: PointerButton) -> u8 {
    match b {
        PointerButton::Primary => 1,
        PointerButton::Secondary => 2,
        PointerButton::Middle => 4,
        PointerButton::Extra1 => 8,
        PointerButton::Extra2 => 16,
    }
}

fn modifier_bits(m: Modifiers) -> i64 {
    (m.shift as i64) | (m.ctrl as i64) << 1 | (m.alt as i64) << 2 | (m.mac_cmd as i64) << 3
}

/// The path of `node`: the root first, `node` last (through shadow hosts; attached nodes are
/// never targets). Empty if `node` is not live.
fn path_of(s: &Scene, node: NodeId) -> Vec<NodeId> {
    let mut v = Vec::new();
    let mut at = Some(node);
    while let Some(n) = at {
        let Some(i) = s.idx(n) else { break };
        v.push(n);
        at = s.core[i].parent;
    }
    v.reverse();
    v
}

/// The index in `path` of the target as Svelte sees it: the host of the outermost shadow tree
/// the target is in (DOM retargeting), else the target.
fn light_index(s: &Scene, path: &[NodeId]) -> usize {
    path.iter().position(|&n| s.idx(n).is_some_and(|i| s.core[i].flags.contains(NodeFlags::SHADOW))).map_or(path.len() - 1, |k| k.saturating_sub(1))
}

type FilterRc = Rc<RefCell<dyn EventFilter>>;

/// The Rust half of a dispatch, over a read-only scene: the `All` and `Root` filters at the root,
/// then on each node of the path the `Subtree` filters and the behaviours' `filter`, root to
/// target (capture, target) and back up (bubble), until one stops it. The behaviours' writes are
/// returned to be applied (`behavior::apply`), with the filters that saw it (for `after`).
fn rust_walk(s: &Scene, ev: &mut SceneEvent, path: &[NodeId]) -> (Outcome, Vec<crate::provider::behavior::HookRun>, Vec<FilterRc>) {
    let mut o = Outcome::default();
    let mut runs = Vec::new();
    let mut ran: Vec<FilterRc> = Vec::new();
    let n = path.len();
    let target = path[n - 1];
    let root = s.core[s.ix(target)].root;
    let filters: Vec<(FilterScope, FilterRc)> = s.filters.borrow().filters.iter().map(|e| (e.scope, e.f.clone())).collect();
    let call = |f: &FilterRc, ev: &SceneEvent, ran: &mut Vec<FilterRc>| -> Flow {
        let Ok(mut fb) = f.try_borrow_mut() else { return Flow::Pass };
        if !ran.iter().any(|r| Rc::ptr_eq(r, f)) {
            ran.push(f.clone());
        }
        fb.filter(ev, SceneRef::new(s))
    };
    (ev.current, ev.phase) = (path[0], if n == 1 { EventPhase::Target } else { EventPhase::Capture });
    for (scope, f) in &filters {
        let at_root = match *scope {
            FilterScope::All => true,
            FilterScope::Root(r) => r == root,
            FilterScope::Subtree(_) => false,
        };
        if at_root && o.note(call(f, ev, &mut ran), ev.cancelable) {
            return (o, runs, ran);
        }
    }
    let steps = path[..n - 1].iter().map(|&x| (x, EventPhase::Capture)).chain(std::iter::once((target, EventPhase::Target))).chain(path[..n - 1].iter().rev().map(|&x| (x, EventPhase::Bubble)));
    for (node, phase) in steps {
        (ev.current, ev.phase) = (node, phase);
        for (scope, f) in &filters {
            if *scope == FilterScope::Subtree(node) && o.note(call(f, ev, &mut ran), ev.cancelable) {
                return (o, runs, ran);
            }
        }
        let Some(i) = s.idx(node) else { continue };
        for flow in crate::provider::behavior::filter(s, i, ev, &mut runs) {
            if o.note(flow, ev.cancelable) {
                return (o, runs, ran);
            }
        }
    }
    (o, runs, ran)
}

/// Every filter that saw `ev` gets the outcome.
fn after(s: &Scene, ran: Vec<FilterRc>, ev: &SceneEvent, o: &Outcome) {
    for f in ran {
        if let Ok(mut fb) = f.try_borrow_mut() {
            fb.after(ev, o, SceneRef::new(s));
        }
    }
}

/// EV1 (ARCH-WIDGETS §1.2): a widget's or behaviour's own event `name` from `node`, through the
/// Rust filters before its default action. Returns the outcome and the behaviours' writes to apply.
pub(crate) fn dispatch_signal(s: &Scene, node: NodeId, name: &'static str) -> (Outcome, Vec<crate::provider::behavior::HookRun>) {
    let path = path_of(s, node);
    if path.is_empty() {
        return (Outcome::default(), Vec::new());
    }
    let vp = s.roots.get(s.core[s.ix(node)].root).map_or(ViewportId::ROOT, |e| e.viewport);
    let mut ev = SceneEvent::new(EventKind::Signal(name), Target::Node(node), vp);
    ev.light_target = path[light_index(s, &path)];
    let (o, runs, ran) = rust_walk(s, &mut ev, &path);
    (ev.current, ev.phase) = (node, EventPhase::Target);
    after(s, ran, &ev, &o);
    (o, runs)
}

impl EguiBackend {
    fn target_at(&self, vp: ViewportId, pos: Pos2) -> Option<Target> {
        let s = self.0.scene.try_borrow().ok()?;
        SceneRef::new(&s).event_target(vp, pos)
    }

    /// The innermost common ancestor (inclusive) of `a` and `b`.
    fn common_ancestor(&self, a: NodeId, b: NodeId) -> Option<NodeId> {
        let s = self.0.scene.borrow();
        let pa = path_of(&s, a);
        path_of(&s, b).into_iter().rev().find(|n| pa.contains(n))
    }

    /// The window drawn in viewport `vp` (the legacy runner's windows), the target of keys when
    /// no node has focus.
    fn window_of(&self, vp: ViewportId) -> Option<NodeId> {
        let s = self.0.scene.borrow();
        let ws: Vec<_> = self.0.windows.borrow().iter().copied().filter(|&w| s.contains(w)).collect();
        let w = ws.iter().enumerate().find(|&(k, &w)| if k == 0 { vp == ViewportId::ROOT } else { vp == ViewportId::from_hash_of(("svl-window", w)) })?.1;
        s.live(*w).ok()
    }

    /// Dispatches `ev` now, or queues it (not cancelable) when JS cannot run (`nested`).
    fn fire(&self, ctx: &Context, nested: bool, ev: SceneEvent) -> Outcome {
        if nested {
            if self.wanted(&ev) {
                self.0.ev.borrow_mut().late.push(SceneEvent { cancelable: false, ..ev });
                ctx.request_repaint();
            }
            return Outcome::default();
        }
        self.dispatch(ctx, ev)
    }

    /// Whether anything would see `ev`: a filter, a behaviour or a listener on its path.
    fn wanted(&self, ev: &SceneEvent) -> bool {
        if !self.0.scene.cell().events.borrow().filters.is_empty() {
            return true;
        }
        let Ok(s) = self.0.scene.try_borrow() else { return true };
        path_of(&s, ev.target.node()).into_iter().any(|n| {
            let i = s.ix(n);
            let sig = kinds::signal_id(s.core[i].kind as usize, ev.kind.name());
            s.core[i].flags.contains(NodeFlags::HAS_ATTACHED) || s.listens(i, sig) || s.listens_capture(i, sig)
        })
    }

    /// The input hook: dispatches this viewport's pointer, wheel and key input before egui's pass
    /// begins, and takes out what was consumed or default-prevented.
    pub(crate) fn dispatch_input(&self, ctx: &Context, input: &mut RawInput) {
        let vp = input.viewport_id;
        if self.0.scene.try_borrow_mut().is_err() {
            return; // inside the walk of this scene: nothing to hit-test against now
        }
        // a viewport whose pass runs inside another's walk (an immediate viewport): no JS now
        let nested = self.0.pass_active.get();
        if !nested {
            self.deliver_late(ctx);
        }
        let time = input.time.unwrap_or_else(|| ctx.input_for(vp, |i| i.time) + input.predicted_dt as f64);
        let max_click_dist = ctx.options(|o| o.input_options.max_click_dist);
        let dbl_delay = ctx.options(|o| o.input_options.max_double_click_delay);
        // SV50: the moves since the last button change, dispatched as one `pointermove`
        let mut moved: Option<Pos2> = None;
        // the modifiers held: last pass's, then the latest an event of this input carries
        let mut mods = ctx.input_for(vp, |i| i.modifiers);
        let mut k = 0;
        while k < input.events.len() {
            if moved.is_some() && matches!(input.events[k], Event::PointerButton { .. } | Event::MouseWheel { .. } | Event::PointerGone) {
                self.pointer_move(ctx, nested, vp, moved.take(), mods);
            }
            if let Event::ModifiersChanged(modifiers) | Event::Key { modifiers, .. } | Event::PointerButton { modifiers, .. } | Event::MouseWheel { modifiers, .. } = &input.events[k] {
                mods = *modifiers;
            }
            match input.events[k].clone() {
                Event::PointerMoved(pos) => {
                    self.0.ev.borrow_mut().vp(vp).pointer = Some(pos);
                    moved = Some(pos);
                }
                Event::PointerGone => self.0.ev.borrow_mut().vp(vp).pointer = None,
                Event::PointerButton { pos, button, pressed: true, modifiers } => {
                    {
                        let mut st = self.0.ev.borrow_mut();
                        let v = st.vp(vp);
                        v.pointer = Some(pos);
                        v.buttons |= dom_buttons_bit(button);
                    }
                    let target = self.target_at(vp, pos);
                    let mut press = Press { target: target.map(Target::node), pos, consumed: false, prevented: false };
                    if let Some(t) = target {
                        let mut ev = SceneEvent::new(EventKind::PointerDown, t, vp);
                        (ev.pos, ev.button, ev.modifiers) = (Some(pos), Some(button), modifiers);
                        ev.buttons = self.0.ev.borrow_mut().vp(vp).buttons;
                        let o = self.fire(ctx, nested, ev.clone());
                        (press.consumed, press.prevented) = (o.consumed, o.prevented);
                        if button == PointerButton::Secondary && !o.consumed {
                            ev.kind = EventKind::ContextMenu;
                            let o = self.fire(ctx, nested, ev);
                            press.consumed |= o.consumed;
                            press.prevented |= o.prevented;
                        }
                    }
                    self.0.ev.borrow_mut().vp(vp).press[button as usize] = Some(press);
                    if press.consumed || press.prevented {
                        input.events.remove(k);
                        continue;
                    }
                }
                Event::PointerButton { pos, button, pressed: false, modifiers } => {
                    let press = {
                        let mut st = self.0.ev.borrow_mut();
                        let v = st.vp(vp);
                        v.pointer = Some(pos);
                        v.buttons &= !dom_buttons_bit(button);
                        v.press[button as usize].take()
                    };
                    if press.is_some_and(|p| p.consumed) {
                        input.events.remove(k);
                        continue;
                    }
                    let target = self.target_at(vp, pos);
                    let mut cancel = false;
                    let mut consumed = false;
                    let held = self.0.ev.borrow_mut().vp(vp).buttons;
                    let pointer_ev = |kind, t| {
                        let mut ev = SceneEvent::new(kind, t, vp);
                        (ev.pos, ev.button, ev.modifiers, ev.buttons) = (Some(pos), Some(button), modifiers, held);
                        ev
                    };
                    if let Some(t) = target {
                        let o = self.fire(ctx, nested, pointer_ev(EventKind::PointerUp, t));
                        consumed |= o.consumed;
                        cancel |= o.prevented;
                    }
                    // a click: the innermost node holding the press and the release (DOM)
                    if !consumed
                        && button == PointerButton::Primary
                        && let Some(Press { target: Some(a), .. }) = press
                        && let Some(t) = target
                        && let Some(c) = self.common_ancestor(a, t.node())
                    {
                        let ct = if c == t.node() { t } else { Target::Node(c) };
                        let o = self.fire(ctx, nested, pointer_ev(EventKind::Click, ct));
                        consumed |= o.consumed;
                        cancel |= o.prevented;
                        let last = self.0.ev.borrow_mut().vp(vp).last_click.take();
                        let double = last.is_some_and(|(n, t0, p0)| n == c && time - t0 <= dbl_delay && p0.distance(pos) <= max_click_dist);
                        if double && !consumed {
                            let o = self.fire(ctx, nested, pointer_ev(EventKind::DoubleClick, ct));
                            consumed |= o.consumed;
                            cancel |= o.prevented;
                        } else if !double {
                            self.0.ev.borrow_mut().vp(vp).last_click = Some((c, time, pos));
                        }
                    }
                    if consumed || press.is_some_and(|p| p.prevented) {
                        input.events.remove(k);
                        continue;
                    }
                    if cancel && let Some(p) = press {
                        // egui decides the click from this release: a move beyond the click
                        // distance first makes it a release without a click
                        input.events.insert(k, Event::PointerMoved(p.pos + Vec2::splat(max_click_dist.max(1.0) * 1000.0)));
                        k += 1;
                    }
                }
                Event::MouseWheel { unit, delta, modifiers, .. } => {
                    let pos = self.0.ev.borrow_mut().vp(vp).pointer.or_else(|| ctx.input_for(vp, |i| i.pointer.latest_pos()));
                    if let Some(pos) = pos
                        && let Some(t) = self.target_at(vp, pos)
                    {
                        let pts = match unit {
                            egui::MouseWheelUnit::Point => delta,
                            egui::MouseWheelUnit::Line => delta * ctx.options(|o| o.input_options.line_scroll_speed),
                            egui::MouseWheelUnit::Page => delta * input.screen_rect.map_or(0.0, |r| r.height()),
                        };
                        let mut ev = SceneEvent::new(EventKind::Wheel, t, vp);
                        (ev.pos, ev.delta, ev.modifiers) = (Some(pos), -pts, modifiers);
                        let o = self.fire(ctx, nested, ev);
                        if o.consumed || o.prevented {
                            input.events.remove(k);
                            continue;
                        }
                    }
                }
                Event::Text(_) if std::mem::take(&mut self.0.ev.borrow_mut().vp(vp).eat_text) => {
                    input.events.remove(k);
                    continue;
                }
                Event::Key { key, pressed, repeat, modifiers, .. } => {
                    let focused = {
                        let mut st = self.0.ev.borrow_mut();
                        let v = st.vp(vp);
                        if pressed {
                            v.eat_text = false;
                        }
                        v.focused
                    };
                    let live = |n: NodeId| self.0.scene.borrow().idx(n).is_some();
                    let target = focused.filter(|&n| live(n)).or_else(|| self.window_of(vp));
                    if let Some(t) = target {
                        let mut ev = SceneEvent::new(if pressed { EventKind::KeyDown } else { EventKind::KeyUp }, Target::Node(t), vp);
                        (ev.key, ev.repeat, ev.modifiers) = (Some(key), repeat, modifiers);
                        let o = self.fire(ctx, nested, ev);
                        if o.consumed || o.prevented {
                            input.events.remove(k);
                            // and the text the key types (egui-winit sends it after the key)
                            if pressed && !modifiers.command && !modifiers.ctrl && !modifiers.mac_cmd {
                                self.0.ev.borrow_mut().vp(vp).eat_text = true;
                            }
                            continue;
                        }
                    }
                }
                _ => {}
            }
            k += 1;
        }
        self.pointer_move(ctx, nested, vp, moved, mods);
    }

    /// SV50: one `pointermove` at `pos` (the last point of a run of moves) to the node under it,
    /// when anything on its path would see it. Not cancelable: egui always sees the move.
    fn pointer_move(&self, ctx: &Context, nested: bool, vp: ViewportId, pos: Option<Pos2>, modifiers: Modifiers) {
        let Some(pos) = pos else { return };
        let Some(t) = self.target_at(vp, pos) else { return };
        let mut ev = SceneEvent::new(EventKind::PointerMove, t, vp);
        (ev.pos, ev.modifiers, ev.cancelable) = (Some(pos), modifiers, false);
        ev.buttons = self.0.ev.borrow_mut().vp(vp).buttons;
        if self.wanted(&ev) {
            self.fire(ctx, nested, ev);
        }
    }

    /// The end of each pass: focus that moved to or from one of this scene's nodes queues
    /// `focusout` and `focusin` (dispatched before the next pass, which it asks for when anything
    /// would see them).
    pub(crate) fn track_focus(&self, ctx: &Context) {
        let vp = ctx.viewport_id();
        let id = ctx.memory(|m| m.focused());
        if self.0.ev.borrow_mut().vp(vp).focus == id {
            return;
        }
        let Ok(s) = self.0.scene.try_borrow() else { return }; // retried at the next pass
        let node = id.and_then(|id| s.ids().find(|&n| s.core[s.ix(n)].egui_id == id && !s.core[s.ix(n)].flags.contains(NodeFlags::ATTACHED)));
        drop(s);
        let old = {
            let mut st = self.0.ev.borrow_mut();
            let v = st.vp(vp);
            v.focus = id;
            std::mem::replace(&mut v.focused, node)
        };
        if old == node {
            return;
        }
        let mut evs = Vec::new();
        if let Some(o) = old.filter(|&o| self.0.scene.borrow().idx(o).is_some()) {
            let mut ev = SceneEvent::new(EventKind::FocusOut, Target::Node(o), vp);
            ev.related = node;
            evs.push(ev);
        }
        if let Some(n) = node {
            let mut ev = SceneEvent::new(EventKind::FocusIn, Target::Node(n), vp);
            ev.related = old;
            evs.push(ev);
        }
        let mut any = false;
        for ev in evs {
            if self.wanted(&ev) {
                self.0.ev.borrow_mut().late.push(ev);
                any = true;
            }
        }
        if any {
            ctx.request_repaint_of(vp);
        }
    }

    fn deliver_late(&self, ctx: &Context) {
        let late = std::mem::take(&mut self.0.ev.borrow_mut().late);
        for ev in late {
            if self.0.scene.borrow().idx(ev.target.node()).is_some() {
                self.dispatch(ctx, ev);
            }
        }
    }

    /// One event along its path: the Rust filters and behaviours (capture, target, bubble), then
    /// the Svelte listeners in DOM order up to the light target, then every filter's `after`.
    fn dispatch(&self, ctx: &Context, mut ev: SceneEvent) -> Outcome {
        let (path, light, mut o, runs, ran) = {
            let s = self.0.scene.borrow();
            let path = path_of(&s, ev.target.node());
            if path.is_empty() {
                return Outcome::default();
            }
            if let Some(pos) = ev.pos.filter(|_| ev.local.is_none()) {
                ev.local = local_point(SceneRef::new(&s), ev.viewport, ev.target, pos);
            }
            let light = light_index(&s, &path);
            ev.light_target = path[light];
            let (o, runs, ran) = rust_walk(&s, &mut ev, &path);
            (path, light, o, runs, ran)
        };
        if !runs.is_empty() {
            let mut out = Vec::new();
            {
                let mut s = self.0.scene.borrow_mut();
                for r in runs {
                    crate::provider::behavior::apply(&mut s, r, Some(ctx), &mut out);
                }
            }
            self.post_out(ctx, out);
        }
        (ev.current, ev.phase) = (ev.target.node(), EventPhase::Target);
        if !o.stopped {
            self.dispatch_js(&ev, &path[..=light], &mut o);
        }
        if let Ok(s) = self.0.scene.try_borrow() {
            after(&s, ran, &ev, &o);
        }
        o
    }

    /// The Svelte listeners: capture from the root down, the target's capture and plain listeners,
    /// plain from the parent up; each listener set is one `call_sync` whose answer carries
    /// stopPropagation / preventDefault / stopImmediatePropagation.
    fn dispatch_js(&self, ev: &SceneEvent, path: &[NodeId], o: &mut Outcome) {
        let Some(host) = self.host() else { return };
        let n = path.len();
        let target = path[n - 1];
        let mut steps: Vec<(NodeId, EventPhase, bool)> = Vec::with_capacity(2 * n);
        steps.extend(path[..n - 1].iter().map(|&x| (x, EventPhase::Capture, true)));
        steps.push((target, EventPhase::Target, true));
        steps.push((target, EventPhase::Target, false));
        steps.extend(path[..n - 1].iter().rev().map(|&x| (x, EventPhase::Bubble, false)));
        let mut stopped_at: Option<NodeId> = None;
        let mut immediate_at: Option<NodeId> = None;
        for (node, phase, capture) in steps {
            if stopped_at.is_some_and(|s| s != node) {
                break;
            }
            if immediate_at == Some(node) {
                continue;
            }
            let sig = {
                let s = self.0.scene.borrow();
                let Some(i) = s.idx(node) else { continue };
                let sig = kinds::signal_id(s.core[i].kind as usize, ev.kind.name());
                let on = if capture { s.listens_capture(i, sig) } else { s.listens(i, sig) };
                if !on {
                    continue;
                }
                sig
            };
            let flags = if capture { SVL_LISTEN_CAPTURE } else { 0 };
            let args = js_args(ev, phase, o.prevented);
            o.listeners += 1;
            self.assert_no_pass("call_sync");
            let bits = match host.call_sync(node.to_raw(), sig | (flags << SVL_SIGNAL_FLAGS_SHIFT), &args) {
                Ok(Value::I64(b)) => b,
                Ok(Value::F64(b)) => b as i64,
                Ok(Value::Bool(true)) => STOP | PREVENT,
                _ => 0,
            };
            if bits & PREVENT != 0 && ev.cancelable {
                o.prevented = true;
            }
            if bits & (STOP | IMMEDIATE) != 0 {
                o.stopped = true;
                stopped_at = Some(node);
            }
            if bits & IMMEDIATE != 0 {
                immediate_at = Some(node);
            }
        }
    }

    /// Delivers what behaviours' filters emitted (queued events, prop changes).
    fn post_out(&self, ctx: &Context, out: Vec<Out>) {
        if out.is_empty() {
            return;
        }
        if let Some(host) = self.host() {
            for x in out {
                match x {
                    Out::Event(h, sig, args) => {
                        self.bump(|s| s.events += 1);
                        host.post_event(h, sig, &args);
                    }
                    Out::Changed(h, prop) => {
                        self.bump(|s| s.changes += 1);
                        host.prop_changed(h, prop);
                    }
                }
            }
        }
        self.0.checkpoint_pending.set(true);
        ctx.request_repaint_of(self.logic_vp());
    }
}

/// The arguments of a listener call: `target`, `phase` (with the prevented / not-cancelable bits),
/// then the event's own (`kinds::POINTER_ARGS`, `WHEEL_ARGS`, `KEY_ARGS`, `FOCUS_ARGS`).
fn js_args(ev: &SceneEvent, phase: EventPhase, prevented: bool) -> Vec<Value> {
    let p = phase as i64 | if prevented { PHASE_PREVENTED } else { 0 } | if ev.cancelable { 0 } else { PHASE_NOT_CANCELABLE };
    let mut a = vec![Value::I64(ev.light_target.to_raw() as i64), Value::I64(p)];
    let pos = ev.pos.unwrap_or(Pos2::ZERO);
    let local = ev.local.unwrap_or(pos);
    match ev.kind {
        k if k.pointer() => {
            // DOM: `button` is -1 for a move
            let button = if k == EventKind::PointerMove { -1 } else { ev.button.map_or(0, dom_button) };
            a.extend([Value::F64(pos.x as f64), Value::F64(pos.y as f64), Value::I64(button), Value::I64(modifier_bits(ev.modifiers))]);
            a.extend([Value::F64(local.x as f64), Value::F64(local.y as f64), Value::I64(ev.buttons as i64)]);
        }
        EventKind::Wheel => {
            a.extend([Value::F64(pos.x as f64), Value::F64(pos.y as f64), Value::F64(ev.delta.x as f64), Value::F64(ev.delta.y as f64), Value::I64(modifier_bits(ev.modifiers))]);
            a.extend([Value::F64(local.x as f64), Value::F64(local.y as f64)]);
        }
        EventKind::KeyDown | EventKind::KeyUp => {
            a.extend([Value::Str(ev.key.map_or("", |k| k.name()).to_string()), Value::Bool(ev.repeat), Value::I64(modifier_bits(ev.modifiers))]);
        }
        _ => a.push(Value::I64(ev.related.map_or(0, |n| n.to_raw() as i64))),
    }
    a
}

/// CV1: `pos` relative to the top left of `t`'s rect drawn last pass in `vp`: a container's box
/// rect, a widget's rect, a provider part's rect where the provider has one, else its host's.
fn local_point(s: SceneRef<'_>, vp: ViewportId, t: Target, pos: Pos2) -> Option<Pos2> {
    let node = t.node();
    let rect = match t {
        Target::Node(_) => s.box_rect(vp, node).or_else(|| s.target_rect(t).map(|r| r.1)),
        Target::Virtual(_) => s.target_rect(t).or_else(|| s.target_rect(Target::Node(node))).map(|r| r.1),
    }?;
    Some((pos - rect.min).to_pos2())
}
