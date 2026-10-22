# ICONS: an icon library and visual language for cadatomic (chrome, MCAD, ECAD, simulation)

Status: research, 2026-10-22. Branch `research/icons`. Not a plan of record. No Rust was built and no cargo was run.
The prototype in `plans/research/icons-proto/` was drawn and rendered for this document.

The request (user): "We should consider what icon library we will be using. How hard is it to make our own with
AI? We have a lot of stuff here in the ui from ecad to mcad to simulation so the icon visual language will be
important to get right."

Inputs:
- svelters: `DESIGN.md` §2 (affordance rules R1–R18), §4.5 (icons), §5 (DS5) and §6 (VIS1); `ASSETS.md` §2.3
  (`SvgLoader`, tint) and AS1; `CADATOMIC.md` SVP6; WG1's `icon` kind.
- cadatomic, read only: `docs/plan/00-decisions.md` (product direction, U1–U6), `docs/plan/01-overview.md`,
  `docs/plan/02-phase-a.md` (licences file), `docs/research/2026-10-22-ide-shell-and-ux.md` (§1 domains, §2 shell
  layout, §3 what to keep), `…-component-framework.md` (command buttons take their icon from the registry),
  `…-widget-survey-and-native-acceptance.md` (selection filter, ribbon), `…-command-architecture.md`,
  `crates/cadatomic-app/src/theme/icons.rs` (33 Phosphor glyphs in use today).
- Library repositories, read through the GitHub API on 2026-10-22: licence files, icon counts from the git trees,
  sample SVGs. epaint 0.37.0, egui_extras 0.37.0 and resvg 0.45.1 from the local cargo registry.

Marks: **[repo]** means checked in the library's repository today. **[unverified]** means from memory or a
search excerpt and not opened.

## 0. Summary

1. **Base library: codicons** for the IDE chrome (DESIGN Q3 stands). Its 16px, roughly 1px-weight style is the
   look the rest of the set has to match. No generic library has CAD tool icons. The best permissive one is
   Iconoir (MIT), which has about 15 3D-modelling glyphs. Tabler (MIT) has 19 schematic symbols. They are
   drawn on 24px grids with 1.5–2px strokes, and at 16px they render softer and lighter than codicons
   (contact sheet).
2. **Inventory: about 460 icons**, and generic libraries cover about 170 of them. Codicons covers nearly all of
   the chrome. The domains need **about 290 custom 16px drawings**, plus about 120 24px masters for the command
   strip (§1).
3. **The custom domain set is drawn on codicons' grid**: 16px masters with a 1px stroke on half-pixel centres,
   24px masters with a 1.5px stroke on half-pixel centres. It is monochrome, tinted by the theme, with one
   secondary tone (40% opacity) that marks the subject of an operation. Each domain has its own metaphor
   rules (§3).
4. **AI-drawn SVG works for this kind of icon.** I hand-wrote 20 domain icons and 6 24px masters, rendered them
   at 16 and 24, at 1x and 2x, and iterated twice. After the two iterations, 11 are good, 8 are acceptable and
   1 (mate) needs a human decision on metaphor. In v1, four of the 20 had a misread metaphor, which only showed
   up in the render: revolve read as an anchor, mate as a rubber stamp, the mesh was noise, and the
   coincident constraint read as a straight line. **The render-and-look loop is essential, and judging
   metaphors is the human's job** (§4). Contact sheet: `plans/research/icons-proto/contact-sheet.png`.
5. **Packaging: SVG through AS1** for the domain set. A font cannot carry the 40% tone or the badge knockouts,
   and resvg rasterises at the exact pixel size. Codicons stay a font through WG1. Ids are semantic
   (`mcad.extrude`, `status.error`), and a per-theme table maps each id to a glyph or an SVG (DS5) (§5).
6. **Lanes**: DS5 (extended), IC1 (toolchain: lint, sheet, build), then the cadatomic-owned waves IC2 (MCAD,
   selection, status, document kinds), IC3 (ECAD) and IC4 (simulation). **cadatomic owns the final choice**;
   this document is a recommendation to them.

## 1. Inventory

Counts are rough and come from the shell layout (title, command strip, docks, editor header, bottom panel,
status bar), the domain lists in the shell research (§1, §3) and the usual tool sets of SolidWorks, Onshape,
Fusion, KiCad, Altium and COMSOL. "Generic" means a stock icon in some library carries the same meaning.

### 1.1 By group

| # | Group | Contents | ~Count | Generic covers | Custom |
|---|---|---|---|---|---|
| 1 | **IDE chrome** | files and explorer (new, open, save, save all, close, folder, reveal); version control (branch, commit, merge, proposal, review, diff, history, sync, hub); search and palette (search, replace, filter, regex, case, whole word); run, debug and jobs (run, stop, pause, step, queue, remote worker, cloud, local); panels and layout (side bars, bottom panel, split, maximise, float, pin, close, overflow, gripper, chevrons); editing (undo, redo, cut, copy, paste, zoom); notifications, account, settings, extensions, agent | 110 | **~100, codicons** (all VS Code verbs; `git-pull-request*`, `remote`, `server`, `cloud`, `sparkle`, `gripper`, `layout-*`) | ~10: hub proposal and review states, agent proposal, worker placement |
| 2 | **Document kinds** | part, assembly, drawing, schematic, board, symbol library, footprint library, stackup, rules, simulation case, saved run, campaign, Foundry part, proposal, kernel dump | 16 | codicons `file` only | 16 |
| 3 | **MCAD** | sketch (line, arc ×3, circle ×2, rectangle ×3, ellipse, spline, point, slot, polygon, construction, trim, extend, offset, mirror, project, text, dimension) ~25; constraints (coincident, horizontal, vertical, parallel, perpendicular, tangent, equal, concentric, midpoint, symmetric, fix, collinear, curvature, pierce, distance, angle, radius) ~17; features (extrude, revolve and their cuts, sweep, loft, boundary, fillet, chamfer, shell, draft, rib, hole, thread, mirror, linear/circular/curve pattern, union/subtract/intersect, split, thicken, move face, delete face, replace face, offset) ~28; reference geometry (plane, axis, point, coordinate system, mate connector) ~6; assemblies and mates (insert, in-context part, fix, fastened, revolute, slider, planar, cylindrical, ball, parallel, tangent, gear, explode, interference, motion) ~16; measure and inspect (distance, angle, area, mass, curvature, zebra, draft analysis, deviation) ~10; views and display (section plane, clipping, the view cube's 7 views, perspective/ortho, shaded, edges, wireframe, hidden lines, x-ray, fit, normal to) ~20; kernel inspection (R5 bodies, 2D arrangements, SSI lanes, p-curves, decision ledger, Parasolid diff) ~8 | 130 | ~25: booleans (Lucide `squares-*`, Phosphor `unite`/`subtract`/`intersect`), Iconoir's 3D series (`extrude`, `fillet-3d`, `loft-3d`, `sweep-3d`, `pipe-3d`, `select-{point,edge,face}-3d`, `box-3d-*`), ruler, zoom, axes, cube, cylinder, perspective | ~105 |
| 4 | **ECAD** | schematic tools (place symbol, wire, bus, bus entry, junction, no-connect, net label, global and hierarchical labels, sheet, power port, annotate, ERC, netlist, BOM, cross-probe) ~20; symbol categories for libraries and filters (resistor, capacitor, inductor, diode, LED, transistor, IC, connector, ground, power, crystal, fuse, switch, op-amp, transformer, battery) ~16; PCB tools (route, differential pair, length tune, via kinds, track, arc track, pad, footprint, copper pour, keepout, cutout, board outline, mounting hole, fiducial, silk text, dimension, 3D view, shove, autoroute, fanout, teardrop, stitching, ratsnest, net highlight, DRC, clean-up, length match) ~40; layers (stackup, top, bottom and inner copper, silk, mask, paste, fab, courtyard, edge cuts, visibility, layer pair) ~12; rules and constraint manager (net class, rule, clearance, width, impedance) ~8; outputs (Gerber, drill, pick and place, BOM, IPC-2581, STEP) ~6 | 100 | ~20: Tabler's 19 `circuit-*` symbols cover about 14 of the 16 categories; codicons `circuit-board`; layers; ruler | ~80 |
| 5 | **Simulation** | model tree (geometry, materials, physics, mesh, study, results); mesh (mesh, refine, size field, boundary layer, quality, element kinds); materials; loads and boundary conditions (fixed, pinned, roller, displacement, symmetry, contact, force, pressure, moment, gravity, temperature, heat flux, convection, radiation, voltage, current, terminal); solver and jobs (solve, queue, worker, sweep or campaign, convergence, residuals, stop); probes and results (point, line and surface probes, contour, vectors, streamlines, isosurface, deformed shape, animate, compare runs, legend, XY plot, time step) | 60 | ~10: thermometer, magnet, gauge, chart-line, wave-sine, beaker, play | ~50 |
| 6 | **Status and severity** | error, warning, info, hint, success, running, queued, stale (inputs changed), held result, partial result, sync state, locked or computed (R5), dirty, offline or remote, proposal pending | 20 | ~16, codicons | ~4 badges: stale, held, partial, remote |
| 7 | **Selection filters** | MCAD: vertex, edge, face, body, sketch entity, plane, axis, component, mate; ECAD: symbol, wire, net, pin, footprint, pad, track, via, zone, text, graphic, dimension; simulation: node, element, region | 24 | 3 (Iconoir `select-*-3d`) | ~21, mostly reused from groups 3–5 |
| | **Total** | | **~460** | **~170** | **~290 at 16px**, plus ~120 24px masters for the command strip |

### 1.2 What cadatomic uses today

`theme/icons.rs` defines 33 Phosphor glyphs. Of these, **23 map to codicons** directly:

`ARROW_COUNTER_CLOCKWISE`→`discard`, `ARROW_CLOCKWISE`→`redo`, `CARET_*`→`chevron-*`, `CHECK`→`check`,
`CHECK_CIRCLE`→`pass`, `CIRCLE_NOTCH`→`loading`, `CORNERS_OUT`→`screen-full`, `FILE`→`file`, `INFO`→`info`,
`LIST_CHECKS`→`checklist`, `MAGNIFYING_GLASS`→`search`, `QUESTION`→`question`,
`SIDEBAR_SIMPLE`→`layout-sidebar-left`, `SLIDERS_HORIZONTAL`→`settings`, `TREE_STRUCTURE`→`type-hierarchy`,
`WARNING`→`warning`, `X`→`close`, `X_CIRCLE`→`error`, `CIRCLE`→`circle-large`, `DOT`→`circle-small-filled`,
`SQUARE`→`primitive-square`, `STACK`→`layers` (all checked in the codicons tree **[repo]**).

The **10 that are domain icons** become custom ids: `CUBE` (body), `CUBE_FOCUS` (fit to selection),
`CUBE_TRANSPARENT` (x-ray), `CYLINDER`, `UNITE` (boolean), `SHAPES`, `LINE_SEGMENT` (sketch line),
`CURSOR_CLICK` (select tool), `STETHOSCOPE` (kernel inspector) and `COMMAND`.

## 2. Library comparison

| Library | Licence | Count (2026-10-22) | Grid | Stroke and style | Weights / variants | Domain coverage | SVG / font |
|---|---|---|---|---|---|---|---|
| **Codicons** (Microsoft) | **CC-BY-4.0** for the icons [repo SPDX], so attribution is required | **658** SVGs **[repo]** | 16 (6 of 49 sampled now use a 24 viewBox: `files`, `settings-gear`, `source-control`, `debug-alt`, `terminal`, and `output` at 24×25 **[repo]**; they still render cleanly at 16) | filled outline shapes, ~1px features, rounded terminals | one | full IDE set; `circuit-board`, `graph-line`, `beaker`, `pulse`, `symbol-ruler`; no CAD | both (TTF plus a name mapping) |
| **Phosphor** | MIT [repo] | **1,512** per weight **[repo]** | 256 viewBox (designed at 16/24 [unverified]) | stroke 16/256 (1.5 at 24), round | 6: thin, light, regular, bold, fill, duotone | `unite`/`subtract`/`intersect`, `cube`, `cylinder`, `circuitry`, `perspective` | both (cadatomic vendors the regular TTF) |
| **Lucide** | **ISC**, plus MIT for the icons that came from Feather [repo LICENSE] | **1,903** **[repo]** | 24 | stroke 2, round caps and joins | one (stroke is a parameter) | `axis-3d`, `rotate-3d`, `move-3d`, `scale-3d`, `cylinder`, `squares-unite/subtract/intersect`, `circuit-board`, `cable`, `spline`, `drafting-compass`, `radius` | both (font in `lucide-static` [unverified]) |
| **Tabler** | MIT [repo] | **5,184** outline + **1,054** filled **[repo]** | 24 | stroke 2 (adjustable), round | outline and filled | **19 `circuit-*` schematic symbols** (resistor, capacitor, inductor, diode, Zener, ground, battery, switch, motor and others), `mesh`, `section`, `perspective`, `cylinder`, `axis-x/y`, `layers-*`, `temperature` | both (webfont) |
| **Fluent UI System Icons** | MIT [repo] | **2,994** metaphors **[repo]**, each in several sizes | per-size masters: 10, 12, 16, 20, 24, 28, 32, 48 [unverified list] | ~1px at 16–20, 1.5 at 24 [unverified], rounded | regular, filled (some light) | `Cube*`, `Shape*` booleans, ruler, layers, beaker, flowchart | both |
| **Material Symbols** (Google) | Apache-2.0 [repo] | ~3,800 [unverified] | 24 (optical size 20–48) | variable | variable font: FILL 0–1, wght 100–700, GRAD, opsz; 3 styles | `deployed_code`, `view_in_ar`, `3d_rotation`, `polyline`, `memory`, `electrical_services`, `cable`, `thermostat` | both (variable TTF) |
| **Remix Icon** | **Remix Icon License v1.0** (January 2026, at v4.9.1) [repo]: free to use and modify, but it **forbids redistributing the icons as an icon pack or library** | **3,229** SVGs (≈1,600 line/fill pairs) **[repo]** | 24 | filled outline | line and fill | little (`sketching`, `compass`, `ruler`) | both |
| **Material Design Icons** (Pictogrammers) | Pictogrammers Free License: Apache-2.0 for the icons [repo] | **7,447** **[repo]** | 24 | filled; community-drawn, so styles vary | mostly one | `resistor`, `integrated-circuit-chip`, `axis-*-rotate-*`, `cylinder`, `vector-*`, `sine-wave`, `current-ac/dc` | both |
| **Iconoir** | MIT [repo] | **1,383** **[repo]** | 24 | **stroke 1.5**, round | regular and solid | **the best permissive MCAD set**: `extrude`, `fillet-3d`, `loft-3d`, `sweep-3d`, `pipe-3d`, `select-point/edge/face-3d`, `box-3d-*`, `arc-3d`, `ellipse-3d`, `cube-hole`, `cube-cut-with-curve`, `unjoin-3d`, `project-curve-3d`, `angle-tool`, `ruler-combine` | SVG (and a CSS set [unverified]) |
| Carbon (IBM) | Apache-2.0 [repo] | ~2,000 [unverified] | 32 (16/20/24 sizes) | filled outline | one | charts and some 3D | SVG |
| **FreeCAD** (domain) | the repo is **LGPL-2.1+**; the icon SVGs carry author metadata and link to the wiki's Artwork page [repo] | **~1,650** SVGs under `Resources/icons` **[repo]** | 64, full colour, Tango-derived | colour gradients | — | complete: PartDesign, Sketcher constraints, Assembly, FEM (loads, BCs, mesh, solvers, results) | SVG |
| **KiCad** (domain) | **CC-BY-SA 4.0**: "KiCad icons were redesigned in 2020 by Aleksandr Zyrianov … License: CC-BY-SA 4.0" (`resources/bitmaps_png/sources/CREDITS` **[repo]**). Older icons were GPL. | **703** SVGs in the light set **[repo]**, plus a dark set | 24 and 16 [unverified] | two-tone colour | light and dark | complete ECAD | SVG (built to PNG) |
| Blender (domain) | CC-BY-SA 4.0 according to the designer; the project's own statement is GPL [unverified, devtalk thread] | ~900 [unverified] | 16 (in 2.8+) | white glyphs with colour accents | — | mesh and modelling ops | SVG sheet |
| Horizon EDA, LibrePCB, SolveSpace | GPL-3.0 (repo licence; the icons have no separate licence) [repo] | — | — | — | — | EDA and constraint sketching | — |

Findings:
- **No permissive library covers MCAD, ECAD and simulation tools.** Iconoir (MCAD) and Tabler (schematic
  symbols) are the only real starts, and both use a 24-grid stroke style that is softer at 16 than codicons
  (contact sheet: compare the right-hand set in each row at 16@1x).
- **FreeCAD (LGPL) and KiCad (CC-BY-SA) are references for metaphors, not sources.** Tracing or redrawing their
  geometry makes a derivative under their licences. A CC-BY-SA icon font would itself be CC-BY-SA. Their
  full-colour styles don't match monochrome chrome anyway. Conventions such as a hatched ground for a fixed
  support, or the section hatch, are not copyrightable, so we use those freely.
- **Remix is out.** Its 2026 licence forbids redistributing the icons as an icon library, and an icon theme for
  extension authors is close to that.
- **Codicons is the only base whose geometry is native to 16px**, which is VS Code's icon size and DESIGN's
  `icon-size`.

## 3. Visual language (spec for cadatomic's domain set)

Every rule below can be checked by a machine (IC1 lint) unless it is marked *(review)*.

### 3.1 Geometry

| Item | 16 master | 24 master | Why |
|---|---|---|---|
| Use | trees, lists, menus, editor header, selection filter, status bar, small toolbars, tabs | command strip (large tool buttons), task panel headers, empty states; 32/48 tiles use the 24 master scaled ×2 | Codicons are 16. CAD tool strips use larger buttons. |
| viewBox | `0 0 16 16` | `0 0 24 24` | — |
| Live area | 14×14 (1px padding); arrowheads and badges may touch the edge | 20×20 (2px padding) | Same as codicons |
| Stroke | **1px**, centred on **.5** coordinates | **1.5px**, centred on **.5** coordinates | Tested (§4.3): 1.5 on integer centres blurs into two grey pixels; on .5 centres it gives one solid pixel with a faint fringe. 1/16 = 1.5/24, so the two masters weigh the same. |
| Copper traces, spokes | 2px on integer centres | 3px on .5 centres | Copper must read heavier than wire or outline |
| Caps and joins | round; **butt** for dashed lines and for strokes that end in a pad or edge | same | Matches codicons' rounded terminals and the Iconoir and Lucide neighbours |
| Corner radius | 0 on engineering geometry (parts, pads, boards, frames); 1 on UI frames (callouts) | 0 / 1.5 | Sharp corners say "precise geometry", and a fillet stays meaningful |
| Optical size | squares up to 12–13 wide; circles 13–14 across; iso slabs use the full 12 width | ×1.5 | Squares look larger than circles of the same width |
| Diagonals | 45° and **2:1 pixel isometric** (dx 2 : dy 1) only | same | Both rasterise into clean steps at 1x. True 30° lines alias. |
| Not allowed | `transform`, text, raster images, gradients, filters, strokes under 1px (hatching is the exception at 1px), colours other than `currentColor` | same | resvg and tint (ASSETS §2.3) |
| 20px | **not a master.** egui rows are 18–22px and codicons are 16. A third master would double the work for a size no base library uses there. | | Fluent's 20 default is a Windows-shell convention |
| Hinting | none needed: SVG is rasterised at the exact pixel size, and the half-pixel rule makes it crisp at 1x and 2x. At 1.5x scale nothing is crisp, which is accepted. | | |

### 3.2 Fill, tone and badges

| Element | Treatment |
|---|---|
| Default | outline, `stroke="currentColor"`, `fill="none"` |
| **Secondary tone** | `fill-opacity=".4"` (or `opacity=".4"` on strokes): **the subject or input of an operation** (the profile being extruded, the part being placed, the body under load), large areas (copper pours, dielectric), and the geometry a constraint relates. It is never used for state. |
| Solid fill (100%) | copper features (pads, vias), arrowheads of loads, constraint markers, the run triangle, badges |
| Allowed opacities | exactly 1 and .4. The only exception is the results fringe (.2 / .45 / 1). |
| Badges | modifiers from a closed list (`add`, `remove`, `run`, `check`, `error`, `settings`, `link`, `lock`, `queued`, `stale`), placed in the **bottom-right 8×8 at 16** (12×12 at 24) with a **1px knockout**: a `<mask>` removes the base under the badge plus 1px. Badges are composed at build time (IC1), not hand-drawn per icon. |
| Hatching | 45°, 1px, pitch 3–4px, clipped: section cuts and fixed supports only |
| Centre lines | dash-dot `4 1 1 1` (16) / `6 1.5 1.5 1.5` (24), butt caps: axes of revolution and symmetry |

### 3.3 State, mode and colour (DESIGN §2)

| Need | How | Rule |
|---|---|---|
| Active tool, toggled option | the **button** shows it (pressed fill, `inputOption` border, accent marker), and the icon stays the same | R1, R2; VS Code's toggles |
| Current mode (in a sketch, editing in context) | icon tinted `accent` in the command strip, plus a status-bar chip | — |
| Disabled | the whole control at 40% with a reason tooltip. The tone becomes 16%, which is acceptable because the tone is never the only cue. | R9 |
| Semantic state pairs | separate ids: `eye`/`eye-closed`, `lock`/`unlock`, `pin`/`pinned`, `layer.visible`/`layer.hidden` | — |
| Colour | **monochrome, tinted** by the `icon` token (or `fg`, `disabled-fg`, `accent`). Severity ids (`status.error` and so on) are tinted by their token (`error`, `warning`, `info`, `success`). | DESIGN §4.5 |
| Data colours (layer, net, results colour map) | **never inside an icon**; shown as a swatch chip beside it (layer manager, net list, legend). The icon works in any theme, and the data colour stays the user's. | — |
| Non-tinted multi-colour | only extension brand icons and file-type brands (`tint=false`, ASSETS) | — |
| Meaning | an icon is never the only signal: every icon-only control has the catalog's accessible name and a tooltip | cadatomic `icons.rs` rule; R9 |

### 3.4 Metaphor rules per domain

| Domain | Projection | Metaphors |
|---|---|---|
| **MCAD** | solids in **2:1 isometric**; sketch entities flat 2D | Sketch points are hollow 3×3 squares. Operations are the **subject in the tone plus the action at 100%** (extrude: profile slab and up arrow; revolve: profile, centre line and curved arrow; fillet: rounded corner and radius leader). A cut variant is the same icon with the `remove` badge. Constraints show **the geometry at 40% and the relation glyph at 100%** (the dot for coincident, `∥`, `⊥`, the tangent arc). Sections use the hatch. Mates show the parts in side view, the moving part in the tone. |
| **ECAD** | **top view only**; 90° and 45° only | **Copper is filled** (pads and vias at 100%, pours at 40%). Traces are 2px. Silkscreen, courtyard and board outline are 1px strokes. Schematic wires are 1px, junction dots 3px, and symbols follow **IEC 60617** shapes by default, with an ANSI/IEEE 315 icon theme as a mapping swap (zigzag resistor). Labels are flags. Checks (DRC, ERC) are the checked thing plus the `check` badge. |
| **Simulation** | flat 2D, as in FE textbook diagrams | The body is in the tone. Supports use structural notation: hatched ground (fixed), triangle (pinned), circles (roller). Loads have solid arrowheads: one arrow (force), an arrow row (pressure), a curved arrow (moment), wavy arrows (convection). Electrical BCs **reuse the ECAD terminal and ground glyphs**. Results use the **banded fringe**. Probes are a crosshair plus a callout. Solving is the subject plus the codicons-shaped `run` badge. |
| **Shared** | — | One arrowhead, one centre line, one hatch, one point marker and one pad, defined once in the generator's primitives, so that MCAD, ECAD and simulation icons read as one family, and the projection keeps them apart: iso means MCAD, top view means ECAD, flat diagram means simulation. |

## 4. AI-made icons, tested

### 4.1 The spec as a generator prompt

This is what IC2 feeds the agent with each batch. The ids, the inventory rows and the neighbours vary per batch.

```text
You draw monochrome UI icons for cadatomic, an MCAD + ECAD + simulation IDE whose chrome uses VS Code's
codicons. Draw each requested icon id as a standalone SVG, written by hand (no tools, no tracing, no copying of
any third-party icon's geometry; FreeCAD and KiCad icons are licensed and must not be reproduced).

Hard rules (a linter enforces them):
- 16 master: <svg viewBox="0 0 16 16" width="16" height="16" fill="none" stroke="currentColor"
  stroke-width="1" stroke-linecap="round" stroke-linejoin="round">. Live area 1..15.
- 24 master (only if asked): viewBox 0 0 24 24, stroke-width 1.5. Live area 2..22.
- Every stroke centre lies on a .5 coordinate (x.5) so lines are crisp at 1x and 2x. 2px copper strokes lie
  on integer coordinates. Diagonals are 45 degrees or 2:1 (dx 2 : dy 1).
- Colours: currentColor only. Opacity: 1 or .4 only (fill-opacity=".4" = the secondary tone).
- No transform, text, image, gradient, filter, or style attribute. clipPath and mask are allowed.
- Badges are not drawn by hand; request them as `badge: <name>` and the build composes them.

Meaning rules:
- The subject or input of an operation is drawn in the .4 tone; the action (arrow, radius leader, relation
  glyph) is at 100%.
- MCAD: solids in 2:1 isometric, sketch geometry flat, sketch points as hollow 3x3 squares, constraint
  geometry at .4 with the relation glyph at 100%, section cuts hatched at 45 degrees.
- ECAD: top view only, 90/45 degrees; copper filled (pads and vias 100%, pours .4); traces 2px; outlines 1px;
  schematic symbols in IEC 60617 shapes.
- Simulation: flat 2D; body in .4; fixed = hatched ground, pinned = triangle, roller = circles; force =
  solid-headed arrow; results = banded fringe (.2/.45/1).
- One idea per icon; at most 3 distinct elements at 16px. Prefer the convention engineers already know.

For each icon output: the id, one line naming the metaphor, the SVG. Then render the batch with
`tools/icons/sheet` and Read the PNG. For every icon answer: (1) at 16@1x, what would a CAD/EDA user call it?
If the answer is not the id's meaning, redraw. (2) Does it look like a neighbour (anchor, stamp, key, gift
box...)? (3) Is its weight equal to the codicons in the toolbar strip? (4) Are all its strokes crisp at 16@1x?
Iterate at most 3 times, then list the icons you are unsure of for human review.
```

### 4.2 What was drawn

20 icons were drawn as 16 masters, and 6 of them also as 24 masters (marked in the sheet). All are hand-written
SVG (`icons-proto/icons_src.py` holds the source strings, `16/` and `24/` the emitted files). They were
rendered by `icons-proto/render.py`, which runs rsvg-convert 2.63 (`resvg` is not installed here, and egui uses
resvg 0.45.1) and Pillow, with `currentColor` replaced by Dark Modern and Light Modern `icon.foreground`.
Each row shows **16@1x, 24@1x, 16@2x and 24@2x** next to the closest base-library icon, under a toolbar strip
that mixes codicons chrome with the prototypes.

- `icons-proto/contact-sheet.png` (232 KB): the final version (v3), dark and light.
- `icons-proto/pixels-v1.png` and `pixels-v3.png`: 16@1x and 24@1x upscaled ×6, before and after iterating.

### 4.3 Results (honest reading of the PNGs)

| Icon | v1 problem (seen in the render) | v3 | Verdict |
|---|---|---|---|
| `mcad.extrude` | the arrow crossed a narrow 8px box and read as a gift box | wide slab, profile in the tone, arrow from the face | good |
| `mcad.revolve` | vertical axis, curved arrow underneath and the profile read as an **anchor** | horizontal centre line, profile above it, curved arrow on the right | acceptable (the centre line is faint at 16@1x) |
| `mcad.fillet` | — | — | good |
| `mcad.sketch` | — | — | acceptable (the pencil is the weakest shape; organic shapes are the LLM's weak point) |
| `mcad.constraint.coincident` | the two lines were nearly collinear and read as one line with a dot | 45° line plus a horizontal line, dot at the joint | acceptable (it needs its 16 siblings to be judged as a family) |
| `mcad.mate` | read as a **rubber stamp** (v1), then a pedestal or chain link (v2) | a peg about to enter a slot (insert/fit) | **weak**: SolidWorks users expect a paperclip and Fusion users a joint glyph, so a human picks the convention |
| `mcad.section` | — | v2's ghost of the removed half was noise and was removed in v3 | acceptable (busy at 16@1x, clear at 2x) |
| `ecad.via` | — | — | good |
| `ecad.trace` | — | — | good |
| `ecad.footprint` | — | — | good |
| `ecad.net-label` | a junction dot at the wire end read as a key | dot removed | good |
| `ecad.copper-pour` | — | — | good (the most distinctive of the set) |
| `ecad.drc` | — | — | acceptable |
| `ecad.layer-stack` | — | — | good, and clearly distinct from codicons' `layers` |
| `sim.mesh` | the irregular mesh was noise at 16 | structured 2×2 triangle mesh; the 3×3 24 master was a grey blur, so it became 2×2 | acceptable |
| `sim.bc.fixed` | — | — | good |
| `sim.load.force` | — | — | good |
| `sim.probe` | — | — | acceptable |
| `sim.solve` | — | — | good (the mesh plus the run badge reads at once) |
| `sim.result.contour` | — | — | good |

Tally: **11 good, 8 acceptable, 1 weak.** Further observations:
- **The prototypes match the codicons' weight in the toolbar strip.** At 16@1x, the 24-grid libraries (Lucide,
  Tabler, Iconoir) come out lighter and softer. That is the reason to draw the domain set on codicons' grid
  rather than adopt one of them.
- **24 masters are needed wherever 24 is used at 1x.** The 16 master scaled ×1.5 is soft in every row without
  a 24 master. The first 24 masters used integer centres with a 1.5 stroke and blurred; half-pixel centres
  fixed it (rule §3.1).
- The 40% tone worked as the family signature in both themes, and it survives tinting, because tint multiplies
  the alpha.
- `mask` (badge knockout) and `clipPath` (hatch, fringe) render in rsvg, and resvg 0.45.1 implements both
  (`resvg-0.45.1/src/{mask,clip}.rs`). They are not checked in egui yet; IC1 adds a kittest snapshot for that.

### 4.4 What LLM-written SVG does well and badly

| Does well | Does badly |
|---|---|
| Exact grid arithmetic: half-pixel centres, 2:1 iso slopes and 45° traces all came out crisp the first time | **It can't see while drawing.** Overlaps and misreads (the anchor, the stamp, the key) appear only in the render. Without the loop, about 1 in 5 icons ships misread. |
| Consistent rules across a set: every icon used the same stroke, tone and caps without drift | Elliptical arcs and arrowhead placement on curves: the arc parameters needed working out, and the first revolve was wrong |
| Engineering conventions it knows by name (section hatch, hatched ground, thermal relief, IEC shapes, dash-dot centre lines) | Organic or hand-drawn shapes (pencil, hand, magnet, paperclip) are stiff |
| Small, diffable, reviewable files (300–550 bytes each), plus systematic variants (badges, cut versions, 24 masters) | Optical balance (the narrow first extrude) and the density limit at 16 (3×3 mesh) are judged from pixels only |
| Fast: v1 of 20 icons in one pass | Choosing between competing domain conventions (mate) is a product decision, not a drawing task |

**Effort, measured in this lane:** 20 16px icons, 6 24px masters, three versions and four contact-sheet reads
took about **30–40 minutes of agent time**, not counting the render tool, so roughly **1–2 agent-minutes per
drawing per pass**, and about 3 passes to converge. A human reviewer at contact-sheet level needs about
**30 seconds per icon per pass**. For the whole set (≈290 16px drawings plus ≈120 24px masters), that is about
**10–15 agent-hours over the waves and 6–8 hours of human review**, spread over IC2–IC4. Raster image models
(diffusion, then vectorising) are not worth trying: they don't produce pixel-snapped 1px geometry or a
consistent stroke, and tracing their output costs more than drawing directly.

**Answer to "how hard is it to make our own with AI?":** for CAD, EDA and simulation glyphs on a strict grid,
not hard, as long as the agent renders and looks after every pass and a domain human signs off on metaphors.
The spec and the lint keep the family consistent. The human's job is about 1 in 20 icons where conventions
compete, plus a final pass on the contact sheet.

## 5. Recommendation (to cadatomic, which owns the choice)

### 5.1 Libraries

- **Base: codicons** (font, CC-BY-4.0) for chrome, status and generic verbs. This is DESIGN Q3, and this
  research confirms it.
- **Custom domain set** for MCAD, ECAD, simulation, selection filters, document kinds and the status badges,
  drawn to §3 on codicons' 16 grid, with 24 masters for the command strip.
- **Phosphor** stays as a fallback **only during migration**. Once DS5 and IC2 cover the 33 glyphs in use
  (§1.2), cadatomic can drop the font, because its round 24-grid style is a third look.
- **Reference only, never copied**: Iconoir, Tabler and Lucide (MIT/ISC, so deriving with a notice would be
  allowed, but redrawing to §3 is cheaper than adapting), FreeCAD (LGPL), KiCad (CC-BY-SA) and Blender
  (CC-BY-SA). **Not used**: Remix (its licence restricts redistribution).

### 5.2 Semantic ids

- Ids are `<domain>.<group>.<name>`: `chrome.close`, `vcs.branch`, `status.error`, `doc.board`,
  `mcad.extrude`, `mcad.constraint.coincident`, `ecad.via`, `sim.bc.fixed`, `select.face`. Badge-composed ids
  get a suffix: `mcad.extrude+remove` (cut-extrude), `ecad.drc+check`.
- Ids come from the command registry (`Button::command` already takes its icon from there), the type registry
  (tree node kinds, selection filter kinds) and document kinds. The extension manifest declares ids, plus
  SVGs through AS3.
- **One table per icon theme** maps id → `{ glyph: (family, codepoint) }` or `{ svg: asset ref, sizes:
  [16, 24] }`. The fallback chain is theme → cadatomic set → codicons → Phosphor (while kept) → WG1b's
  missing box. This is DS5's table with an `svg` source added.
- IC1 generates a `.d.ts` union of the ids, so `icon="mcad.extrud"` is a type error in Svelte, and a Rust
  `const`/enum for Rust panels.

### 5.3 Packaging

| Option | For | Against | Verdict |
|---|---|---|---|
| **SVG through AS1** (`icon source`, `SvgLoader` with resvg 0.45.1 and tint) | keeps the tone, masks and clips; rasterised at the exact pixel size, so the half-pixel rule holds at 1x and 2x; hot reload through AS1; extensions use the same path | needs AS1 (4 lanes in ASSETS) | **domain set** |
| Generated icon font (fantasticon or similar) | works today through WG1's `icon` and in plain `RichText` | a glyph has one alpha, so **the tone and the knockouts are lost**. epaint 0.37 has a `color_fonts` feature for COLR (`font_face.rs`), but whether it tints a COLR foreground is unverified. Hinting (skrifa) may move 1px strokes. 16 and 24 masters need two fonts. | no |
| Codicons font | already a font, works through WG1 today | — | **chrome** |
| Before AS1 | cadatomic's Rust panels can use egui_extras' SVG loader (`egui::Image` plus `include_image!`) | its default `usvg::Options` render `currentColor` **black**, so tinting gives black (`egui_extras-0.37.0/src/loaders/svg_loader.rs`). IC1's build therefore emits **tint-ready copies with `currentColor`→`#fff`**, which is the same trick ASSETS' own loader does with a style sheet. | bridge |

### 5.4 Production and maintenance

1. **Spec** = §3, kept in `tools/icons/SPEC.md`, with the lint (IC1) as its executable half: viewBox, stroke
   widths, half-pixel centres (parse the paths, check the coordinates), allowed attributes and opacities, size
   budget.
2. **Generator prompt** = §4.1, plus a per-batch list of ids, inventory rows and neighbouring approved icons as
   style anchors.
3. **Render-and-review loop** (like VIS1): the agent draws, runs `tools/icons/sheet` (render.py generalised),
   Reads the PNG, answers the four questions per icon, and iterates at most 3 times. The sheet always includes
   the codicons toolbar strip.
4. **Human approval**: a domain human (the user, or whoever cadatomic names) approves each batch's contact
   sheet, marking each icon approve, redraw (with a note) or decide-convention. Approved ids are frozen.
   Changing a frozen icon needs a reason.
5. **Regression**: a kittest snapshot of every approved icon at 16@1x and 16@2x through the real egui path
   (resvg), so that a renderer or tint change is caught.
6. **Licence credits** in cadatomic's `assets/licenses.toml` and the About box: "Codicons © Microsoft
   Corporation, CC BY 4.0", Phosphor's MIT notice while it is kept, and the custom set's own licence.
   Recommendation: **CC-BY-4.0 or MIT for the custom set**, so that extension authors can match it. The
   project licence itself is a user decision (`02-phase-a.md`).

### 5.5 Lanes

| Lane | Owner | Content | Size | Needs |
|---|---|---|---|---|
| **DS5** (extended) | svelters | codicons font vendored with notice; generated name table; id → glyph/svg mapping with fallback chain; per-theme tables; `icons.d.ts` id union | 0.5 → **0.75** | WG1 (merged) |
| **IC1** icon toolchain | svelters | `tools/icons/`: `lint` (§3 rules), `sheet` (contact sheets and pixel zooms, dark/light, codicons strip), `build` (badge composition through masks, tint-ready `#fff` copies, id manifest and Rust consts), SPEC.md and the prompt; a kittest snapshot of an SVG icon through resvg with mask and clipPath | 0.5 | nothing (snapshot part: AS1) |
| **AS1** | svelters | as in ASSETS: `icon source`, `tint`, `SvgLoader` | 4 (planned) | — |
| **IC2** wave 1 | cadatomic (agent plus human approval) | ~70 icons: the 10 domain glyphs in use today, kernel inspection views, sketch and constraints, core features, booleans, views and display, selection filters, status badges, document kinds; 24 masters for the command strip's common groups | 1 lane plus ~1h of review | IC1; AS1 for Svelte panels (Rust panels can use the bridge) |
| **IC3** wave 2 | cadatomic | ECAD: ~80 custom plus an IEC/ANSI mapping | 1 lane | IC2's approved primitives; starts with ECAD (after the workspace rework) |
| **IC4** wave 3 | cadatomic | simulation: ~50 | 0.5–1 lane | IC2; starts with U6 (Sinbad case authoring) |
| cadatomic switch | cadatomic | replace `theme/icons.rs` consts with ids (23 to codicons, 10 custom); drop Phosphor when it is no longer referenced | 0.25 | DS5, IC2 |

Order: DS5 ∥ IC1 → IC2 (with AS1 landing meanwhile) → IC3 and IC4 as their domains start. SC1 needs none of
this.

## 6. Open questions for cadatomic (with recommendations)

| # | Question | Recommendation |
|---|---|---|
| I1 | Codicons for chrome (DESIGN Q3)? | Yes |
| I2 | Who approves icon batches? | The user or a named domain reviewer, at contact-sheet level; the agent never self-approves |
| I3 | Default schematic convention in icons: IEC 60617 or ANSI/IEEE 315? | IEC by default (KiCad's default symbol style [unverified]), with ANSI as an icon-theme swap |
| I4 | Mate metaphor: SolidWorks paperclip, Fusion joint glyph, or peg-in-slot (prototype)? | Decide with I2's reviewer; the prototype's peg-in-slot is the most self-explanatory, and the paperclip the most familiar to SolidWorks users |
| I5 | Licence of the custom set | CC-BY-4.0 or MIT, so extensions can match |
| I6 | Keep Phosphor after migration? | No: drop it when §1.2's glyphs are covered |

### Decided (main-cadatomic, 2026-10-22; recorded in cadatomic's register)

- I1: codicons for the chrome, yes (DS5 starts on it). I6: drop Phosphor once its glyphs are covered (main-cadatomic decided I1 and I6; the user deferred both to it).
- I2: the user approves each custom-icon batch at contact-sheet level; agents never approve their own icons.
- I3: IEC 60617 by default, ANSI as an icon-theme swap. I4: mate is peg-in-slot. I5: the custom set is CC-BY-4.0.
- cadatomic's IC2-IC4 and the Phosphor switch-over start after DS5 and IC1.

## Sources

- Codicons: <https://github.com/microsoft/vscode-codicons> (licence CC-BY-4.0, `src/icons/`)
- Phosphor: <https://github.com/phosphor-icons/core> · Lucide: <https://github.com/lucide-icons/lucide>
  (`LICENSE`) · Tabler: <https://github.com/tabler/tabler-icons> · Fluent:
  <https://github.com/microsoft/fluentui-system-icons> · Material Symbols:
  <https://github.com/google/material-design-icons> · Remix: <https://github.com/Remix-Design/RemixIcon>
  (`License`, v1.0 January 2026) · Pictogrammers MDI: <https://github.com/Templarian/MaterialDesign> ·
  Iconoir: <https://github.com/iconoir-icons/iconoir> · Carbon: <https://github.com/carbon-design-system/carbon>
- FreeCAD: <https://github.com/FreeCAD/FreeCAD> (LGPL-2.1, `src/Mod/*/Gui/Resources/icons`),
  <https://wiki.freecad.org/License>
- KiCad: <https://github.com/KiCad/kicad-source-mirror/blob/master/resources/bitmaps_png/sources/CREDITS>,
  <https://gitlab.com/kicad/code/kicad/-/raw/master/LICENSE.README>, <https://www.kicad.org/libraries/license/>
- Blender icons licence discussion: <https://devtalk.blender.org/t/license-for-blender-icons/5522>
- Horizon EDA, LibrePCB, SolveSpace: their GitHub repositories (GPL-3.0)
- Local: `epaint-0.37.0/src/text/font_face.rs` (skrifa outlines, vello_cpu, `color_fonts`),
  `egui_extras-0.37.0/src/loaders/svg_loader.rs` (default `usvg::Options`), `resvg-0.45.1/src/{mask,clip}.rs`
