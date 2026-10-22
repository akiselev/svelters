# OW1b: own `slider`, `range-slider` and `drag-value` (DragToValue, Steppable, Draggable)

Track: WIDGETS (own widgets, ARCH-WIDGETS §7.2). Lane OW1b, branch `ci/ow1b`, macbook. Stacked on OW0a
(`ci/ow0` 7eef7362), then OW0b (6f013c72); OW0 and EV1 are now in master (2305a42d), and the branch is
merged with master. Required reading done: DESIGN §1, §2 (R1–R18), §4, §6; ICON-STYLE; QT-SURVEY and
WIDGET-PARITY §2.6/§2.7; ZED-LESSONS §1; VIS1 (`ci/vis1` record, §8 gaps); the OW0 "Write a widget" guide.

## What landed

| Piece | Where | What |
|---|---|---|
| `DragToValue` unit | `interact/scrub.rs` | Relative scrub (`speed` per point, Shift ×0.1, Ctrl/Cmd ×10) and absolute (a slider's track: value under the pointer, grab offset on the thumb so it does not jump; linear or log `Scale`, vertical, inverted). Intents `Start`, `Scrub(v)`, `Commit(v)` (release, when it differs from the start), `End`, `Cancel { restore }` (Escape). Owns `:active`, `:dragging`; claims Escape while dragging. |
| `Steppable` unit | `interact/step.rs` | Up/Down (+Left/Right, Home/End on sliders), PageUp/PageDown (`page`, ten steps by default), the wheel (`hover`/`focused`/`off`; one step per line or page notch, one per 40 pt of touchpad), AccessKit `Increment`/`Decrement`/`SetValue`. `StepParams::apply` clamps, wraps (Qt: from an end past it to the other end), rounds (10 places or the fixed decimals); adaptive steps (Qt AdaptiveDecimalStepType). a11y: numeric value, min, max, step, jump, actions (Increment/Decrement only when a step can change the value). |
| `Draggable` unit | `interact/drag.rs` | A plain drag past a threshold (`None`: egui's), axis, Escape cancels; for OW2/OW5 (sashes, scrollbar thumbs, DnD). |
| Pressable `accelerate` | `interact/press.rs` | On OW0b's `repeat`: the interval halves every 8 repeats, down to a fifth (Qt `accelerated`). |
| Value box | `widgets/value_box.rs` | The number field of `drag-value` and of the sliders' value boxes: Hoverable, Focusable, DragToValue (relative), Steppable, the interim entry, and `dec`/`inc` step buttons (Pressable repeat 500/100 ms). Implements the commit seam (below). R4 look: `input-*` tokens, 1 px boundary, the 1 px `focus` outline at −1 while focused, `invalid-border`; R5 read-only looks like text; R10 cursors (resize while scrubbable, I-beam while editing, pointer on buttons). |
| Entry (interim) | `widgets/entry.rs` | One-line editor on a `String`: typing over a select-all selection, Backspace/Delete, Left/Right/Home/End (+Shift), Ctrl/Cmd+A, Copy/Cut/Paste events, caret and selection paint, IME rect. OW4's TextInput on TC0 replaces it (same three calls). |
| Number formats | `widgets/numfmt.rs`, `numeric.rs` | IN1's locale format and parse (`numeric::format_grouped` added, `format` unchanged), decimals (fixed, from the step, or egui's rule from the scrub speed / track resolution), `prefix`/`suffix` (typed or not), `special-value-text`, `group-separator`, and the `formatter`/`parser` registry (`numfmt::register_formatter`/`register_parser`, named Rust functions). |
| Rail | `widgets/rail.rs` | Geometry and paint of the sliders' parts with Fluent 2 / WinUI metrics on VS Code tokens: 4 px track (`muted`, radius 2), fill/range (`accent`), rest, 20 px thumb (`input-bg` disc, 1 px `control-border`, accent core 12 px, 14 hovered, 10 pressed), ticks 1×4 px at 4 px (`muted`), mark labels (`font-size-sm`). |
| `drag-value` | `widgets/drag_value.rs` | The node is the value box. Extras: `step`, `page-step`, `buttons`, `commit`, `wheel`, `wrap`, `special-value-text`, `accelerated`, `step-type`, `correction`, `read-only`, `group-separator`, `formatter`, `parser`; events `input`, `submit` (with the row's `changed`, `commit`, `drag-started`, `drag-stopped`). |
| `slider` | `widgets/slider.rs` | Rail + optional value box + caption; units on the rail (`#[behavior]`: Hoverable, Focusable, DragToValue, Steppable). Extras: `page-step`, `orientation`, `ticks`, `tick-interval`, `marks`, `snap-to-marks`, `inverted`, `track-click`, `scale`, `commit`, `wheel`; events `input`, `commit`, `submit`. |
| `range-slider` | `widgets/range_slider.rs` | Two thumbs, each a tab stop with its own units and an AccessKit `Slider` named `<name> low`/`<name> high` (name: `text`, else `accessible-name`, else `tooltip`); a press on the rail moves the nearer thumb; a value box at each end (`SpinButton`, labelled by its thumb). Extras: `page-step`, `orientation`, `ticks`, `tick-interval`, `inverted`, `scale`, `commit`, `wheel`; events `input`, `commit` (no payload: read `low`/`high`). |
| Wrappers deleted | `leaf.rs`, `elements/{mod,inputs}.rs`, `scene/{node,tree,props,keys}.rs`, `layout.rs`, `a11y.rs`, `focus.rs`, `style.rs`, `embed/mod.rs`, `numeric.rs` | `W::Slider`, `W::RangeSlider`, `W::DragValue`, their egui calls, size copies, `focus::Role::Slider`, `numeric::{slider, drag_value, range_slider}`, and the `// OW: wrapper until OW1b` allow lines. CX3's drag-value commit stand-in is gone (the own widget raises `commit`). `widgets::OWN_ONLY` gives these tags the own widget under every switch (`KindSet::None` too). `keys::slider::*` still write; their borrowed read is `None` (the fields are in the widget struct). |
| Provider | `provider/mod.rs` | `WidgetCx::computed()`; an own widget's `Vec` payload spreads over a row signal with several args (`changed(low, high)`). |

## The commit seam (FM1 QW7, ARCH-WIDGETS §2.4, CX3)

- `changed` and `input`: every user change of the bound prop (keystroke in live mode, scrub step, key step).
  `changed` is dispatched first (EV1); a filter that prevents it prevents the write, and `input` is not sent.
- `commit`: at a commit point when the value differs from where that interaction began: drag released,
  Enter or focus lost after typing, each step (keys, PageUp/PageDown, wheel notch, step button, AccessKit).
  `drag-value`'s and `slider`'s `commit(value)` carry the value, so egui.json's
  `bindings.value.commit = {"event":"commit","arg":0}` holds (CX3 lowers `bind:value|commit` onto it).
  `range-slider`'s `commit` has no payload (an extra event can carry one kind-table value only).
- `submit`: Enter in a value box (`drag-value`, the slider's value box).
- Commit mode (`commit={true}`, Qt `keyboardTracking=false` / `tracking=false`): typing and scrubbing keep a
  draft that is painted; `value`, `changed` and `input` move only at the commit. Escape drops the draft (and in
  live mode restores the value from before the edit or drag). A host write that differs from where the draft
  began ends it.

## Parity rows (WIDGET-PARITY)

**§2.6 QSpinBox → `drag-value`:** `singleStep` (`step`), PgUp/PgDn ×10 (`page-step`), step buttons (`buttons`),
`wrapping` (`wrap`), wheel (`wheel`), `keyboardTracking=false` (`commit`), `editingFinished` (`commit`),
`specialValueText`, `accelerated`, `stepType` adaptive (`step-type`), `correctionMode` (`correction`),
`readOnly` (`read-only`, `:read-only`), `showGroupSeparator` (`group-separator`), locale display and parsing
(IN1), `textFromValue`/`valueFromText` (`formatter`/`parser` registry), `prefix`/`suffix`, `decimals`;
`::part(inc)`/`::part(dec)` with `:hover`/`:active` and `:disabled` at a limit without `wrap`, `::part(text)`;
`:dragging`.

**§2.7 QSlider → `slider`, `range-slider`:** `pageStep` (`page-step`), `orientation`, `tickPosition` (`ticks`),
`tickInterval` (`tick-interval`, 0 = page step), tick labels (`marks`, `snap-to-marks`), `invertedAppearance`
(`inverted`), `tracking=false` (`commit`), `sliderPosition` (the draft), track click jump/page
(`track-click`), wheel, `scale="log"`; parts `track`, `fill` (`range` on the range slider), `rest`, `thumb`
(`low-thumb`/`high-thumb`), `tick`, `mark-label`, `value-box` (`low-value`/`high-value`).

## R-rules, VIS1 and pins

- R3 (hover in the same frame: the thumb's core grows), R4 (sunken fields), R5 (read-only as text), R8
  (keyboard-only focus ring: the thumb's outline under `:focus-visible`; fields: 1 px at −1 while focused),
  R9 (disabled hosts keep Hoverable; parts paint `disabled-fg`), R10 (cursors), R17 (state not by colour alone:
  the fill and the thumb position carry the value).
- VIS1: no VS Code reference exists for slider and range slider (VIS1 record: "review only"); per Q4 they use
  Fluent metrics on the VS Code tokens. The field look of `drag-value` uses VIS1's `input-*` roles (the "number
  box" probe). The rule-based checks (≥ 3:1 boundaries) and the PaintTrace targets (`slider::part(thumb)`,
  `drag-value::part(inc)`) are VIS1c's to add on this branch's parts.
- Pins: WG3s pin 10 (pointer/drag thresholds) is not captured yet; `Draggable` takes its threshold as a param.

## Decisions taken in the lane (docs left them open)

- **Parts are paint parts with their own interaction rects**, not shadow nodes: OW0 creates no node parts yet.
  Each thumb, value box and step button calls `ui.interact` with an id derived from the node's and keeps its own
  state bits; part styles come from the parts' UA tables until OW-CSS computes `::part()` styles. ARCH §6.1 lists
  `track`/`thumb`/`low`/`high`/`inc`/`dec` as node parts; switching to node parts later keeps the names.
- **Wheel default `focused`** for all three (FM1's choice: a scrolled property grid does not change values),
  not a Policy row yet (OW0b's Policy has none for it).
- **`commit` is the commit-mode prop on all three** (FM1's `commit={true}`), next to the `commit` event.
- **`marks` is a string** (`"0:Low, 50, 100:High"`): an extra prop of a built-in kind must be a kind-table
  type, so WIDGET-PARITY's `[{ value, label }]` waits for record-typed extras.
- **Escape in a value box** restores the text and the value from before the edit and keeps focus (Qt keeps
  focus; egui dropped it).
- **The formatter/parser registry has no policy hook** (Rust-only registration, pure text functions; Svelte can
  only name a function the host registered), as IN1's decision for formatting. Recorded for ARCH-SECURITY.

## Deferred

- `step(n)`/`select-all` methods (the macro has no methods for own widgets yet).
- Paging repeat while the track is held (`track-click="page"` steps once per press).
- Entry: word motions, undo, IME preedit display, caret blink, AccessKit text runs (OW4's TextInput).
- EV3 scope Tab order of the range slider's unfocused thumbs (the walk records the node's response; a thumb is
  recorded while it has focus).
- `inverted-controls` (P2), `step` signal with `{ n, page }` (P2), `base` (P2), input masks and `fixup` (OW4b).
- PaintTrace-based VIS1 checks for the new parts (VIS1c).

## Tests

`crates/svelters-egui/tests/ow1b.rs` (FM1's two drag-value cases ported from `ci/fm1` `tests/forms.rs`, spin
box parity, held-button repeat, slider keys/page/inverted/track-click/commit-on-release, drag Escape, range
slider thumbs), unit tests in `interact/{step,scrub,press}.rs`, `widgets/{numfmt,entry}.rs`. Existing tests that
now run on the own widgets: `tier0` (range slider, drag-value, bad input), `observe::slider_clamp_is_announced`,
`intl`, `scene`, `counter`, `gallery`, `bind_targets`, `spec`.
