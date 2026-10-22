# SG2: extension columns and the change journal (`crates/svelters-egui/src/scene`)

Design: `plans/research/ARCH-SCENE.md` §4.6, §6.3, §6.5, §8 and the §14 SG2 row, with S2(a), S5(a) and the
cadatomic review items R14, R19 and R23 from its "Decided" section. Starts from SG1 + EM1 + RA1 (master
`8ce6d7d0`). Lane: SG2 (SCENE writer), branch `ci/sg2`.

## What landed

| File | Content |
|---|---|
| `scene/ext.rs` | **One column per `ExtKey`** (S2(a)), created on first insert. `Storage::Sparse` (`ExtKey::new`) is a sparse set: slot → packed position, with node ids, values and `ExtTicks` packed; O(1) access, swap-remove, iteration over the entries only. `Storage::Dense` (`ExtKey::dense`) is `Vec<Option<T>>` by slot, with ids and ticks. Columns are `Box<dyn ErasedColumn>` downcast with `Any`, so there is no `unsafe`. `ExtId` is a process-wide id with `ExtId::name()` from a name registry. `ExtTicks { added, changed }`. `ExtEntry` (RA1's API: `get`, `get_mut`, `set`, `remove`, `or_insert_with`, plus `ticks`) stamps the node's tick and the entry's ticks and journals `Ext { Added, Changed, Removed }`. `ExtBlock<'a>` is now a read view of one node's entries (`get`, `ids`, `names`, `len`, `is_empty`). `ExtCx` holds the disjoint scene parts an ext write touches, so a provider widget writes its own node's data while its payload is borrowed. |
| `scene/journal.rs` | `Change` (`Created`, `Inserted`, `Removed`, `Destroyed`, `Prop`, `State`, `Ext`, `Anchor`, `Layout`, `Root`, `Batch`; `Copy`, `#[non_exhaustive]`), `ExtChange`, `RootChange` (`Added`, `Removed`, `Visible`, `Viewport`), `ChangeMask`, `OriginMask`, `Scope` (`Root(RootIx)` / `Subtree(NodeId)`, `From` both; SG3's `Query::within` can reuse it), `ChangeFilter` (scope, what, kinds, ext keys, props, state bits, origins, `ignore(provider)`, capacity; builder methods), `Observer` (`take`, `pending`, `filter`; drop unsubscribes), `ChangeSet` (`records`, `overflowed`, `damage`, `by_node()`), `Damage` (`relayout`, `repaint` per viewport, `a11y`). `Journal`: the union of interests (kinds of change, ext ids, state bits), so a mutation nobody wants costs one mask test. |
| `scene/mod.rs` | `Scene::{ext: ExtColumns, journal: Journal}`; `record`, `record_state`, `ext_cx`, `begin_batch`/`end_batch` (one tick plus a `Batch` label); `validate` checks that ext entries belong to live nodes; re-exports. |
| `scene/tree.rs` | Journals `Created`, `Inserted`, `Removed` (also for each child a runtime's `destroy` orphans), `Destroyed`, `Root { Added, Removed }`. **R19:** freeing a node drops each of its ext entries with an `Ext { Removed }` record before `Destroyed`. `free_subtree_from(top, origin)` (Rust destroys journal as `Rust(p)`; native closes as `Host`). `RootEntry::viewport` and `Scene::root_changed(r, RootChange)` (for EM2, below). |
| `scene/access.rs` | `SceneHandle::{observe, take_changes}`, `SceneMut::observe` (inside a write). `SceneRef::{with_ext, with_ext_ticks, ext_len, changed_since}`, `NodeRef::{ext_ticks, ext_block() -> ExtBlock<'a>}`. `set_value` journals `Prop` (and `Anchor` for the anchor prop). `NodeMut::set_state` journals `State`. `SceneMut::batch(label, …)` uses its label (a nested batch joins the outer one). `write` and deferred writes are batches labelled `write` and `defer`. |
| `scene/dump.rs` | `DumpOpts { state, ext }` (names of state bits and ext keys); `SceneStats::observers`; `bytes` counts the ext columns. |
| `provider/state.rs` | The registry hookup: `StateBits::{is_empty, intersects}`; state changes (built-in and provider bits) are `Change::State { before, after }` records filtered by bit (`ChangeFilter::states`, which takes a `StateBit` too). |
| `provider/mod.rs` | `WidgetCx` holds an `ExtCx` (ext writes from `Widget::ui` are journaled with `Origin::Widget`); `paint` journals the widget's state-bit and `#[prop]` changes. Re-exports `ExtTicks`. |
| `leaf.rs` | A built-in widget's own value change (checkbox, slider, text edit) is a `Prop` record with `Origin::Widget`. |
| `layout.rs` | `place` journals `Layout { before, after }` for changed rects, only when an observer wants `LAYOUT`; `run_root` feeds `Damage::relayout` (the boundaries laid out, or the root's top for a whole-root layout). |
| `kinds/registry.rs` (RUSTAPI file, small) | `chain_iter` over precomputed base indexes; `find_prop`/`prop_def`/`find_signal`/`signal_def`/`slot_of` no longer allocate a `Vec` per call (every prop write did). |

## The observer/journal API cadatomic gets (`svelters_egui::scene::*`)

```rust
let obs = scene.observe(ChangeFilter::ext_of(&PICK_TARGET).scope(root));       // or SceneMut::observe in a write
let obs2 = scene.observe(ChangeFilter::only(ChangeMask::PROPS | ChangeMask::STRUCTURE)
    .scope(panel_node).kind(kind).prop(pid).ignore(my_provider));
let obs3 = scene.observe(ChangeFilter::only(ChangeMask::STATE).states(&cad::PENDING));
// later, any phase (also inside a write):
let set: ChangeSet = scene.take_changes(&obs);   // or obs.take()
for c in &set.records { match c { Change::Ext { node, ext, what: ExtChange::Removed } => …, _ => {} } }
if set.overflowed { /* re-read */ }
for (node, mask) in set.by_node() { … }
set.damage.relayout / .repaint / .a11y
// ticks instead of a journal:
let t = scene.read(|s| s.tick());
s.changed_since(t); n.changed_since(t); n.ext_ticks(&KEY); s.with_ext_ticks(&KEY)
// extension data:
s.with_ext(&KEY) -> (NodeRef, &T)   // O(entries)
s.ext_len(&KEY); n.ext(&KEY); n.ext_block().names(); m.node(id)?.ext_mut(&KEY).set(v)
```

Semantics, documented in `journal.rs`:
- Pull only. Nothing is called back during a mutation.
- Records name what changed, in order. The observer reads values from the scene when it takes them.
- A move is `Removed` then `Inserted`.
- A queue overflows at `capacity` (default 64k): it clears, sets `overflowed` and stops until the next take.
- A filter's scope is checked at record time against the record's subject: the parent for `Inserted`/`Removed`, else the node. So a `Root` scope sees a subtree built detached as one `Inserted`.

## Acceptance (§14 SG2)

- **The journal replays to the same dump:** `scene/journal_tests.rs::journal_replay_rebuilds_the_dump`.
  - Setup: 16 seeds × 400 random runtime and Rust ops. The ops cover create, insert, remove, destroy, native subtree frees, text, state bits, sparse and dense ext keys (set, get_mut, remove, or_insert_with) and batches.
  - Check: every 7 steps an observer applies its records to a mirror tree, and the mirror's text (tag, id, owner, text, state names, ext names) must equal `SceneRef::dump` with `state` and `ext`. The mirror also asserts that ext entries were removed before `Destroyed`.
  - Alongside, a second observer (only `Ext` of one key) rebuilds that key's marked set, which must equal `with_ext` (R19). A third, short-lived observer is dropped midway.
- **No allocation per mutation without observers:** `tests/scene.rs::mutations_allocate_nothing_without_observers`, with a counting global allocator.
  - The mutations: a number prop, a bool prop, a common prop, state bits, ext set/get_mut/remove/re-add on a sparse and a dense key, a sibling reorder and a nested batch.
  - Result: 50 rounds allocate 0 times after warm-up. Also 0 with an observer that wants only `ROOT`, and 0 after the observers are dropped. This needed the registry fix above: `find_prop`/`prop_def` allocated on every write before.
- **`with_ext` over 5k entries among 100k nodes ≤ 50 µs:** `tests/scene.rs::with_ext_over_5k_of_100k_nodes`, best of 20 runs.
  - Release, run locally once: **1.9 µs**; a scan of every node took 1.67 ms.
  - Debug (what CI runs), measured locally: ~0.44 ms. The debug assertions are a 4 ms budget and "beats a full-node scan 5×".
- Other new tests:
  - `ext_columns_sparse_and_dense`
  - `ext_removal_is_journaled_on_remove_and_on_destroy` (R19)
  - `observer_filters` (root and subtree scope, kinds, props, state bits including a provider bit, origins, ignore)
  - `batches_overflow_and_dropped_observers`
  - `layout_records_and_damage`

## Deviations from ARCH-SCENE, and why

- **`ChangeFilter` lists are `Vec`s; empty means any** (§6.5 has `kinds: Option<SmallVec>`, `SmallVec` elsewhere). The crate has no `smallvec`, and filters are built once. Added fields:
  - `states` (state-bit interest, the registry hookup);
  - `ignore` (one's own writes; §6.5's "everything but Rust(self)" needed a provider, not a mask bit);
  - `capacity`.
- **Origin filtering applies to mutations only.** `Layout` and `Root` records always pass it. Only `Prop` carries its origin in the record (as §6.5); the filter sees the origin of every record.
- **`ChangeMask::CREATED` covers `Created` and `Destroyed`.** `STRUCTURE` covers `Inserted`/`Removed`.
- **`Damage` is accumulated per observer at record time,** so `take_changes` never borrows the scene and works inside a write.
  - `repaint`: per viewport, the union of the subjects' last drawn rects. The viewport comes from `RootEntry::viewport` (ROOT until EM2 sets it).
  - `a11y`: the subjects of prop, state, structure, anchor and layout records.
  - `relayout`: from `run_root`.
- **`ExtEntry::get_mut` counts as a change** (journaled, ticks stamped) when the entry exists, as in Bevy. `ExtEntry::get` and `NodeMut::ext_mut` alone no longer stamp the node; before SG2, `ext_mut` stamped even for a read.
- **`ExtBlock` is now `ExtBlock<'a>`, a read view.** Its old `get_mut`/`insert`/`remove` are gone (`ExtEntry` is the write API, as RA1 said). `n.ext_block().names()` and `n.ext(&K)` are source-compatible. `ext_block()` returns the view by value instead of `&ExtBlock`.
- **Batch markers only for explicit batches:** `write`, `defer`, `SceneMut::batch`. Host ops are one tick each, as before. A nested `batch` joins its outer batch and keeps the outer label.
- **Ext changes stamp `NodeCore::changed` but set no dirty bits.** The walk repaints anyway; an observer decides what ext data means.
- **`Query::changed_since` (R14) is left for SG3:** `query.rs` and `Query` do not exist yet. Its basis landed: `SceneRef::changed_since(t)` (a hot-column scan), `NodeRef::changed_since`, and `ExtTicks` via `with_ext_ticks`. SG3 adds the predicate and puts it on §16's stable list.

## Not journaled (yet)

- TR1 visual channels (`set_visual`: opacity, transform, clip). Animator-rate writes; add an `Origin::Animator` record if a consumer needs them.
- `NodeMut::set_flags`; listen/observe registrations (`set_listen`, R1 `observe`).
- egui id re-derivation (`Scene::attached`, `set_root_base`): the `Anchor` record covers anchor changes only.
- `Root { Visible, Viewport }`: the variants exist. EM2 owns `set_visible`/`set_viewport` (EM1's `Slot`) and should call `Scene::root_changed(r, RootChange::Visible(v) | RootChange::Viewport(vp))` there. That also keeps `RootEntry::viewport` right for `Damage::repaint`.

## Notes for SG3

- R14: add `Query::changed_since(tick)` as a predicate over `NodeCore::changed`. When driven from an ext column, use `ExtTicks::is_changed_since` instead of the node tick.
- `Scope` lives in `journal.rs`; `Query::within(impl Into<Scope>)` can take it as is.
- The planner's ext driver is `ExtColumns::col(key)` and `Column::values()`, the O(entries) path. Use `ErasedColumn::len` to pick the smallest driver.
- `SceneRef::of_tag` and `nodes` are still slot scans; the kind index goes in `Scene::create`/`free_slot`. `free_slot` now journals before it clears, so keep index removal after the records.

## Notes for SG4

- **R23 (`register_state_provider` → `ProviderHandle<P>`) is not done here and is not blocked.** A `StateProvider` toggling 5k bits in one batch is one `SceneMut::batch` of `NodeMut::set_state` calls: one tick, one `Batch` marker.
  - Observers filtered to other bits (`ChangeFilter::states`) record nothing.
  - With no state observer the cost is one mask test per node.
- Payload take-out (§7.3) is still SG4's.
  - SG2 gave `WidgetCx` an `ExtCx` (the ext columns, journal, hot column, roots and tick borrowed apart from the payload) instead of `&mut Scene`.
  - `WidgetCx::scene()` can replace it once the payload is taken out.
- **R19:** `RowPool::clears(&KEY)` can rely on removal records: `ExtEntry::remove` and freeing a node both journal `Ext { Removed }`. A PickProvider can keep its marked set from `ChangeFilter::ext_of(&PICK_TARGET)`; `journal_replay_rebuilds_the_dump` checks exactly that pattern.

## Verification (local)

- `tools/dev/cargo-slot cargo check -p svelters-egui --all-targets`: clean.
- `cargo check --workspace --all-targets`: clean. The only warnings are pre-existing: svelters-host `CatchUnwind`, and svelters-gtk generated `slots.rs` unused `p`.
- `tools/dev/cargo-slot cargo nextest run -p svelters-egui -p egui-kinds-example`: 50/50, after the last code change.
  - That is every SG1, EM1 and RA1 test unchanged, the op-log replays, and the 8 new SG2 tests.
  - Fixtures were built with `pnpm --filter "./fixtures/*-egui" build` and `pnpm --filter ./examples/egui-kinds/app build`, after `cargo build -p svelters-compiler --bin svelters-compile`.
- `cargo nextest run --release -p svelters-egui --test scene -E 'test(with_ext_over) | test(mutations_allocate)'`: 2/2 (`with_ext` 1.9 µs).
- Not run locally: workspace tests, conformance, the JS packages' tests, the other backends, `SG2_SEEDS` stress runs. CI runs them.
