//! Test support for the kinds: record a kind with the monospace test shaper, measure it, and drive it
//! with pointer, key, focus and AccessKit events on a virtual clock, collecting the signals it emits
//! and the props it changes. Tests only (`#[cfg(test)]`).
#![allow(dead_code)]

use crate::interact::{FocusReason, Policy, StateBits};
use crate::shim::core::{A11yCx, Event, EventCx, Handled, Kind, MeasureCx, PrepareCx, PropId, RecordCx, Styles, Value};
use crate::shim::testing::{MemResources, MonoShaper};
use presto_base::accesskit;
use presto_base::display::{DisplayList, Prim};
use presto_base::event::{KeyEvent, KeyLocation, KeyState, Modifiers, MouseButton, PointerEvent, PointerPhase, PointerType};
use presto_base::geom::{Point, Size, Vec2};
use presto_base::key::Key;
use presto_base::time::{Span, Time};
use smallvec::SmallVec;

/// Records `k` at `size` in `state` (dark scheme, no CSS: UA tables only) at scale 1.
pub fn record(k: &dyn Kind, size: Size, state: StateBits) -> (DisplayList, MemResources) {
    let mut list = DisplayList::new();
    let mut res = MemResources::default();
    let mut text = MonoShaper;
    let mut cx = RecordCx { size, style: Styles { computed: None, parts: &[], state, dark: true }, list: &mut list, text: &mut text, res: &mut res, scale: 1.0 };
    k.record(&mut cx);
    (list, res)
}

/// The kind's measured size with no width constraint.
pub fn measure(k: &dyn Kind, state: StateBits) -> Size {
    let mut text = MonoShaper;
    let mut cx = MeasureCx { style: Styles { computed: None, parts: &[], state, dark: true }, text: &mut text, scale: 1.0 };
    let avail = taffy::Size { width: taffy::AvailableSpace::MaxContent, height: taffy::AvailableSpace::MaxContent };
    let s = k.measure(&mut cx, taffy::Size { width: None, height: None }, avail);
    Size::new(s.width, s.height)
}

/// Runs the kind's `prepare` at `size` in `state` (what presto-core does after layout, before
/// recording and the events that follow).
pub fn prepare(k: &mut dyn Kind, size: Size, state: StateBits) {
    let mut text = MonoShaper;
    let mut cx = PrepareCx { size, style: Styles { computed: None, parts: &[], state, dark: true }, text: &mut text };
    k.prepare(&mut cx);
}

/// The kind's AccessKit node.
pub fn a11y(k: &dyn Kind, state: StateBits, size: Size) -> accesskit::Node {
    let mut node = accesskit::Node::new(accesskit::Role::Unknown);
    k.a11y(&mut A11yCx { node: &mut node, state, size });
    node
}

/// A short name per primitive, for goldens that pin the shape of a list.
pub fn kinds(list: &DisplayList) -> Vec<&'static str> {
    list.items
        .iter()
        .map(|p| match p {
            Prim::Quad(_) => "quad",
            Prim::Shadow(s) if s.inset => "inset-shadow",
            Prim::Shadow(_) => "shadow",
            Prim::Line(_) => "line",
            Prim::Glyphs(_) => "glyphs",
            Prim::Icon(_) => "icon",
            Prim::Image(_) => "image",
            Prim::Path(_) => "path",
            Prim::Surface(_) => "surface",
            Prim::Canvas(_) => "canvas",
            Prim::PushClip(_) => "push-clip",
            Prim::PopClip => "pop-clip",
        })
        .collect()
}

/// Drives one kind through events.
pub struct Driver<K: Kind> {
    pub kind: K,
    pub state: StateBits,
    pub policy: Policy,
    pub size: Size,
    pub now: Time,
    pub focused: bool,
    /// Signal names emitted so far, in order.
    pub signals: Vec<(&'static str, Vec<Value>)>,
    /// Props the kind changed natively, in order.
    pub changed: Vec<&'static str>,
    pub wake: Option<Time>,
    pub handled: Vec<Handled>,
}

impl<K: Kind> Driver<K> {
    pub fn new(kind: K, size: Size) -> Driver<K> {
        Driver { kind, state: StateBits::NONE, policy: Policy::default(), size, now: Time(1_000_000_000), focused: false, signals: Vec::new(), changed: Vec::new(), wake: None, handled: Vec::new() }
    }

    pub fn set(&mut self, prop: &str, v: Value) {
        let id = self.kind.kind_spec().prop(prop).unwrap_or_else(|| panic!("no prop `{prop}`"));
        self.kind.set_prop(id, &v).unwrap();
    }

    pub fn get(&self, prop: &str) -> Option<Value> {
        let id = self.kind.kind_spec().prop(prop)?;
        self.kind.get_prop(id)
    }

    /// Sends `ev` at the current time; applies focus requests and collects what the kind did.
    pub fn send(&mut self, ev: Event) -> Handled {
        let spec = self.kind.kind_spec();
        let policy = self.policy;
        let mut cx = EventCx::new(self.now, self.size, &mut self.state, &policy);
        cx.focused = self.focused;
        let h = self.kind.event(&mut cx, &ev);
        let emitted = std::mem::take(&mut cx.emitted);
        let changed = std::mem::take(&mut cx.changed);
        let focus = cx.focus_request;
        self.wake = cx.wake;
        for e in emitted {
            self.signals.push((spec.signals[e.signal.0 as usize].name, e.args));
        }
        for p in changed {
            self.changed.push(spec.props[p.0 as usize].name);
        }
        self.handled.push(h);
        if let Some(f) = focus
            && f != self.focused
        {
            self.focused = f;
            self.send(Event::Focus { gained: f, reason: FocusReason::Pointer });
        }
        h
    }

    pub fn advance(&mut self, ms: i64) {
        self.now = self.now + Span::from_millis(ms);
    }

    fn pointer(phase: PointerPhase, p: Point, buttons: u32, button: Option<MouseButton>) -> Event {
        Event::Pointer(PointerEvent { phase, pointer: 0, kind: PointerType::Mouse, button, buttons, pos: p, modifiers: Modifiers::default(), pressure: 0.0, tilt: Vec2::ZERO, history: SmallVec::new() })
    }

    pub fn hover(&mut self, on: bool) {
        self.send(Event::Hover(on));
    }
    pub fn move_to(&mut self, x: f32, y: f32) {
        let held = if self.state.contains(StateBits::PRESSED) { 1 } else { 0 };
        self.send(Self::pointer(PointerPhase::Move, Point::new(x, y), held, None));
    }
    pub fn down(&mut self, x: f32, y: f32) {
        self.send(Self::pointer(PointerPhase::Down, Point::new(x, y), 1, Some(MouseButton::Primary)));
    }
    pub fn up(&mut self, x: f32, y: f32) {
        self.send(Self::pointer(PointerPhase::Up, Point::new(x, y), 0, Some(MouseButton::Primary)));
    }
    pub fn secondary_click(&mut self, x: f32, y: f32) {
        self.send(Self::pointer(PointerPhase::Down, Point::new(x, y), 2, Some(MouseButton::Secondary)));
        self.advance(20);
        self.send(Self::pointer(PointerPhase::Up, Point::new(x, y), 0, Some(MouseButton::Secondary)));
    }
    /// Hover, press and release at the centre, 50 ms apart.
    pub fn click(&mut self) {
        let (x, y) = (self.size.w / 2.0, self.size.h / 2.0);
        self.hover(true);
        self.down(x, y);
        self.advance(50);
        self.up(x, y);
        self.advance(50);
    }
    /// A key press (with its text for printable keys) and release.
    pub fn key(&mut self, key: Key) {
        self.key_with(key, Modifiers::default());
    }
    pub fn key_with(&mut self, key: Key, modifiers: Modifiers) {
        let text = match key {
            Key::Space => Some(" ".to_owned()),
            _ => None,
        };
        let ev = |state| Event::Key(KeyEvent { key: Some(key), physical: 0, text: text.clone(), state, repeat: false, location: KeyLocation::Standard, modifiers });
        self.send(ev(KeyState::Pressed));
        self.send(ev(KeyState::Released));
    }
    pub fn focus(&mut self, on: bool) {
        self.focused = on;
        self.send(Event::Focus { gained: on, reason: FocusReason::Keyboard });
    }
    pub fn a11y_action(&mut self, action: accesskit::Action) {
        self.send(Event::A11y(accesskit::ActionRequest { action, target_tree: accesskit::TreeId::ROOT, target_node: accesskit::NodeId(1), data: None }));
    }
    /// The names of the signals emitted so far.
    pub fn names(&self) -> Vec<&'static str> {
        self.signals.iter().map(|(n, _)| *n).collect()
    }
    pub fn take(&mut self) -> Vec<&'static str> {
        let n = self.names();
        self.signals.clear();
        n
    }
    pub fn prop_id(&self, name: &str) -> PropId {
        self.kind.kind_spec().prop(name).unwrap()
    }
}
