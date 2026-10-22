# ARCH-WIDGETS: svelters' own widget system on egui-as-engine

Lane OW0-design (research, docs only). Owner: svelters-agent. Date: 2026-10-22.

User decisions this design implements (2026-10-22):

- "We want complete control of rendering and behavior, with our composition framework to handle
  abstractions and behaviors to reduce development work."
- svelters draws every widget itself. egui stays only as the **engine**: painter (epaint shapes,
  tessellation, textures), raw input, text shaping and galleys, layers/Areas/viewports, AccessKit
  plumbing. No `egui::Button`, `TextEdit`, `Checkbox`, `Slider`, `DragValue`, `ComboBox`,
  `ScrollArea`, `CollapsingHeader`, `Popup`, `menu`, `Modal` or `Window`, no `egui_extras`
  pickers; egui's `Visuals`/`Style` do not drive our look.
- Text fields use a shared editing core extracted from quire: **TC0 = `quire-text`** (quire repo,
  no egui or svelters dependency, wasm32-clean, quire-core depends on it, svelters pins it by git
  rev; macbook-cadatomic extracts it). cadatomic's requirement: quire's Document/EditorView split
  stays (a shared buffer, selections per view).

> **Push order.** §4 (the TC0 interface) was pushed first so the TC0 extraction can start; the
> rest of the document follows in the same file.

---

## 4. Text input on TC0 (`quire-text`)

### 4.1 Division of work

| Concern | TC0 (`quire-text`) | svelters (`text-field`/`text-area` kinds, `behaviors/text_input.rs`) | quire-core (stays) |
|---|---|---|---|
| Buffer, versions, change sets, anchors | **yes** | — | uses TC0 |
| Selections (multi-range, primary), per view | **yes** | single range in single-line fields (TC0 allows many) | multi-cursor |
| Transactions, origin, grouping hints | **yes** | builds them from input | builds them from `EditorCommand` |
| Undo history and grouping rules (pluggable) | **yes** (Monaco, GTK, Qt rule sets) | picks the rule set from `native::Policy` | Monaco set |
| Word rules (pluggable) | **yes** (VS Code classifier, Pango, Qt: FX1's `native/words.rs` moves here) | picks from `Policy` | VS Code |
| Grapheme motion, line motion within logical lines | **yes** | — | uses TC0 |
| Visual-line motion (Up/Down across wraps, PageUp/Down, Home/End of a wrapped row) | asks a `VisualLines` trait the caller implements | implements it on the galley | implements it on its wrap map |
| IME composition model (preedit text, active range, commit, cancel) | **yes** (on the view) | maps `egui::Event::Ime`, paints preedit | same |
| GTK hex entry (Ctrl+Shift+U) | as a preedit source (`Preedit::Hex`) | key binding per `Policy` | — |
| Clipboard, PRIMARY | emits **host requests** (`SetClipboard`, `ReadClipboard`, `SetPrimary`) | runs them through EV3's host clipboard (DX2) and `primary.rs` | same |
| Single-line rules (line breaks in paste, char limit, input filter, secret mode) | **yes** (`FieldRules`) | sets them from props | — |
| Tokens, diagnostics, folds, find, syntax | — | — | **stays in quire-core** |
| Key map (which chord is which command) | — (commands only) | per `Policy` (FX3's table) | its keymap |
| Painting: galley, selection highlight, caret and blink, placeholder, password bullets, preedit underline, scroll into view | — | **yes** | egui_quire |
| Hit-testing a point to an offset | — | galley `cursor_from_pos` | its layout |
| AccessKit text runs, caret and selection | offset conversions (byte ↔ char ↔ UTF-16) | builds the nodes (§5) | its own |
| Validation states, `commit`/`submit` signals, `bind:value` | — | **yes** (§2.4) | — |

TC0 is pure: no clock (callers pass `time_ms`), no threads, no I/O, no egui. Everything platform-
or toolkit-specific is a value the caller passes in (rule sets, `FieldRules`), so the GTK and Qt
reference behaviour measured by WG3/FX1/FX3 lives in TC0 as data and pure functions, and quire
gets it too.

### 4.2 The model: Document and View (cadatomic's requirement)

TC0 keeps quire's split. Names below are placeholders; the extractor (macbook-cadatomic) picks
final names, and quire-core keeps its `Document`/`EditorView` as wrappers around them.

```rust
/// Shared per buffer: text, version, history, recent changes (for rebasing and view sync).
pub struct TextDoc { /* text: Text, version: Version, history: History, recent: RecentChanges,
                        rules: DocRules */ }

/// One view of a TextDoc: selection, preedit, focus, selection_version, synced_at.
pub struct TextView { /* … */ }

impl TextDoc {
    pub fn new(text: &str, rules: DocRules) -> Self;
    pub fn text(&self) -> &Text;                    // backend-neutral read API (§4.3)
    pub fn version(&self) -> Version;
    pub fn apply(&mut self, tr: Transaction, view: &mut TextView) -> Result<Applied, Refusal>;
    pub fn apply_remote(&mut self, changes: ChangeSet, base: Version) -> Result<Applied, Refusal>;
    /// Replace everything from outside (a prop write). `ResetHistory` (GTK `set_text`,
    /// Qt `setText`: FX3) or `Undoable` (quire's reload, an app-level revert).
    pub fn replace_all(&mut self, text: &str, mode: ExternalSet) -> Applied;
    pub fn close_undo_group(&mut self) -> Option<UndoGroupClosed>;
    pub fn tick(&mut self, now_ms: u64) -> Option<UndoGroupClosed>;
    pub fn undo_mode(&self) -> UndoMode;            // Local | Host (quire CE3; app-level undo)
    pub fn can_undo(&self) -> bool;  pub fn can_redo(&self) -> bool;
    pub fn anchor(&self, pos: usize, bias: Bias) -> Anchor;
}

impl TextView {
    pub fn new(doc: &TextDoc) -> Self;
    pub fn selection(&self) -> &Selection;          // byte offsets, valid at synced_at()
    pub fn set_selection(&mut self, doc: &TextDoc, sel: Selection);
    pub fn selection_version(&self) -> u64;
    pub fn sync(&mut self, doc: &TextDoc) -> SyncOutcome;
    pub fn preedit(&self) -> Option<&Preedit>;
    pub fn composing(&self) -> bool;
    pub fn set_focused(&mut self, f: bool) -> bool; // closes the undo group on blur (Monaco, GTK)
}
```

A single-line field is one `TextDoc` + one `TextView`. A multi-view editor (quire) is one
`TextDoc` + N views. svelters never needs two views of one field, but nothing in the field code
assumes one view per doc: a "same value in two places" case (a property shown in an inspector and
a table cell) binds both fields to the same JS value, not to the same `TextDoc`.

### 4.3 The text backend stays behind an API (open, pending TC-EVAL)

`Text` exposes only: `len()`, `len_chars()`, `line_count()`, `line(i) -> TextSlice`,
`slice(range) -> TextSlice` (iterates `&str` chunks), `to_string()`, `char_at`,
`byte_to_char`/`char_to_byte`/`byte_to_utf16`, `line_of(byte)`, `line_start(i)`,
`grapheme_next(byte)`/`grapheme_prev(byte)`, and `as_str() -> Option<&str>` (Some when the
backend is contiguous: lets a single-line field hand egui one `&str` with no copy).

**Not decided here**: rope vs `String` vs promote-on-focus (a `String` that becomes a rope above a
size or on first multi-line edit). TC-EVAL (`research/tc-eval`) measures per-field memory,
creation cost for 10k fields and keystroke latency; the backend is chosen from its numbers.
Interface rules that keep every option open:

- no `ropey` (or other backend) type in TC0's public API;
- positions are UTF-8 byte offsets valid at a `Version` (as quire-core today); conversions are
  methods on `Text`;
- `TextDoc::new` must be cheap for a short string (no history allocation until the first edit,
  no recent-changes ring until the first transaction): svelters creates fields lazily (§4.6) but a
  10k-row editable table still creates one per edited cell.

### 4.4 Pluggable rules

```rust
pub struct DocRules {
    pub undo: UndoRules,            // Monaco | Gtk | Qt  (FX3's undo.rs models move here)
    pub undo_pause_ms: Option<u32>, // Monaco's pause rule; None for Gtk/Qt
    pub undo_mode: UndoMode,        // Local | Host
}

#[non_exhaustive]
pub enum UndoRules {
    /// quire-core's history.rs: word-boundary stops, cursor-jump stops, pause, non-typing = own group.
    Monaco,
    /// FX3 (fitted to GTK 4.24.1's golden, pin 4): one step per word, each deletion its own step,
    /// typing over a selection one closed step, a caret move ends merging; undoing a deleted
    /// selection restores it selected, caret at its start.
    Gtk,
    /// FX3 (QWidgetLineControl's command history, ported): a typing run one step, a BackSpace or
    /// Delete run one step, a selection removal plus the typing after it one step; separators at
    /// caret moves and pastes; Qt's internalUndo/internalRedo walk.
    Qt,
}

pub struct FieldRules {           // per view; svelters sets it from props and Policy
    pub lines: LineMode,          // Single { paste_breaks: Keep | Spaces | Strip } | Multi
    pub max_chars: Option<u32>,   // `char-limit` / `maxlength`
    pub secret: bool,             // password: word motions jump to the ends, no copy/cut requests
    pub read_only: bool,          // motions and copy only
    pub words: WordRules,         // VsCode(classifier) | Pango | Qt  (FX1's words.rs moves here)
    pub word_move_collapses: bool,// GTK: Ctrl+Arrow on a selection collapses to its edge
    pub backspace: DeleteUnit,    // Grapheme | CodePoint (Pango's backspace_deletes_character scripts)
    pub filter: Option<InputFilter>, // a pure fn(&str) -> Cow<str> (numeric fields, identifiers)
}
```

`WordRules` is an enum with an extension variant (`Custom(&'static dyn WordClassifier)`) so a
cadatomic field (an identifier field with `snake_case` as one word) can bring its own. Word
offsets are byte offsets (FX1's are char offsets today: the port converts).

### 4.5 Commands, input and host requests

TC0 owns a small, closed set of field commands (quire-core's `EditorCommand` stays the rich
VS Code-id set and lowers into these where they overlap):

```rust
#[non_exhaustive]
pub enum EditCommand {
    Insert(String, InputKind),          // Typing, Paste, Ime, Drop, …  (InputKind from quire)
    Move { unit: Unit, dir: Dir, extend: bool },   // Unit: Grapheme | Word | WordEnd | LineEdge
                                                    //       | VisualLine | Page | DocEdge
    Delete { unit: Unit, dir: Dir },    // BackSpace/Delete/Ctrl+BackSpace/Ctrl+Delete/Ctrl+U/K
    SelectAll, SelectWordAt(usize), SelectLineAt(usize), // double/triple click
    SetSelection(Selection),            // pointer drag, AccessKit SetTextSelection
    ReplaceSelection(String),           // AccessKit ReplaceSelectedText, drop
    Undo, Redo,
    Cut, Copy, Paste,                   // → host requests (Paste: ReadClipboard, then Insert)
    Preedit(PreeditUpdate),             // Start | Update { text, active } | Commit(String) | Cancel
    HexEntry(HexKey),                   // GTK Ctrl+Shift+U: digits, Space/Enter commit, Esc cancel
}

pub trait VisualLines {                 // implemented by the caller on its layout
    fn visual_line_of(&self, byte: usize, affinity: Affinity) -> VisualLine; // range + x of byte
    fn byte_at_x(&self, line: VisualLineIx, x: f32) -> usize;
    fn lines_per_page(&self) -> usize;
}

pub struct Executed {
    pub applied: Option<Applied>,       // text and/or selection changed
    pub requests: SmallVec<[HostRequest; 1]>, // SetClipboard(String) | ReadClipboard
                                         // | SetPrimary(String) | ClearPrimary | Beep
    pub reveal: Option<Reveal>,         // the caller scrolls the caret into view
    pub rejected: Option<Rejected>,     // MaxChars | Filtered | ReadOnly (svelters may flash)
}

impl TextDoc {
    pub fn execute(&mut self, view: &mut TextView, cmd: EditCommand, rules: &FieldRules,
                   layout: &dyn VisualLines, time_ms: u64) -> Executed;
}
```

- **Sticky x** (the goal column for Up/Down) lives on `TextView`, reset by any non-vertical motion.
- **Undo grouping inputs** come from `InputKind` plus the command (FX3's "what made the edit":
  typing, paste, BackSpace, Delete, word deletions, cut) and from caret moves executed through
  `Move`/`SetSelection`, so svelters no longer reconstructs them from a before/after text diff.
- **PRIMARY**: TC0 emits `SetPrimary` when a selection made by the user becomes non-empty and
  `ClearPrimary` when it collapses *if* `rules` ask (GTK); svelters decides ownership across
  fields (`one_selection_owner`) because that spans views.

### 4.6 What svelters keeps (paint, caret, scroll) and how a field lives

- **Lazy session.** *Decided (design).* An unfocused, never-edited field holds no `TextDoc`: it
  paints its `value` prop (a `String` the scene already stores) through the galley cache. On
  focus, pointer press or an AccessKit text action, the kind creates `TextDoc + TextView` in an
  ext column (`TEXT_SESSION: ExtKey<TextSession>`); it keeps it while the node lives so undo
  survives blur and refocus (GTK and Qt keep a field's undo stack across focus changes). This
  holds whatever backend TC-EVAL picks, and bounds the 10k-field case to the fields touched.
- **Prop sync.** `value` from JS that equals the session's text is a no-op (the echo of our own
  `bind:` write). Any other value is `replace_all(v, ExternalSet::ResetHistory)` and clamps the
  selection (FX3: a text set from outside starts a new history).
- **Paint** (§3): one galley per field, rebuilt when text, font or wrap width change. Selection
  highlight from `::selection` (`background-color`, `color`; focused vs unfocused per Policy
  `keep_selection_unfocused`), caret as a 1-px (DPI-snapped) rect in `caret-color`, blink driven
  by `anim.rs` at the platform rate (`native::Policy::caret_blink_ms`, new; 0 = no blink; GTK
  stops blinking after `gtk-cursor-blink-timeout`, 10 s, which also saves frames), preedit underlined (active range thicker),
  `::placeholder` text when empty, the platform's mask character (U+25CF on GTK and Qt Fusion) for `secret`.
- **Scroll into view.** Single-line: horizontal offset kept so the caret stays in the content
  box; how far it jumps when the caret leaves is a WG3 measurement to add (a pin), not a guess. Multi-line: the `text-area` is a scrollable
  (§1.2.9) and asks it to reveal the caret rect.
- **Keys.** The text-input behaviour maps `egui::Event::{Text, Key, Paste, Copy, Cut, Ime}` to
  `EditCommand`s through the platform key table (FX3's chords, Mac's Option/Command motions), and
  claims them for EV3 (`consume_key`, `composing`) exactly as FX3 does today.
- **IME**: `egui::Event::Ime(Enabled|Preedit|Commit|Disabled)` → `PreeditUpdate`; the caret rect
  goes to `PlatformOutput::ime` every frame the field is focused (`IMEOutput { rect, cursor_rect }`).
- **Clipboard** goes through EV3's host clipboard (DX2 `ClipboardBackend`), which keeps CRLF from
  a Ctrl+V paste (FX3's open paste-multiline checkpoint: svelters reads the clipboard itself
  instead of egui-winit's `Event::Paste`).

### 4.7 TC0 acceptance (for the extraction lane)

1. quire-core builds on `quire-text` with its tests unchanged (cadatomic's gate).
2. FX1's word tests and FX3's undo replays (`native/undo.rs` unit tests: pin 4 against the GTK and
   Qt goldens) move into `quire-text` and pass there; svelters' `egui_measure` keeps its floors
   once the svelters field runs on TC0 (lane OW-TEXT, §7).
3. `cargo check --target wasm32-unknown-unknown -p quire-text`; no egui, svelters, ropey (public)
   or std-only clock in the dependency tree.
4. `TextDoc::new("short")` allocates no history or ring (a test with a counting allocator), so
   TC-EVAL's creation numbers hold.

---

*(Sections 0–3 and 5–8 follow in the next push.)*
