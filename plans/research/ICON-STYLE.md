# ICON-STYLE: cadatomic's own icon style (IC0)

Status: research (IC0), 2026-10-22. Branch `research/ic0`. This is a style guide proposed for approval. Nothing
was built and no cargo was run.

The request (user): "Codicons are fine for now but i want our distinctive and consistent icon style across the
app", and, on the prototype sheet: "I like some of those icons. I think we should definitely use agents to create
our own visual language and consistent design (and we'll need multiple icon weights for smaller icons so that
theyre still legible)". The end state is about 460 of our own icons, chrome included. Codicons are placeholders
until then.

Inputs:
- `ICONS.md`: §3 (visual language), §4 (the prototype and its results) and its "Decided" block: CC-BY-4.0,
  IEC 60617 by default, mate as peg-in-slot, the user approves batches at contact-sheet level.
- `icons-proto/`: the 20 prototype icons. The user liked extrude, section, footprint, via, trace, layer stack,
  mesh, fixed support, force load and probe, and they are kept recognisable here.
- `DESIGN.md`: §1.5 (Dark and Light Modern tokens), §2 (affordances R1–R18), §2.6 (density), §4.5 (icons).
- egui 0.37 / emath 0.37 from the local cargo registry: `gui_zoom.rs` (zoom steps of +0.1) and
  `gui_rounding.rs` (`GuiRounding::round_to_pixels`).

What was made, all in `plans/research/icon-style/`:
- `src.py`: the 24 samples as hand-written SVG path data (16 and 24 masters, plus 12, 14 and 20 masters for 6
  of them), the badge composer, the open-choice variants, and a self-check of the grid rules. It writes
  `<out>/<size>/<id>.svg`.
- `svg/<size>/<id>.svg`: 66 masters plus 12 badge compositions. *(IC1 moved the 66 masters to
  `tools/icons/samples/`, the toolchain's reference set; `tools/icons/icons.py build` makes the compositions.)*
- `sheet.py` and **`contact-sheet.png`**: the approval sheet (§12).

Where this guide and `ICONS.md` §3 differ, this guide is the newer proposal. The differences are listed in §13.

## 0. Summary

1. **Three traits make the set ours.** These are the traits that are not codicons-with-extras:
   - **the subject tone**: one 40% tone that always means "the thing the command acts on";
   - **the cut corner**: square corners, with one 45° cut at the top right of every frame that stands for an
     object or container. Round corners are kept for geometry that really is round;
   - **the drafting arrowhead**: every arrow ends in one solid 90° head, while chevrons stay open.

   All three are legible at 16@1x, and a linter can check all three (§2).
2. **Grid:** 1px strokes on half-pixel centres at 12, 14 and 16; 1.25 at 20; 1.5 at 24; the 16 and 24 masters
   at ×2 for 32 and 48. Diagonals are 45° or 2:1 only. Copper strokes are 2px (16) or 3px (24) (§3).
3. **Optical sizes 12, 14, 16, 20, 24 and 32+.** Each is its own drawing. Smaller sizes are relatively heavier,
   because 1px is the floor: 1/12 against 1/16. They also carry less detail. Below 16, small subjects become
   solid and severity glyphs become solid with knocked-out marks. **No separate bold or emphasis weight** (§5).
4. **Fractional zoom:** icon boxes always snap to whole physical pixels. The master is chosen by physical pixel
   size, so the 20 master serves 1.25× and the 24 master takes over from about 1.4× (§5.3).
5. **Colour slots:** the classes `subject`, `accent` and `badge`. ICR1 can recolour them, and it may set only
   their `color`, never geometry (§6).
6. **Badges** are composed by the build. It knocks out the bottom-right cell and draws one of five glyphs: add,
   remove, warning, link, lock (§7).
7. **Projection is the domain signal.** MCAD uses 2:1 isometric, ECAD a top view with copper filled, and
   simulation flat diagrams. Chrome is flat and frontal, and uses a domain's projection only when it shows
   that domain's object (§8).
8. **Samples:** 24 icons at 16 and 24, in three passes. 19 are good and 5 are acceptable, but each of those 5
   should get a look at review (§11).
9. **Open choices for the user**, each shown as a strip on the sheet: terminals (round, recommended), corners
   (cut on every frame, recommended) and arrowheads (solid 90°, recommended) (§12).

## 1. Principles

- **Pixel-first.** Every master is drawn for the size it is shown at, and every line is crisp at 1x on its own
  grid. Scaling is a fallback, never the design path.
- **One ink plus one tone.** Primary lines are 100% `currentColor`, and the subject tone is 40%. No other
  values exist, except the results fringe (§7.1).
- **One idea per icon.** Up to 3 elements at 16, up to 2 at 12 (§5).
- **Conventions over invention.** Engineers already know the section hatch, the hatched ground, IEC symbols,
  dash-dot centre lines and filled copper. The style holds these conventions; it does not replace them.
- **The icon never carries state or data.** Pressed, active and selected states belong to the control
  (DESIGN R1, R2, R6), and layer, net and result colours belong to swatches beside the icon (`ICONS.md` §3.3).
  The `accent` slot is the only exception, and only for state glyphs (§6).

## 2. What makes the style ours

Codicons are monotone outlines with rounded rectangle corners and open arrowheads. Most icon sets are drawn
that way, and so are Lucide, Tabler and Iconoir. We keep codicons' grid and weight, so the two sets mix during
migration, and add three traits that a user can see at 16@1x.

### T1. The subject tone

Every operation shows **what it acts on** in a 40% tone of the icon colour, and **the action** at 100%. Extrude
shows the profile in the tone and the arrow solid. A load shows the body in the tone and the force arrow solid.
In a constraint the geometry is in the tone and the relation glyph solid. Mate shows the moving part in the
tone.

- **Why it is ours:** Phosphor's duotone is a decorative 20% background on every icon. Ours has one fixed
  meaning, appears only where the icon has a subject, and is checked by the lint (class `subject`, §6). Over the
  ~290 domain icons it teaches the reading "grey is the input, solid is what happens".
- **Legibility:** the tone always sits inside a 100% outline, so it never forms a boundary by itself. It gives
  2.1:1 (Light Modern) to 2.8:1 (Dark Modern) against the background, enough to separate it from the outline
  (10–11:1) at 16@1x. Section 2 of the sheet shows this in both themes.
- **Chrome:** chrome nouns (file, folder, gear) have no tone. Chrome verbs get it only where they draw their
  object: for example, a later `copy` shows the copied document in the tone.

### T2. The cut corner ("round means round")

Corners are **square**. Every frame that stands for an object or a container, such as documents, folders, save,
panels, windows and document-kind icons, has **one 45° cut at the top right**: 3px at 16, 5px at 24 (§5 has the
other sizes). No other corner is rounded.

- **Why it is ours:** it is a CAD-native mark that already appears in the conventions: chamfers, an IC
  package's pin-1 bevel, a PCB corner, the floppy disk and the folded page. It gives chrome a silhouette that
  codicons' rounded rectangles don't have. And it keeps **roundness meaningful**: a rounded corner in our set
  is a fillet, a hole or an arc, never styling. In `mcad.fillet` the round corner is the whole message.
- **Legibility:** a 3px cut is three diagonal pixels at 16@1x, which the eye reads at once. That is about
  twice as visible as codicons' 1px radius (sheet §2, file, folder and save at 16@1x).
- **Engineering geometry is drawn true**: a footprint body, a board outline or a mesh domain has the shape the
  object has, and no cut is added to it.

### T3. The drafting arrowhead

Every arrow ends in **one solid head with 45° sides**, which gives a 90° apex. The head's half-base equals its
length: 3px at 16 and 4.5px at 24 (§5). Its base sits on a whole pixel, so the base edge is crisp. Arrows that
point diagonally use a right-angled head whose two legs are axis-aligned, so both of its edges are crisp
(`mcad.fillet`). **Chevrons stay open**: a chevron means "navigate or expand", and a solid head means "motion,
direction or force".

- **Why it is ours:** codicons and most sets draw arrows and chevrons with the same open V. A solid head is the
  dimension and load arrow of engineering drawings, and the run triangle (`chrome.run`) is the same shape.
  Undo, redo, extrude, revolve, loads, sync, upload and download then share one mark.
- **Legibility:** a 6×3 solid head carries more ink than two 1px diagonals, and it survives at 12px, where an
  open V turns into a smudge (sheet §5 and §7, O3).

The three traits compound: on a toolbar of 30 icons the user sees cut corners, solid heads and grey subjects,
and none of them depends on colour.

## 3. Grid, keylines and strokes

### 3.1 The rules every size shares

| Rule | Value | Lint |
|---|---|---|
| Root | `viewBox="0 0 S S"`, `width`/`height` = S, `fill="none"`, `stroke="currentColor"`, `stroke-width` per §5, `stroke-linecap="round"`, `stroke-linejoin="round"` | L1 |
| Stroke centres | strokes of width 1, 1.25, 1.5 or 3 lie on **x.5**; width 2 lies on integers. This covers horizontal and vertical segments, `rect` edges and the axis extremes of circles (cx ± r) | L6 |
| Fill edges | solid shapes (pads, arrowheads, the run triangle, badges) put their horizontal and vertical edges on **integers** | L6 |
| Diagonals | 45° (dx = dy), or 2:1 (dx = 2dy or dy = 2dx); nothing else. Arcs and circles are free. Radially symmetric generators (gear teeth) are exempt by id | L7 |
| Terminals | round caps. **Butt** caps for dashed lines, copper traces and gear teeth, and for any stroke that ends in a pad or an edge | L1 (per element) |
| Joins | round; miter for copper traces, so that a 45° trace corner stays a corner | — |
| Corners | square; the T2 cut on object and container frames; `rx`/`ry` never used | L5, *(review: which frames get the cut)* |
| Copper | traces and spokes 2px on integer centres at 16, 3px on .5 centres at 24 | L9 |
| Hatching | 1px, 45°, pitch 3–4px at 16 and 4.5px at 24, always clipped: section cuts and fixed supports only | L9 |
| Centre lines | dash-dot `4 1 1 1` (16) / `6 1.5 1.5 1.5` (24), butt | — |
| Forbidden | `transform`, `style`, `<style>`, `<text>`, `<image>`, gradients, filters, `xlink`, colours other than `currentColor` (`#000`/`#fff` only inside a `<mask>`) | L2, L3 |
| Allowed elements | `svg g defs path rect circle ellipse clipPath mask` | L2 |

### 3.2 Keylines (stroke-centre coordinates)

The outer ink is half a stroke wider than these numbers on every side.

| Shape | 16 | 24 | Used by |
|---|---|---|---|
| Live area | 1..15 (pad 1) | 2..22 (pad 2) | everything; arrowheads, badges and the extreme ends of round caps may touch the edge |
| Square | 1.5..13.5 or 2.5..13.5 | 2.5..20.5 | mesh, result, footprint body height |
| Circle | centre (8, 8), r 5.5 → 2.5..13.5; or centre .5 with integer r | centre (12, 12), r 8.5; or centre .5 with integer r | lens, via, rings |
| Portrait (document) | 3.5..12.5 × 1.5..14.5 | 5.5..18.5 × 2.5..21.5 | file and document kinds |
| Landscape (container) | 1.5..14.5 × 2.5..13.5 | 2.5..21.5 × 3.5..20.5 | folder, save, panels |
| Iso slab (2:1) | 1.5..13.5 wide, top face 6 tall | 1.5..21.5 wide, top face 10 tall | MCAD solids |

**Optical size inside the box:** a square reads larger than a circle of the same width, so squares are drawn
1px narrower than circles (12 against 13 at 16). A triangle's centroid sits a third of the way up from its base,
so the run triangle is moved 1px right of the box centre. Odd-width symmetric glyphs (add, warning) centre on
7.5 at 16, half a pixel left of the box centre, as codicons does.

### 3.3 Gaps and counters

A **gap** is the clear (unpainted) distance between two inked strokes. A **counter** is an enclosed clear area.
§5 gives the minimum per size. At 16, parallel stroke centres are at least 2px apart (1 clear pixel), and
every counter is at least 2×2 clear. The checks are geometric (L10): stroke centres, minus the stroke width.

A raster "crispness" metric is **not** a usable gate. Correct icons that are mostly diagonal or round score high
on partial-alpha pixels (16@1x: search 0.76, chevron 0.71, close 0.69; footprint 0.05, mate 0.00, measured on
the samples). So IC1 lints the geometry and leaves the raster to the human.

## 4. Strokes in the two themes and at 1x and 2x

| | Dark Modern | Light Modern | High Contrast (dark / light) |
|---|---|---|---|
| Icon colour (`icon`) | `#CCCCCC` on `#181818` (11.1:1) | `#3B3B3B` on `#F8F8F8` (10.6:1) | `#FFFFFF` on `#000000` / `#292929` on `#FFFFFF` |
| Subject tone at .4 | `#606060`, 2.8:1 | `#ACACAC`, 2.1:1 | .4 gives 3.7:1 / 2.4:1, so **HC raises the tone to .6**: 7.4:1 / 4.0:1 |
| On list selection | `#FFFFFF` on `#04395E` (`list-sel-fg`) | `#000000` on `#E8E8E8` | — |
| Disabled | whole control at 40% (R9); the tone becomes 16% (1.3–1.5:1), which is acceptable because the tone is never the only cue | same | same |

- **1x:** every master is drawn for its size, so at 1x it is pixel-exact: 1px lines are one solid pixel. The
  1.25 and 1.5 strokes of the 20 and 24 masters are one solid pixel plus a faint fringe (.125 and .25 on each
  side), which looks slightly softer than codicons at 1x and much crisper than a scaled 16.
- **2x:** a master at ×2 is crisp by construction, because .5 coordinates land on odd whole pixels. 16@2x
  draws the 16 master at 32, and 24@2x draws the 24 master at 48. No 32 or 48 drawing is needed for UI use.
- **Light against dark:** the same SVG, tinted. On a light ground the eye reads dark-on-light strokes as
  slightly heavier and the tone as lighter. The sheet shows every sample in both themes, and no sample needed a
  per-theme change. **No per-theme masters.**

## 5. Optical sizes

The user asked for "multiple icon weights for smaller icons so that they're still legible". A crisp line can't
be thinner than one device pixel, so at small sizes the weight rises by itself. The rest of the work is
**dropping detail**. Each optical size is a separate drawing of the same metaphor.

### 5.1 Per size

| | **12** | **14** | **16** | **20** | **24** | **32 / 48** |
|---|---|---|---|---|---|---|
| Used for | tree badges, inline in text, dense tables | status bar, `compact` density, inline chips | default chrome (trees, lists, menus, tabs, small toolbars) | `comfortable` density (DESIGN §2.6), larger toolbars, **16 at 1.25×** | command strip, task-panel headers, **16 at 1.5×** | empty states, dialogs; HiDPI 2x |
| Stroke | 1 | 1 | 1 | 1.25 | 1.5 | the 16 / 24 master at ×2 (2 / 3) |
| Relative weight (stroke ÷ size) | **.083** | **.071** | .0625 | .0625 | .0625 | .0625 |
| Centres | .5 | .5 | .5 | .5 | .5 | ×2 |
| Live area | 10 (pad 1); the main shape may use the pad | 12 (pad 1) | 14 (pad 1) | 18 (pad 1) | 20 (pad 2) | ×2 |
| Min gap (clear) | 1 | 1 | 1 | 1.75 (centres ≥ 3 apart) | 1.5 (centres ≥ 3 apart) | ×2 |
| Min counter (clear) | 2×2 | 2×2 | 2×2 | 3×3 | 3×3 | ×2 |
| Max elements | **2** | 3 | 3 | 4 | 4 | 4 |
| Arrowhead half-base = length | 2 | 2.5 | 3 | 3.5 | 4.5 | ×2 |
| Corner cut (T2) | 2 | 3 | 3 | 4 | 5 | ×2 |
| Subject tone | **no**: a simple subject area becomes **solid** when the action glyph doesn't overlap it, otherwise it is an outline | yes, if its clear interior is at least 2×4; otherwise dropped | yes | yes | yes | yes |
| Badges | none: use a separate state id | none | 8×8 cell | 10×10 cell (not drawn yet) | 12×12 cell | ×2 |
| Severity glyphs (warning, error, info) | **solid**, with the mark knocked out | **solid**, with the mark knocked out | outline | outline | outline | outline |
| Dropped detail | slab thickness, pin-1 dot, the third pad row, hatching (becomes tone or solid), dash-dot (becomes solid), the 2×2 mesh (one diagonal), fringe bands 3 → 2 | slab thickness, pin-1 dot, hatching (becomes tone) | — | — | may add one detail | a dedicated 32+ master only for illustrations |

Sheet §5 shows six samples, `file`, `search`, `warning`, `extrude`, `footprint` and `load` (three chrome icons
and one from each domain), across all six sizes in both themes, with 12 and 14 enlarged ×5. What changes:
- `extrude` 12: the slab becomes a solid 2:1 rhombus and the arrow starts above it. At 14 the slab loses its
  thickness and keeps the tone. Both are still the liked prototype's shape.
- `footprint` 12: two pad rows and no pin-1 dot. At 14: three rows and no dot.
- `load` 12: the body becomes a solid 2px plate.
- `warning` 12 and 14: a solid triangle with the "!" knocked out. In v2 the outlined "!" was too thin at 14.

### 5.2 Which masters an icon must have

| Master | Required for | Estimate (of ~460) |
|---|---|---|
| 16 | every id | 460 |
| 24 | every id that can sit in the command strip, plus all chrome (so chrome is crisp at 150%) | ~230 |
| 20 | chrome, and the domain ids in toolbars that `comfortable` density or 125% zoom shows | ~150 |
| 12 / 14 | ids listed for status bar, tree badges and inline use | ~60 |
| 32+ | illustrations only | a handful |

This is more than `ICONS.md`'s "~290 at 16 plus ~120 at 24", because the user now asks for optical sizes and
fractional zoom (below) depends on them. The fallback for a missing master is §5.3, rule 4: the icon is soft,
never broken.

### 5.3 Fractional zoom and fractional DPI

egui zooms in steps of 0.1 (`gui_zoom.rs`: `zoom_factor += 0.1`, so 1.1, 1.2, 1.3 …). Windows adds 125% and
150% display scaling. Icons are rasterised from SVG at the exact physical size, and so they see every
fractional scale.

**Rule Z1: icon boxes always snap to whole physical pixels, at every zoom.**
The box origin is `round(origin × ppp)` and the size is `P = round(size × ppp)`, where `ppp` is
`pixels_per_point`: native scale × zoom. In egui that is `rect.round_to_pixels(ppp)` (emath `GuiRounding`).
The texture is requested at P physical pixels and painted 1:1. Layout reserves the logical size and centres
the snapped box inside it, so nothing jitters. Without snapping, a box at a .35px offset smears every 1px line
over two pixels. Sheet §6 enlarges the 1.25× case: "naive" is the 16 master scaled from a fractional origin,
and "rule" is the 20 master on a snapped box.

**Rule Z2: choose the master by physical size P**, in this order:
1. a master of exactly P;
2. P = 2m: master m at ×2 (crisp);
3. the largest master m ≤ P with P − m ≤ 2: drawn at its native size, centred with an offset of
   `floor((P − m) / 2)`. It stays crisp and is at most 2px smaller than the box;
4. otherwise the master nearest to P, scaled. This one is soft, which is accepted, and it is what VS Code does
   with codicons at every zoom.

For a 16pt icon with the 16, 20 and 24 masters:

| Zoom (or DPI) | 1.0 | 1.1 | 1.2 | 1.25 | 1.3 | 1.4 | 1.5 | 1.6 | 1.7 | 1.8 | 2.0 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| P | 16 | 18 | 19 | 20 | 21 | 22 | 24 | 26 | 27 | 29 | 32 |
| Master | 16 | 16, padded 1 | 20 scaled ×.95 (soft) | **20** | 20, padded 0/1 | 20, padded 1 | **24** | 24, padded 1 | 24 scaled ×1.125 (soft) | 16×2 scaled ×.9 (soft) | **16×2** |

**So the answer to "should 24 masters take over above some scale?" is yes, from about 1.45× (P ≥ 23) for a
16pt icon**, with the 20 master covering 1.2–1.4×. A 24pt icon (the command strip) uses its 24 master up to
P = 26. Above that the virtual masters 32 (16×2), 40 (20×2) and 48 (24×2) take over. They are crisp at exactly
those sizes and soft in between: at 1.5× (P = 36) the command strip is soft. That is accepted, because those
buttons are large and their strokes are over 2px there.

The texture cache key is `(id, P, colours)` (§6). Sheet §6 shows the six optical samples at 1.0, 1.1, 1.25 and
1.5, naive against the rule, in both themes.

### 5.4 Is there a bold or emphasis weight? Recommendation: no.

- **Small sizes are already the heavy weights** (§5.1). That is what the user asked for.
- **Selected, active and pressed states belong to the control**, not the glyph (DESIGN R1, R2, R6, R13). The
  selected-row foreground (`#FFFFFF` on `#04395E`) and `fg-strong` already raise contrast where it matters.
- **High Contrast** uses the same masters. The root colour becomes the HC foreground, and the loader raises the
  subject tone from .4 to .6, which takes it from 2.4–3.7:1 to 4.0–7.4:1. The lint guarantees that the literal
  `fill-opacity=".4"` occurs only on `.subject`, so the substitution is safe. VS Code's HC themes don't swap in
  bolder codicons either [unverified].
- **Cost:** a bold set doubles every master (~460 × 2–4 sizes) and gives the style a second look to keep
  consistent.
- **Revisit** if VIS1's HC captures show glyphs failing 3:1. In that case the fix would be HC-specific 16
  masters for the ~20 status glyphs, not a whole bold weight.

## 6. Colour slots (the contract with ICR1)

Icons are rasterised by resvg and tinted from outside: `currentColor` renders white and is multiplied by the CSS
`color`. The `<icon>` kind (ICR1) can also set three named slots, which it injects into usvg as a stylesheet.

| Class on the SVG part | Meaning | Default (no slot set) | CSS slot | Allowed on |
|---|---|---|---|---|
| *(none)*: primary | the drawing | `currentColor`, 100% | `color` | everything |
| `subject` | T1's subject tone | `currentColor` at `fill-opacity=".4"` (`opacity=".4"` on stroke-only parts). At 12 a subject may be solid (§5.1) | `--icon-subject` | the subject areas only |
| `accent` | a **state** colour | `currentColor`, 100%, so it looks like primary until a slot is set | `--icon-accent` | **state glyphs only**: run (success), stop and error (error), warning, info, breakpoint, pass/fail. The id must be on an allowlist (L4). **Never data colours** such as layers, nets, materials or result maps |
| `badge` | the corner modifier | `currentColor`, 100% | `--icon-badge` | the badge group that the build composes (§7). Never hand-drawn |

**How the slots apply.** Every part is drawn with `currentColor`, and the injected sheet sets only the CSS
`color` on these class selectors, for example `.accent{color:#CCA700}`. `currentColor` then resolves per
element, so the drawing's own opacity (.4 on the subject) is untouched. A `subject` part's 100% outline takes
the slot colour too, so "the subject is blue" colours the whole subject.
*[unverified: that usvg resolves `currentColor` from a `color` set by an injected stylesheet. ICR1 tests it.
The fallback is to inject `fill` or `stroke` per class, and the lint (L4) guarantees that every classed element
is fill-only or stroke-only with the outline as a separate unclassed element. The samples would need that
split for `subject` parts, a mechanical change in `src.py`.]*

**Tinting with slots.** Multiplying white by `color` is correct only while every part derives from
`currentColor`. When any slot is set, the loader must rasterise with the root `color` set to the resolved CSS
`color` and the slot colours injected, and must **not** multiply. Otherwise #CCA700 × #CCCCCC darkens the
warning yellow. So the cache key includes every colour.

**Prohibitions** (L3, L4):
- Only `color`, `--icon-subject`, `--icon-accent` and `--icon-badge` reach an icon. No other CSS variables.
- The injected sheet contains only `color` declarations on `.subject`, `.accent` and `.badge`. **No
  `stroke-width`**, opacity, `transform`, `display`, `fill-rule` or any other geometry property: a stroke-width
  override moves every line off the half-pixel grid (§3.1).
- The SVGs carry no `style` attributes, no `<style>` elements and no classes outside the three.

Sheet §4 shows four cases in both themes: `warning` with accent set to the warning colour, `run` with accent set
to success, `sketch+warning` with badge set to the warning colour, and `extrude+remove` with badge set to the
error colour. Each is shown next to its default.

## 7. Tone and badges

### 7.1 Opacities

The only allowed values are 1 and .4 (`fill-opacity` on fills, `opacity` on stroke-only parts), and only on
`.subject`. The single exception is the results fringe (`sim.result*`): .2 / .45 / 1 (L5).

### 7.2 Badges

Five glyphs, closed list for IC0: **add, remove, warning, link, lock**. `ICONS.md` also lists run, check, error,
settings, queued and stale; those follow the same rules.

- **Composition (build time, IC1):** `id+badge` becomes the base in `<g mask>` plus `<g class="badge">` holding
  the glyph. The mask **clears the bottom-right cell**: 8×8 at 16 (pixels 8–15), 12×12 at 24. The glyph is drawn
  inside the cell with 1px of clearance: centres 9.5..15.5 at 16, 14.5..22.5 at 24.
- **Why the whole cell and not the glyph's outline:** a contour knockout leaves pieces of the base between the
  plus's arms, which read as a grid. Clearing the cell matches codicons' `new-file` and reads as "a modifier".
- **Glyph shapes:** add and remove are 1px (1.5 at 24) strokes, 7px (9px) long. Warning is a solid 2:1
  triangle with the "!" knocked out. Link is two brackets joined by a bar. Lock is an arc shackle on a solid
  body.
- **Drawing for badges:** a base that will be badged keeps its identity in the top-left 8×8 at 16. File, folder,
  extrude, sketch and footprint all pass that test (sheet §3).
- **Not at 12 or 14:** a cell would take half the icon. Use a separate state id or a status-bar chip instead.
- **Link at 16 is the weakest badge.** At 7px the brackets almost close, but it still reads as a link next to
  its 24 version. Acceptable.

## 8. Projection per domain, and how chrome relates

| Domain | Projection | Rules |
|---|---|---|
| **Chrome** | **flat, frontal** (elevation view) | silhouettes of everyday objects: page, folder, floppy, gear, lens, bug. No iso and no depth cues. Tone only on a drawn object of a verb (T1). Uses T2 on object frames and T3 on every arrow. **When a chrome icon shows a domain object, it uses that domain's projection**: `doc.part` puts an iso cube in the page, and `doc.board` a top-view board |
| **MCAD** | **2:1 pixel isometric** for solids, viewed from above-front. Sketch geometry is **flat** | visible edges only (hidden lines never drawn, except the section cut). The top face carries the subject tone when it is the profile. Sketch points are hollow 3×3 squares (4-span at 24). Constraints show the geometry at .4 and the relation glyph at 100%. Section cuts are hatched at 45°. Operations are subject in the tone plus a solid-headed action |
| **ECAD** | **top view only**; 0°, 45° and 90° | **copper is filled**: pads and vias at 100%, pours in the tone with an outline. Traces are 2px (3px at 24) with miter joins. Silkscreen, courtyard and board outline are 1px strokes. Schematic wires are 1px with solid junction dots and hollow pin rings. Symbols are **IEC 60617** (rectangle resistor), with ANSI as an icon-theme swap |
| **Simulation** | **flat 2D**, as FE textbook diagrams | the body in the tone. Structural notation: hatched ground (fixed), triangle (pinned), circles (roller). Loads are solid-headed arrows: one (force), a row (pressure), curved (moment), wavy (convection). Electrical boundary conditions reuse the ECAD glyphs. Results use the banded fringe |

**What holds them together:** one stroke weight, one arrowhead (T3), one cut corner (T2), one tone (T1), one set of
badges, and shared primitives (point marker, pad, hatch, centre line). **What keeps them apart:** the projection.
Iso means a 3D model, a top view means a board, a flat diagram means analysis, and a frontal silhouette means the
application.

## 9. Metaphor per core concept

"Drawn" means a sample on the sheet; "proposed" is guidance for IC2–IC4. All of them follow §8.

| Concept | Metaphor | State |
|---|---|---|
| file | portrait page with the T2 cut, no fold line | drawn |
| folder | tab at the top left, body with the T2 cut | drawn (O2) |
| search | lens and a 45° handle at the bottom right | drawn |
| settings | six-tooth gear: ring, hub and butt teeth (axis teeth crisp) | drawn |
| close | 45° X, 8×8 | drawn |
| chevron | open 90° V (navigation, never an arrow) | drawn |
| add / remove | plus / minus, 11px at 16; the same glyphs as the badges | drawn |
| run | **solid** 1:2 triangle (the T3 shape), `accent` | drawn |
| stop | solid square, `accent` | proposed |
| debug | bug: round body, arc head, six legs on 2:1 diagonals | drawn |
| save | floppy, whose own cut is T2 | drawn |
| undo / redo | U-turn shaft with a solid head | drawn (undo) |
| warning / error / info | outline triangle / circle with a mark, solid at ≤14, `accent` | drawn (warning) |
| lock / link | padlock / two brackets and a bar (also badges) | badges drawn |
| document kinds | the file page plus a domain mark in its projection (part: iso cube; assembly: two cubes; drawing: title-block line; schematic: IEC resistor; board: top-view outline with a via) | proposed |
| sketch | flat profile with hollow square points, plus a pencil (solid body, open cone) | drawn |
| extrude | iso slab, top face in the tone, solid arrow up from the face | drawn (liked) |
| revolve | profile in the tone above a dash-dot axis, curved arrow with a solid head | proposed (prototype exists) |
| fillet | block with the rounded corner (the only round corner), radius leader, right-angle head | drawn |
| chamfer | the same block with a 45° cut and a dimension leader | proposed |
| mate | peg (in the tone) over a slot (decided by cadatomic) | drawn |
| section | iso box with the cut face hatched | drawn (liked) |
| plane / axis | iso parallelogram in the tone / dash-dot line with a solid end | proposed |
| cut variants | the feature icon plus the `remove` badge | drawn (extrude+remove) |
| trace | two pads joined by a 2px 45° trace | drawn (liked) |
| via | solid ring with a trace entering | drawn (liked) |
| footprint | body with pad rows, pin-1 dot at ≥16 | drawn (liked) |
| net | three hollow pin rings joined by wires at a solid junction dot | drawn (new) |
| net label | flag on a wire (prototype) | proposed |
| copper pour / layer stack | pour in the tone with thermal reliefs / solid copper and toned dielectric bands (prototype) | proposed (liked) |
| DRC / ERC | the checked thing plus the `check` badge | proposed |
| mesh | square domain, 2×2 structured triangles | drawn (liked) |
| load (force) | body in the tone, solid arrow onto it | drawn (liked) |
| fixed support | body in the tone on a hatched ground (prototype) | proposed (liked) |
| result | banded fringe from one corner (.2 / .45 / 1) | drawn |
| probe | crosshair ring plus a callout (prototype) | proposed (liked) |
| solve | the mesh in the tone plus the run shape | proposed |

## 10. Lint rules for IC1

These are the checks for IC1 to implement in `tools/icons/lint`. `src.py`'s `check()` is a prototype of
L2–L6, and all 66 samples pass it.

| Id | Check |
|---|---|
| L1 | root attributes as in §3.1 for the file's size directory (12, 14, 16, 20, 24); `stroke-width` per §5.1 |
| L2 | elements and attributes from the allowlist; no `transform`, `style`, `<style>`, `<text>`, `<image>`, gradients, filters or `xlink` |
| L3 | colours: `currentColor` or `none`; `#000`/`#fff` only inside `<mask>` |
| L4 | `class` ∈ {`subject`, `accent`, `badge`}; `accent` only on allowlisted ids; `badge` only in build output; classed elements are fill-only or stroke-only, or an explicitly allowed `subject` fill with an outline (pending the §6 usvg test) |
| L5 | opacities: `.4` on `.subject` only; `.2`/`.45`/`1` only in `sim.result*`; no `rx`/`ry` on `rect` |
| L6 | half-pixel rule: for each stroke, horizontal and vertical segments, `rect` edges and circle extremes lie on `.5` (widths 1, 1.25, 1.5, 3) or on integers (width 2); fill-only edges lie on integers |
| L7 | diagonals 45° or 2:1 (tolerance 0.01), except arcs, circles and ids flagged `free_angles` |
| L8 | live area: the stroke bounding box (plus half the width) within the pad of §5.1, except fills marked as arrowheads or badges, and except the main shape at 12 |
| L9 | stroke widths: the size's base, copper (2 at 16, 3 at 24), or hatch 1 inside a `clipPath` group |
| L10 | gaps and counters: parallel stroke centres at least the §5.1 minimum apart (a geometric check over horizontal and vertical segments; diagonals *(review)*) |
| L11 | arrowheads: a 3-point solid path whose sides are at 45° to its axis and whose half-base matches §5.1 |
| L12 | required masters per §5.2 from the id manifest; a missing 24 for a command-strip id is an error |
| L13 | file size ≤ 1.5 KB (the samples are 0.2–0.9 KB) |
| *(review)* | metaphor, the T2 cut on the right frames, the tone on the right part, legibility at 1x: the human at contact-sheet level (`ICONS.md` I2) |

## 11. The samples

24 icons. All 24 have 16 and 24 masters; 6 of them also have 12, 14 and 20 masters. Composed badges are
`file+add`, `folder+add`, `extrude+remove`, `sketch+warning`, `footprint+link` and `file+lock`. Each was rendered
at 16 and 24, 1x and 2x, in both themes, and enlarged ×5 for pixel review, in three passes.

| Pass | Seen in the render | Change |
|---|---|---|
| v1 | `net` end rings sat on integer centres and blurred vertically; the 8-tooth gear's diagonal teeth turned to noise at 16@1x; the outlined pencil read as a lozenge; sheet sections overlapped | rings moved to .5 centres with r 2 (16) and r 3 (24); 6 teeth; solid pencil body with an open cone |
| v2 | `extrude` at 12 was a blob and at 14 the slab's thickness was noise; the 14 warning's "!" was too thin | at 12 a solid subject with the arrow clear of it; at 14 no thickness; warning solid at ≤14 |
| v3 | final check of the full sheet | none needed |

| Icon | Verdict | Note |
|---|---|---|
| file, search, close, chevron, add, run, save, undo, warning | good | |
| folder | acceptable | the cut makes it a little like a tag at 16@1x; this is O2's question |
| settings | acceptable | the 6-tooth gear is clean, but less familiar than codicons' 8 teeth |
| debug | good | |
| sketch | acceptable | better than the prototype; still the busiest MCAD icon |
| extrude, fillet, mate, section | good | extrude and section kept as liked; fillet's head is now T3 |
| trace, via, footprint | good | unchanged from the liked prototype |
| net | acceptable | a new metaphor (the prototype only had net-label); it can look like a topology diagram |
| mesh, load, result | good | load's head is now T3, a little blunter than the prototype's |

Tally: **19 good, 5 acceptable, 0 weak.** Sheet §8 has before and after (×6) for the five icons that changed
most from the prototype: extrude, fillet, sketch, load and net.

## 12. Open choices for the user

Each is a strip in sheet §7, at 16@1x and 16@2x in both themes.

| # | Choice | Options | Recommendation |
|---|---|---|---|
| **O1** | **Line terminals** | **A round** (prototype, codicons) · B square | **A.** At 16@1x the two differ by half a pixel of antialiasing. At 2x, square caps look more technical, but they lengthen every line by .5px, which breaks the keylines of close, add and chevron, and their 45° ends look cut off. Round stays compatible with codicons while the sets are mixed |
| **O2** | **Corners** | **A the cut on every object or container frame** · B the cut on documents only (file, save); other frames have a 1px radius · C rounded corners plus a dog-ear fold (codicons' look) | **A.** It is the most visible of the three traits and makes chrome ours. B is the safe fallback if the folder looks too much like a tag. C gives up T2 |
| **O3** | **Arrowheads** | **A solid 90°** (half-base = length) · B solid narrow (1:2) · C open V | **A.** It is the most legible at 12–16, its edges are crisp, and it matches the run triangle. B is more elegant at 24 but thin at 16, and C is codicons' look |

## 13. What this changes elsewhere

- **`ICONS.md` §3, superseded where they differ:**
  - the arrowhead is solid on every arrow (§3.4 had solid heads only for loads);
  - UI frames get the T2 cut instead of radius 1;
  - **20 is now a master.** §3.1 had said it was not; the user's optical-size request and fractional zoom (§5.3)
    reverse that research recommendation (it was not a user decision);
  - 12 and 14 masters are added;
  - colour slots (§6) are added;
  - the badge knockout is the whole cell.
- **IC1 (`tools/icons/`)**: lint L1–L13; the `sheet` tool gains the optical-size and zoom strips (`sheet.py`
  here is a starting point); `build` composes badges with the cell mask and applies the HC tone substitution.
- **ICR1 / AS1**: Z1 snapping, Z2 master choice, the `(id, P, colours)` cache key, slots via `color`, and no
  multiply when a slot is set.
- **IC2–IC4 (cadatomic)**: draw to this guide. The generator prompt in `ICONS.md` §4.1 needs the three traits,
  §5.1's per-size table and the class names added.

## Sources

- `plans/research/ICONS.md`, `plans/research/icons-proto/`, `plans/research/DESIGN.md` (§1.5, §2, §2.6, §4.5).
- egui 0.37.0 `src/gui_zoom.rs` (`zoom_in`: `zoom_factor += 0.1`; range 0.2–5.0); emath 0.37.0
  `src/gui_rounding.rs` (`GuiRounding::round_to_pixels`).
- Codicons, the comparison renders on the sheet: <https://github.com/microsoft/vscode-codicons> (`src/icons/`),
  © Microsoft Corporation, CC BY 4.0. They are not committed and are fetched for rendering only.
- Contrast ratios: WCAG 2.x relative luminance, computed for the tone over each surface in §4.
