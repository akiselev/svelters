//! skelter-core's document timers under svelters' app document (`SkelterDocument`): a kind's
//! `wake_at` (a toast's timeout, a `title` tip, a status message) wakes the runner and fires on its
//! clock. Before, `SkelterDocument` never ran `run_timers`, never reported `next_wakeup`, and a due
//! timer asked for no frame.

use skelter::seams::DocumentApi;
use skelter_base::change::ChangeClass;
use skelter_base::geom::{Scale, Size};
use skelter_base::ids::WindowId;
use skelter_base::time::{Clock, Span, Time};
use skelter_core::{Event, EventCx, Handled, Kind, KindId, KindSpec, MethodId, MethodSpec, PropError, PropId, PropSpec, RecordCx, Value, ValueType};
use std::sync::{Arc, Mutex};
use svelters_abi::*;
use svelters_skelter::SkelterBackend;
use svelters_skelter::app::SkelterDocument;
use svelters_skelter::kinds::Kinds;

struct TestClock(Mutex<Time>);

impl Clock for TestClock {
    fn now(&self) -> Time {
        *self.0.lock().unwrap()
    }
}

static TIMED_PROPS: &[PropSpec] = &[PropSpec::new("fired", ValueType::Bool, ChangeClass::Paint).native()];
static TIMED_METHODS: &[MethodSpec] = &[MethodSpec::new("arm", &[], None)];
static TIMED: KindSpec = KindSpec { props: TIMED_PROPS, methods: TIMED_METHODS, ..KindSpec::new("timed") };

/// `arm()` asks for a timer 500 ms on (a toast's timeout); the timer writes `fired`.
#[derive(Default)]
struct Timed;

impl Kind for Timed {
    fn spec() -> &'static KindSpec {
        &TIMED
    }
    fn record(&self, _cx: &mut RecordCx) {}
    fn call(&mut self, cx: &mut EventCx, _method: MethodId, _args: &[Value]) -> Result<Option<Value>, PropError> {
        cx.wake_at(cx.time() + Span::from_millis(500));
        Ok(None)
    }
    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        if matches!(ev, Event::Timer) {
            let _ = cx.set_native(PropId::of("timed", "fired", "bool"), Value::Bool(true));
            return Handled::Yes;
        }
        Handled::No
    }
}

#[test]
fn a_kinds_timer_wakes_the_runner_and_fires_on_its_clock() {
    let kinds = Kinds::with_app(&svelters_skelter::surface::builtin(), |r| r.register::<Timed>().map(|_| ())).unwrap();
    let b = SkelterBackend::new(Arc::new(kinds));
    let w = b.create(b.resolve(NameKind::Kind, 0, "window")).unwrap();
    const W: WindowId = WindowId(7);
    b.attach_presented(w, W, Size::new(200.0, 100.0), Scale(1.0)).unwrap();
    let wn = b.node(w).unwrap();
    let t = b.with_doc(|d| {
        let t = d.create(KindId::of("timed"));
        d.insert(wn, t, None, None).unwrap();
        t
    });
    let clock = Arc::new(TestClock(Mutex::new(Time::ZERO)));
    let mut doc = SkelterDocument::new(b.clone()).with_clock(clock.clone());
    let at = |ms: i64| Time::ZERO + Span::from_millis(ms);
    doc.frame(W, at(0));
    b.with_doc(|d| d.call(t, TIMED.method_id(0), &[])).unwrap();
    doc.frame(W, at(16));
    assert_eq!(doc.next_wakeup(W), Some(at(500)), "the runner arms its timer for the kind's");
    *clock.0.lock().unwrap() = at(100);
    assert!(!doc.needs_frame(W), "nothing due yet: no frame");
    *clock.0.lock().unwrap() = at(500);
    assert!(doc.needs_frame(W), "the timer is due: a frame");
    doc.frame(W, at(500));
    let fired = b.with_doc(|d| d.view(t).unwrap().prop_bool(PropId::of("timed", "fired", "bool")));
    assert!(fired, "the frame ran the kind's timer");
    assert_eq!(doc.next_wakeup(W), None);
}
