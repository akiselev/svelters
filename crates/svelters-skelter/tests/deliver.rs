//! Event-time delivery: what the tree produced reaches the host before the input's handling ends,
//! changes first, cancelable events synchronously, then one checkpoint.

mod common;

use common::Rig;
use svelters_abi::*;
use svelters_skelter::surface::Value as PValue;

#[test]
fn unlistened_events_and_unobserved_changes_stay_native() {
    let r = Rig::new();
    let c = r.create("checkbox");
    r.native_set(c, "checked", PValue::Bool(true));
    r.emit(c, "changed", vec![PValue::Bool(true)]);
    let d = r.backend.pump();
    assert_eq!((d.events, d.changes), (0, 0));
    let l = r.take();
    assert!(l.events.is_empty() && l.changed.is_empty());
    assert_eq!(l.checkpoints, 0, "nothing for JS: no checkpoint");
}

#[test]
fn changes_then_events_then_one_checkpoint() {
    let r = Rig::new();
    let c = r.create("checkbox");
    r.listen(c, "changed");
    r.observe(c, "checked", true);
    r.native_set(c, "checked", PValue::Bool(true));
    r.emit(c, "changed", vec![PValue::Bool(true)]);
    let d = r.backend.pump();
    assert_eq!((d.events, d.changes), (1, 1));
    let l = r.take();
    assert_eq!(l.changed, [(c, r.prop_id(c, "checked"))]);
    assert_eq!(l.events, [(c, r.signal_id(c, "changed"), vec![Value::Bool(true)])]);
    assert_eq!(l.checkpoints, 1);
}

#[test]
fn cancelable_events_run_synchronously_and_report_cancellation() {
    let r = Rig::new();
    let b = r.create("button");
    r.listen(b, "clicked");
    r.listen(b, "keydown");
    r.log.borrow_mut().sync_answer = Some(Value::Bool(true));
    r.emit(b, "clicked", vec![]);
    r.emit(b, "keydown", vec![PValue::I64(b as i64), PValue::I64(2), PValue::Str("Enter".into()), PValue::Bool(false), PValue::I64(0)]);
    let d = r.backend.pump();
    assert_eq!(d.cancelled, [false, true]);
    let l = r.take();
    assert_eq!(l.events.len(), 1, "the widget signal is posted");
    assert_eq!(l.syncs.len(), 1, "the DOM-style event runs synchronously");
    assert_eq!(l.syncs[0].2[2], Value::Str("Enter".into()));
    // the posted event was flushed before the synchronous one ran, and the rest after
    assert_eq!(l.checkpoints, 2);
}

#[test]
fn observe_counts_nest() {
    let r = Rig::new();
    let s = r.create("slider");
    r.observe(s, "value", true);
    r.observe(s, "value", true);
    r.observe(s, "value", false);
    r.native_set(s, "value", PValue::F64(3.0));
    r.backend.pump();
    assert_eq!(r.take().changed.len(), 1, "still observed once");
    r.observe(s, "value", false);
    r.native_set(s, "value", PValue::F64(4.0));
    r.backend.pump();
    assert!(r.take().changed.is_empty());
}

#[test]
fn cancelable_events_answer_prevented_and_stopped() {
    let r = Rig::new();
    let b = r.create("button");
    r.listen(b, "keydown");
    let n = r.backend.node(b).unwrap();
    let ev = r.backend.target_event(n, "keydown", vec![PValue::I64(b as i64), PValue::I64(2), PValue::Str("Enter".into()), PValue::Bool(false), PValue::I64(0)]).unwrap();
    // preventDefault (bit 2) and no stop
    r.log.borrow_mut().sync_answer = Some(Value::I64(2));
    let o = r.backend.deliver_cancelable(&ev);
    assert!(o.prevented && !o.stopped);
    // stopPropagation (bit 1)
    r.log.borrow_mut().sync_answer = Some(Value::I64(1));
    let o = r.backend.deliver_cancelable(&ev);
    assert!(!o.prevented && o.stopped);
    assert_eq!(r.take().syncs.len(), 2);
}

#[test]
fn the_lent_document_follows_the_borrow_rules() {
    let r = Rig::new();
    let b = r.create("button");
    // outside a delivery the backend's own document is used, with `RefCell`'s rules
    let rev = r.backend.doc().revision();
    let again = r.backend.doc().revision();
    assert_eq!(rev, again);
    r.set(b, "text", Value::Str("x".into()));
    assert_eq!(r.get(b, "text"), Value::Str("x".into()));
}

/// QC1-EXP-002/005 (cadatomic qc1): skelter-core delivers a text field's `changed` at event time
/// (`JsEvent::sync`), but the surface signal is not sync. Its listeners get the text alone, as on
/// egui, not a `NativeEvent` first; a handler `(t) => setQuery(t)` got the event object.
#[test]
fn a_signal_the_surface_does_not_declare_sync_is_posted_with_its_arguments() {
    let r = Rig::new();
    let t = r.create("text-edit");
    r.listen(t, "changed");
    let n = r.backend.node(t).unwrap();
    let mut ev = r.backend.target_event(n, "changed", vec![PValue::Str("cyl".into())]).unwrap();
    ev.sync = true; // as skelter-core marks skelter-widgets' text-field signals
    r.backend.queue(svelters_skelter::Output { events: vec![ev], ..Default::default() });
    let d = r.backend.pump();
    assert_eq!(d.events, 1);
    let l = r.take();
    assert!(l.syncs.is_empty(), "not run as a sync signal: {:?}", l.syncs);
    assert_eq!(l.events, [(t, r.signal_id(t, "changed"), vec![Value::Str("cyl".into())])]);
}

/// FQ-65 (cadatomic rmegui): a checkbox's `changed` is cancelable, so skelter-core delivers it at
/// event time and the backend calls it synchronously, but the surface signal is not sync: its
/// listeners take the value (`(v) => setVisible(v)`), not a `NativeEvent` first. The call carries
/// `SVL_SIGNAL_ARGS`, and the runtime's `true` (a `preventDefault()`) still cancels.
#[test]
fn a_cancelable_widget_signal_is_called_with_its_arguments_first() {
    let r = Rig::new();
    let c = r.create("checkbox");
    r.listen(c, "changed");
    let n = r.backend.node(c).unwrap();
    let mut ev = r.backend.target_event(n, "changed", vec![PValue::Bool(true)]).unwrap();
    ev.cancelable = true; // as skelter-core marks skelter-widgets' checkbox `changed`
    r.log.borrow_mut().sync_answer = Some(Value::Bool(true));
    let o = r.backend.deliver_cancelable(&ev);
    assert!(o.prevented, "preventDefault cancels the toggle");
    let l = r.take();
    assert_eq!(l.syncs.len(), 1);
    let (h, id, args) = &l.syncs[0];
    assert_eq!(*h, c);
    assert_eq!(*id, r.signal_id(c, "changed") | SVL_SIGNAL_ARGS << SVL_SIGNAL_FLAGS_SHIFT);
    assert_eq!(args, &vec![Value::Bool(true)]);
    // a DOM-style event keeps the `(event, ...args)` call: no flag
    let b = r.create("button");
    r.listen(b, "keydown");
    let nb = r.backend.node(b).unwrap();
    let kd = r.backend.target_event(nb, "keydown", vec![PValue::I64(b as i64), PValue::I64(2), PValue::Str("a".into()), PValue::Bool(false), PValue::I64(0)]).unwrap();
    r.backend.deliver_cancelable(&kd);
    let l = r.take();
    assert_eq!(l.syncs[0].1, r.signal_id(b, "keydown"));
}
