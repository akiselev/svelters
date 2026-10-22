# OW3: own popups, menus, select and tooltips

Lane OW3 (WSL PC), branch `ci/ow3`, from master 2305a42d (OW0 and EV1 merged), merged with master
befca905 (OW1a slice 1, SG6, RT2, LD3). Spec: `plans/research/ARCH-WIDGETS.md` §1.2 (units), §3.2
(layers), §6.1 rows `popup`, `context-menu`, `menu-bar`, `menu`, `action`, `menu-separator`,
`menu-button`, `combo-box`, common `tooltip`; `plans/research/WIDGET-PARITY.md` §2.8, §2.14, §2.15
and the `menu-button` rows of §2.1; DESIGN R-rules; the OW0 "Write a widget" guide; EV1's
`dispatch`.

svelters now draws its own menus, popups, select lists and tooltips. egui supplies only Areas,
the painter, picking and focus memory. No `egui::Popup`, `MenuBar`, `MenuButton`, `SubMenuButton`,
`ComboBox` or `on_hover_text` runs on the default path. The egui wrappers remain reachable with
`SVELTERS_OWN_WIDGETS=none` (or `KindSet::None`) until VIS1c, as the `button` wrapper does.

## What landed

### 1. Units (`crates/svelters-egui/src/interact/`)

| Unit | File | Public shape |
|---|---|---|
| **Anchored** | `anchor.rs` | `Placement { side: Side, align: Align }` (`Placement::parse("bottom-start" \| "right-start" \| "top" \| "left-end" …)`), `AnchorParams { anchor, size, bounds, placement, gap, cross, flip, shift, match_anchor_width }`, `place(&AnchorParams) -> Placed { side, rect }` (Floating UI's offset, flip, shift), `anchor::bounds(ctx)`. The unit reports `Placed` each pass. |
| **Dismissable** | `dismiss.rs` | `DismissParams { inside: Vec<Rect>, on_outside: Outside::{Close, Ignore}, escape }`, `Dismissable::check(&p, &InputState) -> Option<Dismiss { reason, at }>`. `Reason` (`#[non_exhaustive]`) is `Escape`, `OutsidePress`, `FocusOut`, `AnchorHidden` or `Programmatic`. It claims Escape when `escape` is set. |
| **Roving** | `roving.rs` | `RovingParams { enabled: Vec<bool>, orientation: Orientation::{Vertical, Horizontal}, wrap }`; `Roving { current() , set(Option<usize>), key(&p, Key) -> Option<usize> }`; `Roving::next(enabled, from, ±1, wrap)`, `Roving::edge(enabled, forward)`. It has one current index, moves with the orientation's arrows, Home and End, wraps optionally and skips disabled items. As a unit it reports `Move { to }` while the host is focused, and it claims those keys. |
| **Typeahead** | `typeahead.rs` | `TypeaheadParams { timeout }` (1 s), `Typeahead { push(&p, text, now) -> Option<String>, reset(), typing(&p, now) }`, `typeahead::find(labels, enabled, query, current)` (case-insensitive; typing one letter repeatedly cycles). The unit reports `Search { text }`. |
| **Tooltipable** | `tooltip.rs` | `TooltipParams { has_tip, delay, warm, duration }`, `Tooltipable { step(&p, over, pressed, now, &mut Intents<Tip>), shown() }`, `Tip::{Show, Hide}`. A press hides the tooltip until the pointer leaves. `next_frame` asks for the frame at the delay. |

Each unit's pure logic has a unit test in its file.

`Roving` is the shape the coordinator asked for, so that it can be reconciled with OW1a2's.

### 2. Shared surfaces (`widgets/popup.rs`)

- **`SURFACE_UA`** follows R7: `menu-bg` (`surface-raised`), a 1px `menu-border-color` stroke,
  `menu-radius` (VIS1 measured 8), `popup-shadow` (`elev-3`), 4px padding and a 4px margin from the
  anchor.
- **`popup::area`** creates one `egui::Area` per surface at `Order::Foreground`. Areas are
  interactable, so presses do not reach what is below.
- **`popup::dismissed`** is `Dismissable::check`, except that a press on another floating layer
  (a select inside a popup, a submenu) does not count as outside.
- **`Grab`** is the keyboard grab of an open menu or list:
  - it moves egui focus to the surface's id, with a focus-lock filter for Tab, arrows and Escape,
    so the field that had focus sees no keys;
  - it gives focus back when the surface closes, to the focus held at the press that opened it.
    EV3's `focus::pass_start_focus` provides that, because a text field drops focus on the press.
- **The `popup` overlay** (`popup::show`):
  - its children are laid out by taffy inside the surface box, placed by `placement` with flip and
    shift;
  - Escape closes it (when `dismissable`), and so does a press outside it and its owner (with
    `autohide`);
  - it gets AccessKit `Role::Dialog` and keeps its EV3 dialog scope.

### 3. Menus (`widgets/menu.rs`)

The WG2a data model stays: the coordinator decided on 2026-10-22 that entries remain `W::Menu`
scene children of their root. `crate::menus` keeps the props, shortcuts, radio groups and
`activate`. The own code draws and drives the menus.

**Layout and look**
- Each open level is an Area:
  - below the bar title (`bottom-start`, gap 0);
  - below the menu-button (the surface margin);
  - at the pointer for a context menu;
  - `right-start` for a submenu, flipped at the right edge, with its first row level with the
    opener row.
- Rows are the entry nodes, styled by CSS on `action`, `menu` and `menu-separator` over
  `ITEM_UA`:
  - 26px rows (22 + 2·2, VS Code's 2em), 8px sides, radius 4;
  - `:current` (pointer or keyboard) in `menu-sel`/`menu-sel-fg`; a disabled current row is only
    tinted with `list-hover`.
- State bits written on each row node: `:hover`, `:current`, `:checked`, `:open` (a submenu
  row), `:disabled`, `:focus-visible`.
- Paint parts:
  - `::part(check)`: a column reserved only when the level has a check or radio item;
  - `::part(shortcut)`: muted unless the row is current;
  - `::part(chevron)`;
  - `menu::part(popup)` and `menu-button::part(popup)`. A context menu is styled through its own
    node.
- Glyphs come from `content: icon(…)` when CSS sets one (P2), otherwise paths (`widgets/glyph.rs`).
  No icon font is bundled until ICR1.
- Separators collapse as QMenu's `separatorsCollapsible` does: leading, trailing and doubled
  separators are hidden.

**Keyboard**
- In an open level:
  - Up, Down, Home and End move through the rows. Separators are skipped. Disabled rows are
    reachable, as R9 asks, but cannot be picked.
  - Right opens a submenu. Left closes one. On the bar, Right and Left cross to the next or
    previous title, with wrap.
  - Enter or Space picks the row or opens its submenu. Escape closes one level.
  - Typing moves the current row by type-ahead. Tab does nothing.
- On the bar, F10 enters keyboard mode, as does a lone Alt when the new
  `Policy::alt_focuses_menu_bar` is set (Qt and Windows rows). Left, Right, Home and End then move
  between titles, and Down, Enter or Space opens one at its first row.
- A context menu opens with Shift+F10 or the Menu key while its owner is focused (FM1 QA8). The
  innermost owner wins.
- A focused menu-button opens with Enter, Space, Down or Alt+Down.
- The key that opens a level is never also a pick.

**Pointer**
- A click on a bar title opens its menu; a click on the open title closes it. Hovering another
  title while a menu is open switches to it.
- A submenu opens after `Policy::submenu_delay_ms` (new field: GTK and Qt 225, Windows 400, Mac 0),
  or at once on a click. Hovering another row closes an open submenu after the same delay.
- The safe triangle: a pointer moving from the opener row towards the open submenu does not change
  the current row.
- A press outside every level and the root closes all of them. The press that opened a context
  menu does not count.

**Events and accessibility**
- `activate` now runs through EV1's Rust filters before its default action, in both
  implementations:
  - `Prevent` keeps the check state, and JS still sees the event;
  - `Stop` and `Prevent` together drop it.
  - Shortcuts take the same path.
- New `action` signal **`hover`** (QAction `hovered`): the deepest level's current row changed.
  Only the own menus emit it.
- AccessKit roles:
  - `MenuBar`;
  - a `Menu` per level, with `active_descendant` set to the current row;
  - `MenuItem`, `MenuItemCheckBox` and `MenuItemRadio`, with `toggled`;
  - `expanded` and `has_popup` on submenu rows, bar titles and the menu-button;
  - the accelerator as `keyboard_shortcut`.
- An action's `tooltip` shows in menus (QMenu `toolTipsVisible`).

**menu-button**
- It uses the own button's look (`button::UA`) plus a `chevron-down` part (R14).
- `:open` mirrors `active`. `active` follows the user and the host, as before.
- A press focuses it per `focus_on_click`. Focus goes back to it, or to the field that had focus,
  when the menu closes.

**Switch.** A new menu root (`menu-bar`, `menu-button`, `context-menu`) or `popup` records whether
it is own when it is created (`widgets::mark_own`, an `own` flag in its payload). Its entries
follow their root. Own roots are measured from CSS (`menu::measure`): the bar is 30 tall, and the
menu-button is label + chevron + the button's padding and border.

### 4. Select (`widgets/select.rs`, `#[svelters::widget(replaces = "combo-box")]`)

**Look**
- The closed select uses `control-bg`, a `control-border` boundary and `input-radius`, and is 26
  tall.
- It shows the `chevron-down` part (closing VIS1 gap 8, which was a filled triangle) and a
  `label` part. The hint text takes the `placeholder` colour.
- It gets a pointer cursor (R10), a 1px `focus` ring at -1 on keyboard focus (R8), a `focus`
  border while open, and R9 when disabled.

**Keys while closed and focused**
- Up, Down, Home and End pick an option at once.
- Typing picks by prefix (WG6 QW9).
- Enter, Space, F4 and Alt+Down open the list.

**The list**
- Placement: `match_anchor_width`, flipped above the select at the bottom edge.
- Rows are `::part(option)` (R6 flat): `list-sel` when current, `list-hover` under the pointer,
  `:selected` for the chosen option.
- It scrolls past `max-visible` with the wheel, and the current row stays shown.
- Keys: Up, Down, Home, End, PageUp and PageDown; type-ahead; Enter or Space picks; Escape, Tab and
  F4 close.
- A press outside the list and the select closes it.
- AccessKit roles: `ComboBox` (value, `expanded`, `has_popup`), `ListBox` (`active_descendant`) and
  `ListBoxOption` (`selected`).

**Signals.** A pick dispatches `changed` when `selected` changes; EV1 can prevent it. It then
dispatches `activate` and `commit`. `highlight` reports the current row of the open list.

**Extras** (spec, typings and docs regenerated):
- `open` (bindable: `showPopup`/`hidePopup`);
- `max-visible` (30, Qt's default);
- `wheel` (`off` \| `hover` \| `focused`, default `off`);
- the events `activate`, `highlight` and `commit`.

A static `options="a b"` string becomes a list for own widgets too (`scene/props.rs`).

### 5. Tooltips (`widgets/tooltip.rs`)

**Showing**
- `tooltip::attach(ui, resp, text, own)` replaces `on_hover_text` at these call sites: `leaf.rs`
  (leaves, containers, collapsing header, scroll area), `elements/mod.rs`, `elements/panes.rs`,
  `views/mod.rs`, `provider/mod.rs` (own widgets) and the own menus.
- Delay: `Policy::tooltip_delay_ms`.
- Follow-on window: new `Policy::tooltip_follow_on_ms` (Qt 2 s). The next tooltip shows at once.
- A press hides the tooltip until the pointer leaves.
- Disabled hosts show their tooltip too (R9, WG6 QA5).
- The innermost host (smallest rect) wins. Its tooltip is painted once at the end of the pass, in
  an `Order::Tooltip` Area that takes no input, 18px below the pointer and flipped above it at the
  bottom edge.

**Look.** `TOOLTIP_UA` follows R7 on `tooltip-bg`, with 4/8 padding, radius 4 and 12px text
wrapped at 400.

**Accessibility.** The tooltip node is `Role::Tooltip`. The host's AccessKit `description` is the
tooltip text whether it shows or not, so R9's reason is read.

**API.** `tooltip::show(ctx, text, at)`, `hide(ctx)` and `is_shown(ctx)` are QToolTip's
`showText` and `hideText`.

**Switch.** `tooltip` in the KindSet, checked when painting (it has no tag of its own).

### 6. Defaults

`DEFAULT_OWN` gains `menu-bar`, `menu-button`, `context-menu`, `popup`, `combo-box` and `tooltip`.

## DESIGN rules, parity rows, VIS1

- R-rules met:
  - R7 (menus, popups, the select list, tooltips);
  - R14 (submenu rows, menu-button, select);
  - R9 (disabled rows reachable but not pickable; tooltips on disabled hosts; AccessKit
    description);
  - R8 and R10 (select);
  - R6 (option rows flat);
  - R3 (row and title states are written in the pass that paints them).
- VIS1: gap 8 is closed by construction. For gap 9, menus and tooltips now paint from the
  measured tokens (`menu-radius` 8, `popup-shadow`). Neither is probed: VIS1c is not merged.
- WIDGET-PARITY rows done, by section:
  - §2.14:
    - separators collapsible;
    - tooltips in menus;
    - `hover`;
    - submenu delay and the safe triangle;
    - the Alt/F10 bar;
    - hover-to-switch;
    - the context-menu keys;
    - every styling row except icons, sections and scrollers.
  - §2.8: `open`, `max-visible`, closed and open type-ahead, `wheel`, `activate`, `highlight`,
    `commit`, and the `label`, `chevron`, `popup` and `option` parts.
  - §2.15: disabled hosts, the delay and follow-on, `show`/`hide`/`is_shown` (Rust), placement
    below the pointer with flip, and the description.
  - §2.1: the menu-button chevron and `:open`.
- **Parity rows skipped** (follow-ups, with why):
  - Not before ICR1, which bundles an icon font: `action icon` and `show-icon`, `menu icon`, option
    icons.
  - Need EV3 chord dispatch: shortcut chords and `std:` names.
  - Each needs a new `P` key or method in the WG2a tables; IDE-relevant but not blocking SC1:
    `group-optional`, `status-tip` (no status bar yet), `menu-separator label` (sections),
    `action default`, `menu` `changed` (aboutToShow), `popup(x, y)`.
  - Scrollable tall menus: OW2's Scrollable is on master now, but wiring it in is not small (see
    "Merge with master (OW3m)").
  - Also deferred: `slot="end"` corner widgets, widgets inside menus, and split and delayed
    menu-buttons (OW3b).
  - Select:
    - option objects (`{ label, value, icon, disabled, separator, tooltip }`) and the per-option
      `part` names (P10), `value`, `size-to`: OW3b;
    - the `editable` combo family: OW3b, after OW4;
    - `::part(button)`: OW3b.
  - Tooltip:
    - `svelters:tooltip` JS module;
    - the `tooltip-rect`, per-node `tooltip-delay`, `tooltip-duration` and `tooltip-placement`
      props (the unit already supports duration);
    - rich and snippet tooltips: these need compiler support for a `tooltip` snippet;
    - the `::part(tooltip)` style host: the tooltip reads only its UA table until OW-CSS computes
      parts and `attach` gets the host node.

## Deviations

- **The menu family is not `#[svelters::widget] replaces=` kinds** (coordinator decision,
  2026-10-22). Its entries are out-of-flow children whose picks write their siblings (radio
  groups), and `WidgetCx` writes only its own node. OW3 replaces drawing and behaviour behind the
  switch. Follow-up: once OW1a2's `WidgetCx::with_peer` is on master, move the entries to own
  kinds.
- **AccessKit roles change on the own path**:
  - menu items are `MenuItem`, `MenuItemCheckBox` and `MenuItemRadio` (egui used `Button`,
    `CheckBox` and `RadioButton`);
  - bar titles are `MenuItem`;
  - the tooltip node is `Role::Tooltip` (egui used `Label`).
  `tests/menus.rs` takes the role through `item(own, …)`, and the tooltip sweep in
  `tests/widgets.rs` accepts `Label` or `Tooltip`.
- **The look comes from UA tables only.** No `theme/base.css` rules were added: the base sheet's
  rules for a tag also restyle the egui wrapper of that tag through the Visuals mapping. Rules move
  to the base sheet with OW-CSS's `::part()` and VIS1c.
- **The egui wrappers are kept** behind `none` (coordinator's instruction), against ARCH-WIDGETS
  §6.2 step 3 (delete in the lane). The `// OW: wrapper until OW3` lines in `menus.rs`,
  `overlays.rs` and `leaf.rs` stay until VIS1c deletes the wrappers.
- **Kittest timing:** kittest's `run` does not wait for `request_repaint_after`, so delay tests
  switch the policy (60 s, then 0) instead of stepping a virtual clock. The submenu step in
  `menus.rs` uses `run_steps(30)`.

## Tests

- New, `crates/svelters-egui/tests/ow3.rs` (10):
  - menu-bar keyboard (F10, roving, submenus, type-ahead, crossing the bar, Escape, `hover`);
  - hover-to-switch and the submenu delay, plus outside dismissal;
  - EV1 filters preventing `activate`;
  - the menu look via PaintTrace (`menu-sel` row, 26px rows, check column, chevron);
  - context-menu keys, flip and focus return;
  - popup flip and dismissal;
  - select keyboard;
  - select pointer, width and chevron;
  - tooltip delay, disabled host and description;
  - menu-button keyboard, R9 and focus return.
- Unit tests: `interact::{anchor, roving, typeahead, tooltip}`.
- Run on **both** implementations (`KindSet::None` and the OW3 tags): every test in
  `tests/menus.rs`, `widgets.rs::combo_box_picks_an_option` and
  `focus_keys.rs::ev3_focus_returns_after_a_menu_closes`.
- `widgets.rs::every_kind_has_a_tooltip_and_inputs_are_named` runs on the new default (own
  tooltips).

### Run locally after the last code change

All local runs used `cargo-slot`.

- `cargo check -p svelters-egui --all-targets`, and `cargo clippy -p svelters-egui
  --all-targets`, which raised no warning in OW3's files.
- nextest `-p svelters-egui`:
  - `--test ow3 --test menus --test focus_keys --test widgets --test own_widgets --test ow1a
    --lib`: all green except 3 tests that need `pnpm install` (`from_source`:
    `ev3_svelte_dialog_from_source`, `ev3_edit_commands_use_the_host_clipboard`,
    `ow0b_svelte_onclicked_reaches_js_through_dispatch`);
  - `--test scene_memory` (3/3);
  - `SVELTERS_UPDATE=1 … --test spec` (4/4), which regenerated `egui.json` and `egui.d.ts`;
  - `python3 tools/docs/egui/gen_reference.py` (combo-box and action pages).

### Not run locally (CI)

- `menus_app.rs`, which needs the built `gallery-egui-menus` fixture;
- the `from_source` tests above;
- the full workspace;
- conformance;
- the other backends;
- `svelters-automation`;
- `examples/egui-kinds`.

## Follow-ups

1. Move menu entries to own kinds once `with_peer` lands.
2. The skipped parity rows above.
3. Base-sheet rules and `::part()` style hosts for menu, select and tooltip parts (with OW-CSS and
   VIS1c).
4. Delete the egui menu, popup, combo and tooltip wrappers in VIS1c.
5. Unify `widgets/glyph.rs` with OW1a's `widgets/parts.rs` path glyphs.
6. Reconcile `Roving` with OW1a2's.
7. Tooltip call sites in the views (`table` cells, `virtual-list` rows, `tabs`, `toast`) belong
   to OW5 and OW7. They still use `on_hover_text`. (`split-view` is OW2's own widget now, so its
   tooltip goes through `tooltip::attach`.)
8. Scrollable tall menus on OW2's `Scrollable` (see "Merge with master (OW3m)").

## Merge with master (OW3m)

Lane OW3m merged master d0542a3a (OW-CSS stage 2, FX4, CS5f, OW2) into `ci/ow3` on 2026-10-22.

- **Conflicts**, all resolved by keeping both sides:
  - the `interact` and `widgets` module lists (`roving` beside OW2's `resize` and `scroll`; `menu`
    and `select` beside `panel`, `scroll_area` and `scrollbar`);
  - `Policy`: OW3's `submenu_delay_ms`, `tooltip_follow_on_ms` and `alt_focuses_menu_bar`, then
    OW2's `scroll: ScrollPolicy`, in all five tables;
  - `widgets/mod.rs`: OW3's `mark_own` beside OW2's `is_own_container`, `Len` and `pick`;
  - `leaf.rs`: OW2 deleted the egui scroll-area wrapper, which OW3 had moved to
    `tooltip::attach`. The deletion wins: the own scroll-area is an own widget, so it gets its
    tooltip through `provider/mod.rs`, which already calls `tooltip::attach`.
- **Generated files** were not hand-merged. `spec` (with `SVELTERS_UPDATE=1`), `gen_props.py`
  and `gen_reference.py` were rerun and changed nothing, and their `--check` runs pass.
- **Tall menus** still do not scroll. OW2's `Scrollable` needs `InputCx`, the bar geometry and
  post-order wheel chaining. A menu level is an `egui::Area` whose rows are scene nodes laid out
  in `Menu::level`, not a taffy subtree. Wiring the two together means:
  - clamping the level to `anchor::bounds`;
  - a per-level offset in the menu state;
  - clipping, and hit-testing only the visible rows;
  - keeping the current row in view on Roving moves;
  - scroller arrows (QMenu `scrollable`).
  That is a feature with its own tests, not a merge fix, so it stays a follow-up (item 8).
- **Run locally after the merge** (all through `cargo-slot`; egui under `headless`):
  - `cargo check -p svelters-egui --all-targets`;
  - `cargo clippy -p svelters-egui --all-targets`, which raised no new warning in the merged
    files;
  - nextest `-p svelters-egui` with `--test menus --test ow3 --test widgets --test own_widgets
    --test ow2 --test style --test style_text --test ow_css --test theme --test focus_keys`:
    80 of 80 green. The `from_source` tests pass now that `pnpm install` ran;
  - `--test spec`, 4 of 4.
  `widgets_panel` failed locally only because the `embed-egui` fixture was not built (CI builds
  it).
- **Not run locally:** `menus_app.rs` and `widgets_panel.rs` (both need built fixtures), the
  full workspace, conformance, the other backends, `svelters-automation` and
  `examples/egui-kinds`.

## Push and CI

See the coordinator report: the sha, the five run ids and the results.
