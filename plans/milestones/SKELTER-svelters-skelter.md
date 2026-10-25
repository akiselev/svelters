# SKELTER-svelters-skelter: the svelters backend on skelter (lane record)

Lane SVELTERS-SKELTER of batch presto-b2 (`plans/cloud/presto-b2/LANE-svelters-presto.md`), branch
`ci/presto-svelters-presto`, session `session_01497YqbhMLBiY7YWGtiLKAX`, 2026-10-25. Handed off at the
context limit (see "Hand-off" below); the branch builds (`cargo check -p svelters-skelter --all-targets`)
after merging `ci/presto-base` at 7a947d45 (CORE, TEXT, RENDER, COMPOSITOR, FRAME-ANIM skeletons).

## What landed (`crates/svelters-skelter/**`, session 1; `ids`/`wire`/`shim` superseded in session 2 below)

| Module | What |
|---|---|
| `backend.rs` | `SkelterBackend`: `impl svelters_abi::Backend` (resolve, create, insert with slots, remove, destroy, set/get typed by id, call `present`/`close`, listen and `listen_ex` with capture and `LISTEN_PASSIVE` = 4, handle_info with borrowed slots, run through a `Driver`, waker, quit, schedule_wake, request_frame, style, observe with nesting counts, start, describe). Handles are the backend's own, counted up and never reused, so a template's nodes are `base + k`. The `Driver` trait is the seam to whoever runs the loop (the skelter app, a harness). |
| `deliver.rs` | Event-time delivery of a `Dispatched`: observed native changes first (`prop_changed`), events in order, cancelable/sync ones through `call_sync` (its `true` cancels the default), one closing checkpoint. `pump()` delivers native output produced outside input. |
| `template.rs` | `Template`/`TemplateNode` (decoded table), `instantiate` (contiguous handles, CF2 order, roots left to the caller, rollback on failure), `destroy_range`, `Templates` (content-hash registry with refcounts). |
| `style.rs` | `style` op: `TARGET` → 4 (engine), sheets/classes/id/inline/scheme into the tree, TR1 visual channels (opacity, transform, clip, inert); `ANIMATE` answers 0 so the runtime plays animations itself until skelter-motion is wired; `DECLS` refused. |
| `ids.rs`, `ids.txt`, `wire.rs` | Stable wire ids: an append-only table (one row per kind/prop/signal/method/slot, keyed by the declaring kind, so inherited props share their base's id), FNV-1a fingerprints per member, a spec hash. See Q1: skelter-core chose hash ids instead. |
| `spec.rs`, `css.rs` | The `skelter.json` generator (svelters-semantics/v0 like egui.json, plus `id`/`fingerprint` per kind and member, `ids.spec_hash`, `change` per prop and per engine CSS property, `passive`/`cancelable` per signal, `a11y` with `name_required`, `native_state`, `slot_ids`, `capabilities: ids templates native_transitions native_motion css_precompiled`). `css.rs` is skelter's engine property table with change classes (egui's list plus `transform`, `transform-origin`, `visibility` as property-class). |
| `embed/` | The embed model minus egui: `Embed` (config, budgets, fault policy, load/load_from/load_app, slots as borrowed `box` nodes, mount/props/unmount/move/close/restart queued and applied in `logic()`, root info sync, faults per root, `settled()`), `roots.rs`, `slots.rs` (window/size/scale/focus reported by the app: `set_slot_layout`, `set_slot_focused`), `settle.rs` (egui's `Busy` plus `COMMIT_IN_FLIGHT`, set with `set_commit_in_flight`). Panes (CV1) not ported. |
| `value.rs` | ABI `Value` ↔ prop values by type (numbers across int/float, enums by name, `#rrggbbaa` colours, lists, records). |
| `shim/` | Stand-ins (below). |

Generated: `metadata/semantics/skelter.json` (from `builtin_registry()`), `crates/svelters-skelter/ids.txt`.
Staleness: `tests/spec.rs` (`SVELTERS_UPDATE=1 cargo test -p svelters-skelter --test spec`).

Tests (written, **not run**, except `tests/spec.rs` run once with `SVELTERS_UPDATE=1` to generate the
spec and the id table): `tests/ops.rs` (the ABI through the real vtable), `tests/oplog_replay.rs` (every
Tier 0 egui fixture's `expected.oplog` — counter, if-toggle, keyed-list, motion, timers — replayed on
skelter: ops succeed and read back; `event`/`changed` lines produced natively through the tree's
`emit`/`native_set` and delivered once, `changed` exactly when observed), `tests/template.rs`,
`tests/deliver.rs`, unit tests in `value.rs` and `style.rs`.

## Cross-lane edits

| File | Why |
|---|---|
| `crates/svelters-compiler/src/semantics/backend.rs`, `kinds.rs` | `SpecBackend::builtin("skelter")`; skelter in `every_builtin_spec_builds` (lane brief). |
| `crates/svelters-modules/src/source/transform.rs` | `styles()` checks the spec's `style.engine == "backend"` instead of `backend == "egui"`; the warning text names both engines. |
| `crates/svelters-host/tests/modules.rs` | Follows that warning text (lane RUNTIME's crate; one assertion string). |
| `packages/native-runtime/vite.js` | `runsCssEngine(backend)` reads the spec's `style.engine` (two `'egui'` checks). |
| `packages/testhost/src/index.js` | Same check for the default `styleDialect`. |
| `tools/metadata/style/gen_props.py` + `packages/style/src/props.generated.js`, `crates/svelters-style/src/tables.generated.rs` | skelter in `TOOLKITS` (regenerated: a `SKELTER` table in `tables::ALL`). Rerun whenever skelter.json's style section changes. |
| `tools/metadata/style/gen_utilities.py` | `--report` lists the utility rows skelter's style table cannot apply (none today); outputs unchanged. |

## Session 2 (2026-10-25): the port to the real skelter-core

Session `session_012kp44HgxNR751yz6GvNAH5`, after merging `ci/presto-base` at ac7cc8c4 (real skelter-core,
skelter-text, the skelter-widgets substrate). Coordinator answers applied: Q1 (skelter-core's FNV-1a ids on the
wire, `ids.txt` dropped), Q2 (egui's JS names onto core's DOM names), Q3 (keep the side table until CORE's
`KindSpec` grows), Q4 (pending: CORE adds `Document::attach_window`).

| Module | What changed |
|---|---|
| `kinds.rs` (new) | `Kinds`: the JS-facing kinds on one `skelter_core::KindRegistry`. skelter-widgets' kinds are registered through `skelter_widgets::host::register` (`button` today); every other concrete Tier 0 kind gets a core kind synthesized from its surface spec (`Placeholder`: real props, signals, slots and role, draws nothing) until its widget lands. Per member a map: props `Native(PropId)` / `Inverted(PropId)` (`enabled`↔`disabled`, `visible`↔`hidden`) / `Side` (a surface prop the skelter-widgets kind does not take yet, kept by the backend); signals the kind's, else core's common one (`click`, `keydown`), else none (never fires: `drag-*`); slots (first surface slot = core's default). Wire ids: skelter-core's hash of (declaring kind, member, type), the declaring kind being the tag, `*` for `Widget` (core's common pseudo-kind) and the lowercased name for `Run`; so own native members have their core id, inherited ones one id on every kind. |
| `surface/` (was `shim/`) | Only the Q3 side table now: the surface `KindSpec` (docs, methods, named signal args, nullable, a11y name sources, base chain), `Value`/`ValueType`/`DefaultValue`, `tier0`, plus `ColorScheme`/`Visual` (inputs core does not take yet). The shim `Document`, registry, ids, errors and events are gone. |
| `backend.rs` | On `skelter_core::Document` (`Document::new(registry, text)`; `NoText` until the runner installs skelter-text). `free` detaches JS-known children before `destroy` (core destroys subtrees; the ABI leaves children to the host). Writes during delivery run in `js_transaction(seen)` (§4.5; `Superseded` is not an error). `request-focus` calls `Document::focus` when the node is in a window (none are until Q4). Side stores: `Side` props, per-node `StyleInput`, visual channels, colour scheme, sheets by name (`add_sheet`/`remove_sheet`). |
| `deliver.rs` | `Output` (events + native changes + revision) from `Document::dispatch`, `take_events` and `drain_changes` (`Change::Prop { native: true }`); `dispatch(&InputEnvelope)`, `pump`, `queue`, `target_event`. Events go along skelter-core's paths (capture/target/bubble, `target`/`phase` args, record fields by the surface arg names); `call_sync` answers egui's bits (1 stop, 2 prevent, 4 immediate) or `true`. |
| `value.rs` | `to_core`/`from_core`, `signal_arg` (surface args → core's one arg), `signal_args` (core arg → JS args). |
| `spec.rs` | ids/fingerprints from the hashes (`fingerprint` = the id in hex), `ids.scheme`, `native` = core's name (Q2), `widget` per kind (`skelter-widgets` / `placeholder`). Style section unchanged, so `gen_props.py` was not rerun. |
| `ids.rs`, `wire.rs`, `ids.txt` | Deleted (Q1). |

Regenerated `metadata/semantics/skelter.json` (`SVELTERS_UPDATE=1 cargo test -p svelters-skelter --test spec`, the
only run: 4 passed). Tests updated (written, not run): `tests/common` queues native output as skelter-core
reports it (`target_event`, a set plus a reported change), `tests/spec.rs` checks the hash ids and Q2 names
instead of the table, `kinds.rs` has a unit test.

### Session 2, second part: skelter-base e58a6d8a (CORE-2's additions, skelter-widgets' Tier 0)

- Merged `origin/ci/presto-base` at e58a6d8a. skelter-widgets now has 17 Tier 0 kinds; all are registered
  (`kinds::register_hosted`). Still placeholders: `window`, `slider`, `text-edit` and the inline runs. A surface
  member a hosted kind lacks falls to `Side` (props) or never fires (signals).
- Synthesized core specs now carry CORE's new fields: prop `nullable`/`doc`, signal `args`/`doc`, `methods`
  (`ArgSpec`s), `name_from` (egui's sources mapped to `NameSource`), kind `doc`. Method wire ids are
  `MethodId::of(decl, name, ret)`.
- Q4 wired: `present` attaches the JS-built `window` node with `Document::attach_window(WindowId(handle),
  node, default size, scale 1)`; freeing a window root closes its window (`close_window`), and
  `window_closed` frees the handle. The driver resizes with the real size when it opens the OS window.
- A hosted kind's own methods go through `Document::call` (their events are queued for `pump`).
- skelter-widgets' `label` is a leaf, so inserting runs (`<label>a <b>b</b></label>`) into it now fails with
  an error that says so. No Tier 0 fixture does it. **Q7** (recommendation): skelter-widgets' label takes
  run children (its `rich` module), or skelter-widgets keeps a `rich-label` kind that svelters-skelter maps a
  label with runs onto.
- Not done: dropping `surface/tier0.rs` (Q3). Core's `KindSpec` has no base chain and skelter-widgets' names
  are core's, not egui's, so the JS-facing surface still comes from the table. Once WIDGETS names follow Q2,
  `spec.rs` can generate from the registry plus a `Widget` table.

### Missing behaviour props and signals per hosted kind (coordinator Q6, for WIDGETS-BASIC/TIER1)

The surface (egui) members skelter-widgets' kinds do not take yet, from `Kinds::builtin()` at 7546a2a6+ (stored
by the backend as `Side`, not acted on). The `Widget` layout props (`width`, `height`, `min-*`, `max-*`,
`margin`, `flex-*`, `align-self`, `position`, `top/left/right/bottom`) are CSS-only on skelter (Q6) and not
listed.

| Kind | Props | Signals |
|---|---|---|
| every kind (`Widget`) | `anchor`, `anchor-scope`, `invalid`, `dialog-role`, `status-tip`, `status-section`, `drag-data`, `drag-type`, `drop-types`, `source-loc` (runtime-set, dev builds) | `drag-begin`, `drag-end`, `drag-enter`, `drag-leave`, `drop` |
| `button` | ~~`default`, `cancel`~~ mapped (2dcedb93) | |
| `label` | ~~`selectable`, `selection-start`, `selection-end`, `for`~~ mapped (2dcedb93); `copy(text)` added to the surface (skelter-only) | |
| `slider` | | ~~`submit`~~ mapped (2dcedb93) |

`focus-scope` on `box`/`frame` maps onto core's common `focus-scope`. Every other hosted kind
(`toggle-button`, `checkbox`, `radio-button`, `switch`, `progress-bar`, `spinner`, `separator`, `icon`, `image`,
`hyperlink`, `stack`, `sides`, `grid`, `frame`, `box`) takes all its own surface props. Placeholders
(`window`, `slider`, `text-edit`, runs) are not listed: they are not skelter-widgets' yet.

Slider (2026-10-25, against `ci/presto-widgets-basic` c089c816, merged into this branch): registered through
`host::register_tier0` (all 18 kinds); every slider prop maps, its enums by name (a surface enum value is remapped
to the widget's index on write and back on read, so the two tables may order values differently; a surface value
the widget lacks makes the prop `Side`). Run kinds (against `ci/presto-widgets-basic` 26e57a9c, merged): `span`, `b`, `i`,
`code`, `a` are skelter-widgets' (label is a container folding them, Q7 done); the label-runs error is gone; every
run member maps (`Run`'s `accessible-name` onto core's `aria-label`, as `Widget`'s). Placeholders left: `window`,
`text-edit`.

Decided (coordinator, 2026-10-25): Q7 skelter-widgets' label will take inline run children (WIDGETS-BASIC); the explicit error stays until then, then runs map onto it. `surface/tier0.rs` stays the JS-facing table. Q5 publish core's `tabindex`/`focus-scope` on `Widget` later, not now; Q6
as above.

### CORE-3's common members and sync delivery (2026-10-25)

- `Widget` props now map onto a common prop of the same name when its type fits (enums by name): with CORE-3's
  (`ci/presto-core-3` eb669f1e) `anchor`, `anchor-scope`, `invalid`, `dialog-role`, `status-tip`,
  `status-section`, `drag-data`, `drag-type`, `drop-types` reach the tree, and the `drag-*`/`drop` signals map
  through the existing by-name common-signal fallback. Until CORE-3 is in `ci/presto-base` they stay `Side`
  (name-based, no code change needed when it lands; regenerate skelter.json then).
- **Q8, final (coordinator, 2026-10-25): skelter-core's lent document** (`SyncDelivery`, core-3 d39eaf4b, base
  b387ef42). `sync.rs`: the backend installs the deliverer at construction; for its call it points
  `SkelterBackend::doc`/`doc_mut` at the `&mut Document` core lends (guards with `RefCell`'s borrow rules,
  checked), so listeners read the live tree, write in `js_transaction` at the event's revision and build nodes
  directly; listener nodes gone by their turn are skipped; `call_sync` answers egui's bits (1 stop, 2 prevent,
  4 immediate) or `true`. The interim op queue, the "reads fail" path and `take_deferred_errors` are gone.
  Tests in `tests/deliver.rs` (written, not run); the lent path itself runs in `tests/run.rs`'s click (pointer
  events are cancelable).

### MOTION-CSS's skelter side (2026-10-25, merged ci/presto-motion-css 4a4374fe + ci/presto-base)

- `motion.rs`: `SVL_STYLE_ANIMATE` parses `skelter_motion::op::AnimateOp`; the plan is the `native`
  descriptor's when the compositor can play all of it (opacity, transform, clip; no statics, no
  `overflow: hidden`), else the op's channel frames; each track starts with skelter-core's new
  `Document::start_animation`, `cancel` cancels them; when the compositor reports the op's last track
  finished or aborted (`SkelterDocument::animation_event` → `SkelterBackend::animation_event`), `pump` posts
  `animationend` with the op id. Always answers 1 (the runtime stops offering native animations to a host
  that refuses one); a node in no window ends at once. The frames' transform origin is the node's own.
- `SVL_STYLE_SHEET_BIN` (12): `binary::decode(from_base64(..))` → skelter-core's new `add_parsed_sheet` and
  `preload_utilities`, replacing sheet `id`. Rule hints are unused (need COMPILER's template-node map).
- Capabilities: `native_transitions native_motion css_precompiled` were already in skelter.json's
  `capabilities` (unchanged), which the compiler now reads.
- **Cross-lane edit (skelter-core, additive):** `Document::start_animation(node, AnimProperty, Generator) ->
  Option<AnimationId>`, `Document::cancel_animation(id)` (frame.rs), `Document::add_parsed_sheet`,
  `Document::preload_utilities` (document.rs, style/mod.rs). For CORE to keep or reshape at consolidation.
- Tests: `tests/motion.rs` (4, **run once, pass**); `tests/ops.rs` follows (a bad ANIMATE payload errors).

### The host's own Document (CADATOMIC blocker 1, 2026-10-25)

- `SkelterBackend::over(doc)` (the built-in kinds) and `SkelterBackend::shared(kinds, Rc<RefCell<Document>>)` (the
  host keeps its handle): the backend's tree is the host's; the registry must hold every concrete kind under
  its tag (else an error naming the kind). `new`/`with_text` stay the convenience (the backend's own document).
  `with_doc` and `SkelterBackend::document()` reach the shared document; the sync deliverer is installed on it.
- `SkelterBackend::adopt(node)` gives a host node a borrowed handle JS mounts into; `release`/`destroy_borrowed`
  forget it without destroying the node. `Embed::adopt_slot(key, node)` makes a host node an embed slot;
  disposing a root or destroying the slot frees only what JS put there (host children without handles stay).
- Tests: `tests/host_document.rs`: adopt/insert/release on a host document and the registry check (**run, pass**);
  `a_svelte_root_mounts_into_a_node_of_the_host_document` (a component compiled for skelter from source, mounted
  into an adopted host `box`; needs `pnpm install`, **not run** here).

## Not done

- **`svelters-skelter-run`**: `crates/skelter` and `crates/skelter-test` are still one-line stubs in `ci/presto-base`
  (APP-TEST not merged). The `Driver` trait is the seam; `SkelterBackend::with_text` takes skelter-text's system.
- **Q3**: when CORE's `KindSpec` has docs, methods, named args, nullable and a11y name sources, `surface/tier0.rs`
  goes and `spec.rs` reads the registry; skelter-widgets' remaining Tier 0 kinds replace the placeholders as they
  land (add each to `kinds::register_hosted`; members it lacks fall to `Side` automatically).
- `scroll-into-view`, `bounds`, native animations (`SVL_STYLE_ANIMATE` answers 0), colour scheme and TR1 channels
  into core, `host.instantiate` ABI wiring, `skelter.d.ts`, provider kinds, CV1 panes: as before.
- `oplog_replay.rs` still produces native lines through the backend (not skelter-test's `Harness`, not landed).

## Questions for the coordinator (each with a recommendation)

- **Q5 Common props without a surface name.** core's `tabindex` and `focus-scope` have no egui counterpart, so
  skelter.json does not publish them. *Recommendation:* publish them on `Widget` under core's names (they are
  DOM names already), once a cadatomic panel needs them.
- **Q6 Hosted kinds' missing props.** skelter-widgets' `button` lacks egui's `default`, `cancel` and the `Widget`
  layout props (`width`, `margin`, …); they are `Side` (stored, not drawn). *Recommendation:* layout props become
  CSS-only on skelter (the engine already has them) and WIDGETS lanes add the behaviour props to their kinds.

## Hand-off (next session, in order)

1. Run `svelters-skelter-run --entry fixtures/counter-egui` under `tools/dev/headless` once a GPU/lavapipe is at
   hand; `close-request` before a user close; text services for `text-edit` (TEXT-INPUT).
2. `bounds` and `scroll-into-view` on attached windows (layout runs in `Document::frame`).
3. As skelter-widgets lands kinds: add them to `kinds::register_hosted`, regenerate skelter.json.
4. When CORE's `KindSpec` grows (Q3): drop `surface/tier0.rs`, generate from the registry.

## Not run

Session 1: everything except `tests/spec.rs` (once, with `SVELTERS_UPDATE=1`). Session 2: `cargo check -p
svelters-skelter --all-targets` and `tests/spec.rs` with `SVELTERS_UPDATE=1` only; every other test is written,
not run. CI is the gate.
