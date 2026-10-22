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

- **EM1**: the attach hook is `Scene::attached(id, root)` (`scene/tree.rs`); move `ids::node_id` there (it is still
  called in `Scene::create`). Slot roots: add `TreeRoot::Slot { .. }` and call `Scene::add_root(kind, top)` /
  `remove_root(r)`; layout per slot is `layout::remeasure(s, m, r)` then `layout::run_root(s, m, r, size)`, the same
  calls `window_ui` makes. Phases: `SceneHandle::set_phase(Phase::Logic)` at the start of `Embed::logic`,
  `run_deferred()` and the announcement flush (see `frame.rs::before_pass`), `Phase::Ui` at its end, `Phase::Idle`
  at egui's end of pass (R17: cover the app's ui between slots and deferred viewports). **R16**: `Busy` lives in your
  `embed/settle.rs`; `SceneHandle::deferred_len() > 0` must count as busy (CHANGES or COMMANDS), and a pending
  announcement queue (`Scene::announce`, non-empty until flushed) as CHANGES. `RootEntry::size` replaces
  `layout_size`; `NodeFlags::PRESENTED`, `WANT_FOCUS`, `HAD_FOCUS` replace the old node booleans.
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

## Handoff: the EM1 merge (in progress)

State: `ci/sg1` was green at `ca26017a` (SG1 + RA1 merge; CI runs 39758049162 macos-appkit, 39758049086 linux-qt,
39758049178 linux-gtk, 39758049103 windows-winui, 39758049123 js). The next commit on `ci/sg1` is a **WIP merge of
origin/master `ff04be32` (EM1) with the conflicts still marked** (the SG1 lane ran out of budget). It does not
build. To finish it, check out `ci/sg1`, resolve, and continue from there (do not reset: amend or add a fix-up commit).

Conflicted (markers in the files): `frame.rs`, `leaf.rs`, `ops.rs`, `scene/node.rs`, `scene/tree.rs`. Auto-merged
but written against the pre-SG1 `Arena`/`Node` (will not compile): `embed/mod.rs` (~38 sites: `scene.alloc` +
`scene::new_node` for slot boxes, `get_mut`, `n.cache`, `n.children.clone()`, `n.root`, `n.c.anchor`,
`tree::detach`/`tree::free_subtree(&mut Arena, Handle)`, `b.0.dirty.remeasure`/`layout_size`), `ids.rs`
(`adopt`/`reanchor` walk `Node.root`), `walk.rs`, `a11y.rs` (`decorate(resp, n)`), `runner/*`, `tests/embed.rs`.

How to resolve, keeping both lanes' intent:
- **Node fields → columns.** EM1's `Node.root: u32` (the slot handle of the embedded root, 0 = none) is SG1's
  `NodeCore::root: RootIx` plus the root table: add `TreeRoot::Slot { node }` and create slot roots with
  `Scene::add_root(TreeRoot::Slot { node }, top)` (end them with `remove_root`). Keep EM1's per-root id base
  (`root_bases`) keyed by the slot handle, reachable from the `RootEntry` (`roots.get(r).top`). `Node.host_owned`
  → `Owner::Host` (`Scene::create(kind, Owner::Host)` for slot boxes; `handle_info` answers `SVL_LIFE_BORROWED`
  for `Owner::Host`). `Common.anchor` stays in `Common` (cold column) with EM1's `P::Anchor` arm in `props.rs`; the
  `Node` view exposes `c.anchor` already.
- **Re-iding.** EM1's `ids::adopt` (insert op) and `ids::reanchor` (anchor prop) belong in SG1's attach hook
  `Scene::attached(id, root)` (`scene/tree.rs`, called for every node entering a root from `set_root_subtree`) and
  in `Scene::set_value` for `P::Anchor`. The hook needs the root's id base: store it on `RootEntry` (e.g. a
  `base: egui::Id` field set by EM1's slot creation) rather than in `Inner::root_bases`.
- **Layout and Busy.** Slot layout = `layout::remeasure(s, m, r)` + `layout::run_root(s, m, r, size)` (what
  `walk.rs::window_ui` does for windows). `Busy::LAYOUT` = the root's `RootDirty` is non-empty
  (`relayout`/`remeasure`/`full`) or its size changed; replace the reads of `n.cache.is_empty()` and
  `dirty.remeasure`. R16: `SceneHandle::deferred_len() > 0` and a non-empty `Scene::announce` count as busy.
- **Slot teardown.** `tree::detach` + `tree::free_subtree` → `Scene::free_subtree(id)` (unlinks, ends the root,
  returns the ids in pre-order; convert with `to_raw()` for `on_destroyed`); `layout_size` is gone (`RootEntry::size`).
- **Frame.** EM1 split `ui` into `logic_pass` (JS) and `show_windows` (walk). SG1's phase calls go there:
  `set_phase(Logic)` + `run_deferred()` + the announcement flush at the start of `logic_pass`/`before_pass`,
  `set_phase(Ui)` at its end, `set_phase(Idle)` at the end of the egui pass (R17: `Embed` should do it at end of
  pass, not at the end of `show_slot`). Keep SG1's per-root remeasure in `before_pass` and EM1's `bind_ctx` and
  public `after_pass` (with SG1's deferred-repaint request).
- **leaf.rs.** Keep SG1's index-based body and replace the `a11y::name` block with EM1's `a11y::decorate(resp,
  &arena.n(h))` (adapt `decorate` to the `Node<'_>` view).
- Then: `tools/dev/cargo-slot cargo check -p svelters-egui --all-targets`, `cargo check -p egui-kinds-example
  --all-targets`, `cargo nextest run -p svelters-egui` (bundles: `pnpm --filter "./fixtures/*-egui" build` after
  `cargo build -p svelters-compiler --bin svelters-compile`), push, all five workflows green.
