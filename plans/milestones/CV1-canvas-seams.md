# CV1: canvas interaction seams for cadatomic's ECAD canvases (SV49, SV50, SV51, SV53)

Lane `canvas-seams` of cloud batch 8 (`plans/cloud/batch8/LANE-canvas-seams.md`), branch
`ci/b8-canvas-seams`, base master c3034451. cadatomic's board and schematic canvases are panes it
paints itself (`ViewClass` panes), not scene nodes; this lane gives them hit tests, Svelte overlays
over them `pointermove`, agents a way to drive them, and keyed lists stable anchor ids.

**Status:** in progress (WIP pushed; see "Handoff" for what is left).

cadatomic's request text (`docs/plan/04-kernel-requests.md`) could not be read: attaching
`akiselev/cadatomic` was refused in this session. The lane brief's wording was the spec.

## What landed

### SV49: hit tests from a pane the host paints (`embed/panes.rs`, `scene/hit.rs`)

- `scene::PaneHit` (trait): `hit_test(&HitCx, local) -> Hit`, `part_rect(key)`, `part_anchor(key)`.
- `Embed::create_pane(key, P) -> PaneHandle<P>`, `Embed::show_pane(&Ui, PaneId, Rect)`,
  `Embed::destroy_pane(PaneId)`; `PaneId::{slot, node}`, `PaneHandle::{id, node, with, with_mut}`.
- A pane is a host-owned box at the top of its own root (a slot without a mount); `show_pane`
  records it in the viewport's hit list in the host's draw order. `SceneRef::hit_test`,
  `event_target`, `target_rect`, `target_anchor` (`"<pane key>/<part name>"`) report its parts as
  `Target::Virtual(VNodeId { host: pane node, key })`, so FW3-style picking, overlays
  (`OverlayCx::rect`, `hovered`) and event filters (`FilterScope::Subtree(pane.node())`) see it as a
  node. Pointer events on a pane part go through the Rust filters (no JS listeners on it).

### SV50: `pointermove` (`events.rs`, `kinds/mod.rs`, runtime `index.js`)

- New DOM signal `pointermove` on every widget and window (appended to `dom_signals!`), dispatched
  along EV1's path (capture, target, bubble) to the node under the pointer; one per pass at the last
  point (moves between button changes are coalesced; a button change flushes the pending move
  first, so DOM order holds). Not cancelable: egui always sees the move. Dispatched only when a
  filter, behaviour or listener on the path would see it.
- `EventKind::PointerMove`; `SceneEvent::local` (the point relative to the target's drawn rect: a
  box's layout rect, a widget's rect, a provider or pane part's rect where it has one) and
  `SceneEvent::buttons` (DOM `buttons` bits). Both `#[non_exhaustive]` types, additive.
- Pointer and wheel args appended `ox`, `oy` (and `buttons` for pointer events); the runtime sets
  `offsetX`, `offsetY`, `buttons` (and `button` -1 for a move) on the event object; `egui.d.ts`'s
  `SvlUiEvent` has them.

### SV53: repeated anchors in a keyed `{#each}` (`ids.rs`, `scene/tree.rs`, runtime `index.js`)

- The runtime sets the new Widget prop `anchor-scope` on each keyed each item's top natives: the
  item's key after the keys of the keyed items around it up to its native parent (U+001F between;
  object keys by an identity serial). Kinds or backends without the prop throw once and are not
  asked again (only egui has it).
- An anchored node with a scope on itself or an ancestor (in its root) takes
  `root_base.with(("a", anchor, path))`, the path being every scope and every anchored ancestor's
  anchor from the root down; anchors outside keyed eaches keep `root_base.with(("a", anchor))`.
  An `anchor` or `anchor-scope` change re-derives the subtree.
- `NodeRef::item_key()`: the innermost item key (selectors: anchor plus key).
- **Behaviour change:** anchored nodes inside keyed eaches get new egui ids once (egui memory of
  such rows, a scroll offset or a collapsing state keyed by them, starts fresh after the upgrade).

## Tests

`crates/svelters-egui/tests/cv1.rs` (scratch apps loaded from source; needs `pnpm install`):
`sv49_a_host_painted_pane_answers_hit_tests_with_its_own_parts`,
`sv50_pointermove_reaches_svelte_once_per_frame_with_local_coordinates`,
`sv53_repeated_anchors_in_a_keyed_each_get_one_id_per_row` (1,000 rows, a reorder, nested keyed
eaches, a fresh embed). Run locally: these three, and `--test spec` (regenerated `egui.json`,
`egui.d.ts`). Not run locally: everything else (CI is the gate).

## Generated files

`metadata/semantics/egui.json`, `packages/native-runtime/types/egui.d.ts` (`SVELTERS_UPDATE=1 cargo
test -p svelters-egui --test spec`), `docs/egui/reference/widgets/*.md`
(`python3 tools/docs/egui/gen_reference.py`).

## CI

(pending)

## Handoff

Left: SV51 automation acts, docs pages, the full CI round.
