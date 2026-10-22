# SG3: indexes, queries, traversal and hit-testing (`crates/svelters-egui/src/scene`)

Design: `plans/research/ARCH-SCENE.md` §6.2, §6.3, §7.4, §14 (SG3 row), with S12(a) and the cadatomic review
items R13, R14 and R20 from its "Decided" section. Starts from master `db7f62c5` (SG1 + EM1 + RA1 + EM2 + SG2).
Lane: SG3 (SCENE writer), branch `ci/sg3`. The last scene milestone on SC1's path.

## What landed

| File | Content |
|---|---|
| `scene/query.rs` | **Kind sets** (`Indexes`: per kind index a packed `Vec<NodeId>` plus a position per slot; added in `Scene::create`, swap-removed in `free_slot` after the journal records). **Anchor maps** (`RootEntry::anchors`: anchor → the nodes of that root carrying it; `index_anchor`). `SceneRef::{by_anchor, all_by_anchor, by_path, root_by_key, of_kind, of_tag, with_state, query, root_visible, root_viewport}`. `NodeRef::{anchor, screen_rect, cursor, index_in_parent, visit}`. `Visit`, `Cursor` (parent, first/last child, next/prev sibling, `depth`; may move above its start). `Query` and its planner. `validate` checks both indexes. |
| `scene/hit.rs` | Per-viewport hit lists (`HitLists`): the walk records each drawn leaf and provider node with its response rect, clip rect, layer order and root; a viewport's list starts over at its next pass (`cumulative_pass_nr`). `SceneRef::{hit_test, hit_node}`. `Hit { Node, Part(u64), Miss }` (R13), `HitCx`. |
| `scene/tree.rs` | `RootEntry::{visible, key, anchors}`; anchors follow nodes in `set_root_subtree` (attach, detach, moves between roots) and leave on `free_slot`; `root_changed` now also keeps `visible` and is no longer dead code; `set_root_key`. |
| `scene/access.rs` | `set_value` keeps the anchor map on `anchor` writes. `NodeRef::is_visible` also requires the root to be shown (and attached). `of_tag` moved to `query.rs` (index-backed). `NodeRef::{at, s, i}` crate-visible for `query.rs`/`hit.rs`. |
| `scene/dump.rs` | `SceneStats::hit_entries`. |
| `provider/mod.rs` (RUSTAPI file, small) | `Widget::hit_test(&self, &HitCx, local) -> Hit` (defaulted: `Hit::Node`), forwarded through `ErasedWidget`. Asked only for kinds with `Caps::HIT_TEST`. Re-exports `Hit`, `HitCx`; both are in `prelude`. |
| `walk.rs` (one line) | `crate::scene::hit::record(arena, i, &child)` after a leaf or provider node is painted. |
| `embed/mod.rs` (small) | SG2's note for EM2: `set_visible`/`set_viewport` call `Scene::root_changed(r, RootChange::Visible(v) / Viewport(vp))`, so the journal records them and per-root damage and R20 see the current viewport. `set_slot_base` also sets the root key (`by_path`). `Embed::anchored` (behind `anchor_rect`/`anchor_id`) uses the anchor map instead of a slot scan. |

## The query/hit-test API cadatomic gets (`svelters_egui::scene::*`)

```rust
scene.read(|s| {
    // anchors and id paths (per root; O(1))
    let r: RootIx = s.get_raw(slot.handle()).unwrap().root();   // a slot's scene root
    s.by_anchor(r, "heading");                 // Option<NodeRef>; first indexed if duplicated
    s.all_by_anchor(r, "item-x");               // every node with that anchor in r
    s.by_path("panel.task/slot/extents[0]");    // "<root key>/<anchor>"; the anchor may contain '/'
    s.root_by_key("panel.task"); s.root_visible(r); s.root_viewport(r);
    // indexes and scans
    s.of_kind(kind); s.of_tag("label");        // O(nodes of that kind)
    s.with_state(StateBits::SELECTED);          // hot-column scan
    s.with_ext(&PRESENTS);                      // SG2, O(entries)
    // queries: all predicates must hold; the planner drives from the smallest ext column or kind set,
    // else the scope's subtree, else every node
    s.query()
        .within(r)                              // RootIx or NodeId (subtree, node included)
        .tag("label")                           // or .kind(KindId)
        .has(&PRESENTS)                         // .ext(&KEY, |v| …), .ext_changed_since(&KEY, tick)
        .state(cad::PICK_TARGET).not_state(StateBits::DISABLED)
        .visible()                              // prop, ancestors, root shown
        .changed_since(tick)                    // R14: node tick (props, state, ext, children)
        .filter(|n| …)
        .iter() /* .first() .count() .ids() .targets() */;
    // traversal
    n.descendants(); n.ancestors(); n.children();
    n.visit(|n| Visit::Continue /* SkipChildren, Stop */);
    let mut c = n.cursor(); c.goto_first_child(); c.goto_next_sibling(); c.goto_parent(); c.node();
    n.anchor(); n.screen_rect();                // Option<(ViewportId, Rect)> from last frame
    // hit-testing: last (or current) pass of that viewport, topmost first
    let path: Vec<Target> = s.hit_test(viewport, pos);   // Target::Node(id) | Target::Virtual(VNodeId { host, key })
    s.hit_node(viewport, pos);                  // the topmost node (or the host of a part)
});

// a provider kind answers for its parts (R13)
#[svelters::widget(namespace = "cad", tag = "cad-viewport", caps = HIT_TEST)]
impl Widget for Viewport3d {
    fn hit_test(&self, cx: &HitCx, local: egui::Pos2) -> Hit {
        match self.pick_at(local) { Some(id) => Hit::Part(id), None => Hit::Miss }
    }
}
```

Hit-test rules: entries of freed nodes, of nodes moved to another root, and of hidden roots are skipped;
**R20**: entries of a slot root whose current viewport is not the one asked are skipped (the frame after
`set_viewport`); `HIT_TRANSPARENT` on a node *or any ancestor* lets the point through (CSS `pointer-events: none`,
inherited); `Hit::Miss` lets it through to what is below; a higher `egui::Order` is on top, then later-drawn.

## Acceptance (§14 SG3)

- **Query by anchor, kind, ext and state across two roots and two viewports:**
  `tests/queries.rs::queries_and_hit_tests_across_two_roots_and_two_viewports`. Two `Panel` panes through the dock
  adapter, one in the root viewport and one in deferred window 2. Anchors and id paths per root; kind and scope;
  ext and state written from Rust in both roots, queried together and per root; **R14** `changed_since` for Rust
  writes and for a JS change (root props), per root. Hit-tests in each viewport return the anchored label.
  **R20**: right after `set_viewport(b, window 3)`, window 2's stale entries for b are skipped and the `Root {
  Viewport }` record is journaled; after window 3 draws, b is hit there. A hidden root is not hit and its nodes are
  not `visible()`; shown again, it is hit from its last pass's entries.
- **Hit-test returns the topmost and honours `HIT_TRANSPARENT`:**
  `tests/queries.rs::hit_test_returns_the_topmost_and_honours_hit_transparent`. Two slots drawn over each other with
  Rust-built buttons: later-drawn first; `HIT_TRANSPARENT` on the button, then on the slot's box (inherited), lets the
  point through; a slot drawn first in a `Foreground` area is on top of the later background one.
- **R13 (`Hit::Part`, `Hit::Miss`):** `examples/egui-kinds/tests/hit.rs::a_hit_test_provider_reports_parts_and_misses`.
  A test kind `row-strip` (`caps = HIT_TEST`) reports rows on its left half as `Target::Virtual(VNodeId { host,
  key })`, misses on its right half so the label under it is hit.
- Unit: `scene/tests.rs::queries_indexes_cursor_and_changed_since` (anchor maps through anchor writes, detach,
  moves between roots, duplicates and frees; kind sets; every `Query` predicate and the drivers; `ext_changed_since`;
  cursor and `visit`). `random_op_interleavings` now also writes anchors from a small set (duplicates, moves) and
  `validate` checks the kind sets and anchor maps after every op.

## Deviations from ARCH-SCENE, and why

- **Result order** is the driver's order (an ext column's packed order, a kind set's order, pre-order for a scope),
  not tree order. Deterministic for a given history; documented on `Query`.
- **Only leaves and provider nodes are in the hit list** (§7.4: "drawn nodes with `Sense` or `Caps::HIT_TEST`";
  containers draw nothing). `HIT_TRANSPARENT` is therefore inherited (checked up the ancestor chain at hit time),
  so it works on containers; there is no `pointer-events: auto` override yet.
- **Layer order** within one `egui::Order` is draw order, not egui's area z-order (`Areas::order` is crate-private
  in egui 0.37). Two overlapping floating windows of the same order can be misordered; a later fix could resolve
  ties with `ctx.layer_id_at(pos)`.
- **R20 applies to slot roots.** `<window>` roots of the desktop runner are drawn in immediate viewports whose ids
  the scene's root entry does not track; their entries are matched by the list's viewport only.
- **A viewport's list is reset only when that viewport records again.** A viewport that stops drawing any slot keeps
  its old list; its entries are still filtered (liveness, root, R20, visibility).
- **`by_path`** is `"<root key>/<anchor>"`, the root key being the embed's `RootKey` of the mounted root (set with the
  id base in `set_slot_base`). Painted-row segments (`row[feature:1234]`) are SG5.
- **`Query::ext_changed_since(&KEY, t)`** is added next to `changed_since(t)` (SG2's note: the entry's own ticks when
  the change of one key matters).
- **Not done** (later milestones): `Query::include_virtual`, `Descendants::with_shadow`/`with_virtual` and
  `Target::Virtual` from queries (SG5); `select` (CS2); a `NodeMut::set_anchor` (anchors are written through the
  `anchor` prop, `set_by_name("anchor", …)`).
- `NodeRef::is_visible` now includes the root's visibility (§6.1 says so); a detached node is not visible.

## Notes for the next lanes

- **SG4:** `Caps::HIT_TEST` is live (`Widget::hit_test`); overlay roots should get `TreeRoot::Overlay` and decide
  whether the R20 check applies (it does for `Slot` only). `ScenePass::overlay` can call `scene.hit_test(cx.viewport(),
  pos)`: after the viewport's slots are drawn, its list is complete for the current pass. A provider's `hit_test`
  is skipped (answered `Hit::Node`) while its widget is borrowed by its own `ui`.
- **SG5:** `Hit::Part(key)` already becomes `Target::Virtual(VNodeId { host, key })`; add `include_virtual`,
  `by_path` row segments through `VirtualChildren::find_anchor`.
- **SG7:** the snapshot can take `RootEntry::{anchors, key, visible, viewport}`; `SceneStats::hit_entries`.
- **EV2b** also edits `walk.rs`: SG3's change there is the one `hit::record` line at the end of `paint_node`.

## Verification (local)

- `tools/dev/cargo-slot cargo check --workspace --all-targets` (public API change: `Widget`/`ErasedWidget`): clean,
  existing warnings in other crates only. `cargo check -p svelters-egui -p egui-kinds-example --all-targets`: clean.
- `tools/dev/cargo-slot cargo nextest run -p svelters-egui -p egui-kinds-example`: 61/61 after the last code change
  (fixtures built with `pnpm --filter "./fixtures/*-egui" build` and `pnpm --filter ./examples/egui-kinds/app build`
  after `cargo build -p svelters-compiler --bin svelters-compile`).
- Not run locally: workspace tests, conformance, JS package tests, other backends, `SG1_SEEDS` stress runs. CI runs
  them.
