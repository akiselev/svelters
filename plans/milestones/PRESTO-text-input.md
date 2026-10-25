# PRESTO-text-input: text editing on quire-text (presto batch 2, lane TEXT-INPUT)

Lane file: `plans/cloud/presto-b2/LANE-text-input.md`. Branch `ci/presto-text-input` (from `ci/presto-base`
at `e34079e6`; `ci/presto-base` at `622aef24`, with the presto-core, presto-text and presto-platform
skeletons, then at `ac7cc8c4`, with the WIDGETS-BASIC substrate, merged in). Owns `crates/presto-widgets/src/text/**` and `crates/presto-widgets/tests/text_pins.rs`.

## What landed

`crates/presto-widgets/src/text/` (module table in `text/mod.rs`):

| Module | What |
|---|---|
| `editor.rs` | `TextEditor`: one field's quire-text `TextDocument` + `TextView` (quire-text at cadatomic's pin `3e78dcd3`), per-platform `TextPolicy`, keys through `keys.rs`, IME events (winit-style `ImeEvent`) and the `TextInputClient` operations in document-wide UTF-16 offsets (the IME document is the text with the preedit spliced in), pointer selection (click, double/triple click, drags by word and line, Shift+click), the middle-click PRIMARY paste, focus rules (select-all on keyboard/programmatic focus, unfocused selection kept or collapsed, GTK's one selection owner), clipboard/PRIMARY/beep requests (`TextRequest`), input filters |
| `kind.rs` | `TextEdit`: `impl presto_core::Kind` for `text-edit` (spec with change classes, `text` native-owned; `prop_changed`, `measure`, `prepare`, `record` through `RecordCx`, `event` for keys, IME, pointer, focus; `text_input`; `a11y`), signals emitted with `EventCx::emit` |
| `geometry.rs` | presto-text's `TextLayout` as the field's geometry (`LaidOut`: lines without their line feed, affinity, runs recorded through `RecordCx::draw_run`) and `SystemLayouter` over `TextSystem` |
| `field.rs` | `TextField`: the `text-edit` kind's state: layout (through the shim), horizontal/vertical scroll to the caret, `record` (selection quads behind the glyphs, placeholder, preedit underline with the active clause thicker, clipped to the content box), the caret as `FastCommit::Caret`, the IME candidate area, `impl TextInputClient` (bounds and character index through the layout, password-aware), AccessKit role/value/placeholder/actions, signals (`changed`, `commit`, `submit`, `cancel`, `select`, kept from svelters' `text-edit` names), `TEXT_EDIT_PROPS` with change classes |
| `keys.rs` | the §11.2 IME-first route (`route`: composing → IME; IME source active and a printable key without a chord → IME) and the key maps: GTK/Qt/Windows (Ctrl words, Ctrl+Backspace/Delete, Shift+Delete/Insert, Ctrl+Insert, Ctrl+Y and Ctrl+Shift+Z, Ctrl+Shift+U hex entry on GTK) and AppKit's standard bindings (Option words, Cmd line/document, Emacs Ctrl keys, Home/End scroll only) |
| `policy.rs` | `TextPolicy::of(Platform)`: FX1's text rows from svelters-egui `native/mod.rs` (word rules, word-move collapse, focus selection rules, PRIMARY, undo grouping, blink, double-click time, password mask) |
| `caret.rs` | `CaretState` (fast commit on move, hide, or blink restart) and `blink_visible`/`next_blink` (the blink rule, see request R3) |
| `lines.rs` | `GeometryLines`: quire-text `VisualLines` over a layout (Up/Down/PageUp/PageDown and Home/End on wrapped rows, goal x in 1/64 pt) |
| `shown.rs` | the shown text (password mask, preedit) and its byte mapping to the document |
| `utf16.rs` | document-wide UTF-16 ↔ byte over quire's `Text` (O(n) chunk walk; request Q-R1) |
| `number.rs` | `NumberField`: the value boxes' text part on quire-text (replaces svelters-egui's interim `Entry`): `NumFormat` (prefix/suffix, special text, decimals, formatter/parser registry), `Steps` (svelters-egui `StepParams` arithmetic: step, page, wrap, adaptive, rounding), `Correction`, commit mode, the FM1 commit seam (`Input`/`Commit`/`Submit`), Up/Down/PageUp/PageDown steps, wheel notches |
| `hex.rs` | `HexField`: the colour editor's hex field (filter, `#rgb`/`#rgba`/`#rrggbb`/`#rrggbbaa`, apply on Enter/blur, `:invalid`, Escape restores) |
| `cell.rs` | `CellEditor`: the table's cell editor (Enter commits, Escape reverts, Tab/Shift+Tab commit and move ±1, blur commits; F2 selects the value, typing replaces it) |
| `shim.rs` | stand-ins for wave-1 APIs (below), and `MonoGeometry`/`MonoLayouter` (a fixed-advance layout for tests and headless use; records no glyphs) |

Tests (written, **not run**, per the batch rule): unit tests in every module (editing, selection, word
motion per platform, IME composition by events and by `TextInputClient`, UTF-16 mapping across surrogate
pairs and rope chunks, password, char limit, read-only, clipboard and PRIMARY, multi-click and drags, focus
rules, wrapped-row motion, caret commits and blink, record output, value box seam, hex, cell editor), and
`tests/text_pins.rs`: pins 1–7 (the `egui_measure` set) on `TextField` under the `Gtk` and `Qt` policies
against `conformance/golden/linux/` (GTK 4.24.1, Qt 6.12.0), writing `target/na/linux/presto-gap.md` and the
job summary. It is a measurement (fails only when the harness breaks), like `egui_text_edit_against_windows_goldens`.

## Shims (the consolidation replaces them)

`text/shim.rs`:
- `TextServices` (installed per UI thread with `install_services`): what presto-core's contexts do not
  hand a kind yet: the clipboard, the caret's fast-commit channel, the event time. The app runner (lane
  APP-TEST / svelters-presto) installs it beside the substrate's text system
  (`host::install_text_system`); without both `text-edit` records its box only and takes no input.
  Request R4.

**`text-edit` and the substrate.** `text-edit` implements `presto_core::Kind` directly, not through the
substrate's `Hosted<W>` / `shim::core` bridge (as the coordinator asked kinds to be built): the bridge
carries no `Event::Ime` and no `text_input` hook, both required by architecture §11.3. It shares the
substrate's per-thread `TextSystem` (through the new `host::with_text_system`: layouts with geometry,
which the substrate's `TextShaper` does not expose) and its platform policy (`TextPolicy::from_policy`
over `host::policy()`; the word rules, undo grouping, PRIMARY and mask stay in `text/policy.rs`, which
`interact::Policy` lacks). Question 4 below.
- `Clipboard`: presto-platform's `Clipboard` is by `ClipboardKind` and presto-widgets does not depend on
  presto-platform; the app adapts it.
- `TextGeometry`, `Layouter`, `RecordSink` are the field's own interfaces (implemented for presto-text's
  `TextLayout`/`TextSystem`, presto-core's `RecordCx`, and the test `MonoGeometry`/`ListSink`).

## Cross-lane edits

- `crates/presto-widgets/Cargo.toml`: `quire-text` (git, rev `3e78dcd3bf74abc2c81209f0109caff4d9193529`)
  and the dev-dependency `svelters-conformance` (pins and goldens). Other lanes edit this file too: a
  merge conflict there is additive.
- `Cargo.lock`: quire-text and its dependencies (ropey, unicode-segmentation, unicode-general-category).

- `crates/presto-widgets/src/host.rs` (WIDGETS-BASIC's): one additive function, `with_text_system`, so the
  text kinds lay out with the same per-thread `TextSystem` the other kinds shape with.

No change to `presto-base` or any wave-1 crate.

## Requests

- **R1 (quire-text):** document-wide UTF-16 offsets in O(log n): `Text::byte_to_utf16_offset` and
  `utf16_offset_to_byte` (ropey's `char_to_utf16_cu`/`utf16_cu_to_char`, as `Text::utf16_to_byte` does per
  line). presto-widgets walks chunks meanwhile (`text/utf16.rs`); fine for fields, slow for long documents.
- **R2 (quire-text):** a preedit that starts over a selection: quire draws the preedit at the caret and
  keeps the selection; AppKit and TSF replace the selection. The editor deletes it first
  (`TextEditor::begin_composition`); a quire `PreeditUpdate` flag would make it one transaction.
- **R3 (presto-base / compositor):** `FastCommit::Caret { window, rect, revision }` carries no blink cycle or
  timeout, but they differ per platform (GTK 1200 ms with a 10 s timeout, Qt 1000 ms, Windows and macOS
  1060 ms in FX1's table). Recommendation: the compositor takes them per window in its config (or a new
  `FastCommit::CaretBlink { window, cycle, timeout }` variant, a breaking change for exhaustive matches), and
  applies `text::caret::blink_visible`'s rule (on for the first half of each cycle, steady after the timeout,
  restarted by every `Caret` commit).
- **R4 (presto-core):** for the text kinds: the text system (layouts with geometry, `TextLayout`) in
  `PrepareCx` and `EventCx` (a key can move over rows the previous key changed), the clipboard and the
  fast-commit channel in `EventCx`, the event time in `Event`, the node's padding edges and its
  window transform in `NodeView` (the kind uses the padding box and `pos - local`/the sum of layout rects
  meanwhile); GTK's one selection owner as a document rule (`TextEditor::collapse_to_start` is the hook;
  the pins fixture does it in `settle`); the IME-first route (`keys::route`) before the command keymap.
- **R5 (presto-text):** done by lane TEXT (`lines()`, `caret_rect_at(Cursor)`, affinity); its lines
  include the trailing line feed, which `geometry::LaidOut` strips.
- **R6 (presto-platform):** `Clipboard` with PRIMARY get/set/clear (X11, Wayland); `ImeEvent::Preedit`
  cursor `None` read as "hide the caret" (the field does so).

## Not done (left out, not faked)

- Bidi caret motion and selection (quire-text has none; architecture §11.3 notes it).
- AccessKit text runs and `TextSelection` (needs per-run child nodes from presto-core's a11y pass); the
  field gives role, value (masked for passwords), placeholder, read-only and `SetValue`/`ReplaceSelectedText`.
- Locale-aware number display and parsing (svelters-egui IN1, `svelters-intl`): `number.rs` uses the
  default notation; the locale hook goes in with the intl port.
- The value boxes' scrub (DragToValue), step buttons (Pressable repeat) and state bits: the interaction
  units' (WIDGETS-BASIC); `NumberField` is the text part they compose with.
- Text drag and drop, the context menu, spell-check underlines, multi-cursor in fields, macOS
  Option+Up/Down paragraph motions.
- The beep (`TextRequest::Beep`) is dropped by `TextField::serve` until the platform has a bell.

## Questions for the coordinator (with recommendations)

1. **Undo grouping on Windows and macOS.** FX3 measured GTK and Qt only; `policy.rs` uses quire's Monaco
   grouping on Windows and macOS. Recommendation: keep Monaco until WG3w/WG3m measure the native fields.
2. **Caret blink parameters** (R3): which channel? Recommendation: per-window compositor config, set by
   the platform policy, so `FastCommit` stays as it is.
3. **The pins' floor.** `text_pins.rs` reports without a ratchet. Recommendation: after the first green CI
   run, set per-pin floors from its job summary (as `egui_measure`'s `FLOOR`).

4. **`text-edit` outside `Hosted<W>`.** Recommendation: keep it on `presto_core::Kind` directly; if kinds
   must all go through the bridge, add `Event::Ime` and a `text_input` hook to `shim::core::Kind` and
   `Hosted<W>` first (WIDGETS-BASIC's files), and the text kinds move over in a few lines.

## What I ran and did not run

- Ran: `cargo check -p presto-widgets --tests` (clean, no warnings in presto-widgets), and once, after CI
  reported three failing expectations, `cargo test -p presto-widgets --lib text::` (37 passed).
- CI on `ab3b6934` (run 40223629517): the five svelters workflows green; `presto` red for reasons outside
  this lane: Linux `cargo check` fails building `yeslogic-fontconfig-sys` (the runner lacks
  `libfontconfig-dev`; the workflow's apt list needs it, presto-text's dependency), and on macOS/Windows
  presto-render's GPU references, presto-anim, presto-frame and presto-platform unit tests fail. In
  presto-widgets, `text_pins` passed (the measurement ran) and three text unit tests failed on wrong
  expectations (Qt's double click takes the trailing space; the field scrolled to reveal the caret; GTK's
  keyboard-focus select-all did not set PRIMARY: a real gap, now fixed); fixed in the next commit.
- Not run: clippy, other crates.
