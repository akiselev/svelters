---
title: Events, canvases and panes
description: "DOM-style events on the scene (pointermove with local points), Rust event filters, hit tests of panes the host paints, and anchors in keyed lists."
status: pending
verified_at: 3d24bc94
sources:
  - crates/svelters-egui/src/events.rs
  - crates/svelters-egui/src/embed/panes.rs
  - crates/svelters-egui/src/scene/hit.rs
  - crates/svelters-egui/src/ids.rs
  - packages/native-runtime/src/index.js   # PROPAGATING, anchorScope
  - crates/svelters-egui/tests/{events,cv1}.rs
  - plans/milestones/EV1-events.md
  - plans/milestones/CV1-canvas-seams.md
---

# Events, canvases and panes

Pending: built on `ci/b8-canvas-seams` (CV1), not merged yet. EV1's propagation itself is on master.

## DOM-style events

Every widget and window has `pointerdown`, `pointerup`, `pointermove`, `click`, `dblclick`,
`contextmenu`, `wheel`, `keydown`, `keyup`, `focusin` and `focusout`. They propagate along the scene
as in the DOM: Rust filters first, then capture listeners from the root down, the target, and plain
listeners back up. `onpointermove`, `onpointermovecapture` and `on:pointermove|capture` listen.

The handler's event has `x`/`y` (the viewport's points) and `offsetX`/`offsetY`: the point relative
to the top left of the **target's** drawn rect (a box's layout rect, a widget's rect, a provider's
or pane's part rect when it has one). Pointer events also have `button` (-1 for a move) and
`buttons` (held: 1 primary, 2 secondary, 4 middle, 8 back, 16 forward).

`pointermove` (CV1, SV50):

- goes to the node under the pointer, one per frame at the last point; a button change in the same
  frame dispatches the pending move first, so the order is the DOM's;
- is dispatched only when a filter, a behaviour or a listener on the path would see it;
- is not cancelable: egui always sees the move (hover, tooltips).

```svelte
<!-- an overlay root over a canvas: the crosshair follows the pointer in the box's points -->
<box anchor="hud" onpointermove={(e) => (at = [e.offsetX, e.offsetY])}>…</box>
```

In Rust, a filter sees the same event as `SceneEvent` with `kind == EventKind::PointerMove`,
`local` (the offset point) and `buttons`.

## Panes the host paints

A pane drawn by your own Rust code (a board or schematic canvas, a 3D view) is not a scene node, so
by default hit tests, overlays and filters cannot see it. Give it a `PaneHit`:

```rust
// not run
struct Board { /* the canvas' geometry, shared with its painter */ }
impl PaneHit for Board {
    fn hit_test(&self, _cx: &HitCx, local: egui::Pos2) -> Hit {
        match self.pick(local) { Some(id) => Hit::Part(id), None => Hit::Node }   // or Hit::Miss
    }
    fn part_rect(&self, key: u64) -> Option<egui::Rect> { self.bounds(key) }   // pane-local
    fn part_anchor(&self, key: u64) -> Option<String> { self.name(key) }
}

let board = embed.create_pane("board.canvas", Board::new());   // once
// every frame, where the canvas is painted (after the slots under it, before overlays over it):
paint_board(ui, rect);
embed.show_pane(ui, board.id(), rect);
```

Then, as for any node:

- `SceneRef::hit_test` / `event_target` / `OverlayCx::hovered` report a part as
  `Target::Virtual(VNodeId { host: board.node(), key })`, the pane itself as `Target::Node`;
- `SceneRef::target_rect` and `OverlayCx::rect` give a part's screen rect (from `part_rect`), and
  `target_anchor` gives `"board.canvas/<name>"`;
- pointer and wheel events on it go through the event filters
  (`embed.register_filter(FilterScope::Subtree(board.node()), …)`) with `ev.target` the part and
  `ev.local` the point in the part's rect (or the pane's);
- automation finds it as `anchor=board.canvas` and drives it with `input.drag`, `input.pointer_move`
  and `input.wheel` ([automation](automation.md#canvases-drags-moves-and-wheels-at-local-points)).

A pane is a host-owned node at the top of its own root: `embed.set_viewport(board.id().slot(), vp)`
and `set_visible` work as for a slot, and nothing in it runs JS. Change the tester's state with
`board.with_mut(..)`; `destroy_pane` removes it.

## Anchors in keyed lists

An `anchor` gives a node a stable egui id (and its AccessKit author id). Inside a keyed `{#each}` the
runtime scopes it by the item's key: rows with the same anchor get one id each, the id follows the
item across reorders and runs, and nested keyed lists scope by every key on the way. Anchors outside
keyed lists keep their plain id. `NodeRef::item_key()` gives a node's item key; select a row with
`anchor=row` plus `nth=` or by its text. An unkeyed `{#each}` is not scoped: use a key, or make the
anchor unique (`anchor={`row-${id}`}`).
