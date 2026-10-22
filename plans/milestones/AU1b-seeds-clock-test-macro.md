# AU1b: seeds, the virtual clock and `#[svelters_automation::test]`

Goal: the AU1 items left over from `AU1-automation-core.md` ("Not in AU1") and AU2's deferred
`--seed`/`--clock real` (E2E-INSPECTOR §1.6, §2.3, rows AU1/AU1b). Coordinator decision, approved as
HOST owner, including the small host `core`/QuickJS change for `Math.random`.

## What landed

| Area | API | Notes |
|---|---|---|
| Seeded `Math.random` (host) | `HostSched::seed: Option<u64>`, `HostSched::seed(n)` | It sits next to SCHED1's clock/spawner/wake hooks. With a seed set, the QuickJS engine replaces `Math.random` after the prelude with sfc32, seeded from the seed's two 32-bit halves, warmed up, giving 53-bit doubles. Every engine the host makes gets it, a full reload included. `None` (the default) installs nothing and costs nothing. Test: `svelters-host/tests/sched.rs::a_seed_makes_math_random_replayable` (same seed gives the same sequence, another seed differs, no seed keeps QuickJS's native function). |
| Virtual clock | `svelters_automation::clock::{VirtualClock, Clocked, RealTime, ClockMode}` | `VirtualClock::new(seed)` is a SCHED1 `TestScheduler`. `host_sched()`/`config(cfg)` give the host its spawner, its clock and the seed. Timers, `performance.now()` and timer wakeups run on the scheduler's clock. `#[js(worker)]`/`#[js(blocking)]` jobs run on the UI thread in seeded order (E1 ordering). `Clocked<D>` is a `Driver`: each `step` first does `run_for(frame_ms)` (due timers wake the host, ready jobs complete), then runs the frame. `RealTime<D>` paces frames in wall time and leaves the host on the wall clock. The livelock guard is off (`set_max_steps(u64::MAX)`) for long runs. |
| Driver | `Driver::clock_ms()`, `Driver::seed()` (default `None`), `impl Driver for Box<D>` | |
| Session | `Grants::clock` (in `ALL`, not in `READ`/`ACT`), `clock.now`, `clock.advance {ms}`, `Session::{clock_now, advance_clock}` | `clock.now` returns `{mode, ms, seed, session_ms}`. `clock.advance` steps frames until `ms` of frame time has passed and returns `{frames, ms}`. It fails with `grant.denied` when the session lacks the grant and with `capability.absent` on the wall clock. `session.hello` reports `grants.clock` and a `clock` object. |
| `svelters-run` | `--clock virtual\|real`, `--seed N`, `<app dir \| App.svelte \| dist/app.js>` | Virtual (the default) uses a `VirtualClock` (seed 0 without `--seed`). Real keeps the wall clock with `RealTime` pacing, and `--seed` then seeds only `Math.random`. A `.js` app loads as a built bundle (`TestApp::load_with`, new in svelters-egui's kittest runner). `session.hello`'s `app` reports the clock that is actually in use. |
| Test macro | `#[svelters_automation::test(app = "...", seed = 3, iterations = 20, clock = "virtual", step_ms = 16)]` on `fn name(d: &mut E2e) -> TestResult` | Lives in `svelters-macros` (with `#[behavior]`/`#[widget]`). It is re-exported as `svelters_automation::test` behind the `run` feature and expands to `svelters_automation::e2e::run`. The `#[test]` it emits names the built-in by path. |
| `e2e` module | `E2e` (`call`, `locator`, `root`, `get_by_role/text/text_exact/label`, `press`, `type_text`, `wait_settled`, `snapshot`, `clock().run_for/now_ms`, `in_process`, `process`), `Locator`, `expect(..)` (`to_be_visible/hidden/attached/detached/enabled`, `to_have_text`, `to_contain_text`, `to_have_value`, `to_have_count`, `to_match_scene_snapshot`, `.timeout(ms)`), `name("..")`, `TestSpec`, `seeds`, `launch`, `run` | By default the app runs in process (kittest `TestApp` from source or bundle, `Session` with every grant, `VirtualClock`). With `SVELTERS_E2E=process` it runs out of process: `svelters-run` (from `SVELTERS_RUN`, `CARGO_BIN_EXE_svelters-run`, or `target/<profile>/`), or `SVELTERS_APP=<bin>`, over AU2's `client::App`. The seeds are `seed..seed+iterations`, and `SEED`/`ITERATIONS` override them. A failure prints `FAILED with SEED=s` and the replay command. Locator selectors match `@svelters/test`'s. Snapshots are kept in `<crate>/tests/snapshots/<name>.yaml`, compared with indentation ignored; `SVELTERS_UPDATE_SNAPSHOTS=1` rewrites them. The name is `E2e`, not the spec's `Driver`, because `Driver` is the frame-loop trait. |
| Proof | `svelters-automation/tests/e2e.rs` | The tests are: a 30 s timer that fires after 30 s of virtual time, in a few seconds of wall time (`fixtures/counter-egui/src/Clock.svelte`); the seed replaying `Math.random`, compared in process against a fresh in-process app (in process mode this compares `svelters-run` with kittest); and gallery.rs's `inputs` and `containers` tests, rewritten with locators on `fixtures/gallery-egui/dist/app.js`. All four pass both in process and with `SVELTERS_E2E=process`. |

## Decisions and deviations

- **`#[svelters::test]` is `#[svelters_automation::test]`.** The `svelters` facade does not depend on
  svelters-automation (that would make the facade depend on egui). Re-exporting a macro named `test` from the facade
  would also shadow the built-in `#[test]` for any `use svelters::*`. The facade can re-export it later if
  the coordinator wants that.
- **The embed-egui kittests are not rewritten.** `E2e` drives `TestApp` apps (and `svelters-run`, which runs only
  those). An `EmbedHarness` whose ui belongs to the host needs a driver over the host's own frame, and that is cadatomic's `KittestDrive`.
  The gallery tests use the built bundle because the live loader has no `import.meta.glob`.
- **Windowed `svelters-run`: not done.** A `Driver` steps frames synchronously, but eframe's callbacks own the loop. A
  windowed app mounts `Server::pump` in its own frame loop (as AU2 said).
- **`Origin::Tool(session)`: deferred** to AU3, as AU1 planned. It needs a new `Origin` variant in svelters-egui's scene
  (SCENE owner), so it is not a trivial change.
- **`events`/`log`/`perf` streams: not here** (they need host hooks). They stay with AU2b/AU5.

## Follow-ups found

- `wait.for` with `visible` reads a `<stack>`'s inactive child as visible (gallery `containers`: "Stack page two"
  before the switch). The rewritten test checks only the positive side of the switch.
- `input.set_value` has no handle target for a two-handle `range-slider` (one scene node named "Range"). gallery.rs
  set "Range high" through AccessKit. The rewrite leaves that step out.

## What AU2b needs from AU1b

- `clock.now`/`clock.advance` and `session.hello`'s `clock` object are what `@svelters/test`'s `app.clock`
  calls; the TS client does not have them yet (its header still lists `clock.*` as AU1b).
- The runner's `SVELTERS_SEED=` print and per-test seeds map to `svelters-run --seed`. A worker-scoped app needs
  `app.reset()` (host remount) and then a fresh `VirtualClock`; `VirtualClock` cannot be reset in place.
- `mount` (`app.mount`) must build its host with `VirtualClock::config` so that mounted components share the clock and the seed.
- The `E2e` API shape (locators, `expect`, `clock().run_for`) is the Rust mirror the TS runner can follow.

## Not run locally

The full workspace nextest, the full gates and conformance. Only these were run: `svelters-automation` `e2e` (in
process and `SVELTERS_E2E=process`) and `ipc`, and `svelters-host` `sched::a_seed_makes_math_random_replayable`.
