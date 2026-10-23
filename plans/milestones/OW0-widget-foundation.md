# OW0: the own-widget foundation (paint primitives, behaviour units, `button`)

Lane OW0, started 2026-10-22 on master a0e109e0 (ARCH-WIDGETS merged), merged with master 91fcf421 before the
OW0b push. Spec: `plans/research/ARCH-WIDGETS.md` §1–§3, §6, §7.1 (OW0a/OW0b), and its "Required reading" (DESIGN §1,
§2 R1–R18, §4; ICON-STYLE; VIS1). Later scope from `plans/research/WIDGET-PARITY.md` "Decided" (Q1 painters,
`style_as`, Policy overrides, P2/P3/P8, §2.1 button rows). The user's decision: svelters draws every widget itself, and
egui is only the engine.

- OW0a (the API slice) was pushed as `ci/ow0` at **7eef7362**. OW1a, OW1b and OW2 branch from it.
- OW0b is this record's last push (see "Push and CI").

## What landed

### 1. Paint primitives (`crates/svelters-egui/src/paint/`)

- **`StyleView`** is what every primitive reads. A property resolves in this order:
  1. the node's (or part's) computed style;
  2. the widget's **UA table** (`Ua`: `UaRule`s by state, values from the built-in VS Code theme's tokens);
  3. for inherited properties only, the parent view (a part's host);
  4. `Ua::ROOT` (foreground colour, body font).

  Getters: `color`, `px`, `number`, `keyword`, `font_weight`, `font_size`, `shadows` (CSS lists, `elev-*`
  tokens, literal lists), `cursor`, `opacity`, `is_hidden`, `sides(group)`.

  Constructors: `StyleView::new(css, ua, state, dark)`, `StyleView::ua_only(ua, state, dark)` (no cascade), and
  `host.part(css, ua)` for a part.

  DS2's transitions plug in here; today the view reads the computed value.
- **`PaintCx`** holds the painter, `ppp` (snapping: `snap_rect`, `snap_width`), the trace target `(node, part)`,
  `tag` and `args` (for part painters).
  - `PaintCx::new(painter)` needs no scene. `PaintCx::for_node` is the scene's version.
  - `cx.part("thumb")` re-targets it to a part.
- The **primitives**:
  - `paint_box`: outer shadows, fill, inset shadows, a `paint()` painter, uniform border (one `RectShape`,
    `StrokeKind::Inside`); returns a `BoxGeom` (border, padding and content boxes);
  - `box_geom`, `box_extra` (padding + border, for `measure`);
  - `paint_outline` (`outline-*`, outside the box, the focus ring);
  - `text_galley`, `paint_text` (`Align`), `measure_text` (`line-height`);
  - `paint_icon` (icon font; `content: icon(<key>)`, `content: none`, `content: paint(<name>)`, the P2 glyph
    rule), `icon_size`, `content_icon`;
  - `paint_image` (`object-fit`, `opacity`).
- **P3 part layout**: `layout_parts`, `measure_parts` and `PartItem`. Paint parts are flex items of the host's
  content box, controlled by `flex-direction`, `gap`, `justify-content`, and each part's `order`, `width`/`height`,
  `margin` and `display: none`.
- **Part painters** (WIDGET-PARITY Q1, the QProxyStyle equivalent):
  - API: `PartPainter`, `register_painter(name, painter)` (a closure works), `PartInfo`, `PaintArgs`
    (`#[non_exhaustive]`), `EmbedConfig::painter(name, p)`.
  - `background-image: paint(name)` paints between the inset shadows and the border, in the padding box.
    `content: paint(name)` replaces a glyph.
  - An unknown name falls back to the default paint and is reported once.
  - The values are read as raw text until OW-CSS parses `paint()`.
- **PaintTrace** (debug builds only):
  - `paint::trace::enable(ctx, true)` turns it on and `paint::trace::last(ctx)` returns the last pass's
    `TraceEntry { node, part, kind: Box|Text|Icon|Image|Outline, rect, fill, stroke, font_size }`.
  - `PaintTrace::find(node, part, kind)` is VIS1c's lookup by `(node, part)`.
  - The walk's own boxes (containers and non-stock leaves) are painted through `paint_box` too, so they are
    traced.

### 2. Behaviour units (`crates/svelters-egui/src/interact/`)

- **`trait Unit`** declares `Params`, `Intent`, `OWNS`, and the methods `sense`, `input`, `a11y`, `claims` (EV3
  R11) and `next_frame`.
- **`InputCx`** gives a unit:
  - `resp: &Resp`. This is a snapshot of the egui `Response`, because `Response` methods take egui's context
    lock, which is held while units read the raw input.
  - `input: &InputState`, `policy`, `now`, `state` (the host's bits, written in place), `enabled`, `a11y`
    (AccessKit requests for the host), `focus: &mut FocusCx` (requests deferred until after the lock),
    `parts`.
- **`interact::run(ui, &resp, &mut StateBits, &parts, |cx| …)`** is the scene-free path, for OW-RUST and
  cadatomic's berth `ChromePainter`. **`UseUnit<U>`** is the `use:` adapter's shape.
- The units:
  - **Hoverable** owns `hovered` and stays hoverable while disabled (R9).
  - **Pressable** owns `pressed`. Intents: `Down`, `Up{inside}`, `Cancel`, `Click{count, button, mods}`
    (egui's clicks, including Space/Enter and AccessKit `Click`), and `Repeat` (`PressParams::repeat`, which
    asks for frames through `next_frame`). It claims Space/Enter.
  - **Focusable** owns `focused` and `focus-visible`.
    - It honours the policy's `focus_on_click`.
    - `focus-visible` is set when focus came from the keyboard or the program, never from a pointer (FX1).

### 3. The macro (`#[svelters::widget]`, svelters-macros)

- `#[behavior] x: U` and `#[behavior(params = method)]`. The macro generates:
  - `self.run_units(cx, ui, &resp) -> <Name>Fired` (one `Intents` field per unit; it also schedules
    `next_frame` repaints);
  - `units_sense()`, `units_a11y(node)`, `units_claims(key, mods)`, `units_next_frame()`.
- `#[part]`, `#[part(node)]` and `#[part(name = "icon")]` are stored as `PartEntry`s on `KindEntry::parts`.
  Paint parts are names only; node parts are created by OW1b and later.
- `replaces = "<tag>"` (svelters-egui only): the tag is the built-in's, and there is no export-schema module.
- `style_as = "<tag>"`: the kind also matches the built-in's type selector. svelters-style gains a defaulted
  `Element::style_as`, checked by `has_local_name` and the rule index.
- `#[prop(enumeration = "a|b")]`. Each `PropEntry` and `EventEntry` records its kind-table type (`RowTy`), which
  is used for extras.

### 4. State bits 14–22 (`provider/state.rs`)

- The new bits are `focus-visible`, `indeterminate`, `placeholder-shown`, `default`, `drop-target`, `current`,
  `backdrop`, `required` and `user-invalid`, with the names and order of §2.2.
- OW-CSS adds the same rows to svelters-style. On a textual conflict, take OW-CSS's rows.
- The pin test (`style.rs::state_bit_tables_are_pinned_together`) lets one table lead by bits 14–22 until both
  lanes are merged.
- `Element::is_focus_visible` now reads the bit. Stock leaves set it together with `focused`, so their behaviour
  is unchanged.
- The automation snapshot treats `focus-visible` as transient, like `focused`.

### 5. Replacement without a flag day (`kinds/registry.rs`, `widgets/mod.rs`)

- **`Replacement`** routes a `replaces` struct to the built-in row by name.
  - The startup check requires every row prop not stored in `Common` (`default`/`cancel` stay in `Common` and EV3
    reads them there) and every row signal to have a field of the same name.
  - Props and events the row lacks become **extras** of the kind:
    - ids follow the row's;
    - `find_prop`, `prop_def`, `find_signal` and `signal_def` resolve them;
    - `KindRegistry::spec_kind` appends them for egui.json and the typings;
    - the old egui wrapper ignores them (set is a no-op, get returns the default).
  - Parity props no longer touch the KINDS row or the wrapper. This answers OW1a's request.
- **Switch**:
  - `KindSet {Default, All, None, Tags}`, set with `EguiBackend::set_own_widgets` or
    `SVELTERS_OWN_WIDGETS=all|none|default|tag,…`; it applies to nodes created afterwards.
  - `DEFAULT_OWN = ["button"]`.
  - The `button` egui wrapper stays reachable with `none` until VIS1c confirms the look (see Not done).
- Built-in prop routing (`scene/props.rs`) goes to `set_replaced_prop`/`get_replaced_prop`. `Node::text` reads a
  text mirror kept on `ExtWidget`, so names, queries and a11y see the caption.
- An own widget that replaces a leaf stays a layout leaf: its children are attached overlays, such as a context
  menu.
- `focus.rs` gives a replaced `button` EV3's `Role::Button` (default and cancel keys). The automation crate maps
  `W::Ext` roles by tag.

### 6. `WidgetCx` and `MeasureCx` (provider)

- `Caps::PAINTS_BOX`: the walk paints no box behind the widget and sets only `disabled`/`exiting`. The units own
  `hovered`, `pressed`, `focused` and `focus-visible`. Without a sheet, `run_units` mirrors `disabled` itself.
- `WidgetCx` gains:
  - `interact(ui, sense)` and `run_units`;
  - `commit_state()`, which writes the bits now, restyles what they reach in the same frame and moves
    `:focus-within`;
  - `style(ua)`, `part_css(name)`, `paint_cx(ui)`, `paint_box`, `paint_outline`, `paint_text`;
  - `dispatch(&event, payload) -> Dispatched{prevented, stopped}`. This is EV1's seam: a direct emit today,
    with the same signature once EV1 runs filters before the default action.
- `MeasureCx::style(ua)` and `MeasureCx::part_css(name)`. Layout passes the node's computed style and part
  styles, so `measure` is content + padding + border, with no egui `Spacing`.
- OW-CSS hooks: `SceneStyle::part_computed(id, part)` and `part_styles(id)` return nothing until the engine
  computes `::part()` styles.
- R9: a disabled own widget's tooltip shows (`on_disabled_hover_text`).

### 7. Policy (`native/mod.rs`)

- `Native::Windows` and `Native::Mac` rows come from the platform guidelines, as ARCH-WIDGETS W6 says, until WG3w
  and WG3m pin them. They are **not yet the default** on those OSes.
- `Policy` is `#[non_exhaustive]` (it is `PartialEq` only now). It gains `drag_threshold`, `double_click_ms`,
  `tooltip_delay_ms`, `caret_blink_ms`, `kinetic`, `scrollbars`, `space_activates_on` and `button_order`.
- `native::override_policy(ctx, |p| …)`, `clear_policy_override` and `policy_of(ctx)`, plus
  `EmbedConfig::with_policy(f)`. This is WIDGET-PARITY §4.3's `styleHint` equivalent.

### 8. `button`, own (`widgets/button.rs`), the default

- Props, routed from the row: `text`, `selected`, `frame`, `small`, `font-weight`. Extras: `icon` (key, or
  `family:name`) and `repeat` (300 ms, then every 100 ms).
- Signals: `clicked`, `double-clicked`, `secondary-clicked`, plus the extras `press` and `release` (P8).
- Paint parts:
  - `::part(icon)` (UA 16px);
  - `::part(label)`, laid out by P3. `button { flex-direction: column }` gives text under the icon;
    `::part(label) { display: none }` gives an icon-only button.
- States:
  - `:hover`, `:active`, `:focus`, `:focus-visible`, `:selected` and `:default` (prop mirror);
  - `:disabled` (also without a sheet).
- AccessKit: `Role::Button`, named by its text. It handles `Click` and `Focus`, and reports `toggled`/`selected`
  as egui's button did.
- **DESIGN rules met with the default theme** (UA table, and the theme's base sheet where it sets the same
  properties):

  | Rule | How |
  |---|---|
  | R1 raised | `button-bg` fill (`button2-bg` with the affordance overlay, never transparent), `button-border-color` boundary (`control-border`), radius `button-radius` (4), the lip `box-shadow: elev-1` |
  | R2 pressed sinks | `:active`: fill `button-bg-active` (`toolbar-active`), no lip, `inset 0 1px 2px rgba(0,0,0,.25)`, no transition |
  | R3 hover | `:hover` fill `button-bg-hover`, same frame (`commit_state`), lip kept |
  | R8 focus | keyboard-only (`:focus-visible`), 2px `focus` outline at offset +1, outside the fill |
  | R9 disabled | opacity .4, still hoverable, tooltip shown, `cursor: not-allowed`, no activation |
  | R10 cursor | `pointer` while hovered |
  | §1.3 metrics | padding 4/8 (`button-pad-y/x`), 16px lines, 1px border: 26px tall; 12px text (VIS1's measured `fs`) |

- **VIS1 checks this targets** (`button-secondary`, the gate on `ci/vis1`):
  - met by construction: `rest|hover|focus/h` 26, `r` 4, `b` 1, `fs` 12, and `fg` (`button-fg`);
  - `focus/oc` = `focus`, which closes VIS1 gap 1, since the outline is ours;
  - `bg`/`bc` follow the theme roles.
  - VS Code's own secondary button is transparent at rest, while R1 gives it the `button2-bg` fill (affordances
    are on by default, a user decision). VIS1c's baseline for `button-secondary/rest/bg` must use the affordance
    theme, or carry a recorded exception.
  - Not run here: VIS1c merges the gate and switches it to `PaintTrace`.

### 9. The lint (`crates/svelters-egui/clippy.toml`, `lib.rs`)

- `disallowed-types` lists egui's widgets (Button, TextEdit, Checkbox, RadioButton, Slider, DragValue, ComboBox,
  ScrollArea, CollapsingHeader, Popup, Modal, Window, ProgressBar, Spinner, Link, Hyperlink, Label, Separator,
  MenuBar, menu::{MenuButton, SubMenuButton, SubMenu}), `egui_extras::DatePickerButton` and `egui_table::Table`.
- The library denies them (`#![cfg_attr(not(test), deny(clippy::disallowed_types))]`); tests and bins are
  allowed.
- **Burn-down: 23** `#[allow(clippy::disallowed_types)] // OW: wrapper until <lane>` lines, which reach 0 at
  OW-DONE. Count them with `grep -rn "OW: wrapper until" crates/svelters-egui/src | wc -l`.

  | File | Lanes |
  |---|---|
  | `leaf.rs` (3) | OW0 (the `button` wrapper behind the switch), OW1a, OW1b, OW2, OW3, OW4 |
  | `menus.rs` (5) | OW1a, OW3 |
  | `elements/inputs.rs` (3), `elements/media.rs` (1) | OW1a, OW1b |
  | `numeric.rs` (3) | OW1b |
  | `views/table.rs` (4) | OW1a, OW4, OW5b |
  | `views/tabs.rs` (1) | OW2, OW5c |
  | `views/toast.rs` (1) | OW1a, OW7 |
  | `overlays.rs` (1) | OW3, OW7 |
  | `kinds/pickers.rs` (1) | OW6 |

## Tests

`crates/svelters-egui/tests/own_widgets.rs` (13):

- `ow0_*`, which run on **both** implementations through `KindSet`:
  - the row routing;
  - props round-trip (including null resets and `Node::text`);
  - click, Space, Enter and AccessKit `Click`, a disabled button, and a11y role and name;
  - default and cancel (EV3);
  - own-only: hover, press, focus by pointer (no `focus-visible`) and by Tab (`focus-visible`);
  - CSS sizing.
- `ow0b_*`:
  - the extras rule, using a test-only `separator` replacement with an extra prop and an extra event;
  - the PaintTrace box, label part and focus ring: R1 fill and boundary, 12px text, R8 2px ring outside the box,
    no ring after a click;
  - the disabled R9 states and AccessKit `disabled`;
  - `press`, `release` and `repeat`;
  - a CSS-selected part painter, called scene-free through `PaintCx::new` (the ChromePainter shape), plus the
    unknown-name fallback;
  - the policy override;
  - `onclicked` from compiled Svelte reaching JS through `dispatch` (the FocusDialog fixture).

Run locally:

- `own_widgets` (13/13);
- `spec` (regenerated `metadata/semantics/egui.json`, `packages/native-runtime/types/egui.d.ts` and
  `docs/egui/reference/widgets/button.md` with `gen_reference.py`);
- with the own button as the default:
  - `animator`, `native_focus`, `tier0`, `style_shadow`, `window`, `menus`, `theme`, `focus_keys`, `style`,
    `scene`, `widgets`, `observe`, `passes`, `views`, `queries` (all green apart from tests that need built
    fixtures);
  - `svelters-automation` (6/6).

Not run locally: tests that need `pnpm build` fixtures (`counter`, `embed`, `motion`, `wg4b` gallery, `queries`
two-roots), the full workspace suite and conformance. CI runs them.

## Write a widget (the guide for OW1a, OW1b, OW2 and later)

1. **A file per widget** in `crates/svelters-egui/src/widgets/<tag>.rs`, added to `widgets/mod.rs`.
2. **The struct** declares its props, events, units and parts:

   ```rust
   #[svelters::widget(namespace = "sv", replaces = "checkbox", caps = FOCUSABLE | PAINTS_BOX)]
   pub struct Checkbox {
       #[prop(measure)] text: String,           // every non-Common prop of the KINDS row, same names
       #[prop(bind)] checked: bool,             // bound props: `cx.set(&mut self.checked, v)` announces
       #[prop] tristate: bool,                  // a prop the row lacks = an extra (spec regenerated)
       #[event] changed: Event<bool>,           // every row signal, same names; extras allowed
       #[behavior] hover: Hoverable,
       #[behavior] focus: Focusable,
       #[behavior(params = press_params)] pressable: Pressable,
       #[part(name = "indicator")] indicator: Part,   // paint parts: CSS `checkbox::part(indicator)`
       #[part(name = "mark")] mark: Part,
   }
   ```

   Enums are `#[prop(enumeration = "horizontal|vertical")] orientation: String`.
3. **The UA table** is the look with no sheet: a `pub const UA: Ua` from theme tokens (DESIGN §1.5 roles, the R
   rules). Parts get their own tables (`ICON_UA`). There are no egui `Visuals` or `Spacing`, and no size
   constants in Rust (P9): sizes are tokens or `Px` in the UA table.
4. **`measure`**:
   - `let s = cx.style(UA)`, then the parts' views (`s.part(cx.part_css("mark"), MARK_UA)`);
   - content sizes (`paint::measure_text`, `paint::icon_size`), arranged with `paint::measure_parts(&s,
     &items)`;
   - plus `paint::box_extra(&s)`.
5. **`ui`**:
   1. Mirror the props into state bits (`cx.set_state(StateBits::CHECKED, self.checked)`).
   2. `let resp = cx.interact(ui, self.units_sense())`.
   3. `let fired = self.run_units(cx, ui, &resp)`. Bits are committed and restyled in the same frame.
   4. For each user event, `cx.dispatch(&self.changed, v)`, then the default action unless
      `dispatched.prevented` (`cx.set(&mut self.checked, …)`).
   5. Paint: `cx.paint_box(ui, UA)` → `layout_parts` → `paint_icon`/`paint_text` with `cx.paint_cx(ui).part("mark")`
      → `cx.paint_outline(ui, UA)` last.
   6. Set the cursor from `style.cursor()`, apply `opacity`, and call `resp.widget_info(...)` for the a11y name.
6. **`a11y`**: set the role, then `self.units_a11y(node)`, then the widget's states (toggled, value).
   **`consume_key`**: `self.units_claims(key, m)`.
7. **New units** go in `interact/<unit>.rs` with `Unit` implemented. They read `cx.resp` (a `Resp` snapshot) and
   never call `Response` or `Context` methods inside `input`. Focus changes go through `cx.focus.request()` and
   `surrender()`. Intents are a plain enum per unit.
8. **Switch and acceptance**:
   - Add the tag to `DEFAULT_OWN` in the commit whose acceptance passes, after running the kind's existing tests
     on both `KindSet`s (as `tests/own_widgets.rs` does).
   - Delete the egui wrapper and its `// OW: wrapper until` allow line in the same lane.
   - Regenerate egui.json, the typings and the reference docs if the struct adds extras
     (`SVELTERS_UPDATE=1 … --test spec`, `python3 tools/docs/egui/gen_reference.py`).
9. **Part painters**: document the `PaintArgs` your parts pass (`cx.paint_cx(ui)` and set `pc.args`). The
   primitives call a CSS-selected painter for you.
10. **Scene-free use** (OW-RUST, ChromePainter): keep the widget's paint in a function of
    `(PaintCx, StyleView, rect, …)` and its behaviour in units driven by `interact::run`. A foreign painter then
    calls the same code with `StyleView::ua_only(UA, state, dark)` (or a theme's computed style) and a `Response`
    of its own.
    OW-RUST did this for OW1a's kinds: `pub(crate) fn paint`/`measure` in the widget file, called by the kind's
    `ui` and by its `svelters_egui::imm` builder (`src/imm/widgets.rs`). Give a new kind both, add its `imm`
    builder, and add a parity case to `tests/imm.rs` (plans/milestones/OW-RUST-immediate.md).

## Not done (and where it goes)

- **Delete the egui `button` wrapper.** Today it is reachable only through `KindSet::None` and
  `SVELTERS_OWN_WIDGETS=none`. It goes once CI and VIS1c have confirmed the own look, in a follow-up commit of
  this lane or in VIS1c, together with the leaf.rs `W::Button` arm and the `layout.rs` size copy.
- **Per-side borders, per-corner radii, dashed borders and 2-stop gradients** (OW0b in ARCH-WIDGETS §7.1) go to
  OW-CSS with the engine values. The primitive paints a uniform border from the widest side.
- **Text elision** (`elide`, WG6 QW13), `letter-spacing` and `text-transform` go to OW1a (label), which needs
  them first.
- **The `use:` export of the three units** (`svelters:behaviors/sv`). The `UseUnit<U>` shape exists, and
  registering it through EV2b's attached kinds is OW1a's (the first composite users).
- **Windows/Mac as defaults**: `Native::for_os` still maps them to `Egui` until WG3w and WG3m pin the rows.
- **`url(asset://…)` in `content`**: AS1's loaders exist on master, and the icon primitive reads only `icon()`
  and `paint()` today. This is ICR1's swap.
- **`::part()` computed styles** are OW-CSS's: fill `SceneStyle::part_computed` and `part_styles`. Until then,
  parts paint from their UA tables, inheriting the host's CSS.

## Push and CI

- OW0a: `ci/ow0` at 7eef7362.
  - CI runs: macos-appkit 39839133061 (success), linux-qt 39839133095 (success), js 39839133054 (success),
    windows-winui 39839133044 (success), linux-gtk 39839133058 (failure).
  - The linux-gtk failure was `svelters-automation::counter::acts_reach_js_and_reply_after_settle`: the snapshot
    printed `[focus-visible]`. This is fixed in OW0b.
- OW0b first push, 6f013c72.
  - CI runs: js 39845196088, macos-appkit 39845196221, linux-qt 39845196212 and windows-winui 39845196219
    succeeded; linux-gtk 39845196138 failed on two tests.
  - The TK1 egui snapshots `counter-three` and `transition-back` changed, because the button now has its own
    look. They were regenerated with `UPDATE_SNAPSHOTS=1 … -p svelters-testkit --features snapshots` on lavapipe,
    as §6.2 step 4 says.
  - `viewports::dock_w2_without_the_prepass_plugin_input_costs_one_root_frame` saw 4 root frames, not 1–2. The
    own button takes focus on the press under the node's id, so the embed now sees the slot gain focus and runs
    one more logic frame for `root().focused`. The stock button focused an auto id that the embed never matched.
    The test now allows the handler frame plus the focus frame.
  - The own widgets' interaction bits are no longer journaled when no sheet runs the cascade, and they are
    journaled with the walk's `Style` origin when one does.
- OW0b second push: see the coordinator report (sha and run ids).
