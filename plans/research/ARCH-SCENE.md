# ARCH-SCENE: the retained scene graph of svelters-egui

Status: architecture proposal (research, no code). 2026-10-22. svelters master `a0e1e768`, cadatomic master as read
on 2026-10-22 (decision register entries R1–R12 at `ecd49c5`). Scope: the one retained scene that svelters-egui owns
and that cadatomic reads, queries, mutates, observes and extends from Rust. It fits inside ARCH-EMBED (slots, roots,
ids, `Widget`, `ExtKey`/state bits, attached kinds, `settled`), whose "Decided" section is binding here.

Refines: ARCH-EMBED §4.4 (ids), §5 (frame), §8.1/§8.4/§8.5 (node, provider data, behaviours), §9 (module split);
EGUI.md §1 (scene graph); CSS.md §2.2/§2.5 for the egui half (CS5e: "the scene implements `Element` directly");
TRACKS LISTS (LS1–LS3) for the scene side of virtualisation. Answers the scene half of CADATOMIC.md SV1, SV2, SV4,
SV6, SV7, SV16 (minimum), SV40, SV41 and of the user's request below.

Short names for cadatomic sources (all under `~/projects/cadatomic/docs/`), as in ARCH-EMBED:

| Name | File |
|---|---|
| scene-node | `research/2026-10-22-scene-node-and-svelte-semantics.md` |
| framework | `research/2026-10-22-component-framework.md` |
| models | `research/2026-10-22-models-filtering-and-dynamic-data.md` |
| docking | `research/2026-10-22-docking-windows-and-svelters-roots.md` |
| slint | `research/2026-10-22-slint-lessons.md` |
| shell | `research/2026-10-22-ide-shell-and-ux.md` |
| widgets | `research/2026-10-22-widget-survey-and-native-acceptance.md` |
| native-ext | `research/2026-10-22-native-extensions-abi-and-engines.md` |
| editor | `research/2026-10-22-code-editor-and-lsp.md` |
| decisions | `plan/00-decisions.md` |

## Binding inputs

- **User (svelters CLAUDE.md, 2026-10-21/22):** one retained scene, owned by svelters-egui; cadatomic extends it
  through kind, behaviour and state providers, and gets **full access** for optimisation and extensions (the user's
  words for this document: "the scene graph in svelters-egui is going to be of critical importance. we want to
  architect that properly, and give full access to it to cadatomic for optimization and extensions and other
  purposes"). Rust CSS engine, egui first. Own module loader with `$state`-keeping hot reload. egui stays 0.37.
- **ARCH-EMBED "Decided" (user, 2026-10-22):** D1(a) free-standing slots drawn from any `Ui`; D2(a) `inventory`
  kind registration; D3(a) `Widget` = `measure`/`ui`/`a11y` + opt-in `cached_shapes`; D4(a) kebab tags; D5(a)
  behaviours as attached kinds; D7(a) host-set visibility; D11(a) root info in a per-root store; seam items 2, 3, 4
  (cadatomic owns the per-frame order, `prepass_plugin = false`, one root frame per interaction in deferred windows),
  9 and 10.
- **Corrections R1–R12 (decisions, "Seam corrections"):** R5 (embedded windows call `pane_ui` through the host
  reference, never the thread-local), R7 (focus restore through the host), R8 (`Widget::busy()` and a provider bit in
  `Busy`), R11 (`consume_key`, `composing()`).
- **cadatomic rules adopted by ARCH-EMBED §2:** K-1 (`logic` drains and executes; `ui` builds widgets only), K-2 (idle
  is 0 frames), K-3 (ids are domain tuples, never positions or labels), K-7 (no `unsafe` in cadatomic: every API
  here is usable without it).

## 0. Summary

1. **One arena, split by temperature.** Nodes keep today's generational handle (`gen << 24 | idx`), now a
   `NodeId(NonZeroU64)` that *is* the ABI `svl_handle` for in-process roots. Per-node data lives in parallel columns
   indexed by slot: a ~104-byte hot `NodeCore` (tree link, kind, root, flags, dirty bits, state bits, egui id,
   rects, ticks), a `LayoutBox` (taffy style, cache, layouts), the behaviour `Payload` (built-in `W`, provider
   `Box<dyn Widget>`, behaviour, slot), a rare `NodeCold` box, and **typed extension columns** (§4.6). No ECS crate.
2. **Four identities, one per job.** `NodeId` (process-unique, generation-checked), egui `Id` (derived per
   ARCH-EMBED §4.4 at attach), `anchor` (semantic, per root, stable across runs), and later D4 identity. Virtual rows
   that are not nodes get `VNodeId { host, key }` (§8.2).
3. **Free-standing roots.** Slots (ARCH-EMBED), desktop `window`s and detached subtrees are all entries in one root
   table. Every node caches its root (`NodeCore::root`), so dirtiness, repaint routing, ids and error attribution are
   one field read (ARCH-EMBED §5.5).
4. **Full access through four views.** `SceneRef`/`NodeRef` (read, traverse, query: by id, anchor, kind, extension,
   state, hit-test; CSS selectors after CS2), `SceneMut`/`NodeMut` (mutation from Rust, dirty bits set by the setters,
   Masonry's `WidgetMut` rule), `Observer` (pull-based change journal with coalescing and damage), and provider hooks
   (`Widget` additions, `Behavior`, `StateProvider`, `ScenePass`) for custom layout, paint, hit-testing, a11y and
   per-frame logic.
5. **Ownership is per node, writes are per prop.** JS owns the structure of the subtrees it created; Rust owns slots,
   Rust-created subtrees and provider *shadow* subtrees. A Rust write to a prop goes through the same exact path as a
   user edit, so Svelte bindings stay correct by construction (§5). No mutation during the walk except a widget's own
   node, buffered.
6. **Invalidation is local.** Dirty bits (STYLE, MEASURE, LAYOUT, PAINT, A11Y, CHILDREN) are set by setters and
   propagate upward only to the nearest **layout boundary** (Flutter's relayout boundary, CSS `contain`); dirty work
   is queued per root, and hidden roots do none of it.
7. **Virtualisation has two tiers.** *Realised rows* are recycled scene subtrees bound in Rust before layout (property
   grid). *Painted rows* are not nodes at all: the widget paints them and answers a `VirtualChildren` protocol for
   a11y, anchors, hit-testing and queries (model tree, 1M rows, zero JS and zero nodes per row while scrolling).
8. **Targets.** 100k live nodes in ≤ 64 MB of scene memory (≤ 640 B per node, measured by a test); a clean frame
   check of ~1 µs per visible root; one label change in an 11k-node root re-lays out only its boundary; a 10k-row
   model tree scrolls with 0 JS entries and ≤ 1 ms of scene work per frame.
9. **Milestones** SG0 (in EG0) to SG7, 27–34 lane-days in all, with SG1–SG3 (11–13 days) on SC1's path next to
   EM1 and RA1 (§14). Twelve decisions are open (§15), each with a recommendation.

## 1. What cadatomic needs from the scene

"Full access", unpacked from the user's request and cadatomic's research:

| # | Need | Source | Section |
|---|---|---|---|
| A1 | Read any node: kind, props (typed), state, layout and screen rects, anchor, provider data | scene-node §1; framework §2.1 | §6.1 |
| A2 | Traverse: parent, children, siblings, ancestors, subtrees, roots; held ids re-validated on every read | slint §8.2 ("re-query on every read") | §6.2 |
| A3 | Query by id, anchor, kind, provider data, state bit; by selector later (CS2) | framework §7 anchor registry; CSS.md §2.9; slint SL5 | §6.3 |
| A4 | Mutate from Rust, coexisting with JS ops and Svelte reactivity | framework §5 step 4 ("updates scene nodes through `NodeMut`") | §5, §6.4 |
| A5 | Observe changes, with damage info (what moved, what needs repaint) | models §5.1; docking §5.8 (A11Y-dirty per window) | §6.5 |
| A6 | Custom layout, paint, hit-testing and a11y per kind and per behaviour | scene-node §1, §4; framework §2.2 | §7 |
| A7 | Per-node extension data, typed, cheap to iterate for hot paths | scene-node §1 (`op`, `pick`, `presents`, `owner`); TRACKS decision 3 | §4.6 |
| A8 | Optimisation hooks: virtualisation, paint caching, batching, containment | models §5.5, §6.3; framework §8; slint §4, §5 | §8 |
| A9 | Many roots and windows; deferred viewports reach the scene on the UI thread | docking §4.3, §6; ARCH-EMBED §5.2 | §9 |
| A10 | Testing and introspection: dumps, invariants, MCP/kittest/scenarios on one selector | slint SL5; CSS.md §2.9 | §11 |

Numbers cadatomic set (quoted from its research):

| Target | Value | Source |
|---|---|---|
| Frame budget at 120 Hz | 8.3 ms; model work ≤ 2 ms per frame | models §1.1 |
| T3 rows (Rust model, templates) | ≤ 1 ms per page with zero JS calls (MX5) | models §9 |
| T2 rows (Rust model, Svelte rows) | ≤ 15 µs host cost per row; page ≤ 4 ms (MX5) | models §9 |
| Expand/collapse a tree node | ≤ 2 ms, holds 120 fps (MX3); jump ≤ 1 ms, zero pixel movement (MX4) | models §9 |
| Row heights | uniform, estimated or measured; Fenwick tree, O(log n) at 1M rows | models §5.5 |
| Isolate batches | 3 ms at 60 Hz budget; 16 batches or 64k ops queued; `set_prop` coalesced | docking §6.3 |
| Viewport primitives | 1M triangles / 100k primitives at 60 fps, per-triangle `TypedRef` pick ids (NX9, deferred) | native-ext §6.6 |
| Settling | ≤ 3 iterations (SL6) | slint §10 |
| Editor | p99 frame ≤ 8.3 ms; editor CPU ≤ 2 ms per frame | editor §9 |
| Anything over ~1k rows, cameras, picking, hot input paths | Rust-native kinds | widgets §2.2 |

## 2. Today's scene (svelters master `a0e1e768`)

| Fact | Where |
|---|---|
| `Arena { slots: Vec<Slot { generation, node: Option<Node> }>, free }`; handle `generation << 24 \| idx`, generations retire at 2^29 so handles are never reused | `scene.rs:28-33`, `:167-245` |
| One fat `Node`: kind index, handle, parent, `children: Vec<Handle>`, `egui::Id`, `Common` (23 props, layout and non-layout mixed), `W` (closed enum with typed fields), `listen: Vec<u32>`, `observe: HashMap<u32,u32>`, window/focus flags, taffy `Style`, `Cache`, two `Layout`s, `intrinsic`, `rect`, `drawn`, `had_focus` | `scene.rs:114-143` |
| Estimated `size_of::<Node>()` ≈ 1.4–1.7 KB (taffy `Style` ~300–450 B, `Cache` ~500 B with 9 measure entries, two `Layout`s ~170 B, `Common` ~250 B). Not measured: SG1 adds a size test | `taffy-0.15.0/src/tree/cache.rs:214-221` |
| Node ids are `Id::unique(("svl", handle))`: collide across two embeds | `scene.rs:267` |
| Generations run to 2^29, but handles with bit 52 (generation ≥ 2^28) are app-export handles to the host | `scene.rs:42`; `svelters-host/src/exports/mod.rs:50-61` |
| `Arena::len()` and `handles()` scan every slot; `handles_of(tag)` scans the arena | `scene.rs:234-244`; `backend.rs:156-162` |
| Prop writes return `Dirty { style, measure }`; layout props rebuild the whole taffy `Style` | `scene.rs:340-423`; `layout.rs:17-83` |
| Taffy runs directly on the arena (no mirrored `TaffyTree`); the adapter maps `NodeId` ↔ handle | `layout.rs:158-312` |
| `invalidate` clears caches from the node to the window root, with no boundary | `layout.rs:314-323` |
| `place` recurses and clones each node's `children` Vec per layout; `paint` clones it per container per frame | `layout.rs:343-352`; `render.rs:403` |
| One global `layout_dirty` flag: any change re-runs layout of every presented window (cheap only because taffy caches) | `backend.rs:68-69`, `:380-399` |
| Remeasure list may hold duplicates (`push` per text write) | `backend.rs:566-568` |
| `destroy` orphans attached children; the host destroys them itself | `backend.rs:527-553` |
| The walk culls containers by clip rect, paints leaves at their rects, pushes `Out::Event`/`Out::Changed` to an outbox; no host call while the scene is borrowed | `render.rs:390-553`; `backend.rs:310-343` |
| Read access for others: `with_scene(&Arena)` and `prop(h, name)` | `backend.rs:143-154` |

The model is right (EGUI.md §1.1: arena, typed props, taffy on our own tree, outbox) and cadatomic agrees with it
(CADATOMIC.md C2: "close to `scene.rs` already"). What is missing is everything that makes it a *shared* structure:
a public read/write/observe API, roots, ownership, extension storage, local invalidation and a memory budget.

## 3. Prior art and what we take

Read from primary sources (local registry sources for taffy, egui, accesskit, slotmap, bevy; default-branch files for
the rest). "Take" says what this design adopts; "leave" what it deliberately does not.

| System | Lesson (with the concrete mechanism) | Take / leave |
|---|---|---|
| **Masonry/Xilem** | Arena node = `WidgetArenaNode { widget: Box<dyn Widget>, state: WidgetState, properties, class_set }`; widgets do not own their framework state, contexts lend it ([widget_arena.rs](https://github.com/linebender/xilem/blob/HEAD/masonry_core/src/core/widget_arena.rs)) | take: `Payload` separate from `NodeCore`, cx types lend access (§7.3) |
| | `WidgetId(NonZeroU64)` from a global counter, never reused, and `From<WidgetId> for accesskit::NodeId` ([widget.rs](https://github.com/linebender/xilem/blob/HEAD/masonry_core/src/core/widget.rs)) | take `NonZeroU64`; keep our generations (stale detection) |
| | `request_*` (this widget) vs `needs_*` (it or a descendant) flags merged upward by `merge_up`; "zombie flags" if a pass returns early ([widget_state.rs](https://github.com/linebender/xilem/blob/HEAD/masonry_core/src/core/widget_state.rs)) | take: `Dirty` local + `DESC_*` bits; passes always clear what they visit (§7.1) |
| | Pass order event → anim → rewrite passes (mutate, update, layout, compose, …) re-run with a cap → paint → accessibility; scrolling is compose, not layout ([pass_system.md](https://github.com/linebender/xilem/blob/HEAD/masonry_core/src/doc/pass_system.md)) | take: fixed step order (§7.2), settle loop capped (ARCH-EMBED §5.4); scroll offsets do not dirty layout |
| | External mutation only through `edit_root_widget(\|WidgetMut\|)`; in-pass mutation via `mutate_later`; `PaintCtx`/`AccessCtx` read-only | take: `SceneMut`/`NodeMut`, `defer`, read-only walk except own node (§5.2, S9) |
| | `tree_arena`'s safe variant gives O(depth) access; the unsafe one allows `&mut` only below a borrowed node ([lib.rs](https://github.com/linebender/xilem/blob/HEAD/tree_arena/src/lib.rs)) | leave: flat slot-indexed columns give O(1) access without `unsafe` |
| **Slint** | Static flat item tree (`ItemTreeNode::Item { children_index, parent_index, … }`, `DynamicTree` for repeaters) behind an `extern "C"` `ItemTreeVTable` per component ([item_tree.rs](https://github.com/slint-ui/slint/blob/HEAD/internal/core/item_tree.rs)) | leave the static tree (it cannot reparent, slint §3); take "behaviour table per kind" (provider vtables) |
| | Pull-based dirtiness: `PropertyTracker` records reads, `ItemCache` entries carry their own tracker and die with what they read ([item_rendering.rs](https://github.com/slint-ui/slint/blob/HEAD/internal/core/item_rendering.rs)) | leave read tracking (slint §2: setters declare effects); take "cache entry keyed by its inputs" for `cached_shapes` (§8.3) |
| | Partial rendering: per-item cached bounding box, `DirtyRegion` of at most 3 rects ([partial_renderer.rs](https://github.com/slint-ui/slint/blob/HEAD/internal/core/partial_renderer.rs)) | leave for now: egui repaints whole viewports; `Damage::repaint` carries rects for a later SL2 experiment |
| **GPUI** | `Element::request_layout` → `prepaint` → `paint` every frame on a bump arena; retained state in `element_states` keyed by `(GlobalElementId, TypeId)` ([element.rs](https://github.com/zed-industries/zed/blob/HEAD/crates/gpui/src/element.rs), [window.rs](https://github.com/zed-industries/zed/blob/HEAD/crates/gpui/src/window.rs)) | leave the per-frame tree; take typed retained state per (node, type) = ext columns |
| | `Hitbox { id, bounds, content_mask, behavior }` pushed during prepaint in paint order; `hit_test` reads the *rendered* (previous) frame | take: per-viewport hit list in paint order, previous frame (§7.4) |
| | `uniform_list` (measure one row, arithmetic range) and `list` over a `SumTree<ListItem>` with `Unmeasured { size_hint }`/`Measured`, `splice`, overdraw ([list.rs](https://github.com/zed-industries/zed/blob/HEAD/crates/gpui/src/elements/list.rs)) | take: uniform/estimated/measured heights with O(log n) offsets and an overdraw margin (§8.2; models §5.5's Fenwick tree) |
| | `.cached(style)` views replay last frame's prepaint/paint ranges when not dirty ([view.rs](https://github.com/zed-industries/zed/blob/HEAD/crates/gpui/src/view.rs)) | take as the model for `cached_shapes` |
| **Flutter** | `markNeedsLayout` walks up to the relayout boundary (`!parentUsesSize \|\| sizedByParent \|\| constraints.isTight \|\| parent == null`) and enqueues it; `flushLayout` processes shallow-first ([object.dart](https://github.com/flutter/flutter/blob/HEAD/packages/flutter/lib/src/rendering/object.dart)) | take: layout boundaries and per-root boundary queues, shallow-first (§7.1) |
| | `markNeedsPaint` to the repaint boundary, `flushPaint` deep-first; order `flushLayout → flushCompositingBits → flushPaint → composite → flushSemantics` ([binding.dart](https://github.com/flutter/flutter/blob/HEAD/packages/flutter/lib/src/rendering/binding.dart)) | take the order (layout before walk, a11y last); repaint boundaries map to provider layers (`Caps::LAYER`) |
| | Slivers ask a child manager to `createChild`/`removeChild` lazily; `KeepAlive` keeps off-screen state | take: `prepare` + `RowPool` (§8.2) |
| **Bevy** | `StorageType::Table` (dense, fast iteration) vs `SparseSet` (fast add/remove) per component ([component/mod.rs](https://github.com/bevyengine/bevy/blob/HEAD/crates/bevy_ecs/src/component/mod.rs)) | take: `ExtKey::new` (sparse) vs `ExtKey::dense` (§4.6) |
| | `ComponentTicks { added, changed }` per component; observers compare with their last-run tick; removals in a separate journal ([tick.rs](https://github.com/bevyengine/bevy/blob/HEAD/crates/bevy_ecs/src/change_detection/tick.rs)) | take: per-node and per-entry ticks plus a journal for structure (§6.5); use `u64` ticks (no wraparound clamping) |
| | `ChildOf`/`Children` relationship kept in sync by hooks; bevy_ui mirrors entities into a high-level `TaffyTree` (`UiSurface`, taffy 0.10) with two id spaces ([ui_surface.rs](https://github.com/bevyengine/bevy/blob/HEAD/crates/bevy_ui/src/layout/ui_surface.rs)) | leave the mirror: taffy 0.15's low-level traits run on our columns (already so, `layout.rs:158`) |
| **Druid / Iced** | Druid `WidgetPod` keeps `needs_layout`, an `invalid: Region` merged up with clipping, and a `Bloom<WidgetId>` of descendants for routing ([core.rs](https://github.com/linebender/druid/blob/HEAD/druid/src/core.rs)) | take damage rects per root (`Damage::repaint`); leave the bloom (we have parent links) |
| | Iced `Tree { tag: TypeId, state: Box<dyn Any>, children }` and `Operation` visitors (`container`, `focusable`, `scrollable`, `text_input`, `custom(id, bounds, &mut dyn Any)`) that widgets opt into ([tree.rs](https://github.com/iced-rs/iced/blob/HEAD/core/src/widget/tree.rs), [operation.rs](https://github.com/iced-rs/iced/blob/HEAD/core/src/widget/operation.rs)) | take the idea that widgets answer queries about their inner parts: `VirtualChildren`, `Widget::hit_test` |
| **React Native Fabric** | Immutable `ShadowNode`s cloned along the spine; commits are compare-and-swap transactions; any thread can read an old `ShadowTreeRevision` ([ShadowTree.h](https://github.com/facebook/react-native/blob/HEAD/packages/react-native/ReactCommon/react/renderer/mounting/ShadowTree.h)) | leave persistence (§9, S6): our readers are on the UI thread; take "a batch is atomic" |
| | Mount mutations `Create/Delete/Insert/Remove/Update` with `parentTag`, old/new child and `index` ([ShadowViewMutation.h](https://github.com/facebook/react-native/blob/HEAD/packages/react-native/ReactCommon/react/renderer/mounting/ShadowViewMutation.h)) | take as the shape of `Change` (§6.5) |
| **Blink** | Style invalidation sets built from rule features, pushed down only (`NeedsStyleInvalidation`, `ChildNeedsStyleRecalc`) ([style-invalidation.md](https://chromium.googlesource.com/chromium/src/+/main/third_party/blink/renderer/core/css/style-invalidation.md)) | take for CS5e: `STYLE`/`DESC_STYLE` bits fed by `svelters-style`'s maps (CSS.md §2.5) |
| | LayoutNG caches a result keyed by the full `ConstraintSpace`; reading anything outside it breaks caching ([layout_ng.md](https://chromium.googlesource.com/chromium/src/+/main/third_party/blink/renderer/core/layout/layout_ng.md)) | take as a rule for `measure` and custom layout: depend only on the inputs (taffy's `LayoutInput`) |
| | `content-visibility: auto` = layout/style/paint containment + size containment while off-screen, skips paint and hit-testing ([web.dev](https://web.dev/articles/content-visibility)) | take: `LAYOUT_BOUNDARY`, `SKIP_OFFSCREEN` flags (§8.1) |
| **AccessKit 0.25** | `TreeUpdate { nodes, tree, tree_id, focus }`; nodes sent whole; ids stable per `TreeId`; `TreeId` grafting is new (`accesskit-0.25.1/src/lib.rs`) | egui builds the tree per frame from widgets (EGUI.md §1.5), so the scene only fills nodes; per-root `TreeId` grafting is a later option |
| **taffy 0.15** | Low-level traits; **`compute_root_layout` requires `LayoutContainingBlock + CacheTree`**: absolutely positioned boxes are hoisted to their containing block (`taffy-0.15.0/src/tree/traits.rs:212-240`) | the scene must store hoisted children per node once `position: absolute` exists (today stubbed, `layout.rs:262-264`): `NodeCold::hoisted` (§4.3) |
| | `Cache` = 1 final entry + 9 measure entries; "dirty" means "cache empty" (`cache.rs:214`) | take; `CacheTree` lets a compact cache replace it (S11) |
| **egui 0.37** | `Id(NonZeroU64)` is a hash; interaction hit-tests last pass's `WidgetRects` at `begin_pass`; `read_response(id)` works before the widget is created; `show_viewport_deferred` needs `Fn(&mut Ui, ViewportClass) + Send + Sync` | ids derived deterministically (ARCH-EMBED §4.4); state bits from `read_response` (§7.2 d); thread-local access for deferred viewports (§9) |
| **slotmap / thunderdome** | 8-byte keys `{ idx: u32, version: NonZeroU32 }`; thunderdome `to_bits`/`from_bits` to u64 ([slotmap](https://github.com/orlp/slotmap/blob/HEAD/src/lib.rs), [thunderdome](https://github.com/LPGhatguy/thunderdome/blob/HEAD/src/arena.rs)) | keep our own 64-bit `gen << 24 \| idx` (JS-safe below 2^53, already the ABI handle) |
| **DOM `MutationObserver`** | records `type`, `target`, added/removed nodes, siblings, `attributeName`, `oldValue`; batched; `takeRecords()` drains ([MDN](https://developer.mozilla.org/en-US/docs/Web/API/MutationRecord)) | take: filters by subtree/attribute, pull with `take_changes` (§6.5) |

## 4. Data model

### 4.1 Where the scene lives

A module tree, `svelters_egui::scene`, not a new crate (§15 S8). The core files (`id`, `tree`, `ext`, `journal`,
`query`, `dirty`) use egui only for `Id`, `Rect` and `Pos2`, which keeps a later move into a `svelters-scene` crate
mechanical. The public API is re-exported as `svelters_egui::scene::{…}` and through the `svelters` facade's
`prelude`.

### 4.2 Identity

```rust
// scene/id.rs
/// A live or stale node. Equal to the ABI `svl_handle` of nodes created by an in-process runtime.
#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct NodeId(core::num::NonZeroU64);        // gen << 24 | idx; gen < 2^29 → < 2^53 (JS-safe)

impl NodeId {
    pub const fn index(self) -> u32;             // slot, < 2^24 (16.7M live slots)
    pub const fn generation(self) -> u32;
    pub const fn to_raw(self) -> u64;
    pub const fn from_raw(raw: u64) -> Option<NodeId>;
}

/// A row painted by a virtualising widget, not a node (§8.2). `key` is the model's stable row key.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct VNodeId { pub host: NodeId, pub key: u64 }

/// Either; what hit-tests, anchors and queries return.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Target { Node(NodeId), Virtual(VNodeId) }

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)] pub struct KindId(pub svl_id);   // ARCH-EMBED §8.1 ids
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)] pub struct PropId(pub svl_id);
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)] pub struct RootIx(u32);         // index into the root table
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)] pub struct Tick(u64);
```

| Identity | Scope and lifetime | Who assigns | Used for |
|---|---|---|---|
| `NodeId` | process; never reused (generations retire, `scene.rs:228`); a stale id fails every lookup | the arena at `create` | everything inside the process; the JS handle |
| `egui::Id` | per embed salt and root key; stable across moves (ARCH-EMBED §4.4) | `ids.rs` at attach (first insert under a tree that reaches a root) | egui memory, focus, AccessKit node id |
| anchor | per root, unique, stable across runs and reloads (domain ids, K-3) | JS (`anchor` prop), Rust (`NodeMut::set_anchor`), `VirtualChildren::anchor` | tests, tutorials, MCP, overlays, AccessKit `author_id` |
| D4 identity | per root, stable across reloads (HMR ID1) | the compiler, later | HMR, SV30 overrides |
| `VNodeId` | the host node's lifetime; `key` from the model | the virtualising widget | painted rows (§8.2) |

- **One handle space.** In-process JS gets `NodeId::to_raw()` as its handle, so `ops.rs` does no translation. An
  isolate (SV15, later) mints its own handles; its proxy applies batches through a per-isolate `HandleMap
  (u64 → NodeId)` (framework §5.1, docking §6.3). Nothing else in the scene changes for isolates.
- **App handles must not collide (a latent bug today).** The host treats any handle in [2^52, 2^53) as an app export
  handle (`svelters-host/src/exports/mod.rs:47-61`, `is_app_handle`). With `gen << 24`, bit 52 is generation bit 28,
  and today's arena recycles a slot up to generation 2^29 (`scene.rs:42`, `:228`), so a slot reused 2^28 times would
  hand JS a handle the host routes to the export table. SG1 retires generations at 2^28, which makes the two spaces
  disjoint by construction (still 268M reuses per slot).
- **Re-id rules** (ARCH-EMBED §4.4): changing `anchor` re-ids the node and drops its egui memory; moving a subtree into
  another root re-derives the non-anchored ids of that subtree (Rust-only operation, documented); `move_root` keeps
  ids because the base is the root key.

### 4.3 Columns

```rust
// scene/mod.rs (crate-private layout; the public surface is §6)
pub struct Scene {
    slots: SlotTable,                    // generation per index, free list, live count (O(1) len)
    core: Vec<NodeCore>,                 // hot, walked every frame
    children: Vec<Vec<NodeId>>,          // light children in order (layout + paint order)
    layout: Vec<LayoutBox>,              // taffy data
    payload: Vec<Payload>,               // behaviour per kind
    cold: Vec<Option<Box<NodeCold>>>,    // rare data, boxed
    ext: ExtColumns,                     // typed provider columns (§4.6)
    roots: RootTable,                    // free-standing roots (§4.5)
    index: Indexes,                      // kind → nodes, (root, anchor) → node (§6.3)
    dirty: DirtyQueues,                  // per root (§7.1)
    journal: Journal,                    // change records for observers (§6.5)
    hits: HitLists,                      // last frame's paint-ordered rects per viewport (§7.4)
    tick: Tick,                          // bumped per mutation batch
    stats: SceneStats,
}
```

Indexed by `NodeId::index()`. A freed slot resets every column (`ExtColumns::remove(idx)` visits each registered
column). This is "parallel arrays per slot" rather than archetypes: nodes never change archetype, kinds are fixed at
create, and the per-frame walk reads `core` and `children` only (§3, Bevy and Masonry rows).

```rust
#[derive(Clone)]
pub(crate) struct NodeCore {              // ≈ 104 B
    pub parent: Option<NodeId>,          //  8  light parent; for an attached node, its host
    pub kind: KindId,                    //  4
    pub root: RootIx,                    //  4  cached at attach; RootIx::DETACHED otherwise
    pub flags: NodeFlags,                //  4  ATTACHED, SHADOW, SLOT, LAYOUT_BOUNDARY, HIT_TRANSPARENT,
                                         //     SKIP_OFFSCREEN, VIRTUAL, EXITING, HAS_ANCHOR, HAS_COLD, …
    pub dirty: Dirty,                    //  2  STYLE MEASURE LAYOUT PAINT A11Y CHILDREN + DESC_* (§7.1)
    pub owner: Owner,                    //  2  Js | Rust(provider) | Host (§5)
    pub state: StateBits,                //  8  0–31 built-in, 32–63 provider (ARCH-EMBED §8.4)
    pub listen: SignalMask,              //  8  bit per signal index of the kind (> 64: cold overflow)
    pub explicit: PropMask,              //  8  props JS set (CSS.md §2.8 precedence)
    pub egui_id: egui::Id,               //  8
    pub rect: egui::Rect,                // 16  root-relative, from the last layout
    pub drawn: egui::Rect,               // 16  screen rect last frame (Rect::NOTHING if not drawn)
    pub changed: Tick,                   //  8  last tick any prop, state or ext of this node changed
}

pub(crate) struct LayoutBox {
    pub style: StyleRef,                 // Arc<taffy::Style>, interned (§8.5); today inline
    pub cache: taffy::Cache,             // v1 inline; CompactCache behind CacheTree later (§15 S11)
    pub unrounded: taffy::Layout,
    pub layout: taffy::Layout,
    pub intrinsic: Option<egui::Vec2>,   // leaf measure; None = measure again
    pub ink: egui::Rect,                 // subtree paint bounds (culling with overflow), root-relative
}

pub(crate) enum Payload {
    Builtin(W),                          // today's enum, fields borrowed &mut by egui widgets
    Widget(Box<dyn ErasedWidget>),       // provider kinds (RA1)
    Behavior(Box<dyn ErasedBehavior>),   // attached kinds (EV2b)
    Slot(SlotData),                      // ARCH-EMBED slot node: sizing, label, fault ui
    Vacant,                              // taken out while its own code runs (§7.3)
}

pub(crate) struct NodeCold {
    pub common: Common,                  // tooltip, accessible name, … (layout fields move to style in CS5e)
    pub anchor: Option<Box<str>>,
    pub attached: SmallVec<[NodeId; 2]>, // behaviours and controllers on this node (ARCH-EMBED §8.5)
    pub shadow: Option<NodeId>,          // a provider's Rust-owned subtree root (§4.4)
    pub observe: SmallVec<[(PropId, u32); 2]>,   // R1 observation counts (today a HashMap, scene.rs:125)
    pub listen_overflow: Vec<u32>,
    pub hoisted: Vec<NodeId>,            // taffy 0.15 LayoutContainingBlock: absolute boxes hoisted here
}
```

- `children` stays a `Vec<NodeId>` per node (taffy wants `get_child_id(index)`; `layout.rs:199`). Insert-before and
  remove are O(siblings). For a 10k-child flat list fed by keyed `{#each}` reorders that is quadratic; the answer is
  virtualisation (§8.2), and an `index_in_parent` cache is the fallback if a benchmark shows it (§15 S7).
- `Common` leaves `NodeCore` and `LayoutBox`: its layout fields already live in taffy's `Style` (`layout.rs:17-83`),
  the rest is rare. Until CS5e, layout setters write the interned style through copy-on-write.
- Built-in widget props stay typed fields in `W` (EGUI.md §1.1: egui borrows `&mut String`), and provider props stay
  typed fields of the provider struct (ARCH-EMBED §8.2). There is no dynamic prop map.

### 4.4 Tree shape: light, attached and shadow children

| Relation | Stored | Laid out | Painted | Seen by JS | Example |
|---|---|---|---|---|---|
| light child | `children[parent]`, `core.parent` | yes | yes, in order | yes (its own ops) | everything Svelte inserts |
| attached | `cold.attached`, `core.parent = host`, `ATTACHED` | no | runs `Behavior` hooks around the host | yes (generated action) | `use:pick` (ARCH-EMBED §8.5) |
| shadow subtree | `cold.shadow`, root flagged `SHADOW`, `core.parent = host` | by the host (`cx.children` or custom layout) | by the host | no | property-grid row pool, tab strip inside a provider |

Shadow subtrees are what lets a Rust widget build real nodes (with ids, anchors, a11y, CSS states) without entering
JS's child list, which Svelte's runtime orders by its own `before` handles. They are the scene's equivalent of a
shadow root.

### 4.5 Roots

```rust
pub enum TreeRoot {
    Slot { slot: SlotId, node: NodeId },          // ARCH-EMBED §4.1; mounted roots live here
    Window { node: NodeId },                      // desktop runner `window` (windows.rs)
    Overlay { node: NodeId, layer: egui::Order }, // Rust-created free trees drawn above a slot (§7.5)
    Detached,                                     // RootIx::DETACHED: created and not inserted, or orphaned
}

pub(crate) struct RootEntry {
    pub kind: TreeRoot,
    pub key: RootKey,                             // salts egui ids (ARCH-EMBED §4.4)
    pub viewport: egui::ViewportId,               // where it was last drawn / set_viewport
    pub visible: bool,                            // set_visible (D7)
    pub dirty: RootDirty,                         // queues (§7.1)
    pub anchors: HashMap<Box<str>, NodeId>,       // per-root anchor index
    pub size: Option<egui::Vec2>,                 // available size of its last layout
}
```

- Attaching a subtree to a node of root `r` sets `core.root = r` for the whole subtree (O(subtree), once per
  insert); removing sets `DETACHED`. ARCH-EMBED §5.5 relies on this cache.
- `Embed::slot`, `root_info` and `show_slot` resolve through this table; the scene owns it and `embed/slots.rs` owns
  the policy (visibility, viewport, fault UI).
- Detached nodes cost nothing per frame. The arena's debug validator (§11) reports detached subtrees older than N
  checkpoints that no JS handle or Rust owner references, which is how a leak shows up.

### 4.6 Extension storage: typed columns

ARCH-EMBED §8.4 fixed the API: `static PICK: ExtKey<PickState>`, `n.ext_mut(&OP)`. This design keeps that API and
chooses the storage: **one column per key, as a sparse set** (Bevy's `SparseSet` storage, §3), so that "every node
that presents a value" is an iteration over k entries, not a tree walk over n nodes.

```rust
// scene/ext.rs
pub struct ExtKey<T: 'static> {
    name: &'static str,                    // "cad.pick": debug dumps, MCP, the snapshot
    storage: Storage,
    id: std::sync::OnceLock<ExtId>,        // process-wide id, assigned on first use (no inventory needed)
    _t: PhantomData<fn() -> T>,
}
pub enum Storage { Sparse, Dense }         // Dense: Vec<Option<T>> by slot, for keys on most nodes
impl<T: 'static> ExtKey<T> {
    pub const fn new(name: &'static str) -> Self;       // Sparse
    pub const fn dense(name: &'static str) -> Self;
}

pub(crate) struct Column<T> {
    sparse: Vec<u32>,                      // slot index → dense position (u32::MAX = absent)
    ids: Vec<NodeId>,                      // dense position → node
    data: Vec<T>,
    added: Vec<Tick>, changed: Vec<Tick>,  // change ticks per entry (Bevy's ComponentTicks)
}
pub(crate) trait ErasedColumn: Any {
    fn remove(&mut self, idx: u32);        // node freed
    fn len(&self) -> usize;
    fn debug(&self, idx: u32) -> Option<String>;        // dump / inspector, when T: Debug (registered)
}
```

- Access is O(1) (`sparse[idx]`), iteration is dense (`ids`/`data` in parallel), removal is swap-remove. No
  `unsafe` is needed: columns are `Box<dyn ErasedColumn>` downcast with `Any` once per access call.
- `T: 'static`, not `Send`: the scene is UI-thread only (§9).
- **Hot data.** cadatomic's `pick` (one byte per node that can be picked) and `presents` (`TypedRef`) are iterated on
  every session state change; the column makes that O(pickable nodes). `op` (`OpView`) is updated by a state provider
  per job event; same argument.
- **State bits stay in `NodeCore`.** They are read by the walk for every node (style, a11y), so they are not an
  extension column (ARCH-EMBED §8.4 bits 32–63).
- **Provider fields vs extension data.** A provider kind's own state (`#[state] text: String`) is a field of its
  struct (`Payload::Widget`). Extension data is for data attached *to other kinds' nodes* (a `presents` on a built-in
  `label`) or shared across kinds. The rule of thumb: if only one kind reads it, it is a field.

## 5. Ownership and mutation rules

### 5.1 Owners

```rust
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Owner {
    Js,                    // created by a runtime through host ops (root from core.root)
    Rust(ProviderId),      // created by Rust through SceneMut; ProviderId names the crate/module (debug, MCP)
    Host,                  // slots, windows, overlay roots: the embed itself
}
```

| Operation | On a JS-owned node | On a Rust- or Host-owned node |
|---|---|---|
| create | JS (`create` op) | Rust (`SceneMut::create*`) |
| insert / remove children | **JS only**. Rust may not change the light children of a JS-owned node: Svelte's runtime orders siblings by its own `before` handles (`index.js` `nextNative`) and would misplace or double-remove | Rust; JS only as a `mount` target (slot) or through a handle Rust gave it on purpose |
| destroy | JS, or Rust through `Embed::unmount` (the root as a whole) | Rust; JS gets a borrowed handle (`SVL_LIFE_*` borrowed) and cannot destroy it |
| attach a behaviour | JS (`use:` action), Rust (`NodeMut::attach`) | same |
| build a shadow subtree | the node's own provider widget only | same |
| props | see §5.3 | Rust; JS if it holds the handle |
| state bits 0–31 | the walk (responses), the animator | same |
| state bits 32–63 | the provider that registered them (`state_bits!`), through `NodeMut`/`WidgetCx` | same |
| extension columns | the provider that declared the `ExtKey` (convention, not enforced: keys are statics in the provider's crate) | same |
| anchor | the `anchor` prop (JS) | `NodeMut::set_anchor` |

A violation is a `SceneError::NotOwner` in release and a debug assertion with the node's dump in debug builds.

### 5.2 Phases

| Phase | When | Who may mutate |
|---|---|---|
| `Logic` | `Embed::logic` and anything cadatomic runs in `App::logic` before or after it (seam 4: cadatomic owns the order) | JS ops (checkpoint), `SceneMut` from Rust, state providers |
| `Prepare` | `show_slot` before layout | a virtualising widget, inside its own shadow subtree only (§8.2) |
| `Layout` | `show_slot` | nobody (layout writes rects only) |
| `Walk` | `show_slot` | the current widget, on its own node, through `WidgetCx` (buffered, §7.3); everything else is queued with `SceneHandle::defer` |
| `Idle` | between frames | same as `Logic` (UI thread) |

The phase is a `Cell` on the scene. `SceneHandle::write` outside `Logic`/`Idle` returns `Err(SceneBusy)` (and logs
once in debug) instead of hitting a `RefCell` double borrow: the walk holds the scene borrow, so a `write` from inside
a widget would otherwise panic at a random spot. Code that may run in any phase uses `defer`.

### 5.3 Props: who writes what

There are four writers of a node prop. The scene keeps them apart by origin, and the journal records the origin:

```rust
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Origin { Js, Widget, Rust(ProviderId), Style, Animator, Host }
```

| Writer | Path | Announced to JS observers (`prop_changed`) | Notes |
|---|---|---|---|
| JS template / binding | `set_prop` op → `SceneMut` with `Origin::Js`; marks `explicit` | never (no echo, `backend.rs:559`) | today's behaviour |
| Widget (user edit) | `cx.set(&mut field, v)` / built-in `&mut` field diff (`render.rs:537-552`) | yes, when observed (exact) | ARCH-EMBED §8.2 |
| Rust (`NodeMut::set`) | the **same exact path as a widget write**: value stored, prop announced if observed, `Change::Prop { origin: Rust }` | yes | keeps `bind:` correct: the Svelte state learns the new value exactly as if the user typed it |
| CSS engine | non-explicit props only (CSS.md §2.8) | no | CS5e |

Consequences, stated so cadatomic can rely on them:

- **Bound props** (`bind:value`, `bind:checked`, …) may be written from Rust at any time in `Logic`; Svelte state
  follows on the next checkpoint. This is how a Rust store or a command draft pushes a value into a form field
  without JS (scene-node §3 "native binding": the compiler's `link` op later removes even the JS round trip).
- **Props JS sets but does not bind** are JS's. A Rust write is allowed, is announced, and *will be overwritten* the
  next time the JS expression re-evaluates. Debug builds report "Rust write to an explicit unbound prop" once per
  (kind, prop). Cadatomic should model such data as extension data or provider state instead.
- **Props nobody set from JS** (defaults) are free for Rust.
- **Overrides** (devtools, SV30, tutorials forcing a value) are a later layer: `NodeMut::override_prop(prop, v) ->
  OverrideGuard` stores the JS value underneath and restores it on drop (§15 S12). Not in SG1.

### 5.4 Reactivity conflicts, settled

- **No re-entrancy.** Rust mutation never calls JS synchronously. Announcements go to the host's dirty set
  (`prop_changed` only inserts, ABI v2.6) and run at the next checkpoint.
- **Ordering inside `Logic`.** cadatomic's typical order (seam 4) is `Jobs::drain` → its scene writes →
  `embed.logic` (JS sees the writes at step 4) → its observers. Writes after `embed.logic` are seen by JS in the next
  frame's checkpoint; `settled()` reports `CHANGES` until then, so wait-idle covers it.
- **Destroy races.** A JS handle to a node Rust destroyed (Rust-owned only) fails with "stale handle" exactly as
  today; a Rust `NodeId` to a node JS destroyed fails every lookup (generation check). Holders that need to know use
  an observer with `Change::Destroyed`.
- **Batches.** `SceneMut::batch(label, f)` groups writes into one journal batch and one tick, the unit isolates also
  use for atomic op batches (docking §6.3: "no half-built panel").

## 6. The access API

### 6.1 Entry points and reads

```rust
// scene/access.rs
/// The scene of one embed. `Rc`, `!Send`. From `Embed::scene()` or `svelters_egui::embed::with_current`.
#[derive(Clone)]
pub struct SceneHandle(Rc<SceneCell>);

impl SceneHandle {
    /// Read access, any phase, UI thread. Inside a widget's `ui`, use `WidgetCx::scene()` instead.
    pub fn read<R>(&self, f: impl FnOnce(SceneRef<'_>) -> R) -> R;
    /// Write access in Logic/Idle; `Err(SceneBusy)` in Prepare/Layout/Walk (use `defer` there).
    pub fn write<R>(&self, origin: ProviderId, f: impl FnOnce(&mut SceneMut<'_>) -> R) -> Result<R, SceneBusy>;
    /// Queue a mutation for the start of the next `logic` (callable from anywhere on the UI thread, including widgets).
    pub fn defer(&self, origin: ProviderId, f: impl FnOnce(&mut SceneMut<'_>) + 'static);
    pub fn observe(&self, filter: ChangeFilter) -> Observer;
    pub fn take_changes(&self, obs: &Observer) -> ChangeSet;
    pub fn register_state_provider(&self, p: impl StateProvider);
    pub fn register_pass(&self, p: impl ScenePass);
}

#[derive(Copy, Clone)]
pub struct SceneRef<'a> { /* &'a Scene */ }
impl<'a> SceneRef<'a> {
    pub fn get(self, id: NodeId) -> Option<NodeRef<'a>>;
    pub fn len(self) -> usize;                                         // O(1)
    pub fn tick(self) -> Tick;
    pub fn roots(self) -> impl Iterator<Item = (RootIx, TreeRoot)> + 'a;
    pub fn root_node(self, slot: SlotId) -> Option<NodeRef<'a>>;
    pub fn stats(self) -> SceneStats;
    // queries: §6.3; hit-testing: §7.4; dumps and snapshots: §11
}

#[derive(Copy, Clone)]
pub struct NodeRef<'a> { /* scene, id, idx */ }
impl<'a> NodeRef<'a> {
    pub fn id(self) -> NodeId;
    pub fn kind(self) -> KindId;
    pub fn tag(self) -> &'static str;                                  // "quantity-field"
    pub fn kind_info(self) -> &'static KindInfo;                       // registry entry: props, signals, caps
    pub fn owner(self) -> Owner;
    pub fn root(self) -> RootIx;
    pub fn slot(self) -> Option<SlotId>;
    pub fn parent(self) -> Option<NodeRef<'a>>;                        // light parent, or host for attached/shadow
    pub fn children(self) -> Children<'a>;                             // ExactSizeIterator + DoubleEnded + index()
    pub fn attached(self) -> impl Iterator<Item = NodeRef<'a>> + 'a;
    pub fn shadow(self) -> Option<NodeRef<'a>>;
    pub fn anchor(self) -> Option<&'a str>;
    pub fn egui_id(self) -> egui::Id;
    pub fn rect(self) -> egui::Rect;                                   // root-relative, last layout
    pub fn screen_rect(self) -> Option<(egui::ViewportId, egui::Rect)>;// last frame; None if not drawn
    pub fn state(self) -> StateBits;
    pub fn flags(self) -> NodeFlags;
    pub fn is_visible(self) -> bool;                                   // visible prop, ancestors, root visibility
    pub fn changed_since(self, t: Tick) -> bool;
    /// Typed prop read, zero-copy. Built-in keys live in `scene::props::*`; provider keys are generated by RA1.
    pub fn get<T: 'static>(self, key: &PropKey<T>) -> Option<&'a T>;
    /// Untyped read by id, as the host would (`get_prop`).
    pub fn value(self, prop: PropId) -> Result<Value, PropError>;
    pub fn ext<T: 'static>(self, key: &'static ExtKey<T>) -> Option<&'a T>;
    /// The provider widget, typed (full access to a kind's own state).
    pub fn widget<W: Widget>(self) -> Option<&'a W>;
}
```

`PropKey<T>` is a static descriptor with accessor functions, so a typed read is a borrow of the field and costs no
`Value` conversion:

```rust
pub struct PropKey<T: 'static> {
    pub kind: &'static str, pub name: &'static str,
    get: fn(&Payload) -> Option<&T>,
    set: fn(&mut Payload, T) -> Result<Dirty, PropError>,
}
// generated for built-ins by kinds/ (EG0 tables), for providers by #[svelters::widget] (RA1)
pub mod props { pub mod label { pub static TEXT: PropKey<String>; pub static WRAP_MODE: PropKey<WrapMode>; } /* … */ }
```

### 6.2 Traversal

```rust
impl<'a> NodeRef<'a> {
    pub fn ancestors(self) -> Ancestors<'a>;                // parent chain, crossing shadow/attached hosts
    pub fn descendants(self) -> Descendants<'a>;            // pre-order, light children (+ shadow if asked)
    pub fn cursor(self) -> Cursor<'a>;
    pub fn visit(self, f: impl FnMut(NodeRef<'a>) -> Visit) -> Visit;
}
pub enum Visit { Continue, SkipChildren, Stop }
impl<'a> Descendants<'a> {
    pub fn skip_subtree(&mut self);                         // after the last yielded node
    pub fn with_shadow(self) -> Self;
    pub fn with_virtual(self) -> VirtualDescendants<'a>;    // also yields painted rows as Target::Virtual (§8.2)
}
pub struct Cursor<'a> { /* stack of (node, child index) */ }
impl<'a> Cursor<'a> {
    pub fn node(&self) -> NodeRef<'a>;
    pub fn goto_parent(&mut self) -> bool;
    pub fn goto_first_child(&mut self) -> bool;
    pub fn goto_next_sibling(&mut self) -> bool;
    pub fn goto_prev_sibling(&mut self) -> bool;
}
```

All iterators borrow the scene immutably, so nothing can change under them; a held `NodeId` is re-validated on every
`get` (slint §8.2: "re-query on every read"; stale ids fail by generation, SL5).

### 6.3 Queries

| Query | Cost | Index |
|---|---|---|
| `scene.get(id)` | O(1) | slot table |
| `scene.by_anchor(root, "slot/extents[0]")`, `scene.by_path("panel.task/slot/extents[0]")` | O(1) hash | per-root anchor map, maintained on `anchor` writes and attach/detach |
| `scene.of_kind(kind)` | O(nodes of that kind) | per-kind sparse set of slot indices (maintained at create/free) |
| `scene.with_ext(&KEY)` → `(NodeRef, &T)` | O(entries) | the column itself (§4.6) |
| `scene.with_state(bit)` | O(n) over `core` (8 B stride per node, ~0.1 ms at 100k) | none; a column scan |
| `scene.query()…` | the smallest driving index, then filters | planner below |
| `scene.select(&Selector)` | CS2, the `svelters-style` matcher on the scene's `Element` impl | rule index + bloom filter (CSS.md §2.4) |
| `scene.hit_test(viewport, pos)` | O(hits on that viewport) | last frame's hit list (§7.4) |

```rust
pub struct Query<'a> { /* scene, predicates */ }
impl<'a> Query<'a> {
    pub fn within(self, root_or_node: impl Into<Scope>) -> Self;
    pub fn kind(self, k: KindId) -> Self;
    pub fn tag(self, tag: &str) -> Self;
    pub fn has<T: 'static>(self, key: &'static ExtKey<T>) -> Self;
    pub fn ext<T: 'static>(self, key: &'static ExtKey<T>, pred: impl Fn(&T) -> bool + 'a) -> Self;
    pub fn state(self, bits: StateBits) -> Self;
    pub fn not_state(self, bits: StateBits) -> Self;
    pub fn visible(self) -> Self;
    pub fn include_virtual(self) -> Self;                   // ask VirtualChildren hosts too (§8.2)
    pub fn filter(self, f: impl Fn(NodeRef<'a>) -> bool + 'a) -> Self;
    pub fn iter(self) -> impl Iterator<Item = NodeRef<'a>> + 'a;
    pub fn targets(self) -> impl Iterator<Item = Target> + 'a;
    pub fn first(self) -> Option<NodeRef<'a>>;
    pub fn count(self) -> usize;
}
```

The planner drives from, in order: an ext column, a kind set, a scope subtree, the whole core column; it picks the
smallest by current length. `within` is checked by `core.root` and an ancestor walk only when the scope is not a
root. Selectors (CS2) plug in as one more predicate, with `:text`, `:role`, `:name` test pseudo-classes as in
CSS.md §2.9, so kittest, scenarios and MCP resolve the same selector to the same `NodeId` (slint SL5).

### 6.4 Mutation

```rust
pub struct SceneMut<'a> { /* &'a mut Scene, origin */ }
impl<'a> SceneMut<'a> {
    pub fn read(&self) -> SceneRef<'_>;
    pub fn create(&mut self, kind: KindId) -> Result<NodeId, SceneError>;          // Owner::Rust(origin), detached
    pub fn create_widget<W: Widget + KindDecl>(&mut self, w: W) -> NodeId;        // a provider node with its struct
    pub fn create_root(&mut self, key: RootKey, kind: OverlayRoot) -> NodeId;     // Rust-only free trees (§7.5)
    pub fn insert(&mut self, parent: NodeId, child: NodeId, before: Option<NodeId>) -> Result<(), SceneError>;
    pub fn remove(&mut self, child: NodeId) -> Result<(), SceneError>;            // detaches
    pub fn destroy(&mut self, id: NodeId) -> Result<(), SceneError>;              // subtree; Rust/Host-owned only
    pub fn splice(&mut self, parent: NodeId, at: usize, remove: usize, insert: &[NodeId]) -> Result<(), SceneError>;
    pub fn node(&mut self, id: NodeId) -> Option<NodeMut<'_>>;
    pub fn batch<R>(&mut self, label: &'static str, f: impl FnOnce(&mut Self) -> R) -> R;
}

pub struct NodeMut<'a> { /* … */ }
impl NodeMut<'_> {
    pub fn read(&self) -> NodeRef<'_>;
    pub fn set<T: 'static>(&mut self, key: &PropKey<T>, v: T) -> Result<(), PropError>;   // §5.3 exact path
    pub fn set_value(&mut self, prop: PropId, v: &Value) -> Result<(), PropError>;
    pub fn set_state(&mut self, bits: StateBits, on: bool);
    pub fn ext_mut<T: 'static>(&mut self, key: &'static ExtKey<T>) -> ExtEntry<'_, T>;   // get/insert/remove; marks changed
    pub fn set_anchor(&mut self, anchor: Option<&str>) -> Result<(), SceneError>;      // Rust/Host-owned
    pub fn set_flags(&mut self, flags: NodeFlags, on: bool);   // LAYOUT_BOUNDARY, HIT_TRANSPARENT, SKIP_OFFSCREEN
    pub fn attach(&mut self, behavior: impl Behavior + KindDecl) -> NodeId;
    pub fn widget_mut<W: Widget>(&mut self) -> Option<WidgetMut<'_, W>>;
    pub fn invalidate(&mut self, what: Invalidate);            // MEASURE | LAYOUT | PAINT | A11Y | STYLE
}

/// Deref to the widget; dirty bits are those the caller declares (Masonry's rule: no dirtiness without a setter).
pub struct WidgetMut<'a, W> { /* … */ }
impl<W> WidgetMut<'_, W> { pub fn request(&mut self, what: Invalidate); }
```

Every mutation, including JS's, goes through one crate-private function per operation (`tree::insert`,
`props::set`, …): `ops.rs` calls `SceneMut` with `Origin::Js`. That is what makes the journal complete and the
invariants checkable in one place.

### 6.5 Observing changes

```rust
// scene/journal.rs
#[non_exhaustive]
#[derive(Clone, Debug)]
pub enum Change {
    Created   { node: NodeId, kind: KindId, owner: Owner },
    Inserted  { parent: NodeId, node: NodeId, before: Option<NodeId> },
    Removed   { parent: NodeId, node: NodeId },
    Destroyed { node: NodeId, kind: KindId },
    Prop      { node: NodeId, prop: PropId, origin: Origin },
    State     { node: NodeId, before: StateBits, after: StateBits },
    Ext       { node: NodeId, ext: ExtId, what: ExtChange },   // Added | Changed | Removed
    Anchor    { node: NodeId, root: RootIx },
    Layout    { node: NodeId, before: egui::Rect, after: egui::Rect },
    Root      { root: RootIx, what: RootChange },              // Visible(bool) | Viewport(ViewportId) | Moved
    Batch     { label: &'static str, tick: Tick },             // boundary marker
}

#[derive(Clone, Default)]
pub struct ChangeFilter {
    pub scope: Option<Scope>,            // a root or a subtree
    pub what: ChangeMask,                // CREATED | STRUCTURE | PROPS | STATE | EXT | ANCHOR | LAYOUT | ROOT
    pub kinds: Option<SmallVec<[KindId; 4]>>,
    pub ext: SmallVec<[ExtId; 4]>,
    pub props: SmallVec<[PropId; 4]>,
    pub origins: OriginMask,             // e.g. everything but Rust(self) to ignore one's own writes
}

pub struct Observer { /* id; drop unsubscribes */ }
pub struct ChangeSet {
    pub records: Vec<Change>,            // in order
    pub overflowed: bool,                // capacity hit: re-read what you need (AccessKit's full-tree fallback)
    pub damage: Damage,
}
impl ChangeSet {
    /// Coalesced per node: one entry per node with the union of what changed (MutationObserver records, reduced).
    pub fn by_node(&self) -> impl Iterator<Item = (NodeId, ChangeMask)> + '_;
}
pub struct Damage {
    pub relayout: SmallVec<[NodeId; 8]>, // boundaries laid out since the observer's last take
    pub repaint: SmallVec<[(egui::ViewportId, egui::Rect); 4]>,
    pub a11y: SmallVec<[NodeId; 8]>,
}
```

- **Pull, not push.** Observers are drained by their owner at a time it chooses (`take_changes`), never called back
  from inside a mutation. This is what keeps Rust mutation re-entrancy-free (§5.4), and it matches egui's own frame
  model.
- **Pay only for interest.** The journal records a change only if some live observer's filter could match it (a union
  `ChangeMask` and per-kind/ext/prop interest sets). With no observers, mutations cost a mask test. `Layout` records
  exist only for nodes some observer scopes (rect diffs are compared in `place`).
- **Ticks for cheap polling.** Every node's `core.changed` and every ext entry's `changed` tick allow "what changed
  since t" without a journal (`NodeRef::changed_since`, `Query::changed_since`), Bevy's change detection.
- **Capacity.** Each observer has a bounded queue (default 64k records). On overflow the queue is cleared and
  `overflowed` is set; the observer re-reads. Isolate batches up to 64k ops (docking §6.3) fit.
- **To JS** nothing changes: JS keeps R1 observation (`observe` op, `prop_changed`), which the same write paths feed.

## 7. Passes, invalidation and the frame

### 7.1 Dirty bits and propagation

```rust
bitflags! { pub struct Dirty: u16 {
    const STYLE = 1;  const MEASURE = 2; const LAYOUT = 4; const PAINT = 8; const A11Y = 16; const CHILDREN = 32;
    const DESC_STYLE = 64; const DESC_LAYOUT = 128;      // "some descendant has it" (Flutter's needsLayout walk)
}}
```

| Cause | Sets | Propagates | Queued in |
|---|---|---|---|
| prop with `layout` effect (style setter) | `LAYOUT` | `DESC_LAYOUT` up to the nearest layout boundary, clearing taffy caches on the way (today: to the root, `layout.rs:315`) | `root.dirty.relayout` (the boundary) |
| prop with `measure` effect (text) | `MEASURE` | none until remeasure: only a changed intrinsic size sets `LAYOUT` (EGUI.md §1.3 guard) | `root.dirty.remeasure` (dedup by the bit, fixing `backend.rs:567`) |
| prop with `paint` effect, state bit, ext with paint effect | `PAINT` | none | `root.dirty.repaint = true` → viewport repaint routing |
| prop with `a11y` effect, anchor | `A11Y` | none | `root.dirty.a11y` (consumed by custom a11y caches; egui rebuilds its own tree per frame) |
| insert/remove/splice | `CHILDREN` on the parent + `LAYOUT` | as `LAYOUT` | as `LAYOUT` |
| class, inline style, state bit used by a selector, attribute | `STYLE` (CSS.md §2.5 invalidation maps) | `DESC_STYLE` up for the walk | `root.dirty.restyle` |
| scale or font change (viewport move, token reload) | `MEASURE` on every leaf of the root, `PAINT` | — | per root (docking §5.6: that window or subtree only) |

**Layout boundaries.** A node is a boundary when its size cannot depend on its content: the root of a slot (definite
size from the `Ui`, `layout.rs:330`), a node whose style has definite width and height, a row of a virtualiser with a
known extent, a node with `contain: size layout` (CS5e maps the CSS property to `NodeFlags::LAYOUT_BOUNDARY`), or a
provider with `Caps::LAYOUT_BOUNDARY`. Relayout starts at the boundary with its last available size
(`compute_root_layout(tree, boundary, last_size)`), so a label changing inside a 10k-node root re-lays out the
label's boundary, not the root. This is Flutter's relayout boundary and Blink's containment applied to taffy (§3).

### 7.2 The frame, with the scene's steps

ARCH-EMBED §5.1 is the frame; the scene adds the steps marked **S**:

```text
App::logic (pass 0; cadatomic orders its own steps around embed.logic, seam 4)
  cadatomic: Jobs::drain → scene.write(…) (models, Op states) → embed.logic(ctx) → take_changes(observers)
  embed.logic:
    1 flush last outbox                        (no scene change)
    2 queued Rust commands: mount/props/…      SceneMut(Host)
  S 2b deferred mutations (SceneHandle::defer) queued during last frame's walk; JS sees them in step 4
    3 visibility & viewport                    Root changes; hidden roots drop out of 6–8
    4 checkpoint: JS ops                       SceneMut(Js) through ops.rs; journal
    5 rAF of visible roots
  S 6 state providers                          StateProvider::update (Rust), e.g. Op → PENDING bits
  S 7 restyle STYLE-dirty nodes (CS5e)         computed style → interned taffy Style → MEASURE/LAYOUT/PAINT
    8 remeasure MEASURE-dirty leaves           egui fonts; size change → LAYOUT up to boundary
    9 route repaints                           per root viewport (request_repaint_of)
   10 busy probe                               + Busy::PROVIDER from Widget::busy() and StateProvider::busy() (R8)
App::ui → dock → pane_ui → embed.show_slot(ui, slot)
  a  gate: hidden/empty/faulted/mounting
  S  b prepare: virtualisers place their window of rows (Prepare phase, own shadow subtree only, §8.2)
     c layout: relayout boundaries of this root, or all of it if the available size changed; place rects;
       ink bounds; Layout changes for observed nodes
     d walk (paint order): cull by ink bounds; state bits from ctx.read_response (hover, pressed, focus: last
       pass's interaction, egui's own model); Behavior::before_ui; leaf/widget ui or children; Behavior::after_ui;
       record hit rect; a11y
  S  e ScenePass::overlay for this root (§7.5)
     f post: buffered own-node writes applied; anchors' screen rects; focus-within; outbox; Damage
```

State bits found in step d (hover, press, focus) restyle that node in place before it paints (CS5e's "same-frame
state"); if the new style changes layout, the node is marked and the change shows in the next frame's layout, which is
also egui's own one-frame interaction lag. Hovering 1000 rows therefore never enters `logic` or JS.

Seam item 4 makes cadatomic the owner of the order, so the scene's hooks are calls cadatomic can see and reorder
(state providers can also simply be code cadatomic runs before `embed.logic`). `prepass_plugin = false` is
cadatomic's setting; with it, deferred windows run `logic` only through the root frame (ARCH-EMBED §5.2).

### 7.3 Custom passes through providers

The `Widget` trait from ARCH-EMBED §8.2 (D3(a): `measure`/`ui`/`a11y` + `cached_shapes`) keeps its required methods.
This design adds only defaulted methods and capability bits, so D3 holds:

```rust
pub trait Widget: 'static {
    // ARCH-EMBED §8.2 + R8
    fn measure(&mut self, cx: &mut MeasureCx, avail: AvailableSpace) -> egui::Vec2;
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx) -> egui::Response;
    fn a11y(&self, cx: &A11yCx, node: &mut accesskit::Node) {}
    fn caps(&self) -> Caps { Caps::NONE }
    fn busy(&self) -> bool { false }
    // this design (all optional; enabled by a Caps bit so the scene skips the call otherwise)
    /// Caps::CUSTOM_LAYOUT: lay out light or shadow children yourself (taffy is used for the rest of the tree).
    fn layout(&mut self, cx: &mut LayoutCx, inputs: LayoutInput) -> LayoutOutput { cx.default_layout(inputs) }
    /// Caps::PREPARE: before layout, with last frame's viewport of this node; virtualisers bind rows here.
    fn prepare(&mut self, cx: &mut PrepareCx) {}
    /// Caps::HIT_TEST: sub-node hit-testing (rows, handles, ports) in node-local coordinates.
    fn hit_test(&self, cx: &HitCx, local: egui::Pos2) -> Option<Hit> { None }
    /// Caps::VIRTUAL: painted rows exposed to a11y, anchors, queries and hit-tests (§8.2).
    fn virtual_children(&self) -> Option<&dyn VirtualChildren> { None }
    /// Attach/detach to a root (ids assigned, root visible) — for widgets that hold GPU or model subscriptions.
    fn attached(&mut self, cx: &mut AttachCx) {}
    fn detached(&mut self, cx: &mut AttachCx) {}
}

pub struct LayoutCx<'a> { /* the taffy adapter */ }
impl LayoutCx<'_> {
    pub fn children(&self) -> &[NodeId];                                             // light, or shadow when it has one
    pub fn measure(&mut self, child: NodeId, known: Size<Option<f32>>, avail: Size<AvailableSpace>) -> Size<f32>;
    pub fn layout(&mut self, child: NodeId, inputs: LayoutInput) -> LayoutOutput;    // taffy compute_child_layout
    pub fn place(&mut self, child: NodeId, at: egui::Pos2, size: egui::Vec2);
    pub fn default_layout(&mut self, inputs: LayoutInput) -> LayoutOutput;           // flex/grid/block per style
}

pub struct WidgetCx<'a> { /* ARCH-EMBED §8.2 members, plus: */ }
impl WidgetCx<'_> {
    pub fn scene(&self) -> SceneRef<'_>;              // read everything else while this widget is borrowed
    pub fn node(&self) -> NodeRef<'_>;                // this node (its payload reads as Vacant)
    pub fn ext<T: 'static>(&self, key: &'static ExtKey<T>) -> Option<&T>;
    pub fn ext_mut<T: 'static>(&mut self, key: &'static ExtKey<T>) -> BufferedExt<'_, T>;   // applied in step f
    pub fn children(&mut self, ui: &mut egui::Ui);    // walk light/shadow children at their rects
    pub fn child(&mut self, ui: &mut egui::Ui, id: NodeId); // one child, e.g. a row of a virtual pool
    /// An AccessKit node for a painted row under this node (§8.2), with set size and position filled in.
    pub fn a11y_row(&mut self, id: egui::Id, index: usize, f: impl FnOnce(&mut accesskit::Node));
}
```

**How a widget gets `&mut self` and `&Scene` at once, without `unsafe`.** Before calling a provider method the walk
moves the payload out of its column (`std::mem::replace(&mut payload[idx], Payload::Vacant)`), hands the widget a cx
holding `&Scene`, and puts the payload back afterwards. Own-node writes (state bits, ext, `cx.set`) are buffered in
the cx and applied in step f. This is Masonry's per-widget mutable access with a read-only rest of the tree, and it
is also what `catch_unwind` needs (ARCH-EMBED §6.2: a provider panic faults one root; the payload is dropped, not
left half-borrowed). Built-in leaves keep today's direct `&mut W` path (`render.rs:424`).

**Behaviours** (ARCH-EMBED §8.5) get two hooks around the host's `ui`, plus the event-filter hook EV1 adds for SV6:

```rust
pub trait Behavior: 'static {
    fn before_ui(&mut self, host: NodeRef<'_>, cx: &mut BehaviorCx) {}          // gate input (tutorials), set state
    fn after_ui(&mut self, host: NodeRef<'_>, resp: &egui::Response, cx: &mut BehaviorCx) {}
    fn paint_over(&mut self, host: NodeRef<'_>, painter: &egui::Painter, cx: &mut BehaviorCx) {}   // decorations
    fn filter(&mut self, ev: &mut SceneEvent, phase: Phase, cx: &mut BehaviorCx) -> Filter { Filter::Pass } // EV1
}
```

**State providers** are the "state provider" of the user's decision: Rust code that owns some provider state bits or
ext columns and updates them in bulk in `Logic`, step 6.

```rust
pub trait StateProvider: 'static {
    fn name(&self) -> &'static str;
    /// Called once per logic pass when `wants()` reports pending work (no call when idle).
    fn update(&mut self, scene: &mut SceneMut<'_>, cx: &ProviderCx);
    fn wants(&self) -> bool;
    fn busy(&self) -> bool { false }       // counts toward settled() (R8's provider bit)
}
```

### 7.4 Hit-testing

- **Interaction stays egui's.** egui hit-tests against the previous pass's widget rects at `begin_pass`
  (`egui-0.37.0/src/context.rs:515-535`), skipping non-interactable areas. The walk keeps registering widgets at their
  layout rects (`render.rs:412-421`), so egui's picking, drag capture and focus keep working unchanged.
- **The scene keeps a hit list.** During the walk each drawn node with `Sense` or `Caps::HIT_TEST` appends
  `(NodeId, screen rect, layer, clip)` to the viewport's `HitList` in paint order. `SceneRef::hit_test(viewport, pos)
  -> HitPath` walks it backwards, asks `Widget::hit_test` for sub-node hits, honours `HIT_TRANSPARENT` (CSS
  `pointer-events: none`), and returns `Vec<Target>` from topmost down. A provider's answer is
  `enum Hit { Node, Part(u64), Miss }`: `Part(key)` becomes `Target::Virtual(VNodeId { host, key })`, a row key, a
  handle index or a pick id that only the provider interprets. Uses: typed picking under the pointer, drop
  targets "from the model plus last laid-out rects, never during painting" (docking §5.4), tutorial gates, MCP "what
  is here".
- **No hit-testing in JS.** Events still come from egui responses into the outbox.

### 7.5 Overlays (SV7) and layers

- **`ScenePass`**, registered by a provider, runs after a root's walk (step e) or once per viewport after all roots,
  with a painter on a chosen `egui::Order`:

```rust
pub trait ScenePass: 'static {
    fn scope(&self) -> PassScope;                            // Root(RootIx) | Viewport(ViewportId) | AllViewports
    fn overlay(&mut self, ui: &mut egui::Ui, scene: SceneRef<'_>, cx: &mut PassCx) {}
    fn after_layout(&mut self, scene: SceneRef<'_>, damage: &Damage) {}
}
pub struct PassCx<'a> { /* … */ }
impl PassCx<'_> {
    pub fn viewport(&self) -> egui::ViewportId;
    pub fn root(&self) -> Option<RootIx>;               // for PassScope::Root
    pub fn request_repaint(&self);                      // this viewport only
}
```

- **Overlay roots** are Rust-created free trees (`TreeRoot::Overlay`) mounted like slots and drawn by
  `show_overlay(ctx, root, rect, order)` in an `egui::Area` with `interactable(false)` unless a node captures input,
  so the content under it (the 3D viewport) keeps receiving pointer events. JS can mount into them like any slot.
- `NodeRef::screen_rect` and `scene.by_anchor` give overlay code the anchored rects (ARCH-EMBED `anchor_rect`).

## 8. Optimisation hooks

### 8.1 Containment and culling

| Hook | Effect | Set by |
|---|---|---|
| `LAYOUT_BOUNDARY` | relayout stops here (§7.1) | definite size, `contain: size layout`, `Caps::LAYOUT_BOUNDARY`, virtual rows |
| `SKIP_OFFSCREEN` | subtree not walked when its ink bounds are outside the clip; not laid out while offscreen if its size is known (CSS `content-visibility: auto`) | CS5e, `NodeMut::set_flags` |
| ink bounds | culling uses the subtree's paint bounds, not the layout rect, so overflowing children are not lost (today's container cull, `render.rs:398-401`, would drop them) | computed in `place` |
| hidden roots | no prepare, layout, walk, rAF (ARCH-EMBED §5.3) | `set_visible` |

### 8.2 Virtualisation: realised rows and painted rows

| Tier | What the scene holds | JS per scroll frame | Fits |
|---|---|---|---|
| T1 `virtuallist` (LS1) | recycled row subtrees, JS `setup`/`bind` before the pass | bind calls for new rows only | JS arrays ≲ 5k rows (models §6.3) |
| **Realised rows (Rust)** | a pool of shadow row subtrees bound in `prepare` from a Rust model | 0 | rows with real widgets: property grid, forms, tables with editors |
| **Painted rows (Rust)** | no nodes per row; the widget paints rows and implements `VirtualChildren` | 0 | 10k–1M rows of text and icons: model tree, problems, logs |

```rust
pub struct PrepareCx<'a> { /* … */ }
impl PrepareCx<'_> {
    pub fn host(&self) -> NodeId;
    pub fn visible_rect(&self) -> egui::Rect;                // the node's rect ∩ its clip, last frame (outer scrolling)
    pub fn scene(&mut self) -> SceneMut<'_>;                 // restricted to this node's shadow subtree
}

/// Owned by the widget (a `#[state]` field); its rows are the host's shadow subtree.
pub struct RowPool { /* key → NodeId; free rows per template */ }
impl RowPool {
    pub fn new() -> RowPool;
    /// The row bound to `key`: the same node while the key stays requested, else a free row of `template`,
    /// else `build` (once per new row).
    pub fn row(&mut self, s: &mut SceneMut<'_>, key: u64, template: u32,
               build: impl FnOnce(&mut SceneMut<'_>) -> NodeId) -> NodeId;
    pub fn bound_in_order(&self) -> impl Iterator<Item = NodeId> + '_;   // request order of this prepare
    pub fn release_unused(&mut self, s: &mut SceneMut<'_>);             // unrequested rows → free list (hidden, not destroyed)
}

pub trait VirtualChildren {
    fn len(&self) -> usize;
    fn key(&self, row: usize) -> u64;
    fn row_of(&self, key: u64) -> Option<usize>;
    fn rect(&self, row: usize) -> Option<egui::Rect>;        // node-local; None when not laid out
    fn anchor(&self, key: u64) -> Option<Cow<'_, str>>;      // "row[feature:1234]"
    fn find_anchor(&self, anchor: &str) -> Option<u64>;
    fn row_a11y(&self, key: u64, node: &mut accesskit::Node);  // role, name, level, expanded, selected
    fn scroll_to(&mut self, key: u64);                       // tutorials and MCP "scroll into view"
}
```

- **Identity survives recycling.** A realised row is bound by model key; a key that stays visible keeps its node, so
  focus, egui memory and a11y identity survive refilters (models §5.5). A row's egui id is
  `host_id.with(("row", key))`, not its pool slot.
- **Row geometry** comes from the widget's row-height model (uniform, estimated, measured; a Fenwick tree for
  O(log n), models §5.5). The virtualiser reports an estimated content extent to taffy and never measures rows into its
  parent's intrinsic size (slint §4: 8 → 52 ms for 5000 rows when it does).
- **Accessibility.** egui builds the AccessKit tree from widgets each frame; the walk emits nodes for realised and
  visible painted rows, with `size_of_set`/`position_in_set` from `VirtualChildren` (models §5.5, SV40). Field names to
  verify against accesskit 0.25 in SG5.
- **Queries and anchors** see painted rows through `Query::include_virtual` and `by_path("…/row[feature:1234]")`,
  which ask `find_anchor` and return `Target::Virtual`.

### 8.3 Paint caching

- ARCH-EMBED's opt-in `cx.cached_shapes(version, |painter| …)` stores `Arc<[Shape]>` per node in a `PAINT_CACHE`
  dense column keyed by `(version, size, theme generation, pixels_per_point)`; `PAINT` dirt clears it.
- egui tessellates every shape every frame (`Context::tessellate`, `context.rs:3358`), so a shape cache saves shape
  construction and text layout, not tessellation. framework §8 says to measure first; SG6's paint-cache benchmark does.
- **Layers to textures** (the real GPU saving for canvases, framework §8 "layer boundaries") belong to the provider:
  a `Caps::LAYER` widget renders into its own texture and the scene tells it when its version, size or scale changed
  (`attached`/`PAINT` dirt). The scene does not own wgpu.

### 8.4 Batching

- `SceneMut::batch` (one tick, one journal batch, one invariant check in debug).
- `splice` for list updates (one `CHILDREN` dirt, one relayout).
- `RowPool` reuse (no create/destroy while scrolling).
- Isolate batches arrive as one `batch` with coalesced `set_prop` (docking §6.3).

### 8.5 Memory

| Item | v1 (SG1) | After SG6 |
|---|---|---|
| `NodeCore` | ≈ 104 B | ≈ 104 B |
| children `Vec` header | 24 B (+ heap for parents) | same |
| `LayoutBox` | ≈ 1 KB (inline `Style` + `Cache` + 2 `Layout`s) | ≈ 350 B: `Arc<Style>` interned (most nodes share one of a few dozen computed styles), `CompactCache` (final entry + 2 measure entries through the `CacheTree` trait, `taffy-0.15.0/src/tree/traits.rs:260`) |
| payload | 24–140 B (`W` largest variant; boxed provider) | same |
| cold | 8 B pointer, boxed only when used | same |
| **per node** | ≈ 1.3 KB (100k ≈ 130 MB) | **≤ 640 B (100k ≤ 64 MB)** |

A test (`scene_memory`) creates 100k nodes in the shape of the SC1 panels and asserts the budget through
`SceneStats::bytes()` (columns' `capacity × size_of`, plus heap estimates for strings and children).

## 9. Threading and windows

| Thread | Scene access |
|---|---|
| UI thread | everything: `SceneHandle` is `Rc`, `!Send`, as `Embed` (ARCH-EMBED §6.1) |
| deferred viewport callbacks | on the UI thread through `embed::with_current` / `SlotPainter` (ARCH-EMBED §5.2). In the `EmbeddedWindow` class the dock calls `pane_ui` through the host reference, never the thread-local (R5) |
| any thread | `SceneSnapshot` (owned, `Send`, built on the UI thread on request), `BusyProbe`, wakes |
| isolates (later) | none: op batches applied in `logic` through `HandleMap` |

**No off-thread live reads.** A persistent immutable tree that other threads could read (React Native Fabric's
shadow-tree revisions, §3) costs a structural copy per commit and buys nothing here: layout, paint and JS all run on
the UI thread, and the off-thread readers (MCP server, test reporters, `IdleBarrier`) are served by on-demand
snapshots and atomics (§15 S5).

**Windows.** One scene serves every viewport; a root's `viewport` field routes repaints and records where its rects
live. Focus is per viewport in egui (`memory/mod.rs:110`), so `set_viewport` asks the root's last focused node to
re-request focus (ARCH-EMBED §7.1; R7 routes the call through the host). Hit lists are per viewport.

## 10. End to end: three cadatomic features

### 10.1 Typed-picking overlay over the 3D viewport

Goal (framework §4.3): while a command session acquires a slot of type `T`, every node presenting an assignable
value, in every panel and in the 3D viewport, shows the pick-target state; hovering one previews it; clicking fills
the slot through the command path.

```rust
// cadatomic-ui/src/svelte/pick.rs   (no unsafe; all statics are Sync)
pub static PRESENTS: ExtKey<TypedRef> = ExtKey::new("cad.presents");
svelters_egui::state_bits! { pub mod cad { PICK_TARGET = "pick-target", PICK_REJECTED = "pick-rejected", PICK_HOVER = "pick-hover" } }

/// Marks pickable nodes in bulk whenever the acquiring slot type changes (logic step 6).
pub struct PickProvider { want: Option<TypeKey>, dirty: bool, registry: Arc<TypeRegistry> }   // cadatomic types
impl StateProvider for PickProvider {
    fn name(&self) -> &'static str { "cad.pick" }
    fn wants(&self) -> bool { self.dirty }
    fn update(&mut self, scene: &mut SceneMut<'_>, _: &ProviderCx) {
        self.dirty = false;
        let hits: Vec<(NodeId, bool)> = scene.read().with_ext(&PRESENTS)            // O(presenting nodes), not O(n)
            .map(|(n, r)| (n.id(), self.want.is_some_and(|t| self.registry.assignable(r.ty, t)))).collect();
        scene.batch("pick.mark", |s| for (id, ok) in hits {
            let mut n = s.node(id).unwrap();
            n.set_state(cad::PICK_TARGET, self.want.is_some() && ok);
            n.set_state(cad::PICK_REJECTED, self.want.is_some() && !ok);
        });
    }
}
```

- **Svelte side.** Panels present values with the `use:presents` behaviour (or a provider kind's own `presents`
  prop) which writes `PRESENTS` on its host node; styles react to `:pick-target`/`:pick-rejected` with no JS
  (CS5e). The tree and the property grid present per row through `VirtualChildren` + their own pick hooks (10.3).
- **The 3D viewport** is a provider kind `cad-viewport` (`Caps::LAYER | Caps::HIT_TEST`): it renders through
  cadatomic's wgpu callback, and its `hit_test` answers `Hit::Part(pick_id)` from cadatomic's GPU pick buffer
  (per-triangle pick ids, native-ext §6.6), with no engine call per hover (native-ext §5.4). The scene reports it as
  `Target::Virtual(VNodeId { host: viewport, key: pick_id })`; only cadatomic knows that the key is a `TypedRef` id.
- **The overlay.** A `ScenePass` over all viewports:

```rust
impl ScenePass for PickOverlay {
    fn scope(&self) -> PassScope { PassScope::AllViewports }
    fn overlay(&mut self, ui: &mut egui::Ui, scene: SceneRef<'_>, cx: &mut PassCx) {
        let Some(pos) = ui.ctx().pointer_hover_pos() else { return };
        let path = scene.hit_test(cx.viewport(), pos);                          // last frame's hit list, topmost first
        let hovered: Option<TypedRef> = path.iter().find_map(|t| match *t {
            Target::Node(id) => scene.get(id).and_then(|n| n.ext(&PRESENTS)).cloned(),
            // A part of a provider: ask the provider through typed access (viewport pick id, tree row key).
            Target::Virtual(v) => scene.get(v.host).and_then(|h| {
                h.widget::<Viewport3d>().map(|w| w.typed_ref(v.key))
                    .or_else(|| h.widget::<ModelTree>().map(|w| w.typed_ref(v.key)))
            }).flatten(),
        });
        if let Some(r) = hovered {
            // Highlight every place that shows the same value, in every panel: one column scan.
            for (n, _) in scene.with_ext(&PRESENTS).filter(|(_, x)| **x == r) {
                if let Some((vp, rect)) = n.screen_rect() && vp == cx.viewport() {
                    ui.painter().rect_stroke(rect, 2.0, self.theme.pick_hover, egui::StrokeKind::Outside);
                }
            }
            self.intents.push(PickPreview(r));                                    // cadatomic's queue, drained in logic
        }
    }
}
```

- **Overlay labels in 3D** (dimension values, callouts) are Svelte in an overlay root (`TreeRoot::Overlay`) over the
  viewport's rect: Rust writes each label's projected position as `left`/`top` props of absolutely positioned
  `box`es after a camera change (one `batch`, each label a layout boundary; absolute positioning needs taffy 0.15's
  hoisting, SG4), and the Area is non-interactable except
  for nodes that `Sense` input, so the viewport keeps the pointer.
- **Click** arrives as an egui response on whatever was clicked; cadatomic's behaviour or the viewport emits a draft
  event (command path), never a scene mutation of documents.
- **Cost per hover frame**: one hit-list walk (≤ a few hundred rects), one column scan over presenting nodes (~1–5k),
  no JS.

### 10.2 Property grid with 2k rows

```svelte
<!-- cadatomic/ui/src/Inspector.svelte -->
<script lang="ts">
  import { inspector } from 'svelters:app/inspect';           // Obj<GridModel> from a #[svelters::store]
</script>
<property-grid model={inspector.model} anchor="inspector" density="compact"
               onedited={(e) => inspector.commit(e.detail)} />
```

```rust
// cadatomic-ui/src/widgets/property_grid.rs
#[svelters::widget(namespace = "cad", tag = "property-grid", caps = PREPARE | CUSTOM_LAYOUT | VIRTUAL | FOCUSABLE)]
pub struct PropertyGrid {
    #[prop] model: Obj<GridModel>,               // Rust model (reflection-driven rows), never copied into JS
    #[prop(default = "compact")] density: Density,
    #[event] edited: Event<EditIntent>,          // one event per committed edit
    #[state] heights: RowHeights,                // uniform per density; Fenwick for sections
    #[state] pool: RowPool,
    #[state] visible: Range<usize>,
    #[state] scroll_vp: egui::Rect,              // last frame's scroll viewport (content coordinates)
}

impl Widget for PropertyGrid {
    fn measure(&mut self, _: &mut MeasureCx, avail: AvailableSpace) -> egui::Vec2 {
        egui::vec2(avail.width_or(320.0), self.heights.total())   // estimated extent; rows never measured into it
    }
    fn prepare(&mut self, cx: &mut PrepareCx) {
        let m = self.model.borrow();
        self.visible = self.heights.range(self.scroll_vp.y_range().expand(64.0));   // ~40 rows + margin
        let mut s = cx.scene();                                   // limited to this node's shadow subtree
        for row in self.visible.clone() {
            let r = m.row(row);                                   // name, unit, editor kind, value, problems
            let id = self.pool.row(&mut s, r.key, r.editor as u32, |s| build_row(s, r.editor));  // once per row
            let mut n = s.node(id).unwrap();
            bind_row(&mut n, &r);                                 // NodeMut::set on label/editor props; unchanged = no-op
            n.ext_mut(&PRESENTS).set(r.typed_ref.clone());        // rows present their value (10.1)
        }
        self.pool.release_unused(&mut s);
    }
    fn layout(&mut self, cx: &mut LayoutCx, inputs: LayoutInput) -> LayoutOutput {
        let width = cx.definite_width(&inputs);                   // LayoutCx helpers over taffy's LayoutInput
        for (row, id) in self.visible.clone().zip(self.pool.bound_in_order()) {
            let h = self.heights.height(row);
            cx.layout(id, cx.exact(width, h));                    // each row a layout boundary
            cx.place(id, egui::pos2(0.0, self.heights.offset(row)), egui::vec2(width, h));
        }
        cx.output(egui::vec2(width, self.heights.total()))
    }
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx) -> egui::Response {
        let out = egui::ScrollArea::vertical().id_salt(cx.id()).show_viewport(ui, |ui, vp| {
            self.scroll_vp = vp;                                  // the next prepare binds for it
            for id in self.pool.bound_in_order() { cx.child(ui, id); }
        });
        ui.interact(out.inner_rect, cx.id(), egui::Sense::hover())
    }
    fn virtual_children(&self) -> Option<&dyn VirtualChildren> { Some(&self.rows_view) } // a11y set-size, anchors
}
```

- **Nodes**: ~40 visible rows × (row box, label, editor, unit chip, problem line) ≈ 200 shadow nodes, regardless of
  the 2k rows. Editors are provider kinds (`quantity-field`) or built-ins (`checkbox`, `combo-box`), so they keep
  focus, IME, a11y names (K-6) and CSS states.
- **Scrolling** re-binds rows in `prepare`: no JS, no create/destroy, `set` writes only for changed values (the
  setter compares), relayout only of rebound rows (each a boundary).
- **Edits**: the editor's `accepted` event is handled in Rust (`#[event]` on the editor → the grid's `edited`), and
  the Svelte `onedited` handler runs once per commit. A command-draft binding (scene-node §3) would make even that
  Rust-only.
- **Anchors**: `inspector/row[param:extents]` resolves through `find_anchor` even for rows scrolled out of view; MCP
  and tutorials call `scroll_to` first.

### 10.3 Model tree that bypasses JS for scrolling

```svelte
<model-tree model={doc.featureTree} anchor="tree" bind:selection onactivate={(e) => open(e.detail)} />
```

```rust
#[svelters::widget(namespace = "cad", tag = "model-tree", caps = PREPARE | VIRTUAL | HIT_TEST | FOCUSABLE)]
pub struct ModelTree {
    #[prop] model: Obj<TreeModel>,                 // cadatomic's RowSet/Model (models §4.2), Rust
    #[prop(bind)] selection: Vec<RowKey>,          // exact: written only via cx.set (SV46 deltas later)
    #[event] activate: Event<RowKey>,
    #[state] heights: RowHeights, #[state] expanded: HashSet<RowKey>, #[state] scroll_anchor: (RowKey, f32),
    #[state] galleys: GalleyCache,                 // per (row key, text hash, width, scale)
    #[state] theme: Arc<Theme>,                    // cadatomic's role tokens
}

impl Widget for ModelTree {
    fn measure(&mut self, _: &mut MeasureCx, a: AvailableSpace) -> egui::Vec2 { egui::vec2(a.width_or(240.0), self.heights.total()) }
    fn prepare(&mut self, cx: &mut PrepareCx) {
        // Apply model deltas drained by cadatomic in logic (Jobs::drain → TreeModel::apply); keep the scroll anchor.
        self.heights.sync(&self.model.borrow(), &self.expanded);
    }
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx) -> egui::Response {
        let m = self.model.borrow();
        let out = egui::ScrollArea::vertical().id_salt(cx.id()).show_viewport(ui, |ui, vp| {
            for row in self.heights.range(vp.y_range()) {           // only visible rows; O(log n) lookup
                let r = m.row(row);
                let rect = self.heights.rect(row, ui.max_rect());
                let id = cx.id().with(("row", r.key));                // stable per key, not per position
                let resp = ui.interact(rect, id, egui::Sense::click());
                let picked = cx.ext(&MODEL_PICK).map(|p| p.state_of(r.key));   // cadatomic's ext on this node
                paint_row(ui.painter(), rect, &r, &mut self.galleys, &self.theme, resp.hovered(),
                          self.selection.contains(&r.key), picked);
                if resp.clicked() { cx.set(&mut self.selection, vec![r.key]); }
                if resp.double_clicked() { cx.emit(&self.activate, r.key); }
                cx.a11y_row(id, row, |node| self.row_a11y(r.key, node));  // tree item, level, expanded, set size
            }
        });
        ui.interact(out.inner_rect, cx.id(), egui::Sense::hover())
    }
    fn hit_test(&self, cx: &HitCx, local: egui::Pos2) -> Option<Hit> {
        let row = self.heights.row_at(local.y)?;
        Some(Hit::Virtual(self.model.borrow().key(row)))            // → Target::Virtual(VNodeId{host, key})
    }
    fn virtual_children(&self) -> Option<&dyn VirtualChildren> { Some(self) }
}
```

- **Scrolling** runs no JS and creates no nodes: the frame paints visible rows from the Rust model with cached
  galleys. Per frame ≈ 40 rows × (interact + a few shapes): well under 1 ms (EGUI.md §1.2 measured 1.8–3.5 ms for a
  full dense screen of 380 nodes).
- **JS** sees only `selection` changes (exact, one `prop_changed` per click) and `activate` events.
- **Picking**: `cx.ext(&MODEL_PICK)` reads an extension cadatomic keeps on the tree node, keyed by model key (the model owns "pickable" as a
  row-set column, models §5.9), so 100k rows "cost nothing extra".
- **Anchors and tests**: `tree/row[feature:1234]` resolves through `VirtualChildren::find_anchor`;
  `harness.get_by_label("Extrude 1")` finds the AccessKit node built for visible rows.
- **Expand/collapse** changes `heights` (Fenwick update, O(log n)) and the scroll anchor keeps the anchored row in
  place: MX3's ≤ 2 ms and MX4's zero pixel movement are properties of the widget's height model, which the scene does
  not constrain.

## 11. Testing and debugging

| Tool | What | Where |
|---|---|---|
| `scene.dump(root, DumpOpts)` | stable text tree: `#id <tag> anchor=… [state] {rect} props…`, optional ext columns by name; golden tests | `scene/dump.rs` |
| `Scene::validate()` | invariants after every `batch` in debug builds: parent/child symmetry, `root` cache, no cycles, dirty-bit consistency (a `DESC_LAYOUT` chain reaches a boundary), anchors unique per root, ext columns only on live slots, owners respected | `scene/mod.rs` |
| op-sequence property test | random JS-like and Rust op interleavings against a reference model (a plain `Vec` tree), then `validate` and dump equality | `tests/scene_props.rs` |
| op-log replays | every existing svelters-egui replay runs unchanged (EG0 gate) and with a Rust observer attached (journal completeness: replaying the journal rebuilds the same tree) | `tests/oplog_replay.rs` |
| counters | `SceneStats`: live nodes per kind, bytes, layouts, boundaries laid out, leaves measured, nodes walked/culled, journal records, deferred writes | `Embed::stats` |
| `SceneSnapshot` | owned, `Send`, serde: nodes with tag, anchor, state names, rects, selected props and ext debug strings | MCP `ui.tree`/`ui.inspect` (TK2), test failure reports |
| F12 inspector | an egui overlay `ScenePass` showing the tree, the hovered node's dump, dirty bits, boundaries | dev builds (EGUI.md §4) |

The MCP and selector surface (CS2/TK2) is a consumer of this API, not a separate path: `query` → `scene.select`,
`inspect` → `snapshot`, `wait_idle` → `settled()`.

## 12. What changes in today's code

| Where | Today | Change | Milestone |
|---|---|---|---|
| `scene.rs:38-42` | `Handle = svl_handle`, `MAX_GEN = 1 << 29` | `NodeId(NonZeroU64)`, retire at 2^28 (§4.2) | SG1 |
| `scene.rs:114-143` | one fat `Node` | `NodeCore` + `LayoutBox` + `Payload` + `NodeCold` columns | SG1 |
| `scene.rs:167-245` | `Arena` with `Option<Node>` slots; O(n) `len`/`handles` | `SlotTable` + columns; O(1) `len`; kind index | SG1 |
| `scene.rs:247-285` | `new_node` builds `W`, `Common`, egui id | `tree::create` (no egui id until attach), payload by kind registry | SG1 |
| `scene.rs:267` | `Id::unique(("svl", handle))` | `ids.rs` derivation at attach (ARCH-EMBED §4.4) | EM1 (uses SG1's attach hook) |
| `scene.rs:340-423` | `set_prop` → `Dirty { style, measure }` | prop effects from the kind table → `Dirty` bits; origin; `explicit` mask; journal | SG1 |
| `layout.rs:158-312` | taffy adapter over `Arena` | adapter over columns; `CacheTree` on `LayoutBox`; custom-layout dispatch for `Caps::CUSTOM_LAYOUT` | SG1, SG4 |
| `layout.rs:254-265`, `:303-311` | hoisting stubbed ("no absolute positioning in G1") | store hoisted children in `NodeCold::hoisted`; `is_out_of_flow` from style (needed by overlay labels, §10.1, and CS5e `position`) | SG4 |
| `layout.rs:314-323` | `invalidate` to the root | propagate to the nearest boundary; queue the boundary | SG1 |
| `layout.rs:325-352` | `compute(root)`; recursive `place` cloning children | `relayout(boundary)`; iterative place without clones; ink bounds; `Layout` journal | SG1 |
| `layout.rs:354-371` | `remeasure(&[Handle])` | per-root `MEASURE` queue | SG1 |
| `render.rs:390-422` | `paint` clones children, culls by rect | walk without clones, culls by ink, hit list, state bits from `read_response`, payload take-out for providers | SG1 (no clones, hit list), SG4 (providers) |
| `backend.rs:52-79` | `layout_dirty`, `remeasure`, `layout_size` on `Inner` | move into per-root `RootDirty` in the scene | SG1 |
| `backend.rs:143-162` | `with_scene(&Arena)`, `handles_of` | `Embed::scene() -> SceneHandle`; tests use `scene.of_kind` | SG1 |
| `backend.rs:436-444`, `:468-553` | detach/insert/remove/destroy in the backend | thin calls into `SceneMut` with `Origin::Js` (cycle check, owner check, root cache, journal) | SG1 |
| `backend.rs:555-571` | `set_prop` + `touched()` | `SceneMut::set_value`; repaint routing through dirty roots (ARCH-EMBED §5.5) | SG1 + EM1 |
| `backend.rs:688-703` | `observe` counts in a `HashMap` per node | `NodeCold::observe` small vec | SG1 |

## 13. Module split: reconciling EG0 with ARCH-EMBED §9

ARCH-EMBED §9 cuts `scene/{mod,node,props}.rs`. That is too coarse for this design: SCENE, WIDGETS, RUSTAPI, STYLE
and LISTS would all edit `node.rs`. The refined cut, still mechanical at EG0:

| File | Content | EG0 | Writer lane after |
|---|---|---|---|
| `scene/mod.rs` | `Scene`, `SceneCell`, phase, `validate` | today's `Arena` moved here | SCENE |
| `scene/id.rs` | `NodeId`, `SlotTable`, `VNodeId`, `Target` | handle math (`scene.rs:38-42`, `:178-232`) | SCENE |
| `scene/node.rs` | `NodeCore`, `LayoutBox`, `Payload`, `NodeCold`, `NodeFlags`, `Dirty`, `StateBits`, `Owner` | today's `Node` (`scene.rs:114-165`) | SCENE (RA1 adds payload variants) |
| `scene/tree.rs` | create/insert/remove/destroy/splice, root cache, roots table | ops bodies from `backend.rs:436-553` | SCENE |
| `scene/props.rs` | built-in get/set, `PropKey` tables | `scene.rs:287-493` | WIDGETS |
| `scene/ext.rs` | `ExtKey`, columns | empty | RUSTAPI (RA1) |
| `scene/journal.rs` | `Change`, `Observer`, `Damage` | empty | SCENE |
| `scene/query.rs` | indexes, traversal, cursor, `Query` | empty | SCENE; STYLE adds `select` (CS2) |
| `scene/dirty.rs` | dirty bits, boundaries, per-root queues | `layout.rs:314-323` + `backend.rs` flags | SCENE |
| `scene/access.rs` | `SceneHandle`, `SceneRef`, `NodeRef`, `SceneMut`, `NodeMut` | `with_scene` | SCENE |
| `scene/hit.rs` | hit lists, `hit_test` | empty | SCENE (EVENTS reads) |
| `scene/virt.rs` | `RowPool`, `VirtualChildren`, `PrepareCx` | empty | LISTS |
| `scene/dump.rs` | dumps, snapshots, `SceneStats` | empty | SCENE (TESTKIT reads) |
| `layout.rs` | taffy adapter (top-level, as ARCH-EMBED §9) | unchanged | SCENE (adapter), WIDGETS (measure), STYLE (computed → style) |
| `walk.rs`, `leaf.rs`, `ids.rs`, `a11y.rs`, `provider/*`, `embed/*`, `frame.rs`, `ops.rs`, `events.rs`, `style.rs` | as ARCH-EMBED §9 | as ARCH-EMBED §9 | as ARCH-EMBED §9; `provider/` gains `pass.rs` (`ScenePass`, `StateProvider`) |

EG0 stays "no behaviour change, `egui.json` byte-identical"; it only creates these files and moves code. A new
**SCENE** writer lane (or the EMBED lane's owner, if one agent does both) owns `scene/*` except `props.rs`, `ext.rs` and
`virt.rs`.

## 14. Milestones

| Id | Content | Size (lane-days) | Depends | Acceptance tests |
|---|---|---|---|---|
| **SG0** | The §13 file cut, inside EG0 | +0.5 on EG0 | — | EG0's gate |
| **SG1** | Columns and `NodeId`; owners and phases; tree ops through one path (`Origin`), root table and root cache; dirty bits, boundaries, per-root queues; walk and place without clones; `SceneHandle`/`SceneRef`/`NodeRef`/`SceneMut`/`NodeMut`, built-in `PropKey`s; `validate`, `dump`, `SceneStats`, size test | 5–6 | EG0 | every existing test and op-log replay unchanged; property test of random op interleavings; one label change in an 11k-node root lays out ≤ 1 boundary and measures 1 leaf (counters); `size_of::<NodeCore>() ≤ 112` |
| **SG2** | Extension columns (`ExtKey`, sparse and dense), state-bit registry hook-up with RA1, journal and observers with interest masks, ticks, `Damage` | 3–4 | SG1; RA1 shares `ExtKey` | an observer sees a replayable journal (rebuilds the same dump); no allocation per mutation with no observers (counting allocator); `with_ext` over 5k entries of 100k nodes ≤ 50 µs |
| **SG3** | Indexes and queries: kind sets, anchor maps, `Query` planner, traversal and cursor, hit lists and `hit_test` | 3 | SG1 (anchors from EM1) | kittest: query by anchor, kind, ext and state across two roots and two viewports; hit-test returns the topmost and honours `HIT_TRANSPARENT` |
| **SG4** | Provider passes: payload take-out, `WidgetCx::scene`, `Caps::CUSTOM_LAYOUT`/`PREPARE`/`HIT_TEST`, `Behavior::before_ui`/`paint_over`, `StateProvider`, `ScenePass`, overlay roots, taffy hoisting (`position: absolute`) | 4–5 | SG1, RA1 | a provider container lays out shadow children; a `StateProvider` toggles 5k bits in one batch with one repaint; an overlay root over a canvas passes pointer input through; a provider panic faults one root |
| **SG5** | Virtualisation: `RowPool`, `VirtualChildren`, virtual anchors, a11y set-size for rows, `Target::Virtual` in queries and hit-tests; reference `virtual-tree` and `property-grid`-shaped test kinds | 6–8 | SG4; shares rows/heights with LISTS LS1 | 10k-row painted tree scrolls with 0 JS entries and ≤ 1 ms scene time per frame; 1M rows with a uniform height model; 2k-row realised grid creates ≤ 60 rows' nodes and none while scrolling; anchors resolve for off-screen rows |
| **SG6** | Memory and paint: interned `Arc<Style>`, `CompactCache`, ink-bound culling, `SKIP_OFFSCREEN`, `cached_shapes` storage and its benchmark | 3–4 | SG1; CS5e for interning by computed style | `scene_memory`: 100k nodes ≤ 64 MB; cache oracle (cached equals uncached paint, slint SL1) |
| **SG7** | `SceneSnapshot`, F12 inspector pass, MCP `ui.tree`/`ui.inspect` data (TK2 consumes) | 2–3 | SG3 | snapshot round-trips through serde; inspector shows dirty bits and boundaries |

- **SC1 path:** EG0 (+SG0) → SG1 alongside EM1 → SG2 with RA1 → SG3. SG1 is the base EM1 and RA1 build on; EM1 lands
  ids and anchors in it, RA1 lands provider payloads and `ExtKey` in it. 11–13 days of SCENE work, in parallel with
  EM1/RA1 after EG0.
- **After SC1:** SG4 (needed by EV2b behaviours and cadatomic's first Rust panels), SG5 with LISTS (LS1 shares row
  pools and the height model), SG6 when the first 10k-node screen exists, SG7 with TESTKIT.

## 15. Open decisions (each taken by default as recommended, revisited later)

| # | Decision | Options | Recommendation |
|---|---|---|---|
| S1 | Node storage | (a) parallel columns per slot: hot `NodeCore`, `LayoutBox`, `Payload`, boxed cold, typed ext columns; (b) full SoA per field; (c) `bevy_ecs` | **(a)**: the walk touches `core` + `children` only; (b) spreads every access over a dozen arrays for little gain; (c) brings archetype moves, a scheduler and `Send` assumptions we do not need |
| S2 | Extension storage behind ARCH-EMBED's `ExtKey` API | (a) one sparse-set (or dense) column per key; (b) per-node `ExtBlock` vec (ARCH-EMBED §8.4 sketch) | **(a)**: picking, `Op` states and highlights iterate "all nodes with X"; the API is unchanged |
| S3 | Rust structural edits | (a) only in Rust/Host-owned subtrees (slots, overlay roots, shadow subtrees); JS-owned children are JS's; (b) anywhere | **(a)**: Svelte's runtime orders siblings by its own handles |
| S4 | Rust writes to JS-set props | (a) the exact (user-edit) path, announced, bound props stay in sync, unbound ones get overwritten (dev diagnostic); (b) forbidden; (c) an override layer | **(a)** now, **(c)** later for devtools and tutorials (SV30) |
| S5 | Change observation | (a) pull observers with filters, coalescing, overflow flag, plus change ticks; (b) push callbacks during mutation | **(a)**: no re-entrancy, zero cost without observers |
| S6 | Off-thread access | (a) on-demand owned snapshots, atomics for busy; (b) a persistent immutable tree (Fabric) | **(a)** |
| S7 | Children representation | (a) `Vec<NodeId>` per node; (b) intrusive sibling links | **(a)** (taffy indexes children); add an `index_in_parent` cache if a keyed-reorder benchmark shows O(n²) |
| S8 | Crate | (a) `svelters_egui::scene` module with egui-light core files; (b) a new `svelters-scene` crate now | **(a)**: one less crate to build on three machines; the cut keeps (b) a move |
| S9 | Mutation during the walk | (a) only the current widget's own node, buffered; everything else `defer`red to the next `logic`; (b) allowed | **(a)** (Masonry's rule; egui passes may be discarded and re-run) |
| S10 | Virtualisation tiers | (a) both realised-row pools and painted rows with `VirtualChildren`; (b) realised rows only | **(a)**: 1M-row trees cannot afford nodes per row; forms need real widgets |
| S11 | Memory budget and the taffy cache | (a) ≤ 640 B/node at 100k after SG6, with `Arc<Style>` interning and a compact `CacheTree` cache; (b) accept ~1.3 KB/node | **(a)**, measured by `scene_memory`; keep taffy's `Cache` until the measurement asks |
| S12 | Hit-testing | (a) egui keeps interaction; the scene keeps a paint-ordered hit list for queries and sub-node hits; (b) the scene hit-tests and feeds egui | **(a)**: egui's interaction model (drag capture, focus, layers) stays intact |

## 16. Stability contract for cadatomic

cadatomic pins svelters by sha (decisions, "Pins"). Within that, these are the promises:

**Stable** (changed only with a coordinator notice, a migration note in `STATUS.md` and one pin cycle of
deprecation; enums `#[non_exhaustive]`, traits extended only with defaulted methods):

- `NodeId` (`to_raw`/`from_raw`, generation semantics), `VNodeId`, `Target`, `KindId`, `PropId`, `Tick`.
- `SceneHandle` (`read`, `write`, `defer`, `observe`, `take_changes`, registrations); `SceneRef`, `NodeRef`, traversal
  iterators, `Cursor`, `Query` and its predicates; `by_anchor`/`by_path`; `hit_test`.
- `SceneMut`, `NodeMut` operations of §6.4 and the ownership rules of §5 (an operation may become *more* permissive).
- `ExtKey`, `ExtEntry`, `state_bits!`, `StateBits` built-in bit names (bit positions are not promised).
- `Widget` (ARCH-EMBED §8.2 + §7.3 here), `Behavior`, `StateProvider`, `ScenePass`, `VirtualChildren`, `RowPool`, the
  cx types' documented methods, `Caps` names.
- `Change`, `ChangeFilter`, `ChangeSet`, `Damage`, `Origin`, `Owner`.
- `PropKey` statics for built-in kinds (`scene::props::*`), and the ones RA1 generates.
- The frame contract: which phase allows what (§5.2), and the step order of §7.2 as seen from cadatomic's `logic`.

**Unstable** (behind the `unstable-scene` feature; may change in any sha):

- `NodeRef::layout()` and anything exposing taffy types; `LayoutCx` internals beyond §7.3.
- Dirty-bit values, `Dirty` itself, boundary heuristics (the *behaviour* "relayout is local" is stable; how the scene
  decides is not).
- `SceneStats` fields, the dump text format (goldens should be regenerated, not hand-edited), `SceneSnapshot` schema
  until TK2 freezes it.

**Internal** (not exported): columns, `Payload`, `W`, slot tables, journal storage, indexes, the taffy adapter, id
derivation internals beyond ARCH-EMBED §4.4's table.

## Sources

- svelters (`a0e1e768`): `crates/svelters-egui/src/{scene,layout,render,backend,kinds,lib}.rs`;
  `crates/svelters-abi/src/safe.rs` (`Backend`, host callbacks); `plans/architecture/backend-abi.md` (data model, M5
  list protocol, v2.6 observation); `plans/research/{ARCH-EMBED,ARCH-CRATES,EGUI,CADATOMIC,TRACKS,CSS}.md`.
- egui 0.37.0 sources (cargo registry): `context.rs` (`read_response` :1644, `request_repaint_of` :2145,
  `add_plugin` :2376, `tessellate` :3358, `accesskit_node_builder` :4203, `show_viewport_deferred` :4608, hit test at
  `begin_pass` :515-535), `hit_test.rs`, `widget_rect.rs`, `plugin.rs`, `memory/mod.rs`.
- taffy 0.15.0 sources: `src/tree/traits.rs` (`TraversePartialTree` :147, `LayoutPartialTree` :173, `CacheTree` :260,
  `RoundTree` :275, container traits :307-374), `src/tree/cache.rs` (`Cache` :214, `CACHE_SIZE = 9`),
  `src/style/mod.rs` (`CoreStyle` :77).
- cadatomic (read-only): every file under `docs/research/` dated 2026-10-22, in particular scene-node §1–§10,
  framework §2–§8, models §1, §4–§9, docking §4–§7, slint §2–§10, widgets §2.2, native-ext §5.4/§6.6, editor §4, §9;
  `docs/plan/00-decisions.md` (product direction, shell, docking, svelters direction, seam corrections R1–R12,
  architecture recommendations taken).
- Prior art: see §3.
