# OW7: own modal, inner-window and toasts

Lane OW7 (thinkpadx1), branch `ci/ow7`, from master e832e001 (OW3 merged), merged with master
62f06ce8 (OW5a1, ICR1, DOC2). Spec: `plans/research/ARCH-WIDGETS.md` §3.2 (layers: "modal = an Area at
`Foreground` whose first node is a full-viewport backdrop part"), §6.1 rows `modal`, `inner-window`,
`toast-host`, §7 row OW7; `plans/research/WIDGET-PARITY.md` §2.21 and the OW7 rows; DESIGN R-rules; OW3's
popup surface and units; EV1's overlay layers; EV3's dialog scopes; RT2's `crate::rich`.

svelters now draws its own modal dialogs, inner windows and toasts. egui supplies only Areas, the
painter, picking, focus memory and the modal layer. No `egui::Modal`, `Window`, `Frame`, `Label`
or `Button` runs on the default path:
- the `modal` and `inner-window` egui wrappers stay reachable with `SVELTERS_OWN_WIDGETS=none`
  until VIS1c, as OW3's are;
- the toast host's egui drawing is deleted, so `toast-host` is `OWN_ONLY`.

## What landed

### 1. `modal` (`widgets/dialog.rs::modal`)

**Layer**
- One `egui::Area` at `Order::Foreground` covers the viewport.
- Its first part is the backdrop, `::part(backdrop)`: a click-sensing rect painted `rgba(0,0,0,.3)`, VS Code's dialog block. It blocks presses to anything below.
- Each pass sets egui's modal layer (`Memory::set_modal_layer`), so hover and keyboard focus stay above it. Popups opened from inside it are newer Areas and sit above it.

**Look (R7, `elev-4`)**
- The dialog box is `DIALOG_UA`: `menu-bg` (surface-raised), a 1px `menu-border-color` stroke, `window-radius`, `window-shadow` (`elev-4`), and 12px padding around the content.
- `::part(title)` is semibold `fg-strong`, above the content.
- The box is centred and pixel-rounded.

**Dismissal**
- With `dismissable`, a press on the backdrop closes it with result `"reject"`. Presses on other layers don't count.
- With `dismissable`, Escape closes it with `"reject"`. Escape is checked after the content ran, so an open select or menu inside takes the first Escape: one Escape closes one level.
- EV3 still routes Enter and Escape to the scope's `default` and `cancel` buttons first.

**Focus.** The modal is an EV3 dialog scope through `overlays::draw_content`, unchanged:
- initial focus goes to `autofocus` or the first field;
- Tab is trapped inside;
- focus goes back to the opener when it closes.

**`resizable`** (new modal prop, default false; Qt `sizeGripEnabled`)
- The right edge, the bottom edge and the corner are OW2 `Resizable` handles.
- `::part(grip)` is three dot rows in the corner.
- A user resize pins the top-left and lays the content out at the new size.

**AccessKit.** `Role::Dialog`, `modal`, named by its title.

**State.** `:open` (and `:disabled`) on the node. They are cleared when it closes.

### 2. `inner-window` (`widgets/dialog.rs::window`)

**Layer and position**
- An Area at `Order::Middle`: above the window content, below popups.
- Placed at `default-x`/`default-y`, or centred.
- Look: `WINDOW_UA` (R7 `elev-4`, as the modal, 8px content inset).

**Title bar**
- `::part(titlebar)` is 30px tall.
- Drag it to move the window. The bar stays reachable: at least 40px stays in the viewport.
- Double click collapses the window when `collapsible`.

**Parts**
- `::part(collapse)` and `::part(close)` are 22px tool buttons (`TOOL_UA`):
  - R3 hover (`toolbar-hover`), R2 press (`toolbar-active`), R10 pointer;
  - R8: a keyboard-only 1px `focus` ring at -1. A press focus shows no ring.
  - Glyphs: codicon `chevron-down`/`chevron-right` and `close` when CSS sets `content: icon(...)`, else paths. `Glyph::Close` is new.
- `::part(title)`.
- When `resizable`, eight `::part(edge-*)` handles: four edges and four corners, each driving one or two OW2 `Resizable` units. Left and top handles keep the opposite edge still. Resize cursors per handle (R10).
- Collapsed, only the title bar shows, and the content leaves the tab chain.

**Z-order and focus**
- A press inside the window brings it to the top (`Context::move_to_top`).
- `:backdrop` (bit 20) is on while another inner window is above it. The title then takes `fg-muted`.
- It is an EV3 dialog scope.

**AccessKit.** `Role::Window`, named by its title. Close is a `Button` named "Close window"; collapse is "Collapse window".

### 3. Dialog results (WIDGET-PARITY §2.21, QDialog `done`/`accepted`/`rejected`/`finished`)

`modal` and `inner-window` have new signals and a new method (`DIALOG_SIGNALS`, `DIALOG_METHODS`):
- **`closed(result)`**, where `result` is:
  - `"reject"` when the user dismissed it (Escape, the backdrop, the close button);
  - the argument of `close(result)`;
  - `""` when `open` was set to false.
- **`accepted`** and **`rejected`**, for the results `"accept"` and `"reject"`.
- **`close(result?)`** (`ops.rs`): applied and announced at the next pass, like `popdown`.

`popup` keeps OVERLAY_SIGNALS (`closed` has no arguments). The egui wrapper path reports the same results.

### 4. Toasts (`views/toast.rs`)

**Drawing.** Each toast is svelters' own surface, styled through the host's parts:
- `::part(toast)`: R7 `elev-3` (VS Code notifications, `shadow-lg`): `menu-bg`, a 1px stroke, `menu-radius`, `popup-shadow`, max width 360, a 12px margin from the corner, 6px between toasts;
- `::part(accent)`: a 4px level bar in the theme's `info`/`success`/`warning`/`error` token. It replaces the egui `Visuals` colours;
- `::part(text)`;
- `::part(close)`: the tool button above, with an R8 ring;
- `::part(action)`: R1 raised `button2-bg` with `elev-1`, R3, R2, and an R8 2px ring at +1.
The toasts stack from the host's corner in one foreground Area, newest lowest.

**RT2 runs.** A `show` run list is laid out with `crate::rich::layout_job`: text colour from `::part(text)`, links in `link`. A click on a run with an `href` opens it.

**Actions** (new). `show(text, level?, duration?, actions?)` takes a list of button labels. Pressing one emits `action(id, index)` and closes the toast with `closed(id, "action")`.

**Timeouts.** They run on egui's input clock, which is kittest's virtual clock. Hovering holds a toast open, as before.

**AccessKit**
- Each toast is an `Alert` named `Level: text`, with a live region: `assertive` for warning and error, `polite` for info and success.
- The text is its own node, and a click on it (pointer or AccessKit) is the toast's `clicked`.
- Close is a `Button` named "Dismiss".

**Lint.** The `#[allow(clippy::disallowed_types)] // OW: wrapper until OW1a3, OW7` line in `toast.rs` is deleted, because the toast host no longer uses egui widgets. The `table.rs` lines tagged `until OW1a3` stay for OW1a4.

### 5. Defaults

`DEFAULT_OWN` gains `modal` and `inner-window`. `OWN_ONLY` gains `toast-host`.

Generated files were regenerated in order: spec (`SVELTERS_UPDATE=1 --test spec`), then `gen_props.py` (no change), then `gen_reference.py`. That covers `egui.json`, `egui.d.ts`, and the modal, inner-window and toast-host pages.

## Acceptance

**R-rules met**
- R7: the modal and inner window are `elev-4`, toasts `elev-3`. All have the surface-raised tone and a 1px stroke. HC drops the shadow through the tokens.
- R8: keyboard-only focus rings on the close, collapse and action buttons (1px at -1 on tool buttons, 2px at +1 on actions).
- R1, R2, R3: action buttons; R2 and R3 on tool buttons.
- R10: pointer on buttons, resize cursors on edges.
- R11 does not apply: window edges have no sash bar. The cursor shows at once.

**DOC1 / ARCH-WIDGETS §2.2 state bits written**
- `:open` (7) on `modal` and `inner-window`, cleared on close;
- `:backdrop` (20) on an inner window that is not on top;
- `:disabled`;
- `:hover`, `:active`, `:focus`, `:focus-visible` on the tool and action parts.
There is no `:modal` bit in §2.2. A modal is selected by its tag.

**WIDGET-PARITY rows done**
- §2.21 configuration:
  - `accept`/`reject`/`done(r)`/`result`/`accepted`/`rejected`/`finished` → `close(result)`, `closed(result)`, `accepted`, `rejected`;
  - `sizeGripEnabled` → `modal resizable` and `::part(grip)`.
- §2.21 styling: `QDialog` box (the modal node, R7 `elev-4`), `::part(backdrop)`, `::part(title)`, inner-window `::part(titlebar)`/`close`/`collapse`/`edge-*`, and active versus inactive window (`:backdrop`).
- §2.22 "Beyond Qt" toasts: corner, duration, levels, `dismiss`; actions and live regions are new.

**VIS1.** Row "Menus, tooltip, toast, dialog" (references captured, not probed) and gap 9: the dialog, window and toast surfaces paint from the measured tokens (`window-radius`, `window-shadow`, `popup-shadow`, `menu-radius`). They are not probed because VIS1c is not merged.

**Pins.** None of the native-behaviour pins 1-7 cover dialogs or toasts.

**Existing tests, run on the own implementation**
- `menus.rs::modal_dialog`
- `menus.rs::inner_window_with_content_and_close_button`
- `focus_keys.rs::ev3_modal_overlay_is_a_dialog_scope`
- `focus_keys.rs::ev3_dialog_scope_focus_tab_default_cancel_restore`
- `focus_keys.rs::ev3_svelte_dialog_from_source`
- `views.rs::toast_host_shows_toasts_from_js_methods_and_rust`
- `rt2_rich.rs::a_toast_takes_a_plain_run_list`

## Skipped (follow-ups, proposed as OW7b to the coordinator)

- **The `dialog-buttons` kind**: roles, `Policy::button_order` (FM1 QW11), `clicked(role)`, `help`, `destructive`.
- **The `status-bar` kind** and the status-tip feed (§2.22).
- **`Policy::auto_default`** (§2.1): the focused push button becomes the default in a dialog.
- **Declarative `toast` nodes.** Today a toast from `show` is not a scene node, so its runs keep RT2's tag defaults; `::part(text)` styles the whole text. Per-run CSS needs toasts as nodes. The proposal sent to the coordinator: a `toast` child kind of `toast-host` (a Box), whose label and run children are styled per run by `rich::collect`. Not built while the question is open.
- **The `Draggable` unit.** The title bar drags through its own response, because no Draggable unit exists in `interact` yet.
- **Keyboard window resize.** Edge handles are not focusable, so Resizable's keys don't reach them. Qt's Alt+F8 window menu equivalent is not built either.
- **Open and close transitions** (`:exiting` with transitions) for the modal and toasts.

## Deviations

- **The Run state** (position, user size, collapsed, the Resizable units) lives in `OverlayW::run`, an `Arc<Mutex<…>>`. Equality always holds for it: it is pass state, not a prop.
- **Default sizes.** A resizable inner window's `default-width`/`default-height` are its frame size, title bar included. egui's were the content size.
- **Modal resizing** pins the top-left on the first user resize. It is centred until then.
- **Toast stacking.** Toasts are laid out top-down in list order in every corner, as egui's anchored Area did. Toasts in bottom corners grow upward from the corner.

## Tests

New, `crates/svelters-egui/tests/ow7.rs` (5):
- the modal look (backdrop part, centred box, stroke, title part), `:open`, modal Dialog AccessKit, backdrop blocking and rejecting, `close("accept")` with `accepted`;
- nested Escape: an open select inside the modal closes first;
- the size grip: a corner drag grows the box by the drag and keeps the top-left;
- inner windows: move by the title bar, right and left edges (the left keeps the right edge), collapse and expand, z-order and `:backdrop`, close rejects;
- toasts: stacking, R7 stroke, level bars, assertive and polite live regions, actions (`action` then `closed(id, "action")`), a run list with a link, and an AccessKit click on the text.

### Run locally after the last code change

All runs used `cargo-slot`; the egui tests ran under `headless`.
- `cargo check -p svelters-egui --all-targets` (after the merge).
- `cargo clippy -p svelters-egui --all-targets`: no warning in OW7's files. The `ops.rs` warnings it reports were there before.
- `pnpm install --frozen-lockfile`.
- nextest `-p svelters-egui --test ow7 --test menus --test focus_keys --test views --test ow1a2 --test widgets --test ow3 --test spec --test rt2_rich`: 69 of 69 green, after the merge.
- `SVELTERS_UPDATE=1 … --test spec`, `gen_props.py` and `gen_reference.py` (no change after the merge).

### Not run locally (CI)

- `gallery_views.rs::toast_page_shows_toasts_from_js_and_reports_closes` and `menus_app.rs`: both need built fixtures;
- the full workspace, conformance, the other backends, `svelters-automation`, `examples/egui-kinds`.

## Push and CI

See the coordinator report: the sha, the five run ids and the results.
