# Lane RENDER: presto-render

Branch `ci/presto-render`. Owns `crates/presto-render/**` (and `THIRD_PARTY_NOTICES.md` additions for GPUI).
Read: architecture §8; notes `round2/gpui-port.md` (the port list with sha a5c4ca8, struct layouts, shaders,
licensing), `gpu-2d-text.md`, `round2/text-stack.md` §gamma.

Deliver (the brief's `Renderer` API first, within 45 min):
1. Pipelines (created at startup), WGSL ported from GPUI `crates/gpui_wgpu/src/shaders.wgsl` (Apache-2.0,
   keep the Windows Terminal MIT notice on the gamma functions): Quad (per-corner radii, per-edge borders,
   dashed/dotted, solid/linear fill; add a rounded clip per instance), Shadow (erf blur, inset), Line (caps,
   dash, wavy), Mask sprite (R8 coverage with the gamma/contrast formula; gamma/contrast as uniforms), Color
   sprite (premultiplied RGBA), Image (radii, src rect, nearest/linear), Path (lyon fill/stroke tessellation
   into a 4× MSAA intermediate sized to the batch bounds; cache per (PathId, scale)), Surface (registered
   textures).
2. Batching: draw order via a bounds R-tree (port GPUI `bounds_tree.rs`, rebuilt per frame or per layer),
   sort by (order, pipeline, atlas page). Instance data: one storage buffer per frame in flight (a ring),
   `StagingBelt` uploads. Non-sRGB targets; blending in sRGB-encoded space (architecture §8.3).
3. Atlases: etagere pages (1024², grow to 2048²), R8 + RGBA8, texture generations, frame-generation LRU
   eviction (GPUI never evicts glyphs; we must). Glyph keys: presto-base `GlyphKey`, rasterised through the
   `GlyphRasterizer` passed to `new`. Icons/images from `SceneResources`.
4. Damage: render only `scene.damage` (scissor per rect, or the union when cheaper); clear damaged areas.
5. `render_to_image` headless path (render target → buffer → RGBA bytes) for tests.
6. `RenderStats` (draws, instances, uploads, atlas pages, µs CPU).

Tests (write, don't run; GPU tests gated on `PRESTO_GPU_TESTS=1`, which CI sets): one image per primitive
with tolerance comparison against a small checked-in reference *made by the CI run* (write the test to
record when the reference is missing and fail with "reference recorded, inspect it" — the consolidation
blesses them); batching counts for a 10k-quad scene; damage-only redraw leaves other pixels untouched.
