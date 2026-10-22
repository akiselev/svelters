# tools/icons: the icon toolchain (IC1)

The style is `plans/research/ICON-STYLE.md` (IC0). This file says how the tool reads and enforces it. The
numbers live in `style.toml`, and nowhere else.

```sh
tools/icons/icons.py lint  <set>                      # ICON-STYLE §10 L1-L13; exit 1 on any finding
tools/icons/icons.py build <set> <out> [--only ids]   # badges, HC tone, white copies, manifest, id code
tools/icons/icons.py sheet <set> <out.png> --codicons <vscode-codicons/src/icons> [--zoom <zoom.png>] [--sizes 12,16]
tools/icons/icons.py hash  <set> [ids]                # approval hashes
python3 -m pytest -q tools/icons                      # the tool's own tests (CI: js job)
```

`lint`, `build` and `hash` need only Python 3.11. `sheet` also needs Pillow and `rsvg-convert`, or `resvg`
with `--renderer resvg`. egui renders with resvg 0.45 (`crates/svelters-egui/tests/icon_loader.rs` checks
that path). Codicons are not vendored here (DS5 does that): without `--codicons` the sheet names the nearest
codicon and skips its image.

## A set

```
<set>/icons.toml          the manifest (below)
<set>/<px>/<id>.svg       one master per optical size: 12, 14, 16, 20, 24 (32: illustrations)
<set>/style.toml          optional: overrides tools/icons/style.toml
```

`tools/icons/samples` is IC0's 24 samples (66 masters), the reference set the lint must pass. The real
set lives in cadatomic (IC2–IC4).

### icons.toml

```toml
[set]
name = "..."
licence = "CC-BY-4.0"                       # required (ICONS.md, decided I5)
domains = ["chrome", "vcs", "status", "doc", "mcad", "ecad", "sim", "select"]
allow = [{ rule = "L12", size = 20, ids = ["chrome.*"], reason = "..." }]   # set-wide waivers, id globs

[icons."mcad.extrude"]
metaphor = "iso slab, top face in the tone, solid arrow up"   # shown on the sheet
codicon = "arrow-up"                        # the nearest codicon, by name (sheet column)
uses = ["strip", "toolbar", "status", "inline", "compact", "tree-badge"]   # L12: required masters (§5.2)
sizes = [20]                                # extra required masters
badges = ["remove"]                         # build composes mcad.extrude+remove
free_angles = true                          # L7 off (radially symmetric generators: gear teeth)
allow = [{ rule = "L10", size = 16, reason = "..." }]        # waivers: a reason is required, unused ones fail
approved = "sha256:..."                     # frozen by the user's approval; `hash` prints it

[icons."sim.solve"]                         # a composed id has no SVG of its own
compose = { base = "sim.mesh", badge = "run", base_tone = true }
```

Ids are `<domain>.<group>.<name>` (lower case, words joined by `-`). Badge variants are `<id>+<badge>` and
exist only in build output.

## The rules

| Id | What `lint` checks | Notes |
|---|---|---|
| L1 | root: `viewBox="0 0 S S"`, `width`/`height` S, `fill="none"`, `stroke="currentColor"`, the size's `stroke-width`, round caps and joins; per element, caps `round`/`butt` and joins `round`/`miter`; dashes only with the size's pattern, butt-capped, and none at 12 | O1 (round terminals) is the user's choice |
| L2 | elements `svg g defs path rect circle clipPath mask`; attributes from the allowlist (no `transform`, `style`, `href`, `xlink:*`, filters); `defs` holds only masks and clip paths; no `data:` | |
| L3 | `fill`/`stroke`: `currentColor` or `none`; `#fff`/`#000` only inside `<mask>` | |
| L4 | `class` ∈ subject, accent, badge; `accent` only on `accent_ids`; `badge` only in build output; a classed part is fill-only or stroke-only, except a subject fill with its outline (`subject_outline`, pending ICR1's usvg test) | |
| L5 | opacity 1, or .4 inside `.subject` (and not at sizes with `tone = false`: 12); .2/.45 in `fringe_ids`; no `rx`/`ry` | O2 (square corners, the T2 cut) is the user's choice |
| L6 | stroke centres: axis-aligned segments, `rect` edges and circle extremes on .5 (or whole pixels for even whole widths: 2, 4); fill-only axis-aligned edges on whole pixels | |
| L7 | diagonals 45° or 2:1 / 1:2 (±1%), strokes and fills; arcs free; `free_angles` per id | |
| L8 | stroke centres within `pad..S-pad`, fills within half a pixel more; ink on the canvas. Exempt: solid triangles (heads), clipped parts, the ends of open strokes (round caps may touch the edge, butt ends may end on it), composed badges | `pad` is ICON-STYLE's pad + .5, except at 24 (see style.toml) |
| L9 | stroke widths: the size's stroke, copper, or hatch inside a clip-path | |
| L10 | gaps: parallel axis-aligned strokes that run side by side for longer than a counter keep `min_gap` clear; counters: a stroked `rect` or `circle` keeps `min_counter` clear | diagonals and general counters stay *(review)* |
| L11 | a solid isosceles triangle with an axis-aligned base and 45° sides (a T3 head) has half-base = the size's `arrowhead` | O3 (solid 90° heads) is the user's choice; diagonal heads (axis-aligned legs) are not size-checked |
| L12 | the masters each id needs: `masters.all`, `masters.domains`, the id's `uses` and `sizes` | |
| L13 | file ≤ `max_bytes` (1.5 KB) | |
| other | `id` naming, `orphan` (SVG without entry and the reverse), `manifest` keys, `size` (a directory without a table), `badge` (glyphs inside the cell, closed list), `frozen`, `waiver`, `waiver-unused` | |

Raster crispness is not a gate (ICON-STYLE §3.3). The human judges pixels on the sheet.

## build

```
<out>/<px>/<id>.svg            currentColor; badge variants composed
<out>/hc/<px>/<id>.svg         High Contrast: the subject tone .4 -> hc_tone (.6)
<out>/white/<px>/<id>.svg      tint-ready: currentColor -> #fff, for egui_extras' SvgLoader (renders currentColor black)
<out>/white/hc/<px>/<id>.svg
<out>/manifest.json            id -> sizes, codicon, composed, approved
<out>/icon_ids.d.ts            `IconId` union and `ICON_IDS`
<out>/icon_ids.rs              one `pub const` per id (`+` -> `__`) and `ALL`
```

**Badges** (ICON-STYLE §7.2): the base goes into `<g mask>`, and the mask clears the bottom-right cell (8 at
16, 12 at 24; none at 12 and 14). Then the glyph from `badges/<px>/<name>.svg` goes into `<g class="badge">`.
The closed list is in `style.toml`. The glyphs are IC0's add, remove, warning, link and lock, plus IC1's run,
check, error, settings, queued and stale. No 20 glyphs are drawn yet, so variants skip 20. `base_tone` puts
the base in `class="subject" opacity=".4"`.

**Z2** (`pick_master(p, masters)`): the reference for ICR1's master choice by physical pixel size: exact,
then ×2, then padded by up to 2px, then the nearest scaled.

## Review loop

1. The agent draws to ICON-STYLE, then runs `lint` until it is clean. A rule that can't hold gets a waiver
   with a reason, and the reviewer sees it.
2. The agent runs `sheet` (with `--codicons`, and `--zoom` for pixels), Reads the PNG and answers ICONS.md
   §4.1's four questions per icon. At most 3 passes.
3. **The user approves at contact-sheet level** (ICONS.md, decided I2). Agents never approve their own icons.
   For each approved id, write `approved = "<hash>"` (from `hash`). From then on `lint` fails if the icon
   changes, until the approval is removed with a reason.

The generator prompt is ICONS.md §4.1, with ICON-STYLE's three traits (subject tone, T2 cut, T3 head), §5.1's
per-size table and the slot classes added.
