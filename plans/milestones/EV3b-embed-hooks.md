# EV3b: Embed hooks for cadatomic's Edit menu and keymap (last text field, Latin fallback, provider edit commands)

Track: EVENTS (egui). Lane EV3b, branch `ci/ev3b`, from master `2cf3f4db` (after RA3). Asked by cadatomic after
its SH1 integration (`docs/plan/SH1-shell-adoption.md`, "EV3 hook requests"); signatures accepted by
svelters-agent. Builds on EV3+ (`plans/milestones/EV3-focus-keys.md`) and RA3's `Widget::text_input`
(`plans/milestones/RA3-widget-api.md` §6).

## As built

| Hook | Where | What |
|---|---|---|
| `Embed::last_text_field(&self, ctx) -> Option<FocusInfo>` | `embed/mod.rs`; `EguiBackend::last_text_field` in `focus.rs` | The text field `text_command` acts on, with its root: the focused one, else the last one that had focus while nothing has focus (a click on host chrome) or a menu item has it (an egui popup is open, or an own menu or list holds the keyboard: EV3b2). A built-in `text-edit`, an own text widget or a provider kind whose `text_input()` is `Some`. Ends when another widget takes focus (a svelters node, or a host widget with no menu open), or when the field is no longer recorded: removed, hidden, disabled or inert. It does not come back when the field is enabled again. |
| `Embed::shortcut_override_press(&self, ctx, logical, physical, modifiers) -> bool` | `embed/mod.rs`; `EguiBackend::shortcut_override_press`, `focus::latin_fallback` | Asks the existing `shortcut_override` claim for the logical key, then, for a Ctrl/Cmd chord whose logical key is not a Latin letter or digit and differs from the physical key, for the physical key: KL1's rule (cadatomic `keylayout`), the typed key wins. For a non-Latin letter egui reports the physical key as the logical one, so it is asked once. |
| `Widget::text_command(&mut self, cmd: &TextCommand, cx: &mut WidgetCx) -> bool` (default false), forwarded by `ErasedWidget` | `provider/mod.rs` (`paint`) | A pending edit command for a provider kind with `text_input()` `Some` is given to the kind first, at its next paint, just before `ui`, with its cx. True: the kind applied it (its own undo, a host-owned document). False: it is fed as the input it stands for, as before RA3/EV3 (`Ctrl+Z`, `Event::Copy`, ...). Through `Embed::text_command` a Paste arrives as the `Insert` of the host clipboard's text, and what a Cut or Copy puts in egui's clipboard also reaches the host's (`after_text`). New `WidgetCx::ctx()` so the hook can `copy_text`. |
| `text_command` fallback | `focus.rs` (`text_target`) | It now also falls back to the last field while a menu item has focus (same rule as `last_text_field`, so the Edit menu's enabled state and the command agree). |

`focus::before_text` is split into `take_command` (takes the pending command and resolves the host clipboard)
and `feed` (pushes the input events), so the provider path can put the hook between them.

### About cadatomic's report ("Undo in a focused quantity-field falls through to the document command")

On master after RA3 a provider kind is an edit-command target only when it declares `text_input()`.
cadatomic's pin predates RA3, and its `QuantityField` does not declare it yet. Since its egui `TextEdit` has the
node's id, `fn text_input(&self) -> Option<bool> { Some(false) }` is enough: `Embed::text_command(Undo)` then
answers true (so the shell skips the document undo) and the undo is fed to the field as `Ctrl+Z`. The tests
cover this with `t-field`. `<code-editor>` (quire, host-owned undo) should also declare
`text_input() -> Some(true)` and implement `Widget::text_command`, mapping Undo and Redo to its `HostTextEvent`s.
The tests cover this case with `t-editor`.

## Tests (`crates/svelters-egui/tests/focus_keys.rs`)

| Criterion | Test |
|---|---|
| `last_text_field` is the focused field, stays after a click on host chrome and while a menu item (a host widget) has focus with a menu open, and the Edit command reaches the field from there; it ends when a host widget (no menu) or a svelters button takes focus, when the field is disabled (and stays ended after re-enabling) and when it is removed; a provider kind with `text_input` counts | `ev3b_last_text_field_survives_menus_and_ends_with_the_field` |
| Latin-fallback matrix: Latin layouts (AZERTY, QWERTZ, QWERTY), symbol layouts (Dvorak `;`, AZERTY `,`), non-Latin (Russian, logical = physical), each with and without Ctrl or Cmd, against a focused field; a focused button claims none | `ev3b_shortcut_override_press_latin_fallback` |
| Provider-kind undo: a `TextEdit`-backed kind (quantity-field shape) takes Undo, so the host keymap never sees Ctrl+Z; a kind with its own undo gets Undo, Insert, Copy, Delete and Paste through `Widget::text_command` with no key fed, and a declined command (Delete) is fed as its key; the last-field fallback reaches the kind and focuses it; the same kind without `text_input` gets nothing (`text_command` false, no claim), so the host's document undo runs | `ev3b_text_commands_reach_provider_kinds` |
| Through `Embed` (a Svelte dialog from source): `last_text_field` equals `focus_info` for the focused field, survives host chrome, ends when the dialog closes; `shortcut_override_press` | `ev3b_embed_hooks` |

## Notes

- `FocusInfo.root` is `None` for a top-level `<window>` app (TestApp). Only nodes in embed slots have a root,
  as for `focus_info`.
- A change from EV3: once another widget has had focus after the field, `text_command` no longer falls back to
  the field (it used to after a later click on host chrome). This matches Qt (the Edit menu acts on the focus
  widget) and cadatomic's own `text_target` flag, which the hook replaces.
- Under the GTK native policy a press focuses svelters' own `menu-button` (a recorded node), so opening that
  menu ends the last text field. Host menus (cadatomic's are Rust) are unaffected. This was already the
  case for EV3's `text_command`.

## Verification

Local, after the last code change: `cargo check -p svelters-egui --tests` and `cargo clippy -p svelters-egui
--all-targets` (no new warnings); nextest under `tools/dev/headless`: `-p svelters-egui --test focus_keys`
(13/13, the 4 `ev3b_*` and the 9 EV3 tests), `--lib ev3` (2/2), `-p egui-kinds-example --test methods` (4/4, the
RA3 provider `text_input` path). After merging master `52729604` (OW1a2: own widgets on by default, ZM1; no
conflicts): `cargo check -p svelters-egui --all-targets` (clean), nextest `--test focus_keys --lib ev3` (15/15).
Not run locally: the full workspace, conformance, the other backends, the JS
suites (CI).

## EV3b2: after OW3 (own menus by default)

Merged master `30a0be2c` (OW3: own menus, menu-bar, context-menu and popup by default; no textual conflicts).
OW3's menus hold the keyboard while open (`widgets::popup::Grab` moves focus to the surface's id, which no
node records) and are no egui popup, so `ctx.any_popup_open()` and EV3's `st.popup` missed them and the last
text field ended as soon as an own menu opened. Now `Grab::take` and `Grab::keep` call `focus::note_grab`, a
per-pass flag in the focus record (`Pass::grab`): `route` keeps the last field while the last pass had a grab,
and `text_target` treats a grab in this or the last pass as an open menu. That covers own menus, the bar in
F10 keyboard mode and the own select list, as the egui popup rule covered their wrappers.

`ev3b_last_text_field_survives_menus_and_ends_with_the_field` now runs under `KindSet::Default` (own menus)
and `KindSet::None` (the egui wrappers), also checks the field right after the menu opens (the own menu
holding focus), and closes the menu with a press outside instead of Escape: an own menu takes Escape only
while it holds the keyboard, and the command had given the keyboard to the field.

Verification (after the last code change): `cargo check -p svelters-egui --all-targets` (no new warnings),
`cargo clippy -p svelters-egui --all-targets` (nothing in the changed files); nextest under
`tools/dev/headless`: `focus_keys`, `menus`, `ow3`, `widgets` and `--lib ev3` (47/47), and `focus_keys` +
`menus` with `SVELTERS_OWN_WIDGETS=none` (21/21). Not run locally: the rest of the workspace, conformance,
the other backends, the JS suites (CI).
