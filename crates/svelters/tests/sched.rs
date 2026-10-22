//! SCHED1: E1 offloaded exports under the deterministic scheduler (seeded random schedules, simulated
//! clock): worker, blocking and JS-thread `async fn` calls racing their aborts, and timers racing
//! promise reactions and worker completions. Every schedule must keep the invariants; across the
//! seeds both sides of each race must be seen.
//!
//! `ITERATIONS=50` runs fifty schedules; `SEED=n` replays the one a failure printed.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;
use svelters::prelude::*;
use svelters_host::HostConfig;
use svelters_host::svelters_sched::{self as sched, TestScheduler, run_seeds};
use svelters_host::testing::SchedHost;

/// Futures of `sim_async` calls dropped (finished or aborted), by call number.
static FUTURE_DROPS: Mutex<Vec<u32>> = Mutex::new(Vec::new());

struct DropNote(u32);

impl Drop for DropNote {
    fn drop(&mut self) {
        FUTURE_DROPS.lock().unwrap().push(self.0);
    }
}

/// A worker that reports progress, then checks for a cancel.
#[svelters::export(module = "sched")]
#[js(worker)]
pub fn sim_work(n: u32, cx: WorkCx<f64>) -> Result<u32, NativeError> {
    cx.progress(n as f64);
    cx.check()?;
    Ok(n * 2)
}

#[svelters::export(module = "sched")]
#[js(blocking)]
pub fn sim_blocking(n: u32, cx: WorkCx) -> Result<u32, NativeError> {
    cx.check()?;
    Ok(n * 5)
}

/// A JS-thread future that yields to the scheduler at random, offloads to a worker, and yields again.
#[svelters::export(module = "sched")]
pub async fn sim_async(n: u32) -> u32 {
    let _note = DropNote(n);
    sched::simulate_random_delay().await;
    let v = svelters::exports::worker(move || n * 3).await;
    sched::simulate_random_delay().await;
    v
}

fn quiet() -> HostConfig {
    HostConfig { echo_console: false, echo_errors: false, ..HostConfig::default() }
}

fn stage(name: &str, seed: u64, main: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("svelters-sched-{name}-{}-{seed}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("main.js"), main).unwrap();
    dir.join("main.js")
}

/// JS shared by the tests: `macro(what)` marks the start of a macrotask (a timer callback) and logs a
/// violation if microtasks queued by an earlier macrotask (`later`) have not run yet.
const MICROTASK_RULE: &str = r#"
const log = (s) => console.log(s);
let micro = 0;
const macro = (what) => {
  if (micro !== 0) log(`VIOLATION ${what} started with ${micro} microtask(s) of an earlier task pending`);
};
const later = (f) => {
  micro++;
  queueMicrotask(() => {
    micro--;
    f();
  });
};
"#;

fn bad_lines(lines: &[String]) -> Vec<&String> {
    lines.iter().filter(|l| l.starts_with("VIOLATION") || l.starts_with("DOUBLE") || l.starts_with("LATE")).collect()
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Kind {
    Work,
    Async,
    Blocking,
}

#[test]
fn e1_async_exports_race_abort() {
    let mut outcomes: BTreeMap<(Kind, &str), u32> = BTreeMap::new();
    let seeds = run_seeds("e1_async_exports_race_abort", 12, |s| {
        s.set_job_delay(Duration::from_millis(6));
        s.set_turn_time(Duration::from_millis(2));
        FUTURE_DROPS.lock().unwrap().clear();
        let kinds = [Kind::Work, Kind::Async, Kind::Blocking];
        let calls: Vec<(Kind, i64)> = (0..6).map(|i| (kinds[i % 3], if s.random_bool(0.7) { s.random_below(8) as i64 } else { -1 })).collect();
        let spec = calls.iter().map(|(k, a)| format!("['{}', {a}]", format!("{k:?}").to_lowercase())).collect::<Vec<_>>().join(", ");
        let main = format!(
            r#"import {{ simWork, simAsync, simBlocking }} from 'svelters:app/sched';
{MICROTASK_RULE}
const calls = [{spec}];
const done = new Map();
const settle = (i, what) => {{
  if (done.has(i)) log(`DOUBLE ${{i}} ${{done.get(i)}} ${{what}}`);
  done.set(i, what);
  log(`S ${{i}} ${{what}}`);
}};
calls.forEach(([kind, abortAt], i) => {{
  const c = new AbortController();
  const opts = {{ signal: c.signal }};
  if (kind === 'work') opts.onProgress = (p) => {{
    if (done.has(i)) log(`LATE progress ${{i}}`);
    later(() => log(`P ${{i}} ${{p}}`));
  }};
  const call = kind === 'work' ? simWork : kind === 'async' ? simAsync : simBlocking;
  call(i, opts).then((v) => settle(i, `ok ${{v}}`), (e) => settle(i, `err ${{e.code}}`));
  if (abortAt >= 0) setTimeout(() => {{ macro(`abort ${{i}}`); log(`A ${{i}}`); c.abort(); }}, abortAt);
}});
for (let t = 0; t < 12; t++) setTimeout(() => macro(`tick ${{t}}`), t);
setTimeout(() => log(`END ${{done.size}}`), 50);
"#
        );
        let h = SchedHost::new(s, quiet()).unwrap();
        h.host.load_bundle(stage("e1", s.seed(), &main)).unwrap();
        s.run_until_idle();
        let lines = h.console();
        let errors = h.errors();
        assert!(errors.is_empty(), "{errors:#?}\n{lines:#?}");
        assert!(bad_lines(&lines).is_empty(), "{:#?}\nall lines: {lines:#?}", bad_lines(&lines));
        assert!(lines.iter().any(|l| l == "END 6"), "{lines:#?}");
        assert_eq!(h.host.ops_in_flight(), 0);
        for (i, (kind, abort_at)) in calls.iter().enumerate() {
            let s_at = lines.iter().position(|l| l.starts_with(&format!("S {i} "))).unwrap_or_else(|| panic!("call {i} never settled: {lines:#?}"));
            let a_at = lines.iter().position(|l| *l == format!("A {i}"));
            let expected = match kind {
                Kind::Work => i * 2,
                Kind::Async => i * 3,
                Kind::Blocking => i * 5,
            };
            let got = lines[s_at].split_once(' ').unwrap().1.split_once(' ').unwrap().1;
            let aborted_first = a_at.is_some_and(|a| a < s_at);
            if aborted_first {
                // an abort rejects at once; a later result is dropped
                assert_eq!(got, "err cancelled", "call {i}: {lines:#?}");
            } else {
                assert_eq!(got, format!("ok {expected}"), "call {i}: {lines:#?}");
            }
            if *abort_at >= 0 {
                *outcomes.entry((*kind, if aborted_first { "cancelled" } else { "won" })).or_default() += 1;
            }
        }
        // every JS-thread future was dropped, finished or aborted, on this thread
        let async_calls = calls.iter().filter(|c| c.0 == Kind::Async).count();
        drop(h);
        assert_eq!(FUTURE_DROPS.lock().unwrap().len(), async_calls);
    });
    if seeds >= 12 {
        for kind in [Kind::Work, Kind::Async, Kind::Blocking] {
            for side in ["cancelled", "won"] {
                assert!(outcomes.get(&(kind, side)).is_some_and(|n| *n > 0), "{kind:?} never {side} its race in {seeds} schedules: {outcomes:?}");
            }
        }
    }
}

#[test]
fn timers_race_promises_and_completions() {
    run_seeds("timers_race_promises_and_completions", 12, |s| {
        timers_case(s);
    });
}

/// The same seed replays the same run exactly: the same console, turns and simulated end time.
#[test]
fn the_same_seed_replays_a_host_run_exactly() {
    let run = |seed| {
        let s = TestScheduler::new(seed);
        let lines = timers_case(&s);
        (lines, s.steps(), s.now())
    };
    for seed in [3, 11, 29] {
        assert_eq!(run(seed), run(seed), "seed {seed}");
    }
    assert_ne!(run(3).0, run(11).0, "different seeds should give different interleavings");
}

/// Timers, an interval, a nested timer and progress-reporting workers on one host; returns the console.
fn timers_case(s: &TestScheduler) -> Vec<String> {
    {
        s.set_job_delay(Duration::from_millis(12));
        s.set_turn_time(Duration::from_millis(3));
        let main = format!(
            r#"import {{ simWork }} from 'svelters:app/sched';
{MICROTASK_RULE}
const t0 = performance.now();
const spec = [[5, 'a'], [0, 'b'], [5, 'c'], [1, 'd'], [10, 'e'], [0, 'f']];
for (const [ms, name] of spec) setTimeout(() => {{
  macro(name);
  log(`T ${{name}} ${{performance.now() - t0}}`);
  later(() => log(`M ${{name}}`));
}}, ms);
let n = 0;
const iv = setInterval(() => {{
  macro('interval');
  log(`I ${{++n}} ${{performance.now() - t0}}`);
  later(() => log(`M I`));
  if (n === 3) clearInterval(iv);
}}, 4);
Promise.resolve().then(() => setTimeout(() => {{
  macro('nested');
  log(`T nested ${{performance.now() - t0}}`);
  later(() => log(`M nested`));
}}, 2));
for (let i = 0; i < 4; i++) simWork(i, {{ onProgress: () => later(() => log(`P ${{i}}`)) }}).then((v) => log(`W ${{i}} ${{v}}`));
log('sync end');
"#
        );
        let h = SchedHost::new(s, quiet()).unwrap();
        h.host.load_bundle(stage("timers", s.seed(), &main)).unwrap();
        s.run_until_idle();
        let lines = h.console();
        let errors = h.errors();
        assert!(errors.is_empty(), "{errors:#?}\n{lines:#?}");
        assert!(bad_lines(&lines).is_empty(), "{:#?}\nall lines: {lines:#?}", bad_lines(&lines));
        // One-shots fire in (deadline, registration) order: the 1 ms floor puts b, d, f together.
        let timers: Vec<&str> = lines.iter().filter_map(|l| l.strip_prefix("T ")).map(|l| l.split(' ').next().unwrap()).collect();
        assert_eq!(timers, ["b", "d", "f", "nested", "a", "c", "e"], "{lines:#?}");
        // Each fires no earlier than its deadline on the simulated clock (performance.now() is that clock).
        let deadline = |name: &str| match name {
            "b" | "d" | "f" => 1.0,
            "nested" => 2.0,
            "a" | "c" => 5.0,
            "e" => 10.0,
            _ => unreachable!(),
        };
        for l in lines.iter().filter_map(|l| l.strip_prefix("T ")) {
            let (name, at) = l.split_once(' ').unwrap();
            assert!(at.parse::<f64>().unwrap() >= deadline(name), "{name} fired at {at}: {lines:#?}");
        }
        // A timer's microtasks run before anything else starts.
        for (k, l) in lines.iter().enumerate() {
            if let Some(rest) = l.strip_prefix("T ") {
                let name = rest.split(' ').next().unwrap();
                assert_eq!(lines.get(k + 1).map(String::as_str), Some(format!("M {name}").as_str()), "{lines:#?}");
            }
            if l.starts_with("I ") {
                assert_eq!(lines.get(k + 1).map(String::as_str), Some("M I"), "{lines:#?}");
            }
        }
        // The interval ran three times, at least 4 ms apart.
        let ivs: Vec<f64> = lines.iter().filter_map(|l| l.strip_prefix("I ")).map(|l| l.split(' ').nth(1).unwrap().parse().unwrap()).collect();
        assert_eq!(ivs.len(), 3, "{lines:#?}");
        assert!(ivs.windows(2).all(|w| w[1] - w[0] >= 4.0 - 1e-6) && ivs[0] >= 4.0, "{ivs:?}");
        for i in 0..4 {
            assert_eq!(lines.iter().filter(|l| **l == format!("W {i} {}", i * 2)).count(), 1, "{lines:#?}");
        }
        assert_eq!(lines[0], "sync end");
        lines
    }
}

/// SCHED1b: a settled offload batch is a task of its own. The `onProgress` callbacks and resolutions
/// of one checkpoint's settle batch queue microtasks, and those run before the first queued event is
/// delivered (REACTIVITY §5.3: a microtask drain after every task). Clicks are posted at random times
/// against progress-reporting workers on a busy UI thread, so a completion and a click often meet in
/// one checkpoint.
#[test]
fn progress_microtasks_run_before_a_queued_event() {
    use std::rc::Rc;
    use svelters_abi::{Backend, HostRef, NameKind};
    const CLICKS: usize = 8;
    const WORKERS: u32 = 6;
    let mut between = 0;
    let seeds = run_seeds("progress_microtasks_run_before_a_queued_event", 12, |s| {
        s.set_job_delay(Duration::from_millis(6));
        s.set_turn_time(Duration::from_millis(2));
        let main = format!(
            r#"import {{ simWork }} from 'svelters:app/sched';
{MICROTASK_RULE}
const h = globalThis.__svelters_host;
const b = h.create('button');
h.listen(b, 'clicked', true);
log(`H ${{b}}`);
let clicks = 0;
let done = 0;
globalThis.__svelters_dispatch = () => {{
  macro(`event ${{clicks}}`);
  log(`E ${{clicks++}} ${{done}}`);
  later(() => log('M E'));
}};
for (let i = 0; i < {WORKERS}; i++)
  simWork(i, {{ onProgress: () => later(() => log(`P ${{i}}`)) }}).then((v) => {{
    done++;
    log(`W ${{i}} ${{v}}`);
  }});
"#
        );
        let h = SchedHost::new(s, quiet()).unwrap();
        h.host.load_bundle(stage("events", s.seed(), &main)).unwrap();
        let mut lines = h.console();
        let handle: u64 = lines.iter().find_map(|l| l.strip_prefix("H ")).expect("button handle").parse().unwrap();
        let kind = h.log.resolve(NameKind::Kind, 0, "button");
        let sig = h.log.resolve(NameKind::Signal, kind, "clicked");
        for _ in 0..CLICKS {
            let at = Duration::from_micros(s.random_below(8_000));
            let host = Rc::downgrade(&h.host);
            s.spawn_local_after(at, "click", move || {
                let Some(host) = host.upgrade() else { return };
                // SAFETY: the callbacks live as long as `host`, which this action holds.
                let cb = unsafe { HostRef::from_raw(host.host_callbacks()) };
                cb.post_event(handle, sig, &[]);
            });
        }
        s.run_until_idle();
        lines.extend(h.console());
        let errors = h.errors();
        assert!(errors.is_empty(), "{errors:#?}\n{lines:#?}");
        assert!(bad_lines(&lines).is_empty(), "{:#?}\nall lines: {lines:#?}", bad_lines(&lines));
        let events: Vec<usize> = lines.iter().filter_map(|l| l.strip_prefix("E ")).map(|l| l.split(' ').nth(1).unwrap().parse().unwrap()).collect();
        assert_eq!(events.len(), CLICKS, "{lines:#?}");
        // an event's own microtasks run before anything else
        for (k, l) in lines.iter().enumerate() {
            if l.starts_with("E ") {
                assert_eq!(lines.get(k + 1).map(String::as_str), Some("M E"), "{lines:#?}");
            }
        }
        for i in 0..WORKERS {
            assert_eq!(lines.iter().filter(|l| **l == format!("P {i}")).count(), 1, "{lines:#?}");
            assert_eq!(lines.iter().filter(|l| **l == format!("W {i} {}", i * 2)).count(), 1, "{lines:#?}");
        }
        between += events.iter().filter(|d| **d > 0 && **d < WORKERS as usize).count();
    });
    if seeds >= 12 {
        assert!(between > 0, "no click landed while workers were still settling in {seeds} schedules");
    }
}
