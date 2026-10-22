# H1: component HMR on the Rust module loader

Design: `plans/research/HMR.md` §2.1, §3.2–3.5, §4 H1, §6; `plans/research/QUICKJS-RUNTIME.md` §3.4. Builds on Q1
(`svelters-modules`, the provider adapter in `quickjs.rs`). This file records H1 as built. `$state` is lost on a swap
(Svelte 5 semantics); H3 keeps it.

## As built

- **Compiler**: `CompileOptions.hmr` / `svelters-compile --hmr` (with `--dev`). The component is a plain `function C`,
  then `if (import.meta.hot) { C = $n.hmr(C, "<filename>"); import.meta.hot.acceptExports(["default"], (m) => m && C[$n.HMR].update(m.default)); }`
  and `export default C`. Output without `--hmr` is unchanged. `SourceConfig.hmr` (with `dev`) turns it on for a
  `SourceProvider`.
- **Runtime** (`packages/native-runtime/src/index.js`): `$n.hmr(fn, id)`, `$n.HMR` / `$n.FILENAME` (Svelte's symbols),
  `setHmrReporter`, `hmrStats`.
  - One region and one block effect per instance, reading the shared `current` source; "the first version's source
    drives every update" as upstream; the `instance` object is refilled, so `bind:this` stays valid.
  - **Rollback**: the new version is built in a new branch after the old one; if it throws, the partial branch and
    its natives are dropped, the old version stays, the error goes to the reporter (default `console.error`) and
    the id is marked failed until the next good swap. Upstream destroys first and cannot do this.
- **Host** (`svelters-host`): `hot.js` is the JS side of `import.meta.hot` (`accept`, `acceptExports`, `dispose`,
  `data`, `invalidate`; only self-accepting modules are boundaries). `accept(cb)` accepts every export; with
  `acceptExports(names, cb)` every other export must be unchanged (`===`, first version's exports read by a deferred
  `import()`), else the change falls back to a full reload (propagating to importers instead is a follow-up).
  - The QuickJS adapter defines `import.meta.hot` per module, strips and adds `?v=<n>` (`Versions`): a swapped module
    resolves to its newest version, so re-evaluated importers link to new code and untouched modules are shared.
  - `JsEngine::hot_update` walks `ModuleProvider::importers` from each changed module to the first accepting
    module, bumps versions, evaluates the boundaries, then commits (`update` runs the *previous* version's accept
    callbacks with the new namespace, as Vite does).
  - A compile or evaluation error restores the versions and keeps the running app (`ReloadReport.ok == false`,
    `hot == Some([])`).
- **Fallback to a full reload**: no accepting module on a path (`fallback = Some(..)`), `invalidate()`, a changed file
  that is not a module (manifest, spec), or the swap budget. `ReloadReport` gained `hot` (modules swapped) and
  `fallback`.
- **Swap budget**: `HostConfig.hmr_max_swaps` (default 200) and `hmr_memory_fraction` (default 0.5 of
  `memory_limit`) force a full reload; replaced module versions are not freed while a runtime lives.
- **`Host::apply_changes(paths)`**: the watcher's work without the thread, for editors and embedders; `load_entry_from`
  now remembers its provider.
- `svelters-modules`: `ModuleProvider::{hmr, modules_at, importers}` (defaults empty; `SourceProvider` implements them).

## Tests

- `crates/svelters-compiler/tests/h1_hmr.rs`: the wrapper's shape, unchanged output without `--hmr`.
- `packages/native-runtime/test/hmr.test.mjs`: parent handles untouched, live props, `bind:this` object, rollback then
  recovery, independent instances.
- `crates/svelters-host/tests/hmr.rs` (log backend, watcher on): child swap keeps the window and the parent's label
  (created once), runes-module edit re-runs its accepting importer, nothing accepts → fallback, broken edit keeps the
  old version and a fix applies, swap budget → full reload.
- `crates/svelters-egui/tests/hmr.rs` (kittest, three roots): editing `Panel` swaps the two Panel roots; the Pane root
  keeps its widgets, state and title; no root faulted or re-mounted.
- `experiments/native-compiler/conformance/run.mjs` passes `compileOptions.hmr` as `--hmr`.

## Not done / open

- Not run: the upstream hmr conformance samples (`hmr-removal`, `hmr-each-keyed-unshift`, `hmr-dynamic-component`; no
  upstream checkout on this machine; the runner also needs `import.meta.hot` for the wrapper to apply there), the
  op-log fixture `fixtures/hmr` and its GTK/Qt replay, the GTK/Qt `watch` integration tests, #18079's scenario.
- Dev timer attribution (HMR.md D10) lives in the host prelude: follow-up (coordinator: out of H1). Edit-to-UI time and memory per swap are not
  measured; the budget defaults are guesses (D-note: measure with a 50-component app).
- The full-reload fallback is not supported for an embedder's entries (`load_entry`); embed apps must accept their
  changes (documented on `Host::apply_changes`).
- Per-module source maps for swapped versions: maps are stored by versioned name when declared; the M2b open item (a
  failed reload replacing the running bundle's map) is not touched.
- Style updates (H2), `$state` keeping (H3), native UI state (H4) are later milestones.
