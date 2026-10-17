# P3 GTK lane notes

## Layout
- `tools/metadata/gtk/gir.py` (GIR parsing), `ingest.py` (GIR + PyGObject live pass + overrides -> `metadata/gtk/<Kind>.json`), `gen_backend.py` (JSON -> `crates/svelters-gtk/src/generated/*.rs` and `metadata/gtk/gtk.d.ts`).
- Overrides: `metadata/overrides/gtk/<Kind>.yaml`. Interface-owned facts (`text`, `changed`, `orientation`) are annotated on the interface kind (`Editable`, `Orientable`) with `verify_subject` naming the element used by tests.
- Runtime: `backend.rs` (vtable over handle table), `convert.rs`, `lookup.rs`, `testing.rs` (GTK thread, stub host, name-driven harness). Raw `svl_backend` vtable, not P2's `Backend` trait.

## Findings
- `GObject.ParamSpec.get_default_value()` segfaults PyGObject for object/boxed pspecs: only scalar/enum/flags defaults are read.
- `list_properties` on a class reports interface properties with the interface as owner, so they live in the interface kind JSON. The runtime walks kind -> interfaces -> parent.
- GTypes register lazily: `generated/types.rs` calls each class's `*_get_type` before `g_type_from_name`.
- GtkApplication exports on the session bus, one per process: the backend shares one registered `GtkApplication` per UI thread.
- `gtk_window_destroy` only drops GTK's own reference, so toplevel handles hold a weak ref (otherwise `on_destroyed` never fires). Other widgets are held strongly until `destroy`, so remove + re-insert works.
- ScrolledWindow wraps a non-scrollable child in a Viewport; `get_child` returns the viewport. Annotated as `effects.child: wrapped:Viewport`; remove accepts the child or an ancestor.
- Setting `text` makes GTK emit `changed` twice (delete + insert); tests accept repeats.
- GtkHeaderBar has no `title` property. A virtual prop `title` (annotated, `virtual.op: managed_title_label`) manages a GtkLabel (css class `svl-managed`) in `title-widget`. A widget placed through the `title` slot replaces it.

## Schema/format extensions (additive)
- Per-record `annotated: {origin, provenance, fields}` (written by ingest for every overridden or new fact; generated tests exist per `annotated` record).
- Property: `runtime{}`, `virtual{op,property}`, `echo_suppress[]`, `type.gtype`; signal: `trigger`, `derived_from`; method: `js_name`, `dispatch`; kind: `element{tag}`, `native.get_type`, `children.candidates`, `children.policy: unknown`, `verify_subject`.
- Slots use signals-and-slots-semantics.md 2.5 descriptors (`native`, `args`, `effects`, `get`, `provenance[]`). GTK additions: `get: {contains: parent|ancestor}` for many-slots without a getter, `list: {iterate: {first, next}}`, and the `prev_sibling` argument expression (previous sibling of `before`, or the last child when `before` is null). `verify:` DSL steps are not implemented; tests are derived from `ops`/`effects`/`order`.

## Open
- `.d.ts` is written to `metadata/gtk/gtk.d.ts` (P3 may not edit `packages/dom/types/`); copy it there at integration.
- `handler_result: bool_stop` for bool-returning signals is a heuristic default, not annotated.
- No dynamic GI method invocation (M3). Only `Window.present` is a method.
- `remove` of a non-slot child (not inserted via a slot) is an error.
