//! E3 reactive exports end to end on the log backend: stores in the R1 change batch (10k worker writes,
//! one batch), writable fields through `on_write` (a clamp delivered once, the echo dropped), proportional
//! fields, reactive getters, `Emitter` events (order, `latest`, after the batch, flood warning), streams
//! (`Sink<T>` with backpressure, early break, abort, an error after items, `Stream<T>` results,
//! `batches()`), `persist` and app-wide stores across hot reloads, and the typings.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use svelters::prelude::*;
use svelters_backend_log::LogBackend;
use svelters_host::{Host, HostConfig, testing::fixture_config};

// ---- records and gates shared by the exports and the tests (distinct numbers per test) ----

static RECORD: Mutex<Vec<String>> = Mutex::new(Vec::new());
static GATES: Mutex<Vec<u32>> = Mutex::new(Vec::new());

fn record(s: String) {
    RECORD.lock().unwrap().push(s);
}

fn recorded(s: &str) -> bool {
    RECORD.lock().unwrap().iter().any(|x| x == s)
}

fn records_with(prefix: &str) -> Vec<String> {
    RECORD.lock().unwrap().iter().filter(|x| x.starts_with(prefix)).cloned().collect()
}

fn open(g: u32) {
    GATES.lock().unwrap().push(g);
}

fn await_record(s: &str) {
    let end = Instant::now() + Duration::from_secs(20);
    while !recorded(s) {
        assert!(Instant::now() < end, "timed out waiting for the record {s:?}");
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn pool() {
    let _ = svelters::exports::set_worker_pool(svelters::rayon::ThreadPoolBuilder::new().num_threads(16).build().unwrap());
}

// ---- the exports: svelters:app/rx ----

/// Simulation statistics.
#[svelters::store(module = "rx")]
#[derive(Default)]
pub struct Stats {
    /// Steps so far.
    pub count: u64,
    pub energy: f64,
    pub running: bool,
    /// Steps per frame, clamped to 0.1..=10 by `on_write`.
    #[store(writable)]
    pub speed: f64,
    #[store(writable)]
    pub label: String,
    /// A growing log: delivered as an invalidation.
    #[store(proportional)]
    pub log: String,
}

impl StoreHooks for Stats {
    fn on_write(&mut self, field: StatsField) -> Result<(), NativeError> {
        match field {
            StatsField::Speed => {
                if !self.speed.is_finite() {
                    return Err(NativeError { domain: "app:rx".into(), code: svelters::Value::Str("bad-speed".into()), message: "speed must be finite".into() });
                }
                self.speed = self.speed.clamp(0.1, 10.0);
            }
            StatsField::Label if self.label.len() > 10 => {
                return Err(NativeError { domain: "app:rx".into(), code: svelters::Value::Str("too-long".into()), message: "at most 10 characters".into() });
            }
            _ => {}
        }
        Ok(())
    }
}

/// A store of its own (one per test).
#[svelters::export(module = "rx")]
pub fn fresh_stats() -> Store<Stats> {
    Store::new(Stats { speed: 1.0, ..Default::default() })
}

/// `n` single-field writes on a worker thread.
#[svelters::export(module = "rx")]
#[js(worker)]
pub fn bump(s: Store<Stats>, n: u32, tag: u32) -> u64 {
    for _ in 0..n {
        s.write(StatsField::Count, |st| st.count += 1);
    }
    record(format!("bumped {tag}"));
    s.get(|st| st.count)
}

/// Marks every field without changing any.
#[svelters::export(module = "rx")]
pub fn touch_all(s: Store<Stats>) {
    s.update(|_| ());
}

/// A Rust-side write of the writable field (no hook: Rust is trusted).
#[svelters::export(module = "rx")]
pub fn set_speed_from_rust(s: Store<Stats>, v: f64) {
    s.write(StatsField::Speed, |st| st.speed = v);
}

#[svelters::export(module = "rx")]
#[js(worker)]
pub fn append_log(s: Store<Stats>, line: String) -> u32 {
    s.write(StatsField::Log, |st| {
        st.log.push_str(&line);
        st.log.push('\n');
        st.log.len() as u32
    })
}

/// An app-wide store: the same Rust object in every JS runtime of the process.
#[svelters::export(module = "rx")]
pub static APP_STATS: StoreStatic<Stats> = StoreStatic::new();

#[svelters::export(module = "rx")]
pub fn bump_app(n: u32) -> u64 {
    APP_STATS.write(StatsField::Count, |s| s.count += n as u64);
    APP_STATS.get(|s| s.count)
}

#[derive(svelters::Events, Clone)]
pub enum SensorEvent {
    /// A reading.
    Reading { value: f64, seq: u32 },
    /// The latest level.
    #[event(latest)]
    Level(f64),
    Reset,
}

/// A sensor on worker threads.
#[svelters::object(shared, module = "rx")]
pub struct Sensor {
    events: Emitter<SensorEvent>,
    stats: Store<Stats>,
}

#[svelters::export]
impl Sensor {
    #[js(constructor)]
    pub fn new(stats: Store<Stats>) -> Sensor {
        Sensor { events: Emitter::new(), stats }
    }

    /// Updates the store, then emits `n` readings and `n` levels, on a worker.
    #[js(worker)]
    pub fn burst(self: Arc<Self>, n: u32) -> u32 {
        self.stats.write(StatsField::Count, |s| s.count = n as u64);
        for i in 0..n {
            self.events.emit(SensorEvent::Reading { value: i as f64 * 0.5, seq: i });
            self.events.emit(SensorEvent::Level(i as f64));
        }
        n
    }

    /// Emits `reset`; returns how many hosts listened.
    pub fn reset(&self) -> u32 {
        self.events.emit(SensorEvent::Reset) as u32
    }
}

/// Sends 1..=n (bounded: at most 4 unread), recording each send.
#[svelters::export(module = "rx")]
#[js(worker, buffer = 4)]
pub fn count_to(n: u32, tag: u32, out: Sink<u32>) -> Result<u32, Closed> {
    for i in 1..=n {
        if let Err(c) = out.send(i) {
            record(format!("closed {tag} at {i}"));
            return Err(c);
        }
        record(format!("sent {tag} {i}"));
    }
    Ok(n)
}

/// Sends 1..=n, then fails.
#[svelters::export(module = "rx")]
#[js(worker)]
pub fn fail_after(n: u32, out: Sink<u32>) -> Result<(), NativeError> {
    for i in 1..=n {
        out.send(i)?;
    }
    Err(NativeError { domain: "app:rx".into(), code: svelters::Value::Str("boom".into()), message: "failed after the items".into() })
}

/// Waits for gate `g` on a worker.
#[svelters::export(module = "rx")]
#[js(worker)]
pub fn wait_gate(g: u32) -> u32 {
    let end = Instant::now() + Duration::from_secs(20);
    while !GATES.lock().unwrap().contains(&g) && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(1));
    }
    g
}

/// `0..n` from a plain thread through a returned stream; the stream ends when the thread drops its sink.
#[svelters::export(module = "rx")]
pub fn numbers(n: u32, tag: u32) -> Stream<u32> {
    let (tx, rx) = channel(2);
    std::thread::spawn(move || {
        for i in 0..n {
            if tx.send(i).is_err() {
                record(format!("numbers {tag} closed"));
                return;
            }
        }
        record(format!("numbers {tag} done"));
    });
    rx
}

/// A UI object that records its drop.
#[svelters::object(module = "rx")]
pub struct Counter {
    value: i64,
    tag: u32,
}

impl Drop for Counter {
    fn drop(&mut self) {
        record(format!("counter-drop {}", self.tag));
    }
}

#[svelters::export]
impl Counter {
    #[js(constructor)]
    pub fn new(start: i64, tag: u32) -> Counter {
        Counter { value: start, tag }
    }

    #[js(getter)]
    pub fn value(&self) -> i64 {
        self.value
    }

    pub fn add(&mut self, n: i64) -> i64 {
        self.value += n;
        self.value
    }

    /// A stream of the counter's next `n` values (a returned stream on an object).
    pub fn upcoming(&self, n: u32) -> Stream<i64> {
        let (tx, rx) = channel(8);
        let start = self.value;
        std::thread::spawn(move || {
            for i in 1..=n as i64 {
                let _ = tx.send(start + i);
            }
        });
        rx
    }
}

/// Keeps bytes from JS: the JS memory itself (`keep`: pins the runtime that made it) or a copy in Rust
/// memory (`keep_copy`: `Buffer::into_vec`).
#[svelters::object(module = "rx")]
pub struct Keeper {
    held: Option<svelters::Buffer<u8>>,
    copy: Vec<u8>,
}

#[svelters::export]
impl Keeper {
    #[js(constructor)]
    pub fn new() -> Keeper {
        Keeper { held: None, copy: Vec::new() }
    }

    pub fn keep(&mut self, b: svelters::Buffer<u8>) -> u32 {
        let n = b.len() as u32;
        self.held = Some(b);
        n
    }

    pub fn keep_copy(&mut self, b: svelters::Buffer<u8>) -> u32 {
        self.copy = b.into_vec();
        self.copy.len() as u32
    }
}

// ---- harness ----

fn stage(name: &str, main: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("svelters-reactive-{name}-{}", std::process::id()));
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

    fn count(&self, prefix: &str) -> usize {
        self.lines.iter().filter(|x| x.starts_with(prefix)).count()
    }

    fn no_errors(&self) {
        let e = self.host.take_errors();
        assert!(e.is_empty(), "{e:#?}\nlines: {:#?}", self.lines);
    }
}

fn with_host(name: &str, main: &str, f: impl FnOnce(&mut Run<'_, '_>)) {
    with_config(name, main, fixture_config().0, f)
}

fn with_config(name: &str, main: &str, cfg: HostConfig, f: impl FnOnce(&mut Run<'_, '_>)) {
    pool();
    let dir = stage(name, main);
    let log = LogBackend::new(Vec::new());
    let vt = log.vtable();
    let host = Host::new(vt.backend_ref(), cfg).unwrap();
    if let Err(e) = host.load_bundle(dir.join("main.js")) {
        panic!("load: {e}");
    }
    let mut run = Run { host: &host, lines: Vec::new() };
    run.lines.extend(host.take_console().into_iter().map(|l| l.text));
    f(&mut run);
}

/// JS that logs every `__svelters_changed` entry as `changed <prop>=<value>` and every batch as `batch <n>`.
const WATCH: &str = r#"
const H = globalThis.__svelters_host;
globalThis.__svelters_changed = (b) => {
  console.log(`batch ${b.length / 3}`);
  for (let i = 0; i < b.length; i += 3) console.log(`changed ${b[i + 1]}=${b[i + 2]}`);
};
"#;

// ---- stores ----

#[test]
fn ten_thousand_worker_writes_are_one_batch() {
    with_host(
        "batch",
        &format!(
            "{WATCH}
import {{ freshStats, bump }} from 'svelters:app/rx';
const s = freshStats();
H.observe(s.svlHandle, 'count', true);
H.observe(s.svlHandle, 'energy', true);
bump(s, 10000, 1).then((v) => console.log('done', v));
"
        ),
        |r| {
            // The worker writes 10k times while no checkpoint runs; then one checkpoint delivers once.
            await_record("bumped 1");
            let s0 = r.host.reactive_stats();
            let t0 = Instant::now();
            r.turn();
            let took = t0.elapsed();
            r.line("done 10000");
            let s = r.host.reactive_stats();
            assert_eq!(r.count("batch "), 1, "{:?}", r.lines);
            assert!(r.has("changed count=10000"), "{:?}", r.lines);
            assert_eq!(r.count("changed "), 1, "only `count` changed: {:?}", r.lines);
            assert_eq!(s.store_batches - s0.store_batches, 1);
            assert_eq!(s.store_delivered - s0.store_delivered, 1);
            println!("10k worker writes: 1 batch, 1 field delivered, checkpoint {:.3} ms", took.as_secs_f64() * 1e3);
            r.no_errors();
        },
    );
}

#[test]
fn update_marks_every_field_and_the_compare_drops_the_unchanged() {
    with_host(
        "compare",
        &format!(
            "{WATCH}
import {{ freshStats, touchAll }} from 'svelters:app/rx';
const s = freshStats();
for (const p of ['count', 'energy', 'running', 'speed', 'label']) H.observe(s.svlHandle, p, true);
touchAll(s);
console.log('touched');
"
        ),
        |r| {
            let s0 = r.host.reactive_stats();
            r.turn();
            r.turn();
            let s = r.host.reactive_stats();
            assert!(r.has("touched"));
            assert_eq!(r.count("batch "), 0, "nothing changed: {:?}", r.lines);
            assert_eq!(s.store_unchanged - s0.store_unchanged, 5, "{s:?}");
            r.no_errors();
        },
    );
}

#[test]
fn writable_field_clamps_once_and_drops_the_echo() {
    with_host(
        "clamp",
        &format!(
            "{WATCH}
import {{ freshStats, setSpeedFromRust }} from 'svelters:app/rx';
const s = freshStats();
globalThis.s = s;
H.observe(s.svlHandle, 'speed', true);
s.speed = 50;
console.log('wrote 50, reads', s.speed);
"
        ),
        |r| {
            assert!(r.has("wrote 50, reads 10"), "{:?}", r.lines);
            r.turn();
            assert_eq!(r.count("changed speed="), 1, "{:?}", r.lines);
            assert!(r.has("changed speed=10"), "the clamped value, once: {:?}", r.lines);
            r.turn();
            r.turn();
            assert_eq!(r.count("changed speed="), 1, "the store's own dirty bit for the write is an echo: {:?}", r.lines);
            r.no_errors();
        },
    );
}

#[test]
fn accepted_writes_echo_nothing_and_rejected_writes_throw() {
    with_host(
        "accepted",
        &format!(
            "{WATCH}
import {{ freshStats, setSpeedFromRust }} from 'svelters:app/rx';
const s = freshStats();
H.observe(s.svlHandle, 'speed', true);
H.observe(s.svlHandle, 'label', true);
s.speed = 5;
s.label = 'ok';
try {{ s.speed = NaN; }} catch (e) {{ console.log('rejected', e.code, s.speed); }}
try {{ s.label = 'much too long'; }} catch (e) {{ console.log('rejected', e.code, s.label); }}
try {{ s.count = 3; }} catch (e) {{ console.log('read-only', e.name); }}
setSpeedFromRust(s, 7);
console.log('end');
"
        ),
        |r| {
            r.line("end");
            assert!(r.has("rejected bad-speed 5"), "{:?}", r.lines);
            assert!(r.has("rejected too-long ok"), "{:?}", r.lines);
            assert!(r.has("read-only TypeError"), "{:?}", r.lines);
            r.turn();
            r.turn();
            // Only the Rust-side write is news; JS's own accepted writes are echoes.
            let changed: Vec<_> = r.lines.iter().filter(|l| l.starts_with("changed ")).cloned().collect();
            assert_eq!(changed, vec!["changed speed=7".to_string()], "{:?}", r.lines);
            r.no_errors();
        },
    );
}

#[test]
fn proportional_fields_are_invalidations() {
    with_host(
        "proportional",
        &format!(
            "{WATCH}
import {{ freshStats, appendLog }} from 'svelters:app/rx';
const s = freshStats();
H.observe(s.svlHandle, 'log', true);
appendLog(s, 'one').then(() => appendLog(s, 'two')).then((n) => console.log('appended', n, JSON.stringify(s.log)));
"
        ),
        |r| {
            r.line("appended 8 \"one\\ntwo\\n\"");
            r.turn();
            assert!(r.count("changed log=undefined") >= 1, "{:?}", r.lines);
            assert!(!r.lines.iter().any(|l| l.starts_with("changed log=") && !l.ends_with("undefined")), "{:?}", r.lines);
            r.no_errors();
        },
    );
}

#[test]
fn getters_subscribe_through_the_tracker() {
    with_host(
        "tracker",
        r#"
import { installTracker } from 'svelters:core';
import { freshStats } from 'svelters:app/rx';
const H = globalThis.__svelters_host;
const subs = [];
installTracker({ track(h, p) { subs.push(p); H.observe(h, p, true); }, local() {}, invalidate() {}, untrack: (f) => f() });
const s = freshStats();
console.log('read', s.speed, s.count, subs.join(','));
globalThis.s = s;
"#,
        |r| {
            assert!(r.has("read 1 0 speed,count"), "{:?}", r.lines);
            assert_eq!(r.host.observed_props(), 2);
            r.no_errors();
        },
    );
}

// ---- events ----

#[test]
fn events_reach_listeners_in_order_after_the_change_batch() {
    with_host(
        "events",
        &format!(
            "{WATCH}
import {{ freshStats, Sensor }} from 'svelters:app/rx';
const s = freshStats();
H.observe(s.svlHandle, 'count', true);
const sensor = new Sensor(s);
const seqs = [];
const levels = [];
const offReading = sensor.on('reading', (e) => {{ if (seqs.length === 0) console.log('first reading sees count', s.count); seqs.push(e.seq); }});
sensor.on('level', (v) => levels.push(v));
globalThis.keep = sensor;
console.log('unheard reset', sensor.reset());
sensor.burst(300).then((n) => {{
  console.log('burst', n, seqs.length, seqs.every((x, i) => x === i), 'levels', levels.length, levels[levels.length - 1]);
  offReading();
  return sensor.burst(5);
}}).then(() => console.log('after off', seqs.length));
"
        ),
        |r| {
            assert!(r.has("unheard reset 0"), "an event nobody listens to goes nowhere: {:?}", r.lines);
            r.line("after off 300");
            // Readings in order; the `latest` level at most once per checkpoint (the harness may run one
            // while the worker is still emitting), ending with the last value.
            let line = r.lines.iter().find(|l| l.starts_with("burst 300 ")).cloned().expect("burst line");
            assert!(line.starts_with("burst 300 300 true levels ") && line.ends_with(" 299"), "{line}");
            let levels: u64 = line.split(' ').nth(5).unwrap().parse().unwrap();
            assert!(levels >= 1 && levels <= r.host.checkpoint_count(), "{levels} level deliveries");
            // The change batch came first: the first handler already saw the store's new count.
            assert!(r.has("first reading sees count 300"), "{:?}", r.lines);
            let first_batch = r.lines.iter().position(|l| l == "changed count=300").expect("count delivered");
            let first_reading = r.lines.iter().position(|l| l == "first reading sees count 300").unwrap();
            assert!(first_batch < first_reading, "{:?}", r.lines);
            let s = r.host.reactive_stats();
            assert!(s.events >= 300 + 1, "{s:?}");
            r.no_errors();
        },
    );
}

#[cfg(debug_assertions)]
#[test]
fn an_event_flood_warns_once() {
    with_host(
        "flood",
        r#"
import { freshStats, Sensor } from 'svelters:app/rx';
const sensor = new Sensor(freshStats());
let n = 0;
sensor.on('reading', () => n++);
globalThis.keep = sensor;
sensor.burst(1500).then(() => sensor.burst(1500)).then(() => console.log('readings', n));
"#,
        |r| {
            r.line("readings 3000");
            let warns: Vec<_> = r.lines.iter().filter(|l| l.contains("more than 1000 events")).collect();
            assert_eq!(warns.len(), 1, "{:?}", r.lines);
            r.no_errors();
        },
    );
}

// ---- streams ----

#[test]
fn a_sink_stream_is_bounded_and_ordered() {
    with_host(
        "bounded",
        r#"
import { countTo, waitGate } from 'svelters:app/rx';
const it = countTo(20, 1);
waitGate(801).then(async () => {
  const got = [];
  for await (const x of it) got.push(x);
  console.log('got', got.join(','));
});
"#,
        |r| {
            await_record("sent 1 4");
            for _ in 0..20 {
                r.turn();
                std::thread::sleep(Duration::from_millis(2));
            }
            assert!(!recorded("sent 1 5"), "the producer runs at most 4 items ahead: {:?}", records_with("sent 1"));
            open(801);
            r.line("got 1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20");
            r.until("op done", |h, _| h.ops_in_flight() == 0);
            r.no_errors();
        },
    );
}

#[test]
fn leaving_the_loop_closes_the_stream() {
    with_host(
        "break",
        r#"
import { countTo } from 'svelters:app/rx';
(async () => {
  let n = 0;
  for await (const x of countTo(1000, 2)) {
    if (++n === 3) break;
  }
  console.log('left after', n);
})();
"#,
        |r| {
            r.line("left after 3");
            r.until("producer closed", |_, _| !records_with("closed 2 at ").is_empty());
            r.until("op done", |h, _| h.ops_in_flight() == 0);
            assert!(!recorded("sent 2 1000"));
            r.no_errors();
        },
    );
}

#[test]
fn a_stream_dropped_unread_is_closed_when_collected() {
    with_host(
        "dropped",
        r#"
import { countTo } from 'svelters:app/rx';
countTo(1000, 6);
console.log('started');
"#,
        |r| {
            // The producer fills the buffer and blocks; nothing will ever read it.
            await_record("sent 6 4");
            r.until("producer closed", |h, _| {
                h.collect_garbage();
                !records_with("closed 6 at ").is_empty()
            });
            r.until("op done", |h, _| h.ops_in_flight() == 0);
            r.no_errors();
        },
    );
}

#[test]
fn an_abort_closes_the_stream_and_rejects() {
    with_host(
        "abort",
        r#"
import { countTo } from 'svelters:app/rx';
const ac = new AbortController();
(async () => {
  try {
    for await (const x of countTo(1000, 3, { signal: ac.signal })) {
      if (x === 2) ac.abort();
    }
    console.log('ended?!');
  } catch (e) {
    console.log('aborted', e.code);
  }
})();
"#,
        |r| {
            r.line("aborted cancelled");
            r.until("producer closed", |_, _| !records_with("closed 3 at ").is_empty());
            r.until("op done", |h, _| h.ops_in_flight() == 0);
            r.no_errors();
        },
    );
}

#[test]
fn an_error_rejects_after_the_items_sent_before_it() {
    with_host(
        "fail",
        r#"
import { failAfter } from 'svelters:app/rx';
(async () => {
  const got = [];
  try {
    for await (const x of failAfter(3)) got.push(x);
  } catch (e) {
    console.log('items', got.join(','), 'then', e.code);
  }
})();
"#,
        |r| {
            r.line("items 1,2,3 then boom");
            r.no_errors();
        },
    );
}

#[test]
fn returned_streams_end_when_their_sinks_are_dropped() {
    with_host(
        "returned",
        r#"
import { numbers, Counter } from 'svelters:app/rx';
(async () => {
  const got = [];
  for await (const x of numbers(10, 4)) got.push(x);
  console.log('numbers', got.join(','));
  let total = 0;
  let batches = 0;
  for await (const b of numbers(50, 5).batches()) { total += b.length; batches++; }
  console.log('batched', total, batches > 0);
  const c = new Counter(40, 700);
  const up = [];
  for await (const v of c.upcoming(3)) up.push(v);
  console.log('upcoming', up.join(','));
})();
"#,
        |r| {
            r.line("upcoming 41,42,43");
            assert!(r.has("numbers 0,1,2,3,4,5,6,7,8,9"), "{:?}", r.lines);
            assert!(r.has("batched 50 true"), "{:?}", r.lines);
            assert!(recorded("numbers 4 done") && recorded("numbers 5 done"));
            r.until("ops done", |h, _| h.ops_in_flight() == 0);
            r.no_errors();
        },
    );
}

// ---- hot reload: persist, app-wide stores ----

#[test]
fn persisted_objects_and_app_stores_survive_a_reload() {
    pool();
    let v1 = format!(
        "{WATCH}
import {{ Counter, appStats, bumpApp }} from 'svelters:app/rx';
import {{ persist }} from 'svelters:core';
const c = persist('counter', () => new Counter(5, 901));
c.add(1);
globalThis.keep = [c, new Counter(100, 902)];
bumpApp(7);
H.observe(appStats.svlHandle, 'count', true);
console.log('v1', c.value, appStats.count);
"
    );
    let dir = stage("reload", &v1);
    let (cfg, _) = fixture_config();
    let log = LogBackend::new(Vec::new());
    let vt = log.vtable();
    let host = Host::new(vt.backend_ref(), cfg).unwrap();
    host.load_bundle(dir.join("main.js")).unwrap();
    let mut r = Run { host: &host, lines: Vec::new() };
    r.turn();
    assert!(r.has("v1 6 7"), "{:?}", r.lines);
    assert_eq!(host.persisted(), vec!["counter".to_string()]);
    let reloads = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let seen = reloads.clone();
    host.on_reload(move |rep| seen.borrow_mut().push(rep.ok));
    let reload = |src: &str| {
        std::fs::write(dir.join("main.js"), src).unwrap();
        host.request_reload();
        // The reload runs from the backend's checkpoint callback.
        unsafe { svelters_abi::HostRef::from_raw(host.host_callbacks()) }.checkpoint();
    };

    // A failed reload: the old runtime keeps its observation of the app-wide store, and what the failed
    // bundle persisted before it threw is not kept.
    reload(
        r#"
import { Counter, appStats } from 'svelters:app/rx';
import { persist } from 'svelters:core';
persist('other', () => new Counter(1, 904));
globalThis.__svelters_host.observe(appStats.svlHandle, 'count', true);
throw new Error('broken v2');
"#,
    );
    assert_eq!(*reloads.borrow(), vec![false]);
    let _ = host.take_errors();
    assert_eq!(host.persisted(), vec!["counter".to_string()]);
    r.turn();
    assert!(recorded("counter-drop 904"), "the failed runtime's object was released");
    APP_STATS.write(StatsField::Count, |s| s.count += 1);
    r.turn();
    assert!(r.has("changed count=8"), "{:?}", r.lines);

    // A good reload: the persisted counter and the app-wide store are the same Rust objects; the other
    // counter the old runtime made dies with it.
    reload(&format!(
        "{WATCH}
import {{ Counter, appStats }} from 'svelters:app/rx';
import {{ persist }} from 'svelters:core';
const c = persist('counter', () => new Counter(0, 903));
globalThis.keep = c;
H.observe(appStats.svlHandle, 'count', true);
console.log('v2', c.value, c instanceof Counter, appStats.count);
"
    ));
    assert_eq!(*reloads.borrow(), vec![false, true], "{:#?}", host.take_errors());
    r.turn();
    assert!(r.has("v2 6 true 8"), "{:?}", r.lines);
    assert!(recorded("counter-drop 902"), "the unpersisted counter died with its runtime");
    assert!(!recorded("counter-drop 901"), "the persisted counter lives on");
    assert_eq!(host.observed_props(), 1, "only the new runtime observes the store");
    let before = r.count("changed count=");
    APP_STATS.write(StatsField::Count, |s| s.count += 1);
    r.turn();
    assert_eq!(r.count("changed count=") - before, 1, "delivered once: {:?}", r.lines);
    assert!(r.has("changed count=9"), "{:?}", r.lines);
    r.no_errors();
    drop(r);
}

/// E2 x E3: a persisted object that keeps a JS `Buffer<T>` pins the old runtime's memory, so a reload waits
/// 1 s and leaks that runtime; the host says so at the reload. Keeping a copy (`into_vec`) avoids both.
#[test]
fn a_persisted_object_holding_js_memory_is_diagnosed_at_reload() {
    pool();
    let reload = |name: &str, method: &str| -> (Vec<String>, u64, Duration) {
        let v1 = format!(
            "import {{ Keeper }} from 'svelters:app/rx';\nimport {{ persist }} from 'svelters:core';\nconst k = persist('keeper', () => new Keeper());\nconsole.log('kept', k.{method}(new Uint8Array(32)));\n"
        );
        let dir = stage(name, &v1);
        let (cfg, _) = fixture_config();
        let log = LogBackend::new(Vec::new());
        let vt = log.vtable();
        let host = Host::new(vt.backend_ref(), cfg).unwrap();
        host.load_bundle(dir.join("main.js")).unwrap();
        let mut r = Run { host: &host, lines: Vec::new() };
        r.turn();
        assert!(r.has("kept 32"), "{:?}", r.lines);
        let leaked = host.buffer_stats().leaked_runtimes;
        std::fs::write(dir.join("main.js"), "import { Keeper } from 'svelters:app/rx';\nimport { persist } from 'svelters:core';\nconsole.log('v2', persist('keeper', () => new Keeper()) instanceof Keeper);\n").unwrap();
        let rep = std::rc::Rc::new(std::cell::Cell::new(None));
        let flag = rep.clone();
        host.on_reload(move |x| flag.set(Some((x.ok, x.elapsed))));
        host.request_reload();
        unsafe { svelters_abi::HostRef::from_raw(host.host_callbacks()) }.checkpoint();
        let (ok, took) = rep.get().expect("reloaded");
        assert!(ok, "{:#?}", host.take_errors());
        r.turn();
        assert!(r.has("v2 true"), "{:?}", r.lines);
        r.no_errors();
        let lines = r.lines.clone();
        (lines, host.buffer_stats().leaked_runtimes - leaked, took)
    };
    let warns = |lines: &[String]| lines.iter().filter(|l| l.contains("from the old JS runtime are still held by Rust")).count();

    let (lines, leaked, took) = reload("pin-held", "keep");
    assert_eq!(warns(&lines), 1, "{lines:?}");
    assert!(lines.iter().any(|l| l.contains("Buffer::into_vec()")), "{lines:?}");
    assert_eq!(leaked, 1, "the old runtime is leaked: Rust still holds its memory");
    assert!(took >= Duration::from_millis(900), "{took:?}");

    let (lines, leaked, took) = reload("pin-copy", "keepCopy");
    assert_eq!(warns(&lines), 0, "{lines:?}");
    assert_eq!(leaked, 0);
    assert!(took < Duration::from_millis(900), "{took:?}");
}

// ---- typings ----

#[test]
fn typings_show_stores_values_events_and_streams() {
    let dts = svelters::exports::typescript();
    for want in [
        "import type { NativeObject, AsyncOptions, NativeStream } from '@svelters/types/core';",
        "export class Stats extends NativeObject {",
        "readonly count: number;",
        "speed: number;",
        "readonly log: string;",
        "export function freshStats(): Stats;",
        "export const appStats: Stats;",
        "App-wide (a Rust `static`): the same Rust object after a hot reload.",
        "on(event: 'reading', fn: (e: { value: number; seq: number; }) => void): () => void;",
        "on(event: 'level', fn: (e: number) => void): () => void;",
        "on(event: 'reset', fn: () => void): () => void;",
        "Latest value: delivered at most once per checkpoint.",
        "export function countTo(n: number, tag: number, options?: AsyncOptions): NativeStream<number>;",
        "export function numbers(n: number, tag: number, options?: AsyncOptions): NativeStream<number>;",
        "upcoming(n: number, options?: AsyncOptions): NativeStream<number>;",
    ] {
        assert!(dts.contains(want), "typings lack {want:?}:\n{dts}");
    }
    let js = svelters::exports::javascript("rx", "@svelters/types/core", &|m| format!("./{m}.js")).unwrap();
    for want in [
        "get speed() {\n    return observed(this, 'speed');",
        "set speed(v) {\n    setProp(this, 'speed', v);",
        "export const appStats = /* @__PURE__ */ ((x) => wrapObject(Stats, x))(callStatic('app:rx', 'appStats', []));",
        "return streamStatic('app:rx', 'countTo', [n, tag], options);",
        "return stream(this, 'upcoming', [n], options);",
        "registerClass(Counter, Keeper, Sensor, Stats);",
    ] {
        assert!(js.contains(want), "stub lacks {want:?}:\n{js}");
    }
    let json: serde_json::Value = serde_json::from_str(&svelters::exports::json()).unwrap();
    let m = json["modules"].as_array().unwrap().iter().find(|m| m["name"] == "rx").unwrap().clone();
    let stats = m["classes"].as_array().unwrap().iter().find(|c| c["name"] == "Stats").unwrap().clone();
    assert_eq!(stats["store"], true);
    let prop = |n: &str| stats["props"].as_array().unwrap().iter().find(|p| p["name"] == n).unwrap().clone();
    assert_eq!(prop("speed")["observability"], "exact");
    assert_eq!(prop("speed")["verified"], "delivery");
    assert_eq!(prop("speed")["readonly"], false);
    assert_eq!(prop("count")["readonly"], true);
    assert_eq!(prop("log")["proportional"], true);
    let values = m["values"].as_array().unwrap();
    assert!(values.iter().any(|v| v["name"] == "appStats" && v["kind"] == "static" && v["type"] == "Stats"), "{values:?}");
    let sensor = m["classes"].as_array().unwrap().iter().find(|c| c["name"] == "Sensor").unwrap().clone();
    assert_eq!(sensor["events"][1]["name"], "level");
    assert_eq!(sensor["events"][1]["latest"], true);
    let f = m["functions"].as_array().unwrap().iter().find(|f| f["name"] == "countTo").unwrap().clone();
    assert_eq!(f["stream"], "number");
}

// Compile-only: the non-scripted config the GTK example uses also works for stores (no virtual clock).
#[test]
fn stores_work_in_a_live_host_too() {
    let cfg = HostConfig { echo_console: false, echo_errors: false, ..HostConfig::default() };
    with_config(
        "live",
        &format!(
            "{WATCH}
import {{ freshStats, bump }} from 'svelters:app/rx';
const s = freshStats();
H.observe(s.svlHandle, 'count', true);
bump(s, 3, 9).then((v) => console.log('done', v));
"
        ),
        cfg,
        |r| {
            r.line("done 3");
            r.until("count delivered", |_, l| l.iter().any(|x| x == "changed count=3"));
            r.no_errors();
        },
    );
}
