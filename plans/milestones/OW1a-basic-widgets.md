# OW1a: simple controls as svelters' own widgets (slice 1)

Lane OW1a (thinkpad, `ci/ow1a`), 2026-10-22. Branched from `ci/ow0` at 7eef7362 (OW0a, the API
slice) and merged with `origin/master` 91fcf421. No newer ci/ow0 commits existed when this was
pushed. Design: ARCH-WIDGETS §1–§3, §6.1, §7.2 (OW1a row), WIDGET-PARITY §2.1–§2.3, §2.9, §2.10,
§2.16, §5, DESIGN §1–§2.

**Status: slice 1 of 2.** The lead accepted the split (ow1a-lane → team-lead, 2026-10-22). Slice 1
lands eight kinds as own widgets behind the per-kind switch. Slice 2 takes the rest (§5).

## 1. What landed

| Kind | File | Units | Parts (paint) | Replaces |
|---|---|---|---|---|
| `checkbox` | `widgets/checkbox.rs` | Hoverable, Focusable, Pressable, **Toggleable** | `indicator`, `mark`, `label` | `egui::Checkbox` |
| `switch` | `widgets/switch.rs` | Hoverable, Focusable, Pressable, Toggleable | `track`, `knob`, `label` | self-painted wrapper (`elements/inputs.rs`) |
| `toggle-button` | `widgets/toggle_button.rs` | Hoverable, Focusable, Pressable, Toggleable (`radio`: on only) | — | `egui::Button::selected` |
| `label` | `widgets/label.rs` | Hoverable; Pressable only while `clicked` is listened to | — | `egui::Label` |
| `hyperlink` | `widgets/hyperlink.rs` | Hoverable, Focusable, Pressable | — | `egui::Link`/`Hyperlink` |
| `separator` | `widgets/separator.rs` | — | — | `egui::Separator` |
| `progress-bar` | `widgets/progress.rs` | — | `track`, `fill`, `label` | `egui::ProgressBar` |
| `spinner` | `widgets/progress.rs` | — | — (arc path) | `egui::Spinner` |

New pieces:
- **`interact/toggle.rs`: the `Toggleable` unit.** It has `ToggleParams { checked, indeterminate,
  tristate, radio }`, sends the intent `Toggle::Toggled`, owns `checked` and `indeterminate`, and
  sets AccessKit `toggled` (Mixed when indeterminate) and `Click`. The cycle follows Qt and GTK:
  a click on an indeterminate box checks it, and tristate cycles through partial. It is ready for
  slice 2's `indeterminate`/`tristate` props. Unit test: `ow1a_toggle_cycles`.
- **`widgets/parts.rs`: paint-part helpers.** `part_view` and `part_size` give each part its own
  UA table in the host's state, until OW-CSS computes part styles. The checkbox indicator reads
  the host's computed style for what `theme/base.css`'s `checkbox` rule already styles (fill,
  border, radius). Every other part is UA-only. Also here: path glyphs (check and dash, dot,
  fill) and `reduced_motion`.
- **`WidgetCx::listens(&Event)`** (`provider/mod.rs`): whether a runtime listens to a widget
  event. The label senses clicks only then, as the wrapper did.
- **`TraceKind::Path`** (`paint/trace.rs`): path parts in PaintTrace (check, dot, knob, arc,
  bar fill).

The switch's default is unchanged: `DEFAULT_OWN` stays empty and the egui wrappers stay. Use
`SVELTERS_OWN_WIDGETS=checkbox,switch,…` or `EguiBackend::set_own_widgets` to get the own widgets.
The flip and the wrapper deletion come in slice 2, once the kinds' existing test files pass under
both implementations (ARCH-WIDGETS §6.2 steps 2–3).

## 2. Acceptance status

### 2.1 DESIGN R-rules (affordances on by default, in the UA tables)

| Rule | Status |
|---|---|
| R2 pressed sinks / R3 hover in the same frame | done as fills: `:active` → `button-bg-active`, `:hover` → `button-bg-hover`, on the checkbox indicator and the toggle-button (the same frame, through `commit_state`); the switch track's hover. The lip and inset shadow are DS3's `affordance.css` (CSS `box-shadow`; the UA tables set no shadows) |
| R8 focus visible, keyboard only | done on checkbox (ring around the indicator), switch (around the track), toggle-button and hyperlink: 1 px `focus` outline at +1 on `:focus-visible` only. A click focuses (GTK) without a ring. Tested |
| R10 cursors | hyperlink: pointing hand. The rest waits for OW-CSS `cursor` + OW0b |
| R17 state by fill, not colour alone | checkbox: check or dash glyph plus the `toggle-on-bg`/`toggle-on-border` fill; switch: knob position plus accent fill; toggle-button: `toggle-on-bg` + 1 px `toggle-on-border` |
| R18 links | `link` colour, underlined while hovered |
| R9 disabled with a reason | partial: disabled colour `disabled-fg`, no toggling (tested). The tooltip on disabled hosts is OW3's (Tooltipable) |
| R14 chevrons | slice 2 (collapsing-header twistie) |

### 2.2 State bits (DOC1's gaps, coordinator decision)

| Bit | Status |
|---|---|
| `:checked` on toggle, switch (and checkbox) | **done**: Toggleable's prop mirror, the same frame. Tested (`ow1a_own_toggles_mirror_checked_and_show_focus_only_by_keyboard`) |
| `:checked` on radio | slice 2 (radio-button) |
| `:busy` | **done** on spinner (`spinning`) and progress-bar (`animate`). Tested |
| `:indeterminate` | unit-ready; the checkbox `indeterminate` prop comes in slice 2 (it needs a row change) |
| `:expanded` | slice 2 (collapsing-header) |
| `:invalid`, `:read-only` | none of slice 1's kinds has `invalid` or `read-only` props. These belong to slice 2's selectable label (read-only) and OW4 (fields) |

### 2.3 WIDGET-PARITY OW1a rows (§5: 3.9 d)

All are **deferred to OW1a2**, the fresh lane that takes the rest (coordinator, 2026-10-22). Q4
rich text went to RT2. The how-to for each row is in §5.2.

| Row (§) | Status |
|---|---|
| toggle-button `group` (§2.1) | deferred (needs the sibling-write seam, §5.1) |
| checkbox `indeterminate`/`tristate` (§2.2) | deferred; Toggleable already implements the semantics |
| checkbox `icon`, `::part(label)` (§2.2) | `label` part painted and traced; `icon` deferred |
| radio `icon`, label part, Roving (§2.3) | deferred with radio-button |
| progress `value/min/max`, `indeterminate`, `orientation`, `inverted`, `format`, `::part(label)` (§2.9) | parts `track`/`fill`/`label` and the busy sweep done; the props are deferred |
| label `for`, selection props (§2.10) | deferred; Q4 inline rich text → **RT2** (ci/rt2) |
| frame `checkable`, accordion, header `icon`, header actions (§2.16) | deferred with frame/collapsing-header |

### 2.4 VIS1

The VIS1 gate (`ci/vis1`) has not merged and VIS1c has not landed, so no VIS1 check ran. The
own kinds trace their parts (`indicator`, `mark`, `track`, `knob`, `fill`, `label`) for VIS1c's
PaintTrace lookup. The `check-bg`/`check-fg`, `toggle-hover`/`toggle-on-fg` and progress-height
(gap 7) checks move to own once VIS1c lands.

### 2.5 Pins

Pin 7 (focus-click/focus-tab): the own toggles follow FX1/GTK, where a click focuses without a
ring and Tab shows one. This is tested in kittest, not against the goldens.

## 3. Tests (all added in this lane)

`crates/svelters-egui/tests/ow1a.rs`. Behaviour tests run on both implementations:
- `ow1a_every_kind_replaces_its_builtin_row`
- `ow1a_checkbox_switch_and_toggle_toggle_on_both`: click, the `changed`/`clicked` payloads,
  radio stays on, disabled, AccessKit roles, toggled and Click
- `ow1a_space_toggles_the_focused_checkbox`
- `ow1a_own_toggles_mirror_checked_and_show_focus_only_by_keyboard`
- `ow1a_label_and_hyperlink_click_only_when_listened`
- `ow1a_wrapping_label_grows_in_height_for_width`
- `ow1a_separators_progress_and_spinner`

Plus `interact::toggle::tests::ow1a_toggle_cycles`.

## 4. Deviations and notes

- **Glyphs are paths, not icons.** ARCH says the check, dash and dot are registry icons. No icon
  font ships today and ICR1 is not on master, so `parts.rs` draws them as paths sized from the
  part box. P2 (`content: icon(check)`) replaces them once OW-CSS and ICR1 land.
- **Part styles are hard-coded UA tables** (as ARCH §7.2 allows) until OW-CSS lands `::part()`.
  UA tokens resolve against the built-in theme, so a `--sv-*` token override does not reach
  UA-only parts yet. Host CSS does reach the checkbox indicator.
- **Label `truncate`** uses an egui `LayoutJob` with `max_rows = 1` and the proportional face (not
  the weighted face). OW0b's elision primitive replaces it.
- **`reduced_motion`** reads egui's `animation_time == 0`, as kittest and `Viewports` set it, not
  the backend's `MotionConfig`, which a widget cannot reach. A follow-up should expose it on
  `WidgetCx`.

## 5. Handoff to OW1a2 (a fresh agent)

Start from master once this lane merges. Read ARCH-WIDGETS §1.2, §1.3, §6.1 and §6.2, and
WIDGET-PARITY §2.1–§2.3, §2.9, §2.10, §2.16 and §5. Use the slice 1 files as the pattern:
`widgets/checkbox.rs` (indicator kinds), `widgets/toggle_button.rs` (button-like),
`widgets/parts.rs` (paint parts) and `tests/ow1a.rs` (tests on both implementations).

### 5.1 Kinds left, in order

1. **radio-button** (`widgets/radio.rs`). It reuses `checkbox::measure_with_indicator` and
   `split` with a round `indicator` (radius 50 %) and a `dot` part (`parts::paint_dot`). Use
   `Toggleable` with `radio: true` and `Hoverable`/`Focusable`/`Pressable`. The `changed` signal
   carries **two** args (`checked`, `group`). The OW0 emit path wraps the payload in one arg
   (`provider/mod.rs` "an own widget's event keeps the built-in signal's arity"), so add a
   multi-arg payload there, or make the event's type a tuple that `IntoJs` spreads.
   **Sibling-write seam:** the wrapper (`elements/inputs.rs::radio`) unchecks siblings and writes
   their `group`. A `WidgetCx` writes only its own node, so this needs one of:
   - a `WidgetCx::defer(f)` that forwards to `SceneHandle::defer` (`scene/access.rs:303`; it
     applies next logic, so siblings repaint a frame late, as the wrapper's already do);
   - or EV1's dispatch.
   Ask the coordinator which. toggle-button `group` (§2.1) uses the same seam.
   **Roving** (`interact/roving.rs`, ARCH §1.2 catalogue): arrows move focus and check within
   the group (siblings with the same `group`, or the same parent when `group` is empty). The group
   has one tab stop: only the checked button, or the first, is a tab stop (`FocusParams::tab_stop`).
   Claims Up/Down/Left/Right.
2. **frame** and **collapsing-header**: containers, with their own `layout.rs` cases.
   - `layout.rs:271` (`W::CollapsingHeader`) pads the node by the header row and the indent. Move
     it to the own kind's `measure`/style: header row height = `header-h` token, indent =
     `indent` token, both through UA tables.
   - `layout.rs:417` measures the header row.
   - `layout.rs:515`/`:640` `Tree::collapsing` lays out the body only when `open`. The own kind
     needs `Caps::CONTAINER | CUSTOM_LAYOUT` (or a `display: none` style on the children when
     closed). Draw the children with `cx.children(ui)`.
   - The header is a **node part** `header` (`#[part(node)]`, SG4 `insert_shadow`) with
     Pressable/Focusable/Hoverable, plus the `Expandable` unit (new, `interact/expand.rs`:
     owns `expanded`, claims Enter/Space, AccessKit `expanded` + Expand/Collapse). The `twistie`
     paint part is a chevron (R14), a path until ICR1. Prop `open` (bind), signal `changed`.
   - frame: `elements/panes.rs:58`/`:158` (`W::Frame { label, style }`, `frame_of` maps
     `frame-style` to egui Frames). The own frame paints a box from a UA table per `frame-style`
     (`group` = 1 px `border`, radius 4), the `caption` paint part, `Role::Group` named by the
     label, and draws its children with `cx.children`. The padding the wrapper's
     `elements::style` gives must move to the UA table (P9: no size constants).
   - Parity: frame `checkable` (an indicator part like the checkbox; unchecked disables the
     subtree), `accordion` (one open at a time: the sibling-write seam again), header `icon`,
     header actions (`slot="end"` children shown on `:hover`/`:focus-within`).
3. **icon**, **image**: wrappers in `leaf.rs:348` (`icon_image`) and `elements/media.rs`.
   - Own `icon` paints with `paint::paint_icon` (font path) or `paint_image` (an AS1 `source`).
     Take ICR1's `IconRegistry::resolve` if it is on master.
   - Props: `family`, `name`, `glyph`, `size`, `source`, `tint`, `frame`, `selected`,
     `font-weight`. Signals: `clicked`, `double-clicked`, `secondary-clicked`. Pressable only
     when listened (`WidgetCx::listens`, as `label`). A framed icon is a button look (copy the
     button UA).
   - Keep the `icons.rs` tests' fallback behaviour (`icon_fallback_*`: placeholder square, warnings
     through `fonts::report`).
   - Own `image` paints `paint_image` with `fit` → object-fit.

### 5.2 Parity props: declare them as extras on the own struct

OW0b ships **superset props** (coordinator, 2026-10-22; the sha comes from the coordinator).
- A `#[prop]` on a `replaces` struct that the KINDS row lacks becomes an extra prop of the
  built-in tag. Types are inferred; enums use `#[prop(enumeration = "a|b")]`.
- An extra `#[event]` becomes an extra signal.
- The old wrapper ignores extras.
- egui.json and egui.d.ts list extras regardless of the switch, so after adding one, **regenerate
  those two files** with their generators and never hand-merge them.

So **do not edit KINDS rows, `P` keys or wrappers** for parity props. Until the OW0b sha is on
your base, 7eef7362 rejects extras at startup (`Replacement::new`, "has no others"). Wait for it,
or as a stopgap add the prop to the row and regenerate.

| Kind | Extras to declare | Behaviour |
|---|---|---|
| checkbox | `indeterminate: bool` (bind), `tristate: bool`, `icon: String` | pass to `ToggleParams`; `Toggle::Toggled` carries `indeterminate`, so write both props; with `tristate`, `changed` carries `"partial"` (WIDGET-PARITY §2.2; check the signal's arg type with the coordinator); paint `mark` as the dash when indeterminate (`parts::paint_mark(.., true)`) |
| radio-button | `icon` | — |
| toggle-button | `group: String` | exclusive set through the sibling-write seam + Roving |
| progress-bar | `value`, `min`, `max` (f64), `indeterminate: bool`, `orientation` (`enumeration = "horizontal\|vertical"`), `inverted: bool`, `format: String` (`%p%`, `%v`, `%m`) | `fraction` stays; value/min/max is the alternative form; `indeterminate` = the busy sweep (alias of `animate`) + `:indeterminate`; vertical swaps the axes of `track`/`fill` |
| label | `for: String` (an anchor; a click focuses the target; AccessKit `labelled_by`), selection props (§2.4 shapes) | `selectable` = a read-only TextInput on OW4's unit (or egui's text-selection helpers until then) |
| frame | `checkable`, `checked` (bind), signal `changed` | — |
| collapsing-header | `icon: String`, `accordion` group (name) | — |

### 5.3 Rich text: RT2 (ci/rt2), not OW1a2

RT2 adds:
- run kinds `span`, `b`, `i`, `code`, `a` (abstract base `Run`, `kinds/rich.rs`);
- a `runs` slot on `label`;
- a `rich` field on `W::Label`;
- the seam `crate::rich::layout_job(ctx, style, &Rich, colors, wrap, underline) -> LayoutJob`.

The own label plugs it in at **one place**: `widgets/label.rs::Label::galley` (and `measure`).
When the node has run children, build the galley from that `LayoutJob` instead of
`text_galley`. Nothing else in the own label changes.

### 5.4 Deleting the wrappers and flipping the default (per kind, last)

1. Run the kind's existing test files under both implementations: `tier0.rs`, `widgets.rs`,
   `observe.rs`, `scene.rs`, `style.rs` (prop states), `passes.rs`, and for checkbox
   `observe.rs::widget_changes_are_announced_exactly_when_observed` and
   `mutations_allocate_nothing_without_observers`. A `#[test_matrix]`-style helper in
   `tests/common` (ARCH §6.2 step 2) is the cheap way.
2. Add the tag to `widgets::DEFAULT_OWN`.
3. Delete the wrapper:
   - `leaf.rs` arms: Label 248, Checkbox 268, ToggleButton 320, Separator 347;
   - `elements/inputs.rs`: Hyperlink, Switch, radio;
   - `elements/media.rs`: ProgressBar, Spinner;
   - the `W::` variants and their `scene/props.rs`/`elements/mod.rs` prop arms;
   - `layout.rs` size copies;
   - `style.rs:578` checkbox mirror;
   - `embed/mod.rs:1683`/`:1699` lists;
   - the `#[allow(clippy::disallowed_types)]` lines, which are the burn-down.
4. Regenerate TK1 snapshot cases for the swapped kinds (sizes change on purpose, ARCH §6.2
   step 4).
5. VIS1c: move the kind's checks to own (`check-bg`/`check-fg`, `toggle-hover`/`toggle-on-fg`,
   progress height gap 7) through PaintTrace (`node`, `part`).

Not run locally: the full workspace nextest, the full gates, conformance, and the VIS1 gate (not
on master). CI runs these.
