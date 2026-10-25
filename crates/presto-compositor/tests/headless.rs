//! A headless frame on a real GPU (skipped when the machine has no adapter, software ones included):
//! the engine drives the wgpu backend into an offscreen target, the GPU reports completion, and the
//! pixels read back. The renderer here only clears its target, which is what this test checks; scene
//! content is presto-render's to test.

use presto_base::color::Color;
use presto_base::commit::{Commit, ToUi};
use presto_base::geom::{DeviceSize, Rect, Scale};
use presto_base::ids::{LayerId, NodeId, PropNodeId, Revision, WindowId};
use presto_base::layer::{ItemRef, LayerDelta, LayerDesc, LayerKind};
use presto_base::color::Fill;
use presto_base::display::{DisplayList, Prim, Quad};
use presto_base::geom::{Affine, Corners, Insets};
use presto_base::props::{PropOp, TransformNode};
use presto_base::text::{GlyphImage, GlyphKey, GlyphRasterizer};
use presto_base::time::{Clock, Span, Time};
use presto_compositor::core::{Engine, EngineConfig, Msg};
use presto_compositor::gpu::GpuBackend;
use presto_compositor::seams;
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

#[test]
fn headless_frame_renders_and_reports_gpu_completion() {
    let clock: Arc<dyn Clock> = Arc::new(InstantClock(Instant::now()));
    let (tx, rx) = mpsc::channel();
    let instance = presto_compositor::gpu::create_instance(None);
    let mut gpu = GpuBackend::new(
        instance,
        wgpu::PowerPreference::LowPower,
        Arc::new(std::sync::RwLock::new(None)),
        seams::presto_renderer(),
        Arc::new(NoGlyphs),
        tx,
        clock.clone(),
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
    let white = Color::WHITE;
    let mut engine = Engine::new(
        EngineConfig { refresh: Span::from_micros(16_667), clear: Some(white), caret_color: Color::BLACK, caret_blink: None, ppi: 96.0 },
        Box::new(seams::frame_clock),
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
    c.props.ops = vec![PropOp::Transform(PropNodeId(1), TransformNode { parent: None, local: Affine::IDENTITY, scroll: None })];
    let node = NodeId::new(1, 1);
    c.layers.push(LayerDelta::Items {
        layer: LayerId(1),
        items: vec![ItemRef { node, transform: PropNodeId(1), clip: None, effect: None }],
    });
    let left = Rect::new(0.0, 0.0, 32.0, 32.0);
    let mut list = DisplayList::new();
    list.push(
        Prim::Quad(Quad {
            rect: left,
            radii: Corners::ZERO,
            fill: Fill::Solid(red),
            border_widths: Insets::ZERO,
            border_color: Color::TRANSPARENT,
            border_style: Default::default(),
        }),
        left,
    );
    c.lists = vec![(node, Arc::new(list))];
    engine.handle(Msg::Commit(Box::new(c)), clock.now(), &mut gpu);
    assert_eq!(engine.stats.frames_drawn, 1);

    let done = rx.recv_timeout(Duration::from_secs(10)).expect("GpuDone");
    assert!(matches!(done, Msg::GpuDone { window, .. } if window == w));
    engine.handle(done, clock.now(), &mut gpu);
    assert!(sent.lock().unwrap().iter().any(|m| matches!(m, ToUi::Presented { estimated: true, .. })));

    let (got, format, pixels) = gpu.read_pixels(w, SurfaceKey::Main).expect("read back");
    assert_eq!(got, size);
    let (r, g, b) = match format {
        wgpu::TextureFormat::Bgra8Unorm => (2, 1, 0),
        _ => (0, 1, 2),
    };
    let at = |x: u32, y: u32| {
        let i = ((y * size.w + x) * 4) as usize;
        [pixels[i + r], pixels[i + g], pixels[i + b], pixels[i + 3]]
    };
    assert_eq!(at(8, 16), [255, 0, 0, 255], "inside the quad");
    assert_eq!(at(56, 16), [255, 255, 255, 255], "the clear colour outside it");
}

#[test]
fn adapter_selection_and_limits_hooks_are_used() {
    let clock: Arc<dyn Clock> = Arc::new(InstantClock(Instant::now()));
    let (tx, _rx) = mpsc::channel();
    let shared = Arc::new(std::sync::RwLock::new(None));
    let mut gpu = GpuBackend::new(
        presto_compositor::gpu::create_instance(None),
        wgpu::PowerPreference::LowPower,
        shared.clone(),
        seams::presto_renderer(),
        Arc::new(NoGlyphs),
        tx,
        clock,
    );
    let seen = Arc::new(Mutex::new(0usize));
    let s = seen.clone();
    let limits_called = Arc::new(Mutex::new(false));
    let l = limits_called.clone();
    gpu.set_selection(
        Some(Arc::new(move |adapters: &[wgpu::Adapter]| {
            *s.lock().unwrap() = adapters.len();
            adapters.first().cloned().ok_or_else(|| "no adapters".to_string())
        })),
        Some(Arc::new(move |a: &wgpu::Adapter| {
            *l.lock().unwrap() = true;
            a.limits()
        })),
    );
    if !gpu.attach_offscreen(WindowId(1), SurfaceKey::Main, DeviceSize { w: 8, h: 8 }) {
        assert_eq!(*seen.lock().unwrap(), 0, "the hook saw no adapter");
        eprintln!("no GPU adapter: skipping");
        return;
    }
    assert!(*seen.lock().unwrap() >= 1);
    assert!(*limits_called.lock().unwrap());
    assert!(shared.read().unwrap().is_some(), "the device is published");
}

/// CADATOMIC's live bug: with a caret in the window, everything but the caret went black. A
/// swapchain that cycles through more images than assumed got only the caret's damage copied into
/// images that had missed frames. Here an emulated six-image swapchain takes content, then caret-only
/// frames, and the image last presented must still hold the content.
#[test]
fn caret_only_frames_keep_the_content_on_every_swapchain_image() {
    let clock: Arc<dyn Clock> = Arc::new(InstantClock(Instant::now()));
    let (tx, rx) = mpsc::channel();
    let mut gpu = GpuBackend::new(
        presto_compositor::gpu::create_instance(None),
        wgpu::PowerPreference::LowPower,
        Arc::new(std::sync::RwLock::new(None)),
        seams::presto_renderer(),
        Arc::new(NoGlyphs),
        tx,
        clock.clone(),
    );
    let w = WindowId(1);
    let size = DeviceSize { w: 64, h: 32 };
    if !gpu.attach_offscreen_chain(w, SurfaceKey::Main, size, 6) {
        eprintln!("no GPU adapter: skipping");
        return;
    }
    let sent = Arc::new(Mutex::new(Vec::new()));
    let s = sent.clone();
    let red = Color::rgba(1.0, 0.0, 0.0, 1.0);
    let mut engine = Engine::new(
        EngineConfig {
            refresh: Span::from_micros(16_667),
            clear: Some(Color::WHITE),
            caret_color: Color::BLACK,
            caret_blink: None,
            ppi: 96.0,
        },
        Box::new(seams::frame_clock),
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
    c.props.ops = vec![PropOp::Transform(PropNodeId(1), TransformNode { parent: None, local: Affine::IDENTITY, scroll: None })];
    let node = NodeId::new(1, 1);
    c.layers.push(LayerDelta::Items {
        layer: LayerId(1),
        items: vec![ItemRef { node, transform: PropNodeId(1), clip: None, effect: None }],
    });
    let left = Rect::new(0.0, 0.0, 32.0, 32.0);
    let mut list = DisplayList::new();
    list.push(
        Prim::Quad(Quad {
            rect: left,
            radii: Corners::ZERO,
            fill: Fill::Solid(red),
            border_widths: Insets::ZERO,
            border_color: Color::TRANSPARENT,
            border_style: Default::default(),
        }),
        left,
    );
    c.lists = vec![(node, Arc::new(list))];
    engine.handle(Msg::Commit(Box::new(c)), clock.now(), &mut gpu);

    // Caret-only frames, more than the swapchain has images: each moves the caret on the white half.
    let mut revision = 2;
    let deadline = Instant::now() + Duration::from_secs(20);
    while engine.stats.frames_drawn < 12 && Instant::now() < deadline {
        if let Ok(m) = rx.recv_timeout(Duration::from_millis(5)) {
            engine.handle(m, clock.now(), &mut gpu);
        }
        let x = 40.0 + (revision % 8) as f32 * 2.0;
        let caret = presto_base::commit::FastCommit::Caret {
            window: w,
            rect: Some(Rect::new(x, 8.0, x + 1.0, 24.0)),
            revision: Revision(revision),
        };
        engine.handle(Msg::Fast(caret), clock.now(), &mut gpu);
        revision += 1;
        engine.advance(clock.now(), &mut gpu);
    }
    assert!(engine.stats.frames_drawn >= 12, "only {} frames", engine.stats.frames_drawn);

    let (_, format, pixels) = gpu.read_pixels(w, SurfaceKey::Main).expect("read back");
    let (r, g, b) = match format {
        wgpu::TextureFormat::Bgra8Unorm => (2, 1, 0),
        _ => (0, 1, 2),
    };
    let at = |x: u32, y: u32| {
        let i = ((y * size.w + x) * 4) as usize;
        [pixels[i + r], pixels[i + g], pixels[i + b], pixels[i + 3]]
    };
    assert_eq!(at(8, 16), [255, 0, 0, 255], "the content survives caret-only frames");
    assert_eq!(at(36, 2), [255, 255, 255, 255], "so does the background");
}
