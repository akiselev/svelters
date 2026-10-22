//! SCHED1 on egui: a real host, the egui backend and kittest frames under the deterministic scheduler.
//! A window with Start and Cancel buttons runs an offloaded worker export; clicks land at seeded
//! random times against random worker durations, a busy UI thread and an interval timer. Every
//! schedule must leave the UI showing the last state the app reached, each run settled exactly once;
//! across the seeds, runs must both finish and get cancelled.
//!
//! `ITERATIONS=50` runs fifty schedules; `SEED=n` replays the one a failure printed.

use std::path::PathBuf;
use std::time::Duration;
use svelters::prelude::*;
use svelters_host::HostConfig;
use svelters_testkit::sched::{EguiSched, TestScheduler, run_seeds};

#[svelters::export(module = "schedegui")]
#[js(worker)]
pub fn crunch(n: u32, cx: WorkCx) -> Result<u32, NativeError> {
    cx.check()?;
    Ok(n * 7)
}

const APP: &str = r#"import { crunch } from 'svelters:app/schedegui';
const h = globalThis.__svelters_host;
const win = h.create('window');
h.setProp(win, 'title', 'sched');
const col = h.create('box');
h.insert(win, col, null);
const button = (text) => {
  const b = h.create('button');
  h.setProp(b, 'text', text);
  h.listen(b, 'clicked', true);
  h.insert(col, b, null);
  return b;
};
const label = (text) => {
  const l = h.create('label');
  h.setProp(l, 'text', text);
  h.insert(col, l, null);
  return l;
};
const start = button('Start');
const cancel = button('Cancel');
const status = label('idle');
const tick = label('tick 0');
const set = (s) => {
  h.setProp(status, 'text', s);
  console.log(s);
};
let ctl = null;
let runs = 0;
globalThis.__svelters_dispatch = (handle) => {
  if (handle === start && !ctl) {
    const c = (ctl = new AbortController());
    const n = ++runs;
    set(`running ${n}`);
    crunch(n, { signal: c.signal })
      .then((v) => set(`done ${n} ${v}`), (e) => set(`${e.code} ${n}`))
      .finally(() => {
        if (ctl === c) ctl = null;
      });
  } else if (handle === cancel && ctl) {
    ctl.abort();
  }
};
let t = 0;
const iv = setInterval(() => {
  h.setProp(tick, 'text', `tick ${++t}`);
  if (t === 5) clearInterval(iv);
}, 3);
h.call(win, 'present', []);
"#;

fn stage(seed: u64) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("svelters-sched-egui-{}-{seed}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("main.js"), APP).unwrap();
    dir.join("main.js")
}

#[test]
fn egui_clicks_race_a_worker_and_a_timer() {
    let (mut finished, mut cancelled) = (0, 0);
    let seeds = run_seeds("egui_clicks_race_a_worker_and_a_timer", 12, |s| {
        let (f, c) = egui_case(s).1;
        finished += f;
        cancelled += c;
    });
    if seeds >= 12 {
        assert!(finished > 0 && cancelled > 0, "finished {finished}, cancelled {cancelled} in {seeds} schedules");
    }
}

/// Real frames replay too: the same seed gives the same console, turns and simulated end time.
#[test]
fn egui_same_seed_replays_exactly() {
    let run = |seed| {
        let s = TestScheduler::new(seed);
        let lines = egui_case(&s).0;
        (lines, s.steps(), s.now())
    };
    for seed in [2, 9] {
        assert_eq!(run(seed), run(seed), "seed {seed}");
    }
}

/// One schedule; returns the console and (runs finished, runs cancelled).
fn egui_case(s: &TestScheduler) -> (Vec<String>, (u32, u32)) {
    let (mut finished, mut cancelled) = (0, 0);
    {
        s.set_job_delay(Duration::from_millis(8));
        s.set_turn_time(Duration::from_millis(2));
        let app = EguiSched::load(s, stage(s.seed()), HostConfig { echo_console: false, echo_errors: false, ..HostConfig::default() }).unwrap();
        // the first frames: the window is laid out and its buttons can be clicked
        s.run_until_parked();
        assert!(app.has_label("Start") && app.has_label("idle"), "the window is not up");
        let ms = |s: &TestScheduler| Duration::from_micros(s.random_below(10_000));
        for b in ["Start", "Cancel", "Start", "Start", "Cancel"] {
            if s.random_bool(0.8) {
                let at = ms(s);
                app.click_after(s, at, b);
            }
        }
        s.run_until_idle();
        let lines = app.console();
        let errors = app.errors();
        assert!(errors.is_empty(), "{errors:#?}\n{lines:#?}");
        // runs are sequential: each `running n` is followed by exactly its own end, before the next run
        let mut open: Option<u32> = None;
        for l in &lines {
            if let Some(n) = l.strip_prefix("running ") {
                assert!(open.is_none(), "{lines:#?}");
                open = Some(n.parse().unwrap());
            } else if let Some(rest) = l.strip_prefix("done ") {
                let (n, v) = rest.split_once(' ').unwrap();
                let n: u32 = n.parse().unwrap();
                assert_eq!(open.take(), Some(n), "{lines:#?}");
                assert_eq!(v.parse::<u32>().unwrap(), n * 7);
                finished += 1;
            } else if let Some(n) = l.strip_prefix("cancelled ") {
                assert_eq!(open.take(), Some(n.parse().unwrap()), "{lines:#?}");
                cancelled += 1;
            } else {
                panic!("unexpected line {l:?}: {lines:#?}");
            }
        }
        assert!(open.is_none(), "a run never settled: {lines:#?}");
        // the UI shows the last state, and the interval ran out on the simulated clock
        let last = lines.last().cloned().unwrap_or_else(|| "idle".into());
        assert!(app.has_label(&last), "the status label does not show {last:?}");
        assert!(app.has_label("tick 5"), "the interval did not finish");
        assert_eq!(app.app.borrow().host().ops_in_flight(), 0);
        (lines, (finished, cancelled))
    }
}
