# M2: ABI v2 and the dev loop

Status: started 2026-10-17. Depends on P5 (done).

M2 has two lanes. They run in parallel with S1 (ABI v3 value model), F1 (fork bindings) and D1 (docs generator).

## Ordering with S1 (ABI v3)

S1 is rewriting `svl_value` in the same files (`svelters_backend.h`, `svelters-abi/src/{lib,safe}.rs`). The agreed order is **v2 lands first, then v3 bumps the version**:

1. M2a's first commit is the v2 *interface delta only*: the new vtable/host entries, the enums and `SVL_ABI_VERSION 2u`. Every backend gets the smallest implementation that compiles and passes (`schedule_wake` may still feed the old timer thread at that point). It lands on `master` as early as possible.
2. The coordinator tells S1 to merge `master`. S1 owns the conflict resolution in `svl_value`, and bumps to `3u` when it lands.
3. M2a's remaining work (real native timers, lifetime semantics) touches the backends and host core, not `svl_value`.

## M2a: ABI v2 (host + GTK + Qt + log backend)

Spec: `plans/architecture/backend-abi.md#abi-v2-approved-2026-10-17-lands-at-the-start-of-m2` and the "timers belong to the toolkit" section.

- **Timers.** Add `schedule_wake(self, ms)`. The host keeps its timer heap and arms only the earliest deadline after each checkpoint.
  - GTK: `glib::timeout_add_local_once`.
  - Qt: a single-shot `QTimer`, Precise under ~20 ms and Coarse otherwise.
  - Log backend: a virtual clock, so the timers fixture is deterministic and instant.
  - Delete the host timer thread. `wake` stays for thread-safe requests only.
- **Lifetime.**
  - Add `handle_info` and the lifetime classes (OWNED, TOPLEVEL, BORROWED, VALUE).
  - `on_destroyed(h, reason)` takes `svl_gone` reasons and fires at most once, after the backend drops the handle.
  - Native death cascades to every descendant.
  - Handles are never reused and stay below 2^53. A bounded dead-handles ring gives "handle #n was destroyed (reason)" errors.
- **Destroy.** `destroy` is idempotent and acts per lifetime class. No synchronous deletion inside a signal emission: Qt uses `deleteLater`, GTK an idle-deferred unref.
- **Window close and quit.** Window close follows `onRequestClose`: if JS listens, the backend vetoes the close and posts the event; otherwise it destroys the window and reports `SVL_GONE_CLOSED`. The host decides quit, with Qt `setQuitOnLastWindowClosed(false)`. This fixes the P5 gap where a native Qt close didn't report `on_destroyed`.
- **JS side.**
  - The renderer learns about native deaths through the event queue, so a dead node is detached without further ops.
  - A new fixture, `native-close`, closes a child window natively and checks the tree and op log.
- **Op log.** Add `gone #h <reason>` lines and update the log backend and the oplog replay tests.
- **Generated code.** Qt changes go through the generator/templates (`tools/metadata/qt/`), never hand edits of generated output. GTK keeps its hand-written vtable.

Acceptance:
- Timers fire through the toolkit on both backends with no host thread (`ps -T` shows none).
- The timers fixture is deterministic on the log backend.
- New integration tests cover native window close (GTK + Qt), destroy idempotence, cascade, and dead-handle errors.
- All gates are green.

## M2b: dev loop

- **Watch build.** `packages/build` gets a watch mode (Vite `build --watch`, dev source maps).
- **Hot reload, no Cargo.** The host gets `--watch <bundle>`. On change it tears down the app's toplevels, drops the JS runtime/context, loads the new bundle and runs again, all inside the same process and toolkit loop.
  - The file watcher may be a thread that only calls the thread-safe `wake`, or the toolkit's own file monitor.
  - Full reload is enough for M2. State-preserving HMR is later.
- **Errors.** A syntax error or a throw during load keeps the process alive, prints the error and waits for the next change.
- **Source maps.** QuickJS stacks, uncaught errors, rejected promises and `console.error` stacks are mapped to `.svelte`/`.ts` file:line:col through the bundle's `.map`. Use a Rust source-map crate (e.g. `sourcemap`); no JS-side mapping.
- **Types.** Wire `svelte-check` against `packages/renderer/types` into `pnpm test`, or a separate `pnpm check` if it is slow.

Acceptance:
- An integration test edits a fixture source, rebuilds and observes the reloaded UI in the same host process (op log or screenshot).
- A test throws from a `.svelte` file and asserts the mapped `File.svelte:line` in the error.
- All gates are green.

M2b touches `crates/svelters-host` (new modules plus load/reload plumbing), `packages/build` and `examples/counter`. If both lanes edit `core.rs`, M2b rebases onto M2a after M2a lands.

## Gates

- `pnpm test`
- `CARGO_BUILD_JOBS=4 xvfb-run -a cargo nextest run --workspace --test-threads 4`
