# OW-CSS: the CSS engine side of svelters' own widgets (DS2, re-scoped)

Design: `plans/research/ARCH-WIDGETS.md` (c5c34599, final) §0 W3/W4, §1.1 (paint primitives and the
CSS they read), §2.2 (state bits), §3.4 (transitions), §7.2 row OW-CSS; `plans/research/CSS.md`.
Lane: OW-CSS on thinkpad, branch `ci/ow-css`. Paint wiring is OW0's (`ci/ow0`): this lane did not
touch svelters-egui paint code.

**Decided (coordinator, 2026-10-22):** the nine new state bits go into both bit tables
(`svelters-style/src/state.rs` and `svelters-egui/src/provider/state.rs`), with the same names,
order and bit numbers as §2.2. The pin test `style.rs::state_bit_tables_are_pinned_together`
requires both. OW0a's identical rows merge cleanly.

## Stage 1: engine (this push)

| Area | What landed |
|---|---|
| State bits 14–22 | `StateMask::{FOCUS_VISIBLE, INDETERMINATE, PLACEHOLDER_SHOWN, DEFAULT, DROP_TARGET, CURRENT, BACKDROP, REQUIRED, USER_INVALID}` and the `BUILTIN` rows, in both tables. Bits 23–31 stay free. |
| Pseudo-classes | `:indeterminate`, `:placeholder-shown`, `:default`, `:drop-target`, `:current`, `:backdrop`, `:required`, `:optional` (= `:not(:required)`) and `:user-invalid` are rows of `STATE_PSEUDO_CLASSES`. `:focus-visible` stays `Element::is_focus_visible`. Its default now reads `FOCUS_VISIBLE`, and falls back to `FOCUSED` for trees that never set the bit. Invalidation counts both bits. **OW0:** once Focusable sets the bit from EV3's focus reason, override `is_focus_visible` on the scene's `NodeRef` to read the bit alone. |
| Pseudo-elements | `PseudoElement::{Part(Atom), Placeholder, Selection}`, written `::part(name)`, `::placeholder` and `::selection`. A state pseudo-class goes on the host (`slider:hover::part(thumb)`). Nothing may follow the pseudo-element, and `::before` is still rejected. Selectors ending in a pseudo-element go into their own rule bucket, so an element never matches them itself. Invalidation treats the host compound as the subject, so a state change that a part rule reads reaches the host (`Reach::SELF`). |
| Shorthands (`shorthand.rs`) | `outline`, `border`, `border-{top,right,bottom,left}`, `border-{width,style,color}` (1–4 values) and `border-radius` (1–4 values; `/` is rejected). They expand after `var()` substitution, inside the cascade's winner pass, so a shorthand and its longhands are ordered by normal cascade order. `border-width`, `border-color` and `border-radius` stay uniform properties too, because CS5g's paint reads them. One value sets both the uniform property and every side; 2–4 values leave the uniform property unset. A shorthand expands only on tables where the engine computes the property (egui), never where GTK's own CSS engine takes `border` whole. `check_decl` validates a shorthand by expanding it, which gives `style_bad_value` with no new codes. |
| New egui properties (`ENGINE_PROPS`) | `border-{side}-{width,style,color}`, `border-{corner}-radius`, `outline-{width,style,color,offset}`, `cursor` (36 CSS keywords), `letter-spacing` (length or `normal`), `text-transform`, `text-overflow` (`clip`/`ellipsis`), `transition`, `background-image`. The width keywords `thin`/`medium`/`thick` give 1/3/5 px. `cursor`, `letter-spacing` and `text-transform` inherit. |
| Values | `ValueType::Transition` gives `ComputedValue::Transitions([Transition { property, duration, delay, timing }])`. It parses `none` or a list, with entries in any order. Times are in `s`/`ms`. Timing is `linear`, `ease*`, `step-start`/`step-end`, `cubic-bezier()` or `steps(n, jump-start/end)`. `TimingFunction::apply` solves the bezier. `ValueType::Image` gives `ComputedValue::Image(Option<LinearGradient>)` from `none` or `linear-gradient([angle | to side/corner,] stops…)`. Stops get resolved positions in 0–1; corners assume a square box. The box primitive paints 2 stops; more are kept. |
| Text primitive inputs | `font-family` and `line-height` (raw text, for the primitive), `font-style`, `font-weight`, `text-align`, `white-space`, `text-decoration-line` (R18). |
| Transitions store (`transition.rs`) | `Transitions<K>` (K = node, or `(node, part)`), described below. |
| Generated | `metadata/semantics/egui.json` (`SVELTERS_UPDATE=1 cargo test -p svelters-egui --test spec`), `tables.generated.rs` and `props.generated.js` (`gen_props.py`; `RS_TYPE` gains `transition` and `image`). `egui.d.ts` did not change. |

### The API OW0 reads

```rust
// computing (the walk, per node and per part)
let host: ComputedStyle = stylist.compute(node, parent, inline, &cx, &mut caches);
if stylist.has_pseudo_rules(&PseudoElement::part("thumb")) {
    let thumb = stylist.compute_pseudo(node, &PseudoElement::part("thumb"), &host, &cx, &mut caches);
}   // ::placeholder / ::selection: PseudoElement::Placeholder / ::Selection, the same call

// transitions: one store on the scene, keyed by node or (node, part)
transitions.restyled(key, &old_style, &new_style, now_secs, reduced_motion) -> bool;  // after each restyle
let view: StyleView = transitions.view(key, &style, now_secs);   // or StyleView::new(&style)
if transitions.tick(now_secs) { /* request a frame for running_keys()' roots (TR2) */ }
transitions.remove(key);   // node dropped
transitions.clear();       // reduced motion switched on

// painting: StyleView only
view.background() -> Option<Color>;   view.background_image() -> Option<&LinearGradient>
view.border() -> [BorderSide; 4];     view.border_side(Side::Left);  view.border_is_uniform()
view.radii() -> [f32; 4] /* tl tr br bl */;  view.shadows() -> &[Shadow];  view.opacity()
view.outline() -> Outline { width, style, color, offset }
view.text() -> TextSettings { transform, letter_spacing, overflow };  text.transform(&str) -> Cow<str>
view.cursor() -> Option<&'static str>  // CSS keyword; None = auto/unset
view.get(name) / color(name) / px(name) / keyword(name) / number(name)  // anything else
```

Resolution rules in `StyleView`:
- A side reads its longhand first, then the uniform `border-width`/`border-color`.
- An unset `border-*-style` counts as `solid`. This deviates from CSS on purpose: CS5g sheets set only width and colour. `none`/`hidden` give width 0.
- An unset `outline-style` is `none`, which gives width 0. `auto` should paint as solid.
- A colour of `None` means currentcolor: use the text colour.
- `cursor` keywords map 1:1 onto `egui::CursorIcon`; `pointer` is `PointingHand`.

The store's rules:
- After a restyle, every changed property that a `transition` entry names starts a transition, if the entry's duration is above 0. An entry matches by name, by `all`, or by a shorthand that covers the property. The last matching entry wins.
- A transition starts from the value painted at that moment: the running transition's sample, else the old computed value. This gives a smooth reversal on un-hover.
- Retargeting to the same end value doesn't restart a transition.
- With `instant = true` (reduced motion), nothing starts and the key's running transitions end.
- DESIGN R2 is plain CSS: put `transition: none` on `:active`.
- Interpolated types: colours (premultiplied sRGB), lengths, numbers, insets, font weights, shadow lists (the shorter list is padded with transparent zero shadows; `inset` must match), and gradients with equal stop counts. Keywords switch at once.

### Tests

`crates/svelters-style/tests/ow_css.rs` (13 tests) and `shorthand.rs` unit tests (2):
- every new pseudo-class, and the bit order;
- which pseudo-element selectors parse and which are rejected;
- part, placeholder and selection styles: host state, inheritance from the host, specificity, other parts unmatched, `has_pseudo_rules`, host invalidation;
- side, outline and radius resolution, and shorthand/longhand cascade order;
- shorthand and value diagnostics;
- text settings and cursor inheritance;
- gradients;
- transition parsing and easing;
- the store: halfway sample, reversal from the painted value, end, `transition: none` on `:active`, reduced motion, zero duration with a delay (R11);
- the text primitive's properties;
- multi-name parts and `:has-children`;
- glyph `content` and `paint()` painters.

The whole `svelters-style` suite passes (55/55, plus the ignored measurement), as do the egui pin test and the spec test.

### Coverage: DESIGN R-rules, VIS1 gaps, parity, pins

The engine supports the CSS behind every R-rule. The behaviour and paint are the widget lanes'.

| Rule | CSS it needs | Engine status |
|---|---|---|
| R1 raised buttons | `background-color`, a ≥3:1 `border`, `border-radius: 4px`, an `elev-1` lip as a `box-shadow` token through `var()` | ✓ (`box-shadow` from CS5g; shorthands here) |
| R2 pressed sinks | `:active`, an `inset` shadow, `transition: none` | ✓ (tested) |
| R3 hover, 100 ms ease | `:hover`, `transition` under `@media (prefers-reduced-motion: no-preference)` | ✓ (media feature from CS0; the store takes `instant`) |
| R4 sunken fields | an inset `box-shadow`, `cursor: text`, `:focus`/`:focus-visible` border, `:invalid`/`:user-invalid` | ✓ |
| R5 read-only | `:read-only`, `cursor: default` | ✓ |
| R6 four row states | `:hover`, `:selected`, inactive selection (`list:not(:focus-within) row:selected`), the focus outline (`outline: 1px solid; outline-offset: -1px`), `:current` | ✓ |
| R7 floating surfaces | `elev-3`/`elev-4` shadows, a 1px border, `@media (prefers-contrast: more)` for HC | ✓ |
| R8 focus rings | `:focus-visible` (bit 14), `outline-width/style/color/offset` (−1 rows and fields, +1 controls) | ✓. OW0 must override `is_focus_visible` to read the bit alone |
| R9 disabled with reason | `:disabled`, `opacity: .4`, `cursor: not-allowed` | ✓ (the tooltip is behaviour) |
| R10 cursors | `cursor` (36 keywords) | ✓ (applying it on hover is OW0b) |
| R11 sashes | `cursor: col-resize/row-resize` at once; the bar after 300 ms via `transition: … 0s 300ms` | ✓ (zero-duration-with-delay, tested) |
| R12 drag | `:drop-target` (bit 18), `opacity`, `elev-3`; the insertion line is paint | ✓ |
| R13 tabs | `border-top: 2px solid` marker (per side), `:hover` `elev-2` | ✓ |
| R14 chevrons | icons (ICR1/OW0b) | n/a |
| R15 scroll edge shadows | `box-shadow: inset 0 6px 6px -6px var(--sv-scroll-shadow)`; the scrollbar fade is an `opacity` transition. "When scrolled" needs a state | engine ✓. **Recommended:** OW2's Scrollable declares provider states `sv.overflow-{top,bottom,left,right}` (matched as `:state(sv.overflow-top)`), or paints `::part(shadow-top)` |
| R16 cards | `border`, background tone | ✓ |
| R17 toggles | `:checked`, `:indeterminate` (bit 15), `::part(mark)`/`::part(knob)` | ✓ |
| R18 links | `text-decoration-line: underline` on `:hover` | ✓ (added) |

VIS1 gaps (`ci/vis1` milestone §8) that the engine side unblocks:
- gap 1 (focus rings): `outline` + `:focus-visible`;
- gap 2 (pane header uppercase, 11px bold): `text-transform`, `font-weight`;
- gap 6 (tab close shown only on active or hover): `::part(close)` under `tab:hover`/`:selected`.

The `oc` checks pass once OW0's outline primitive paints. Gaps 3–5 and 7–9 are theme or paint work.

WIDGET-PARITY.md had not landed on master by the time of this record. QSS sub-controls map onto `::part(name)` and QSS pseudo-states onto the state pseudo-classes; the per-widget rows belong to each OW lane. No native-behaviour pin is engine work.

Not in this lane: `transform` transitions (ARCH §3.4 lists translate/scale, but no R-rule needs them and the egui table has no `transform`); elliptical radii (`/`); `transition-*` longhands (only the `transition` shorthand).

### WIDGET-PARITY P1–P10 and `paint()` (added scope, coordinator 2026-10-22)

| Rule | Engine status |
|---|---|
| P1 logical position of repeated parts | **open.** Two pieces: (a) `Element` gains `fn logical_index(&self) -> Option<(usize, usize)>`, which the widget sets with `cx.set_logical_index`; (b) structural pseudo-classes after `::part()`. `selectors` 0.41 matches `:nth-child` and friends itself, walking siblings, and its parser rejects anything but non-TS pseudo-classes after a pseudo-element. Plan: our own `PseudoClass::Logical { a, b, of_type: false, last }` variants. In the part context the parser maps `:first-child`/`:last-child`/`:only-child`/`:nth-child()`/`:nth-last-child()` to them, by giving the part pseudo-element `accepts_state_pseudo_classes() = true` and parsing those names in `parse_non_ts_pseudo_class`. They match through `logical_index`. Pooled rows are light nodes, so `Node::prev_sibling_element` can't express logical order; they need the same `Logical` variants for `:nth-child` on the row (selected when `logical_index` is `Some`). ~0.5 day. |
| P2 glyphs from CSS | ✓ `content: none \| icon(key) \| url(…) \| paint(name)` → `ComputedValue::Content(Content)`, read with `StyleView::content()`. Keys and painter names may be dotted (`cad.extrude`). The base-sheet defaults (`checkbox:checked::part(mark) { content: icon(check) }`) are OW1a's. |
| P3 part layout from CSS | OW0b (the layout side). The engine already computes `order`-free flex props for parts through `compute_pseudo`. `order` is not in the egui table yet: OW0b adds it with the part layout. |
| P4 gradients | partial: `linear-gradient` with any number of stops and angles ✓. `radial-gradient` **open** (~0.25 day: `ComputedValue::Image` becomes an enum `{ Linear, Radial }`). `border-image` is OW-P2. |
| P5 bit 23 `has-children` | ✓ in both bit tables, `:has-children`. Free bits are 24–31. |
| P6 per-item style host | **open** (~0.25 day). `compute_pseudo` already takes the element to match against. A node part standing for a child element is computed with `compute_pseudo(child_element, &PseudoElement::part("header"), …)`, so rules like `tab.dirty::part(header)` match the child's classes, attributes and states. What's missing is the widget-side indirection, which OW5c (the first user) wires. Engine change: none beyond documenting this. |
| P7–P9 | widget lanes (props, signals, metric tokens). |
| P10 multi-name parts | ✓ `::part(day today)` needs every name. `PseudoElement::parts(&["day", "today", "weekend"])` is the element side, and `matched_by` does subset matching. |
| Q1 part painters | ✓ `background-image: paint(cad.knurl)` → `ComputedValue::Painter(name)`, read with `StyleView::background_painter()`. `content: paint(name)` covers glyph parts. The painter registry is OW0b's. |

### CSS parse cost (coordinator request)

Measured on the gallery-egui sheets in a release build, median of 200 runs, through `Stylesheet::parse` + `Stylist::add_sheet` (what `install_sheet` does):

| Set | Sheets | Bytes | Parse | Parse + add_sheet |
|---|---|---|---|---|
| Startup (theme token sheets for both schemes, `base.css`, 3 component sheets) | 6 | 16,048 | 200 µs | 227 µs |
| Theme only | 3 | 15,690 | 189 µs | 205 µs |
| One HMR swap (the component sheets) | 3 | 358 | 9 µs | 17 µs |

**Decided (coordinator, 2026-10-22): the parsed-sheet handoff, the content-hash cache and the Q4 pre-parsed form are dropped.** Startup is ~0.23 ms, under the 1 ms threshold. The handoff was not built because:
- the live loader doesn't apply `<style>` yet (`svelters-modules/src/source/transform.rs` warns; Q3w's `scoped_css` is only wired into Vite);
- the saving would be at most ~17 µs per swap;
- a compile-time parse resolves `:state()` with an any-name resolver, so it would carry wrong provider bits.

LD2 wires `scoped_css` into the live loader and sends text, as Vite does.

To reproduce:
1. `OWCSS_SHEETS=<dir> cargo test -p svelters-egui --test css_parse_cost -- --ignored` dumps the theme sheets into `<dir>`.
2. Put each component's `<style>` body in `<dir>` as well.
3. `OWCSS_SHEETS=<dir> cargo test --release -p svelters-style --test parse_cost -- --ignored --nocapture` prints the numbers.

## Handoff (context limit reached)

State: `ci/ow-css` pushed with the engine stage and P2/P5/P10/`paint()`. All svelters-style tests pass (55), as do the egui pin and spec tests. The JS `packages/style` golden is refreshed. CI: cd9a6bc3 4/5 green (linux-gtk caught the `border-width` uniform regression, fixed in 9f8a2545).

**Stage 2 starts from `ci/ow0` 6f013c72 (OW0b).** What OW0b gives and expects:
- **Part styles** go through OW0b's hooks `SceneStyle::part_computed(id, part)` and `part_styles(id)`. They return `None` until OW-CSS fills them: compute each part with `Stylist::compute_pseudo(node, &PseudoElement::part(name), &host, …)` (multi-name parts: `PseudoElement::parts`), and store it next to `NodeStyle::computed`, restyled whenever the host restyles.
- **Keep `Element::style_as`**, the defaulted method OW0b added to svelters-style's `Element`, when merging.
- **`paint(name)` mismatch.** OW0b's `paint::painters::selected` reads `background-image` with `StyleView::keyword` and parses the text `paint(name)`. In OW-CSS the value is typed (`ComputedValue::Painter(name)`, and `ComputedValue::Content(Content::Paint(name))` for `content`), so `keyword()` returns `None`. On merge, make `selected` match the typed values (or teach `paint::StyleView::keyword` to return `Painter`'s name), and keep OW-CSS's typed `background-image`/`content` rows in `ENGINE_PROPS`.
- **Breaking:** `paint::StyleView::shadows()` returns `Option<Cow<[Shadow]>>`.
- Per-side borders and gradients are OW-CSS's to feed. `paint_box` reads them through `paint::StyleView`; the typed getters (`border_side`, `radii`, `background_image`) are on `svelters_style::StyleView`.

Next, in order:
1. **Wire to OW0 (`ci/ow0` 6f013c72).** OW0a's `paint::StyleView` holds `css: Option<&'a ComputedStyle>` with a UA fallback and is `Copy`. Make that field `Option<&'a svelters_style::StyleView<'a>>`, so the view stays `Copy`, and change its private `css(name)` to `self.css.and_then(|v| v.get(name))`. Build the `svelters_style::StyleView` per node at paint with `scene.transitions.view(key, &computed, ctx.input(|i| i.time))`. Then:
   - put `Transitions<NodeId>` (parts: `Transitions<(NodeId, u16)>`) on `SceneStyle`;
   - call `restyled(id, &old, &new, now, reduced_motion)` where the walk swaps a node's `computed` (`style.rs` `NodeStyle::computed`);
   - call `tick(now)` once per frame, and request a repaint of the running keys' roots (TR2 `running_roots`);
   - call `remove` when a node is dropped.

   The coordinator says OW0b is active on `ci/ow0`, so merge it (rather than read it) and keep these hunks small.
2. P1 logical indices and P4 `radial-gradient` (plans above).
3. A part-styled button variant in the gallery, and the kittest transition test on the virtual clock (OW-CSS acceptance in ARCH-WIDGETS §7.2).
4. Override `is_focus_visible` on the scene's `NodeRef` to read `FOCUS_VISIBLE` alone, once OW0's Focusable sets it.

## Stage 2: the engine behind OW0's paint layer (branch `ci/ow-css2`, from `ci/ow0` 6f013c72)

Lane: OW-CSS stage 2 on thinkpad. This stage merged `ci/ow-css` (stage 1) into OW0b, so until OW0
merges into master, the branch carries OW0's commits too.

| Area | What landed |
|---|---|
| Part styles | `SceneStyle::part_computed` / `part_styles` (OW0b's hooks) are filled. `restyle_node` computes each paint part of an own widget (`KindEntry::parts`, not `#[part(node)]`) with `Stylist::compute_pseudo(node, &PseudoElement::part(name), &host, …)`, but only for parts that some sheet has a rule for (`has_pseudo_rules`). It stores them next to `NodeStyle::computed`, keeping the host's restyle (a state change that only a part rule reads still restyles). The part list is cached on `NodeStyle`, because the payload is taken out while the widget paints. A part change that `measure` reads (box and text metrics, `content`) relayouts the host. Other part changes repaint only. `WidgetCx::part(&host, name, ua)` gives a part's view with its transitions, and the own button uses it. |
| Transitions | `Transitions<NodeId>` and `Transitions<(NodeId, u16)>` (part = declaration index) live on `SceneStyle`. A restyle that swaps a node's or part's computed style calls `restyled` at `SceneStyle::now`, which is egui's input time from the logic step (kittest's virtual clock in tests). What starts is sampled at once, so this pass paints the start value. The logic step (`frame.rs` `transitions_step`) drops the ended transitions and samples the running ones into a per-key `Overlay`. `transitions_demand` requests frames for the logic viewport and the viewports of the running nodes' roots (as SV37 does for TR2). A freed node forgets its transitions (`tree.rs` `free_slot`). |
| `StyleView` | `paint::StyleView` stays `Copy`: it holds `css: Option<&ComputedStyle>` and `over: &[(usize, ComputedValue)]` (the frame's samples, from `with_overlay`), and its private `css(name)` reads through `svelters_style::overlay_get`. `engine()` returns the engine's typed `svelters_style::StyleView` (borrowed overlay) for per-side borders, radii, gradients, painters and `content`. Engine side: `StyleView::with_overlay`, `overlay()`, `Transitions::sample`, `overlay_get[_index]`, `type Overlay`. `Element::style_as` is kept. `shadows()` already returned `Option<Cow<[Shadow]>>` in OW0b. |
| SG6 paint cache | `WidgetCx::cached_shapes` is keyed only by the widget's version, and a restyle was only PAINT dirt, so restyled or animating nodes would have replayed stale shapes. Now a restyle that changes a node's style (or its parts' styles) drops its cache entry, and so does every logic step for each node whose transitions ran in that frame, including the frame they end on (`Scene::step_style_transitions`). Test: `cached_shapes_follow_restyles_and_running_transitions`. |
| Reduced motion | `EguiBackend::set_motion(MotionConfig { reduced: true, .. })` now also sets the media env's `prefers-reduced-motion`. The next logic step ends every running CSS transition, and restyles start none. |
| Painter seam (bug fixed) | `painters::selected` matches the typed values `ComputedValue::Painter(name)` (`background-image`) and `Content::Paint(name)` (`content`). A UA table's keyword text `paint(name)` still works. `paint::content_icon` had the same bug: it read `content` as keyword text, so `content: icon(close)` / `none` from CSS were ignored. It now matches `Content::{None, Icon, Url}`. |
| Box primitive | Per-corner radii (`StyleView::radii`: each `border-{corner}-radius`, else `border-radius`). Per-side borders (`border_sides`): one `RectShape` stroke when uniform (what VIS1 measures), otherwise one quad per side, with mitred square joins that don't follow the radius. Gradients: a vertex-coloured mesh over the background colour, built from the rounded-rect outline (edges split every 6 pt) fanned in 8 rings from the centre. It is exact for 2-stop linear gradients; more stops and radial gradients are sampled at the vertices. The border goes on top of a gradient or painter. |
| P1 logical indices | After `::part()`, `:first-child`, `:last-child`, `:only-child`, `:nth-child(An+B)` and `:nth-last-child(An+B)` match the part's logical position, which the widget passes to `Stylist::compute_pseudo_at(el, pseudo, Some((index, count)), host, …)`. `None` (and `compute_pseudo`) never match them. The plan's mechanism didn't work as written, because `selectors` 0.41 (a) matches nothing after a pseudo-element in `ForStatelessPseudoElement` mode and (b) refuses `:nth-child()` after one. Instead: `::part()` is element-backed and accepts state pseudo-classes. `first/last/only-child` after it reach our `parse_non_ts_pseudo_class` as `PseudoClass::Logical`. `:nth[-last]-child(` after `::part(` is renamed to `:-sv-nth[-last]-child(` before parsing (`selector::rename_part_nth`, also on the nesting reparse), and serializes back to the standard names. Pseudo rules now match in `Normal` mode from the host compound's offset. What follows the pseudo-element is tested in `cascade::pseudo_offset`: logical classes against the position, state classes against the host, so `::part(thumb):hover` = `:hover::part(thumb)` (CSS Shadow Parts allows that). `of S` after a part is a diagnostic, and a second pseudo-element is still rejected (`selector::check_list`). Logical classes add no invalidation dependency: the widget that sets the index restyles. **Not done:** the light-node half (pooled rows' `:nth-child` by logical row). `selectors` matches `:nth-child` itself by walking siblings, with no hook. The way open is for OW5a's pooled list to style its rows as `list::part(row)` paint or node parts, and pass the row index through `compute_pseudo_at`. |
| P4 `radial-gradient` | `ComputedValue::Image(Option<Box<Gradient>>)`, `Gradient::{Linear(LinearGradient), Radial(RadialGradient)}`. Radial parses `[circle \| ellipse] \|\| <extent>` (`closest-side`, `closest-corner`, `farthest-side`, `farthest-corner`; default `ellipse farthest-corner`) and `at <position>` (keywords and percentages, 1–2 values). Explicit sizes are a diagnostic. `Gradient::t_at(size, point)` and `color_at(t)` (premultiplied) are the paint math, tested in the engine. Transitions interpolate radial↔radial with the same shape and extent (centre and stops), as well as linear↔linear. `StyleView::background_image()` stays linear-only; `gradient()` returns either. |
| Gallery | `fixtures/gallery-egui/src/pages/23-own-buttons.svelte`: own buttons styled from a page `<style>`: `.pill::part(label)` (uppercase, letter-spacing, bold), `.accent` (linear gradient, eased hover, instant inset `:active`, a tinted `::part(icon)`), `.tab` (R13 top marker as a per-side border, top-corner radii), `.glow` (radial gradient, underline on hover). `tests/gallery.rs` `own_buttons_page_styles_parts_from_css`. |

### Tests (stage 2)

- `crates/svelters-style/tests/ow_css.rs`: `logical_positions_after_parts` (first, last, only, `nth-child(even)`, `nth-last-child(2)`, no position, host state after a part, nested `&::part():nth-child()`, serialization, `Logical::matches`), `radial_gradients` (parse, extents, `t_at`/`color_at`, diagnostics), `radial_gradient_transitions_and_borrowed_overlays`. The parse/reject test now accepts `a::part(x):hover` and rejects `:nth-child(… of S)`, `.y` and `:first-child` after other pseudo-elements. The whole suite passes: 58/58.
- `crates/svelters-egui/tests/ow_css.rs` (new; kittest, `step_dt` 0.25 s): a registered painter runs when CSS selects it (`background-image: paint()` on the host, `content: paint()` on `::part(icon)`); part styles reach the paint and follow host state, and `content: icon()`/`none` from CSS work; a gradient mesh, a per-side top border and radii; a transition sampled over virtual time (+50 per step on a 1 s linear fade, then the end value; `:active` instant; the ease back after release); reduced motion ends and skips transitions; the R-rules' CSS (R8, R10, R11).

### Coverage (stage 2)

- **DESIGN R-rules, end to end on the own button:** R2 (`:active` with `transition: none` is instant), R3 (hover eases over time; the sample starts from the painted value), R8 (the sheet's `:focus-visible` outline after Tab, none after a pointer press, offset +1 outside the box), R10 (`cursor` on hover), R11 (`transition: … 0s 300ms` switches between 250 and 500 ms), R13 (top marker as a per-side border, in the gallery). R1 and R9 stay OW0b's UA tests. **Not added:** R3's default 100 ms ease in `theme/base.css`. It is a theme change, needs the `@media (prefers-reduced-motion: no-preference)` wrapper, and would change the timing of every hover test. Recommended for the DS/OW1a theme pass.
- **WIDGET-PARITY:** P1 (parts ✓, pooled light rows open, see above), P2 (`content` typed values now reach `paint_icon`), P4 (n-stop linear and radial ✓; `border-image` stays OW-P2), Q1 (`paint()` selected from CSS runs). Parity rows these unblock: `::tab:first/:last/:middle/:only-one` → `::part(tab):first-child` … (§tabs), `QHeaderView::section:first/:last` positions, slider `::part(tick)` first/last, combo `::part(option)` positions. Each lane sets the indices.
- **VIS1:** a uniform border is still one `RectShape` with `StrokeKind::Inside`, so the box measurements are unchanged. Gap 1 (focus rings) is CSS-driven: a sheet's `:focus-visible` outline paints. Gap 6 (the tab close button only on active or hover) needs only `::part(close)` under `tab:hover`/`:selected`, which parts now compute. The VIS1 gate was not run here.
- **Not in this stage:** dashed borders for HC (`border-style: dashed` paints solid), radii followed by non-uniform borders, `paint_bounds` growth for outlines (culling, SG6's `walk::culled`), the slow-motion factor (SV37 `slow`) for CSS transitions, and transitions on stock-painted widgets (egui animates those itself).

## Not run locally

The full workspace nextest, the compiler suite and conformance; CI runs them.
