# PRESTO-text: presto-text (presto batch 1, lane TEXT)

Branch `ci/presto-text` from `ci/presto-base`. Brief: `plans/cloud/presto-b1/BRIEF.md`,
`plans/cloud/presto-b1/LANE-text.md`. Design: architecture §9, `notes/round2/text-stack.md`.

## Landed

All of `crates/presto-text`. The brief's API is kept as specified and is implemented, not stubbed.

| Deliverable | Where | Notes |
|---|---|---|
| 1. `FontStore`, `FontConfig` | `src/fonts.rs` | fontique `Collection` in shared mode (each `TextSystem` clones it); `system_fonts` on/off; bundled fonts; a font-set generation counter (`register_font`, `system_fonts_changed` bump it). Faces interned as `FontId` from (blob id, index); variation coordinates interned by hash (`intern_coords`/`coords`) so `GlyphRun.variations`/`GlyphKey.variations` round-trip. With system fonts off, each bundled family becomes the fallback of every script whose fontique sample it covers (≥ 75 %), and the default family backs the `sans-serif`/`system-ui`/`ui-sans-serif` generics. Bundled colour fonts join the `emoji` generic (what parley queries for emoji clusters). `FontConfig::testing()`. |
| 2. `TextSystem` | `src/system.rs` | parley 0.12 `LayoutContext<u32>`/`FontContext` reused. `layout(&RichText, max_width) -> Arc<TextLayout>`. The parley brush is the **span index**, and `PlacedRun.style` is looked up from it at placement, so paint ids (`TextStyle::paint`) never enter the shaping key. Ellipsis ("…" shaped once per base style; cut from cluster advances in logical order; realigned) and `max_lines`; tab stops (an in-flow inline box one tab wide makes breaking reserve room; placement advances to the next stop; tab glyphs dropped). `TextMeasure` impl: `measure` = placed size; `content_widths` = `calculate_content_widths` (memoised), with max-content from placement when the text has tabs. |
| 3. Caches | `src/system.rs` | Shaped paragraphs keyed by (shaping hash, font generation): entries used in this or the previous generation (`end_frame`) are kept, older ones evicted LRU once estimated bytes exceed the budget (16 MB default, `set_cache_budget`). Per paragraph: the last break (width, alignment), content widths, the last three placed layouts keyed by (width, align, ellipsis, max lines, paint hash). Symbol fallback memo per codepoint for the Common-script symbol blocks (U+2100–214F, 2190–243F, 2460–2BFF) over a per-OS chain (Windows: Segoe UI Symbol, Segoe UI Emoji, Cambria Math; macOS: Apple Symbols, Apple Color Emoji, STIX Two Math; Linux: Noto Sans Symbols 2, Noto Color Emoji, Noto Sans Math, from text-stack.md §4.1); per (script, locale) fallback is fontique's own memo. `stats()` exposes counters. |
| 4. `TextLayout` geometry | `src/layout.rs` | Self-contained (no parley state). Per-line clusters in visual order with logical byte range and direction; `hit_test` → `Cursor { byte, affinity }` (upstream at a wrapped line's end), `caret_rect` / `caret_rect_at(Cursor)`, `selection_rects` (one rect per visual piece, so bidi ranges split). Grapheme-correct because parley 0.12 clusters are grapheme clusters; a byte inside one snaps to its start. Also `lines()`, `truncated()`, `is_rtl()`, `first_baseline()`, and `clusters(line)` → `ClusterInfo { range, x, advance, rtl, newline }` in visual order (QUIRE's request via the coordinator). |
| 5. `FontRasterizer` | `src/raster/` | skrifa 0.44 (the one parley pins, single read-fonts copy) outlines; `HintMode::Vertical` = `Engine::AutoFallback` + `Target::Smooth { Light, symmetric_rendering: false, preserve_linear_metrics: true }`, hinting instances cached (64, LRU). Own rasteriser `coverage.rs` (font-rs accumulation port, attributed) with symmetric and horizontal-only (`AaMode::HorizontalOnly`: centre-scanline sampling, exact x coverage). DirectWrite gasp policy copied from skrifa 0.49 (`raster_policy`, attributed), exposed as `FontRasterizer::preferred_aa(font, ppem)`. 4×1 x-bins (`x_bin/4` px offset). Synthetic bold (`embolden_64`, bisector offset as FreeType's EmboldenXY) and skew. Dilation levels via `RasterConfig::dilation_px`. Colour: COLRv0/v1 through skrifa's `ColorPainter` into premultiplied RGBA (solid, linear, two-point conical, sweep; pad/repeat/reflect; Porter-Duff + separable blend modes), sbix/CBDT PNG and BGRA, EBDT masks, resampled bilinearly to the ppem. |
| 6. Bundled fonts | `fonts/`, `src/test_fonts.rs` | Subsets of Noto Sans Regular/Bold (hinted, Latin-1 + punctuation + fi/fl), Noto Sans Hebrew (hinted), Noto Color Emoji COLRv1 (3 emoji): 143 KB total, OFL (`fonts/OFL.txt`, `fonts/README.md` with sources and the `pyftsubset` command). svelters ships no TTF/OTF in the repo (egui's fonts come from the `epaint_default_fonts` crate), so these are new. |

Tests (`src/tests/`): layout (shaping, Hebrew fallback, content widths, measure, wrapping, ellipsis,
max_lines, tabs, paint-id reuse, rewrap reuse, eviction, bold face, alignment, empty text), geometry
(LTR round trip, RTL carets and selections, mixed-direction selection, graphemes, wrapped-line
affinity, hard and empty lines, clamping), coverage (exact pixel values, x-only vs symmetric,
winding, flattening, emboldening, determinism), raster (byte-identical masks, x-only ≠ grayscale,
x-bins, hinting, empty space glyph, embolden/dilation, COLRv1 colour image, gasp policy), and exact
goldens (`src/tests/goldens/runs.txt`: glyph runs of a 9-item corpus; `masks.txt`: mask sizes,
offsets and FNV digests in three AA/bin combinations; `UPDATE=1` regenerates).

Cross-lane notes for RENDER: `GlyphRun.aa` is `Grayscale` from layout (layout does not know the
device scale); the renderer should build its `GlyphKey.aa` with `FontRasterizer::preferred_aa`
(DirectWrite policy) or its own setting. Layout runs at scale 1, unquantised: baselines must be
snapped to device pixels by the renderer (architecture §9.3). `GlyphRun.ascent`/`descent` are
parley's run metrics (both positive). The ellipsis run has the empty range `len..len`.

## Not run

Per the batch rule, no tests were run except two filtered runs of the golden tests with `UPDATE=1`
to generate `goldens/*.txt` (golden values cannot be written by hand; the generated files were read
and checked for plausibility: fallback faces, ligature glyphs, RTL order, ellipsis glyph, COLR image
size). Everything else is CI's (`presto.yml`, three OSes). `cargo check -p presto-text --tests` is clean.

## Stubbed, approximated or left out

- **Dilation amounts.** The CoreGraphics dilation per luminance level is not public (text-stack.md
  §2.5), so `RasterConfig::dilation_px` defaults to empty and a key's dilation level then draws
  undilated. Needs measured amounts (CoreText side-by-side) before macOS look work.
- **`AaMode::Lcd`** rasterises as grayscale (LCD is a later, gated path, architecture §9.4).
- **COLR**: the four HSL blend modes composite as source-over; the foreground colour (palette index
  0xFFFF) is black, because a glyph key carries no text colour.
- **Tabs and alignment**: breaking reserves a full tab per tab; centred/right-aligned lines with tabs
  are aligned by parley with that reserved width, so they can sit up to one tab off.
- **Ellipsis with mixed direction**: clusters are kept in logical order and re-placed contiguously,
  ellipsis at the paragraph's end edge; not a CSS-exact bidi truncation.
- **Byte budget** uses an estimate (512 + 64 per text byte + 128 per span), not a measured size.
- Left out (not in the lane list): the editor-line path (`parley_engine` per line, ASCII fast path),
  the Slug band builder, inline boxes for icons/chips in `RichText`, vertical-align, decorations
  (parley's underline/strikethrough are not passed through; paint ids leave them to the caller).

## Cross-lane edits

- `THIRD_PARTY_NOTICES.md`: a "presto-text (lane TEXT)" section before the MIT License text (font-rs,
  skrifa 0.49 policy, the Noto fonts). Other lanes adding sections there may conflict textually.
- `Cargo.lock`: parley 0.12 / fontique / skrifa 0.44 / png entries.
- No `presto-base` changes.

## Dependencies

`parley` (workspace), `fontique = "0.12"` with `fontconfig-dlopen` (so Linux builds without
`libfontconfig-dev`; fontconfig is loaded at runtime only when system fonts are on), `skrifa = "0.44"`
and `png = "0.18"` in `crates/presto-text/Cargo.toml` (kept out of
the workspace table to avoid conflicts with other lanes; the coordinator may move them).

## Questions for the coordinator

1. **skrifa version.** I used option (c) of text-stack.md §2.6 (stay on parley's skrifa 0.44, copy
   the gasp policy). Recommendation: keep it until parley moves to read-fonts ≥ 0.46, then drop
   `raster_policy` for skrifa's `ScalerFont::raster_policy`.
2. **Dilation amounts** (above). Recommendation: a later macOS-look task measures CoreText output
   for the five levels and fills `RasterConfig::dilation_px`; until then macOS draws lighter than
   native, as the research expects.
3. **Workspace deps.** Recommendation: move `skrifa`/`png` into the workspace table at consolidation
   if another crate needs them.

## CI

- `9221fcc` (presto-base `7a947d45` merged): js, linux-gtk, linux-qt, macos-appkit, windows-winui green
  (run ids 40223218359, …416, …386, …360, …402). presto (40223218361): all 39 presto-text tests pass on
  macOS and Windows, goldens included (byte-identical masks across OSes). Linux failed in `cargo
  check`: parley's default `system` feature links fontconfig via pkg-config and the runner has no
  `libfontconfig-dev` — fixed in the next push with fontique's `fontconfig-dlopen`. The other presto
  failures (`presto-anim`, `presto-frame`, `presto-platform` lib tests, `presto-render --test gpu`
  missing reference PNGs) are other lanes' crates, not touched here.
- `f023687` (fontconfig-dlopen, `TextLayout::clusters`, presto-base `cc4f184d` merged; merged into
  presto-base as `9af594f6`): presto run 40226699805 builds on all three OSes and presto-text passes
  on Linux, Windows and macOS. The run is red only for `presto-compositor --test engine`,
  `--test tree_damage`, `presto-core --lib` and `presto-widgets --lib` (other lanes). The five svelters
  workflows skip presto-only pushes since base `cc4f184d`; they were green on `9221fcc`.
- Lane done.
