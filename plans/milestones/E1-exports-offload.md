# E1: Rust exports offload (workers, async, cancel, progress)

Design: `docs/notes/egui-rust-exports.md` §6 (threading and compute offload), §10 (phasing), §11 (decisions);
`plans/research/EGUI.md` §7, §8 D6. Builds on E0 (`plans/milestones/E0-rust-exports.md`). This file records E1
as built.

## User decisions (2026-10-21)

1. **A bare `async fn` runs on the JS thread** (local, Deno-style). Pool execution is opt-in: `#[js(tokio)]
   async fn` (tokio, behind the optional `tokio` feature of `svelters`; tokio is not a default dependency),
   `#[js(worker)]` and `#[js(blocking)]` for sync code. Guard against blocking the UI: documented on the facade
   crate, and debug builds print one console warning per member when a single poll of a JS-thread future takes
   over 20 ms.
2. **Abort rejects immediately** with `NativeError { domain: 'svelters', code: 'cancelled' }`. A late result is
   dropped, and freed on the JS thread; the worker sees the cancel through `WorkCx`.

Recorded as decided in design note §11 (Q2, Q3) and EGUI.md §8 (D6). Still open, not touched here: foreign
traits called from workers (Q4, left out of E1), the rquickjs fast path, store granularity, subsecond.

## As built

### Execution classes

| Declared as | Runs on | Receiver | JS |
| --- | --- | --- | --- |
| `fn` | JS thread, synchronously (E0) | any | value / throw |
| `#[js(worker)] fn` | rayon: the global pool, or `svelters::exports::set_worker_pool(pool)` | `self: Arc<Self>` or `&self` of a shared object | `Promise<T>` |
| `#[js(blocking)] fn` | `svelters-blocking-N` threads, grown on demand up to 64, idle exit after 10 s | as worker | `Promise<T>` |
| `async fn`, `#[js(local)] async fn` | the JS thread: polled at the call, then when its waker fires (at checkpoint start) | any, incl. `&mut self` of a UI object (borrowed across `.await`) | `Promise<T>` |
| `#[js(tokio)] async fn` (feature `tokio`) | `set_tokio_handle(h)`, else a 2-thread runtime of ours (`svelters-tokio`) | as worker | `Promise<T>` |

- **Compile-time checks** (targeted messages through `#[diagnostic::on_unimplemented]` or macro errors):
  worker/blocking/tokio parameters and results must be `Send + 'static` (`WorkerSend`); worker methods need
  `#[svelters::object(shared)]` (`OffloadSelf`); `&mut self`, borrowed parameters (`&str`, `&[T]`, `&T`),
  `#[js(worker)] async fn`, `#[js(tokio)]` on a sync fn, `#[js(local)]` on a sync fn, offloaded
  constructors/accessors, and `supersede`/`max_in_flight` without offload are macro errors with fix-its;
  `#[js(tokio)]` without the feature fails on `TokioFuture` with "needs the `tokio` feature".
- **Helpers for JS-thread futures:** `svelters::exports::worker(|| ...)` and `blocking(...)` return a future of
  the closure's result; a panic resumes in the awaiting future.

### `WorkCx<P>`

Injected (never a JS argument). `check() -> Result<(), Cancelled>`, `is_cancelled()`, `abort_if_cancelled()`
(unwinds with a `Cancelled` payload, no panic message), `cancelled()` (a future, for `select!` in async code),
`progress(p)`. `Cancelled` converts with `?` into `NativeError`, `String` and boxed errors. `P` (default `()`)
is the `onProgress` type; `WorkCx` is `Clone + Send + Sync` (for `par_iter`).

Progress is a latest-value slot: `progress()` stores under a mutex and sets an atomic; the first report after a
delivery pushes one notification to the queue. The drain delivers the value once per checkpoint, before a
completion of the same call in the same batch.

### Path of a call

1. **Start** (`host.call`/`host.callStatic` of a member with `OffloadDef`, in `core/offload.rs`): the declared
   arguments are encoded with the member descriptor (E0's validation), then an optional trailing
   `{ signal, onProgress }`; the call is logged like E0's; the engine makes the Promise (`JsPromises::new_op`,
   which registers the abort listener); the generated start thunk converts the owned values and builds the
   `Job`. An already-aborted signal rejects without starting. `supersede` cancels the key's earlier calls
   (`superseded`); `max_in_flight` queues beyond `n` per (member, object).
2. **Run:** pool jobs are skipped when cancelled before they start; panics are caught (`catch_unwind`, a
   `CatchUnwind` future wrapper for local/tokio). A JS-thread future is polled at once; ready means the Promise
   settles one microtask later in the same turn.
3. **Complete:** jobs push `(id, generation, outcome)` to `Completions` (`exports/offload.rs`): a mutex'd queue
   plus an atomic "wake pending", so a burst costs one backend `wake`. The host detaches the waker on drop.
4. **Drain** at checkpoint start (`core::checkpoint`, before the R1 change batch, both live and scripted): woken
   local futures are polled, progress taken, completions converted with the object table at hand (at most 1024
   or 4 ms per checkpoint, the rest re-wakes), host-side rejections added, and the batch settled in **one
   engine entry** (`JsEngine::settle_ops`: QuickJS builds one `[kind, id, value, ...]` array and calls the
   prelude's `settleOps`). Reactions then run in the checkpoint's single job drain.
5. **Cancel:** the abort listener rejects in JS and calls `native.cancelOp`; the core fires the op's token
   (`OpShared`): a queued job is dropped, a JS-thread future dropped at once, a tokio task aborted (a drop guard
   reports its end), and a running pool job stays as a zombie until it ends; its result is dropped without
   conversion, so it never mints a handle and its objects are freed on the JS thread. `max_in_flight` slots are
   held until the job really ends.
6. **Owner disposed** (`dispose()` while a call runs): the call rejects with `disposed` at the next checkpoint and
   its token fires; the job keeps its own `Arc`, so the object drops when the job ends.
7. **Reload:** every op carries the generation of the runtime that started it. A reload starts a new generation;
   on success the old runtime's calls are retired (tokens fired, results dropped), on failure the failed
   load's calls are, and the generation is restored. A completion whose generation is not current is dropped.

### Typings, stubs, schema

- `.d.ts`: `Promise<T>` results and a trailing `options?: AsyncOptions` or `options?: WorkOptions<P>` (new in
  `@svelters/types/core`: `AsyncOptions` + `onProgress`), with a doc line saying where it runs, supersede/cap,
  and the `cancelled`/`panic` rejections. Types are imported per module only when used.
- Stubs: `callAsync(this, name, [args], options, shape)` / `callStaticAsync(...)` from the core (E0's async
  helpers, which keep the wrapper reachable while a call is in flight).
- `svelters-exports/1` members gain `runsOn` (`ui`/`worker`/`blocking`/`local`/`tokio`), `async`, `supersede`,
  `maxInFlight`, `progress`.

### API additions

- `svelters`: `WorkCx`, `Cancelled` (also in the prelude), `rayon` re-export, feature `tokio`; module docs.
- `svelters_host::exports`: `offload` module (`Exec`, `Job`, `WorkCx`, `Cancelled`, `CancelledFuture`,
  `Spawned`, `worker`, `blocking`, `set_worker_pool`, `set_tokio_handle`), `OffloadDef`, `StartThunk`,
  `MemberDef::offload`, `Sig::progress`, `MemberSchema::offload`.
- `svelters_host`: `OpSettle`/`OpOutcome`, `JsEngine::settle_ops` (default: unsupported),
  `Host::ops_in_flight()`, `Host::offload_stats()` (`OffloadStats`).
- Dependencies: `rayon` 1.12 (svelters-host), `tokio` 1.x optional.

## Deviations from the note

- **`async fn` is JS-thread by default**, `#[js(tokio)]` opts into tokio (decision 1; the note had tokio).
- **Results convert on the JS thread.** Workers return the Rust value; `IntoJs` runs in the drain, because
  returning an exported object needs the host's object table (identity). Cost: conversion is on the UI thread,
  which is fine for E1's data sizes; large data is E2's `Buffer`/`Shared`.
- **No `#[svelters::service]`, `Emitter`, `Sink`/streams, `Buffer`**: E2/E3 as phased. `WorkCx::emit` waits
  for `Emitter` (E3).
- **Dispose rejects at the next checkpoint**, not inside `dispose()`: the release path has no engine handle,
  and a deferred rejection keeps it free of re-entrancy. Abort and supersede reject at once.
- **Local futures that complete at once settle one microtask later** (as E0's async CALLBACK path does), so a
  rejection is never reported as unhandled before the caller attached a handler.
- **A conversion failure of a result** (an integer outside ±2^53) rejects with `svelters` / `range` instead of
  throwing a `RangeError` (there is no caller frame to throw into).
- **Progress slots** are typed per call and allocation-free per report; the note's "delivered to
  `options.onProgress`" holds, and a throwing `onProgress` is reported as a job exception.
- **No backend change.** The GTK `wake` is an idle-priority source; the example's measurement therefore drives
  the loop by hand (a default-priority 1 ms probe timer would starve it; noted for FU1/G1).

## Measurements

Machine: WSL2, 16 cores, **load average 16–20** during the runs (other lanes building), so absolute times are
pessimistic; ratios within one run are the reliable part. Release builds.

**UI responsiveness on GTK** (`examples/exports` test `worker_job_keeps_the_ui_responsive`, Xvfb, debug build):
the app's "Crunch" button runs a 200 ms prime count; the test measures the longest single GTK main-loop
dispatch while it runs, and the app counts a 5 ms JS `setInterval` during it.

| Job | Longest GTK dispatch | GTK dispatches | JS timer ticks during the job |
| --- | --- | --- | --- |
| `crunch` (`#[js(worker)]`) | **5.3 ms** | 124 | **31** |
| `crunchBlocking` (same code, sync on the JS thread) | 201.3 ms | 4 | 0 |

**Settle cost** (`cargo test --release -p exports-example --test exports -- --ignored --nocapture
offload_settle`, log backend, the host driven in a tight loop; 3 rounds each; "settle" = drain time / settled
completions, "engine" = the part inside `settle_ops`):

| Path | JS wall per call | settle per completion | of which engine |
| --- | --- | --- | --- |
| 10,000 worker no-ops in flight (`Promise.all`), ~1000 per drain | 30.9–39.0 µs | **4.1–4.7 µs** | 2.9–3.4 µs |
| 2,000 sequential `await noopWorker(i)` (one per drain) | 48.0–51.7 µs round trip | 8.6–8.9 µs | 6.6–6.8 µs |
| 10,000 JS-thread `noopLocal` (ready at once, settled in the call) | 27–33 µs | n/a (no drain) | n/a |

**The CALLBACK path, re-measured on the same machine and load** (the note's probe,
`docs/notes/scripts/egui-exports-probe`, `cargo run --release -- 1048576`): `invoke_callback` **19.8 µs** per
completion; async no-op on rayon 240.8 µs per sequential round trip and 74.0 µs/op with 10,000 in flight; a
200 ms worker step delivered 169 1 ms ticks meanwhile.

So settling through the completion queue costs about **4–5x less per completion in a batch** (and 2.3x less
alone) than the CALLBACK path, the sequential round trip is ~5x faster, and 10k in flight ~2x. The note's
projection of 1–3 µs was optimistic: about 3 µs per completion is the engine entry (building the JS value and
settling the Promise, whose reactions are queued as jobs). Before making labels lazy and slots static the batch
figure was 5.5–8.2 µs.

## Tests

- `crates/svelters/tests/offload.rs` (11, plus the 6 E0 tests in the same crate): worker/blocking/local/`&mut self` local calls resolve, eager local
  settlement; abort rejects at once (same turn, `opsLive` drops), already-aborted calls never start, a queued
  cancelled job never runs, a running worker sees the cancel, a local future is dropped, the late result is
  dropped and its object freed with no handle minted; progress (latest value, once per checkpoint, before the
  completion in the same drain); panics (worker, local, worker panic resumed in a local future) become
  `svelters`/`panic`; supersede and `max_in_flight` (peak 2, FIFO, an aborted queued call never runs);
  disposing the owner rejects `disposed` and the object drops after the job; tokio runs and aborts (the
  future is dropped); 500 completions coalesce wakes (wakes ≤ checkpoints + 1); the debug long-poll warning
  (once per member); hot reload drops the old runtime's result (generation tag) and frees it; typings/stubs/JSON.
- `examples/exports/tests/exports.rs`: `worker_job_keeps_the_ui_responsive` (above; also the local `async fn`
  `wordCount`), `committed_typings_are_current` (regenerated `.svelters/app`), `offload_settle_cost` (ignored
  benchmark).
- `examples/exports/app`: `pnpm test` (vite build + svelte-check against the dumped typings): 0 errors.

Local gates and CI: see "Completion record" below.

## Completion record

Code head **734f6ddf** on `ci/e1` (based on master 746b2500). This record is a docs-only commit on top.

Local gates, all re-run after the last code edit (734f6ddf), `SVELTERS_SLOT_JOBS=4`:

- `cargo check --workspace --all-targets`: clean (pre-existing warnings only).
- `cargo nextest run --workspace --test-threads 4` (headless, under the shared nextest lock): **2402/2402
  passed**, 9 skipped.
- `pnpm -r build`, `pnpm test` (155 passed, 1 skipped, incl. the example's svelte-check: 0 errors),
  `python3 tools/docs/build_ir.py` + `pnpm docs:test` (17/17): all exit 0.
- `crates/svelters` offload tests also pass pinned to one and two cores (`taskset -c 0`, `-c 0,1`), and in
  `cargo test`'s shared-process mode.

CI on 734f6ddf, all success: js 39641602270, linux-gtk 39641602246 (gtk 2198/2198, gtk-4-14 1928/1928 with 2
retried flakes outside E1's code: `counter::integration bind_observe_gtk` and a SEGV in
`generated::verify::observe_gtk_shortcutsgroup`, both passed on try 2; portable 48/48), linux-qt 39641602259,
macos-appkit 39641602299, windows-winui 39641602334.

An earlier WIP head (0b3d3497) failed linux-gtk: two offload tests aborted/disposed calls before their jobs
started (correctly skipped, so no "saw cancel"), and on a small runner the gated jobs filled rayon's per-core
pool. Fixed by gating the abort/dispose on the job's start and giving the test process a 32-thread worker pool
(`set_worker_pool`).

## Open items

- E2–E5 (buffers, stores/services/events/streams, widgets, compiler types).
- Foreign traits from workers (Q4): out of E1, still open.
- Settle cost: ~3 µs per completion is the QuickJS entry; a direct `JS_NewPromiseCapability` resolve (no prelude
  loop) might halve it. Not needed for the acceptance.
- GTK `wake` runs at idle priority: a busy default-priority source starves checkpoints (seen with a 1 ms probe
  timer). A backend question for FU1/G1, not changed here.
- Node TestHost: offloaded exports, like E0's, are QuickJS-host only.

## Conflict risks

- `crates/svelters-host/src/core.rs` (one `mod`, one field, one line in `checkpoint`), `core/apps.rs` (two
  routing branches, `app_check_live`, `app_destroy`), `lib.rs` (`Host::drop`, two accessors), `engine.rs`
  (`settle_ops` with a default), `convert.rs` (`JsPromises` gains two methods), `quickjs.rs` (`settle_ops`,
  `cancelOp`), `quickjs/conv.rs`, `prelude.js` (ops map), `reload.rs` (generation calls).
- G1 (svelters-egui) touches the host's start/frame API and may implement `JsEngine`-adjacent pieces; the
  drain is in `core::checkpoint` only (not `frame`).
- `packages/types/src/core.d.ts` (`WorkOptions`), `examples/exports` (app, typings, tests),
  `crates/svelters-host/Cargo.toml`, `Cargo.lock`, the probe's `Cargo.lock`.
