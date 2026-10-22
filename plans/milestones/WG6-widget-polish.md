# WG6: widget polish (egui)

QT-SURVEY ranked list row 3 (`plans/research/QT-SURVEY.md` §3). Lane `ci/wg6`, writer lane WIDGETS.
Code: `crates/svelters-egui/src/tips.rs` (new), `kinds/dnd.rs`, `kinds/pickers.rs`, `kinds/popups.rs`,
`kinds/builtin.rs`, `overlays.rs`, `leaf.rs`, `walk.rs`, `ops.rs`, `scene/{node,props,tree}.rs`, small
hooks in `focus.rs`. Tests: `tests/wg6.rs`, unit tests in `tips.rs` and `kinds/dnd.rs`. Gallery page
`fixtures/gallery-egui/src/pages/43-widget-polish.svelte`.

## Status by ID

| ID | Status | What |
|---|---|---|
| QA5 (P0) | done | Tooltips on disabled widgets: every `tooltip` call site goes through `tips::text` (`on_hover_text` + `on_disabled_hover_text`; egui hides `on_hover_text` on disabled widgets, verified in egui 0.37 `Tooltip::for_enabled`). Menu entries too. |
| QA5 rich | done (as an element) | `<tooltip>` kind (an attached overlay, like `popup`): its children are the owner's tooltip, enabled or disabled (`Tooltip::should_show_tooltip` + `for_widget`). The survey's `{#snippet tooltip()}` cannot be a prop of a native element in Svelte 5, so a snippet is rendered into it: `<tooltip>{@render tip()}</tooltip>`. One tooltip per layer: a `tooltip` prop on the same element wins. |
| QA5 status tip | done | `status-tip` prop (Widget, appended). While the pointer is over a node (enabled or not, innermost wins) its text is published; `svelters_egui::status_tip(&ctx)` reads it (kept one pass; a 150 ms repaint clears it after the pointer leaves). cadatomic's status bar (QS4) reads it. |
| QA5 timing | not done | Wake-up/fall-asleep delays stay egui's (`tooltip_delay` 0.35 from the theme, `tooltip_grace_time`); no `tooltip-delay` token. `whatsThis` not done. |
| QW13 | done | `elide` on `label` (`none`, `start`, `middle`, `end`): one line cut with `…` to the width (binary search over measured cuts, `tips::elide_with`); the full text is the accessible name and, without a `tooltip`, the hover text. With `elide` set the label does not wrap (layout's height-for-width skips it). |
| QA13 | done | `scroll-into-view(align?)` method on every node (`nearest` or `center`, `Ui::scroll_to_rect`): flags the node (`NodeFlags::WANT_SCROLL`) and its ancestors (`SCROLL_BELOW`, so the walk does not cull the way there). A leaf that gains focus scrolls itself into view (`Response::scroll_to_me`). A pass with Tab pressed culls nothing, so egui's focus can move to a widget outside the visible part of a scroll area (verified: egui does not scroll on focus by itself, and culled widgets are not in its focus order). Not in `virtual-list` (it has `scrollTo`) or `table` (`scroll-to-row`). |
| QW14 | deferred | SVG: AS1 (`ci/as1`, not merged when this lane started) adds the resvg loader and `icon source`/`tint`. Building a second loader is out; the remainder (disabled/selected icon tint from the cascade, state-aware icons) waits for AS1 on master. |
| QA14 | done (egui side) | `cursor` prop (Widget, appended; CSS names, `tips::cursor_icon`): set while the pointer is over the node, inherited by children at `auto` unless their widget sets its own (a text field's I-beam). **CSS `cursor` in the style engine is DS2's**, not done here. |
| QW10 | done | `key-sequence-edit` (one chord, Qt's `QKeySequenceEdit`): a `Picker` variant with `bind:value` (`Ctrl+Shift+K`: `Ctrl`, `Alt`, `Shift`, `Cmd`, then `Key::name`) and `changed`. Click/Space/Enter records; Escape or Tab cancels; Backspace/Delete clears. While recording it claims every key but a plain Tab (`focus::Role::Capture`), so `shortcut_override` answers true and host shortcuts do not fire. Multi-chord sequences not done. |
| QW9 | done (combo) | `combo-box` type-ahead while focused: typed text selects the next option starting with it (`tips::combo_type_ahead` on WG5a's `views::selection::KeySelect::type_ahead`, the lists' and tables' rule: 1 s reset, a repeated letter cycles). The combo claims printable keys while focused (`focus::Role::Combo`). Completer (`QCompleter`) stays cadatomic's. |
| QA9 | done (in part) | Drop actions: `drop` gets a 5th arg `action` and `drag-end` a 2nd (`move`; `copy` with Ctrl, Option on macOS; `link` with Ctrl+Shift, Cmd+Option on macOS; `none` when not dropped), with Copy/Alias cursors; `DragPayload::accept_as`/`action` for Rust targets. File drops from the OS on any node whose `drop-types` accepts `text/uri-list`: `drag-enter`, `drag-leave`, `drop("text/uri-list", uris, x, y, "copy")`, `file://` URIs (DX2's `file_uri`) joined by `\r\n`. Auto-scroll: a `scroll-area` scrolls while a drag is within 24 pt of its edge. Not done: tree auto-expand on hover (WG5b's tree), drag-out to the OS (winit has no drag source; P2). **Verify** on real platforms: whether winit reports the pointer position while the OS drags files over the window (the target hit test needs it). |

## Hooks added to shared code

- `focus.rs` (EV3+, not rewritten): two `Role` variants (`Capture`, `Combo`), their `claims` arms, and two
  arms in `record`.
- `walk.rs` `paint_own`: `tips::reveal` before culling, `tips::keep` in the two cull checks, `tips::hover` for
  containers and leaves, `scroll_to_me` on `gained_focus`.
- `NodeFlags` bits 17-19 (`WANT_SCROLL`, `SCROLL_BELOW`, `SCROLL_CENTER`).
- Appended to the tables (ids of earlier kinds and members unchanged): Widget props `status-tip`, `cursor`;
  label prop `elide`; Widget method `scroll-into-view`; kinds `tooltip`, `key-sequence-edit`. Signals `drop`
  and `drag-end` got a trailing arg.

## Tests

- `tests/wg6.rs`: disabled tooltip and `<tooltip>` (with `everything_is_visible`), status tip and cursor
  following the pointer, middle elision, `scroll-into-view` below the fold, key-sequence recording and
  clearing, combo type-ahead, a file drop from the OS as a uri-list.
- Unit: `tips::tests` (elide modes, cursor names), `kinds::dnd::tests::wg6_drop_actions_and_uri_list`.

## Run locally / not run

- Run: `cargo check -p svelters-egui --all-targets --all-features` and clippy (no new warnings in WG6
  files); nextest `-p svelters-egui`: `--test wg6` (7/7), `--test focus_keys`, `--test menus`, the `wg4b`
  drop tests, `--test spec` (`SVELTERS_UPDATE=1`), `gallery_export_stubs_are_current`, the lib's `wg6_*`
  and `dnd` units; `gen_props.py` (regenerated), `gen_kinds.py --check`.
- Not run: `pnpm build` of `fixtures/gallery-egui` and `gallery_pages_drive_pickers_dnd_and_dialogs` (needs
  the compiler build; the new page 43 is compiled first in CI), the rest of the svelters-egui suite, full
  workspace nextest, conformance, other backends. CI runs them.
