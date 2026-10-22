# H1b + H3c: export propagation and state of an edited parent's children

Design: `plans/research/HMR.md` §2.1, §3.4 "Children of an edited component", §6 D6. Follow-ups of
`H1-component-hmr.md` (H1b) and `H3-state-preservation.md` (H3c). This file records both as built.

## H1b: `acceptExports` propagates to importers

- `JsEngine::hot_update` (`crates/svelters-host/src/quickjs.rs`): a boundary that accepts only some exports is
  checked against the new namespace (`__svelters_hot.check`). When another export changed, that module is
  **rejected**: it is treated as non-accepting, and the walk goes up its `ModuleProvider::importers` to the next
  accepting module. The loop re-plans until every boundary passes. Module versions only grow across the
  attempts (a name is never declared twice); a failure restores the saved versions as before.
- Only when no importer accepts, the change falls back to a full reload (`... changes an export that nothing
  accepts`, or the existing "not accepted by it or by any importer").
- Cost: the importer's component is swapped (its subtree is re-created); H3c keeps the state of its children.

## H3c: children of an edited parent keep their keyed state

- Positional key (`packages/native-runtime/src/index.js`): `<parent path>/<component id>#<n>`, n counting the
  instances of that component created under the same parent instance since the parent's current version
  started (call order). Found through the ownership `Scope` chain (`scope.hmr` on each instance's scope).
- When a parent swaps, `parkDescendants` snapshots the changed values (SV36) of every live hmr instance below it
  (all levels) before they are destroyed; instances created while the new version builds claim their entry by
  path (`takeClaim`) and restore it if their signature matches (same all-or-nothing rule as H3). Unclaimed
  entries are dropped when the build ends.
- The global switch (`setHmrPreserveState(false)`) and `// @hmr:reset` apply to children too.
- Limits: children created after the parent's init (inside later `{#if}` / `{#each}` updates) are not claimed,
  and an inserted or removed sibling of the same component shifts the numbering of the ones after it.

## Tests

- `crates/svelters-host/tests/hmr.rs`: `accept_exports_covers_default_only_and_a_changed_other_export_propagates_to_the_importer`
  (was the fallback test: Child's other export changes, App is re-evaluated, no fallback).
- `packages/native-runtime/test/hmr.test.mjs`: two children, the changed one keeps its state, the unchanged one
  takes the initializer; the global switch resets children.
- `crates/svelters-egui/tests/hmr_state.rs` `editing_a_parent_keeps_the_state_of_its_children`: a Shell root
  with two Panels; editing Shell keeps each Panel's own `clicks`.

## Not done

- Keys from compiler call-site ids (each-block key paths, if-branch ids) to survive inserted siblings and late
  children; measure first (HMR.md D6).
- `$bindable` fallback state (D5) and `bind:this` exclusion remain H3 follow-ups.
- Not run: the upstream hmr conformance samples, GTK/Qt `watch` tests, full workspace tests.
