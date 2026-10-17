# X2: sync handlers and frame clock (working notes)

Started 2026-10-17 on master 8997ea0.

## Findings before coding

- `call_sync` already exists in `svl_host` and in the safe wrappers; the host stub returned an error. The GTK backend special-cased `close-request` (always veto, post event); Qt did the same with a synthetic signal and `CloseFilter`.
- The engine is held in a `RefCell` for the duration of a checkpoint, and rquickjs `Context::with` is not re-entrant, so JS cannot be re-entered from inside a host op. That is why a `call_sync` that arrives while JS runs is deferred rather than nested.
- GTK tables carry `handler_result` (33 bool_stop signals) and `sync_required` (all false today). Qt metadata sets both to none for every real signal.
- No controller kind is attachable in GTK today (no slot, and controllers are not widgets).
