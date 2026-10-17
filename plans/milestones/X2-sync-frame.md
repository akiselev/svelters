# X2 - sync signal handlers and the frame clock

Framework lane. Touches `svelters-host`, `svelters-abi`, the log/GTK/Qt backends (vtable parts only), `packages/renderer` event dispatch and `packages/testhost`. Lane S1 owns `svl_value` (ABI v3), X1 owns borrowed handles and GTK GI invocation, A1 owns annotations: none of those are touched here. Notes: `docs/notes/x2-sync-frame.md`.

## 1. Sync handlers (`call_sync`)

A native signal whose handler result matters (GTK `close-request`, `key-pressed`, ...; Qt close and key events) makes the backend call `host->call_sync(h, signal, args) -> value` from inside the toolkit's emission. The host runs the JS listeners immediately and returns a result.

- **Which signals.** The backend decides, from generated metadata: a GTK signal is sync when `handler_result != none` (`bool_stop`/`value`) or `sync_required`. Qt signals are void; Qt sync points are synthetic signals (`synthetic: close_request | key_press`) raised by an event filter. Only listened signals are connected, so an unlistened sync signal keeps the toolkit default.
- **Result.** From the event object, not the handler's return value: `e.result = v` wins; else `e.preventDefault()` or `e.stopPropagation()` gives `true`; else `null` (= toolkit default). `call_sync` returns the value; the backend maps it to the signal's return type (`bool_stop`: truthy means stop/handled).
- **Rules inside the call.** The call holds the host's JS guard. A checkpoint requested meanwhile is deferred (as for any JS entry). The turn budget (time) applies. No jobs are drained inside the call.
- **Busy.** If JS is already running when the toolkit emits a sync signal (a host op such as `close()` emitting synchronously, or a handler that triggers another sync signal) the host cannot re-enter. It queues the event for ordinary async delivery and returns `-2`. The backend then uses the signal's deferred result: `false` (do not stop), except `close-request` which vetoes, as in ABI v2. The async delivery gives the handler the event object with `deferred = true` (`preventDefault` is a no-op).
- **Ops from a handler.** Host ops the handler issues directly reach the backend immediately (there is no batching). Effects and microtasks the handler schedules (Svelte flush) run at the next checkpoint: the host calls `wake` when `call_sync` leaves jobs pending.
- **Errors.** A throwing handler returns `null` (the default) and reports the error through the host's error list.
- Generalises the close-request veto: a listener now decides per event (`e.preventDefault()` vetoes; otherwise the window closes natively).

## 2. Listen phases

Only if cheap: GTK `connect_after` through an attachment, and `...capture` through controller propagation phase. Decision recorded in the notes.

## 3. Frame clock

`requestAnimationFrame`/`cancelAnimationFrame` run on the toolkit's frame clock. ABI v2.1 additive (tail members): backend `request_frame`, host `frame`. GTK: `add_tick_callback` on a mapped toplevel (timeout fallback). Qt: `QWindow::requestUpdate` filtered on `UpdateRequest` (timer fallback). Log backend: a virtual frame delivered on the next loop turn. Scripted fixtures keep the virtual 16 ms timer.

## 4. Tests

close-request veto via `call_sync` (GTK, Qt); key handler marks handled (GTK controller `key-pressed` return, Qt `QKeyEvent` consumed by the filter); throwing handler; re-entrancy; rAF + cancel on log/GTK/Qt without a host thread; op-log parity fixture `sync-handlers` (Node testhost and the Rust log backend).

## 5. Gaps

`metadata/A1-GAPS.md` from lane A1, when available.
