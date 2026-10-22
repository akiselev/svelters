# LS1 + SG5: virtual lists on egui (`virtual-list`, realised and painted rows)

Design: `plans/research/ARCH-SCENE.md` §8.2, §10.2, §10.3, §14 (SG5 row), with the cadatomic review items R19 and R22
from its "Decided" section; `plans/research/TRACKS.md` LISTS (LS1: SV40, SV47); the M5 protocol
(`plans/architecture/backend-abi.md` "Model-view protocol (M5)", `plans/milestones/M5-lists.md`). Lane LISTS, branch
`ci/lists`, started from `ci/sg3` and merged with master after SG3, EV2b and SG4 landed (SG4's hooks kept: see "SG4" below).

## What landed

| File | Content |
|---|---|
| `scene/virt.rs` (LISTS) | **`RowHeights`**: uniform rows with no storage (1M rows cost nothing), a Fenwick tree over all heights from the first height that differs; `offset`, `total`, `row_at`, `range`, `rect`, `splice`, `set_estimate`; offsets in `f64`. **`VScroll`**: a virtualiser's own offset; `advance(ctx, max)` decides a pass's offset once from the stored offset, pending scroll-bar drags and this pass's wheel input (R22); `reveal`. **`RowPool`** (`row`, `bound_in_order`, `release_except`, `release_unused`, `node_of`, `created`, `rebinds`) with R19's **`clears(&KEY)`**. **`RowSource`** (Rust model → realised rows: `len`, `key`, `template`, `uniform_height`, `build`, `bind`, `anchor`, `find_anchor`, `version`). **`VirtualChildren`** (§8.2's trait) `a11y_row` (set size and position) and `label_row_by_contents`. `SceneRef::{virtual_rows, with_virtual, target_by_anchor, target_by_path, target_rect, target_anchor}`. |
| `kinds/virtuallist.rs` (LISTS) | The **`virtual-list`** kind (built-in, kind id appended to `KINDS`): props `selected`, `selection`, `layout`, `row-height`; methods `splice`, `count`, `scrollTo`, `setRow`, `rowAt`; sync signals `setup`, `bind`, `unbind`, `teardown`; async `activate`, `select`. `VList` state, the logic step `EguiBackend::realize_lists` (JS rows), `prepare_root` (Rust rows), `place_rows` (heights and anchoring after layout), `paint` (the walk). `SceneMut::{set_row_source, with_row_source, scroll_list_to}`, `SceneRef::{virtual_list, list_rows}`, `ListStats`. |
| `layout.rs` | `vlist_layout`: the list sizes as an empty leaf (rows never reach its intrinsic size), lays out its bound rows at its width (measured, or the source's uniform height), hidden layout for unbound pool rows. |
| `walk.rs`, `frame.rs`, `ops.rs`, `leaf.rs` (one-line hooks) | `paint_own` hands `virtual-list` to `virtuallist::paint`; `window_ui`/`slot_ui` call `prepare_root` before `layout_root`; `before_pass` calls `realize_lists` after the checkpoint and rAF; `call` routes `virtual-list` methods (with arguments) to `vlist_call`. |
| `scene/{node,tree,props,query,mod}.rs` | `W::VirtualList(Box<VList>)`; created with `LAYOUT_BOUNDARY`; `Scene::adopt` (a virtualiser takes a row as its child: no slot or owner check, journaled `Inserted`); prop arms; **`Query::include_virtual`** (targets then also yield each matching virtualiser's rows as `Target::Virtual`, in row order); `pub mod virt` and re-exports. |
| `provider/mod.rs`, `provider/behavior.rs` | `Widget::virtual_children(&self) -> Option<&dyn VirtualChildren>` (defaulted `None`; `Caps::VIRTUAL`), forwarded through `ErasedWidget` (`BehaviorBox` answers `None`). |
| `metadata/semantics/egui.json`, `packages/native-runtime/types/egui.d.ts` | Regenerated (`SVELTERS_UPDATE=1 … --test spec`): the `virtual-list` element. |
| `fixtures/lists-egui` | `<ListView>` from `@svelters/lists` with 2k items on egui (build only; no op-log test script). |

## How it works

**One protocol.** The egui `virtual-list` speaks M5 exactly as GTK and Qt do, so `@svelters/lists`' `ListView` (the
native runtime's `lists.js`) runs unchanged: row slots are JS outlets (`rowOutlet()` resolves `box` on egui) that
`setRow` makes children of the list; `bind(row, pos, id, gen)` mounts or rebinds the row component. N3 model mode is
not on egui (there is no native list model; LS2's Rust row-model trait replaces it); `layout="grid"` draws a list.

**The frame (R22).** No JS runs in the pass, so JS rows are bound in `logic`, after the checkpoint: each shown list
decides **this frame's** offset first (`VScroll::advance`: stored offset + last pass's scroll-bar drags + this pass's
wheel delta when the pointer is over the list's last drawn rect and no other layer is on top), then unbinds slots
that left the window, sets up the slots it lacks and binds the positions that entered (`call_sync`; two rounds so a
slot set up in round one is bound in round two). The window is the viewport plus two estimated rows each side, and never
fewer rows than estimated rows need anywhere in the list (a margin cut at the top or bottom goes to the other side), so
the slots set up at the top are all a fling needs. The walk draws with that same offset and consumes the wheel delta
it took, so outer scroll areas do not scroll too. A bind that ran deferred (JS busy) leaves the row out of the drawn
set and is bound again at the next logic; a position in view that no bound row covers is drawn as a placeholder and
counted (`ListStats::unbound_drawn`), and asks for another frame.

Why not egui's `ScrollArea` for this: it applies wheel input at the end of its pass
(`egui-0.37.0/src/containers/scroll_area.rs:1326`) and its kinetic and animated steps at its begin, from private
state, so its offset cannot be known before the pass; rows bound from it would lag a frame. A virtualiser's offset
is therefore `VScroll`'s. Painted-row widgets bind nothing and may keep `ScrollArea::show_viewport` (the
reference `virtual-tree` does).

**Rust rows (SG5 realised tier, ARCH-SCENE §10.2).** `SceneMut::set_row_source(list, source, RowPool::new())` makes a
`virtual-list` show rows from a `RowSource`. In the walk, before the root's layout (`prepare_root`, phase `Ui`), the
list decides the frame's offset and binds its window through the pool: a key that stays keeps its node, a new key
takes a free row of its template, else `build` runs once. Rows whose keys leave the window are freed first
(`RowPool::release_except`), so the keys that enter take them and a fling builds no rows. Rows are `Origin::Rust`-owned children of the list.

**Layout and anchoring.** The list is a layout boundary: scrolling or rebinding relays out the list alone (cached rows
are cache hits). Rows are measured at the list's width (`row-height` is the estimate and the minimum) into
`RowHeights`; a height change above the first visible row, and a `splice` at or above it, keep that row (by id) at its
place on screen, unless the list is at the top (CSS scroll anchoring's rule).

**A11y (SV40 set size).** The list is a `ListBox` (or `List` with `selection="none"`) with `size_of_set`; each drawn
row is a `ListBoxOption`/`ListItem` with `position_in_set`/`size_of_set` over the whole list and `selected`; the row's
widgets are its children (`UiBuilder::accessibility_parent`) and name it: the row is `labelled_by` the static texts
painted in it (`virt::label_row_by_contents`; a list box option needs a name, and `get_by_label` finds the row).
Painted rows use `virt::a11y_row`.

**Queries, anchors, hit-tests.** `VirtualChildren` is answered by `virtual-list` (keys are the JS ids or the source's
keys; anchors `row[<id>]` or the source's) and by provider kinds through `Widget::virtual_children`.
`target_by_anchor(r, "<host anchor>/<row anchor>")` and `target_by_path("<root key>/<host anchor>/<row anchor>")`
resolve rows whether or not they are on screen; `target_rect` gives their rect inside the host's drawn rect;
`Query::include_virtual().targets()` yields them. Hit-tests: a provider's `Hit::Part(key)` is already
`Target::Virtual` (SG3); realised rows are nodes and are recorded in the hit list as such (the row's outlet and its
widgets), the list itself under them.

## SG4 (merged into this branch)

- SG4's `provider::PrepareCx` and `Widget::prepare` are the provider hook; LISTS' stub `scene::PrepareCx` was dropped.
  The walk runs SG4's `provider::pass::prepare_root` and then `virtuallist::prepare_root` (the built-in list needs
  `egui::Context` to decide this frame's offset, R22) before each root's layout. A provider virtualiser reads its
  offset from its own `VScroll`.
- `Scene::adopt` stands in for shadow children: a virtualiser's rows are ordinary children it adopted. Moving them
  into SG4's shadow subtrees is left to LS2.
- `Widget::virtual_children` sits next to SG4's `layout`/`prepare`; `takes_input` (overlays) counts a
  `virtual-list` as taking input.

## Acceptance

| Gate (§14 SG5, LS1) | Test | Result |
|---|---|---|
| 2k-row realised grid creates ≤ 60 rows' nodes and none while scrolling | `tests/vlist.rs` `a_2k_row_list_binds_its_window_and_flings_without_unbound_rows` (JS protocol), `a_rust_row_source_realises_2k_rows_with_60_nodes` (Rust rows), `list_view_runs_on_egui` (real `<ListView>`) | JS protocol and Rust rows: no setup or build during the fling, passed locally (debug); `list_view_runs_on_egui`: CI only (needs the built fixture) |
| R22: a fling through 2k rows never draws an unbound row | the same three: wheel flings to the end, every frame checks `unbound_drawn` and that each drawn row shows its own item | passed locally (debug) (JS protocol, Rust rows); `<ListView>` in CI |
| anchors resolve for off-screen rows | `a_rust_row_source_…` (`grid/row[param:p10]`), `examples/egui-kinds/tests/virt.rs` `a_1m_row_tree_…` | passed locally (debug) |
| 10k-row painted tree: 0 JS entries, ≤ 1 ms scene time per frame | `virt.rs` `a_10k_row_painted_tree_scrolls_with_no_js_and_a_small_scene_time` (≤ 1 ms asserted in release; debug asserts ≤ 30 ms and prints the median) | passed locally (debug) (≤ 30 ms debug bound); the release bound not run locally |
| 1M rows with a uniform height model | `virt.rs` `a_1m_row_tree_resolves_anchors_and_queries_off_screen` | passed locally (debug) |
| scroll anchoring by key, `scrollTo`, `rowAt`, set-size a11y, select/activate | `vlist.rs` `scroll_to_anchoring_and_set_size`, `select_and_activate` | passed locally (debug) |
| `RowHeights`, `reveal` | `scene/virt.rs` unit tests | passed locally (debug) |

## Not done

- **T1 benches (SV47)** as a separate bench target: the tests print the painted tree's scene time; a bench table
  (fill, splice, fling on egui, against M5's GTK/Qt numbers) is left to LS2's T2 benches.
- Keyboard navigation in `virtual-list` (arrows, Enter to activate), `layout="grid"`, N3 model mode on egui.
- `Descendants::with_virtual`/`with_shadow` (SG3 deferred them here): `Query::include_virtual` and
  `SceneRef::virtual_rows` cover queries; a traversal variant waits for SG4's shadow subtrees.
- A `RowSource` whose length changes resets measured heights (uniform again); keyed height preservation across model
  changes is LS2's (Rust row-model deltas).

## Verification

Local, on the final code (LISTS2, 2026-10-22): `cargo check -p svelters-egui -p egui-kinds-example --all-targets`;
`cargo nextest run -p svelters-egui -E '(binary(vlist) | test(/virt/)) & !test(list_view_runs_on_egui)'` (7 passed);
`cargo nextest run -p egui-kinds-example -E 'binary(virt) | test(/virt/)'` (2 passed). Not run locally:
`list_view_runs_on_egui` (needs `fixtures/lists-egui` built; CI's linux-gtk job builds it), release-mode timing,
everything else (CI).
