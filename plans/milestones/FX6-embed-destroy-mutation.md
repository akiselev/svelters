# FX6: `state_unsafe_mutation` when an embed root is destroyed after an interrupt

Lane FX6, started 2026-10-22 on master 943b4c67. A bug fix reported by macbook-gui from cadatomic's
MAC1b check on macOS (cadatomic 4ed3c99, svelters snapshot befca905).

## The report

A Svelte panel in an egui embed died with Svelte's `state_unsafe_mutation`. The trigger was to open
a Svelte panel, then start a tool. The stack:

```
at state_unsafe_mutation (svelte/internal/client/errors.js:470)
at set (reactivity/sources.js:159) / update (sources.js:320)
at packages/native-runtime/src/core/svelte.js:174   <- subscriber(): the cleanup microtask's `$.update(version)`
at run_micro_tasks / flush_tasks / flushSync (batch.js:1049)
at packages/native-runtime/src/embed.js:294   <- root destroy: within(r, () => { unmount(r.exports); flushSync(); })
at within (embed.js:157)
```

## Root cause

`set` throws `state_unsafe_mutation` only while a derived or block effect is Svelte's
`active_reaction`. Every host call into a root (`root_call`) is a top-level entry, because the host's
JS guard refuses nested ones. So no reaction can legitimately be active when `unmount` drains its
microtasks. One was active anyway, left over from an earlier entry.

The embed's turn budget (50 ms per entry, `Budgets::interrupt`) stops a script with QuickJS's
interrupt. That error is uncatchable, and QuickJS's unwinder (`JS_CallInternal`, `exception:`)
skips catch handlers and `finally` blocks for it. So Svelte's module state stays as it was in the
middle of the update. The interrupted reaction is still `active_reaction`, along with the effect,
the component context, the batch being flushed and `is_processing`. The branches that the batch's
traversal had not reached yet are left without `CLEAN`. The embed then faults the root
(`FaultCause::Interrupt`) and disposes it: `__svelters_embed.unmount` → `flushSync` → the
subscriber's last-unsubscribe microtask → `$.update(version)`, inside the stale reaction.

Starting a tool re-renders the task panel (`{#key task.session}` rebuilds every row). In a debug
build on the Intel MacBook, that can take longer than 50 ms. The panel is then torn down (the fault)
and re-created (the restart).

Stock Svelte's `createSubscriber` does the same `increment(version)` from `queue_micro_task`. In a
browser nothing aborts a script halfway, so the problem never arises there.

A Node reproduction confirms it: V8's termination (`vm.runInThisContext(…, { timeout })`) also skips
`finally`. A derived that never returns, aborted inside Svelte's microtask flush, makes the next
`api.unmount` throw `https://svelte.dev/e/state_unsafe_mutation`.

## Fix

The host tells the runtime when it aborted an entry, and the runtime puts its state back at rest.
No component code runs.

- **Host** (`crates/svelters-host/src/quickjs.rs`): the interrupt handler sets `aborted` whenever it
  stops a script (budget or external flag). `disarm`, the common exit of `call_guarded` and
  `run_jobs`, then calls `globalThis.__svelters_recover()` if the runtime installed one. The budget
  is already disarmed at that point. Errors are reported like those of jobs.
- **Runtime** (`packages/native-runtime/src/index.js`): `__svelters_recover` resets the runtime's
  own per-entry state, which is restored in `finally` blocks. That covers the ownership `scope`,
  the build nesting (`depth`, `shows`) and the HMR init (`hmrCtx`, `hmrClaims`). Modules register
  their own state with `onRecover`. `embed.js` resets `current`, the root of the host call in
  progress.
- **Core** (`recover` in `src/core/svelte.js`):
  - Svelte's own `unset_context` clears the reaction, the effect and the component context, and
    deactivates the batch. `set_is_destroying_effect(false)` and `set_untracked_writes(null)` follow.
  - `clear()` (Svelte's test reset) drops the aborted batch. Then an empty `Batch.ensure().flush()`
    runs, and its `finally` resets `is_processing`, `current_batch`, `batch_values` and
    `old_values`.
  - The effect trees of the live roots are repaired to how a finished traversal leaves them. Every
    branch and root becomes `CLEAN`, and the effects still `DIRTY` or `MAYBE_DIRTY` are scheduled
    into a new batch. Inert (outroing) subtrees are left alone. Without the repair, `#resolve`
    treats a non-`CLEAN` ancestor as "already collected", and the effects below it never run again.
    In the Node test, another root that shared the aborted batch froze.
  - `root` (the core's `component_root`) keeps the set of live root effects for this.
- `svelte/internal/client/runtime` became a hidden module: the alias is in `package.json` and
  `vite.js` `HIDDEN_MODULES`. `tools/check-svelte-internals.mjs` asserts the new functions, the five
  flags, and that `Batch.flush` resets `is_processing` in its `finally`.

The other `within(...)` + `flushSync` call sites in `embed.js` (`mount`, `props`, `info`) have the
same shape as `unmount`. They are top-level host calls, so with the context recovered none of them
runs inside a reaction. The dispatch hook's `within` does not flush.

## Not fixed: `is_flushing_sync` after an abort inside `flushSync`

`is_flushing_sync` has no writer outside `flushSync`, whose `finally` restores the value it found on
entry. An abort inside a `flushSync` therefore leaves the flag set for the life of the runtime. In
the embed, `mount`, `props`, `info` and `unmount` all flush with `flushSync`. While the flag is set:
- `queue_micro_task` no longer schedules a microtask for an empty queue;
- `Batch.ensure` no longer queues a flush.

So writes outside any flush (handlers, timers) render only at the next `flushSync`, which is the
next host call into a root. `recover` queues one `flushSync` for the work it rescheduled, but it
cannot restore the automatic flush.

Possible fixes, all outside this lane:
- a build-time patch of `batch.js` that exports a reset;
- an upstream Svelte change;
- an embed that lets the host's job drain do the flush (Svelte's microtask) instead of `flushSync`.
  This would change the error path of `props` and `info`.

The second Node test (an abort inside `api.props`) covers what does work in that state: the
unmount, and host calls into the other roots.

## Tests

- `packages/native-runtime/test/embed.test.mjs`, two tests at the end of the file, which abort with
  `vm`'s timeout and then call `__svelters_recover` as the host does:
  - a flush aborted in a derived: the slow root unmounts without an error and its subscription
    stops. The other root receives the aborted batch's change in a later microtask, and its later
    writes still flush, both automatically and through `props`;
  - a host call aborted inside its `flushSync`: the slow root unmounts and the other root updates
    through `props`.
- `crates/svelters-host/tests/host.rs` `an_interrupted_entry_is_followed_by_one_recover_call`
  (`tests/js/runaway_recover.js`): the interrupted handler's `finally` did not run, and the host
  called `__svelters_recover` exactly once.

Run locally:
- `pnpm --filter @svelters/native-runtime test`: the internals check, 137 tests and svelte-check;
- `cargo check -p svelters-host --all-targets`;
- `cargo nextest run -p svelters-host --test host -E 'test(/interrupt|runaway|recover/)'`, 4 tests.

Not run locally (CI runs them): the svelters-egui embed tests (they need the `embed-egui` fixture
bundle, built by the compiler) and the full workspace.
