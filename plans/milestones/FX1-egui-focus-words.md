# FX1: egui focus policy and word rules against the native references

Track: WIDGETS. Follows WG3 (`plans/milestones/WG3-native-references.md`), which measured egui `TextEdit`
against the GTK 4.24.1 and Qt 6.12.0 goldens and found the gaps in svelters-egui's focus policy (pin 7) and in
the word rules (pin 1). User decisions used (`plans/research/EGUI.md` §8): W1 (native per-platform behaviour for
text, selection and keys), W2 (GTK4 and Qt are both Linux references), W4 (keep egui `TextEdit`, close the gaps
around it). egui `TextEdit` is not forked: every change sits before or after it in the leaf.

## As built

| Piece | Where | What |
|---|---|---|
| Policy table | `crates/svelters-egui/src/native/mod.rs` | `Native::{Gtk, Qt, Egui}` and `Policy` (one switch per measured behaviour). Per-context setting `native::set(ctx, …)` / `native::get(ctx)`; default `Native::for_os(ctx.os())`: Linux and the BSDs follow GTK, Windows and macOS keep egui's behaviour until WG3 captures their references. |
| Word rules | `crates/svelters-egui/src/native/words.rs` | `WordRules::Pango` (GTK: Pango's `is_word_start`/`is_word_end` from general categories, letters and numbers make words, punctuation and symbols end them, Japanese script-class breaks) and `WordRules::Qt` (`QTextLayout` `SkipWords`: white space, Qt's separator set, the rest). New dependency `unicode-general-category` 1.1 (already in `Cargo.lock`). |
| Leaf hooks | `crates/svelters-egui/src/leaf.rs` (+19 lines, additive) | `text-edit`: Escape restore and the word keys before `TextEdit`, focus and selection policy after; `button`, `checkbox`: focus on press, `:focus-visible` drawing; other leaves: Escape restore. |
| Runner | `crates/svelters-conformance/tests/egui_measure.rs` | Runs every pin three times (`Egui`, `Gtk`, `Qt`), writes `target/na/linux/egui-gap.md` and the table into the CI job summary (`GITHUB_STEP_SUMMARY`, linux-gtk job), and gates the native runs with a per-pin ratchet (`FLOOR`). |
| Tests | `native/words.rs`, `native/mod.rs` (unit), `crates/svelters-egui/tests/native_focus.rs` | Word stops of pin 1 against both goldens; the platform table; GTK policy and the unchanged egui policy through real kittest input. |

### The policy, per toolkit

| Behaviour | GTK (Linux default) | Qt | egui (Windows, macOS for now) |
|---|---|---|---|
| Ctrl+Left/Right, Ctrl+BackSpace/Delete | Pango word rules | Qt word rules | egui's |
| Word move without Shift on a selection | collapses to its end (forward) or start, no move | moves from the caret | moves from the caret |
| Tab/Shift+Tab into a single-line field | selects all, caret at the end | selects all | caret kept |
| `focus` method (programmatic) | selects all (`grab_focus`) | caret kept (`OtherFocusReason`) | caret kept |
| Unfocused field's selection | kept | collapsed to the caret | collapsed to the caret |
| A field selects while another holds a selection | the other collapses to its start (PRIMARY changes hands) | nothing | nothing |
| Press on a button or check box | focuses it | focuses it | nothing |
| Escape | focus stays | focus stays | focus dropped |

How each is done without touching egui:

- **Word keys**: before a focused `TextEdit` runs, its Ctrl+arrow/BackSpace/Delete key events are taken out of
  this pass's input and applied to the text and `TextEditState` with the rules (a password field moves to its
  ends, as GTK and Qt do); the leaf reports the text change itself (`changed`, observe), since `TextEdit` did not
  make it.
- **Select-all on keyboard focus**: after the `TextEdit` ran, when it gained focus this pass (egui `Memory`:
  focused now, not at the end of the last pass) with no pointer on it and a Tab or Shift+Tab in this pass or the
  one before (egui moves Shift+Tab's focus a pass later). Focus restored after a viewport change (EM1) or given by
  assistive technology does not select.
- **Kept unfocused selection**: egui collapses an unfocused field's selection to its caret on every pass; the leaf
  puts the range it had before the run back.
- **One selection owner**: the owner's id lives in the egui context's temp data; a focused field with a selection
  that is not the owner collapses the owner's selection to its start and takes over.
- **Focus on press**: after a button or check box ran, a pointer press on it requests focus. egui draws a focused
  widget as pressed (`WidgetState::Active`); GTK shows focus only when it came from the keyboard
  (`:focus-visible`), so a widget focused by a press is drawn with its hovered or inactive visuals until focus
  leaves it. Pointer-only use therefore draws exactly as before (the TK1 snapshots do not change).
- **Escape**: egui's `Memory` drops focus on a plain Escape at the start of the pass; the leaf that had it asks for
  it back, before a `TextEdit` runs (else it would collapse its selection for want of focus) and after other
  leaves (their id is known only then).

## Measured (kittest, `OperatingSystem::Nix`, against the checked-in goldens)

`tools/dev/cargo-slot cargo nextest run -p svelters-conformance --test egui_measure --no-capture`. Before is the
`Egui` run (the same as WG3's numbers); after is the `Gtk` run against GTK's golden and the `Qt` run against Qt's.

| Pin | Script | Checkpoints | before = GTK | before = Qt | after: gtk = GTK | after: qt = Qt |
|---|---|---|---|---|---|---|
| 1 | word-move | 33 | 14 | 13 | **33** | **33** |
| 1 | word-delete-back | 16 | 5 | 8 | **16** | **16** |
| 1 | word-delete-fwd | 16 | 8 | 4 | **16** | **16** |
| 1 | word-delete-mid | 7 | 7 | 1 | **7** | **7** |
| 2 | select-extend | 14 | 14 | 12 | **14** | **14** |
| 2 | select-collapse | 17 | 14 | 14 | **17** | **17** |
| 2 | select-all | 11 | 11 | 11 | **11** | **11** |
| 3 | multi-click | 14 | 9 | 9 | 11 | 9 |
| 7 | focus-tab | 9 | 1 | 1 | **9** | **9** |
| 7 | focus-click | 7 | 2 | 2 | **7** | **7** |
| | **total** | **144** | **85** | **75** | **141** | **139** |

Per pin, sums: pin 1 34→72 (GTK) and 26→72 (Qt) of 72; pin 2 39→42 and 37→42 of 42; pin 3 9→11 and 9→9 of
14; pin 7 3→16 and 3→16 of 16. The ratchet (`FLOOR` in `egui_measure.rs`) holds these numbers: CI fails when a
native run matches fewer checkpoints of a pin; raise it when a change matches more.

Pin 3's GTK gain is the kept unfocused selection and the PRIMARY hand-over (#12, #13: a click, then a
double-click, into the other field).

## Not done / follow-ups

- **Pin 3, double-click word selection** (GTK 3 checkpoints, Qt 5). egui selects the word at the second press
  with its own rules, in `TextCursorState::pointer_interaction`. Matching needs the click's character, so the leaf
  would use `TextEdit::show` (galley and position) instead of `ui.add`, plus the press count; no fork. The rules
  the goldens show:
  - GTK 4.24: start = the previous word start before the caret of the first click (white space stops it), end =
    the next word end *or start* after it: `-` in `bar-baz` gives `bar-`, a space after `foo` gives `foo `. This
    is inferred from WG3's five double-clicks, not from GTK's source; double-click-drag already matches.
  - Qt: `QWidgetLineControl::selectWordAtPos` (the Qt word rules, trailing white space trimmed: `foo`, `-`), and
    a drag after a double-click extends by characters from the word's start (`[4,13]`, `[12,2]`), not by words.
  - #10 (a single click after the CJK run lands one character early) is the click point computed from egui's
    tofu glyphs (WG3), not a policy gap [unverified].
- Provider kinds (`#[svelters::widget]`) get none of this: the hooks are in the built-in leaves. A provider that
  wraps a `TextEdit` can call the same functions once they are made public for it (not needed by SC1).
- Windows and macOS stay on egui's behaviour until WG3 has their goldens; `Native::Qt` is the closer model for
  Windows (word starts) but is not measured there.
- Unmeasured keys keep egui's handling: Alt+arrows, Ctrl+W/U/K/H, Ctrl+Shift+BackSpace.

## Verification

Run locally (WSL PC): `cargo check -p svelters-egui`; `cargo nextest run -p svelters-egui -p svelters-conformance
-E 'test(fx1_) | test(egui_text_edit_against_native_goldens)'` (6 tests, all pass). Not run locally: the rest of
the svelters-egui and svelters-testkit suites (including the TK1 snapshot cases) and every other crate; CI runs
them.
