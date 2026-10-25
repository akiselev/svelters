# PRESTO-compiler: svelters-compiler integrations (presto batch 1, lane COMPILER)

Branch `ci/presto-compiler` from `ci/presto-base`. Brief: `plans/cloud/presto-b1/BRIEF.md`,
`plans/cloud/presto-b1/LANE-compiler.md`; design: `plans/research/presto/compiler-and-js.md` §2–§4.
Date: 2026-10-25.

Every integration is a **backend capability** (spec `capabilities`). No backend advertises one yet
(egui's list is empty, `svelters-egui/src/spec.rs` `CAPABILITIES`), so the output for the DOM
conformance backend, GTK, Qt, AppKit, WinUI and egui is today's: conformance, `core-contract` and
the `expected.oplog` files are not touched. The tests switch the capabilities on in a copy of the
egui spec (`crates/svelters-compiler/tests/presto.rs`).

## Landed

### Spec (compiler model, `semantics/spec.rs`, `mod.rs`, `backend.rs`, `overlay.rs`)

- `Spec.capabilities` (`capability::{IDS, TEMPLATES, PASSIVE, CHANGE_CLASSES}`), `Spec.spec_hash`.
- `id` + `fingerprint` on kinds, props, signals, methods; `id` on slots.
- `change` (`ChangeClass`: property < paint < layout < structure, codes 1–4) on props and style
  properties.
- `cancelable` (absent: `sync`) and `passive` on signals; `EventSpec` carries them and the id.
- `a11y` on kinds (`A11ySpec`: `role`, `requires_name`, `name_props`, `icon_props`, `labelled_by`).
- `PlatformSemantics`: `capabilities`, `has_capability`, `spec_hash`, `kind_id`, `a11y` (defaults;
  `SpecBackend` and `Overlay` answer them). `LISTEN_PASSIVE = 4`.

### W-C1 stable ids

- egui: `crates/svelters-egui/src/kinds/ids.rs` and the checked-in table
  `metadata/semantics/egui.ids.json` (kind ids; per kind and category, member numbers; never
  renumbered, rows of removed members stay). The registry (`kinds/registry.rs`) now takes built-in
  kind ids and member ids from the table (`kind id << 16 | number`, declaring kind); provider kinds
  keep positional ids (member owner `0x8000 | (local + 1)`). The table was generated from the
  positional ids, so every id is the same number as before.
- egui spec (`spec.rs`): `id`/`fingerprint` per kind/prop/signal/method, slot ids, `spec_hash`
  (`spec::spec_hash()`, FNV-1a over the id rows), `capabilities`, `source.ids`.
  `tests/spec.rs`: `checked_in_ids_are_current` (staleness; `SVELTERS_UPDATE=1` appends rows),
  `registry_ids_are_the_tables`, `spec_carries_ids`.
- Codegen (`codegen/component.rs`, `Features`/`IdSource`, `compile.rs` `SpecIds`): with `ids`,
  `$n.element_id($$K_Kind)` for plain elements and `$n.setp(n, $$P_Kind_prop, v)` for static,
  one-off and reactive prop writes; module-end `const $$K_… = id;` and
  `$n.ids(backend, specHash, [[id, tag], [id, tag, prop], …])`.
- Runtime `packages/native-runtime/src/ids.js`: `ids`, `element_id`, `setp`. Ids are checked before
  their first use against `host.specHash()` when the host has it, else each id against the host's
  `kindId`/`propId` (which exist today), so a module compiled for another backend build throws
  naming both. Writes then go through `element`/`set` by name (op logs stay by name).

### W-C5 passive listeners

- `lower/passive.rs`: inference (no parameter; event only read as `event.<name>` outside
  `preventDefault`/`stopPropagation`/`stopImmediatePropagation`, never written; destructured
  fields; `arguments`, rest, computed keys, escapes and invisible handlers are non-passive;
  a non-cancelable signal is always passive). A handler named by a variable is followed to its
  top-level `function` or `const` function declaration in the component's scripts.
- With `passive`: `LISTEN_PASSIVE` in the listener's flags (`$n.on(n, sig, h, 4)`);
  `on:x|passive={h}` / `|nonpassive` (also with `|capture`) lower instead of `unsupported_feature`;
  warnings `passive_listener_cancels`, `nonpassive_redundant`, `nonpassive_scroll_listener`
  (a non-passive listener of a spec `passive` signal). Without the capability nothing changes
  (modifiers stay an error, so the conformance score cannot move).
- egui spec: `cancelable` (sync signals, except `pointermove`, `focusin`, `focusout`: `events.rs`),
  `passive` (`wheel`, `pointermove`).

### W-C2 native templates (compiler side + runtime expansion)

- IR `Item::Template { index, root }` (`lower/ir.rs`); `lower/templates.rs` wraps subtrees whose
  elements are slotted (not toplevel/attached), have only table ops or post-build ops (no spread,
  action, attachment, `bind:this`, style-rule ops), declare no snippet/`{@const}`/`{@debug}`, and
  whose other children are holes (blocks, components, render tags, `<svelte:element>`).
- `codegen/component/template.rs`: `const $$tN = $n.instantiate($$TN, container, [thunks], $$src?)`,
  `$n.at($$tN, k)` for the nodes later code uses, dynamic writes in the fragment's template effect
  as before, `$n.template_listen($$tN, [handlers])` after it when the template's listeners form
  one run in `after_update` order (else one `$n.on` each and the table lists no slots); module-end
  `const $$TN = $n.template("<content hash>", nodes, listeners)`. Dev builds keep each node's source
  offset (DZ0) in the table.
- Runtime `packages/native-runtime/src/template.js`: `template`, `instantiate`, `at`,
  `template_listen`. `instantiate` expands the table into exactly the per-node op sequence
  (create, source location, ops, children and holes in place, insert).

### W-C4 change classes

- `lower/change.rs` (run in `compile.rs` after lowering): `SetDynamic.change` from the prop's
  `change`; `Classes.change` from what the component's `<style>` rules for the toggled classes
  declare (any class for a dynamic class set); `InlineStyle.change` from its `style:` directives'
  properties; unknown when any property involved has no class.
- With `change_classes`: reactive writes pass the class code as one more argument
  (`$n.set(n, "width", w, 3)`, `$n.attr(n, "class", …, 2)`).
- Dev warning `layout_write_per_frame`: a layout-class write whose value reads a variable that a
  handler of a per-frame (spec `passive`) signal assigns.
- egui spec: `change: "layout"` on layout props and text-content props; style properties: setters
  `layout`, engine properties `paint`/`layout` by CSS semantics (`css_change`), the rest unknown.

### W-C9 accessibility diagnostics

- `lower/a11y.rs`: `a11y_missing_name` (a `requires_name` control setting none of its
  `name_props`/`labelled_by`), `a11y_icon_only_control` (only an `icon_props` prop set). Runs on
  every backend whose spec has the facts, i.e. egui now (warnings only).
- egui spec `a11y`: `requires_name` (WAI-ARIA name-required roles whose every name source is a
  prop), `name_props` (from `name_from`), `icon_props`.

- The provider-kind fragments the generators write carry the new fields too:
  `examples/egui-kinds/app/.svelters/app/kinds.json`, `fixtures/gallery-egui/.svelters/app/kinds.json`
  (regenerated with `SVELTERS_UPDATE=1` on `egui-kinds-example --test kinds` and
  `svelters-egui --test wg4b`).

### Codes and docs

- `codes.rs` rows for the six new codes; `docs/site/src/content/docs/guides/diagnostics.md`
  regenerated (`UPDATE=1 … --test cx1_codes`).

## Run / not run

- Ran: `cargo check -p svelters-compiler [--tests]`, `cargo check -p svelters-egui --tests`;
  `SVELTERS_UPDATE=1 cargo test -p svelters-egui --test spec` (the generator: rewrote `egui.json`
  and wrote `egui.ids.json`; its 9 tests passed); `cargo test -p svelters-compiler --test presto`
  (14 pass); `cargo test -p svelters-compiler --lib -- lower::passive lower::templates
  lower::change` (8 pass); `UPDATE=1 cargo test -p svelters-compiler --test cx1_codes` (3 pass);
  `python3 -I tools/docs/egui/gen_reference.py --check`, `gen_utilities.py --check`,
  `tools/docs/test_egui_reference.py` (pass, unchanged by the new spec fields).
- Not run (CI): every other test; `packages/native-runtime/test/template.test.mjs` (new; no
  `node_modules` in the container); conformance; the op-log goldens; the egui fixture suites.
  Risk to watch in CI: the a11y warnings now appear on egui compiles (tests that expect no
  warnings on egui sources would see them).

## Integration with RUNTIME and MOTION-CSS (coordinator, 2026-10-25)

Merged `origin/ci/presto-runtime` (4c65de8d) and `origin/ci/presto-motion-css` (1a016796). The
conflicts were all in `semantics/` (both lanes added `Spec.capabilities`): kept this lane's shape
and added MOTION-CSS's three constants (`NATIVE_TRANSITIONS`, `NATIVE_MOTION`, `CSS_PRECOMPILED`).

- **`setp` (the duplicated export that broke `pnpm -r build`):** one export, RUNTIME's scalar
  fast path in `index.js` (name or id). `ids.js` no longer has one. Codegen with `ids` emits
  `$n.setp(n, $$P_…, v)` only for a scalar-typed prop (bool, number, string, enum) other than
  `anchor` that no track, token, style rule or binding on the same element uses; every other
  write stays `$n.set(n, "name", v)`, which keeps that bookkeeping (`IdSource::prop` returns
  whether the type is scalar). `pnpm -r --filter '!@svelters/docs' build` exits 0 (the docs site
  needs `tools/docs/build_ir.py` first, which CI runs).
- **(c) `LISTEN_PASSIVE` (cross-lane, `index.js`):** exported; `hkey` leaves the bit out, so a
  passive listener is found by dispatch; a (signal, phase) connection is passive while all its
  handlers are (`Native.passiveConn`): a handler that can cancel joining one makes the runtime
  listen again without the flag; `off` unlistens with the flags it listened with.
- **(d) `adopt` (cross-lane, `index.js`):** records the nodes a host built from a template as
  `create` and `attach` would, without host ops. `template.js` uses RUNTIME's `host.template` /
  `host.instantiate` when `host.templates` is true (a backend that builds templates) and the table
  holds only static scalar props, classes, styles and ids (no holes, thunks, child attributes,
  tokens, tracks, source locations); the registration is cached per host; classes, styles and ids
  go through `attr`, then the root is appended. Everything else expands as before. Teardown stays
  per node (`destroyRange` is not used: dispose destroys natives one by one).
- **(e) `specHash` (cross-lane, `svelters-host`, `svelters-egui`):** `svelters_host::
  register_spec_hash(backend, hash)` / `spec_hash(backend)` (new `src/spec_hash.rs`) and
  `host.specHash(backend)` in `quickjs.rs`; svelters-egui registers `spec::spec_hash_of(&registry)`
  when its kind registry is built. `ids()` checks at declaration when a host exists (by the hash,
  else per id through `kindId`/`propId`), else before the first `element_id`.
- Tests: `packages/native-runtime/test/template.test.mjs` (6: expansion op log, `at`, `element_id`,
  refused ids, passive keying and re-listen, the host-template path with a TestHost that builds
  templates); `tests/presto.rs` `ids_keep_set_for_props_with_bookkeeping`; the host's
  `spec_hash` unit test. Ran: those, `cargo test -p svelters-egui --test spec`, the whole
  native-runtime package suite (157 pass), `pnpm -r` build as above. Not run: everything else.
- Note: the TestHost resolves prop ids only on nodes it created by kind id, while `element_id`
  creates by tag (the runtime's node keeps its tag); the real host resolves ids by the node's
  kind. Nothing advertises `ids` yet; when egui does, RUNTIME's create-by-id fast path (create by
  id, keep the tag on the node) covers both.

## Stubbed / left out (honestly unimplemented)

- **`destroy_range` in teardown** is not used (dispose destroys natives one by one, adopted ones
  included), and host-built templates cover only static tables (see above).
- **`element_id`** creates by tag; signal ids (`$n.on` by id) and ids in template tables are not
  emitted (tables keep names; the host resolves them once per registration).
- **No backend advertises a capability yet** (see questions).
- The `layout_write_per_frame` fix-it (rewrite to `transform`) is not written (only the help line).
- Passive inference does not follow handlers imported from other modules, or `let` bindings
  (they are non-passive).

## Cross-lane edits

- `packages/native-runtime/src/index.js` (RUNTIME's file): the re-exports of `ids.js`
  (`ids`, `element_id`) and `template.js`; `LISTEN_PASSIVE` and its keying in `hkey`,
  `addHandler`, `off`, `Native.passiveConn` (Q2); `adopt` (Q3) — both decided by the coordinator
  (2026-10-25) as this lane's.
- `crates/svelters-host` (RUNTIME's crate): `src/spec_hash.rs`, its `lib.rs` exports, and
  `host.specHash` in `quickjs.rs` (Q4, coordinator 2026-10-25).
- New runtime file `packages/native-runtime/src/ids.js` (the lane file named only `template.js`; ids
  need their own runtime entry points).
- `crates/svelters-egui/src/kinds/registry.rs` (RUSTAPI's writer lane, listed as this lane's for the
  stable ids): the id scheme reads the table; `kinds/mod.rs`: `pub mod ids`.
- `presto-base`: no change.

## Questions for the coordinator

1. Decided (coordinator, 2026-10-25): egui advertises no capability yet; at consolidation 2,
   `templates` first (the six egui op-log fixtures prove it), then `ids` after the `setp` fast
   path; `passive` and `change_classes` for presto only.
2.–4. Decided (coordinator, 2026-10-25) as this lane's work: done above.
5. Decided (coordinator, 2026-10-25): this lane's `Spec.capabilities` shape, with MOTION-CSS's
   three constants: done in the merge.

## Handoff

The lane's five deliverables have first implementations; the next steps are the RUNTIME seams
above and flipping capabilities at consolidation.
