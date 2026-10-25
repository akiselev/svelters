//! AU7 (cadatomic WAITS §3): one wait engine. Cursor waits on the event journal (the
//! arm-before-act race cannot happen, identical waits consume distinct events, `never`, `seen`),
//! named busy sources (background never blocks settle, a stuck foreground source names itself),
//! clock domains (a busy wall-clock source holds the virtual clock), actionability checks named at
//! the timeout, replayed transcripts, and the failure report, on the counter fixture.

use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;
use svelters_automation::clock::{Clocked, VirtualClock};
use svelters_automation::replay::{Recorder, Redactor, SourceSeam, Transcript};
use svelters_automation::{
    ActOptions, Budget, BusyKind, BusyReport, BusySource, ClockDomain, Condition, EventKind, Expect, FieldType, Matcher,
    Session, TextMatch, WaitState,
};
use svelters_egui::harness::TestApp;
use svelters_host::HostConfig;

fn fixture(file: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/counter-egui").join(file)
}

fn app() -> Session<TestApp> {
    let mut s = Session::for_test(TestApp::from_source(fixture("")).unwrap());
    s.wait_settled(10_000.0).unwrap();
    s.journal()
        .register(EventKind::new("job.finished", ClockDomain::Wall).field("status", FieldType::String))
        .unwrap();
    s.journal().register(EventKind::new("job.failed", ClockDomain::Wall)).unwrap();
    s.journal().register(EventKind::new("ui.done", ClockDomain::Virtual)).unwrap();
    s
}

fn quiet() -> HostConfig {
    HostConfig { echo_console: false, echo_errors: false, ..HostConfig::default() }
}

#[test]
fn an_event_emitted_inside_the_act_is_seen_by_a_wait_taken_after_it() {
    let mut s = app();
    s.bridge_scene_changes(Default::default()).unwrap();
    let at = s.cursor();
    // the act replies after the app settled: the label's change is already in the journal
    s.click("role=button[name=Increment]", ActOptions::default()).unwrap();
    let e = s.wait_event(&Matcher::kind("scene.changed"), at, 0.0).unwrap();
    assert!(e.seq > at.0, "{e:?}");

    // an emitter on another thread, done before the wait is even called
    let j = s.journal().clone();
    let at = s.cursor();
    std::thread::spawn(move || j.emit("job.finished", "drc", json!({"status": "ok"})).unwrap()).join().unwrap();
    let e = s.wait_event(&Matcher::kind("job.finished").subject("drc"), at, 0.0).unwrap();
    assert_eq!(e.fields["status"], "ok");
}

#[test]
fn two_identical_waits_consume_two_events() {
    let mut s = app();
    let at = s.cursor();
    for status in ["ok", "warn"] {
        s.journal().emit("job.finished", "drc", json!({"status": status})).unwrap();
    }
    let m = Matcher::kind("job.finished").subject("drc");
    assert_eq!(s.wait_event(&m, at, 100.0).unwrap().fields["status"], "ok");
    assert_eq!(s.wait_event(&m, at, 100.0).unwrap().fields["status"], "warn");
    // a third has nothing left: the timeout reports the condition and the events it saw
    let err = s.wait_event(&m, at, 500.0).unwrap_err();
    assert_eq!(err.code, "timeout", "{err}");
    let r = err.data.clone().unwrap();
    assert_eq!(r["events"].as_array().unwrap().len(), 2, "{r}");
    assert!(r["unmet"][0]["condition"].as_str().unwrap().contains("job.finished drc"), "{r}");
    assert!(err.message.contains("#2 job.finished drc"), "{err}");
    // waiting on a kind nobody registered fails at once
    assert_eq!(s.wait_event(&Matcher::kind("job.typo"), at, 100.0).unwrap_err().code, "event.unregistered");
}

#[test]
fn never_trips_every_wait_until_the_step_ends() {
    let mut s = app();
    let at = s.cursor();
    s.never(Matcher::kind("job.failed").subject("drc"), at).unwrap();
    s.wait_settled(100.0).unwrap();
    s.journal().emit("job.failed", "route", serde_json::Value::Null).unwrap();
    s.wait_settled(100.0).unwrap();
    s.journal().emit("job.failed", "drc", serde_json::Value::Null).unwrap();
    let err = s.wait_for("role=button", WaitState::Visible, None, 100.0).unwrap_err();
    assert_eq!(err.code, "wait.never", "{err}");
    assert_eq!(s.end_step().unwrap_err().code, "wait.never");
    // the step is over: the guard is gone
    s.wait_settled(100.0).unwrap();
}

#[test]
fn seen_latches_transient_state() {
    let mut s = app();
    let id = s
        .arm_seen(Condition::new("role=label", WaitState::Visible, Some(Expect::Text(TextMatch::Exact("Count: 1".into())))))
        .unwrap();
    assert!(!s.seen(id).unwrap());
    s.click("role=button[name=Increment]", ActOptions::default()).unwrap();
    s.click("role=button[name=Increment]", ActOptions::default()).unwrap();
    // "Count: 1" is gone, but it held on a settled frame after the latch was armed
    assert!(s.query(r#"text="Count: 1""#, None).unwrap().is_empty());
    s.wait_seen(id, 0.0).unwrap();
    // one that never held times out with its last value
    let never = s.arm_seen(Condition::new(r#"text="Count: 9""#, WaitState::Visible, None)).unwrap();
    let err = s.wait_seen(never, 300.0).unwrap_err();
    assert!(err.message.contains("0 matches"), "{err}");
}

#[test]
fn background_sources_never_block_settle_and_stuck_foreground_ones_name_themselves() {
    let mut s = app();
    s.register_busy_source(BusySource::new("lsp", || BusyReport::busy(1, "long-lived connection")).background().wall())
        .unwrap();
    s.register_busy_source(BusySource::new("spinner", || BusyReport::busy(1, "indeterminate")).background().virtual_clock())
        .unwrap();
    s.wait_settled(100.0).unwrap();

    let n = Arc::new(AtomicU32::new(1));
    let m = n.clone();
    s.register_busy_source(
        BusySource::new("jobs", move || BusyReport::busy(m.load(Ordering::SeqCst), "drc on board 1")).foreground().virtual_clock(),
    )
    .unwrap();
    s.wait_settled(1_000.0).unwrap_err();
    s.register_busy_source(BusySource::new("tools", || BusyReport::busy(2, "move")).foreground().virtual_clock()).unwrap();
    let err = s.wait_settled(1_000.0).unwrap_err();
    assert_eq!(err.code, "timeout");
    assert!(err.message.contains("busy: jobs ×1 (drc on board 1) for "), "{err}");
    let busy = err.data.as_ref().unwrap()["busy"].as_array().unwrap().clone();
    // longest busy first; background sources are not listed
    assert_eq!(busy.iter().map(|b| b["id"].as_str().unwrap()).collect::<Vec<_>>(), ["jobs", "tools"], "{busy:?}");
    assert!(busy[0]["busy_for_ms"].as_f64().unwrap() > busy[1]["busy_for_ms"].as_f64().unwrap(), "{busy:?}");
    // a source without a kind is refused
    assert_eq!(s.register_busy_source(BusySource::new("x", BusyReport::idle).wall()).unwrap_err().code, "busy.invalid");

    // the JSON surface lists them
    let j = s.call("busy.sources", &json!({})).unwrap();
    assert_eq!(j["sources"].as_array().unwrap().len(), 4, "{j}");
    n.store(0, Ordering::SeqCst);
    assert!(s.busy_registry().unregister("tools"));
    s.wait_settled(100.0).unwrap();
}

#[test]
fn a_virtual_timeout_does_not_fire_while_a_wall_source_is_busy() {
    let clock = VirtualClock::new(1);
    let app = TestApp::from_source_with(fixture(""), 0.25, clock.config(quiet())).unwrap();
    let mut s = Session::for_test(Clocked::new(app, clock));
    s.wait_settled(10_000.0).unwrap();
    s.journal().register(EventKind::new("ui.done", ClockDomain::Virtual)).unwrap();

    // a 30 s virtual wait with nothing to hold the clock runs out of virtual time
    let at = s.cursor();
    let err = s.wait_event(&Matcher::kind("ui.done"), at, 30_000.0).unwrap_err();
    assert_eq!(err.data.as_ref().unwrap()["exhausted"], "virtual", "{err}");

    // real work on another thread, behind a foreground wall-clock source: the virtual clock holds
    let busy = Arc::new(AtomicBool::new(true));
    let b = busy.clone();
    s.register_busy_source(
        BusySource::new("solver", move || if b.load(Ordering::SeqCst) { BusyReport::busy(1, "solve") } else { BusyReport::idle() })
            .foreground()
            .wall(),
    )
    .unwrap();
    let (j, b) = (s.journal().clone(), busy.clone());
    let worker = std::thread::spawn(move || {
        // longer in wall time than the frames of 30 virtual seconds take
        std::thread::sleep(Duration::from_millis(1500));
        j.emit("ui.done", "solve", serde_json::Value::Null).unwrap();
        b.store(false, Ordering::SeqCst);
    });
    let (v0, c0) = (s.now_ms(), s.driver().clock().now_ms());
    let at = s.cursor();
    s.wait_event_within(&Matcher::kind("ui.done"), at, Budget::new(30_000.0, 60_000.0)).unwrap();
    worker.join().unwrap();
    assert!(s.now_ms() - v0 < 30_000.0, "the virtual budget did not run: {}", s.now_ms() - v0);
    assert!(s.driver().clock().now_ms() - c0 < 1_000.0, "the virtual clock held: {}", s.driver().clock().now_ms() - c0);
    s.wait_settled(1_000.0).unwrap();

    // a stuck wall source runs out of wall time, naming itself
    busy.store(true, Ordering::SeqCst);
    let err = s.wait_settled_within(Budget::new(30_000.0, 200.0)).unwrap_err();
    let r = err.data.unwrap();
    assert_eq!((r["exhausted"].as_str(), r["busy"][0]["id"].as_str()), (Some("wall"), Some("solver")), "{r}");
}

fn waits_app() -> Session<Clocked<TestApp>> {
    let clock = VirtualClock::new(3);
    let app = TestApp::from_source_with(fixture("src/Waits.svelte"), 0.016, clock.config(quiet())).unwrap();
    let mut s = Session::for_test(Clocked::new(app, clock));
    s.advance_clock(200.0).unwrap();
    s
}

fn quick() -> ActOptions {
    ActOptions { timeout_ms: 500.0, ..ActOptions::default() }
}

#[test]
fn actionability_names_the_failing_check() {
    let mut s = waits_app();
    let err = s.click("role=button[name=Mover]", quick()).unwrap_err();
    assert_eq!(err.code, "node.not_actionable", "{err}");
    assert!(err.message.contains("still moving"), "{err}");
    assert_eq!(err.data.as_ref().unwrap()["actionability"]["check"], "stable", "{err}");

    let err = s.click("role=button[name=Under]", quick()).unwrap_err();
    assert!(err.message.contains("covered by") && err.message.contains("Cover"), "{err}");
    let a = &err.data.as_ref().unwrap()["actionability"];
    assert_eq!(a["check"], "hit", "{err}");
    let cover = s.locate("role=button[name=Cover]").unwrap();
    assert_eq!(a["cover"].as_u64(), Some(cover.id.to_raw()));

    let err = s.fill("role=button[name=Cover]", "x", quick()).unwrap_err();
    assert_eq!(err.data.as_ref().unwrap()["actionability"]["check"], "editable", "{err}");

    let err = s.click("role=button[name=Nowhere]", quick()).unwrap_err();
    assert_eq!(err.data.as_ref().unwrap()["actionability"]["check"], "attached", "{err}");
}

#[test]
fn a_replayed_source_emits_its_events_on_the_virtual_clock_and_holds_settle() {
    let mut s = app();
    s.journal().register(EventKind::new("net.response", ClockDomain::Wall).field("url", FieldType::String)).unwrap();
    let rec = Recorder::new();
    let live = SourceSeam::record("net", s.journal().clone(), &rec).redactor(Redactor::new().url("url"));
    let x = live.exchange("GET route:/parts");
    x.emit("net.response", "route:/parts", json!({"url": "https://h/parts?k=1"})).unwrap();
    x.finish(json!({"n": 3}));
    let mut t = Transcript::from_ndjson(&rec.transcript().to_ndjson()).unwrap();
    // pretend the response took 2 s
    if let svelters_automation::replay::TranscriptLine::Event { after_ms, .. } = &mut t.lines[0] {
        *after_ms = 2_000.0;
    }

    let seam = SourceSeam::replay("net", s.journal().clone(), &t);
    s.register_busy_source(seam.busy_source(BusyKind::Foreground)).unwrap();
    let at = s.cursor();
    assert_eq!(seam.exchange("GET route:/parts").replay().unwrap(), json!({"n": 3}));
    let t0 = s.now_ms();
    s.wait_settled(10_000.0).unwrap();
    assert!(s.now_ms() - t0 >= 2_000.0, "settle waited for the replayed response: {}", s.now_ms() - t0);
    let e = s.wait_event(&Matcher::kind("net.response"), at, 0.0).unwrap();
    assert_eq!(e.fields["url"], "route:/parts");
}

#[test]
fn the_json_surface_carries_the_waits() {
    let mut s = app();
    let hello = s.call("session.hello", &json!({})).unwrap();
    for m in ["journal.cursor", "wait.event", "wait.never", "wait.seen"] {
        assert!(hello["capabilities"].as_array().unwrap().contains(&json!(m)), "{m}");
    }
    let at = s.call("journal.cursor", &json!({})).unwrap()["cursor"].clone();
    s.journal().emit("job.finished", "drc", json!({"status": "ok"})).unwrap();
    let e = s
        .call("wait.event", &json!({"matcher": {"kind": "job.finished", "fields": {"status": "ok"}}, "since": at, "timeout": 100}))
        .unwrap();
    assert_eq!(e["subject"], "drc");
    assert_eq!(s.call("wait.event", &json!({"kind": "job.finished"})).unwrap_err().code, "params.invalid");
    let err = s.call("wait.event", &json!({"kind": "job.finished", "since": at, "timeout": 100})).unwrap_err();
    assert!(err.data.is_some(), "{err}");

    let seen = s.call("wait.seen", &json!({"selector": "role=button[name=Increment]"})).unwrap();
    assert_eq!(seen["seen"], true);
    s.call("wait.seen", &json!({"id": seen["id"], "timeout": 0})).unwrap();
    s.call("wait.never", &json!({"kind": "job.failed"})).unwrap();
    s.journal().emit("job.failed", "x", serde_json::Value::Null).unwrap();
    assert_eq!(s.call("wait.step_end", &json!({})).unwrap_err().code, "wait.never");
    assert_eq!(s.call("journal.events", &json!({"since": at})).unwrap()["events"].as_array().unwrap().len(), 2);
    assert!(s.call("journal.kinds", &json!({})).unwrap().as_array().unwrap().len() >= 3);
}
