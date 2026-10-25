# GPU 2D rendering and text for a retained Rust GUI (cadatomic): research notes

Compiled 2026-10-25 for cadatomic's planned replacement of egui 0.37's renderer. Web research only:
nothing was built or measured here.

**Conventions**
- **[D]** means documented: the cited source says it. **[I]** means inferred, either by me or from
  indirect evidence. **[M]** means a measurement someone else reported, on their own setup.
- Versions and dates are given as the source states them. Where the source has no date, the note
  says so.
- Source code was read at `main` on 2026-10-25 (Zed/GPUI, wgpu `trunk` CHANGELOG, Vello `main`).
  Those files change, so a quoted line can move.

---

## 0. Headline findings

1. **For UI, the winning design is a small fixed set of instanced analytic primitives plus a glyph
   atlas.** That covers rounded and bordered rects, shadows, underlines, mono and colour sprites, and
   a path fallback. GPUI (Zed) and WebRender both converge on this, and Graphite and Impeller both moved
   toward a *small, precompiled* pipeline set.
2. **Pure GPU-compute vector rendering (Vello classic) is still "experimental"** in 2026. Its
   intermediate buffers must be sized on the CPU up front, which is a structural problem. Linebender's
   production direction is now **sparse strips**: Vello CPU, and Vello GPU (renamed from
   "Vello Hybrid" in 0.3.0, 2026-10-02), which flattens and tiles on the CPU and composites on the GPU
   without compute shaders.
3. **The big idle and latency wins come from not drawing,** not from a faster rasteriser.
   - In Zed, damage tracking with partial redraw cut GPU usage from 50–60% to 5–10% and Zed's CPU
     from 80–90% to 30–50% [M].
   - A depth-buffer opaque pre-pass halved GPU busy time, from 0.958 ms to 0.439 ms [M].
   - wgpu has **no damage or partial-present API** upstream as of 2026-10. The maintainers said in
     Aug 2026 that a `present_with_damage` V1 PR "would be accepted".
4. **Text quality on low-DPI Windows depends on gamma and contrast correction of the grayscale
   coverage,** using DirectWrite's values. Zed ported Windows Terminal's shader (gamma 1.8, enhanced
   contrast 1.0) in 2025.
   - True LCD (subpixel) AA needs dual-source blending and an opaque, known background.
   - GPUI added a `SubpixelSprite` primitive using dual-source blending.
   - macOS dropped LCD AA (Mojave). GTK 4 is grayscale-only.
5. **SDF/MSDF text is wrong for small UI text:** no grid-fitting, softer stems, and large-size
   artifacts (Qt docs). Slug, an analytic-from-outline renderer, went public domain on 2026-03-17 and
   suits zoomable canvas text, not 11–13 px UI text.
6. **Present and latency:**
   - Use FIFO with `desired_maximum_frame_latency` 1–2. Pace frames with the platform display link:
     CAMetalDisplayLink on macOS 14+, the DXGI waitable object on Windows.
   - Render 3D viewports either into the same frame (as a texture) or as their own OS compositor
     layer (DComp visual, CAMetalLayer sublayer, wl_subsurface). The separate layer gives zero-copy
     composition and decouples the viewport's frame rate from the UI's.

---

## 1. 2D GPU rendering approaches

### 1.1 Instanced analytic primitives (GPUI, WebRender brushes, Warp)

**GPUI (Zed)**
- [D] Zed blog, "Leveraging Rust and the GPU to render user interfaces at 120 FPS", Antonio
  Scandurra, 2023-03-07, https://zed.dev/blog/videogame:
  - Most UIs reduce to rectangles, shadows, text, icons and images. Zed writes one shader per
    primitive and uses instanced rendering, so each batch of a primitive kind is one draw call.
  - Rounded rects are an SDF in the fragment shader: shrink the rect by the corner radius, then
    subtract the radius.
  - Shadows use Evan Wallace's closed form. A box blur is separable (erf). Rounded rects get an exact
    integral along one axis and a few samples along the other (Zed's code: 4 steps over about ±3σ).
  - The budget is 8.33 ms per frame at 120 Hz.
  - Pathfinder was tried and judged too slow for this use.
- [D] Evan Wallace, "Fast rounded rectangle shadows", https://madebyevan.com/shaders/fast-rounded-rectangle-shadows/
- [D] Current GPUI source (`crates/gpui/src/scene.rs`, read 2026-10-25):
  - The primitive set is `Shadow`, `Quad`, `Path`, `Underline`, `MonochromeSprite`,
    `SubpixelSprite`, `PolychromeSprite` and `Surface`.
  - A `Quad` carries bounds, `content_mask` (a rect clip), background (solid or gradient),
    `border_color`, `corner_radii`, `border_widths` and `border_style`. All of these are evaluated
    analytically in the shader.
- [D] GPUI's wgpu renderer (`crates/gpui_wgpu/src/wgpu_renderer.rs`):
  - Instance data goes in one storage buffer, capped at `MAX_INSTANCE_BUFFER_SIZE` = 256 MiB, with a
    uint-texture transport for WebGL2.
  - Paths draw into an intermediate MSAA texture (`path_msaa_texture`).
  - Subpixel sprites need `dual_source_blending`.
  - Present mode is `Fifo` by default, with `desired_maximum_frame_latency: 2`.
- [D] Warp, "How to draw styled rectangles using the GPU and Metal", https://www.warp.dev/blog/how-to-draw-styled-rectangles-using-the-gpu-and-metal
  uses the same SDF approach, and notes that the edge needs explicit AA.

**WebRender (Firefox)**
- [D] Servo wiki "Webrender Overview" (design notes from gw and nical), https://github.com/servo/servo/wiki/Webrender-Overview:
  - WR1 drew everything as quads in "2-3 draw calls" per frame, with complex items (border corners)
    pre-rendered into an atlas.
  - One überquad shader was hard to extend: arbitrary clip paths slowed pages that didn't use them.
  - "If you have alpha blend enabled, the framerate is halved, even with an alpha of 1" was observed
    on their hardware.
  - Shader switching was expensive on some Intel hardware.
- [D] Current WebRender batching (`webrender::batch`, https://doc.servo.org/webrender/batch/index.html)
  has brush batch kinds: Solid, Image, Blend, LinearGradient, Opacity and others. Clip masks are
  rendered into a render-target cache in 128-device-pixel tiles.
- [I] Lesson: a handful of specialised instanced shaders (one per primitive kind) beats both one
  überquad shader and a general vector rasteriser for UI.

### 1.2 CPU tessellation (egui/epaint, lyon)
- [D] epaint 0.37.0 `TessellationOptions`, https://docs.rs/epaint/latest/epaint/tessellator/struct.TessellationOptions.html:
  - AA is "feathering": an extra 1 physical pixel ring of triangles (`feathering_size_in_pixels`),
    which costs more triangles.
  - Other options are `coarse_tessellation_culling`, `bezier_tolerance`, and optional rayon
    `parallel_tessellation`.
  - egui re-tessellates the whole frame's shapes on the CPU each frame (the user's description,
    consistent with these docs).
- [D] lyon 1.0.19, https://docs.rs/lyon/latest/lyon/:
  - Fill and stroke tessellation of flattened paths, within a tolerance.
  - "Lyon does not provide with any GPU abstraction or rendering backend". There is no AA built in,
    so you add MSAA or feathering.
- [I] Fit:
  - CPU tessellation is fine for **static** geometry tessellated once and cached in a GPU buffer:
    board outlines, polygons, copper pours.
  - It is wrong for per-frame UI, because the CPU cost grows with scene size every frame, and it
    loses the analytic AA quality of SDF primitives.

### 1.3 Compute-based vector rendering: Vello classic, and sparse strips (Vello CPU / Vello GPU)

**Vello's current layout**
- [D] Vello README and ARCHITECTURE.md at `main` (read 2026-10-25),
  https://raw.githubusercontent.com/linebender/vello/main/README.md and `/ARCHITECTURE.md`. There
  are three renderers:
  - **Vello CPU** (`vello_cpu`): "the most mature choice".
  - **Vello GPU** (`vello_gpu`): "preprocesses paths on the CPU and uses the GPU for rasterization
    and compositing … wgpu and WebGL2 backend … without requiring compute shaders … intended to
    become the primary renderer for production GPU use cases as it matures; Vello CPU is currently
    overall more mature."
  - **Vello compute renderer** (`vello`, moved under `research/`): "remains an experimental
    implementation for compute-capable GPUs."
- [D] Sparse strips: "Paths are flattened, binned into tiles, and represented as sparse horizontal
  strips. Coverage data is retained where it is needed around path boundaries, while fully covered
  regions can be represented compactly." (ARCHITECTURE.md)

**Vello GPU changelog**
- [D] `vello_gpu` CHANGELOG, 0.3.0 (2026-10-02), https://raw.githubusercontent.com/linebender/vello/main/vello_gpu/CHANGELOG.md:
  - "Vello Hybrid has been renamed into Vello GPU".
  - Updated to wgpu v30.
  - Depth buffering is optional (caller-owned depth texture).
  - Adds `Scene::push_clip_rect` for non-isolated rectangular clipping.
  - Image atlases use individual 2D textures.

**Linebender progress posts**
- [D] Linebender in 2026 Q1 (Raph Levien, 2026-04-19), https://linebender.org/blog/tmil-25/:
  - "Vello Hybrid is currently at roughly beta quality".
  - Optimisations: fast paths that skip full coarse rasterisation, a rectangle special case, and
    heavy-gradient fixes.
  - Glyph caching is a "first cut … more work is needed". The glyph crate `parley_draw` was renamed
    **Glifo** and moved into the Vello repo to iterate on "atlas-based glyph caching".
  - Masonry moved to the `imaging` abstraction, so it can run on Vello CPU.
- [D] Linebender in October 2025 (2025-11-13), https://linebender.org/blog/tmil-22/: sparse strips
  0.0.4 is "the initial release of Vello Hybrid, … GPU to do pixel compositing, but SIMD-accelerated
  geometry processing on the CPU".
- [D] Vello 0.8.0, the compute renderer, was released 2026-03-20 on wgpu 28 (Vello CHANGELOG via
  search result; https://linebender.org/atom.xml). The Dec 2025 post reports a 30% improvement on a
  flower-overlay benchmark from overdraw removal in Vello CPU and Hybrid [M].

**Why the compute design is hard**
- [D] Raph Levien, "I want a good parallel computer", 2025-03-21, https://raphlinus.github.io/gpu/2025/03/21/good-parallel-computer.html:
  - "the buffers for the intermediate results need to be allocated (under CPU control) before
    kicking off the pipeline".
  - Overflow recovery needs a GPU→CPU readback, "a big performance problem".
  - Candidate workarounds each have "a fatal flaw on hardware today".
- [I] This explains the shift to CPU-side sparse strips.

**Thesis (Vello CPU)**
- [D] Laurenz Stampfl, "High-performance 2D graphics rendering on the CPU using sparse strips",
  MSc thesis, ETH Zürich, 2025-10-10,
  https://ethz.ch/content/dam/ethz/special-interest/infk/inst-pls/plf-dam/documents/StudentProjects/MasterTheses/2025-Laurenz-Thesis.pdf:
  - Tiles are 4x4 pixels.
  - In the Blend2D benchmark harness, **Blend2D wins nearly every test**. Vello CPU "lands a second
    place" for solid rect fills and scales best at large shapes.
  - It is weak on small shapes, because the 4x4 minimum tile includes uncovered pixels.
  - Single-threaded example, "Fill - Solid - RectU", time for 1000 calls: Vello CPU 0.30 ms at 8x8
    up to 14.2 ms at 256x256. Blend2D is faster still.
  - [M] on the thesis' machine.
- [I] Fit:
  - Vello GPU / Vello CPU are good for **arbitrary vector content**: SVG-ish icons at any scale,
    schematic symbols, complex fills and strokes, gradients and blends.
  - They are not the cheapest way to draw 10^4 rounded rects and glyphs per frame. Every path still
    goes through CPU flattening and tiling each time it is submitted, unless strips are cached.
  - The thesis calls strips "usable … for caching", but no stable public cache API was found.

### 1.4 Stencil-then-cover and MSAA paths (Impeller, Graphite)
- [D] Flutter Impeller docs (page for Flutter 3.47, updated 2026-08-21), https://docs.flutter.dev/perf/impeller:
  - "Precompiles a smaller, simpler set of shaders at Engine-build time".
  - "All pipeline state objects upfront".
  - Default on iOS (only engine), Android API 29+ with Vulkan, and, as of 3.47, macOS, Linux and
    Windows.
- [D] Flutter issue #123671, https://github.com/flutter/flutter/issues/123671. Impeller engineers on
  path fill:
  - Convex fills get specialised routines, without generic tessellation.
  - Concave fills use stencil-then-cover: stencil without colour, then cover with a quad, with
    stencil and colour multisampled "in tile memory" on tilers.
  - The issue frames this as a direction, not a confirmed final design.
- [I] Stencil-then-cover costs at least two draws per path plus MSAA memory. It works on tilers
  (mobile and Apple Silicon), where MSAA resolve is nearly free. On desktop immediate-mode GPUs, MSAA
  costs bandwidth, which is Graphite's own caveat (below).

### 1.5 Skia Graphite vs Ganesh
- [D] Chromium blog, "Introducing Skia Graphite", 2025-07-08, https://blog.google/chromium/introducing-skia-graphite-chromes/:
  - Shipped in Chrome on Apple Silicon Macs. Motionmark 1.3 is up "almost 15%" on a MacBook Pro M3,
    between M133 and M134 [M].
  - **Painter's order is encoded as depth (z).** Opaque draws occlude earlier ones and draws can be
    reordered to cut state changes. Transparent content still goes back to front.
  - **Clips are depth-only draws,** not a clip stack, so shader choice does not depend on clip state.
  - **Fewer, consolidated pipelines,** compiled at startup to avoid jank. Ganesh had too many
    specialised GL-centric paths.
  - Independent Recorders produce Recordings on threads. Recordings can be re-issued with a dynamic
    translation, which can speed scrolling and reduce tile-cache memory.
  - Paths use **MSAA** where possible, falling back to CPU rasterisation into an atlas when MSAA is
    slow (older integrated GPUs) or memory-hungry (non-tiling GPUs).
  - GPU compute path rasterisation is planned, "possibly as a hybrid approach".
  - Dawn abstracts Metal, Vulkan and D3D12.
- [I] Graphite's "depth = painter's order" plus "a few uber-ish pipelines" matches GPUI's
  BoundsTree ordering and Zed's 2026 depth experiment (§4.1). Expect convergence on this pattern.

### 1.6 Rive renderer
- [D] Rive blog, "Rive Renderer now open source", 2024-03-19,
  https://rive.app/blog/rive-renderer-now-open-source-and-available-on-all-platforms. It describes
  "a novel geometric reduction of antialiased vector paths into unique triangle patches" on the
  normal raster pipeline. There are no benchmarks.
- [D] Rive C++ renderer docs, https://rive.app/docs/runtimes/cpp/renderers.md. The modes are:
  - **Pixel Local Storage** on Apple and modern mobile GPUs.
  - **Fragment Shader Interlock** on NVIDIA and Intel.
  - **Atomic** mode (`disableRasterOrdering`).
  - An **MSAA** fallback.
  - Backends are D3D11/12, Metal, Vulkan and GL/WebGL2.
- [D] "Introducing vector feathering", https://rive.app/blog/introducing-vector-feathering: a new
  **Clockwise** fill rule enables feathering, an analytic soft edge without blur.
- [I] Rive needs raster-ordered or pixel-local storage for its best path. wgpu exposes neither
  portably, so the design is not directly reproducible on wgpu. The atomic mode is closest
  (storage-texture atomics).

### 1.7 Slug (Eric Lengyel)
- [D] Lengyel, "A Decade of Slug", 2026-03-17, https://terathon.com/blog/decade-slug.html:
  - **The patent (US 10,373,352) is dedicated to the public domain from 2026-03-17.**
  - Reference vertex and pixel shaders are MIT on GitHub (EricLengyel/Slug).
  - Slug renders glyphs and vectors directly from Bézier data per pixel, with no texture cache.
  - The band-split optimisation was removed: it caused divergence that hurt small text.
  - Adaptive supersampling was removed.
  - "Dynamic dilation" expands each glyph polygon by half a pixel in the vertex shader.
  - Colour emoji are stacked per-layer glyphs.
  - The post does not discuss hinting.
- [D] Original paper: "GPU-Centered Font Rendering Directly from Glyph Outlines", JCGT 2017.
- [I] Fit:
  - Excellent for text and vector symbols on **zoomable, rotatable 2D canvases and in 3D
    viewports**: dimension text, net labels, silkscreen, pin names.
  - Not a replacement for hinted, atlas-cached UI text at 11–13 px on 96-DPI screens, where you
    want the OS rasteriser's grid fitting and gamma behaviour.

### 1.8 Pathfinder
- [D] Pathfinder 3 README (mirrors, e.g. https://github.com/pcwalton/pathfinder):
  - A CPU tiling prepass (Rayon), then GPU raster.
  - Exact area coverage ("effectively 256x AA").
  - Tile occlusion culling.
- [D] Behdad Esfahbod, "State of Text Rendering 2024" (2024-07), https://behdad.org/text2024/:
  Pathfinder is "by far the most advanced rasterizer" but "does not seem to be under active
  development".
- [D] Zed found it too slow for UI (2023 blog, above).

### 1.9 Fit matrix (inferred from the above)

| Workload | Best fit | Why |
|---|---|---|
| Dense UI chrome: rects, borders, shadows, separators, selection | Instanced SDF quads (GPUI/WebRender style) | One draw per kind, exact AA, trivial CPU cost, cacheable instance buffers |
| UI text and icons | Glyph/icon atlas, alpha masks, instanced sprites | Hinting, gamma control, near-zero GPU cost |
| Occasional UI paths (charts, curves, spinners) | MSAA path into an intermediate (GPUI) or Vello GPU for a sub-scene | Rare; quality is enough |
| Schematic/PCB canvas, 10^5–10^6 primitives | Retained GPU buffers of *typed* instances (segments, arcs, circles, pads, polygons) drawn by analytic shaders; polygons tessellated once (lyon) or sparse strips | Re-encoding 10^6 paths per frame is the main cost; keep geometry on the GPU and change only the view uniform on pan and zoom |
| Canvas text (net labels, refdes) at arbitrary zoom | Slug-style analytic glyphs or an MSDF atlas for labels | Scale-independent; small-size hinting matters less in a zoomed canvas |
| Arbitrary SVG, gradients, blend modes | Vello GPU / Vello CPU | General imaging model |
| 3D viewport | Own wgpu pipeline into a texture or its own compositor layer | §5.6 |

---

## 2. Text

### 2.1 Glyph atlas with subpixel positioning
- [D] Zed blog (2023-03-07):
  - Rasterisation by the OS (CoreText on macOS), stored as alpha-only.
  - Up to 16 subpixel variants per glyph (in 2023), packed with `etagere`, drawn by one instanced
    draw call.
  - Shaped glyphs are cached per text and font pair, with eviction.
- [D] Current GPUI: `SUBPIXEL_VARIANTS_X: u8 = 4`, `SUBPIXEL_VARIANTS_Y: u8 = 1`. Glyph origin x is
  quantised with `round_half_toward_zero(x*4)/4` (`crates/gpui/src/text_system.rs`, `window.rs`,
  read 2026-10-25).
- [D] Zed PR #37167 (merged 2025-09-01), https://github.com/zed-industries/zed/pull/37167. Review
  notes: "Zed uses 4 subpixel variants, while Direct2D uses 8", Direct2D rounds rather than floors,
  and Direct2D does not use y subpixel positioning.
- [D] cosmic-text 0.19.0 (docs 2026-09-23), https://docs.rs/cosmic-text/latest/cosmic_text/:
  - `SubpixelBin` bins subpixel positions for the cache.
  - Shaping by harfrust, discovery by fontdb.
  - Optional swash rasterisation (`SwashCache`).
  - `ShapeRunCache`.
- [D] glyphon (cosmic-text + etagere + wgpu), https://docs.rs/crate/glyphon/latest:
  - Atlas starts at 256 and grows by re-uploading.
  - Subpixel offsets come from cosmic-text bins.
- [D] xterm.js 5.1.0 notes, https://github.com/xtermjs/xterm.js/discussions/4334:
  - The atlas moved from one 1024² page to multiple **512x512 pages** merged up to 4096².
  - Smaller pages upload faster, and a 4096² RGBA page is 16 MB.
- [I] Recommendation: 4 horizontal subpixel bins (3 or 4 is the common choice), 1 vertical bin, and
  y snapped to whole device pixels. That is GPUI's choice, and GTK's when hinting (below).

### 2.2 Grayscale vs subpixel (LCD) AA
- [D] GTK blog, "On fractional scales, fonts and hinting", Matthias Clasen, 2024-03-07 (GTK 4.14),
  https://blog.gtk.org/2024/03/07/on-fractional-scales-fonts-and-hinting/:
  - GTK 4 never uses LCD AA, "since our compositing does not have component alpha". Font AA is
    always grayscale.
- [D] WebRender text doc (gecko-dev mirror),
  https://github.com/mozilla/gecko-dev/blob/66ce9f5cbc6578f4fb7b5e0181b6c234ad40d558/gfx/wr/webrender/doc/text-rendering.md:
  - Subpixel text masks carry a separate alpha per colour channel.
  - Blending needs dual-source blending, two passes, or a constant-colour trick.
  - Gamma and linear pre-blend live in the rasteriser backends.
- [D] Chromium needs an **opaque background** for LCD text and falls back to grayscale for
  composited, animated, transformed or non-opaque layers. Igalia, 2021-07-20,
  https://blogs.igalia.com/obrufau/2021/07/20/improved-raster-transforms-with-non-uniform-scales.html.
- [D] GPUI now has `SubpixelSprite`, enabled only when `dual_source_blending` is available
  (gpui_wgpu source). wgpu's `Features::DUAL_SOURCE_BLENDING` is supported on Vulkan
  (`dualSrcBlend`), Metal (MSL 1.2+), DX12, GLES (with an extension) and WebGPU
  (https://docs.rs/wgpu/latest/wgpu/struct.Features.html, wgpu 30.0.1).
- [D, community] macOS 10.14 Mojave disabled subpixel AA by default
  (`CGFontRenderingFontSmoothingDisabled`). There is no Apple statement, only community sources,
  e.g. https://stanislas.blog/2018/09/how-to-fix-font-rendering-macos-10-14-mojave/.
- [I] Recommendation:
  - **Grayscale everywhere by default,** with gamma and contrast correction.
  - Optional **LCD path on Windows only,** for text drawn over an opaque, known background inside an
    opaque layer: editor, tables, property grids. Use dual-source blending and fall back to
    grayscale on transparent, animated or scaled layers.
  - LCD text never goes into cached textures that might be composited at sub-pixel offsets.

### 2.3 Gamma and contrast tricks
- [D] Zed PR #37167 (2025-08-29 → merged 2025-09-01):
  - Rasterise glyphs as grayscale (`DWRITE_TEXTURE_ALIASED_1x1`).
  - Read gamma from DirectWrite `GetGamma` and contrast from `GetGrayscaleEnhancedContrast`.
  - Apply them in a shared `alpha_correction.hlsl` taken from **Windows Terminal's `dwrite.hlsl`**
    (with attribution).
  - Use 96 DPI in `GetRecommendedRenderingMode` to re-enable grid fitting.
  - Use premultiplied alpha for emoji layers.
- [D] Zed PR #38862 (merged 2025-09-25), https://github.com/zed-industries/zed/pull/38862: ported to
  Linux with `ZED_FONTS_GAMMA` (range 1.0–2.2, default **1.8**) and
  `ZED_FONTS_GRAYSCALE_ENHANCED_CONTRAST` (default **1.0**). A reviewer raised that the result
  depends on the framebuffer colour space (linear vs sRGB).
- [D] Chrome blog, "Better text rendering in Chromium-based browsers on Windows", 2025-02-12,
  https://developer.chrome.com/blog/better-text-rendering-in-chromium-based-browsers-on-windows:
  - Skia ignored the ClearType Tuner contrast and gamma, and its defaults differed from DirectWrite.
  - A text contrast of **1.0** "closely matched … pre-Chromium Edge" and became the default in
    **Chrome 132**.
- [D] Edge blog, 2021-06-02, https://blogs.windows.com/msedgedev/2021/06/02/improving-font-rendering-in-microsoft-edge/:
  ClearType contrast in the registry ranges 50–400, default 100.
- [D] Behdad, State of Text Rendering 2024: common GUI toolkits "do not render text correctly" for
  lack of linear blending and gamma correction, so text looks worse on bright and dark backgrounds
  than on Windows and macOS.
- [I] Recommendation:
  - Store linear coverage in an R8 atlas.
  - In the fragment shader, apply DirectWrite-style correction: a gamma-ratio polynomial plus
    enhanced contrast, with a luminance-dependent adjustment for light-on-dark text. On Windows, read
    the user's values from `IDWriteRenderingParams`. Use the same constants on Linux.
  - On macOS, take CoreText's output as is, since it is already tuned. Disable extra correction, or
    use a mild one, on HiDPI.
  - Make it a theme or setting knob (as Zed does), not a constant.

### 2.4 SDF and MSDF text
- [D] Qt `QSGTextNode::RenderType` (Qt 6.7+), https://doc.qt.io/qt-6/qsgtextnode.html:
  - `QtRendering` (distance field) is the faster option, but "uses more memory and will exhibit
    rendering artifacts at large sizes".
  - `NativeRendering` looks native but gives poor results under transforms.
  - `CurveRendering` (6.7) rasterises curves on the GPU and is recommended where SDF looks bad or to
    save memory.
- [D] Qt blog, "Text Rendering in the QML Scene Graph" (2011-07-15),
  https://qt.io/blog/2011/07/15/text-rendering-in-the-qml-scene-graph: hinting matters most at low
  pixel density and small sizes, and distance-field glyphs are rendered without grid-fitting, to stay
  scalable.
- [D] Behdad 2024 survey:
  - SDF approaches: freetype-gl, GLyphy (no overlapping contours) and msdfgen.
  - Qt offers atlases, SDF atlases and curve data.
- [I] For 11–13 px UI text on 96-DPI Windows, SDF gives soft stems, no hinting and wrong stem
  weights. Use SDF, MSDF or Slug only for canvas or 3D text that zooms or rotates.

### 2.5 Hinting
- [D] GTK 4.14 post (2024-03-07). Two modes:
  - Uniform spacing: unhinted, with subpixel positioning.
  - Crisp: hinted, at integral positions.
  - With hinting on, GTK rounds only **y** to device pixels, because the autohinter hints only
    vertically.
- [D] GTK 4.16:
  - New `gtk-font-rendering` setting, default `GTK_FONT_RENDERING_AUTOMATIC`, which considers screen
    resolution and scale (https://docs.gtk.org/gtk4/property.Settings.gtk-font-rendering.html).
  - `gtk-hint-font-metrics` defaults to TRUE and "turns off subpixel positioning of glyphs, since it
    conflicts with metrics hinting" (https://docs.gtk.org/gtk4/property.Settings.gtk-hint-font-metrics.html).
  - The default change came via a GTK merge request (https://gitlab.gnome.org/GNOME/gtk/-/merge_requests/7157.diff).
- [D] skrifa 0.49.0, https://docs.rs/skrifa/latest/skrifa/: hinted or unhinted scaling, using the
  font's TrueType instructions, PostScript hints, or an automatic hinter. Behdad (2024) notes Chad
  Brokaw porting FreeType's TrueType hinter, CFF hinter and autohinter to fontations.
- [I] Recommendation:
  - At scale 1.0 on Windows and Linux, use light, vertical-only hinting (autohinter or DirectWrite
    grid-fit). Snap y and baselines to device pixels, keep x subpixel-positioned, and keep advances
    unhinted, so layout does not change with DPI.
  - At HiDPI (≥ 1.75x) and on macOS, use no hinting.

### 2.6 Shaping, layout and font libraries (Rust, 2026)

**harfrust**
- [D] HarfRust 0.13.2 matches HarfBuzz 14.3.1 (https://docs.rs/crate/harfrust/0.13.2).
- It is a RustyBuzz fork on read-fonts (fontations), with no unsafe.
- HarfBuzz project: HarfRust 0.2.0 was "less than 25% slower than HarfBuzz" on OpenType and AAT
  shaping benchmarks [M]. That figure is from search-result snippets, from a post whose date was not
  shown.
- RustyBuzz is archived and recommends HarfRust.

**skrifa / fontations**
- [D] Chrome blog, "Memory safety for web fonts", 2025-03-19, https://developer.chrome.com/blog/memory-safety-fonts:
  - Chrome replaced FreeType with skrifa (fontations) for web fonts.
  - Chrome 128 used it for COLRv1 and CFF2.
  - Chrome 133 used it for all web fonts on Linux, Android and ChromeOS.
  - Output is pixel-compared against FreeType.

**swash**
- [D] swash 0.2.10 (2026-09-28), https://docs.rs/swash/latest/swash/: shaping, scaling, hinting and
  rasterising.

**parley and fontique**
- [D] parley 0.12.0, https://docs.rs/parley/latest/parley/: rich text layout with shaping, line
  breaking, bidi and alignment. Line breaking can be re-run without re-shaping. `LayoutContext`
  provides reusable scratch, and there is an `editing` module.
- [D] fontique 0.12.0, https://docs.rs/fontique/latest/fontique/: font enumeration and fallback
  (`FallbackKey` by script and language), CSS generic families, and `SourceCache`.
- [D] Linebender Q1 2026 (2026-04-19):
  - Parley gained CoreText enumeration and AccessKit text properties.
  - Bevy, Gosub and others adopted it.
  - In Dec 2025, text analysis migrated to ICU4X.

**cosmic-text**
- [D] cosmic-text 0.19.0: harfrust shaping, fontdb, a `Fallback` trait, and `Editor`/`SyntaxEditor`.

**Choice**
- [I] Choose parley + fontique + harfrust (via parley) for layout, and skrifa for outlines and
  hinting. Rasterise with the platform where it matters: DirectWrite on Windows and CoreText on
  macOS, behind one trait, as GPUI does. Use skrifa plus a CPU rasteriser on Linux.
- Platform rasterisers are what users compare against, and they carry the user's ClearType and
  font-smoothing settings.

### 2.7 Line-layout caching for code editors
- [D] GPUI `LineLayoutCache` (`crates/gpui/src/text_system/line_layout.rs`, read 2026-10-25):
  - It keeps `previous_frame` and `current_frame` caches.
  - Lines used this frame are moved from previous to current (`reuse_layouts`). Whatever is not
    reused is dropped at frame end, which is frame-generational eviction.
  - Keys include text, font runs, font size and wrap width (commit history via
    https://git.secluded.site/zed/commit/9e7a579365f44b0b0f02427bb60e858ce73c8057).
  - The cache is per window (commit a0b52cc).
- [D] GPUI also **replays cached paint ranges** of unchanged views (`reuse_paint` / `scene.replay`
  in `window.rs`). The CPU skips re-painting subtrees, but the GPU still redraws the full frame.
- [D] Raph Levien, "xi-editor retrospective", 2020-06-27, https://raphlinus.github.io/xi/2020/06/27/xi-retrospective.html:
  - "GPU acceleration [is] essentially required for good GUI performance".
  - The async front-end/core split made line-cache and word-wrap-on-resize very hard. Having the text
    as an in-process structure would have been "quite straightforward".
  - Monolithic was better.
- [D] VS Code 1.97 (Feb 2025) previewed WebGPU rendering in the editor (devclass,
  https://devclass.com/2025/02/07/vs-code-update-treats-copilot-as-out-of-the-box-feature/). No
  primary design doc was found.
- [I] Recommendation:
  - Cache shaped lines keyed by (content hash, style-run hash, font set, size, wrap width), with
    two-generation eviction (GPUI) plus an LRU cap for off-screen lines in big files.
  - Shape per visible line only, plus the overscan band.
  - Keep the shaped glyph list separate from positioned and wrapped output. Wrap changes then
    re-break only (parley supports re-running line breaking).

### 2.8 Emoji and colour fonts
- [D] Behdad 2024: the formats are sbix, CBDT/CBLC, COLR v0/v1 and SVG. COLRv1 (around 2022) adds
  gradients and variations, and Chrome and Android ship it.
- [D] skrifa models "color and bitmap glyphs".
- [D] Zed PR #37167: emoji layers rasterised grayscale per layer, with premultiplied alpha, because
  straight alpha "is not associative".
- [D] GPUI uses `PolychromeSprite` (RGBA atlas) for emoji and images, separate from
  `MonochromeSprite` (R8, tintable).
- [I] Use two atlases (R8 mask and RGBA premultiplied) and rasterise COLRv1 to RGBA at the size
  needed. Slug's per-layer approach is the vector alternative for canvases.

### 2.9 Font fallback
- [D] fontique: system enumeration plus script and locale fallback (§2.6).
- [I] Precompute fallback chains per (script, locale, family) and cache by codepoint run.
  Fallback lookup is otherwise a hidden per-frame cost in tables with mixed scripts.

---

## 3. Retained rendering, caching and damage

### 3.1 Display lists recorded once and replayed
- [D] GPUI: a frame produces a `Scene` of typed primitive vectors, sorted by draw order then texture
  (`scene.rs` `finish`). Unchanged cached views replay their range of the previous scene.
- [D] Graphite: Recordings can be "reissued with dynamic changes like translation" (2025-07-08
  post).
- [D] Chromium cc: content arrives as `cc::PaintRecord` / `DisplayItemList` and is rastered into
  tiles. "How cc Works", https://chromium.googlesource.com/chromium/src/+/main/docs/how_cc_works.md.

### 3.2 Layer and picture caching

**WebRender**
- [D] `webrender/src/picture.rs` module docs, https://doc.servo.org/src/webrender/picture.rs.html:
  - The scene is cut into a few **slices** (content, UI, background UI).
  - Tiles are 2048x512 for content and **128x128 for the UI slice**.
  - A tile is cached raster or a solid "clear tile".
  - Each tile tracks dependencies (primitives, clips, image keys, opacity bindings, transforms),
    rebuilt per frame and **compared with the previous frame**.
  - A per-tile quadtree's invalidated leaves form a **dirty rect**. It is the scissor when re-drawing
    the tile and can drive partial present.
  - Large or fast-changing opaque primitives (video, WebGL) are promoted to **native compositor
    surfaces**, with tile cut-outs and "overlay tiles" drawn in alpha mode when content overlaps.

**Failure modes in that design**
- [D] Bug 2052391 (2026): invalidation keys quantised to 1/4 px left stale tiles. Two corners
  rendering to different device pixels shared a key; fixed in Firefox 154.
  https://bugzilla.mozilla.org/show_bug.cgi?id=2052391
- [D] Bug 2064321: wrong content baked into a tile "with nothing left to invalidate it".
- [I] Lesson: exact invalidation keys in device pixels, and debug overlays that flash re-rastered
  tiles.

**Chromium**
- [D] "How cc Works": software-raster tiles are about 256x256. GPU-raster tiles are about viewport
  width x a quarter of viewport height.
- [D] Damage is "draw invalidation" (invalidation plus expose damage), computed by `DamageTracker`
  and used for partial swap, which "saves power".

### 3.3 Damage tracking and partial present per platform

**Windows (DXGI)**
- [D] DXGI 1.2 docs, "Flip model, dirty rectangles, scrolled areas",
  https://learn.microsoft.com/en-ie/Windows/Win32/direct3ddxgi/dxgi-1-2-presentation-improvements:
  - `IDXGISwapChain1::Present1` takes dirty rects plus **one scroll rect and offset**.
  - These "save on the usage of memory bandwidth and the related usage of system power". DWM uses
    the metadata.
  - The app must redraw all of each declared dirty region.
  - Not supported with DISCARD or SEQUENTIAL swap effects, or with multisampled swap chains.

**Vulkan and EGL**
- [D] Vulkan: `VK_KHR_incremental_present`. GTK 4 implemented it in 2019,
  https://feaneron.com/?p=4190 (Georges Stavracas, 2019-10-05).
- [D] EGL:
  - `EGL_EXT_buffer_age` gives the back buffer's age in frames, so you redraw the union of damage
    since then.
  - `EGL_KHR_partial_update` declares the write region up front, so tilers skip loading the rest.
  - Swap-with-damage reports surface damage.
  - The Weston and Mesa discussion is summarised at https://gitlab.freedesktop.org/wayland/weston/issues/134
    and https://www.phoronix.com/news/Weston-EGL_KHR_partial_update.

**macOS (Metal / Core Animation)**
- [D] No public damage API exists for `CAMetalLayer`. The Zed PR #59441 author: "Metal has no API
  for announcing damaged regions".
- [D] Zed PR #53932 kept a persistent render target and scissored partial redraws on macOS, then
  presented the whole drawable.
- [I] The OS compositor still recomposites the layer. Most of the savings come from fewer GPU
  fragments, and you can also split the UI into several CALayers.

**wgpu**
- [D] wgpu issue #1397 ("Incremental presentation") was closed in Oct 2023 in favour of tracking
  issue **#2869** ("Extended Presentation API Investigation", open), https://github.com/gfx-rs/wgpu/issues/2869.
- [D] Activity on #2869:
  - 2024-05: jimblandy says partial present matters to the Xilem team.
  - 2026-07-08: Zed's results, blocked on upstream.
  - 2026-08-14: cwfitzgerald says a damage-tracking PR "would be accepted!", and a `present_with_damage`
    approach is "fine" for V1.
- [D] wgpu 30.0.0 (2026-07-01) replaced `SurfaceTexture::present()` with
  `Queue::present(surface_texture)` (wgpu CHANGELOG).

**Zed's measurements** [M]
- [D] PR #59441, "gpui: Add damage-tracking with partial redraw (WIP)" (2026-06-16, closed
  2026-08-08), https://github.com/zed-industries/zed/pull/59441:
  - Damage comes from diffing consecutive scenes by prefix and suffix match, so a blinking cursor
    damages only its own bounds.
  - Only damage is redrawn into a persistent target, which is copied to the swapchain and presented
    with damage. Unchanged frames skip present entirely.
  - On sway with an iGPU at 4K:
    - GPU 50–60% → 5–10%.
    - GPU power 2–3 W → 0.3–5 W; package power 12–13 W → 8–9 W.
    - Zed CPU 80–90% → 30–50%.
  - A depth prototype alone gave about 20–30% GPU; depth plus damage gave about 3–5%.
- [D] PR #53932 (2026-04-14, closed 2026-07-02) added `notify_with_damage(bounds)`. A reviewer found
  wrong damage on mid-string inserts and sidebar scroll.
- [I] Lesson: compute damage by **diffing the retained scene** (or by dependency tracking, as
  WebRender does), not by hand-annotated notifications.

### 3.4 When caching to textures beats re-drawing
- [I] Re-drawing a UI frame of about 10^4 instanced quads and glyphs costs well under 1 ms of GPU on
  any discrete GPU. Zed's representative scene took 0.958 ms before depth culling and 0.439 ms after
  [M, PR #61461].
- [I] Texture caches cost memory (a 2560x1440 RGBA layer is 14.7 MB) and need correct invalidation.
- [I] They pay off for:
  - (a) expensive content that changes rarely: blurred backdrops, complex vector icons, schematic
    symbol bodies at a fixed zoom, a PCB canvas between edits;
  - (b) power: skipping draws entirely on idle frames;
  - (c) **scrolling** large static content, where a blit or offset beats re-raster.
- [I] They do not pay off for cheap analytic UI primitives. Damage plus scissor is better there.
- [D] WebRender's choice is a picture cache with small UI tiles (128²) plus dirty rects. It shows
  that both can be combined.

### 3.5 Scrolling by blit or offset
- [D] DXGI `Present1` scroll rect plus offset (above): the OS moves the pixels.
- [D] Chromium scrolls on the compositor thread through property-tree changes without involving
  Blink ("How cc Works").
- [D] Graphite re-issues recordings with translation (above).
- [D] Firefox's native compositor (DirectComposition, CoreAnimation, wl_subsurface) means "less
  copying work for the GPU, especially when scrolling, where partial present doesn't help much"
  (search summary of Mozilla sources; `CompositorConfig`,
  https://doc.servo.org/webrender/composite/enum.CompositorConfig.html; bug 1581650).
- [I] Recommendation:
  - Scroll containers with a lot of content (code editor, long tables, big trees) render into a
    per-container tile cache: tiles of about 256–512 px tall at device resolution. Scrolling then
    offsets tiles and rasterises only newly exposed tiles.
  - Short lists just redraw.
  - Keep LCD text out of these caches, or snap scroll offsets to whole device pixels.

---

## 4. Batching and draw-call minimisation

### 4.1 Z-order vs batch breaking

**GPUI**
- [D] Each primitive's draw order = 1 + the max order of already-inserted bounds it intersects.
  This is an R-tree `BoundsTree` with branching 12 (`crates/gpui/src/bounds_tree.rs`).
- Per-kind vectors are then sorted by (order, texture), and a `BatchIterator` merges kinds by order.
- Non-overlapping primitives share low orders, so long batches form even across kinds.

**Zed depth experiment**
- [D] PR #61461, "Reduce overdraw in GPUI by using the depth buffer" (2026-07-22, closed pending
  all-platform work), https://github.com/zed-industries/zed/pull/61461:
  - Opaque solid quads go **front-to-back with depth write**. Rounded quads contribute their opaque
    core.
  - Everything else goes back-to-front with depth test and no write.
  - GPU busy time 0.958 ms → **0.439 ms** [M].

**Graphite and WebRender**
- [D] Graphite: depth = painter's order; clips are depth-only draws (§1.5).
- [D] WebRender talk (MadRust, 2018-04-19), https://www.slideshare.net/slideshow/webrender-madrust/94726063:
  opaque front-to-back, then translucent back-to-front. "Find a good balance between aggressive
  batching and opportunities for using the depth buffer."
- [D] The older Gecko Advanced Layers z-buffer occlusion was off by default, being "significantly
  costly on integrated GPUs"
  (https://github.com/cliqz-oss/browser-f/blob/DB-2451_2/mozilla-release/gfx/docs/AdvancedLayers.rst).
- [I] Measure the depth pre-pass on iGPUs. It helps when there are large opaque panels over other
  content (docked panels, modal backdrops). Damage tracking makes it matter less.

### 4.2 Clips
- [D] GPUI: every primitive carries a rect `content_mask`, applied in the shader. Primitives fully
  outside are culled at insert (`scene.rs`).
- [D] WebRender: clip masks are rendered into a mask cache in 128 px tiles and sampled by brushes.
  In WR1, "four clip masks in the texture, one per channel" was considered.
- [D] Graphite: depth-only clip draws. Impeller: stencil clips (issue #123671).
- [D] Vello GPU 0.3.0: `push_clip_rect` (non-isolated rect clip) was added as a fast path.
- [I] Recommendation:
  - Rect clips in the instance data, intersected on the CPU. This covers about 99% of UI.
  - Rounded-rect clips as an analytic SDF in the instance: one rounded clip per primitive, enough
    for rounded panels and scroll views.
  - Arbitrary-path clips only via a mask texture (rare).
  - Never use the scissor rect per primitive, because that breaks batches. Use the scissor only per
    damage rect or per layer.

### 4.3 Texture atlas management
- [D] nical, "Etagere" post, 2021-02-04, https://nical.github.io/posts/etagere.html:
  - Shelf allocators: bucketed for many small items, or simple with per-item tracking.
  - Guillotiere packs best but is slowest with thousands of items and can fragment.
  - Firefox 86 uses the simple shelf allocator (two columns per texture) for glyph and image caches.
  - The glyph cache usually needs 1–2 textures.
  - Guillotine is kept for single-frame atlases.
- [D] etagere 0.3.0 docs, https://docs.rs/etagere/latest/etagere/.
- [D] GPUI uses etagere (Zed blog). xterm.js uses 512² pages merged to a 4096² maximum.
- [I] Recommendation:
  - Pages of 1024² or 2048², an R8 mask atlas and an RGBA atlas, with an etagere bucketed allocator
    for glyphs and the simple allocator for icons and images.
  - Evict by frame-generation LRU.
  - Upload dirty sub-rects only, through one staging buffer per frame.

### 4.4 Buffer upload strategies
- [D] wgpu 30.0.1 `Queue::write_buffer`: staging is copied immediately. "Currently on native
  platforms … the staging memory will be a new allocation". The transfer starts at the next submit.
  `write_buffer_with` avoids one copy. There is no mapped texture.
  https://docs.rs/wgpu/latest/wgpu/struct.Queue.html
- [D] `wgpu::util::StagingBelt` is a ring of sub-allocated staging buffers for "many buffer writes".
  The chunk should exceed any single write and be about ¼ to 1x of per-submit upload. Steps: finish,
  submit, recall. https://docs.rs/wgpu/latest/wgpu/util/struct.StagingBelt.html
- [D] wgpu 0.15 DX12 sub-allocation saw a "100x improvement … in a simple scene with 200
  `write_buffer` calls per frame" (wgpu CHANGELOG) [M].
- [D] GPUI: one storage buffer per frame for all instance arrays, bound with offsets.
- [I] Recommendation:
  - One growable instance storage buffer per frame-in-flight, written with one
    `write_buffer_with` or `StagingBelt` copy.
  - Retained layers keep their own persistent GPU buffers, uploaded only on change.
  - Small per-draw data (layer transform, clip index) goes in immediates
    (`Features::IMMEDIATES`, wgpu 30) or a dynamic-offset uniform.

### 4.5 Pipeline caching and shader precompilation
- [D] wgpu 30.0.1 `PipelineCache` works **only on Vulkan**. "Most desktop GPU drivers will manage
  their own caches", so the benefit is mainly on Android. Store the data atomically.
  https://docs.rs/wgpu/latest/wgpu/struct.PipelineCache.html (added in wgpu 22, PR #5319).
- [D] Impeller compiles all shaders offline and builds all PSOs up front. Graphite consolidates
  pipelines so Chrome can compile them at startup.
- [I] Recommendation:
  - A closed set of about 8–12 render pipelines. Create them all at startup on a background thread,
    before the first frame, with WGSL embedded.
  - Use a Vulkan `PipelineCache` on disk where available.
  - No runtime shader generation per style. Styles are instance data.

---

## 5. wgpu specifics

### 5.1 Version
- [D] wgpu 30.0.1 (2026-08-21) is current. 30.0.0 was 2026-07-01 and 29.0.0 was 2026-03-18 (wgpu
  CHANGELOG, `trunk`, https://raw.githubusercontent.com/gfx-rs/wgpu/trunk/CHANGELOG.md).
- egui 0.37 (cadatomic) predates or tracks a wgpu in this range. Vello GPU 0.3.0 is on wgpu 30.

### 5.2 Present modes
- [D] wgpu 30.0.1 `PresentMode`, https://docs.rs/wgpu/latest/wgpu/enum.PresentMode.html:
  - **Fifo**: the default and supported everywhere. The queue is about 3 frames, and
    `get_current_texture` blocks.
  - **FifoRelaxed**: AMD on Vulkan.
  - **Immediate**: tears; not on older DX12 or Wayland.
  - **Mailbox**: single-slot replace. DX12 on Windows 10, NVIDIA Vulkan, Wayland Vulkan.
  - `AutoVsync` / `AutoNoVsync` choose with fallback.
  - The `SurfaceConfiguration` docs say "Fifo is the only mode guaranteed", and unsupported modes
    crash.

### 5.3 Frame latency knobs
- [D] wgpu 0.19 added `desired_maximum_frame_latency` (previously hard-coded 2). "By setting it to 1
  you can reduce latency under the risk of making GPU & CPU work sequential." On DX12 it sets
  `MaximumFrameLatency`; elsewhere it sets swapchain size; it has no effect on GL (CHANGELOG, PR
  #4899). The 30.0.1 docs call it "a hint … clamped".
- [D] DXGI: `GetFrameLatencyWaitableObject` needs the
  `DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT` flag. `SetMaximumFrameLatency` works only on
  waitable swap chains (default 1); without the flag the effective latency is 3.
  https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_3/nf-dxgi1_3-idxgiswapchain2-getframelatencywaitableobject
- [D] `CAMetalLayer.maximumDrawableCount` may be 2 or 3 only; the default is 3.
  https://developer.apple.com/documentation/quartzcore/cametallayer/maximumdrawablecount
- [D] `presentsWithTransaction`:
  - Default false, which presents asynchronously to Core Animation transactions.
  - True presents synchronously with the current CA transaction. Commit, then `waitUntilScheduled`,
    then `present()`.
  - Needed when Metal content must line up with other CA content in the same frame (window resize,
    overlaid native views).
- [D] `CAMetalDisplayLink` (macOS 14+) has `preferredFrameLatency` and `preferredFrameRateRange`.
  The callback asks for frames at the right time. https://developer.apple.com/documentation/quartzcore/cametaldisplaylink
- [D] An Apple forum thread reports that `nextDrawable` can stall about 1 s after a `drawableSize`
  change. DTS recommends CAMetalDisplayLink. https://www.developer.apple.com/forums/thread/773229
- [D] Raph Levien, "Swapchains and frame pacing", 2021-10-22, https://raphlinus.github.io/ui/graphics/gpu/2021/10/22/swapchain-frame-pacing.html:
  - A compositor adds about 1 frame of latency.
  - Avoid a 2-image swapchain with blocking present when latency matters.
  - On DXGI, use the waitable object plus `SetMaximumFrameLatency` (1 if the work fits in a frame).
  - Start rendering at deadline minus the high-percentile render time.
  - Measure input-to-photon latency on hardware (PresentMon, Instruments, Perfetto).
- [I] Recommendation for cadatomic:
  - Fifo with latency 1 on the UI surface when frames are cheap (they will be, with damage).
  - Event-driven: render only on input, animation or data change; no continuous loop.
  - Pace with CAMetalDisplayLink on macOS, the waitable object on Windows, and `wl_surface.frame`
    callbacks on Wayland.
  - Sample input as late as possible before encoding.

### 5.4 Surface creation
- [D] wgpu 30 `SurfaceTargetUnsafe`: `RawHandle` (raw-window-handle) and `Drm`; non-exhaustive.
  https://docs.rs/wgpu/latest/wgpu/enum.SurfaceTargetUnsafe.html
- [D] wgpu 27.0.0 (2025-10-01): **built-in DXGI swapchains on DirectComposition visuals**
  (`Dx12BackendOptions { presentation_system: Dx12SwapchainKind::DxgiFromVisual }`) enable
  transparent windows. Apps managing their own composition tree pass a visual through
  `SurfaceTargetUnsafe::CompositionVisual` (PR #7550). A visual target existed since wgpu 0.13 (PR
  #2434), and WinUI 3 `SwapChainPanel` since 0.18 (PR #4191).
- [I] The variant list shown on the 30.0.1 page has only `RawHandle` and `Drm`. Check whether
  `CompositionVisual` is still public, or feature-gated, before relying on it.
- [D] wgpu 23: unless an explicit `CAMetalLayer` is given, surfaces render to a sublayer, which
  fixes resize glitches (PR #6107).
- [D] Unreleased (after 30.0.1):
  - Non-opaque `CAMetalLayer` advertises `PreMultiplied` (breaking).
  - sRGB colour space is set on the layer to fix oversaturation on wide-gamut displays.

### 5.5 Multi-window and multi-surface cost
- [I] No authoritative wgpu measurements were found.
- [I] Each surface has its own swapchain images: VRAM about (2–3) x W x H x 4 bytes each.
- [I] With Fifo, presenting several windows from one thread can serialise on vblank waits, because
  `get_current_texture` blocks. So:
  - acquire all surfaces first and submit once;
  - or drive each window from its own display-link callback;
  - and render only windows with damage.
- [D] wgpu 22: `.submit()` became 39–64% faster (`submit` + `poll` 22–32%), PR #5910 [M].
- [D] wgpu 27: command encoding is deferred to `CommandEncoder::finish` (PR #8220).
- [I] No authoritative per-draw overhead numbers were found for wgpu vs native. Treat it as
  "noticeably higher than raw Vulkan or Metal per call because of validation and tracking". Design
  for ≤ 50 draws per UI frame, so it does not matter.

### 5.6 3D viewports and big canvases in the frame

**Option A: same frame**
- [I] Render the viewport into an offscreen texture with its own pipeline (depth, MSAA), then
  sample it as a UI `Surface` primitive. GPUI has `PaintSurface` for this.
- Simple, with exact z-order with overlays and popovers, and one present.
- Costs one extra full-viewport copy and couples the viewport to the UI frame rate.

**Option B: separate OS compositor layer (zero-copy)**
- [D] macOS: a child `CAMetalLayer` per viewport. Use `presentsWithTransaction` when it must move
  in sync with UI layers (resize, docking animations).
- [D] Windows: DirectComposition visuals with separate DXGI swapchains (wgpu
  `CompositionVisual`/`DxgiFromVisual`).
- [D] Wayland: `wl_subsurface`. It starts in synchronized mode (state applied with the parent's
  commit) and can be set to desync. "The aim of sub-surfaces is to offload some of the compositing
  work within a window from clients to the compositor", e.g. to overlay hardware.
  https://wayland.app/protocols/wayland#wl_subsurface
- [D] Firefox's native compositor uses exactly these three for its tiles and video (§3.5).
- [D] Raph (2021): hardware overlays can save a frame of latency on Windows, but transitions can
  cause jank.

**Recommendation**
- [I] Use B for heavy 3D viewports: orbiting runs at 120 Hz without redrawing the UI, and an idle
  UI costs nothing.
- [I] Use A for small previews, thumbnails and anything overlapped by UI popovers. With B, popovers
  over a viewport need their own layer above it, or a cut-out (WebRender's "overlay tile" pattern).
- [I] Keep B behind a trait: X11 and some Linux paths lack a good subsurface story, so fall back to
  A.

### 5.7 Software fallback
- [D] WARP (Microsoft Learn, updated 2025-07-24), https://learn.microsoft.com/en-us/windows/win32/direct3darticles/directx-warp:
  - A conformant software rasteriser with a JIT (SSE2 through AVX-512, ARM64) and multithreaded,
    deferred rendering. It supports D3D12 FL 12_0/12_1.
  - Historical data: low-end discrete GPUs are 4–5x faster than WARP.
  - Usable for UI, but "lower the quality settings and resolution".
- [D] lavapipe/llvmpipe is a multithreaded CPU rasteriser via LLVM, a fallback only (Phoronix
  history). wgpu CI tests on lavapipe since v0.9 and on WARP since v0.18.
- [D] Vello CPU is a credible software path for vector content (thesis, §1.3).
- [I] Under lavapipe or WARP, per-pixel fragment cost dominates:
  - damage plus scissor becomes essential;
  - avoid MSAA;
  - prefer R8 atlases;
  - skip shadows' sampling loops (use cheaper approximations);
  - optionally drop to 60 Hz.
- [I] cadatomic already runs under lavapipe in WSL and CI (its CLAUDE.md). Make "fallback adapter"
  a tested configuration, with a frame-time budget test.

---

## 6. Icons
- [D] GPUI rasterises SVG icons on the CPU with **resvg/tiny-skia** as **alpha masks** (tintable),
  at `SMOOTH_SVG_SCALE_FACTOR = 2.0`. That is 2x oversampling, then a downscale when drawn
  (`svg_renderer.rs`, `window.rs`). Images go in a separate colour atlas. Zed blog: icons are alpha
  masks so they can be tinted.
- [I] Recommendation:
  - Rasterise each icon at each (device scale, logical size) actually used, with resvg into the R8
    mask atlas. Use RGBA for multicolour icons.
  - Pixel-snap at 1x (icons are designed on a 16 px grid). Cache keys are (icon id, size, scale,
    variant).
  - Render vector at draw time (Vello GPU or analytic) only for icons on zoomable canvases
    (schematic symbols are not icons; see §7).
- [I] cadatomic's own icon kit (`ui/icons/`) produces SVG specs, so resvg rasterisation at load or
  first use fits directly.

---

## 7. 10^5–10^6-primitive CAD canvases (schematic and PCB)

- [D] KiCad GAL (CERN, 2013–2015), https://ohwr.org/project/cern-kicad/wikis/Kicad-gal:
  - VIEW (layers, visibility, zoom), VIEW_ITEMs, PAINTERs, and OpenGL and Cairo backends.
  - The OpenGL backend caches item geometry in GPU buffers (VBO groups). Draw lists are copied into
    mapped GPU memory, and per-group depth handles layer order.
- [D] Instanced lines (Rye Terrell, 2019-11-18), https://wwwtyro.net/2019/11/18/instanced-lines.html:
  - Each segment is an instanced quad, with joins and caps as separate instanced draws sharing one
    buffer.
  - 5 lines x 100,000 points (500k segments) "at interactive speeds" on an old Chromebook [M].
- [I] Architecture:
  1. **Retained GPU geometry per layer and per tile.** Board and schematic primitives are typed
     instance records:
     - segment (p0, p1, width, cap);
     - arc (centre, radius, angles, width);
     - circle or annulus (pad or via);
     - rounded-rect pad;
     - polygon (pre-tessellated once with lyon, or rendered via sparse strips);
     - text (Slug or MSDF glyph instances).
     Each kind gets one analytic SDF shader. Upload once; update by range on edit.
  2. **Pan and zoom change only a view uniform.** Zero CPU work per frame. Copper and silk layers
     are one instanced draw per (layer, kind); about 10 layers x 6 kinds is about 60 draws.
  3. **Culling and LOD:**
     - spatial grid or BVH chunks of about 4–16k primitives;
     - frustum-cull chunks on the CPU (or with an indirect-count compute cull, using
       `MULTI_DRAW_INDIRECT_COUNT` where supported);
     - below a pixel-size threshold, switch text to boxes and tracks to hairlines;
     - drop glyphs under about 3 px.
  4. **Integer-nm coordinates** (Copperfield/ecadabra EQ4) are uploaded as camera-relative i32 or
     f32 offsets per chunk, which avoids f32 precision loss at 10^9 nm extents.
  5. **Static-scene texture cache** for idle frames: the canvas is redrawn only on camera, edit or
     highlight changes. Selection and hover highlights go in a small overlay layer of their own, so
     hovering never re-renders 10^6 primitives.
  6. **DRC markers, ratsnest and router previews** are dynamic overlay buffers, rebuilt per change.
- [I] Order-independent overlapping of translucent copper layers can use per-layer offscreen
  targets composited with layer alpha. This is the KiCad/Altium look, with no per-primitive blending
  order issues.

---

## 8. Recommendation for cadatomic's renderer (inferred, built on the above)

### 8.1 Primitive set (closed, about 10 pipelines, all precompiled at startup)
1. **Quad.** Rect plus per-corner radii, per-edge border widths, border colour or style
   (solid/dashed), fill (solid, linear or radial gradient as two stops plus an optional ramp-texture
   index), and an analytic AA edge. One instanced pipeline. This is GPUI's `Quad` plus gradient
   ramps.
2. **Shadow.** Rounded-rect Gaussian, Evan Wallace closed form.
3. **Line/segment.** Hairlines, separators, tree guides, underlines, squiggles. An instanced capsule
   SDF, with dash parameters.
4. **MaskSprite.** R8 atlas glyphs and icons, tinted, with DWrite-style gamma and contrast in the
   shader.
5. **SubpixelSprite.** Windows-only LCD text on opaque layers, needing `DUAL_SOURCE_BLENDING`.
6. **ColorSprite.** RGBA premultiplied atlas: emoji, images, thumbnails.
7. **Path.** Rare UI vectors. MSAA 4x into an intermediate, or a Vello GPU sub-scene drawn into an
   atlas region and cached.
8. **Surface.** An externally rendered texture (3D viewport, canvas cache, video).
9. **Canvas kinds** (separate pipelines, retained buffers): segment, arc, circle/annulus, pad
   rounded-rect, tessellated polygon, and canvas glyph (Slug or MSDF).

Clips: a rect clip plus one optional rounded-rect clip per instance (analytic). Mask-texture clips
are a rare fallback.

### 8.2 Text pipeline
- Layout: parley (+ fontique fallback, harfrust shaping). Outlines and hinting: skrifa.
- Rasterise with **DirectWrite** (Windows) and **CoreText** (macOS), behind a trait, and skrifa plus
  a CPU rasteriser on Linux, with light vertical hinting at 1x.
- 4x1 subpixel bins with y snapped to device pixels. Grayscale R8 atlas by default.
- Gamma and contrast from the system (DirectWrite `IDWriteRenderingParams`) applied in the shader;
  defaults gamma 1.8, enhanced contrast 1.0.
- Optional LCD on Windows for opaque text regions.
- Shaped-line cache with two-generation eviction (GPUI) plus an LRU cap. Re-break without re-shape
  on wrap-width change.
- Canvas text uses analytic (Slug) or MSDF glyphs, never the UI atlas.

### 8.3 Caching and damage
- A retained scene with a stable id per node. Each frame, **diff the new primitive stream against
  the previous one per layer** (prefix and suffix, or per-node content hashes) to get damage rects.
  This is the approach Zed measured, not hand annotation.
- Render damaged regions into a **persistent back texture** (scissored), copy or present with
  damage:
  - Windows: `Present1` dirty rects, plus the scroll rect for editor scroll.
  - Vulkan: `VK_KHR_incremental_present`.
  - macOS: persistent target plus full present.
- Upstream wgpu lacks the API. Contribute `present_with_damage` (the maintainers invited it,
  2026-08), or carry a minimal patch in a registered fork per cadatomic's fork rules.
- Skip present entirely when nothing changed: idle is zero GPU.
- Per-scroll-container tile caches (256–512 px tiles) for the code editor, large tables and trees.
  Scroll by offset and raster only exposed tiles.
- Optional opaque depth pre-pass (Zed PR #61461 pattern). Benchmark on iGPU and lavapipe before
  enabling it by default.

### 8.4 Batching and upload
- Draw order from a bounds R-tree (GPUI `BoundsTree`), then sort by (order, pipeline, texture page),
  giving about 10–50 draws per frame.
- One storage buffer of instances per frame-in-flight, filled by one `write_buffer_with` or
  `StagingBelt`. Retained layers and canvas chunks keep persistent buffers.
- Atlases: etagere, 1–2k pages, R8 and RGBA, frame-generation LRU, dirty sub-rect uploads.
- Shaders: a closed set, created at startup off the main thread; a Vulkan `PipelineCache` on disk.

### 8.5 Present
- Fifo, `desired_maximum_frame_latency` = 1 for the UI surface. Event-driven redraw only.
- macOS: CAMetalDisplayLink (14+), with `maximumDrawableCount` 2 when frames are cheap.
- Windows: DComp visual swapchains (wgpu 27+), with a waitable-object pace.
- Wayland: frame callbacks.
- Measure input-to-photon with PresentMon (Windows) and Instruments (macOS), not just CPU timers.
- Test the fallback adapter: lavapipe in CI and WSL, WARP on Windows. On a software adapter, turn
  off MSAA and the depth pre-pass, and rely on damage.

### 8.6 3D viewports and big canvases
- Heavy 3D viewports get their own compositor layer (DComp visual, child CAMetalLayer,
  wl_subsurface) with their own frame loop. Fall back to render-to-texture plus a `Surface`
  primitive where layers are unavailable or a popover must overlap.
- Schematic and PCB canvases use retained per-chunk instance buffers with typed analytic shaders.
  Pan and zoom is a uniform change. Culling is by chunk, with LOD.
  - The static canvas is cached to a texture between edits.
  - Hover and selection go in a separate overlay layer.
  - Use Vello GPU only for arbitrary-vector sub-content (symbol art, imported SVG/DXF graphics),
    cached per zoom bucket.

### 8.7 Open questions (for the coordinator / user)
1. Fork wgpu for `present_with_damage` now, or upstream first? The maintainers signalled
   acceptance on 2026-08-14.
2. Is the LCD text path on Windows worth the complexity, given grayscale plus DWrite gamma is what
   Zed ships?
3. Platform rasterisers (DirectWrite, CoreText) vs pure-Rust skrifa everywhere: fidelity vs one
   code path and determinism for snapshot tests.
4. Should Vello GPU (beta, 2026-04) be a dependency for paths and SVG, or should we keep our own MSAA
   path fallback until it stabilises?

---

## Appendix: source list (accessed 2026-10-25)
- Zed blog, 2023-03-07: https://zed.dev/blog/videogame
- Zed PRs: #37167 https://github.com/zed-industries/zed/pull/37167 · #38862 https://github.com/zed-industries/zed/pull/38862 ·
  #53932 https://github.com/zed-industries/zed/pull/53932 · #59441 https://github.com/zed-industries/zed/pull/59441 ·
  #61461 https://github.com/zed-industries/zed/pull/61461 · #46758 (Linux renderer → wgpu, 2026-02-13)
  https://git.secluded.site/zed/commit/af8ea0d6c26192c45f44f473c0d4a7d6f72ed018
- GPUI source (main, 2026-10-25): crates/gpui/src/{scene.rs,bounds_tree.rs,text_system.rs,text_system/line_layout.rs,window.rs,svg_renderer.rs}, crates/gpui_wgpu/src/wgpu_renderer.rs
- Evan Wallace shadows: https://madebyevan.com/shaders/fast-rounded-rectangle-shadows/
- Warp: https://www.warp.dev/blog/how-to-draw-styled-rectangles-using-the-gpu-and-metal
- Vello README / ARCHITECTURE / vello_gpu CHANGELOG (main): https://github.com/linebender/vello
- Linebender posts: https://linebender.org/blog/tmil-25/ (2026-04-19), https://linebender.org/blog/tmil-22/ (2025-11-13), https://linebender.org/atom.xml
- Stampfl thesis (2025-10-10): https://ethz.ch/content/dam/ethz/special-interest/infk/inst-pls/plf-dam/documents/StudentProjects/MasterTheses/2025-Laurenz-Thesis.pdf
- Raph Levien: https://raphlinus.github.io/gpu/2025/03/21/good-parallel-computer.html ·
  https://raphlinus.github.io/ui/graphics/gpu/2021/10/22/swapchain-frame-pacing.html ·
  https://raphlinus.github.io/xi/2020/06/27/xi-retrospective.html
- Skia Graphite (2025-07-08): https://blog.google/chromium/introducing-skia-graphite-chromes/
- Impeller: https://docs.flutter.dev/perf/impeller · https://github.com/flutter/flutter/issues/123671
- Rive: https://rive.app/blog/rive-renderer-now-open-source-and-available-on-all-platforms · https://rive.app/docs/runtimes/cpp/renderers.md · https://rive.app/blog/introducing-vector-feathering
- Slug: https://terathon.com/blog/decade-slug.html (2026-03-17)
- Behdad, State of Text Rendering 2024: https://behdad.org/text2024/
- WebRender: https://github.com/servo/servo/wiki/Webrender-Overview · https://doc.servo.org/src/webrender/picture.rs.html ·
  https://doc.servo.org/webrender/batch/index.html · https://bugzilla.mozilla.org/show_bug.cgi?id=2052391 ·
  https://bugzilla.mozilla.org/show_bug.cgi?id=1581650 · text-rendering.md (gecko-dev mirror above)
- Chromium: https://chromium.googlesource.com/chromium/src/+/main/docs/how_cc_works.md ·
  https://developer.chrome.com/blog/better-text-rendering-in-chromium-based-browsers-on-windows ·
  https://developer.chrome.com/blog/memory-safety-fonts · https://blogs.igalia.com/obrufau/2021/07/20/improved-raster-transforms-with-non-uniform-scales.html
- Edge fonts (2021-06-02): https://blogs.windows.com/msedgedev/2021/06/02/improving-font-rendering-in-microsoft-edge/
- GTK: https://blog.gtk.org/2024/03/07/on-fractional-scales-fonts-and-hinting/ · https://docs.gtk.org/gtk4/property.Settings.gtk-font-rendering.html ·
  https://docs.gtk.org/gtk4/property.Settings.gtk-hint-font-metrics.html · https://feaneron.com/?p=4190
- Qt: https://doc.qt.io/qt-6/qsgtextnode.html · https://qt.io/blog/2011/07/15/text-rendering-in-the-qml-scene-graph
- Crates: https://docs.rs/parley/latest/parley/ (0.12.0) · https://docs.rs/fontique/latest/fontique/ (0.12.0) ·
  https://docs.rs/skrifa/latest/skrifa/ (0.49.0) · https://docs.rs/swash/latest/swash/ (0.2.10) ·
  https://docs.rs/cosmic-text/latest/cosmic_text/ (0.19.0) · https://docs.rs/crate/harfrust/0.13.2 ·
  https://docs.rs/crate/glyphon/latest · https://docs.rs/etagere/latest/etagere/ (0.3.0) · https://nical.github.io/posts/etagere.html ·
  https://docs.rs/lyon/latest/lyon/ (1.0.19) · https://docs.rs/epaint/latest/epaint/tessellator/struct.TessellationOptions.html (0.37.0)
- wgpu 30.0.1 docs: PresentMode, SurfaceConfiguration, PipelineCache, StagingBelt, Queue, Features, SurfaceTargetUnsafe (docs.rs/wgpu/latest) ·
  CHANGELOG https://raw.githubusercontent.com/gfx-rs/wgpu/trunk/CHANGELOG.md · issues #1397, #2869
- DXGI: https://learn.microsoft.com/en-ie/Windows/Win32/direct3ddxgi/dxgi-1-2-presentation-improvements ·
  https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_3/nf-dxgi1_3-idxgiswapchain2-getframelatencywaitableobject ·
  WARP https://learn.microsoft.com/en-us/windows/win32/direct3darticles/directx-warp
- Apple: https://developer.apple.com/documentation/quartzcore/cametallayer · maximumDrawableCount · presentsWithTransaction ·
  https://developer.apple.com/documentation/quartzcore/cametaldisplaylink · https://www.developer.apple.com/forums/thread/773229
- Wayland wl_subsurface: https://wayland.app/protocols/wayland#wl_subsurface
- EGL partial update / buffer age: https://gitlab.freedesktop.org/wayland/weston/issues/134 · https://www.phoronix.com/news/Weston-EGL_KHR_partial_update
- xterm.js atlas: https://github.com/xtermjs/xterm.js/discussions/4334
- KiCad GAL: https://ohwr.org/project/cern-kicad/wikis/Kicad-gal
- Instanced lines: https://wwwtyro.net/2019/11/18/instanced-lines.html
