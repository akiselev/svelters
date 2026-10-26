# QA11Y-fixes: list options, focus ring repaint, sibling dialog order

Lane FIXES of batch qc-a11y (`plans/cloud/qc-a11y/LANE-fixes.md`, background `BRIEF.md`). Branch `ci/qa11y-fixes`
from `master` 43caa4b9. These are follow-ups that cadatomic's shell-a11y lane found by driving the live skelter app
through AT-SPI.

## What landed

| Item | Commit | What |
|---|---|---|
| 2 | `8b71d300` | **Focus ring after a programmatic move.** `Document::set_state` marks `PAINT` when `FOCUSED` or `FOCUS_VISIBLE` flips. Kinds draw the ring from those bits when they record (a hosted button's UA rule), and before this nothing re-recorded the node unless a stylesheet tested `:focus`. In a bare document even Tab left the ring stale. skelter-widgets' `Focusable` now takes `:focus-visible` from the focus event's reason (`Resp::focus_reason`, set by the tracker from `Event::Focus`) instead of inferring it from the pass's input, which a host's `focus()` between dispatches doesn't have. |
| 3 | `0dc1eb39` | **Sibling dialog scopes opening in one frame.** `focus::after_style` used to sort watched nodes by depth, then by `NodeId`, which carries slot and generation bits. It now sorts by tree path: outermost first, then tree order. Each scope that opens takes focus from the one before it, so an inner scope wins over its outer one, and among siblings the last in tree order (the one painted on top) wins. This is documented in `input/focus.rs`'s module docs. |
| 1 | `f427a087` | **List rows as options.** skelter-core adds `VirtualChildren::row_a11y(row, &mut Node)`. It applies to a realised row's node after the row kind's own a11y and before the row's `role`/`aria-*` props, which still win. A virtualising node's realised rows go out again with it (a selection change), and a realise marks the list `A11Y` (positions). skelter-widgets: `virtual-list` rows are `ListBoxOption` (`ListItem` when `selection=none`) with `selected`, `position_in_set`, `size_of_set` and the `splice` label; `tree-view` rows are `TreeItem` with level and expanded. svelters-skelter: the Widget surface gets `role` and `aria-selected` (`metadata/semantics/skelter.json` regenerated). `aria-activedescendant` (F4) already worked on `text-edit`, so the combobox pattern works from Svelte. |

## Tests

- skelter-test `tests/qa11y_fixes.rs` (consumer, plus AT-SPI on Linux):
  - `list_rows_are_options_and_a_combobox_names_the_selected_one`: the rows (plain boxes) are `ListBoxOption` with
    position, set size 5, `selected` and label. `selected=2` re-sends them. A row's own `role`/`aria-selected` win.
    A `role=combobox` field with `aria-activedescendant` on the selected row reports that row as the consumer's
    focus, and AT-SPI shows Selectable/Selected and announces focus on the option.
  - `a_hosts_focus_move_repaints_both_rings`: Tab, then `doc.focus(b, Keyboard)` from the host. The old button
    re-records without the ring and the new one with it, read from the compositor's applied commits. I checked the
    test against the code without the fix: it fails.
  - `sibling_dialogs_opening_in_one_frame_focus_the_last_in_tree_order`: the later sibling has the larger id and
    still wins.
- skelter-core `tests::focus::sibling_dialogs_opening_together_open_in_tree_order` (siblings and nested).
- skelter-widgets `host::tests::a_programmatic_focus_move_repaints_both_rings`.

Run locally (all passed): the tests above; skelter-widgets `--lib interact:: controls:: views::virtual_list
views::tree`; skelter-core `--lib tests::focus tests::virtual_rows tests::a11y`; svelters-skelter `--test spec`
(with `SVELTERS_UPDATE=1`, then without it); `cargo check -p skelter-core -p skelter-widgets -p skelter-test
-p svelters-skelter --all-targets` (clean apart from pre-existing warnings). **Not run locally:** the full suites of
these crates, the rest of the workspace, clippy, conformance (no compiler change). CI is the gate: see below.

## Deviations, notes

- Item 2's root cause was in skelter-core, not only in the event context. Core never re-recorded a node whose focus
  bits changed. The marking now lives in `set_state`, so every focus path is covered: Tab, pointer, `focus()`, dialog
  restore, peers.
- Item 1 overrides the row node's role whatever kind renders it (a `<box>`, a `<button>`). Svelte's `role` on the
  row overrides that override.
- Tree rows keep the flat position (`position_in_set` = row index + 1), as `TreeView::row_a11y` did before.

## Open questions (with a recommendation each)

- **Names of option rows from their content.** AccessKit's consumer names `ListBoxOption` only from `label`, never
  from descendants (only buttons, links and menu items are named from their content). A row without a `splice`
  label or an `accessible-name` is nameless. Recommendation: cadatomic sets `accessible-name` on its rows (below).
  A later skelter change could fold a row's descendant labels into its name in `row_a11y`; I'd do it only if more
  apps need it.
- **Restore when one of two sibling dialogs closes.** Both record the same opener, so closing the top one while the
  other stays open sends focus to the opener rather than to the remaining dialog. This is the pre-existing
  behaviour, not in this lane's scope. Recommendation: leave it until a real UI opens sibling dialogs together.

## cadatomic wave 2 notes (what cadatomic must change)

1. **Palette rows** (`ui/panels/palette/PalettePanel.svelte`): the rows are now `option`s with `selected` from
   `bind:selected` without any change. Add `accessible-name={r.label}` (or the label plus category) on the row
   `<box>`, because AccessKit doesn't name options from their content. Header rows: `role="presentation"` (or
   `aria-selected="false"` if they should stay options).
2. **Combobox:** on the palette's `<text-edit>` set `role="combobox"`,
   `aria-activedescendant={current && !current.header ? \`palette-row:${current.key}\` : ''}`, and
   `aria-controls="palette-list"`. AT then follows the selected row while the field keeps focus.
3. **Title bar focus moves** (`chrome.rs` `key`): no change needed. `doc.focus(n, FocusReason::Keyboard)` now
   repaints both rings.
4. **Probes that read row roles:** rows of any `ListView`/`virtual-list` now report `ListBoxOption` (or `ListItem`
   with `selection="none"`), and `tree-view` rows report `TreeItem`, instead of their content kind's role. A tour or
   probe locating rows by `Button`/`Label` role must use `ListBoxOption`/`TreeItem`, or the author id.
5. **Sibling dialogs:** if two `focus-scope=dialog` panels show in one frame, the later one in tree order gets focus.

- skelter-core `tests::hooks::a_kind_that_scrolls_itself_scrolls_on_the_compositor` (and its fixed-part twin) now
  run a frame after `focus(page)`. A focus move re-records the node now (item 2), and the test checks that the
  *scroll's* frame records no list.

## CI

Run on 08fec53c (master fa19495f merged in): js 40353934393, linux-gtk 40353934386, linux-qt 40353934205,
macos-appkit 40353934351 and windows-winui 40353934280 green; skelter 40353934387 failed on all three OSes, in one
test, `tests::hooks::a_kind_that_scrolls_itself_scrolls_on_the_compositor` (the expectation above). Fixed in the next
commit; its runs are in the coordinator report.
