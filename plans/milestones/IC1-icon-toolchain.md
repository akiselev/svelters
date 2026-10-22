# IC1: the icon toolchain (`tools/icons/`)

Design: `plans/research/ICONS.md` §3, §5.4 and row IC1 of §5.5, with its "Decided" block (codicons for chrome,
a custom set on codicons' grid, CC-BY-4.0, the user approves each batch at contact-sheet level), and
**`plans/research/ICON-STYLE.md` (IC0)**, which supersedes ICONS.md §3 where they differ: lint rules L1–L13
(§10), the per-size table (§5.1), the master set (§5.2: 20 is now a master), colour slots (§6) and the
whole-cell badge knockout (§7.2). Lane IC1, branch `ci/ic1`, merged with `research/ic0`.

Course corrections from the coordinator during the lane, all applied:
- **Optical sizes** (user): 12, 14, 16, 20, 24 and 32+, each with its own weights. The per-size rules are a
  table the lint reads (`style.toml`), and the sheet lays each icon out across all its sizes.
- **IC0 landed**: lint geometry with L1–L13, with no raster crispness gate. Take the size table and master
  set from ICON-STYLE, lint the slot classes with the accent allowlist, and absorb IC0's `src.py` check and
  `sheet.py`.
- **O1–O3 decided** (user, 2026-10-22): round terminals, the 45° cut on every frame, solid 90° heads. They
  are the lint's fixed defaults, not options.

## What landed

| Path | Content |
|---|---|
| `tools/icons/icons.py` (new) | `lint` (L1–L13 plus id, manifest, orphan, size, badge, frozen and waiver checks; a real SVG path parser with arcs and curves), `build` (badge composition with the cell mask, HC copies with the tone at .6, white tint-ready copies, `manifest.json`, `icon_ids.d.ts`, `icon_ids.rs`), `sheet` (the approval sheet), `hash` (approval hashes), and `pick_master` (ICON-STYLE Z2, the reference for ICR1). The standard library only, except `sheet` (Pillow plus rsvg-convert or resvg) |
| `tools/icons/style.toml` (new) | every number the lint uses: the per-size table (stroke, copper, hatch, pad, min gap, min counter, arrowhead, cut, tone, badge cell, dash), the master set, the badge list, the accent allowlist, fringe ids, the HC tone, the byte budget |
| `tools/icons/SPEC.md` (new) | commands, set layout, `icons.toml` keys, the rule table with where the lint departs from the guide, build outputs, the review and approval loop |
| `tools/icons/badges/{16,24}/*.svg` (new) | the closed badge list: IC0's add, remove, warning, link and lock (warning's mark moved to whole pixels, link redrawn to stay on the canvas), plus run, check, error, settings, queued and stale (new) |
| `tools/icons/samples/` (moved) | IC0's 66 masters (`plans/research/icon-style/svg/`, without the 12 compositions that `build` now makes) plus `icons.toml`: the reference set the lint must pass |
| `tools/icons/test_icons.py` (new) | 21 tests: each rule, the size table, the samples lint clean, manifest and waiver rules, approval freezing, composition, HC and white copies, Z2 against ICON-STYLE §5.3's table, the build tree, and that the egui fixtures are current |
| `crates/svelters-egui/tests/icon_loader.rs` (new) | three tests through egui_extras' `SvgLoader` (resvg 0.45): 15 renders (mcad.extrude at 12/14/16/20/24 and 16@2x; section, result, extrude+remove and warning) compared with committed PNGs (±2 per channel); white copies render white; the loader still draws `currentColor` black (when that fails, the white copies can go); the badge mask clears the cell |
| `crates/svelters-egui/tests/icons/{build,png}/` (new) | the fixtures: `icons.py build tools/icons/samples … --only mcad.extrude,mcad.section,sim.result,mcad.extrude+remove,chrome.warning`, and the PNGs (`UPDATE=1`) |
| `crates/svelters-egui/Cargo.toml` | dev-dependency `egui_extras` with `svg` (no text, no raster images). Cargo.lock gains resvg 0.45.1's stack (18 crates: resvg, usvg, tiny-skia, roxmltree, svgtypes, kurbo, …), for svelters-egui's tests only |
| `.github/workflows/js.yml` | the js job lints the samples and runs the tool's tests |
| `plans/research/ICON-STYLE.md`, `ICONS.md` | pointers: the samples moved, and the sheet command is `tools/icons/icons.py sheet` |

The ICONS.md prototype (`plans/research/icons-proto/`) stays where it was, as the research record. The new
lint fails it as intended: tone without `class="subject"`, and so on.

## Values and where they come from

`style.toml` takes ICON-STYLE §5.1 for 12–24 (strokes 1, 1, 1, 1.25, 1.5; min gap 1, 1, 1, 1.75, 1.5; counters
2, 2, 2, 3, 3; heads 2, 2.5, 3, 3.5, 4.5; cuts 2, 3, 3, 4, 5; no tone at 12; badge cells 8, 12, and 10 at 20;
dash patterns at 16 and 24). §5.2 gives the master set: 16 for every id, plus 20 and 24 for chrome, 24 for
`strip`, 20 for `toolbar`, 14 for `status` and `compact`, 12 for `inline` and `tree-badge`. **Placeholders** that
ICON-STYLE does not give: copper widths at 12, 14 and 20 (2px), dash patterns at 14 and 20, and the whole 32
table (illustration masters: the 16 values doubled). Change them in `style.toml` when ICON-STYLE does.

## Decisions taken here (recorded for the coordinator)

- **L8 pad.** §5.1 gives 24 a 2px pad, but §3.2's iso-slab keyline (1.5..21.5) and IC0's samples use 1.5. The
  lint checks stroke centres against `pad` (ICON-STYLE's pad + .5 at 12–20, 1.5 at 24) and lets fills reach
  half a pixel further. The ends of open strokes (round caps, butt ends at an edge), heads and clipped parts
  are exempt, as §3.2 allows.
- **L10** checks axis-aligned parallels that run side by side for longer than a counter, and the counters of
  stroked `rect`s and `circle`s. Diagonal gaps and general counters stay review items (as §10 says).
- **L11** size-checks T3 heads only: solid isosceles triangles with an axis-aligned base and 45° sides. The
  run triangle (1:2) and diagonal heads (axis-aligned legs) are not heads by that test.
- **Stroke phase** is derived, not configured: even whole widths (2, 4) centre on whole pixels, every other
  width (1, 1.25, 1.5, 3) on .5. That matches §3.1.
- **The knockout mask's rects set `stroke="none"`.** Otherwise they inherit the root's `currentColor` stroke,
  which in the white copies draws a half-strength seam along the cell edge. The egui snapshot test found
  this. IC0's `src.py` composer has the same inheritance, so its renders have the seam too.
- **Waivers** need a reason, unused waivers fail, and set-wide waivers take id globs. The samples use six,
  which are real deviations for IC2 (and three that are guide questions, below).
- **Approval**: `approved = "sha256:…"` per id. The lint fails when an approved icon changes, so a changed icon
  goes back to the user.

## Questions for the coordinator (guide inconsistencies found by the lint)

1. **Sketch points vs counters:** §8 says sketch points are hollow 3×3 squares (4-span at 24), which leaves 1px
   (2.5px) of counter, but §5.1 asks for 2×2 (3×3). Waived on `mcad.sketch`. Recommendation: exempt the
   point marker by rule (a hollow marker's dot is the point), or make the marker 4×4 at 16.
2. **24 pad:** as above. Recommendation: state the 24 live area for stroke centres as 1.5..22.5 in
   ICON-STYLE §5.1.
3. **IC0 sample deviations** to fix in IC2: `sim.load` 12's head base at y=5.5 (T3 says a whole pixel),
   `ecad.footprint` 14's 2.5px pads (half-pixel fill edges), the `chrome.settings` 24 hub counter (2.5 < 3),
   and missing 20 masters for 9 chrome samples (§5.2 requires them).

## Verification

- `python3 tools/icons/icons.py lint tools/icons/samples`: 66 masters, 0 findings (6 waivers with reasons).
- `python3 -m pytest -q tools/icons`: 21 passed.
- `tools/dev/cargo-slot cargo nextest run -p svelters-egui --test icon_loader`: 3 passed, run after recording
  the PNGs with `UPDATE=1` and checking them by eye (×6 montage).
- `cargo check -p svelters-egui --test icon_loader` and `cargo clippy` on that test: clean.
- The sheet was rendered on the samples with codicons fetched for the run (not committed) and read: the
  multi-size rows, the codicon column, the badges, the slots and the zoom sections.
- **Not run locally:** the svelters-egui test suite beyond `icon_loader`, the workspace, conformance. CI runs
  them.

## Follow-ups

- DS5 vendors codicons. `sheet --codicons` then points at the vendored SVGs, or a font-to-SVG export of them.
- Badge glyphs at 20 (a 10px cell). Until then, badge variants skip 20.
- ICR1/AS1: tint without white copies, slot injection, HC tone at load time, `pick_master` in Rust. The
  `the_loader_draws_current_color_black` test tells when egui_extras no longer needs the white copies.
- The generator prompt (ICONS.md §4.1) needs ICON-STYLE's traits, the size table and the slot classes. IC2
  writes it with its first batch.
