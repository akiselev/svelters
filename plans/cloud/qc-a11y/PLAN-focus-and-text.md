# Plan: real focus and text in skelter's accessibility tree

> Copied 2026-10-26 from cadatomic `docs/research/2026-10-26-skelter-focus-and-text-a11y.md` (branch `claude/determined-gates-c0yj5i`) so svelters lanes can read it. **Superseded in one point by the user (2026-10-26):** fork AccessKit for the Linux expanded state (`akiselev/accesskit`), no upstream PR; §2.5's "file it upstream" and "don't fork" no longer apply. That fork is not in this wave.

Date 2026-10-26. Read-only research. Code read: svelters `cd2adc4f` (the pin), cadatomic `93af4c7`
(qc1-probe merge), crates accesskit 0.25.1, accesskit_consumer 0.39.1, accesskit_atspi_common 0.21.0,
accesskit_unix 0.24.0, egui 0.37.0 from `~/.cargo/registry`, and AccessKit `main` at `f302622`
(2026-10-25) for what upstream has since.

Scope: focus ownership and how it reaches `TreeUpdate.focus`, text fields' Text, EditableText and
value, bounds under UI zoom, and menu roles and expanded state. Selection, canvas semantic nodes and
the designer belong to the semantic-object-model plan. §7 says where that plan and this one meet.

---

## 1. Root causes

### 1.1 AT-SPI reports only the window frame as focused

**(a) Two focus owners.** skelter-core holds a focus model per window: `WindowState.focus`, Tab order,
scopes and restore (`skelter-core/src/input/focus.rs`). cadatomic runs a second one beside it. It
keeps a separate "keyboard focus" flag on each egui island:

- At startup the main island is marked focused while skelter's focus is `None`
  (`cadatomic-app/src/skelter/document.rs:545`).
- Any press that no region takes sets the island focused again (`document.rs:1207-1208`), including
  a press on a native chrome button.
- The island node's kind `cadatomic-island` is not focusable (`skelter/host.rs:63-70`). So skelter
  never sees the island as focused.

The AccessKit projection takes its focus from skelter alone:
`self.focused(window).filter(known).unwrap_or(root)` (`skelter-core/src/a11y.rs:192`). Whenever egui
"has" the keyboard, the tree's focus is the root, and AT-SPI reports the frame.

**(b) The graft can't help.** accesskit_consumer can follow focus into an egui subtree:
`compute_effective_focus` follows a focused graft node into its subtree's focus
(`accesskit_consumer/src/tree.rs:101-121`). That only works when the host focuses the graft node,
and cadatomic never does.

**(c) Programmatic focus on a node that isn't shown yet is dropped.** The palette calls
`doc.focus(field)` at `palette.rs:279` while its backdrop still carries `display: none`. The
`classed(...)` call that clears it comes about 10 lines later.

Even in the right order the call would fail:

- `can_focus` checks `doc.layout[i].style.display` and the computed `visibility_hidden` through
  `hidden_by_ancestor` (`focus.rs:33-48`).
- Those values are written only by the deferred restyle (`style/cascade.rs:716`, assignment at
  `:821`), not by `set_style` (`document.rs:1154`).
- `set_focus` then returns without a word (`focus.rs:110-114`). Nothing owns focus.

**(d) No focus fixup.** When the palette closes, its backdrop goes back to `display: none`
(`palette.rs:281`), but the hidden field keeps focus:

- skelter clears focus for a node that becomes disabled (`document.rs:1085`), but not for one that
  becomes hidden.
- It gives focus back only when a dialog scope leaves the window (`scopes_closed`,
  `document.rs:727`).
- `autofocus` is declared (`common.rs:114`), and svelters-skelter aliases it, but skelter-core never
  reads it.
- `open_dialog` is called only from tests. svelters-egui's EV3 had both behaviours: autofocus, and a
  dialog that takes focus when it appears and gives it back on hide or inert
  (`svelters-egui/src/focus.rs:1-20, 340-352`). The skelter port lost them.

**(e) Tab never moves in the shell.** skelter does implement Tab (`dispatch.rs:634-640`), but the
key never reaches it:

- While the main island's flag is set, `ShellDocument::dispatch` offers every key to the island
  (`document.rs:1211`).
- The island's input translator consumes every key while it is "focused"
  (`skelter-egui-island/src/input.rs:139-143`).
- egui's own Tab cycles inside the island, with no way out.

**(f) Window bounds start empty.** The AT-SPI adapter gets its window bounds only on `Moved` and
`Resized` (`skelter-platform/src/a11y.rs:95-109`). Before the first one, and with no WM, extents
can be stale. This is also the gui-qc README's "stale bounds until next input".

### 1.2 Text fields expose neither Text nor Value

How accesskit_atspi_common decides the interfaces:

- It adds `Text` iff `supports_text_ranges()`, and `EditableText` iff that and `is_text_input()`
  (`atspi_common/src/node.rs:456-458, 476-512`).
- `supports_text_ranges()` asks for a text-input, Label, Document or Terminal role **and at least one
  `Role::TextRun` child** (`accesskit_consumer/src/text.rs:1402-1406`).
- AT-SPI `Value` is numeric only (`current_value()` → `numeric_value()`, `node.rs:589`). A text
  field never gets it, whatever `value` holds.

`TextField::a11y` (`skelter-widgets/src/text/field.rs:662-684`) sets only the role, `value`,
placeholder and actions:

- no TextRun children, no `text_selection`, so no caret;
- `Action::SetTextSelection` is not handled (`:686-695`);
- typing doesn't re-send the node. `TextEdit::finish` asks only for paint (`text/kind.rs:198-219`),
  `EventCx::request_paint` marks only `PAINT` (`cx.rs:833`), and the frame collects a11y nodes only
  on `A11Y|CHILDREN` (`frame.rs:352`).
- The value goes stale after a key edit until some prop write (`ChangeClass::Paint` marks `A11Y`,
  `document.rs:943`) or a focus move re-sends it.

The fix is mostly in place already. skelter has **virtual children**: kind-built AccessKit nodes that
go out with their owner (`a11y.rs:12-16, 28-50`, `A11yCx::add_virtual_child`, `cx.rs:911`). They are
exactly what TextRuns need.

### 1.3 Other findings

- **Zoom (and HiDPI).**
  - skelter runs the document at `os_scale × zoom` (`skelter/src/app.rs:661, 688-697`).
  - AccessKit wants bounds in physical window pixels: atspi_common adds the window's inner origin,
    `node.rs:545-567`.
  - Every bound skelter sends is in document points (`a11y.rs:104-112`), and no scale sits on the
    root.
  - The island graft's doc assumes "the host's root already scales points to pixels"
    (`skelter-egui-island/src/a11y.rs:6-9`). It doesn't.
  - This is wrong under any zoom, and under any OS scale other than 1 (not seen yet only because
    Xvfb runs at 1).
- **Menus.**
  - cadatomic draws its title-bar and context menus as `box` and `button` nodes
    (`chrome.rs:23-24, 994-1064`) because the runner doesn't draw skelter's overlay stack yet.
    skelter-widgets' `menu-bar`, `menu` and `menu-item` kinds already carry the right roles
    (`menu.rs:846-861, 1156-1200`).
  - The common `role` prop has no `menubar`, `menuitemcheckbox` or `menuitemradio`
    (`common.rs:40-76`), and there are no `aria-haspopup`, `aria-checked`, `aria-modal`,
    `aria-activedescendant` or `aria-controls` props.
  - **AccessKit's AT-SPI adapter never sets `STATE_EXPANDABLE` or `STATE_EXPANDED`.** It isn't in
    0.21.0 (`node.rs:301-385`), nor on upstream `main` at f302622 (2026-10-25).
  - UIA has ExpandCollapse (`adapters/windows/src/node.rs:714-720`), and macOS maps expanded. On
    Linux the expanded state therefore needs an upstream change. Orca checks Expandable before
    Expanded (MuseScore report, below).
- **Fixed in cadatomic, keep them fixed:** the prop-id bug behind the disabled state, and icon
  glyphs exposed as labels. Add regression checks for both in §5.

---

## 2. Design: focus

### 2.1 One owner: skelter-core

skelter-core's `WindowState.focus` is the only keyboard focus. Everything else derives from it:
islands, Svelte slots, panes and regions.

- **Islands are focusable nodes.** Give `cadatomic-island` `focusable: true`, as svelters' own
  `egui-island` kind already is (`skelter-egui-island/src/host.rs:60, 89, 111-113`). The island's
  egui flag follows `Event::FocusIn` and `Event::FocusOut` on its node.
- Delete cadatomic's direct `island.set_focused` calls (`document.rs:545, 1182, 1197, 1208, 1228`,
  `regions.rs:107, 124`). A press on an island focuses its node through skelter's normal
  press-to-focus path (`dispatch.rs:410-424`).
- **Keys go to the focus owner.** `ShellDocument::dispatch` routes a key by `doc.focused(window)`
  only: an island node goes to its island, anything else to `core.dispatch`. It no longer checks
  "island.focused()".
- **Tab between stops belongs to skelter.** A focused island gets Tab only when egui wants it, so the
  island needs a tab-out protocol:
  - The island answers "consumed" for Tab only if egui moved focus to another widget *without
    wrapping*. Compare egui's `memory().focused()` before and after, plus a first/last check through
    egui's AccessKit order.
  - Otherwise skelter moves to the next stop.
  - Entering an island by Tab sends egui a synthetic Tab, or Shift+Tab for backwards, so the first or
    last egui widget takes focus. This is the iframe model, and Chromium views'
    `FocusTraversable` for embedded widgets.
  - Until that lands, an island is one Tab stop. Ctrl+Tab or F6 cycles panes (§2.6).
- **Focus reaches AccessKit through the graft.** With the island node focused, `TreeUpdate.focus` is
  the graft node, and accesskit_consumer follows into egui's focus.
  - Guard: accesskit_consumer **panics** if a focused graft node's subtree doesn't exist
    (`tree.rs:111-117`).
  - `Document::a11y_update` therefore needs a "subtree sent" bit for each graft node. skelter-app
    sets it after it sends the island's first subtree update, and `validate` maps focus to the
    island node's parent until then.
  - Order the sends as today: host update first, then subtrees (`skelter/src/app.rs:489-495`). The
    first focus on a graft goes out in the frame *after* its subtree.

### 2.2 Focus requests, fixup and restore (skelter-core)

1. **Deferred requests.** `Document::focus(node, reason)`:
   - If `can_focus` holds against the current computed style, focus now, as today.
   - If it fails only because style is stale (the node or an ancestor is `DESC_STYLE`- or
     `STYLE`-dirty), store `WindowState.pending_focus = (node, reason)`.
   - After `restyle(window)` in the frame, before layout and before `a11y_update`, apply the pending
     request if `can_focus` now holds. Drop it if not.
   - Programmatic `focus()` on a node that will never be focusable stays a no-op, as in the DOM.
   - Considered and rejected: the alternative is to flush style synchronously inside `focus()`, as
     browsers do for `element.focus()`. That ties focus to a style flush mid-transaction and makes
     JS `focus()` costly. The deferred form keeps the transaction cheap, and the result is the same
     within one frame.
   - `blur`, `focus` and `focusin` still fire in the frame that applies the request. Document that
     for JS: "focus applies at the next style pass". Delivering it in the same task as svelters-egui
     did is not possible without a flush.
2. **Focus fixup** (the HTML "focus fixup rule"). After restyle:
   - If the focused node no longer passes `can_focus` (hidden, `display: none`, concealed, inert,
     disabled, removed), focus goes to the top of that window's **restore stack** if it can be
     focused, else to `None`.
   - Generalise `WindowState.restore` (`focus.rs:209-232`): an entry is pushed whenever a `dialog`
     scope or an overlay takes focus, and popped when the scope stops being *available*. That covers
     removed (today's case), hidden, inert, and an overlay closing (`toplayer.rs:142-171` already
     reports `restore_focus`).
3. **Scopes that appear.** In the same post-restyle step, a `focus-scope=dialog` node that turned
   available since the last frame runs `open_dialog`:
   - it pushes a restore entry;
   - it focuses its `autofocus` descendant, else the first tab stop.
   
   Outside a dialog, an `autofocus` node focuses once when it first becomes available. This ports
   svelters-egui EV3 (`focus.rs:340-352`). Track "available last frame" for each scope node in
   `WindowState`, so the check costs one pass over the scope nodes, not the whole tree.
4. **Initial focus.**
   - A window's first frame focuses its root's `autofocus` node or, failing that, nothing (the DOM).
   - cadatomic sets `autofocus` on its main working surface. With a project open that is the main
     island; with none, the start page's first action. AT then has a focus from frame one.
   - Gate the change of default behind an explicit prop, so other skelter apps keep the old default.

### 2.3 Tab order

Keep `tab_sequence` (`focus.rs:70-87`): tree order with positive `tabindex` first, one stop for each
peer group, wrapping inside the nearest `contain` or `dialog` scope.

Add the following:

- **Layout order versus tree order.** cadatomic's chrome is built in visual order, so tree order is
  right for it. Absolutely positioned popups (palette, menus) sit in their own `dialog` scopes, so
  they don't interleave. Don't build Flutter's reading-order policy now. Note it as the hook if
  Svelte layouts diverge.
- **Islands and virtual focus.** A node can be one stop whose inside is managed elsewhere: an
  island, or later a canvas with semantic nodes. Represent it as `FocusTarget { node, sub:
  Option<u32> }`. `sub` is a virtual-child index, an island's internal focus, or a semantic object
  id from the semantic model. `TreeUpdate.focus` then becomes the graft node or the virtual child id
  (`virtual_id(owner, sub)`, `a11y.rs:39`). This is the seam where canvas semantic nodes become
  focusable without being skelter nodes.
- **`aria-activedescendant`.** A common prop (an anchor name) that the projection turns into
  `set_active_descendant`. Note that accesskit_consumer's `focus()` *returns the active descendant*
  (`tree.rs:538-543`), so AT focus lands on the option, not on the input: the combobox pattern. Use
  it for the palette's selected row and for menus opened from the keyboard.

### 2.4 How focus reaches `TreeUpdate.focus`

`a11y_update` (`a11y.rs:148-199`) chooses the focus in this order:

1. `pending` is never reported; only applied focus counts.
2. `FocusTarget` resolves to an AccessKit id: a host node, `virtual_id`, or a graft node with a
   subtree that has been sent.
3. If that id is not in the known set, use the nearest ancestor that is, not the root. That keeps a
   focused but hidden-for-a11y node from collapsing to the frame.
4. The root only when nothing is focused.

Mark the old and the new focus `Dirty::A11Y` as today (`focus.rs:142, 170`), so an update always goes
out when focus moves.

### 2.5 Menus

Interim, in cadatomic:

- Put `role=menubar` on the title-bar menu strip, `role=menuitem` plus `aria-haspopup=menu` and
  `aria-expanded` on its buttons, and `role=menu` on each `menu_level` box.
- Items get `role=menuitem`, or `menuitemcheckbox` with `aria-checked` where `checked` is `Some`.
  Separators get `role=separator`.
- Each menu level is a `focus-scope=dialog`, so it takes focus when it opens and gives it back on
  close. Arrow keys follow WAI-ARIA APG menubar.
- Drop the drawn mark ("✓ ", "▸") from the accessible name: set `aria-label` to the bare label.

Final: switch to skelter-widgets `menu-bar` once the runner draws the overlay stack. Its kinds
already carry roles, roving focus and restore. The interim props then go away.

In skelter-core: add the missing ROLES entries (`menubar`, `menuitemcheckbox`, `menuitemradio`,
`alertdialog`, `searchbox`, `spinbutton`, `tooltip`, `none`/`presentation`). Add the common props
`aria-haspopup`, `aria-checked`, `aria-modal`, `aria-activedescendant` and `aria-controls`, applied
in `a11y_node` after the kind's node (`a11y.rs:67-101`).

Linux expanded state: file it upstream with AccessKit. Map `is_expanded().is_some()` to `Expandable`
and `Some(true)` to `Expanded` in `atspi-common` `state()`, and emit `StateChanged(Expanded)` in
`notify_state_changes`. Filing upstream is public, so it goes to the coordinator and then the user.
Until it lands, gq checks expanded through the probe, which reads the AccessKit node, not AT-SPI.
Don't fork for this (cadatomic rule: a fork only when a release blocks the work).

### 2.6 Pane cycling (optional, after the core work)

F6 and Shift+F6 move between the top-level regions: title bar, ribbon, dock panes, status bar. This
is Chromium's `AccessiblePaneView` and the Windows convention. Each region root gets `focus-scope` set
to a new `group` value. That value isn't a trap: Tab leaves it, F6 jumps between groups, and each
group remembers its last focus. Ship it as a cadatomic command through the one command path.

---

## 3. Design: text

### 3.1 TextRun children for `text-edit` (skelter-widgets)

`TextField::a11y` gains an `&mut A11yCx` path (`kind.rs:378-381` passes `cx.out` today) and emits
virtual children:

- **Runs.**
  - One `Role::TextRun` per laid-out row: `TextGeometry::line(i)` and `LineInfo.range`
    (`text/shim.rs:35-55`). A row longer than 255 characters is split, because `word_starts` is
    `u8`. Copy egui's scheme (`egui-0.37.0/src/text_selection/accesskit_text.rs`): chunks linked
    with `previous_on_line` and `next_on_line`.
  - A row ending in a line feed appends `"\n"` with width 0.
  - **An empty field still emits one empty run.** Otherwise `supports_text_ranges()` is false and
    Text disappears exactly when an agent focuses an empty search field.
- **Per run:**
  - `value` is the run's text: the *shown* text, so a password field gives mask characters.
  - `character_lengths` is the UTF-8 length of each grapheme.
  - `character_positions` and `character_widths` come from `geom.caret_rect(byte)` at each grapheme
    boundary, relative to the run's start. Add a `TextGeometry::glyph_extents(range)` to skelter-text
    later if this costs too much. It's O(n) per change, so compute it in `prepare` when the layout
    changes, not in `a11y()`.
  - `word_starts` comes from the editor's word-boundary function, so AT and Ctrl+Arrow agree.
  - `text_direction` comes from the run's bidi level; LTR until skelter-text exposes it.
  - `bounds` is the row rect in node space, offset by `content.min - scroll`. Clip it to the content
    box, as egui does with its rows.
- **Selection.** `set_text_selection` maps the editor's anchor and head bytes, as *shown* bytes
  (`Shown::to_shown`), to `(run id, character index)`. A caret at a chunk boundary belongs to the
  end of the earlier chunk, as egui does. With a preedit, report the caret at the preedit's active
  end; the preedit text is in the runs because they're built from `shown`.
- **Value.** Keep `set_value` for single-line fields:
  - UIA's ValuePattern reads it (`windows/src/node.rs:592-596`), and so does macOS `AXValue`;
    accesskit_consumer computes it from the runs only when it's unset (`node.rs:852-867`).
  - For multiline, leave `value` unset. Consumers read the runs, and a large document isn't sent
    twice.
  - Passwords: send the real text masked by the consumer (`is_password_input`), or keep the shown
    masks as today. Keep today's choice and test that no clear text leaves the process.
- **Actions.** Handle `SetTextSelection`, mapping `TextPosition` back to a byte: the owner's
  `Event::A11yChild` path is for actions on runs, and `SetTextSelection` arrives on the field. Keep
  `SetValue` and `ReplaceSelectedText`. Add `ScrollIntoView` for runs if needed later.
- **Re-send on change.**
  - Add `EventCx::request_a11y()`, mirroring `PropCx` at `cx.rs:247`.
  - `TextEdit::finish` calls it when the text, selection, preedit, scroll or layout changed. The
    field already tracks these as `c.text`, `c.selection`, `c.preedit` and `stale` (`field.rs:280-300`).
  - A caret blink must not mark it.
  - Selection events: accesskit_atspi_common emits `TextCaretMoved` and `TextSelectionChanged` only
    when the node is focused and the raw selection changed (`adapter.rs:197-245`). It emits
    `TextInserted` and `TextRemoved` from run diffs (`:121-183`). Both come for free once runs and
    selection are sent.
- **Read-only, labels and multiline.** `multiline` sets `MultilineTextInput`, which AT-SPI shows as
  `STATE_MULTI_LINE`. `read_only` stays. Optional later: a selectable `label` (`LABEL_SELECTABLE`)
  with runs gives `Label` nodes the Text interface. Low priority, because AT-SPI reads labels' names.
- **Limits.**
  - Virtual children are capped at 1024 per owner (`a11y.rs:33-36`). For a multiline field past
    that, send only the rows in view plus a margin, as Chromium's virtualised editors do.
  - quire, the code editor, needs its own approach and isn't covered here.

### 3.2 Svelte inputs

svelters-skelter maps `<input>` and `<textarea>` to the same `text-edit` kind, so they get §3.1
without extra work. `tabindex`, `autofocus` and `accessible-name` are already aliased
(`svelters-skelter/src/kinds.rs:131-137`); §2.2 makes `autofocus` work.

### 3.3 Zoom and scale on bounds

In `a11y_node`, for the root (`parent == None`, `a11y.rs:106-109`), use
`rel = Affine::scale(window.scale) * world`. `window.scale` is already `os × zoom` (`app.rs:661`).

When the scale changes, `resize_window` (`document.rs:494-502`) marks the root `A11Y`. Only the root
re-sends, because children's transforms are relative.

Fix whatever else assumed point-space bounds:

- skelter-test's `A11yTree` bounds ("window points through every transform", `skelter-test/src/a11y.rs:1-8`)
  must divide by the root scale, or skip it, to stay in points.
- cadatomic's probe already skips a subtree root's transform and multiplies by `scale` itself
  (`probe.rs:13-22, 602-605`). Check that it skips the host root's new scale too.
- At adapter creation, call `set_root_window_bounds` once with the window's current geometry, not
  only on `Moved` or `Resized` (`skelter-platform/src/a11y.rs:95-109`).

---

## 4. Platform notes

Windows and macOS need a real machine; they wait in `REVISIT.md`.

- **Linux (accesskit_unix 0.24):**
  - Focus events come from `focus_moved` and only while the window is host-focused
    (`update_window_focus_state`, `atspi_common/src/adapter.rs:324-342, 541`). Without a WM, X11
    may never send `Focused(true)`, which is why gq's `--wm` (openbox) is the baseline.
  - `Text` and `EditableText` follow from §3.1. `Value` stays absent for text, which is correct.
  - Expanded needs the upstream change (§2.5).
- **Windows (UIA, accesskit_windows 0.35):**
  - TextPattern needs text ranges (`is_text_pattern_supported` = `supports_text_ranges`), and
    ValuePattern needs `has_value`.
  - The adapter raises `UIA_AutomationFocusChangedEventId` from `update_if_active(...).raise()`,
    which skelter already calls (`skelter-platform/src/a11y.rs:79-81`).
  - ExpandCollapse works today.
  - To check with Narrator and NVDA plus Accessibility Insights: Tab order, focus after a popup
    closes, caret announcements, and bounds at 125%/150% scale × zoom.
- **macOS (NSAccessibility, accesskit_macos 0.27):**
  - `AXValue` comes from `value()` (`macos/src/node.rs:344-365`). `AXSelectedTextRange` and the
    parameterised text attributes need runs.
  - Focus follows `update_view_focus_state`, already wired (`skelter-platform/src/a11y.rs:89-94`).
  - With the OS menu bar (`set_menu_bar`), cadatomic's title-bar menus don't exist; check that the
    native menus' expanded state comes from AppKit.
  - To check with VoiceOver: rotor text navigation, and caret echo while typing.

---

## 5. Tests

All of them wait on frames and events, never on wall-clock time.

**skelter-core unit tests** (`crates/skelter-core/src/tests/focus.rs`):

- focus requested under a `display: none` ancestor whose style changes in the same transaction
  applies after the next frame's restyle, and `TreeUpdate.focus` names the field;
- hiding the focused node's container moves focus to the restore entry, or `None`;
- a `dialog` scope that appears focuses its `autofocus` node, and focus returns to the opener when it
  hides;
- the initial frame with an `autofocus` node focuses it;
- `a11y_update` never names an unsent graft node: focus falls back to its parent and moves to the
  graft in the frame after the subtree is sent;
- the root transform equals `scale(window.scale)` and changes when zoom changes.

**AccessKit-consumer tests** (new helper in skelter-test, dev-dependencies `accesskit_consumer` and,
on Linux, `accesskit_atspi_common`):

- Fold the frame's `TreeUpdate`s into a real `accesskit_consumer::Tree`, built with
  `is_host_focused = true`.
- Feed the same updates to an `accesskit_atspi_common::Adapter` whose `AdapterCallback` records
  `register_interfaces` and `emit_event`. This is AT-SPI's own logic without D-Bus, and it catches
  "Text not exported" in CI rather than only under Xvfb.
- Assertions:
  - **Tab order:** `h.key_press(Tab)` repeated over the shell fixture gives `tree.state().focus()`
    ids equal to the expected sequence; Shift+Tab gives the reverse; it wraps inside a dialog scope.
  - **Focus restore:** open the palette fixture, then Escape. The recorded events contain
    `StateChanged(Focused, true)` for the field, then for the opener.
  - **Text:** a focused `text-edit` with "hello wörld" has interfaces ⊇ {Text, EditableText};
    `document_range().text()` equals the text; after `h.type_text("!")` and Shift+Left,
    `text_selection()` covers "!", and events include `TextInserted` and `TextSelectionChanged`.
  - An empty field still has Text, with `characterCount` 0.
  - A 600-character line splits into 3 linked runs.
  - A password field's runs and value hold no clear text.
  - `SetTextSelection` moves the editor's selection.
- **Bounds:** at zoom 1.25, the extents from atspi_common's `extents(CoordType::Window)` of a button
  equal the renderer's world rect × 1.25.

**cadatomic kittest** (`crates/cadatomic-app/tests/`):

- shell Tab moves from the title bar into the ribbon, then into the dock island and out of it;
- the palette opens with the field focused and closes with focus back on the opener;
- a title-bar menu carries `menubar`, `menuitem`, `expanded=true` while open, and `menuitemcheckbox`
  where checked;
- regressions for the two fixed bugs: an icon glyph node has role generic with no private-use
  characters in its name, and a disabled drawn button has `disabled` in AccessKit.

**gq / AT-SPI checks** (a QC script under `tools/gui-qc/`, run by agent QC lanes, not by `check.sh`):

- `gq focus` (AT-SPI) and `gq focus --probe` name the same node after each step: start-up,
  click a ribbon button, Tab ×5, Ctrl+Shift+P, type, Escape;
- `gq text` with no target, for the focused field, returns the typed text through `queryText()`
  (AT-SPI), not only the probe;
- `gq a11y-bounds` reports 0 differences at zoom 1.0 and 1.25 (with a seeded profile).

Add a `gq atspi-text` mode that prints `characterCount`, `caretOffset` and `getSelection(0)`.

---

## 6. Work breakdown

Sizes: S ≤ ½ day, M ≈ 1 day, L ≈ 2–3 days. svelters lanes run on `ci/<lane>` branches under
svelters' rules; cadatomic lanes on `claude/b<N>-<lane>` branches. Order: wave 1, then a svelters pin
bump, then wave 2.

**Wave 1, svelters, two lanes in parallel**

| Step | What | Crate | Size |
|---|---|---|---|
| F1 | Deferred focus requests + fixup after restyle + generalised restore stack | skelter-core `input/focus.rs`, `frame.rs`, `window.rs` | M |
| F2 | Dialog scope on appear + `autofocus` + initial focus (opt-in) | skelter-core | M |
| F3 | `FocusTarget` (node + sub); graft-sent guard; ancestor fallback in `a11y_update`; root scale transform; initial window bounds | skelter-core `a11y.rs`, skelter-platform `a11y.rs`, skelter-test bounds | M |
| F4 | ROLES additions; `aria-haspopup`, `aria-checked`, `aria-modal`, `aria-activedescendant`, `aria-controls` | skelter-core `common.rs`, `a11y.rs`; svelters-skelter tier0 aliases | S |
| F5 | Island tab-out protocol (`focus_entered(dir)`, Tab consumed only when egui moved inside without wrapping) | skelter-egui-island | M |
| T1 | TextRun virtual children, selection, word starts, chunking, empty run, `SetTextSelection`, `EventCx::request_a11y` | skelter-widgets `text/field.rs`, `text/kind.rs`; skelter-core `cx.rs` | L |
| Q1 | Consumer- and atspi_common-backed test helper + the tests in §5 | skelter-test | M |

Lane split: **lane focus** takes F1–F5. **Lane text** takes T1, plus Q1's text half. Q1's focus
half goes with the focus lane.

**Pin bump.** The coordinator moves cadatomic's svelters pin in its own commit.

**Wave 2, cadatomic, one lane (shell-a11y)**

| Step | What | Size |
|---|---|---|
| C1 | `cadatomic-island` focusable; island focus derived from FocusIn/FocusOut; remove the seven direct `set_focused` calls; key routing by focus owner (`document.rs` 1040–1215, `regions.rs`) | M |
| C2 | Initial focus (`autofocus` on the main surface or start page) | S |
| C3 | Palette: `focus-scope=dialog` on the panel (or the overlay stack), focus via F1, active descendant for the selected row, `role=listbox` and `option` | S |
| C4 | Menus: the interim role and expanded props, dialog scope for each level, APG arrow keys, clean `aria-label`s | M |
| C5 | kittests in §5; gq QC script + `atspi-text` | M |
| C6 (later) | F6 pane groups; switch menus to skelter-widgets `menu-bar` when the overlay stack draws | M |

**Upstream (coordinator → user, because it is public):** an AccessKit PR mapping
Expandable/Expanded in `adapters/atspi-common` state and events.

**REVISIT.md:** Windows and macOS checks (§4).

---

## 7. Where this meets the semantic-object-model plan

- **The projection.** Today `a11y_node` builds each node from `Kind::a11y` plus common props
  (`a11y.rs:56-137`). Later its input is the semantic model's node: role, name, states, value and
  text ranges, actions. `Kind::a11y` becomes one *provider* into the model, as do canvas semantic
  objects and islands.
- Keep three things in one place so that switch is mechanical:
  - the *policy* this plan adds: focus target resolution (§2.4), root scale (§3.3), ARIA-prop
    overrides (§2.5);
  - TextRun construction (§3.1), which should sit behind a small `TextRanges` description that the
    model can carry, not only `accesskit::Node`s;
  - `FocusTarget.sub` (§2.3), which should be the model's object id for canvas objects, so focusing
    a sketch entity and reporting it through AccessKit both name model ids.
- **Focus versus selection.** The semantic plan owns selection. This plan owns keyboard focus. The
  link is `aria-activedescendant` and `FocusTarget.sub`: focus can sit on a model object without a
  skelter node.
- **The QC probe.** `probe.focus` and `probe.text` fold `TreeUpdate`s today
  (`cadatomic-app/src/skelter/probe.rs:268-317`, `A11yFold`). When the model lands, the probe reads
  it directly. The gq checks in §5 stay valid because they compare AT-SPI (the projection's output)
  with the probe (the model).

---

## 8. Risks

1. **Panic on graft focus** if the guard in §2.1 is wrong (`accesskit_consumer/src/tree.rs:111-117`).
   This is a release crash. `validate` must repair it, with a test.
2. **Tab trapped inside egui,** or egui losing Tab it needs (a code field that indents with Tab). The
   tab-out rule must respect egui's `lock_focus`.
3. **Deferred focus versus JS expectations.** Svelte code that reads `document.activeElement` right
   after `focus()` sees the old value. svelters-egui behaved differently. Document it, and test
   `onMount` + `focus()`.
4. **Active descendant changes what AT reports as focused,** so gq's "focused field" checks must look
   at the focus owner as well.
5. **Performance.** Runs re-sent on every keystroke cost O(line length) in positions. Large multiline
   fields hit the 1024-virtual-child cap. Measure under the real app; timings in the cloud are
   "cloud, not a gate".
6. **Root scale breaks consumers** that assumed point-space AccessKit bounds: skelter-test's queries,
   cadatomic tours (`tour/generic/skelter.rs` `A11yFold`), the probe. They all need updating in the
   same lane.
7. **The Linux expanded state depends on upstream timing.**
8. **The initial-focus default** could change other skelter apps. Keep it opt-in.

---

## 9. Prior art (retrieved 2026-10-26)

- **egui 0.37.0, `text_selection/accesskit_text.rs`** (local crate source; docs at
  https://doc.servo.org/egui/text_selection/accesskit_text/index.html): one TextRun per row, chunks of
  255 with `previous_on_line` and `next_on_line`, selection mapping, the line-feed character. This is
  the model for §3.1. egui's AccessKit focus is the widget focus id, and islands inherit it through
  the graft.
- **AccessKit main** at f302622 (2026-10-25), https://github.com/AccessKit/accesskit
  (`adapters/atspi-common`): no Expandable or Expanded mapping. Windows has ExpandCollapse. Nested
  trees and the graft focus are in `accesskit_consumer/src/tree.rs`.
- **Orca and expandable** (MuseScore, 2020): https://musescore.org/en/node/305102. Orca needs
  Expandable before it presents Expanded.
- **GPUI / Zed:** tab focus with `FocusHandle::tab_index`/`tab_stop` and
  `window.focus_next`/`focus_previous` (#33008),
  https://git.secluded.site/zed/commit/caa4b529e4b130006990a31b4908e4252aa96fe4. The first-focus fix
  (#35247) is https://git.secluded.site/zed/commit/8f952f1b5812abe9fa88b5f60d5df1660c295c1a: Tab with
  no focus picks the first stop, as `next_focus` does.
- **Flutter:** `FocusTraversalGroup` and `FocusTraversalPolicy`, with reading-order versus
  widget-order policies, https://api.flutter.dev/flutter/widgets/FocusTraversalGroup-class.html and
  https://api.flutter.dev/flutter/widgets/ReadingOrderTraversalPolicy-class.html. `FocusNode`, where
  a request is applied at the end of the frame (the model for §2.2's deferred request),
  https://api.flutter.dev/flutter/widgets/FocusNode-class.html.
- **Chromium views:**
  - `FocusManager` stores and restores focus and owns traversal,
    https://gitcode.com/openharmony-tpc/chromium_src/blob/114_trunk/ui/views/focus/focus_manager.h.
  - `AccessiblePaneView` traps focus in a pane, the F6 model,
    https://chromium.googlesource.com/chromium/src/+/lkgr/ui/views/accessible_pane_view.h.
  - `ViewAccessibility` with virtual views (`AXVirtualView`) for drawn children that have no View,
    the model for `FocusTarget.sub`,
    https://chromium.googlesource.com/chromium/src/+/a3dafb8/ui/views/accessibility/view_accessibility.h.
- **Slint `FocusScope`** (`focus()`, `clear-focus()`, `forward-focus`; a disabled scope leaves the Tab
  chain), https://releases.slint.dev/1.9.0/docs/slint/reference/keyboard-input/focusscope.
- **Xilem/Masonry:** Masonry builds its AccessKit tree from widgets' `accessibility()` and produces
  text through Parley's AccessKit support (TextRuns from layout). Not verified in source this
  session; see https://docs.rs/masonry/latest/masonry/ and https://docs.rs/parley/latest/parley/.
- **HTML:**
  - the focus fixup rule,
    https://html.spec.whatwg.org/multipage/interaction.html#focus-fixup-rule;
  - dialog focusing steps (autofocus, else the first focusable),
    https://html.spec.whatwg.org/multipage/interactive-elements.html#dialog-focusing-steps;
  - WAI-ARIA APG menubar, https://www.w3.org/WAI/ARIA/apg/patterns/menubar/.
- **svelters-egui EV3** (in repo, `crates/svelters-egui/src/focus.rs`): this project's own earlier
  autofocus, dialog-on-appear and popup-restore semantics, which the skelter port is to match.
