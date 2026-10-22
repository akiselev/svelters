# FX3s: salvage from the stopped FX3 lane

FX3 (`plans/milestones/FX3-text-editing.md` on `ci/fx3`, f120f7f3) was stopped by the user's decision that
svelters draws its own widgets. Its work commits are 800a4199 and e6e8c5e6. This lane brings four
self-contained pieces onto master, ported by hand. The rest stays on `ci/fx3`.

## Came over

| Piece | Where | What |
|---|---|---|
| W5 double paste | `crates/svelters-egui/src/primary.rs`, `leaf.rs` (+1 line) | Under eframe, egui-winit sends `MiddleClickPaste` with the middle press and egui 0.37's `TextEdit` pastes it. W5 then pasted PRIMARY a second time on the release. `primary::note_egui_paste` (called before the leaf's `TextEdit`) records whether the integration sent it, and `primary::text_edit` skips its paste when it did. Test: `tier0::text_edit_middle_click_pastes_once` (gives `PPhello` without the fix). |
| W5 on DX2's clipboard | `crates/svelters-egui/src/primary.rs` | `PrimarySelection` is now also any `ClipboardBackend`'s `ClipboardKind::Primary` (`primary::install_backend`). `use_system` installs `platform::SystemClipboard` instead of W5's own arboard code. The `install`/`uninstall`/`provider` API is unchanged. |
| FX1 repaint loop | `crates/svelters-egui/src/native/mod.rs` (`TextFocus::after`) | egui collapses an unfocused field's selection on every pass, and FX1 (GTK) put it back with a repaint each time, so the app repainted endlessly while an unfocused GTK field held a selection. It now repaints only when the range differs from the one before the pass. Test: `native_focus::fx1_unfocused_gtk_selection_settles` (fails without the fix: `Rig::run` never settles). |
| egui_measure per-pass output | `crates/svelters-conformance/tests/egui_measure.rs` | A kittest `step` runs one pass per queued event and keeps only the last pass's output. Keys are now fed one event per step (`Egui::press`), and every pass's `CopyText`/`TextSelectionSettled` is collected. Like eframe on Linux, the runner's W5 PRIMARY provider is installed (it shares one PRIMARY with egui's `TextSelectionSettled`) and a `RequestPaste` is answered with a paste in the next pass. Nothing on master sends `RequestPaste` yet. |
| Undo models | `crates/svelters-egui/src/native/undo.rs` | FX3's pure GTK (`GtkTextHistory`, fitted to the 4.24.1 golden) and Qt (`QWidgetLineControl`) undo models, with their golden-replay unit tests (`fx3_undo_*`, `fx3_splice_*`). This is the spec the own-widget text field will run against. **Choice:** it is `pub(crate) mod undo` under `#[allow(dead_code)]` in `native/mod.rs`. It is not test-only, so the own field can use it directly, and it is not public, so it adds no API. Nothing in the crate uses it yet. |

## Measured (egui_measure, kittest `OperatingSystem::Nix`)

With the per-pass counter, the stock egui and policy runs change only on `primary`:

| Script | before gtk / qt (floor) | after gtk / qt |
|---|---|---|
| primary | 5 / 5 | **6 / 8** |
| all 18 scripts (native policy runs) | 192 / 197 | **193 / 200** |

Stock egui: 133 / 136. The `primary` floor is raised to (6, 8), the measured values. No other floor changes.
WG3b's `clipboard-keys` copy-alt miss (#12) remains on master: Ctrl+Insert is not mapped by egui on Linux.
The fix was FX3's CUA chord, which was left behind.

## Left on `ci/fx3`

- The `TextEdit` behaviour wiring (`native/edit.rs`: undo through keys, the CUA chords, line breaks kept by a
  paste, Ctrl+Shift+U hex entry with its preedit, Enter keeping focus, keys given back for EV3's claim).
- The `Policy` switches (`undo`, `cua_clipboard_keys`, `paste_keeps_breaks`, `primary_clears_on_collapse`,
  `ctrl_shift_u`/`ShiftU`, `enter_keeps_focus`) and `primary::clear` (GTK emptying PRIMARY on collapse).
- `tests/native_edit.rs` (except the two tests ported above) and FX3's raised floors (232 / 235).

## Verification

Local (thinkpadx1): `cargo clippy -p svelters-egui --all-targets` (no warnings in the changed files);
`cargo nextest run -p svelters-egui -E 'test(fx1_) | test(primary) | test(middle_click) | test(fx3_)'` (12 pass);
`cargo nextest run -p svelters-conformance -E 'test(egui_text_edit_against_native_goldens)'` (pass, numbers above).
Both new tests were checked to fail with their fix removed. Not run locally: everything else (CI runs it).
`cargo clippy -p svelters-conformance` stops on a clippy error in `svelters-gtk` that is already on master.
