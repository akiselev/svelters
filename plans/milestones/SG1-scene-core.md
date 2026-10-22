# SG1: the scene core of `crates/svelters-egui`

Design: `plans/research/ARCH-SCENE.md` §4–§7, §11–§14, with the cadatomic review items R16 (the SG1 part), R17 and
R18 from its "Decided" section. Starts from EG0 (`plans/milestones/EG0-module-split.md`, master `aac6a8aa`). EM1 and
RA1 build on it; SG2 and SG3 come next.

## What landed

| File | Content |
|---|---|
| `scene/id.rs` | `NodeId(NonZeroU64)` (= the ABI handle: `to_raw`/`from_raw`, `index`, `generation`), `SlotTable` (O(1) `len`), `VNodeId`, `Target`, `KindId`, `PropId`, `RootIx` (`DETACHED`), `Tick`, `ProviderId` (interned names, `ProviderId::named("cad.pick")`). **Generations retire at 2^28** (the handle-collision fix): no node handle ever has bit 52, the host's app-export bit. |
| `scene/node.rs` | Columns: `NodeCore` (104 B: parent, kind, root, flags, dirty, owner, state, listen mask, explicit mask, egui id, rect, drawn, changed tick), `LayoutBox` (taffy style, cache, layouts, intrinsic), `Payload` (`Builtin(W)`, `Vacant`), `NodeCold` (boxed lazily: `Common`, `Visual`, observe counts, listen overflow). Bit sets `NodeFlags`, `Dirty`, `StateBits`; `Owner`. `Node<'a>` is now a borrowed view (pre-SG1 read paths). |
| `scene/mod.rs` | `Scene` (= `Arena`): the columns, root table, tick, counters, announcement queue; `Phase`; `validate`; compat reads (`get`, `n`, `handles`, `len`); `member_bit` (per-kind ordinals for the listen and explicit masks). |
| `scene/tree.rs` | One path for create/insert/remove/destroy/free_subtree from every origin: kind checks (moved from `ops.rs`), owner checks (S3), cycle check, root cache (O(subtree) when a subtree changes root), root table (`TreeRoot::Window`), the attach hook `Scene::attached`. |
| `scene/dirty.rs` | Content vs style invalidation up to the nearest **layout boundary**; per-root `RootDirty { relayout, remeasure, full }` (replaces the global `layout_dirty`, `remeasure` and `layout_size`); `mark_measure` (dedup by flag, fixes the duplicate remeasure entries). |
| `scene/access.rs` | `SceneHandle` (`read`, `try_read`, `write`, `defer`, `deferred_len`, `phase`), `SceneRef`, `NodeRef`, `Children`, `Descendants`, `SceneMut`, `NodeMut`, `Origin`, `SceneError`, `SceneBusy`, `PropError`; `Scene::set_value` (the one prop write path), `set_visual`, `observe`; `EguiBackend::scene()`. |
| `scene/keys.rs` | `PropKey<T>`, `PropValue`, built-in keys: `keys::{widget, container, window, label, button, checkbox, text_edit, slider}::*`. |
| `scene/dump.rs` | `SceneRef::dump(top, DumpOpts)`, `SceneStats` (live, roots, per tag, bytes estimate, layouts, boundaries laid out, leaves measured, leaf layouts, nodes placed, writes, busy writes, deferred, announced). |
| `layout.rs` | Adapter over the columns; `style_of` (was `restyle(&mut Node)`); `run_root` (whole root when new/resized/top changed, else each queued boundary at its current size, then `round_from` + `place` of that boundary only); per-root `remeasure`; iterative `place` without child-list clones. |
| `walk.rs`, `leaf.rs` | Walk by slot index, children by index (no per-frame clones); per-root layout in `window_ui`. |
| `frame.rs` | Phases (R17), deferred writes at the start of logic (§7.2 step 2b), Rust announcements flushed to `prop_changed` before and after the checkpoint, per-root remeasure, `layout_now` per root. |
| `ops.rs` | Every op through `Scene` with `Origin::Js`. |
| `scene/props.rs` (WIDGETS file, small edit) | `set_prop(c, w, def, v)` returns `Dirty` bits; `is_common`. |
| RA1 integration (merged master `12f34937`) | RA1's `StateBits` is `NodeCore::state` (8 B, hot); RA1's `ExtBlock` is the scene's `ext` column (`Vec<ExtBlock>`, one per slot, empty and allocation-free on most nodes) until SG2's per-key columns; `W::Ext(ExtWidget)` is a `Payload::Builtin` variant; `provider::paint` takes the walk's slot index and borrows the payload and the ext column disjointly; `provider::set_prop(w, i, v)` returns `Dirty::MEASURE` for `measure` props. Accessors moved to the scene API: `NodeRef::{ext, ext_block, widget(f)}`, `NodeMut::{ext_mut, set_state(impl IntoStateBits)}`; the pre-SG1 `Node` view keeps the readers (`ext`, `ext_block`, `widget`, `state`). `examples/egui-kinds/tests/kinds.rs` uses `SceneHandle` instead of the removed `scene::new_node`. |

Tests: `scene/tests.rs` (unit: random op interleavings with a reference model, `validate` after each op and a
layout oracle; the 11k-node counters; phases and defer; ticks and typed reads; key names), `scene/id.rs` (packing,
retirement), `scene/node.rs` (`size_of::<NodeCore>() ≤ 112`, it is 104), `tests/scene.rs` (R18 announce without
events, owner rules with a runtime's tree, defer across frames, boundary relayout counters and `bounds`).

## The API cadatomic gets (all `svelters_egui::scene::*`)

- `EguiBackend::scene() -> SceneHandle` (`Rc`, UI thread). `read(|s: SceneRef| …)` in any phase (outside a running
  mutation), `write(provider, |s: &mut SceneMut| …) -> Result<R, SceneBusy>` in `Idle`/`Logic`, `defer(provider, f)`
  anywhere (runs at the start of the next logic), `deferred_len()`, `phase()`.
- Reads: `SceneRef::{get, get_raw, len, tick, roots, root_top, nodes, of_tag, stats, validate, dump}`;
  `NodeRef::{id, kind, tag, owner, root, parent, children, descendants, ancestors, egui_id, rect, drawn, state, flags,
  is_layout_boundary, is_visible, changed, changed_since, get(&PropKey), value(PropId), value_of(name), prop_id, text,
  listens, observes}` (`dirty`, `widget`, `common` are unstable).
- Writes: `SceneMut::{create, create_tag, insert, remove, destroy, splice, node, batch}` (structure: Rust- and
  Host-owned only, S3); `NodeMut::{set(&PropKey, v), set_value, set_by_name, set_state, set_flags (LAYOUT_BOUNDARY,
  HIT_TRANSPARENT, SKIP_OFFSCREEN), invalidate}`.
- R18: `NodeMut::set` on an observed prop queues `prop_changed` (only if the value changed), flushed at the next logic
  with a checkpoint; it never posts the kind's events.

## Deviations from ARCH-SCENE, and why

- **`scene::keys`, not `scene::props`**, for the built-in `PropKey`s: `props.rs` is the WIDGETS lane's
  value-conversion file. §16's stable list should read `scene::keys::*`.
- **`PropKey<T>` reads through `fn(&Common, &W) -> Option<&T>`**, and writes convert through `PropValue` into the exact
  `set_value` path (one write path, as §6.4 asks). RA1 widens the accessor to the provider payload.
- **Phases are `Idle`, `Logic`, `Ui`** (R17 merged Prepare/Layout/Walk into `Ui`). The legacy `EguiBackend::ui` is the
  whole frame, so it returns to `Idle` when it returns; EM1's `Embed` keeps `Ui` until egui's end of pass.
- **Boundary rule** (§7.1 lists "definite width and height"): also `flex-shrink: 0` or definite min sizes (the
  automatic minimum size of a flex item is content-based), not baseline-aligned (own `align-self` or the parent's
  `align-items`), no TR1 clip. Plus `NodeFlags::LAYOUT_BOUNDARY` and root tops. The *behaviour* is what §16 promises.
- **A whole-root run does not relayout queued boundaries separately**: it clears the caches from each queued boundary
  to the top, then lays out once.
- **Journal, `Change`, observers, ext columns, `ChangeFilter`** are SG2; `SceneMut::create_widget`, `attach`,
  overlay roots, `register_*` are SG4; `by_anchor`/`Query`/`hit_test`/`Cursor` are SG3. `of_tag` is a slot scan until
  SG3's kind index.
- **`explicit` and `listen` are exact masks** over per-kind member ordinals (`member_bit`); listen ordinals ≥ 64 go to
  `NodeCold::listen_overflow`. The "Rust write to an explicit unbound prop" debug diagnostic is not implemented.
- **`Owner::Rust` destroy** rejects a subtree with any runtime-owned node or a runtime-owned parent: a Rust panel a
  runtime mounted stays the runtime's child until the runtime removes it.

## Bugs fixed on the way

- A visible node under a hidden (`display: none`) container reached taffy's leaf code with `PerformHiddenLayout`
  (`unreachable!`). Latent on master; the property test found it. `compute_child_layout` now honours the hidden run mode.
- Handles with generation ≥ 2^28 would have collided with app-export handles (§2).
- Duplicate remeasure entries (§2) and the global relayout of every presented window on any change.

## Notes for the next lanes

- **EM1** (landed on master first; merged here, see "The EM1 merge" below): slot roots, the attach hook's id
  derivation, per-root slot layout, R16 in `Busy` and R17's phases on `Embed::logic`/`show_slot` are done.
- **RA1** (landed first; merged here): provider widgets stay `W::Ext` inside `Payload::Builtin` (a separate
  `Payload::Widget` variant is a later cleanup, not needed by any caller). The scene reads kinds only through the
  registry (`kinds::kind(k)`, `KindRegistry::global().indexes()` for the `member_bit` table, `provider(k)` in
  `builtin_payload`). `PropKey` has no provider-payload accessor yet: provider props are read with
  `NodeRef::widget(|w: &MyWidget| …)` and written with `NodeMut::set_by_name`/`set_value` (the exact path).
- **SG2**: every mutation already stamps `NodeCore::changed` and goes through `tree.rs`/`set_value`/`set_visual`/
  `NodeMut::{set_state, ext_mut}`: the journal hooks go there, with `Origin` already passed. `Counters` has room for
  journal records. Layout `Damage`: `run_root` returns the boundaries it laid out. Ext storage: replace
  `Scene::ext: Vec<ExtBlock>` with per-key columns behind the same `NodeRef::ext`/`NodeMut::ext_mut`/`WidgetCx::ext`
  calls (`provider::paint` hands `&mut scene.ext[i]` to `WidgetCx`; that is the one place that needs a new shape),
  and add ext change ticks. Widget-side state changes in `provider::paint` stamp the node's tick.
- **SG3**: `of_tag` and `SceneRef::nodes` scan slots; the kind index belongs in `Scene::create`/`free_slot`.
  `Descendants::skip_subtree` exists; `Cursor`, `Query` and anchors do not.
- **SG6**: `LayoutBox` is ~1 KB (inline taffy style and cache); `SceneStats::bytes` is the measure. Wrapping labels
  re-shape their text for every min-content query (`Metrics::wrapped` at width 0), which dominates a debug layout of
  10k labels (the 11k test uses `wrap-mode: extend`).

## Verification (this lane, local)

- `tools/dev/cargo-slot cargo check -p svelters-egui --all-targets` and `--no-default-features --lib`: clean.
- `tools/dev/cargo-slot cargo nextest run -p svelters-egui` (fixture bundles built with `pnpm --filter
  "./fixtures/*-egui" build` after `cargo build -p svelters-compiler --bin svelters-compile`): 29/29, every existing
  test and op-log replay unchanged. After merging RA1: `cargo check --workspace --all-targets` clean, and
  `nextest run -p svelters-egui -p egui-kinds-example` 34/34 (the egui-kinds app bundle built with `pnpm --filter
  ./examples/egui-kinds/app build`).
- `SG1_SEEDS=400` run of the property test: passes (1111 boundary relayouts, 27115 root layouts, each checked against a
  from-scratch layout).
- Not run locally: `--workspace` builds and tests (nothing outside this crate uses its Rust API), conformance, the
  other backends' suites. CI runs them.

## The EM1 merge (resolved)

`ci/sg1` was green at `ca26017a` (SG1 + RA1). `01b64aae` merged master `ff04be32` (EM1) with the conflicts still
marked; the next commit on `ci/sg1` resolves them (a new commit, no reset). Both lanes' intent is kept: EM1's
`Embed` API and behaviour are unchanged, and its scene additions became SG1 columns and hooks.

| EM1 (pre-SG1 `Node`) | Now |
|---|---|
| `Node.root: u32` (the embed's `RootId`, set by `ids::adopt`) | `NodeCore::root: RootIx` into the root table. A slot is `TreeRoot::Slot { node }`, created with the slot (`Scene::add_root`) and ended by `Scene::free_subtree` when the slot is destroyed. The mounted root's id base is `RootEntry::base` (`Scene::set_root_base`, set by `do_mount`, cleared by `do_unmount`). `Embed::root_of_handle` maps node → `RootIx` → `TreeRoot::Slot` → slot → `Slot::root`; `anchored` scans the slot root's nodes. |
| `Node.host_owned` | `Owner::Host` (`Scene::create(box, Owner::Host)`); `handle_info` answers `SVL_LIFE_BORROWED` for it. |
| `Common.anchor` | unchanged (cold column), plus `P::Anchor` in `props::is_common` (it was missing: the write went to a throwaway `Common`) and `keys::widget::ANCHOR`. |
| `ids::adopt` (insert op), `ids::reanchor` (anchor prop), `EguiBackend::root_bases` | SG1's attach hook `Scene::attached(id, r)`: `rooted_id(base, handle, anchor)` when the root has a base, else the unsalted `node_id`; host-owned nodes keep the id the embed gave them. `Scene::set_value` re-derives on `P::Anchor`; `set_root_base` re-derives the root's nodes. `root_bases` is gone. |
| `layout_size`, `n.cache.is_empty()`, global `remeasure`/`layout_dirty` | `walk.rs::slot_ui`: `layout::remeasure` (if the slot root queued leaves) + `layout::run_root` when the size changed or the root has queued work; `window_ui` shares the `layout_root` helper. Content size from the children's `NodeCore::rect`. |
| `tree::detach` + `tree::free_subtree(&mut Arena, Handle)` | `Scene::free_subtree(id)` (unlinks, ends a root, returns pre-order ids; `to_raw()` for `on_destroyed`). |
| `a11y::name` in `leaf.rs` and `provider::paint` | `a11y::decorate(resp, &arena.n(h))` in both (anchored provider nodes get their author id too). |

**R17 on EM1's split frame.** `EguiBackend::logic_pass` sets `Logic` (then deferred writes, announcements,
closes, checkpoint, rAF, remeasure). `Embed::logic` sets `Logic` at its start and `Ui` at its end; a second
call in the same pass, or a later pass of the frame, sets `Ui` and returns. The `Ui` phase ends at the end of the
egui pass: `SceneHandle::hook_passes(ctx)` (from `Embed::new` and every `Embed::logic`, once per context)
registers `on_begin_pass`/`on_end_pass` callbacks on a shared pass clock (`Arc<PassClock>`, atomics, because
egui's callbacks are `Send + Sync`). The outermost pass ending sets `Idle`; a root-viewport pass starting at depth 0
sets `Idle` (what a logic-only frame or a missed hook left); a deferred viewport's outermost pass runs in `Ui`;
an immediate viewport's pass nests and changes nothing. A logic-only frame (`run_logic`, no pass) ends `Embed::logic`
in `Idle`. The legacy `EguiBackend::ui` keeps its own whole-frame phases (`Ui` after logic, `Idle` when it returns).
The first frame of an embed created on another context (the runners' `TestApp`/`run_app`) has no hooks yet: its
logic ends in `Idle`.

**R16.** `embed/settle.rs::scene_busy`: `SceneHandle::deferred_len() > 0` or a non-empty announcement queue
(`SceneHandle::announcements_len`) sets `Busy::CHANGES`; `after_pass` already requests the repaint for deferred
writes. `Busy::LAYOUT` is a visible mounted slot not drawn yet or whose root has queued work (`RootDirty::is_empty`),
or any root with leaves to remeasure. `Busy::PROVIDER` stays reserved (state providers are SG4/R23).

API changes against the two records: none to `Embed`, `EmbedHarness`, `TestApp` or the `scene::*` API listed above.
Added: `TreeRoot::Slot`, `keys::widget::ANCHOR`, `SceneHandle::announcements_len`. Removed (crate-private):
`EguiBackend::root_bases`, `ids::{adopt, reanchor, id_of}`.

## Notes for SG2, SG3, EM2

- **SG2**: `Scene::attached` and `set_root_base` change egui ids (journal them with the node's tick if a consumer
  needs id changes). `set_root_base` walks the whole root; it runs only on mount and unmount.
- **SG3**: `Embed::anchored` is a slot scan filtered by `NodeCore::root`; the per-root anchor index of §4.5
  (`RootEntry::anchors`) belongs in `attached` and the `P::Anchor` arm of `set_value`, with `by_anchor` on top.
- **EM2**: repaint routing can read node → `RootIx` → `TreeRoot::Slot { node }` → slot → viewport. Deferred
  viewports already run in `Ui` (the pass hooks); `show_viewport_passes` (R21) goes after the walk in that phase.
  `RootEntry` has no viewport or visibility yet: they live in EM1's `Slot` (§4.5 lists them on the root entry).

## Verification (the EM1 merge, local)

- New test `tests/embed.rs::slot_roots_phases_and_deferred_work_follow_the_embed_frame` (slot = `Owner::Host`
  top of a `TreeRoot::Slot`, the root's nodes in it; R17 `Ui` + `SceneBusy` in the app's ui, `Idle` after the
  pass; R16 `CHANGES` for a deferred write until the next logic).
- `tools/dev/cargo-slot cargo check -p svelters-egui --all-targets` and `cargo check --workspace --all-targets`:
  clean (pre-existing warnings in other crates only).
- `tools/dev/cargo-slot cargo nextest run -p svelters-egui -p egui-kinds-example`: 41/41 (every SG1 test, the
  property test, the 11k-node one-boundary test, `node_core_fits_112_bytes`, the egui tests and op-log replays,
  EM1's 7 `tests/embed.rs` tests and RA1's `egui-kinds` tests unchanged), then the new embed test (fixtures built
  with `pnpm --filter "./fixtures/*-egui" build` and `pnpm --filter ./examples/egui-kinds/app build` after
  `cargo build -p svelters-compiler --bin svelters-compile`).
- Not run locally: workspace tests, conformance, the JS packages' tests, the other backends. CI runs them.
