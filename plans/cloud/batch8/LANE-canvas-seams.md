# Lane canvas-seams: canvas interaction seams for cadatomic's ECAD canvases

Branch `ci/b8-canvas-seams`; record `plans/milestones/CV1-canvas-seams.md`. The board and
schematic canvases in cadatomic are `ViewClass` panes painted by cadatomic, not scene nodes. Their
HUD is still egui painter code, picking can't use FW3's `PickProvider`, and agents can't drive
them end to end. The request text is in cadatomic's `docs/plan/04-kernel-requests.md` (SV49–SV53).

1. **SV49:** a pane that is not a scene node answers SG3 hit tests (`Hit::Part` with the pane's own
   typed part) through a thin provider, so FW3-style picking and overlays treat it like a node.
2. **SV50:** `pointermove` for Svelte roots (an overlay root over a canvas), coalesced to one per
   frame, with local coordinates; propagation as EV1 defines.
3. **SV51:** automation acts `drag` (press, moves, release, with steps), `pointer_move` and `wheel`
   (line and pixel deltas, modifiers) at positions in a node's local coordinates, in
   `svelters-automation` and its MCP/transport surface, deterministic under the virtual clock.
4. **SV53:** repeated `anchor`s inside a keyed `{#each}` share one egui id (`ids.rs:33`,
   `rooted_id`: base plus anchor alone); kittest panics on the clash. Scope the id by the keyed
   each's key (and the parent anchor path); selectors keep addressing rows by anchor plus key.
   Test: a keyed list of 1,000 rows with the same anchor.
5. Docs in `docs/egui` (`rust/automation`, events) for each new surface.
