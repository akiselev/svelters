# Native binding linker (Q1)

The binding linker preserves native-toolkit reachability across the JavaScript/native boundary. Its purpose is to let ordinary JS tree shaking decide what application code survives, then generate only the native bridge required by that optimized program.

## 1. Resolve native calls before bundling

Generated toolkit typings and Svelte `bind:this` provenance make most calls statically identifiable:

```svelte
<QTreeView bind:this={tree} />
<script lang="ts">
  function reveal(index: QModelIndex) {
    tree.scrollTo(index, QAbstractItemView.ScrollHint.PositionAtCenter);
  }
</script>
```

The Svelters transform resolves the receiver and overload against the canonical API IR and lowers the call conceptually to a virtual import:

```js
import __b from "svelters-bind:qt:<stable-id>";
__b(tree, index, QAbstractItemView.ScrollHint.PositionAtCenter);
```

Do not lower to `native.invoke(tree, "scrollTo", ...)`; that hides identity from the module graph.

## 2. Resolution inputs

Use, in order:

1. generated native `.d.ts` declarations carrying canonical binding IDs;
2. TypeScript semantic type information for receiver/arguments;
3. Svelte template provenance for `bind:this` and generated native components;
4. literals/enums/nullability/default-argument rules;
5. explicit user disambiguation.

A resolution result is either `Exact(BindingId)`, `FiniteCandidates(Vec<BindingId>)`, `Dynamic`, or `Error`. Release strict mode rejects unresolved ambiguity.

Provide an explicit escape hatch:

```ts
qt.call<"QTreeView::scrollTo(QModelIndex,QAbstractItemView::ScrollHint)">(tree, index, hint)
```

This is rare but deterministic.

## 3. Let Rolldown do DCE

During stage A, every `svelters-bind:*` module is external. Normal Rolldown tree shaking removes dead application paths and therefore their dead native imports.

After stage A, collect only surviving external binding imports. This set, not the unoptimized source scan, is the release native root set.

Add manual roots from `svelters.toml` and expand transitive dependencies from the API IR:

```text
live binding
  -> receiver/value types
  -> argument/result converters
  -> callback trampolines
  -> ownership helpers
  -> structural/custom adapters
```

Sort by stable ID and assign compact per-build dense IDs.

## 4. Release manifest

Emit deterministic JSON similar to:

```json
{
  "schema": 1,
  "backend": "qtwidgets",
  "api_fingerprint": "...",
  "roots": ["..."],
  "bindings": [
    {"stable_id":"...","dense_id":0,"canonical":"QWidget::show()","reason":"src/App.svelte:18"}
  ],
  "dependencies": ["..."],
  "dynamic_roots": []
}
```

Record source reasons separately from the canonical hash so source-location churn does not invalidate API identity.

## 5. Native generation

QtWidgets: emit direct C++ thunks for ordinary methods/constructors and use the generic metaobject path when that is smaller/safer. Compile only selected thunks plus transitive converters.

GTK: dev may invoke GI metadata dynamically; release emits direct `-sys`/C calls for selected methods where that improves size/static-link visibility. GObject properties/signals can remain generic.

egui: most operations are curated renderer ops and do not need raw Rust reflection. User-defined renderer operations become explicit plugin binding IDs.

Stage B rewrites virtual imports to dense calls such as `__sv_native(17, receiver, args...)` or an equivalent per-module table.

## 6. Dynamic bindings

Truly dynamic code is explicit:

```toml
[bindings.qt]
include = ["QFileDialog::getOpenFileName(QWidget*,QString,QString,QString,QString*,Options)"]

[bindings.gtk]
include = ["Gtk.Widget.grab_focus"]
```

Permit wildcard roots for migration/development, but warn in release and report their expansion cost.

Development records every binding invoked, including calls resolved dynamically by a fat bridge/GIRepository. Commands:

```text
svelters bindings trace
svelters bindings explain <symbol>
svelters bindings unused
svelters bindings diff <old> <new>
```

`--strict-bindings` fails CI when runtime traces observe a binding with no static/manual explanation.

## 7. Dev versus release

Development optimizes iteration:

- GTK uses GIRepository plus generic GObject paths wherever possible.
- Qt uses a prebuilt/cached fat development bridge generated per Qt/API database version.
- adding ordinary Svelte/TS calls does not invoke native compilers.

Release optimizes shipped code:

- optimized JS determines live imports;
- only live native thunks/converters are generated;
- normal native linker section GC/LTO sees direct native references.

The fat dev bridge is an Artifactum-cached tool artifact, not part of the final application by default.

## 8. Verification

The linker phase must include tests for dead branches, dynamic imports, overloads, generic helper functions, inherited methods, aliases, enum conversions, callback signatures, manual roots, wildcard roots, and deliberate unresolved calls.

A release fixture must prove that adding an unreachable native call changes neither the native manifest nor the final native symbol set.
