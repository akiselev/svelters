# EV3+: keys and focus on egui (focus scopes, restore, key claims, edit commands, default/cancel)

Track: EVENTS (egui). Lane EV3+, branch `ci/ev3`, from master `3c7d67be`, merged with master `16da721d` (INT1's
widget library: WG1, WG2a overlays and menus, WG2b, WG4a, WG4b; DX1), then with master `dcfdcc71` (H3, DX2: edit
commands use DX2's clipboard), then with master `a52d437c` (LISTS: `virtual-list` rows get a key role). Design: `plans/research/QT-SURVEY.md`
§2.3 (`focus_info`, `shortcut_override`, `text_command`), §2.4 (claims), §3 row 1 (acceptance); ARCH-EMBED R11
(`consume_key`, `composing()`) and E-7 (no Enter or Escape to default/cancel buttons while composing). Builds on FX1
(`svelters_egui::native`) and TR2b (inert subtrees leave the tab order). egui `Memory` and `TextEdit` are not
forked: everything sits before the first svelters node of a pass, after each leaf, or behind host calls.

## As built

| Piece | Where | What |
|---|---|---|
| Focus router | `crates/svelters-egui/src/focus.rs` (new) | The record of what the walk painted (per viewport and pass, in egui temp data, keyed by egui ids so several embeds can share a context), `begin` (once per pass and viewport, before the first svelters node), the end-of-pass repaint hook, key claims, edit commands, default/cancel activation, the host API. |
| Walk hooks | `walk.rs` (+8 lines), `overlays.rs` (+1 line) | `focus::begin` at the top of `window_ui` and `slot_ui`; `record_scope` for a container, and for an open overlay before its children; `record` after each leaf or provider kind. |
| Leaf hooks | `leaf.rs` (+3 lines) | `text-edit`: a pending edit command is fed in before `TextEdit` runs; `button`: an Enter/Escape activation fires `clicked`. |
| Props | `kinds/builtin.rs`, `kinds/containers.rs`, `kinds/mod.rs`, `scene/node.rs` (`Common`), `scene/props.rs` | `autofocus` (every widget), `focus-scope` (`box`: `none`, `contain`, `dialog`), `default` and `cancel` (`button`). Stored in `Common` (additive fields). |
| Provider seam (R11) | `provider/mod.rs` | `Widget::consume_key(key, modifiers)` and `Widget::composing()`, both defaulted (additive), forwarded through `ErasedWidget`. |
| Host API | `focus.rs` (`EguiBackend`), `embed/mod.rs` (`Embed`; Cut/Copy/Paste through `Host::platform()`), `leaf.rs` (`focus::after_text` after a `text-edit`) | `focus_info(ctx) -> FocusInfo` (widget, node, kind, `text_input`, `multiline`, `composing`, scope; `Embed` adds the root), `shortcut_override(ctx, &KeyboardShortcut) -> bool`, `text_command(ctx, TextCommand) -> bool`, `composing(ctx)`; free `focus::consumed_keys(ctx)`, `focus::composing(ctx)`. |
| Generated | `metadata/semantics/egui.json`, `packages/native-runtime/types/egui.d.ts` | Regenerated (`SVELTERS_UPDATE=1 … --test spec`). Style tables unchanged (`gen_props.py --check`, `gen_kinds.py --check` clean: the new props are not layout props). |
| Tests | `focus.rs` (unit), `tests/focus_keys.rs`, `fixtures/motion-egui/src/FocusDialog.svelte` (loaded from source only, like `Flip.svelte`) | See Tests. |

### Behaviour

- **Focus scopes.** `focus-scope="contain"`: while focus is in the box, Tab and Shift+Tab cycle through its
  focusable nodes in tree order and wrap (egui's own traversal is cancelled for that key with
  `Memory::move_focus(FocusDirection::None)`). Nested scopes: a node belongs to its nearest scope. Outside every
  scope egui's traversal is unchanged.
- **Dialogs.** `focus-scope="dialog"` is `contain` plus: when it appears (first painted), focus goes to its
  `autofocus` node, else its first focusable node, unless focus is already inside; the widget focused before (the
  opener, which may be a host widget) is remembered. When it goes away (hidden, removed, its slot hidden, or its
  subtree inert in an outro) and nothing has focus (egui drops focus whose widget is gone), focus goes back to
  the opener. With nothing focused, Tab enters the topmost open dialog.
- **Overlays (WG2a).** `modal`, `inner-window` and `popup` are `dialog` scopes while open (`focus-scope="contain"` on
  one keeps traversal and drops the initial focus and restore). egui's `Modal` drops the focus of the layers below
  as it opens, so the opener is what had focus when the pass that showed it began.
- **Menus and other egui popups** (svelters' `menu-button`, `context-menu`, `combo-box`, or the host's menus): egui
  knows a popup is open only once it is shown in the pass, so the end-of-pass hook notes the focus from before it
  opened and, when it closes with nothing focused, focus goes back there at the next pass. This is also what
  makes an Edit-menu click in the host land back in the Svelte field.
- **`autofocus`** on any widget: focused once when the node first appears; again only after it left and came back.
- **Out of the chain**: disabled nodes and inert (TR2b, exiting) subtrees are not recorded, so Tab skips them and a
  scope whose box is inert counts as gone.
- **Key claims (R11 `consume_key`).** The focused node claims: a `text-edit` every key without Ctrl/Cmd/Alt except
  Tab, Escape and F-keys (Enter, Up/Down and PageUp/Down only when multi-line); with Ctrl/Cmd A, C, V, X, Y, Z,
  Left/Right, Home/End, BackSpace, Delete, Insert (Up/Down when multi-line); on macOS Option+Left/Right/BackSpace/
  Delete. `slider`, `range-slider` and `drag-value` claim the navigation keys; `button` and every other clickable
  leaf (check box, toggle, switch, radio, menu button, combo box) Space and Enter; a selectable `virtual-list`
  row (LS1) Up/Down/Home/End/PageUp/PageDown, Space and Enter (the rows are recorded by `kinds/virtuallist.rs`
  with the list as their node, and only the selected row, else the first drawn one, is a tab stop: a list is one
  stop in a scope); a provider kind what its `consume_key` says (WG5b's `tree-view` adopts it). After the focused node ran, its claimed key events are taken out of the pass's input (so a
  keymap or a menu shortcut later in the pass never sees them) and listed in `focus::consumed_keys(ctx)` for that
  pass. A host keymap that runs before the content asks `shortcut_override(ctx, chord)` (Qt's `ShortcutOverride`).
- **Composing (R11 `composing()`, E-7).** From a non-empty `Ime::Preedit` in the focused field until a commit, an
  empty preedit or a focus change: every key is the field's (`shortcut_override` is true for any chord), and
  Enter, Escape and Tab are the input method's: taken out before the field runs (else egui ends a one-line field's
  input on Enter), no default/cancel button, no traversal.
- **Edit commands.** `text_command(ctx, cmd)` with `Undo`, `Redo`, `Cut`, `Copy`, `Paste`, `Insert(text)`,
  `SelectAll`, `Delete` goes to the focused text field of that backend; when nothing has focus (a click on host
  chrome such as a Rust menu took it: egui surrenders focus on a click elsewhere), to the last text field that had
  focus, which is focused again. It is applied as the input it stands for when the field is next drawn
  (`Ctrl/Cmd+Z`, `Ctrl/Cmd+Shift+Z`, `Ctrl/Cmd+A`, `Delete`, `Event::Cut`, `Event::Copy`, `Event::Paste`), so
  egui's undoer and FX1's policy apply unchanged. **Clipboard (DX2):** `Embed::text_command` uses the host's
  clipboard, `Host::platform().clipboard`, the one `svelters:clipboard` uses. Paste reads its `text/plain` and
  inserts it. Cut and Copy write the copied text there, taken from egui's `OutputCommand::CopyText` after the field
  ran. They also leave that output command in place, so egui's clipboard (egui-winit, arboard) gets it too. DX2's
  desktop `SystemClipboard` is arboard as well, so both writes land in the same system clipboard, and a `text-edit`,
  JS and Rust panels see one clipboard. `EguiBackend::text_command` (no host) keeps egui's path: Paste is
  `ViewportCommand::RequestPaste`. `Insert` takes the host's text (another format chosen by the host). Returns
  false when another widget has focus or there is no field.
- **Default and cancel buttons.** Plain Enter activates the `default` button of the focused node's scope (its root
  when it is in none), plain Escape the `cancel` button: the key is taken out and the button fires `clicked` at its
  next paint. Not when the focused node takes the key itself (a button or check box, a multi-line field, a provider
  kind that claims it), while composing, while an egui popup is open, or when a host widget has focus. With nothing
  focused, the topmost open dialog's buttons answer. Escape keeps focus where it was (egui drops it). A disabled
  default or cancel button is not recorded, so it is not activated.
- **Idle.** `begin` acts on the last pass's record; an end-of-pass hook (registered once per context) asks for one
  more pass only when a dialog scope appeared or went away or an `autofocus` node appeared, so idle stays at 0
  frames (K-2).

### Acceptance (QT-SURVEY §3 row 1)

| Criterion | Test |
|---|---|
| Tab cycles a dialog and returns focus to the opener | `ev3_dialog_scope_focus_tab_default_cancel_restore`, `ev3_svelte_dialog_from_source` |
| Ctrl+Z in a `text-edit` reaches the field while the keymap stub sees `shortcut_override = true` (and not the key after) | `ev3_text_field_claims_keys_from_the_host_keymap` |
| Enter activates the default button except while composing | `ev3_dialog_scope_focus_tab_default_cancel_restore`, `ev3_composing_holds_enter_and_every_key` |
| Edit commands reach the focused field, and the last one after a click on host chrome | `ev3_text_commands_reach_the_focused_field` |
| Cut, Copy and Paste through the host's clipboard (DX2) | `ev3_edit_commands_use_the_host_clipboard` |
| A focused `virtual-list` row claims the navigation keys | `tests/vlist.rs` `a_focused_row_claims_the_navigation_keys` |
| Initial focus, `autofocus`, disabled nodes skipped | `ev3_autofocus_and_disabled_nodes_leave_the_chain` |
| A WG2a `modal` is a dialog scope (initial focus, Tab, Enter, restore) | `ev3_modal_overlay_is_a_dialog_scope` |
| Focus returns to the field after a menu closes | `ev3_focus_returns_after_a_menu_closes` |

## Degradations and notes

- **WG2a `action` shortcuts** (`consume_shortcut`, active while the menu's owner is drawn) are not yet told about
  claims: a menu bar drawn after the focused field never sees a claimed chord (it was taken out), but one drawn
  before it can still take Ctrl+Z from a field. Fix in `menus.rs`: skip a chord when
  `backend.shortcut_override(ctx, chord)` (follow-up, a few lines).
- **Restore needs the opener to exist** at restore time; a host widget opener is focused by id and egui drops the
  request if that widget is not drawn. The opener of a dialog that opens while nothing is focused is nothing.
- **Platform button order** (QW11's OK/Cancel order per platform) is not done: it is a layout convention for the
  dialog author or a later `dialog-buttons` kind.
- **QA8 Menu key / Shift+F10** to open a `context-menu` from the keyboard is not done.
- **Not done here:** component-scope `use:keys` (QT-SURVEY §2.4; an EV2b behaviour), `std:` standard key names.
- **FX1 follow-up found:** a GTK-policy field that loses focus while egui's `TextEdit` still holds an IME
  composition range repaints every frame (FX1 restores the "kept" selection, egui collapses the composition range
  again). EV3 avoids the case it caused (Enter during composition no longer reaches the field); a click away during
  a composition can still hit it. Fix belongs in `native::TextFocus::after` (do not keep a composition range).

## Verification

Local, after the last code change: `cargo check -p svelters-egui --all-targets` (clean apart from existing
warnings); nextest `-p svelters-egui --test focus_keys` (6/6), `--test native_focus` (2/2), `--lib ev3` (2/2),
`--test spec` (3/3, with `SVELTERS_UPDATE=1` to regenerate after the merge), and after the merge with master
`--test focus_keys --test menus --test widgets --test native_focus --test animator --test scene --test spec --lib`
(81/81) and `--test tier0 --test views --test wg4b --test widgets_panel --test menus_app` (28/31: the 3 others need
built `dist/` bundles, CI); `gen_props.py --check`. `--test queries` and the gallery apps need the built `dist/`
bundles (CI). `gen_kinds.py` (GTK/Qt tags) needs a display: not run. Not run locally: the full workspace, conformance, the
other backends, the JS suites (CI).

After merging master `dcfdcc71` (H3, DX2; no conflicts) and routing the clipboard: `cargo check -p svelters-egui
-p svelters-host --all-targets` (clean); nextest `-p svelters-egui --test focus_keys` (11/11), `--lib ev3` (2/2),
`--test clipboard` (DX2, 3/3), `--test spec` (3/3, current), `-p svelters-host platform` (4/4); `gen_props.py
--check`.

After merging master `a52d437c` (LISTS; a conflict in the `P` keys of `kinds/mod.rs`, both sides kept; `egui.json`
and `egui.d.ts` regenerated, unchanged from the auto-merge; `gen_kinds.py --check` under `tools/dev/headless` and
`gen_props.py --check` clean): `cargo check -p svelters-egui --all-targets` (clean); nextest `--test focus_keys`
(11/11), `--lib ev3` (2/2), `--test vlist` (14/15: `list_view_runs_on_egui` needs the built
`fixtures/lists-egui/dist`, CI), `--test spec` (3/3).
