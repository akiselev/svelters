# OW1a3: basic widgets, part 3 (own label by default, selection, parts, focus stops)

Lane: OW1a3 on thinkpadx1 (`ci/ow1a3`, from master ba5f7239). Scope from the coordinator's
assignment and OW1a2 §6. Coordinator split (proposed by the lane, 2026-10-22): items 1, 4, 6, 7, 8,
9, 10 and 11 here; **items 2, 3 and 5 go to OW1a4** (see §5).

## 1. What landed

| # | Item | Where |
|---|---|---|
| 1 | **RT2 in the own label; `label` flipped to own by default.** A label with run children lays out one paragraph with `rich::layout_job_at` (the RT2 job at the label's CSS `font-size`). `rich::refresh` keeps the runs on the own label for measuring and sets its text mirror, so `Node::text`, queries and AccessKit read all runs. The draw collects the runs again for this frame's hover and focus styles. Links go through the shared `rich::link_pass` (hover, click, Tab, Enter, AccessKit `Link`, `:hover`/`:focus` bits) and `rich::activate` (`clicked`, else `href`); the wrapper path uses the same helpers. A run's label is found by its kind row (`is_label`), so a run that restyles while the own label draws (its payload out of the scene) still remeasures it. `"label"` is in `DEFAULT_OWN`. | `rich.rs`, `widgets/label.rs`, `widgets/mod.rs`, `leaf.rs` |
| 4 | **Label selection.** The theme's egui style sets `interaction.selectable_labels = false` (cadatomic: no stray selections, no TextRun nodes per chrome label). The own label is not selectable by default. With `selectable` it is a read-only TextInput: `:read-only`, the I-beam (R5), pointer selection (drag, double-click a word, triple-click a line, Shift-click; egui's `TextCursorState`), keyboard selection once focused (Shift+arrows, Ctrl+A), Ctrl+C (egui's `Copy` event), AccessKit text runs with the selection (only when selectable), and `bind:selection-start` / `bind:selection-end` (chars of the shown text; a negative end is the end; WIDGET-PARITY §2.10 `selectedText`/`setSelection`). | `widgets/label.rs`, `theme/egui_style.rs` |
| 6 | **Pressable `repeat` past egui's 0.8 s cutoff.** A press that began on the host stays down while the primary button is held. egui stops reporting a click-only widget as pressed after `max_click_duration`, and that no longer ends the press. `repeat` steps and `:active` last as long as the hold. The test fails without the fix. | `interact/press.rs` |
| 7 | **Own leaves remeasure on restyle.** `restyle_node` marks an own widget that replaces a leaf (`replaces` set, no `CONTAINER` cap) for measuring when a measured property changes (`font-*`, `padding*`), as it does for built-in leaves. Also, height-for-width (`Metrics::wrapped`) now measures own widgets with their computed style and parts, as their intrinsic size does. Before, a wrapping own label at a CSS font size wrapped at the default font. | `style.rs`, `layout.rs` |
| 8 | **R3's default 100 ms hover ease** in `theme/base.css`: `button`, `toggle-button` and `checkbox` get `transition: background-color 100ms, border-color 100ms, color 100ms` under `@media (prefers-reduced-motion: no-preference)`, and R2's `transition: none` on `:active`. No existing hover test changed timing: they run without a theme (no base sheet) or settle with `run()`. The new test checks both the computed style and the paint on the virtual clock (the hover frame paints the start value, the next step the end; with reduced motion the change is instant). | `theme/base.css` |
| 9 | **`::part()` on the indicator kinds.** checkbox (`mark`; `indicator` already did), switch (`knob`), radio-button (`dot`), progress-bar (`track`, `fill`), frame (`indicator`, `mark`, the caption-row size) and collapsing-header (`header`, `twistie`, while measuring too) paint through `parts::part_of`: the part's computed style (with its transitions, `WidgetCx::part`) over its UA table when a sheet has a rule for the part, otherwise the UA table as before (the UA layer). The `TODO(OW-CSS ::part)` notes are gone. | `widgets/{parts,checkbox,switch,radio,progress,frame,collapsing}.rs` |
| 10 | **Public focus-stop API.** `WidgetCx::focus_stop(ui, &resp, FocusStop::new(role).tab_stop(bool))`: `FocusStop` is `#[non_exhaustive]` with `role: accesskit::Role` and `tab_stop`. It sets the node's AccessKit role and records the response in EV3's tab order and scopes. The role picks the keys the stop claims while focused (R11): buttons and toggles take Space and Enter, sliders the arrows, rows the arrows plus Space and Enter, text inputs the editing keys. `focus::record_with` stays `pub(crate)` and now keeps one entry per id per pass, so a widget's own `focus_stop` on its response wins over the walk's record. `with_peer` stays crate-internal. | `provider/mod.rs`, `focus.rs` |
| 11 | **ICR1 re-point: verified.** The own icon (`widgets/image.rs`) paints registry icons through `paint::paint_icon_sized` (`paint_icon` with the `size` side). Font glyphs (`family`/`name`/`glyph`) keep the fonts path, by design. | no change |

Generated files, regenerated in the required order (spec, then `gen_props.py`, then `gen_reference.py`):
`metadata/semantics/egui.json`, `packages/native-runtime/types/egui.d.ts`, and
`docs/egui/reference/widgets/{label,button,checkbox,toggle-button}.md`.

## 2. Acceptance

### 2.1 DESIGN R-rules

- **R2:** the press is instant (`transition: none` on `:active`, base sheet).
- **R3:** hover changes in the same restyle, with a 100 ms ease on standalone controls when motion is allowed (test `ow1a3_the_base_sheet_eases_hover_over_100ms_unless_motion_is_reduced`).
- **R5:** a selectable read-only label has no field chrome and shows the I-beam. A plain label shows the default cursor.
- **R8:** a focused link draws its ring round its glyphs (as RT2 did), and links reach Tab.
- **R10:** the pointer cursor on links, the I-beam on selectable labels.
- **R11 (keys):** the focus-stop roles claim their keys from the host keymap.
- **R17:** unchanged. The checked fill and mark of checkbox and radio are now restylable per part.
- **R18:** link colour, and an underline on hover or focus, in the own label.

### 2.2 WIDGET-PARITY rows

- §2.10 QLabel, Configuration:
  - `textFormat` RichText: Q4, inline children, now on the own label;
  - `textInteractionFlags`: `selectable` (mouse and keyboard, read-only TextInput);
  - `selectedText`, `setSelection`, `hasSelectedText`: `bind:selection-start` and `bind:selection-end`;
  - `linkActivated`: the run's `clicked`.
- §2.10 QLabel, Styling: `label a:hover` and `label span.cls` (Q4) on the own label.
- `label::selection` is **not** done: the selection paints with the theme's `Visuals::selection`; `::selection` part colours are a follow-up.
- §2.2, §2.3, §2.9, §2.16 Styling:
  - `QCheckBox::indicator` → `::part(indicator)`, and `::part(mark)`;
  - `QRadioButton::indicator` → `::part(indicator)`, and `::part(dot)`;
  - `QProgressBar::chunk` → `::part(fill)`, and `::part(track)`;
  - `QGroupBox::indicator` → `frame::part(indicator)`, and `::part(mark)`;
  - `QToolBox::tab` → `collapsing-header::part(header)`;
  - the twistie → `::part(twistie)`.

  All of these are now reached by author rules.
- QT-SURVEY: QAbstractButton `autoRepeat` (OW1b's spin buttons and scroll arrows build on it): it no longer stops at 0.8 s.

### 2.3 VIS1

- The label's text colour is now the theme's `fg` (the own label's `color`), not egui's `Visuals::text_color()`. The TK1 snapshots `counter-three` and `transition-back` change in label text colour only (regenerated from CI's lavapipe artifact, see §4).
- A uniform part border is still one `RectShape` (`StrokeKind::Inside`), so the box measurements are unchanged.
- The VIS1 gate was not run (not on master).

### 2.4 Pins

No native-behaviour pin changed. Pin 10's pointer thresholds are untouched; the hold past egui's click cutoff is a press, not a drag.

## 3. Tests

`crates/svelters-egui/tests/ow1a3.rs` (new):

- `ow1a3_label_is_own_by_default`
- `ow1a3_labels_are_not_selectable_by_default`: the theme style; a drag selects nothing; no `:read-only`.
- `ow1a3_a_selectable_label_selects_copies_and_binds_its_selection`: drag-select, the bind announcement, Ctrl+C copies, and a JS-set selection copies.
- `ow1a3_a_rich_own_label_runs_restyle_and_remeasure`
- `ow1a3_own_leaves_remeasure_when_css_changes_their_font_or_padding`: label `font-size`, button `padding`, and back.
- `ow1a3_part_rules_paint_the_indicator_kinds`: one case per kind and part (nine).
- `ow1a3_a_provider_kind_registers_focus_stops_with_a_role`: a test provider kind with three part stops: AccessKit `Tab` role, Tab order, click focus.
- `ow1a3_a_held_repeat_button_repeats_past_the_click_cutoff`
- `ow1a3_the_base_sheet_eases_hover_over_100ms_unless_motion_is_reduced`

Changed tests:

- `tests/rt2_rich.rs`: the helper reads the own label's runs. All four RT2 tests pass on the own label.
- `tests/ow1a2.rs`: the default test now expects `label` to be own.

Run locally (after merging origin/master), all passing (94): `--test` ow1a3, ow1a, ow1a2, own_widgets, rt2_rich, widgets, style, scene_memory, spec, ow_css, theme, tier0. `cargo clippy -p svelters-egui --all-targets` gives no warnings in this lane's files.

## 4. Deviations and notes

- **`tab_stop: false`** is honoured in EV3's scoped chains (dialogs, focus scopes). Outside a scope, egui's own Tab traversal visits every focusable response (as for LS1's rows). The doc says so.
- **Selection indices are chars of the shown text** (an elided label selects the elided string).
- **Selection colours** come from `Visuals::selection` (the theme's), not `::selection`.
- **The TK1 snapshots** need lavapipe (CI's `egui-snapshots` job). They are taken from that job's artifact after looking at them, not rendered locally.

## 5. Handoff: OW1a4 (fresh agent, proposed split)

1. **`use:` action exports** of the interact units (Pressable, Hoverable, Focusable, Toggleable, Expandable, Roving and the rest; `UseUnit<U>` is OW0's adapter shape).
2. **collapsing-header:**
   - `accordion` (one open at a time, `WidgetCx::with_peer` as radio does);
   - `slot="end"` header actions shown on `:hover`/`:focus-within`, which need `header` as a node part (`#[part(node)]`, SG4 `insert_shadow`);
   - the header `icon`.
3. **The in-crate egui uses tagged `OW: … until OW1a3`** in `views/table.rs` (cell checkbox and label) and `views/toast.rs`: replace them with own drawing and delete their lint lines. Re-tag them OW1a4 when that lane starts. `menus.rs` is OW3's.
4. `label::selection` colours, and `:visited` (OW-P2).

## 6. Not run locally

The full workspace nextest, the full gates, conformance, the VIS1 gate, the snapshot job
(lavapipe), GTK/Qt. CI runs these.
