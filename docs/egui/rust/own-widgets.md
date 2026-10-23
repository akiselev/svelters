---
title: Writing an own widget
description: "Build a widget that svelters draws itself: behaviour units, paint primitives, parts, events and focus."
status: available
verified_at: d0542a3a
sources:
  - crates/svelters-egui/src/provider/mod.rs   # WidgetCx, MeasureCx
  - crates/svelters-egui/src/interact/          # Unit, the units, run
  - crates/svelters-egui/src/paint/             # StyleView, Ua, primitives, painters
  - crates/svelters-egui/src/widgets/           # button, checkbox, ... (worked examples)
  - crates/svelters-macros/src/lib.rs           # #[svelters::widget]
  - plans/milestones/OW0-widget-foundation.md
  - plans/milestones/OW1a-basic-widgets.md
  - plans/milestones/EV1-events.md
  - plans/milestones/EV3-focus-keys.md
  - plans/milestones/SG6-scene-memory-culling.md
---

# Writing an own widget

svelters draws its widgets itself. egui supplies the painter, input, text shaping, layers and AccessKit,
and nothing else. An **own widget** is a `#[svelters::widget]` struct built from three pieces:

- **behaviour units** (`svelters_egui::interact`), which turn input into state bits and typed intents;
- **paint primitives** (`svelters_egui::paint`), which draw boxes, text, icons and outlines from CSS;
- **parts**, the named pieces CSS reaches with `::part(name)`.

Use this page when you need a control the built-in tags don't have (a cadatomic `cad-pin-toggle`, a
knob, a gauge), or when you are porting a built-in tag to an own widget inside svelters-egui.

Don't use it for:

- **plain Rust UI drawn with egui calls.** A custom kind with `Widget::ui` that calls egui is still
  allowed for app kinds. Own widgets are the path when you want CSS states, parts and the theme look.
- **behaviour on an existing element.** Write a `#[svelters::behavior]` (`use:` action) for that.

## The shape of a widget

```rust
// not run
use svelters_egui::prelude::*;
use svelters_egui::interact::{Toggle, ToggleParams, Toggleable};
use svelters_egui::paint::{self, Align, Ua, UaRule, UaValue::*};

/// The host: its gap and the focus ring.
pub const UA: Ua = Ua(&[
    UaRule::base(&[("gap", Px(6.0)), ("outline-style", Keyword("none")),
                   ("outline-color", Token("focus")), ("outline-width", Px(2.0)), ("outline-offset", Px(1.0))]),
    UaRule::when(StateBits::FOCUS_VISIBLE, &[("outline-style", Keyword("solid"))]),
]);
/// `::part(pin)`: the box. Sizes come from tokens or `Px` here, never from Rust constants.
pub const PIN: Ua = Ua(&[
    UaRule::base(&[("width", Px(16.0)), ("height", Px(16.0)), ("background-color", Token("checkbox-bg")),
                   ("border-color", Token("checkbox-border-color")), ("border-width", Token("stroke")),
                   ("border-radius", Px(3.0))]),
    UaRule::when(StateBits::CHECKED, &[("background-color", Token("toggle-on-bg"))]),
]);

/// A pin toggle with a caption.
#[allow(dead_code)] // paint parts are names only
#[svelters::widget(namespace = "cad", tag = "pin-toggle", caps = FOCUSABLE | PAINTS_BOX)]
pub struct PinToggle {
    #[prop(measure)] text: String,
    #[prop(bind)] pinned: bool,
    #[event] changed: Event<bool>,
    #[behavior] hover: svelters_egui::interact::Hoverable,
    #[behavior] focus: svelters_egui::interact::Focusable,
    #[behavior] press: svelters_egui::interact::Pressable,
    #[behavior(params = toggle_params)] toggle: Toggleable,
    #[part] pin: Part,
}

impl PinToggle {
    fn toggle_params(&self) -> ToggleParams { ToggleParams { checked: self.pinned, ..Default::default() } }
}

impl Widget for PinToggle {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, _avail: AvailableSpace) -> egui::Vec2 {
        let s = cx.style(UA);
        let pin = s.part(cx.part_css("pin"), PIN);
        let p = egui::vec2(pin.px("width").unwrap_or(16.0), pin.px("height").unwrap_or(16.0));
        let t = paint::measure_text(cx.ctx, &s, &self.text, AvailableSpace::MaxContent);
        egui::vec2(p.x + s.px("gap").unwrap_or(6.0) + t.x, p.y.max(t.y)) + paint::box_extra(&s)
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        // Toggleable sets :checked from toggle_params(); for a bit no unit owns, call cx.set_state first
        let resp = cx.interact(ui, self.units_sense());          // 1. one egui interact at the node's rect
        let fired = self.run_units(cx, ui, &resp);               // 2. units run; bits restyle this frame
        for Toggle::Toggled { checked, .. } in fired.toggle.iter().copied() {
            if !cx.dispatch(&self.changed, checked).prevented() { // 3. filters first, then the default action
                cx.set(&mut self.pinned, checked);
            }
        }
        if ui.is_rect_visible(cx.rect()) {                       // 4. paint
            let host = cx.style(UA);
            let pin = cx.part(&host, "pin", PIN);
            let geom = cx.paint_box(ui, UA);
            let size = egui::vec2(pin.px("width").unwrap_or(16.0), pin.px("height").unwrap_or(16.0));
            let r = egui::Rect::from_min_size(egui::pos2(geom.content.left(), geom.content.center().y - size.y / 2.0), size);
            let mut pc = cx.paint_cx(ui);
            paint::paint_box(&mut pc.part("pin"), &pin, r);
            let text_rect = geom.content.with_min_x(r.right() + host.px("gap").unwrap_or(6.0));
            let g = paint::text_galley(ui.ctx(), &host, &self.text, None);
            paint::paint_text(&mut pc.part("label"), &host, &g, text_rect, Align::Start);
            paint::paint_outline(&mut pc, &host, cx.rect());     // the focus ring, last
        }
        let (on, enabled, text) = (self.pinned, cx.enabled(), &self.text);
        resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, enabled, on, text));
        resp
    }

    fn a11y(&self, _cx: &A11yCx<'_>, node: &mut egui::accesskit::Node) {
        node.set_role(egui::accesskit::Role::CheckBox);
        self.units_a11y(node);
    }

    fn consume_key(&self, key: egui::Key, m: egui::Modifiers) -> bool {
        self.units_claims(key, m)                                // Space/Enter are ours while focused
    }
}
```

From Svelte: `<pin-toggle text="Pin" bind:pinned onchanged={(v) => …} />`, styled with
`pin-toggle:checked::part(pin) { … }`. The dump (`kinds.json`, `kinds.d.ts`) and linking work as for
any custom kind.

## The macro

`#[svelters::widget(namespace = "cad", tag = "…", caps = A | B)]` on the struct. For own widgets:

| Attribute | What it gives |
|---|---|
| `#[behavior] x: U` | runs unit `U` with `U::Params::default()` |
| `#[behavior(params = method)] x: U` | runs it with `fn method(&self) -> U::Params` (the prop mirror) |
| `#[part] x: Part` | a paint part named `x` (kebab-case): `tag::part(x)` in CSS |
| `#[part(name = "icon")]` | a part with another CSS name |
| `#[part(node)] x: Part` | a node part (a real child node, such as a thumb) |
| `#[prop(enumeration = "a\|b")] x: String` | an enum prop with those values in the typings |
| `replaces = "<tag>"` | **svelters-egui only.** The struct implements that built-in tag. Props and signals are routed to the row by name |
| `style_as = "<tag>"` | the kind also matches the built-in's type selector: the theme styles `cad-slider` like `slider`, and parts of the same name get the built-in's part rules |

The macro generates, from the `#[behavior]` fields in declaration order:

- `self.run_units(cx, ui, &resp) -> <Name>Fired`, with one `Intents<U::Intent>` field per unit
  (named after the field). It also schedules the frames that units ask for (key repeat);
- `self.units_sense()`: the merged `egui::Sense` to pass to `cx.interact`;
- `self.units_a11y(node)`, `self.units_claims(key, mods)`, `self.units_next_frame()`.

Caps that matter here: `FOCUSABLE`, and `PAINTS_BOX`, which tells the walk to paint no box behind the
widget. With `PAINTS_BOX`, the units own `hovered`, `pressed`, `focused` and `focus-visible`, and the walk
sets only `disabled` and `exiting`. A widget with children adds `CONTAINER` (and `CUSTOM_LAYOUT` to lay
them out itself; see `widgets/panel.rs`).

### Replacing a built-in tag (svelters-egui lanes)

`replaces = "checkbox"` keeps the built-in row (tag, ids, spec, typings) and routes it to the struct:

- every row prop not stored in `Common`, and every row signal, needs a field of the same name (a
  startup check);
- **superset extras**: props and events the row lacks become extras of the kind. They get ids after
  the row's, appear in `egui.json` and the typings, and the old egui wrapper ignores them. Regenerate
  with `SVELTERS_UPDATE=1 cargo test -p svelters-egui --test spec` and
  `python3 tools/docs/egui/gen_reference.py`.

Which implementation a new node gets is the own-widget switch, `widgets::KindSet`:

| Set with | Values |
|---|---|
| `EguiBackend::set_own_widgets(KindSet::…)` (new nodes only) | `Default`, `All`, `None`, `Tags(..)` / `KindSet::tags(["checkbox"])` |
| `SVELTERS_OWN_WIDGETS` (read when the backend is created) | `all`, `none`, `default`, or `tag,tag` |

`DEFAULT_OWN` lists the tags whose own widget is the default; `OWN_ONLY` lists tags whose egui wrapper is
gone (`panel`, `scroll-area`, `scrollbar`, `split-view`). Check both in `widgets/mod.rs` at your pin.

## Behaviour units

| Unit | Owns (bits) | Intents | Params |
|---|---|---|---|
| `Hoverable` | `hovered` (stays hoverable while disabled) | `Hover` | — |
| `Pressable` | `pressed` | `Press::{Down, Up{inside}, Cancel, Click{count, button, mods}, Repeat}` | `PressParams { activate_on, repeat: Option<(delay, interval)> }` |
| `Focusable` | `focused`, `focus-visible` (keyboard or program focus only) | `Focus` | `FocusParams` |
| `Toggleable` | `checked`, `indeterminate` | `Toggle::Toggled { checked, indeterminate }` | `ToggleParams { checked, indeterminate, tristate, radio }` |
| `Resizable`, `Scrollable` | see `interact/resize.rs`, `interact/scroll.rs` (used by `split-view`, `scroll-area`) | | |
| `Expandable` | `expanded` | `Expand::Set(bool)`: Right/Left, AccessKit `Expand`/`Collapse`, and a click when `on_click` | `ExpandParams { expanded, on_click }` |
| `Roving` | — | `Rove::{By(i32), First, Last}`; `Rove::target(at, len)` wraps | `RovingParams { on }` |
| `RovingList` | — | `Move { to }` | `RovingListParams { enabled: Vec<bool>, orientation, wrap }` |

`Press::Click` covers pointer clicks, Space/Enter and AccessKit `Click`. A click is `Click { count: 1, .. }`
first; a double click then sends `count: 2`. Roving only reports the move: the widget finds the members,
skips disabled ones, and moves focus and checks through `with_peer` (below).

**`Roving` or `RovingList`?** Both make a group one Tab stop and move with the arrows, Home and End.

- `Roving` moves **focus across sibling nodes**, each its own widget: radio buttons,
  `toggle-button group`.
- `RovingList` moves a **highlighted index inside one widget**: menu rows, a menu bar, a select's
  options, list, tree and tab rows. It skips items whose `enabled` entry is false, wraps when `wrap` is
  set, and follows `orientation` (`Vertical`/`Horizontal`). The widget reads `current()`, sets it on hover
  with `set(Some(i))`, and can use the helpers `RovingList::edge(&enabled, forward)` and
  `RovingList::next(&enabled, from, step, wrap)` without the unit.

### Floating surfaces and tooltips

These units serve popups, menus, select lists and tooltips (`widgets/menu.rs`, `select.rs`,
`popup.rs`, `tooltip.rs` are the worked examples). None of them owns a state bit. The widget owns
`open` and decides.

| Unit | Reports | Use |
|---|---|---|
| `Anchored` | `Placed { side, rect }` | where a surface goes beside its anchor. `AnchorParams { anchor, size, bounds, placement, gap, cross, flip, shift, match_anchor_width }`, `Placement::parse("bottom-start")`. `interact::anchor::place(&p)` is the geometry alone; `anchor::bounds(ctx)` gives the bounds |
| `Dismissable` | `Dismiss { reason, at }`, with `Reason::{Escape, OutsidePress, FocusOut, AnchorHidden, Programmatic}` | asks to close on Escape (claimed when `escape`) or on a press outside `inside` rects (`Outside::{Close, Ignore}`). `Dismissable::check(&p, &input)` without the unit |
| `Typeahead` | `Search { text }` | printable keys within `timeout` (1 s) build a prefix. `typeahead::find(labels, &enabled, query, current)` picks the next match; typing one letter again cycles |
| `Tooltipable` | `Tip::{Show, Hide}` | the delay (`Policy::tooltip_delay_ms`), the follow-on window, a press hiding it until the pointer leaves, and showing on disabled hosts (R9) |

**Tooltips need no code.** The common `tooltip` prop works on every own widget: the provider passes it
to the shared tooltip (`widgets/tooltip.rs`, crate-level `attach`). The innermost host's tooltip is
painted once per pass below the pointer, and the text becomes the host's AccessKit description.
`widgets::tooltip::{show(ctx, text, at), hide(ctx), is_shown(ctx)}` are for showing one by hand.

**A new unit** goes in `interact/<unit>.rs` and implements `Unit` (`Params`, `Intent`, `OWNS`, `sense`,
`input`, `a11y`, `claims`, `next_frame`). Inside `input`, read `cx.resp` (a `Resp` snapshot) and
`cx.input`. Never call `Response` or `Context` methods there: egui's context lock is held. Request focus
changes through `cx.focus`.

**Without a scene node** (berth chrome, immediate Rust UI): `interact::run(ui, &resp, &mut bits,
&PartResponses::default(), |cx| unit.input(&params, cx, &mut out))`. You keep the bits between frames.
Paint with `StyleView::ua_only(UA, bits, dark)` and the same primitives.

## Paint primitives

Every primitive reads a `paint::StyleView`. A property resolves from the computed CSS (with running
transitions), then the widget's UA table for the current state, then the parent view (inherited
properties only), then `Ua::ROOT`.

| Need | Call |
|---|---|
| The host's view | `cx.style(UA)` (paint), `cx.style(UA)` on `MeasureCx` (measure, at rest) |
| A part's view | `cx.part(&host, "pin", PIN_UA)` (paint), `s.part(cx.part_css("pin"), PIN_UA)` (measure) |
| Box: shadows, fill, gradient or `paint()`, border | `cx.paint_box(ui, UA) -> BoxGeom { border, padding, content }`, or `paint::paint_box(&mut pc, &s, rect)` |
| Padding + border for `measure` | `paint::box_extra(&s)`, `paint::box_geom` |
| Text | `paint::text_galley`, `paint::paint_text(&mut pc, &s, &g, rect, Align)`, `paint::measure_text` |
| Icon or `content:` glyph | `paint::content_icon(&s, &default)`, `paint::icon_size`, `paint::paint_icon` |
| Image | `paint::paint_image` (`object-fit`, `opacity`) |
| Focus ring | `paint::paint_outline` / `cx.paint_outline(ui, UA)`, **last** |
| Parts as flex items of the content box | `paint::measure_parts`, `paint::layout_parts` with `PartItem`s (`flex-direction`, `gap`, `order`, `display: none`) |

`cx.paint_cx(ui)` gives a `PaintCx` that traces to the node. `pc.part("pin")` retargets it to a part, so
painters and the debug trace (`paint::trace`) see `(node, part)`. Snap with `pc.snap_rect` /
`pc.snap_width`. Apply `s.cursor()` while hovered and `s.opacity()`.

**UA tables** are the look with no sheet. Values are `Token("button-bg")`, `Px`, `Color`, `Number`,
`Keyword` or `Shadows`. Rules are `UaRule::base`, `UaRule::when(bits, …)` and
`UaRule::when_not(bits, unless, …)`; later rules win. There are no egui `Visuals` or `Spacing`, and no
size constants in Rust.

**Part painters.** A part (or the host box) can be painted by a Rust painter that CSS picks:

```rust
// not run
svelters_egui::paint::register_painter("cad.knurl", |cx: &mut PaintCx, s: &StyleView, r: egui::Rect,
                                                     part: &PartInfo<'_>, args: &PaintArgs<'_>| {
    let pitch = s.px("--knurl-pitch").unwrap_or(2.0);
    // pure paint: no input, no layout, no scene writes
});
// or EmbedConfig::painter("cad.knurl", p)
```

Pass `PaintArgs` (`Value`, `Progress`, `Expanded`, `Custom`) through `pc.args` and say per part which you
pass. The styling side is in [Styling own widgets](../css/own-widgets.md).

## Events: dispatch, then the default action

`cx.dispatch(&self.event, payload) -> Dispatched` runs the scene's Rust filters (`EventFilter`s and
ancestors' `Behavior::filter`, capture → target → bubble) **before** your default action, then queues the
JS listener call (delivered after the pass), unless a filter stopped it. Apply the default action only
when the outcome is not prevented:

```rust
// not run
if !cx.dispatch(&self.clicked, ()).prevented() { self.activate(cx) }
```

`cx.emit` queues an event with no filters; use it only for notifications with no default action.
`cx.set(&mut self.field, v)` writes a `#[prop]` and announces it to `bind:`; a write to a non-prop field
is reported as an error.

### Writing to sibling widgets

`cx.with_peer::<Wd, R>(id, |w: &mut Wd, peer: &mut PeerCx| …) -> Option<R>` writes to another own widget
of type `Wd` in the same pass: `peer.set(&mut w.checked, false)`, `peer.emit(&w.changed, …)`,
`peer.focus()`. The writes are announced and the events queued exactly like the peer's own, and a peer
drawn earlier in the pass is repainted. It returns `None` for this node, a dead node, or another type.
Find members with `cx.sibling_ids()` and `cx.tag_of(id)`. Radio exclusivity, `toggle-button group` and
Roving use it (`widgets/radio.rs`). It is crate-level: `pub` items in a `pub(crate)` seam,
not re-exported for app crates.

## Focus and keys

- `Focusable` sets `focused` and `focus-visible`; style the ring with `:focus-visible`.
- `consume_key(key, mods)`: return true for keys your widget takes while focused. Claimed keys are
  removed from the pass's input, so the host keymap and menu shortcuts after the content don't see them,
  and a host keymap that runs earlier sees `embed.shortcut_override(ctx, &chord) == true`
  (`Embed`, or `EguiBackend`). With units,
  return `self.units_claims(key, m)` (Pressable claims Space/Enter; Roving claims the arrows,
  Home/End).
- A part that is its own Tab stop (a sash) records itself with `cx.record_part_focus(ui, &resp)`. Inside
  svelters-egui the general form is `focus::record_with(arena, i, ui, resp, Role, tab_stop)`
  (crate-private).
- `Widget::composing()`: return true while an input method composes, so Enter/Escape/Tab go to it.

## The paint cache

`cx.cached_shapes(painter, version, |painter, rect, shapes| …) -> bool` keeps your shapes, built for a
rect of the node's size at the origin, and replays them at the node's rect. The entry is kept while
`version`, the size, `pixels_per_point` and the theme stay the same. It saves shape building and text
layout, not tessellation.

- Bump `version` when what you draw changes for a reason the cache can't see (your own fields).
- A prop write, a restyle that changes the node's or its parts' style, and every frame in which one of
  its CSS transitions runs (including the last) drop the entry. You don't handle those.
- Don't cache what depends on state bits you paint without a restyle (for example a hover drawn from
  `cx.state(..)` with no CSS rule for it): bump `version` from the bits.

## Check it

- Run the kind's tests with the own widget on and off (`KindSet::All` and `KindSet::None`), as
  `crates/svelters-egui/tests/own_widgets.rs` does.
- `paint::trace::enable(ctx, true)` and `paint::trace::last(ctx)` give `TraceEntry { node, part, kind,
  rect, fill, stroke, font_size }` in debug builds; `PaintTrace::find(node, part, kind)` finds one.

## Worked examples

| File (master) | Shows |
|---|---|
| `widgets/button.rs` | parts laid out by P3 (`icon`, `label`), `cx.part` (computed `::part()` styles), `repeat`, extra events |
| `widgets/checkbox.rs` | Toggleable, an indicator part and a path mark, `dispatch(..).prevented()` |
| `widgets/switch.rs`, `widgets/progress.rs` | parts with a knob or fill, reduced motion |
| `widgets/scroll_area.rs`, `widgets/split_view.rs` | containers: `record_hit`, `children_offset`, `run_units_with`, `record_part_focus` |
| `widgets/radio.rs`, `widgets/collapsing.rs` | Roving and `with_peer`; Expandable and a container with a collapsed body |
| `widgets/menu.rs`, `widgets/select.rs`, `widgets/popup.rs` | RovingList, Typeahead, Anchored, Dismissable |

## Mistakes we have seen

- Calling `Response` methods inside a unit's `input`: deadlock on egui's context lock. Use `cx.resp`.
- Applying the default action before `dispatch`, or ignoring `prevented()`: Rust filters can't veto it.
- Painting the focus ring before the content: the fill covers it. `paint_outline` goes last.
- Hard-coding sizes in Rust: they don't follow density or CSS. Put them in the UA table.
