# P2 host lane: working notes

## Layout
- `svelters-abi/src/safe.rs` (additive; raw types untouched): `Value`, `BackendRef`/`HostRef` (call a vtable safely), `Backend` trait + `BackendVtable` (implement a backend in Rust), `HostCallbacks` + `HostVtable`, `WakeHandle`.
- `svelters-host`: `core.rs` (engine-agnostic: handle table, name->ID caches, event queue, timer heap, checkpoint, op forwarding), `engine.rs` (`JsEngine` trait, `JsError`), `quickjs.rs` (rquickjs 0.14.0 / QuickJS-NG implementation, bundle loader), `prelude.js` (console, timers, queueMicrotask), `testing.rs` (run a bundle on the log backend; fixture harness).
- `svelters-backend-log`: `LogBackend` (implements `Backend`), script parsing, op-log writer.

## Decisions
- **rquickjs 0.14.0**, features `std` + `loader`, no `rust-alloc` (that feature turns `set_memory_limit` into a no-op). Sync `Runtime`, never leaves the creating thread.
- **Trait methods take `&self`.** The backend is re-entered by design (its `run` calls `host.checkpoint`, which calls `create`/`set_prop`), so `&mut self` would alias. Backends use `RefCell`/`Cell`. `Backend::waker()` is called once at vtable build; the returned closure is what `svl_backend.wake` runs, from any thread, and touches nothing else in the shim.
- **Timers without ABI support.** The ABI has no "wake me at T". The host runs one timer thread that sleeps (condvar, no polling) until the earliest deadline and calls the backend's thread-safe `wake`; the backend then calls `checkpoint` on its loop thread, which fires due timers. One wake per deadline update. The thread is joined in `Host::drop`, before the backend can go away.
- **Checkpoint** (`core::checkpoint`): events -> JS (jobs drained after each), due timers (jobs drained after each), final drain; the total job count is capped by `max_jobs_per_checkpoint`; if jobs or events remain, `backend.wake()`.
- **Re-entrancy.** Every entry into JS holds a `JsGuard`. A `checkpoint` requested while JS runs (nested toolkit loop from a backend op, e.g. a modal dialog) does nothing except mark `deferred`; when the outermost guard drops it calls `backend.wake()`, so the work runs later as a plain checkpoint. No `RefCell` is borrowed across a backend or engine call. Tested with a trait backend whose `set_prop` calls `host.checkpoint`.
- **Engine boundary.** `core.rs` names no rquickjs type. An engine implements `JsEngine` (`load_module`, `dispatch`, `fire_timer`, `run_jobs`, `take_errors`) and reaches the core through `op_*`, `timer_*`, `console`, `report`. `Host::with_engine` is `pub(crate)` for now.
- **Handles are JS numbers** (u64 as f64, below 2^53). The core keeps handle -> kind id, which scopes prop/signal/method/slot resolution.
- **Value conversion.** JS int/integral float -> `I64`, other floats -> `F64`, arrays -> `List` (depth limit 16), `undefined` and objects -> TypeError. Log backend prints numbers like `JSON.stringify`.
- **Module loader.** Entry file's directory is the root. Only `./` and `../` specifiers; paths are normalised in the resolver and canonicalised in the loader (symlinks cannot leave the root).
- **Errors.** Uncaught exceptions in dispatch/timers/jobs and unhandled rejections go to `Host::take_errors()` with message + JS stack (no source maps, M2). `ErrorKind` distinguishes Interrupted / OutOfMemory / Load / UnhandledRejection. Rejections are reported when the tracker sees them unhandled, not after the microtask drain, so `p = Promise.reject(); p.catch(..)` in the same tick is reported as a false positive.
- **Limits.** `memory_limit` (default 256 MiB), `max_stack_size` (1 MiB), `turn_budget` (10 s per entry into JS, enforced by the interrupt handler), `interrupt_flag()` for external interrupts.

## Scripted mode (`HostConfig::scripted`)
The P1 contract (`docs/notes/p1-js-lane.md`) steps differently from a live toolkit: an `event` step dispatches at once with no job drain, a `checkpoint` step drains jobs and fires the earliest timer on a virtual clock until both are empty, loading drains nothing, and the run ends with a final job-only drain. Scripted mode implements exactly that on top of the same `checkpoint` entry point:
- checkpoint with queued events: deliver them, nothing else;
- checkpoint with no queued events: drain jobs, then fire timers in (due, creation) order, draining jobs after each, until empty (cap `max_virtual_timer_fires`);
- timers use a virtual clock with no 1 ms floor and never wake the backend; the log backend's `event` step posts and then checkpoints.
`Host::drain_jobs()` provides the final flush. Live mode uses wall-clock timers with a 1 ms floor.

## Fixture harness
`svelters_host::testing::run_fixture(dir)` -> `Pass | Fail(diff) | Skipped(why)`; `tests/fixtures.rs` runs `fixtures/*/`. `SVELTERS_REQUIRE_FIXTURES=1` makes skips fail (for P5/CI). Build the bundles first: `pnpm install && pnpm -r build`.

## Open points
- Nested-loop checkpoints turn into an extra wake. In scripted mode an extra event-less checkpoint can fire virtual timers early; no fixture backend nests, so this is theoretical.
- `call_sync` returns an error until M4.
- `Host::with_engine` stays crate-private until a second engine exists.
