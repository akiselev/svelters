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

All are **deferred to slice 2**. The finding behind this: `Replacement::new` requires an own
widget's props to equal the built-in row's. So every new prop has to be added to the KINDS row,
the wrapper's `P` handling and the regenerated `egui.json`/`.d.ts` in the same change, or the own
kind and the wrapper diverge.

| Row (§) | Status |
|---|---|
| toggle-button `group` (§2.1) | deferred (needs a sibling-write seam; see radio below) |
| checkbox `indeterminate`/`tristate` (§2.2) | deferred; Toggleable implements the semantics already |
| checkbox `icon`, `::part(label)` (§2.2) | `label` part painted and traced; `icon` deferred |
| radio `icon`, label part, Roving (§2.3) | deferred with radio-button |
| progress `value/min/max`, `indeterminate`, `orientation`, `inverted`, `format` (§2.9) | deferred; `track`/`fill`/`label` parts and the busy sweep done |
| label `for`, selection props, Q4 inline rich text (§2.10) | deferred |
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

## 5. Slice 2 (handoff)

In order:
1. **radio-button**: own widget with Toggleable(radio) and Roving. It needs a **sibling-write
   seam**: exclusivity writes `checked`/`group` on sibling nodes. `WidgetCx` writes only its own
   node, so this needs a `cx.defer`-style write through `SceneHandle::defer`, or EV1. Ask the
   coordinator which.
2. **frame, collapsing-header** (containers). These need `layout.rs`'s `W::CollapsingHeader`
   padding and `Tree::collapsing` moved to the own kind (`Caps::CONTAINER`/`CUSTOM_LAYOUT`), with
   the `header` node part and `twistie` paint part, Expandable (`:expanded`), and R14.
3. **icon, image**: own widgets on `paint_icon`/`paint_image` (take ICR1's IconRegistry if it is
   on master).
4. **Parity props** (§2.3 above): add each to its KINDS row, the wrapper's `P` handling and the
   own struct together, then regenerate `metadata/semantics/egui.json` and the typings with their
   generators.
5. **Q4 inline rich text**: the compiler lowers `<b>`, `<i>`, `<code>`, `<a>` and `<span
   class>` children of `label` (and string tooltips) into one layout job of styled runs (no markup
   parser). The `link` signal carries `href`.
6. **The flip**: run each kind's existing test files under both implementations (`tier0.rs`,
   `widgets.rs`, `observe.rs`, `scene.rs`), add the tags to `DEFAULT_OWN`, delete the wrappers and
   their `layout.rs` size copies, and add VIS1c checks.

Not run locally: the full workspace nextest, the full gates, conformance, and the VIS1 gate (not
on master). CI runs these.
