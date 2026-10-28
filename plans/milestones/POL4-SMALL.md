# POL4-SMALL: popup `anchor-to`, a disabled progress bar, table rows by density (batch polish-b4, lane small)

**Branch:** `ci/pb4-small` (base `8bcc50d`: master `9f0272b` plus the batch's brief commit).
**Brief:** `plans/cloud/polish-b4/BRIEF.md`, `LANE-small.md`.
**Status (2026-10-28 16:30 UTC):** done. Items 1–3, the coordinator's additions (WCAG 24 px default-density rows,
S-L1, the rtl context menu) and dir-widgets' R1 are built and tested; `origin/master` merged twice (dir-core
`e4bf38e`, then dir-widgets `cdacc80`; dir-widgets' rtl line in `controls/progress.rs` kept). Last code commit and its
CI: see "CI". Open questions: none (`menu-bar` titles stay 22 under 2.5.8's spacing exception unless the coordinator
wants 24).

## Design note: `anchor-to` and closing a top-layer entry from its kind (core, small)

POL3-OVERLAYS Q1 recommended a surface prop naming another element's `anchor`. Two things were missing in core:

1. **Finding the element.** `TopLayerCx::find_anchor(name)` (the node of the same window whose common `anchor` is
   `name`, `Document::find_anchor`). skelter-widgets' shim `TopLayerCx` gets a `find_anchor` callback, filled by the
   host.
2. **Closing when it goes away.** A kind's `top_layer` is `&self` and runs inside the frame's rebuild walk, so it
   cannot write `open` back. `TopLayer::close: Option<DismissReason>` (builder `closing(reason)`): the frame records
   the asking node (`WindowState::top_close`) and, after the rebuild walk, `close_asked_tops` closes those entries
   through the stack and tells the kinds (`overlays_closed`, as light dismiss and Escape do; their signals are queued).
   The kind writes `open` back in `overlay_closed`; the next frame takes it out of the top layer.

Removing a node and scrolling both set the window's `rebuild` flag, so `top_layer` is asked again on each: the popup
follows a scrolled anchor and notices its removal with no other hook. Additive only (one field on a
`#[non_exhaustive]` struct, one method, one window field, a call after the rebuild walk). skelter-core is dir-core's
this batch: these lines are in `cx.rs` (`TopLayerCx` impl), `overlay.rs` (`TopLayer`), `window.rs`, `frame.rs`
(`enter_top`, after `rebuild_walk`), `toplayer.rs` (`close_asked_tops`), away from layout and cascade code.

## What changed

1. **Popup `anchor-to`** (`views/popup.rs`): a string prop naming the element it positions against; it wins over the
   `anchor` record and the owner. `placement` (sides and alignment), `flip` and `shift` work as before
   (`views/placement.rs` unchanged: it already flipped and shifted). The named element's box counts as inside for
   light dismiss (a press on the button toggles, not dismisses). When no element of the window has the name, the
   popup closes with `close(reason: "anchor-hidden")` and `open` written back. Surface row in
   `svelters-skelter/src/surface/tier1.rs` (and `placement`'s doc), `skelter.json` regenerated, `docs/skelter/surface.md`.
   Story `popup/anchor-to` (a toolbar at the window's bottom edge; the popup declared after it).
2. **Disabled progress bar** (`controls/progress.rs`, `base.css`; POL3-FINISH Q2 / O-6): `progress-bar` left R9's
   whole-leaf `opacity: .4` list. Its UA: disabled, the `track` and `fill` parts take `opacity: .4` (the kind fades
   what each paints; the busy segment is the fill), the label is `disabled-fg` (host and fill part; `base.css`
   `progress-bar:disabled::part(fill)` beats the fill's `accent-fg` label rule), `cursor: not-allowed`. The style view
   also takes `:disabled` from a disabled ancestor.
3. **Table rows follow density** (cadatomic FQ-146; `table/kinds.rs`): an unset `row-height` follows the theme's
   `row-h` grown with the table's `font-size` as SV83 grows a virtual list's rows (`Theme::row_height_for`), the
   header the theme's `header-h` grown the same way (`density_rows`); a written `row-height` still wins (the header
   stays `header-h`). Read at each restyle (`layout_style`), used by `measure` and `prepare`. Without a theme service:
   as before (`row-height`, header row + 4). `TableKind::as_any` (for tests and apps).

## Addition: default-density rows at WCAG 2.5.8's 24 px (coordinator, relaying the user's 2026-10-28 trial)

`theme/data/density.json` `default`: `row-h` 22 → 24, `header-h` 22 → 24 (sortable table headers and collapsing
headers are clickable). `compact` (20) stays the per-panel opt-in below the target; `comfortable` unchanged.
`plans/research/DESIGN.md` §2.6 updated (table and paragraph). The table's `row-height` surface doc names the theme
fallback (`skelter.json` regenerated). svelters-egui's own `density.json` copy is left at 22 (egui is maintenance only).

Every kind that reads `row-h`/`header-h` or has its own row constant, with its default-density height:

| Kind | Source | Before | Now |
|---|---|---|---|
| `table` rows (unset `row-height`, FQ-146) | `row-h` (`table/kinds.rs` `density_rows`) | 20 (prop default) | 24 |
| `table` header (sortable) | `header-h` | 24 (row + 4) | 24 |
| `virtual-list` with `auto-row-height` (SV83) | `row-h` (`Theme::row_height_for`) | 22 | 24 |
| `virtual-list` default | `row-height` prop 32 | 32 | 32 |
| `tree-view` rows | `row-height` prop 24 (`TREE_ROW_HEIGHT`) | 24 | 24 |
| `select`/`combo-box` options | `row-h` (`views/select.rs` option UA) | 22 | 24 |
| `collapsing-header` | `header-h` (`controls/collapsing.rs`) | 22 | 24 |
| calendar navigation row (pickers) | `row-h` (`PickerLook::row_h`) | 22 | 24 |
| `menu`/`context-menu` entries | item UA 26 (`views/menu.rs`) | 26 | 26 |
| `tab-view` strip | `tab-h` 35 | 35 | 35 |
| tab overflow menu rows | text line + 2 × 5 (`tabs/mod.rs`) | ≈26 | ≈26 |
| `menu-bar` titles | title UA 22 (`views/menu.rs`, dir-widgets') | 22 | 22, left: a bar of wide targets with nothing above or below meets 2.5.8 by its spacing exception (a 24 px circle on each title meets no other target); raise to 24 if the coordinator wants the letter |

No-theme fallbacks left at 22 (`select.rs` `unwrap_or(22.0)`, `PickerLook::default`, `TableState` 20): the theme
always supplies the tokens; those files are dir-widgets' and the fallbacks never show in an app.

Seen with `--diff` (dark-modern and hc-light; images under the scratchpad's `img/`): `w-table-after.diff.png` (rows
20 → 24, header unchanged), `w-combo-box-after.diff.png` (options 22 → 24), `w-tree-view-after.diff.png` and
`w-context-menu-after.diff.png` (no change: already 24 and 26). Row heights only.

Snapshots: no checked-in golden moved locally; `cargo test -p skelter-widgets --lib` (585) and
`cargo test -p svelters-skelter` (all targets) pass. CI decides for the rest (listed under "CI" if any moved).

## Addition: S-L1, a host's font wins over a bundled one of the same family (coordinator, from cadatomic gal4)

The gallery registered a host's `Extensions::fonts` after its bundled set, so a host's full JetBrains Mono never won
over the ligature-free subset under the same family name (cadatomic's `code-editor/ligatures` story could not show
ligatures). `skelter-gallery/src/fonts.rs` `config_with` now registers the host's files first; skelter-gallery-app
uses the same `store_with`. Test `skelter-gallery` `tests/host.rs`
`a_host_font_wins_over_a_bundled_font_of_the_same_family` (failed before the fix: the bundled subset drew), with a
new fixture `tests/fixtures/fonts/probe-mono.ttf` (the G4 probe renamed "JetBrains Mono"; README updated).
`cargo test -p skelter-gallery --test host` 6 pass. A host font covering a script now also comes first in that
script's fallback (its own fonts, as a host would expect).

## dir-widgets' request R1 (POL4-DIR-WIDGETS.md): logical `-start`/`-end` in rtl

Built after merging dir-core: `Popup::placement` swaps `Align::Start`/`End` on a top or bottom side when the node's
direction is rtl (`left`/`right` sides stay physical); the shim `TopLayerCx` carries `rtl` (host: `NodeView::is_rtl`).
Test `views::popup::tests::start_and_end_are_logical_in_rtl`. `cargo test -p skelter-widgets --lib` 586 pass,
`skelter-core --lib toplayer` 4, `svelters-skelter --test spec --test widgets_all` 6 + 5.

## Addition: a context menu with no point opens at its inline start in rtl (coordinator)

`views/menu.rs` (dir-widgets', taken for this fix): `open = true` and Shift+F10/Menu opened at the menu node's top
left (`Point::ZERO`, fixed before the first prepare knew the direction). Now `Menus::open_default` keeps no point and
the root level is placed at the node's inline-start top corner when the levels are laid out (top right in rtl,
opening toward the left as `bottom-end`). svelters-skelter `backend.rs` (`<context-menu>.popup()` with no point):
the owner's bottom right in rtl (bottom left in ltr, as before). Tests: `views::menu::tests::
a_context_menu_without_a_point_opens_at_its_inline_start`, `svelters-skelter --test tier1`
`context_menu_popup_opens_below_the_owner_or_at_the_point` (an rtl case added). `cargo test -p skelter-widgets --lib`
593 pass, `svelters-skelter --test tier1` 14.

## Seen (before: a gallery built at `8bcc50d`; images in this container, not committed)

Under `/tmp/claude-0/-home-user-svelters/42a42938-7530-5bf5-b46d-e3d509f1fc39/scratchpad/img/`:
- `popup-anchor-to.png`: `bottom-start` and `bottom-end` flip above the Route button at the bottom edge, aligned to
  its start and end; dark and light modern.
- `pb-before.png` → `pb-after.png` (`pb-after.diff.png`): percentage, format, indeterminate, disabled, four themes,
  zoom 2. HC Dark's `62%` over the fill was dark on cyan (unreadable), now `disabled-fg`; the other themes keep about
  the same look, the label no longer faded twice.
- `table-before.png` → `table-after.png` (`table-after.diff.png`): `table/basic` at compact/default/comfortable:
  rows were 20 at every density (header 24); now 20/22/28 (header 22/22/28), dark-modern and hc-light.

No snapshot test was re-blessed (none ran locally; CI runs them).

## Tests (filtered, on the last code commit)

- `cargo test -p skelter-core --lib toplayer` 4 pass (new: `a_top_layer_node_that_asks_to_close_is_closed_after_the_frame`).
- `cargo test -p skelter-widgets --lib popup` 14 pass (new: `anchor_to_names_the_element_it_positions_against`,
  `anchor_to_follows_a_scrolled_button_flips_at_the_edge_and_closes_with_it`: in a document, flipped above the button
  at the bottom edge, below it after a 100 pt scroll, closed with `open` written back when the button is removed).
- `cargo test -p skelter-widgets --lib progress` 10 pass (new: `disabled_dims_its_parts_not_the_leaf`, plain and busy).
- `cargo test -p skelter-widgets --lib table` 37 pass (new: `rows_follow_the_density_unless_row_height_is_written`:
  compact/default/comfortable, twice the text size, written and reset `row-height`).
- `cargo test -p svelters-skelter --test spec --test widgets_all` (6, 5); `SVELTERS_UPDATE=1 … --test spec`
  regenerated `skelter.json`; `tools/metadata/style/gen_props.py --check` clean.
- Not run locally: the workspace, pixel snapshots, conformance, `skelter-gallery check` (CI runs them).

## Cross-lane edits (for the coordinator)

- `crates/skelter-core/` (dir-core's): the design note's additive lines.
- `crates/skelter-widgets/src/{shim.rs,host.rs}` (dir-widgets'): the `find_anchor` and `rtl` fields and their forwarding.
- `crates/skelter-widgets/src/views/menu.rs` (dir-widgets'): the context menu's no-point open (above).
- `crates/svelters-skelter/src/backend.rs`, `tests/tier1.rs`: `popup()` with no point in rtl.
- `crates/svelters-skelter/src/surface/tier1.rs`, `metadata/semantics/skelter.json` (dir-core's for `dir`): one
  `popup` row; at a merge keep both sides' rows and rerun the generator.
- `base.css`: only the progress-bar lines of the R9 block and one new rule.

## Questions

None open. Noted: `progress-bar`'s label over a faded fill in Light Modern is low contrast (`disabled-fg` is half
transparent there), as VS Code's disabled text is everywhere.

## CI

- `1986caf` (items 1–3, 24 px rows): green in all six (js 40711800613, linux-gtk 40711800598, linux-qt
  40711800674, macos-appkit 40711800681, skelter 40711800616, windows-winui 40711800617).
- `deb5ac6`, `b66f6e3`: cancelled by later pushes.
