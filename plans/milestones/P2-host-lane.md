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

## Completion record

Branch `worktree-agent-ae7a2d33fa6de094d` (merged with P1's master). Details: `docs/notes/p2-host-lane.md`.

- `cargo nextest run --workspace --test-threads 4` (builds with `CARGO_BUILD_JOBS=4`): 21 tests, 21 passed. `cargo clippy --workspace --all-targets`: no warnings.
- The four P1 fixtures (`counter`, `keyed-list`, `if-toggle`, `timers`) built with `pnpm install && pnpm -r build` run in QuickJS against the log backend, and their op logs equal `expected.oplog` byte for byte (`tests/fixtures.rs`; skipped when `dist/` is absent, `SVELTERS_REQUIRE_FIXTURES=1` forbids skips).
- Hand-written bundles in `crates/svelters-host/tests/js/` cover: ops and events, a timer/promise/async chain on both the virtual clock and the wall clock (wake count 4 to 10 for four timer deadlines, so no polling), runaway loops in load and in a handler (interrupted, host keeps working), an external interrupt flag, the memory limit, the stack limit, relative imports, escaping/bare imports rejected, console capture, error and unhandled-rejection reporting with stacks, and a re-entrant `checkpoint` from a nested loop.
- ABI: raw types and header unchanged. Added safe wrappers and the `Backend` trait helper with tests (`crates/svelters-abi/tests/safe.rs`).
