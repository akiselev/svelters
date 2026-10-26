# SKELTER-core: skelter-core (skelter batch 1, lane CORE)

Branch `ci/presto-core` from `ci/presto-base`. Brief: `plans/cloud/presto-b1/LANE-core.md`. Date: 2026-10-25.
Owns `crates/skelter-core/**`. No skelter-base changes.

## Cross-lane edits

- `crates/skelter-widgets/src/host.rs` (WIDGETS-BASIC), two lines so it builds against the coordinator's
  skelter-core additions: `..KindSpec::new(ws.tag)` in its `KindSpec` literal, and the new `Event::Timer` /
  `Event::A11y` arms mapped to its `w::Event::Timer` / `w::Event::A11y`. Its widgets still ask for timers
  through their own shim; wiring that to `EventCx::wake_at` is WIDGETS-BASIC's.

## Coordinator requests (2026-10-25), done

- WIDGETS-BASIC: (1) `NodeView::css()` (the engine's `ComputedStyle`); (2) `EventCx::wake_at`/`cancel_wake`
  → `Event::Timer`, driven by `Document::next_timer`/`run_timers(now)`; (3) `Document::a11y_action(window,
  req)` → `Event::A11y(&ActionRequest)` to the kind, then defaults for Focus/Blur/ScrollIntoView;
  (4) `EventCx::blur`; (5) `Services` (typed map): `Document::set_service::<T>`/`service::<T>`,
  `cx.service::<T>()` in Measure/Prepare/Record/EventCx; (6) `EventCx::time()`.
- SVELTERS-SKELTER: (a) ids are unique across kinds: `RegistryError::CrossKindCollision` names both members;
  (b) `KindSpec.{methods, name_from, doc}`, `MethodSpec`/`ArgSpec` (typed, nullable params and return,
  `MethodId`, `Kind::call`, `Document::call`), `SignalSpec.{args, doc}`, `PropSpec.{nullable, doc}`,
  `NameSource` (default a11y names follow `name_from`); (c) `Document::attach_window(window, node, size,
  scale)` presents a detached JS-created node.

## Landed

All nine deliverables have a working first version. Module map: `crates/skelter-core/src/lib.rs`.

1. **API skeleton:** `Document`, `Kind`, `KindRegistry`, `KindSpec` (props with `ChangeClass`, signals with
   passive/cancelable/sync/bubbles, slots, a11y role, text prop, focusable, container), `KindId`/`PropId`/
   `SignalId`/`SlotId` (stable u32 FNV-1a fingerprints of kind + member + type, collision-checked at
   registration), `Value`, `TreeError`, `PropError`, the contexts (`PropCx`, `StyleCx`, `MeasureCx`,
   `PrepareCx`, `RecordCx`, `HitCx`, `EventCx`, `A11yCx`). Common props and DOM events every node has are the
   pseudo-kind `"*"` (`common.rs`).
2. **Tree:** svelters-egui's slot table (generations, retirement below 2^52) ported; columns in `Document`;
   create/insert (before, slot, move)/remove/destroy with cycle, container, slot and root checks; journal.
   `SCROLL_CENTER` (1<<20) and `INK_OVERFLOW` (1<<21) are apart (test `flag_bits_are_distinct`).
   Virtualisation: `RowHeights` and `RowPool` ported (`virt.rs`).
3. **Change classes and transactions:** `set_prop` returns the spec's class and marks exactly its work;
   one `Revision` per transaction; `js_transaction(seen, f)` rejects JS writes of native-owned props the
   kind changed after `seen` (`PropError::Superseded`).
4. **Style:** svelters-style's `Stylist` on the tree through an `Element` impl (CS5e); computed values mapped
   to `ComputedBox` + `taffy::Style`; each difference marks its class (layout / measure / paint / property);
   restyle invalidation follows the stylist's `Invalidation` maps (attr, state, class, id, structure reach);
   CSS transitions of opacity/transform start `AnimationCmd::Start` with the effect/transform node as target.
5. **Layout:** taffy 0.15 low-level traits on the columns (flex, grid, block, absolute hoisting, rounding),
   boundaries, boundary-only relayout, the measure guard (unchanged max-content size → no layout).
6. **Recording and commit:** transform node per node, clip/effect/scroll(+content) nodes where needed;
   `Kind::prepare`/`record` for content-dirty nodes only; layers for window roots, scroll containers,
   `will-change` and running compositor animations; items in paint order (z-index sorted); the `Commit`
   delta (layers, props, lists, removed, runs, dropped runs, images, animations, a11y, cursor).
7. **Input:** hit testing on the last frame's hit list; capture/target/bubble `JsEvent` paths (passive,
   cancelable, sync); hover enter/leave; press/click (egui semantics: release over the pressed node),
   double click; pointer capture; wheel scrolling of scroll containers as native state; focus model (tab
   order, positive tabindex, `contain`/`dialog` scopes, `:focus-visible` by modality, focus events);
   IME-first key routing, then the keymap hook (`set_key_hook`), then the focused kind, then JS;
   `with_text_input` for the platform's IME bridge.
8. **Accessibility:** AccessKit projection (full tree on first frame and structure changes, dirty nodes
   otherwise) through `a11y::validate`, which repairs or drops with a debug assertion, never panics in release.
9. **Settle:** `Document::busy()` → `Busy` (EVENTS, FRAME, LAYOUT, ANIMATING, COMMIT_IN_FLIGHT);
   `commit_applied` clears COMMIT_IN_FLIGHT.

Additions beyond the brief's signatures: `create_window`/`resize_window`/`close_window`, `set_text_shaper`
(`TextShaper` trait: skelter-base's `TextMeasure` only measures; recording needs glyph runs),
`js_transaction`, `take_events`, `frame_with_stats`, `commit_applied`, `animation_event`, `apply_scroll`,
`set_scroll_offset`, `scroll_into_view`, `hit_test`, `hit_list`, `world_rect`, `add_sheet`/`remove_sheet`,
`set_state`, `focus`/`blur`/`focused`, `next_focus`, `open_dialog`, `testkinds` (Box, Text, Toggle,
`FixedText` measure+shaper, public for other crates' tests).

## Stubbed / not done (honestly absent, no fake successes)

- **Paint and layout transitions** (colours, widths): only opacity and transform start animations; a
  reversed transition restarts from the old computed value (no shortening factor, no `transition*` events).
- **`Kind::virtual_children`** is declared but the document does not drive it yet (realising rows into
  layout is the virtual-list widget's job, wave 2); `RowHeights`/`RowPool` are ready for it.
- **Queries/anchors** (svelters-egui `query.rs`) and the style **interner/compaction** (`compact.rs`) not
  ported: no caller yet.
- **Commit fields not filled:** `ime_area`, `resources.paths`, layer `opaque`, layer `Damage`.
- **Rebuild walk is O(window)** on any structure/layout/property change (layers, items, hit list, and
  then a full a11y tree). Fine for now; incremental items are a later optimisation.
- Mixed `calc(px + %)` lengths take the px term (no calc resolver).
- Stacking contexts are simplified: children sorted by `z-index` per parent only.
- Gestures, file drops and modifier-only events dispatch nothing.

## Not run

Ran locally once, because the skelter workflow was failing before building (fixed since on base):
`cargo test -p skelter-core` (67 passed) and `cargo check -p skelter-core -p skelter-widgets --all-targets`
after merging `origin/ci/presto-base` (ac7cc8c4). Not run: clippy, other OSes, any other crate's tests.

## Questions for the coordinator

1. **Style table.** skelter-core cascades with `svelters_style::tables::EGUI`, which lacks `transform`,
   `transform-origin`, `z-index`, `visibility`, `pointer-events`, `will-change`. The mapping already reads
   them by name if present. Recommend: a `SKELTER` table (`metadata/style/skelter.yaml` + `gen_props.py`)
   owned by the style track; skelter-core switches with a one-line change.
2. **Transform node per node.** `ItemRef` carries no offset, so every node gets a transform node (a layout
   move is then a Property-class op, never a re-record). Recommend keeping it; if the compositor lane wants
   fewer nodes, add an `offset: Vec2` to `ItemRef` in skelter-base (additive) and share the parent's node.
3. **Passive scroll bit** uses the container and its ancestors' listeners only, not listeners inside the
   container. Recommend: a `DESC_NONPASSIVE` bit maintained on listen, when svelters-skelter needs it.
