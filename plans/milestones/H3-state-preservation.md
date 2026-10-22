# H3: `$state` kept across HMR

Design: `plans/research/HMR.md` §2.4, §3.3 items 3–5, §3.4 "State registry", §4 H3a/H3b, §6 D2–D5, D7; SV20/SV36 in
`plans/research/CADATOMIC.md`. Builds on H1 (`$n.hmr`, `import.meta.hot`). This file records H3 as built.

## State rule (HMR.md recommendations, picked)

- On by default in dev (D2). Off for one component with `// @hmr:reset`, globally with
  `setHmrPreserveState(false)` (`@svelters/native-runtime`).
- **All or nothing per component** (D3) gated by a **signature** (D4): the ordered keyed declarations
  `(kind, name, normalized initializer source)`. Equal signature: the changed values carry over. Anything else
  (declaration added, removed, renamed, initializer edited, `@hmr:reset`): the whole component starts fresh.
- **SV36, only changed values**: a declaration whose value still equals what its initializer produced is not
  carried; the new version runs the new initializer (so a callee's change shows). Objects and proxies are always
  carried (the initial value cannot be compared cheaply).
- Restore replaces the initializer (it does not run), so there is no "restore after init" misbehaviour.
- Children of an edited parent are re-created with fresh state (D6 (a)); H3c (positional keys) is not built.

## As built

- **Compiler** (`--hmr`, `ScopeRewriter::with_options(.., hmr)`):
  - a top-level instance-script `let x = $state(init)` / `$state.raw(init)` that can change (a reassigned source
    or a proxy) becomes `$.state($n.hmr_keep("x", () => init, () => $.get(x)))`; a never-reassigned proxy
    `$.proxy($n.hmr_keep("x", () => init, () => x))`. Not keyed: `$state` in functions, blocks and class fields,
    destructuring declarations, `$derived`, `$props`, never-changing primitives.
  - `$n.hmr(C, id, sig)`: `sig` is a 64-bit FNV-1a hex of the keyed declarations, or `null` under `@hmr:reset`
    (a `//` or `/*` comment anywhere in the file); omitted without keyed declarations. Whitespace and comments in
    an initializer do not change it; a template or `<style>` edit does not either.
  - `.svelte.js` (`ModuleOptions.hmr`, `svelters-modules` passes it): top-level `$state` via `$$hmr().keep(..)`;
    `const s = writable(..)` (svelte/store) and `createContext()` (svelte) via `keep_object`. The output imports
    `$n` and ends with `$$hmr().commit()`.
- **Runtime** (`packages/native-runtime/src/index.js`):
  - `hmr_keep(key, init, read)`; `hmr(fn, id, sig)` keeps a registry per wrapper call (one instance), so two
    instances never share values. At a swap the live registry is snapshotted untracked, and handed to the new
    version's init when signatures match. A rolled-back swap keeps the old registry; the next good one still
    restores from it.
  - `hmr_module(hot, sig)` (`keep`, `keep_object`, `commit`): the registry lives in `import.meta.hot.data.$keep`.
    The new version evaluates before the old one is disposed (H1's order), so it reads the old version's live
    values; `commit()` is the last statement, so a throwing version leaves the previous registry in place.
    The module is not self-accepting: importers re-run and find the old values and the same store objects.
  - `hmrStats.statesKept` / `statesReset`, `setHmrPreserveState`.

## Tests

- `crates/svelters-compiler/tests/h1_hmr.rs`: keyed shapes, signature stable under formatting/template edits and
  changed by initializer/name edits, `@hmr:reset` → `null`, release output has no keys, only top-level instance
  state keyed, module output.
- `packages/native-runtime/test/hmr.test.mjs`: per-instance values (two instances of one wrapper), only changed
  values carried, proxy contents kept, other signature / `null` / global switch reset, rollback then restore,
  module `$state` and store object identity.
- `crates/svelters-egui/tests/hmr_state.rs` (kittest, three roots, `EmbedHarness`): editing `Panel` keeps
  each Panel root's own `clicks` (2 and 1) while the Pane root is untouched; the kept state is live; an edited
  initializer resets both roots; `@hmr:reset` resets; no root faulted or re-mounted.

## Not done / open

- Not built: H3c (children of an edited parent kept by positional key), `$bindable` fallbacks (D5), native UI state
  (H4), a build-time global switch (`hmr: { preserveState }`; only the runtime switch exists), upstream
  PR #17995's `hmr-preserve-state` sample, Rich Harris's two cases as conformance samples, `bind:this` targets
  are not excluded at compile time (a restored dead handle is overwritten when the binding runs).
- Callees of an initializer are not part of the signature (HMR.md 3.3 item 3); SV36 covers the unchanged ones, a
  changed one needs `@hmr:reset`.
- A keyed module `$state` that is a *primitive left unchanged* takes the new initializer; an object is always kept.
- Module keys are by declaration name; two modules never share a registry (it is per module `import.meta.hot`).
