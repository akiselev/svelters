# AX1: list, scroll-area and menu content reachable by AccessKit and automation

Lane `list-a11y` of cloud batch 10 (`plans/cloud/batch10/LANE-list-a11y.md`), branch `ci/b10-list-a11y`, base
`5d86f1d0`. cadatomic's gap G-shell-7 (batch 9 tours-shell report, `docs/notes/batch9-reports/tours-shell.md` on
cadatomic `claude/b9-tours-shell`): Jobs rows and their Cancel buttons in a `scroll-area`, the Output `ListView`'s
anchor, Problems rows' context menu (`waive`, `unwaive`) were drawn but unreachable by tours.

## Reproduced (item 1)

Fixture: `crates/svelters-automation/tests/list_a11y.rs` (`PANELS`: 12 job rows with Cancel buttons in a
120 pt `scroll-area`, a 1,000-row `ListView` with `<label anchor="line">` rows, two Problems rows with a
`context-menu`), in an embed slot (cadatomic's shape) and as an app (`TestApp`, `svelters-run`).

| Case | What AccessKit and `Session` saw before |
|---|---|
| scroll-area | rows in view: fine. Rows scrolled out: in the scene, `rect` `None` (culled), so every act failed `not visible` after its timeout; nothing scrolled them in. A row cut by the area's edge was clicked at its rect's centre, outside the clip. `rect` of a node scrolled out after being drawn stayed at its old place (`drawn` is never cleared). |
| ListView | **every row's `anchor="line"` gave the same egui id** (the rows are recycled slots, not a keyed `{#each}`, so SV53's scope never applied): one AccessKit node for all rows (the last drawn), the other options named "Row 2", "Row 3"…; `key=` found nothing. Unrealised rows: absent (right). |
| context menu | closed: entries reported **visible** (`is_visible` reads props only). Open: entries had no drawn rect and no hit-list entry, so automation could not act on them; and the menu's egui `Area` spent its first frame on an invisible sizing pass, so right after opening it was not in the AccessKit tree. |

## What landed (item 2)

svelters-egui:
- **Hit lists know what was culled** (`scene/hit.rs`): a culled container (`walk.rs::cull`) and a realised
  `virtual-list` row out of view are recorded per pass (`record_culled`). New `SceneRef::shown(viewport, node)
  -> Option<Shown { rect, visible }>` (the drawn rect and its part inside the clip, from the last pass; `None`
  below a culled container or row, inside a closed overlay, or never drawn), `SceneRef::scrolled_out(viewport,
  node)`, `NodeRef::shown()` and `NodeRef::is_shown()` (automation's visibility: props, stack, root, and drawn
  with a non-empty rect or only scrolled out; a root's top needs no rect).
- `NodeMut::scroll_into_view(center)`: WG6's `scroll-into-view` from Rust.
- `virtual-list`: a `scroll-into-view` on a realised row (or inside it) scrolls the list to the row
  (`reveal`), and the row nodes out of view are recorded as culled. The list node `clips_children`.
- `scroll-area`: AccessKit `clips_children`.
- Own menus (`widgets/menu.rs`): each row of an open level gets its drawn rect and a hit-list entry; the
  context menu's own node gets the surface rect; closing a level clears them.
- `popup::area` seeds egui's `AreaState` with the rect svelters placed, so a new surface (menus, selects,
  popups) is drawn and in the AccessKit tree in the frame it opens (no invisible sizing pass).

`@svelters/native-runtime`:
- `rowScope(outlet, key)` (`index.js`), called by `ListController.bind` after the row mounts: the row outlet's
  `anchor-scope` is its item's key (model mode: the native id), with `anchorScope`'s rules (only once the app
  wrote an anchor, so no extra op on other backends or anchor-free apps). Anchors in `ListView`/`TreeView` rows
  get one egui id per item (SV53), the id follows the item when the slot is recycled, and `key=<item key>` finds
  a realised row's content.

svelters-automation:
- Visibility (`wait_for` `Visible`/`Hidden`, `expect(...).to_be_visible/hidden`, the `visible` state) is
  `NodeRef::is_shown`: entries of a closed menu and content of a closed overlay are hidden; a node scrolled out
  of view is still visible (Playwright's sense).
- `NodeInfo.rect` is the rect drawn **last pass** (`None` when not drawn then); new `NodeInfo.visible_rect`
  (JSON `visible_rect`), the part in view.
- Actionability scrolls a target out of view (wholly, or partly clipped) into view first, then aims at the
  centre of its part in view (`At::Center` too).
- New acts: `Session::context_click(target, opts)` (JSON `input.click {button: "secondary"}`),
  `Session::scroll_into_view(target, center, opts)` and `Session::scroll_to_row(list, pos, opts)` (JSON
  `input.scroll_into_view {selector|ref, align?: "center", row?}`, listed in `session.hello`).

## Public API (additive)

- `svelters_egui::scene::Shown` (`#[non_exhaustive]`, `rect`, `visible`), `SceneRef::shown`,
  `SceneRef::scrolled_out`, `NodeRef::shown`, `NodeRef::is_shown`, `NodeMut::scroll_into_view`.
- `svelters_automation::NodeInfo::visible_rect` (new field), `Session::context_click`,
  `Session::scroll_into_view`, `Session::scroll_to_row`; JSON `input.click.button`, `input.scroll_into_view`.
- `@svelters/native-runtime`: `rowScope(node, key)`.

Behaviour changes: `NodeInfo.rect` no longer reports a stale rect for a node that was not drawn last pass; a
closed menu's entries and a closed overlay's content are no longer `visible`; anchored nodes in list rows get new
egui ids once (as SV53's upgrade did for keyed eaches).

## Tests (item 3)

`crates/svelters-automation/tests/list_a11y.rs`: `scroll_area_content_is_reachable_and_scrolled_into_view`,
`list_rows_have_keyed_anchors_and_scroll_to_any_row`, `context_menu_items_show_while_open_and_are_actionable`
(in process, kittest `EmbedHarness`), `the_three_cases_in_and_out_of_process` (the same JSON body via
`Session::call` on a `TestApp` and via AU2's socket to `svelters-run`). Run locally: these 4, passed.

## Findings and questions

- **AccessKit `Click` (and keyboard activation) does not raise a DOM `onclick`** (EV1 dispatches `click` from
  pointer releases only); `onclicked` works. So `Tier::Semantic` clicks on a button with `onclick` do nothing
  (in an embed and in a `TestApp`). Not this lane's (EVENTS). Recommendation: an activation of a node through
  its `Pressable` (Space/Enter, AccessKit `Click`) fires a DOM `click` on it, as browsers do. cadatomic panels
  using `onclick` are reachable with the default synthetic tier. **Decided (coordinator, 2026-10-25): yes**, as an
  EV1 follow-up outside AX1 (queued in `STATUS.md`, cloud batch 10).
- Unrealised list rows are reached by position (`scroll_to_row`); a `key=` for an unrealised row would need the
  list to know app keys (it holds serial ids). Not built.

## CI

(filled in below as runs complete)

## Handoff

(at the end)
