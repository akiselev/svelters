# SG6: scene memory, paint cache and culling

Spec: `plans/research/ARCH-SCENE.md` §8.3 (paint caching), §8.5 (memory), §7.3 (`SKIP_OFFSCREEN`, ink bounds), §14 SG6 row,
§15 S11. Lane `ci/sg6`, svelters-egui only.

## Numbers

All runs are release builds (`cargo nextest run --release -p svelters-egui --no-capture -E 'binary(scene_memory)'`)
on thinkpadx1, headless kittest at 480×360. The test grid has the shape of cadatomic's SC1 panels (`ui/src/TaskPanel.svelte`
sets layout through props): a scroll area of collapsing groups. Each group holds 20 rows, and each row is a `box` with a
`label` and an editor (`drag-value`, `text-edit`, `checkbox` or `combo-box`). Times vary ±30% between runs on this
machine, so each line gives the range of 2–3 runs.

| | before (master 424ebcf4) | after |
|---|---|---|
| **100k nodes** (`scene_memory`, `SceneStats::bytes`) | 241 MB, **2410 B/node** | 50.0 MB, **500 B/node** (budget 640) |
| by column (B/node) | core 136, children 46, layout 1520, payload 147, cold 554 | core 110, children 40, layout 208, payload 118, cold 8, common 9, slots 7 |
| `LayoutBox` | 1160 B (`Style` 552, `Cache` 408, 2 × `Layout` 92) | 64 B (`Arc<Style>`, boxed cache, 2 × 16 B rects) |
| boxed cold data | 600 B on nearly every node (`Common` 448 inside) | none in the grid (`Common` is its own interned column) |
| distinct styles / common sets shared by 10k nodes | — | 11 / 22 |
| **10k nodes**, steady frame | 0.77–0.93 ms | 0.29–0.45 ms |
| 10k, scrolling frame | 1.01–1.37 ms | 0.41–0.77 ms |
| 10k, frame relaying out after a label's text changed | 8.7–9.9 ms | 3.7–5.3 ms |
| 10k, first run (build, layout, frames until settled) | 155–185 ms | 84–111 ms |
| nodes walked per frame (10 008 live) | not counted | 220 |
| paint cache, 1000 widgets × 25 shapes, 8 text layouts each | 23.7 ms/frame uncached | 14.4 ms/frame cached (18.5 with tessellation) |

For the 10k rows, "before" is this branch with `layout.rs` `CACHE_LEAVES = true`, which gives every node taffy's cache
as master does. The master run measured 0.77 ms steady and 1.10 ms scrolling before any change. In the asserts,
`scene_memory` checks ≤ 640 B/node, `frame_time_10k` checks fewer than 300 nodes walked per frame, and both are
independent of the machine. The times are only printed.

## What changed

- **Interned taffy styles.** `LayoutBox::style` is an `Arc<taffy::Style>` from `Scene::intern_style`
  (`scene/compact.rs`, `Interner`). Values are bucketed by a structural key and compared with `==`, and entries that
  only the table holds are pruned as it grows. One-off writes (a collapsing header's padding, a root's size, an
  overlay's measuring run) use `Arc::make_mut`.
- **Interned common props.** `NodeCold::common` moved to its own column `Scene::common: Vec<Option<Arc<Common>>>`,
  written only through `Scene::set_common`. `None` means all defaults. `NodeCold` (152 B) is now boxed only for the rare
  data (visual channels, observation counts, attached and hoisted lists).
- **Layout caches for containers only.** `LayoutCache` is an `Option<Box<taffy::Cache>>`, and `cache_store` keeps
  entries only for nodes with children, containers and provider widgets. A built-in leaf runs taffy's leaf layout again
  from its cached intrinsic size. That costs less than the memory traffic of 408 B per leaf: more leaf layouts run (166
  against 14 for a text change), but the relayout is faster overall.
- **Layout rects.** `unrounded` and `layout` keep only location and size (`BoxRect`, 16 B). `round_layout` reads
  nothing else from what we store, and `place` and the walk use nothing else.
- **Column growth.** The node columns grow by a quarter, not double, because the budget counts capacity.
- **Memory report.** `SceneStats::bytes_by` gives memory by column (now including slots, the CSS per-node data and the
  interned values). Added counters: `styles_interned`, `commons_interned`, `layout_caches`, `nodes_walked`,
  `nodes_culled`, `ink_overflows`, `paint_cached`, `paint_hits`, `paint_misses`.
- **Ink bounds.** `layout::ink` runs at the end of `place`, children before parents, then up the ancestors of a
  relaid boundary while they change. A node's ink is its paint bounds (CS5g shadows) joined with its children's ink.
  Two cases change that:
  - Scroll areas, virtual lists and `clip` channels clip their content, so their ink is their own bounds.
  - A transformed child makes its parent's ink everything.

  Ink is stored only when it is larger than the node's own bounds: `NodeFlags::INK_OVERFLOW` (**new bit 21**) plus
  `Scene::ink`. Before this, a container laid out off screen culled a child that painted on screen (an absolute box,
  overflow). The test `overflowing_child_of_an_offscreen_container_is_painted` covers this.
- **One culling predicate.** `walk::culled(arena, i, origin, ui)` is used by every cull in the walk (containers, leaves,
  behaviour hosts). WG6's rule against culling on a Tab pass goes there.
- **Paint cache.** `WidgetCx::cached_shapes(painter, version, |painter, rect, shapes| …)` stores `Arc<[Shape]>` in
  local coordinates, keyed by version, size, `pixels_per_point` and dark mode (`scene/paint_cache.rs`). A hit replays
  the shapes moved to the node's rect. A prop write to the node or freeing the node drops the entry. The oracle test
  `cached_paint_equals_uncached_paint` (slint SL1) checks a miss, a hit at another position, and a scale change.

## Decided against, with the measurement

- **`CompactCache`** (§8.5 and S11: the final entry plus 2 measure entries). I built it and measured it on the grid.
  taffy 0.15's flexbox stores **7 measure entries per node** on average (69 878 entries for 10 008 nodes with 9
  slots). With 2 or 4 inline entries, every node spilled to the full cache and 23–47% more leaves were laid out. Keeping
  taffy's cache but only on containers gives a bigger saving with no extra layout work for containers. S11 said to
  "keep taffy's `Cache` until the measurement asks", and this follows that.

## Not done, for a next slice

- **`SKIP_OFFSCREEN` deferring layout** ("not laid out while offscreen if its size is known"). The walk already culls
  every container by ink, so the flag still adds nothing at paint time. Deferring a flagged boundary's relayout while
  it is off screen needs the walk to queue it when it becomes visible, which takes one frame of stale layout.
- **Tab reachability of culled nodes.** This is unchanged: a culled leaf records no focus entry. WG6 adds the Tab-pass
  rule to `walk::culled`.
- **Interning CSS computed styles** (`NodeStyle::computed` is an `Arc` per node). The grid uses props, so the `style`
  column is 0 here. `SceneStats::bytes_by` now counts that column for a CSS-styled screen.
- **Unique strings.** A screen whose editors each have a unique `accessible-name` gets one `Common` per editor (448 B
  shared by nobody). The grid repeats names per group. Moving the strings out of `Common` (a public type) is an API
  change, so it is left for the CS5e follow-up that moves the layout fields into the computed style.
- **Ink staleness.** Ink is recomputed per layout run. A CSS shadow change without a relayout leaves the ink stale until
  the next layout. Shadows grow the node's own paint bounds, which the walk still reads live.
