# Lane TEXT: presto-text

Branch `ci/presto-text`. Owns `crates/presto-text/**`. Read: architecture §9; notes `round2/text-stack.md`
(the stack, versions, the gamma formula, caches, risks), `gpu-2d-text.md`, `round2/gpui-port.md` item 6.

Deliver (push after each; the brief's API first, within 45 min):
1. `FontStore` (fontique; system fonts on/off; bundled fonts; a font-set generation counter), `FontConfig`.
2. `TextSystem` (parley 0.12 `LayoutContext`/`FontContext` reuse): `layout(&RichText, max_width)` →
   `Arc<TextLayout>` with placed `GlyphRun`s (presto-base) whose style is a style id, so colour changes never
   reshape; ellipsis and `max_lines`; tab stops; `TextMeasure` impl (min/max-content via
   `calculate_content_widths`, re-break via `break_all_lines`).
3. Caches: shaped paragraphs (two generations + LRU byte budget), line breaks per width, fallback memo with
   per-codepoint memos for Common-script symbols.
4. `TextLayout` geometry: `hit_test`, `caret_rect`, `selection_rects` (bidi-correct, grapheme-correct).
5. `FontRasterizer` (impl `GlyphRasterizer`): skrifa outlines (use the skrifa version parley pins), vertical-
   only hinting (`HintOptions` auto-fallback with preserved linear metrics), our own area-coverage rasteriser
   (font-rs/zeno style, deterministic) with `AaMode::HorizontalOnly`; the DirectWrite `gasp` raster policy
   (copy skrifa 0.49's ~30-line policy if parley's skrifa lacks it, with attribution); 4×1 subpixel x-bins;
   dilation levels (macOS look); colour glyphs: COLRv0/v1 via skrifa's paint API into premultiplied RGBA,
   sbix/CBDT PNG decode.
6. Bundled default fonts for deterministic tests (Inter or another OFL font as svelters already ships, check
   `crates/svelters-egui/src/fonts.rs` and `assets`); `system_fonts: false` in tests.

Tests (write, don't run): exact glyph-run goldens for a small corpus; content widths; ellipsis; caret and
selection geometry incl. RTL; rasteriser determinism (byte-identical masks); x-only AA differs from grayscale.
