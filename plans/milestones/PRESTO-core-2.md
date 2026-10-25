# PRESTO-core-2: presto-core, continued (presto batch 2, lane CORE-2)

Branch `ci/presto-core-2` from `ci/presto-base` (f40f8353). Session `session_01QDYxkmxLL2rFtDDfSbu4ow`,
2026-10-25. Owns `crates/presto-core/**`. Brief: the coordinator's message (parts A–D) on top of
`plans/cloud/presto-b2/BRIEF.md` and `plans/milestones/PRESTO-core.md`.

## A and B: superseded by CORE's a07c3189 (on base)

I implemented parts A and B (6c7a3b42, 5620019a). Meanwhile the coordinator merged CORE's a07c3189 into
`ci/presto-base`, which answers the same requests, and WIDGETS-BASIC was already building on it. At
8d45666c I merged base and took **base's presto-core and presto-widgets wholesale**, so my A and B are
gone from the tree. The API is base's: `NodeView::css`, `EventCx::{wake_at, cancel_wake, time, blur,
service}`, `Document::{next_timer, run_timers, a11y_action(window, req), set_service/service, call,
attach_window(window, node, ..)}`, `MethodSpec`/`ArgSpec`/`NameSource`, and
`RegistryError::CrossKindCollision`.

Added on top of base:
- `KindRegistry::member(id) -> Option<(kind, member)>`, to decode wire ids, and `spec_hash()`, which does
  not depend on registration order (test `services::wire_ids_decode_and_the_spec_hash_ignores_order`).

## C: style table, transform node, passive bit

- **Q1 style table.** SVELTERS-PRESTO's generated `tables::PRESTO` is not on base. It is derived from
  their presto.json, which lacks `z-index`, `pointer-events` and `will-change`. The brief's fallback
  (`metadata/style/presto.yaml` + a generator row) would produce the same `metadata/semantics/presto.json`
  that SVELTERS-PRESTO's spec generator writes, so the two would conflict either way. Instead
  `presto_core::style::table::table()` builds the table once: egui's generated table plus the six
  (`transform`, `transform-origin` raw, `z-index` int, `visibility` visible/hidden/collapse,
  `pointer-events` auto/none, `will-change` raw), sorted. The cascade uses it. It follows egui's table
  automatically. Once `tables::PRESTO` (with the six) merges, `table()` returns that and the file goes
  (see Q1 below). `transform-origin` is now parsed (keywords and percentages; lengths keep the centre).
- **Q2:** one transform node per node, unchanged.
- **Q3:** `NodeFlags::DESC_NONPASSIVE` (bit 7) is maintained on listen/unlisten, insert and detach (an
  upward walk that stops at the first unchanged node). A scroll node is passive only when neither its
  path nor its subtree has a non-passive wheel/pointermove listener.

## D: the not-done list

| Item | State |
|---|---|
| Paint and layout transitions | Done. `color`, `background-color` (solid), `border-color`, `outline-color` and `width`/`height` (both in points) transition on the UI thread, sampled with `presto_anim::sample` at the frame's target time (`frame(window, target)` now uses `target`). A change while one runs starts from the shown value, which gives correct reversal. A restyle to the same target keeps it running; a change without a `transition` stops it. `transitionend` (`propertyName`, `elapsedTime`) is queued when a UI transition ends, and also when the compositor reports a CSS opacity/transform transition finished. `Busy::ANIMATING` covers both. Limitation: an animated inherited colour reaches descendants only at their own restyle, not per frame. |
| Driving `virtual_children` | Done. New `VirtualChildren::realize(cx: &mut RealizeCx, rows) -> Vec<(row, node)>` (default: nothing). It runs before layout, and again after layout when layout moved the viewport; new rows are restyled before they are laid out. The document flags rows `ROW`, places them absolutely at `row_rect`, parks rows that were bound before and are not returned (`display: none`, kept alive for the pool), and uses `content_extent` as the scroll range. Also `Document::refresh_virtual(node)` and `virtual_rows(node)`. |
| `ime_area` in the Commit | Done: the focused text kind's marked range (or else selection) via `bounds_for_range`, or else its box. Sent only when it moves. |
| Queries/anchors port, style interner/compaction port | Not done. |
| `resources.paths`, layer `opaque`, layer `Damage` | Not done. |
| Incremental rebuild walk | Not done. It is still O(window) on structure, layout or property changes. |

## Cross-lane edits

`crates/presto-widgets/src/host.rs`, in my A/B commits only. Base's version replaced it at the merge, so
the branch now has no diff outside `crates/presto-core/**` and this record.

## Tests

Written, not run (the brief's rule): `tests/style.rs` (presto properties reach the computed box; paint
transitions; layout transitions with reversal; stopping), `tests/commit.rs` (DESC_NONPASSIVE and the
scroll node's `passive`), `tests/virtual_rows.rs` (realise, place, recycle on scroll, model extent, park),
`tests/ime.rs`, `tests/services.rs` (wire-id decoding and spec hash), the unit tests of `style/table.rs`
and of `parse_origin` in `style/cascade.rs`.

## Built / not run

Built: `cargo check -p presto-core -p presto-widgets --all-targets`, after every part. Not run: any test,
clippy, other OSes. CI on the pushed sha is reported in the coordinator message.

## Questions for the coordinator

1. **The PRESTO style table's source.** Recommendation: SVELTERS-PRESTO adds `z-index` (int),
   `pointer-events` (auto/none) and `will-change` (raw) to `crates/svelters-presto/src/css.rs`, so the
   generated `tables::PRESTO` has all six. Then `style::table::table()` becomes `&tables::PRESTO`, a
   one-line change.
2. **A11y name sources.** Base's `NameSource::Children` returns `None` and leaves the name to AccessKit,
   but accesskit 0.25 computes names from descendants only for some roles. Recommendation: compute it in
   presto-core (concatenate descendants' text, as my superseded 5620019a did), the next time presto-core
   is touched.
3. **Method ids** hash only the return type (base). Changing a parameter type then keeps the id, so a
   stale compiled call passes the id check and fails at the argument check instead. Recommendation: hash
   the signature (`(int,string?)->int`) before any spec ships.

## Hand-off (next session)

Remaining from D: the query/anchor port (svelters-egui `scene/query.rs`), the interner/compaction port
(`compact.rs`), `resources.paths`, layer `opaque` and `Damage` in the commit, and the incremental rebuild
walk (items, layers and hit list for the dirty subtrees only).
