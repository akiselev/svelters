# WG3: native-behaviour references (NA1)

Track: WIDGETS (`plans/research/TRACKS.md` §2.1, row WG3). Request: cadatomic's widget survey
(`~/projects/cadatomic/docs/research/2026-10-22-widget-survey-and-native-acceptance.md`) §4.1 and §5, NA1 and
NA5. User decisions used (`plans/research/EGUI.md` §8, 2026-10-22): W1 (text fields follow each platform's
native behaviour), W2 (GTK4 and Qt are both Linux references), W3 (svelters owns the capture and publishes the
goldens as files; cadatomic takes no dependency on the native backends), W4 (measure egui `TextEdit` first).

## As built

| Piece | Where | What |
|---|---|---|
| Pin scripts | `conformance/pins/*.json` | Survey pins 1 (word movement and deletion), 2 (selection extension, anchor, collapse, select-all), 3 (single/double/triple click, double-click-drag, plain drag, click into the other field) and 7 (Tab/Shift+Tab with wrap-around, focus by click). 10 scripts, 144 checkpoints (setup included). Format in `crates/svelters-conformance/src/pins.rs`. |
| Key-role table | `conformance/keys.json` | Platform-neutral intents (`word-next`, `delete-word-back`, `select-all`, `backtab`, ...) per platform (`linux`, `windows`, `macos`), as in survey §5.2. |
| Goldens | `conformance/golden/linux/gtk-<version>.json`, `qt-<version>.json` | One file per toolkit version: settings, observation evidence and, per pin, its fingerprint and one line per checkpoint with every anchor's state. |
| Harness | `crates/svelters-conformance` (new, dev/test only) | Pin parsing and fingerprints, the `Driver` trait and `run_pin`, the golden writer/reader and comparison, the private-display re-run, the XTEST injector. |
| GTK capture | `crates/svelters-conformance/tests/gtk_capture.rs` | `GtkEntry` ×2 + `GtkButton` in an undecorated `GtkWindow`, created through the GTK backend's C ABI. |
| Qt capture | `examples/counter-qt/tests/na_capture.rs` | `QLineEdit` ×2 + `QPushButton` in a `QMainWindow`, through the Qt backend's C ABI. |
| egui measurement | `crates/svelters-conformance/tests/egui_measure.rs` | The same pins on svelters-egui `text-edit` (egui 0.37 `TextEdit`) under kittest; writes the gap report. No egui source file is touched. |
| Staleness (NA5) | `crates/svelters-conformance/tests/staleness.rs` | Every golden holds every pin with today's fingerprint and checkpoint count. Runs anywhere. |

### The fixture and the script

Every pin runs on a fresh window holding text fields `a` and `b` and a button `c` (label `Done`), in that order.
`setup` gives each field its text and selection (`[anchor, caret]`, Unicode scalar offsets, negative = from the
end) and the focused anchor; setup is programmatic (GTK `grab_focus` + `select_region`, Qt `setFocus(Other)` +
`setSelection`, egui `request_focus` + `TextEditState`), the user path starts after it. Each step is one action
followed by one checkpoint (`repeat: n` runs the pair n times); checkpoint 0 is the state after setup. Clicks land
inside the character at `at`, left of its centre, so a single click puts the caret before it: GTK 30% into it
(`gtk_text_compute_cursor_extents`), egui 30% into it (its galley), Qt at the last x that
`QLineEdit::cursorPositionAt` maps to `at` (hook `svl_qt_na_point`). Before every click or drag the harness waits 0.7 s, so a sequence never joins the previous one.

A fingerprint (FNV-1a 64) covers the harness version (`HARNESS_VERSION`), the pin file and the chords its intents
resolve to on the platform. A golden whose fingerprint differs is stale and `staleness.rs` fails.

### Survey §4.1 gaps closed

1. **Caret and selection observation.** GTK: `text` through the backend; anchor and caret from the entry's inner
   `GtkText` (`sel = [selection-bound, cursor-position]`), because the `entry` element's own `cursor-position` and
   `selection-bound` (GtkEditable delegate forwarding) report the selection as (start, end) and lose its direction
   (finding 1 below); the capture still reads them through the backend at every checkpoint and records how often
   they agree. Qt: `QLineEdit` has no selection-start
   property, so the hook `svl_qt_na_state` (in `tools/metadata/qt/templates/backend.cpp.in`) reads
   `selectionStart/End` and `cursorPosition`, converts UTF-16 to scalars, and the capture cross-checks its caret
   against the backend's `cursor-position` at every checkpoint (it fails on a mismatch). egui: `TextEditState`
   char range (`[secondary, primary]`). Every capture also observes these props through ABI v2.6 `observe` and
   records, per prop, how many checkpoint-to-checkpoint changes the user path made and how many of them the backend
   announced (`observe` in the golden). See the findings for what that showed.
2. **A GTK user-path injector.** `crates/svelters-conformance/src/xtest.rs`: real X events through the XTEST
   extension (x11rb, pure Rust, already in the tree via winit) on a private Xvfb: keysym→keycode from the server's
   keymap, modifiers pressed and released around the key, pointer motion and button 1 in root coordinates (window
   found by title, focused with `SetInputFocus`; there is no window manager). Every event is followed by a round
   trip to the server and a pump of the toolkit loop. The same injector drives Qt (through its xcb plugin), so both
   Linux references see the same physical-path input; Qt's `sendEvent` synthesizers (`verify.cpp.in` `ut_key`)
   skip the platform plugin and are not used here.
3. WinUI user path: not in scope (Linux references only).
4. **Focus observation.** GTK: an entry's own `has-focus` stays false (GTK4 focuses its inner `GtkText`), so the
   capture reads the window's `focus-widget` through the backend and counts a node focused when that widget is the
   node or inside it, has focus, and the window is active. Qt: `QWidget::hasFocus` in an active window (hook).
   egui: `Memory::has_focus` for the text edits, the AccessKit node's focus for the button.
5. **The `state #h {…}` trace writer.** `crates/svelters-conformance/src/trace.rs` writes, per pin, the ops the
   capture made through the backend plus `state #h {"text","sel","caret","focus"}` per node at each checkpoint
   (`target/na/<platform>/<toolkit>-<version>/<pin>.oplog`). The line is an additive op-log extension, documented
   in `plans/architecture/backend-abi.md`. `tools/oplog-compare.mjs` treats each `state` line as a boundary and its
   object as node state; `--states` compares two traces by their `state` lines only (handles matched by first
   appearance), so the GTK, Qt and egui traces of a pin compare directly:
   `node tools/oplog-compare.mjs target/na/linux/gtk-4.24.1/word-move.oplog target/na/linux/egui-0.37.0/word-move.oplog --states`.

### Running a capture (maintainer step)

```sh
tools/dev/cargo-slot cargo nextest run -p svelters-conformance --test gtk_capture            # GTK
tools/dev/cargo-slot cargo nextest run -p counter-qt --test na_capture                       # Qt
NA_UPDATE=1 tools/dev/cargo-slot cargo nextest run -p svelters-conformance --test gtk_capture  # rewrite the golden
tools/dev/cargo-slot cargo nextest run -p svelters-conformance --test egui_measure --no-capture # W4 gap table
```

A capture re-runs its own test process under `tools/dev/headless` with `SVELTERS_HEADLESS_FORCE=1` (new), which
starts a private Xvfb even inside CI's `xvfb-run` or an outer headless call: XTEST input goes to the whole display,
so a capture never shares one with other tests. Without `NA_UPDATE=1` a capture compares itself byte for byte with
the checked-in golden of the same toolkit version (NA1's determinism criterion) and passes with a note when there
is none; it always writes `target/na/linux/<toolkit>-<version>.json` and the traces. CI uploads `target/na/` in the
`linux-gtk-artifacts` and `linux-qt-artifacts` artifacts, so a capture on CI's toolkit versions can be imported as
a golden (copy the JSON into `conformance/golden/linux/`), as `ledger.py import` does for macOS and Windows facts.

## Captured (2026-10-22, WSL PC, private Xvfb)

| Golden | Toolkit | Settings recorded | Deterministic |
|---|---|---|---|
| `conformance/golden/linux/gtk-4.24.1.json` | GTK 4.24.1 (Adwaita, cairo renderer) | double-click 400 ms / 5 px, drag threshold 8 px, `gtk-entry-select-on-focus` true | yes: a second capture reproduced the file byte for byte |
| `conformance/golden/linux/qt-6.12.0.json` | Qt 6.12.0 (xcb, Fusion) | double-click 400 ms / 5 px, start-drag distance 10 px | yes: a second capture reproduced the file byte for byte |

CI's toolkits differ (linux-gtk: Arch's current GTK; linux-qt: Qt 6.11.\*); there the capture runs, compares
when a golden of that exact version exists, and uploads `target/na/` either way.

### Findings on the native side

1. **GtkEntry's `cursor-position`/`selection-bound` lose the selection direction.** Read through the backend, the
   entry element reports (start, end) of a selection instead of (caret, anchor): 243 of 288 field checkpoints agree
   with the inner `GtkText`; the disagreements read like (start, end) of a selection whose caret is not its end. A Svelte `bind:` on those entry props cannot tell a backward selection from a forward one. The
   observe evidence agrees: `selection-bound` changed 104 times between checkpoints and was announced 66 times,
   `cursor-position` 82 and 79 (`text` 33/33, the window's `focus-widget` 14/14). The R0 facts call both props
   `exact` (`metadata/observe/gtk.json`, `Gtk:Editable`); on `GtkEntry` they are not. Recommendation for the
   coordinator (not done here; it is a facts change): mark `Gtk:Entry` `cursor-position`/`selection-bound` as
   `partial` with a `missed: ["direction"]` note, or forward them from the `GtkText` delegate in the GTK backend.
2. **Qt delivers every change it makes**: `cursor-position` 103/103, `selectedText` 69/69, `focus` 28/28, `text`
   33/33 announced (ABI v2.6 observe, on the XTEST user path). The `exact` facts hold.
3. **GTK and Qt disagree with each other on most text pins** (W1 makes both "native" on Linux):
   - word stops: GTK (Pango) stops at word ends and splits `snake_case` at `_` and `日本語|テキスト` by
     dictionary; Qt (ICU-style boundary finder) stops at word starts, Windows-like, and treats `3.14` as three
     words;
   - selection after focus leaves a field: GTK collapses it to `[0,0]`, Qt to its end;
   - keyboard focus into a field (Tab, Shift+Tab) selects all on both, as `[0,len]`;
   - word-next with a selection: GTK collapses to the selection end without moving, Qt moves from the caret;
   - double-click on `-`: GTK selects `bar-`, Qt only `-`;
   - a single click into the other field: Qt collapses the left field's selection to its caret at once; GTK keeps
     it until the other field takes the X11 PRIMARY selection (a double-click there), then collapses it to its
     start. This bears on W5 (primary selection).

## W4: egui `TextEdit` against the goldens

`crates/svelters-conformance/tests/egui_measure.rs`, svelters-egui `text-edit` (egui 0.37.0) under kittest as
egui-winit's X11 path would feed it (`OperatingSystem::Nix`). A checkpoint counts as equal when every anchor's
text, selection `[anchor, caret]` and focus are equal. Later checkpoints of a pin build on earlier ones, so the
first differing step matters more than the count.

| Pin | Script | Checkpoints | = GTK 4.24.1 (first difference) | = Qt 6.12.0 (first difference) | = either |
|---|---|---|---|---|---|
| 1 | word-move | 33 | 14 (#3 `word-next`) | 13 (#1 `word-next`) | 20 |
| 1 | word-delete-back | 16 | 5 (#4 `delete-word-back`) | 8 (#2 `delete-word-back`) | 10 |
| 1 | word-delete-fwd | 16 | 8 (#3 `delete-word-fwd`) | 4 (#1 `delete-word-fwd`) | 9 |
| 1 | word-delete-mid | 7 | **7** | 1 (#1 `delete-word-fwd`) | 7 |
| 2 | select-extend | 14 | **14** | 12 (#7 `shift+word-next`) | 14 |
| 2 | select-collapse | 17 | 14 (#10 `word-next`) | 14 (#10 `word-next`) | 14 |
| 2 | select-all | 11 | **11** | **11** | 11 |
| 3 | multi-click | 14 | 9 (#3 `click a@7 x2`) | 9 (#3 `click a@7 x2`) | 11 |
| 7 | focus-tab | 9 | 1 (#1 `tab`) | 1 (#1 `tab`) | 1 |
| 7 | focus-click | 7 | 2 (#2 `click c x1`) | 2 (#2 `click c x1`) | 2 |
| | **total** | **144** | **85** | **75** | **99** |

The gap, per pin:

- **Pin 1, word rules.** egui's word stops are GTK-like (word ends, not Qt's word starts) and match GTK for plain
  words, spaces and `3.14`, but egui stops after `,` (punctuation counts as a word), keeps `snake_case` and
  `日本語テキスト` as one word, and runs the deletion rules from the same stops, so deletions drift from GTK after
  the third step. Moving to GTK's rules means a Unicode word-boundary (UAX #29) segmenter with GTK's
  punctuation and dictionary handling; matching Qt as well needs a per-toolkit rule switch. Size: a word-rule
  module, not a new editing core.
- **Pin 2, selection.** Anchor/caret direction, extension across the anchor, Home/End, select-all and collapse
  with arrows all match both toolkits. The one gap is word-next with a selection (#10): egui moves from the caret
  by a word (`11`), GTK collapses to the selection end (`9`), Qt moves from the caret to the next word start
  (`12`).
- **Pin 3, clicks.** Single, double (word), triple (all), double-click-drag by words and plain drag match GTK.
  Gaps: double-click on punctuation (`-`) selects the neighbouring word in egui; double-click on a space selects
  `foo bar` (`[0,7]`) where GTK selects `foo ` and Qt `foo`; on a click into the other field egui collapses the
  left field's selection to its caret, like Qt (GTK keeps it until PRIMARY moves). One step (#10, a single click after the CJK run) lands one character
  early in egui; the click point comes from egui's galley with tofu glyphs (egui's bundled fonts have no CJK), so
  that one is not attributed to `TextEdit` [unverified].
- **Pin 7, focus.** The largest gap, and not in the editing core: (a) Tab into a field does not select its text
  (both natives select all, `[0,len]`); (b) clicking the button does not move keyboard focus to it (both natives
  focus it), so the following Tab starts from the field; (c) Escape in a field drops keyboard focus (both natives
  keep it). All three are focus policy in svelters-egui's `text-edit` and
  `button` kinds, or egui `Memory` options, not `TextEdit` internals.

**Reading for W4.** Selection mechanics (pin 2) and click selection (pin 3) are at parity with GTK; the gaps are
word rules (pin 1: a boundary module) and focus policy (pin 7: kind-level behaviour). None of them needs a
different editing core. On this evidence (pins 1–3 and 7; undo grouping, clipboard and IME, pins 4–6, are not
measured yet) the recommendation is to keep egui `TextEdit` and close the gaps around it: a platform word-rule
module and the focus policies above. The decision is W4's owner's.

## Not done / follow-ups

- Pins 4–6 (undo grouping, clipboard and primary selection, IME) and 8–10 (menus, scrolling, pointer thresholds):
  the format and harness take them; their scripts and observables (undo stack, clipboard, preedit) are not
  written.
- Windows (WinUI user path through UIA or `SendInput`, survey gap 3) and macOS goldens: `keys.json` has their
  key tables; capture drivers for those backends are not written.
- The GtkEntry direction finding (finding 1) is reported, not fixed; it needs a coordinator decision (facts vs
  backend).
- Qt 6.11 (CI's version) has no golden yet; importing CI's capture from the `linux-qt-artifacts` artifact makes
  CI check Qt determinism too.

(WG3b below closes the first, third and fourth items for pins 4–6; pins 8–10, Windows and macOS remain.)

(macOS: `plans/milestones/WG3m-macos-references.md`, AppKit goldens for the 14 pins that apply there and egui
measured against them.)

## WG3b (2026-10-22): GtkEntry selection fidelity, pins 4–6, Qt 6.11 golden

### GtkEntry `cursor-position`/`selection-bound` (coordinator decision: fix the backend)

`crates/svelters-gtk/src/backend.rs` `editable_delegate`: GtkEditable `cursor-position` and `selection-bound`
of an editable with a delegate (GtkEntry, GtkSpinButton, GtkPasswordEntry, GtkSearchEntry, GtkEditableLabel) are
read and observed on the delegate, the inner GtkText. The wrapper's own values come from
`gtk_editable_delegate_get_property`, which reports a selection as (start, end). GtkText holds the real caret and
anchor and notifies both. Observation connects `notify::<prop>` on the delegate (`Target::Delegate` in
`backend/observe.rs`; the delegate is fixed for the widget's lifetime, so nothing re-targets). GtkText itself has
no delegate and reads its own; both props are read-only, so writes are unchanged. The facts
(`metadata/observe/gtk.json`) are unchanged and stay `exact`. The one manual fact that encoded the old reading,
`Gtk:Editable:method:select_region`, now expects `select_region(1, 3)` to give caret 3 and anchor 1
(`metadata/overrides/gtk/Editable.yaml`; `generated/verify.rs` and `facts.json` regenerated with
`gen_backend.py`).

`metadata/gtk/Editable.json` (the merged record) was updated to match the override field for field. `ingest.py`
needs PyGObject, which is not on the WSL PC, and it would re-ingest against GTK 4.24.1 instead of the metadata's
4.22.4. The ledger entry was re-verified with `ledger.py`'s own `run_gtk`, limited to the changed fact plus the
`observe_gtk_editable` table (9 tests, all pass). `ledger.py check --toolkit gtk` passes.

The capture now reads caret and anchor through the backend and fails if they differ from the GtkText's. On GTK
4.24.1:

| Evidence | WG3 (10 pins) | WG3b (18 pins) |
|---|---|---|
| entry props equal to the GtkText's | 243 / 288 checkpoints | **486 / 486** |
| `selection-bound` changes announced | 66 / 104 | **130 / 130** |
| `cursor-position` changes announced | 79 / 82 | **181 / 181** |
| `text`, window `focus-widget` | 33/33, 14/14 | 83/83, 17/17 |

The checkpoint lines of pins 1–3 and 7 in `gtk-4.24.1.json` are unchanged (they were already read from the
GtkText).

### Harness additions

- Pin format (`pins.rs`): `{"type": text}` types one key per character. `"button": 2` on a click is the middle
  button (label `middle-click …`). `setup.clipboard` puts text on CLIPBOARD, owned by another client.
  `observe: ["clipboard", "primary"]` adds those pseudo-anchors to every checkpoint as `{"text":…}` (in traces,
  `state #18446744073709551614` and `…613`). Existing pins' labels and lines are unchanged, so `HARNESS_VERSION`
  stays `na1-v1`.
- Key table: `undo`, `redo` (Linux Ctrl+Shift+Z), `redo-alt` (Linux Ctrl+Y), `copy`/`cut`/`paste`, the CUA chords
  `copy-alt` (Ctrl+Insert), `cut-alt` (Shift+Delete), `paste-alt` (Shift+Insert), `dead-acute`, `dead-diaeresis`,
  `unicode-entry` (Ctrl+Shift+U, Linux only), `space`, `return`. Windows and macOS have their own chords (see the
  table's note).
- XTEST injector (`xtest.rs`): typed characters come from the server keymap (Shift for the second column). Dead
  keys are bound to unused keycodes because the Xvfb keymap has none; toolkits reload their keymap on the
  MappingNotify. CLIPBOARD and PRIMARY are read with `ConvertSelection` (UTF8_STRING) from the capture's own
  connection while the toolkit loop is pumped, and both are cleared at the start of every pin. The clipboard seed
  is served by a thread with its own X connection: Qt's xcb clipboard blocks its thread while it reads.
- Input methods are fixed for captures: `GTK_IM_MODULE=gtk-im-context-simple`, `QT_IM_MODULE=compose`,
  `XMODIFIERS` unset. These are the toolkits' built-in methods with no IM server; each golden records its setting.
  A real IBus or Fcitx under Xvfb (survey NA4) is not covered.

### Pins 4–6

| Pin | Script | Checkpoints | What it pins |
|---|---|---|---|
| 4 | undo-typing | 11 | one typing run with spaces, undo to empty, redo with both chords |
| 4 | undo-edits | 21 | grouping across a word deletion, typing, backspaces, a caret move, typing over select-all |
| 4 | undo-paste | 10 | a paste between two typing runs (clipboard seeded) |
| 5 | clipboard-keys | 20 | Ctrl+C/X/V and the CUA chords, with the clipboard after each step |
| 5 | paste-multiline | 4 | LF, CRLF and a tab pasted into a single-line field |
| 5 | primary | 8 | mouse and keyboard selection as PRIMARY, middle-click paste at the pointer, CLIPBOARD untouched |
| 6 | dead-keys | 12 | dead acute/diaeresis + letter, + space, + a letter that does not compose, + BackSpace |
| 6 | unicode-entry | 13 | Ctrl+Shift+U hex entry: text during the preedit, commit with space and Enter, cancel with Escape, BackSpace |

The preedit itself is not observed (GTK has no public read of it). The checkpoints show the committed text, which
stays unchanged while a composition is open.

Both goldens were re-captured with all 18 scripts (243 checkpoints). Each reproduced byte for byte on a second
capture (WSL PC, private Xvfb).

Native findings (W1 counts both toolkits as native):

1. **Undo grouping differs between the toolkits.** GTK makes one step per word of a typing run (`foo bar baz`
   undoes as `foo bar`, `foo`, `""`). Qt makes the whole run one step. Both split steps at deletions, caret moves
   and pastes, and a paste is its own step. undo-edits takes 7 undos to reach the setup text on GTK and 4 on Qt.
   Ctrl+Y redoes on both. GTK restores a replaced select-all with its direction reversed (`[15,0]`).
2. **Clipboard:** Ctrl+C/X/V and the CUA chords (Ctrl+Insert, Shift+Insert, Shift+Delete) behave the same on
   both. **Multi-line paste keeps LF, CRLF and the tab verbatim** in a single-line GtkEntry and QLineEdit.
3. **PRIMARY:** both put a mouse selection (double-click, drag) and a keyboard selection (select-all) into
   PRIMARY. Middle-click pastes it at the pointer and focuses the field; CLIPBOARD is untouched. When the selection
   collapses (`line-end`), GTK drops PRIMARY (`""`) and Qt keeps the old text. After a middle-click into the other
   field, GTK keeps the first field's selection and Qt collapses it (as in pin 3).
4. **Dead keys:** a composable letter and space agree (`é`, `ü`, `'`). On a letter that does not compose, GTK's
   simple context gives the letter plus U+0301 (`x́`) and Qt's compose plugin drops the accent (`x`). Dead key +
   BackSpace: GTK cancels the composition only; Qt cancels it and deletes a character.
5. **Ctrl+Shift+U is GTK-only** (without an IM server). GTK commits `日` on space and `B` on Enter, and Escape
   cancels with focus kept. Qt types the hex digits literally.
6. **Qt observe evidence:** `selectedText` was announced 93 times for 96 changes. Pins 1–3 and 7 alone gave
   69/69, so the 3 missed changes come from pins 4–6, likely undo/redo restoring a selection [unverified which
   checkpoints]. The R0 fact calls `QLineEdit:selectedText` exact; this is reported, not fixed.
   `cursor-position` 167/167, `text` 82/82 and `focus` 34/34 were all announced.

### Qt 6.11.3 golden (CI)

`conformance/golden/linux/qt-6.11.3.json` was imported unchanged from the `linux-qt-artifacts` artifact of run
39807534068 (`ci/wg3b` at 904d223b, aqt Qt 6.11.3, ubuntu-24.04). Its fingerprints are current, and all 243 of its
checkpoints equal the local Qt 6.12.0 golden: no behaviour difference between the two Qt versions on any pin.
Its observe evidence (`selectedText` 93/96, everything else complete) and its settings (Fusion, xcb, 400 ms / 5 px
double-click, 10 px drag, `QT_IM_MODULE=compose`) are also identical. From now on, CI's Qt capture compares
itself byte for byte with this file, so CI checks Qt determinism. When aqt resolves `6.11.*` to a newer patch
release, the capture finds no golden, passes with a note, and the new file can be imported the same way.

### W4: egui `TextEdit` on pins 4–6

Same method as for pins 1–3 and 7 (`egui_measure.rs`). Input follows egui-winit 0.37 on Linux, as described in the
test's header: Ctrl+X/C/V become `Cut`/`Copy`/`Paste`, output commands set the clipboard and PRIMARY, and a middle
press sends `MiddleClickPaste`. The dead-key rows are a model of winit's X11 compose path, not a measurement.

| Pin | Script | Checkpoints | = GTK 4.24.1 (first difference) | = Qt 6.12.0 (first difference) | = either |
|---|---|---|---|---|---|
| 4 | undo-typing | 11 | 6 (#2 `undo`) | **11** | 11 |
| 4 | undo-edits | 21 | 12 (#10 `undo`) | 10 (#10 `undo`) | 12 |
| 4 | undo-paste | 10 | 6 (#4 `undo`) | 6 (#4 `undo`) | 6 |
| 5 | clipboard-keys | 20 | 12 (#12 `copy-alt`) | 12 (#12 `copy-alt`) | 12 |
| 5 | paste-multiline | 4 | 1 (#1 `paste`) | 1 (#1 `paste`) | 1 |
| 5 | primary | 8 | 2 (#2 `middle-click b@2 x1`) | 5 (#5 `select-all`) | 5 |
| 6 | dead-keys [model] | 12 | 8 (#8 `type "x"`) | **12** | 12 |
| 6 | unicode-entry | 13 | 1 (#1 `unicode-entry`) | 1 (#1 `unicode-entry`) | 1 |
| | **pins 4–6** | **99** | **48** | **58** | **60** |
| | pins 1–3, 7 (WG3, unchanged) | 144 | 85 | 75 | 99 |
| | **all 18 scripts** | **243** | **133** | **133** | **159** |

The gap, per pin:

- **Pin 4, undo.** egui's `Undoer` groups by time, not by edit kind: it saves an undo point once the text has been
  stable for `stable_time` (1 s, the egui 0.37 default). A run typed without a pause is one step, which matches
  Qt and differs from GTK's per-word steps. Everything else in undo-edits and undo-paste also happened within one
  second, so one undo returns to the setup text and one redo restores everything. Both toolkits split at
  deletions, caret moves and pastes. This is undo-point policy, which svelters-egui could set per edit kind
  through `TextEditState`'s undoer; it is not the editing core.
- **Pin 5, clipboard.** Ctrl+C/X/V match both toolkits. Gaps:
  - egui-winit treats the CUA chords as clipboard commands only on Windows. On Linux, Ctrl+Insert and
    Shift+Insert do nothing, and Shift+Delete deletes without copying. This is input-layer key mapping.
  - A single-line `TextEdit` turns line breaks into spaces; both toolkits keep them. This is a policy choice, and
    egui's is arguably the better one.
  - egui 0.37 has PRIMARY (`TextSelectionSettled`, `MiddleClickPaste`), and mouse selection and middle-click
    paste match Qt. A keyboard select-all does not become PRIMARY, though it does on both toolkits (W5).
- **Pin 6, IME.** Dead-key composition happens in the platform input layer (XIM or xkbcommon under winit), not in
  `TextEdit`. With the model following the X Compose table, egui equals Qt's compose plugin; GTK differs on a
  letter that does not compose and on BackSpace. For Ctrl+Shift+U, egui has no hex entry (GTK has one built in,
  Qt does not). It also hits `TextEdit`'s Ctrl+U binding (delete to line start), which ignores Shift. Then Escape
  drops focus (pin 7's policy gap), so the rest of the script types into nothing. Preedit handling (`Event::Ime`)
  needs an IM server or recorded input traces (survey §4.4, NA4) and is not measured.

**Reading for W4 (numbers only, not a decision).** On pins 4–6, egui matches Qt on 58 of 99 checkpoints and GTK on
48. Across all 18 scripts it matches each toolkit on 133 of 243 checkpoints and either one on 159. The largest
new gaps are undo-point policy (pin 4), the Linux CUA chords and keyboard PRIMARY (pin 5), and the Ctrl+U binding
plus Escape focus (pin 6). Each sits in svelters-egui's kind code, egui-winit's key mapping or `TextEdit` options,
not in the editing core. W4's owner decides.
