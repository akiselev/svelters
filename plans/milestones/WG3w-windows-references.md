# WG3w: Windows native-behaviour references

Track: WIDGETS. Follows WG3 (`plans/milestones/WG3-native-references.md`) and WG3b (pins 4–6), which captured the
Linux references and listed "WinUI and macOS drivers" as not written. User decisions used (`plans/research/EGUI.md`
§8): W1 (text fields follow each platform's native behaviour), W3 (svelters owns the capture and publishes the
goldens as files), W4 (measure egui `TextEdit` first). Everything here ran on the WSL PC's Windows host
(Windows 10 Pro 22H2, build 19045.7807, 120 dpi, US keyboard layout).

## As built

| Piece | Where | What |
|---|---|---|
| Windows helper | `tools/dev/win` | Runs a PowerShell command line in a git.exe clone on the Windows filesystem (`C:\Users\dev\projects\svelters-win`, `SVELTERS_WIN_CLONE`). `sync` fetches the current branch from this checkout's git directory and fast-forwards the clone (`--autostash`: cargo.exe rewrites `Cargo.lock` there). Cargo builds and tests take a build slot through `tools/dev/cargo-slot`, with the slot's job count passed to cargo.exe; `cargo check` takes none. One line in `CLAUDE.md` ("Machines and sessions"). |
| Windows driver | `crates/svelters-conformance/src/win.rs` | `UiaDriver<F: WinFixture>`: one `Driver` for any fixture that UI Automation can see (two `Edit` elements, then a `Button` named `Done`). It injects input with `SendInput` and reads state through UI Automation. A `WinFixture` opens and closes the window for each pin. |
| Win32 capture | `crates/svelters-conformance/tests/win32_capture.rs` | Two common-controls v6 `EDIT`s and a `BUTTON` in a dialog-class window (`DLGWINDOWEXTRA` + `DefDlgProcW`, `IsDialogMessageW` in the loop, so the dialog manager handles Tab, Shift+Tab and focus restore). The window runs on its own thread, because a UIA client must not share the UI thread. `build.rs` embeds a Common-Controls 6.0 manifest dependency into the test binaries (MSVC only). |
| WinUI capture | `examples/counter-win/tests/na_capture.rs`, `counter-win --na-fixture <title>` | `Window > StackPanel > TextBox, TextBox, Button("Done")`, built through the svelters-winui backend's ABI (no JavaScript, no bundle). One process per pin; it exits when its stdin closes. |
| egui on Windows | `crates/svelters-conformance/tests/egui_measure.rs`, `egui_text_edit_against_windows_goldens` | The WG3 kittest driver with `OperatingSystem::Windows` (egui's own behaviour, `Native::Egui`), compared against the Windows goldens for pins 1–3 and 7. It writes `target/na/windows/egui-gap.md`. |
| Goldens | `conformance/golden/windows/win32-edit-6.10.19041.7802.json`, `winui-3.2.3.0.json` | All 18 pins, 1–7 (WG3's 10 plus WG3b's 8). The version in each file name is comctl32.dll's for Win32 and Microsoft.UI.Xaml.dll's for WinUI. `settings` records the OS build, the WinAppSDK package (`Microsoft.WindowsAppRuntime.2_2.5.1.0`), the double-click time and rectangle, the drag rectangle, the dpi and the keyboard layout. |

### How the driver works

- **Setup** is programmatic, through UIA. `ValuePattern.SetValue` sets the text. `TextRange.Select` sets each unfocused
  field's selection (every setup selection is forward or collapsed). Then `SetFocus`, then the focused field's
  selection. Before the user path starts, the clipboard is emptied and seeded with `setup.clipboard`, owned by the
  capture process.
- **Keys** go as one `SendInput` batch per chord, with virtual-key and scan codes. Navigation keys carry the
  extended flag; without it, Shift+arrow reads as the numeric pad. A `type` step sends each character as the key
  that makes it on the fixture's layout (`VkKeyScanExW`, so dead keys compose with it). A character with no key on
  the layout goes as a Unicode packet.
- **Pointer** moves are absolute `SendInput` moves, corrected until `GetCursorPos` is exactly on the target point.
  `SetCursorPos` is not used: it posts `WM_MOUSEMOVE` but no pointer input, so WinUI never saw the drags
  (the first WinUI capture had every drag as a no-op; Win32 was unaffected). A click lands 30% into the character,
  using its UIA bounding rectangle. Before each click or drag the driver waits for the double-click time plus
  200 ms (at least 700 ms). Button 2 is the middle button.
- **State** comes from UIA. The text is read from the document range. The selection comes from
  `TextPattern.GetSelection`, with offsets measured as the UTF-16 length of the text before each endpoint and then
  converted to scalars. The selection's direction:
  - WinUI: `TextPattern2.GetCaretRange`.
  - Win32 `EDIT`: the system caret (`GetGUIThreadInfo`, compared with `EM_POSFROMCHAR` of the selection start).
    **Finding:** the EDIT UIA proxy's `GetCaretRange` always reports the selection end, so after Shift+Left it
    gives the wrong direction.
  - An unfocused EDIT has no caret, so its direction is not observable and the selection is recorded forward.
  `observe.direction` in each golden counts where every reading's direction came from: WinUI 66 from the caret
  range, 0 unknown; Win32 51 from the system caret, 13 unknown (all of them after the dialog manager's
  select-all, which is forward anyway).
- **Focus** comes from `GetFocusedElement`, counted only while the fixture window is in front. A focus-changed
  event handler records what UIA announced. `observe.focus` was 17/17 changes announced on WinUI and 14/14 on
  Win32 (the WG3 pins).
- **Clipboard** reads `CF_UNICODETEXT`. Windows has no PRIMARY selection, so the `primary` pseudo-anchor is always `""`.
- **Settling:** after each action the driver reads until two readings 50 ms apart agree (at most 2 s).
- **Intents without a chord.** `run_pin` treats an intent the platform has no chord for (`unicode-entry` off
  Linux) as a step with no input, so its checkpoint records that nothing happened. This is new in
  `crates/svelters-conformance/src/lib.rs`, and Linux has a chord for every intent. Reconcile with WG3b if a
  different rule is wanted.
- **Dead keys.** These need the US-International layout (`00020409`, as `keys.json` says). Windows 8 and later
  activate only layouts in the user's language list, and this machine has only US (`04090409`). Neither
  `WM_INPUTLANGCHANGEREQUEST` from outside nor `LoadKeyboardLayoutW(KLF_ACTIVATE)` inside the fixture switched
  it. So the dead-key pin ran on the US layout, and the goldens' `settings.deadKeyLayout` says so. Capturing real
  dead keys means adding US-International to the user's input methods, which is a user setting and was not done.
  Both fixtures still try (`--keyboard-layout`).

### Running a capture (maintainer step, Windows, nobody using the mouse)

```sh
tools/dev/win sync
tools/dev/win cargo test -p svelters-conformance --test win32_capture -- --nocapture             # compare
tools/dev/win cargo test -p counter-win --test na_capture -- --nocapture                         # compare
tools/dev/win '$env:NA_UPDATE="1"; cargo test -p svelters-conformance --test win32_capture'      # rewrite
tools/dev/win '$env:NA_PINS="select-collapse,focus-tab"; cargo test -p counter-win --test na_capture -- --nocapture'
```

The capture writes `target/na/windows/` in the clone. Copy a new golden from the clone's
`conformance/golden/windows/` into the WSL checkout and commit it there. The WinUI capture also needs:

- the Windows App Runtime 2.5 Main, Singleton and DDLM packages (installed on this machine from the NuGet runtime
  package, as `windows-winui.yml` does; only the framework package was there before);
- the bootstrapper DLL in the clone's `target/winsdk` (`python3 tools/metadata/winui/fetch.py --bootstrap` on
  Linux, then copy the DLL);
- for the counter-win selftests, a fixture bundle built on Linux and copied into the clone (there is no Windows
  node).

## Captured (2026-10-22)

| Golden | Control | Run time | Deterministic |
|---|---|---|---|
| `win32-edit-6.10.19041.7802.json` | `EDIT` + `BUTTON`, comctl32 6.10.19041.7802 (WinSxS) | 72 s (18 pins) | yes: a second capture reproduced it byte for byte |
| `winui-3.2.3.0.json` | WinUI 3 `TextBox` + `Button`, WinAppSDK 2.5.1 | 131 s (18 pins) | yes: a second capture reproduced it byte for byte |

The determinism check was also run on the WG3 pins alone, both before and after the pointer fix, with the same
result each time.

### Findings on the native side (Win32 vs WinUI)

They agree on pin 2 (selection: 42/42) and on clipboard keys (20/20). They differ on everything else:

- **Word stops** (`word-move`; both stop at word starts, like Qt):
  - Win32 `EDIT` skips punctuation together with the following space: `5, 10, 18, 28, 39, 45, 50`. It then steps
    one character at a time through `日本語テキスト`.
  - WinUI treats punctuation runs as words (`5, 8, 10, 13, 14, 18, 28, 33, 34, 39, ...`): `,`, `.` and `_` are
    stops, and `3.14` is three words.
- **Ctrl+Backspace in a Win32 `EDIT` inserts U+007F** (DEL) instead of deleting a word. This is the classic EDIT
  behaviour: only `SHAutoComplete` fields get word deletion. Ctrl+Delete deletes to the next word start. WinUI
  deletes words both ways. For pin 1 the WinUI golden is the useful Windows reference.
- **Tab into a field:** the Win32 dialog manager selects all (`[0,len]`, like GTK and Qt). **WinUI keeps the caret**,
  as egui does.
- **Double-click:**
  - On `bar-baz`, Win32 selects `bar-` (the word plus its trailing delimiter); WinUI selects `bar`, and on `-`
    only `-`.
  - On `qux.quux`, Win32 selects the whole run plus the space; WinUI selects `qux` only.
  - On the CJK run, WinUI selects one character.
  - Triple-click selects all in WinUI and does nothing special in Win32.
- **Double-click-drag:** WinUI extends by words, both forward and backward. Win32 extends by its word rule.
- **Undo:** a Win32 `EDIT` has one undo level: Ctrl+Z (and Ctrl+Shift+Z) toggles between the last two states,
  and Ctrl+Y does nothing. WinUI has a real stack: `undo-typing` undoes the whole typed run, `undo-paste` undoes the
  paste and typing in steps, Ctrl+Y redoes, and Ctrl+Shift+Z does nothing.
- **Multi-line paste into a single-line field:** WinUI keeps the text up to the first line break (`one`). Win32 keeps
  `one\ntwo` and cuts at the first CRLF.
- **Middle-click** does nothing in either. After a middle-click into a WinUI `TextBox`, a plain drag in it did not
  select (`primary` #3). The cause is not known; it may be a WinUI pan mode [unverified].
- **Escape** keeps focus in both. An unfocused field keeps its selection in both (Win32 shows `[1,9]` in `a` after
  a click into `b`).

## Task 2: egui `TextEdit` against the Windows goldens (pins 1–3 and 7)

`egui_text_edit_against_windows_goldens` runs egui 0.37 `TextEdit` with egui's own behaviour, as egui-winit feeds it
on Windows. It ran under kittest on the Windows host before the merge of origin/master and on Linux after it. Both
gave the same table: the measurement is synthetic input, so it does not depend on the OS. FX1's runner had not
merged when this started, so the Windows run is a separate test function in the same file. After the merge it uses
FX1's `Egui::new(native, os)`, with `Native::Egui` (FX1's default on Windows), and it can move into FX1's runner
when a Windows policy row exists.

| Pin | Script | Checkpoints | = Win32 EDIT (first difference) | = WinUI TextBox (first difference) | = either |
|---|---|---|---|---|---|
| 1 | word-move | 33 | 2 (#1 `word-next`) | 4 (#1 `word-next`) | 4 |
| 1 | word-delete-back | 16 | 1 (#1 `delete-word-back`) | 3 (#3 `delete-word-back`) | 3 |
| 1 | word-delete-fwd | 16 | 1 (#1 `delete-word-fwd`) | 3 (#1 `delete-word-fwd`) | 3 |
| 1 | word-delete-mid | 7 | 1 (#1 `delete-word-fwd`) | 1 (#1 `delete-word-fwd`) | 1 |
| 2 | select-all | 11 | **11** | **11** | 11 |
| 2 | select-collapse | 17 | 14 (#10 `word-next`) | 14 (#10 `word-next`) | 14 |
| 2 | select-extend | 14 | 12 (#7 `shift+word-next`) | 12 (#7 `shift+word-next`) | 12 |
| 3 | multi-click | 14 | 3 (#2 `click a@5 x2`) | 8 (#3 `click a@7 x2`) | 8 |
| 7 | focus-click | 7 | 2 (#2 `click c x1`) | 5 (#2 `click c x1`) | 5 |
| 7 | focus-tab | 9 | 1 (#1 `tab`) | **9** | 9 |
| | **total** | **144** | **48** | **70** | **70** |

Per pin against WinUI: pin 1 is 11/72, pin 2 37/42, pin 3 8/14 and pin 7 14/16. The gaps against WinUI:

- **Pin 1:** egui stops at word ends (3, Linux-like). Windows stops at word starts (5) and makes punctuation its
  own word. A `WordRules::Windows` next to FX1's `Pango` and `Qt` rules would close it.
- **Pin 2:** `word-next` and `shift+word-next` move to the next word start on Windows (12). egui moves to the
  word end (11).
- **Pin 3:** WinUI's double-click selects only `-` on `-` (egui: `bar`) and `foo ` on a space (egui: `foo bar`).
  Its double-click-drag extends by words. A click after the CJK run lands one character early in egui (WG3 saw the
  same tofu-font offset). WinUI keeps an unfocused field's selection, and egui collapses it.
- **Pin 7:** in WinUI a press on the button focuses it, and Escape keeps focus; egui does neither. Tab selects
  nothing in either egui or WinUI.

Most of this is FX1's policy table with a Windows row. `Native::for_os` keeps egui's behaviour on Windows until
references exist; they exist now.

## Task 3: the counter-win selftest flake

CI's failure (windows-winui run 39806267666, attempt 1, branch ci/fx1) reported three failures at once:

- `count: expected "Count: 3", got "Count: 0"`
- `hello: expected "Hello Ada", got "Hello "`
- `check: expected "Level 42, agree: yes", got "Level 20, agree: no"`

Its report also held the `tree` that only a successful driving step writes. So the driving ran, and none of its
effects had reached JavaScript when the check read the widgets.

**Cause.** The selftest scheduled the driving 800 ms and the check 2500 ms after startup, both before the loop
started. When the UI thread is busy for more than 2.5 s before its loop begins (a slow first frame on CI's
software-rendered runners), both timers are already due when the loop starts. They then run back to back. The
click and change events reach JavaScript in a checkpoint that the host schedules with a wake, so that checkpoint
had not run yet when the check read the widgets.

**Reproduced.**

| Run | Before the fix | After the fix |
|---|---|---|
| Plain selftest, 20× on Windows | 20/20 pass | 20/20 pass |
| `--stall-ms 3000` (new: blocks the UI thread before its loop) | 3/3 fail, with CI's exact three failures | 5/5 pass |

**Fix** (`examples/counter-win/src/main.rs`):

1. The check is no longer timed from startup. The driving schedules it, and it polls every 100 ms until the
   read-back matches or 10 s pass. Here it settles in about 105 ms. The report gains `settled_ms`.
2. The driving waits until the elements it drives are loaded (`is-loaded`; the report gains `loaded_wait_ms`).
   **Finding:** a WinUI `TextBox` whose `Text` is set before it is loaded never raises `TextChanged`. With only
   fix 1, the stalled runs still failed on `hello` after 10 s.
3. A driving error is now reported. Before, the timed check overwrote it.

`tests/counter.rs` gains `counter_win_selftest_slow_start` (`--stall-ms 3000`), which turns the flake into a
deterministic regression test. Both counter selftest cases passed on Windows.

## Task 4: CI

The capture determinism check was **not added** to `windows-winui.yml`. It would not be quick:

- Each check is two captures. Locally that is 2 × 72 s for Win32 and 2 × 131 s for WinUI with all 18 pins,
  before CI's slower runners.
- The Win32 test also compiles this crate's egui and kittest dev-dependencies.
- CI's Windows image has other comctl32 and WinUI versions, so it would compare two captures with each other, not
  with these goldens.

The check stays a local maintainer step on the WSL PC. The workflow does run the new `counter_win_selftest_slow_start`
case, a few seconds more.

## Not done / follow-ups

- **Dead keys on US-International:** add the layout to the user's input methods (a user setting) and re-capture
  `dead-keys`.
- **egui pins 4–6 on Windows:** the kittest driver models egui-winit's Linux clipboard and dead-key paths. Windows
  needs its own model (CUA chords become clipboard commands only on Windows, per WG3b).
- **A real egui window on Windows** (eframe and AccessKit's UIA provider driven by `UiaDriver`) was not tried.
  AccessKit's Windows adapter may lack `TextPattern2`, in which case directions would read as unknown.
- **macOS** goldens are still not captured.
- **Interference while capturing:** input goes to the interactive desktop, so a capture fails or diverges if
  someone uses the mouse or another agent drives the desktop at the same time. Captures clear the clipboard.
