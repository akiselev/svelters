# QA11Y-text: text fields expose Text, EditableText, selection and value

Lane TEXT of batch qc-a11y, wave 1 (`plans/cloud/qc-a11y/BRIEF.md`, `LANE-text.md`, plan
`PLAN-focus-and-text.md` §3.1, §3.2, §5). Branch `ci/qa11y-text`, from `origin/claude/determined-gates-c0yj5i`
(9e3781e5), with `origin/ci/qa11y-focus` 9ac6d4ab (the Q1 consumer helper) merged in. `origin/master`
(bb50d6c3) is already an ancestor.

## What landed

| Commit | What |
|---|---|
| 5b1b4d46 | T1: `text/runs.rs` (`TextRanges`), `TextField::a11y` with runs and selection, `a11y_action` with `SetTextSelection`, `TextEditor::word_starts`, `EventCx::request_a11y`, kind wiring |
| dde76497 | Field unit tests (runs, selection, password, multiline, `SetTextSelection`) |
| 00b0576b | Merge of `origin/ci/qa11y-focus` (consumer helper) |
| 055eba80 | `skelter-test/tests/text_a11y.rs`: §5's text tests through the consumer and AT-SPI |
| ed47e098 | svelters-skelter hosts `TextEdit` (§3.2; see deviations), with `tests/text_edit.rs` and a regenerated `skelter.json` |

T1 in detail (plan §3.1):

- **Runs.** `TextRanges::build` (`skelter-widgets/src/text/runs.rs`) cuts the *shown* text into one
  `Role::TextRun` per laid-out row (`TextGeometry::line`). Rows over 255 characters become chunks linked with
  `previous_on_line`/`next_on_line` (egui 0.37's scheme). A row ending in a line feed carries `"\n"` with width
  0, and an empty text gives one empty run. A character is a grapheme cluster (`unicode-segmentation`, a new
  direct dependency of skelter-widgets at the lockfile's 1.13.3). Positions and widths come from
  `caret_rect` at character boundaries (upstream at the row end), relative to the run's left edge. Word starts
  come from `TextEditor::word_starts`: VS Code's classifier runs, Pango/Qt via `WordRules::word_at`, none for a
  password. Direction is LTR. `TextRanges` is plain data, the "small `TextRanges` description" of plan §7.
- **When computed.** In `TextField::relayout`, whenever the layout is rebuilt (prepare and every event that
  relays out), not in `a11y()`. Without a layout (no text system) `a11y()` builds unplaced runs, so Text
  still exists.
- **Bounds.** The run box in node space (`content.min - scroll`), **not clipped** (see deviations).
- **Selection.** `set_text_selection` maps the editor's anchor and head (shown bytes) to `(run, character)`.
  A chunk boundary ends the earlier chunk, a soft wrap starts the next row, and with a preedit the caret is at
  the active clause's end.
- **Value.** Single-line keeps `value` (masks for a password); multiline leaves it unset.
  `MultilineTextInput`, `PasswordInput` and `read_only` are as before.
- **Actions.** `SetTextSelection` (positions back to document bytes), `SetValue` and `ReplaceSelectedText`.
  **These were never wired before:** `TextEdit::event` had no `Event::A11y` arm, so AT's `SetValue` did
  nothing. It now handles all three without needing a text system.
- **Re-send.** `EventCx::request_a11y()` (`skelter-core/src/cx.rs`, the only skelter-core change).
  `TextField` keeps an `a11y_changed` flag, set by text, selection, preedit, external change, scroll or
  relayout. A caret blink never goes through `after()`. `TextEdit::finish` calls `request_a11y` when the flag
  is set. In `prepare` the flag is cleared: core already marks `A11Y` there, through `mark_class` for prop
  writes and `layout/mod.rs:303` for layout changes.
- **The 1024 cap.** Past `MAX_VIRTUAL_CHILDREN` runs, a field sends the whole rows within one viewport above
  and below the view, cut at row boundaries. Selection ends outside the window clamp to its edges.

## Tests

Unit tests (`cargo test -p skelter-widgets --lib text::`, 42 passed locally):
`runs::rows_chunks_feeds_and_the_empty_text`, `runs::graphemes_are_characters_and_soft_wraps_start_the_next_row`,
`field::runs_carry_the_text_and_the_selection_names_them`,
`field::an_empty_field_has_one_empty_run_and_a_password_no_clear_text`,
`field::multiline_runs_end_in_line_feeds_and_leave_value_unset`.

Consumer and AT-SPI tests (`cargo test -p skelter-test --test text_a11y`, 5 passed locally). They use lane
focus's `Consumer` (`HarnessBuilder::with_consumer`) and run on a real `CoreDocument` with the TextSystem:

- `a_focused_field_has_text_ranges_that_follow_typing_and_selection`: `supports_text_ranges`,
  `document_range().text()` == "hello wörld", then after `type_text("!")` and Shift+Left the selection covers
  "!". On AT-SPI: Text and EditableText registered, `characterCount` 12, `text(0,-1)`, `selection(0)` ==
  (11, 12), and the events include `TextInserted("!")` and `TextSelectionChanged`.
- `an_empty_field_still_has_text`: Text interface, `characterCount` 0.
- `a_600_character_line_is_three_linked_runs`: 255/255/90, linked both ways.
- `a_password_field_sends_no_clear_text`: the field's value and every run hold masks only.
- `set_text_selection_moves_the_editor_selection`: AT-SPI `setSelection(0, 6, 11)` becomes AccessKit's
  `SetTextSelection`; delivered through `Harness::accesskit_action`, the consumer's selection reads "world".

svelters-skelter (`cargo test -p svelters-skelter --test text_edit --test spec`, and `--lib kinds::`, all passed
locally): `text_edit_is_skelter_widgets_and_sends_text_runs`, plus the flipped spec and kinds asserts.

**CI:** see the run ids in the coordinator report for the last push (filled in below once green).

**Not run locally:** everything else, including the full skelter-widgets, skelter-test and svelters-skelter
suites. `svelters-skelter --test host_document` fails locally only with "run `pnpm install` first" (no JS deps
in this container); CI runs it. Conformance was not run (no compiler change).

## Deviations from the plan

1. **§3.2 was wrong about the code.** svelters-skelter did not map Svelte text fields to skelter-widgets'
   `TextEdit`. The surface `text-edit` was a synthesized Placeholder (pinned by `kinds.rs` and
   `tests/spec.rs`), and there is no `<input>`/`<textarea>` alias: Svelte uses `<text-edit>`. I registered
   `TextEdit` in `register_hosted` (proposed to the coordinator, with a recommendation to proceed) and
   regenerated `metadata/semantics/skelter.json`: only `TextEdit.widget` changed, and `spec_hash` hashes ids,
   so it is unchanged. **This changes behaviour for svelters-skelter apps:** `<text-edit>` now edits and draws
   instead of being an inert box. The surface's `gained-focus`/`lost-focus` signals are not `TextEdit`'s own,
   so they resolve as the kinds map resolves any non-native signal (the common one if it exists, else
   none).
   Surface/core mismatches, noted rather than widened (coordinator, 2026-10-26): the core kind's
   `read-only` and `wrap` props have no surface prop, so Svelte can't set them yet; the surface's
   `request-focus` method and `gained-focus`/`lost-focus` signals are not `TextEdit`'s own; the core's
   `cancel` and `select` signals have no surface signal. Surface `char-limit`, `hint-text`,
   `multiline`, `password` and `text` match the core props by name and type.
2. **Run bounds are not clipped to the content box.** Clipping the box while `character_positions` stay
   relative to the run's start would misplace every character's extents, so runs scrolled out of the box
   keep their true position. A consumer can intersect them with the field's bounds.
3. **A wheel or fling scroll** that the field only learns about at its next event or prepare does not
   re-send runs by itself, so run bounds can be stale by that scroll until then. Fixing it needs the frame to
   mark `A11Y` on a self-scroll offset change (`frame.rs`, lane focus's file). Not done.
4. **Word starts** are computed for the whole text on each relayout. With VS Code rules this is O(n). With
   Pango/Qt it is O(words × line), because `word_at` copies the line. Fine for fields; measure under the
   real app (risk 5).

## Open questions (each with a recommendation)

- **Frame-side A11Y on a self-scroll change** (deviation 3). Recommendation: lane focus or a follow-up adds
  `Dirty::A11Y` where `frame.rs`/`scroll_fast` moves a self-scrolling node's offset. It is one line, but not
  in my files.
- **`Kind::prepare` cannot request a11y** (`PrepareCx` has no `request_a11y`, and `frame.rs` drops
  `cx.request`). Core marks cover today's cases. Recommendation: leave it until a kind needs it.

## Upstream (not filed: no public actions)

None needed for text. AccessKit's atspi_common already maps TextRun children to Text, EditableText and their
events.

## cadatomic wave 2 notes

- Any cadatomic code that registers `skelter_widgets::text::TextEdit` itself gets runs and selection with no
  change. cadatomic registries built through `svelters_skelter::kinds::register_builtin` now get `TextEdit`
  for `<text-edit>`. The hosted kind is the same `skelter_widgets::text::kind::TextEdit` with the same spec
  (`KindId::of("text-edit")`), so cadatomic's `registry_with_svelters` merge, which skips any kind whose id or
  name it already has, still skips it, and nothing is registered twice (the coordinator's constraint,
  2026-10-26). Only a host that registers its own `text-edit` *before* `register_builtin` would get
  `RegistryError::DuplicateKind`.
- `TextField::a11y` changed signature: `a11y(&mut Node, run_id: &dyn Fn(usize) -> NodeId) -> Vec<Node>` (the
  caller adds the runs as virtual children in order), and `a11y_action(&req, run_of) -> bool`. Wrappers of
  `TextField` outside `TextEdit` (none in svelters today) must follow.
- Multiline fields no longer send `value`. Probes and tours that read a textarea's text from `value` must read
  the runs instead (`document_range().text()`, or concatenate the TextRun children's values). Single-line
  fields still send `value`.
- `probe.text` / `A11yFold`: the field's node now has TextRun virtual children, with ids `1<<62 | owner<<10 |
  k` (`skelter_core::a11y::virtual_id`). A fold that counts children or walks every node sees them.
- The gq `atspi-text` mode (plan §5) can rely on `characterCount`, `caretOffset` and `getSelection(0)` for
  `text-edit` fields.
