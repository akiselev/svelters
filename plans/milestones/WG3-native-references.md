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
