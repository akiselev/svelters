//! The kind-facing services widgets asked for: timers, AccessKit actions, blur, typed services, the
//! engine's computed style and the event time in the contexts.

use super::*;
use crate::cx::{EventCx, MeasureCx, RecordCx};
use crate::kind::{Event, Handled, Kind, KindSpec, SignalFlags, SignalSpec};
use crate::{KindRegistry, ListenFlags, SignalId};
use presto_base::time::Span;
use std::cell::Cell;
use std::sync::Arc;

/// A service the test kind reads while measuring.
struct Metric(f32);

/// Counts calls through a `Cell`, the pattern for services that change while kinds use them.
#[derive(Default)]
struct Calls(Cell<u32>);

const FIRED: SignalId = SignalId::of("repeater", "fired", "void");

static REPEATER: KindSpec = KindSpec {
    signals: &[SignalSpec::new("fired", None, SignalFlags::NONE)],
    focusable: true,
    ..KindSpec::new("repeater")
};

/// Arms a timer 100 ms after a press, re-arms on each fire while `repeat` is left, blurs on Escape.
#[derive(Default)]
struct Repeater {
    fired: u32,
    pressed_at: Option<Time>,
    last: Option<Time>,
}

impl Kind for Repeater {
    fn spec() -> &'static KindSpec {
        &REPEATER
    }
    fn measure(&self, cx: &mut MeasureCx, _known: taffy::Size<Option<f32>>, _avail: taffy::Size<taffy::AvailableSpace>) -> taffy::Size<f32> {
        if let Some(c) = cx.service::<Calls>() {
            c.0.set(c.0.get() + 1);
        }
        let s = cx.service::<Metric>().map_or(1.0, |m| m.0);
        taffy::Size { width: s, height: s }
    }
    fn record(&self, cx: &mut RecordCx) {
        cx.draw_box();
    }
    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        match ev {
            Event::PointerDown(_) => {
                self.pressed_at = Some(cx.now());
                cx.wake_at(cx.now() + Span::from_millis(100));
                Handled::Yes
            }
            Event::PointerUp(_) => {
                cx.cancel_wake();
                Handled::Yes
            }
            Event::Timer => {
                self.fired += 1;
                self.last = Some(cx.now());
                cx.emit(FIRED, None);
                if self.fired < 3 {
                    cx.wake_at(cx.now() + Span::from_millis(50));
                }
                Handled::Yes
            }
            Event::Key(k) if k.key == Some(Key::Escape) => {
                cx.blur();
                Handled::Yes
            }
            Event::A11y(r) if r.action == accesskit::Action::Increment => {
                self.fired += 10;
                Handled::Yes
            }
            _ => Handled::No,
        }
    }
}

const REP: KindId = KindId::of("repeater");

fn rdoc() -> (Document, NodeId, NodeId) {
    let mut r = KindRegistry::new();
    r.register::<testkinds::BoxKind>().unwrap();
    r.register::<testkinds::ToggleKind>().unwrap();
    r.register::<Repeater>().unwrap();
    let mut d = Document::new(Arc::new(r), Box::new(FixedText));
    let root = d.create_window(W, BOX, Size::new(200.0, 100.0), Scale(1.0));
    let n = child(&mut d, root, REP, "width: 40px; height: 40px");
    frame(&mut d);
    (d, root, n)
}

#[test]
fn timers_fire_in_order_and_rearm() {
    let (mut d, _root, n) = rdoc();
    d.listen(n, FIRED, ListenFlags::default());
    d.dispatch(&pointer(PointerPhase::Down, 10.0, 10.0, 1_000_000));
    assert_eq!(d.next_timer(), Some(Time(101_000_000)));
    assert!(d.busy().contains(crate::Busy::TIMER_SOON));
    // not yet due
    let early = d.fire_timers(Time(50_000_000));
    assert!(early.events.is_empty() && !early.handled);
    let first = d.fire_timers(Time(101_000_000));
    assert!(first.handled);
    assert_eq!(first.target, Some(n));
    assert_eq!(first.events.iter().filter(|e| e.signal == FIRED).count(), 1);
    // the handler re-armed from the due time
    assert_eq!(d.next_timer(), Some(Time(151_000_000)));
    d.fire_timers(Time(500_000_000));
    assert_eq!(d.next_timer(), Some(Time(550_000_000)));
    d.fire_timers(Time(600_000_000));
    assert_eq!(d.next_timer(), None);
    assert!(!d.busy().contains(crate::Busy::TIMER_SOON));
}

#[test]
fn release_cancels_and_destroy_forgets_timers() {
    let (mut d, _root, n) = rdoc();
    d.dispatch(&pointer(PointerPhase::Down, 10.0, 10.0, 0));
    d.dispatch(&pointer(PointerPhase::Up, 10.0, 10.0, 10));
    assert_eq!(d.next_timer(), None);
    d.dispatch(&pointer(PointerPhase::Down, 10.0, 10.0, 20));
    assert!(d.next_timer().is_some());
    d.destroy(n);
    assert_eq!(d.next_timer(), None);
}

#[test]
fn event_time_is_the_platform_timestamp() {
    let (mut d, _root, n) = rdoc();
    d.dispatch(&pointer(PointerPhase::Down, 10.0, 10.0, 7_000));
    assert_eq!(d.next_timer(), Some(Time(7_000) + Span::from_millis(100)));
    let _ = n;
}

#[test]
fn kind_blurs_itself() {
    let (mut d, _root, n) = rdoc();
    d.focus(n, crate::FocusReason::Programmatic);
    assert_eq!(d.focused(W), Some(n));
    d.dispatch(&key(Key::Escape, Modifiers::default()));
    assert_eq!(d.focused(W), None);
    assert!(!d.state(n).contains(crate::StateMask::FOCUSED));
}

fn action(n: NodeId, a: accesskit::Action) -> accesskit::ActionRequest {
    accesskit::ActionRequest { action: a, target_tree: accesskit::TreeId::ROOT, target_node: accesskit::NodeId(n.0), data: None }
}

#[test]
fn a11y_actions_reach_the_kind_then_the_defaults() {
    let (mut d, root, n) = rdoc();
    // the kind handles Increment itself
    let r = d.a11y_action(&action(n, accesskit::Action::Increment), Time(0));
    assert!(r.handled);
    // Focus falls back to the document's default
    let r = d.a11y_action(&action(n, accesskit::Action::Focus), Time(0));
    assert!(r.handled);
    assert_eq!(d.focused(W), Some(n));
    // Click on a toggle: its own click handler flips it and the JS click is listed
    let t = child(&mut d, root, TOGGLE, "width: 16px; height: 16px");
    frame(&mut d);
    d.listen(t, crate::common::CLICK, ListenFlags::default());
    let r = d.a11y_action(&action(t, accesskit::Action::Click), Time(0));
    assert!(r.handled);
    assert_eq!(d.get_prop(t, testkinds::TOGGLE_CHECKED), Some(crate::Value::Bool(true)));
    assert!(r.events.iter().any(|e| e.signal == crate::common::CLICK && e.target == t));
    // an unknown node does nothing
    let r = d.a11y_action(&action(NodeId(u64::MAX), accesskit::Action::Click), Time(0));
    assert!(!r.handled && r.events.is_empty());
}

#[test]
fn services_reach_measure() {
    let (mut d, root, _n) = rdoc();
    d.set_service(Metric(30.0));
    d.set_service(Calls::default());
    let m = child(&mut d, root, REP, "");
    frame(&mut d);
    // the root is a column: the measured height is kept, the width stretches
    assert_eq!(d.layout_rect(m).unwrap().height(), 30.0);
    assert!(d.service::<Calls>().unwrap().0.get() >= 1);
    assert!(d.set_service(Metric(2.0)).is_some());
    assert_eq!(d.remove_service::<Metric>().map(|m| m.0), Some(2.0));
    assert!(d.service::<Metric>().is_none());
}

#[test]
fn views_carry_the_engine_style() {
    let (mut d, root, _n) = rdoc();
    let b = child(&mut d, root, BOX, "color: red; padding: 3px");
    let v = d.view(b).unwrap();
    assert!(v.computed_style().is_none(), "no restyle yet");
    frame(&mut d);
    let v = d.view(b).unwrap();
    let css = v.computed_style().expect("restyled");
    assert!(css.get("color").is_some());
}
