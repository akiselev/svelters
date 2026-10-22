# WG3m: macOS native references (NA1 on AppKit)

Track: WIDGETS (`plans/research/TRACKS.md` §2.1), the macOS half of WG3's "Windows and macOS goldens" follow-up
(`plans/milestones/WG3-native-references.md`, which this extends; WG3w does Windows). User decisions used
(`plans/research/EGUI.md` §8): W1 (text fields follow each platform's native behaviour), W3 (svelters owns the
capture and publishes the goldens as files), W4 (measure egui `TextEdit` first).

## As built

| Piece | Where | What |
|---|---|---|
| macOS user path | `crates/svelters-conformance/src/macos.rs` (macOS only) | CGEvent input, AX reads, the fixture's run loop with an event barrier, AppKit's selection direction, the settings readers. |
| AppKit capture | `crates/svelters-conformance/tests/appkit_capture.rs` (`harness = false`, macOS only, opt-in) | Every pin on two `NSTextField`s + an `NSButton`, then on two `NSTextView`s + the button. |
| Goldens | `conformance/golden/macos/nstextfield-15.7.5.json`, `nstextview-15.7.5.json` | Same format as the Linux and Windows goldens; `staleness.rs` checks them unchanged. |
| egui on macOS | `tests/egui_measure.rs` `egui_text_edit_against_macos_goldens` | Every pin on egui `TextEdit` as egui-winit delivers macOS input, against both AppKit goldens. A report, not gated. |
| CI | `.github/workflows/macos-appkit.yml`, step "svelters-conformance (macOS)" | Builds and runs the macOS unit tests, staleness and the capture binary (which skips without `NA_MACOS=1`). |

### Pins without a macOS equivalent

All 18 scripts run, with WG3w's convention (`run_pin`): an intent the platform's `keys.json` table has no chord for
is a step without input, and its checkpoint records that nothing happened. On macOS that is `redo-alt`
(`undo-typing`), the CUA chords `copy-alt`/`cut-alt`/`paste-alt` (`clipboard-keys`) and `unicode-entry`. macOS
has no primary selection, so `primary` always reads `""`, as on Windows. The middle clicks of the `primary` pin are
real middle-button events (AppKit's text fields ignore them). The macOS chords come from `conformance/keys.json`
(Option+arrows for words, Cmd+arrows for line ends, Option+Delete for word deletion, Cmd+Z and Shift+Cmd+Z,
Option+E and Option+U dead keys).

### The fixture

A tiny AppKit app in the capture's own process (svelters' AppKit backend is not involved: the reference is
AppKit's behaviour): `NSApplication` with the standard Edit menu (Undo, Redo, Cut, Copy, Paste, Select All with
their usual key equivalents and first-responder actions, as Xcode's template has them: without it Cmd+Z/X/C/V/A
do nothing in a text field), and per pin a fresh titled window at the floating level (above other apps, so the
clicks land on it) holding fields `a` and `b` and the button `c` (`Done`, `NSBezelStyleRounded`), with
`autorecalculatesKeyViewLoop`. The `NSTextView` run uses bare `NSTextView`s with `richText = NO` and
`allowsUndo = YES` (Interface Builder's default; a text view made in code has no undo). Setup is programmatic:
`makeFirstResponder:` on the focused field, then `setSelectedRange:` on its editor (an `NSTextField` has a
selection only while it is edited, so an unfocused text field gets none; an `NSTextView` keeps its own).

### Input: CGEvent

- **Keys go to the capture's own process** (`CGEventPostToPid`) with explicit modifier flags, so a capture never
  types into another app. Typed characters use the US (ANSI) key codes; the golden records the input source.
- **Clicks and drags go through the session event tap** (`CGEventPost(kCGSessionEventTap)`): the window server
  routes them to the window under the pointer, as for a real mouse. Mouse events posted to a process skip that
  routing and AppKit drops them (they arrive with no window), and an event monitor cannot fix that because text
  views track drags in their own loop. Click counts are set explicitly (`kCGMouseEventClickState`) and every
  click or drag waits 0.7 s first (the double-click interval is 0.5 s), as the GTK and Qt captures do. Mouse
  events carry no modifier flags: without `CGEventSetFlags(e, 0)` one capture's clicks arrived with a Shift left
  from the previous pin's keys and extended the selection.
- **Barrier.** Every action ends with an F20 key event posted the same way as the action (to the process after
  keys, through the session tap after the mouse), and the run loop dispatches until it arrives and swallows it.
  So a checkpoint never reads a stale state: AX requests are served by the same run loop and otherwise answered
  before queued events had been handled (seen in the prototype). Within a multi-click or a double-click drag,
  each press and release gets its own barrier before the next press. On one capture under load, a held press
  inside an existing selection became `NSTextView`'s drag of the selected text, a modal loop.
- **Guards.** A capture fails, instead of recording something wrong, when the window is not key or the app not
  active at a checkpoint, a barrier does not arrive within 20 s, the pointer is not where the last mouse action
  put it, or a dispatched click carried modifiers.

### Observation: AX, cross-checked

At every checkpoint, from a helper thread while the main thread keeps dispatching (an AX request to the own
process is served by its main run loop): `AXValue`, `AXSelectedTextRange` (UTF-16, converted to scalars) and
`AXFocused` of `a` and `b` (found by `AXIdentifier`, the views' `identifier`), and `AXFocused` of the button.
Click points come from AX too: 30% into the character at `at` from `AXBoundsForRange` (1.8 pt past the last
character at the end), the button's centre from `AXPosition`/`AXSize`. The capture fails when AX and AppKit
disagree: `AXValue` against `stringValue`/`string`, the range against the focused editor's `selectedRange`,
`AXFocused` against the first responder (the field editor whose delegate is the field, the text view, the
button). Every one of the 382 field checkpoints per golden agreed. The golden's `observe` records the counts.

**Direction.** AX and the public API give a selection as a range: neither says which end is the caret.
AppKit keeps it in the private 2-bit `selectionExtensionAnchor` of `NSTextViewSharedData._sdFlags` (0 none yet,
1 the anchor is the end, 2 the anchor is the start). `macos::extension_anchor` reads it from the focused editor
(the bit position comes from the ivar's type encoding, never hard-coded); it is read, never written. A selection
recorded with 0 (made by the mouse, by select-all or by focus) is written `[start, end]`. The golden's
`observe.selection-direction` counts the three cases among non-empty selections. An unfocused `NSTextField`
reports `[0,0]` through AX: it has no selection of its own.

### Settings recorded

`macos` 15.7.5, `appkit` 2575.7, `text-kit` 1, `double-click-interval` 0.5, `keyboard-ui-mode` 0 (full keyboard
access off: Tab skips buttons), `input-source` `com.apple.keylayout.US`, the editor's text-checking flags
(spelling correction and text replacement on, smart insert/delete on; quote and dash substitution off for the
field editor, on for a text view) and `ax-role` (`AXTextField`, `AXTextArea`).

**TextKit 1, because of the deployment target.** A Rust binary declares macOS 10.12 as its minimum
(`LC_VERSION_MIN_MACOSX`, rustc's x86_64 default; the SDK is 26.2), and AppKit then gives text views and field
editors TextKit 1. A Swift prototype built with a 15.0 minimum got TextKit 2 for the same field editor. svelters'
AppKit backend and cadatomic are Rust binaries, so TextKit 1 is what they get today (CI builds with
`MACOSX_DEPLOYMENT_TARGET=11.0`, also below 12). A TextKit 2 reference needs a capture built with a deployment
target of 12 or later and its own golden name (follow-up).

### Running the capture (maintainer step, on a Mac nobody is using)

```sh
tools/dev/cargo-slot cargo test -p svelters-conformance --test appkit_capture --no-run
NA_MACOS=1 tools/dev/cargo-slot target/build/debug/deps/appkit_capture-<hash> --nocapture             # compare
NA_MACOS=1 NA_UPDATE=1 tools/dev/cargo-slot target/build/debug/deps/appkit_capture-<hash> --nocapture # rewrite
NA_MACOS=1 NA_MACOS_KIND=nstextview NA_PINS=multi-click …   # one kind, some pins, printed (writes nothing)
```

It needs Accessibility for the terminal that runs it (System Settings → Privacy & Security → Accessibility). It
moves the real pointer and shows its window at the top left of the main screen for about four minutes per kind.
Do not touch the keyboard or the mouse meanwhile: the guards fail the run. It saves the general pasteboard's text
and puts it back at the end (other pasteboard types are not kept). Without `NA_MACOS=1` it prints a skip note
and passes (CI).

## Captured (2026-10-22, macbook: Intel, macOS 15.7.5, AppKit 2575.7)

| Golden | Pins | Checkpoints | Deterministic |
|---|---|---|---|
| `conformance/golden/macos/nstextfield-15.7.5.json` | 18 | 243 | yes: a second capture reproduced the file byte for byte |
| `conformance/golden/macos/nstextview-15.7.5.json` | 18 | 243 | yes: a second capture reproduced the file byte for byte |

### Findings on the native side

NSTextField and NSTextView agree on 218 of 243 checkpoints: everything except focus and Tab (pin 7), Return, and
a click into the other field (pin 3). NSTextField agrees with GTK 4.24.1 on 118 and with Qt 6.12.0 on 126 (the steps
without input on macOS count where the Linux toolkits also did nothing).

1. **Word stops (pin 1).** Option+Right stops at word ends (like GTK, unlike Qt's word starts) and Option+Left at
   word starts. `.` and `_` between letters and digits do not split a word (`baz.qux`, `snake_case`, `3.14` are one
   word each, where GTK and Qt split them). `,` is not a word. CJK is split by dictionary (`日本語|テキスト`, as
   GTK), and `x😀y` is three stops. Option+Delete and Option+Forward Delete use the same stops.
2. **Selection (pin 2).** Shift+arrows extend from the caret and shrink back across the anchor as on Linux.
   Differences:
   - Shift+Option+Right across the anchor stops at the anchor (`[8,6]` → `[8,8]`), then goes on (`[8,11]`).
   - Shift+Cmd+Right and Shift+Cmd+Left grow the selection to the line end or start and keep the far end: from
     `[8,0]`, Shift+Cmd+Right gives `0..21` with the anchor at the end (`[21,0]`), and Shift+Cmd+Left leaves it.
   - After select-all the direction is undetermined. Shift+Left keeps everything selected and pins the anchor at
     the end (`[21,0]`), where GTK and Qt shrink to `[0,20]`. Shift+Option+Left does the same.
   - Option+Right on a selection moves from the selection's end to the next word end (`[9,6]` → `11`). GTK
     collapses to the end without moving (`9`), and Qt moves from the caret.
3. **Clicks (pin 3).** A single click puts the caret before the clicked character, a double click selects the word,
   and a triple click selects everything. A double click on `-` selects only `-` (like Qt; GTK selects `bar-`), and
   on a space only the space (`[3,4]`; GTK `foo `, Qt `foo`). A double-click drag extends by words. A click into
   the other field collapses an NSTextField's selection (it reports `[0,0]` unfocused), while an NSTextView keeps
   its own (each text view has its selection).
4. **Undo (pin 4).** One step per typing run, split at a deletion, a caret move and a paste (a paste is its own
   step). A run with spaces is one step, as on Qt (GTK steps per word). Undoing a word deletion selects the
   restored text with the anchor at its start (`[11,16]`; Qt `[16,11]`). Redo restores the caret, not the
   selection. NSTextField and NSTextView (with `allowsUndo`) behave the same.
5. **Paste (pin 5).** Multi-line paste keeps LF, CRLF and the tab verbatim in a single-line NSTextField, as GTK and
   Qt do. Cmd+C/X/V behave as Ctrl+C/X/V on Linux. A middle click does nothing at all (no focus, no paste).
6. **Dead keys (pin 6, US layout).** The composition is visible: Option+E shows the marked `´` in the field, and AX
   reports it in `AXValue` with the caret after it (GTK and Qt show nothing until it commits). Space commits `´`
   (the macOS spacing accent; X11's compose table gives `'`). A letter that does not compose commits the accent
   and the letter (`´x`; GTK `x́`, Qt `x`). Backspace removes only the marked accent.
7. **Return.** In an NSTextField, Return ends editing and selects all of the text (`[0,len]`; the field sends its
   action), so the next typed key replaces it. In an NSTextView it inserts a line break.
8. **Focus (pin 7).**
   - NSTextField: Tab and Shift+Tab alternate between the two fields and skip the button (full keyboard access is
     off by default). Keyboard focus into a field selects all of it (`[0,len]`), as on GTK and Qt. Clicking the
     button does not focus it (GTK and Qt focus it). Escape keeps focus.
   - NSTextView: Tab inserts a tab character and Shift+Tab does nothing, so focus never leaves the text view. That
     is a text view's native behaviour, not a field's (a multi-line editor in cadatomic would get it).

## W4: egui `TextEdit` against the macOS goldens

`egui_text_edit_against_macos_goldens`: svelters-egui `text-edit` (egui 0.37.0) under kittest with
`OperatingSystem::Mac`, as egui-winit delivers macOS input, with egui's own behaviour (`Native::Egui`, FX1's
default on macOS). Same counting as WG3: a checkpoint is equal when every anchor's text, selection and focus are
equal, and later checkpoints build on earlier ones.

| Pin | Script | Checkpoints | = NSTextField (first difference) | = NSTextView (first difference) |
|---|---|---|---|---|
| 1 | word-delete-back | 16 | 10 (#4 `delete-word-back`) | 10 (#4 `delete-word-back`) |
| 1 | word-delete-fwd | 16 | 5 (#3 `delete-word-fwd`) | 5 (#3 `delete-word-fwd`) |
| 1 | word-delete-mid | 7 | **7** | **7** |
| 1 | word-move | 33 | 16 (#3 `word-next`) | 16 (#3 `word-next`) |
| 2 | select-all | 11 | 9 (#2 `shift+left`) | 9 (#2 `shift+left`) |
| 2 | select-collapse | 17 | **17** | **17** |
| 2 | select-extend | 14 | 9 (#7 `shift+word-next`) | 9 (#7 `shift+word-next`) |
| 3 | multi-click | 14 | 6 (#3 `click a@7 x2`) | 6 (#3 `click a@7 x2`) |
| 4 | undo-edits | 21 | 10 (#10 `undo`) | 10 (#10 `undo`) |
| 4 | undo-paste | 10 | 6 (#4 `undo`) | 6 (#4 `undo`) |
| 4 | undo-typing | 11 | **11** | **11** |
| 5 | clipboard-keys | 20 | **20** | **20** |
| 5 | paste-multiline | 4 | 1 (#1 `paste`) | 1 (#1 `paste`) |
| 5 | primary | 8 | 2 (#2 `middle-click b@2 x1`) | 2 (#2 `middle-click b@2 x1`) |
| 6 | dead-keys [model] | 12 | 7 (#1 `dead-acute`) | 7 (#1 `dead-acute`) |
| 6 | unicode-entry | 13 | 6 (#6 `escape`) | 6 (#6 `escape`) |
| 7 | focus-click | 7 | 0 (#0 `setup`) | 2 (#2 `click c x1`) |
| 7 | focus-tab | 9 | 0 (#0 `setup`) | 1 (#1 `tab`) |
| | **total** | **243** | **142** | **145** |

The gap, per pin:

- **Pin 1, word rules.** egui's Option+arrow and Option+Delete work on macOS (EV3's key routing), but egui stops
  at `.` and `,` (`foo  bar,|` at 9 where AppKit goes on to `baz.qux|` at 17), so moves and deletions drift from the
  third step. The rule set AppKit needs is close to GTK's (word ends, dictionary CJK) with `.` and `_` inside
  words: `svelters_egui::native::words` already has a per-toolkit rule switch.
- **Pin 2, selection.** Plain extension, collapse with arrows and Option+Right on a selection match. Differences:
  Shift+Option+Right does not stop at the anchor, and egui has no undetermined direction after select-all
  (Shift+Left shrinks to `[0,20]`, like GTK and Qt).
- **Pin 3, clicks.** Single, triple and drag selection match. A double click on `-` or on a space selects the
  neighbouring word in egui (as on Linux).
- **Pin 4, undo.** egui's `Undoer` groups by time (one second of stability), so one undo returns to the setup
  text. AppKit splits at deletions, caret moves and pastes. This is the undo-point policy gap of WG3b.
- **Pin 5, clipboard.** Cmd+C/X/V and the steps without input match. A single-line `TextEdit` turns line breaks
  into spaces; AppKit keeps them (as GTK and Qt). A middle click focuses the field in egui and does nothing in
  AppKit.
- **Return and Escape (pin 6, `unicode-entry`).** The hex digits are typed literally on both. Escape drops keyboard
  focus in egui (AppKit keeps it), so the rest of the script types into nothing.
- **Pin 6, dead keys [model].** The committed text matches AppKit at every step. The differences are the visible
  marked text: macOS shows `´` while composing, and egui under winit gets it as an IME preedit, which the model
  does not show. Not measured on winit.
- **Pin 7, focus.** Against NSTextField: an unfocused text field has no selection in AppKit (`[0,0]`) where egui
  keeps it (`#0`), and Tab into a field does not select all in egui. Clicking the button takes keyboard focus
  from the field in egui with nothing focused afterwards; in AppKit the field keeps it (the button does not take
  focus). Against NSTextView the Tab rows differ by nature (a text view takes the Tab).

**Reading for W4 (numbers, not a decision).** On macOS, egui matches NSTextField on 142 of 243 checkpoints
(NSTextView 145).
Selection mechanics and click selection are close. The gaps are word rules (a rule set like GTK's with `.` and
`_` inside words), focus policy (Tab selects all, an unfocused field drops its selection, a click on a button
leaves keyboard focus in the field, Escape keeps focus, a middle click does nothing), undo points, and multi-line
paste. They are the same kinds of gaps WG3 found against GTK and Qt,
so an `AppKit` entry in FX1's policy table, rather than a different editing core, would close most of them.

## Not done / follow-ups

- **TextKit 2.** These goldens are TextKit 1 (see above). A capture built with `MACOSX_DEPLOYMENT_TARGET>=12`
  would record TextKit 2's behaviour under its own name. Word rules and undo are the likely differences
  [unverified].
- **macOS equivalents of the steps without input.** The macOS input methods (the press-and-hold accent menu, the
  Unicode Hex Input source) would give `unicode-entry` a macOS meaning. The other no-input steps (`redo-alt`, the
  CUA chords) have no macOS counterpart.
- **Full keyboard access.** With `AppleKeyboardUIMode` 2 (System Settings → Keyboard → Keyboard navigation), Tab
  reaches the button. The goldens record mode 0, the default. A second capture under mode 2 would pin that.
- **The `svelters_egui::native` policy for macOS.** FX1 keeps egui's behaviour on macOS. The table above is the
  input for a `Native::AppKit` policy (word rules, focus and undo grouping), as FX1 did for GTK and Qt. W4's
  owner decides.
- **Apple silicon.** Captured on an Intel Mac. AppKit's behaviour should not depend on the CPU, and the
  bitfield read assumes clang's low-bit-first allocation, true on both [unverified on arm64].
