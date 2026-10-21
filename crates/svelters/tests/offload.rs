//! E1 offload end to end on the log backend: worker/blocking/JS-thread/tokio exports, cancel (immediate
//! reject, late result dropped and freed), progress, panics, supersede, max_in_flight, generations
//! across hot reload, disposing the owner while work is in flight, and the typings.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use svelters::prelude::*;
use svelters_backend_log::LogBackend;
use svelters_host::{Host, testing::fixture_config};

// ---- gates and records shared by the exports and the tests (distinct numbers per test) ----

static GATES: Mutex<Vec<u32>> = Mutex::new(Vec::new());
static RECORD: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn open(g: u32) {
    GATES.lock().unwrap().push(g);
}

fn is_open(g: u32) -> bool {
    GATES.lock().unwrap().contains(&g)
}

fn record(s: String) {
    RECORD.lock().unwrap().push(s);
}

fn recorded(s: &str) -> bool {
    RECORD.lock().unwrap().iter().any(|x| x == s)
}

/// Waits for gate `g` (or, with a context, for a cancel). Gives up after 20 s.
fn wait_gate(g: u32, cx: Option<&WorkCx>) {
    let end = Instant::now() + Duration::from_secs(20);
    while !is_open(g) && Instant::now() < end {
        if let Some(cx) = cx {
            if cx.is_cancelled() {
                record(format!("saw-cancel {g}"));
                return;
            }
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// A result object: its drop is recorded, so a dropped late result is visible.
#[svelters::object(shared, module = "work")]
pub struct Payload {
    n: u32,
}

impl Drop for Payload {
    fn drop(&mut self) {
        record(format!("payload-drop {}", self.n));
    }
}

#[svelters::export]
impl Payload {
    #[js(getter)]
    pub fn n(&self) -> u32 {
        self.n
    }
}

/// Waits for gate `g` (cancellable), then doubles it.
#[svelters::export(module = "work")]
#[js(worker)]
pub fn gated(g: u32, cx: WorkCx) -> u32 {
    wait_gate(g, Some(&cx));
    g * 2
}

/// Ignores cancellation: waits for gate `g`, then returns a new object.
#[svelters::export(module = "work")]
#[js(worker)]
pub fn late_payload(g: u32) -> Arc<Payload> {
    wait_gate(g, None);
    record(format!("late-done {g}"));
    Arc::new(Payload { n: g })
}

/// Spins until cancelled.
#[svelters::export(module = "work")]
#[js(worker)]
pub fn until_cancelled(tag: u32, cx: WorkCx) -> Result<u32, NativeError> {
    record(format!("uc-start {tag}"));
    let end = Instant::now() + Duration::from_secs(20);
    while Instant::now() < end {
        if cx.check().is_err() {
            record(format!("saw-cancel {tag}"));
            cx.check()?;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    Ok(tag)
}

/// Reports 1..=n, waits for gate `g`, reports n + 1 and returns n + 1.
#[svelters::export(module = "work")]
#[js(worker)]
pub fn progressing(n: u32, g: u32, cx: WorkCx<f64>) -> u32 {
    for i in 1..=n {
        cx.progress(i as f64);
    }
    record(format!("reported {g}"));
    wait_gate(g, None);
    cx.progress((n + 1) as f64);
    record(format!("finished {g}"));
    n + 1
}

#[svelters::export(module = "work")]
#[js(worker)]
pub fn boom() -> u32 {
    panic!("kaboom")
}

/// The blocking pool.
#[svelters::export(module = "work")]
#[js(blocking)]
pub fn sleepy(ms: u32) -> String {
    std::thread::sleep(Duration::from_millis(ms as u64));
    std::thread::current().name().unwrap_or("").split('-').take(2).collect::<Vec<_>>().join("-")
}

#[svelters::export(module = "work")]
#[js(worker)]
pub fn noop(x: f64) -> f64 {
    x
}

/// A JS-thread async fn that is ready at once.
#[svelters::export(module = "work")]
pub async fn local_now(x: u32) -> u32 {
    x + 1
}

/// A JS-thread async fn that offloads its work to a worker.
#[svelters::export(module = "work")]
pub async fn local_sum(n: u64) -> u64 {
    svelters::exports::worker(move || (1..=n).sum::<u64>()).await
}

/// Blocks the JS thread (debug builds warn).
#[svelters::export(module = "work")]
#[js(local)]
pub async fn local_slow() -> u32 {
    std::thread::sleep(Duration::from_millis(40));
    1
}

#[svelters::export(module = "work")]
pub async fn local_boom() -> u32 {
    panic!("local kaboom")
}

/// A worker panic resumed in the awaiting JS-thread future.
#[svelters::export(module = "work")]
pub async fn local_worker_boom() -> u32 {
    svelters::exports::worker(|| -> u32 { panic!("worker kaboom") }).await
}

struct DropNote(u32);
impl Drop for DropNote {
    fn drop(&mut self) {
        record(format!("future-drop {}", self.0));
    }
}

/// Never finishes on its own: an abort drops the future (on the JS thread).
#[svelters::export(module = "work")]
pub async fn local_forever(tag: u32) -> u32 {
    let _note = DropNote(tag);
    std::future::pending::<()>().await;
    tag
}

/// tokio (the `tokio` feature, on for these tests).
#[svelters::export(module = "work")]
#[js(tokio)]
pub async fn tokio_sleep(ms: u64) -> u64 {
    let _note = DropNote(ms as u32);
    tokio::time::sleep(Duration::from_millis(ms)).await;
    ms
}

/// A shared object with offloaded methods.
#[svelters::object(shared, module = "work")]
pub struct Engine {
    tag: u32,
    running: AtomicU32,
    peak: AtomicU32,
    started: AtomicU32,
}

impl Drop for Engine {
    fn drop(&mut self) {
        record(format!("engine-drop {}", self.tag));
    }
}

#[svelters::export]
impl Engine {
    #[js(constructor)]
    pub fn new(tag: u32) -> Engine {
        Engine { tag, running: AtomicU32::new(0), peak: AtomicU32::new(0), started: AtomicU32::new(0) }
    }

    /// A new call cancels the previous one.
    #[js(worker, supersede)]
    pub fn preview(&self, g: u32, cx: WorkCx) -> u32 {
        wait_gate(g, Some(&cx));
        g
    }

    /// At most two run at once.
    #[js(worker, max_in_flight = 2)]
    pub fn limited(self: Arc<Self>, g: u32) -> u32 {
        record(format!("limited-start {g}"));
        self.started.fetch_add(1, Ordering::SeqCst);
        let now = self.running.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak.fetch_max(now, Ordering::SeqCst);
        wait_gate(g, None);
        self.running.fetch_sub(1, Ordering::SeqCst);
        g
    }

    #[js(getter)]
    pub fn peak(&self) -> u32 {
        self.peak.load(Ordering::SeqCst)
    }

    #[js(getter)]
    pub fn started(&self) -> u32 {
        self.started.load(Ordering::SeqCst)
    }

    /// Holds the object until gate `g` opens or the call is cancelled.
    #[js(worker)]
    pub fn hold(self: Arc<Self>, g: u32, cx: WorkCx) -> u32 {
        wait_gate(g, Some(&cx));
        record(format!("hold-done {}", self.tag));
        g
    }
}

/// A UI object with a JS-thread async method borrowing `&mut self` across an await.
#[svelters::object(module = "work")]
pub struct Journal {
    lines: Vec<String>,
}

#[svelters::export]
impl Journal {
    #[js(constructor)]
    pub fn new() -> Journal {
        Journal { lines: Vec::new() }
    }

    pub async fn push_later(&mut self, s: String) -> u32 {
        let up = svelters::exports::worker(move || s.to_uppercase()).await;
        self.lines.push(up);
        self.lines.len() as u32
    }

    #[js(getter)]
    pub fn text(&self) -> String {
        self.lines.join(",")
    }
}

// ---- harness ----

fn stage(name: &str, main: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("svelters-offload-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("main.js"), main).unwrap();
    dir
}

struct Run<'h, 'a> {
    host: &'h Host<'a>,
    lines: Vec<String>,
}

impl Run<'_, '_> {
    /// Runs queued JS jobs only (no checkpoint, so no completion is drained).
    fn jobs(&mut self) {
        self.host.drain_jobs();
        self.lines.extend(self.host.take_console().into_iter().map(|l| l.text));
    }

    fn turn(&mut self) {
        self.host.checkpoint();
        self.host.drain_jobs();
        self.lines.extend(self.host.take_console().into_iter().map(|l| l.text));
    }

    fn until(&mut self, what: &str, cond: impl Fn(&Host<'_>, &[String]) -> bool) {
        let end = Instant::now() + Duration::from_secs(20);
        loop {
            self.turn();
            if cond(self.host, &self.lines) {
                return;
            }
            assert!(Instant::now() < end, "timed out waiting for {what}\nlines: {:#?}\nerrors: {:#?}", self.lines, self.host.take_errors());
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    fn line(&mut self, line: &str) {
        self.until(line, |_, l| l.iter().any(|x| x == line));
    }

    fn has(&self, line: &str) -> bool {
        self.lines.iter().any(|x| x == line)
    }

    fn no_errors(&self) {
        let e = self.host.take_errors();
        assert!(e.is_empty(), "{e:#?}\nlines: {:#?}", self.lines);
    }
}

fn with_host(name: &str, main: &str, f: impl FnOnce(&mut Run<'_, '_>, &LogBackend)) {
    let dir = stage(name, main);
    let (cfg, _) = fixture_config();
    let log = LogBackend::new(Vec::new());
    let vt = log.vtable();
    let host = Host::new(vt.backend_ref(), cfg).unwrap();
    if let Err(e) = host.load_bundle(dir.join("main.js")) {
        panic!("load: {e}");
    }
    let mut run = Run { host: &host, lines: Vec::new() };
    run.lines.extend(host.take_console().into_iter().map(|l| l.text));
    f(&mut run, &log);
}

// ---- tests ----

#[test]
fn worker_blocking_and_local_calls_resolve() {
    with_host(
        "resolve",
        r#"
import { gated, sleepy, localNow, localSum, Journal } from 'svelters:app/work';
localNow(1).then((v) => console.log('eager', v));
gated(101).then((v) => console.log('gated', v));
sleepy(5).then((v) => console.log('blocking', v));
localSum(100).then((v) => console.log('sum', v));
const j = new Journal();
j.pushLater('a').then((n) => j.pushLater('b')).then((n) => console.log('journal', n, j.text));
"#,
        |r, _| {
            // A JS-thread future that is ready at once settles within the call's turn (no drain needed).
            r.jobs();
            assert!(r.has("eager 2"), "{:?}", r.lines);
            assert!(r.host.offload_stats().settled_eagerly >= 1);
            open(101);
            r.line("gated 202");
            r.line("blocking svelters-blocking");
            r.line("sum 5050");
            r.line("journal 2 A,B");
            r.until("ops done", |h, _| h.ops_in_flight() == 0);
            r.no_errors();
        },
    );
}

#[test]
fn abort_rejects_at_once_and_drops_the_late_result() {
    with_host(
        "abort",
        r#"
import { latePayload, untilCancelled, gated, localForever } from 'svelters:app/work';
const live = () => globalThis.__svelters_internal.opsLive();
const c = new AbortController();
latePayload(201, { signal: c.signal }).then(
  () => console.log('resolved?!'),
  (e) => console.log('rejected', e.name, e.domain, e.code),
);
const before = live();
c.abort();
console.log('after-abort', before - live());
const c2 = new AbortController();
untilCancelled(202, { signal: c2.signal }).catch((e) => console.log('uc', e.code));
// Aborted once it runs (the test opens gate 205 when the job has started).
gated(205).then(() => c2.abort());
const c5 = new AbortController();
untilCancelled(206, { signal: c5.signal }).catch((e) => console.log('skipped', e.code));
c5.abort();
const c3 = new AbortController();
c3.abort();
gated(203, { signal: c3.signal }).catch((e) => console.log('pre', e.code));
const c4 = new AbortController();
localForever(204, { signal: c4.signal }).catch((e) => console.log('local', e.code));
c4.abort();
"#,
        |r, _| {
            // Rejected in the same turn as the abort, before any checkpoint drained a completion.
            r.jobs();
            assert_eq!(r.host.offload_stats().drains, 0);
            let at = |s: &str| r.lines.iter().position(|l| l == s).unwrap_or_else(|| panic!("no {s}: {:?}", r.lines));
            assert!(at("after-abort 1") < at("rejected NativeError svelters cancelled"));
            assert!(r.has("skipped cancelled") && r.has("pre cancelled") && r.has("local cancelled"), "{:?}", r.lines);
            // The JS-thread future was dropped by the abort, on this thread.
            assert!(recorded("future-drop 204"));
            // A running worker sees the cancel through WorkCx.
            let end = Instant::now() + Duration::from_secs(20);
            while !recorded("uc-start 202") && Instant::now() < end {
                std::thread::sleep(Duration::from_millis(1));
            }
            open(205);
            r.line("uc cancelled");
            r.until("worker saw cancel", |_, _| recorded("saw-cancel 202"));
            // A job cancelled while still queued never runs; an already-aborted call never starts.
            r.until("zombies done", |h, _| h.ops_in_flight() == 1);
            assert!(!recorded("uc-start 206") && !recorded("saw-cancel 203"));
            // The late result arrives after its call was settled: dropped, the object freed, no handle minted.
            assert_eq!(r.host.ops_in_flight(), 1, "the cancelled late_payload job still runs (a zombie)");
            let objects = r.host.live_app_objects();
            open(201);
            r.until("late result dropped", |h, _| h.ops_in_flight() == 0 && recorded("payload-drop 201"));
            assert!(recorded("late-done 201"));
            assert_eq!(r.host.live_app_objects(), objects, "a dropped result mints no handle");
            assert!(r.host.offload_stats().dropped >= 2, "{:?}", r.host.offload_stats());
            assert!(!r.has("resolved?!"));
            r.no_errors();
        },
    );
}

#[test]
fn progress_is_latest_value_once_per_checkpoint() {
    with_host(
        "progress",
        r#"
import { progressing } from 'svelters:app/work';
globalThis.seen = [];
progressing(5, 301, { onProgress: (p) => seen.push(p) }).then((v) => console.log('progress', v, JSON.stringify(seen)));
"#,
        |r, _| {
            // Wait (without checkpoints) until the worker reported 1..5: one delivery of the latest value.
            let end = Instant::now() + Duration::from_secs(20);
            while !recorded("reported 301") && Instant::now() < end {
                std::thread::sleep(Duration::from_millis(1));
            }
            r.turn();
            r.turn();
            open(301);
            let end = Instant::now() + Duration::from_secs(20);
            while !recorded("finished 301") && Instant::now() < end {
                std::thread::sleep(Duration::from_millis(1));
            }
            // The last progress value and the result arrive in one drain: progress first.
            r.line("progress 6 [5,6]");
            assert_eq!(r.host.offload_stats().progress, 2);
            r.no_errors();
        },
    );
}

#[test]
fn panics_become_native_errors() {
    with_host(
        "panics",
        r#"
import { boom, localBoom, localWorkerBoom } from 'svelters:app/work';
const show = (what) => (e) => console.log(what, e.name, e.domain, e.code, /kaboom/.test(e.message));
boom().catch(show('worker'));
localBoom().catch(show('local'));
localWorkerBoom().catch(show('local-worker'));
"#,
        |r, _| {
            r.line("worker NativeError svelters panic true");
            r.line("local NativeError svelters panic true");
            r.line("local-worker NativeError svelters panic true");
            r.no_errors();
        },
    );
}

#[test]
fn supersede_and_max_in_flight() {
    with_host(
        "caps",
        r#"
import { Engine } from 'svelters:app/work';
const e = new Engine(1);
e.preview(401).catch((x) => console.log('first', x.code));
e.preview(402).then((v) => console.log('second', v));
const ps = [411, 412, 413, 414].map((g) => e.limited(g));
Promise.all(ps).then((v) => console.log('limited', JSON.stringify(v), e.peak, e.started));
const c = new AbortController();
e.limited(415, { signal: c.signal }).catch((x) => console.log('queued', x.code));
c.abort();
"#,
        |r, _| {
            r.line("first superseded");
            r.until("superseded worker saw the cancel", |_, _| recorded("saw-cancel 401"));
            open(402);
            r.line("second 402");
            r.line("queued cancelled");
            // Two of the limited calls run; the others wait in order.
            r.until("two running", |_, _| recorded("limited-start 411") && recorded("limited-start 412"));
            std::thread::sleep(Duration::from_millis(30));
            r.turn();
            assert!(!recorded("limited-start 413") && !recorded("limited-start 414"));
            open(411);
            r.until("third starts", |_, _| recorded("limited-start 413"));
            assert!(!recorded("limited-start 414"));
            open(412);
            open(413);
            open(414);
            r.line("limited [411,412,413,414] 2 4");
            r.until("all done", |h, _| h.ops_in_flight() == 0);
            // The aborted queued call never ran.
            assert!(!recorded("limited-start 415"));
            r.no_errors();
        },
    );
}

#[test]
fn disposing_the_owner_rejects_in_flight_calls() {
    with_host(
        "dispose",
        r#"
import { Engine } from 'svelters:app/work';
const e = new Engine(5);
e.hold(501).then(() => console.log('resolved?!'), (x) => console.log('hold', x.domain, x.code));
e.dispose();
"#,
        |r, _| {
            r.line("hold svelters disposed");
            // The job saw the cancel and ended; it held the last reference, so the object drops after it.
            r.until("job ended", |h, _| h.ops_in_flight() == 0);
            assert!(recorded("saw-cancel 501"));
            r.until("engine dropped", |_, _| recorded("engine-drop 5"));
            assert_eq!(r.host.live_app_objects(), 0);
            assert!(!r.has("resolved?!"));
            r.no_errors();
        },
    );
}

#[test]
fn tokio_runs_and_aborts() {
    with_host(
        "tokio",
        r#"
import { tokioSleep } from 'svelters:app/work';
tokioSleep(5).then((v) => console.log('tokio', v));
const c = new AbortController();
tokioSleep(60000, { signal: c.signal }).catch((e) => console.log('tokio-abort', e.code));
c.abort();
"#,
        |r, _| {
            r.line("tokio-abort cancelled");
            r.line("tokio 5");
            // The aborted task's future is dropped at its next await; its slot is released.
            r.until("aborted task gone", |h, _| h.ops_in_flight() == 0 && recorded("future-drop 60000"));
            r.no_errors();
        },
    );
}

#[test]
fn completions_coalesce_wakes() {
    with_host(
        "many",
        r#"
import { noop } from 'svelters:app/work';
const ps = [];
for (let i = 0; i < 500; i++) ps.push(noop(i));
Promise.all(ps).then((v) => console.log('all', v.length, v[499]));
"#,
        |r, log| {
            let wakes0 = log.wake_count();
            let mut turns = 0u64;
            let end = Instant::now() + Duration::from_secs(20);
            while !r.has("all 500 499") {
                assert!(Instant::now() < end, "{:?}", r.lines);
                std::thread::sleep(Duration::from_millis(2));
                r.turn();
                turns += 1;
            }
            let wakes = log.wake_count() - wakes0;
            // One wake per burst: never more wakes than drains (each drain re-arms one).
            assert!(wakes <= turns + 1, "{wakes} wakes for {turns} checkpoints");
            let s = r.host.offload_stats();
            assert_eq!(s.settled, 500);
            r.no_errors();
        },
    );
}

#[cfg(debug_assertions)]
#[test]
fn long_local_poll_warns_in_debug_builds() {
    with_host(
        "slow",
        r#"
import { localSlow } from 'svelters:app/work';
localSlow().then((v) => console.log('slow', v));
localSlow().then((v) => console.log('slow again', v));
"#,
        |r, _| {
            r.line("slow again 1");
            let warns: Vec<_> = r.lines.iter().filter(|l| l.contains("blocked it for")).collect();
            assert_eq!(warns.len(), 1, "warned once per member: {:?}", r.lines);
            assert!(warns[0].contains("app:work.localSlow") && warns[0].contains("#[js(worker)]"), "{}", warns[0]);
        },
    );
}

#[test]
fn reload_drops_results_of_the_old_runtime() {
    let dir = stage(
        "reload",
        r#"
import { latePayload, gated } from 'svelters:app/work';
latePayload(601).then(() => console.log('late v1?!'), (e) => console.log('v1 rejected?!', e.code));
console.log('v1');
"#,
    );
    let (cfg, _) = fixture_config();
    let log = LogBackend::new(Vec::new());
    let vt = log.vtable();
    let host = Host::new(vt.backend_ref(), cfg).unwrap();
    host.load_bundle(dir.join("main.js")).unwrap();
    let mut r = Run { host: &host, lines: Vec::new() };
    r.turn();
    assert!(r.has("v1"));
    assert_eq!(host.ops_in_flight(), 1);
    std::fs::write(
        dir.join("main.js"),
        r#"
import { gated } from 'svelters:app/work';
gated(602).then((v) => console.log('v2 gated', v));
console.log('v2');
"#,
    )
    .unwrap();
    let reloaded = std::rc::Rc::new(std::cell::Cell::new(false));
    let flag = reloaded.clone();
    host.on_reload(move |rep| flag.set(rep.ok));
    host.request_reload();
    // The reload runs from the backend's checkpoint callback.
    unsafe { svelters_abi::HostRef::from_raw(host.host_callbacks()) }.checkpoint();
    assert!(reloaded.get(), "{:#?}", host.take_errors());
    r.turn();
    assert!(r.has("v2"));
    let dropped = host.offload_stats().dropped;
    open(601);
    open(602);
    r.line("v2 gated 1204");
    r.until("old result dropped", |h, _| h.ops_in_flight() == 0 && recorded("payload-drop 601"));
    assert_eq!(host.offload_stats().dropped, dropped + 1);
    assert!(!r.lines.iter().any(|l| l.contains("?!")), "{:?}", r.lines);
    assert_eq!(host.live_app_objects(), 0);
    r.no_errors();
    drop(r);
}

#[test]
fn typings_show_promises_and_options() {
    let dts = svelters::exports::typescript();
    for want in [
        "import type { NativeObject, AsyncOptions, WorkOptions } from '@svelters/types/core';",
        "export function gated(g: number, options?: AsyncOptions): Promise<number>;",
        "export function progressing(n: number, g: number, options?: WorkOptions<number>): Promise<number>;",
        "export function latePayload(g: number, options?: AsyncOptions): Promise<Payload>;",
        "export function localNow(x: number, options?: AsyncOptions): Promise<number>;",
        "preview(g: number, options?: AsyncOptions): Promise<number>;",
        "pushLater(s: string, options?: AsyncOptions): Promise<number>;",
        "Runs on a worker thread (rayon); a new call supersedes the one in flight.",
        "Runs on a worker thread (rayon); at most 2 in flight per object, more wait.",
        "Runs on the JS thread (async).",
        "Runs on tokio.",
        "Runs on the blocking pool.",
    ] {
        assert!(dts.contains(want), "typings lack {want:?}:\n{dts}");
    }
    let js = svelters::exports::javascript("work", "@svelters/types/core", &|m| format!("./{m}.js")).unwrap();
    assert!(js.contains("return callStaticAsync('app:work', 'latePayload', [g], options, (x) => wrapObject(Payload, x));"), "{js}");
    assert!(js.contains("return callAsync(this, 'preview', [g], options);"), "{js}");
    let json: serde_json::Value = serde_json::from_str(&svelters::exports::json()).unwrap();
    let fns = json["modules"][0]["functions"].as_array().unwrap();
    let f = |n: &str| fns.iter().find(|f| f["name"] == n).unwrap().clone();
    assert_eq!(f("gated")["runsOn"], "worker");
    assert_eq!(f("sleepy")["runsOn"], "blocking");
    assert_eq!(f("localNow")["runsOn"], "local");
    assert_eq!(f("tokioSleep")["runsOn"], "tokio");
    assert_eq!(f("progressing")["progress"], "number");
    assert_eq!(f("gated")["async"], true);
}
