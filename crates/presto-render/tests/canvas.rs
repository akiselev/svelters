//! Canvas layers through the renderer (architecture §9.5): a `Prim::Canvas` drawn by the canvas
//! renderer, its static cache reused while nothing changes, drawn directly while the camera moves,
//! the overlay drawn over the cache, and a Slug label. Pixel checks, no reference images.
//!
//! GPU tests: they run only with `PRESTO_GPU_TESTS=1` (CI sets it).

use std::sync::Arc;

use presto_base::color::Color;
use presto_base::display::{CanvasPrim, DisplayList, Prim};
use presto_base::geom::{Affine, DeviceSize, Rect, Scale};
use presto_base::ids::CanvasId;
use presto_base::scene::{DrawItem, FrameScene, SceneResources};
use presto_base::text::{GlyphImage, GlyphKey as RasterKey, GlyphRasterizer, GlyphRun};
use presto_canvas::chunk::Cap;
use presto_canvas::{Camera, Canvas, CanvasFonts, CanvasView, ChunkBuilder, DVec2, Label, Plane, WorldRect};
use presto_render::headless::{FORMAT, HeadlessTarget};
use presto_render::{RenderStats, Renderer};

const SIZE: DeviceSize = DeviceSize { w: 64, h: 64 };
const ID: CanvasId = CanvasId(7);
const NOTO: &[u8] = include_bytes!("../../presto-text/fonts/NotoSans-Regular.subset.ttf");

fn gpu() -> Option<(wgpu::Device, wgpu::Queue)> {
    if std::env::var("PRESTO_GPU_TESTS").as_deref() != Ok("1") {
        eprintln!("skipped: set PRESTO_GPU_TESTS=1 to run GPU tests");
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
        .expect("PRESTO_GPU_TESTS=1 but no wgpu adapter");
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).expect("wgpu device");
    Some((device, queue))
}

struct NoGlyphs;
impl GlyphRasterizer for NoGlyphs {
    fn rasterize(&self, _: &RasterKey) -> Option<GlyphImage> {
        None
    }
}

struct NoRes;
impl SceneResources for NoRes {
    fn run(&self, _: presto_base::ids::RunId) -> Option<&GlyphRun> {
        None
    }
    fn image(&self, _: presto_base::ids::ImageId) -> Option<&presto_base::commit::ImageData> {
        None
    }
    fn path(&self, _: presto_base::ids::PathId) -> Option<&presto_base::commit::PathData> {
        None
    }
}

struct Setup {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: Renderer,
    target: HeadlessTarget,
    list: DisplayList,
    canvas: Arc<Canvas>,
}

const RED: Color = Color::rgba(1.0, 0.0, 0.0, 1.0);
const BLUE: Color = Color::rgba(0.0, 0.0, 1.0, 1.0);

/// A 64×64 canvas at 1 point per world unit centred on the world origin: a red square from −10 to
/// 10 (device 22..42) and a blue horizontal segment through y = 20 (device row 52) in the static
/// plane.
fn setup() -> Option<Setup> {
    let (device, queue) = gpu()?;
    let mut renderer = Renderer::new(&device, &queue, FORMAT, Arc::new(NoGlyphs));
    let canvas = Canvas::new(ID, CanvasFonts::new());
    canvas.edit(|e| {
        let mut b = ChunkBuilder::new(DVec2::new(0.0, 0.0));
        b.rect(WorldRect::new(-10.0, -10.0, 10.0, 10.0), 0.0, 0.0, RED);
        b.segment(DVec2::new(-30.0, 20.0), DVec2::new(30.0, 20.0), 3.0, Cap::Butt, BLUE);
        e.insert(Plane::Static, b.build());
    });
    renderer.register_canvas(canvas.clone());
    let mut list = DisplayList::new();
    let r = Rect::new(0.0, 0.0, 64.0, 64.0);
    list.push(Prim::Canvas(CanvasPrim { canvas: ID, rect: r }), r);
    let target = HeadlessTarget::new(&device, SIZE);
    Some(Setup { device, queue, renderer, target, list, canvas })
}

impl Setup {
    fn frame(&mut self) -> RenderStats {
        let items = [DrawItem { transform: Affine::IDENTITY, clip: None, opacity: 1.0, list: &self.list }];
        let scene = FrameScene { target: SIZE, scale: Scale(1.0), clear: Some(Color::WHITE), damage: &[], items: &items, resources: &NoRes };
        self.target.render(&self.device, &self.queue, &mut self.renderer, &scene)
    }
    fn pixels(&self) -> Vec<u8> {
        self.target.read_rgba(&self.device, &self.queue).expect("readback")
    }
}

fn px(img: &[u8], x: u32, y: u32) -> [u8; 4] {
    let i = ((y * SIZE.w + x) * 4) as usize;
    [img[i], img[i + 1], img[i + 2], img[i + 3]]
}

fn near(a: [u8; 4], b: [u8; 4]) -> bool {
    a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= 8)
}

#[test]
fn canvas_shapes_draw_at_their_camera_position() {
    let Some(mut s) = setup() else { return };
    let stats = s.frame();
    assert_eq!(stats.canvas.placements, 1);
    assert_eq!(stats.canvas.cache_renders, 1);
    let img = s.pixels();
    assert!(near(px(&img, 32, 32), [255, 0, 0, 255]), "the square's centre: {:?}", px(&img, 32, 32));
    assert!(near(px(&img, 23, 23), [255, 0, 0, 255]));
    assert!(near(px(&img, 20, 32), [255, 255, 255, 255]), "left of the square");
    assert!(near(px(&img, 32, 52), [0, 0, 255, 255]), "the segment: {:?}", px(&img, 32, 52));
    assert!(near(px(&img, 32, 56), [255, 255, 255, 255]), "below the segment");
}

#[test]
fn an_unchanged_canvas_reuses_its_cache_and_a_moving_one_draws_directly() {
    let Some(mut s) = setup() else { return };
    s.frame();
    let again = s.frame();
    assert_eq!((again.canvas.cache_hits, again.canvas.cache_renders), (1, 0));

    // Two camera changes in a row: the first re-renders the cache, the second draws directly.
    let mut view = CanvasView::default();
    view.camera = Camera::new(DVec2::new(5.0, 0.0), 1.0);
    s.renderer.set_canvas_view(ID, view);
    assert_eq!(s.frame().canvas.cache_renders, 1);
    view.camera = Camera::new(DVec2::new(10.0, 0.0), 1.0);
    s.renderer.set_canvas_view(ID, view);
    let moving = s.frame();
    assert_eq!((moving.canvas.direct, moving.canvas.cache_renders), (1, 0));
    let img = s.pixels();
    // The square moved 10 device pixels left (the camera looks 10 units right).
    assert!(near(px(&img, 15, 32), [255, 0, 0, 255]), "{:?}", px(&img, 15, 32));
    assert!(near(px(&img, 36, 32), [255, 255, 255, 255]), "{:?}", px(&img, 36, 32));
    // Still: the cache comes back.
    s.frame();
    assert_eq!(s.frame().canvas.cache_hits, 1);
}

#[test]
fn the_overlay_draws_over_the_cached_static_plane() {
    let Some(mut s) = setup() else { return };
    s.frame();
    s.canvas.edit(|e| {
        let mut b = ChunkBuilder::new(DVec2::ZERO);
        b.circle(DVec2::new(0.0, 0.0), 4.0, 0.0, BLUE);
        e.insert(Plane::Overlay, b.build());
    });
    let stats = s.frame();
    assert_eq!((stats.canvas.cache_hits, stats.canvas.cache_renders), (1, 0), "the static plane did not change");
    let img = s.pixels();
    assert!(near(px(&img, 32, 32), [0, 0, 255, 255]), "the overlay circle: {:?}", px(&img, 32, 32));
    assert!(near(px(&img, 24, 24), [255, 0, 0, 255]), "the square around it");
}

#[test]
fn slug_labels_cover_their_glyphs() {
    let Some(mut s) = setup() else { return };
    let fonts = s.canvas.fonts().clone();
    let font = fonts.register(Arc::from(NOTO), 0).expect("font");
    s.canvas.edit(|e| {
        e.clear(Plane::Static);
        let mut b = ChunkBuilder::new(DVec2::ZERO);
        // An "l" 48 units tall, its baseline 20 below the centre.
        b.label(&fonts, &Label { text: "l", font, pos: DVec2::new(-4.0, 20.0), size: 48.0, rotation: 0.0, color: Color::BLACK, align: 0.0 });
        e.insert(Plane::Static, b.build());
    });
    let stats = s.frame();
    assert_eq!(stats.canvas.glyphs, 1);
    let img = s.pixels();
    let g = fonts.glyph(fonts.layout(font, "l").unwrap().glyphs[0].key).unwrap();
    // The stem's centre, in device pixels: x = 32 − 4 + centre·48, y = 32 + 20 − centre·48.
    let cx = 28.0 + (g.bbox[0] + g.bbox[2]) * 0.5 * 48.0;
    let cy = 52.0 - (g.bbox[1] + g.bbox[3]) * 0.5 * 48.0;
    let p = px(&img, cx as u32, cy as u32);
    assert!(near(p, [0, 0, 0, 255]), "the stem of l at ({cx}, {cy}): {p:?}");
    let right = px(&img, (cx + 8.0) as u32, cy as u32);
    assert!(near(right, [255, 255, 255, 255]), "beside the stem: {right:?}");
}
