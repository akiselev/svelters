# ARCH-WIDGETS: svelters' own widget system on egui-as-engine

Lane OW0-design (research, docs only). Owner: svelters-agent. Date: 2026-10-22.

User decisions this design implements (2026-10-22):

- "We want complete control of rendering and behavior, with our composition framework to handle
  abstractions and behaviors to reduce development work."
- svelters draws every widget itself. egui stays only as the **engine**: painter (epaint shapes,
  tessellation, textures), raw input, text shaping and galleys, layers/Areas/viewports, AccessKit
  plumbing. No `egui::Button`, `TextEdit`, `Checkbox`, `Slider`, `DragValue`, `ComboBox`,
  `ScrollArea`, `CollapsingHeader`, `Popup`, `menu`, `Modal` or `Window`, no `egui_extras`
  pickers; egui's `Visuals`/`Style` do not drive our look.
- Text fields use a shared editing core extracted from quire: **TC0 = `quire-text`** (quire repo,
  no egui or svelters dependency, wasm32-clean, quire-core depends on it, svelters pins it by git
  rev; macbook-cadatomic extracts it). cadatomic's requirement: quire's Document/EditorView split
  stays (a shared buffer, selections per view).

Inputs read: `crates/svelters-egui` (scene, provider, walk, leaf, style, theme, focus, views, kinds,
elements, menus, overlays, native, a11y), ARCH-SCENE §7, ARCH-EMBED §8, EGUI §5, DESIGN §1.4/§2,
CSS §2/§3, ZED-LESSONS §1, the milestone records EV2b, SG4, EV3, WG5a, FX1, WG3/WG3b, TR1, DS1,
ZU3, and the unmerged branches `ci/fx3` (FX3 record) and `ci/vis1` (VIS1 record and gate).

---

## 0. Summary

svelters-egui gets its own widget layer with three levels, all in Rust and all styled by the
Rust CSS engine:

1. **Paint primitives** (`paint/`): box, text, icon, image, path. They read only computed style
   (through a `StyleView` that DS2's transitions overlay) and emit epaint shapes. No `Visuals`.
2. **Behaviour units** (`interact/`): small Rust state machines (pressable, focusable, scrollable,
   anchored, text input, …) that read input from egui's `interact` response and raw input, own
   their state bits, produce typed intents, contribute AccessKit roles/states/actions, and claim
   keys for EV3. One implementation serves two adapters: a field of a Rust kind, and a `use:`
   action on any element.
3. **Widgets**: RA1 provider kinds (`#[svelters::widget]`, the same path cadatomic uses) composed
   from units, primitives and **parts** (`::part(name)`, either shadow nodes or paint-only), with
   their look entirely in the theme's CSS (`@layer sv.base`, `sv.affordance`).

Decisions (each justified in its section; the true architecture choices are the questions of §8):

| # | Decision | Where |
|---|---|---|
| W1 | **Primary path: Rust kinds composed from behaviour units.** Every built-in element and cadatomic's own widgets are `#[svelters::widget]` kinds. The same units are exported as `use:` actions, so Svelte composites (app and extension level) get identical behaviour over `box`/`text`/`icon` primitives. Built-ins are not written in Svelte. | §1.4, Q1 |
| W2 | egui's `Ui::interact` stays the pointer picking, capture and focus-memory substrate; every semantic (thresholds, repeat, cancel, click count, drag-to-value, dismissal) is ours. | §1.2, Q2 |
| W3 | Look = CSS only. Each widget documents its parts and states; the theme's base and affordance sheets style them. `::part(name)`, `::placeholder`, `::selection` enter the engine's dialect. | §1.3, Q3 |
| W4 | New built-in state bits: `focus-visible`, `indeterminate`, `placeholder-shown`, `default`, `drop-target`, `current`, `backdrop`, `required`, `user-invalid`. | §2.2 |
| W5 | Migration without a flag day: the built-in kind row (tag, prop ids, signals, spec) stays; an own kind declares `replaces = "<tag>"`, a per-kind switch picks the implementation, and the egui wrapper is deleted in the lane whose replacement passes. A ZU3-style `disallowed_types` lint ends at zero egui widgets. | §6 |
| W6 | Behaviour policy per platform: `native::Policy` gains `Windows` and `Mac` rows (W1 of EGUI §8: native per-platform behaviour); the `Egui` row is retired with the last wrapper. | §1.2 |
| W7 | Text fields run on TC0 (`quire-text`) behind the interface of §4; the text backend stays open until TC-EVAL reports. | §4 |
| W8 | Lane plan: OW0 (foundation + button) lands an API slice within hours; nine lanes fan out over three machines. | §7 |

---

## 1. Layers

### 1.1 Paint primitives (layer a)

Module `crates/svelters-egui/src/paint/` (new). Every primitive takes a `StyleView` (the node's or
part's computed style with running transitions applied, §3.4), a rect in points and the paint
context (painter, `pixels_per_point`, clip), and emits epaint shapes. Primitives never read
`egui::Style`/`Visuals`.

| Primitive | CSS it reads | Emits | Notes |
|---|---|---|---|
| **box** | `background-color`, `background-image` (none, `linear-gradient` 2-stop only, for DESIGN's lip), `border-{top,right,bottom,left}-{width,color,style}` (solid, dashed for HC), `border-*-radius` (per corner), `box-shadow` (outer, inset; CS5g's `style/shadow.rs` moves here), `outline-{width,color,style,offset}`, `opacity` | outer shadows → `RectShape` fill → inset shadows → border → (content) → outline | Uniform border: one `RectShape` with `StrokeKind::Inside` (what VIS1 measures). Non-uniform sides: a filled path per side (tessellated, corner joins mitred at the radius). Outline is outside the border box by `outline-offset` (−1 px for rows and fields, +1 px for standalone controls, DESIGN R8); it does not affect layout or hit-testing, but grows `paint_bounds` (culling). |
| **text** | `color`, `font-family`, `font-size`, `font-weight` (FU's `fonts::text_font`), `font-style`, `line-height`, `letter-spacing`, `text-transform`, `text-align`, `white-space`, `text-overflow`, the `elide` prop (`end`/`middle`/`start`, WG6 QW13), `text-decoration-line` (underline for links, R18) | one `TextShape` (galley) + optional selection rects + caret rect | Layout job built from the style; galley cache is egui's (keep-if-used-last-frame). Single-line text is vertically centred in the content box (VS Code labels and buttons). Middle elision: binary search over glyph advances on the galley. Selection highlight from `::selection` (`background-color`, `color`). |
| **icon** | `color` (tint), `width`/`height` or `font-size` (box px), `--icon-subject/accent/badge` slots (ICR1) | glyph `TextShape` or textured `RectShape` | `IconSource` from ICR1's `IconRegistry::resolve(key, box_px × ppp)`; until ICR1 lands, the codicon font path that `icon` uses today. Chevrons, check marks, twisties, close glyphs are icons (codicon `check`, `dash`, `chevron-down`, `chevron-right`, `close`), never hand-drawn paths: VIS1 gap 8 (select chevron not the codicon) closes by construction. |
| **image** | `object-fit` (the `fit` prop maps to it), `border-radius` (clips), `opacity` | textured `RectShape` | images.rs keeps loading; the primitive only paints. |
| **path** | `fill`/`stroke` from the canvas display list, or `color` for a part | `PathShape`, `Mesh` | `canvas` keeps its display list (`elements/canvas.rs`); parts with geometry no icon covers (a radio's dot = a circle, a progress bar's fill = a box, a spinner's arc) use path or box. |

Paint order of one node (CSS 2.1 Appendix E, simplified): outer shadows, background, inset
shadows, border, content (text/icon/image/path, then node parts and children in tree order),
behaviour `paint_over` hooks (SG4), outline (focus ring last, so children never cover it).

```rust
// paint/mod.rs (OW0)
pub struct PaintCx<'a> { painter: &'a egui::Painter, ppp: f32, clip: egui::Rect, trace: Option<&'a mut PaintTrace> }
pub fn paint_box(cx: &mut PaintCx, s: &StyleView, border_box: egui::Rect) -> BoxGeom;   // content/padding boxes
pub fn paint_outline(cx: &mut PaintCx, s: &StyleView, border_box: egui::Rect);
pub fn text_galley(fonts: &egui::text::Fonts, s: &StyleView, text: &str, wrap: Wrap) -> Arc<Galley>;
pub fn paint_text(cx: &mut PaintCx, s: &StyleView, galley: &Arc<Galley>, content: egui::Rect, sel: &[Range<usize>]);
pub fn paint_icon(cx: &mut PaintCx, s: &StyleView, key: &IconKey, rect: egui::Rect);
pub fn paint_image(cx: &mut PaintCx, s: &StyleView, tex: TextureId, size: Vec2, rect: egui::Rect);
pub fn measure_text(fonts: &egui::text::Fonts, s: &StyleView, text: &str, avail: AvailableSpace) -> Vec2;
```

`BoxGeom` gives the border, padding and content boxes (padding and border widths from computed
style; layout already used the same values through `layout_style`), so a widget's `measure` is
"content size + padding + border" with no egui `Spacing` constants. `layout.rs`'s copies of egui
widget sizes (`:395` separator, `:405` collapsing header, the per-kind `intrinsic`s) go away kind by
kind.

**PaintTrace** (debug and tests only): an optional per-pass side table `shape index → (node,
part)`, so the VIS1 gate and kittests measure "the thumb's background" instead of guessing which
`RectShape` inside a rect is which (§3.7).

### 1.2 Behaviour units (layer b)

Module `crates/svelters-egui/src/interact/` (new). A unit is a plain struct: no scene access, no
JS. It reads an `InputCx`, updates its own state, sets the state bits it owns, and pushes typed
intents that the widget (or the `use:` adapter) turns into prop writes and events.

```rust
pub trait Unit: Default + 'static {
    type Params: Default;                       // what the kind or the `use:` params configure
    type Intent;                                // what happened this pass, typed
    const OWNS: StateBits;                      // cleared on detach, on disable (except focus rules), on inert
    fn sense(&self, p: &Self::Params) -> egui::Sense { egui::Sense::hover() }
    fn input(&mut self, p: &Self::Params, cx: &mut InputCx<'_>, out: &mut Intents<Self::Intent>);
    fn a11y(&self, p: &Self::Params, node: &mut accesskit::Node) {}
    fn claims(&self, p: &Self::Params, key: egui::Key, m: egui::Modifiers) -> bool { false }   // EV3 R11
    fn next_frame(&self) -> Option<std::time::Duration> { None }                                 // repeat, momentum, blink
}

pub struct InputCx<'a> {
    pub resp: &'a egui::Response,               // ui.interact(rect, id, merged sense) of the host or part
    pub input: &'a egui::InputState,            // raw events; keys only when focused (EV3 already filters)
    pub policy: &'a native::Policy,             // platform row (W6)
    pub now: f64,                               // egui input time (kittest's virtual clock)
    pub state: &'a mut StateBits,               // the host's bits, written in place before paint
    pub a11y: &'a [accesskit::ActionRequest],   // routed to this id
    pub focus: FocusCx<'a>,                     // EV3: request, has, reason (pointer/keyboard/program), scope
    pub parts: &'a PartResponses,               // responses of node parts (thumb, buttons)
}
```

Two adapters, one implementation:

- **In a kind**: `#[behavior] press: Pressable` fields; `#[svelters::widget]` generates
  `self.run_units(cx)`, which runs each unit in declaration order and returns their intents (§1.3).
- **On any element**: `svelters:behaviors/sv` exports one `use:` action per unit
  (`use:pressable={{ onpress, repeat: true }}`). The existing EV2b attached-kind machinery hosts
  it: a generic `UseUnit<U: Unit>` implements `Behavior` (`sense`, `before_ui` runs `input`,
  intents become the action's `on<intent>` callbacks). A `use:` unit on a host that already
  composes the same unit is a diagnostic (double handling).

EV1 seam: an intent that is a user-visible event (a press, a toggle, a selection change) is
dispatched as a `SceneEvent` through EV1's propagation before the widget applies its default
action, so a Rust `Behavior::filter` (SV6) in capture or bubble can veto it
(`Filter::Stop`/`PreventDefault`); JS listeners observe it after the pass, as today.

The catalogue (the minimum set every lane builds on; the "from" column is the spec that carries over):

| Unit | Owns bits | Intents | Params (props) | Keys claimed | AccessKit | From |
|---|---|---|---|---|---|---|
| `Hoverable` | hovered | `Enter`, `Leave` | — | — | — | CS5e same-frame hover |
| `Pressable` | pressed | `Down`, `Up{inside}`, `Cancel` (Escape, capture lost, pointer left with `cancel_on_leave`), `Click{count, button, mods}`, `Repeat` | `repeat: Option<(delay, interval)>` (spin buttons, scroll arrows), `buttons`, `activate_on: Release|Press` | Space (release activates, GTK/Qt), Enter (press activates) when focused | `Action::Click`; `Role` from widget | EV3 "clickable leaves claim Space and Enter"; WG3s pin 10 (pointer thresholds) |
| `Focusable` | focused, focus-visible, focus-within (ancestors, walk) | `Gained{reason}`, `Lost` | `tab_stop`, `focus_on_click` (Policy `focus_on_click`), `autofocus` | — | `Action::Focus`, focusable | EV3 scopes, FX1 policy; `focus-visible` = gained by keyboard or program while the last input was a key (CSS rule), never by pointer (FX1) |
| `Toggleable` | checked, indeterminate | `Toggled(bool)` | `tristate`, `radio_group` (exclusivity is the widget's, as `radio-button` today) | — (Pressable's) | `Toggled` state, `Action::Click` | WG2b radio/switch, DESIGN R17 |
| `Draggable` | dragging | `Start{origin}`, `Move{delta, total}`, `End`, `Cancel` | `threshold` (Policy: GTK `gtk-dnd-drag-threshold` 8 px, Qt `startDragDistance` 10 px, Windows `SM_CXDRAG` 4 px; WG3s pin 10 measures), `axis` | Escape cancels while dragging | — | panel/split sashes, scrollbar thumbs, DnD source |
| `DragToValue` (scrub) | dragging | `Scrub{value}`, `Commit{value}`, `Cancel{restore}` | `speed`, `min`, `max`, `step`, `log`, modifiers (Shift fine ×0.1, Ctrl coarse ×10), `commit_on: Release|Live` (FM1 QW7) | — | numeric value, min, max, step | `drag-value`, slider thumbs, cadatomic quantity fields |
| `Steppable` | — | `Step{n}` (±1, ±page, home/end), `Set{value}` | `step`, `page` (PgUp ×10, FM1 QW7), `wrap`, `wheel: Hover|Focused|Off` (Policy default), buttons (node parts `inc`/`dec`, with Pressable repeat) | Up/Down (+Left/Right for sliders), PgUp/PgDn, Home/End | `Action::Increment/Decrement/SetValue` | EV3 slider claims; FM1 QW7 |
| `Scrollable` | (internal: scrolled-top/bottom/left/right exposed as paint-part states) | `Scrolled{offset}` | `axes`, `overscroll` (none), `bar: Overlay|Classic|None` (VS Code overlay 10 px, fade 800 ms), `kinetic: Os|Own|None` (Policy), `scroll_chaining` | PgUp/PgDn/Home/End/arrows when the scrollable itself is focused | `scroll_x/y`, min/max, `Action::ScrollUp/Down/Left/Right`, `ScrollIntoView` | `scroll-area`, virtual-list's own bar, WG6 QA13, WG3s pin 9, DESIGN R15 |
| `Selectable` (+ models) | selected, current | `Select{ids, anchor, cursor}`, `Activate{id}` | `mode: None|Single|Multi|Range`, keyed by id (`views/selection.rs` `KeySelect`) | arrows, PgUp/PgDn, Home/End, Shift/Ctrl variants, Ctrl+A, Space, Ctrl+Space, Enter | `selected`, `multiselectable`, `active_descendant` | WG5a/WG5b (one implementation, moved, not rewritten) |
| `Roving` | — | `Move{to}` | `orientation`, `wrap`, `home_end` | arrows within a group (toolbars, radio groups, tab strips, menu bars, segmented buttons): one tab stop per group | `active_descendant` or moved focus | ARIA APG roving tabindex; QT-SURVEY radio groups |
| `Expandable` | expanded | `Expanded(bool)` | — | Right/Left (tree), Enter/Space (header) | `expanded`, `Action::Expand/Collapse` | collapsing-header, tree rows |
| `Dismissable` | open | `Dismiss{reason}` (`Escape`, `OutsidePress`, `FocusOut`, `AnchorHidden`, `Programmatic`) | `on_outside: Close|Ignore`, `modal` | Escape (before EV3's cancel button) | `modal` | popup `autohide`, modal `dismissable`, EV3 popup focus return |
| `Anchored` | — | `Placed{side, rect}` | `placement` (`bottom-start`, `right-start`, …), `offset`, `flip`, `shift`, `match_anchor_width` (select), `arrow` | — | — | popup `placement`, menus, select, tooltip; Floating UI's flip/shift algorithm |
| `Typeahead` | — | `Match{id}` | `timeout` (1 s, WG5a), `prefix` source (labels) | printable keys when not editing | — | WG5a type-ahead, WG6 QW9 combo type-ahead |
| `TextInput` (on TC0) | focused-text bits: placeholder-shown, read-only, user-invalid | `Edited{version}`, `Committed`, `Submitted`, `SelectionChanged` | §4 `FieldRules` | everything EV3 lists for text fields; all keys while composing | text runs, caret, selection (§5) | FX1, FX3, EV3 R11 |
| `Tooltipable` | — | `Show{after}`, `Hide` | `tooltip` (text or snippet), `delay` (Policy), shows on disabled hosts (WG6 QA5, R9 disabled reason) | — | `described_by` | common `tooltip` prop, WG6 QA5 |
| `DragSource` / `DropTarget` | dragging / drop-target | `DragBegin`, `DragEnd`, `DragEnter`, `DragLeave`, `Drop{data, action}` | MIME list, drop actions (copy/move/link, WG6 QA9), auto-scroll edges | Escape cancels | — | `kinds/dnd.rs` (moved), WG4b tests |
| `Resizable` | dragging | `Resize{size}`, `Commit` | `min`, `max`, `axis`, keyboard step | arrows when the sash has focus | `Role::Splitter`, value | panel edges, split-view, inner-window edges, table columns; DESIGN R11 |

**Platform rows (W6).** `native::Policy` today has `Gtk`, `Qt`, `Egui`. Own widgets cannot fall
back to "egui's behaviour", so OW0 adds `Windows` and `Mac` rows. Until WG3w (Windows, running)
and a macOS reference lane pin them, their values come from the platform guidelines (Mac: Option
word motions, no focus on click for buttons, Return activates the default button; Windows: Ctrl
word motions with Qt-like stops, focus on click). `Native::Egui` stays only while egui wrappers
exist. New Policy fields for the units: `drag_threshold`, `double_click_ms`, `tooltip_delay_ms`,
`caret_blink_ms`, `kinetic`, `scrollbar` style, `space_activates_on: Release|Press`,
`button_order` (FM1 QW11).

### 1.3 Widgets as compositions (layer c)

A widget = **primitives** (how its node and its parts paint) + **units** (how it behaves) + **CSS**
(how it looks, in the theme) + **parts** (internal pieces with their own style).

**Parts.** Two kinds, one selector syntax:

- **Node parts** are shadow children (SG4 `insert_shadow`, `NodeFlags::SHADOW`): real scene nodes
  with their own rect (taffy or `Caps::CUSTOM_LAYOUT`), state bits, hit-list entries and egui ids.
  Use them for anything interactive or laid out: a slider's `track` and `thumb`, a scrollbar's
  `thumb`, spin `inc`/`dec` buttons, a select's `popup`, a tab's `close` button, a window's
  `titlebar`.
- **Paint parts** have no node: the widget paints them with the part's computed style, which the
  engine computes as for a pseudo-element of the host. Use them for passive pieces: a checkbox's
  `mark`, a select's `chevron`, a progress bar's `fill`, a scrollable's `shadow-top`, a text
  field's `placeholder` and `selection`.

CSS reaches both with `host::part(name)`, and the standard `::placeholder` and `::selection` are
aliases of `::part(placeholder)` and `::part(selection)`. Host state pseudo-classes come before the
pseudo-element (`checkbox:checked::part(mark)`, `slider:focus-visible::part(thumb)`); a node part's
own states follow it (`slider::part(thumb):hover`). Part names are public API per widget (§6
lists them) and documented in the widget's docs page (DOCS-PLAN).

**Definition form** (the macro is RA1's; `#[behavior]`, `#[part]` and `replaces` are OW0's additions):

```rust
#[svelters::widget(namespace = "sv", tag = "button", replaces = "button", caps = FOCUSABLE)]
pub struct Button {
    #[prop] text: String,
    #[prop] icon: Option<IconKey>,
    #[prop] default: bool,
    #[prop] cancel: bool,
    #[event] clicked: Event<()>,
    #[behavior] hover: Hoverable,
    #[behavior] focus: Focusable,
    #[behavior(params = press_params)] press: Pressable,
}

impl Widget for Button {
    fn measure(&mut self, cx: &mut MeasureCx, avail: AvailableSpace) -> Vec2 {
        cx.content(|c| c.icon(self.icon.as_ref()).gap().text(&self.text, avail))   // + padding + border
    }
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx) -> egui::Response {
        let resp = cx.interact();                       // ui.interact(layout rect, node id, units' merged sense)
        for i in self.run_units(cx, &resp) {            // generated; bits set, same-frame restyle after
            if let Intent::Press(Press::Click { .. }) = i { cx.dispatch(&self.clicked, ()); }   // EV1, then JS
        }
        let b = cx.paint_box();                         // node style: shadows, background, border
        cx.paint_content(b.content, |c| c.icon(self.icon.as_ref()).text(&self.text));
        cx.paint_outline();                             // focus ring from :focus-visible rules
        resp
    }
    fn a11y(&self, cx: &A11yCx, n: &mut accesskit::Node) { n.set_role(Role::Button); self.units_a11y(n); }
}
```

Default and cancel buttons stay EV3's (`focus::take_activation`), called from the generated unit
runner when `default`/`cancel` are set. Slider shows the node-part form:

```rust
#[svelters::widget(namespace = "sv", tag = "slider", replaces = "slider", caps = FOCUSABLE | EDITABLE | CUSTOM_LAYOUT)]
pub struct Slider {
    #[prop(bind)] value: f64, #[prop] min: f64, #[prop] max: f64, #[prop] step: f64, …
    #[event] changed: Event<f64>, #[event] commit: Event<f64>,
    #[part(node)] track: Part, #[part(node)] thumb: Part,     // shadow children created on first prepare
    #[behavior] focus: Focusable, #[behavior] hover: Hoverable,
    #[behavior] scrub: DragToValue,                           // on the thumb part and the track (jump then drag)
    #[behavior] step: Steppable,
}
```

### 1.4 Where widgets are defined: Rust kind, Svelte component, or both

| Criterion | (A) Rust kind composed from units | (B) Svelte component over primitives + `use:` units | (C) both |
|---|---|---|---|
| Per-frame cost | none in JS; one `ui` call per visible node | none per frame (units are Rust), but each instance is a component: mount cost, `$state`, effects | per path |
| Mount cost (10k-row tables, property grids, cadatomic's 2k-row grid of ARCH-SCENE §10.2) | one scene node + parts | a component instance per widget per realised row (JS allocation, effects, template clone); virtual lists bind rows in Rust today (SG5) and could not use Svelte-defined cells there | — |
| Hot reload | look reloads (CSS through the loader and engine); behaviour needs a rebuild (cadatomic D10: dev-only Rust hot patch) | full, `$state` kept (HMR, Q1) | — |
| Self-editability (the GUI edits its own `.svelte` live) | the widget's CSS and every composite are editable; its Rust is not | everything editable | — |
| Extension authors (no native code, later deny-by-default) | cannot add Rust kinds without shipping native code | can, with CSS and `use:` units | — |
| cadatomic's own widgets | same macro, same units, same parts and CSS: first-class | possible for composites | — |
| Testing | kittest per kind, AccessKit queries, VIS1 per check | needs the JS runtime per test (slower), same queries | — |
| Behaviour depth (IME, TC0, momentum, popups across layers, a11y text) | natural | each needs a Rust unit anyway; Svelte only arranges | — |

**Recommendation (W1, Q1): (A) is the primary path for every built-in element and for cadatomic's
performance-sensitive widgets; (B) is the supported path for composites** (a search box = text
field + icon + clear button, a labelled form row, a toolbar, a property row, extension UIs), and it
needs nothing beyond (A): the same units exported as `use:` actions, plus the primitives `box`,
`label`/`text`, `icon`, `image`, `canvas` as elements. Rejected: writing built-ins in Svelte. It
would make every checkbox in a virtualised table a component instance, put JS on the mount path of
every row, split behaviour between two languages, and buy hot reload only for the part (behaviour)
that changes least; the look, which changes most, already hot-reloads as CSS.

Consequences: the built-in vocabulary is a set of Rust kinds in `crates/svelters-egui/src/widgets/`
(one file per widget, ≤ 300 lines each, most far less); the provider macro gains the features they
need (`#[behavior]`, `#[part]`, `replaces`), which cadatomic then has too; `examples/egui-kinds`
gains a "write your own widget" example that the docs page uses (DOCS-PLAN).

---

## 2. The state model

### 2.1 Sources of truth

- **Props are the truth** for everything the app owns: `value`, `checked`, `open`, `expanded`,
  `selected`, `disabled`, `text`. A widget never keeps a second copy except a **draft** while the
  user is mid-interaction (§2.4).
- **State bits are derived**, never set by JS. Two writers: units (interaction bits: hovered,
  pressed, focused, focus-visible, dragging, current) and the walk's prop mirror (checked from
  `checked`, disabled from `enabled`/`disabled`, selected, open, expanded, invalid, read-only,
  indeterminate, placeholder-shown, required, default). Provider bits (32–63) stay the provider's.
- **Widget memory** (egui `Memory`/`IdTypeMap` is not used for widget state): units' state lives in
  the widget struct (the node's payload) or, for state that must survive payload replacement
  (a TC0 session, a scroll offset across hot reload), in an ext column of the node.

### 2.2 State bits and pseudo-classes

| Bit | Pseudo-class | Writer | Note |
|---|---|---|---|
| 0 hovered | `:hover` | Hoverable (and the walk for containers) | same-frame restyle (CS5e) |
| 1 pressed | `:active` | Pressable, DragToValue | |
| 2 focused | `:focus` | Focusable | |
| 3 focus-within | `:focus-within` | walk (ancestors) | |
| 4 disabled | `:disabled` | prop mirror | `:enabled` is its negation |
| 5 checked | `:checked` | prop mirror | |
| 6 selected | `:selected` | Selectable / prop mirror | |
| 7 open | `:open` | prop mirror (popup, select, menu-button) | |
| 8 expanded | `:expanded` | prop mirror | |
| 9 invalid | `:invalid` | prop mirror (`invalid`, `pattern`) | |
| 10 read-only | `:read-only` | prop mirror | `:read-write` is its negation for fields |
| 11 dragging | `:dragging` | Draggable, DragToValue, DragSource | |
| 12 exiting | `:exiting` | TR2b | |
| 13 busy | `:busy` | R8 | |
| **14 focus-visible** | `:focus-visible` | Focusable | new; the engine's `Element::is_focus_visible` reads it instead of "focused" |
| **15 indeterminate** | `:indeterminate` | prop mirror | checkbox tristate, progress without a value |
| **16 placeholder-shown** | `:placeholder-shown` | TextInput | |
| **17 default** | `:default` | prop mirror (`default` button) | |
| **18 drop-target** | `:drop-target` | DropTarget | GTK's `:drop(active)`; DESIGN R12 |
| **19 current** | `:current` | Selectable | the keyboard cursor row ("focused row" of DESIGN §1.4, distinct from selected) |
| **20 backdrop** | `:backdrop` | walk (viewport not focused) | GTK's name; inactive-window chrome |
| **21 required** | `:required` | prop mirror | FM1 QW8 |
| **22 user-invalid** | `:user-invalid` | TextInput/commit | invalid shown only after the first commit (avoids red empty forms) |

The two bit tables (`provider/state.rs`, `svelters-style/src/state.rs`) gain the rows together and
the existing pin test keeps them equal. Bits 23–31 stay free.

### 2.3 Same-frame update

Order within one node's `ui` (the walk already restyles in place before paint, CS5e):
`interact` → units run (intents, bits) → EV1 dispatch of user events → default actions (prop
writes through `cx.set`, which journals and announces `prop_changed`) → prop-mirror bits refreshed
for the props just written → **restyle this node and its parts if any bit changed** → paint. So a
click that checks a checkbox paints checked in the same frame, and `:hover`/`:active` never lag.
Changes that alter layout (a bold `:checked` label) apply at next frame's layout, as today.

### 2.4 `bind:`, drafts, `commit` and `submit` (CX3/FM1 seam)

Every value widget emits three signals, with the same names on every kind:

| Signal | When | Who listens |
|---|---|---|
| `input` (today's `changed` on value widgets; `changed` stays as an alias until the docs pass) | every user change of the bound prop (keystroke, scrub step) | `bind:value` (default lowering) |
| `commit` | the user finished: Enter, blur after an edit, pointer release after a scrub, a step-button click, a picker's OK, a select choice | `bind:value|commit` (CX3 lowers to observing `commit`), `oncommit` |
| `submit` | Enter in a single-line field (or Ctrl+Enter in a multi-line one) | the nearest `form` scope's `onsubmit` (FM1 QW8), else the scope's default button (EV3) |

- **Live widgets** (default) write the prop on each change; `bind:value|commit` reads it only on
  `commit`. A JS write during interaction (store echo) that equals the current value is a no-op.
- **Commit-mode widgets** (`commit-mode="release"` on `drag-value`/`slider`, FM1 QW7; every text
  field under `|commit`) keep a **draft** in the widget while the user interacts and paint the
  draft; the prop is written once at `commit`. Escape cancels: the draft is dropped and `commit`
  is not sent. A prop write from JS during a draft wins only if it differs from the value the
  draft started from (an external update), and then ends the draft.
- **Validation** (`pattern`, `min`/`max`, a provider validator): `invalid` mirrors the result on
  each input; `user-invalid` is set from the first `commit` on; WG5a's sync `oncommit` reject
  (`e.result = 'reason'`) keeps the draft open and shows the reason (tooltip part `error`).
- CX3's native `link(store, field, node, prop)` (no JS per keystroke) observes the same signals,
  so a cadatomic field bound to a Rust store under `|commit` costs one store write per commit.

### 2.5 Disabled, read-only, inert

Disabled: units stop producing intents except `Hoverable` and `Tooltipable` (R9: disabled
controls stay hoverable and show the reason); `Focusable` keeps menu/palette focus per R9 only
where the widget opts in. Read-only: TextInput allows motion, selection and copy; Steppable and
DragToValue stop. Inert (TR2b, outros): no units run, no hit-list entries, AccessKit hidden.

---

## 3. Rendering

### 3.1 The per-frame paint path

Unchanged frame (ARCH-SCENE §7.2); step d (the walk) for an own kind:

1. cull by `paint_bounds` (outer shadows and outline included);
2. `Behavior::before_ui` of attached behaviours (`use:` units run here);
3. `Widget::ui` with `with_taken` (SG4): `cx.interact()` → units → dispatch → prop writes →
   same-frame restyle → primitives paint the node, then node parts and children
   (`cx.children(ui)`), then paint parts that go above children;
4. `Behavior::after_ui` / `paint_over`; outline last;
5. record: hit list (SG3), EV3 focus record (`focus::record_with`, role and tab stop from the
   units), AccessKit node (§5), PaintTrace in tests.

No egui widget is called anywhere in this path. The enforcement is a lint, as ZU3 did for blocking
calls: `crates/svelters-egui/clippy.toml` gets `disallowed_types`/`disallowed_methods` for
`egui::{Button, TextEdit, Checkbox, RadioButton, Slider, DragValue, ComboBox, ScrollArea,
CollapsingHeader, Popup, Modal, Window, ProgressBar, Spinner, Link, Hyperlink, Label, Separator,
MenuBar, containers::menu::*}` and `egui_extras::DatePickerButton`; every legacy wrapper carries
`#[allow(clippy::disallowed_types)] // OW: wrapper until <lane>`; the count of those lines is the
migration's burn-down and must reach zero (OW-DONE gate, §7). Host code (cadatomic's Rust chrome)
is not linted.

### 3.2 Hit-testing and layering

- **Picking stays egui's** (W2): each own node (and each node part) calls `ui.interact(rect, id,
  sense)` at its layout rect in its layer; egui picks against last pass's rects, keeps drag
  capture, click/double-click timing and focus memory. Units interpret the response.
- **The scene's hit list** (SG3) records every drawn node and node part with rect, clip and layer,
  and serves `hit_test` (picking, drop targets, MCP, tutorial gates) and `Dismissable`'s outside
  test (a press whose topmost target is outside the popup's subtree and its anchor).
- **Layers:** in-flow content in its slot's layer (`Order::Middle` for windows, the dock's layer for
  slots); popups, menus, select lists and popovers in an `egui::Area` per open popup at
  `Order::Foreground`; tooltips, the drag ghost and the hex-entry preedit at `Order::Tooltip`;
  modal = an Area at `Foreground` whose first node is a full-viewport backdrop part sensing clicks
  (blocks what is below, dims per `::part(backdrop)`). Nested popups (submenus) open later and are
  moved to the top of their order (`Context::move_to_top`), which is also the hit-list order.
- Areas are the only egui container used: `interactable(false)` unless a node in them senses
  (SG4's overlay-root rule).

### 3.3 Clip and scroll

A `Scrollable` container lays its children out unclipped in content coordinates (taffy, as today;
the content size is its children's extent). At paint, the walk offsets the children's origin by
`−offset`, sets the clip to the container's padding box (intersected with the parent clip), culls
children against it, and records the clip in the hit list so hidden parts of children do not hit.
Scrollbars are node parts painted after the children (VS Code overlay bars: 10 px, thumb
`scrollbarSlider.*` colours, fade out 800 ms after the last scroll, visible on hover of the
container); edge shadows are paint parts (`::part(shadow-top)` etc., DESIGN R15) shown when
scrolled away from that edge.

- Wheel and trackpad: the innermost scrollable under the pointer that can move in the delta's
  direction takes it (scroll chaining at the edge, as browsers); WG3s pin 9 is the reference.
- Kinetic: `Policy::kinetic` = `Os` (macOS: momentum phases come from the OS), `Own` (Linux
  touchpads without OS momentum: decay 0.95/16 ms, stop below 0.5 pt/frame), `None` (mouse wheels).
  Smooth wheel steps use egui's `smooth_scroll_delta`.
- `scrollIntoView(node, block, inline)` (WG6 QA13), focus moves scroll the focused node into view,
  and TC0's `Reveal` scrolls the caret: all through one `scroll_into_view(rect)` on the nearest
  scrollable ancestors, innermost first.
- `virtual-list`, `table` and tree adopt the same unit (their rows already virtualise; only the bar
  and wheel code is replaced).

### 3.4 Transitions (DS2)

Primitives read styles only through `StyleView`. DS2 adds a `Transitions` store on the scene keyed
by `(node or part, property)`: when a restyle changes a property that has a `transition` with
non-zero duration (colours, `opacity`, `box-shadow`, `outline-color`, `transform` translate/scale),
the store starts a transition from the **currently painted** value to the new computed value; the
`StyleView` interpolates at paint time from egui's input time; running transitions request frames
for their root only (TR2's `running_roots`); `prefers-reduced-motion` and TR F3 make them instant.
DESIGN R2 ("pressed takes effect at once") is a CSS rule (`transition: none` on `:active`), not code.
Svelte transitions (TR1/TR2 `Visual`: opacity, transform, clip) compose on top of the node's whole
paint as today.

### 3.5 Pixel snapping

- Rects from layout are in points; at paint, box edges snap to the physical pixel grid
  (`round_to_pixels(ppp)`), so a 1 px border at 150 % is 1 or 2 physical pixels, never a blurred 1.5.
- Border, outline and caret widths round to whole physical pixels with a minimum of 1 (CSS rounds
  border widths down to device pixels; so do we).
- Text galleys are placed on the pixel grid (egui does it for `TextShape` positions); icon textures
  are rasterised at the physical size (ICR1) and placed on the grid.
- Radii, shadows and blur are not snapped. Hit rects use unsnapped layout rects (no 1-px gaps
  between adjacent rows).

### 3.6 Zoom (ZM1)

ZM1's host API sets `pixels_per_point` (scale 1.2^level). Everything above is in points, so own
widgets need nothing per level beyond snapping (§3.5) and caches keyed by `ppp`: galleys (egui's
cache already is), icon rasters (ICR1 picks the optical master per physical size), box shapes
(recomputed each frame). ZM1's acceptance (80–150 % against VIS1 sheets) applies to own widgets
unchanged.

### 3.7 How VIS1 measures us

The VIS1 gate (`crates/svelters-testkit/tests/vis1.rs::default_look_matches_vscode`, on
`ci/vis1`) mounts `fixtures/gallery-egui/src/pages/VisControls.svelte`, drives hover, press and
AccessKit focus, and compares **painted shapes**, not pixels: `h`, `r`, `b` within 1 px, `fs`
within 0.01, `bg`/`bc`/`fg`/`oc` by CIEDE2000 ΔE ≤ 2.0, `ta` exact; ratchet in
`conformance/visual/baseline.json` plus `MIN_PASS_RATE = 0.9`. It reached 292/308 against stock
widgets restyled through `Visuals`, which the user's decision supersedes.

For own widgets:
- The gate stays and becomes every widget lane's visual acceptance (§7). It finds shapes through
  PaintTrace (`node`, `part`) rather than "the RectShape inside the rect", so checks can target
  parts (`slider::part(thumb)` bg, `checkbox::part(mark)` fg) and the focus ring (`oc`, VIS1's
  open gap 1, which own painting closes: the outline is ours).
- The VIS1 references, capture tooling and theme roles (`button2-border`, `check-bg`, `check-fg`,
  `toggle-hover`, `toggle-on-fg`, `tab-hover-bg`) carry over unchanged.
- A kind's checks move from "stock" to "own" in the lane that swaps it; the baseline may only grow.
  Kinds VS Code lacks (slider, switch, date picker, spin buttons) get checks against the DESIGN
  rules (R1–R18: ≥ 3:1 boundaries, lip, sunken fields) rather than a VS Code crop (Q4).

### 3.8 Paint caching (later, measured first)

Each own node's shapes are a pure function of (style generation incl. running transitions, rect,
state bits, content generation, `ppp`). ARCH-SCENE §8.3's `cached_shapes` can replay them when
nothing changed; OW-PERF does it only if ZU3's frame histogram on cadatomic's heaviest panel shows
paint above budget (p99 ≤ 8.3 ms, ZED-LESSONS 1.7).

---

## 4. Text input on TC0 (`quire-text`)

### 4.1 Division of work

| Concern | TC0 (`quire-text`) | svelters (`text-edit` kind, `interact/text_input.rs`) | quire-core (stays) |
|---|---|---|---|
| Buffer, versions, change sets, anchors | **yes** | — | uses TC0 |
| Selections (multi-range, primary), per view | **yes** | single range in single-line fields (TC0 allows many) | multi-cursor |
| Transactions, origin, grouping hints | **yes** | builds them from input | builds them from `EditorCommand` |
| Undo history and grouping rules (pluggable) | **yes** (Monaco, GTK, Qt rule sets) | picks the rule set from `native::Policy` | Monaco set |
| Word rules (pluggable) | **yes** (VS Code classifier, Pango, Qt: FX1's `native/words.rs` moves here) | picks from `Policy` | VS Code |
| Grapheme motion, line motion within logical lines | **yes** | — | uses TC0 |
| Visual-line motion (Up/Down across wraps, PageUp/Down, Home/End of a wrapped row) | asks a `VisualLines` trait the caller implements | implements it on the galley | implements it on its wrap map |
| IME composition model (preedit text, active range, commit, cancel) | **yes** (on the view) | maps `egui::Event::Ime`, paints preedit | same |
| GTK hex entry (Ctrl+Shift+U) | as a preedit source (`Preedit::Hex`) | key binding per `Policy` | — |
| Clipboard, PRIMARY | emits **host requests** (`SetClipboard`, `ReadClipboard`, `SetPrimary`) | runs them through EV3's host clipboard (DX2) and `primary.rs` | same |
| Single-line rules (line breaks in paste, char limit, input filter, secret mode) | **yes** (`FieldRules`) | sets them from props | — |
| Tokens, diagnostics, folds, find, syntax | — | — | **stays in quire-core** |
| Key map (which chord is which command) | — (commands only) | per `Policy` (FX3's table) | its keymap |
| Painting: galley, selection highlight, caret and blink, placeholder, password bullets, preedit underline, scroll into view | — | **yes** | egui_quire |
| Hit-testing a point to an offset | — | galley `cursor_from_pos` | its layout |
| AccessKit text runs, caret and selection | offset conversions (byte ↔ char ↔ UTF-16) | builds the nodes (§5) | its own |
| Validation states, `commit`/`submit` signals, `bind:value` | — | **yes** (§2.4) | — |

TC0 is pure: no clock (callers pass `time_ms`), no threads, no I/O, no egui. Everything platform-
or toolkit-specific is a value the caller passes in (rule sets, `FieldRules`), so the GTK and Qt
reference behaviour measured by WG3/FX1/FX3 lives in TC0 as data and pure functions, and quire
gets it too.

### 4.2 The model: Document and View (cadatomic's requirement)

TC0 keeps quire's split. Names below are placeholders; the extractor (macbook-cadatomic) picks
final names, and quire-core keeps its `Document`/`EditorView` as wrappers around them.

```rust
/// Shared per buffer: text, version, history, recent changes (for rebasing and view sync).
pub struct TextDoc { /* text: Text, version: Version, history: History, recent: RecentChanges,
                        rules: DocRules */ }

/// One view of a TextDoc: selection, preedit, focus, selection_version, synced_at.
pub struct TextView { /* … */ }

impl TextDoc {
    pub fn new(text: &str, rules: DocRules) -> Self;
    pub fn text(&self) -> &Text;                    // backend-neutral read API (§4.3)
    pub fn version(&self) -> Version;
    pub fn apply(&mut self, tr: Transaction, view: &mut TextView) -> Result<Applied, Refusal>;
    pub fn apply_remote(&mut self, changes: ChangeSet, base: Version) -> Result<Applied, Refusal>;
    /// Replace everything from outside (a prop write). `ResetHistory` (GTK `set_text`,
    /// Qt `setText`: FX3) or `Undoable` (quire's reload, an app-level revert).
    pub fn replace_all(&mut self, text: &str, mode: ExternalSet) -> Applied;
    pub fn close_undo_group(&mut self) -> Option<UndoGroupClosed>;
    pub fn tick(&mut self, now_ms: u64) -> Option<UndoGroupClosed>;
    pub fn undo_mode(&self) -> UndoMode;            // Local | Host (quire CE3; app-level undo)
    pub fn can_undo(&self) -> bool;  pub fn can_redo(&self) -> bool;
    pub fn anchor(&self, pos: usize, bias: Bias) -> Anchor;
}

impl TextView {
    pub fn new(doc: &TextDoc) -> Self;
    pub fn selection(&self) -> &Selection;          // byte offsets, valid at synced_at()
    pub fn set_selection(&mut self, doc: &TextDoc, sel: Selection);
    pub fn selection_version(&self) -> u64;
    pub fn sync(&mut self, doc: &TextDoc) -> SyncOutcome;
    pub fn preedit(&self) -> Option<&Preedit>;
    pub fn composing(&self) -> bool;
    pub fn set_focused(&mut self, f: bool) -> bool; // closes the undo group on blur (Monaco, GTK)
}
```

A single-line field is one `TextDoc` + one `TextView`. A multi-view editor (quire) is one
`TextDoc` + N views. svelters never needs two views of one field, but nothing in the field code
assumes one view per doc: a "same value in two places" case (a property shown in an inspector and
a table cell) binds both fields to the same JS value, not to the same `TextDoc`.

### 4.3 The text backend stays behind an API (open, pending TC-EVAL)

`Text` exposes only: `len()`, `len_chars()`, `line_count()`, `line(i) -> TextSlice`,
`slice(range) -> TextSlice` (iterates `&str` chunks), `to_string()`, `char_at`,
`byte_to_char`/`char_to_byte`/`byte_to_utf16`, `line_of(byte)`, `line_start(i)`,
`grapheme_next(byte)`/`grapheme_prev(byte)`, and `as_str() -> Option<&str>` (Some when the
backend is contiguous: lets a single-line field hand egui one `&str` with no copy).

**Not decided here**: rope vs `String` vs promote-on-focus (a `String` that becomes a rope above a
size or on first multi-line edit). TC-EVAL (`research/tc-eval`) measures per-field memory,
creation cost for 10k fields and keystroke latency; the backend is chosen from its numbers.
Interface rules that keep every option open:

- no `ropey` (or other backend) type in TC0's public API;
- positions are UTF-8 byte offsets valid at a `Version` (as quire-core today); conversions are
  methods on `Text`;
- `TextDoc::new` must be cheap for a short string (no history allocation until the first edit,
  no recent-changes ring until the first transaction): svelters creates fields lazily (§4.6) but a
  10k-row editable table still creates one per edited cell.

### 4.4 Pluggable rules

```rust
pub struct DocRules {
    pub undo: UndoRules,            // Monaco | Gtk | Qt  (FX3's undo.rs models move here)
    pub undo_pause_ms: Option<u32>, // Monaco's pause rule; None for Gtk/Qt
    pub undo_mode: UndoMode,        // Local | Host
}

#[non_exhaustive]
pub enum UndoRules {
    /// quire-core's history.rs: word-boundary stops, cursor-jump stops, pause, non-typing = own group.
    Monaco,
    /// FX3 (fitted to GTK 4.24.1's golden, pin 4): one step per word, each deletion its own step,
    /// typing over a selection one closed step, a caret move ends merging; undoing a deleted
    /// selection restores it selected, caret at its start.
    Gtk,
    /// FX3 (QWidgetLineControl's command history, ported): a typing run one step, a BackSpace or
    /// Delete run one step, a selection removal plus the typing after it one step; separators at
    /// caret moves and pastes; Qt's internalUndo/internalRedo walk.
    Qt,
}

pub struct FieldRules {           // per view; svelters sets it from props and Policy
    pub lines: LineMode,          // Single { paste_breaks: Keep | Spaces | Strip } | Multi
    pub max_chars: Option<u32>,   // `char-limit` / `maxlength`
    pub secret: bool,             // password: word motions jump to the ends, no copy/cut requests
    pub read_only: bool,          // motions and copy only
    pub words: WordRules,         // VsCode(classifier) | Pango | Qt  (FX1's words.rs moves here)
    pub word_move_collapses: bool,// GTK: Ctrl+Arrow on a selection collapses to its edge
    pub backspace: DeleteUnit,    // Grapheme | CodePoint (Pango's backspace_deletes_character scripts)
    pub filter: Option<InputFilter>, // a pure fn(&str) -> Cow<str> (numeric fields, identifiers)
}
```

`WordRules` is an enum with an extension variant (`Custom(&'static dyn WordClassifier)`) so a
cadatomic field (an identifier field with `snake_case` as one word) can bring its own. Word
offsets are byte offsets (FX1's are char offsets today: the port converts).

### 4.5 Commands, input and host requests

TC0 owns a small, closed set of field commands (quire-core's `EditorCommand` stays the rich
VS Code-id set and lowers into these where they overlap):

```rust
#[non_exhaustive]
pub enum EditCommand {
    Insert(String, InputKind),          // Typing, Paste, Ime, Drop, …  (InputKind from quire)
    Move { unit: Unit, dir: Dir, extend: bool },   // Unit: Grapheme | Word | WordEnd | LineEdge
                                                    //       | VisualLine | Page | DocEdge
    Delete { unit: Unit, dir: Dir },    // BackSpace/Delete/Ctrl+BackSpace/Ctrl+Delete/Ctrl+U/K
    SelectAll, SelectWordAt(usize), SelectLineAt(usize), // double/triple click
    SetSelection(Selection),            // pointer drag, AccessKit SetTextSelection
    ReplaceSelection(String),           // AccessKit ReplaceSelectedText, drop
    Undo, Redo,
    Cut, Copy, Paste,                   // → host requests (Paste: ReadClipboard, then Insert)
    Preedit(PreeditUpdate),             // Start | Update { text, active } | Commit(String) | Cancel
    HexEntry(HexKey),                   // GTK Ctrl+Shift+U: digits, Space/Enter commit, Esc cancel
}

pub trait VisualLines {                 // implemented by the caller on its layout
    fn visual_line_of(&self, byte: usize, affinity: Affinity) -> VisualLine; // range + x of byte
    fn byte_at_x(&self, line: VisualLineIx, x: f32) -> usize;
    fn lines_per_page(&self) -> usize;
}

pub struct Executed {
    pub applied: Option<Applied>,       // text and/or selection changed
    pub requests: SmallVec<[HostRequest; 1]>, // SetClipboard(String) | ReadClipboard
                                         // | SetPrimary(String) | ClearPrimary | Beep
    pub reveal: Option<Reveal>,         // the caller scrolls the caret into view
    pub rejected: Option<Rejected>,     // MaxChars | Filtered | ReadOnly (svelters may flash)
}

impl TextDoc {
    pub fn execute(&mut self, view: &mut TextView, cmd: EditCommand, rules: &FieldRules,
                   layout: &dyn VisualLines, time_ms: u64) -> Executed;
}
```

- **Sticky x** (the goal column for Up/Down) lives on `TextView`, reset by any non-vertical motion.
- **Undo grouping inputs** come from `InputKind` plus the command (FX3's "what made the edit":
  typing, paste, BackSpace, Delete, word deletions, cut) and from caret moves executed through
  `Move`/`SetSelection`, so svelters no longer reconstructs them from a before/after text diff.
- **PRIMARY**: TC0 emits `SetPrimary` when a selection made by the user becomes non-empty and
  `ClearPrimary` when it collapses *if* `rules` ask (GTK); svelters decides ownership across
  fields (`one_selection_owner`) because that spans views.

### 4.6 What svelters keeps (paint, caret, scroll) and how a field lives

- **Lazy session.** *Decided (design).* An unfocused, never-edited field holds no `TextDoc`: it
  paints its `value` prop (a `String` the scene already stores) through the galley cache. On
  focus, pointer press or an AccessKit text action, the kind creates `TextDoc + TextView` in an
  ext column (`TEXT_SESSION: ExtKey<TextSession>`); it keeps it while the node lives so undo
  survives blur and refocus (GTK and Qt keep a field's undo stack across focus changes). This
  holds whatever backend TC-EVAL picks, and bounds the 10k-field case to the fields touched.
- **Prop sync.** `value` from JS that equals the session's text is a no-op (the echo of our own
  `bind:` write). Any other value is `replace_all(v, ExternalSet::ResetHistory)` and clamps the
  selection (FX3: a text set from outside starts a new history).
- **Paint** (§3): one galley per field, rebuilt when text, font or wrap width change. Selection
  highlight from `::selection` (`background-color`, `color`; focused vs unfocused per Policy
  `keep_selection_unfocused`), caret as a 1-px (DPI-snapped) rect in `caret-color`, blink driven
  by `anim.rs` at the platform rate (`native::Policy::caret_blink_ms`, new; 0 = no blink; GTK
  stops blinking after `gtk-cursor-blink-timeout`, 10 s, which also saves frames), preedit underlined (active range thicker),
  `::placeholder` text when empty, the platform's mask character (U+25CF on GTK and Qt Fusion) for `secret`.
- **Scroll into view.** Single-line: horizontal offset kept so the caret stays in the content
  box; how far it jumps when the caret leaves is a WG3 measurement to add (a pin), not a guess. Multi-line: a multi-line `text-edit` is a
  `Scrollable` (§1.2, §3.3) and asks it to reveal the caret rect.
- **Keys.** The text-input behaviour maps `egui::Event::{Text, Key, Paste, Copy, Cut, Ime}` to
  `EditCommand`s through the platform key table (FX3's chords, Mac's Option/Command motions), and
  claims them for EV3 (`consume_key`, `composing`) exactly as FX3 does today.
- **IME**: `egui::Event::Ime(Enabled|Preedit|Commit|Disabled)` → `PreeditUpdate`; the caret rect
  goes to `PlatformOutput::ime` every frame the field is focused (`IMEOutput { rect, cursor_rect }`).
- **Clipboard** goes through EV3's host clipboard (DX2 `ClipboardBackend`), which keeps CRLF from
  a Ctrl+V paste (FX3's open paste-multiline checkpoint: svelters reads the clipboard itself
  instead of egui-winit's `Event::Paste`).

### 4.7 TC0 acceptance (for the extraction lane)

1. quire-core builds on `quire-text` with its tests unchanged (cadatomic's gate).
2. FX1's word tests and FX3's undo replays (`native/undo.rs` unit tests: pin 4 against the GTK and
   Qt goldens) move into `quire-text` and pass there; svelters' `egui_measure` keeps its floors
   once the svelters field runs on TC0 (lane OW4, §7).
3. `cargo check --target wasm32-unknown-unknown -p quire-text`; no egui, svelters, ropey (public)
   or std-only clock in the dependency tree.
4. `TextDoc::new("short")` allocates no history or ring (a test with a counting allocator), so
   TC-EVAL's creation numbers hold.

---

## 5. Accessibility

Each unit contributes to the AccessKit node of its host (`Unit::a11y`), the widget sets the role
and name, and the walk adds what is common (disabled, hidden for inert, bounds, `described_by`
for tooltips). Nodes are built with `Context::accesskit_node_builder(id, …)`; action requests are
read per id from `InputState` and handed to the units in `InputCx::a11y`, so an AccessKit `Click`
goes through the same `Pressable` path as a pointer click (and the same EV1 dispatch).

| Unit | Properties and states | Actions handled |
|---|---|---|
| Pressable | — | `Click` |
| Focusable | focusable | `Focus`, `Blur` |
| Toggleable | `toggled` (`True`/`False`/`Mixed` for indeterminate) | `Click` toggles |
| Steppable / DragToValue | `numeric_value`, `min_numeric_value`, `max_numeric_value`, `numeric_value_step`, `numeric_value_jump` (page) | `Increment`, `Decrement`, `SetValue` |
| Scrollable | `scroll_x`, `scroll_x_min/max`, `scroll_y`, `scroll_y_min/max`; scrollbar parts as `Role::ScrollBar` with value | `ScrollUp/Down/Left/Right`, `ScrollIntoView` (on descendants), `SetScrollOffset` |
| Selectable | `selected` on items, `multiselectable` on the container, `active_descendant` for the current row; virtual rows through `Widget::virtual_children` (SG5) | `Click` selects, `Focus` on a row moves the cursor |
| Expandable | `expanded` | `Expand`, `Collapse` |
| Dismissable / Anchored | popup node `modal` (modal only); opener gets `expanded` and `controls` → popup; popup role from widget (`Menu`, `ListBox`, `Dialog`, `Tooltip`) | `Click` on the opener toggles; Escape is keyboard only |
| Tooltipable | host `described_by` → the tooltip node (also when disabled: R9's reason is read) | — |
| TextInput | `value` (or none when `secret`), `placeholder`, `read_only`, `required`, `invalid`, `multiline`; children `Role::TextRun` with `character_lengths`, `character_positions`, `character_widths`, `word_lengths`; `text_selection` (anchor and focus `TextPosition`s) | `SetTextSelection`, `ReplaceSelectedText`, `SetValue`, `Focus` |
| DropTarget / Resizable | `Role::Splitter` + value for sashes | `Increment`/`Decrement` on sashes |

**Text.** egui 0.37 exposes the text-run builder used by `TextEdit`
(`egui::text_selection::accesskit_text::update_accesskit_for_text_widget`, a function, not a
widget); OW4 reuses it with our galley and maps TC0's byte offsets to galley char cursors and
back (`TextPosition` ↔ `CCursor`). The preedit (IME, hex entry) is exposed as part of the value
while composing, which FX3 lists as missing today. A `secret` field exposes `•` runs of the
right length and no value.

**Gates.** The existing AccessKit-query kittests (`widgets.rs::every_kind_has_a_tooltip_and_inputs_are_named`,
`invalid_reaches_accesskit`, the per-kind role queries in `tier0.rs`, `menus.rs`, `views.rs`,
`wg5a.rs`) must pass unchanged on the own implementation; that is the "no a11y regression" proof
of each swap. TK3's "no id clashes across roots" applies to node parts too.

---

## 6. Widget inventory and migration

### 6.1 Every current kind, its composition and acceptance

45 built-in tags (`kinds/builtin.rs` `KINDS`, inventory of 2026-10-22). "Units" lists behaviour
units; "Parts" lists `::part` names (N = node part, P = paint part). Sizes are estimates of the
own implementation (S < 200 lines, M 200–600, L 600–1500, XL > 1500), excluding tests. Tests are
under `crates/svelters-egui/tests/` unless noted; "sweep" = the every-kind tests
(`spec.rs::kind_table_is_consistent`, `observe.rs::every_prop_is_observable_and_host_writes_never_echo`,
`widgets.rs::every_kind_has_a_tooltip_and_inputs_are_named`, `scene/tests.rs::every_builtin_key_names_a_prop_of_its_kind`),
which every swap must keep green.

| Kind (today's egui) | Primitives | Units | Parts | Acceptance (existing tests; VIS1; pins) | Size | Lane |
|---|---|---|---|---|---|---|
| `button` (Button) | box, text, icon | Hoverable, Pressable, Focusable | — | `focus_keys.rs` ev3_* (default/cancel), `native_focus.rs` fx1_*, `style.rs::hover_restyles_in_the_same_frame`, `style_shadow.rs`, `theme.rs` (both), `animator.rs` exiting focus, `queries.rs` hit test; VIS1 button checks incl. `oc`; pin 7 (focus-tab, focus-click: button "c") | S | **OW0** |
| `box` (none; RectShape from style.rs) | box | (Hoverable for `:hover`, as the walk does) | — | `style.rs`, `style_shadow.rs`, `passes.rs` | S | **OW0** |
| `label` (Label) | text | Hoverable; Pressable when it has `onclicked`; selection (`selectable`) via TextInput read-only | `selection` P | `scene.rs::a_label_change_relays_out_its_boundary_only`, `widgets.rs::font_weight_maps_to_registered_families`, `weight_fallback_without_glyphs`; VIS1 text checks | S (M with selectable) | OW1a |
| `hyperlink` (Link/Hyperlink) | text (underline on hover, R18) | Hoverable, Pressable, Focusable | — | `tier0.rs::hyperlink_opens_its_url_and_is_a_link` | S | OW1a |
| `checkbox` (Checkbox) | box, text, icon | Hoverable, Pressable, Focusable, Toggleable | `indicator` P (the box), `mark` P (codicon `check`/`dash`) | `observe.rs::widget_changes_are_announced_exactly_when_observed`, `scene.rs::rust_write_of_a_bound_prop…`, `mutations_allocate_nothing_without_observers`, `style.rs::prop_states_and_grid`, `passes.rs::an_overlay_root_over_a_canvas…`; VIS1 `check-bg`/`check-fg` | S | OW1a |
| `radio-button` (RadioButton) | box, text, path (dot) | + Toggleable(radio group), Roving (arrows in group) | `indicator` P, `dot` P | `tier0.rs::radio_buttons_bind_group_and_checked` | S | OW1a |
| `switch` (self-painted) | box, text | Hoverable, Pressable, Focusable, Toggleable | `track` P, `knob` P (knob position animates via DS2 `transition` on a `--knob` offset; R17 "a switch moves its knob") | `tier0.rs::switch_toggles_active` | S | OW1a |
| `toggle-button` (Button::selected) | box, text, icon | Hoverable, Pressable, Focusable, Toggleable (+radio) | — | `widgets.rs::toggle_buttons_toggle_and_radio_ones_stay_checked`; VIS1 `toggle-hover`, `toggle-on-fg` | S | OW1a |
| `separator` (Separator) | box | — | — | `widgets.rs::separators_fill_along_their_orientation` | S | OW1a |
| `icon` (Button/Label) | icon, box (frame) | Hoverable, Pressable (when clickable), Focusable (when framed) | — | `widgets.rs::icons_resolve_names_and_icon_buttons_click`, `icon_fallback_*` (3); ICR1's acceptance | S | OW1a |
| `image` (self-painted) | image | Pressable (when clickable) | — | `tier0.rs::image_shows_its_source_at_its_size` | S | OW1a |
| `progress-bar` (ProgressBar) | box, text | — | `track` P, `fill` P (indeterminate: animated `fill`) | `tier0.rs::progress_bar_and_spinner_are_progress_indicators`; VIS1 progress height (open gap 7) | S | OW1a |
| `spinner` (Spinner) | path (arc) | — | — | same test | S | OW1a |
| `frame` (Frame as shape) | box, text (caption) | — | `caption` P | `tier0.rs::frame_pads_its_content_by_its_margins_and_caption` | S | OW1a |
| `collapsing-header` (self-painted, egui icon helper) | box, text, icon (twistie) | Hoverable, Pressable, Focusable, Expandable | `header` N, `twistie` P | `widgets.rs::collapsing_header_toggles_open_and_lays_out_its_body`; VIS1 pane header (open gap 2) | S | OW1a |
| `slider` (Slider) | box, text (value) | Hoverable, Focusable, DragToValue, Steppable | `track` N, `fill` P, `thumb` N, `value` P | `observe.rs::slider_clamp_is_announced`, `observe.rs::widget_changes…`, `scene.rs` bound-prop tests; EV3 slider key claims | M | OW1b |
| `range-slider` (RangeSlider + DragValue) | box, text | + two thumbs, Roving between them | `track` N, `range` P, `low` N, `high` N | `tier0.rs::range_slider_moves_one_end`, `uncaptioned_range_slider_names_its_value_boxes`, `bad_input_falls_back_and_never_panics` | M | OW1b |
| `drag-value` (DragValue) | box, text; TextInput when typed into | Hoverable, Focusable, DragToValue, Steppable, TextInput (on click) | `inc` N, `dec` N (FM1 QW7 step buttons, opt-in), `text` P | `tier0.rs::drag_value_edits_its_value_within_bounds`, `bad_input…`; FM1 QW7 commit mode, PgUp ×10, wrap, prefix/suffix | M | OW1b |
| `scroll-area` (ScrollArea) | box | Scrollable | `scrollbar-x` N, `scrollbar-y` N, `thumb` N (per bar), `shadow-top/bottom/left/right` P | `widgets.rs::scroll_area_scrolls_and_binds_its_offset`; WG3s pin 9; DESIGN R15 | M | OW2 |
| `panel` (self-painted) | box | Resizable (edge) | `sash` N | `tier0.rs::panel_resizes_by_its_edge` | S | OW2 |
| `split-view` (self-painted) | box | Resizable | `sash` N (R11 grip after 300 ms) | `views.rs::split_view_places_two_panes_and_the_divider_moves_them` | S | OW2 |
| `window` (CentralPanel, Frame::central_panel) | box (background) | — | — | `window.rs`, viewports tests | S | OW2 |
| `grid`, `sides`, `stack` (none) | — (layout only) | — | — | `widgets.rs::grid_places_children_in_columns`, `tier0.rs::sides_…`, `stack_shows_one_child` | — | unchanged |
| `popup` (Popup) | box | Dismissable, Anchored, Focusable (dialog scope, EV3) | `backdrop` N (none for popups) | `menus.rs::popup_attached_to_a_button` | M | OW3 |
| `context-menu` (Popup Menu) | box | Dismissable, Anchored (at pointer), Roving, Typeahead | — | `menus.rs::context_menu_on_a_leaf_and_on_a_box`; FM1 QA8 (Menu key, Shift+F10) | M | OW3 |
| `menu-bar` (MenuBar) | box, text | Roving (left/right), Pressable; hover-to-switch while open | — | `menus.rs::kinds_resolve_and_entries_stay_out_of_boxes`, `menus_app.rs::gallery_page_menus_and_popups`; pin 8 (menus, not yet captured) | M | OW3 |
| `menu` (MenuButton/SubMenuButton) | box, text, icon (submenu chevron) | Pressable, Hoverable (open on hover after delay), Anchored (`right-start`, flip), Dismissable | `chevron` P | same | M | OW3 |
| `action` (Button/Checkbox/RadioButton in menus) | box, text, icon (check), text (shortcut) | Pressable (activate on release), Toggleable (checkable/radio) | `check` P, `shortcut` P | same; `focus_keys.rs::ev3_focus_returns_after_a_menu_closes` | S | OW3 |
| `menu-separator` | box | — | — | same | S | OW3 |
| `menu-button` (Button + MenuButton) | box, text, icon | Pressable, Focusable, Dismissable, Anchored | `chevron` P (R14) | `menus.rs::menu_button_opens_its_menu_and_reports_active` | S | OW3 |
| `combo-box` → select (ComboBox + selectable_label) | box, text, icon | Pressable, Focusable, Dismissable, Anchored (`match_anchor_width`), Selectable (list), Typeahead (WG6 QW9), Roving | `chevron` P (codicon, VIS1 gap 8), `popup` N, `option` N per option | `widgets.rs::combo_box_picks_an_option`; VIS1 select checks | M | OW3 |
| common `tooltip` prop (`on_hover_text`) | box, text | Tooltipable (also on disabled, WG6 QA5) | — | `widgets.rs::every_kind_has_a_tooltip…` (sweep); VIS1 tooltip (gap 9) | S | OW3 |
| `text-edit` (TextEdit) | box, text (galley, selection, caret, preedit) | Hoverable, Focusable, TextInput (TC0) | `placeholder` P, `selection` P, `caret` P | `focus_keys.rs` ev3_text_* (3), `native_focus.rs` (2), `tier0.rs::text_edit_uses_the_primary_selection`, `widgets.rs::invalid_reaches_accesskit`, `observe.rs`, `clipboard.rs::table_copies_as_tsv…_and_pastes_into_text_edit`, `viewports.rs` caret tests (rewrite: they read `TextEdit::load_state`), `native_edit.rs` fx3_* (ci/fx3); **pins 1–7** via `svelters-conformance/tests/egui_measure.rs` (`FLOOR`: today 141/139 on pins 1–3, 7; 232/235 on all 18 scripts with FX3) | L | OW4 |
| `virtual-list` (self-painted) | box, text | Selectable, Typeahead, Scrollable (replaces its own bar), DragSource/DropTarget (opt-in) | `row` (virtual), `scrollbar-y` N | `vlist.rs` (4), `wg5a.rs` virtual_list_* (2), `scene/virt.rs` (3); R6 four row states | M | OW5 |
| `table` (egui_table + Label + TextEdit) | box, text, icon (sort) | Selectable (cells/rows), Scrollable (both axes), Resizable (columns), Typeahead; editor via OW4's field | `header` N, `header-cell` (virtual), `row`/`cell` (virtual), `sort` P, `resize` N, `error` (tooltip) | `views.rs::table_asks_for_its_window…`, `wg5a.rs` (3 table tests), `views/table.rs` unit tests, `gallery_views.rs`; FM1 QW4 header state | XL (drops `egui_table`, whose ScrollArea is a forbidden widget) | OW5 |
| `table-column` (interact) | — | — | — | same | S | OW5 |
| `tab-view` (ScrollArea + Button::selectable) | box, text, icon | Selectable(single), Roving, Scrollable (x, hidden bar), Pressable (middle-click close) | `tab` N, `close` N (R13: always on active), `dirty` P, `overflow` N | `views.rs::tab_view_shows_the_selected_page…`; VIS1 tabs (gap 6) | M | OW5 |
| `tab` (none) | — | — | — | same | S | OW5 |
| drag and drop (any node, `kinds/dnd.rs`) | box (ghost, drop line) | DragSource, DropTarget (move, unchanged behaviour) | `ghost` P, `drop-line` P | `wg4b.rs::drag_and_drop_between_two_nodes`, `nested_drop_targets…`, `dnd.rs` units; WG6 QA9 file drops | M | OW5 |
| `date-picker` (egui_extras DatePickerButton) | box, text, icon | Pressable, Dismissable, Anchored; calendar: Selectable + Roving (grid), Steppable (month) | `popup` N, `day` N, `header` N | `wg4b.rs::date_picker_saves_a_day_through_its_popup` | M | OW6 |
| `color-picker` (color_edit_button_hsva) | box, path (SV square, hue strip), text field | Pressable, Dismissable, Anchored; DragToValue (2-D on the square, 1-D on strips), TextInput (hex) | `swatch` P, `popup` N, `square` N, `hue` N, `alpha` N | `wg4b.rs::color_picker_normalizes_hex_and_edits_through_its_popup` | M | OW6 |
| `modal` (Modal) | box, text (title) | Dismissable (`dismissable`), Focusable (dialog scope), Anchored (centred) | `backdrop` N, `title` P | `menus.rs::modal_dialog`, `focus_keys.rs::ev3_modal_overlay_is_a_dialog_scope`; FM1 QW11 button order | M | OW7 |
| `inner-window` (Window) | box, text, icon | Draggable (title bar), Resizable (edges, corners), Pressable (close, collapse), Focusable (dialog scope) | `titlebar` N, `close` N, `collapse` N, `edge-*` N | `menus.rs::inner_window_with_content_and_close_button` | M | OW7 |
| `toast-host` (Area + Frame + Label + Button) | box, text, icon | Dismissable (timeout, close), Pressable | `toast` N, `close` N, `accent` P | `views.rs::toast_host_shows_toasts_from_js_methods_and_rust` | S | OW7 |
| `canvas` (self-painted) | path | Pressable (when clicked) | — | `tier0.rs::canvas_replays…`, `canvas.rs` units | — | unchanged (moves its `allocate_rect` to `cx.interact`) |

Not in the table: provider kinds (cadatomic's, `examples/egui-kinds`) are already self-drawn and
gain units, primitives and parts as opt-in helpers; `dialogs.rs` (`rfd` native file and message
dialogs) stays native; the `invalid` outline (`leaf.rs::invalid_outline`) becomes a base-sheet rule
(`:invalid { outline: 1px solid var(--sv-invalid-border) }`).

### 6.2 Migration with no flag day

1. **Same vocabulary.** The built-in row in `KINDS` (tag, kind id, prop and signal ids, methods,
   spec fragment, typings) is not touched. An own kind declares `replaces = "<tag>"`; the registry
   checks at startup (and `tests/spec.rs` checks at test time) that the struct's props and events
   cover the row's with the same names and types, and routes the row's prop ids to the struct's
   fields. Compiled Svelte, `metadata/semantics/egui.json` and cadatomic's code see no change.
   `replaces` is crate-private to svelters-egui (*Decided (design)*: apps make new tags in their
   namespace and restyle built-ins through CSS; replacing a built-in is not an extension point).
2. **Both implementations live during the lane.** `EguiBackend::set_own_widgets(KindSet)` plus
   `SVELTERS_OWN_WIDGETS=all|none|<tag,…>` pick the implementation per kind; the default for a kind
   flips to "own" in the commit that makes its acceptance pass. The kind's test files run under
   both in that lane only (a `#[test_matrix]`-style helper in `tests/common`), never in CI after it.
3. **Delete in the same lane.** When the own kind passes its acceptance, the lane deletes the egui
   wrapper code, its `layout.rs` size copies and its `#[allow(clippy::disallowed_types)]` line, and
   the switch entry. One lane may not leave both alive at merge (coordinator's merge check).
4. **Sizes change on purpose.** Own widgets are sized by CSS (VS Code metrics through the theme),
   not by egui's `Spacing`; TK1 snapshot cases and kittest pixel positions of the swapped kinds are
   regenerated in the lane (generated-file rule), and tests that assert egui sizes are rewritten to
   assert CSS-derived sizes.
5. **Interim code is not extended.** No new feature lands on an egui wrapper once its lane has
   started; FX3 is the exception (§7.3) because it is finished and holds the reference behaviour.

---

## 7. Lane plan

### 7.1 OW0: the foundation, in two pushes

**OW0a, the API slice (target: 3–4 hours, pushed as `ci/ow0` and announced; every OW lane branches
from it).** Compiles, does little:
- `paint/` signatures with working `paint_box` (uniform borders, radius, outer/inset shadows moved
  from `style/shadow.rs`), `paint_text` (single-line, colour, size, weight, alignment), `paint_outline`;
  `StyleView` over `ComputedStyle` (DS2's transition overlay is a no-op hook);
- `interact/` with `Unit`, `InputCx`, `Intents`, `FocusCx`, and `Hoverable`, `Pressable`,
  `Focusable` implemented; `UseUnit<U>` adapter skeleton;
- state bits 14–22 in both tables (pin test updated); `focus-visible` driven by EV3's focus reason;
- macro: `#[behavior]` fields and `replaces` (props routing + startup check); `#[part]` parsed and
  stored (node parts created through `insert_shadow`, paint parts as names only);
- `cx.interact()`, `cx.paint_box()`, `cx.paint_content()`, `cx.paint_outline()`, `cx.dispatch()`
  (direct emit until EV1 lands), the own-widget switch;
- `button` as the proof widget behind the switch, with its tests passing on both implementations.

**OW0b, the rest of the foundation (same lane, about one more day):** per-side borders, gradients
(2-stop), text wrapping and elision (end/middle/start), icon primitive on today's codicon font (ICR1
swaps the source), PaintTrace, the ZU3-style lint with the wrapper allow-list, `box` painted by the
primitives, `Native::Windows`/`Mac` rows, the `use:` export of the three units
(`svelters:behaviors/sv`), docs page draft "Write a widget". Acceptance: button's tests and VIS1
button checks pass on own; `box`'s style tests pass; the lint lists every remaining wrapper.

### 7.2 Parallel lanes

Machines: **WSL** (16 cores; coordinator), **thinkpad** (12 cores, 62 GB), **macbook** (12 cores;
also runs the TC0 extraction for cadatomic). Sizes: S < 1 day, M 1–2 days, L 2–4 days, XL > 4 days
for one agent.

| Lane | Scope | Depends on | Machine | Size | Acceptance |
|---|---|---|---|---|---|
| **OW0** | §7.1: primitives, unit core (Hoverable, Pressable, Focusable), macro additions, state bits, switch, `button`, `box` | — | WSL | L (OW0a: hours) | button tests on both impls; VIS1 button checks incl. `oc`; spec/observe sweeps |
| **OW-CSS** (DS2 re-scoped) | engine: `::part()`, `::placeholder`, `::selection` (paint-part styles computed as pseudo-elements), pseudo-classes for bits 14–22, `outline-*`, per-side `border-*`, `cursor`, `transition` (+ the `Transitions` store behind `StyleView`), `text-transform`, `letter-spacing`, `text-overflow`, 2-stop `linear-gradient` | none for the engine crate; OW0a for the `StyleView` wiring | thinkpad | L | `svelters-style` unit tests per feature; a part-styled button variant in the gallery; transitions sampled in kittest with the virtual clock; reduced motion instant |
| **VIS1c** | merge `ci/vis1`'s references, capture tooling, gate and theme roles, without the `Visuals` mapping; switch the gate to PaintTrace lookup | OW0b (PaintTrace) for the switch; the merge itself none | WSL | S | gate runs in linux-gtk job on own `button`; baseline recorded |
| **OW1a** simple controls | `label`, `hyperlink`, `checkbox`, `radio-button`, `switch`, `toggle-button`, `separator`, `icon`, `image`, `progress-bar`, `spinner`, `frame`, `collapsing-header`; units Toggleable, Expandable, Roving (radio groups) | OW0a (branch), OW-CSS parts for `mark`/`indicator` (paint with hard-coded part styles until it lands) | thinkpad | L (13 small kinds) | §6.1 tests per kind; VIS1 check/toggle/link/progress checks; wrappers deleted |
| **OW1b** value controls | `slider`, `range-slider`, `drag-value` (+ FM1 QW7: commit mode, step buttons, wheel, PgUp ×10, wrap, prefix/suffix); units DragToValue, Steppable, Draggable; `commit`/`input` signals (§2.4) | OW0a; CX3 for `|commit` lowering tests | macbook | M | §6.1 tests; EV3 key claims; a `bind:value|commit` fixture writes once per release; AccessKit numeric value + Increment/Decrement |
| **OW2** scroll and containers | Scrollable (wheel, trackpad, kinetic per Policy, chaining, overlay bars, edge shadows, `scrollIntoView`, focus scroll); Resizable; `scroll-area`, `panel`, `split-view`, `window` background | OW0a; WG3s pin 9–10 goldens when captured | WSL | L | `scroll_area_scrolls_and_binds_its_offset`, `panel_resizes_by_its_edge`, `split_view_places…`; WG3s pins 9–10 floors; R15 shadows; WG6 QA13 test |
| **OW3** popups, menus, select, tooltip | Anchored (placement, flip, shift), Dismissable, Typeahead, Tooltipable; Area layering (§3.2); `popup`, `context-menu`, `menu-bar`, `menu`, `action`, `menu-separator`, `menu-button`, `combo-box`, common `tooltip`; FM1 QA8; WG6 QA5, QW9 | OW0a; **EV1** (dispatch, filters, overlays) for outside-press and Escape ordering | WSL | L | `menus.rs` (all), `menus_app.rs`, `focus_keys.rs::ev3_focus_returns_after_a_menu_closes`, `combo_box_picks_an_option`; VIS1 menu/tooltip/select checks (gaps 8–9); pin 8 when captured |
| **OW4** text input on TC0 | TextInput unit; `text-edit` single/multi/password, placeholder, validation states, IME and hex entry, PRIMARY, caret blink, scroll into view, AccessKit text runs; table cell editor uses it | OW0a; TC0 (`quire-text` rev) for phase b; phase a (paint, caret, hit-test, IME rect, a11y runs) runs on a private `String` buffer behind the §4 shapes | macbook | L | `egui_measure` floors kept or raised on **all 18 scripts** (≥ 232 GTK / 235 Qt after FX3); ev3_text_* tests; `invalid_reaches_accesskit`; clipboard test; TC0 acceptance §4.7 |
| **OW5** lists, tree, tabs, table, DnD | restyle `virtual-list` (R6 four states as CSS), tree rows (Expandable), `tab-view`/`tab` (R13), own `table` without `egui_table` (virtual rows and columns on Scrollable, Resizable columns, FM1 QW4 header state), DnD units (WG6 QA9 file drops) | OW0a, OW2 (Scrollable), OW4 for the table cell editor (until then the table keeps its editor) | thinkpad (after OW1a) | XL | `vlist.rs`, `wg5a.rs` (all 6, incl. the 100k-row keyboard test), `views.rs` table/tab tests, `wg4b.rs` DnD; VIS1 list/tab checks (gaps 5–6) |
| **OW6** pickers | `date-picker`, `color-picker` own popups | OW1b, OW3, OW4 | macbook (after OW1b) | M | `wg4b.rs` picker tests; AccessKit grid roles |
| **OW7** dialogs, windows, toasts | `modal`, `inner-window`, `toast-host`; FM1 QW11 button order (Policy) | OW2 (Resizable), OW3 (Dismissable, layers) | any free | M | `menus.rs::modal_dialog`, `inner_window_with_content_and_close_button`, `ev3_modal_overlay_is_a_dialog_scope`, toast test; VIS1 dialog/toast checks |
| **OW-DONE** | lint to zero; delete `Native::Egui`, egui size copies in `layout.rs`, the `egui_extras`/`egui_table` dependencies and the `pub use egui_extras`; own-widget switch removed | all OW lanes | coordinator | S | `cargo tree -i egui_extras` empty; lint with no allow-list |

Critical path: OW0a → OW3 (needs EV1) → OW6/OW7; OW0a → OW2 → OW5. With OW0a pushed in hours,
OW-CSS, OW1a, OW1b, OW2 and OW4 phase a start the same day on three machines.

### 7.3 In-flight lanes that change

| Lane | Change |
|---|---|
| **EV1** (`ci/ev1`, WSL) | Add `WidgetCx::dispatch` / `BehaviorCx::dispatch` for unit intents: the `SceneEvent` runs Rust filters (capture, target, bubble) **before** the widget's default action and returns whether it was prevented. Events from node parts retarget to the host (DOM shadow-tree rule). SV7's overlay trait uses §3.2's Areas. OW3 depends on EV1; OW0 ships a direct-emit stub with the same signature. |
| **CX3** (`ci/cx3`) | Lower `bind:value|commit` to the uniform `commit` signal (§2.4) on every value kind; `input` is the live signal (alias of `changed` until docs move). |
| **DS2** | Re-scoped as OW-CSS (engine and transitions store); the painting half is OW0's. Starts now, no longer waits for VIS1 to merge. |
| **DS3** (affordances) | Written against parts and the new pseudo-classes (`affordance.css`, R1–R18) after OW1a; no `Visuals`. `focus_ring`/`paint_row_state` helpers become CSS rules. |
| **DS4** (importer), **DS5** (icons) | DS4 unchanged; DS5 stays absorbed by ICR1. |
| **DS1** `theme/egui_style.rs` | Kept only for host Rust chrome that still draws egui widgets (cadatomic, until Q5); svelters widgets never read it. |
| **ICR1** | The icon primitive calls `IconRegistry::resolve(key, physical_px) -> IconSource { Glyph(FontId, char) | Texture(TextureId, Rect) }`; ICR1 adds no egui widget use (the `icon` element itself is OW1a's). |
| **ZM1** | No API change; its 80–150 % tests include each own kind as it lands. |
| **WG3s** (pins 9–10) | Start capture now (native side is independent); the goldens are OW2's (scroll) and OW0/OW1b's (pointer and drag thresholds) acceptance. |
| **WG3w** (Windows refs) | Its results fill `Native::Windows` (W6). A macOS reference lane (WG3m) is added to the queue for `Native::Mac`. |
| **FX3** (`ci/fx3`) | Merge it as is (232/235 on all scripts; its record lists the tests that pass) so `egui_measure`'s floors on master are the bar OW4 must keep; `native/undo.rs` and the `words.rs` rules move to `quire-text` (§4.7); `native/edit.rs` becomes OW4's key table. |
| **FM1** (queued) | Dissolved: QW7 → OW1b, QW8 → OW4 and §2.4, QW4 → OW5, QW11 → OW7, QA8 → OW3. |
| **WG6** (stopped, `ci/wg6`) | Dissolved: QA5 and QW9 → OW3, QW13 (`elide="middle"`) → OW0b text primitive, QA13 → OW2, QW14 (SVG) → ICR1, QA14 (`cursor`) → OW-CSS + OW0b (cursor set on hover from computed `cursor`), QW10 (`key-sequence-edit`) → a new kind after OW4, QA9 → OW5. |
| **VIS1** (stopped, `ci/vis1`) | Resumed as VIS1c (§7.2). Its `vscode_look` → `Visuals` mapping is not merged. |
| **Docs** (DOCS-PLAN) | Each widget page lists props, signals (`input`/`commit`/`submit`), states, parts, units, AccessKit role; a "Write a widget" guide from OW0b's example; pages land with each OW lane. |
| **TK1** snapshot cases | Regenerated per kind in the swapping lane (sizes change on purpose, §6.2). |

---

## 8. Open questions for the user

Only architecture choices; each has a recommendation, and lanes proceed on the recommendation
until answered (the coordinator brings them together).

| # | Question | Recommendation |
|---|---|---|
| **Q1** | Where built-in widgets are defined: Rust kinds composed from behaviour units (A), Svelte components over primitives and `use:` units (B), or both? | **(A) for every built-in and for cadatomic's performance-sensitive widgets; (B) for composites** (app, extension and form-level), using the same units as `use:` actions. Built-ins in Svelte would put a component instance in every virtualised cell and JS on every row mount, and hot reload would buy little because the look is CSS and reloads anyway (§1.4). |
| **Q2** | Keep egui's `Ui::interact` (pointer picking against last pass's rects, drag capture, click timing, focus memory, AccessKit action routing) as the substrate under our units, or replace it with dispatch from our own hit list? | **Keep it.** It is engine plumbing, not a widget, and it already works across layers, viewports and AccessKit. Every semantic (thresholds, repeat, cancel, scrub, dismissal) is ours. Revisit only if EV1's capture/bubble needs picking egui cannot give. |
| **Q3** | Is the styling surface `::part(name)` plus `::placeholder`/`::selection` and the nine new pseudo-classes a **public, stable API** (cadatomic themes and extensions style built-ins through it), or internal to the theme? | **Public and stable**, versioned with each widget's docs page: it is what lets cadatomic restyle without Rust, and it is the standard CSS mechanism (Shadow Parts), so authors already know it. |
| **Q4** | The look of widgets VS Code does not have (slider, range slider, switch, spin buttons, date and colour pickers, inner windows): what reference? | **DESIGN's rules (R1–R18) with Fluent 2 / WinUI metrics mapped onto the VS Code tokens**, checked by rule-based VIS1 checks (contrast ≥ 3:1, lip, sunken fields) instead of crops. Fluent is the closest family to VS Code Modern. |
| **Q5** | Should cadatomic's Rust-owned shell chrome (toolbars, tab strips, menus that Rust draws in egui) also use our widget system, through an immediate-mode Rust façade (`svelters_egui::ui::button(ui, "Save")`) that runs the same units and primitives with styles computed for a synthetic element (tag + classes) against the theme? | **Yes, as lane OW-RUST after OW1a** (M). Otherwise the shell keeps egui widgets styled through DS1's `Visuals` mapping and the two looks drift (DESIGN §4.6); with the façade there is one look, one behaviour table and one a11y path. |

Not questions (left open on purpose): the TC0 text backend (rope, `String`, or promote-on-focus)
waits for TC-EVAL's numbers (§4.3).

### Decided (design, 2026-10-22)

- Units live in `crates/svelters-egui/src/interact/`, primitives in `paint/`, built-in widgets in
  `widgets/` (one file each); the provider macro gains `#[behavior]`, `#[part]` and the crate-private
  `replaces`.
- Two part kinds (node, paint) behind one `::part()` syntax; part names are listed per kind in §6.1.
- State bits 14–22 as in §2.2; prop-mirror bits refreshed and the node restyled in the same frame
  as the interaction (§2.3).
- Signals `input`, `commit`, `submit` on every value kind; drafts only in commit mode (§2.4).
- Glyphs (check, dash, chevrons, twisties, close) are icons from the registry, never hand-drawn.
- Layers: in-flow content in its slot's layer; popups `Foreground`; tooltips, drag ghost and
  preedit `Tooltip`; modal backdrop is a node part (§3.2).
- Pixel snapping as in §3.5; caches keyed by `ppp` (§3.6).
- PaintTrace for tests and VIS1; paint caching only after measurement (§3.8).
- The lint (`disallowed_types`) is the migration's burn-down and its end gate (§3.1, OW-DONE).
- TC0 sessions are created lazily and kept for the node's life (§4.6).
- `native::Policy` gains `Windows` and `Mac` rows and the unit fields of §1.2 (W6, following the
  user's earlier W1: native per-platform behaviour).
- FX3 merges as interim; FM1 and WG6 are dissolved into OW lanes (§7.3).

---

## Appendix A. OW0a file list

| File | Contents |
|---|---|
| `crates/svelters-egui/src/paint/mod.rs` | `PaintCx`, `StyleView`, `BoxGeom`, re-exports |
| `paint/boxes.rs` | `paint_box`, `paint_outline` (shadows moved from `style/shadow.rs`, which keeps `paint_bounds`) |
| `paint/text.rs` | `text_galley`, `paint_text`, `measure_text` |
| `paint/icon.rs`, `paint/image.rs` | stubs over today's codicon font and `images.rs` (OW0b) |
| `crates/svelters-egui/src/interact/mod.rs` | `Unit`, `InputCx`, `Intents`, `FocusCx`, `PartResponses`, `UseUnit` |
| `interact/{hover,press,focus}.rs` | the three units |
| `crates/svelters-egui/src/widgets/{mod,button}.rs` | own-widget switch (`KindSet`), `Button` |
| `provider/mod.rs` | `WidgetCx::{interact, paint_box, paint_content, paint_outline, dispatch, style}` |
| `provider/state.rs`, `svelters-style/src/state.rs` | bits 14–22, names |
| `crates/svelters-macros` | `#[behavior]`, `#[part]`, `replaces` |
| `kinds/registry.rs` | `replaces` routing and the startup coverage check |
| `tests/own_widgets.rs` | button on both implementations; registry coverage check |
