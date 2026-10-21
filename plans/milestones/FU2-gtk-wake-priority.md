# FU2: GTK wake priority

Follow-up from E1 (`plans/milestones/E1-exports-offload.md`, open items): the GTK backend's `wake` was a
`g_idle_add` source (`G_PRIORITY_DEFAULT_IDLE`, 200). GLib dispatches, in each iteration, only the ready
sources of the best ready priority, so an idle is starved for as long as some default-priority source is
ready at every iteration. A 1 ms timer whose handler takes about as long as its interval is enough: worker
completions, store updates, `prop_changed` batches and everything else that arrives through `wake` were
never delivered while it ran. E1 had to measure its responsiveness test by driving the loop by hand.

## 1. GTK `wake` (`crates/svelters-gtk/src/backend/wake.rs`)

- **Priority: `G_PRIORITY_DEFAULT` (0).** The priority of GDK input events and of ordinary timeouts
  (`g_timeout_add`, including our own `schedule_wake` and frame fallback timers). A wake that is ready is
  dispatched in the same iteration as any other ready default source, so nothing at the default priority
  can starve it. It does not starve input either: input is at the same level, and GLib dispatches every
  ready source of that level in one iteration. `HIGH_IDLE` (100) would still be starved by a busy default
  source; `HIGH` (-100) would put JS ahead of input.
- **Redraw guard.** GTK's frame clock lays out and paints at `GDK_PRIORITY_REDRAW` (`HIGH_IDLE + 20` = 120),
  below `DEFAULT`. The old idle wake always let redraw go first; at `DEFAULT`, a checkpoint that keeps
  re-waking (a JS-thread future that yields, a completion backlog over E1's 1024 / 4 ms drain budget, a
  thread waking faster than checkpoints run) would freeze the window. So after **4 wakes in a row** at
  `DEFAULT`, the next wake *yields*: it is an idle at `GDK_PRIORITY_REDRAW + 1`, which runs at the first
  iteration where nothing at redraw priority or above is ready (immediately when the loop is calm), and it
  resets the count. If the loop stays saturated at those priorities (somebody else's busy default source),
  a backup timeout at `DEFAULT` delivers the yielded wake after **4 ms**, so a wake is never starved, only
  up to 4 ms late, and the count is reset again. The two sources carry the request's ticket; whichever runs
  second finds the ticket gone and does nothing. 4 × E1's 4 ms drain budget is about one 60 Hz frame.
- **Coalescing.** The old code queued one idle per `wake` call (800 calls, 800 checkpoints). Now one
  wake is pending at a time (an atomic ticket); requests while it is pending are dropped. The ticket is
  cleared before the checkpoint runs, so a wake requested during the checkpoint gets a checkpoint of its own.
- **Threads.** `request` uses atomics and `g_idle_add_full`/`g_timeout_add_full` on the default main
  context (thread-safe); dispatch is on the UI thread. The host pointer is re-read at dispatch, as before
  (`start(NULL)` / `run` returned: no call).

## 2. Other places the GTK backend schedules host work

| Site | Mechanism | Priority | Starvable by a busy default source? |
| --- | --- | --- | --- |
| `wake` | idle (FU2) | `DEFAULT`, guard below redraw + 4 ms backup | no (fixed here) |
| `schedule_wake` (JS timers) | `timeout_add_local_once` | `DEFAULT` | no |
| `request_frame` (rAF) | frame clock `update` | `GDK_PRIORITY_REDRAW` | yes, like GTK's own redraw; the 250 ms watchdog timeout (16 ms fallback without a frame clock) is `DEFAULT` and delivers the frame |
| `run` first checkpoint | `GApplication::activate` | synchronous inside `run` | n/a |
| G0 `start` | attaches only; the first checkpoint comes from the host's `wake` | `DEFAULT` (via `wake`) | no |
| `observe` reports from another thread (`backend/observe.rs`) | idle hop to the main context | was `DEFAULT_IDLE`, now `DEFAULT` | no (fixed here) |
| async GI completions (NT1) | GIO's own `GTask` return | GIO's default | no |
| closure frees (`closures.rs` `free_later`), toplevel destroy / strong-ref drop after `destroy` | idle | `DEFAULT_IDLE` | yes, but these are cleanup, not host work: they are only late under saturation. Left as they are (running cleanup ahead of input buys nothing) |
| inline style flush (`backend/style.rs`) | idle | `HIGH` | no (one-shot, not host work) |

## 3. Other backends

| Backend | `wake` mechanism | Priority | Starvation by a busy timer | Coalesced |
| --- | --- | --- | --- | --- |
| GTK | `g_idle_add_full` | `G_PRIORITY_DEFAULT`; after 4 in a row, `GDK_PRIORITY_REDRAW + 1` with a 4 ms `DEFAULT` backup | no (FU2) | yes (FU2) |
| Qt | `QMetaObject::invokeMethod(waker, ..., Qt::QueuedConnection)`: a posted event, `Qt::NormalEventPriority` | GLib dispatcher: the post-event `GSource` at `G_PRIORITY_DEFAULT`; Unix dispatcher: posted events are sent at the start of every `processEvents` | no. Also, while `exec` runs, Qt's GLib dispatcher demotes timers after one fires (they wait for an idle-priority turn), its own guard against timer starvation | no (one posted event per `wake`; the host coalesces its own wakes) |
| AppKit | version-0 `CFRunLoopSource` (`CFRunLoopSourceSignal` + `CFRunLoopWakeUp`), common modes | order 0 | no: every signalled source0 runs on each run-loop pass, timers or not | yes (a signalled source runs once) |
| WinUI | `DispatcherQueue.TryEnqueue` | `DispatcherQueuePriority::Normal` | no: the queue is FIFO within a priority, and `DispatcherQueueTimer` ticks come through the same queue (read from the docs, not measured: CI-only backend) | yes (`pending` flag) |
| egui | atomic flag + `Context::request_repaint` | n/a (consumed at the start of the next pass, `before_pass`) | no: no prioritised sources; every pass checks the flag | yes (flag) |

No change outside GTK. AppKit and WinUI are read from the code only (CI-verified backends).

## 4. Tests

- `crates/svelters-gtk/tests/wake_priority.rs` (stub `svl_host` counting checkpoints, real GTK loop):
  - `a_busy_default_priority_timer_does_not_starve_wake`: a 1 ms `DEFAULT` timer whose handler spins
    1.5 ms (ready at every iteration); 30 rounds of `wake` from a fresh thread, each must reach the host
    within 2 s, worst latency < 100 ms. New code: worst 7.1 ms. **Old code: fails in round 0**, "a wake from
    another thread was not delivered within 2.0017s while a default-priority 1 ms timer was busy (1319 timer
    dispatches)" (both nextest tries).
  - `wakes_coalesce_until_the_checkpoint_runs`: 8 threads × 100 wakes before the loop turns → 1
    checkpoint; then one more each from another thread and from the UI thread. **Old code: fails** (800
    checkpoints).
  - `rewaking_checkpoints_do_not_starve_redraw`: every checkpoint re-wakes (400 in a chain); every 10th
    makes a one-shot idle at `GDK_PRIORITY_REDRAW` ready (a label change queuing a redraw). Each must run
    within 8 checkpoints (new code: at most 3). Passes on the old code (idle wakes always yield); **fails
    with the guard disabled** (`STREAK = u32::MAX`: no redraw ran during the chain), which is what it pins.
- `examples/counter-qt/tests/wake_priority.rs` `qt_a_busy_timer_does_not_starve_wake`: the same inside
  `QCoreApplication::exec` (the backend's `run`) with a self-re-arming 1 ms Qt timer spinning 1.5 ms per
  tick; each of 30 rounds waits for a fresh tick, then wakes from another thread. Worst 1.7 ms. Qt needed no
  change; the test pins it.
- `examples/exports/tests/exports.rs` `worker_job_keeps_the_ui_responsive`: the 1 ms `DEFAULT` probe
  timer E1 wanted is back (it runs during both jobs; the worker job's completion must get through it). New
  asserts: the probe's longest gap < 100 ms during the worker job (8.5 ms locally) and ≥ 150 ms during
  the blocking one (204 ms). The hand-driven dispatch measurement stays.

## Completion record

Branch `ci/fu2`, code head **da8fd6e0** (based on master 4ce61e32). This record is a docs-only commit on top.

CI on da8fd6e0, all success: js 39656941892, linux-gtk 39656942002 (GTK 4.14 1933 tests and Arch GTK 2222,
`wake_priority` 3/3 on both, `worker_job_keeps_the_ui_responsive` ok), linux-qt 39656941937 (97/97,
`qt_a_busy_timer_does_not_starve_wake` ok), macos-appkit 39656942016, windows-winui 39656941976.

Local gates on da8fd6e0, all rc 0:

| Gate | Result |
| --- | --- |
| `tools/dev/cargo-slot cargo check --workspace --all-targets` | rc 0 (only warnings that were already there) |
| `flock ... tools/dev/headless tools/dev/cargo-slot cargo nextest run --workspace --test-threads 4` | 2427/2427 passed, 9 skipped (an earlier run hit `svelters-host::reload failed_reload_keeps_the_process_and_recovers` on both tries: a log-backend file-watcher timing test that FU2 does not touch; 3/3 passes alone, and the rerun was clean) |
| `pnpm -r build` (after `cargo build -p svelters-compiler`, `python3 tools/docs/build_ir.py`) | rc 0 |
| `pnpm test` (incl. `ledger.py check`: gtk 580, qt 634, appkit 205, winui 105 manual facts) | rc 0 |
| `pnpm docs:test` | rc 0 (11 tests, 1 skipped) |

## Open items

- Qt `wake` does not coalesce in the backend (one posted event per call; `g.queued` is only read by
  `run`). The host coalesces its own wakes (offload's pending flag, the dirty set), so this costs
  extra empty checkpoints only with many independent wakers. A one-line `if (g.queued.exchange(true))
  return;` would do it; left out to avoid a Qt backend rebuild/regeneration here.
- AppKit: a checkpoint chain that re-wakes on every pass keeps the run loop from sleeping, and Core
  Animation commits in the `BeforeWaiting` observer, which CFRunLoop skips while sources keep firing. So the
  redraw-starvation case of §1 may exist there (not observed; CI-only backend). WinUI's layout/render
  ordering against `Normal` queue items under a re-wake chain is likewise unmeasured.
- GTK rAF (`request_frame`) is still starved with the rest of redraw by a busy default source; the
  `DEFAULT` watchdog (250 ms) bounds it. Lowering the watchdog is a separate trade-off (spurious frames).

## Conflict risks

- `examples/exports/tests/exports.rs` (`measure` in `worker_job_keeps_the_ui_responsive`: a probe timer and a
  fourth tuple element); E2/E3 also edit `examples/exports`.
- `crates/svelters-gtk/src/backend.rs` (`Ctl` gained `wake`, `b_wake` body, `mod wake`),
  `crates/svelters-gtk/src/backend/observe.rs` (`Reporter::on_main`).
- `plans/architecture/backend-abi.md` (new "Wake delivery (FU2)" paragraph after "Checkpoint semantics"),
  `plans/milestones/E1-exports-offload.md` (one open-item line).
- No host, ABI, macro or `convert.rs` change.
