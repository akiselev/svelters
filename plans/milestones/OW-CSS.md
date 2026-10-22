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

`crates/svelters-style/tests/ow_css.rs` (11 tests) and `shorthand.rs` unit tests (2):
- every new pseudo-class, and the bit order;
- which pseudo-element selectors parse and which are rejected;
- part, placeholder and selection styles: host state, inheritance from the host, specificity, other parts unmatched, `has_pseudo_rules`, host invalidation;
- side, outline and radius resolution, and shorthand/longhand cascade order;
- shorthand and value diagnostics;
- text settings and cursor inheritance;
- gradients;
- transition parsing and easing;
- the store: halfway sample, reversal from the painted value, end, `transition: none` on `:active`, reduced motion, zero duration with a delay (R11);
- the text primitive's properties.

The whole `svelters-style` suite passes (53/53, plus the ignored measurement), as do the egui pin test and the spec test.

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

### CSS parse cost (coordinator request)

Measured on the gallery-egui sheets in a release build, median of 200 runs, through `Stylesheet::parse` + `Stylist::add_sheet` (what `install_sheet` does):

| Set | Sheets | Bytes | Parse | Parse + add_sheet |
|---|---|---|---|---|
| Startup (theme token sheets for both schemes, `base.css`, 3 component sheets) | 6 | 16,048 | 200 µs | 227 µs |
| Theme only | 3 | 15,690 | 189 µs | 205 µs |
| One HMR swap (the component sheets) | 3 | 358 | 9 µs | 17 µs |

Startup is ~0.23 ms, under the 1 ms threshold, so the content-hash cache and a pre-parsed Q4 form are not recommended. The **parsed-sheet handoff was not built**:
- the live loader doesn't apply `<style>` yet (`svelters-modules/src/source/transform.rs` warns; Q3w's `scoped_css` is only wired into Vite);
- the saving would be at most ~17 µs per swap;
- a compile-time parse resolves `:state()` with an any-name resolver, so it would carry wrong provider bits.

When a loader lane wires `scoped_css` into the live path, it should send text, as Vite does.

To reproduce:
1. `OWCSS_SHEETS=<dir> cargo test -p svelters-egui --test css_parse_cost -- --ignored` dumps the theme sheets into `<dir>`.
2. Put each component's `<style>` body in `<dir>` as well.
3. `OWCSS_SHEETS=<dir> cargo test --release -p svelters-style --test parse_cost -- --ignored --nocapture` prints the numbers.

## Stage 2 (waiting on OW0a's `ci/ow0`)

- `StyleView` behind OW0's primitives. The `Transitions` store goes on the scene, sampled with egui's input time (kittest's virtual clock in tests).
- Cursor set on hover from the computed `cursor` (WG6 QA14).
- A part-styled button variant in the gallery.

## Not run locally

The full workspace nextest, the compiler suite and conformance; CI runs them.
