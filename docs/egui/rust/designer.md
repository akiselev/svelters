---
title: Designer seams
description: "What a visual designer or inspector reads and writes on egui: source locations of nodes, the pick stack, matched rules with <style> offsets, preview overrides, and state kept by call site."
status: pending
verified_at: f3dd475c
sources:
  - crates/svelters-compiler/src/codegen/component.rs   # $n.source_loc, $n.region sites
  - crates/svelters-compiler/src/scoped_css.rs          # ScopedCss::source_map
  - packages/native-runtime/src/index.js                # source_loc, instancePath, hmr_edit
  - crates/svelters-egui/src/scene/{source,hit}.rs      # SourceLoc, by_source, hit_stack
  - crates/svelters-egui/src/style.rs                   # matched_rules, force_style
  - crates/svelters-egui/src/overrides.rs               # set_override, clear_override
  - crates/svelters-egui/tests/{source_loc,style_source,overrides,hmr_sites}.rs
  - crates/svelters-automation/tests/source_loc.rs
  - crates/svelters-compiler/src/design/{mod,edit}.rs  # Design, Intent, Plan, rebase
  - plans/milestones/DZ0-designer-seams.md
  - plans/milestones/DZR-design-resolver.md
---

# Designer seams

cadatomic's visual designer edits `.svelte` source while the app runs
(cadatomic `docs/research/2026-10-25-visual-ui-designer.md`). These are the svelters pieces it builds on.
Everything here is for dev builds (`SourceConfig::dev`) on egui. Release builds emit none of it. The
reads are tool reads: they scan or recompute, so call them on a click, never per frame.

Status `pending`: on branch `claude/practical-lamport-y8z11o`, not yet merged to master.

## Which template element is this node

In a dev build every native node gets `source-loc`. It holds the template element's byte offset in its
`.svelte` file, then the component call sites it renders inside, innermost first.

```rust
backend.scene().read(|s| {
    let n = s.get(id).unwrap();
    if let Some(loc) = n.source_loc() {
        let el = loc.element();            // SourceSpot { file, offset }: `<button` in Row.svelte
        for site in loc.sites() { /* `<Row` in Panel.svelte, then outward */ }
    }
    // every node declared at one template element (each row of an {#each}, each instance)
    let rows: Vec<_> = s.by_source(file, offset).collect();
});
```

- `file` is the path the compiler was given, which for `SourceProvider` is the absolute path.
  `offset` is a byte offset, the position of the element's `<`.
- Rows of a keyed `{#each}` share their location. `NodeRef::item_key` says which row a node is.
- Automation: `source=src/Row.svelte#120` or `source=Row.svelte` finds nodes, and `scene.query`
  results carry `source: { file, offset, sites }`.
- Mechanism: for a backend whose spec has the `source-loc` prop, the compiler emits
  `$n.source_loc(node, $$src, offset)` per element and `$n.region(container, $$src, offset)` per
  static component call. The runtime sends one prop write per node, at its first placement.

## What is under the pointer

`SceneRef::hit_stack(viewport, pos)` returns every node drawn under a point, container boxes
included. They come topmost first and, within a layer, innermost first. A designer's click selects
the first, and a modifier click walks out to the boxes around it. Unlike `hit_test`, it ignores
`pointer-events: none`, which hides a node from input but not from the tool that edits it.

## Which rules style it, and where they are

`EguiBackend::matched_rules(h)` returns the rules that match a node in cascade order. Each comes with
its selectors, specificity and declarations, and in dev builds with where it is in its component's
`<style>`:

```rust
for r in backend.matched_rules(h) {
    // r.sheet: Some("app/src/Panel.svelte"); r.selectors: ".title.svelte-1x2y3z"
    if let Some(src) = &r.source {
        // src.file: the component's path; src.style_offset: bytes from the start of the
        // <style> element's content to the rule's selector
    }
    for d in &r.decls { /* d.name, d.value, d.important, d.source */ }
}
```

Offsets are relative to the `<style>` content, so they stay right while the template above it is
edited (that edit doesn't re-run the sheet module either). Add the content start of the file as it is
now. Theme and host sheets, and generated utility rules, have no source.

## Previewing a value before writing it

A designer writes the source once, when a gesture ends. While the gesture lasts, it previews on the
live nodes:

- `set_override(h, "gap", &Value::F64(8.0))` overrides a prop. The runtime's writes to that prop are
  held back, and `clear_override` (or `clear_overrides`) writes the latest of them back. An override
  never reaches `bind:` state. A node freed by a swap takes its overrides with it.
- `force_style(h, Some("color: #00ff00; padding: 4px"))` sets declarations above `style=` and every
  rule. `None` or `clear_overrides` removes them.

The usual sequence:
1. Override while dragging.
2. On release, write the source through the loader's overlay.
3. Call `apply_changes`. A template change swaps the nodes, which drops the overrides. For a style-only
   change, clear the overrides yourself.

## Writing the edit

`svelters_compiler::design` turns a designer intent into source edits. It needs no running app.
`Design::parse(src)` returns:
- the outline: template nodes with their attributes, each marked editable, structural or opaque with a
  reason;
- the `<style>` rules with their declaration spans.

`plan(&Intent)` returns `TextEdit`s in the file's own style, or a `Refusal` that carries the span to
edit by hand:

```rust
use svelters_compiler::design::{Design, Intent, Literal};

let d = Design::parse(&src)?;
let plan = d.plan(&Intent::SetAttr { node: offset, name: "gap".into(), value: Literal::Num(16.0) })?;
// plan.edits: [TextEdit { start, end, text: "{16}" }]; plan.text: the new file
```

- `node` is the element offset from `source-loc`. `rule` is a matched rule's `style_offset`.
- The intents are `SetAttr`, `ClearAttr`, `SetDecl` and `ClearDecl`.
- `rebase(offset, &plan.edits)` keeps a held selection pointing at the same node after the edit.
- Only literal attributes are written (`"row"`, `{12}`, `{true}`), and only rules whose selectors are
  kinds, classes and pseudo-classes. Expressions, handlers, spreads and blocks are refused with their
  span.

## State across the designer's edits

On egui in dev, a component instance's kept `$state` is keyed by its call site: the tag's place in the
parent's source plus the keyed `{#each}` items around it. The loader tells the runtime how the parent's
text changed, so after the designer inserts a call before others, they keep their state. Keyed rows keep
theirs too. [Hot reload](hot-reload.md) has the rules.
