# FX3: egui text editing against the native references (pins 4–6)

Track: WIDGETS. Follows WG3b (`plans/milestones/WG3-native-references.md`, pins 4–6 and their first egui
measurement) and FX1 (`plans/milestones/FX1-egui-focus-words.md`, whose approach this repeats). User decisions
used (`plans/research/EGUI.md` §8): W1 (native per-platform behaviour), W2 (GTK4 and Qt both Linux
references), W4 (keep egui `TextEdit`, close the gaps around it). `TextEdit` is not forked: every change sits
before or after it in the `text-edit` leaf, switched per toolkit by `svelters_egui::native::Policy`.

## As built

| Piece | Where | What |
|---|---|---|
| Policy switches | `crates/svelters-egui/src/native/mod.rs` | `undo`, `cua_clipboard_keys`, `paste_keeps_breaks`, `primary_clears_on_collapse`, `ctrl_shift_u` (`ShiftU::{Egui, Nothing, HexEntry}`), `enter_keeps_focus`. GTK and Qt set them; `Native::Egui` keeps egui's behaviour. |
| Undo models | `crates/svelters-egui/src/native/undo.rs` | `History` per field: `UndoRules::Gtk` (fitted to GTK 4.24.1's golden) and `UndoRules::Qt` (`QWidgetLineControl`'s command history, ported). Pure; unit tests replay pin 4 against both goldens. |
| Key layer | `crates/svelters-egui/src/native/edit.rs` | `before` (ahead of FX1's word keys) and `after` (after `TextEdit` and W5) around the leaf's `TextEdit`. |
| PRIMARY (W5 fold) | `crates/svelters-egui/src/primary.rs` | Provider over DX2's `ClipboardBackend` (`install_backend`; `use_system` now installs `platform::SystemClipboard`'s `ClipboardKind::Primary`; W5's own arboard code is gone). `clear` for GTK. No second middle-click paste when egui already pasted `MiddleClickPaste`. |
| Leaf | `crates/svelters-egui/src/leaf.rs` (+9 lines) | The two calls, `return_key(None)` when Enter keeps focus, the W5 paste guard. |
| Measurement | `crates/svelters-conformance/tests/egui_measure.rs` | Keys fed one event per kittest step; eframe's `RequestPaste` and the runner's PRIMARY provider modelled; floors raised. |
| Tests | `crates/svelters-egui/tests/native_edit.rs`, `native/undo.rs` | `fx3_*`: chords, undo through keys, hex entry and Enter, line breaks in a paste, one middle-click paste and PRIMARY cleared, the unfocused-selection repaint loop. |

### Behaviour, per toolkit

| Behaviour | GTK (Linux default) | Qt | egui (Windows, macOS for now) |
|---|---|---|---|
| Undo grouping | one step per word (an insertion merges unless it starts with white space after a non-space; a paste merges like typing), each deletion its own step, typing over a selection one closed step, a caret move ends merging | a typing run one step, a BackSpace or Delete run one step, a selection removal and the typing after it one step, Separators at caret moves and pastes | egui's `Undoer` (time-based) |
| Undo of a deletion | caret after the restored text; a deleted selection comes back selected, caret at its start | Qt's walk (`internalUndo`/`internalRedo`) | egui's |
| Ctrl+Insert, Shift+Insert, Shift+Delete | copy, paste, cut | same | nothing, nothing, delete (egui-winit maps them on Windows only) |
| Paste with line breaks into a one-line field | kept | kept | turned into spaces |
| A focused selection collapses | PRIMARY emptied | PRIMARY kept | PRIMARY kept |
| Middle click with PRIMARY empty | focuses, caret stays | egui's | egui's |
| Ctrl+Shift+U | hex entry: digits in a preedit drawn at the caret (text unchanged), Space or Enter commits, Escape cancels, BackSpace deletes a digit | nothing | egui's Ctrl+U (delete to the line start) |
| Enter in a one-line field | keeps focus | keeps focus | ends editing, drops focus |

How each is done without touching egui:

- **Undo, Redo** (Ctrl+Z; Ctrl+Shift+Z, Ctrl+Y, and EV3's `TextCommand::Undo`/`Redo`, which arrive as those keys):
  taken out of the input before `TextEdit` runs and answered by the field's `History`; egui's `Undoer` never sees
  them. `after` feeds the history each pass's edit (text before and after, and what made it, from the pass's
  events: typing, paste, BackSpace, Delete, Ctrl+BackSpace/Delete, cut) and each caret move. A text set from
  outside (a prop) starts a new history, as `setText`/`gtk_editable_set_text` do.
- **CUA chords**: Ctrl+Insert becomes `Event::Copy`, Shift+Delete `Event::Cut`, Shift+Insert sends
  `ViewportCommand::RequestPaste` (eframe then reads its clipboard and sends `Event::Paste`, as for Ctrl+V).
- **Keys stay claimed for EV3**: the keys taken before `TextEdit` (FX3's and FX1's word keys) are put back into
  the input after it ran, so EV3's `focus::record` (right after the leaf) claims them for the field
  (`consumed_keys`) and removes them before anything else sees them.
- **Line breaks**: a `Paste` with `\n` or `\r` into a one-line field is inserted by `before` (selection replaced,
  `char-limit` kept); `TextEdit` would turn the breaks into spaces.
- **Hex entry**: `before` keeps the digits in the field's state and swallows keys while it is open; the commit
  is an ordinary insertion (and an undo step). The preedit is painted on a tooltip-order layer at the caret.
- **PRIMARY**: W5's select-to-copy already put a keyboard selection into PRIMARY (through the installed
  provider). GTK's emptying is `primary::clear` from `after` when the focused selection owner (FX1) collapses.

### Fixes found on the way

- **Double middle-click paste under eframe** (W5): egui 0.37's `TextEdit` pastes `MiddleClickPaste` (sent by
  egui-winit with the press) and W5 pasted again on the release. W5 now skips when the integration sent it.
- **Endless repaint** (FX1): egui collapses an unfocused field's selection on every pass and FX1 (GTK) put it
  back with a repaint each time, so the app repainted continuously while an unfocused field held a selection.
  It now repaints only when the range differs from the one before the pass.
- **Measurement artifact** (WG3b, egui_measure): a kittest `step` runs one pass per queued event and keeps only
  the last pass's output, so a copy or a settled selection made on a key press was lost. WG3b's "a keyboard
  select-all does not become PRIMARY" and the copy-alt miss were partly this. Keys are now fed one event per
  step and every pass's output is collected.

## Measured (kittest, `OperatingSystem::Nix`, against the checked-in goldens)

`tools/dev/cargo-slot cargo nextest run -p svelters-conformance -E 'test(egui_text_edit_against_native_goldens)'`.
Before is the ratchet as WG3b and FX1 left it (the `Gtk` run against GTK 4.24.1, the `Qt` run against Qt 6.11.3,
whose 243 checkpoints equal 6.12.0's); after is this lane.

| Pin | Script | Checkpoints | before gtk | before qt | after gtk | after qt |
|---|---|---|---|---|---|---|
| 4 | undo-typing | 11 | 6 | 11 | **11** | **11** |
| 4 | undo-edits | 21 | 12 | 10 | 20 | **21** |
| 4 | undo-paste | 10 | 6 | 6 | **10** | **10** |
| 5 | clipboard-keys | 20 | 12 | 12 | **20** | **20** |
| 5 | paste-multiline | 4 | 1 | 1 | 1 | 1 |
| 5 | primary | 8 | 5 | 5 | **8** | **8** |
| 6 | dead-keys | 12 | 8 | 12 | 8 | **12** |
| 6 | unicode-entry | 13 | 1 | 1 | **13** | **13** |
| | **pins 4–6** | **99** | **51** | **58** | **91** | **96** |
| | pins 1–3, 7 (FX1, unchanged) | 144 | 141 | 139 | 141 | 139 |
| | **all 18 scripts** | **243** | **192** | **197** | **232** | **235** |

The ratchet (`FLOOR` in `egui_measure.rs`) now holds the after numbers. The stock `Egui` columns of the report
moved from 133/133 to 133/136 (equal to GTK / to Qt): the measurement fixes above, not a behaviour change.

What is left on pins 4–6:

- **undo-edits #19 (GTK)**: redoing the merged `delta` insertion selects its last character (`[15,16]`); the
  model puts the caret after it. Not reproduced: the evidence is one checkpoint.
- **paste-multiline (both, 1 of 4)**: line breaks are kept now, but egui-winit and eframe turn CRLF into LF
  before svelters sees a Ctrl+V paste (the harness models it), and the goldens keep `\r\n`. A paste through
  EV3's `TextCommand::Paste` (the host's clipboard, verbatim) keeps CRLF. Matching Ctrl+V would need svelters
  to read the clipboard itself instead of egui-winit.
- **dead-keys (GTK, 8 of 12)**: GTK's simple input context (a letter that does not compose gives letter +
  U+0301; dead key + BackSpace cancels only). Platform input layer, not FX3's list.

## Not done / follow-ups

- **EV3 seams** (focus.rs is EV3's): while a hex entry is open, `focus::composing` does not report it and
  `focus::claims` does not claim Ctrl+Shift+U, so the host keymap is not told, and an Enter that commits a hex
  entry also reaches the scope's default button. Recommendation: `composing` and `claims` consult
  `native::edit` (a `pub(crate) fn hex_open(ctx, id)`), a few lines in focus.rs.
- The preedit is drawn but not exposed to AccessKit, and not observed by the pins (GTK has no public read of it).
- Provider kinds (`#[svelters::widget]`) get none of this, as with FX1.
- Windows and macOS keep egui's behaviour until WG3 has their goldens (on Windows egui-winit already maps the
  CUA chords).

## Verification

Run locally (thinkpadx1, after merging origin/master with EV3+): `cargo check -p svelters-egui
-p svelters-conformance --all-targets`; `cargo nextest run -p svelters-egui -p svelters-conformance -E
'test(fx3_) | test(fx1_) | test(text_edit_uses_the_primary_selection) |
test(egui_text_edit_against_native_goldens) | binary(focus_keys)'` (26 tests, all pass) and
`-E 'binary(clipboard) | binary(animator)'` (10, pass). Not run locally: the rest of the svelters-egui and
svelters-testkit suites (TK1 snapshots included), every other crate, conformance; CI runs them.

## Handoff (2026-10-22): stopped, svelters draws its own widgets

Stopped by the user's decision (via the coordinator): svelters will draw all its own widgets, and egui stays
the engine only (painter, input, text shaping, layers, AccessKit). FX3 ends at `ci/fx3` e6e8c5e6 plus this
note. It was merged with origin/master at c41dca7a (EV3+). It was **not** merged with later master (424ebcf4:
CS5g, WG5a cell editors) and has no clippy run (ZU3). CI was started on e6e8c5e6 and not waited for.

**Done** (all measured and tested locally, see above): undo grouping (GTK and Qt), the CUA chords, line breaks
kept by a paste, GTK PRIMARY (cleared on collapse, middle click without paste), Ctrl+Shift+U hex entry, Enter
keeping focus, the W5 fold onto `ClipboardBackend`, the double middle-click paste, FX1's repaint loop, and the
egui_measure per-pass output fix. **Not done**: EV3 seams for the hex entry (`composing`, claiming
Ctrl+Shift+U, Enter reaching the default button); CRLF through Ctrl+V; GTK's redo selection (undo-edits #19);
GTK dead keys.

**Carry over to the own-widget text field** (keep these intact; they don't depend on egui's `TextEdit`):

- `crates/svelters-egui/src/native/undo.rs`: the undo models (`History`, `UndoRules::{Gtk, Qt}`, `EditKind`,
  `Snap`, `splice`). They are pure: a field feeds them text and selection before and after each edit, plus caret
  moves, and they answer Undo and Redo. The `fx3_undo_*` unit tests replay pin 4 against both goldens.
- The behaviour spec in "Behaviour, per toolkit" above, and the `Policy` switches in `native/mod.rs`
  (`undo`, `cua_clipboard_keys`, `paste_keeps_breaks`, `primary_clears_on_collapse`, `ctrl_shift_u`,
  `enter_keeps_focus`). An own field implements them directly instead of rewriting input around `TextEdit`.
- `native/edit.rs`: the key mapping (chords to commands, the hex entry state machine, giving keys back for
  EV3's claim). Its event rewriting exists only because `TextEdit` reads the input; an own field reads the keys
  itself.
- `primary.rs`: the PRIMARY provider over `ClipboardBackend` (`install_backend`, `use_system`, `clear`).
- Tests: `crates/svelters-egui/tests/native_edit.rs` (`fx3_*`: chords, undo through keys, hex entry and Enter,
  line breaks, one middle-click paste, PRIMARY cleared, unfocused selection settles). They drive the `text-edit`
  kind through real input, so they should keep passing against the own field unchanged.
- Pins 4–6 (`conformance/pins/4-*`, `5-*`, `6-*`), the goldens, WG3b's reference numbers
  (`WG3-native-references.md`), and `egui_measure.rs` with its `FLOOR` ratchet (GTK 232, Qt 235 of 243).
  Keep feeding keys one event per kittest step (`Egui::press`): a step keeps only its last pass's output.
