# tessel: IME, text editing, accessibility and OS integration (research notes, 2026-10-25)

Context: `cadatomic/docs/research/2026-10-25-own-gui-toolkit.md` §7–§9 (tessel, winit first, own
platform backends later, quire-text for editing, AccessKit as the single producer, an egui island).
Every claim carries a source and a grade:

- **documented**: stated by the vendor or project documentation;
- **source-read**: read in the source at the named version or sha (clones made for this note, now deleted);
- **inferred**: my reading or estimate from the cited sources; alternatives named where they matter.

Versions read (source-read, `git log` in each clone):

| Project | Version / sha | Date |
|---|---|---|
| winit | `v0.30.13` tag | 2026-03-02 |
| winit | `v0.31.0-beta.4` tag (master `0c737280`) | 2026-10-24 |
| AccessKit | `accesskit-v0.25.1`, `accesskit_winit-v0.34.1`, `accesskit_windows-v0.35.1`, `accesskit_macos-v0.27.1`, `accesskit_unix-v0.24.0` (main `9671c2d`) | 2026-09-25 |
| Zed / GPUI | `a5c4ca8` (main) | 2026-10-25 |
| egui | `0.37.0` tag | 2026-10-15 (per cadatomic CLAUDE.md) |
| parley | `0.12.0` (main `2133d78`) | 2026-10-09 |
| quire | `/home/user/akiselev/quire` at `3e78dcd` | local |
| arboard 3.7.0, rfd 0.17.2, muda 0.21.2, notify-rust 4.18.2 | GitHub default branches | 2026-10-25 |
| cadatomic `Cargo.lock` | egui 0.37.0, winit 0.30.13, accesskit 0.25.1, accesskit_winit 0.34.1, arboard 3.7.0, rfd 0.17.2, smithay-clipboard 0.7.3 | local |

---

## 1. IME

### 1.1 What an editor needs from the toolkit (the contract)

Every native IME protocol is a *query-driven* protocol: the input method asks the application
about its document, it does not only push events. The four OS protocols compared:

| Need | Windows TSF (`ITextStoreACP`) | Windows IMM32 | macOS `NSTextInputClient` | Wayland `zwp_text_input_v3` | X11 XIM |
|---|---|---|---|---|---|
| Composition (marked) text pushed to app | `SetText` under a lock + composition sink | `WM_IME_COMPOSITION` + `ImmGetCompositionStringW` | `setMarkedText:selectedRange:replacementRange:` | `preedit_string` + `done` | preedit draw callbacks |
| Commit | `OnEndComposition` / `InsertTextAtSelection` | `GCS_RESULTSTR` | `insertText:replacementRange:` | `commit_string` | commit callback |
| Replace an existing range (reconversion, autocorrect) | `SetText(acpStart, acpEnd)` | `IMR_RECONVERTSTRING` via `WM_IME_REQUEST` | `replacementRange` argument | `delete_surrounding_text` (bytes) | none |
| Read the document (surrounding text) | `GetText`, `GetEndACP` | `IMR_DOCUMENTFEED` | `attributedSubstringForProposedRange:actualRange:` | `set_surrounding_text` (≤4000 bytes) | none |
| Selection | `GetSelection`/`SetSelection` | – | `selectedRange` | in `set_surrounding_text` (cursor, anchor) | – |
| Geometry per range | `GetTextExt` (screen rect of any ACP range), `GetScreenExt` | `IMR_QUERYCHARPOSITION`, else one composition/candidate rect | `firstRectForCharacterRange:actualRange:` (any range) | one `set_cursor_rectangle` | spot location |
| Hit test point → index | `GetACPFromPoint` | – | `characterIndexForPoint:` | – | – |
| App notifies IME of changes | `ITextStoreACPSink::OnTextChange/OnSelectionChange/OnLayoutChange` | `ImmNotifyIME` | `NSTextInputContext invalidateCharacterCoordinates`, `discardMarkedText` | `set_text_change_cause` + `commit` | – |
| Units | UTF-16 code units | UTF-16 | UTF-16 (`NSRange`) | UTF-8 bytes | locale/UTF-8 |

Sources: Microsoft `ITextStoreACP2` method list (documented,
https://learn.microsoft.com/en-us/windows/win32/api/textstor/nn-textstor-itextstoreacp2, minimum client
Windows 8); Apple `NSTextInputClient` (documented, JSON of
https://developer.apple.com/documentation/appkit/nstextinputclient: required methods are
`hasMarkedText`, `markedRange`, `selectedRange`, `setMarkedText:…`, `unmarkText`,
`validAttributesForMarkedText`, `attributedSubstringForProposedRange:actualRange:`,
`insertText:replacementRange:`, `characterIndexForPoint:`, `firstRectForCharacterRange:actualRange:`,
`doCommandBySelector:`; optional ones include `documentVisibleRect`, `unionRectInVisibleSelectedRange`,
`preferredTextAccessoryPlacement`); text-input-v3 (documented, https://wayland.app/protocols/text-input-unstable-v3,
now at version 2 with `set_available_actions`, `show/hide_input_panel`, `action`, `language`,
`preedit_hint`); Chromium's IMM32 handling of `IMR_RECONVERTSTRING`, `IMR_DOCUMENTFEED`,
`IMR_QUERYCHARPOSITION` (source-read,
https://raw.githubusercontent.com/chromium/chromium/main/ui/base/ime/win/input_method_win_base.cc lines ~326–403).

So the editor-side API tessel needs is a **text input client** trait that a focused text kind
(quire editor, svelters field) implements, close to GPUI's `InputHandler` (source-read,
`zed/crates/gpui/src/platform.rs` ~2300–2420 at `a5c4ca8`):
`selected_text_range`, `marked_text_range`, `text_for_range` (with adjusted range),
`replace_text_in_range`, `replace_and_mark_text_in_range(range, text, new_selected_range)`,
`unmark_text`, `bounds_for_range`, `selection_bounds_for_range`, `character_index_for_point`, all
in UTF-16 units. tessel should add what GPUI lacks (inferred from the table above):

- `document_len()` and a bounded `surrounding_text(window)` for Wayland (bytes) and TSF/IMR_DOCUMENTFEED;
- an outbound channel: `text_changed(cause)`, `selection_changed`, `layout_changed` (scroll, zoom,
  reflow), `cancel_composition`, `commit_composition` — the platform layer turns these into
  `OnTextChange`/`OnLayoutChange`, `invalidateCharacterCoordinates`/`discardMarkedText`,
  `set_text_change_cause(other)`+`commit`, `ImmNotifyIME(NI_COMPOSITIONSTR, CPS_CANCEL/CPS_COMPLETE)`;
- `input_purpose`/hints (password, number, URL, multiline, no-autocorrect);
- unit conversion in one place: document-absolute UTF-16 ↔ quire's UTF-8 byte offsets. quire-text
  has per-line `utf16_to_byte(line, col16)` / `byte_to_utf16(b)` and `byte_to_char` / `char_to_byte`
  (source-read, `quire-text/src/text.rs` 693–770) over ropey 1.6.1; ropey also has O(log n)
  `char_to_utf16_cu`/`utf16_cu_to_char` (inferred from ropey 1.x API; not re-read here), so a
  document-absolute UTF-16 mapping is cheap to add.
- **Key routing order** (GPUI's lesson, source-read `gpui_macos/src/window.rs` ~2660–2700): while
  composing, or while an IME input source is active and the focus accepts text, printable keys go to
  the IME *before* keybinding matching; the IME hands unhandled keys back through
  `doCommandBySelector:` which then goes to keybindings. Cmd/Ctrl/Fn chords skip the IME. This
  contradicts a naive reading of the toolkit doc §7.1 ("Keymaps resolve in Rust … before JS sees
  the key"): keymaps must resolve *after* the IME on macOS, or `jj`-style chords break Japanese input.

### 1.2 winit 0.30.13 (what cadatomic uses today)

- Events: `Ime::Enabled`, `Preedit(String, Option<(usize, usize)>)` (byte offsets), `Commit(String)`,
  `Disabled` (documented, https://docs.rs/winit/0.30.13/winit/event/enum.Ime.html). An empty `Preedit` is sent
  before every `Commit`.
- Requests: `set_ime_allowed`, `set_ime_cursor_area`, `set_ime_purpose` with only `Normal`,
  `Password`, `Terminal` (source-read, `src/window.rs` 1829–1839 at `v0.30.13`).
- No surrounding text, no delete-surrounding, no way to cancel/commit a composition. egui-winit
  0.37 works around the latter by toggling `set_ime_allowed(false)`/`(true)` (source-read,
  `egui-winit/src/lib.rs` ~1276–1283, "TODO … use a more proper way to interrupt composition").
- Windows: `KeyboardInput` events processed by the IME still arrive (winit #4508, open); egui-winit
  filters `NamedKey::Process` (source-read, `egui-winit/src/lib.rs` ~656–700). egui also drops a
  bogus zero-width cursor range from the built-in Korean IME on Windows (same file ~800).

### 1.3 winit 0.31.0-beta.4 (2026-10-24)

New IME API (source-read, `winit-core/src/window.rs` at `v0.31.0-beta.4`; changelog
`winit/src/changelog/v0.31.md`):

- `Window::request_ime_update(ImeRequest::{Enable(ImeEnableRequest), Update(ImeRequestData), Disable})`;
  `ImeCapabilities` (cursor area, hint+purpose, surrounding text) must be declared at enable and
  each capability's initial value supplied; to change capabilities you disable and re-enable. The old
  `set_ime_*` methods are deprecated shims.
- `ImePurpose` gains Number, Phone, Url, Email, Pin, Date, Time, DateTime; `ImeHint` bitflags
  (completion, spellcheck, auto-capitalisation, case, hidden text, sensitive, latin, multiline).
  **Both are documented as unsupported on Windows, X11, macOS, iOS, Android, Web** — Wayland only.
- `ImeSurroundingText::new(text, cursor, anchor)`, ≤4000 bytes (the Wayland limit), and
  `Ime::DeleteSurrounding { before_bytes, after_bytes }`. Implemented only in the Wayland backend
  (source-read: `git grep surrounding` finds it in `winit-wayland/src/seat/text_input/mod.rs` and
  core; the macOS backend has only `// TODO: Use _replacement_range, requires changing the event to
  report surrounding text`).
- beta.4 changes: no empty `Preedit` before `Commit`; Windows UTF-16 surrogate fix in preedit cursor;
  Wayland text input no longer destroyed on seat capability change; macOS forwards clicks to the input
  context and discards marked text on disable (changelog v0.31.md "0.31.0-beta.4").
- Open design complaint: winit #4412 "New IME API is too restrictive" (opened 2025-11-17, open):
  cursor area must be known at enable; widget-level data (purpose, surrounding text) and window-level
  data (cursor area) are bundled (documented, https://github.com/rust-windowing/winit/issues/4412).

Per-backend IME implementation in 0.31.0-beta.4 (source-read):

- **Windows: IMM32 only.** `winit-win32/src/ime.rs` (163 lines) uses `ImmGetContext`,
  `ImmGetCompositionStringW` (`GCS_COMPSTR`, `GCS_RESULTSTR`, `GCS_CURSORPOS`),
  `ImmSetCompositionWindow(CFS_POINT)`, `ImmSetCandidateWindow(CFS_EXCLUDE)` with the one cursor area,
  `ImmAssociateContextEx`. No TSF, no `WM_IME_REQUEST` handling (`git grep WM_IME_REQUEST` is empty),
  so no reconversion, no document feed, no per-character positions. A GitHub search for TSF in winit
  issues returns zero results (https://github.com/rust-windowing/winit/issues?q=TSF, fetched 2026-10-25).
- **macOS: partial `NSTextInputClient`.** `winit-appkit/src/view.rs` 239–455: `selectedRange` always
  returns `{NSNotFound, 0}`; `attributedSubstringForProposedRange` returns `nil`;
  `characterIndexForPoint` returns 0; `firstRectForCharacterRange` ignores the range and returns the
  single app-set cursor area; `replacementRange` is ignored in both `setMarkedText` and `insertText`;
  `validAttributesForMarkedText` = underline + marked clause. `doCommandBySelector:` forwards the
  selector name to `ApplicationHandlerExtMacOS::standard_key_binding` (new in 0.31.0-beta.1), which
  is how an app can honour `StandardKeyBinding.dict` / `DefaultKeyBinding.dict` emacs bindings.
- **Wayland: text-input-v3**, with content type, cursor rectangle, surrounding text, delete
  surrounding (`winit-wayland/src/seat/text_input/mod.rs`, 463 lines).
- **X11: XIM** with preedit callbacks (`winit-x11/src/ime/*`), cursor area sent as spot and, since
  0.31.0-beta.1, as an "area" attribute; no direct IBus/Fcitx D-Bus path.

Open winit IME bugs that matter for an IDE (documented, issue titles from
https://github.com/rust-windowing/winit/issues?q=is%3Aissue%20state%3Aopen%20IME, fetched 2026-10-25;
38 open):

- macOS: #4666 Dictation silently drops text (`selectedRange` returns NSNotFound); #4524 Korean IME
  hanja candidate window cannot be opened; #3342 IME and the Character Viewer; #3617 `selected_range`
  and `replacement_range` ignored; #3814 Chinese IME punctuation reversed; #3095 Korean character
  separation; #4537 arrow keys suppressed during dead-key composition; #4526 key-release leaks;
  #4626 `insertText` panics on teardown with marked text; #2651 dead keys in custom layouts trigger
  IME with modifiers.
- Windows: #4508 `KeyboardInput` still emitted while IME allowed; #2780 Sogou candidate window not
  near the caret; #2569 `set_ime_position` broken after toggling with MS Japanese IME.
- X11: #4777 moving the cursor area while the IM forwards keys garbles text; #4525 release events leak
  with Fcitx5; #3761/#3875 crashes with fcitx / IME callback; #2915 dead keys ignored during compose;
  #3092 cannot disable IME or set position; #2498 spurious `Ime::Disabled`.

### 1.4 What GPUI, Chromium and Firefox do beyond winit

- **GPUI (Zed `a5c4ca8`)**, source-read:
  - Owns its platform layers (`gpui_windows`, `gpui_macos`, `gpui_linux`), no winit.
  - macOS: full `NSTextInputClient` through `InputHandler` (UTF-16 ranges, marked range, text for range,
    bounds for range, character index for point); honours `ApplePressAndHoldEnabled`
    (`gpui_macos/src/window.rs` ~2750) so press-and-hold accents work; IME-first key routing (above).
  - Windows: **IMM32, not TSF** (`gpui_windows/src/events.rs`: `WM_IME_STARTCOMPOSITION`,
    `WM_IME_COMPOSITION`, `ImmGetCompositionStringW` incl. `GCS_COMPATTR`; candidate at caret with
    `CFS_CANDIDATEPOS`); no `WM_IME_REQUEST`.
  - Linux: Wayland text-input-v3 with content type and cursor rectangle but no surrounding text;
    X11 via Zed's fork of `xim-rs` with `PREEDIT_CALLBACKS`.
- **Chromium** (source-read, chromium `main` 2026-10-25):
  - `ui/base/ime/win/tsf_text_store.h`: `TSFTextStore : ITextStoreACP, ITfContextOwnerCompositionSink,
    ITfKeyTraceEventSink, ITfTextEditSink`, with a documented lock protocol (RequestLock →
    OnLockGranted, recursive requests queued) and a final check that resyncs TSF with
    `OnSelectionChange/OnLayoutChange/OnTextChange` when the document diverged; `.cc` is 1,796 lines.
  - `input_method_win_tsf.cc`: "Even when TSF is enabled, following IMM32/Win32 messages must be
    handled … Some TSF-native TIPs … such as ATOK and Mozc still rely on WM_IME_REQUEST message to
    implement reverse conversion."
  - The `kTSFImeSupport` flag that once chose TSF over IMM32 is gone from `ui_base_features.cc`; only
    `kTSFHonorAutocorrectOff` remains (inferred: TSF is unconditional now). An older 2013 change had
    made desktop Chrome use IMM32 everywhere (https://crrev.com/223589), and the Windows 8 Metro code
    noted that Metro mode disables CUAS, forcing TSF
    (https://cobalt.googlesource.com/cobalt.git/+/81606a0c0b7eb6f30716c9ecd484f320cb90e530/src/base/win/metro.cc).
  - Wayland: Chromium made `WaylandTextInputV3` the default
    (https://chromium.googlesource.com/chromium/src/+/e48954bdc391fcba2fd20a5a40a8a16332cf1638);
    Mutter supports only v3 (https://gitlab.gnome.org/GNOME/mutter/-/issues/3200).
- **Firefox** (source-read, `mozilla-firefox/firefox` `widget/windows/TSFTextStore.h`, `.cpp` 4,475
  lines): `TSFTextStoreBase : ITextStoreACP`, `TSFTextStore` adds `ITfContextOwnerCompositionSink`,
  `ITfMouseTrackerACP`; keeps a content cache `mContentForTSF`; per-TIP hacks
  (`IsMSJapaneseIMEActive`, `IsATOKActive`, `MaybeHackNoErrorLayoutBugs` because some TIPs mishandle
  `TS_E_NOLAYOUT` from `GetTextExt`).

### 1.5 Known cross-platform IME pitfalls to design for

- **Candidate placement**: TSF IMEs ask `GetTextExt` for the *clause being converted*, not the caret;
  with one cursor rect (winit) the candidate list sits at the caret (inferred from the TSF API and the
  Firefox hack). On Wayland and IMM32 one rect is all there is; send the rect of the composition's
  active clause, re-sent on every layout change (scroll, zoom, multi-line wrap).
- **Reconversion** (Japanese "再変換", Windows IME Ctrl+Backspace / Kana key): needs
  `IMR_RECONVERTSTRING` (IMM32) or TSF `GetText/SetText`; macOS needs `selectedRange` +
  `attributedSubstringForProposedRange`; Wayland only via surrounding text. Not possible through winit
  on Windows or macOS (source-read above).
- **Emoji picker**: Windows Win+. inserts through the IME/TSF path (Chromium opens it by sending Win+.
  through `SendInput`, per Chromium's `emoji_panel_helper_win.cc`, inferred from search snippet); macOS
  Character Viewer (Ctrl+Cmd+Space) calls `insertText:replacementRange:` and positions itself with
  `firstRectForCharacterRange` (winit #3342 open).
- **Dictation** (macOS) needs `selectedRange`; winit drops text (#4666).
- **Press-and-hold accents** (macOS `ApplePressAndHoldEnabled`): the accent popover uses marked text
  and `firstRect`; GPUI checks the default and routes the repeat accordingly (source-read).
- **Dead keys**: on macOS they are marked text through `NSTextInputClient`; on Windows `ToUnicode`
  state in the keyboard layer; on X11/Wayland xkb compose. All three backends have open dead-key bugs in
  winit (#4537, #2651, #2915, #3917).
- **Multi-cursor**: IMEs know one caret. quire-text already renders the preedit at every cursor and
  commits with `replace_prev_chars/replace_next_chars` per cursor (VS Code's `compositionType`;
  source-read `quire-text/src/command.rs` 128–151); the platform sees only the primary range.

### 1.6 Recommendation (IME)

- Phase 1 (winit 0.31): drive `request_ime_update` from the focused text kind each frame it changes
  (cursor area = active clause rect, purpose/hints, Wayland surrounding text from quire's `Text`
  around the primary cursor ≤4000 bytes, handle `DeleteSurrounding`). Implement IME-first key routing.
  Accept: no reconversion on Windows/macOS, no dictation/Character Viewer on macOS, Windows IMM32 bugs.
- Cheap Windows win before the own backend: a `SetWindowSubclass` hook on winit's HWND handling
  `WM_IME_REQUEST` (`IMR_RECONVERTSTRING`, `IMR_DOCUMENTFEED`, `IMR_QUERYCHARPOSITION`) from the text
  input client gives reconversion and per-char candidate placement with IMM32 (inferred; ~300–500 lines,
  following Chromium's `InputMethodWinBase`).
- Own backends: Windows TSF text store (~2–4.5k lines, cf. Chromium 1.8k + header, Firefox 4.5k; plus
  per-TIP quirks over time); macOS full `NSTextInputClient` (~800–1,200 lines inside the NSView
  subclass, cf. GPUI); Wayland text-input-v3 v2 (~500–800 lines, cf. winit 463); X11 XIM (~1k, or
  keep winit/xim-rs). All estimates inferred.

---

## 2. Text editing model

### 2.1 quire-text (local, `3e78dcd`), source-read

- No egui, no threads, no clock; builds for wasm32 (`crates/quire-text/Cargo.toml`). Deps: ropey,
  smallvec, unicode-segmentation, unicode-general-category.
- Model: `Text` (ropey, LF in memory, BOM/line-ending kept), `ChangeSet` (composable, invertible,
  position mapping with `Bias`), multi-range `Selection` with primary and per-range goal column,
  `Transaction` (changes + selection + origin + author + grouping, against a `Version`),
  `TextDocument` shared per buffer and `TextView` per view (selection, focus, preedit), `History`
  with pluggable `UndoRules` (local or host-owned undo). Positions are UTF-8 byte offsets valid only
  with their `Version` (`lib.rs`).
- `EditCommand` (`command.rs`): Insert (with `InputKind`, multi-cursor `distribute`), Move/Delete by
  `Unit::{Grapheme, Word(WordStop), LineEdge, VisualLine, Page, DocEdge}`, SelectAll, SelectWordAt,
  SelectLineAt, `SetSelection` and `ReplaceSelection` (named for AccessKit `SetTextSelection` /
  `ReplaceSelectedText`), Undo/Redo, Cut/Copy/Paste through `HostRequest::{SetClipboard,
  ReadClipboard, SetPrimary, ClearPrimary, Beep, Undo, Redo}`, `Preedit(PreeditUpdate::{Update{text,
  active}, Commit{text, replace_prev_chars, replace_next_chars}, Cancel})`, GTK hex entry (Ctrl+Shift+U).
- Policies: `WordRules::{VsCode(classifier), Pango, Qt, Custom}`; `WordStop` (VS Code's stops);
  `DeleteUnit::{VsCode, Grapheme, CodePoint}`; `PrimaryRule::{Off, Set (Qt), SetAndClear (GTK)}`;
  `secret` fields; `InputFilter`; smart home; empty-selection clipboard.
- Layout boundary: `VisualLines` trait (`visual.rs`) — the renderer supplies rows for Up/Down, Page,
  wrapped Home/End; `NoWrap` built in.
- **Gaps** (source-read by grep): no bidi or visual-order motion anywhere (no `bidi` in `quire-text`
  or `quire-core`; `egui_quire/src/a11y.rs` hard-codes `TextDirection::LeftToRight`); Left/Right are
  logical. No document-absolute UTF-16 API (per-line only). These are the two additions IME/a11y and
  RTL need; visual motion belongs behind `VisualLines`-like layout queries (inferred).
- `egui_quire` exposes AccessKit `MultilineTextInput` with `TextRun` children per visual row of the
  visible lines only, runs split at 255 chars, `Live::Polite` announcements (source-read,
  `egui_quire/src/a11y.rs` header). The 255 limit is quire's choice; AccessKit's constraint is that
  `character_lengths` is a `[u8]`, i.e. each *character* ≤255 bytes (source-read,
  `accesskit/src/lib.rs` ~2335–2353).

### 2.2 parley `PlainEditor` (0.12.0, 2026-10-09), source-read

- Single-style editor over a `String` + `Layout` (`parley/src/editing/editor.rs`), one selection with
  affinity, compose range, `show_cursor`, alignment, `base_direction`, `Generation` for change
  tracking.
- Motion: visual-order left/right and word left/right (`previous_visual`, `next_visual_word`; bidi
  aware), logical word for deletes, line/hard-line start/end, up/down, text start/end; point hit
  testing (`move_to_point`, `select_word_at_point`, `select_line_at_point`, `extend_selection_to_point`,
  `shift_click_extension`); byte-range selection.
- IME: `set_compose(text, cursor)`, `set_compose_byte_range`, `clear_compose`, `finish_compose`,
  `ime_cursor_area()`, `delete_bytes_before/after_selection` (surrounding-text deletes).
- No undo, no multi-cursor, no rope. **AccessKit integration was removed in 0.12.0** (changelog line
  122: "the `accesskit` feature, and the AccessKit integration it enabled, were removed", moved to
  `examples/editor/src/accessibility.rs`).
- Use for tessel (inferred): not as the model (quire-text is richer), but as the reference for
  bidi-aware cursor/selection geometry over a shaped layout, if tessel-text uses parley for shaping.

### 2.3 Keyboard navigation conventions

| Action | Windows / Linux | macOS |
|---|---|---|
| Word left/right | Ctrl+←/→ | Option+←/→ |
| Line start/end | Home/End | Cmd+←/→ (and Ctrl+A/E) |
| Document start/end | Ctrl+Home/End | Cmd+↑/↓ |
| Delete word back | Ctrl+Backspace | Option+Backspace |
| Delete to line start | – (VS Code: none default) | Cmd+Backspace |
| Emacs bindings | GTK only with the Emacs key theme | ^A ^E ^F ^B ^N ^P ^D ^H ^K ^Y ^T ^O ^V system-wide |
| Primary modifier | Ctrl | Cmd |

macOS bindings come from `/System/Library/Frameworks/AppKit.framework/Resources/StandardKeyBinding.dict`,
overridable per user in `~/Library/KeyBindings/DefaultKeyBinding.dict` (documented,
https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/EventOverview/TextDefaultsBindings/TextDefaultsBindings.html);
the Control-key list in the table is from the file's well-known contents, not the page (inferred).
An app honours both by acting on the selector names `doCommandBySelector:` delivers (winit 0.31:
`standard_key_binding`; source-read). quire's keymap is VS Code `keybindings.json` data with
`key`/`mac`/`linux`/`win` fields and maps the logical command modifier to ⌘ or Ctrl
(source-read, `egui_quire/src/keymap.rs` 1–30). Recommendation (inferred): keep VS Code bindings for
the code editor; for plain fields on macOS, map `doCommandBySelector:` selectors to `EditCommand`s so
user `DefaultKeyBinding.dict` works.

### 2.4 Clipboard

- winit 0.30.13: none. egui 0.37 uses arboard 3.7.0 and smithay-clipboard 0.7.3 (cadatomic
  `Cargo.lock`).
- winit 0.31.0-beta.4: `ActiveEventLoop::clipboard`/`set_clipboard` through the data-transfer API on
  Wayland, X11, macOS, Windows, and `primary_selection_ext` on Wayland and X11 (changelog
  "0.31.0-beta.4 Added"; source-read `winit-core/src/data_transfer.rs`). Data is fetched
  asynchronously (`fetch_data_transfer` → `DataTransferReceived`), with typed formats (`TypeHint`).
- arboard 3.7.0: Windows, macOS, X11 by default; pure Wayland needs the `wayland-data-control`
  feature, which requires `ext-data-control-v1` or `wlr-data-control` and "won't function in a pure
  Wayland environment" without them — otherwise XWayland; primary selection via
  `LinuxClipboardKind::Primary` (documented, https://github.com/1Password/arboard README).
  smithay-clipboard uses the app's own `wl_data_device` and so works on every compositor (inferred from
  its design; egui uses it on Wayland).
- Recommendation: on winit 0.31 use winit's data transfer (it binds on the app's own connection);
  on 0.30 keep smithay-clipboard (Wayland) + arboard (others). quire-text already emits
  `SetPrimary/ClearPrimary` per `PrimaryRule`.

### 2.5 Drag and drop

- winit 0.30.13: receive files only (`DroppedFile`, `HoveredFile`, `HoveredFileCancelled`, one event
  per file; X11 XDND in `x11/dnd.rs`, Windows `drop_handler.rs`) (source-read).
- winit 0.31.0-beta.4: new data-transfer DnD (PR #4571, 2026-07-16): `DragEntered/DragPosition/
  DragDropped/DragLeft` with typed data and `set_valid_dnd_actions`; **drag source** via
  `ActiveEventLoop::start_drag(source_window, DataTransferSend, actions, icon)` with
  `OutgoingDragDropped/Canceled`, implemented on **macOS, Wayland, Windows** (not X11: `git grep 'fn
  start_drag'` finds appkit, wayland, win32 only). On Windows it runs the modal `DoDragDrop`
  (source-read, `winit-win32/src/event_loop.rs` ~601–640, `data_transfer.rs` 1,966 lines). The docs say
  internal drags must be recognised by workaround (only one drag at a time).
- GPUI (source-read): Windows has `IDropTarget` only (no `DoDragDrop`, i.e. no drag-out of text/files);
  macOS implements `NSDraggingSource` and `beginDraggingSessionWithItems`; Wayland
  `wl_data_device.start_drag`.
- Implications for tessel (inferred): `DoDragDrop` and `NSDraggingSession` run their own modal/tracking
  loops on the UI thread; tessel's compositor thread (§4 of the toolkit doc) keeps presenting, but
  event-time dispatch must continue from inside the OLE drag source callbacks
  (`IDropSource::QueryContinueDrag`, `GiveFeedback`). Text drag inside an editor never needs the OS
  path; drag *out* (file tree → Explorer, text → another app) does.

---

## 3. Accessibility

### 3.1 AccessKit now

- Versions (source-read, tags dated 2026-09-25): `accesskit 0.25.1`, `accesskit_consumer 0.39.1`,
  `accesskit_windows 0.35.1` (UIA), `accesskit_macos 0.27.1` (NSAccessibility),
  `accesskit_unix 0.24.0` + `accesskit_atspi_common 0.21.0` (AT-SPI over D-Bus), `accesskit_android 0.9.0`,
  `accesskit_ios 0.2.1`, `accesskit_winit 0.34.1`. cadatomic already locks these.
- **`accesskit_winit 0.34.1` depends on `winit = "0.30.5"`** (source-read,
  `adapters/winit/Cargo.toml`, also on `main`). On winit 0.31 tessel must use the platform adapters
  directly (`accesskit_windows::Adapter`/`SubclassingAdapter`, `accesskit_macos::Adapter`/
  `SubclassingAdapter`, `accesskit_unix::Adapter`), as GPUI does (source-read, Zed `Cargo.toml`:
  accesskit 0.24, accesskit_macos 0.26, accesskit_unix 0.22, accesskit_windows 0.34).
- **Model** (documented, `ARCHITECTURE.md`): `TreeUpdate { nodes, tree: Option<TreeInfo>, tree_id,
  focus }`; nodes not listed are unchanged; a listed node must carry all its properties; removing a
  child = updating its parent's children; focus must be sent with every update. Lazy activation:
  `ActivationHandler` requests the first full tree only when an AT connects.
- **Invalid updates panic** in `accesskit_consumer::Tree` (source-read, `accesskit_consumer/src/tree.rs`:
  ~12 `panic!`s — root not in node list, duplicate child, cannot graft the root tree, …). With
  `panic = "abort"` there is no recovery (Zed's experience, below). tessel needs a validating builder
  (debug assert + release repair) in front of the adapter.

### 3.2 Nested trees (egui island) — yes, since 0.23.0

- "Multiple tree support" landed in `c16eb80` (2026-01-10, #655), first tagged in `accesskit-v0.23.0`
  (source-read: `git tag --contains`). `TreeId(Uuid)`, `TreeId::ROOT` = nil UUID; a **graft node**
  in the parent tree sets `Node::tree_id(subtree)`; its child is the subtree's root; the graft node must
  exist before the first subtree update; the first subtree update must include `tree`; removing the
  graft removes the subtree; each subtree has its own `NodeId` space and its own `focus` (used when the
  graft node is focused); `ActionRequest` carries `target_tree` (source-read, `accesskit/src/lib.rs`
  700–760, 2538–2545, 3225–3330). Follow-ups: lookups by `(TreeId, LocalNodeId)` (#707, 2026-04-13),
  tree views on Windows (#698), Unix root index fix (#718), `tree_id` in the JSON schema (#811,
  2026-09-25).
- egui 0.37.0 always emits `tree_id: TreeId::ROOT`, a full node list and `tree: Some(TreeInfo)` every
  pass, and only accepts actions whose `target_tree == ROOT` (source-read, `egui/src/context.rs`
  ~3180–3203, `input_state/mod.rs` 891/909, `memory/mod.rs` 615, `text_edit/builder.rs` 1279).
- **Grafting recipe** (inferred): tessel owns the adapter; for each egui island it creates a graft
  node, rewrites egui's `TreeUpdate.tree_id` to a per-island `TreeId(Uuid::new_v4())`, and rewrites
  incoming `ActionRequest.target_tree` back to `ROOT` before handing it to egui. Set tessel's `focus`
  to the graft node while the island has keyboard focus. Cost: egui resends its whole tree every pass,
  so throttle island updates to frames where the island repainted.

### 3.3 Large trees and virtualised lists

- Updates are incremental by construction (only changed nodes); the consumer recently avoided redundant
  node copies (`2dfdd7b`, 2026-08-11) and made traversals iterative (`01f24bd`) (source-read, git log).
  GPUI nevertheless rebuilds a full `TreeUpdate` every frame and lets the adapter diff (source-read,
  `gpui/src/window/a11y.rs` module docs). tessel's plan (only dirty nodes, from scene props, toolkit doc
  §9.5) is the cheaper shape.
- No UIA `ItemContainerPattern`/`VirtualizedItemPattern` equivalent in AccessKit (source-read: no match
  for "virtualiz" in `accesskit/src/lib.rs` or the Windows adapter); `size_of_set`/`position_in_set`
  exist and map to UIA properties. So virtual lists expose realised rows plus set size/position, and
  scrolling realises more (inferred). Same for the code editor: visible lines only (quire already does).

### 3.4 Text editing roles per platform

- AccessKit text model: `TextRun` children with `value`, `character_lengths` (UTF-8 bytes per
  selectable character), `character_positions`/`character_widths`, `word_starts`, `text_direction`,
  `previous_on_line`/`next_on_line`; `TextSelection { anchor, focus }` as `TextPosition { node,
  character_index }`; actions `SetTextSelection`, `ReplaceSelectedText`, `SetValue`, scroll actions
  (source-read, `accesskit/src/lib.rs`).
- Windows: UIA `TextPattern` (`ITextProvider`, `ITextRangeProvider`: `RangeFromPoint`,
  `DocumentRange`, selection) when `supports_text_ranges()` (source-read,
  `adapters/windows/src/node.rs` ~1432–1470, `text.rs`).
- macOS: `accessibilitySelectedTextRange`, `accessibilityInsertionPointLineNumber`,
  `accessibilityRangeForPosition:`, `accessibilityStringForRange:`, `accessibilityFrameForRange:`,
  `setAccessibilitySelectedTextRange:` (source-read, `adapters/macos/src/node.rs` 781–1031).
- Linux: AT-SPI `Text` (`caret_offset`, `set_caret_offset`, `character_extents`, …) and
  `EditableText` (source-read, `adapters/atspi-common/src/node.rs` 496, 1425–1513;
  `adapters/unix/src/atspi/interfaces/editable_text.rs`).
- So caret, selection, text ranges and editing are supported on all three desktop adapters; the toolkit's
  job is accurate per-character geometry from tessel-text and stable run ids across edits (inferred).

### 3.5 What GPUI hit (documented, Zed PRs)

- #56065 "gpui: Accesskit support" merged 2026-05-27: GPUI-only (Zed widgets later). Testers: VoiceOver
  on macOS 26.5 reached only the top bar until app switch; NVDA/Narrator on Windows 11 saw the window
  title but content stayed inaccessible until focus left and returned; JAWS saw controls but no initial
  focus (https://github.com/zed-industries/zed/pull/56065).
- #57954 `Application::inaccessible()` (2026-05-28): nightly panicked because a focus-node bug produced
  invalid `TreeUpdate`s; "many macOS users apparently triggered the activation callback" (i.e. without
  a screen reader), and `panic=abort` defeated `catch_unwind`; AccessKit was switched off until stable.
- #57885 (2026-05-28): panic setting a focus id not in the tree (screen reader on, open menu, Tab);
  fixed with "only insert focus IDs that are known" plus defensive `repair_tree_update`
  (https://github.com/zed-industries/zed/pull/57885).
- #64143 (2026-09-20): release the macOS adapter when the window drops. #65186 (open, 2026-10-05):
  `SubclassingAdapter` swaps and restores the NSView class and corrupts AppKit's Touch Bar KVO, crashing
  on window close on Touch Bar Macs; fix = plain `Adapter` on GPUI's own NSView implementing the
  accessibility methods (https://github.com/zed-industries/zed/pull/65186).
- Lessons (inferred): validate before submit; never panic on a11y in release; with winit you are forced
  into `SubclassingAdapter` on macOS (you don't own the NSView) — another reason for the own macOS view;
  initial-focus quirks need a test on each screen reader.

### 3.6 Testing tools

- macOS: VoiceOver (Cmd+F5), Accessibility Inspector (Xcode). Linux: Orca, Accerciser, KDE
  Accessibility Inspector; Accerciser needs the AT-SPI bus enabled (`busctl --user set-property
  org.a11y.Bus /org/a11y/bus org.a11y.Status IsEnabled b true`) (documented,
  `accesskit/README-APPLICATION-DEVELOPERS.md`, which also warns inspectors mislead).
- Windows: NVDA, Narrator, JAWS (used in Zed #56065 testing), Accessibility Insights for Windows and
  Inspect.exe (Windows SDK) for the UIA tree (inferred: standard tools, pages not fetched here).
- Automation: AccessKit's own consumer can serve as the test oracle (tessel-test reads the same tree
  the adapters get) (inferred).

---

## 4. Other OS integration

| Item | winit 0.30.13 | winit 0.31.0-beta.4 | GPUI / others | Recommended | Size (inferred) |
|---|---|---|---|---|---|
| macOS menubar | default app menu only | same (`winit-appkit/src/menu.rs`) | GPUI builds its own `NSMenu` (`gpui_macos/src/platform.rs`) | `muda` 0.21.2 (AppKit backend always on; Windows needs `TranslateAcceleratorW` in the loop; Linux needs GTK) — use it for macOS only | small (~300 glue) |
| Windows/Linux menus | – | – | Zed draws its own menu bar | custom-drawn in tessel (same as VS Code/Zed) | part of widget set |
| Context menus / popovers outside the window | own windows | **native popups** `WindowType::Popup` + `WindowPositioner` on Wayland, Windows, macOS (beta.3) | GPUI popups | tessel popups as winit popup windows (Wayland needs `xdg_popup` to escape the main surface) | medium |
| File dialogs | – | – | GPUI: `ashpd` FileChooser portal on Linux, native elsewhere | `rfd` 0.17.2 (default features `xdg-portal`, `wayland`; GTK3 optional; IFileDialog, NSOpenPanel; async API) | small |
| Cursors | `CursorIcon` + RGBA custom | + animated and URL sources; macOS 15 resize cursors | – | winit | none |
| Wayland decorations | CSD via `sctk-adwaita` when no `xdg-decoration` | same (`winit-wayland` features `csd-adwaita*`) | GPUI binds `zxdg_decoration_manager_v1`, else draws its own | draw our own titlebar (Mutter has no xdg-decoration; KWin, Sway, Hyprland, COSMIC etc. do — https://wayland.app/protocols/xdg-decoration-unstable-v1) | medium |
| Windows custom titlebar + snap layouts | `with_decorations(false)`, `drag_window`, `drag_resize_window`; no `WM_NCHITTEST` hook | same; `with_msg_hook` sees thread messages only; `BackdropType` (Mica etc.) | GPUI returns `HTCAPTION`/`HTMAXBUTTON`/… from `WM_NCHITTEST` (`gpui_windows/src/events.rs` 107, 968–1141) | `SetWindowSubclass` on winit's HWND in phase 1, own WndProc later; MS: return `HTMAXBUTTON` for the maximize button, min width ≤500 epx (documented, https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/ui/apply-snap-layout-menu) | ~400 |
| Per-monitor DPI | per-monitor v2, `ScaleFactorChanged` | same + Win11 DPI-drag fix | – | winit | none |
| Fractional scaling (Wayland) | `wp_fractional_scale` + viewporter | same | GPUI binds it | winit; supported by Mutter, KWin, Sway, Hyprland, COSMIC… not Weston (https://wayland.app/protocols/fractional-scale-v1) | none |
| Dark mode | `Theme` + `ThemeChanged` on Windows/macOS; **unsupported on X11/Wayland** | same | GPUI reads `org.freedesktop.appearance color-scheme` via `ashpd` | winit on Win/mac; `ashpd` Settings portal on Linux (`color-scheme`, `accent-color (ddd)`, `contrast`, `reduced-motion`; documented, https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Settings.html) | small |
| Accent colour | – | – | – | Windows `UISettings.GetColorValue(Accent)` (WinRT) or DWM colorisation; macOS `NSColor.controlAccentColor`; Linux portal | small (~150) |
| Notifications | – | – | Zed uses `objc2-user-notifications` (UNUserNotificationCenter) on macOS | `notify-rust` 4.18.2: XDG D-Bus (zbus), Windows via `tauri-winrt-notification`, macOS default backend is the deprecated `NSUserNotificationCenter` (UN backend is a preview feature) (documented, notify-rust README) — prefer UNUserNotificationCenter on macOS | small |
| Tab tear-off, cross-window drag | implicit capture: Windows `SetCapture` on press; macOS/X11 drag events stay with the source window | same; no global pointer query API | – | Win/mac/X11: works with winit (pointer events outside the window during a press; inferred). Wayland: no global coordinates; needs `xdg-toplevel-drag-v1` attached to a `wl_data_source` drag (staging; KWin 6.7, Mutter 51, Jay only — https://wayland.app/protocols/xdg-toplevel-drag-v1). winit binds neither the protocol nor exposes its `wl_data_source` → own Wayland code or the planned winit fork (register D7) | medium (~600) |
| Touchpad gestures | pinch/rotate/smart-magnify/pan on macOS/iOS only (`TouchpadPressure` on macOS) | + Wayland pinch, rotation, pan, hold (`zwp_pointer_gestures`); `ScrollSource` (wheel/finger/momentum) on macOS and Wayland; **Windows and X11: no gestures** (Windows sends pinch as Ctrl+wheel) | GPUI Windows uses **DirectManipulation** for precision-touchpad pinch/pan (`gpui_windows/src/direct_manipulation.rs`, 361 lines) | DirectManipulation viewport on Windows (needs the HWND and `DM_POINTERHITTEST` → own WndProc or subclass) | ~400 |
| Pen | – (touch only) | `PointerKind::TabletTool` with force/tilt on **Windows (WM_POINTER + `GetPointerPenInfo`) and Wayland (tablet-v2)**, iOS Apple Pencil; not macOS, not X11 (source-read: 0 matches in `winit-appkit`/`winit-x11`) | – | winit on Win/Wayland; macOS tablet events (`NSEventSubtypeTabletPoint`) in the own backend | small–medium |
| Pointer history | – | `GetPointerFrameInfoHistory` used for touch/pen on Windows | – | toolkit doc §7.3 wants `GetMouseMovePointsEx`/coalesced events → own code | small |

---

## 5. Capability matrix and phasing

| Capability | winit now (0.31.0-beta.4 unless noted) | Needs own platform code | Crate |
|---|---|---|---|
| Basic IME (preedit/commit, one cursor rect) | yes, all desktop | – | winit |
| IME purpose/hints | Wayland only | Win (TSF InputScope), mac | – |
| Surrounding text / delete-surrounding | Wayland only | Win (TSF / IMR_DOCUMENTFEED), mac (`attributedSubstring`) | – |
| Reconversion | no | Win (`WM_IME_REQUEST` or TSF), mac (`selectedRange`+`replacementRange`) | – |
| Per-character IME geometry | no (one rect) | Win TSF `GetTextExt` / `IMR_QUERYCHARPOSITION`; mac `firstRect` per range | – |
| macOS dictation, Character Viewer, hanja | broken | full NSTextInputClient | – |
| Cancel/commit composition from the app | no (toggle hack) | `ImmNotifyIME`, `discardMarkedText`, text-input disable | – |
| macOS standard key bindings | `standard_key_binding` selector hook | – | winit |
| Clipboard + primary selection | yes (beta.4) | – | winit 0.31; on 0.30 arboard + smithay-clipboard |
| DnD receive (typed) | yes | – | winit 0.31 |
| DnD source | macOS, Wayland, Windows (modal) | X11 | winit 0.31 |
| Accessibility | adapters, not via winit | own NSView avoids `SubclassingAdapter` | accesskit 0.25.1 platform adapters (accesskit_winit pins winit 0.30) |
| egui subtree | – | – | accesskit ≥0.23 `TreeId` + tree_id rewrite |
| macOS menubar | default only | – | muda (macOS) |
| Menus/context menus | native popup windows | – | custom-drawn |
| File dialogs | – | – | rfd (portal) |
| Win custom titlebar + snap layouts | no `WM_NCHITTEST` | subclass/own WndProc | – |
| Wayland CSD | sctk-adwaita fallback | own titlebar (tessel-drawn) | – |
| DPI / fractional scale | yes | – | winit |
| Dark mode | Win/mac | Linux | ashpd Settings |
| Accent colour | no | small per-OS | ashpd on Linux |
| Notifications | no | – | notify-rust (+UN on macOS) |
| Tear-off | Win/mac/X11 via capture | Wayland `xdg-toplevel-drag` | – |
| Pinch/rotate | mac, Wayland | Windows DirectManipulation; X11 XI2.4 | – |
| Pen | Windows, Wayland | macOS, X11 | – |

Recommended phasing (inferred):

1. **Phase 1 (winit 0.31 + crates).** Text input client trait in tessel-core; IME through
   `request_ime_update` with IME-first key routing; winit clipboard/primary/DnD; AccessKit platform
   adapters directly with a validating builder and egui islands grafted by `TreeId`; rfd, muda (macOS),
   ashpd (theme), notify-rust; custom-drawn menus in native popups; tessel-drawn titlebars. Two small
   HWND subclass hooks right away: `WM_NCHITTEST` (snap layouts) and `WM_IME_REQUEST` (reconversion,
   per-char candidate position).
2. **Phase 2 (own Windows backend).** TSF text store (+IMM32 fallback, per-TIP quirks à la Firefox),
   DirectManipulation, pointer history, own WndProc (titlebar, DComp), OLE drag source run so event-time
   dispatch continues during `DoDragDrop`.
3. **Phase 3 (own Wayland backend).** text-input-v3 v2 (actions, language, preedit hints),
   `xdg-toplevel-drag-v1` tear-off, presentation feedback/subsurfaces (toolkit doc §8), cursor-shape,
   portal integration.
4. **Phase 4 (own macOS backend) — consider moving earlier.** Full `NSTextInputClient` (dictation,
   Character Viewer, reconversion, hanja, press-and-hold), own NSView for AccessKit's plain `Adapter`
   (avoids Zed #65186), tablet events, `NSDraggingSource`. The macOS IME gaps are the most user-visible
   failures winit leaves (dictation drops text, #4666); if macOS users matter before phase 4, carry a
   small winit patch that forwards `selectedRange`/`attributedSubstring`/`firstRect`/
   `characterIndexForPoint` queries to an app callback.
