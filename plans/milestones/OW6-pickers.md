# OW6: pickers as own widgets (`calendar`, `color-editor`, `date-picker`, `color-picker`)

Lane `ci/ow6` (thinkpadx1), from `origin/master` `d0542a3a`, with `origin/master` merged at `e832e001` (OW3:
popups, Anchored, Dismissable). Design: `plans/research/ARCH-WIDGETS.md` §6.1 (the `date-picker` and
`color-picker` rows) and the OW6 row of §7.2. Acceptance rows: `plans/research/WIDGET-PARITY.md` §2.19 and
§2.20, its Decided block (the new kinds `calendar` and `color-editor`; DOC1's state-bit gaps), DESIGN.md's R
rules, and the existing picker tests (`wg4b.rs`, `observe.rs`).

**Split (decided by the coordinator, 2026-10-22):** OW6 covers `color-editor`, `color-picker`, `calendar` and
`date-picker`. **OW6b** covers:
- the sectioned date field (`editable`);
- the month and year sub-pickers;
- the rgb/hsv fields as OW1b `drag-value` parts;
- the new `path-field` kind, for which the coordinator is adding a WIDGET-PARITY row.

See "Handoff".

## What landed

### New kinds (always own: `widgets::OWN_ONLY`)

- **`calendar`** (`widgets/calendar.rs`, row `kinds/pickers.rs::CALENDAR`): an inline month calendar
  (`QCalendarWidget`).
  - **Layout:** a header with prev/next and the month and year, a weekday row, ISO week numbers, and a 6×7
    day grid.
  - **Props:** `value` (bind, ISO 8601, refused on a bad write), `min`, `max`, `first-day`
    (`auto|monday…sunday`; `auto` is Monday until IN1), `weekday-format` (`narrow|short|long|none`),
    `calendar-week`, `navigation`, `read-only`, `day-parts` (`["2026-10-22 milestone"]`).
  - **Signals:** `changed`, `commit` (a pick is a commit), `page-changed(year, month)`.
  - **Keys:** the grid is one tab stop. Arrows move ±1 day or ±7 days, Home/End go to the week's start or
    end, PageUp/PageDown move a month (Shift: a year), all clamped to `min`/`max`. Enter or Space picks.
  - **AccessKit:** `Grid` → 6 `Row`s → `GridCell`s. Each cell is named by its ISO date and carries
    `selected`, `disabled` and Click, with a description such as "Thursday 22 October 2026 (today,
    milestone)". Prev/next are `Button`s.
- **`color-editor`** (`widgets/color_editor.rs`, row `COLOR_EDITOR`): the colour dialog's body, inline in a
  panel.
  - **Layout:** a saturation/value square (an exact sRGB-HSV 16×16 mesh), a hue strip, and an alpha strip
    over a checkerboard. Below them, the fields (`fields="hex rgb hsv"`): a hex field plus R/G/B (0–255) and
    H/S/V (0–360, 0–100) channel boxes. Then `swatches` (a palette) and `custom-colors` (bind, up to 16),
    with an add cell.
  - **Model:** HSV is in sRGB space and kept while editing, so a grey keeps its hue.
  - **Props:** `value` (bind, CSS hex, normalized, refused on a bad write) and `alpha`
    (`opaque|blend|blend-or-additive`, refused on a bad write).
  - **Signals:** `changed` per edit (an EV1 filter can prevent it) and `commit` at commit points: drag
    release, a key step, a field set, a swatch.
  - **Keys and AccessKit:** square, hue, alpha and each channel box are tab stops (EV3,
    `record_part_focus`) and AccessKit `Slider`/`SpinButton` nodes with SetValue, Increment and Decrement.
    Arrows step them (Shift ×10); channel boxes also scrub by dragging.
  - **Hex field:** an AccessKit `TextInput` (SetValue). It takes typing through one seam (see below): Enter
    or leaving the field applies the text, Escape drops it, and an invalid hex shows `:invalid`.

### Own replacements (egui wrappers deleted: `W::Picker` and its arms in `leaf.rs`, `layout.rs`, `scene/*`, `embed`)

- **`date-picker`** (`widgets/date_picker.rs`): a drop button showing `format` of the value (today's date
  when the value is empty), with R14's chevron.
  - It opens the calendar body in a popup built from OW3's pieces (`popup::SURFACE`, `anchor::place`
    bottom-start with flip, `popup::dismissed`, and `Grab`, which returns focus to the button). The shared
    code is `widgets/picker_popup.rs::Drop`.
  - **Commit mode:** `confirm` is the default and keeps the egui wrapper's Save/Cancel: a pick stays pending,
    Enter or Save applies it, Cancel or Escape drops it. With `immediate` (Qt), a pick commits and closes.
  - **Keys:** Enter, Space, F4 or Alt+Down open the popup. While open, the calendar takes the arrows,
    Home/End, PageUp/PageDown and Enter.
  - **Props:** every calendar prop above, plus `commit-mode`. **Signals:** `changed`, `commit`,
    `page-changed`.
- **`color-picker`** (`widgets/color_picker.rs`): a swatch drop button (`::part(swatch)`, with a
  checkerboard under alpha) and a chevron, opening the editor body in the same popup.
  - **Commit mode:** `live` is the default and keeps the wrapper's behaviour: each edit changes `value`, and
    closing the popup commits. With `confirm`, edits stay pending until Save.
  - **Props:** `fields`, `swatches`, `custom-colors`, `commit-mode`. **Signal:** `commit`.
  - **AccessKit:** a `ColorWell` with the hex as its value, plus `expanded` and `has_popup`.

### Shared pieces

- `widgets/picker_popup.rs`:
  - the closed button's UA, which matches OW3's select (R4 field look, the `focus` border while `:open`,
    R8's −1 field ring);
  - `POPUP_UA` (OW3's surface, padding 8);
  - the Save/Cancel footer (accent primary, then secondary).
- `widgets/picker_glyphs.rs`: chevrons (`content: icon(…)` first, then a path, as OW3's `glyph.rs`) and
  `gap`.
- Typed props: `calendar::IsoDate`, `color_editor::HexColor`, `color_editor::AlphaMode`. Each refuses a bad
  write with an `invalid-value` error.
- **Additive hook:** `WidgetCx::record_part_text_focus` in `provider/mod.rs`, beside OW2's
  `record_part_focus`. It puts a part's text entry in EV3's tab order with a text field's keys.

### The text-input seam (OW1b/OW4 not on master)

`color_editor::text_entry` is the only place a picker types. It wraps egui's `TextEdit` (the text input
master has) under `#[allow(clippy::disallowed_types)] // OW: wrapper until OW4 (OW1b's entry)`. The caller
paints the field's box; the seam paints only the text and caret. When OW1b's `Entry` lands, and later OW4's
`TextInput`, they replace this function body, and nothing else changes.

### Generated files

`metadata/semantics/egui.json`, `packages/native-runtime/types/egui.d.ts` (spec, `SVELTERS_UPDATE=1`), then
`tables.generated.rs`/`props.generated.js` (gen_props.py: the new tags), then the reference pages
`calendar.md`, `color-editor.md`, `date-picker.md`, `color-picker.md` and `index.md` (gen_reference.py).

## Acceptance

### WIDGET-PARITY §2.19 (date) and §2.20 (colour)

| Row | Status |
|---|---|
| `min`, `max` (days outside `:disabled`) | done (calendar, date-picker; PageDown/arrows clamp) |
| `calendar` kind (inline) | done |
| `editable` sectioned field (Steppable per section) | **OW6b** |
| `first-day` (IN1 locale for `auto`) | done (`auto` = Monday until IN1) |
| `weekday-format` none/narrow/short/long | done (English names until IN1) |
| `navigation={false}` | done |
| `day-parts` (P10 names) | done as AccessKit description and data. **CSS `::part(day milestone)` is not wired** (see Handoff). |
| weekend part name | same as `day-parts` |
| `read-only` (NoSelection) | done (roves, never picks; `:read-only`) |
| `commit-mode` immediate/confirm | done |
| `input`/`commit`, `page-changed(year, month)` | `changed` per pick, `commit` and `page-changed` done |
| `::part(button)`, `::part(chevron)` | the host is the button; `chevron` done (R14) |
| `::part(header)`, `prev`, `next` | done |
| `::part(month)`, `::part(year)` (open month or year picker) | painted parts done; **the sub-pickers are OW6b** |
| `::part(grid)`, `::part(day)` with `:selected :hover :current :disabled` + `today weekend other-month` | painted with those states and names from the UA tables (see Handoff on per-day CSS) |
| `::part(weekday)`, `::part(week-number)` | done |
| colour `commit` | done |
| `commit-mode` live/confirm | done |
| `color-editor` kind | done |
| `custom-colors` (bindable) | done (the add cell; persisting through `uiState()` is the app's) |
| `swatches` | done |
| `fields="hex rgb hsv"` | done (hex types through the seam; rgb/hsv are own channel boxes, not `drag-value` nodes) |
| `swatch`, `popup`, `square`, `hue`, `alpha`, `handle` (`:dragging`, `:focus-visible`), `swatches`/`swatch-cell`, `fields` | painted parts with those names and states |
| verify: egui `DatePickerButton` features | `calendar_week`, `format`, highlight weekends (the `weekend` name) covered; `combo_boxes`/`arrows` replaced by keys and prev/next; `start_end_years` → `min`/`max` |
| P2 rows (time-edit, calendar systems, eyedropper, OKLCH) | not OW6 (OW-P2) |

### DOC1 state bits (WIDGET-PARITY Decided)

- `:open` on `date-picker` and `color-picker`.
- `:read-only` on `calendar` and `date-picker`.
- `:invalid` on the hex `::part(field)`.
- `:focus`/`:focus-visible` from Focusable. Days carry `:current` and `:selected`; handles carry
  `:dragging`.

### R rules (DESIGN §2)

- **R3:** hover restyles in the same frame (days, nav buttons, swatch cells, footer buttons).
- **R4:** the hex field and channel boxes, and both drop buttons, have the `control-bg`/`control-border`
  field look with radius `input-radius`. The inset shadow is not drawn (same as OW3's select).
- **R6:** day cells have list-row states: `list-hover`, `list-sel`/`list-sel-fg`, the `:current` ring.
- **R7:** popups are elevated (OW3's surface, `popup-shadow`).
- **R8:** focus is keyboard-only. Rows and fields get a 1px `focus` ring at −1; handles and the footer
  buttons get 2px at +1.
- **R9:** disabled kinds keep their shape at `opacity` .4 and show their tooltip (the own-widget walk). The
  `not-allowed` cursor shows on disabled days.
- **R10:** pointer on buttons and days, crosshair on the square, `ew-resize` on channel boxes.
- **R14:** both drop buttons show a chevron.

### VIS1 and pins

- VIS1 (`ci/vis1`) has no picker or calendar references; its metrics carry only the `codicon calendar`
  glyph. The pickers reuse surfaces VIS1 already checks: OW3's select field look and popup surface, the
  list-row colours and the focus ring.
- No new VIS1 check was added. The VIS1 gate was not run (it is not on master).
- Pins 1–7 (text) and 8 (menus) don't apply.

### Tests

- `tests/ow6.rs` (new):
  - `ow6_calendar_picks_roves_and_pages`
  - `ow6_calendar_first_day_weekdays_and_day_parts`
  - `ow6_color_editor_edits_through_square_strips_fields_and_swatches`, including typing in the hex field
    and an invalid hex
  - `ow6_date_picker_opens_its_calendar_and_commits`: confirm, Cancel, Alt+Down and keys, focus return,
    immediate, `min`, Escape
  - `ow6_color_picker_live_and_confirm`
- Unit tests: `widgets::calendar::tests` (2) and `widgets::color_editor::tests` (1).
- Existing tests:
  - `wg4b.rs::date_picker_saves_a_day_through_its_popup` and the gallery test now click the day's grid
    cell instead of egui's `>` button. The assertions are unchanged.
  - `color_picker_normalizes_hex_and_edits_through_its_popup` is unchanged, apart from dropping egui's
    emoji-tofu line.
  - `observe.rs` gives the new date and colour props valid values.
  - `widgets.rs::every_kind_has_a_tooltip_and_inputs_are_named` covers the new kinds. The bodies union
    their parts' responses into the host's, so hovering a part shows the host's tooltip.
- **Run locally after the last code edit** (thinkpadx1, `cargo-slot`):
  - nextest `-p svelters-egui` on binaries `ow6`, `wg4b`, `observe` and `spec`, plus
    `every_kind_has_a_tooltip` and the lib unit tests above. All pass except
    `wg4b::gallery_pages_drive_pickers_dnd_and_dialogs`, which needs the gallery bundle
    (`pnpm build`; CI builds it).
  - `cargo clippy -p svelters-egui --all-targets`: no warnings in OW6 files.
- **Not run locally:** the gallery test (no bundle), `focus_keys` (needs `pnpm install`), the full workspace
  nextest, conformance and VIS1.

## Handoff

- **OW6b:**
  - `editable` sectioned date field;
  - the month and year sub-pickers on `::part(month)`/`::part(year)`;
  - the rgb/hsv channel boxes as OW1b `drag-value`/`ValueBox` parts, which brings typed numbers and step
    buttons. `ColorBody`'s channel loop is the one place to change;
  - **`path-field`** (a new kind; the coordinator adds its WIDGET-PARITY row): a text field plus a browse
    button, with modes `file`, `dir` and `save`, plus filters and multi. The dialog goes off the UI thread
    through `dialogs.rs`' `DialogBackend` futures (rfd's async dialogs, polled by the runner), never
    blocking, as ZU3's lint requires. The text part takes the same text-input seam as the hex field.
- **Per-day part CSS (OW-CSS follow-up):** egui computes `::part()` styles once per host (single names, in
  the host's state). Two things are missing:
  - per-instance part states (a day's own `:hover`/`:selected`);
  - multi-name parts (`::part(day today)`, `day-parts` names).

  Both are painted from the UA tables over `::part(day)` today. The engine supports both
  (`PseudoElement::parts`, P10). What's missing is a `SceneStyle` hook that computes a part for given names
  and state, which OW-CSS owns.
- **Text input:** replace `color_editor::text_entry`'s body with OW1b's `Entry`, then OW4's `TextInput`.
- **Roving (coordinator note, 2026-10-22):** the calendar uses no roving unit. Its cursor is a date, moved
  by date arithmetic inside `CalBody`: arrows cross into the next or previous month, Up/Down move ±7 days,
  PageUp/PageDown move a month or a year, everything clamps to `min`/`max`. OW3's `RovingList` is a 1-D
  index over a fixed list, and OW1a2's `Roving` moves focus across nodes; neither crosses months. Nothing
  was added to `interact/`. If a grid unit is wanted later (for tables), `CalBody`'s key block is the one
  place to switch.
- **`page-changed(year, month)`:** the payload is a `[year, month]` list. OW1b's `flush_writes` change
  spreads a list over a signal of several args. Until it merges, JS sees one list argument.
- **IN1:** locale names for months and weekdays, and `first-day="auto"`.
