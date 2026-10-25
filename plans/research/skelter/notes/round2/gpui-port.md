# GPUI port list for tessel (source read, 2026-10-25)

Sources read (no build):

- **Zed `main` at `a5c4ca8bf6c4717a328a4a61b42d6d1d38949bcc`** (2026-10-25 08:55 UTC, "agent_servers: Offer ACP v1
  subagents behind acp-beta (#66078)"). Sparse checkout of `crates/gpui`, `gpui_wgpu`, `gpui_windows`, `gpui_apple`,
  `gpui_macos`, `gpui_linux`, `gpui_platform`, `gpui_util`, `gpui_shared_string`, `gpui_web`.
- **longbridge/gpui-component at `7b14e03f2ed4b46fc4149a8a523c7f91bcad6560`** (2026-10-24).
- PR patches: `https://patch-diff.githubusercontent.com/raw/zed-industries/zed/pull/59441.patch` and `.../61461.patch`.
  The PR pages were read on github.com for their state. The `gh` REST and GraphQL APIs were not available from this
  session.

LOC counts are `wc -l` of whole files, including tests and comments.

Verdicts:
- **copy+adapt**: take the file or function, keep its attribution, and change the types.
- **ideas**: reimplement it from the design.
- **skip**: don't use it.

---

## 0. Crate layout at this sha

The Zed workspace has these GPUI crates: `gpui`, `gpui_apple`, `gpui_ios`, `gpui_linux`, `gpui_macos`, `gpui_macros`,
`gpui_platform`, `gpui_shared_string`, `gpui_tokio`, `gpui_util`, `gpui_web`, `gpui_wgpu`, `gpui_windows`.

| Crate | Total LOC (rs + shaders) | What it holds |
|---|---|---|
| `gpui` | 90,946 `.rs` | core: `scene.rs`, `bounds_tree.rs`, `window.rs` (9,979), text system, elements, platform traits |
| `gpui_wgpu` | 8,039 | wgpu renderer (Linux, web), WGSL shaders, atlas, context, cosmic-text text system |
| `gpui_windows` | 14,805 | Win32 platform, D3D11 renderer, HLSL, DirectComposition, DirectWrite, vsync thread, DirectManipulation |
| `gpui_apple` | 5,443 | Metal renderer, `.metal` shaders, Metal atlas, CoreText text system, thermal |
| `gpui_macos` | 9,797 | AppKit platform, window (4,056), CVDisplayLink pacing, keyboard, events |
| `gpui_linux` | 16,945 | own Wayland client (3,481 client + 2,416 window), own X11 client (3,215 + 2,067), xkb, clipboard |
| `gpui_platform` | 1,191 | platform selection glue |

Dependencies, from Zed's `Cargo.lock`:

| Dependency | Version / pin |
|---|---|
| **wgpu** | **30.0.0 from git** `gfx-rs/wgpu` rev `3b3506ea31154edc04de64debda0e780cba2639c` (upstream; Zed's own wgpu fork is gone) |
| etagere | 0.2.15 |
| cosmic-text | 0.19.0 |
| swash | 0.2.6 |
| lyon | 1.0.16 |
| taffy | =0.15.0 |
| windows | 0.61.3 / 0.62.2 (several versions in the lock) |

`gpui_wgpu` enables wgpu features `std, metal, gles, vulkan, wgsl, webgpu`. Native instances use
`Backends::VULKAN | Backends::GL` (`wgpu_context.rs:476`). **The wgpu path is never DX12.** Windows uses its own D3D11
renderer and macOS its own Metal renderer.

---

## 1. Licensing

- **Every GPUI crate is `license = "Apache-2.0"`**: gpui, gpui_wgpu, gpui_windows, gpui_apple, gpui_macos,
  gpui_linux, gpui_platform, gpui_util, gpui_web.
  - Each crate's `LICENSE-APACHE` is a symlink to the root `LICENSE-APACHE`.
  - Its first line reads "Copyright 2022 - 2025 Zed Industries, Inc."
  - The rest of the Zed repo (the editor and so on) is GPL (`LICENSE-GPL`), so **copy only from `crates/gpui*`.**
- **There is no NOTICE file** (`find -iname 'NOTICE*'` finds nothing). Apache-2.0 §4 therefore requires us to:
  - ship a copy of the licence;
  - keep the copyright and attribution notices in the files we copy;
  - add "prominent notices" in each changed file saying that we changed it.

  Recommendation: put a header in every ported file, for example
  `// Derived from zed-industries/zed crates/gpui_wgpu/src/shaders.wgsl @ a5c4ca8 (Apache-2.0, (c) Zed Industries); modified for tessel.`
  Also add a `THIRD-PARTY.md` and `LICENSES/Apache-2.0.txt` in tessel.
- **The third-party code inside GPUI carries its own licences, which we must keep:**
  - **MIT, Microsoft Corporation (Windows Terminal).** This covers:
    - the gamma/contrast functions in `gpui_wgpu/src/shaders.wgsl:31-33`, from `microsoft/terminal@1283c0f/src/renderer/atlas/dwrite.hlsl`;
    - `get_gamma_correction_ratios` in `gpui/src/platform.rs:1692-1726`, from `.../dwrite.cpp`;
    - `gpui_windows/src/alpha_correction.hlsl`.

    The MIT notice must travel with any copy of these.
  - **Arboard** (MIT/Apache) in `gpui_linux/src/linux/x11/clipboard.rs`. We have no reason to copy it.
- **gpui-component / GPUI Kit is Apache-2.0** in every crate: assets, base, component, component-macros, fps, kit,
  shell, webview.
  - `LICENSE-APACHE` reads "Copyright 2024 - 2026 Longbridge". It has no NOTICE file.
  - Its website docs are also CC BY 4.0 (`LICENSE-DOCS.md`).
  - It depends on `gpui-pre =0.3.9`, a crates.io snapshot of Zed.

---

## 2. Renderer

### 2.1 `crates/gpui/src/scene.rs` (949 LOC): **copy+adapt** (primitive structs, `BatchIterator`)

`Scene` holds one `Vec` per kind plus a `BoundsTree` and a `paint_operations` log, which `replay` uses for cached views.

```rust
pub struct Scene {
    pub(crate) paint_operations: Vec<PaintOperation>,   // Primitive | StartLayer(bounds) | EndLayer
    primitive_bounds: BoundsTree<ScaledPixels>,
    layer_stack: Vec<DrawOrder>,                        // DrawOrder = u32
    pub shadows, quads, paths, underlines, monochrome_sprites,
        subpixel_sprites, polychrome_sprites, surfaces: Vec<_>,
}
```

Primitive layouts (all `#[repr(C)]`; the GPU reads them directly as storage-buffer records):

```rust
pub struct Quad { order: u32, border_style: BorderStyle /*Solid=0,Dashed=1*/,
  bounds: Bounds<ScaledPixels>, content_mask: ContentMask<ScaledPixels> /*{bounds}*/,
  background: Background, border_color: Hsla, corner_radii: Corners<ScaledPixels>,
  border_widths: Edges<ScaledPixels> }
// Background { tag: BackgroundTag /*Solid,LinearGradient,PatternSlash,Checkerboard*/, color_space: ColorSpace,
//   solid: Hsla, gradient_angle_or_pattern_height: f32, colors: [LinearColorStop{color:Hsla,percentage:f32}; 2], pad: u32 }
// => Quad is 160 bytes

pub struct Underline { order: u32, pad: u32, bounds, content_mask, color: Hsla,
  thickness: ScaledPixels, wavy: PaddedBool32 }

pub struct Shadow { order: u32, blur_radius: ScaledPixels, bounds, corner_radii: Corners,
  content_mask, color: Hsla, element_bounds: Bounds, element_corner_radii: Corners,
  inset: u32 /*0 drop,1 inset*/, pad: u32 }

pub struct MonochromeSprite { order: u32, pad: u32, tile: AtlasTile, bounds, content_mask,
  color: Hsla, transformation: TransformationMatrix /*rotation_scale [[f32;2];2], translation [f32;2]*/ }
pub struct SubpixelSprite { /* identical to MonochromeSprite */ }
pub struct PolychromeSprite { order: u32, pad: u32, tile: AtlasTile, grayscale: PaddedBool32,
  opacity: f32, bounds, content_mask, corner_radii: Corners }
pub struct PaintSurface { order, bounds, content_mask,
  #[cfg(macos/ios)] image_buffer: CFRetained<CVPixelBuffer> }   // video only
pub struct Path<P> { id: PathId, order, bounds, content_mask, vertices: Vec<PathVertex<P>>,
  color: Background, start, current, contour_count }
// PathVertex { xy_position, st_position: Point<f32> /*Loop-Blinn quadratic st*/, content_mask }
// AtlasTile { texture_id: AtlasTextureId{index:u32, kind: Monochrome=0|Polychrome=1|Subpixel=2},
//   tile_id: TileId, padding: u32, bounds: Bounds<DevicePixels> }
```

- Colours are **HSLA**, converted per vertex in the shader (`hsla_to_rgba`). Gradients interpolate in sRGB or Oklab.
- The **only clip is an axis-aligned rect** (`ContentMask`), checked in the fragment shader through four
  `clip_distances` varyings ("TODO: use `clip_distance` once Naga supports it").
  - There is no rounded clip and no clip stack.
  - Transforms exist only on mono and subpixel sprites (2×3).

**Draw order.**
- `insert_primitive` clips the bounds by the content mask and drops empty ones.
- It then takes `order = layer_stack.last()`, or else `primitive_bounds.insert(clipped_bounds)`. That is "1 + max order
  of anything already inserted that overlaps".
- `push_layer(bounds)` reserves a single order for the whole layer, so everything inside shares it.
- `finish()` sorts each `Vec` by `order`; the sprites sort by `(order, texture_id.index)`.

**`Scene::batches()` / `BatchIterator`.**
- At each step it peeks the head of all eight kinds and sorts them by `(order, PrimitiveKind)`.
- It emits a run of the lowest kind while `(item.order, kind) < (second_lowest_order, second_kind)`. Sprites also
  require the same `texture_id`.
- The result is the minimum number of draws that keeps painter's order across kinds without per-primitive state
  changes. It is about 220 lines of mechanical code.

Adaptation:
- Replace HSLA with premultiplied linear RGBA (or u32 sRGB8) computed on the CPU. That shrinks `Quad` and removes
  per-vertex HSL math.
- Add a rounded-clip slot and a clip-chain index.
- Make the kinds a closed enum array instead of eight hand-written arms. A macro or a generic `KindQueue<T>` cuts it
  to under 100 lines.
- Keep the `(order, kind, atlas page)` merge rule exactly as GPUI has it.
- Drop `paint_operations` and `replay`, which are GPUI's immediate-mode cache. Retained display lists make them
  unnecessary.

### 2.2 `crates/gpui/src/bounds_tree.rs` (472 LOC): **copy+adapt**

- An R-tree, `MAX_CHILDREN = 12`, with nodes in a `Vec`.
- Each node keeps `max_order`, with the max-order child kept last.
- `max_leaf` gives an O(1) fast path when the new rect overlaps the current top.
- The search prunes by `max_order` and reuses a `search_stack` of `NonNull` (it contains `unsafe`).
- **It is insert-only and has `clear()`, with no removal.** That fits a per-frame rebuild of the instance list. For
  retained per-layer lists, rebuild each layer's tree when that layer is re-recorded, or order within a display
  list by recording order and only across layers by the tree.
- Tests are included (`test_random_iterations`).

### 2.3 wgpu renderer: `crates/gpui_wgpu/src/wgpu_renderer.rs` (2,923 LOC): **copy+adapt** (the core ~1,200 lines)

Files:

| File | LOC | Role |
|---|---|---|
| `wgpu_renderer.rs` | 2,923 | windowed and headless renderer |
| `wgpu_context.rs` | 832 | instance, adapter and device selection, error state, device-lost recovery |
| `wgpu_atlas.rs` | 568 | glyph and image atlas |
| `shaders.wgsl` | 1,362 | shared shader logic |
| `shaders_storage.wgsl` | 42 | storage-buffer instance transport |
| `shaders_webgl.wgsl` | 262 | texture instance transport for WebGL2 |
| `shaders_subpixel.wgsl` | 56 | dual-source subpixel text |
| `cosmic_text_system.rs` | 1,875 | Linux text system |

**Pipelines** (`WgpuPipelines`, built once in `create_pipelines`):
- `quads`, `shadows`, `path_rasterization`, `paths`, `underlines`, `mono_sprites`, `poly_sprites`, `surfaces`;
- `subpixel_sprites: Option`, present only with `DUAL_SOURCE_BLENDING` and not on WebGL.

Every pipeline draws a 4-vertex `TriangleStrip` with `instance_index`. Path rasterization is a `TriangleList` drawn by
`vertex_index`. Blend states:
- surface alpha mode `PreMultiplied` uses `PREMULTIPLIED_ALPHA_BLENDING`, anything else `ALPHA_BLENDING`;
- paths use `PREMULTIPLIED_ALPHA_BLENDING`;
- subpixel uses `Src1 / OneMinusSrc1` for colour and `One / OneMinusSrcAlpha` for alpha.

**Bind groups:**
- `@group(0)` globals, a uniform with three sub-allocations: `GlobalParams{viewport_size, premultiplied_alpha}`, the
  path globals, and `GammaParams{gamma_ratios[4], grayscale_enhanced_contrast, subpixel_enhanced_contrast, is_bgr}`;
- `@group(1)` instances, a storage buffer at an offset;
- `@group(2)` atlas texture and sampler;
- a separate layout for surfaces.

**Instance data:**
- **One** growable storage buffer, initial 2 MiB, doubling up to
  `min(max_storage_buffer_binding_size, MAX_INSTANCE_BUFFER_SIZE = 256 MiB)`.
- Written with `queue.write_buffer` at aligned offsets (`min_storage_buffer_offset_alignment`).
- **A new bind group is created per kind per frame** (`write_instance_binding`).
- There is no per-frame-in-flight ring. It relies on `queue.write_buffer` staging.
- On WebGL2, records go into an `Rgba32Uint` texture instead (`INSTANCE_TEXTURE_TEXEL_SIZE = 16`).

**Paths (MSAA):**
- `path_sample_count` is the first of `[4, 2, 1]` that the surface format supports.
- Each `Paths` batch **ends the main render pass** and rasterizes all of the batch's path triangles. It draws into a
  4× MSAA texture resolved into a full-window `path_intermediate` (or into the intermediate directly at 1×), cleared
  transparent.
- It then **reopens the main pass with `LoadOp::Load`** and composites with `PathSprite { bounds }`.
  - If all paths share one order, there is one sprite per path.
  - Otherwise there is one sprite covering the union.
- The intermediate and MSAA textures are window-sized and lazily recreated on resize.

**Frame:**
- `draw(scene)`:
  1. checks the device error state, with up to 10 failures before panicking and an atlas clear after 5;
  2. calls `get_current_texture()`, reconfiguring on `Suboptimal`/`Lost`/`Outdated` and skipping on
     `Timeout`/`Occluded`;
  3. runs `render_frame`;
  4. calls `queue.present(frame)`.
- `render_frame` writes the globals, writes the instances, records one main pass with `LoadOp::Clear`, iterates
  `scene.batches()` and submits.
- **`PrimitiveBatch::Surfaces` is a no-op on wgpu:** "Surfaces are macOS-only for video playback and are not
  implemented by the WGPU renderer."
- **There is no depth attachment** (`depth_stencil_attachment: None` everywhere).

**Surface config:**
- format: the first of `Bgra8Unorm`, `Rgba8Unorm`, else the first non-sRGB format. Blending happens in gamma space,
  as on every GPUI backend.
- alpha mode: `PreMultiplied`, else `Inherit` when transparent; `Opaque`, else `Inherit` when opaque.
- **`desired_maximum_frame_latency: 2`.**
- `present_mode`: the caller's preference, else `Fifo`. The Wayland window passes `Mailbox` (`wayland/window.rs:585`).

**Device:**
- Created in `WgpuContext`. One per process, shared across windows through `GpuContext = Rc<RefCell<Option<WgpuContext>>>`.
- `required_features` is only `DUAL_SOURCE_BLENDING` when available.
- Limits are `downlevel_defaults()` (`downlevel_webgl2_defaults()` on the web), with `MemoryHints::MemoryUsage`.
- Adapter selection sorts Vulkan/Metal/Dx12 before GL and honours a compositor GPU hint (vendor and device id).
  `new_rejecting_software` exists.

**Headless:** `WgpuHeadlessRenderer` (line 2303) renders a `Scene` to an `RgbaImage` (`render_scene_to_image`), with
pixel tests. This is useful for our `tessel-test`.

**What to change for our scene and a shared `wgpu::Device` that we own:**
1. Replace `WgpuContext` with a thin `GpuContext { device, queue, adapter_info, features }` that we pass in.
   - `WgpuRendererCore::new` only needs `device`, `queue`, the atlas and the target format, so the change is small.
   - `from_native_adapter_and_device` (line 290) already builds a context from an existing adapter and device. It is
     private; we make it the public entry point.
2. Split the renderer into a **target-agnostic `Renderer::encode(&mut CommandEncoder, &TextureView, &DisplayList)`**
   and a separate **surface/present owner** (our compositor thread). Today `draw()` owns acquire, present and error
   recovery together.
3. Replace the per-frame `write_buffer` and bind group per kind:
   - use a `StagingBelt` or a ring per frame in flight;
   - keep **persistent buffers for retained layers and canvas chunks**;
   - give each kind a bind group with dynamic offsets, created once.
4. Implement `Surface` with a sampled `wgpu::TextureView` (our offscreen viewports, egui island, cached tiles). That is
   one more pipeline, a poly-sprite shader without an atlas. The current wgpu path is a no-op.
5. Paths:
   - keep MSAA-into-intermediate;
   - size the intermediate to the batch's union bounds, not the window, or use an atlas of cached path tiles keyed by
     (path, scale), as the proposal says;
   - avoid splitting the main pass by gathering all path batches first when their orders allow it.
6. Add the scissor/damage path from PR #59441 (§5) and set `desired_maximum_frame_latency: 1`.
7. Force `Backends::DX12` on Windows. GPUI never runs wgpu on Windows, so the DX12 path is untested by Zed.

### 2.4 WGSL shaders: `gpui_wgpu/src/shaders.wgsl` (1,362) + storage (42) + subpixel (56): **copy+adapt**

| Section | Lines | Content |
|---|---|---|
| gamma/contrast (MIT, Microsoft) | 31–78 | `light_on_dark_contrast`, `enhance_contrast`, `apply_alpha_correction`, `apply_contrast_and_gamma_correction(3)` |
| colour | 209–317 | sRGB↔linear, HSLA→RGBA, Oklab, `over` |
| shadow math | 319–338 | `gaussian`, `erf` (Abramowitz–Stegun approximation), `blur_along_x` |
| SDF | 340–387 | `pick_corner_radius` (per-quadrant radius), `quad_sdf`, `quad_sdf_impl` (fast path for radius 0) |
| gradients | 398–515 | linear gradient (sRGB/Oklab), slash pattern, checkerboard |
| **quad** | 517–946 | `vs_quad` / `fs_quad` (~330 lines): per-corner radii, per-edge border widths, antialiased **dashed borders** with per-corner dash velocity, `quarter_ellipse_sdf` for unequal border widths at a corner |
| shadow | 948–1046 | drop and inset shadows; geometry expanded by `3*blur_radius`; 4-sample integral of erf×gaussian along y; inset = `1 - alpha` clipped by the element's rounded rect |
| paths | 1048–1145 | path rasterization (Loop–Blinn quadratic `st` coverage, by vertex index) + path sprite composite |
| underline | 1147–1188 | straight or wavy (sine distance) |
| mono sprite | 1190–1237 | `textureSample(...).r` then `apply_contrast_and_gamma_correction(sample, color.rgb, grayscale_enhanced_contrast, gamma_ratios)` |
| poly sprite | 1239–… | rounded corners, grayscale, opacity |
| subpixel | separate file | dual-source `@blend_src(0/1)`, BGR swap, per-channel gamma |

Key constants:

```wgsl
const M_PI_F: f32 = 3.1415926;
const GRAYSCALE_FACTORS: vec3<f32> = vec3<f32>(0.2126, 0.7152, 0.0722);
fn color_brightness(c) { return dot(c, vec3(0.30, 0.59, 0.11)); }          // Rec.601
fn light_on_dark_contrast(k, c) { return k * saturate(4.0 * (0.75 - brightness(c))); }
fn enhance_contrast(a, k) { return a * (k + 1.0) / (a * k + 1.0); }
fn apply_alpha_correction(a, b, g) { let ba = g.x*b + g.y; let corr = ba*a + (g.z*b + g.w); return a + a*(1-a)*corr; }
```

CPU side (`wgpu_renderer.rs` `RenderingParameters`, line 2562):

| Parameter | Value | Override |
|---|---|---|
| gamma | **1.8**, clamped to 1.0..2.2 | `ZED_FONTS_GAMMA` |
| `grayscale_enhanced_contrast` | **1.0** | `ZED_FONTS_GRAYSCALE_ENHANCED_CONTRAST` |
| `subpixel_enhanced_contrast` | **0.5** | `ZED_FONTS_SUBPIXEL_ENHANCED_CONTRAST` |
| `path_sample_count` | first of `[4, 2, 1]` the format supports | — |

`get_gamma_correction_ratios(gamma)` (`gpui/src/platform.rs:1697`, MIT) is a 13-row table for gamma 1.0–2.2 (from
DirectWrite / Windows Terminal), scaled by `NORM13 = 0x10000/(255*255)*4` and `NORM24 = 0x100/255*4`. Row for gamma
1.8: `[0.1469, -0.8911, 1.4644, -0.3234] / 4`.

Adaptation:
- Keep `quad_sdf`, the border code and the shadow code almost verbatim.
- Replace HSLA inputs with linear premultiplied RGBA.
- Replace the four `clip_distances` varyings with `@builtin(clip_distances)` where naga now supports it; check wgpu 30.
- Add a rounded-clip SDF reusing `quad_sdf`.
- Keep the gamma block with its MIT header.
- Drop `shaders_webgl.wgsl` unless the web target matters.

### 2.5 DirectX 11 renderer (comparison): `gpui_windows/src/directx_renderer.rs` (2,070) + `shaders.hlsl` (1,268) + `directx_atlas.rs` (420): **ideas only** (we use wgpu DX12)

- It has the same primitive set and the same `scene.batches()` loop.
- It uses `PATH_MULTISAMPLE_COUNT = 4` (guaranteed on D3D11) and `MAX_INSTANCE_BUFFER_SIZE = 256 MiB`, with
  `RENDER_TARGET_FORMAT = DXGI_FORMAT_B8G8R8A8_UNORM`.
- It has the same gamma/contrast HLSL, with `alpha_correction.hlsl` (49) and `color_text_raster.hlsl` (44).
  DirectWrite rasterizes the glyphs (`direct_write.rs`, 2,170).
- `draw_surfaces` is a no-op: it returns `Ok(())`.
- It has **no depth buffer** on main.
- Swap chain and present are covered in §4.1.

### 2.6 Metal renderer (comparison): `gpui_apple/src/metal_renderer.rs` (1,802) + `shaders.metal` (1,279) + `metal_atlas.rs` (405): **ideas only**

- `PATH_SAMPLE_COUNT = 4`, `INSTANCE_BUFFER_ALIGNMENT = 256`, `MAX_INSTANCE_BUFFER_SIZE = 256 MiB`.
- **`InstanceBufferPool`**: 2 MiB buffers, `StorageModeShared | CPUCacheModeWriteCombined` on unified memory,
  `StorageModeManaged` otherwise. A buffer returns to the pool in the command buffer's completion handler. **This is the
  frames-in-flight ring the wgpu backend lacks. Take this idea.**
- It is the only backend that implements `Surface`, as a CVPixelBuffer with YCbCr 4:2:0 biplanar (video).
- It has **no subpixel batch on macOS** (`#[cfg_attr(target_os = "macos", allow(dead_code))] SubpixelSprites`).
- Layer config is in §4.2.

---

## 3. Atlas

- **`gpui_wgpu/src/wgpu_atlas.rs` (568 LOC): copy+adapt.**
  - Uses `etagere::BucketedAtlasAllocator`.
  - **`DEFAULT_ATLAS_SIZE` 1024×1024.** A new page is `max(min_size, 1024²)`, capped at `max_texture_dimension_2d`.
  - Formats: `R8Unorm` for monochrome; the colour format (`Bgra8Unorm` / `Rgba8Unorm`, swizzled on upload) for
    subpixel and polychrome.
  - One texture list per `AtlasTextureKind`, with a free list of indices and a `generation` per texture, so cached
    bind groups are invalidated when an index is reused.
  - Uploads are queued as `PendingUpload` and flushed in `before_frame()`.
  - `live_atlas_keys` reference-counts each texture; it is freed when unreferenced.
  - Tests are included.
- **Shared key map: `gpui/src/platform.rs:1790-1850`, `AtlasState<Backend>`: copy.**
  - `tiles_by_key: FxHashMap<AtlasKey, AtlasTile>` plus the `AtlasBackend { insert, remove }` trait.
  - `AtlasKey = Glyph(RenderGlyphParams) | Svg(RenderSvgParams) | Image(RenderImageParams)`.
  - `texture_kind()`: emoji maps to Polychrome, subpixel to Subpixel, otherwise Monochrome; SVG to Monochrome; images
    to Polychrome.
  - `HeadlessAtlas` hands out tiles without pixels, which is good for tests.
- **Eviction: there is none for glyphs or SVGs.**
  - Only `Window::drop_image` calls `sprite_atlas.remove` (window.rs:5193), for each image frame.
  - The whole atlas is cleared only on device-error recovery (`atlas.clear()` after more than 5 failed frames) and on
    device loss.
  - **We must add frame-generation LRU per page** (the proposal's plan). GPUI has nothing to copy here.
- **Glyph raster params** (`gpui/src/text_system.rs`):

  ```rust
  pub const SUBPIXEL_VARIANTS_X: u8 = 4;
  pub const SUBPIXEL_VARIANTS_Y: u8 = 1;
  pub struct RenderGlyphParams { font_id, glyph_id, font_size: Pixels, subpixel_variant: Point<u8>,
      scale_factor: f32, is_emoji: bool, subpixel_rendering: bool, dilation: u8 }  // Hash uses f32 to_bits
  ```

  Quantisation (`Window::paint_glyph`, window.rs:4839):
  - `q = round_half_toward_zero(x*4)/4`, with y at whole pixels;
  - `variant = fract(q)*4`;
  - integer origin `trunc(q)`;
  - sprite bounds `integer_origin + raster_bounds.origin`.

  Subpixel rendering is used only when the window is opaque, the platform supports it, and the font's recommended
  mode is Subpixel.

  `dilation` (`glyph_dilation_for_color`) is a per-colour stem-darkening knob; the default is 0.

  Linux rasterization (`cosmic_text_system.rs:516-550`):
  - `swash` `ScaleContext` with `.hint(true)`;
  - sources: `[ColorOutline(0), ColorBitmap(BestFit), Outline]` for emoji, `[Bitmap(ExactSize), Outline]` otherwise;
  - format `Format::Alpha`, or `Format::subpixel_bgra()` for subpixel;
  - `.offset(Vector(variant.x/4/scale, variant.y/1/scale))`.

  **Verdict: copy the constants and the quantisation (about 30 lines). Use skrifa or our own rasteriser instead of swash.**

---

## 4. Frame pacing and present

### 4.1 Windows

**`gpui_windows/src/vsync.rs` (119 LOC): copy+adapt.**

```rust
const VSYNC_INTERVAL_THRESHOLD: Duration = Duration::from_millis(1);
const DEFAULT_VSYNC_INTERVAL: Duration = Duration::from_micros(16_666); // ~60Hz
// VSyncProvider::new: interval from DwmGetCompositionTimingInfo(HWND(0)).qpcRefreshPeriod / QPC freq;
//   if < 1 ms (a 29 µs value was seen) use rateRefresh.uiDenominator/uiNumerator.
// wait_for_vsync: f = DwmFlush(); if !ok || elapsed < 1 ms { sleep(interval); LocalSchedule } else { NativeCallback }
```

The comment still names `DCompositionWaitForCompositorClock`: "DwmFlush and DCompositionWaitForCompositorClock returns
very early ... when the monitor goes to sleep". **At this sha only `DwmFlush` is called.** The compositor-clock path is
gone, and we should re-add it on Windows 11.

**The thread** (`platform.rs:433-495`, `begin_vsync_thread`):
- It is one `"VSyncProvider"` thread for all windows. It runs while any window exists.
- Each tick it:
  1. checks for device loss and recovers;
  2. records a `PlatformFrameSignal` (timestamp plus source) for each visible, non-iconic window;
  3. calls `RedrawWindow(hwnd, RDW_INVALIDATE)`.
- The frame then runs in `WM_PAINT` on the UI thread.
- For tessel, the thread wakes our compositor thread instead of invalidating the HWND.

**Swap chain** (`directx_renderer.rs`):

```rust
const BUFFER_COUNT: usize = 3;
// Composition (default): DCompositionCreateDevice(dxgi_device); CreateTargetForHwnd(hwnd, topmost=true);
//   CreateVisual(); visual.SetContent(swap_chain); target.SetRoot(visual); Commit()
DXGI_SWAP_CHAIN_DESC1 { Format: B8G8R8A8_UNORM, SampleDesc{1,0}, BufferUsage: RENDER_TARGET_OUTPUT,
  BufferCount: 3, Scaling: DXGI_SCALING_STRETCH /*required for composition*/,
  SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL, AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED, Flags: 0 }
// CreateSwapChainForComposition(device, &desc, None)
// GPUI_DISABLE_DIRECT_COMPOSITION fallback: CreateSwapChainForHwnd, Scaling NONE, AlphaMode IGNORE,
//   MakeWindowAssociation(DXGI_MWA_NO_ALT_ENTER)
// Present: swap_chain.Present(0, DXGI_PRESENT(0))   // sync interval 0, GPUI paces itself
// Resize: ResizeBuffers(..., DXGI_SWAP_CHAIN_FLAG(0))
```

There is **no frame-latency waitable object and no `SetMaximumFrameLatency`**: the code has no `FrameLatencyWaitable`
and no `ALLOW_TEARING`. Device creation is D3D11 with `D3D11_CREATE_DEVICE_BGRA_SUPPORT`, at feature levels 11_1,
11_0 or 10_1.

Verdict:
- **ideas / copy the DComp setup sequence** (about 40 lines) for our own DComp tree. Under wgpu DX12 we get the
  swap chain from `Dx12SwapchainKind::DxgiFromVisual` and own the `IDCompositionDevice`/target/visual ourselves.
- **Do better than GPUI:** use a waitable swap chain with latency 1. GPUI's three buffers plus `Present(0, 0)` can
  queue up to three frames, and only the DwmFlush pacing keeps it near one.

**Also worth copying: `gpui_windows/src/direct_manipulation.rs` (361 LOC).**
- It creates an `IDirectManipulationManager` viewport on the HWND with
  `INTERACTION | TRANSLATION_X | TRANSLATION_Y | TRANSLATION_INERTIA | RAILS_X | RAILS_Y | SCALING`.
- On `DM_POINTERHITTEST` it calls `viewport.SetContact(pointer_id)` when `GetPointerType == PT_TOUCHPAD`.
- It calls `update_manager.Update()` every frame and drains the synthesised scroll/pinch events.
- This gives precision-touchpad smooth scrolling and pinch on Windows, which winit lacks. **copy+adapt.**

### 4.2 macOS

**`gpui_macos/src/display_link.rs` (586 LOC): copy+adapt the design.**
- It is built on **`CVDisplayLink`**, not `CAMetalDisplayLink`.
- There is a static `REGISTRY` with **one immortal CVDisplayLink per `CGDirectDisplayID`**, never released, because
  `CVDisplayLinkStop` cannot be torn down safely. Two crash classes are documented: #32116 and ZED-7XR.
- Each window owns one `dispatch_source` (`DATA_ADD`) and subscribes to its display's link. The link runs if and only
  if it has subscribers, and is started and stopped on the main thread only.
- The callback context is the display id integer, so a late callback is harmless.
- It uses `std::sync::Mutex` (os_unfair_lock priority donation) and calls no CoreVideo function while the lock is held.
- Host-time conversion keeps the request's age (tests are included).

The 50-line module doc is worth keeping as a design note even if we move to `CAMetalDisplayLink` (macOS 14+).

**CAMetalLayer** (`gpui_apple/src/metal_renderer.rs:182-230`, `configure_layer`):

```rust
layer.set_pixel_format(BGRA8Unorm);
// optional Display P3 colorspace (use_display_p3)
layer.set_opaque(!transparent);                 // "direct-to-display rendering if the window is not transparent"
layer.set_maximum_drawable_count(3);
#[cfg(all(feature="test-support", debug_assertions))] layer.set_framebuffer_only(false);
objc2_layer.setAllowsNextDrawableTimeout(false);
objc2_layer.setNeedsDisplayOnBoundsChange(true);
objc2_layer.setAutoresizingMask(LayerWidthSizable | LayerHeightSizable);
// displaySyncEnabled: never set (default true)
```

**`presentsWithTransaction`.**
- It is turned on only around synchronous frames: `display_layer` (layer redisplay during live resize) and
  `windowDidBecomeKey` (`gpui_macos/src/window.rs:3196-3216, 3323-3342`).
- Those frames call `set_presents_with_transaction(true)`, stop the display link, render, then turn it back to false
  and restart the link.
- `draw` with the transaction on runs `commit(); wait_until_scheduled(); drawable.present()`. Otherwise it runs
  `present_drawable(drawable); commit()`.
- **copy+adapt.**

### 4.3 Linux / Wayland: `gpui_linux/src/linux/wayland/window.rs` (2,416): **ideas only**

- It has its own frame-loop state machine:
  `FrameLoop { Unconfigured, Ticking, RescheduleRequested, PresentationFailed, AwaitingCallback, Scheduled, RetryScheduled, Parked }`
  and `PresentationState { Unpresented, Presented, RetryBeforeFirstPresent, RetryAfterPresent }`.
- `complete_frame`:
  - when a retry is needed after a present, it requests `wl_surface.frame()`, commits, and waits (`AwaitingCallback`);
  - otherwise it uses a retry timer (`schedule_frame_retry`);
  - when nothing is dirty it **parks**, so there are no callbacks at idle.
- It binds `wp_viewporter` and `wp_fractional_scale_v1`, and `zwp_text_input_v3` for IME.
- **There is no `wp_presentation`, `fifo-v1` or `commit-timing`**: none is referenced.
- The renderer asks for `PresentMode::Mailbox` (falling back to Fifo) so it does not block in acquire.

### 4.4 Shared: `InputRateTracker` and `on_request_frame` (`gpui/src/window.rs:1325-1365`, 1772-1945): **copy**

```rust
impl Default for InputRateTracker { fn default() -> Self { Self {
    timestamps: Vec::new(), window: Duration::from_millis(100), inputs_per_second: 60,
    sustain_until: Instant::now(), sustain_duration: Duration::from_secs(1) } } }
fn record_input(&mut self) { push(now); prune(>100ms);
    if timestamps.len() >= 60 * 100 / 1000 /* = 6 */ { sustain_until = now + 1s } }
fn is_high_rate(&self) -> bool { Instant::now() < self.sustain_until }
```

Frame-request policy:
- `needs_present = require_presentation || needs_present || is_high_rate()`.
- If dirty or forced, draw and present. Otherwise, if `needs_present`, **re-present the last frame** (`window.present()`).
- Throttles:
  - an inactive window without high-rate input is capped at `inactive_frame_interval`, default `Some(Duration::from_micros(33_333))` (`platform.rs:2770`, WindowOptions);
  - thermal `Serious`/`Critical` state caps at `Duration::from_micros(16667)`.
- Re-entrant frame requests (from Windows nested message pumps) are deferred, with `force_render` remembered.

These are about 40 lines to copy, plus the policy.

---

## 5. Damage tracking (#59441) and depth pre-pass (#61461)

**Neither PR is merged.** Main at `a5c4ca8` has no `SceneDamage` and no depth attachment in any renderer.

### PR #59441, "gpui: Add damage-tracking with partial redraw (WIP)"

- Author: Saulius Valatka. Draft, opened 2026-06-16, **closed unmerged 2026-08-08** by a maintainer, who cited
  inactivity, draft status and conflicts, and pointed to #61461.
- Size: +540 −37 over 10 files. The new `crates/gpui/src/scene_damage.rs` is 286 lines.
- Approach: **diff consecutive finished scenes.**
  - `SceneDamage = Full | Rect(Bounds) | Unchanged`.
  - `between(prev, next)` runs `diff_with` per primitive kind with a **common-prefix / common-suffix match**, so a
    single inserted or removed primitive (a caret) damages only itself.
  - It unions the bounds of the unmatched middle of both lists. Shadows are dilated by `3*blur`; sprites use
    transformed bounds.
  - `union()` accumulates over frames that were skipped or failed.
- Renderer (Linux/wgpu only; Wayland and X11 opt in through `wants_render_damage`/`draw_with_damage`):
  1. render into a **persistent target**;
  2. on a partial frame, `LoadOp::Load` with `set_scissor_rect(damage)`;
  3. copy the target to the swap-chain image, which needs `COPY_DST` (otherwise it falls back to full redraws);
  4. `present_with_damage`;
  5. **skip present on `Unchanged`.**
- It needed a wgpu fork (`sauliusvl/wgpu@29b04b33e`) and was blocked once Zed moved to upstream wgpu. Upstream wgpu now
  has an **open** PR #10152, "Add damage-aware presentation via Queue::present_with_damage". Track it: it replaces our
  planned fork (proposal §6.4 / D6).
- Reported: GPU 50–60 % → 5–10 %, CPU 80–90 % → 30–50 %.

Verdict: **ideas + copy `diff_with` (~40 lines)** as the debug "missed dirty mark" checker the proposal wants, since we
take damage from the retained tree. **Copy the persistent-target, scissor and copy-to-swapchain sequence** (about 150
lines of `wgpu_renderer.rs` changes). The swapchain copy costs one full-window blit per frame. Prefer rendering
straight into the swap-chain image with buffer-age tracking where the backend exposes it.

### PR #61461, "Reduce overdraw in GPUI by using the depth buffer"

- Author: John Tur, staff. Opened 2026-07-22, **closed unmerged**. Its groundwork PR #62069 ("Tweak renderer resource
  management") merged on 2026-08-02.
- Three commits:
  1. scene plus D3D11: +533 in `directx_renderer.rs`, +99 HLSL;
  2. wgpu: +960/−… in `wgpu_renderer.rs`, +101 WGSL;
  3. Metal: about 1,153 lines touched.
- `Scene::partition_quads()`:
  - quads with `has_opaque_core()` go to `opaque_quad_indices`, reversed (front to back);
  - quads without an opaque core, **or with rounded corners**, go to `blended_quad_indices`, so a rounded opaque quad
    goes to both lists and only its corners blend.
- `quad_depth(i) = ((i+1) as f32 / 16777216.0).min(1.0)`. Greater means closer, 0 is the clear value, and the test is
  strict greater-than.
- Pass 1 draws opaque quads with depth write. Pass 2 draws everything back to front with depth test and no write.
- Non-quad batches get a constant depth equal to the quad cursor (D3D11 sets the viewport `MinDepth = MaxDepth = depth`).
- Measured MsGPUBusy (with #61274): **0.958 → 0.439** (σ ≈ 0.02).
- **The proposal cites this as "Zed: GPU busy 0.96 → 0.44 ms". It should say "unmerged PR".**
- Verdict: **ideas only.** Keep the design as an optional pass behind a benchmark (proposal §6.3). The code is
  unmerged and conflicts with main.

---

## 6. Text system

| File | LOC | Verdict | Notes |
|---|---|---|---|
| `gpui/src/text_system.rs` | 1,565 | ideas | font resolution, `FontId`, the `TextRun` → `FontRun` split, raster bounds; glyph-param constants copied (§3) |
| `gpui/src/text_system/line_layout.rs` | 1,190 | ideas | `LineLayout { font_size, width, ascent, descent, runs: Vec<ShapedRun{font_id, glyphs: Vec<ShapedGlyph{id, position, index, is_emoji}>}>, len }`, `index_for_x`, `closest_index_for_x`, `x_for_index`. **`LineLayoutCache`** = `previous_frame: Mutex<FrameCache>` + `current_frame: RwLock<FrameCache>`, with maps for lines, wrapped lines and the by-hash variants plus `used_*` lists; `reuse_layouts(range)` moves entries from previous to current for replayed views; finish_frame swaps; anything unused for one frame is dropped. **This is the "two-generation shaped-line cache" in our proposal.** About 150 lines are worth reimplementing over parley `Layout`. |
| `gpui/src/text_system/line_wrapper.rs` | 1,921 (~700 tests) | ideas / copy the algorithm | `wrap_line` walks chars using a cached `width_for_char`, so it re-wraps without re-shaping, with boundaries at word chars and indent kept. Also `truncate_line` (start, end, middle). It is char-advance based and so wrong for complex scripts. **parley's own line breaking replaces it.** The truncation logic is worth taking. |
| `gpui/src/text_system/line.rs` | 1,979 | skip | painting a `ShapedLine` into GPUI's Window |
| `gpui_wgpu/src/cosmic_text_system.rs` | 1,875 | skip (ideas for the fallback cache) | cosmic-text 0.19 shaping + swash rasterization; `Hinting::Disabled` for layout, `.hint(true)` for raster |
| `gpui_apple/src/text_system.rs` | 936 | ideas, for an optional platform rasteriser | CoreText |
| `gpui_windows/src/direct_write.rs` | 2,170 | ideas, for an optional platform rasteriser | DirectWrite, including the recommended rendering mode and gamma |

**How glyphs reach the atlas:**
1. `paint_glyph` quantises the origin and builds `RenderGlyphParams`.
2. It calls `text_system.raster_bounds(params)`, which is cached.
3. It calls `sprite_atlas.get_or_insert_with(AtlasKey::Glyph(params), || rasterize_glyph(params))`, which uploads
   lazily: queued, then flushed in `before_frame`.
4. It pushes a `MonochromeSprite` or `SubpixelSprite` with `bounds = integer_origin + raster_bounds.origin`.

This whole path is about 100 lines and **ports directly onto parley glyph runs.** Parley gives a `(glyph_id, x, y)` per
glyph in a run; we apply the same 4×1 quantisation and key.

---

## 7. Input and IME (platform layer reuse)

**Windows** (`gpui_windows/src/events.rs` 1,825, `keyboard.rs` 371, `window.rs` 1,740):
- **Classic mouse messages:** `WM_MOUSEMOVE`, `WM_[LRMX]BUTTON*`, `WM_MOUSEWHEEL`/`HWHEEL`, `WM_MOUSELEAVE`, the
  `WM_NC*` family and `WM_NCHITTEST` for client-side decorations.
- **There is no `WM_POINTER`, `WM_INPUT` (raw input) or `WM_TOUCH`**: zero occurrences.
- **Precision touchpad goes through DirectManipulation** (`DM_POINTERHITTEST`), as in §4.1.
- **IME is IMM32, not TSF:**
  - `ImmGetContext`, `ImmGetCompositionStringW` (`GCS_COMPSTR`, `GCS_RESULTSTR`, `GCS_COMPATTR`, `GCS_CURSORPOS`),
    `ImmSetCompositionWindow`, `ImmSetCandidateWindow`, `ImmNotifyIME(NI_COMPOSITIONSTR, CPS_COMPLETE)`,
    `ImmAssociateContextEx`;
  - messages `WM_IME_STARTCOMPOSITION`, `WM_IME_COMPOSITION`, `WM_CHAR`, `WM_INPUTLANGCHANGE`.
- Keyboard: `ToUnicode`, `MapVirtualKeyW(MAPVK_VK_TO_VSC / VK_TO_CHAR)`, `GetKeyboardLayoutNameW`.
  - A custom `WM_GPUI_KEYBOARD_LAYOUT_CHANGED` message.
  - `parse_immutable(vkey)` for keys that don't depend on the layout.
- Other: `WM_DPICHANGED`, `WM_ENTERSIZEMOVE`/`EXITSIZEMOVE`, `WM_ENTERMENULOOP`, `WM_GETOBJECT` (AccessKit), `WM_SETCURSOR`.
- Verdict:
  - **ideas, plus copy+adapt for the IMM32 composition handling (~300 lines) and the keyboard mapping (~370 lines).**
  - **copy+adapt `direct_manipulation.rs`.**
  - TSF would be our own work. GPUI does not help there, and a CAD IDE with CJK users may want it; winit also uses IMM32.

**macOS** (`gpui_macos/src/window.rs` 4,056, `events.rs` 572, `keyboard.rs` 1,508):
- `GPUIView` declares the `NSTextInputClient` protocol (window.rs:210): `hasMarkedText`, `markedRange`,
  `selectedRange`, `firstRectForCharacterRange:actualRange:`, `insertText:replacementRange:`,
  `setMarkedText:selectedRange:replacementRange:`, `unmarkText`, `attributedSubstringForProposedRange:actualRange:`,
  `validAttributesForMarkedText`, `doCommandBySelector:` (no `characterIndexForPoint:` among the registered selectors).
- `keyDown:` and `performKeyEquivalent:` go through `interpretKeyEvents`.
- Keyboard layouts: `TISCopyCurrentKeyboardLayoutInputSource` plus `UCKeyTranslate` with `kUCKeyTranslateNoDeadKeysMask`
  (events.rs:512-560) give the shifted and unshifted keys; `keyboard.rs` holds the layout tables.
- Verdict: **copy+adapt** when we write our own AppKit backend (this is the hard-won part: key equivalents, dead keys,
  marked text). Until then winit covers it.

**Linux:**
- `xkbcommon` keymaps and compose (`get_xkb_compose_state`).
- `zwp_text_input_v3` IME on Wayland, with an `ImeCursorRectangleSink`.
- XIM on X11 (`xim_handler.rs`, 133).
- Verdict: **ideas** (smithay-client-toolkit covers most of it).

---

## 8. Hit testing, dispatch, cursor (`gpui/src/window.rs`, `key_dispatch.rs` 2,216, `interactive.rs` 998)

- **Hitboxes are a flat `Vec<Hitbox{id, bounds, content_mask, behavior}>` in paint order, kept from the previous frame.**
- `Frame::hit_test(pos)` walks it in reverse, collecting ids whose `bounds ∩ content_mask` contains `pos`.
- `HitboxBehavior::{Normal, BlockMouse, BlockMouseExceptScroll}`:
  - `BlockMouse` stops the walk;
  - `BlockMouseExceptScroll` sets `hover_hitbox_count`, so hitboxes behind it get scroll but not hover.
- `mouse_hit_test` is recomputed after each frame's paint (window.rs:3758) and on mouse move.
- **`DispatchPhase { Bubble (default), Capture }`:**
  - mouse: capture runs back to front, bubble front to back;
  - keys: capture runs root to focused, bubble focused to root;
  - the dispatch tree is built every frame (`key_dispatch.rs`), with keymap contexts (`keymap/context.rs`, 889).
- **Cursor style:** `set_cursor_style(style, &hitbox)` pushes a `CursorStyleRequest{hitbox_id, style}`. The resolved
  style is the **last request whose hitbox is hovered**, or one without a hitbox, searched in reverse with `fold_while`
  (window.rs:1157). Platforms receive it through `WM_GPUI_CURSOR_STYLE_CHANGED` and similar.
- Verdict: **ideas only.**
  - The concepts match the proposal (§7.1): previous-frame hitboxes, a blocking flag, scroll pass-through, two
    phases, and cursor per hitbox.
  - The code is welded to GPUI's per-frame element tree.
  - Our retained tree gives a spatial index and stable node ids instead of a per-frame Vec.

---

## 9. gpui-component / GPUI Kit (`7b14e03`, Apache-2.0)

- **License:** Apache-2.0 (Longbridge, 2024–2026); docs are also CC BY 4.0.
- **Dock:**
  - `crates/base/src/dock` is 10,534 LOC; `crates/component/src/dock` is 3,767 (styled).
  - Design: `PaneTree` is pure data with stable `NodeId`/`PanelId`. Every edit normalises and returns an
    `EditResult`. `DockArea` reconciles the tree into GPUI entities.
  - This is the same model as our `berth-core`. **ideas only** (we have berth); compare normalisation rules.
- **Editor/input:**
  - `crates/base/src/input` is 33,120 LOC (on ropey, lsp-types); `crates/component/src/input` is 8,036; the
    tree-sitter `highlighter` is 5,005.
  - It is bound to GPUI elements and entities. **skip** (we have quire); look at its `rope_ext.rs` (719) for ideas.
- **Virtual list:** `crates/base/src/virtual_list.rs`, 905 LOC.
  - Variable row/column sizes that must be known up front; only the visible range is rendered.
  - "Inspired by `gpui::uniform_list`" (Zed `uniform_list.rs`, 936).
  - **ideas only.**
- **gpui-shell:** `crates/shell/src`, 107,315 LOC, `publish = false`.
  - A JS runtime on rquickjs plus `rquickjs-jit`, with feature `gpui-fast`.
  - A competitor to the svelters approach. Read it for ideas on the binding surface; copying makes no sense.
- Nothing in gpui-component is renderer- or platform-level, so **it offers no copy targets for tessel-render or
  tessel-platform.**

---

## 10. Port list summary

| # | Source (Zed @ a5c4ca8) | LOC | Verdict | Target |
|---|---|---|---|---|
| 1 | `gpui/src/scene.rs`: primitive structs, `insert_primitive` ordering, `BatchIterator` | 949 (≈450 used) | copy+adapt | tessel-render display list and batcher |
| 2 | `gpui/src/bounds_tree.rs` | 472 | copy+adapt (insert-only; per-layer rebuild) | tessel-render order assignment |
| 3 | `gpui_wgpu/src/shaders.wgsl` + `shaders_storage.wgsl` + `shaders_subpixel.wgsl` | 1,362+42+56 | copy+adapt (RGBA inputs, rounded clip; MIT header kept) | tessel-render shaders |
| 4 | `gpui_wgpu/src/wgpu_renderer.rs`: pipelines, bind groups, instance writer, MSAA path pass, headless renderer | 2,923 (≈1,200 used) | copy+adapt (our device; encode/present split; ring buffers; Surface pipeline) | tessel-render |
| 5 | `gpui_wgpu/src/wgpu_atlas.rs` + `AtlasState` (`platform.rs:1790-1850`) | 568+60 | copy+adapt, **add LRU eviction** | tessel-render atlas |
| 6 | `get_gamma_correction_ratios` (`platform.rs:1692-1726`, MIT) + `RenderingParameters` | 35+60 | copy | tessel-text / render |
| 7 | glyph quantisation (`window.rs:4839-4900`), `SUBPIXEL_VARIANTS_*`, `RenderGlyphParams` | ~80 | copy | tessel-text |
| 8 | `gpui_windows/src/vsync.rs` + vsync thread (`platform.rs:433-495`) | 119+60 | copy+adapt (wake the compositor; re-add the compositor clock) | tessel-platform (win) |
| 9 | DComp setup + swap-chain desc (`directx_renderer.rs:1015-1036, 1294-1354`) | ~80 | ideas/copy sequence (use DX12 + waitable, latency 1) | tessel-platform (win) |
| 10 | `gpui_windows/src/direct_manipulation.rs` | 361 | copy+adapt | tessel-platform (win) |
| 11 | IMM32 composition + keyboard (`events.rs` IME parts, `keyboard.rs`) | ~300+371 | copy+adapt later (own Win backend) | tessel-platform (win) |
| 12 | `gpui_macos/src/display_link.rs` | 586 | copy+adapt (or its design for CAMetalDisplayLink) | tessel-platform (mac) |
| 13 | `configure_layer` + presentsWithTransaction toggling (`metal_renderer.rs:182-230, 553-559`; `gpui_macos/window.rs:3196-3342`) | ~80 | copy+adapt | tessel-platform (mac) |
| 14 | `NSTextInputClient` + `UCKeyTranslate` keyboard (`gpui_macos/window.rs`, `events.rs`, `keyboard.rs`) | ~800 | copy+adapt later (own mac backend) | tessel-platform (mac) |
| 15 | `InputRateTracker` + frame-request policy (`window.rs:1325-1365, 1820-1900`) | ~120 | copy | tessel-compositor |
| 16 | Metal `InstanceBufferPool` | ~70 | ideas (ring per frame in flight) | tessel-render |
| 17 | PR #59441 `scene_damage.rs` `diff_with` + the wgpu partial-redraw sequence | 286+~180 | ideas + copy `diff_with` (debug checker) | tessel-render damage / debug |
| 18 | PR #61461 depth pre-pass | ~1,500 diff | ideas (unmerged) | optional pass, benchmark-gated |
| 19 | `LineLayoutCache` (two-frame) | ~150 | ideas | tessel-text |
| 20 | `line_wrapper.rs` truncation | ~400 | ideas/copy truncation only | tessel-text |
| 21 | hit test, `HitboxBehavior`, `DispatchPhase`, cursor-style resolution | — | ideas | tessel-core |
| 22 | Wayland frame-loop state machine | ~150 | ideas | tessel-platform (wl) |
| — | DX11 renderer, Metal renderer, cosmic-text system, DirectWrite, CoreText, X11/Wayland clients, element/entity model, gpui-component | — | skip | — |

**Roughly 5–6 kLOC copied and adapted in total**:
- renderer and shaders: about 3.5 k;
- platform pacing and present: about 1.2 k;
- text glue: about 0.2 k;
- later, own-backend input: about 1.5 k.
