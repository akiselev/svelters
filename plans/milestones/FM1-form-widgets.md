# FM1: form widget gaps on egui (QW7, QW8, QW4, QW11, QA8)

Track: WIDGETS (egui). Lane FM1, branch `ci/fm1`, from master `1786957f`, merged with master `ed3d0a5f`
(WG5a's table keyboard, keyed selection and cell editors; CS5g; ZU3). Priority P0 (coordinator).
Sources: `plans/research/QT-SURVEY.md` §1 rows QW4, QW7, QW8, QW11, QA8, and the EV3+ follow-ups
(`plans/milestones/EV3-focus-keys.md`, "Platform button order", "QA8"). Builds on EV3+ (`focus.rs`: key
claims, default/cancel buttons), WG2b (`drag-value`), WG4a (`table`), WG2a (`context-menu`). Does not touch
FX3's native text code (`crate::native`): the text-edit work is at the prop/kind level plus a few lines
around the existing `TextEdit` call in `leaf.rs`.

## Status

| ID | What | Status |
|---|---|---|
| QW7 | `drag-value`: `step`, step `buttons`, `commit` mode, `wheel`, Up/Down/PageUp/PageDown steps, `commit` event | done |
| QW8 | `text-edit`: `submit` and `commit` events, `pattern`, `read-only` | done (no input masks, no `fixup`, no clear button) |
| QW4 | `table`: `sortable` header, `sort-column`/`sort-order`, column menu (hide/show, `hidden-columns`), `header-state` save/restore | done (column reorder deferred) |
| QW11 | platform dialog button order: box `button-order`, button `dialog-role` | done |
| QA8 | Menu key and Shift+F10 open the focused node's `context-menu` | done |

## The commit seam (for CX3, `bind:value|commit`)

- **Event:** the kind's signal `commit`, **arg 0 = the bound prop's committed value**, the same shape as
  `changed`. `drag-value`: `commit(value: f64)`. `text-edit`: `commit(text: str)`.
- **Spec:** `metadata/semantics/egui.json` gives each binding whose kind has such a signal a
  `"commit": {"event": "commit", "arg": 0}` entry next to the existing `"event": "changed"`,
  `"value": {"arg": 0}`. CX3 lowers `bind:X|commit` by listening to `bindings.X.commit.event` and reading
  `bindings.X.commit.arg`. A binding without `commit` means the kind has no commit event.
- `spec.rs` derives it: a signal named `commit` whose argument is named like the prop.

## As built

| Piece | Where | What |
|---|---|---|
| Kind tables | `kinds/inputs.rs`, `kinds/builtin.rs`, `kinds/views.rs`, `kinds/containers.rs`, `kinds/mod.rs` | New props and signals below; new `P` keys at the end of the enum (`StepButtons`, `Commit`, `Wheel`, `ButtonOrder`, `DialogRole`, `ReadOnly`, `Pattern`, `Sortable`, `SortColumn`, `SortOrder`, `ColumnMenu`, `HiddenColumns`, `HeaderState`, `Hideable`); `kinds::BUTTON_ORDER`. |
| Scene | `scene/node.rs`, `scene/tree.rs`, `scene/props.rs`, `elements/mod.rs` | Fields on `W::DragValue`, `W::TextEdit`, `W::TableColumn`, `TableState`; `Common::button_order`, `Common::dialog_role` (listed in `is_common`). |
| Helpers | `forms.rs` (new) | `Pattern` (regex-lite, anchored), `text_rejected`, `text_finished` (submit/commit), the button-order tables (`arrange`, `resolve_role`, `platform_layout`). |
| drag-value | `elements/inputs.rs` | Steps, buttons, wheel, commit mode, `commit` event; per-node `DragState` in egui temp data. |
| text-edit | `leaf.rs` (text-edit branch, after-paint) | `read-only` through an immutable `&str` `TextBuffer`; `submit`/`commit` after `changed`; the invalid outline for a failed pattern. |
| Enter to default | `focus.rs` | `State::entered` + `take_enter`: a one-line field whose Enter went to the default button still `submit`s. `focused_node` for QA8. The Menu key is no longer a text field's claim. |
| table | `views/table.rs` | Sort from the header, column menu (`Response::context_menu` on header cells), hidden columns left out of egui_table (cells map shown → data column), `header-state` JSON. |
| Button order | `layout.rs` | taffy's `child_ids`/`get_child_id` return a box's children in `button-order` (the scene keeps tree order). |
| Keyboard context menu | `menus.rs` (`context`, `keyboard_target`) | Menu key / Shift+F10 opens the nearest context menu of the focused node, below it. |
| Events | `leaf.rs` `widget_changed_via`, `elements::emit` | Announce a widget write through a named event (`sort-changed`, …); emit a plain event. |
| Spec | `spec.rs` | Binding event: `changed` when it carries the prop, else a signal that does, else `changed` (a table's new bound props bind through their own events); the `commit` entry. |
| Generated | `metadata/semantics/egui.json`, `packages/native-runtime/types/egui.d.ts` | Regenerated (`SVELTERS_UPDATE=1 … --test spec`); additions only. Style tables unchanged (`gen_props.py --check`, `gen_kinds.py --check` under `tools/dev/headless`). |
| Dependency | `crates/svelters-egui/Cargo.toml` | `regex-lite = "0.1"` (0.1.9; no dependencies, linear time). |
| Tests | `tests/forms.rs` (7), `forms.rs` unit (2) | See Tests. |

### Behaviour

- **`drag-value` (QW7).** `step` (default 1) is what Up/Down (while focused, taken before egui's own arrows,
  which step by `speed`), a step button or a wheel notch adds; PageUp/PageDown step ×10. Results are clamped
  to `min`/`max` and rounded (`decimals` when fixed, else 10 places, so `0.1 + 0.2` stays `0.3`).
  `buttons` draws − and + squares at the right end; they are not tab stops and the node's rect covers them.
  `wheel` (default true) steps while the pointer is over the field **and it has focus** (not on hover
  alone, so scrolling a long property grid does not change values); that scroll is not passed on to an
  enclosing scroll area. Line/page wheels step once per line; touchpads every 40 points.
  `commit` (default false, Qt `keyboardTracking=false`): while dragging or typing the widget shows a draft;
  `value` and `changed` change only at a commit. The `commit` event fires in both modes at a commit point
  (drag released, Enter or focus lost after typing, any step) when the value differs from the value when the
  drag or edit began; in commit mode egui's `update_while_editing(false)` makes typed text apply on Enter or
  blur.
- **`text-edit` (QW8).** `submit(text)` on Enter in a one-line field (also when the Enter went on to a
  `default` button; both fire, as in Qt). `commit(text)` on Enter, or on focus loss after an edit (not on a
  blur with no edit). `pattern` is a regex the whole text must match (HTML semantics: anchored, empty text
  passes; `regex-lite` syntax); a mismatch shows the invalid outline and AccessKit `invalid`, and
  `submit`/`commit` do not fire (Qt only emits `returnPressed`/`editingFinished` for acceptable input). An
  uncompilable pattern is a `set_prop` error. `read-only` keeps the field focusable and selectable (copy
  works) but never edits it: FX1's native keys and W5's primary paste are skipped, and EV3 edit commands
  cannot change the immutable buffer.
- **`table` header (QW4).** `sortable`: a header click sets `sort-column` to that column (child index) with
  `ascending`, or flips the order on the same column, and fires `sort-changed(sort-column, sort-order)`
  (`header-clicked` still fires; JS sorts the rows). While `sort-column` ≥ 0 the arrows come from it, else
  from each column's `sort` (WG4a's). `column-menu` (default true): a right click on the header lists the
  `hideable` columns as check boxes; the last shown column cannot be hidden. Hidden columns are named in
  `hidden-columns` (a column's `name`, else its child index) and fire `columns-changed`. `header-state` is
  JSON `{"widths":{name:pt},"hidden":[name],"sort":{"column":name,"order":…}|null}`: the table writes it
  (`header-state-changed`) after a sort, a menu toggle or a resize (once the pointer is released); setting
  it restores widths, hidden columns and sort (announced as `sort-changed`/`columns-changed`, not echoed as
  `header-state-changed`). All of `sort-column`, `sort-order`, `hidden-columns`, `header-state` are
  bindable.
- **Button order (QW11).** A box with `button-order` (`platform`, `windows`, `macos`, `gnome`, `kde`; default
  `none`) lays its children out in that platform's dialog order by each button's `dialog-role` (`auto`:
  `accept` for `default`, `reject` for `cancel`, else `action`; or `accept`, `reject`, `destructive`,
  `apply`, `reset`, `help`, `action`). Help and reset come first, then children that are not buttons (a
  spacer with `flex-grow` pushes the rest right), then Windows `accept action reject destructive apply`,
  KDE `accept action apply destructive reject`, GNOME `destructive action apply reject accept`, macOS
  `destructive apply action reject accept`. `platform` is Windows/macOS by `target_os`, GNOME on Linux and
  other Unixes. Only taffy sees the order: tree order, paint order and Tab order are unchanged.
- **Keyboard context menu (QA8).** The Menu key or Shift+F10, while focus is on a node that is the
  `context-menu`'s owner or inside it, opens that menu below the focused node; the nearest owner wins (an
  inner context menu shadows an outer one). The keys are consumed. A text field no longer claims the Menu key
  (EV3 claims kept F-keys already).

## Tests

| Criterion | Test |
|---|---|
| Step buttons, PageUp ×10, Down, wheel, clamping; `commit` per step | `fm1_drag_value_steps_by_buttons_keys_and_wheel_and_commits` |
| Commit-only mode holds `value`/`changed` until Enter; live mode commits once at Enter | `fm1_drag_value_commit_mode_holds_the_value_until_enter` |
| `submit`/`commit` on Enter, pattern rejection (AccessKit invalid, no events), bad pattern refused, read-only, commit on blur after an edit only | `fm1_text_edit_submit_commit_pattern_read_only` |
| Enter to the default button still submits the field | `fm1_text_edit_submits_when_enter_goes_to_the_default_button` |
| Windows/macOS order, role changes re-lay out, `none` is tree order | `fm1_box_button_order_follows_the_platform`, unit `fm1_dialog_button_order_per_platform` |
| Shift+F10 and the Menu key open the owner's menu, not from outside it | `fm1_menu_key_and_shift_f10_open_the_context_menu` |
| Sort and flip, column menu hides by name/index, state JSON, restore of widths/hidden/sort without echo | `fm1_table_header_sorts_hides_and_saves_state` |
| Pattern anchoring | unit `fm1_pattern_is_anchored_and_empty_text_passes` |

Run locally (filtered): `--test forms`, `--lib fm1`, `--test spec` (with `SVELTERS_UPDATE=1`), and the
touched areas' existing tests (`tier0`, `views`, `menus`, `focus_keys`, `widgets`, `--lib` focus/text/table/
context filters): 20 of 21 pass; `gallery_views::table_page_feeds_rows_through_range_and_binds_selection`
needs the built gallery fixture (`pnpm build`) and is left to CI.

## Deferred and notes

- **QW4:** moving (reordering) columns and a "resize to contents" menu entry (egui_table's double-click fit
  already works). Sorting stays JS's (the table owns only the indicator state).
- **QW7:** no auto-repeat while a step button is held, no wrap, no special-value text. The step buttons are
  not in the AccessKit tree (the spin button's own Increment/Decrement actions are).
- **Hidden columns and WG5a:** egui_table gets only the shown columns, so WG5a's `cur_col`, `current-column`
  and edits count shown columns; `TableState::col_map` maps them to data columns for cell text, the editor's
  value and the committed write (`data_col`), and Tab skips hidden columns. `current-column` is therefore a
  shown-column index while columns are hidden.
- **QW8:** no input masks, no `fixup`, no intermediate state (Qt's Intermediate is "fails `pattern`" here);
  no clear button.
- **QA8:** the opened menu does not move keyboard focus into its first item (WG2a's arrow navigation applies
  once an item is focused or hovered).
- **QW11:** no automatic `dialog-buttons` kind; the author places the buttons in a box with `button-order`.
- The spec's binding rule changed (a `changed` that lacks the prop gives way to a signal that carries it).
  No existing binding changed (the regenerated egui.json only adds).
