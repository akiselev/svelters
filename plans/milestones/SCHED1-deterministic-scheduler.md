# SCHED1: deterministic scheduler and seeded randomized tests

Goal: Zed lesson #1 (`plans/research/ZED-LESSONS.md` §6.1, §6.2, §9 row 1, §1.9). A `TestScheduler` with a
seeded task order and a simulated clock sits behind the host's job, clock and wake hooks, so a test can
drive a real `Host`, and an egui frame loop, through random but replayable interleavings of its async
seams. The lane also adds the `Spawner` seam that quire's `Spawner` and cadatomic's `JobPool` can implement.

## Design

### Where it lives: a new crate, `crates/svelters-sched`

This is a crate rather than a test-support module, and it has **no dependencies**. It holds:

- the production seams: `Spawner` (`spawn(Job)`, plus `spawn_with(JobMeta, Job)`, whose default
  ignores the meta), `Clock` (`now()`, `wake_at(key, Option<at>, &Waker)`), `JobMeta`/`JobKind`
  (`#[non_exhaustive]`, with room for a priority and a principal) and `wake_key()`;
- `TestScheduler`, `SchedHandle` (its `Send + Sync` half, which implements `Spawner` and `Clock`),
  the futures `Delay` and `Timer`, and `current()`/`simulate_random_delay()` (a thread-local, so code
  under test can call them; with no scheduler running they are ready at once);
- `run_seeds` and `Seeds`, which read the `SEED`/`ITERATIONS` environment variables.

Why a crate: quire, berth and cadatomic must implement `Spawner`, or run their own tests under
`TestScheduler`, without depending on the host, QuickJS or egui. `svelters-host` re-exports it as
`svelters_host::svelters_sched`, and `svelters-testkit` re-exports it from `svelters_testkit::sched`.
`Spawner` has quire's planned shape (`ARCH-CRATES` §3.6: `fn spawn(&self, job: Box<dyn FnOnce() + Send>)`).
A closure `Fn(Job)` already implements it, so cadatomic can adapt its pool with
`Arc::new(move |job| pool.execute(job).expect("job pool"))`. Neither repository was edited.

### The scheduler

- **The ready set.** It holds jobs (`Spawner::spawn`), wakes (`Clock::wake_at` deadlines that came
  due, `simulate_random_delay` yields, `Timer`s), one-shot test actions (`spawn_local`,
  `spawn_local_after`: a reload, a click, an abort) and *foregrounds*. A foreground is an event loop
  registered with `foreground(name)`. Its `waker(fg)` makes it ready, `set_poll` can also make it
  ready, and `set_runner` gives it one turn (a host checkpoint, or one egui frame).
- **Order.** `tick()` picks one ready unit with the seeded RNG (xoshiro256** seeded through
  SplitMix64, the same on every platform) and runs it whole. The order *inside* a turn never changes:
  one checkpoint drains one JS microtask queue, delivers its events in order and fires its timers in
  deadline order. Only the order *between* independent units is random: which job completes first,
  whether a completion lands before or after a timer turn, whether a reload comes before or after a
  pending import's checkpoint.
- **Time.** The clock starts at zero and moves only through these calls:
  - `advance_clock(d)` moves it and runs nothing.
  - `run_until_parked()` runs ready units until none is left.
  - `run_for(d)` steps from deadline to deadline up to now + d.
  - `run_until_idle()` steps from deadline to deadline until nothing is scheduled. Past
    `set_idle_limit` (10 min simulated) it panics, which catches a live interval.
  - `block_until(what, cond)` runs until `cond`; when it parks, it jumps to the next deadline.

  There are two optional realism knobs. `set_job_delay(max)` gives each job a random duration
  (0..=max) before it is ready. `set_turn_time(max)` makes each foreground turn take 0..=max of
  simulated time: a busy UI thread, during which deadlines pass, so one checkpoint meets a completion
  and a due timer together. That is how the bug below was found.
- **Parking.** `forbid_parking()` turns a park inside `block_until` into a failure: "this must finish
  without time passing". The failure lists what is pending: ready labels, scheduled deadlines with
  labels, woken foregrounds. A turn limit (`set_max_steps`, default 1,000,000) catches livelocks. The
  same pending list appears in every scheduler panic; it stands in for Zed's `PENDING_TRACES`.
- **Seeds.** `run_seeds(name, default, body)` runs `body` on a fresh scheduler per seed:
  - nothing set: seeds `0..default`;
  - `ITERATIONS=n`: seeds `0..n`;
  - `SEED=s`: exactly seed `s`;
  - both set: `s..s+n`.

  A failure prints `name: FAILED with SEED=s after N scheduler steps` and the command that replays
  it. `run_seeds` returns the number of seeds run, so a test only asserts coverage ("both sides of
  the race happened") when it ran enough of them.

### The host hooks (`HostConfig::sched: HostSched`, `crates/svelters-host/src/sched.rs`)

| Hook | Default | With the hook |
| --- | --- | --- |
| `spawner` | rayon / blocking pool | `#[js(worker)]` and `#[js(blocking)]` jobs, plus the `exports::worker()`/`blocking()` futures, go to it (`JobMeta` carries the kind and `app:<module>.<member>`). Pool jobs take it from the core; the futures take it from a thread-local the host sets for each entry into JS (`JsGuard`) |
| `clock` | `Instant::now()` | timers, `performance.now()` and the timer wakeup (`Clock::wake_at` with the host's key, never the backend's wall-clock `schedule_wake`). Excludes `scripted`: `Host::new` refuses both |
| `wake` | backend `wake` only | every host wake also reaches this `Waker`. The core's own wakes, the completion queue's and the store hub's now go through one host waker: the backend's wake, then the hook |

A production embedder may set only `spawner`. With the hooks empty, behaviour is unchanged.

A scheduler can still hold a clone of the host's waker when the host goes away (a due `wake_at`
that is ready but has not run). `Host::drop` therefore detaches the waker, and later wakes reach
nobody instead of the freed backend vtable (`a_wake_still_queued_when_the_host_goes_reaches_nobody`).

Two drivers:

- `svelters_host::testing::SchedHost::new(&s, cfg)` builds a host on the log backend (attached through
  `start`, no script). Its foreground turn is the host's `checkpoint` callback (a pending reload
  first, as a toolkit loop does), then `frame` when a requested animation frame is due. Frames come
  every 16 ms of simulated time.
- `svelters_testkit::sched::EguiSched::load(&s, bundle, cfg)` builds a `TestApp` (real host, egui backend,
  kittest). Its foreground turn is `harness.step()`, and it also wants a turn while the last frame
  asked for an immediate repaint. `click`/`click_after` queue kittest input as test actions.

## How to use

```rust
use svelters_host::svelters_sched::run_seeds;
use svelters_host::testing::SchedHost;

#[test]
fn my_race() {
    run_seeds("my_race", 12, |s| {
        s.set_job_delay(Duration::from_millis(6)); // workers take 0..6 ms
        s.set_turn_time(Duration::from_millis(2)); // checkpoints take 0..2 ms
        let h = SchedHost::new(s, quiet()).unwrap();
        h.host.load_bundle(main_js).unwrap();
        s.spawn_local_after(Duration::from_millis(3), "reload", move || { /* apply_changes, ... */ });
        s.run_until_idle();
        // assert invariants that must hold for *every* order
    });
}
```

- Locally: `ITERATIONS=50 tools/dev/cargo-slot cargo nextest run -p <pkg> <test>`. To replay a
  failure: `SEED=<n> ...`.
- CI runs each test with its default of **12 seeds** (linux-gtk's nextest step; `-p svelters-sched`
  was added there). No workflow sets `ITERATIONS`.
- Write invariants, not transcripts: "settled exactly once", "a timer starts on an empty microtask
  queue", "ends on the newest version". A same-seed replay check (`the_same_seed_replays_*`) shows
  the run is deterministic.
- Code under test can call `svelters_sched::simulate_random_delay().await` (a no-op outside a test
  scheduler) to widen the interleavings of a JS-thread `async fn`.

## First users

| Test | File | What races |
| --- | --- | --- |
| `e1_async_exports_race_abort` | `crates/svelters/tests/sched.rs` | 6 calls (worker with progress, JS-thread `async fn` with random yields and an inner `worker()`, blocking) against aborts at random times. Each call settles exactly once: `cancelled` if the abort ran first, else the right value. No progress after settle. Every async future is dropped. No ops left. Across seeds, each kind both wins and loses its race |
| `timers_race_promises_and_completions` | same | 6 one-shots, an interval, a nested timer and 4 progress-reporting workers, on a busy UI thread. Timers fire in (deadline, registration) order, never before their deadline on the simulated `performance.now()`, and each timer's microtasks run before anything else. The interval runs 3 times, at least 4 ms apart |
| `the_same_seed_replays_a_host_run_exactly` | same | the same seed gives the same console, turns and end time; different seeds differ |
| `hmr_reload_races_a_pending_import` | `crates/svelters-host/tests/sched.rs` | an app from source (plain JS, needs `pnpm install` only for the provider's runtime aliases). A dynamic `import()` is held pending 3 ms by top-level await, against an in-place swap of a self-accepting module and a full reload, at random times. Every reload succeeds. The app ends on the newest versions, with exactly one import result in the surviving runtime. Coverage: both a full reload and a swap land while the import is pending |
| `a_scheduler_clock_excludes_scripted_mode` | same | config validation |
| `a_wake_still_queued_when_the_host_goes_reaches_nobody` | same | dropping the host and its due timer wake, at the same instant in either order |
| `egui_clicks_race_a_worker_and_a_timer` | `crates/svelters-testkit/tests/sched_egui.rs` | a raw-ops egui window: Start/Cancel clicks at random times against a worker and an interval. Runs are sequential and each settles once. The AccessKit tree shows the last state and `tick 5`. Across seeds, runs both finish and get cancelled |
| `egui_same_seed_replays_exactly` | same | real kittest frames replay exactly |
| `svelters-sched` unit tests (10) | `crates/svelters-sched/tests/scheduler.rs` | replay, seed spread, `wake_at` replace and cancel, job delays against deadlines, futures, parking, the livelock limit, env parsing, a cross-thread waker |

## Seeds found

- **SEED=0** of `timers_race_promises_and_completions` and of `e1_async_exports_race_abort` (the first
  schedule each ran) found **a host bug**: in a live (non-scripted) checkpoint with no queued event,
  due timers fired before the job drain. A timer callback therefore ran between a settled offload
  batch (`onProgress` callbacks, resolutions) and the microtasks it queued. In the logs, `T e` came
  between worker 3's `onProgress` and its `P 3` microtask; in the abort test, `A 1` came between a
  progress callback and its microtask. Scripted mode drained first, so fixtures never showed it.
  **Fixed** in `core::checkpoint`: when a timer is due, jobs drain before the first timer fires
  (the doc comment there says so). With the fix, both tests pass 300 seeds each.
- **SEED=14** of the timers test failed only on float rounding in the test (4 ms intervals measured as
  3.9999999); the test now has a tolerance.
- The HMR (300 seeds) and egui (300 seeds) tests found nothing.

## Not covered yet (follow-ups)

1. `#[js(tokio)]` tasks still go to tokio: the scheduler would need to poll `Send` futures, and
   `tokio::time` needs a runtime. E3 service threads and the file watcher also keep their own threads;
   tests use `apply_changes` and the provider overlay instead of the watcher (ZED-LESSONS §6.5 fake fs).
2. egui's own time (`input.time`, which native animations read) advances by kittest's `step_dt`, not
   by the scheduler's clock. Unifying them makes TR2 animations deterministic under a seed.
3. §6.2: one `SEED` shared by proptest and the scheduler, so proptest shrinks inputs while the
   schedule replays (berth and quire have proptest).
4. ISO1 isolate batches and berth cross-window drags as users (§9 #1), once those lanes land.
5. quire's `Spawner` and cadatomic's `JobPool`: implement `svelters_sched::Spawner` (or wrap a closure).
   cadatomic's `DirectPool` test pool could become a `TestScheduler`.
6. `DRAIN_BUDGET` (4 ms of *wall* time per offload drain) can make a heavily loaded test machine
   requeue completions differently. It is still deterministic when the machine is fast; a `clock` could
   drive that budget too.
7. Miri on `svelters-sched` in CI (Zed runs its scheduler under Miri). It has no `unsafe`, so the value
   is low.

## Question for the coordinator

**Should settled offload batches be a separate task from queued events?** A checkpoint settles
completions and progress (`offload::drain`), delivers the change batch, then delivers events with
**no drain before the first event**. So an event handler can run between an `onProgress` callback and
the microtasks it queued. I found this by reading the code next to the fix; no test shows it yet. It
is the same class as the timer bug fixed here. For the change batch
plus events, "no drain between" is deliberate (REACTIVITY §5.3: handlers see the new state, and Svelte
flushes once). For the settle batch it is not documented either way.

Recommendation: drain after a non-empty settle batch, before the change batch and events. That costs
one `run_jobs` call, and only when completions arrived. I did not change it, because it touches the
REACTIVITY ordering contract. An `EguiSched` test with clicks against progress would show it.

## Runs

Local, all filtered to this lane's tests (`tools/dev/cargo-slot`, `CARGO_PROFILE_DEV_DEBUG=line-tables-only`):

- `cargo nextest run -p svelters-sched`: 10 pass.
- With `ITERATIONS=50`, after merging `origin/master` and after the last code edit: all 18 SCHED1
  tests pass (`-E 'binary(sched) | binary(sched_egui) | package(svelters-sched)'`). They also pass at
  the default 12.
- `ITERATIONS=300` once each, before the merge: E1, timers, HMR and egui all pass.
- `cargo check -p svelters-host -p svelters -p svelters-sched -p svelters-testkit --all-targets`;
  `cargo clippy -p svelters-sched --all-targets` is clean; `cargo clippy -p svelters-host --all-targets`
  (ZU3's UI-thread lint) has no errors.

Not run locally: the full host, egui and testkit suites, and the workspace gates (CI runs them).
