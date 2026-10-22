# LD3: loader fixes cadatomic asked for (diagnostic re-export, hot-mode `svelters:entry` error)

Spec: coordinator assignment (2026-10-22), two requests from cadatomic after it pinned svelters 5e3bef68.
Lane LD3, branch `ci/ld3`.

## What landed

| File | Content |
|---|---|
| `crates/svelters-modules/src/source/mod.rs`, `src/lib.rs` | The compiler's diagnostic types are re-exported with feature `source` (the feature that already gates svelters-compiler): `Diagnostic`, `Severity`, plus `FixIt`, `TextEdit` and `FixHint`, the types of `Diagnostic`'s public fields, so a host can name everything a `Check` holds. `source` is a private module, so they are reachable at the crate root, next to `Check`: `svelters_modules::Diagnostic`, or `svelters_host::modules::Diagnostic`. The module docs say so. cadatomic can drop its direct svelters-compiler dependency. |
| `crates/svelters-host/src/quickjs.rs` | hot.js is evaluated as script `<svelters:hot>` (`EvalOptions::filename`). `ProviderResolver::resolve` maps a dynamic import whose base is that script to the requested name as is and does not ask the provider. |
| `crates/svelters-host/src/hot.js` | A comment: `import(id)` takes a module id, which the host resolves as is. |
| `crates/svelters-egui/tests/hmr.rs` | `loading_from_source_in_hot_mode_reports_no_error`: an embed loaded from source with `dev`+`hmr`, whose Panel has a `<style>` (so a generated `svelters-style:` module is loaded too), is mounted and run. The test asserts no `EmbedEvent::Error` and no host error. Before the fix it failed with two errors: `svelters:entry` and `svelters-style:<Panel>` "Error resolving module … from 'eval_script'". |

## The hot-mode error: the cause, and why the fix is in the host

In hot mode the host calls hot.js's `ctx(id)` for every module while it is declared. For the first version, `ctx`
queues `import(id)` to snapshot the module's exports (used by `acceptExports`' "other exports unchanged" check).
`import()` from a script goes through the module resolver with the script as base, so the provider was asked to
resolve a **module id** as if it were a **specifier** imported by `eval_script`. For file modules this works, because
an absolute path resolves to itself. For generated ids it fails:

- `svelters:entry` (and `svelters:kinds`): Node resolution treats them as packages, so the error is "cannot find package";
- `svelters-style:<component>`: LD2 lets only the component import its sheet, so it is refused;
- in a `ComposedProvider` with `Visibility::Filter`, a `svelters:app/*` module could be refused for the base `eval_script`.

The rejected `import()` is handled one microtask later, through the `.then` chain. By then the rejection tracker
has already recorded it as unhandled, so every source load in hot mode reported an `UnhandledRejection`.
cadatomic saw it as `EmbedEvent::Error` and filtered it by message.

The coordinator suggested two fixes. Decided (LD3, 2026-10-22): neither of them; the host fixes it instead. The
reasons:

- **hot.js skipping generated or virtual ids**: hot.js cannot tell which ids are generated. That is provider-specific
  (`svelters:entry`, `svelters:kinds`, `svelters-style:`, scheme hooks, native modules), and skipped modules would
  lose their export snapshot, so `acceptExports` would fall back to "previous exports unknown".
- **`SourceProvider::resolve` mapping `svelters:entry` to itself**: this fixes only one id in one provider. The style
  sheet would still fail, unless LD2's "only the component imports its sheet" rule is weakened for every importer.
  It would also let user code import the entry.
- **Host (chosen)**: hot.js's import is not an import by a module. It asks for a module the engine already holds,
  under the exact name `ctx` was given (`ctx(id)` runs inside `declare`, for an unversioned name only on the first
  version). Resolution is importer-relative and policy-laden, so it does not belong on this path. hot.js now runs
  as its own script (`<svelters:hot>`, a name no provider hands out). The resolver returns the requested name for
  that base only, and the provider is not consulted, so the graph gets no `eval_script` edge. Every provider and
  every generated id is covered, and no provider's rules are relaxed for real importers.

Not changed: the rejection tracker still reports a rejection that gets a handler later, because it records on
`handled == false` and never retracts. That is broader host behaviour (browsers report at the end of the microtask
checkpoint), outside this lane. No known path hits it after this fix.

## Verification

- `cargo check -p svelters-modules -p svelters-host --all-targets`: clean (one pre-existing `CatchUnwind` dead-code warning).
- `cargo nextest run -p svelters-egui -p svelters-host -E 'binary(hmr) | binary(hmr_state) | binary(live_styles)'`:
  13/13 passed, the new test included, and so did the `acceptExports` test, which relies on the first-version snapshot.
  The new test was run before the fix and failed with the two errors above.
- Not run locally: the full workspace nextest and the other gates; CI's five workflows are the gate.
