# Lane TEXT: text fields expose Text, EditableText, selection and value

Repository svelters, branch `ci/qa11y-text` from `origin/claude/determined-gates-c0yj5i`. `BRIEF.md` in this
folder applies. Read `PLAN-focus-and-text.md` §1.2, §3.1, §3.2, §5 (the text tests), §6, §8 (risk 5), and egui
0.37.0's `src/text_selection/accesskit_text.rs` (the scheme to copy).

## Step T1 (plan §3.1)

- `TextField::a11y` gets an `&mut A11yCx` path and emits `Role::TextRun` virtual children: one per laid-out row,
  rows over 255 characters split into chunks linked with `previous_on_line`/`next_on_line`, a trailing `"\n"`
  of width 0, and **one empty run for an empty field**.
- Per run: `value` (the shown text: masks for a password), `character_lengths`, `character_positions` and
  `character_widths` (from `caret_rect` at grapheme boundaries, computed in `prepare` when the layout changes),
  `word_starts` from the editor's word-boundary function, `text_direction` (LTR until skelter-text exposes bidi
  levels), `bounds` clipped to the content box.
- Selection through `set_text_selection` (shown bytes → run and character; the chunk-boundary rule; preedit).
- `set_value` for single-line fields only; `MultilineTextInput` for multiline; `read_only` kept.
- Actions: `SetTextSelection` (TextPosition → byte), keep `SetValue` and `ReplaceSelectedText`.
- `EventCx::request_a11y()` in `skelter-core/src/cx.rs`; `TextEdit::finish` calls it when text, selection,
  preedit, scroll or layout changed, never for a caret blink.
- The 1024-virtual-children cap: for a long multiline field send the rows in view plus a margin.

## Tests (plan §5, the text ones)

Through lane focus's consumer/atspi_common helper (merge `origin/ci/qa11y-focus` once it has pushed it): Text and
EditableText interfaces on a focused field, `document_range().text()`, the selection after typing and
Shift+Left, `TextInserted` and `TextSelectionChanged` events, an empty field with Text and `characterCount` 0,
a 600-character line as 3 linked runs, no clear text from a password field, `SetTextSelection` moving the
editor's selection. Svelte `<input>`/`<textarea>` get the same through `text-edit` (§3.2): one test that says so.

## Not yours

Focus, roles, scale and island work are lane focus's. quire (the code editor) is out of scope.

**Record:** `plans/milestones/QA11Y-text.md`.
