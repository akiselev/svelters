# OW7b: declarative toasts, dialog-buttons, auto-default (status-bar handed off)

Lane OW7b (thinkpadx1), branch `ci/ow7b`, from master ec8809fe (OW7 merged). Spec: OW7's "Skipped"
list (`plans/milestones/OW7-modal-window-toasts.md`), WIDGET-PARITY §2.1 (`autoDefault`), §2.21, §2.22,
FM1's QW11 order (`origin/ci/fm1`, stopped and kept as spec), WG6's `status-tip` (`origin/ci/wg6`, stopped).

## What landed

### 1. Declarative `<toast>` (`views/toast.rs`, kind `toast` in `kinds/views.rs`)

```svelte
<toast-host>
  <toast level="warning" duration={5} bind:open onclosed={(reason) => …}>
    <label>Disk <b>full</b> <a href="…">Details</a></label>
    <button onclick={retry}>Retry</button>
  </toast>
</toast-host>
```

- **Kind.** `toast` is a Box (a flex column, laid out by taffy). It is a child of `toast-host`, whose slot `toasts` now accepts only `toast`.
- **Props.** `level` (info, success, warning, error), `duration` (null: the host's; 0: until closed), `open` (bindable, default true; setting it true again shows the toast anew).
- **Signals.** `changed(open)`, `closed(reason)`, `clicked`. `reason` is `timeout`, `dismissed` (the close button), `action` (a child button's `clicked`) or `""` (JS set `open` false).
- **Per-run CSS.** The toast's labels are ordinary nodes, so their RT2 runs take CSS: `toast b { … }`.
- **Look.** `TOAST_UA` (R7 `elev-3`) under the node's own CSS: `toast`, `toast[level=…]`, `toast::part(accent)` (the level bar), `toast::part(close)` (the 22px tool button with R8's ring).
- **States.** `:hover`, and `:exiting`. After a close, the toast stays drawn and inert with `:exiting` until the longest `transition` in its computed style ends. That runs on the virtual clock and is painted with the sampled transitions (`opacity` fades the whole toast).
- **Host.** It stacks the children first, in tree order, then the `show()` toasts, all in one foreground Area. It lays each toast out with `layout::layout_overlay`: at max-content first, then again at the max width minus the chrome when that is wider. Timeouts run on egui's clock, and hover holds a toast open.
- **AccessKit.** Each toast is an `Alert` named `Level: text` (its labels' text, buttons left out), a live region (`assertive` for warning and error) with a `Dismiss` button.
- **Out of flow.** `toast-host` and `toast` are out of flow (`overlays::out_of_flow`): display:none in their parent. `overlays::paint_node` calls `toast::paint` with the walk's origin.
- **`::part()` for the built-in toast kinds.** Built-in kinds had no part declarations (`SceneStyle::part_decl` came only from `W::Ext`), so `::part()` never reached `toast-host` either. `toast::parts_of` now declares `toast-host`'s `toast`, `accent`, `text`, `action`, `close` and `toast`'s `accent`, `close` (`style.rs`).
- **`show()`** is unchanged: no node, tag-default runs, `actions`. Its doc (egui.d.ts, the reference page) and `docs/egui/css/own-widgets.md` say that per-run CSS needs the declarative form.

### 2. `dialog-buttons` (`views/dialog_buttons.rs`, kind in `kinds/views.rs`)

- **Kind.** A Box. It is created as a row: `justify-content: end`, `align-items: center`, `gap: 6`.
- **Roles.** Each child takes the new Widget prop `dialog-role`, as FM1 specified it (`Common::dialog_role`). The values are `auto`, accept, reject, destructive, apply, reset, help and action. `auto` means `accept` for a `default` button, `reject` for a `cancel` one, else `action`.
- **Order.** The `order` prop takes `platform` (default), windows, kde, gnome, macos or none. Help and reset come first, then children without a role, then the platform's roles, using FM1's QW11 tables.
- **Layout only.** Only taffy sees the order: `Tree::children_laid` returns `DialogButtons::laid`, which is rebuilt in `compute_child_layout`. Paint, hit testing and Tab order stay the tree's.
- **`Policy::button_order`** is now `ButtonOrder::{Windows, Kde, Gnome, MacOs}` with `roles()`. GTK uses Gnome, Qt Kde, Windows and egui Windows, Mac MacOs. The old `OkCancel`/`CancelOk` had no users.
- **Signals.** `clicked(role)` for each clicked button, detected from its own `clicked` event or from egui's response. `help` fires for the help role.
- **Dialog results.** An accept or reject click closes the nearest `modal` or `inner-window` with that result, as `close("accept")` does. OW7's `closed(result)`, `accepted` and `rejected` follow.

### 4. `Policy::auto_default` (`native/mod.rs`, `focus.rs`, `widgets/button.rs`)

- **On (Windows, Qt/KDE, GTK, egui).** The push button focused in a dialog scope is that scope's default: it shows `:default` (`focus::shows_default`), and the others in that scope don't. Enter presses it, through the Pressable claim it already had.
- **Off (macOS).** A focused button no longer claims Enter. EV3 routes Enter to the scope's `default` button whatever is focused; Space still presses the focused button.

### Generated

The generators ran in this order: spec (`SVELTERS_UPDATE=1 --test spec`), `gen_props.py`, `gen_utilities.py`, `gen_reference.py`. They cover egui.json, egui.d.ts, tables/utilities.generated, props.generated.js, and the reference pages (new: `toast.md`, `dialog-buttons.md`; every Widget page gains `dialog-role`).

## Handoff (not done in this lane)

- **3. `status-bar`: not started.** Planned design:
  - a Box kind, row, `Role::Status`, with a `::part(message)` area at the start (flex-grow);
  - children are the sections, placed by flex, with QStatusBar's permanent widgets as `slot="end"`;
  - `show-message(text, timeout)`, `clear-message`, the read-only `message` prop and `message-changed`, with timeouts on the virtual clock;
  - WG6's `status-tip` (a Widget prop): while the pointer is over a node with a tip (innermost wins), the nearest `status-bar` of the viewport shows it in `::part(message)` over the message, and `svelters_egui::status_tip(&ctx)` reads it;
  - announcements: polite.
  - `Common` has room: SG6's budget is 640 B a node; take an `Option<Box<str>>` for the tip.
- **5.**
  - Keyboard resizing of inner windows: not done (OW7's note stands: the edge handles are not focusable).
  - `:exiting` transitions: done for `<toast>` only. The modal and inner-window still close at once.
- **`modal` and `inner-window` parts: done.** `toast::parts_of` declares `modal`'s backdrop, title and grip and `inner-window`'s titlebar, title, close and collapse; test `ow7b_modal_and_inner_window_parts_take_author_css`. The `edge-*` handles are not painted, so they are not declared. `popup` still declares none (OW3 styles its popup surface through the host).

### From OW7's handoff (its uncommitted diff, folded in here)

OW7b builds on OW7's `widgets/dialog.rs` (the modal and window chrome, the `tool`/`tool_with` parts, `DIALOG_SIGNALS`) and `views/toast.rs`. Its acceptance is WIDGET-PARITY §2.1, §2.21 and §2.22. Against that plan:
- **`dialog-buttons`.**
  - Done: roles, order in all five policy tables, `clicked(role)`, `help`, accept and reject closing the dialog.
  - Not done: the `buttons="ok cancel apply …"` shorthand (host-localised labels), the `yes` and `no` roles, `orientation` (CSS `flex-direction` works), and "a destructive button is never the default".
  - Styling: `dialog-buttons button::part(icon)` works through the own button's icon part.
- **`status-bar`** (not done), as OW7 specified it:
  - children are the temporary area, hidden while a message shows; children with `slot="end"` are permanent;
  - `show-message(text, timeout)`, `clear-message`, the read-only `message`, `message-changed`, with timeouts on egui's clock;
  - `::part(message)`, with `status-bar > *` styling the sections;
  - `Role::Status`; messages are announced politely (QA11);
  - status tips: the `status-tip` prop on nodes and on menu actions (menu `hover` too) routes to the nearest `status-bar` of the viewport and clears on leave.
- **`Policy::auto_default`.** Done (on for Windows, KDE, GTK and egui; off on Mac).
- **Declarative `<toast>`.** Done (stage 1).

### Handoff details (coordinator, 2026-10-22: OW7b finishes its full scope)

- **Stage 1** (`<toast>`) merges alone from `ci/ow7b-s1` (6d059c4b). This branch (`ci/ow7b`) also has items 2 and 4. Merge master into it once stage 1 is on master.
- **Left for the continuation agent**, in this order:
  1. **Item 3, `status-bar`**: the whole kind and the `status-tip` feed, as specified in "From OW7's handoff" and in the Handoff section above. Nothing of it is built.
  2. **`dialog-buttons` gaps:**
     - the `buttons="ok cancel apply …"` shorthand (host-localised labels);
     - the `yes` and `no` roles (add them to `DIALOG_ROLE` and to every `ButtonOrder::roles` row; Qt puts yes with accept and no with reject);
     - a `destructive` button is never the default: `role_of` and `focus::shows_default` should skip it;
     - `orientation`.
  3. **Item 5:**
     - keyboard resizing of inner windows (edge handles are not focusable; Qt's Alt+F8);
     - `:exiting` transitions for `modal`, `inner-window` and the `show()` toasts (only `<toast>` has it).
  4. **VIS1:** probe rows for `toast` and `dialog-buttons` once references exist.
  5. **Item 2 vs QW11:** FM1 specified `button-order` on any `box`; this lane ships the `dialog-buttons` kind only. Decide whether the box prop is still wanted.
- **Done here and not to redo:** `<toast>` (stage 1, on master), `dialog-buttons` roles, order and results, `Policy::auto_default`, `::part()` for `toast-host`, `toast`, `modal` and `inner-window`, the OW-DONE audit line (WIDGET-PARITY coordinator block).
- **Gotchas:**
  - kittest's `run()` advances the virtual clock past short timeouts, so give toasts in tests durations of 3 s or more and step to the timeout;
  - an egui Area's layer is only as big as what is allocated in it, so `aui.expand_to_include_rect` is needed for `:hover`;
  - built-in kinds have no `::part()` declarations unless they are listed like `parts_of`;
  - `label` is an own widget since OW1a3 (`W::Ext`): read RT2 runs through computed styles or the text mirror, not `W::Label { rich }`;
  - the generator order is spec, gen_props, **gen_utilities**, gen_reference (the js CI job checks the utilities).

## Acceptance

**R-rules:** R7 (`toast`'s elev-3 surface, from OW7's UA), R8 (the toast close button's keyboard-only ring), R1, R2, R3 and R10 (the toast close tool button), R17 (`:exiting` follows the reduced-motion `Transitions` path). `dialog-buttons` draws nothing itself; its buttons keep the own button's R1–R3, R8 and R10.

**WIDGET-PARITY rows (OW7's remaining):**

| Row | Status |
|---|---|
| §2.1 `autoDefault` → `Policy::auto_default` | done |
| §2.21 `standardButtons`/roles → `dialog-buttons` with `dialog-role` children | done (roles; the `buttons="ok cancel"` shorthand is deferred, since its labels need host localisation) |
| §2.21 platform button order → `Policy::button_order` | done |
| §2.21 `clicked(button)`, `helpRequested` | done (`clicked(role)`, `help`) |
| §2.21 destructive role styling | done as a selector (`button[dialog-role=destructive]`); "never the default" is not enforced (deferred) |
| §2.21 `centerButtons`, `orientation` | CSS (`justify-content`, `flex-direction`) |
| §2.21 open/close animation (`:exiting`) | toasts done; modal and inner-window deferred |
| §2.22 `status-bar` rows (messages, sections, status tips, `::part(message)`, `Role::Status`) | deferred (handoff above) |
| §2.22 "Beyond Qt" toasts: per-run CSS | done (declarative `<toast>`) |

**VIS1:** the toast surfaces paint from the same measured tokens as OW7's (`menu-bg`, `menu-border-color`, `menu-radius`, `popup-shadow`, the level tokens). Not probed: the `toast` and `dialog-buttons` kinds have no VIS1 reference rows yet (row "Menus, tooltip, toast, dialog": references captured, not probed).

**Pins:** none of the native-behaviour pins 1–7 cover toasts, button order or auto-default.

## Tests

New, `crates/svelters-egui/tests/ow7b.rs`:
- `ow7b_declarative_toasts_style_per_run_stack_and_close`: per-run CSS, `[level]` border, a 6px `::part(accent)`, stacking with `show()`, Alert names and politeness, `:hover`, a child-button action, the timeout, `:exiting` held and ended, reopening;
- `ow7b_dialog_buttons_platform_order_roles_and_results`: the four orders and `platform` under GTK, the 6px gap, `clicked(role)`, `help`, accept closing the modal with `"accept"`;
- `ow7b_auto_default_moves_default_to_the_focused_button`: Windows (`:default` follows focus, Enter presses the focused button), macOS (Enter presses OK).

`widgets.rs::every_kind_has_a_tooltip_and_inputs_are_named` skips `toast-host` and `toast`, as it skips the overlays: they are out of flow.

### Run locally after the last code change

After the last merge (master 55cf5c9d), all with `cargo-slot` and `headless`: `cargo check -p svelters-egui --all-targets`, and nextest `ow7b`, `ow7`, `observe`, `scene`, `ow5b` and `widgets` (35 of 35). Earlier in the lane: `menus`, `focus_keys`, `views` and `ow_css` (35 of 35, after the modal parts change). Before the last change (adding the tooltip to dialog-buttons), `ow7b`, `ow7`, `views`, `menus`, `focus_keys`, `rt2_rich`, `scene`, `scene_memory`, `spec`, `ow_css` and `widgets` ran: 73 of 74 passed, and the one failure is what the change fixed. Also `cargo clippy -p svelters-egui --all-targets` (no new warnings in this lane's files) and `gen_utilities.py --check`.

### Not run locally (CI)

The full workspace, conformance, the other backends, the gallery and fixture tests, `examples/egui-kinds`.
