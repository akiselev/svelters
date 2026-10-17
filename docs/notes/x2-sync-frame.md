# X2: sync handlers and frame clock (working notes)

Lane X2, started 2026-10-17 on master 8997ea0. Plan: `plans/milestones/X2-sync-frame.md`. ABI text: `plans/architecture/backend-abi.md`, "ABI v2.1 additive".

## Findings before coding

- `call_sync` already existed in `svl_host` and the safe wrappers; the host answered with an error. GTK special-cased `close-request` (always veto, post the event); Qt did the same with its synthetic `close-request` and `CloseFilter`.
- The engine sits in a `RefCell` for the whole checkpoint, and rquickjs `Context::with` is not re-entrant, so JS cannot be re-entered from inside a host op. That is why a `call_sync` that arrives while JS runs is answered "busy" and the event is queued, instead of nesting.
- GTK tables carry `handler_result` (57 bool_stop signals, counting Adw) and `sync_required` (all false today). Qt metadata has `none`/`false` for every real signal; Qt's sync points are synthetic (event filters).
- No controller kind was attachable in GTK: controllers are not widgets and no slot exists.

## Sync handler semantics (as implemented)

1. **Who is sync.** The backend decides from generated metadata. GTK: `handler_result != none || sync_required` in the `SIGNALS` table (`deliver` in `svelters-gtk/src/backend.rs`). Qt: synthetic signals `close_request` and `key_press` (`synthetic:` in the element override; the generator maps them to `CloseFilter`/`KeyFilter`). An unlistened signal is never connected, so it keeps the toolkit default.
2. **Call.** The backend calls `host.call_sync(h, signal, args, out)` inside the emission. The host looks up the listened signal name, enters JS under the `JsGuard`, calls `globalThis.__svelters_dispatch_sync(h, signal, args)` and returns its result. The turn budget (time) applies.
3. **Result.** From the event object the renderer builds (`NativeEvent`): `e.result = v` wins; else `e.preventDefault()` or `e.stopPropagation()` gives `true`; else `null`. Listeners get `(event, ...args)`. The handler's return value is ignored (Svelte ignores it); a handler that returns a Promise draws a warning. Backends read the result as: `null` = toolkit default, bool = stop flag. GTK also maps ints and strings for `i32`/`string` results (no such signal is annotated `value` yet).
4. **Errors.** A throwing listener propagates out of `dispatchSync`; the host reports it (source-mapped, in `take_errors`) and answers `null`, so the toolkit default applies. A result that is not a primitive/array is reported the same way. A failing `call_sync` (no listener, host error) returns `-1` and the backend uses the default.
5. **Busy.** If JS is already running (`busy > 0`, or the engine is borrowed) the host cannot re-enter. It queues the event with `deferred = true`, wakes the backend, and returns `-2`. The backend uses the deferred default: not-stop, except `close-request`, which vetoes (the ABI v2 behaviour, so the late handler can still decide). The late event reaches the renderer as `dispatch(h, signal, args, true)`; listeners get the same `(event, ...args)` shape with `e.deferred = true` and flags ignored. This covers both a host op that emits a sync signal synchronously and a sync handler that triggers another sync signal.
6. **No nested checkpoints.** The sync call holds the guard, so a `checkpoint` from a nested loop is deferred and becomes a `wake` when the guard drops (the existing rule).
7. **Ops from a handler.** Direct host ops (`setProp`, `insert`, ...) reach the backend immediately (there is no batching), so they are visible when the handler returns. Anything the handler leaves for microtasks/effects (Svelte's flush) runs at the next checkpoint: after the call the host checks `run_jobs(0).pending` and wakes the backend. Decision: no job draining inside the call. Draining would run a Svelte flush (inserts, destroys) in the middle of a toolkit emission, which the backends only tolerate because they defer deletes; waiting for the next loop turn costs one idle callback.
8. **Close request.** The old always-veto is gone. A listener decides per event; no listener, or an undecided handler, closes the window natively and reports `closed`. The renderer's `onclose` still maps to `close-request`.
9. **Re-entrancy rule of thumb for backends.** Treat `-2` as "the host will deliver this later": do not post the event yourself, and use the deferred default.

### What A1 should annotate

- GTK (`metadata/overrides/gtk`, schema v0 signal fields): `handler_result: bool_stop` for every signal whose return value means "handled/stop" (the 57 already derived from GIR stay as they are), `handler_result: value` for typed results (`query-tooltip`-style `bool` is fine; `drag-*` `GdkDragAction`, `create-notebook` objects, `i32`/`string` ones need a conversion first), and `sync_required: true` where an async fallback is wrong: `close-request`, `key-pressed`/`key-released`, `drop`, `accept`, `change-value`, `activate-link`, `state-set`. `sync_required` currently only makes a `void` signal go through `call_sync` too; it does not change the busy default.
- GTK controllers: kinds like `EventControllerKey` work today when created by name (`h.create('EventControllerKey')`) and inserted into a widget (any slot, `before` ignored); they need element tags and a typed `controllers` pseudo-slot in the annotations.
- Qt: `synthetic: key_press` is declared for `entry` only (override `QLineEdit.yaml`, `key-press`, args `[key, text, modifiers]`). A1 should add it to the other input widgets, add a `key_release`/`event` family if wanted (the generator accepts only `close_request` and `key_press`), and a `deferred_result` if a synthetic signal ever needs a veto-on-busy default other than close.

## Frame clock

- Host: `requestAnimationFrame` batches callbacks in the prelude; the first request calls `native.frame_request()`, which calls `backend.request_frame()` unless one is pending. `host.frame` runs the batch with a `performance.now()` timestamp, then the jobs they queued; requests made inside a frame ask the backend again, so frame N+1 comes from the clock. A frame that arrives while JS runs is requested again. Scripted hosts (fixtures) get `false` from `frame_request` and use a virtual 16 ms timer, as before.
- GTK: `GdkFrameClock` of the lowest-handle mapped toplevel; one `update` handler per clock, `request_phase(UPDATE)` per request. No mapped toplevel: a 16 ms timer. A mapped window whose clock stalls (hidden, minimised): a 250 ms watchdog timer delivers the frame anyway. Measured under Xvfb: frames arrive by the clock (`GtkBackend::frames_by_clock`).
- Qt: `QWindow::requestUpdate()` on the platform window of the lowest-handle visible toplevel; an event filter on that `QWindow` turns `UpdateRequest` into the callback (it never consumes the event). Fallback and watchdog as for GTK. Qt may deliver an unrelated `UpdateRequest` first; the frame then comes early, still on a repaint boundary.
- Log backend: a virtual frame on the next loop turn (no wall wait); counters `frames_requested`/`frames_delivered`.
- After `run` returns, GTK and Qt drop their host pointer, so a late timer cannot call into a freed vtable.
- Timestamps are the host's `performance.now()` at frame start, not the toolkit's frame time. GDK's frame time is a monotonic microsecond clock that could be mapped with an offset; not needed yet.

## Listen phases (goal 2): skipped

`connect_after` and `…capture` need the listen-time flags of the feature survey (§3a): `listen(h, signal, on, flags)`, separate native connections per flags, and `dispatch(..., {phase, after})`. That changes the `listen` signature (or adds a `listen_ex` tail member), the host op, the renderer's listener bookkeeping and per-widget controller creation in GTK. Not cheap, and not additive in one place. Proposal for when it is done: a v2.2 tail member `listen_ex(self, h, signal, on, flags)` with `flags` bits `AFTER = 1`, `CAPTURE = 2`, `SYNC = 4`; the renderer's `addEventListener(target, type, handler, options)` already receives `capture` from Svelte for `on<name>capture`; GTK `after` maps to `connect_closure(..., after = true)` and `capture` to an `EventControllerKey`/`Legacy` controller with `PropagationPhase::Capture`; Qt `capture` maps to an event filter on the ancestor. What exists today is `e.stopPropagation()` on sync events, which ends GTK's emission (`true_handled` accumulator) and so stops later handlers and the class handler.

## Tests

- `crates/svelters-abi/tests/safe.rs`: sync codes and the frame callback through the safe wrappers.
- `crates/svelters-host/tests/sync_frame.rs` (log backend): results/throw/undefined/object/no-listener; ops in a handler and microtasks; wake after a live sync call; busy (sync inside a sync handler, plus a nested checkpoint); rAF on the backend clock with cancel, a throwing callback, chained frames, no new thread; scripted rAF.
- `fixtures/sync-handlers`: op-log parity between the Node TestHost and the log backend (`sync`/`result` lines).
- `packages/renderer/test`: `NativeEvent`, `dispatchSync`, deferred dispatch, Promise warning.
- GTK: `tests/sync.rs` (close veto/default/busy/failed; key controller handled/undecided/false/failed/busy; frame clock once per request; fallback timer) and `tests/host_sync.rs` (real host: key handled, throwing handler, nested sync, veto then native close, rAF with cancel and no host thread).
- Qt: the generated verification tests `runtime/close_handler_decides_through_call_sync`, `key_press_handler_consumes_the_key` (a real `QKeyEvent` into a `QLineEdit`; a consumed key leaves the text unchanged), `request_frame_delivers_once_per_request`; `examples/counter-qt/tests/host_sync.rs` is the end-to-end twin of the GTK one.

## Gaps

`metadata/A1-GAPS.md` did not exist on lane A1's branch (HEAD 7f96e25) or on master when this was written, so no A1 gap ids could be matched. Gaps this lane closed, under temporary ids:

- X2-G1 sync handler results / `call_sync` stub (GTK, Qt, host).
- X2-G2 close-request is a per-event decision instead of always-veto.
- X2-G3 key press can be marked handled (GTK controller, Qt entry).
- X2-G4 event controllers can be attached to a widget (insert/remove).
- X2-G5 `requestAnimationFrame` on the toolkit frame clock.

Open (not closed here): listen phases (above); `close`/other imperative methods on GTK `window` (only `present` exists, which is why the GTK nested-sync test raises the nested signal from a native hook); typed `value` results; Qt `key_release`/generic `event` synthetic signals; per-widget key handlers in the portable (cross-toolkit) element vocabulary: today GTK needs a controller child and Qt a listener on the entry.

## Surprises

- `GtkWindow::close-request` is RUN_LAST: emitting it by hand with no stopping handler runs the class closure and destroys the window, so a fixture replay cannot reproduce a "null" close result without killing the window. Toolkit replays skip `sync` fixtures for that reason.
- The worktree tool refuses some compound shell commands; edits were scripted from files.
- `GSK_RENDERER=cairo` is needed for GTK tests that close windows under Xvfb (the Vulkan path asserts); the counter integration tests already set it.

## Merge with ABI v3 (S1, 62e1ee1)

Merged master after v3. Conflicts were all tail-member adjacency: `frame` follows `release_callback` in `svl_host`, `request_frame` follows `cancel` in `svl_backend`. `dispatch_sync` decodes its args through the v3 `convert::decode` like `dispatch`; its result is a primitive (null/bool/number/string) read directly, since a handler result is not a typed value. The Qt filters zero-initialise `svl_value` (v3 added `flags`/`type`).
