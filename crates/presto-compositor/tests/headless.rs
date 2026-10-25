//! A headless frame on a real GPU (skipped when the machine has no adapter, software ones included):
//! the engine drives the wgpu backend into an offscreen target, the GPU reports completion, and the
//! pixels read back. The renderer here only clears its target, which is what this test checks; scene
//! content is presto-render's to test.

use presto_base::color::Color;
use presto_base::commit::{Commit, ToUi};
use presto_base::geom::{DeviceSize, Rect, Scale};
use presto_base::ids::{LayerId, PropNodeId, Revision, TextureId, WindowId};
use presto_base::layer::{LayerDelta, LayerDesc, LayerKind};
use presto_base::scene::FrameScene;
use presto_base::text::{GlyphImage, GlyphKey, GlyphRasterizer};
use presto_base::time::{Clock, Span, Time};
use presto_compositor::core::{Engine, EngineConfig, Msg};
use presto_compositor::gpu::GpuBackend;
use presto_compositor::seams::{self, SceneRenderer};
use presto_compositor::tree::SurfaceKey;
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

struct NoGlyphs;
impl GlyphRasterizer for NoGlyphs {
    fn rasterize(&self, _: &GlyphKey) -> Option<GlyphImage> {
        None
    }
}

struct InstantClock(Instant);
impl Clock for InstantClock {
    fn now(&self) -> Time {
        Time(self.0.elapsed().as_nanos() as u64 + 1_000_000_000)
    }
}

/// Clears the target to the scene's clear colour.
struct ClearRenderer;
impl SceneRenderer for ClearRenderer {
    fn register_texture(&mut self, _: TextureId, _: wgpu::TextureView, _: DeviceSize) {}
    fn unregister_texture(&mut self, _: TextureId) {}
    fn render(
        &mut self,
        _: &wgpu::Device,
        _: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        scene: &FrameScene<'_>,
    ) {
        let c = scene.clear.unwrap_or(Color::TRANSPARENT).premultiplied();
        let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color { r: c[0] as f64, g: c[1] as f64, b: c[2] as f64, a: c[3] as f64 }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
    }
    fn end_frame(&mut self) {}
}

#[test]
fn headless_frame_renders_and_reports_gpu_completion() {
    let clock: Arc<dyn Clock> = Arc::new(InstantClock(Instant::now()));
    let (tx, rx) = mpsc::channel();
    let instance = presto_compositor::gpu::create_instance(None);
    let mut gpu = GpuBackend::new(
        instance,
        wgpu::PowerPreference::LowPower,
        Arc::new(std::sync::RwLock::new(None)),
        Box::new(|_, _, _, _| Box::new(ClearRenderer)),
        Arc::new(NoGlyphs),
        tx,
        clock.clone(),
        1,
    );
    let w = WindowId(1);
    let size = DeviceSize { w: 64, h: 32 };
    if !gpu.attach_offscreen(w, SurfaceKey::Main, size) {
        eprintln!("no GPU adapter: skipping");
        return;
    }
    let sent = Arc::new(Mutex::new(Vec::new()));
    let s = sent.clone();
    let red = Color::rgba(1.0, 0.0, 0.0, 1.0);
    let mut engine = Engine::new(
        EngineConfig { refresh: Span::from_micros(16_667), clear: Some(red), caret_color: Color::BLACK, caret_blink: None },
        Box::new(seams::frame_clock),
        None,
        None,
        Box::new(move |m: ToUi| s.lock().unwrap().push(m)),
        Box::new(|| {}),
    );
    engine.insert_window(w, size, Scale(1.0));
    let mut c = Commit { window: w, revision: Revision(1), ..Default::default() };
    c.layers = vec![LayerDelta::Add(LayerDesc {
        id: LayerId(1),
        parent: None,
        kind: LayerKind::Root,
        transform: PropNodeId(1),
        clip: None,
        effect: None,
        z: 0,
        bounds: Rect::new(0.0, 0.0, 64.0, 32.0),
        opaque: true,
    })];
    engine.handle(Msg::Commit(Box::new(c)), clock.now(), &mut gpu);
    assert_eq!(engine.stats.frames_drawn, 1);

    let done = rx.recv_timeout(Duration::from_secs(10)).expect("GpuDone");
    assert!(matches!(done, Msg::GpuDone { window, .. } if window == w));
    engine.handle(done, clock.now(), &mut gpu);
    assert!(sent.lock().unwrap().iter().any(|m| matches!(m, ToUi::Presented { estimated: true, .. })));

    let (got, format, pixels) = gpu.read_pixels(w, SurfaceKey::Main).expect("read back");
    assert_eq!(got, size);
    let px = &pixels[0..4];
    let expect: [u8; 4] = match format {
        wgpu::TextureFormat::Bgra8Unorm => [0, 0, 255, 255],
        _ => [255, 0, 0, 255],
    };
    assert_eq!(px, expect);
    assert!(pixels.chunks(4).all(|p| p == expect));
}
