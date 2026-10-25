//! Event-time delivery: what the tree produced reaches the host before the input's handling ends,
//! changes first, cancelable events synchronously, then one checkpoint.

mod common;

use common::Rig;
use svelters_abi::*;
use svelters_presto::surface::Value as PValue;

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
fn cancelable_events_inside_dispatch_answer_prevented_and_defer_js_ops() {
    let r = Rig::new();
    let b = r.create("button");
    r.listen(b, "keydown");
    let n = r.backend.node(b).unwrap();
    let ev = r.backend.target_event(n, "keydown", vec![PValue::I64(b as i64), PValue::I64(2), PValue::Str("Enter".into()), PValue::Bool(false), PValue::I64(0)]).unwrap();
    // preventDefault (bit 2) and no stop
    r.log.borrow_mut().sync_answer = Some(Value::I64(2));
    let o = r.backend.deliver_cancelable(&ev);
    assert!(o.prevented && !o.stopped);
    assert_eq!(r.take().syncs.len(), 1);
    // what a listener does now waits for the dispatch to return; a written prop reads back
    r.set(b, "text", Value::Str("pressed".into()));
    assert_eq!(r.get(b, "text"), Value::Str("pressed".into()));
    assert!(r.br().get_prop(b, r.prop_id(b, "selected")).is_err(), "the tree is mid-dispatch");
    let made = r.create("label");
    r.backend.pump();
    assert_eq!(r.get(b, "text"), Value::Str("pressed".into()));
    assert!(r.backend.node(made).is_some(), "the deferred create ran");
    assert!(r.backend.take_deferred_errors().is_empty());
}
