# OW4: svelters' own `text-edit` on TC0 (`quire-text`)

Lane `ci/ow4` (macbook), from master d0542a3a, with master merged at 2cf3f4db (RA3) and ec8809fe (OW1b, OW3, OW5a, OW6, OW7, EV3b, CB1, VIS1c, OW-RUST, U0). Design:
`plans/research/ARCH-WIDGETS.md` §1.2 (TextInput), §2.4 (signals), §4 (text input on TC0), §6.1 (the
`text-edit` row) and the OW4 row of §7.2; parity rows `plans/research/WIDGET-PARITY.md` §2.4, §2.5; look from
DESIGN.md §1.3 (input box), R4, R5, R8, R9, R10; references: WG3/WG3b (GTK 4.24.1, Qt 6.12.0), WG3w (Win32
`EDIT`, WinUI `TextBox`), WG3m (NSTextField, NSTextView 15.7.5).

## What landed

### The dependency

- `crates/svelters-egui/Cargo.toml`: `quire-text = { git = "https://github.com/akiselev/quire", rev =
  "6f35566233c4ddbb62500eccf7c63c72ae95bced" }` (TC0 at quire master 6f35566; the repo is public, so CI fetches it
  over https with no key; the coordinator's update of 2026-10-22 replaced the ssh URL of the brief). No
  credentials anywhere. `THIRD_PARTY_NOTICES.md` names it (MIT OR Apache-2.0).
- `native/words.rs` (FX1) and `native/undo.rs` (FX3) are deleted: their code lives in quire-text
  (`words/native.rs`, `history/native.rs`, with provenance). Their only callers were the egui wrapper's FX1/FX3
  code, deleted with it. The `unicode-general-category` dependency went with `words.rs`.

### The TextInput unit (`crates/svelters-egui/src/interact/text_input.rs`)

- Owns the field's **session**: a `TextDocument` + one `TextView`, created lazily (on focus, a press on the field,
  an AccessKit request or EV3's Enter), kept for the node's life, so undo survives blur and refocus (§4.6). A
  `text` written from outside that differs from the session's is `replace_all(ResetHistory)` (FX3: `setText`
  starts a new history); our own echo is a no-op.
- Rules are cached per field and rebuilt only when the props or the policy change: `DocRules::field(lines,
  undo)` with `max_chars`, `read_only`; `EditRules::field(words)` with `word_move_collapses`, `secret`, `primary`,
  Grapheme backspace.
- **Keys** (`keymap`): Ctrl words (Linux, Windows) or Option words and Cmd line ends, Cmd+Up/Down text ends
  (macOS); Home/End; Backspace/Delete and their word forms; Ctrl+A/Z/Shift+Z/Y; the CUA chords (Ctrl+Insert,
  Shift+Insert, Shift+Delete) where the platform has them; GTK's Ctrl+Shift+U hex entry (digits in a preedit,
  Space/Enter commit, Escape cancels, BackSpace drops a digit; a Space that commits does not type); Enter (commit
  and submit; multi-line: a line break, Ctrl+Enter submits); Tab (a multi-line field with `tab-focus={false}`
  inserts it); every key while composing. Typed text comes from `Event::Text`, clipboard from
  `Event::Copy/Cut/Paste` (EV3's `text_command` feeds the same events through RA3's `before_text`/`after_text`).
- **Pointer**: own click counting at press time (`Policy::double_click_ms`, 6 pt slop): one press places the
  caret (Shift extends), two select the word (the platform's `word_at`), three the line (all of a one-line
  field); a drag after them extends by characters, words (GTK, AppKit, WinUI) or from the word's start by
  characters (Qt). A middle press pastes PRIMARY at the pointer where the platform has it: the integration's
  `MiddleClickPaste` (with the press, or a pass later under kittest), else the W5 provider on the release; with
  nothing to paste it focuses and leaves the caret (GTK). AppKit's middle click does nothing.
- **Focus**: keyboard focus into a one-line field selects all (Tab and Shift+Tab: `native::tab_moved_focus`;
  programmatic focus on GTK and AppKit); the unfocused selection is kept (GTK), collapsed to the caret (Qt,
  Windows) or cleared (NSTextField); blur after an edit commits; the undo group closes on focus changes.
  Escape keeps focus per policy (the field sets egui's focus-lock filter: arrows, Escape, Tab).
- **IME**: `ImeEvent::Preedit` (active range in chars → bytes), `Commit`, `DeleteSurrounding`; the preedit is
  shown at the caret and underlined; the caret rect goes to `PlatformOutput::ime` every focused frame.
- **AccessKit**: `SetValue`, `ReplaceSelectedText`; role `TextInput`/`MultilineTextInput`/`PasswordInput`;
  `read_only`; text runs, caret and selection through egui's `update_accesskit_for_text_widget`.
- `Unit::text_input` and `Unit::composing` (new, defaulted): the macro generates `units_text_input()` and
  `units_composing()` and the `KindDecl` hooks, so RA3's `Widget::text_input()` reports `Some(multiline)` for any
  kind composing `TextInput` without a hand-written override (the coordinator's ask); `ErasedWidget` combines
  them with the widget's own answer.

### The widget (`crates/svelters-egui/src/widgets/text_edit.rs`)

- `#[svelters::widget(replaces = "text-edit")]`, in `OWN_ONLY` and `DEFAULT_OWN`: the egui wrapper is gone
  (`leaf.rs` arm, `W::TextEdit`, its `layout.rs` size copy, prop routing, keys, `a11y`/`style`/`embed` arms, FX1's
  `text_keys`/`TextFocus`, CX3's `NodeFlags::EDITED` stand-in (bit 17) and its commit/submit block in `leaf.rs`).
  `focus::take_entered` stays: it is how EV3 tells a one-line field that the scope's default button took its
  Enter (the field still commits and submits).
- Row props and signals unchanged (`text`, `hint-text`, `multiline`, `password`, `char-limit`; `changed`,
  `gained-focus`, `lost-focus`, `commit`, `submit`): `metadata/semantics/egui.json`'s `bindings.text` (with
  `commit`) is byte-identical; `tests/bind_targets.rs` passes unchanged.
- Superset extras (spec, typings and reference regenerated): `placeholder` (alias of `hint-text`), `maxlength`
  (alias of `char-limit`), `read-only`, `echo` (`normal|password|none`), `undo` (`false` turns history off),
  `tab-focus`, `wrap` (`word|char|none`), `rows`, `selection-start`, `selection-end` (writable: a write selects),
  `modified`, `can-undo`, `can-redo`, `can-copy`, `inputmode`; events `rejected` (`max-chars`, `read-only`,
  `filtered`, `line-break`, `secret`) and `selection-changed`.
- Paint parts: `placeholder` and `selection` are the CSS pseudo-elements `::placeholder` and `::selection`
  (`style.rs` computes them so), `::part(caret)` (`background-color`, `width`), `::part(preedit)` (`color`).
- `Node::text` follows the user's edits (the replaced row's `text` mirror is refreshed after an own write, in
  `provider::paint`); the hint is the accessible name (`a11y.rs`); automation reads the field's value (not a
  password's) and role `textbox` by tag.
- Rust API for tests and drivers: `EguiBackend::text_selection(h)`, `set_text_selection(h, anchor, caret)`,
  `text_char_point(h, at, frac)` (char offsets; host-side Rust, not reachable by an extension).

### Platform policy (`crates/svelters-egui/src/native/text.rs`, `Policy::text`)

`TextPolicy` per row (GTK, Qt, Windows, Mac, and egui's by OS): undo rules, words, `paste_breaks`, `primary`,
`middle_click_paste`, `hex_entry`, `cua_keys`, `mac_keys`, `word_drag`, `unfocused`, `return_selects_all`,
`select_all_undirected`, `word_extend_stops_at_anchor`. `Policy::words` and `keep_selection_unfocused` are
replaced by it. `Native::for_os` now maps Windows to `Native::Windows` and macOS to `Native::Mac` (their rows
are pinned by WG3w and WG3m; Linux unchanged). Word boundaries quire-text leaves to the host are
`WordRules::Custom` statics: `GTK_WORDS`, `QT_WORDS` (Pango/Qt stops, measured double-click words) and
`MAC_WORDS`.

## Measured

`tools/dev/cargo-slot cargo nextest run -p svelters-conformance -E 'binary(egui_measure)'` (kittest, this lane's
last code). FX1's floors are raised to these numbers, and the Windows and macOS runs gain floors too.

| Run | Reference | Before OW4 (egui `TextEdit` + FX1/FX3s) | OW4 | Floor now |
|---|---|---|---|---|
| Linux, `Native::Gtk`, 18 scripts | GTK 4.24.1 | 193 / 243 | **235** / 243 | per pin (below) |
| Linux, `Native::Qt`, 18 scripts | Qt 6.11.3 (= 6.12.0) | 200 / 243 | **240** / 243 | per pin (below) |
| Windows input, `Native::Windows`, pins 1-3, 7 | Win32 `EDIT` / WinUI `TextBox` | 48 / 70 of 144 (egui's own) | **61 / 82** (85 either) | 61 / 82 (new) |
| macOS input, `Native::Mac`, 18 scripts | NSTextField / NSTextView 15.7.5 | 142 / 145 (egui's own) | **210 / 198** | 210 on NSTextField (new) |

FX3 (never merged) reached 232 / 235 with egui's field; OW4 passes it on both. Per pin (GTK / Qt), every pin at its
maximum except: `undo-edits` 20 / 21 (GTK's redo of the merged `delta` insertion selects its last char, #19, one
checkpoint, as FX3 left it: TC0's GTK model), `paste-multiline` 1 / 1 (the goldens keep `\r\n`; TC0's `Text` is LF
in memory and egui-winit turns CRLF into LF before a Ctrl+V paste anyway), `dead-keys` 8 / 12 (GTK's simple input
context gives letter + U+0301 for a letter that does not compose: the platform input layer, not the field).

macOS per pin (NSTextField): pins 1 (all four scripts), select-all, select-collapse, multi-click, undo-paste,
undo-typing, clipboard-keys and unicode-entry at their maximum. Gaps: select-extend 12/14 (Shift+Cmd+Right keeps
the far end, #12), undo-edits 12/21 (AppKit's undo model, below), paste-multiline 1/4 (CRLF, as Linux), primary
3/8 and focus-click 3/7 (a press outside the focused field takes egui's focus: AppKit keeps it in the field when
a button or a middle click is pressed), focus-tab 4/9 (Tab reaches the button: AppKit skips buttons with full
keyboard access off), dead-keys 7/12 (macOS shows the marked accent; the harness models only commits).

### Double-click words (closes TC0's open question 11)

TC0 derived `word_at` for Pango and Qt as "the word start at or before the click to that word's end; between
words, an empty range", unmeasured. Pin 3 (`multi-click`, text `foo bar-baz qux.quux 日本語 end`) measures it.
Double clicks, as char ranges:

| click at | GTK 4.24.1 | Qt 6.12.0 | NSTextField 15.7.5 | TC0's derived Pango | TC0's derived Qt |
|---|---|---|---|---|---|
| 5 (`bar`) | 4..7 | 4..7 | 4..7 | 4..7 | 4..7 |
| 7 (`-`) | **4..8** | **7..8** | 7..8 | 4..7 | 7..8 |
| 3 (space) | **0..4** | **0..3** | 3..4 | 0..3 | 0..4 |
| 14 (`qux.quux`) | 12..15 | 12..15 | 12..20 | 12..15 | 12..15 |
| 22 (`本`) | 21..24 | 21..24 | 21..24 | 21..24 | 21..24 |

So both derived rules miss two of five: GTK selects from the last word boundary (start or end) before the click
to the first after it (`gtk_text_select_word` with `allow_whitespace`), and Qt selects the `SkipWords` run before
the position after the click less its trailing white space (`selectWordAtPos`). svelters implements the measured
rules (`native::text::{gtk_word_at, qt_word_at, mac_word_at}`, unit test
`ow4_double_click_words_match_the_goldens`) as `WordRules::Custom`, so they do not depend on a quire-text change.
Recommendation to quire (cadatomic's register): take these two rules into `NativeWords::word_at` and drop the
"unmeasured" marks; pin 5's `primary source words` (double click at 9 → 8..14) agrees on both.

## Acceptance

| Item | Status |
|---|---|
| WG3 pins and goldens on all three OSes | Linux 235 / 240 of 243 (all 18 scripts, floors raised per pin); Windows 61 / 82 of 144; macOS 210 of 243 (floors added for both). Runs on kittest with each OS's input (`egui_measure.rs`), in linux-gtk CI. |
| WG3m's macOS gaps as `EditRules` by platform policy | `Native::Mac` is the default on macOS and its `TextPolicy`: word rules keep `.`, `_`, `'` inside words (`MAC_WORDS`; emoji are words of their own going forward), Option words / Cmd line ends / Cmd+Up/Down, Tab (and programmatic focus) selects all, an unfocused field has no selection, Escape keeps focus, Return selects all, multi-line paste keeps line breaks (`LineBreaks::Keep`), Shift+Option across the anchor stops at it, undirected select-all. Undo coalescing: Qt's model (split at caret moves and pastes, a run with spaces one step: undo-typing and undo-paste at 11/11 and 10/10); AppKit's own (typing and BackSpace in one step, restored selections anchored at their start) needs a quire-text rule set: requested below. |
| DESIGN R-rules for text fields | R4 (sunken: `input-bg` fill, `input-border-color` boundary, radius `input-radius`, inset `0 1px 1px`, I-beam, `focus` border on focus, `invalid-border` when invalid), R5 (read-only as text: no fill, border or inset; disabled keeps the shape at .4), R8 (fields: 1px `focus` at offset −1, keyboard-only through `:focus-visible`), R9 (disabled: .4, `not-allowed`, still hoverable, tooltip shown by `provider::paint`), R10 (`text` cursor). §1.3: 26px tall (test `ow4_field_metrics`). |
| Pango/Qt double-click `word_at` measured | Done (table above); svelters uses the measured rules; recommendation to quire recorded. |
| TC0's gate numbers | Untouched (no quire change; the dependency is by rev). |
| `commit`/`submit` exactly as `bind_targets.rs` expects, metadata unchanged | `bind_targets.rs` 3/3 unchanged; `bindings.text` in egui.json byte-identical; the CX3 stand-in (bit 17, leaf.rs block) deleted. |
| TextInput unit, `text_input()` reports `Some(multiline)` | `Unit::text_input` + macro hooks (no override in the widget); EV3 records the field as `Role::Text` and `text_command` reaches it (focus_keys `ev3_text_*` pass). |
| `native/words.rs`, `native/undo.rs` deleted | Done. |

**VIS1 checks this targets** (gate on `ci/vis1`, not merged; not run here): the input crops (empty with
placeholder, filled, focus, validation error): `h` 26, `r` 4, `b` 1 by construction; `bg`/`bc`/`fg`/placeholder from
the theme roles (`input-*`, `placeholder`); the focus border is `focus`. R4's inset is ours (VS Code has none):
VIS1c's baseline for the input `bg` near the top edge must use the affordance theme or record an exception, as
OW0 noted for the button's lip.

**Parity rows (WIDGET-PARITY §2.4, §2.5).** Landed: `text` (with CX3's `|commit`), `placeholder` alias, `maxlength`
alias, `read-only`, `echo` normal/password/none, `rejected` (`inputRejected`), `selection-start`/`selection-end`
(writable) and `selection-changed`, `modified`, `can-undo`/`can-redo`/`can-copy`, `changed` user-only vs
programmatic writes (no echo), `commit` (`editingFinished`), `submit` (`returnPressed`), focus signals,
`inputmode` (stored; `password` fields ask for a password IME), `::placeholder`, `::selection`, `::part(caret)`,
`::part(preedit)`, `:focus`/`:read-only`/`:disabled`/`:invalid`/`:user-invalid`/`:placeholder-shown`; multi-line
`wrap` (`word`, `none`; `char` wraps as `word` does), `tab-focus`, `undo={false}`, `rows`.

**Pins satisfied**: 1-7 (all 18 scripts) on Linux as above; WG3w pins 1-3, 7; WG3m pins 1-7.

## Not done (and where it goes)

- **quire-text requests** (for main-cadatomic's register, through the coordinator):
  1. `NativeWords::word_at` per the measured GTK and Qt rules (above); svelters does not need it (it has
     `Custom` rules) but quire's fields would.
  2. An AppKit undo rule set (WG3m pin 4: a typing run and the BackSpaces after it are one step; split at caret
     moves, word deletions and pastes; undoing a word deletion selects it anchored at its start; redo restores
     the caret). Until then macOS uses Qt's (undo-edits 12/21).
  3. CRLF kept verbatim in a single-line field with `LineBreaks::Keep` (paste-multiline): TC0's `Text` holds LF.
     Low value: egui-winit turns CRLF into LF for Ctrl+V before svelters sees it.
  4. GTK's redo selection after a merged insertion (undo-edits #19), FX3's open item, now TC0's GTK model.
- **Methods** (`select-all`, `set-selection`, `undo`, `redo`, `cut`, `copy`, `paste`, `insert`, `del`): the
  `KINDS` row may not change and RA3's provider methods do not route to replaced built-ins yet; the edit
  commands reach the field through EV3's `text_command` meanwhile. Needs RA3 routing for `replaces` kinds.
- **`bind:selection-start`/`selection-end`/`modified`** lowering: plain props now (a `#[prop(bind)]` extra would
  generate a `changed` binding carrying the text, wrong for these); needs per-prop bind events in the macro.
- **Multi-line scrolling**: the field scrolls itself (caret reveal, wheel) without the `Scrollable` unit's bars
  (`::part(scrollbar-y)`): OW4b or a follow-up on OW2's unit.
- **Session in an ext column**: the session lives in the widget struct (`#[state]`), so it does not survive a
  payload replacement (hot reload of the kind); §4.6's ext column is a follow-up.
- **macOS focus with buttons and middle clicks** (focus-click, focus-tab, primary on macOS): egui's memory drops
  the field's focus when anything else is pressed, and Tab reaches buttons; AppKit keeps focus in the field and
  skips buttons (keyboard-ui-mode 0). These are EV3/OW0 policy for buttons and the walk, not the field's.
- **AppKit selection details**: Shift+Cmd+Right/Left keep the far end (select-extend #12); Option+Right on a
  selection moves from its end.
- **Not done from the parity list**: the default context menu, `overwrite`, `--sv-password-char` (read when the
  engine exposes custom properties to `StyleView`; the mask is U+2022 meanwhile, which egui's fonts have, where
  GTK and Qt Fusion use U+25CF), AccessKit `SetTextSelection`, the single-line scroll jump (no WG3 pin yet),
  OW4b's `input-mask`, validators, completer seam, `clearable`, adornments, `highlights`.
- **Other kinds' text**: `table`'s cell editor is still egui's `TextEdit` (OW5); OW1b's drag-value entry (on
  `ci/ow1b`, not merged) keeps its own; both can move onto `TextInput` (it runs on any widget struct).
- **EV3 seam**: `Widget::composing` now reports an open hex entry or IME preedit (keys claimed), but EV3's
  pass-start routing of Enter/Escape to default/cancel buttons reads only IME events, so Escape that cancels a
  hex entry would also reach a cancel button in the same scope.
- `::part(placeholder)` and `::part(selection)` are spelled `::placeholder` and `::selection` only.
- The FX1/FX3 egui-era tests on `ci/fx3` (`native_edit.rs`) were not brought over; their behaviours are covered
  by `own_text_edit.rs` and the measurement.

## Tests

New: `crates/svelters-egui/tests/own_text_edit.rs` (9): `ow4_commit_submit_and_changed`,
`ow4_word_rules_per_platform` (GTK, Qt, AppKit), `ow4_undo_grouping_per_platform` (GTK per word, Qt per run, redo),
`ow4_gtk_hex_entry`, `ow4_line_breaks` (one-line paste keeps them; multi-line Enter, Up keeps the column,
Ctrl+Enter submits), `ow4_extras_and_accessibility` (read-only, maxlength + `rejected`, placeholder in AccessKit,
password copies nothing and is masked), `ow4_mac_focus_rules` (Tab selects all, no unfocused selection, Return
selects all, Escape keeps focus), `ow4_field_metrics` (26px), `ow4_middle_click_pastes_primary_into_another_field`;
unit tests `native::text::tests::{ow4_double_click_words_match_the_goldens, ow4_mac_word_stops}`; the floors in
`svelters-conformance/tests/egui_measure.rs` (Linux per pin, Windows and macOS totals).

Ported: `native_focus.rs` (selection through `EguiBackend::text_selection`), `viewports.rs`
(`dock_w4…keeps_caret…`: the caret through the same API), `egui_measure.rs` (selection set and read through the
API, click points from `text_char_point`, the Windows and macOS runs with `Native::Windows`/`Native::Mac`),
`native::tests::fx1_platform_table` (the new OS table).

Run locally (macbook), all passing unless noted: `own_text_edit`, `native_focus`, `bind_targets` (3, unchanged),
`focus_keys`, `clipboard`, `observe`, `own_widgets`, `spec`, `widgets`, `tier0`, the `native::` and `scene::` unit
tests, `svelters-automation` (all but `e2e`), `egui_measure` (3); `cargo clippy` on the changed crates (no warning in
the files this lane touched); `cargo check --all-targets` on every crate that depends on svelters-egui.

Not run locally: `viewports.rs` and other tests that need `pnpm build` fixtures (`fixtures/*/dist`), the TK1
snapshots (`svelters-testkit --features snapshots`: the own field's look changes them, see CI), the full
workspace suite, VIS1c's gate, conformance. CI runs them.

## Push and CI

- `ci/ow4` 1ad6c51c (merged with master ec8809fe): js 39891645956, linux-qt 39891645931, macos-appkit
  39891645953, windows-winui 39891645936 succeeded; linux-gtk 39891645932 failed on one test of 2910:
  `viewports::dock_w4_a_root_moved_between_viewports_keeps_caret_focus_and_instances` saw the caret blink's
  delayed repaint where the test wants the viewports idle. egui's own field blinks only while the viewport has
  input focus (and `visuals.text_cursor.blink`); the own field now does the same. Its `egui-snapshots` job passed
  (no TK1 or VIS1c snapshot changed).
- `ci/ow4` 4103eeef (the blink fix, merged with master f7f67386): js 39896158425, linux-qt 39896158587,
  macos-appkit 39896158386, windows-winui 39896158310 succeeded; linux-gtk 39896158389 had `dock_w4` passing and
  one failure, `eframe_idle::eframe_window_draws_then_idles_at_zero_cpu` (4 frames, 400 ms CPU in 2 s; it passed on
  1ad6c51c with the same field code and its comment records Xvfb's late-event bursts); the rerun of the job
  passed, so all five workflows are green on 4103eeef.
- Local disk: the macbook reached 0 free during the second merge's test build; this lane's `target/…/incremental`
  (9.5 GB) was deleted, leaving 9.8 GB (below the 15 GB rule), so nothing after that merge was built locally
  (regenerated files only: spec through the earlier build, `gen_reference.py`, `gen_utilities.py`).
- `focus::before_text` is now unused (it fed the deleted egui wrapper; EV3b's `take_command`/`feed` serve provider
  kinds): a dead-code warning, to delete in a follow-up.

## Handoff

**State.** Everything above compiles, is tested locally as listed and is pushed as `ci/ow4`. The own field is the
only `text-edit` (no egui wrapper, no switch entry to turn it off).

**Next steps, in order.**
1. TK1 snapshot cases with a text field (`counter-*` and others) change with the own look: take the new PNGs from
   CI's `egui-snapshots` artifact (lavapipe) and commit them; the same for VIS1c ratchet baselines if the input
   crops moved.
2. The quire-text requests (word_at rules, AppKit undo rules, verbatim CRLF, GTK redo selection) through the
   coordinator to main-cadatomic; then advance the rev and drop `GTK_WORDS`/`QT_WORDS` if quire takes the rules.
3. RA3 method routing for `replaces` kinds, then the edit methods; per-prop bind events for
   `selection-start`/`selection-end`/`modified`.
4. The table cell editor (OW5) and drag-value's entry (OW1b) on `TextInput`; multi-line bars on `Scrollable`.
5. macOS focus with buttons and middle clicks (EV3/OW0), then raise the macOS floor.

**Gotchas.**
- quire-text: positions are byte offsets, the harness and the props use chars (`Session::chars`,
  `set_selection_chars`). `TextView::set_selection` is view state (no undo step); caret moves that must split undo
  groups go through `execute(SetSelection)`. `replace_all` normalises CRLF, so the prop echo check compares with
  the normalised value. `NoWrap` is the layout for one-line fields; multi-line uses `GalleyRows` (goal = x in
  1/16 pt) only while the galley shows the document char for char.
- egui-winit (and kittest) delivers `MiddleClickPaste` in its own pass, after the press: the unit keeps the press's
  byte until it comes, or pastes from the provider on the release. `primary::provider()` is `None` off Linux; the
  measurement then reads PRIMARY from `TextSelectionSettled` output commands.
- `#[prop(bind)]` on an extra generates a `changed` binding whose payload is the text: do not use it for other
  props of this kind.
- CX3's stand-in is gone; `focus::take_entered` stays (EV3's seam for Enter taken by a default button).
- WG3m/EditRules decisions: macOS words = Pango plus `.`/`_`/`'` joins and forward-only emoji splits (both
  measured); undo = Qt's until an AppKit rule set exists; NSTextField (not NSTextView) is the single-line target.
- Double-click words: measured, not derived (table above); the GTK rule needs the whole boundary set of the line.
