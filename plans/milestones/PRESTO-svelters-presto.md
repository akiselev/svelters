# PRESTO-svelters-presto: the svelters backend on presto (lane record)

Lane SVELTERS-PRESTO of batch presto-b2 (`plans/cloud/presto-b2/LANE-svelters-presto.md`), branch
`ci/presto-svelters-presto`, session `session_01497YqbhMLBiY7YWGtiLKAX`, 2026-10-25. Handed off at the
context limit (see "Hand-off" below); the branch builds (`cargo check -p svelters-presto --all-targets`)
after merging `ci/presto-base` at 7a947d45 (CORE, TEXT, RENDER, COMPOSITOR, FRAME-ANIM skeletons).

## What landed (`crates/svelters-presto/**`, session 1; `ids`/`wire`/`shim` superseded in session 2 below)

| Module | What |
|---|---|
| `backend.rs` | `PrestoBackend`: `impl svelters_abi::Backend` (resolve, create, insert with slots, remove, destroy, set/get typed by id, call `present`/`close`, listen and `listen_ex` with capture and `LISTEN_PASSIVE` = 4, handle_info with borrowed slots, run through a `Driver`, waker, quit, schedule_wake, request_frame, style, observe with nesting counts, start, describe). Handles are the backend's own, counted up and never reused, so a template's nodes are `base + k`. The `Driver` trait is the seam to whoever runs the loop (the presto app, a harness). |
| `deliver.rs` | Event-time delivery of a `Dispatched`: observed native changes first (`prop_changed`), events in order, cancelable/sync ones through `call_sync` (its `true` cancels the default), one closing checkpoint. `pump()` delivers native output produced outside input. |
| `template.rs` | `Template`/`TemplateNode` (decoded table), `instantiate` (contiguous handles, CF2 order, roots left to the caller, rollback on failure), `destroy_range`, `Templates` (content-hash registry with refcounts). |
| `style.rs` | `style` op: `TARGET` → 4 (engine), sheets/classes/id/inline/scheme into the tree, TR1 visual channels (opacity, transform, clip, inert); `ANIMATE` answers 0 so the runtime plays animations itself until presto-motion is wired; `DECLS` refused. |
| `ids.rs`, `ids.txt`, `wire.rs` | Stable wire ids: an append-only table (one row per kind/prop/signal/method/slot, keyed by the declaring kind, so inherited props share their base's id), FNV-1a fingerprints per member, a spec hash. See Q1: presto-core chose hash ids instead. |
| `spec.rs`, `css.rs` | The `presto.json` generator (svelters-semantics/v0 like egui.json, plus `id`/`fingerprint` per kind and member, `ids.spec_hash`, `change` per prop and per engine CSS property, `passive`/`cancelable` per signal, `a11y` with `name_required`, `native_state`, `slot_ids`, `capabilities: ids templates native_transitions native_motion css_precompiled`). `css.rs` is presto's engine property table with change classes (egui's list plus `transform`, `transform-origin`, `visibility` as property-class). |
| `embed/` | The embed model minus egui: `Embed` (config, budgets, fault policy, load/load_from/load_app, slots as borrowed `box` nodes, mount/props/unmount/move/close/restart queued and applied in `logic()`, root info sync, faults per root, `settled()`), `roots.rs`, `slots.rs` (window/size/scale/focus reported by the app: `set_slot_layout`, `set_slot_focused`), `settle.rs` (egui's `Busy` plus `COMMIT_IN_FLIGHT`, set with `set_commit_in_flight`). Panes (CV1) not ported. |
| `value.rs` | ABI `Value` ↔ prop values by type (numbers across int/float, enums by name, `#rrggbbaa` colours, lists, records). |
| `shim/` | Stand-ins (below). |

Generated: `metadata/semantics/presto.json` (from `builtin_registry()`), `crates/svelters-presto/ids.txt`.
Staleness: `tests/spec.rs` (`SVELTERS_UPDATE=1 cargo test -p svelters-presto --test spec`).

Tests (written, **not run**, except `tests/spec.rs` run once with `SVELTERS_UPDATE=1` to generate the
spec and the id table): `tests/ops.rs` (the ABI through the real vtable), `tests/oplog_replay.rs` (every
Tier 0 egui fixture's `expected.oplog` — counter, if-toggle, keyed-list, motion, timers — replayed on
presto: ops succeed and read back; `event`/`changed` lines produced natively through the tree's
`emit`/`native_set` and delivered once, `changed` exactly when observed), `tests/template.rs`,
`tests/deliver.rs`, unit tests in `value.rs` and `style.rs`.

## Cross-lane edits

| File | Why |
|---|---|
| `crates/svelters-compiler/src/semantics/backend.rs`, `kinds.rs` | `SpecBackend::builtin("presto")`; presto in `every_builtin_spec_builds` (lane brief). |
| `crates/svelters-modules/src/source/transform.rs` | `styles()` checks the spec's `style.engine == "backend"` instead of `backend == "egui"`; the warning text names both engines. |
| `crates/svelters-host/tests/modules.rs` | Follows that warning text (lane RUNTIME's crate; one assertion string). |
| `packages/native-runtime/vite.js` | `runsCssEngine(backend)` reads the spec's `style.engine` (two `'egui'` checks). |
| `packages/testhost/src/index.js` | Same check for the default `styleDialect`. |
| `tools/metadata/style/gen_props.py` + `packages/style/src/props.generated.js`, `crates/svelters-style/src/tables.generated.rs` | presto in `TOOLKITS` (regenerated: a `PRESTO` table in `tables::ALL`). Rerun whenever presto.json's style section changes. |
| `tools/metadata/style/gen_utilities.py` | `--report` lists the utility rows presto's style table cannot apply (none today); outputs unchanged. |

## Session 2 (2026-10-25): the port to the real presto-core

Session `session_012kp44HgxNR751yz6GvNAH5`, after merging `ci/presto-base` at ac7cc8c4 (real presto-core,
presto-text, the presto-widgets substrate). Coordinator answers applied: Q1 (presto-core's FNV-1a ids on the
wire, `ids.txt` dropped), Q2 (egui's JS names onto core's DOM names), Q3 (keep the side table until CORE's
`KindSpec` grows), Q4 (pending: CORE adds `Document::attach_window`).

| Module | What changed |
|---|---|
| `kinds.rs` (new) | `Kinds`: the JS-facing kinds on one `presto_core::KindRegistry`. presto-widgets' kinds are registered through `presto_widgets::host::register` (`button` today); every other concrete Tier 0 kind gets a core kind synthesized from its surface spec (`Placeholder`: real props, signals, slots and role, draws nothing) until its widget lands. Per member a map: props `Native(PropId)` / `Inverted(PropId)` (`enabled`↔`disabled`, `visible`↔`hidden`) / `Side` (a surface prop the presto-widgets kind does not take yet, kept by the backend); signals the kind's, else core's common one (`click`, `keydown`), else none (never fires: `drag-*`); slots (first surface slot = core's default). Wire ids: presto-core's hash of (declaring kind, member, type), the declaring kind being the tag, `*` for `Widget` (core's common pseudo-kind) and the lowercased name for `Run`; so own native members have their core id, inherited ones one id on every kind. |
| `surface/` (was `shim/`) | Only the Q3 side table now: the surface `KindSpec` (docs, methods, named signal args, nullable, a11y name sources, base chain), `Value`/`ValueType`/`DefaultValue`, `tier0`, plus `ColorScheme`/`Visual` (inputs core does not take yet). The shim `Document`, registry, ids, errors and events are gone. |
| `backend.rs` | On `presto_core::Document` (`Document::new(registry, text)`; `NoText` until the runner installs presto-text). `free` detaches JS-known children before `destroy` (core destroys subtrees; the ABI leaves children to the host). Writes during delivery run in `js_transaction(seen)` (§4.5; `Superseded` is not an error). `request-focus` calls `Document::focus` when the node is in a window (none are until Q4). Side stores: `Side` props, per-node `StyleInput`, visual channels, colour scheme, sheets by name (`add_sheet`/`remove_sheet`). |
| `deliver.rs` | `Output` (events + native changes + revision) from `Document::dispatch`, `take_events` and `drain_changes` (`Change::Prop { native: true }`); `dispatch(&InputEnvelope)`, `pump`, `queue`, `target_event`. Events go along presto-core's paths (capture/target/bubble, `target`/`phase` args, record fields by the surface arg names); `call_sync` answers egui's bits (1 stop, 2 prevent, 4 immediate) or `true`. |
| `value.rs` | `to_core`/`from_core`, `signal_arg` (surface args → core's one arg), `signal_args` (core arg → JS args). |
| `spec.rs` | ids/fingerprints from the hashes (`fingerprint` = the id in hex), `ids.scheme`, `native` = core's name (Q2), `widget` per kind (`presto-widgets` / `placeholder`). Style section unchanged, so `gen_props.py` was not rerun. |
| `ids.rs`, `wire.rs`, `ids.txt` | Deleted (Q1). |

Regenerated `metadata/semantics/presto.json` (`SVELTERS_UPDATE=1 cargo test -p svelters-presto --test spec`, the
only run: 4 passed). Tests updated (written, not run): `tests/common` queues native output as presto-core
reports it (`target_event`, a set plus a reported change), `tests/spec.rs` checks the hash ids and Q2 names
instead of the table, `kinds.rs` has a unit test.

### Session 2, second part: presto-base e58a6d8a (CORE-2's additions, presto-widgets' Tier 0)

- Merged `origin/ci/presto-base` at e58a6d8a. presto-widgets now has 17 Tier 0 kinds; all are registered
  (`kinds::register_hosted`). Still placeholders: `window`, `slider`, `text-edit` and the inline runs. A surface
  member a hosted kind lacks falls to `Side` (props) or never fires (signals).
- Synthesized core specs now carry CORE's new fields: prop `nullable`/`doc`, signal `args`/`doc`, `methods`
  (`ArgSpec`s), `name_from` (egui's sources mapped to `NameSource`), kind `doc`. Method wire ids are
  `MethodId::of(decl, name, ret)`.
- Q4 wired: `present` attaches the JS-built `window` node with `Document::attach_window(WindowId(handle),
  node, default size, scale 1)`; freeing a window root closes its window (`close_window`), and
  `window_closed` frees the handle. The driver resizes with the real size when it opens the OS window.
- A hosted kind's own methods go through `Document::call` (their events are queued for `pump`).
- presto-widgets' `label` is a leaf, so inserting runs (`<label>a <b>b</b></label>`) into it now fails with
  an error that says so. No Tier 0 fixture does it. **Q7** (recommendation): presto-widgets' label takes
  run children (its `rich` module), or presto-widgets keeps a `rich-label` kind that svelters-presto maps a
  label with runs onto.
- Not done: dropping `surface/tier0.rs` (Q3). Core's `KindSpec` has no base chain and presto-widgets' names
  are core's, not egui's, so the JS-facing surface still comes from the table. Once WIDGETS names follow Q2,
  `spec.rs` can generate from the registry plus a `Widget` table.

### Missing behaviour props and signals per hosted kind (coordinator Q6, for WIDGETS-BASIC/TIER1)

The surface (egui) members presto-widgets' kinds do not take yet, from `Kinds::builtin()` at 7546a2a6+ (stored
by the backend as `Side`, not acted on). The `Widget` layout props (`width`, `height`, `min-*`, `max-*`,
`margin`, `flex-*`, `align-self`, `position`, `top/left/right/bottom`) are CSS-only on presto (Q6) and not
listed.

| Kind | Props | Signals |
|---|---|---|
| every kind (`Widget`) | `anchor`, `anchor-scope`, `invalid`, `dialog-role`, `status-tip`, `status-section`, `drag-data`, `drag-type`, `drop-types`, `source-loc` (runtime-set, dev builds) | `drag-begin`, `drag-end`, `drag-enter`, `drag-leave`, `drop` |
| `button` | `default`, `cancel` | |
| `label` | `selectable`, `selection-start`, `selection-end`, `for` | |

`focus-scope` on `box`/`frame` maps onto core's common `focus-scope`. Every other hosted kind
(`toggle-button`, `checkbox`, `radio-button`, `switch`, `progress-bar`, `spinner`, `separator`, `icon`, `image`,
`hyperlink`, `stack`, `sides`, `grid`, `frame`, `box`) takes all its own surface props. Placeholders
(`window`, `slider`, `text-edit`, runs) are not listed: they are not presto-widgets' yet.

Decided (coordinator, 2026-10-25): Q7 presto-widgets' label will take inline run children (WIDGETS-BASIC); the explicit error stays until then, then runs map onto it. `surface/tier0.rs` stays the JS-facing table. Q5 publish core's `tabindex`/`focus-scope` on `Widget` later, not now; Q6
as above.

## Not done

- **`svelters-presto-run`**: `crates/presto` and `crates/presto-test` are still one-line stubs in `ci/presto-base`
  (APP-TEST not merged). The `Driver` trait is the seam; `PrestoBackend::with_text` takes presto-text's system.
- **Q3**: when CORE's `KindSpec` has docs, methods, named args, nullable and a11y name sources, `surface/tier0.rs`
  goes and `spec.rs` reads the registry; presto-widgets' remaining Tier 0 kinds replace the placeholders as they
  land (add each to `kinds::register_hosted`; members it lacks fall to `Side` automatically).
- `scroll-into-view`, `bounds`, native animations (`SVL_STYLE_ANIMATE` answers 0), colour scheme and TR1 channels
  into core, `host.instantiate` ABI wiring, `presto.d.ts`, provider kinds, CV1 panes: as before.
- `oplog_replay.rs` still produces native lines through the backend (not presto-test's `Harness`, not landed).

## Questions for the coordinator (each with a recommendation)

- **Q5 Common props without a surface name.** core's `tabindex` and `focus-scope` have no egui counterpart, so
  presto.json does not publish them. *Recommendation:* publish them on `Widget` under core's names (they are
  DOM names already), once a cadatomic panel needs them.
- **Q6 Hosted kinds' missing props.** presto-widgets' `button` lacks egui's `default`, `cancel` and the `Widget`
  layout props (`width`, `margin`, …); they are `Side` (stored, not drawn). *Recommendation:* layout props become
  CSS-only on presto (the engine already has them) and WIDGETS lanes add the behaviour props to their kinds.

## Hand-off (next session, in order)

1. When APP-TEST's `presto::App` is in `ci/presto-base`: `src/bin/svelters-presto-run.rs` implementing `Driver`
   (open windows for `windows()`, feed input through `PrestoBackend::dispatch`, `pump` after frames, presto-text
   via `with_text` and `presto_widgets::host::install_text_system`).
2. `bounds` and `scroll-into-view` on attached windows (layout runs in `Document::frame`).
3. As presto-widgets lands kinds: add them to `kinds::register_hosted`, regenerate presto.json.
4. When CORE's `KindSpec` grows (Q3): drop `surface/tier0.rs`, generate from the registry.

## Not run

Session 1: everything except `tests/spec.rs` (once, with `SVELTERS_UPDATE=1`). Session 2: `cargo check -p
svelters-presto --all-targets` and `tests/spec.rs` with `SVELTERS_UPDATE=1` only; every other test is written,
not run. CI is the gate.
