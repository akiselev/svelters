//! GPU tests: one image per primitive compared against a checked-in reference, batching counts for a
//! 10k-quad scene, and damage-only redraws.
//!
//! They run only with `SKELTER_GPU_TESTS=1` (CI sets it: lavapipe on Linux, WARP on Windows, Metal on
//! macOS). A missing reference is recorded under `tests/reference/` and the test fails with "reference
//! recorded, inspect it"; the PNG is also printed base64-encoded on a `SKELTER_REFERENCE` line so it can
//! be recovered from a CI log. `SKELTER_BLESS=1` rewrites every reference.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use skelter_base::color::{Color, Fill, GradientStop};
use skelter_base::commit::{FillRule, ImageData, ImageFormat, PathCmd, PathData};
use skelter_base::display::*;
use skelter_base::geom::*;
use skelter_base::ids::*;
use skelter_base::props::ClipShape;
use skelter_base::scene::{DrawItem, FrameScene, SceneResources};
use skelter_base::text::*;
use skelter_render::headless::{FORMAT, HeadlessTarget};
use skelter_render::{RenderStats, Renderer};

const SIZE: DeviceSize = DeviceSize { w: 64, h: 64 };

fn gpu() -> Option<(wgpu::Device, wgpu::Queue)> {
    if std::env::var("SKELTER_GPU_TESTS").as_deref() != Ok("1") {
        eprintln!("skipped: set SKELTER_GPU_TESTS=1 to run GPU tests");
        return None;
    }
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
        .or_else(|_| {
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                force_fallback_adapter: true,
                ..Default::default()
            }))
        })
        .expect("SKELTER_GPU_TESTS=1 but no wgpu adapter");
    eprintln!("adapter: {:?}", adapter.get_info());
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).expect("wgpu device");
    Some((device, queue))
}

/// Glyph `id < 1000`: a 6×8 coverage frame; `id >= 1000`: an 8×8 red colour glyph; 0: empty.
struct BoxGlyphs;

impl GlyphRasterizer for BoxGlyphs {
    fn rasterize(&self, key: &GlyphKey) -> Option<GlyphImage> {
        match key.glyph {
            0 => None,
            g if g >= 1000 => Some(GlyphImage {
                left: 0,
                top: -8,
                width: 8,
                height: 8,
                format: GlyphFormat::Color,
                data: [255, 0, 0, 255].repeat(64),
            }),
            _ => Some(GlyphImage {
                left: 0,
                top: -8,
                width: 6,
                height: 8,
                format: GlyphFormat::Mask,
                data: (0..48).map(|i| if i % 6 == 0 || i % 6 == 5 || i < 6 || i >= 42 { 255 } else { 0 }).collect(),
            }),
        }
    }
}

#[derive(Default)]
struct Res {
    runs: HashMap<RunId, GlyphRun>,
    images: HashMap<ImageId, ImageData>,
    paths: HashMap<PathId, PathData>,
}

impl SceneResources for Res {
    fn run(&self, id: RunId) -> Option<&GlyphRun> {
        self.runs.get(&id)
    }
    fn image(&self, id: ImageId) -> Option<&ImageData> {
        self.images.get(&id)
    }
    fn path(&self, id: PathId) -> Option<&PathData> {
        self.paths.get(&id)
    }
}

fn run(glyphs: &[u32]) -> GlyphRun {
    GlyphRun {
        font: FontId(1),
        size: 12.0,
        variations: 0,
        glyphs: glyphs.iter().enumerate().map(|(i, &id)| Glyph { id, x: i as f32 * 9.0, y: 0.0 }).collect(),
        ascent: 9.0,
        descent: 3.0,
        advance: glyphs.len() as f32 * 9.0,
        hint: HintMode::Vertical,
        aa: AaMode::Grayscale,
        embolden: 0.0,
        skew_deg: 0.0,
    }
}

fn list(prims: Vec<Prim>) -> DisplayList {
    let mut l = DisplayList::new();
    for p in prims {
        l.push(p, Rect::new(-1000.0, -1000.0, 1000.0, 1000.0));
    }
    l
}

fn quad(rect: Rect, fill: Fill) -> Quad {
    Quad {
        rect,
        radii: Corners::ZERO,
        fill,
        border_widths: Insets::ZERO,
        border_color: Color::TRANSPARENT,
        border_style: BorderStyle::Solid,
    }
}

const RED: Color = Color::rgba(1.0, 0.0, 0.0, 1.0);
const BLUE: Color = Color::rgba(0.0, 0.0, 1.0, 1.0);

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: Renderer,
}

impl Gpu {
    fn new() -> Option<Gpu> {
        let (device, queue) = gpu()?;
        let renderer = Renderer::new(&device, &queue, FORMAT, Arc::new(BoxGlyphs));
        Some(Gpu { device, queue, renderer })
    }

    fn draw(&mut self, target: &HeadlessTarget, items: &[DrawItem<'_>], res: &Res, damage: &[DeviceRect]) -> RenderStats {
        let scene = FrameScene {
            target: target.size(),
            scale: Scale(1.0),
            clear: Some(Color::WHITE),
            damage,
            items,
            resources: res,
        };
        target.render(&self.device, &self.queue, &mut self.renderer, &scene)
    }

    /// Renders `prims` (one display list, identity transform) on white and reads the pixels back.
    fn image(&mut self, prims: Vec<Prim>, res: &Res) -> Vec<u8> {
        let l = list(prims);
        let items = [DrawItem { transform: Affine::IDENTITY, clip: None, opacity: 1.0, list: &l }];
        let target = HeadlessTarget::new(&self.device, SIZE);
        self.draw(&target, &items, res, &[]);
        target.read_rgba(&self.device, &self.queue).expect("readback")
    }
}

fn px(img: &[u8], x: u32, y: u32) -> [u8; 4] {
    let i = ((y * SIZE.w + x) * 4) as usize;
    [img[i], img[i + 1], img[i + 2], img[i + 3]]
}

fn near(a: [u8; 4], b: [u8; 4], tol: u8) -> bool {
    a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= tol)
}

fn encode_png(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().expect("png header");
    writer.write_image_data(rgba).expect("png data");
    writer.finish().expect("png finish");
    out
}

fn decode_png(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes)).read_info().expect("png info");
    let mut buf = vec![0; reader.output_buffer_size().expect("png size")];
    let info = reader.next_frame(&mut buf).expect("png frame");
    buf.truncate(info.buffer_size());
    (info.width, info.height, buf)
}

fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= c.len() {
                s.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

/// Compares against `tests/reference/<name>.png`: at most 1% of pixels may differ by more than 16
/// in a channel (rasterisers differ slightly across lavapipe, WARP and Metal).
fn check(name: &str, rgba: &[u8]) {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/reference");
    let path = dir.join(format!("{name}.png"));
    let png = encode_png(SIZE.w, SIZE.h, rgba);
    let bless = std::env::var("SKELTER_BLESS").as_deref() == Ok("1");
    if bless || !path.exists() {
        std::fs::create_dir_all(&dir).expect("reference dir");
        std::fs::write(&path, &png).expect("write reference");
        println!("SKELTER_REFERENCE {name} {}", base64(&png));
        if !bless {
            panic!("reference recorded, inspect it: {}", path.display());
        }
        return;
    }
    let (w, h, want) = decode_png(&std::fs::read(&path).expect("read reference"));
    assert_eq!((w, h), (SIZE.w, SIZE.h), "{name}: reference size");
    let bad = rgba.chunks(4).zip(want.chunks(4)).filter(|(a, b)| a.iter().zip(*b).any(|(x, y)| x.abs_diff(*y) > 16)).count();
    if bad * 100 > (SIZE.w * SIZE.h) as usize {
        println!("SKELTER_ACTUAL {name} {}", base64(&png));
        panic!("{name}: {bad} pixels differ from {}", path.display());
    }
}

#[test]
fn quad_rounded_with_border() {
    let Some(mut g) = Gpu::new() else { return };
    let mut q = quad(Rect::new(8.0, 8.0, 56.0, 56.0), Fill::Solid(BLUE));
    q.radii = Corners { tl: 0.0, tr: 8.0, br: 16.0, bl: 4.0 };
    q.border_widths = Insets { top: 2.0, right: 4.0, bottom: 6.0, left: 1.0 };
    q.border_color = RED;
    let img = g.image(vec![Prim::Quad(q)], &Res::default());
    assert_eq!(px(&img, 32, 32), [0, 0, 255, 255], "fill");
    assert_eq!(px(&img, 2, 2), [255, 255, 255, 255], "outside");
    assert_eq!(px(&img, 32, 9), [255, 0, 0, 255], "top border");
    check("quad_rounded_border", &img);
}

#[test]
fn quad_dashed_and_dotted_borders() {
    let Some(mut g) = Gpu::new() else { return };
    let mut a = quad(Rect::new(4.0, 4.0, 60.0, 28.0), Fill::Solid(Color::TRANSPARENT));
    a.border_widths = Insets::all(3.0);
    a.border_color = Color::BLACK;
    a.border_style = BorderStyle::Dashed;
    let mut b = a.clone();
    b.rect = Rect::new(4.0, 36.0, 60.0, 60.0);
    b.radii = Corners::all(8.0);
    b.border_style = BorderStyle::Dotted;
    let img = g.image(vec![Prim::Quad(a), Prim::Quad(b)], &Res::default());
    check("quad_dashed_dotted", &img);
}

#[test]
fn quad_linear_gradient() {
    let Some(mut g) = Gpu::new() else { return };
    let stops = [
        GradientStop { offset: 0.0, color: RED },
        GradientStop { offset: 0.5, color: Color::rgba(0.0, 1.0, 0.0, 1.0) },
        GradientStop { offset: 1.0, color: BLUE },
    ];
    // 90deg: left to right.
    let q = quad(Rect::new(0.0, 0.0, 64.0, 64.0), Fill::Linear { angle_deg: 90.0, stops: stops.into_iter().collect() });
    let img = g.image(vec![Prim::Quad(q)], &Res::default());
    assert!(near(px(&img, 0, 32), [255, 0, 0, 255], 12), "{:?}", px(&img, 0, 32));
    assert!(near(px(&img, 32, 32), [0, 255, 0, 255], 12), "{:?}", px(&img, 32, 32));
    assert!(near(px(&img, 63, 32), [0, 0, 255, 255], 12), "{:?}", px(&img, 63, 32));
    check("quad_gradient", &img);
}

#[test]
fn shadows_drop_and_inset() {
    let Some(mut g) = Gpu::new() else { return };
    let drop = Shadow {
        rect: Rect::new(8.0, 8.0, 28.0, 28.0),
        radii: Corners::all(4.0),
        color: Color::rgba(0.0, 0.0, 0.0, 0.8),
        blur: 8.0,
        spread: 2.0,
        offset: Vec2::new(2.0, 2.0),
        inset: false,
    };
    let inset = Shadow { rect: Rect::new(36.0, 36.0, 60.0, 60.0), blur: 6.0, inset: true, ..drop };
    let img = g.image(vec![Prim::Shadow(drop), Prim::Shadow(inset)], &Res::default());
    // Drop shadow: dark at its centre, white far away; inset: dark at the edge, light in the middle.
    assert!(px(&img, 20, 20)[0] < 100);
    assert!(px(&img, 62, 2)[0] > 250);
    assert!(px(&img, 37, 48)[0] < px(&img, 48, 48)[0]);
    check("shadows", &img);
}

#[test]
fn lines_caps_dashes_and_waves() {
    let Some(mut g) = Gpu::new() else { return };
    let base = Line {
        from: Point::new(8.0, 8.0),
        to: Point::new(56.0, 8.0),
        width: 4.0,
        color: Color::BLACK,
        cap: LineCap::Round,
        dash: None,
        wavy: false,
    };
    let dashed = Line { from: Point::new(8.0, 24.0), to: Point::new(56.0, 24.0), cap: LineCap::Butt, dash: Some((6.0, 4.0)), ..base };
    let wavy = Line { from: Point::new(8.0, 40.0), to: Point::new(56.0, 40.0), width: 1.5, wavy: true, ..base };
    let diagonal = Line { from: Point::new(8.0, 60.0), to: Point::new(56.0, 48.0), width: 2.0, cap: LineCap::Square, ..base };
    let img = g.image(vec![Prim::Line(base), Prim::Line(dashed), Prim::Line(wavy), Prim::Line(diagonal)], &Res::default());
    assert_eq!(px(&img, 32, 8), [0, 0, 0, 255]);
    assert_eq!(px(&img, 32, 2), [255, 255, 255, 255]);
    check("lines", &img);
}

#[test]
fn glyphs_mask_and_colour() {
    let Some(mut g) = Gpu::new() else { return };
    let mut res = Res::default();
    res.runs.insert(RunId(1), run(&[1, 2, 0, 1000]));
    let black = Glyphs { run: RunId(1), origin: Point::new(4.0, 20.0), color: Color::BLACK };
    let white = Glyphs { origin: Point::new(4.25, 52.0), color: Color::WHITE, ..black };
    let bg = quad(Rect::new(0.0, 32.0, 64.0, 64.0), Fill::Solid(Color::BLACK));
    let img = g.image(vec![Prim::Glyphs(black), Prim::Quad(bg), Prim::Glyphs(white)], &res);
    // Glyph 1 at pen (4, 20): its frame's left column is fully covered.
    assert!(px(&img, 4, 16)[0] < 40, "{:?}", px(&img, 4, 16));
    // The colour glyph is red.
    assert!(near(px(&img, 31, 16), [255, 0, 0, 255], 2), "{:?}", px(&img, 31, 16));
    check("glyphs", &img);
}

#[test]
fn images_icons_and_rounded_image() {
    let Some(mut g) = Gpu::new() else { return };
    let mut res = Res::default();
    // A 4×4 checkerboard (premultiplied RGBA), a 4×4 coverage triangle.
    let checker: Vec<u8> = (0..16).flat_map(|i| if (i % 4 + i / 4) % 2 == 0 { [255, 0, 0, 255] } else { [0, 0, 255, 255] }).collect();
    res.images.insert(ImageId(1), ImageData { size: DeviceSize { w: 4, h: 4 }, format: ImageFormat::Rgba8Premul, pixels: checker });
    let tri: Vec<u8> = (0..16).map(|i| if i % 4 <= i / 4 { 255 } else { 0 }).collect();
    res.images.insert(ImageId(2), ImageData { size: DeviceSize { w: 4, h: 4 }, format: ImageFormat::A8, pixels: tri });
    let nearest = Image {
        image: ImageId(1),
        rect: Rect::new(0.0, 0.0, 32.0, 32.0),
        src: None,
        radii: Corners::ZERO,
        opacity: 1.0,
        sampling: Sampling::Nearest,
    };
    let rounded = Image { rect: Rect::new(32.0, 0.0, 64.0, 32.0), radii: Corners::all(10.0), sampling: Sampling::Linear, src: Some(Rect::new(1.0, 1.0, 3.0, 3.0)), ..nearest };
    let icon = Icon { image: ImageId(2), rect: Rect::new(0.0, 32.0, 32.0, 64.0), color: Some(BLUE), slot: None };
    let img = g.image(vec![Prim::Image(nearest), Prim::Image(rounded), Prim::Icon(icon)], &res);
    assert_eq!(px(&img, 4, 4), [255, 0, 0, 255]);
    assert_eq!(px(&img, 12, 4), [0, 0, 255, 255]);
    assert_eq!(px(&img, 33, 1), [255, 255, 255, 255], "rounded corner");
    check("images", &img);
}

#[test]
fn paths_fill_and_stroke() {
    let Some(mut g) = Gpu::new() else { return };
    let mut res = Res::default();
    res.paths.insert(
        PathId(1),
        PathData {
            cmds: vec![
                PathCmd::MoveTo(4.0, 4.0),
                PathCmd::LineTo(28.0, 4.0),
                PathCmd::QuadTo(28.0, 28.0, 4.0, 28.0),
                PathCmd::Close,
            ],
            fill_rule: FillRule::NonZero,
        },
    );
    res.paths.insert(
        PathId(2),
        PathData {
            cmds: vec![PathCmd::MoveTo(0.0, 0.0), PathCmd::CubicTo(10.0, 30.0, 20.0, -10.0, 28.0, 20.0)],
            fill_rule: FillRule::NonZero,
        },
    );
    let fill = PathPrim { path: PathId(1), transform: Affine::IDENTITY, paint: PathPaint::Fill(BLUE), slot: None };
    let stroke = PathPrim {
        path: PathId(2),
        transform: Affine::translate(32.0, 32.0),
        paint: PathPaint::Stroke { color: RED, width: 3.0 },
        slot: None,
    };
    let img = g.image(vec![Prim::Path(fill), Prim::Path(stroke)], &res);
    assert_eq!(px(&img, 8, 8), [0, 0, 255, 255]);
    assert_eq!(px(&img, 60, 4), [255, 255, 255, 255]);
    check("paths", &img);
}

#[test]
fn surfaces_and_rounded_clips() {
    let Some(mut g) = Gpu::new() else { return };
    // A registered texture: an offscreen render of a red quad.
    let surface = HeadlessTarget::new(&g.device, DeviceSize { w: 16, h: 16 });
    let l = list(vec![Prim::Quad(quad(Rect::new(4.0, 4.0, 12.0, 12.0), Fill::Solid(RED)))]);
    let items = [DrawItem { transform: Affine::IDENTITY, clip: None, opacity: 1.0, list: &l }];
    g.draw(&surface, &items, &Res::default(), &[]);
    g.renderer.register_texture(TextureId(7), surface.view().clone(), surface.size());

    let content = list(vec![
        Prim::Surface(SurfacePrim { texture: TextureId(7), rect: Rect::new(0.0, 0.0, 32.0, 32.0), opacity: 1.0 }),
        Prim::PushClip(RoundedRect { rect: Rect::new(36.0, 4.0, 60.0, 28.0), radii: Corners::all(12.0) }),
        Prim::Quad(quad(Rect::new(32.0, 0.0, 64.0, 32.0), Fill::Solid(BLUE))),
        Prim::PopClip,
        Prim::Surface(SurfacePrim { texture: TextureId(99), rect: Rect::new(0.0, 32.0, 32.0, 64.0), opacity: 1.0 }),
    ]);
    let clipped = list(vec![Prim::Quad(quad(Rect::new(0.0, 32.0, 64.0, 64.0), Fill::Solid(Color::BLACK)))]);
    let items = [
        DrawItem { transform: Affine::IDENTITY, clip: None, opacity: 1.0, list: &content },
        DrawItem {
            transform: Affine::IDENTITY,
            clip: Some(ClipShape::Rounded(RoundedRect { rect: Rect::new(36.0, 36.0, 60.0, 60.0), radii: Corners::all(6.0) })),
            opacity: 0.5,
            list: &clipped,
        },
    ];
    let target = HeadlessTarget::new(&g.device, SIZE);
    let stats = g.draw(&target, &items, &Res::default(), &[]);
    assert_eq!(stats.skipped, 1, "the unregistered surface");
    let img = target.read_rgba(&g.device, &g.queue).expect("readback");
    assert!(near(px(&img, 16, 16), [255, 0, 0, 255], 2), "{:?}", px(&img, 16, 16));
    assert_eq!(px(&img, 37, 5), [255, 255, 255, 255], "clipped corner");
    assert_eq!(px(&img, 48, 16), [0, 0, 255, 255]);
    assert!(near(px(&img, 48, 48), [128, 128, 128, 255], 2), "{:?}", px(&img, 48, 48));
    check("surface_clips", &img);
}

#[test]
fn ten_thousand_quads_draw_in_one_call() {
    let Some(mut g) = Gpu::new() else { return };
    let prims = (0..10_000)
        .map(|i| {
            let (x, y) = ((i % 100) as f32 * 4.0, (i / 100) as f32 * 4.0);
            Prim::Quad(quad(Rect::new(x, y, x + 3.0, y + 3.0), Fill::Solid(Color::BLACK)))
        })
        .collect();
    let l = list(prims);
    let items = [DrawItem { transform: Affine::IDENTITY, clip: None, opacity: 1.0, list: &l }];
    let target = HeadlessTarget::new(&g.device, DeviceSize { w: 400, h: 400 });
    let stats = g.draw(&target, &items, &Res::default(), &[]);
    assert_eq!(stats.draws, 1);
    assert_eq!(stats.batches, 1);
    assert_eq!(stats.instances, 10_000);
    // A second frame reuses the ring; same counts.
    let again = g.draw(&target, &items, &Res::default(), &[]);
    assert_eq!((again.draws, again.instances), (1, 10_000));
}

#[test]
fn damage_redraw_leaves_other_pixels_untouched() {
    let Some(mut g) = Gpu::new() else { return };
    let res = Res::default();
    let target = HeadlessTarget::new(&g.device, SIZE);
    let red = list(vec![Prim::Quad(quad(Rect::new(0.0, 0.0, 64.0, 64.0), Fill::Solid(RED)))]);
    let items = [DrawItem { transform: Affine::IDENTITY, clip: None, opacity: 1.0, list: &red }];
    g.draw(&target, &items, &res, &[]);
    let before = target.read_rgba(&g.device, &g.queue).expect("readback");

    // Now the scene is blue, but only two rects are damaged.
    let blue = list(vec![Prim::Quad(quad(Rect::new(0.0, 0.0, 64.0, 64.0), Fill::Solid(BLUE)))]);
    let items = [DrawItem { transform: Affine::IDENTITY, clip: None, opacity: 1.0, list: &blue }];
    let damage = [DeviceRect { x: 4, y: 4, w: 8, h: 8 }, DeviceRect { x: 40, y: 40, w: 10, h: 10 }];
    let stats = g.draw(&target, &items, &res, &damage);
    assert_eq!(stats.regions, 2);
    let after = target.read_rgba(&g.device, &g.queue).expect("readback");
    let damaged = |x: u32, y: u32| damage.iter().any(|d| x >= d.x as u32 && x < d.x as u32 + d.w && y >= d.y as u32 && y < d.y as u32 + d.h);
    for y in 0..SIZE.h {
        for x in 0..SIZE.w {
            if damaged(x, y) {
                assert_eq!(px(&after, x, y), [0, 0, 255, 255], "damaged ({x}, {y})");
            } else {
                assert_eq!(px(&after, x, y), px(&before, x, y), "untouched ({x}, {y})");
            }
        }
    }

    // A damaged area with no content is cleared to the clear colour.
    let empty: [DrawItem<'_>; 0] = [];
    g.draw(&target, &empty, &res, &[DeviceRect { x: 0, y: 0, w: 2, h: 2 }]);
    let cleared = target.read_rgba(&g.device, &g.queue).expect("readback");
    assert_eq!(px(&cleared, 1, 1), [255, 255, 255, 255]);
    assert_eq!(px(&cleared, 3, 3), [255, 0, 0, 255]);
}

#[test]
fn atlas_survives_many_frames_and_glyph_churn() {
    let Some(mut g) = Gpu::new() else { return };
    let mut res = Res::default();
    let target = HeadlessTarget::new(&g.device, SIZE);
    // Many distinct sizes: each frame rasterises new keys; pages are reused, never unbounded.
    for i in 0..50u32 {
        let mut r = run(&[1, 2, 3]);
        r.size = 8.0 + i as f32;
        res.runs.insert(RunId(1), r);
        let l = list(vec![Prim::Glyphs(Glyphs { run: RunId(1), origin: Point::new(2.0, 30.0), color: Color::BLACK })]);
        let items = [DrawItem { transform: Affine::IDENTITY, clip: None, opacity: 1.0, list: &l }];
        let stats = g.draw(&target, &items, &res, &[]);
        assert!(stats.atlas_pages >= 1 && stats.atlas_pages <= skelter_render::MAX_ATLAS_PAGES as u32 * 2);
        assert_eq!(stats.skipped, 0);
    }
}

#[test]
fn real_text_through_skelter_text() {
    let Some((device, queue)) = gpu() else { return };
    use skelter_text::{FontConfig, FontRasterizer, FontStore, RichText, TextStyle, TextSystem};
    let fonts = FontStore::new(FontConfig::testing());
    let mut renderer = Renderer::new(&device, &queue, FORMAT, Arc::new(FontRasterizer::new(fonts.clone())));
    let mut text = TextSystem::new(fonts);
    let layout = text.layout(&RichText::plain("Hag", TextStyle { size: 20.0, ..Default::default() }), None);
    let mut res = Res::default();
    let mut prims = Vec::new();
    for (i, placed) in layout.runs().iter().enumerate() {
        let id = RunId(i as u64 + 1);
        res.runs.insert(id, (*placed.run).clone());
        prims.push(Prim::Glyphs(Glyphs { run: id, origin: Point::new(placed.origin.x + 4.0, placed.origin.y + 8.0), color: Color::BLACK }));
    }
    assert!(!prims.is_empty(), "skelter-text laid out no runs");
    let l = list(prims);
    let items = [DrawItem { transform: Affine::IDENTITY, clip: None, opacity: 1.0, list: &l }];
    let target = HeadlessTarget::new(&device, SIZE);
    let scene = FrameScene { target: SIZE, scale: Scale(1.0), clear: Some(Color::WHITE), damage: &[], items: &items, resources: &res };
    let stats = target.render(&device, &queue, &mut renderer, &scene);
    assert!(stats.instances >= 3, "{stats:?}");
    let img = target.read_rgba(&device, &queue).expect("readback");
    assert!(img.chunks(4).any(|p| p[0] < 64), "no dark pixels: the glyphs did not draw");
    check("real_text", &img);
}

/// SKELTER-svg: a vector icon at 200 px (a 24-unit drawing scaled up, as skelter-widgets'
/// `VectorArt` draws one) keeps sharp edges: across an edge, at most two pixels are partly covered,
/// at 1x and at a fractional 1.25x.
#[test]
fn vector_icons_stay_sharp_at_200_px() {
    let Some(mut g) = Gpu::new() else { return };
    // a disc of radius 10 at (12, 12) in four cubics, and a square ring (two subpaths, even-odd)
    let k = 0.552_284_8 * 10.0;
    let (cx, cy) = (12.0, 12.0);
    let disc = PathData {
        cmds: vec![
            PathCmd::MoveTo(cx + 10.0, cy),
            PathCmd::CubicTo(cx + 10.0, cy + k, cx + k, cy + 10.0, cx, cy + 10.0),
            PathCmd::CubicTo(cx - k, cy + 10.0, cx - 10.0, cy + k, cx - 10.0, cy),
            PathCmd::CubicTo(cx - 10.0, cy - k, cx - k, cy - 10.0, cx, cy - 10.0),
            PathCmd::CubicTo(cx + k, cy - 10.0, cx + 10.0, cy - k, cx + 10.0, cy),
            PathCmd::Close,
        ],
        fill_rule: FillRule::NonZero,
    };
    let mut res = Res::default();
    res.paths.insert(PathId(1), disc);
    let size = DeviceSize { w: 256, h: 256 };
    for scale in [1.0f32, 1.25] {
        // 200 device pixels for the 24 units, from (20, 20)
        let ppu = 200.0 / 24.0;
        let t = Affine::scale(1.0 / scale, 1.0 / scale).then(&Affine::translate(20.0, 20.0)).then(&Affine::scale(ppu, ppu));
        let slot = Some(PaletteSlot::new(0, 1.0));
        let l = list(vec![Prim::Path(PathPrim { path: PathId(1), transform: t, paint: PathPaint::Fill(Color::BLACK), slot })]);
        let items = [DrawItem { transform: Affine::IDENTITY, clip: None, opacity: 1.0, list: &l }];
        let target = HeadlessTarget::new(&g.device, size);
        let scene = FrameScene { target: size, scale: Scale(scale), clear: Some(Color::WHITE), damage: &[], items: &items, resources: &res };
        target.render(&g.device, &g.queue, &mut g.renderer, &scene);
        let img = target.read_rgba(&g.device, &g.queue).expect("readback");
        let at = |x: u32, y: u32| img[((y * size.w + x) * 4) as usize];
        // the disc spans 20 + 8.33 .. 20 + 191.67 device pixels; its centre is black
        assert!(at(120, 120) < 5, "inside: {}", at(120, 120));
        assert!(at(25, 120) > 250 && at(215, 120) > 250, "outside");
        for (name, line) in [("left", (0..60).map(|x| at(x, 120)).collect::<Vec<_>>()), ("top", (0..60).map(|y| at(120, y)).collect()), ("diagonal", (20..90).map(|d| at(d, d)).collect())] {
            let partial = line.iter().filter(|&&v| v > 8 && v < 247).count();
            assert!(partial <= 2, "{scale}x {name} edge: {partial} partly covered pixels: {line:?}");
        }
    }
}
