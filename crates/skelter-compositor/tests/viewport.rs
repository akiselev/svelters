//! Viewports and canvases on the compositor (plan W-V1, W-V2), with a virtual clock and a recording
//! backend:
//! - orbiting a viewport on an OS layer redraws only that layer's surface: no commit, no UI frame,
//!   no draw of the window's surface (the W-V1 counter gate);
//! - in the texture fallback the window's surface redraws inside the viewport's bounds only;
//! - the resize handshake draws the new size before answering;
//! - panning a canvas is a camera property: the canvas is redrawn with its new view, with no commit.

use skelter_base::color::{Color, Fill};
use skelter_base::commit::{Commit, FastCommit, ToUi};
use skelter_base::display::{DisplayList, Prim, Quad};
use skelter_base::geom::{Affine, Corners, DeviceRect, DeviceSize, Insets, Rect, Scale};
use skelter_base::ids::{CanvasId, FrameId, InputSeq, LayerId, NodeId, PropNodeId, Revision, SurfaceId, TextureId, WindowId};
use skelter_base::layer::{ItemRef, LayerDelta, LayerDesc, LayerKind};
use skelter_base::props::{PropOp, TransformNode};
use skelter_base::scene::FrameScene;
use skelter_base::time::{Span, Time};
use skelter_canvas::{Camera, CanvasView, DVec2};
use skelter_compositor::core::{Backend, DrawOutcome, Engine, EngineConfig, Msg};
use skelter_compositor::seams;
use skelter_compositor::tree::SurfaceKey;
use skelter_compositor::viewport::{layer_descs, Presentation, ViewportLayerIds};
use std::sync::{Arc, Mutex};

const W: WindowId = WindowId(1);
const ROOT: LayerId = LayerId(1);
const T_ROOT: PropNodeId = PropNodeId(1);
const SID: SurfaceId = SurfaceId(9);
const TEX: TextureId = TextureId(5);
const CANVAS: CanvasId = CanvasId(3);
const IDS: ViewportLayerIds = ViewportLayerIds { os: LayerId(10), texture: LayerId(11) };
const VIEWPORT: Rect = Rect::new(100.0, 50.0, 300.0, 250.0);
const INTERVAL: Span = Span::from_micros(16_667);
const T0: Time = Time(INTERVAL.0 as u64 * 60 + 1_000_000);

#[derive(Debug, Clone)]
struct Drawn {
    key: SurfaceKey,
    frame: Option<FrameId>,
    damage: Vec<DeviceRect>,
}

#[derive(Default)]
struct FakeGpu {
    draws: Vec<Drawn>,
    resized: Vec<(SurfaceKey, DeviceSize)>,
    views: Vec<(CanvasId, CanvasView)>,
}

impl Backend for FakeGpu {
    fn attach(&mut self, _: WindowId, _: SurfaceKey, _: wgpu::Surface<'static>, _: DeviceSize) {}
    fn detach(&mut self, _: WindowId, _: SurfaceKey) {}
    fn resize(&mut self, _: WindowId, key: SurfaceKey, size: DeviceSize) {
        self.resized.push((key, size));
    }
    fn register_texture(&mut self, _: TextureId, _: wgpu::Texture) {}
    fn unregister_texture(&mut self, _: TextureId) {}
    fn draw(&mut self, _: WindowId, key: SurfaceKey, frame: Option<FrameId>, scene: &FrameScene<'_>) -> DrawOutcome {
        self.draws.push(Drawn { key, frame, damage: scene.damage.to_vec() });
        DrawOutcome::Presented { submitted: Time(0) }
    }
    fn forget(&mut self, _: &[skelter_base::ids::ImageId], _: &[skelter_base::ids::PathId]) {}
    fn set_canvas_view(&mut self, id: CanvasId, view: CanvasView) {
        self.views.push((id, view));
    }
    fn recover(&mut self) -> bool {
        true
    }
}

type Sent = Arc<Mutex<Vec<ToUi>>>;

fn engine() -> (Engine, Sent) {
    let sent: Sent = Arc::default();
    let s = sent.clone();
    let e = Engine::new(
        EngineConfig { refresh: INTERVAL, clear: Some(Color::WHITE), caret_blink: None, ppi: 96.0 },
        Box::new(seams::frame_clock),
        Box::new(move |m| s.lock().unwrap().push(m)),
        Box::new(|| {}),
    );
    (e, sent)
}

fn quad(r: Rect) -> Arc<DisplayList> {
    let mut l = DisplayList::new();
    l.push(
        Prim::Quad(Quad {
            rect: r,
            radii: Corners::ZERO,
            fill: Fill::Solid(Color::BLACK),
            border_widths: Insets::ZERO,
            border_color: Color::TRANSPARENT,
            border_style: Default::default(),
        }),
        r,
    );
    Arc::new(l)
}

/// A window with UI content (a toolbar and a side panel) and `extra` layers above it.
fn commit(extra: Vec<LayerDesc>) -> Commit {
    let mut c = Commit { window: W, revision: Revision(1), input_seq: InputSeq(1), ..Default::default() };
    c.props.ops = vec![PropOp::Transform(T_ROOT, TransformNode { parent: None, local: Affine::IDENTITY, scroll: None })];
    c.layers.push(LayerDelta::Add(LayerDesc {
        id: ROOT,
        parent: None,
        kind: LayerKind::Root,
        transform: T_ROOT,
        clip: None,
        effect: None,
        z: 0,
        bounds: Rect::new(0.0, 0.0, 400.0, 300.0),
        opaque: true,
    }));
    for d in extra {
        c.layers.push(LayerDelta::Add(d));
    }
    let a = NodeId::new(1, 1);
    let b = NodeId::new(2, 1);
    c.layers.push(LayerDelta::Items {
        layer: ROOT,
        items: vec![ItemRef { node: a, transform: T_ROOT, clip: None, effect: None }, ItemRef { node: b, transform: T_ROOT, clip: None, effect: None }],
    });
    c.lists = vec![(a, quad(Rect::new(0.0, 0.0, 400.0, 40.0))), (b, quad(Rect::new(0.0, 40.0, 90.0, 300.0)))];
    c
}

fn viewport_layers(p: Presentation) -> Vec<LayerDesc> {
    layer_descs(p, IDS, ROOT, T_ROOT, 1, VIEWPORT, TEX, DeviceSize { w: 200, h: 200 }, Scale(1.0), true)
}

/// Advances to the next wakeup and completes the GPU work of frames drawn there.
fn step(e: &mut Engine, gpu: &mut FakeGpu) -> Option<Time> {
    let t = e.next_wakeup()?;
    let before = gpu.draws.len();
    e.advance(t, gpu);
    let frames: Vec<FrameId> = gpu.draws[before..].iter().filter_map(|d| d.frame).collect();
    for f in frames {
        e.handle(Msg::GpuDone { window: W, frame: f, at: t + Span::from_millis(1) }, t + Span::from_millis(1), gpu);
    }
    Some(t)
}

/// Runs frames until the engine is idle (bounded).
fn settle(e: &mut Engine, gpu: &mut FakeGpu) -> Time {
    let mut last = T0;
    for _ in 0..200 {
        match step(e, gpu) {
            Some(t) => last = t,
            None => break,
        }
    }
    last
}

fn begin_frames(sent: &Sent) -> usize {
    sent.lock().unwrap().iter().filter(|m| matches!(m, ToUi::BeginFrame { .. })).count()
}

#[test]
fn orbiting_a_viewport_on_an_os_layer_never_redraws_the_ui() {
    let (mut e, sent) = engine();
    let mut gpu = FakeGpu::default();
    e.insert_window(W, DeviceSize { w: 400, h: 300 }, Scale(1.0));
    e.insert_os_layer(W, SID, DeviceSize { w: 200, h: 200 });
    e.handle(Msg::Commit(Box::new(commit(viewport_layers(Presentation::OsLayer(SID))))), T0, &mut gpu);
    let mut now = settle(&mut e, &mut gpu);
    assert!(gpu.draws.iter().any(|d| d.key == SurfaceKey::Main));
    assert!(gpu.draws.iter().any(|d| d.key == SurfaceKey::Os(SID)));

    let (draws, commits, begins) = (gpu.draws.len(), e.stats.commits_applied, begin_frames(&sent));
    // 120 frames of orbit: the producer renders a new image each frame and says so.
    for _ in 0..120 {
        now = now + Span::from_millis(1);
        e.handle(Msg::TextureUpdated { window: W, texture: TEX }, now, &mut gpu);
        now = settle(&mut e, &mut gpu);
    }
    let orbit = &gpu.draws[draws..];
    assert_eq!(orbit.len(), 120, "one viewport frame per image");
    assert!(orbit.iter().all(|d| d.key == SurfaceKey::Os(SID)), "only the OS layer's surface redraws");
    assert_eq!(e.stats.commits_applied, commits, "no UI commit");
    assert_eq!(begin_frames(&sent), begins, "the UI thread is never asked for a frame");
}

#[test]
fn in_the_texture_fallback_only_the_viewport_area_of_the_window_redraws() {
    let (mut e, sent) = engine();
    let mut gpu = FakeGpu::default();
    e.insert_window(W, DeviceSize { w: 400, h: 300 }, Scale(1.0));
    e.handle(Msg::Commit(Box::new(commit(viewport_layers(Presentation::Texture)))), T0, &mut gpu);
    let mut now = settle(&mut e, &mut gpu);
    let (draws, commits, begins) = (gpu.draws.len(), e.stats.commits_applied, begin_frames(&sent));
    for _ in 0..30 {
        now = now + Span::from_millis(1);
        e.handle(Msg::TextureUpdated { window: W, texture: TEX }, now, &mut gpu);
        now = settle(&mut e, &mut gpu);
    }
    let orbit = &gpu.draws[draws..];
    assert_eq!(orbit.len(), 30);
    for d in orbit {
        assert_eq!(d.key, SurfaceKey::Main);
        assert_eq!(d.damage.len(), 1, "{:?}", d.damage);
        let r = d.damage[0];
        // The viewport's rect plus the one-pixel antialiasing margin: never the toolbar or the panel.
        assert!(r.x >= 98 && r.y >= 48 && r.x + r.w as i32 <= 302 && r.y + r.h as i32 <= 252, "{r:?}");
    }
    assert_eq!(e.stats.commits_applied, commits);
    assert_eq!(begin_frames(&sent), begins);
}

#[test]
fn the_os_layer_resize_handshake_draws_the_new_size_before_answering() {
    let (mut e, _) = engine();
    let mut gpu = FakeGpu::default();
    e.insert_window(W, DeviceSize { w: 400, h: 300 }, Scale(1.0));
    e.insert_os_layer(W, SID, DeviceSize { w: 200, h: 200 });
    e.handle(Msg::Commit(Box::new(commit(viewport_layers(Presentation::OsLayer(SID))))), T0, &mut gpu);
    let now = settle(&mut e, &mut gpu);
    let before = gpu.draws.len();
    let (tx, rx) = std::sync::mpsc::channel();
    let size = DeviceSize { w: 260, h: 180 };
    e.handle(Msg::ResizeOsLayer { window: W, surface_id: SID, size, done: Some(tx) }, now + Span::from_millis(1), &mut gpu);
    assert_eq!(gpu.resized.last(), Some(&(SurfaceKey::Os(SID), size)));
    let drawn = &gpu.draws[before..];
    assert!(drawn.iter().any(|d| d.key == SurfaceKey::Os(SID) && d.damage.is_empty()), "a full new-size frame: {drawn:?}");
    assert!(rx.try_recv().is_ok(), "answered after drawing");
}

#[test]
fn panning_a_canvas_is_a_camera_property_with_no_commit() {
    let (mut e, sent) = engine();
    let mut gpu = FakeGpu::default();
    e.insert_window(W, DeviceSize { w: 400, h: 300 }, Scale(1.0));
    let canvas = LayerDesc {
        id: LayerId(20),
        parent: Some(ROOT),
        kind: LayerKind::Canvas(CANVAS),
        transform: T_ROOT,
        clip: None,
        effect: None,
        z: 1,
        bounds: VIEWPORT,
        opaque: true,
    };
    e.handle(Msg::Commit(Box::new(commit(vec![canvas]))), T0, &mut gpu);
    let mut now = settle(&mut e, &mut gpu);
    let (draws, commits, begins) = (gpu.draws.len(), e.stats.commits_applied, begin_frames(&sent));
    for i in 1..=20 {
        now = now + Span::from_millis(1);
        let camera = Camera::new(DVec2::new(i as f64 * 1.0e5, 0.0), 1.0e-4);
        e.handle(Msg::CanvasCamera { window: W, canvas: CANVAS, camera, lod: None, input_seq: InputSeq(100 + i) }, now, &mut gpu);
        now = settle(&mut e, &mut gpu);
        let last = gpu.views.iter().rev().find(|(id, _)| *id == CANVAS).expect("the renderer got the view");
        assert_eq!(last.1.camera, camera);
    }
    let pan = &gpu.draws[draws..];
    assert_eq!(pan.len(), 20, "one frame per camera change");
    for d in pan {
        let r = d.damage[0];
        assert!(d.damage.len() == 1 && r.x >= 98 && r.x + r.w as i32 <= 302, "the canvas only: {:?}", d.damage);
    }
    // A fast-commit view transform (a pinch in progress) redraws it too.
    let before = gpu.draws.len();
    e.handle(Msg::Fast(FastCommit::CanvasView { window: W, canvas: CANVAS, view: Affine::scale(1.5, 1.5), input_seq: InputSeq(200) }), now, &mut gpu);
    settle(&mut e, &mut gpu);
    assert_eq!(gpu.draws.len(), before + 1);
    assert_eq!(gpu.views.last().unwrap().1.view, Affine::scale(1.5, 1.5));
    assert_eq!(e.stats.commits_applied, commits, "no UI commit");
    assert_eq!(begin_frames(&sent), begins);
}
