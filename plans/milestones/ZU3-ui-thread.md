# ZU3: UI-thread enforcement

Lane ZU3, started 2026-10-22 on master 1786957f. Spec: `plans/research/ZED-LESSONS.md` §9 #3, §1.9, §1.10,
§6.7, §6.8. Our rule ("no blocking I/O or waits on the UI thread") was enforced by review only. This lane adds
three checks: a lint, a hang watchdog and a frame-time histogram. It does not add the ARCH-SCENE §0.8 benches
(see Not done).

## What landed

### 1. Lint: clippy `disallowed_methods` (svelters-host, svelters-egui)

- `crates/svelters-host/clippy.toml` and `crates/svelters-egui/clippy.toml` (keep them identical) list the
  blocking calls:
  - `std::fs` (read, write, read_dir, metadata, create/remove/rename/copy, `File::open/create`,
    `OpenOptions::open`) and the `Path` methods that stat (`exists`, `is_file`, `is_dir`, `metadata`,
    `canonicalize`, `read_dir`);
  - `std::process` (`Command::output/status/spawn`, `Child::wait/wait_with_output`);
  - blocking waits (`mpsc::Receiver::recv/recv_timeout/iter`, `Condvar::wait*`, `JoinHandle::join`,
    `thread::sleep`, `block_on` of tokio/futures/pollster, `rayon::ThreadPool::install`);
  - rfd's synchronous dialogs.
  Entries for crates a package doesn't depend on carry `allow-invalid = true`.
- Scope:
  - each `lib.rs` has `#![cfg_attr(not(test), deny(clippy::disallowed_methods))]`, so the library code is
    checked;
  - `[lints.clippy] disallowed_methods = "allow"` in both `Cargo.toml` leaves integration tests, examples
    and bins (`svelters-egui-run`) alone. Unit tests are exempt through `cfg(test)`.
  - The modules are not split into `ui`/`logic` here. Both crates' libraries are UI-thread code apart from
    the I/O sites below, so the deny covers the whole library and the I/O sites are opted out one by one.
- Every opt-out is `#[allow(clippy::disallowed_methods)]` preceded by a `// ZU3: <reason>` line.

  Legitimate (not on the UI thread, or not run per frame):

  | Site | Reason |
  |---|---|
  | `host/bytecode.rs` (module) | the loader's bytecode cache, used while loading modules (startup, reload) |
  | `host/exports/codegen.rs` (module) | build-time typings dump |
  | `host/testing.rs` (module) | test support |
  | `host/exports/offload.rs` `blocking_loop` | the blocking pool's worker threads |
  | `host/exports/service.rs` `StopToken::wait`, `shutdown_services` | service threads; bounded join at shutdown |
  | `host/exports/stream.rs` `send` | producer side (workers), backpressure |
  | `host/platform.rs` `reveal_system` | Linux/BSD: dbus-send on its own thread. macOS (`open -R`) and Windows (`explorer /select,`): a detached spawn, never waited on, once per user action, as `open::that_detached` does for open_url/open_path (FX5, 2026-10-22: the cfg'd sites were missed because CI's clippy runs on Linux only) |
  | `host/quickjs.rs` `Drop` | teardown, bounded `TRANSFER_GRACE` |
  | `host/reload.rs` `start_watch`, `sync_watch`, `Watch::drop` | watcher thread, the loader's reload, teardown |
  | `host/sourcemap.rs` `map_for` | error path only (one `.map` read per module) |
  | `host/dx/frames.rs` `Histogram::finish`, `sampler::capture` | this lane: write at exit; watchdog thread |
  | `egui/spec.rs` `dump` | build-time typings dump |

  **Debt: real UI-thread I/O that the lint found.** These are allowed for now and should move off the UI
  thread:

  | Site | What | Owner |
  |---|---|---|
  | `host/dx/settings.rs` `read_json`, `write_json_atomic` | `FileUiState` reads and writes the per-root UI-state file on the UI thread (mount, save) | DX follow-up |
  | `host/platform.rs` `check_path` | one `exists()` stat per shell open | DX follow-up (small) |
  | `egui/elements/images.rs` `PngLoader::load` | `file://` PNGs are read in egui's `ImageLoader::load`, on the UI thread, the first time each URI loads | AS1 (assets and egui loaders) |

- **Other lanes.** No AU1, AS1, FX3, WG5b or WG6 code was on master when this lane branched. Of their files,
  only `elements/images.rs` (the debt row above) needed an allow. When their code lands, a new call fails
  CI's clippy step. Then add an `#[allow]` with a reason; don't refactor their code.
- **CI.** Before this lane the only clippy run in CI was `cargo clippy -p svelters-cli --all-targets -- -D
  warnings`, in `js.yml`. Nothing covered host or egui. `linux-gtk.yml` now has a step "clippy (UI-thread
  lint)", `cargo clippy -p svelters-host -p svelters-egui --all-targets`, placed after "Build tests" in the
  job that builds both crates. There is no `-D warnings`, because the crates have pre-existing advisory
  warnings (about 40). The step fails only on deny-level lints, and this lint is one of them.

### 2. Hang watchdog (`svelters_host::dx::frames`)

- `FrameMonitor` lives on the UI thread. `begin()`/`end()` bracket a frame; a second `begin` before `end`
  keeps the first start, so eframe's `logic` + `ui` count as one frame.
- A `svelters-hang` thread polls 4× per threshold (clamped to 5–250 ms). When a frame has been running for at
  least N ms, it takes the UI thread's stack and logs one `warn` per hung frame (target
  `svelters_host::frames`) with the symbolised stack. When that frame ends, the UI thread logs "UI thread hang
  ended: the frame took X ms" plus DX1's crash context (`crash::context()`: the last host ops and JS errors).
  That context is thread-local, so only the UI thread can read it. The end line can come before the stack
  report, which is symbolised first.
- Idle time between frames is not a hang: the timer runs only between `begin` and `end`.
- **Stack capture.** ZED-LESSONS names no capture technique; `backtrace` is not mentioned there. I used the
  sampling-profiler approach (as in pprof-rs):
  - `pthread_kill(ui, SIGURG)`;
  - the handler runs `backtrace::trace_unsynchronized` into a static atomic buffer of 128 return addresses,
    with no allocation;
  - the watchdog waits up to 200 ms for it, then resolves the symbols on its own thread.

  SIGURG is unused by svelters and its default action is to ignore it. If the app already has a SIGURG
  handler, capture is skipped and the hang is logged as "no stack: …". Windows has no capture yet: the hang
  is logged without a stack (SuspendThread + StackWalk2 is the follow-up). Caveat: if the UI thread hangs
  while it holds the dynamic loader's lock, the unwinder in the handler can deadlock that thread. It was
  already hung, and the watchdog thread itself never blocks (bounded wait).
- **Config.** `SVELTERS_HANG_MS=<n>` sets N, and `0` turns the watchdog off. When unset, it is on at 500 ms in
  debug builds and off in release builds.
- **Wired** into `runner/eframe.rs` (`EframeApp`, `DesktopApp`), so it covers `svelters-egui-run` and
  `Backend::run`. An embedder that owns its eframe `App`, such as cadatomic, creates `FrameMonitor::from_env()`
  on its UI thread and calls `begin`/`end` around its frame. The kittest harness is not wired, so tests start
  no watchdog threads.
- Deps: `backtrace 0.3` and `libc 0.2`, unix only, in svelters-host. They add addr2line, gimli, object and
  rustc-demangle to the lockfile.

### 3. Frame-time histogram

- With `SVELTERS_MEASUREMENTS=1`, the monitor records each frame's UI-thread time (`begin`→`end`) in µs, up to
  2^20 frames; frames past that are counted as `dropped`.
- On drop (window closed) it logs `n, p50, p90, p99, max` and writes `{"version":1,"unit":"us","frames":[…]}`
  to `SVELTERS_MEASUREMENTS_OUT`, or by default to `<temp>/svelters-frames-<pid>.json`.
- `tools/dev/frame-histogram a.json [b.json] [--fail-above PCT]` prints bucketed histograms (0–1 … >500 ms)
  side by side, plus percentiles with the change per row. With `--fail-above`, it exits 1 when p50/p90/p99
  regressed by more than PCT %.

## Off costs nothing (release)

With `SVELTERS_HANG_MS` and `SVELTERS_MEASUREMENTS` unset, a release build does no extra work:

- **No thread, no allocation.** `FrameConfig::from_env` yields `hang: None, measurements: None`.
  `FrameMonitor::start` returns `None` on its first line (`config.is_off()`), before it allocates the `Shared`
  state or spawns `svelters-hang`. The watchdog thread is spawned in `Watchdog::start` and nowhere else, and
  that is reached only when `hang` is `Some`.
- **No per-frame work.** The runner holds an `Option<FrameMonitor>` that is `None`, so each frame costs one
  `if let Some` per callback: a predictable branch, with no clock read, atomics or sampling.
- **One-time cost.** The env vars are read once when the window is created. The SIGURG handler is installed
  lazily, at the first capture.
- **Test.** `dx::frames::tests::release_default_is_off_and_starts_nothing` checks that the release defaults
  are off and that `start` returns `None`.

## Tests

`cargo nextest run -p svelters-host --lib frames` runs these tests (all pass locally):

- `release_default_is_off_and_starts_nothing`
- `env_values`
- `histogram_records_frames_and_writes_json`
- `watchdog_reports_a_hung_frame_with_its_stack`: a 40 ms threshold and a 400 ms frame. It checks that the
  hang is reported once, that the hang-ended line is logged, and, on unix, that the stack names
  `hung_frame_body`. Locally the stack was `clock_nanosleep` ← `nanosleep` ← `hung_frame_body` ← the test.

## Not done / follow-ups

- The ARCH-SCENE §0.8 targets as benches (§9 #3's last item). They are a separate lane.
- Windows stack capture.
- An out-of-process hang/crash reporter (§7.5).
- Moving the three debt sites off the UI thread.
- cadatomic wiring `FrameMonitor` into its own `App`.
