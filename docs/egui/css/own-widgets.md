---
title: Styling own widgets
description: "::part(), state pseudo-classes, content: icon(), paint() painters and transitions on svelters' own widgets."
status: available
verified_at: c0faf05a
sources:
  - plans/milestones/OW-CSS.md
  - crates/svelters-style/src/state.rs        # pseudo-class table
  - crates/svelters-egui/src/provider/state.rs # state bits
  - crates/svelters-egui/src/paint/painters.rs
  - fixtures/gallery-egui/src/pages/23-own-buttons.svelte   # checked by tests/gallery.rs
  - plans/research/DESIGN.md                   # R-rules
  - plans/research/WIDGET-PARITY.md            # P1-P10
  - plans/milestones/OW5a-lists-tree-dnd.md    # list rows
  - plans/milestones/OW7b-toasts-dialog-buttons-status.md # toasts
---

# Styling own widgets

svelters' own widgets paint from the CSS engine: the host's computed style, each part's
`::part(name)` style, and running transitions. Use this page to restyle a built-in own widget or your
own kind from a component `<style>` or a theme sheet. For the general engine (properties, selectors,
tokens) see the CSS pages; for writing the widget see [Writing an own widget](../rust/own-widgets.md).

## A worked example

From the gallery page `23-own-buttons.svelte` (checked by `tests/gallery.rs`,
`own_buttons_page_styles_parts_from_css`):

```svelte
<button class="pill">Pill</button>
<button class="accent" icon="check">Accent</button>
<button class="tab">Tab</button>

<style>
  .pill { border-radius: 12px; padding-left: 14px; padding-right: 14px; }
  .pill::part(label) { text-transform: uppercase; letter-spacing: 1px; font-weight: 700; }

  .accent {
    background-image: linear-gradient(#3b8eea, #1f6fbf);
    color: #ffffff;
    border-color: #1a5ea3;
    transition: background-image 120ms ease, box-shadow 120ms ease;
  }
  .accent:hover  { background-image: linear-gradient(#4f9df0, #2a7fd0); }
  .accent:active { box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.35); transition: none; }
  .accent::part(icon) { color: #c8f7c5; }

  .tab {
    border-width: 0;
    border-top: 2px solid #3b8eea;
    border-top-left-radius: 4px; border-top-right-radius: 4px;
    border-bottom-left-radius: 0; border-bottom-right-radius: 0;
  }
</style>
```

## `::part()`

A part is a named piece of a widget (`button::part(icon)`, `checkbox::part(indicator)`). Each widget's
reference page lists its parts.

- **State goes on the host**: `button:hover::part(icon)`. `::part(icon):hover` means the same (the state
  is the host's).
- **Several names** must all match: `::part(day today)`.
- **Logical position** of repeated parts: after `::part()`, `:first-child`, `:last-child`,
  `:only-child`, `:nth-child(An+B)` and `:nth-last-child(An+B)` match the part's position among its
  siblings, which the widget supplies. `:nth-child(… of S)` after a part is a diagnostic.
- Nothing else may follow the pseudo-element, and there is no second pseudo-element. `::before` and
  `::after` are rejected. `::placeholder` and `::selection` exist for text widgets.
- Paint parts are laid out as flex items of the host's content box: `flex-direction` and `gap` on the
  host, `order`, `width`/`height`, `margin` and `display: none` on the part. `button { flex-direction:
  column }` puts the text under the icon; `button::part(label) { display: none }` gives an icon-only
  button.
- A part inherits from its host. A part change that affects size (box, text, `content`) relayouts the
  host; anything else only repaints.

**Which parts read `::part()` rules today:**

| Widget | Parts that take CSS | Parts still painted from built-in tables |
|---|---|---|
| `button` | `icon`, `label` | — |
| `checkbox`, `radio-button` | `indicator`, `icon`, `label` | `mark` (checkbox), `dot` (radio) |
| `switch` | `track`, `label` | `knob` |
| `frame` | `caption` | `indicator`, `mark` (checkable frames) |
| `combo-box` (select) | `chevron`, `label`, `popup`, `option` | — |
| menus | `popup` | — |
| `progress-bar` | — | `track`, `fill` |
| `collapsing-header` | — | `header`, `twistie` |
| `toast` (OW7b) | `accent`, `close`; the box is the node (`toast`, `toast[level=warning]`, `toast:hover`, `toast:exiting`) | — |
| `toast-host` (its `show` toasts) | `toast`, `accent`, `text`, `action`, `close` | — |
| `modal`, `inner-window` | — | `backdrop`, `title`, `grip`, `titlebar`, `close`, `collapse`, `edge-*` |

A rule on a part in the last column has no effect yet; style the host (`switch:checked { … }`) instead.
The `scrollbar` thumb and the `split-view` sash have no `::part()` yet either.

**Toasts: per-run CSS needs the declarative form.** A `<toast>` under `toast-host` is a node, and so are
its labels' runs: `toast b { color: … }` and `toast a:hover { … }` reach them. The runs of
`host.call('show', runs)` are not nodes and keep RT2's tag defaults; `toast-host::part(text)` styles the
whole text. A closed `<toast>` stays drawn with `:exiting` while its `transition` plays out
(`toast { transition: opacity .2s } toast:exiting { opacity: 0 }`), on the virtual clock.

### List rows

Pooled rows of `virtual-list` and `tree-view` are styled as `::part(row)`. Each drawn row's part style is
computed with the list's state bits replaced by the row's own state, at the row's logical position:

```css
virtual-list::part(row):hover                         { background-color: var(--sv-list-hover); }
virtual-list:focus-within::part(row):selected         { background-color: var(--sv-list-sel); }
virtual-list:not(:focus-within)::part(row):selected   { background-color: var(--sv-list-sel-inactive); }
virtual-list::part(row):focus-visible                 { outline: 1px solid var(--sv-focus); outline-offset: -1px; }
virtual-list::part(row):nth-child(even)               { background-color: rgba(255, 255, 255, 0.03); }
tree-view::part(row):has-children:expanded            { font-weight: 600; }
```

`tree-view` takes the same selectors as `virtual-list`:

| Look | Selector |
|---|---|
| rest | `virtual-list::part(row)` |
| hover | `virtual-list::part(row):hover` |
| pressed | `virtual-list::part(row):active` |
| active selection | `virtual-list:focus-within::part(row):selected` |
| inactive selection | `virtual-list:not(:focus-within)::part(row):selected` |
| cursor row | `virtual-list::part(row):current` |
| keyboard focus ring (R8) | `virtual-list::part(row):focus-visible` |
| drop into a row | `virtual-list::part(row):drop-target` |
| stripes (1-based) | `virtual-list::part(row):nth-child(even)` |
| tree rows | `tree-view::part(row):has-children`, `tree-view::part(row):expanded` |
| tree chevron | `tree-view::part(twistie)`, `tree-view::part(twistie):expanded` |
| row contents | `virtual-list:focus-within > :selected { color: … }` (row nodes carry the same state bits) |
| scroll bar | `virtual-list::part(thumb-y):hover`, `::part(scrollbar-y)` |
| other parts | `::part(placeholder)`, `::part(drop-line)` |

- Tokens the defaults use: `list-hover`, `list-sel`, `list-sel-fg`, `list-sel-inactive`, `list-drop`,
  `focus`, `twistie-w`, `icon`, `scroll-shadow`.
- If no sheet has a row rule, nothing is computed and rows paint from the list's UA defaults. In Rust the
  API is `Scene::part_at(list, "row", state, (pos, len))`. Results are cached per (node, part, state,
  position) and dropped at any restyle.

## State pseudo-classes

Units and props set state bits; each bit is a pseudo-class. The widget's reference page says which
it sets.

| Pseudo-class | Bit | Typical setter |
|---|---|---|
| `:hover` | 0 | Hoverable (also while disabled) |
| `:active` | 1 | Pressable |
| `:focus` / `:focus-visible` | 2 / 14 | Focusable. `:focus-visible` is keyboard or program focus, never a pointer |
| `:focus-within` | 3 | moved with `focused` |
| `:disabled` / `:enabled` | 4 | the `enabled` prop |
| `:checked` / `:indeterminate` | 5 / 15 | Toggleable, prop mirror |
| `:selected`, `:open`, `:expanded` | 6, 7, 8 | props; Expandable (`:expanded`) |
| `:invalid` / `:valid`, `:user-invalid` | 9, 22 | the `invalid` prop / validation |
| `:read-only` / `:read-write` | 10 | the prop |
| `:dragging`, `:drop-target` | 11, 18 | drag and drop |
| `:exiting` | 12 | an outro is running |
| `:busy` | 13 | `progress-bar`/`spinner` busy |
| `:placeholder-shown` | 16 | text fields |
| `:default` | 17 | the `default` button |
| `:current`, `:backdrop` | 19, 20 | current item; inactive window |
| `:required` / `:optional` | 21 | the prop |
| `:has-children` | 23 | containers |

Bits 24–31 are free. App-defined states are provider states, matched as `:state(ns.name)`.

## Glyphs: `content`

On a glyph part (`::part(icon)`, a check mark, a twistie):

| Value | Effect |
|---|---|
| `content: icon(check)` | that icon. A key may be dotted (`cad.extrude`) or `family:name` |
| `content: none` | no glyph |
| `content: paint(cad.twistie)` | a registered Rust painter draws it |
| `content: url(…)` | parsed; the icon primitive does not draw URLs yet |

Today icons resolve through a registered icon font (`fonts::register_icons`). The icon registry
replaces that with keyed icons; see [Icons](../icons.md).

## Painters: `paint(name)`

A Rust painter registered with `paint::register_painter("cad.knurl", …)` (or
`EmbedConfig::painter`) is picked from CSS:

```css
slider::part(thumb)               { background-image: paint(cad.knurl); --knurl-pitch: 2px; }
tree-view::part(twistie)          { content: paint(cad.twistie); }
slider:focus-visible::part(thumb) { --knurl-color: var(--sv-focus); }
```

- `background-image: paint(name)` paints after the background colour and inset shadows and before the
  border, in the padding box.
- `content: paint(name)` replaces the glyph.
- Custom properties are the painter's arguments (`s.px("--knurl-pitch")`).
- An unknown name falls back to the default paint and is reported once.

## Boxes

The box primitive paints: outer `box-shadow`s, `background-color`, `background-image`
(`linear-gradient` with any stops and angle; `radial-gradient` with `circle|ellipse`, the four extents
and `at <position>`), inset shadows, a painter, then the border. Borders may differ per side
(`border-top: 2px solid …`) and radii per corner (`border-top-left-radius`). `outline-*` paints
outside the box (the focus ring). Shorthands: `border`, `border-<side>`, `border-{width,style,color}`,
`border-radius` (1–4 values, no `/`), `outline`.

Not supported yet: dashed borders (paint solid), radii that follow non-uniform borders, elliptical radii,
`border-image`, `transform`.

## Transitions

`transition` (the shorthand only; no `transition-*` longhands) works on own widgets and their parts:

- A changed property named by an entry (by name, `all`, or a covering shorthand) starts a transition if
  the duration is above 0. The last matching entry wins.
- It starts from the painted value, so un-hover reverses smoothly. Retargeting to the same end value
  doesn't restart it.
- Interpolated: colours, lengths, numbers, insets, font weights, shadow lists, and gradients with equal
  stop counts (linear↔linear, radial↔radial of the same shape). Keywords switch at once.
- Timing: `linear`, `ease*`, `cubic-bezier()`, `steps()`, `step-start`/`step-end`. Times in `s`/`ms`.
- **A delay with zero duration** switches after the delay: `transition: background-color 0s 300ms`
  (DESIGN R11, the sash bar after 300 ms).
- **Instant states**: put `transition: none` on the state (R2, `:active`).
- **Reduced motion**: `EguiBackend::set_motion(MotionConfig { reduced: true, .. })` ends running
  transitions and starts none, and `@media (prefers-reduced-motion: reduce)` matches. Wrap optional
  motion in `@media (prefers-reduced-motion: no-preference)`.
- Transitions run on the logic step's clock, which is kittest's virtual clock in tests.

## The DESIGN rules in CSS

The default theme meets these through the widgets' UA tables and the base sheet. Override them with
the same properties.

| Rule | CSS |
|---|---|
| R1 raised buttons | `background-color`, a ≥3:1 `border-color`, `border-radius: 4px`, `box-shadow: var(--sv-elev-1)` |
| R2 pressed sinks | `:active { box-shadow: inset 0 1px 2px rgba(0,0,0,.25); transition: none }` |
| R3 hover | `:hover` fill; optional `transition: background-color 100ms ease` under `prefers-reduced-motion: no-preference` (not in the base sheet) |
| R4 sunken fields | inset `box-shadow`, `cursor: text`, `:focus` border, `:user-invalid` |
| R6 rows | `:hover`, `:selected`, `list:not(:focus-within) row:selected`, `outline: 1px solid; outline-offset: -1px`, `:current` |
| R8 focus rings | `:focus-visible { outline: 2px solid var(--sv-focus); outline-offset: 1px }` (rows and fields: 1px, offset −1) |
| R9 disabled | `:disabled { opacity: .4; cursor: not-allowed }` |
| R10 cursors | `cursor: pointer`, `text`, `grab`, `col-resize`, … (36 keywords) |
| R11 sashes | `cursor: col-resize` at once, the bar via `transition: … 0s 300ms` |
| R13 tabs | `border-top: 2px solid …` marker, `:hover` `elev-2` |
| R17 toggles | `:checked`, `:indeterminate` |
| R18 links | `:hover { text-decoration-line: underline }` |

## Mistakes we have seen

- `::part(icon):hover` read as "hovering the icon": the state is always the host's.
- Styling `switch::part(knob)` and seeing no change: see "Which parts read `::part()` rules today"
  above.
- A transition on `:active`: pressed must be instant (`transition: none`).
