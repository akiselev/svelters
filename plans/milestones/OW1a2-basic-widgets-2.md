# OW1a2: simple controls as svelters' own widgets (slice 2), and the default flip

Lane OW1a2 (thinkpad, `ci/ow1a2`), 2026-10-22. Branched from `ci/ow1a` (slice 1, c92b9412) and merged
with `ci/ow0` (OW0b, 6f013c72, which includes master 95f376ce). Design: ARCH-WIDGETS §1–§3, §6.1,
§6.2, §7.2 (OW1a row); WIDGET-PARITY §2.1–§2.3, §2.9, §2.10, §2.16, §5; DESIGN §1–§2. Slice 1's
record is `OW1a-basic-widgets.md`. The coordinator split the rest of OW1a into OW1a2 (this lane) and
OW1a3 (§6).

**Status:** done. Own widgets are the **default** for all 13 OW1a kinds. The egui wrappers stay behind
`SVELTERS_OWN_WIDGETS=none` until VIS1c, as button's does (coordinator decision).

## 1. What landed

| Kind | File | Units | Parts (paint) | Replaces |
|---|---|---|---|---|
| `radio-button` | `widgets/radio.rs` | Hoverable, Focusable, Pressable, Toggleable (radio), **Roving** | `indicator`, `dot`, `icon`, `label` | `egui::RadioButton` (`elements/inputs.rs`) |
| `frame` | `widgets/frame.rs` | Hoverable, Focusable, Pressable, Toggleable (when `checkable`) | `caption`, `indicator`, `mark` | `egui::Frame` chrome (`elements/panes.rs`) |
| `collapsing-header` | `widgets/collapsing.rs` | Hoverable, Focusable, Pressable, **Expandable** | `header`, `twistie`, `label` | self-painted wrapper + `layout.rs` `Tree::collapsing` |
| `icon` | `widgets/image.rs` | Hoverable, Focusable (framed), Pressable (framed or listened) | — | `egui::Label`/`egui::Button` glyph, AS1 `icon_image` (`leaf.rs`) |
| `image` | `widgets/image.rs` | Pressable (listened) | — | self-painted (`elements/media.rs`) |

Parity extras (OW0b superset props on the `replaces` structs; the KINDS rows and wrappers are
untouched):

| Kind | Extras | § |
|---|---|---|
| `checkbox` | `indeterminate` (bind), `tristate`, `icon`; `::part(icon)`, `::part(label)` laid out by P3 | §2.2 |
| `radio-button` | `icon`; `::part(icon)`, `::part(label)` | §2.3 |
| `toggle-button` | `group` (an exclusive set of siblings, one Tab stop, Roving) | §2.1 |
| `progress-bar` | `value`, `min`, `max` (Qt's form; `min == max` is busy), `indeterminate`, `orientation` (`horizontal`/`vertical`), `inverted`, `format` (`%p`, `%v`, `%m`, `%%`) | §2.9 |
| `label` | `for` (Qt's buddy: a click focuses the anchored target, AccessKit `labelled_by`), `elide` (`none`/`start`/`middle`/`end`; also `wrap-mode="truncate"` and CSS `text-overflow: ellipsis`) | §2.10, WG6 QW13 |
| `frame` | `checkable`, `checked` (bind, default true), signal `changed` | §2.16 |

New pieces:
- **`interact/roving.rs`: `Roving`.** While the host is focused, Up/Left go to the previous member and
  Down/Right to the next (wrapping), Home/End to the ends. It claims those keys. The widget picks the
  members, moves focus and checks. A group is one Tab stop: only the checked member, or the first,
  keeps `Sense::FOCUSABLE`. Unit test: `ow1a2_rove_wraps`.
- **`interact/expand.rs`: `Expandable`.** It owns `:expanded`. Right/Left and AccessKit `Expand`/
  `Collapse` open and close the host, and an activation toggles it (`on_click`). It sets AccessKit
  `expanded` and the actions.
- **`WidgetCx::with_peer::<Wd>(id, |w, peer| …)` and `PeerCx` (`set`, `emit`, `focus`)**, plus
  `sibling_ids`, `tag_of` and `focus_node`. These are the sibling-write seam, in the same pass:
  - radio exclusivity, toggle-button `group` and Roving use it;
  - the peer's writes and events are flushed exactly like its own (`flush_writes`);
  - a repaint is requested for peers drawn earlier in the pass.
  `SceneHandle::defer` was the first plan, but it cannot emit the siblings' `changed` events, which
  `tier0.rs::radio_buttons_bind_group_and_checked` requires. Lead informed.
- **`provider::Args(Vec<Value>)`.** It is the payload of a several-arg built-in signal, such as
  radio's `changed(checked, group)`. The emit path spreads it into the signal's args.
- **`paint::{Elide, elide, elide_with}`.** One line is cut with `…` at the start, middle or end, by a
  binary search over measured cuts (ported from WG6's `tips::elide_with`). Unit test:
  `ow1a2_elide_modes`. Slice 1's `LayoutJob` truncation is gone.
- **`LayoutCx::{padded_layout, collapsed, with_measure}`.** An own container lays out its children
  inside chrome it paints: the frame's border, UA padding and caption row, and the collapsing
  header's row and indent. When closed, the collapsing header is a leaf with hidden children.
  `layout.rs::provider_caps` now reads an own widget's caps, so `CUSTOM_LAYOUT` reaches replacement
  kinds.
- **The indicator kinds share `checkbox::{with_parts, measure_with_indicator,
  paint_indicator_kind}`.** These are checkbox, radio-button and switch. Their parts are P3 items
  (`indicator` or `track`, `icon`, `label`) in the host's content box, so `flex-direction`, `order`
  and `display: none` on the parts work. `parts::paint_chevron` is the R14 twistie (a path until
  ICR1; `content: icon(..)` on `::part(twistie)` replaces it, P2).

## 2. Acceptance

### 2.1 DESIGN R-rules (affordances on by default, in the UA tables)

| Rule | Status |
|---|---|
| R14 chevrons | **done**: the collapsing header's `twistie` (right, or down when open), DESIGN R14 |
| R9 disabled with a reason | **done** for the new kinds: disabled colours and opacity .4 in the UA tables (framed icon: button's R9 row; unframed icon and image: opacity .4); still hoverable; the tooltip on a disabled own widget is OW0's (`on_disabled_hover_text`); `cursor: not-allowed` on the framed and unframed icon |
| R8 focus visible, keyboard only | radio (ring around the indicator), checkable frame (around its indicator), collapsing header (1px at −1 inside the header row, rows' form), framed icon (button's 2px at +1) |
| R10 cursors | collapsing header row and clickable icons: `pointer`; disabled icon: `not-allowed` |
| R16 sections | `frame` (`group`): 1px `border`, no shadow; floating styles (`window`, `menu`, `popup`) take `elev-2` (R7) |
| R17 state by fill | radio: dot plus `toggle-on-bg`/`toggle-on-border` |
| R1–R3 | framed icon: the button's UA table |

### 2.2 State bits (DOC1's gaps)

| Bit | Status |
|---|---|
| `:checked` on radio, checkable frame | **done** (Toggleable's prop mirror). Tested |
| `:indeterminate` | **done** on checkbox (`indeterminate`) and progress (`indeterminate`, `animate`, `min == max`). Tested |
| `:expanded` | **done** on collapsing-header (Expandable). Tested |
| `:busy` | progress also for `indeterminate` and Qt's busy form. Tested |
| `:read-only`, `:invalid` | none of these kinds has a read-only or invalid state of its own: `invalid` is Common (the walk's outline). Selectable label (read-only TextInput) is OW1a3 |

### 2.3 WIDGET-PARITY OW1a rows (§5)

| Row (§) | Status |
|---|---|
| toggle-button `group` (§2.1) | **done**: `ow1a2_toggle_button_group_is_exclusive` |
| checkbox `indeterminate`/`tristate` (§2.2) | **done**: `ow1a2_checkbox_indeterminate_and_tristate`. `changed` keeps its bool payload; `indeterminate` is announced as a bound prop (the `"partial"` payload would change the row's signal type: not done, recommended against) |
| checkbox `icon`, `::part(label)` (§2.2) | **done** (P3 parts) |
| radio `icon`, label part, Roving (§2.3) | **done**: `ow1a2_radio_roving_moves_focus_and_the_check_and_is_one_tab_stop` |
| progress `value/min/max`, `indeterminate`, `orientation`, `inverted`, `format`, `::part(label)` (§2.9) | **done**: `ow1a2_progress_value_range_orientation_and_busy` |
| label `for` (§2.10) | **done**: `ow1a2_label_for_focuses_its_buddy_and_elide_keeps_the_full_name` |
| label selection props (§2.10) | OW1a3 |
| frame `checkable` (§2.16) | **done**: `ow1a2_checkable_frame_disables_its_content_when_unchecked`. Unchecked disables the content through a disabled egui `Ui` (nothing in it takes input); `:disabled` is not inherited by the content's CSS yet |
| accordion, header `icon`, header actions (§2.16) | OW1a3 |
| inline rich text (Q4, §2.10) | RT2. The own label's one hook is `Label::galley` (`widgets/label.rs`); ci/rt2 had not pushed, so the seam is OW1a3's handoff (§6) |

### 2.4 VIS1

The VIS1 gate (`ci/vis1`) is not on this branch's base and VIS1c has not landed: **no VIS1 check
ran**. The new kinds trace every part for VIS1c's `(node, part)` lookup:
- radio: `indicator`, `dot`, `icon`, `label`;
- frame: `caption`, `indicator`, `mark`;
- collapsing-header: `header`, `twistie`, `label`;
- icon: `Icon` and `Image` entries;
- image: `Image` entries.

VIS1 gap 2 (pane header) is the collapsing header's `header` part (`header-h` 22, `list-hover`). VIS1c
moves the check, toggle, link and progress (gap 7) checks to own.

### 2.5 Pins

Pin 7 (focus-click/focus-tab) holds for radio, the checkable frame, the collapsing header and the
framed icon: a click focuses without a ring, Tab shows one (Focusable, FX1). Roving focus shows the
ring (keyboard). Tested in kittest, not against the goldens.

## 3. Tests

`crates/svelters-egui/tests/ow1a2.rs`, on the new default:
- `ow1a2_the_default_is_own_for_every_ow1a_kind_and_none_keeps_the_wrappers`
- `ow1a2_radio_roving_moves_focus_and_the_check_and_is_one_tab_stop`
- `ow1a2_checkbox_indeterminate_and_tristate`
- `ow1a2_progress_value_range_orientation_and_busy`
- `ow1a2_label_for_focuses_its_buddy_and_elide_keeps_the_full_name`
- `ow1a2_collapsing_header_expands_by_keyboard_and_mirrors_expanded`
- `ow1a2_checkable_frame_disables_its_content_when_unchecked`
- `ow1a2_toggle_button_group_is_exclusive`
- `ow1a2_icon_and_image_name_and_click`

Plus the unit tests `interact::roving::tests::ow1a2_rove_wraps` and
`paint::text::tests::ow1a2_elide_modes`.

Existing tests of these kinds now run on the own widgets (the default): `tier0.rs`, `widgets.rs`,
`ow1a.rs` (both implementations), `own_widgets.rs`. Where a test pinned the egui wrapper's metrics,
it now asserts the own geometry (§4).

## 4. Deviations and notes

- **The wrappers are not deleted.** They stay behind `none` until VIS1c. Their lint lines are
  re-tagged `// OW: wrapper until VIS1c` (`leaf.rs` ×2, `elements/inputs.rs` ×2,
  `elements/media.rs`). The OW1a tags on the in-crate egui uses in `views/table.rs` (×2) and
  `views/toast.rs` are now OW1a3. The one in `menus.rs` is now OW3.
- **The header is a paint part, not a node part.** Header actions (`slot="end"`) need the node part
  (SG4 `insert_shadow`), and both are OW1a3's.
- **Frame sizes.** The UA's border and padding are 1 + 6 = 7, egui's `Frame::group` total margin. The
  caption row is the caption's own height plus `gap`, not egui's `interact_size.y`. A CSS `padding`
  adds to the UA's, as the row's doc says of the egui inner margin.
- **Glyphs are paths** (check, dash, dot, chevron) until ICR1. `content: icon(..)` on the part
  replaces them (P2).
- **Part styles are UA tables** until OW-CSS computes `::part()` (`TODO(OW-CSS ::part)`).

## 5. Pins and what was not run

Not run locally: the full workspace nextest, the full gates, conformance and the VIS1 gate (not on
the base). CI runs these.

## 6. OW1a3 handoff (coordinator split, 2026-10-22)

1. **`use:` action exports** of the units (Hoverable, Pressable, Focusable, Toggleable, Roving,
   Expandable): `UseUnit<U>` is OW0's adapter shape.
2. **collapsing-header:**
   - `accordion` (one open at a time: use `WidgetCx::with_peer`, as radio does);
   - header actions (`slot="end"` children shown on `:hover`/`:focus-within`), which need `header`
     as a node part (`#[part(node)]`);
   - `icon`.
3. **label selection** (`selectable` as a read-only TextInput, the §2.4 selection props) and
   `:read-only`.
4. **The in-crate egui uses** tagged OW1a3: `views/table.rs` 415 and 492 (cell checkbox and
   label), and `views/toast.rs` 185.
5. **RT2's seam**, if ci/rt2 has landed. When the label node has run children, build the galley in
   `widgets/label.rs::Label::galley` (and `measure`) from `crate::rich::layout_job(...)`. Match
   `W::Label { .. }` with `..`.
6. **VIS1c** deletes the wrappers listed in §4 and their `layout.rs`/`elements` size copies, and
   moves the checks to PaintTrace.
