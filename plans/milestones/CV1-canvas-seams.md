# CV1: canvas interaction seams for cadatomic's ECAD canvases (SV49, SV50, SV51, SV53)

Lane `canvas-seams` of cloud batch 8 (`plans/cloud/batch8/LANE-canvas-seams.md`), branch
`ci/b8-canvas-seams`, base master c3034451. cadatomic's board and schematic canvases are panes it
paints itself (`ViewClass` panes), not scene nodes; this lane gives them hit tests, Svelte overlays
over them `pointermove`, agents a way to drive them, and keyed lists stable anchor ids.

**Status:** done on the branch, waiting for the coordinator's merge (see "CI").

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
  object keys by an identity serial). Only once the app has written an `anchor` (egui's prop): from
  then on at each placement, and at an anchor write for the keyed items above that node. So other
  backends and anchor-free apps see no extra op (the runtime's op-log tests are unchanged). Kinds
  without the prop (menus' entries have it too) throw once and are not asked again.
- An anchored node with a scope on itself or an ancestor (in its root) takes
  `root_base.with(("a", anchor, path))`, the path being every scope and every anchored ancestor's
  anchor from the root down; anchors outside keyed eaches keep `root_base.with(("a", anchor))`.
  An `anchor` or `anchor-scope` change re-derives the subtree.
- `NodeRef::item_key()` (the innermost item key) and `NodeRef::anchor_scope()`; automation's new
  `key=<item key>` selector part yields the item's top nodes (`key=k42 >> anchor=row`).
- **Behaviour change:** anchored nodes inside keyed eaches get new egui ids once (egui memory of
  such rows, a scroll offset or a collapsing state keyed by them, starts fresh after the upgrade).

### SV51: automation acts for canvases (`svelters-automation`, `@svelters/test`)

- `Session::drag(target, Drag, opts)`, `Session::pointer_move(target, At, Modifiers, opts)`,
  `Session::wheel(target, Wheel, opts)`; new types `At { Center, Local(Pos2) }`,
  `DragTo { Local(Pos2), Node(Target) }`, `Drag` (`#[non_exhaustive]`; `Drag::to`, `Drag::to_node`,
  `.from`, `.steps`, `.button`, `.modifiers`), `Wheel` (`#[non_exhaustive]`; `Wheel::lines`,
  `Wheel::points`, `.at`, `.modifiers`).
- JSON (`Session::call`, so the IPC server and `svelters-run`): `input.pointer_move {at, modifiers}`,
  `input.wheel {dx, dy, unit: line|pixel|page, at, modifiers}`,
  `input.drag {from, to | to_selector | to_ref, steps, button, modifiers}`; listed in
  `session.hello`'s capabilities. `e2e::Locator::{pointer_move, wheel, drag}` (JSON params);
  `@svelters/test`'s `Locator.pointerMove`, `.wheel`, `.drag` (+ typings).
- Points are the target's local points; a drag sends the move to `from`, the press, each step's
  move and the release, one frame each (virtual clock: the same frames every run); modifiers are
  held with `ModifiersChanged` around the act and carried on the press, release and wheel events.
- Actionability now also takes a container's layout rect from the hit list (a `box` has no widget
  rect, so acts on a box canvas used to fail "not visible"). A pane is addressed by its key as
  `anchor=<key>` (`create_pane` sets the pane node's `anchor`).
- There is no separate MCP server in svelters: the "MCP/transport surface" is the JSON-RPC methods
  above (cadatomic's MCP server forwards them).

## Tests

`crates/svelters-egui/tests/cv1.rs` (scratch apps loaded from source; needs `pnpm install`):
`sv49_a_host_painted_pane_answers_hit_tests_with_its_own_parts`,
`sv50_pointermove_reaches_svelte_once_per_frame_with_local_coordinates`,
`sv53_repeated_anchors_in_a_keyed_each_get_one_id_per_row` (1,000 rows, a reorder, nested keyed
eaches, a fresh embed). `crates/svelters-automation/tests/canvas.rs`:
`sv51_drag_pointer_move_and_wheel_drive_a_canvas_deterministically` (a Svelte box canvas and a pane,
Rust and JSON paths, `key=`, two runs compared). `packages/test/test/client.test.js`: the JS methods'
params. EV1's `ev1_filters_run_before_svelte_and_can_consume` skips the `pointermove` its catch-all
filter now sees.

Run locally: `-p svelters-egui --test cv1 --test events --test spec`, `-p svelters-automation --test
canvas`, `pnpm --filter @svelters/native-runtime test`, `node --test packages/test/test/client.test.js`.
Not run locally: everything else (CI is the gate).

## Docs

New `docs/egui/rust/events.md` (events with local points, panes, anchors in keyed lists; status
`pending` until merged, linked from START-HERE); `docs/egui/rust/automation.md` (canvas acts,
`key=`).

## Generated files

`metadata/semantics/egui.json`, `packages/native-runtime/types/egui.d.ts` (`SVELTERS_UPDATE=1 cargo
test -p svelters-egui --test spec`), `docs/egui/reference/widgets/*.md`
(`python3 tools/docs/egui/gen_reference.py`).

## CI

- 3d24bc94 (WIP): js failed (the runtime's op-log tests and 17 conformance each-* samples saw the
  extra `anchor-scope` writes; fixed in 69018d13 by sending it only once an app writes an anchor).
- 69018d13: js 40047866033, linux-qt 40047866012, macos-appkit 40047865990, windows-winui
  40047865997 green; linux-gtk 40047865992 failed one test outside this lane,
  `svelters-egui::wg4b dialog_exports_resolve_from_the_backend_and_abort`: the host's wall-clock
  warning "`app:egui.openFile` ... blocked it for 33 ms in one poll" landed in the console the test
  compares (a slow runner; the test passes locally on this branch). This session cannot re-run
  jobs (403), so this record's push is the re-run.
- Final sha: see the coordinator report.

## Handoff

Nothing left in scope. For the coordinator:

- **Flaky test (recommendation: fix in a small lane):** `wg4b::dialog_exports_resolve_from_the_backend_and_abort`
  compares the whole console, so the host's 8 ms async-poll warning fails it on a slow runner. Filter
  lines starting `svelters:` out of `lines` there (or raise the warning threshold under test).
- **Question (recommendation: yes):** cadatomic's request text (SV49–SV53) was unreadable here
  (`add_repo akiselev/cadatomic` was refused). Please have main-cadatomic check the API above against
  SV49 (FW3's `PickProvider` consuming `Target::Virtual` of a pane) before its EU canvases use it.
- `pointermove` uses the node under the pointer even while a button is held (no pointer capture);
  a canvas drag that leaves its node reports to the new node. Capture is a follow-up if cadatomic needs it.
