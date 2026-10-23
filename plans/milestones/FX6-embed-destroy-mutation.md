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

## FX6b: the embed's calls flush in the host's job drain

Lane FX6b, started 2026-10-22 on master a63dfa78. It closes "Not fixed" above with FX6's third option,
as the coordinator decided (2026-10-22): the embed's entry points stop calling `flushSync`, and the
host's job drain does the flush. Svelte's `batch.js` is not patched.

### What changed

- **Runtime** (`packages/native-runtime/src/embed.js`): `mount`, `props`, `info` and `unmount` no
  longer call `flushSync`. What a call changed is scheduled by Svelte as usual (`Batch.ensure` queues a
  microtask flush), and the host's job drain right after the call runs it. The turn budget can now stop
  that flush only inside a job, never inside a `flushSync`, so `is_flushing_sync` stays clear. Only a
  component's own `flushSync` can still leave it set (recover's comment says so).
- **Host, error delivery** (`crates/svelters-host/src/core.rs` `root_call`, `DrainErrors`): every embed
  call was already one entry into JS followed by a job drain, so when `mount_root`, `root_props`,
  `root_info` or `unmount_root` returns, the root is rendered, as before. The drain's errors are now the
  call's own. When the call itself succeeded, the drain's first error other than an unhandled rejection
  is returned as the call's error, and the others are reported. Before, these errors came out of the
  call's `flushSync` synchronously: an exception the flush threw, the budget stopping it, OOM. Unhandled
  rejections are reported as before (`Host::take_errors`). Render and effect errors that the root's
  boundary handled still arrive as `RootMessage::Fault`. `set_locale`'s drain keeps reporting its errors
  (`DrainErrors::Report`). The embed's public Rust API is unchanged. `Embed`'s `on_call_error` and
  `do_mount` attribute these errors to the root and call they came from, as they did before.
- `mount_root`: an error of the drain after a successful first render (an `$effect`/`onMount` that
  threw outside a boundary, the budget stopping the first flush) is returned, but the runtime still
  has the root then. The caller unmounts it, and `Embed`'s fault disposes it as for any fault. The
  docs of `Host::mount_root` and `Host::root_props` say this.
- Budgets: the call and its drain each get the budget armed for the entry (as an event and its
  drain do). A mount whose render and first effects together took more than one budget passes now if
  each half fits.
- **Host, drain** (`crates/svelters-host/src/quickjs.rs` `run_jobs`): a drain stops at the first
  interrupted job, leaves the rest pending (`JobStatus::pending`, which wakes the host) and recovers in
  `disarm` before any of them runs. The new egui test found this. The aborted flush had queued
  `createSubscriber`'s cleanup microtask, which ran next in the same drain, before the recover, inside
  the stale derived. It failed with FX6's `state_unsafe_mutation`. With the deadline passed, every later
  job in that drain would also have been stopped, and its work lost.
- `root()` and per-root `requestAnimationFrame` during the flush: effects find their root through
  the active effect (`rootOfActive`), as for flushes after handlers and timers. A plain microtask
  queued during the flush, which has no effect, now gets the global animation-frame queue rather than
  the root's.
- Callers that read state right after `mount`, `props` or `info`: on the host side, nothing changes.
  `Embed`'s `sync_info` and `apply_commands` and the egui runners call through `root_call`, which has
  drained before it returns. The only JS callers were the Node tests, which now drain after each call
  (`call()` in `embed.test.mjs`, the same shape as the host).

### Tests

- `packages/native-runtime/test/embed.test.mjs`: the existing tests drain after each call (and assert
  that `props` itself renders nothing). New test `FX6b: a host call aborted in its flush…`: an abort
  inside the drain's flush after `props` leaves `is_flushing_sync` clear, the root unmounts, and a
  timer's write into another root renders without another host call. On the old `embed.js` it fails.
- `crates/svelters-host/tests/host.rs` `a_root_calls_drain_runs_before_it_returns_and_its_errors_are_the_calls`
  (`tests/js/embed_drain.js`, a hand-written embed). It checks that:
  - the drain has run when `root_props` returns;
  - an exception and an interrupt from the drain are returned, not reported;
  - an unhandled rejection is reported;
  - the drain stops at the interrupted job, and the job queued by it runs only after the recover.
- `crates/svelters-egui/tests/embed.rs` `an_interrupted_flush_disposes_a_store_backed_root_and_timer_changes_still_render`
  (the end-to-end test FX6 did not run; fixtures `Stalled.svelte` and `Later.svelte` in `fixtures/embed-egui`):
  - a store-backed root (`createSubscriber`) whose derived spins after a props change is faulted with
    `Interrupt` and disposed with no other error;
  - a timer's change in another root then renders with no host call into that root.

Run locally:
- `pnpm --filter @svelters/native-runtime test`: internals check, 137 tests, svelte-check;
- `cargo nextest run -p svelters-host --test host -E 'test(/interrupt|runaway|recover|drain/)'`: 5 tests;
- `tools/dev/headless cargo nextest run -p svelters-egui --test embed --test queries --test viewports
  --test widgets_panel`: 19 tests, on the rebuilt `embed-egui` bundle.

Not run locally (CI runs them): the rest of the workspace.
