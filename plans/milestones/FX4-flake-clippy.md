# FX4: interrupt-test flake, svelters-gtk clippy error

Lane FX4, started 2026-10-22 on master a6f72d82. A cleanup lane: one flaky wall-clock test and one
long-standing clippy error.

## 1. `an_infinite_handler_loop_is_interrupted_within_100ms_and_faults_only_its_root`

**The flake.** CI linux-gtk run 39851499792 (attempt 1) failed it at 147 ms against a 100 ms bound,
then passed on rerun. The test bounded `EmbedStats::last_logic`: the wall time of the whole embed
logic pass for the frame where the fault appeared. That pass is more than the stopped entry. It
also applies commands, collects faults, disposes the faulted root and runs `after_pass`. On a
loaded runner (nextest runs tests in parallel), those debug-build steps took the extra ~97 ms. The
interrupt itself was not shown to be late.

**Why not a virtual clock.** The interrupt handler (`crates/svelters-host/src/quickjs.rs`,
`rt.set_interrupt_handler`) reads `Instant` through the engine's `base`, not SCHED1's `Clock`
(`crates/svelters-sched`). The seam would be small to add, but it would be wrong:
- the turn budget bounds real time spent blocking the UI thread, and a virtual clock does not
  measure that;
- with a `TestScheduler` clock, which only advances when the test tells it to, a runaway script
  would never be interrupted. Tests that run on SCHED1 clocks rely on the turn budget to stop
  hangs, so it must stay on wall time;
- a deterministic version (a clock that advances on each poll) would test only the deadline
  arithmetic, not interrupt latency.

**What changed instead.** The host now records its own measurement of a budget stop:
- `svelters_host::BudgetInterrupt { budget, late }` (`#[non_exhaustive]`), read with
  `Host::last_interrupt()`.
- When the deadline (not the external `interrupt_flag`) stops a script, the interrupt handler sets
  `deadline_hit`. `QjsEngine::disarm`, the common exit of `call_guarded` and `run_jobs`, then
  records `late`: the time from the deadline until the entry returned to Rust with the abort.
- The test asserts the following:
  - `budget == 50 ms`, the `Budgets::default().interrupt` handler budget;
  - `budget + late <= 100 ms`. The name's "within 100 ms" now means the host's own arm-to-abort
    time: 50 ms of spinning, then at most 50 ms from the deadline to the abort;
  - `logic >= budget`: the budget really ran;
  - `logic <= 2 s`: a loose sanity bound on the whole pass, which is all that the commands and
    disposal around the entry get.
- Locally: `late` was 144 µs (logic 51.6 ms). QuickJS polls the handler every few thousand
  bytecodes, so only a preemption that lands right at the deadline can make `late` large. A
  50 ms bound leaves room for that, and a broken poll would still fail it.
- `crates/svelters-host/tests/host.rs`:
  - `runaway_loop_during_load_is_interrupted` now also checks that the stop is recorded (budget
    150 ms);
  - `external_interrupt_flag_stops_a_script` checks that a flag stop is not recorded.

## 2. svelters-gtk clippy error: `mut_from_ref` in `backend/visual.rs`

`fn visual(w: &gtk::Widget) -> &mut Visual` returned a `&mut` into the widget's GObject data from
a shared reference. Clippy denies `mut_from_ref` by default, and here the lint is right: two calls
on the same widget give two aliasing `&mut Visual`, which is undefined behaviour. The current
callers (`visual_op`, then `apply_visual`) only avoided overlapping borrows by luck. The fix:
- the widget's data is now an `Rc<RefCell<Visual>>`;
- `visual()` returns a clone of the `Rc`;
- `visual_op` drops its borrow before calling `apply_visual`.

No `allow` was added. The crate's other clippy diagnostics are warnings, not errors, and are left
alone. Two of them come from the generated `slots.rs` and from `gi.rs`.

## Verification

- `cargo clippy -p svelters-gtk -p svelters-host -p svelters-egui --all-targets`: no errors.
- Filtered tests, under `tools/dev/headless`, all pass:
  - `svelters-egui::embed an_infinite_handler_loop_…`;
  - `svelters-host::host runaway_loop_during_load_is_interrupted`;
  - `svelters-host::host external_interrupt_flag_stops_a_script`;
  - `svelters-gtk::style visual_channels_and_bounds`, which exercises the transform, clip and
    inert channels.
- Not run locally: anything else. The five CI workflows are the gate.
