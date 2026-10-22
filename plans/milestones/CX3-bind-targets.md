# CX3: bind-target lowering (cadatomic SV3)

TRACKS §2.1 COMPILER, milestone CX3; CADATOMIC.md SV3 ("a lowering table, not a code hook"); the
scene-node research §3 (`bind:value|commit`, draft bindings, native store bindings). P0 for
cadatomic's CH2 task panels (commit on Enter). Branch `ci/cx3`.

## What landed

### 1. `bind:prop|commit`

- **Contract (coordinator, FM1 seam; ARCH-WIDGETS uniform `input`/`commit`/`submit` signals):** a
  spec binding may carry `"commit": {"event": "commit", "arg": n}` next to its `event`/`value`.
  The compiler reads only that entry (`BindingSpec::commit` → `BindingPlan::commit`), never a
  widget's behaviour.
- **Lowering:** `plan_bind_commit` (`semantics/observe.rs`) → `BindLowering::Commit` →
  `Op::BindCommit` → `$n.bind_commit(node, prop, get, set, signal, arg)`. The runtime helper
  (`packages/native-runtime/src/index.js`) is `bind` on the commit event: state → native at once,
  native → state only when the event fires. Nothing observes the prop and nothing listens to its
  change event, so typing runs no JS.
- **Modifiers:** `|commit` joins `|nonreactive` in `bind_modifiers()`; at most one of them
  (`|commit|nonreactive` and repeats are `bind_modifier_invalid`); `bind:this|commit` and component
  bindings reject it as before.
- **Diagnostic** `bind_commit_unsupported` (`svelters::bind::commit_unsupported`, error): the kind
  declares no commit entry for the prop (egui's `slider` today, every GTK/Qt binding). Fix-it:
  remove `|commit`.
- **egui (the e2e fixture's widget):** `text-edit` got `commit(text)` (Enter in a one-line field, or
  losing focus after an edit since the last commit, as Qt's `editingFinished`) and `submit(text)`
  (Enter only, after `commit`). Enter that the focus scope's default button takes
  (`focus.rs`, `State::entered`) still commits the field first, so the button's handler reads the
  committed value. `spec.rs` writes the `commit` entry for every binding whose kind has a `commit`
  signal (only `text-edit` now; the own-widget lanes add theirs, and the compiler picks them up
  from `egui.json` with no change). `NodeFlags::EDITED` (bit 17) tracks "edited since the last
  commit".

### 2. The lowering table (`bind_sources`)

- `Spec::bind_sources: Vec<BindSourceSpec>` (`{module, export, lowering: link|draft, get?, set?,
  doc?}`), appended to by spec fragments (`merge_fragment`; a duplicate row or a draft row without
  `get`/`set`/named export is an error). `PlatformSemantics::bind_source(module, export, lowering)`
  (default `None`; `SpecBackend` and `Overlay` answer). A row matches the root identifier of the
  bound expression by its import (`ScriptFacts::imports`: every named import, renames included);
  `module` is exact or a `/*` prefix, `export` a name or `*`.
- Format documented in `docs/notes/native-compiler/semantic-spec.md`; runtime helpers in
  `docs/notes/native-compiler/runtime-contract.md`.

### 3. Native links (`link` rows): `bind:prop={store.field}` with no JS per change

- egui's spec declares `{module: "svelters:app/*", export: "*", lowering: "link"}`. GTK/Qt/AppKit/
  WinUI declare none, so E3's `store-field-gtk` golden is unchanged.
- **Compiler:** a plain `bind:` (lowering `Prop` or `Signal`) whose target is `obj.field` with `obj`
  a matching import becomes `Op::BindLink { obj, field, fallback }` →
  `$n.bind_link(node, prop, () => obj, "field", () => { <the planned binding> })`. `|commit`,
  `|nonreactive` and read-only props keep the JS binding.
- **Runtime** `bind_link` (`src/bind-observe.js`): `host.link(obj.svlHandle, field, node.h, prop)`;
  a non-zero id → teardown `host.unlink(id)`; 0, no `host.link`, or no handle → `fallback()`.
- **Host** (`crates/svelters-host/src/core/links.rs`, `host.link`/`host.unlink` in `quickjs.rs`):
  both ends are observed through the ordinary refcounted `observed` table, each counting the link in
  `Observed::links`. `take_changes` propagates a changed value of one end to the other (store:
  generated setter with `on_write`, then R1's read-back; node: `set_prop` + read-back) and delivers
  a pair to JS only when JS observes it itself (`count > links`). A store that refuses a value
  (`on_write` `Err`, type mismatch) gets its own value written back to the node, with a `link`
  warning. At link time the store's value goes to the node. Refused (0): not an app object, not a
  writable non-proportional store field, or a prop the backend cannot observe. Destroying the node
  or releasing the store drops its links (`links_gone`). `ReactiveStats::link_writes` counts the
  writes across.

### 4. Draft bindings (`draft` rows)

- `bind:prop={marker(...args)}`, `marker` imported from a row's `module` with the row's `export`,
  lowers to the planned binding (any modifier) with the accessors
  `() => $$bind_source_<n>.<get>(...args)` and `($$value) => $$bind_source_<n>.<set>(...args, $$value)`;
  the module is namespace-imported once per component (`Component::bind_imports`, codegen
  `draft_call`). Arguments are ordinary template expressions (each-block items included).
- A call that matches no draft row, or a spread argument, is `bind_source_invalid`
  (`svelters::bind::source_invalid`, error); before CX3 it compiled to `f(x) = v`.
- Provider crates declare rows from Rust: `inventory::submit! { svelters_egui::provider::BindSourceDef::draft(module, export, get, set) }`
  goes into the dumped fragment's `bind_sources` (only when there are any, so existing `kinds.json`
  files are unchanged). JS sources work too (the fixture's `#drafts` package import).

### Decided (coordinator delegate, 2026-10-22)

1. **Draft semantics = data, not a session protocol.** A draft row names a value getter and a
   setter; svelters calls them. Validation, logging, problems and `Op` state on the node (scene-node
   §3) are the provider's: its `set` pushes the draft event, its `get` reads the session store. The
   simplest consistent reading of "events to a store method".
2. **Modifiers are orthogonal to the source.** `|commit` on a draft source means "set on commit";
   without it every change calls `set`. A row cannot force commit mode (cadatomic writes `|commit`).
3. **`get` must be reactive for the node to follow.** A plain Rust function export is read when the
   binding's effect runs; reading a store getter (or `$state`) inside `get` makes it track.
4. **Links are plain `bind:` only.** `|commit` on a store field keeps the JS `bind_commit` (one JS
   call per commit); a commit-aware Rust link needs the host to listen to signals and is not built.
5. **The store is the source of truth at link time**; the node is overwritten with the store's value.
6. **Link rows match imports, not types.** The compiler has no export types yet (E5/CX4), so
   `svelters:app/*` member bindings are lowered to `bind_link` and the host decides; a non-store
   object falls back to the JS binding at run time with no diagnostic.
7. **egui `text-edit` commit:** Enter always commits (even unchanged text, as Qt's `returnPressed`
   → `editingFinished`); blur commits only after an edit. Multi-line fields commit on blur only.
   Escape does not revert the uncommitted text.

## Tests

- `crates/svelters-compiler/tests/cx3_bind.rs` (7): commit lowering (plain and function bindings),
  `bind_commit_unsupported` with its fix-it (egui slider, GTK entry), modifier misuse, link lowering
  and its exclusions (`|commit`, `|nonreactive`, non-`svelters:app` imports, local state, GTK), draft
  lowering (renamed import, each-item arguments, one namespace import), `bind_source_invalid`,
  fragment checks.
- `crates/svelters-compiler/tests/r2/diagnostics-gtk.expected`: regenerated (the modifier list and
  help now name `|commit`).
- `crates/svelters-egui/tests/bind_targets.rs` (3, live-compiled from `fixtures/motion-egui/cx3`, a
  nested package that is not a workspace member): `text-edit bind:text|commit` updates `$state` on
  Enter and on blur after an edit only (not while typing, not on blur without an edit), and through
  a default button's Enter; a JS draft source logs only commits with `|commit` and every change
  without; a Rust store (`svelters:app/cx3`, with an uppercasing `on_write`) linked to a text field
  and a slider: typing reaches the store and its normalized value comes back, Rust writes reach the
  node, and `ReactiveStats` shows link writes with no store batch delivered to JS.
- `packages/native-runtime/test/bind-observe.test.mjs`: `bind_commit`, `bind_link` (linked, refused,
  no `host.link`, no handle).
- Regenerated: `metadata/semantics/egui.json`, `packages/native-runtime/types/egui.d.ts`,
  `docs/site/src/content/docs/guides/diagnostics.md`.

## Open (for the coordinator)

- **Stock tooling and draft calls.** Stock Svelte (svelte-check) rejects `bind:x={f(...)}`
  (`bind_invalid_expression`), so a cadatomic component using a draft source compiles with svelters
  but fails svelte-check. Options: (a) accept `bind:x={slot(path).value}` (a member expression stock
  Svelte allows) as the draft form too; (b) a svelte2tsx preprocess in `@svelters/build`. Recommend
  (a): one more match arm in `bind_via`.
- Provider kinds (RA1) have no `bindings`, so `|commit` on them is `bind_commit_unsupported` until
  their fragment writes a `commit` entry (e.g. for a `#[event] commit`); RA3 or an own-widget lane.
