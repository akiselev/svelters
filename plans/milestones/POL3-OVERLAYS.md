# POL3-OVERLAYS: a top layer, pickers' open states, tab overflow (batch polish-b3, lane overlays)

**Branch:** `ci/pb3-overlays` (base `master` `242d688`).
**Brief:** `plans/cloud/polish-b3/BRIEF.md`, `LANE-overlays.md`.
**Status:** in progress.

## Design note: the top layer (POL2-REGISTER Q2)

What exists: each window keeps an overlay stack (`skelter-core/src/overlay.rs`, `toplayer.rs`). A kind records an
**overlay part** (a display list, `RecordCx::overlay`): menus, the combo's list and the pickers' drops already draw
there, in a layer above every content layer, hit before the content, light-dismissed and closed by Escape in order.
What is missing is a **node subtree** in that layer: a `popup`'s or `modal`'s Svelte children, a `tooltip` element,
the toast host. They are laid out and drawn in their owner's subtree, so a clipping or scrolling ancestor cuts them, a
later sibling covers them, and a popup takes room in its parent's flow.

The shape (as the web's top layer: the node stays in the tree, only its box and paint move):

1. **Membership.** A node whose taffy style is `position: fixed` is in its window's top layer. Kinds set it in
   `Kind::layout_style` while they are open (CSS `position: fixed` keeps mapping to `absolute`; this is not the CSS
   value). taffy 0.15 already bubbles a fixed box to the root, which claims it: the node is laid out against the
   window, out of every ancestor's flow and overflow. `out_of_flow` and `positional_parent` treat `fixed` as
   out-of-flow with the window root as containing block.
2. **What it is:** `Kind::top_layer(&self, &TopLayerCx) -> TopLayer` (`#[non_exhaustive]`: class, `at`, inside rects,
   opener, takes input, modal, dismiss flags; defaults from `OverlaySpec::new`). `at` is where the border box goes in
   window points (`None`: where layout put it). The kind computes it with `views/placement.rs` from its anchor's box
   (`TopLayerCx::world_rect`, this rebuild's, so the anchor is placed first; the default anchor is the owner) and its
   own size.
3. **Frame.** The rebuild walk defers each top-layer node it meets and walks its subtree after the whole content (and
   after the top-layer nodes found before it): its transform node hangs off the window root at `at`; no ancestor's
   clip, effect or scroll applies; its subtree's items go in its own layer. That layer is an entry of the window's
   overlay stack (its z from the stack's bands and open order, as an overlay part's), so a combo's list opened inside
   a popup goes above the popup, a tooltip above both. A top-layer node inside another one opens from it (its
   parent entry). The stack's layer deltas are sent before the items of the layers it adds.
4. **Input.** The stack's hit routing comes first, as for parts; a press on a top-layer entry is hit-tested inside
   that node's subtree (the hit list's entries for it), so it reaches the button under the pointer. Light dismiss and
   Escape close entries in the stack's order (children first, one level per Escape); the kind hears it in
   `Kind::overlay_closed` and stops being `fixed`.
5. **Ownership.** The node never leaves its owner: removing the owner removes it (its entry closes at the next
   rebuild), focus order and the accessibility tree are the tree's, so AccessKit keeps the owner as its parent (bounds
   are its new window box relative to the owner's).

Not done: a top-layer node does not take a CSS transform of its own (only `at`); `fit` (shrinking a popup to the room)
is left to the display-list kinds that already do it.

## Scope and findings

(filled in as the lane goes)

## Tests and CI

(filled in at the end)

## Questions (recommendations, for the coordinator)

## Requests to the finish lane
