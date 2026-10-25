//! The compositor engine driven with a virtual clock and a recording backend, with the real frame
//! clock (presto-frame) and animation math (presto-anim): commit folding, fast commits, damage per
//! frame, the frame loop of a fling with no commits, animations, resize, visibility.

use presto_base::anim::{
    AnimProperty, AnimValue, AnimationCmd, AnimationEvent, AnimationSpec, Easing, FillMode, FlingCurve, Generator,
    Keyframe, PlaybackDirection, StartTime, Timing,
};
use presto_base::color::{Color, Fill};
use presto_base::commit::{Commit, FastCommit, ToUi};
use presto_base::display::{DisplayList, Prim, Quad};
use presto_base::geom::{Affine, Corners, DeviceRect, DeviceSize, Insets, Rect, Scale, Size, Vec2};
use presto_base::ids::{AnimationId, FrameId, InputSeq, LayerId, NodeId, PropNodeId, Revision, TextureId, WindowId};
use presto_base::layer::{ItemRef, LayerDelta, LayerDesc, LayerKind};
use presto_base::props::{EffectNode, OverscrollBehavior, PropOp, ScrollNode, TransformNode};
use presto_base::scene::FrameScene;
use presto_base::time::{Span, Time};
use presto_compositor::core::{Backend, DrawOutcome, Engine, EngineConfig, Msg};
use presto_compositor::seams::{self, ScrollMotion};
use presto_compositor::tree::SurfaceKey;
use std::sync::{Arc, Mutex};

const W: WindowId = WindowId(1);
const ROOT: LayerId = LayerId(1);
const T_ROOT: PropNodeId = PropNodeId(1);
const T_CONTENT: PropNodeId = PropNodeId(2);
const SCROLL: PropNodeId = PropNodeId(3);
const EFFECT: PropNodeId = PropNodeId(4);
const A: NodeId = NodeId::new(1, 1);
const B: NodeId = NodeId::new(2, 1);
const INTERVAL: Span = Span::from_micros(16_667);
/// 1 ms after a tick of the timer's vsync grid, so test events land well before the next tick.
const T0: Time = Time(INTERVAL.0 as u64 * 60 + 1_000_000);

#[derive(Debug, Clone)]
struct Drawn {
    frame: Option<FrameId>,
    damage: Vec<DeviceRect>,
    items: usize,
}

#[derive(Default)]
struct FakeGpu {
    draws: Vec<Drawn>,
}

impl Backend for FakeGpu {
    fn attach(&mut self, _: WindowId, _: SurfaceKey, _: wgpu::Surface<'static>, _: DeviceSize) {}
    fn detach(&mut self, _: WindowId, _: SurfaceKey) {}
    fn resize(&mut self, _: WindowId, _: SurfaceKey, _: DeviceSize) {}
    fn register_texture(&mut self, _: TextureId, _: wgpu::Texture) {}
    fn unregister_texture(&mut self, _: TextureId) {}
    fn draw(&mut self, _: WindowId, _: SurfaceKey, frame: Option<FrameId>, scene: &FrameScene<'_>) -> DrawOutcome {
        self.draws.push(Drawn { frame, damage: scene.damage.to_vec(), items: scene.items.len() });
        DrawOutcome::Presented { submitted: Time(0) }
    }
    fn forget(&mut self, _: &[presto_base::ids::ImageId], _: &[presto_base::ids::PathId]) {}
    fn recover(&mut self) -> bool {
        true
    }
}

type Sent = Arc<Mutex<Vec<ToUi>>>;

fn engine(fling: bool) -> (Engine, Sent) {
    let sent: Sent = Arc::default();
    let s = sent.clone();
    let fling: Option<presto_compositor::seams::FlingFactory> = fling.then(|| {
        Box::new(|_c: FlingCurve, v: Vec2, from: Vec2, at: Time| Box::new(LinearFling { from, v, at }) as Box<dyn ScrollMotion>)
            as presto_compositor::seams::FlingFactory
    });
    let e = Engine::new(
        EngineConfig { refresh: INTERVAL, clear: Some(Color::WHITE), caret_color: Color::BLACK, caret_blink: None },
        Box::new(seams::frame_clock),
        fling,
        None,
        Box::new(move |m| s.lock().unwrap().push(m)),
        Box::new(|| {}),
    );
    (e, sent)
}

/// Decelerates linearly to rest over 300 ms.
struct LinearFling {
    from: Vec2,
    v: Vec2,
    at: Time,
}

impl ScrollMotion for LinearFling {
    fn offset_at(&self, t: Time) -> Vec2 {
        let s = ((t - self.at).as_secs_f64() as f32).clamp(0.0, 0.3);
        let k = s - s * s / 0.6;
        self.from + self.v * k
    }
    fn done_at(&self, t: Time) -> bool {
        (t - self.at).as_secs_f64() >= 0.3
    }
}

fn quad(r: Rect, c: Color) -> Arc<DisplayList> {
    let mut l = DisplayList::new();
    l.push(
        Prim::Quad(Quad {
            rect: r,
            radii: Corners::ZERO,
            fill: Fill::Solid(c),
            border_widths: Insets::ZERO,
            border_color: Color::TRANSPARENT,
            border_style: Default::default(),
        }),
        r,
    );
    Arc::new(l)
}

fn layer() -> LayerDesc {
    LayerDesc {
        id: ROOT,
        parent: None,
        kind: LayerKind::Root,
        transform: T_ROOT,
        clip: None,
        effect: None,
        z: 0,
        bounds: Rect::new(0.0, 0.0, 400.0, 300.0),
        opaque: true,
    }
}

/// A root layer with a scroll container: node A inside the scrolled content, node B outside.
fn base_commit(revision: u64) -> Commit {
    let mut c = Commit { window: W, revision: Revision(revision), input_seq: InputSeq(revision), ..Default::default() };
    c.props.ops = vec![
        PropOp::Transform(T_ROOT, TransformNode { parent: None, local: Affine::IDENTITY, scroll: None }),
        PropOp::Scroll(
            SCROLL,
            ScrollNode {
                space: T_ROOT,
                offset: Vec2::ZERO,
                content: Size::new(400.0, 3000.0),
                viewport: Rect::new(0.0, 0.0, 400.0, 300.0),
                scroll_x: false,
                scroll_y: true,
                overscroll: OverscrollBehavior::None,
                passive: true,
            },
        ),
        PropOp::Transform(T_CONTENT, TransformNode { parent: Some(T_ROOT), local: Affine::IDENTITY, scroll: Some(SCROLL) }),
        PropOp::Effect(EFFECT, EffectNode { parent: None, opacity: 1.0, blend: Default::default() }),
    ];
    c.layers = vec![
        LayerDelta::Add(layer()),
        LayerDelta::Items {
            layer: ROOT,
            items: vec![
                ItemRef { node: A, transform: T_CONTENT, clip: None, effect: Some(EFFECT) },
                ItemRef { node: B, transform: T_ROOT, clip: None, effect: None },
            ],
        },
    ];
    c.lists = vec![
        (A, quad(Rect::new(10.0, 10.0, 110.0, 60.0), Color::BLACK)),
        (B, quad(Rect::new(200.0, 200.0, 250.0, 250.0), Color::BLACK)),
    ];
    c
}

fn new_window(e: &mut Engine) {
    e.insert_window(W, DeviceSize { w: 400, h: 300 }, Scale(1.0));
}

/// Advances to the next wakeup, then completes the GPU work of every frame drawn there 1 ms later.
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

fn complete(e: &mut Engine, gpu: &mut FakeGpu, now: Time) {
    let frames: Vec<FrameId> = gpu.draws.iter().filter_map(|d| d.frame).collect();
    if let Some(f) = frames.last() {
        e.handle(Msg::GpuDone { window: W, frame: *f, at: now }, now, gpu);
    }
}

#[test]
fn first_commit_draws_the_whole_window_and_acks() {
    let (mut e, sent) = engine(false);
    let mut gpu = FakeGpu::default();
    let t0 = T0;
    new_window(&mut e);
    e.handle(Msg::Commit(Box::new(base_commit(1))), t0, &mut gpu);
    assert_eq!(gpu.draws.len(), 1, "an urgent commit draws at once");
    assert!(gpu.draws[0].damage.is_empty(), "the first frame is a full redraw");
    assert_eq!(gpu.draws[0].items, 2);
    assert!(sent.lock().unwrap().contains(&ToUi::CommitApplied { window: W, revision: Revision(1) }));
}

#[test]
fn commits_arriving_before_a_draw_fold_into_one_frame() {
    let (mut e, sent) = engine(false);
    let mut gpu = FakeGpu::default();
    let t0 = T0;
    new_window(&mut e);
    e.handle(Msg::Commit(Box::new(base_commit(1))), t0, &mut gpu);
    assert_eq!(gpu.draws.len(), 1);

    // Frame 1 is in flight: these two wait and are applied together, newer over older.
    let mut c2 = Commit { window: W, revision: Revision(2), input_seq: InputSeq(2), ..Default::default() };
    c2.lists = vec![(A, quad(Rect::new(10.0, 10.0, 110.0, 60.0), Color::WHITE))];
    c2.props.ops = vec![PropOp::Effect(EFFECT, EffectNode { parent: None, opacity: 0.5, blend: Default::default() })];
    let mut c3 = Commit { window: W, revision: Revision(3), input_seq: InputSeq(3), ..Default::default() };
    c3.props.ops = vec![PropOp::Effect(EFFECT, EffectNode { parent: None, opacity: 0.25, blend: Default::default() })];
    e.handle(Msg::Commit(Box::new(c2)), t0, &mut gpu);
    e.handle(Msg::Commit(Box::new(c3)), t0, &mut gpu);
    assert_eq!(gpu.draws.len(), 1, "no second frame while one is in flight");

    complete(&mut e, &mut gpu, t0 + Span::from_millis(2));
    while gpu.draws.len() < 2 {
        step(&mut e, &mut gpu).expect("the clock stays armed while commits wait");
    }
    let tree = e.tree(W).unwrap();
    assert_eq!(tree.revision, Revision(3));
    assert_eq!(tree.props.effects[&EFFECT].opacity, 0.25, "the newer commit wins");
    let Prim::Quad(q) = &tree.lists[&A].items[0] else { panic!() };
    assert_eq!(q.fill, Fill::Solid(Color::WHITE), "the older commit's list is kept");
    let acks: Vec<_> = sent.lock().unwrap().iter().filter(|m| matches!(m, ToUi::CommitApplied { .. })).cloned().collect();
    assert_eq!(
        acks,
        vec![
            ToUi::CommitApplied { window: W, revision: Revision(1) },
            ToUi::CommitApplied { window: W, revision: Revision(3) }
        ],
        "one ack per applied batch, for its newest revision"
    );
    // Only node A changed: damage is its rect, not the window.
    let d = &gpu.draws[1].damage;
    assert_eq!(d.len(), 1);
    assert!(d[0].w < 200 && d[0].h < 100, "{d:?}");
}

#[test]
fn stale_fast_commits_are_rejected_and_win_over_older_commits() {
    let (mut e, _) = engine(false);
    let mut gpu = FakeGpu::default();
    let t0 = T0;
    new_window(&mut e);
    e.handle(Msg::Commit(Box::new(base_commit(5))), t0, &mut gpu);
    let fast = |rev: u64, y: f32| {
        Msg::Fast(FastCommit::ScrollOffset {
            window: W,
            node: SCROLL,
            offset: Vec2::new(0.0, y),
            revision: Revision(rev),
            input_seq: InputSeq(rev),
        })
    };
    e.handle(fast(5, 99.0), t0, &mut gpu);
    assert_eq!(e.tree(W).unwrap().props.scrolls[&SCROLL].offset.y, 0.0, "not newer than the applied commit");
    assert_eq!(e.stats.fast_rejected, 1);
    e.handle(fast(6, 30.0), t0, &mut gpu);
    assert_eq!(e.tree(W).unwrap().props.scrolls[&SCROLL].offset.y, 30.0);

    // A JS commit built before the scroll (revision 5) lands late: the offset stays the fast one.
    complete(&mut e, &mut gpu, t0 + Span::from_millis(1));
    e.handle(Msg::Commit(Box::new(base_commit(5))), t0 + Span::from_millis(2), &mut gpu);
    let mut last = t0;
    while gpu.draws.len() < 2 {
        last = step(&mut e, &mut gpu).expect("a tick for the waiting commit");
    }
    assert_eq!(e.tree(W).unwrap().props.scrolls[&SCROLL].offset.y, 30.0);

    // Once a commit has seen the scroll, the tree's value applies again.
    let mut c7 = base_commit(7);
    let PropOp::Scroll(_, s) = &mut c7.props.ops[1] else { panic!() };
    s.offset = Vec2::new(0.0, 50.0);
    e.handle(Msg::Commit(Box::new(c7)), last + Span::from_millis(2), &mut gpu);
    for _ in 0..10 {
        if e.tree(W).unwrap().revision == Revision(7) {
            break;
        }
        step(&mut e, &mut gpu).expect("a tick for the waiting commit");
    }
    assert_eq!(e.tree(W).unwrap().props.scrolls[&SCROLL].offset.y, 50.0);
}

#[test]
fn a_fling_keeps_producing_frames_without_commits_then_goes_idle() {
    let (mut e, sent) = engine(true);
    let mut gpu = FakeGpu::default();
    let t0 = T0;
    new_window(&mut e);
    e.handle(Msg::Commit(Box::new(base_commit(1))), t0, &mut gpu);
    complete(&mut e, &mut gpu, t0 + Span::from_millis(1));
    let at = t0 + Span::from_millis(2);
    e.handle(
        Msg::Fling { window: W, node: SCROLL, velocity: Vec2::new(0.0, 2000.0), curve: FlingCurve::Chromium, at },
        at,
        &mut gpu,
    );
    let drawn_before = gpu.draws.len();
    let mut last = at;
    for _ in 0..200 {
        match step(&mut e, &mut gpu) {
            Some(t) => last = t,
            None => break,
        }
    }
    let frames = gpu.draws.len() - drawn_before;
    // 300 ms at 60 Hz: about 18 frames, every one from the compositor alone.
    assert!(frames >= 15, "only {frames} fling frames");
    assert!(e.next_wakeup().is_none(), "idle after the fling: no wakeups");
    assert!(last - at < Span::from_millis(400));
    let moved: Vec<f32> = sent
        .lock()
        .unwrap()
        .iter()
        .filter_map(|m| match m {
            ToUi::ScrollMoved { node, offset, .. } if *node == SCROLL => Some(offset.y),
            _ => None,
        })
        .collect();
    assert!(moved.len() >= 15);
    assert!(moved.windows(2).all(|w| w[1] >= w[0]), "monotonic: {moved:?}");
    assert!((moved.last().unwrap() - 300.0).abs() < 1.0, "rests at v·t/2 = 300: {moved:?}");
    assert_eq!(e.tree(W).unwrap().revision, Revision(1), "no commits during the fling");
    let presented = sent.lock().unwrap().iter().filter(|m| matches!(m, ToUi::Presented { estimated: true, .. })).count();
    assert!(presented >= frames, "every frame reports an estimated present");
}

#[test]
fn a_scroll_fast_commit_stops_a_fling() {
    let (mut e, _) = engine(true);
    let mut gpu = FakeGpu::default();
    let t0 = T0;
    new_window(&mut e);
    e.handle(Msg::Commit(Box::new(base_commit(1))), t0, &mut gpu);
    complete(&mut e, &mut gpu, t0);
    e.handle(
        Msg::Fling { window: W, node: SCROLL, velocity: Vec2::new(0.0, 2000.0), curve: FlingCurve::Chromium, at: t0 },
        t0,
        &mut gpu,
    );
    for _ in 0..4 {
        step(&mut e, &mut gpu);
    }
    let fast = FastCommit::ScrollOffset {
        window: W,
        node: SCROLL,
        offset: Vec2::new(0.0, 5.0),
        revision: Revision(2),
        input_seq: InputSeq(2),
    };
    let now = e.next_wakeup().unwrap();
    e.handle(Msg::Fast(fast), now, &mut gpu);
    for _ in 0..50 {
        if step(&mut e, &mut gpu).is_none() {
            break;
        }
    }
    assert_eq!(e.tree(W).unwrap().props.scrolls[&SCROLL].offset.y, 5.0);
}

#[test]
fn idle_window_does_not_wake() {
    let (mut e, _) = engine(false);
    let mut gpu = FakeGpu::default();
    let t0 = T0;
    new_window(&mut e);
    e.handle(Msg::Commit(Box::new(base_commit(1))), t0, &mut gpu);
    complete(&mut e, &mut gpu, t0 + Span::from_millis(1));
    for _ in 0..5 {
        if step(&mut e, &mut gpu).is_none() {
            break;
        }
    }
    assert!(e.next_wakeup().is_none());
    assert_eq!(gpu.draws.len(), 1, "nothing changed: no second frame");
}

#[test]
fn begin_frame_follows_a_request_and_waits_for_the_commit() {
    let (mut e, sent) = engine(false);
    let mut gpu = FakeGpu::default();
    let t0 = T0;
    new_window(&mut e);
    e.handle(Msg::Commit(Box::new(base_commit(1))), t0, &mut gpu);
    complete(&mut e, &mut gpu, t0);
    e.handle(Msg::NeedsUiFrame { window: W, on: true }, t0, &mut gpu);
    for _ in 0..3 {
        step(&mut e, &mut gpu);
    }
    let begins = sent.lock().unwrap().iter().filter(|m| matches!(m, ToUi::BeginFrame { .. })).count();
    assert_eq!(begins, 1, "one BeginFrame until its commit arrives (back-pressure)");
}

#[test]
fn compositor_opacity_animation_runs_and_holds_with_fill_forwards() {
    let (mut e, sent) = engine(false);
    let mut gpu = FakeGpu::default();
    let t0 = T0;
    new_window(&mut e);
    let mut c = base_commit(1);
    c.animations = vec![AnimationCmd::Start(AnimationSpec {
        id: AnimationId(9),
        node: A,
        target: Some(EFFECT),
        property: AnimProperty::Opacity,
        generator: Generator::Keyframes {
            keyframes: vec![
                Keyframe { offset: 0.0, value: AnimValue::Number(0.0), easing: None },
                Keyframe { offset: 1.0, value: AnimValue::Number(1.0), easing: None },
            ],
            timing: Timing {
                delay: Span::ZERO,
                end_delay: Span::ZERO,
                duration: Span::from_millis(100),
                iterations: 1.0,
                iteration_start: 0.0,
                direction: PlaybackDirection::Normal,
                fill: FillMode::Forwards,
                easing: Easing::Linear,
            },
        },
        start: StartTime::NextFrame,
    })];
    e.handle(Msg::Commit(Box::new(c)), t0, &mut gpu);
    complete(&mut e, &mut gpu, t0);
    for _ in 0..100 {
        if step(&mut e, &mut gpu).is_none() {
            break;
        }
    }
    let events: Vec<AnimationEvent> = sent
        .lock()
        .unwrap()
        .iter()
        .filter_map(|m| match m {
            ToUi::Animation { event, .. } => Some(*event),
            _ => None,
        })
        .collect();
    assert!(matches!(events[0], AnimationEvent::Started { id: AnimationId(9), .. }));
    assert!(matches!(events[1], AnimationEvent::Finished { id: AnimationId(9), .. }));
    assert!(gpu.draws.len() >= 6, "about 6 frames over 100 ms");
    assert_eq!(e.tree(W).unwrap().props.anim_opacity[&EFFECT], 1.0, "fill: forwards holds the end value");
    assert!(e.next_wakeup().is_none());
}

#[test]
fn ui_thread_animations_are_not_run_here() {
    let (mut e, sent) = engine(false);
    let mut gpu = FakeGpu::default();
    let t0 = T0;
    new_window(&mut e);
    let mut c = base_commit(1);
    c.animations = vec![AnimationCmd::Start(AnimationSpec {
        id: AnimationId(3),
        node: A,
        target: Some(EFFECT),
        property: AnimProperty::BackgroundColor,
        generator: Generator::Spring {
            params: presto_base::anim::SpringParams::DurationBounce { duration: Span::from_millis(300), bounce: 0.0 },
            from: AnimValue::Number(0.0),
            to: AnimValue::Number(1.0),
            velocity: 0.0,
        },
        start: StartTime::NextFrame,
    })];
    e.handle(Msg::Commit(Box::new(c)), t0, &mut gpu);
    assert!(sent
        .lock()
        .unwrap()
        .contains(&ToUi::Animation { window: W, event: AnimationEvent::Aborted { id: AnimationId(3) } }));
}

#[test]
fn resize_draws_the_new_size_and_answers() {
    let (mut e, sent) = engine(false);
    let mut gpu = FakeGpu::default();
    let t0 = T0;
    new_window(&mut e);
    e.handle(Msg::Commit(Box::new(base_commit(1))), t0, &mut gpu);
    complete(&mut e, &mut gpu, t0);
    let size = DeviceSize { w: 800, h: 600 };
    e.handle(Msg::Resize { window: W, size, scale: Scale(2.0), seq: 7 }, t0, &mut gpu);
    let last = gpu.draws.last().unwrap();
    assert!(last.frame.is_none() && last.damage.is_empty(), "a full, unpaced frame");
    assert_eq!(sent.lock().unwrap().last(), Some(&ToUi::ResizeDone { window: W, seq: 7, size, scale: Scale(2.0) }));
}

#[test]
fn hidden_windows_do_not_draw_and_redraw_fully_when_shown() {
    let (mut e, _) = engine(false);
    let mut gpu = FakeGpu::default();
    let t0 = T0;
    new_window(&mut e);
    e.handle(Msg::Commit(Box::new(base_commit(1))), t0, &mut gpu);
    complete(&mut e, &mut gpu, t0);
    e.handle(Msg::SetVisible { window: W, visible: false }, t0, &mut gpu);
    e.handle(Msg::Commit(Box::new(base_commit(2))), t0, &mut gpu);
    for _ in 0..5 {
        step(&mut e, &mut gpu);
    }
    assert_eq!(gpu.draws.len(), 1);
    e.handle(Msg::SetVisible { window: W, visible: true }, t0 + Span::from_millis(100), &mut gpu);
    for _ in 0..5 {
        if gpu.draws.len() > 1 {
            break;
        }
        step(&mut e, &mut gpu);
    }
    assert_eq!(gpu.draws.len(), 2);
    assert!(gpu.draws[1].damage.is_empty(), "full redraw after being hidden");
}
