# P2 - host lane

Owns `crates/svelters-abi`, `crates/svelters-host` and `crates/svelters-backend-log`.

## Work
- `svelters-abi`:
  - the C header from `backend-abi.md`;
  - matching `#[repr(C)]` Rust types;
  - safe Rust wrappers for calling a vtable;
  - a helper for implementing a backend in Rust (trait → vtable).
- `svelters-host`:
  - current `rquickjs` (QuickJS-NG), synchronous runtime pinned to the main thread;
  - load an ESM bundle file;
  - install a native `SveltersHost` object implementing the JS host ops over a `svl_backend`, with name→ID caching;
  - `console.*`, `queueMicrotask`, `setTimeout`/`setInterval`/`clearTimeout`, and an animation-frame stub, via a timer heap driven by checkpoints plus backend `wake`;
  - `post_event` queue + `checkpoint` (events → JS → bounded job drain → `wake` if jobs remain);
  - uncaught exceptions and unhandled rejections reported with JS stacks (source-map translation is M2);
  - memory/stack limits and an interrupt handler.
- `svelters-backend-log`: a Rust backend with a fake loop that records ops in the same op-log format as the P1 TestHost and can inject events.

## Acceptance
- `cargo nextest run` loads each P1 fixture bundle in QuickJS against the log backend. The op logs equal P1's Node snapshots byte for byte, scripted events included.
- A timer and a promise chain fixture complete without busy-polling. The log backend counts `wake` calls.
- A runaway-loop fixture is interrupted.
