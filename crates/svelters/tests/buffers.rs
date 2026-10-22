//! E2 buffers end to end on the log backend: a 64 MiB round trip JS -> worker -> JS without a copy
//! (pointer identity, no large allocation, timing), Rust memory reclaimed, `Shared` views read-only and
//! released on GC, detach semantics, buffers in records/lists/worker args/results, and transfer tokens
//! released on the JS thread whether the worker finishes, panics or is abandoned after an abort, and
//! at shutdown (a runtime whose memory a worker still holds is leaked, never freed under it).

use std::alloc::{GlobalAlloc, Layout, System};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use svelters::prelude::*;
use svelters_backend_log::LogBackend;
use svelters_host::{Host, testing::fixture_config};

// ---- a counting allocator: large Rust allocations would be copies ----

struct Counting;

/// Rust allocations of at least `BIG` bytes (a 64 MiB copy would be one).
static BIG_ALLOCS: AtomicUsize = AtomicUsize::new(0);
const BIG: usize = 32 << 20;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if l.size() >= BIG {
            BIG_ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        if l.size() >= BIG {
            BIG_ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.alloc_zeroed(l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        if n >= BIG {
            BIG_ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

// ---- records and gates shared by the exports and the tests (distinct numbers per test) ----

static RECORD: Mutex<Vec<String>> = Mutex::new(Vec::new());
static GATES: Mutex<Vec<u32>> = Mutex::new(Vec::new());
static PTRS: Mutex<Vec<(u32, usize)>> = Mutex::new(Vec::new());

fn record(s: String) {
    RECORD.lock().unwrap().push(s);
}

fn recorded(s: &str) -> bool {
    RECORD.lock().unwrap().iter().any(|x| x == s)
}

fn open(g: u32) {
    GATES.lock().unwrap().push(g);
}

fn wait_gate(g: u32, cx: Option<&WorkCx>) {
    let end = Instant::now() + Duration::from_secs(20);
    while !GATES.lock().unwrap().contains(&g) && Instant::now() < end {
        if cx.is_some_and(|c| c.is_cancelled()) {
            record(format!("saw-cancel {g}"));
            return;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn await_record(s: &str) {
    let end = Instant::now() + Duration::from_secs(20);
    while !recorded(s) {
        assert!(
            Instant::now() < end,
            "timed out waiting for the record {s:?}"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn note_ptr(tag: u32, p: *const f32) {
    PTRS.lock().unwrap().push((tag, p as usize));
}

fn ptr(tag: u32) -> usize {
    PTRS.lock()
        .unwrap()
        .iter()
        .find(|(t, _)| *t == tag)
        .unwrap_or_else(|| panic!("no pointer noted for {tag}"))
        .1
}

// ---- exports ----

/// Scales in place on a worker and hands the same memory back.
#[svelters::export(module = "buf")]
#[js(worker)]
pub fn scale(mut samples: Buffer<f32>, k: f32, tag: u32) -> Buffer<f32> {
    note_ptr(tag, samples.as_ptr());
    for x in samples.iter_mut() {
        *x *= k;
    }
    samples
}

/// Notes where the memory is and gives it back (sync, JS thread).
#[svelters::export(module = "buf")]
pub fn pass(samples: Buffer<f32>, tag: u32) -> Buffer<f32> {
    note_ptr(tag, samples.as_ptr());
    record(format!("pass {tag} js={}", samples.is_js_memory()));
    samples
}

/// Large Rust allocations so far (the round trip test logs it between steps).
#[svelters::export(module = "buf")]
pub fn big_allocs() -> u32 {
    BIG_ALLOCS.load(Ordering::Relaxed) as u32
}

/// Consumes the buffer.
#[svelters::export(module = "buf")]
pub fn total(samples: Buffer<f32>) -> f64 {
    samples.iter().map(|x| *x as f64).sum()
}

/// Two moved arrays.
#[svelters::export(module = "buf")]
pub fn both(a: Buffer<f32>, b: Buffer<f32>) -> u32 {
    (a.len() + b.len()) as u32
}

/// Rust memory handed to JS.
#[svelters::export(module = "buf")]
pub fn make(n: u32, v: f32, tag: u32) -> Buffer<f32> {
    let b = Buffer::from_vec(vec![v; n as usize]);
    note_ptr(tag, b.as_ptr());
    b
}

static SHARED: Mutex<Option<Arc<[f32]>>> = Mutex::new(None);

fn shared() -> Arc<[f32]> {
    SHARED
        .lock()
        .unwrap()
        .get_or_insert_with(|| vec![1.0f32, 2.0, 3.0, 4.0].into())
        .clone()
}

/// A read-only view of the Rust `Arc`.
#[svelters::export(module = "buf")]
pub fn view() -> Shared<[f32]> {
    Shared::new(shared())
}

/// References to the `Arc` besides the static and this function's own (0 = only Rust holds it).
#[svelters::export(module = "buf")]
pub fn arc_refs() -> u32 {
    let a = shared();
    (Arc::strong_count(&a) - 2) as u32
}

/// True if JS passed back our own `Arc` (no copy).
#[svelters::export(module = "buf")]
pub fn is_ours(v: Shared<[f32]>) -> bool {
    Arc::ptr_eq(v.arc(), &shared())
}

#[svelters::export(module = "buf")]
pub fn shared_sum(v: Shared<[f32]>) -> f64 {
    v.iter().map(|x| *x as f64).sum()
}

/// Buffers inside a record.
#[derive(svelters::Record)]
pub struct Frame {
    pub id: u32,
    pub samples: Buffer<f32>,
    pub stats: Option<Shared<[f32]>>,
}

#[svelters::export(module = "buf")]
#[js(worker)]
pub fn frame_double(mut f: Frame) -> Frame {
    for x in f.samples.iter_mut() {
        *x *= 2.0;
    }
    let n = f.samples.len() as f32;
    Frame {
        id: f.id + 1,
        stats: Some(vec![n, f.stats.as_ref().map_or(0.0, |s| s.len() as f32)].into()),
        samples: f.samples,
    }
}

/// Buffers in a list, of another element type.
#[svelters::export(module = "buf")]
#[js(worker)]
pub fn lens(chunks: Vec<Buffer<u8>>) -> Vec<Buffer<u8>> {
    chunks
        .into_iter()
        .map(|mut c| {
            c.iter_mut().for_each(|b| *b = b.wrapping_add(1));
            c
        })
        .collect()
}

/// Holds the buffer until gate `g` (ignoring cancellation), then drops it and returns its length.
#[svelters::export(module = "buf")]
#[js(worker)]
pub fn hold(samples: Buffer<f32>, g: u32) -> u32 {
    record(format!("hold-start {g}"));
    wait_gate(g, None);
    let n = samples.len() as u32;
    drop(samples);
    record(format!("hold-dropped {g}"));
    n
}

/// Holds the buffer until gate `g` or a cancel.
#[svelters::export(module = "buf")]
#[js(worker)]
pub fn hold_coop(samples: Buffer<f32>, g: u32, cx: WorkCx) -> Result<u32, NativeError> {
    record(format!("coop-start {g}"));
    wait_gate(g, Some(&cx));
    let n = samples.len() as u32;
    drop(samples);
    record(format!("coop-dropped {g}"));
    cx.check()?;
    Ok(n)
}

/// Holds the buffer and gives it back late (ignoring cancellation).
#[svelters::export(module = "buf")]
#[js(worker)]
pub fn hold_return(samples: Buffer<f32>, g: u32) -> Buffer<f32> {
    record(format!("hr-start {g}"));
    wait_gate(g, None);
    record(format!("hr-done {g}"));
    samples
}

/// One at a time: a second call waits (queued) with its buffer.
#[svelters::export(module = "buf")]
#[js(worker, max_in_flight = 1)]
pub fn one_at_a_time(samples: Buffer<f32>, g: u32) -> u32 {
    record(format!("oat-start {g}"));
    wait_gate(g, None);
    samples.len() as u32
}

#[svelters::export(module = "buf")]
#[js(worker)]
pub fn burst(samples: Buffer<f32>) -> u32 {
    let _ = samples.len();
    panic!("buffer kaboom")
}

// ---- the test drives JS steps through a JS-thread async fn (E1) that resolves with a command ----

type Chan = (
    String,
    std::collections::VecDeque<String>,
    Option<std::task::Waker>,
);
static CMDS: Mutex<Vec<Chan>> = Mutex::new(Vec::new());

fn with_chan<R>(chan: &str, f: impl FnOnce(&mut Chan) -> R) -> R {
    let mut g = CMDS.lock().unwrap();
    if !g.iter().any(|c| c.0 == chan) {
        g.push((chan.to_string(), Default::default(), None));
    }
    f(g.iter_mut().find(|c| c.0 == chan).unwrap())
}

/// The next command the test sent on `chan` (resolves at the checkpoint after it was sent).
#[svelters::export(module = "buf")]
pub async fn next_command(chan: String) -> String {
    std::future::poll_fn(move |cx| {
        with_chan(&chan, |c| match c.1.pop_front() {
            Some(cmd) => std::task::Poll::Ready(cmd),
            None => {
                c.2 = Some(cx.waker().clone());
                std::task::Poll::Pending
            }
        })
    })
    .await
}

fn send(chan: &str, cmd: &str) {
    let w = with_chan(chan, |c| {
        c.1.push_back(cmd.to_string());
        c.2.take()
    });
    if let Some(w) = w {
        w.wake();
    }
}

/// Appended to every test's main.js: runs `globalThis[cmd]()` for each command.
fn command_loop(chan: &str) -> String {
    format!(
        r#"
import {{ nextCommand as __next }} from 'svelters:app/buf';
globalThis.live = () => console.log('live', globalThis.__svelters_internal.transfersLive());
(async () => {{
  for (;;) {{
    const c = await __next('{chan}');
    try {{
      const r = globalThis[c]();
      if (r && typeof r.catch === 'function') r.catch((e) => console.log('step-error', c, e.name, e.message));
    }} catch (e) {{
      console.log('step-error', c, e.name, e.message);
    }}
  }}
}})();
"#
    )
}

// ---- harness ----

fn pool() {
    let _ = svelters::exports::set_worker_pool(
        svelters::rayon::ThreadPoolBuilder::new()
            .num_threads(16)
            .build()
            .unwrap(),
    );
}

fn stage(name: &str, main: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("svelters-buffers-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("main.js"), main).unwrap();
    dir
}

struct Run<'h, 'a> {
    host: &'h Host<'a>,
    chan: String,
    lines: Vec<String>,
}

impl Run<'_, '_> {
    fn jobs(&mut self) {
        self.host.drain_jobs();
        self.lines
            .extend(self.host.take_console().into_iter().map(|l| l.text));
    }

    fn turn(&mut self) {
        self.host.checkpoint();
        self.host.drain_jobs();
        self.lines
            .extend(self.host.take_console().into_iter().map(|l| l.text));
    }

    fn until(&mut self, what: &str, cond: impl Fn(&Host<'_>, &[String]) -> bool) {
        let end = Instant::now() + Duration::from_secs(20);
        loop {
            self.turn();
            if cond(self.host, &self.lines) {
                return;
            }
            assert!(
                Instant::now() < end,
                "timed out waiting for {what}\nlines: {:#?}\nerrors: {:#?}",
                self.lines,
                self.host.take_errors()
            );
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    fn line(&mut self, line: &str) {
        self.until(line, |_, l| l.iter().any(|x| x == line));
    }

    fn has(&self, line: &str) -> bool {
        self.lines.iter().any(|x| x == line)
    }

    fn value(&self, prefix: &str) -> String {
        self.lines
            .iter()
            .find_map(|l| l.strip_prefix(prefix))
            .unwrap_or_else(|| panic!("no line {prefix:?}: {:#?}", self.lines))
            .to_string()
    }

    /// Runs `globalThis[cmd]()` in JS (one checkpoint).
    fn cmd(&mut self, cmd: &str) {
        send(&self.chan, cmd);
        self.turn();
    }

    /// `__svelters_internal.transfersLive()`: JS buffers whose memory Rust holds.
    fn transfers_live(&mut self) -> usize {
        self.lines.retain(|l| !l.starts_with("live "));
        self.cmd("live");
        self.value("live ").parse().unwrap()
    }

    fn no_errors(&self) {
        let e = self.host.take_errors();
        assert!(e.is_empty(), "{e:#?}\nlines: {:#?}", self.lines);
    }
}

fn with_host(name: &str, main: &str, f: impl FnOnce(&mut Run<'_, '_>)) {
    pool();
    let dir = stage(name, &format!("{main}\n{}", command_loop(name)));
    let (cfg, _) = fixture_config();
    let log = LogBackend::new(Vec::new());
    let vt = log.vtable();
    let host = Host::new(vt.backend_ref(), cfg).unwrap();
    if let Err(e) = host.load_bundle(dir.join("main.js")) {
        panic!("load: {e}");
    }
    let mut run = Run {
        host: &host,
        chan: name.to_string(),
        lines: Vec::new(),
    };
    run.lines
        .extend(host.take_console().into_iter().map(|l| l.text));
    f(&mut run);
}

// ---- tests ----

/// The acceptance round trip: 64 MiB JS -> worker -> JS -> Rust, the same memory throughout.
#[test]
fn round_trip_64mib_without_a_copy() {
    with_host(
        "roundtrip",
        r#"
import { scale, pass, bigAllocs } from 'svelters:app/buf';
import { alloc } from 'svelters:core';
const N = 16 * 1024 * 1024; // 64 MiB of f32
globalThis.start = async () => {
  // A JS-allocated array: moved with transfer(), scaled on a worker, handed back.
  const a = new Float32Array(N);
  a[0] = 3; a[N - 1] = 5;
  // A Rust-backed array (alloc).
  const b = alloc(Float32Array, N);
  b[0] = 7;
  console.log('big-setup', bigAllocs());
  const t0 = performance.now();
  const p = scale(a, 2, 1);
  const t1 = performance.now();
  console.log('detached', a.length, a.byteLength);
  const r = await p;
  const t2 = performance.now();
  console.log('result', r.length, r[0], r[N - 1]);
  const back = pass(r, 2);
  const t3 = performance.now();
  console.log('again', r.length, back.length, back[0]);
  console.log('ms js', (t1 - t0).toFixed(3), (t3 - t2).toFixed(3));
  console.log('big-js', bigAllocs());
  // The Rust-backed array: reclaimed by Rust, scaled, handed back, reclaimed again.
  const u0 = performance.now();
  const rb = await scale(b, 3, 3);
  const u1 = performance.now();
  console.log('alloc', b.length, rb[0]);
  pass(rb, 4);
  console.log('ms alloc', (u1 - u0).toFixed(3));
  console.log('big-end', bigAllocs());
  console.log('done');
};
"#,
        |r| {
            r.cmd("start");
            r.line("done");
            r.no_errors();
            assert!(
                r.has("detached 0 0"),
                "the moved array is detached: {:?}",
                r.lines
            );
            assert!(r.has("result 16777216 6 10"), "{:?}", r.lines);
            assert!(r.has("again 0 16777216 6"), "{:?}", r.lines);
            assert!(r.has("alloc 0 21"), "{:?}", r.lines);
            // The worker scaled the very memory `pass` saw afterwards: no copy on the way back.
            assert_eq!(
                ptr(1),
                ptr(2),
                "JS buffer: worker and the round-tripped result share memory"
            );
            assert!(recorded("pass 2 js=true"));
            assert_eq!(
                ptr(3),
                ptr(4),
                "Rust-backed buffer: same allocation after the round trip"
            );
            assert!(recorded("pass 4 js=false"));
            // No large Rust allocation after the arrays were made: nothing was copied.
            let setup: u32 = r.value("big-setup ").parse().unwrap();
            assert_eq!(r.value("big-js "), setup.to_string(), "{:?}", r.lines);
            assert_eq!(r.value("big-end "), setup.to_string(), "{:?}", r.lines);
            let s = r.host.buffer_stats();
            assert_eq!(s.copies, 0, "{s:?}");
            assert_eq!(s.transferred, 2, "{s:?}");
            assert_eq!(s.returned, 2, "{s:?}");
            assert_eq!(s.reclaimed, 2, "{s:?}");
            eprintln!(
                "E2 round trip (64 MiB): {} | {} | stats {s:?}",
                r.value("ms js "),
                r.value("ms alloc ")
            );
        },
    );
}

#[test]
fn rust_memory_is_reclaimed_and_shared_views_are_read_only_and_released_on_gc() {
    with_host(
        "shared",
        r#"
import { make, pass, view, arcRefs, isOurs, sharedSum } from 'svelters:app/buf';
globalThis.step1 = () => {
  const m = make(4, 1.5, 10);
  console.log('make', m.length, m[3]);
  m[0] = 9; // JS owns Rust memory exclusively: writes stick
  const back = pass(m, 11);
  console.log('made-back', m.length, back[0]);
  globalThis.v = view();
  console.log('refs', arcRefs());
  'use strict';
  let threw = false;
  try { v[0] = 42; } catch (_) { threw = true; }
  console.log('write', v[0], threw);
  console.log('ours', isOurs(v), isOurs(v.subarray(0, 2)), isOurs(new Float32Array([1, 2, 3, 4])));
  console.log('sum', sharedSum(new Float32Array([1, 2])), sharedSum(v));
  console.log('after-in', arcRefs());
};
globalThis.step2 = () => {
  globalThis.v = null;
};
globalThis.step3 = () => console.log('refs-after-gc', arcRefs());
"#,
        |r| {
            r.cmd("step1");
            r.no_errors();
            assert!(r.has("make 4 1.5"), "{:?}", r.lines);
            assert!(r.has("made-back 0 9"), "{:?}", r.lines);
            assert_eq!(
                ptr(10),
                ptr(11),
                "a buffer Rust handed out comes back as the same allocation"
            );
            assert!(recorded("pass 11 js=false"));
            assert!(
                r.has("refs 1"),
                "the view holds one Arc clone: {:?}",
                r.lines
            );
            assert!(
                r.has("write 1 false"),
                "writes to a read-only view are ignored without an exception: {:?}",
                r.lines
            );
            assert!(r.has("ours true false false"), "{:?}", r.lines);
            assert!(r.has("sum 3 10"), "{:?}", r.lines);
            let s = r.host.buffer_stats();
            assert!(s.shared_in >= 1 && s.copies >= 2, "{s:?}");
            r.cmd("step2");
            r.host.collect_garbage();
            r.turn();
            r.cmd("step3");
            assert!(
                r.has("refs-after-gc 0"),
                "the Arc is released when JS collects the view: {:?}",
                r.lines
            );
            r.no_errors();
        },
    );
}

#[test]
fn moved_arrays_are_detached_and_failed_calls_leave_arrays_alone() {
    with_host(
        "detach",
        r#"
import { total, both, scale, view } from 'svelters:app/buf';
globalThis.go = () => {
  const a = new Float32Array([1, 2, 3]);
  console.log('total', total(a));
  console.log('after', a.length, a.byteLength);
  const err = (f) => { try { f(); return 'ok'; } catch (e) { return e.name + ': ' + e.message; } };
  console.log('reuse', err(() => total(a)));
  const b = new Float32Array(4);
  // A later argument fails: nothing was moved.
  console.log('bad-arg', err(() => scale(b, 'x', 1)), b.length);
  // The same buffer twice: refused before anything moves.
  console.log('twice', err(() => both(b, b.subarray(1))), b.length);
  console.log('ro', err(() => total(view())));
  console.log('elem', err(() => total(new Float64Array(2))));
  console.log('ab', err(() => total(new ArrayBuffer(8))));
  console.log('arr', err(() => total([1, 2])));
  console.log('ok-two', both(b, new Float32Array(2)), b.length);
};
"#,
        |r| {
            r.cmd("go");
            r.no_errors();
            assert!(r.has("total 6"), "{:?}", r.lines);
            assert!(r.has("after 0 0"), "{:?}", r.lines);
            let reuse = r.value("reuse ");
            assert!(
                reuse.starts_with("TypeError") && reuse.contains("detached"),
                "{reuse}"
            );
            let bad = r.value("bad-arg ");
            assert!(bad.starts_with("TypeError") && bad.ends_with(" 4"), "{bad}");
            let twice = r.value("twice ");
            assert!(
                twice.contains("moved twice") && twice.ends_with(" 4"),
                "{twice}"
            );
            assert!(r.value("ro ").contains("read-only"), "{:?}", r.lines);
            assert!(
                r.value("elem ").contains("expected Float32Array"),
                "{:?}",
                r.lines
            );
            assert!(r.value("ab ").contains("ArrayBuffer"), "{:?}", r.lines);
            assert!(
                r.value("arr ").contains("expected Float32Array"),
                "{:?}",
                r.lines
            );
            assert!(r.has("ok-two 6 0"), "{:?}", r.lines);
        },
    );
}

#[test]
fn buffers_in_records_lists_worker_args_and_results() {
    with_host(
        "records",
        r#"
import { frameDouble, lens, view } from 'svelters:app/buf';
globalThis.go = async () => {
  const samples = new Float32Array([1, 2, 3]);
  const f = await frameDouble({ id: 1, samples, stats: view() });
  console.log('frame', f.id, samples.length, Array.from(f.samples).join(','), f.samples instanceof Float32Array, Array.from(f.stats).join(','));
  const g = await frameDouble({ id: 5, samples: new Float32Array([4]) });
  console.log('frame2', g.id, g.samples[0], Array.from(g.stats).join(','));
  const chunks = [new Uint8Array([1, 2]), new Uint8Array([255])];
  const out = await lens(chunks);
  console.log('lens', chunks[0].length, chunks[1].length, out.length, out.map((c) => Array.from(c).join('+')).join(' '), out[0] instanceof Uint8Array);
  console.log('done');
};
"#,
        |r| {
            r.cmd("go");
            r.line("done");
            r.no_errors();
            assert!(r.has("frame 2 0 2,4,6 true 3,4"), "{:?}", r.lines);
            assert!(r.has("frame2 6 8 1,0"), "{:?}", r.lines);
            assert!(r.has("lens 0 0 2 2+3 0 true"), "{:?}", r.lines);
            let s = r.host.buffer_stats();
            assert_eq!(s.transferred, 4, "{s:?}");
            assert_eq!(s.returned, 4, "{s:?}");
        },
    );
}

/// Abort, panic, finish, queue: the JS memory is freed only on the JS thread, at a checkpoint after
/// Rust dropped the token, whatever thread dropped it.
#[test]
fn transfer_tokens_are_released_on_the_js_thread_under_abort() {
    with_host(
        "release",
        r#"
import { hold, holdReturn, oneAtATime, burst } from 'svelters:app/buf';
const big = () => new Float32Array(1024);
globalThis.finish = () => hold(big(), 501).then((n) => console.log('finished', n));
globalThis.abandon = () => {
  const c = new AbortController();
  holdReturn(big(), 502, { signal: c.signal }).then(() => console.log('resolved?!'), (e) => console.log('abandoned', e.code));
  globalThis.abort502 = () => c.abort();
};
globalThis.queued = () => {
  oneAtATime(big(), 503).then((n) => console.log('first', n));
  const c = new AbortController();
  oneAtATime(big(), 504, { signal: c.signal }).catch((e) => console.log('queued', e.code));
  globalThis.abort504 = () => c.abort();
};
globalThis.panics = () => burst(big()).catch((e) => console.log('panic', e.code));
"#,
        |r| {
            let base = r.host.buffer_stats().released;
            // 1. The worker finishes and drops the buffer on its thread: freed at the next checkpoint.
            r.cmd("finish");
            await_record("hold-start 501");
            assert_eq!(r.transfers_live(), 1);
            open(501);
            await_record("hold-dropped 501");
            // Dropped on the worker but not freed there: no checkpoint ran since, so nothing was
            // released yet (the release waits for the JS thread).
            let s = r.host.buffer_stats();
            assert_eq!(
                (s.tokens_live, s.released),
                (0, base),
                "a worker never frees JS memory: {s:?}"
            );
            r.line("finished 1024");
            assert_eq!(r.transfers_live(), 0);
            assert_eq!(r.host.buffer_stats().released, base + 1);

            // 2. Abandoned: the abort rejects at once while the worker still uses the memory.
            let dropped0 = r.host.offload_stats().dropped;
            r.cmd("abandon");
            await_record("hr-start 502");
            r.cmd("abort502");
            assert!(r.has("abandoned cancelled"), "{:?}", r.lines);
            assert_eq!(
                r.transfers_live(),
                1,
                "memory stays pinned while the zombie job runs"
            );
            for _ in 0..3 {
                r.turn();
            }
            assert_eq!(r.transfers_live(), 1);
            open(502);
            await_record("hr-done 502");
            // The late result (the buffer itself) is dropped on the JS thread and the memory freed.
            r.until("zombie done", |h, _| {
                h.offload_stats().dropped >= dropped0 + 1
            });
            r.turn();
            assert_eq!(r.transfers_live(), 0);
            assert_eq!(r.host.buffer_stats().released, base + 2);
            assert!(!r.lines.iter().any(|l| l.contains("?!")));

            // 3. Queued behind a cap, aborted before it starts: the job is dropped on the JS thread.
            r.cmd("queued");
            await_record("oat-start 503");
            assert_eq!(r.transfers_live(), 2);
            r.cmd("abort504");
            assert!(r.has("queued cancelled"), "{:?}", r.lines);
            r.turn();
            assert_eq!(r.transfers_live(), 1, "the queued call's buffer is freed");
            assert!(
                !recorded("oat-start 504"),
                "an aborted queued call never starts"
            );
            open(503);
            r.line("first 1024");
            r.turn();
            assert_eq!(r.transfers_live(), 0);

            // 4. A panic unwinds the worker (dropping the buffer there): freed on the JS thread.
            r.cmd("panics");
            r.line("panic panic");
            r.turn();
            assert_eq!(r.transfers_live(), 0);
            let s = r.host.buffer_stats();
            assert_eq!(s.released, base + 5, "{s:?}");
            assert_eq!(s.tokens_live, 0, "{s:?}");
            r.no_errors();
        },
    );
}

/// Shutdown: a cooperative worker drops its buffer within the grace period (no leak); one that
/// ignores cancellation makes the host leak the runtime instead of freeing memory under it.
#[test]
fn shutdown_waits_for_workers_and_leaks_a_runtime_still_in_use() {
    pool();
    let leaked0 = svelters_host::BufferStats::default().leaked_runtimes;
    let _ = leaked0;
    let run = |name: &str, main: &str, start: &str| -> (Duration, u64) {
        let dir = stage(name, main);
        let (cfg, _) = fixture_config();
        let log = LogBackend::new(Vec::new());
        let vt = log.vtable();
        let host = Host::new(vt.backend_ref(), cfg).unwrap();
        host.load_bundle(dir.join("main.js")).unwrap();
        host.drain_jobs();
        await_record(start);
        let before = host.buffer_stats().leaked_runtimes;
        let t0 = Instant::now();
        drop(host);
        let took = t0.elapsed();
        // The process-wide counter, read through a fresh host.
        let log2 = LogBackend::new(Vec::new());
        let vt2 = log2.vtable();
        let h2 = Host::new(vt2.backend_ref(), fixture_config().0).unwrap();
        let after = h2.buffer_stats().leaked_runtimes;
        (took, after - before)
    };
    let (took, leaked) = run(
        "shutdown-coop",
        "import { holdCoop } from 'svelters:app/buf';\nholdCoop(new Float32Array(64), 601).catch(() => {});\n",
        "coop-start 601",
    );
    await_record("coop-dropped 601");
    assert_eq!(
        leaked, 0,
        "a cooperative worker released its buffer in time"
    );
    assert!(took < Duration::from_millis(900), "{took:?}");

    let (took, leaked) = run(
        "shutdown-stuck",
        "import { hold } from 'svelters:app/buf';\nhold(new Float32Array(64), 602).catch(() => {});\n",
        "hold-start 602",
    );
    assert_eq!(
        leaked, 1,
        "the runtime is leaked, not freed under the worker"
    );
    assert!(
        took >= Duration::from_millis(900),
        "waited for the grace period: {took:?}"
    );
    // The worker may still write its memory safely; let it finish.
    open(602);
    await_record("hold-dropped 602");

    // A worker that finished but whose result (the buffer) was never drained, and a UI object that keeps
    // a buffer: both are dropped before the runtime, so nothing leaks and nothing waits.
    open(603);
    let (took, leaked) = run(
        "shutdown-undrained",
        "import { holdReturn } from 'svelters:app/buf';\nholdReturn(new Float32Array(64), 603).catch(() => {});\n",
        "hr-done 603",
    );
    assert_eq!(leaked, 0);
    assert!(took < Duration::from_millis(900), "{took:?}");
    let (took, leaked) = run(
        "shutdown-keeper",
        "import { Keeper } from 'svelters:app/buf';\nglobalThis.k = new Keeper();\nk.keep(new Float32Array(64));\n",
        "kept 64",
    );
    assert_eq!(leaked, 0);
    assert!(took < Duration::from_millis(900), "{took:?}");
}

/// A UI object that keeps a moved buffer.
#[svelters::object(module = "buf")]
pub struct Keeper {
    held: Vec<Buffer<f32>>,
}

#[svelters::export]
impl Keeper {
    #[js(constructor)]
    pub fn new() -> Keeper {
        Keeper { held: Vec::new() }
    }

    pub fn keep(&mut self, b: Buffer<f32>) {
        record(format!("kept {}", b.len()));
        self.held.push(b);
    }
}

/// Hot reload with a transferred buffer in flight: the old runtime's call is cancelled before the
/// runtime is dropped, so a cooperative worker lets go in time (the runtime is freed); one that does not
/// gets the old runtime leaked instead of freed under it.
#[test]
fn reload_with_a_transferred_buffer_in_flight() {
    pool();
    let reload = |name: &str, v1: &str, started: &str| -> (Duration, u64) {
        let dir = stage(name, v1);
        let (cfg, _) = fixture_config();
        let log = LogBackend::new(Vec::new());
        let vt = log.vtable();
        let host = Host::new(vt.backend_ref(), cfg).unwrap();
        host.load_bundle(dir.join("main.js")).unwrap();
        host.drain_jobs();
        await_record(started);
        assert_eq!(host.buffer_stats().tokens_live, 1);
        let leaked = host.buffer_stats().leaked_runtimes;
        std::fs::write(dir.join("main.js"), "console.log('v2');\n").unwrap();
        let ok = std::rc::Rc::new(std::cell::Cell::new(None));
        let flag = ok.clone();
        host.on_reload(move |rep| flag.set(Some((rep.ok, rep.elapsed))));
        host.request_reload();
        unsafe { svelters_abi::HostRef::from_raw(host.host_callbacks()) }.checkpoint();
        let (ok, took) = ok.get().expect("reloaded");
        assert!(ok, "{:#?}", host.take_errors());
        host.checkpoint();
        assert!(host.take_errors().is_empty());
        (took, host.buffer_stats().leaked_runtimes - leaked)
    };
    let (took, leaked) = reload(
        "reload-coop",
        "import { holdCoop } from 'svelters:app/buf';\nholdCoop(new Float32Array(64), 701).catch(() => {});\n",
        "coop-start 701",
    );
    assert!(
        recorded("coop-dropped 701"),
        "the worker saw the cancel before the old runtime was dropped"
    );
    assert_eq!(leaked, 0);
    assert!(took < Duration::from_millis(900), "{took:?}");
    let (took, leaked) = reload(
        "reload-stuck",
        "import { hold } from 'svelters:app/buf';\nhold(new Float32Array(64), 702).catch(() => {});\n",
        "hold-start 702",
    );
    assert_eq!(
        leaked, 1,
        "the old runtime is leaked while the worker holds its memory"
    );
    assert!(took >= Duration::from_millis(900), "{took:?}");
    open(702);
    await_record("hold-dropped 702");
}

/// Hands the buffer straight back from a worker (measures the path, not a kernel).
#[svelters::export(module = "buf")]
#[js(worker)]
pub fn pass_worker(samples: Buffer<f32>) -> Buffer<f32> {
    samples
}

/// A 64 MiB `Arc<[f32]>` as a read-only view.
#[svelters::export(module = "buf")]
pub fn view64() -> Shared<[f32]> {
    static V: std::sync::OnceLock<Arc<[f32]>> = std::sync::OnceLock::new();
    Shared::new(V.get_or_init(|| vec![1.0f32; 16 << 20].into()).clone())
}

/// Costs of the buffer paths for 64 MiB, against a copy. Run with
/// `cargo test --release -p svelters --test buffers -- --ignored --nocapture buffer_costs`.
#[test]
#[ignore]
fn buffer_costs() {
    with_host(
        "costs",
        r#"
import { pass, passWorker, view64 } from 'svelters:app/buf';
import { alloc } from 'svelters:core';
const N = 16 * 1024 * 1024;
const best = (xs) => Math.min(...xs).toFixed(3);
globalThis.start = async () => {
  let a = new Float32Array(N);
  const sync = [], worker = [], copy = [], viewT = [], allocT = [];
  for (let i = 0; i < 5; i++) {
    let t0 = performance.now();
    a = pass(a, 100 + i);
    sync.push(performance.now() - t0);
    t0 = performance.now();
    a = await passWorker(a);
    worker.push(performance.now() - t0);
    t0 = performance.now();
    const c = a.slice();
    copy.push(performance.now() - t0);
    t0 = performance.now();
    const v = view64();
    viewT.push(performance.now() - t0);
    void [c, v];
  }
  let b = alloc(Float32Array, N);
  for (let i = 0; i < 5; i++) {
    const t0 = performance.now();
    b = pass(b, 200 + i);
    allocT.push(performance.now() - t0);
  }
  console.log(`costs ms (best of 5, 64 MiB): sync JS->Rust->JS ${best(sync)} | worker round trip incl. checkpoint ${best(worker)} | Rust-backed (alloc) sync round trip ${best(allocT)} | read-only view of a 64 MiB Arc ${best(viewT)} | JS copy (slice) ${best(copy)}`);
};
"#,
        |r| {
            r.cmd("start");
            r.until("costs", |_, l| l.iter().any(|x| x.starts_with("costs")));
            println!(
                "{}",
                r.lines.iter().find(|x| x.starts_with("costs")).unwrap()
            );
            println!("{:?}", r.host.buffer_stats());
            r.no_errors();
        },
    );
}

#[test]
fn op_log_names_buffers_without_their_contents() {
    let b = svelters_abi::BufferValue::whole(
        svelters_abi::Elem::F32,
        svelters_abi::BufData::Shared(Arc::new(Bytes(vec![0u8; 64]))),
    );
    use svelters_abi::Value;
    assert_eq!(
        svelters_backend_log::json_string(&Value::List(vec![Value::Buffer(b), Value::I64(1)])),
        "[\"<Float32Array 16>\",1]"
    );
}

struct Bytes(Vec<u8>);
unsafe impl svelters_abi::BufSource for Bytes {
    fn as_ptr(&self) -> *mut u8 {
        self.0.as_ptr() as *mut u8
    }
    fn byte_len(&self) -> usize {
        self.0.len()
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[test]
fn typings_mark_moved_and_read_only_buffers() {
    let ts = svelters::exports::typescript();
    let m = ts
        .split("declare module 'svelters:app/buf'")
        .nth(1)
        .expect("buf module");
    assert!(m.contains("import type { NativeObject, AsyncOptions, ReadonlyFloat32Array } from '@svelters/types/core';"), "{m}");
    assert!(m.contains("export function scale(samples: Float32Array, k: number, tag: number, options?: AsyncOptions): Promise<Float32Array>;"), "{m}");
    assert!(
        m.contains("@param samples moved to Rust: the array is detached"),
        "{m}"
    );
    assert!(
        m.contains("export function view(): ReadonlyFloat32Array;"),
        "{m}"
    );
    assert!(
        m.contains("export function isOurs(v: ReadonlyFloat32Array): boolean;"),
        "{m}"
    );
    assert!(m.contains("export interface Frame { id: number; samples: Float32Array; stats?: ReadonlyFloat32Array | null; }"), "{m}");
    assert!(m.contains("export function lens(chunks: Uint8Array[], options?: AsyncOptions): Promise<Uint8Array[]>;"), "{m}");
}
