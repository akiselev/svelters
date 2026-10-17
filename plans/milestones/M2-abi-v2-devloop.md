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

## M2b dev loop: how to run it

```text
pnpm install && pnpm build                      # once
pnpm watch fixtures/counter                     # terminal 1: Vite build --watch, dev source maps
cargo run -p counter -- --backend gtk --fixture counter --watch      # terminal 2 (or --backend qt)
```

Edit `fixtures/counter/src/App.svelte` and save: the build rewrites `dist/app.js` (+ `.map`), the host's inotify watcher debounces 120 ms, and the running process loads the new bundle into a fresh QuickJS runtime and swaps windows. No Cargo, same process, same toolkit loop. A bundle that throws or does not parse keeps the previous version on screen, prints the source-mapped error (`src/App.svelte:3:9`) and waits for the next change. `pnpm check` (also part of `pnpm test`) runs svelte-check over `packages/renderer/check` against `packages/renderer/types`. In an app of your own, `svelters-build [--watch] [dir]` is the CLI behind `pnpm watch`; any `vite.config.js` that exports `svelteConfig(import.meta.dirname)` works.

## M2b completion record

Done 2026-10-17 on branch `worktree-agent-a407a5f96f9c75540`.

- `packages/build/bin/svelters-build.mjs` (+ root `pnpm watch`, per-fixture `watch` script).
- Host: `reload.rs`, `sourcemap.rs`, `Host::{watch, on_reload, request_reload, toplevels}`; counter `--watch [--reload-exit n]`. Design in `docs/notes/m2b-devloop.md`.
- Tests: `svelters-host/tests/reload.rs` (4: source-mapped console.error / unhandled rejection / uncaught timer error / load error with `src/App.svelte:<line>`; watch-build edit reloads in one host process; mount failure and unparsable bundle keep the process and recover) and `counter/tests/watch.rs` (GTK and Qt: edit, rebuild, reloaded widget tree in the same process). `pnpm check` for the types.
- Edit to new UI is about 1.5-2 s on this machine, of which Vite's rebuild is about 1.3 s; host debounce 0.12 s and load plus teardown 0.2-0.3 s (debug build).
- Open: after M2a lands host-decided quit, drop the GTK `keep_alive` hold in `counter` and the load-new-then-destroy-old ordering is still the right one (it keeps the previous version on a failed load). The old runtime's source maps are replaced by the new bundle's map after a failed reload (stack positions of the still-running old runtime may map wrongly until the next good reload). State-preserving HMR is not attempted.

## M2a completion record

Done 2026-10-17 on branch `worktree-agent-a812b89b266e43c3f` (delta commit `316effd`, merged to master as `4a76e40`). Spec and deviations: `plans/architecture/backend-abi.md#as-implemented-m2a`. Notes: `docs/notes/m2a-abi-v2.md`.

- **Timers.** `schedule_wake` on GTK (glib `timeout_add_local_once`), Qt (single-shot `QTimer`, Precise under 20 ms else Coarse) and the log backend. The host timer thread and `Sched` are gone; the host arms the earliest deadline after each checkpoint and cancels on drop. Scripted fixtures keep the host's virtual clock (deterministic, instant); unscripted log runs honour `schedule_wake` in wall time. Every `counter` report lists the process's threads and the integration tests assert there is no timer thread.
- **Lifetime.** `handle_info`, never-reused handles below 2^53, a 4096-entry dead-handle ring with `handle #n was destroyed (why)` errors, idempotent destroy, dead-handle tolerance for teardown-shaped ops, `on_destroyed(h, reason)` at most once, cascading native death (GTK from slot records, Qt eagerly in the close filter), deferred deletion (Qt `deleteLater`, GTK idle).
- **Window close and quit.** `onclose` (`close-request`): vetoed and posted while listened (GTK `Stop`; Qt synthetic signal plus event filter), else destroyed and reported `closed`. Host-decided quit via `HostConfig::quit_on_last_window`; GTK `run` holds the app, Qt `setQuitOnLastWindowClosed(false)`. `counter`'s GTK `keep_alive` hold is removed.
- **JS side.** `@svelters/renderer` exports `gone` and installs `__svelters_gone`; the Node TestHost and the log backend share the `{"gone": [h, reason]}` script step and the `gone #h <reason>` log line. New fixture `native-close`.
- **Reload.** Reload teardown works with idempotent destroy and the cascade (the watch tests pass on both toolkits). `counter --watch` now reports `native_toplevels` after a reload and `watch_reload_gtk/qt` assert it is 1, which is the Qt leak check.
- **Tests.** 74 nextest tests pass (was 57 before M2a, plus M2b's), `pnpm test` is green. New: `native-close` fixture (log backend, Node parity, GTK replay with real window close), host tests for the cascade, dead-handle errors, the ring and no timer thread, `svelters-gtk/tests/lifetime.rs` (3), three Qt runtime tests in the generated verify suite, `native_close_gtk/qt`.
- **Open.** No backend mints borrowed or value handles yet. `handle_info.flags` is unused. The Qt cascade order is by handle number, not pre-order. A reload whose new bundle creates no window quits the app. The GTK watch test saw one Vulkan device-lost abort when the whole suite ran at once; it passed on rerun and in isolation.
