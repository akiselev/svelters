# POL1-TEXT: text fields, selection, rich text, captions (polish-b1, lane text)

Branch `ci/pb1-text`, base `master` `f77fad0` (plus the batch brief `9d97e7d`). Brief:
`plans/cloud/polish-b1/BRIEF.md`, `LANE-text.md`. Work list: GAL2 "Polish findings" rows in this lane's scope.

## Scope

Text fields (padding, invalid, read-only, multi-line overflow), selection contrast, rich text `<code>`, label
weights, caption wrapping and eliding (button, checkbox, switch, collapsing-header long text).

## Findings

| Finding (GAL2 row) | Cause | Fix (file) | Before/after looked at | Snapshot tests moved |
|---|---|---|---|---|
| text-edit/*, multiline/*: no inner padding, text touches the border | `base.css` gave `text-edit` no padding | `text-edit { padding: 3px 6px }` (VS Code's input box at `control-h`) — `theme/base.css` | yes, 4 themes, x1 and x2 | none |
| (own) text fields draw no field fill: same colour as the panel | base.css's text-edit rule still assumed egui filled the field (`Visuals::text_edit_bg_color`); skelter paints only what CSS gives | `background-color: var(--sv-input-bg)` — `theme/base.css` | yes, 4 themes | none |
| text-edit/invalid, password/invalid: no error border | no `:invalid` rule (the state bit was set) | `text-edit:invalid { border-color: var(--sv-invalid-border) }`; in high contrast, where `inputValidation.errorBorder` equals `input.border` (`contrastBorder`), `var(--sv-danger)` under `@media (prefers-contrast: more)` — `theme/base.css` | yes, 4 themes, x1 and x2 | none |
| text-edit/read-only, multiline/read-only: look editable | no rule for the `read-only` prop | `text-edit[read-only=true]`: no fill (the text sits on its surface) and the `border` hairline; high contrast adds `border-style: dashed` (fills and borders are equal there) — `theme/base.css` | yes, 4 themes, x1 and x2 | none |
| dialog/form (light-modern): selection is dark text on accent blue | the kind used a hard-coded dark selection (`#264F78`) in every theme; and in HC the theme's fill (`#F3F518` yellow, `#0F4A85` navy) hides the text colour | the kind reads `--sv-selection` from its computed style (fallback the old colour) — `text/kind.rs`; the field redraws the selected glyphs, clipped to the selection, in black or white when the text has under 4.5:1 on an opaque fill (VS Code's HC `editor.selectionForeground` idea), for every field (`text-edit`, table cell editor, pickers) — `text/field.rs` | yes, 4 themes, x1 and x2 | none |
| rich-text/runs, rich-text/nested: `<code>` has no field fill | run records had no background: the run's computed `background-color` was never folded | `RunNode.bg` (host reads the run's computed `background-color`) → `bg` in the run record (inherited by inner runs) → `RunFmt.bg` → `rich::paint_backgrounds` fills each placed run's rect (±1 pt, 3 pt radius) under the glyphs — `controls/{run,rich,label}.rs`, `host.rs`; the `code` fill is `var(--sv-toolbar-hover)` (translucent grey: `control-bg` was white on near-white in light-modern and black on black in HC) — `theme/base.css` | yes, 4 themes, x1 and x2 | none |
| rich-text `<code>`: no monospace face | the run does get `--sv-font-mono` (`"JetBrains Mono", monospace`); the gallery's bundled test fonts have no monospace face, so it falls back to Noto Sans | not this lane: request to **gallery** (bundled or system mono face, GAL1 Q3) | yes | — |
| label/weight: 300 draws like 400 | `font-weight=300` reaches the text style (checked); the bundled fonts are Noto Sans Regular and Bold only, and CSS font matching picks Regular for 300 (correct, no synthetic light) | not this lane: request to **gallery** (a Light face or variable Noto Sans in the bundled set, or `--system-fonts`) | yes | — |
| button/long-text, checkbox/long-text, switch/long-text: captions never elide | the kinds' `measure` ignored the available width; parts were laid out at their natural size | `common::fit_width` caps the measured width at a definite available width (CSS fit-content) in button, checkbox, radio and switch — `controls/{common,button,checkbox,radio,switch}.rs`; `layout_parts` gives a row's overflow to the `label` part — `paint/parts.rs`; `paint::fit_layout` elides the caption into its rect (end `…`) — `paint/text.rs`, called from `button::paint` and `checkbox::paint_indicator` | yes, 4 themes, x1 and x2 | none |
| collapsing-header/long-text: header grows past its parent | the caption is skelter-core's `chrome_min_size` (the whole caption is the node's min width, so a content-sized parent never squeezes it); the paint already elides | **recorded, not built** (a layout feature): see Q1 | yes | — |
| multiline/overflow: lines cut with no scroll bar | the field scrolls itself (`self_scroll`, a compositor offset, no re-record on wheel scrolls) and draws no bar | **recorded, not built**: see Q2. The padding fix makes the cut fall inside the padding box | yes | — |

Looked at beyond the list (label, password, text-edit hover/focus sheets): nothing else in this lane's scope.
Seen in passing, for other lanes: **switch** in hc-dark has an invisible track (only the knob shows) — states lane.

## Before/after images (container paths, not committed)

- `target/gallery/before/text-edit.png` → `target/gallery/after/text-edit.png`: field fill, padding, the red invalid
  border; `target/gallery/after/te-ro-inv.png`: read-only and invalid at x1/x2 in the four themes (dashed read-only
  and the error colour in HC).
- `target/gallery/before/dialog-form.png` → `target/gallery/after/dialog-form.png`: light-modern selection is now
  light blue under dark text; HC Dark black on yellow, HC Light white on navy.
- `target/gallery/before/rich-text-runs.png` → `target/gallery/after/rich-text.png`: `make check` on a grey field.
- `target/gallery/before/checkbox-long-text.png` → `target/gallery/after/checkbox-long.png` (and `button-long.png`,
  `switch-long.png`): captions end in `…` inside the 336 px box.

## Story changes

- `text-edit/story.toml`: `size` 320×72 → 320×88, so the invalid variant's message fits under the padded field.

## Tests

- Added: `text::field::tests::selected_text_reads_on_the_selection_fill`, `controls::run::tests::a_background_folds_and_inherits`,
  `paint::parts::tests::the_label_gives_up_a_rows_overflow`, a `bg` case in `controls::rich::tests::a_plain_run_list`.
- Run: `cargo test -p skelter-widgets --lib -- text:: controls::rich controls::label controls::run controls::button
  controls::checkbox controls::switch controls::radio paint:: host::` → 156 passed (before the new tests);
  the four new tests → 4 passed.
- `skelter-gallery check`: 31 stories, 135 variants, 0 problems.
- Not run locally: the workspace tests, conformance, other crates' tests (CI is the gate).

## CI

(filled in below as runs finish)

## Questions (for the coordinator)

- **Q1 (collapsing-header caption).** skelter-core makes a kind's `chrome_min_size` its min size, so a header's
  caption can never elide. Recommendation: let core take a chrome *content* size instead (it adds to the
  max-content width; the min-content width is the twistie and padding only), so the header is caption-wide when
  there is room and elides when there is not. A skelter-core layout change, so recorded, not built.
- **Q2 (multi-line overflow).** A scroll bar for multi-line fields: embed the views lane's `views::Scroller` (overlay
  bars, fade, thumb drag) in `TextEdit`, with the thumb in the fixed part re-recorded when the offset changes (a
  wheel scroll is a core offset today, with no record). Recommendation: do it after the views lane's scroll-bar
  fixes merge, in a text follow-up.
- **Requests to gallery:** a monospace face and a light weight in the gallery's font set (or `--system-fonts`), for
  `<code>` and `font-weight: 300`.
- **Cross-lane files:** the caption fix touches states-lane files (`controls/{button,checkbox,radio,switch}.rs`,
  `paint/{parts,text,mod}.rs`) in small hunks: the measure's width, the label's paint, and the shared part layout;
  and `controls/common.rs` (controls lane) with one appended function. Merge: keep both sides.

## Handoff

Done: every row in scope fixed and seen, or recorded with a recommendation (Q1, Q2, the two font requests).
