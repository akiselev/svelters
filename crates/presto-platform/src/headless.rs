//! A platform without an OS, for tests: a scripted list of timed input and window events, a virtual
//! clock that jumps from one event (or timer deadline) to the next, windows that are only records,
//! and a log of every call the app made.

use crate::api::{
    A11yEvent, AppHandler, HitRegion, ImeState, Monitor, OsLayerCommitter, OsLayerDesc, PlatformCx, SurfaceTarget, WindowDesc,
    Waker,
};
use crate::clipboard::{Clipboard, MemoryClipboard};
use presto_base::accesskit;
use presto_base::cursor::CursorIcon;
use presto_base::event::{InputEnvelope, InputEvent, WindowEvent};
use presto_base::geom::{DeviceSize, Rect, Scale};
use presto_base::ids::{InputSeq, SurfaceId, WindowId};
use presto_base::time::{Clock, Span, Time};
use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

/// A clock tests move by hand.
#[derive(Debug, Default)]
pub struct VirtualClock(AtomicU64);

impl VirtualClock {
    pub fn new(start: Time) -> Arc<Self> {
        Arc::new(VirtualClock(AtomicU64::new(start.0)))
    }
    /// Moves to `t`; never backwards.
    pub fn set(&self, t: Time) {
        self.0.fetch_max(t.0, Ordering::SeqCst);
    }
    pub fn advance(&self, s: Span) {
        let now = self.now();
        self.set(now + s);
    }
}

impl Clock for VirtualClock {
    fn now(&self) -> Time {
        Time(self.0.load(Ordering::SeqCst))
    }
}

/// One scripted event.
#[derive(Clone, Debug, PartialEq)]
pub enum Scripted {
    Input { window: WindowId, event: InputEvent },
    Window { window: WindowId, event: WindowEvent },
    /// A wake from "another thread".
    Wake,
    A11y { window: WindowId, event: A11yEvent },
    /// The user asked to quit (Cmd-Q).
    QuitRequested,
}

/// A call the app made, in order.
#[derive(Clone, Debug, PartialEq)]
pub enum PlatformCall {
    CreateWindow(WindowId, WindowDesc),
    CloseWindow(WindowId),
    CreateOsLayer(WindowId, SurfaceId, OsLayerDesc),
    SetOsLayerBounds(WindowId, SurfaceId, Rect),
    RemoveOsLayer(WindowId, SurfaceId),
    SetCursor(WindowId, CursorIcon),
    SetIme(WindowId, Option<ImeState>),
    EndImeComposition(WindowId, bool),
    SetTimer(Option<Time>),
    SetHitTestRegions(WindowId, Vec<HitRegion>),
    SetMouseCapture(WindowId, bool),
    SetTitle(WindowId, String),
    SetVisible(WindowId, bool),
    DragWindow(WindowId),
    UpdateA11y(WindowId, accesskit::TreeUpdate),
    Exit,
}

/// A window that exists only as a record.
#[derive(Clone, Debug, PartialEq)]
pub struct HeadlessWindow {
    pub desc: WindowDesc,
    pub scale: Scale,
    pub size: DeviceSize,
    pub layers: Vec<SurfaceId>,
}

/// The state the app reaches through [`PlatformCx`].
pub struct HeadlessCx {
    clock: Arc<VirtualClock>,
    windows: BTreeMap<WindowId, HeadlessWindow>,
    next_window: u32,
    next_surface: u32,
    timer: Option<Time>,
    woken: Arc<AtomicBool>,
    clipboard: MemoryClipboard,
    monitors: Vec<Monitor>,
    calls: Vec<PlatformCall>,
    exit: bool,
    /// The scale new windows get.
    pub default_scale: Scale,
}

/// The headless platform: script events, then [`HeadlessPlatform::run`] a handler through them.
pub struct HeadlessPlatform {
    cx: HeadlessCx,
    script: VecDeque<(Time, Scripted)>,
    next_seq: u64,
    started: bool,
}

impl Default for HeadlessPlatform {
    fn default() -> Self {
        Self::new()
    }
}

impl HeadlessPlatform {
    pub fn new() -> Self {
        HeadlessPlatform {
            cx: HeadlessCx {
                clock: VirtualClock::new(Time::ZERO),
                windows: BTreeMap::new(),
                next_window: 1,
                next_surface: 1,
                timer: None,
                woken: Arc::new(AtomicBool::new(false)),
                clipboard: MemoryClipboard::default(),
                monitors: vec![Monitor {
                    name: Some("headless".into()),
                    position: (0, 0),
                    size: DeviceSize { w: 1920, h: 1080 },
                    work_area: None,
                    scale: Scale(1.0),
                    refresh_millihertz: Some(60_000),
                }],
                calls: Vec::new(),
                exit: false,
                default_scale: Scale(1.0),
            },
            script: VecDeque::new(),
            next_seq: 1,
            started: false,
        }
    }

    pub fn virtual_clock(&self) -> Arc<VirtualClock> {
        self.cx.clock.clone()
    }

    pub fn set_monitors(&mut self, monitors: Vec<Monitor>) {
        self.cx.monitors = monitors;
    }

    /// Queues an event at `at` (events at the same time keep their order).
    pub fn push(&mut self, at: Time, ev: Scripted) {
        let i = self.script.partition_point(|(t, _)| *t <= at);
        self.script.insert(i, (at, ev));
    }

    pub fn push_input(&mut self, at: Time, window: WindowId, event: InputEvent) {
        self.push(at, Scripted::Input { window, event });
    }

    pub fn push_window_event(&mut self, at: Time, window: WindowId, event: WindowEvent) {
        self.push(at, Scripted::Window { window, event });
    }

    /// Runs until the script is empty and no timer or wake is pending, or the app exits.
    pub fn run(&mut self, handler: &mut dyn AppHandler) {
        self.run_until(handler, None)
    }

    /// Runs events and timers up to and including `until` (all of them when `None`); the clock ends
    /// at `until` when given.
    pub fn run_until(&mut self, handler: &mut dyn AppHandler, until: Option<Time>) {
        if !self.started {
            self.started = true;
            handler.init(&mut self.cx);
        }
        while !self.cx.exit {
            if self.cx.woken.swap(false, Ordering::SeqCst) {
                handler.wake(&mut self.cx);
                continue;
            }
            let next_event = self.script.front().map(|(t, _)| *t);
            let timer = self.cx.timer;
            let (at, is_timer) = match (next_event, timer) {
                (Some(e), Some(t)) if t <= e => (t, true),
                (Some(e), _) => (e, false),
                (None, Some(t)) => (t, true),
                (None, None) => break,
            };
            if until.is_some_and(|u| at > u) {
                break;
            }
            self.cx.clock.set(at);
            if is_timer {
                self.cx.timer = None;
                handler.timer(&mut self.cx, at);
                continue;
            }
            let (_, ev) = self.script.pop_front().expect("peeked");
            self.dispatch(handler, at, ev);
        }
        if let Some(u) = until {
            self.cx.clock.set(u);
        }
        if self.cx.exit {
            handler.exiting(&mut self.cx);
        }
    }

    fn dispatch(&mut self, handler: &mut dyn AppHandler, at: Time, ev: Scripted) {
        match ev {
            Scripted::Input { window, event } => {
                let seq = InputSeq(self.next_seq);
                self.next_seq += 1;
                handler.input(&mut self.cx, InputEnvelope { seq, window, os_time: at, recv_time: at, event });
            }
            Scripted::Window { window, event } => {
                if let Some(w) = self.cx.windows.get_mut(&window) {
                    match &event {
                        WindowEvent::Resized { size, scale } => {
                            w.size = *size;
                            w.scale = *scale;
                        }
                        WindowEvent::ScaleChanged { scale } => w.scale = *scale,
                        _ => {}
                    }
                }
                handler.window_event(&mut self.cx, window, event)
            }
            Scripted::Wake => handler.wake(&mut self.cx),
            Scripted::A11y { window, event } => handler.a11y_event(&mut self.cx, window, event),
            Scripted::QuitRequested => {
                if handler.quit_requested(&mut self.cx) {
                    self.cx.exit = true;
                }
            }
        }
    }

    pub fn calls(&self) -> &[PlatformCall] {
        &self.cx.calls
    }

    pub fn take_calls(&mut self) -> Vec<PlatformCall> {
        std::mem::take(&mut self.cx.calls)
    }

    pub fn windows(&self) -> &BTreeMap<WindowId, HeadlessWindow> {
        &self.cx.windows
    }

    pub fn timer(&self) -> Option<Time> {
        self.cx.timer
    }

    pub fn exited(&self) -> bool {
        self.cx.exit
    }

    /// The platform side of the context, for driving a handler by hand.
    pub fn cx(&mut self) -> &mut HeadlessCx {
        &mut self.cx
    }
}

impl PlatformCx for HeadlessCx {
    fn create_window(&mut self, desc: WindowDesc) -> WindowId {
        let id = WindowId(self.next_window);
        self.next_window += 1;
        let scale = self.default_scale;
        let size = DeviceSize { w: (desc.size.w * scale.0).round() as u32, h: (desc.size.h * scale.0).round() as u32 };
        self.calls.push(PlatformCall::CreateWindow(id, desc.clone()));
        self.windows.insert(id, HeadlessWindow { desc, scale, size, layers: Vec::new() });
        id
    }

    fn close_window(&mut self, id: WindowId) {
        self.windows.remove(&id);
        self.calls.push(PlatformCall::CloseWindow(id));
    }

    fn surface_target(&self, _id: WindowId) -> SurfaceTarget {
        SurfaceTarget::None
    }

    fn create_os_layer(&mut self, id: WindowId, desc: OsLayerDesc) -> Option<(SurfaceId, SurfaceTarget)> {
        let w = self.windows.get_mut(&id)?;
        let s = SurfaceId(self.next_surface);
        self.next_surface += 1;
        w.layers.push(s);
        self.calls.push(PlatformCall::CreateOsLayer(id, s, desc));
        Some((s, SurfaceTarget::None))
    }

    fn set_os_layer_bounds(&mut self, id: WindowId, layer: SurfaceId, bounds: Rect) {
        self.calls.push(PlatformCall::SetOsLayerBounds(id, layer, bounds));
    }

    fn remove_os_layer(&mut self, id: WindowId, layer: SurfaceId) {
        if let Some(w) = self.windows.get_mut(&id) {
            w.layers.retain(|l| *l != layer);
        }
        self.calls.push(PlatformCall::RemoveOsLayer(id, layer));
    }

    fn os_layer_committer(&self, _id: WindowId) -> Option<OsLayerCommitter> {
        None
    }

    fn set_cursor(&mut self, id: WindowId, c: CursorIcon) {
        self.calls.push(PlatformCall::SetCursor(id, c));
    }

    fn set_ime(&mut self, id: WindowId, state: Option<ImeState>) {
        self.calls.push(PlatformCall::SetIme(id, state));
    }

    fn end_ime_composition(&mut self, id: WindowId, commit: bool) -> bool {
        self.calls.push(PlatformCall::EndImeComposition(id, commit));
        true
    }

    fn clipboard(&mut self) -> &mut dyn Clipboard {
        &mut self.clipboard
    }

    fn set_timer(&mut self, at: Option<Time>) {
        self.timer = at;
        self.calls.push(PlatformCall::SetTimer(at));
    }

    fn waker(&self) -> Waker {
        let woken = self.woken.clone();
        Waker::new(move || woken.store(true, Ordering::SeqCst))
    }

    fn clock(&self) -> Arc<dyn Clock> {
        self.clock.clone()
    }

    fn monitors(&self) -> Vec<Monitor> {
        self.monitors.clone()
    }

    fn set_hit_test_regions(&mut self, id: WindowId, regions: &[HitRegion]) {
        self.calls.push(PlatformCall::SetHitTestRegions(id, regions.to_vec()));
    }

    fn set_mouse_capture(&mut self, id: WindowId, capture: bool) {
        self.calls.push(PlatformCall::SetMouseCapture(id, capture));
    }

    fn set_title(&mut self, id: WindowId, title: &str) {
        if let Some(w) = self.windows.get_mut(&id) {
            w.desc.title = title.to_owned();
        }
        self.calls.push(PlatformCall::SetTitle(id, title.to_owned()));
    }

    fn set_visible(&mut self, id: WindowId, visible: bool) {
        if let Some(w) = self.windows.get_mut(&id) {
            w.desc.visible = visible;
        }
        self.calls.push(PlatformCall::SetVisible(id, visible));
    }

    fn drag_window(&mut self, id: WindowId) {
        self.calls.push(PlatformCall::DragWindow(id));
    }

    fn update_a11y(&mut self, id: WindowId, update: accesskit::TreeUpdate) {
        self.calls.push(PlatformCall::UpdateA11y(id, update));
    }

    fn window_scale(&self, id: WindowId) -> Option<Scale> {
        self.windows.get(&id).map(|w| w.scale)
    }

    fn exit(&mut self) {
        self.exit = true;
        self.calls.push(PlatformCall::Exit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use presto_base::event::{KeyEvent, KeyLocation, KeyState, Modifiers};
    use presto_base::geom::Size;
    use presto_base::key::Key;

    #[derive(Default)]
    struct Recorder {
        window: Option<WindowId>,
        seen: Vec<(Time, String)>,
        waker: Option<Waker>,
        veto_quit: bool,
    }

    impl AppHandler for Recorder {
        fn init(&mut self, cx: &mut dyn PlatformCx) {
            self.window = Some(cx.create_window(WindowDesc::new("main", Size::new(400.0, 300.0))));
            self.waker = Some(cx.waker());
            cx.set_timer(Some(Time(50)));
        }
        fn input(&mut self, cx: &mut dyn PlatformCx, ev: InputEnvelope) {
            self.seen.push((cx.clock().now(), format!("input {} {:?}", ev.seq.0, ev.event)));
            if let InputEvent::Key(k) = &ev.event {
                if k.key == Some(Key::Q) {
                    cx.exit();
                }
                // A key wakes "another thread", which wakes us back.
                if k.key == Some(Key::W) {
                    self.waker.as_ref().unwrap().wake();
                }
            }
        }
        fn window_event(&mut self, cx: &mut dyn PlatformCx, _: WindowId, ev: WindowEvent) {
            self.seen.push((cx.clock().now(), format!("window {ev:?}")));
        }
        fn wake(&mut self, cx: &mut dyn PlatformCx) {
            self.seen.push((cx.clock().now(), "wake".into()));
        }
        fn timer(&mut self, cx: &mut dyn PlatformCx, now: Time) {
            self.seen.push((now, "timer".into()));
            if now == Time(50) {
                cx.set_timer(Some(Time(250)));
            }
        }
        fn quit_requested(&mut self, _: &mut dyn PlatformCx) -> bool {
            !self.veto_quit
        }
    }

    fn key(k: Key) -> InputEvent {
        InputEvent::Key(KeyEvent {
            key: Some(k),
            physical: 0,
            text: None,
            state: KeyState::Pressed,
            repeat: false,
            location: KeyLocation::Standard,
            modifiers: Modifiers::default(),
        })
    }

    #[test]
    fn script_runs_in_time_order_with_timers_between_events() {
        let mut p = HeadlessPlatform::new();
        let w = WindowId(1);
        p.push_input(Time(200), w, key(Key::A));
        p.push_input(Time(100), w, key(Key::B));
        p.push_window_event(Time(100), w, WindowEvent::Focused(true));
        let mut app = Recorder::default();
        p.run(&mut app);
        let times: Vec<_> = app.seen.iter().map(|(t, s)| (t.0, s.split(' ').next().unwrap().to_owned())).collect();
        assert_eq!(
            times,
            vec![
                (50, "timer".into()),
                (100, "input".into()),
                (100, "window".into()),
                (200, "input".into()),
                (250, "timer".into()),
            ]
        );
        assert!(app.seen[1].1.starts_with("input 1 "));
        assert!(app.seen[3].1.starts_with("input 2 "));
        assert_eq!(p.windows().len(), 1);
        assert!(matches!(p.calls()[0], PlatformCall::CreateWindow(WindowId(1), _)));
    }

    #[test]
    fn wakes_coalesce_and_exit_stops_the_loop() {
        let mut p = HeadlessPlatform::new();
        let w = WindowId(1);
        p.push_input(Time(10), w, key(Key::W));
        p.push_input(Time(20), w, key(Key::Q));
        p.push_input(Time(30), w, key(Key::A));
        let mut app = Recorder::default();
        p.run(&mut app);
        let labels: Vec<_> = app.seen.iter().map(|(_, s)| s.split(' ').next().unwrap().to_owned()).collect();
        assert_eq!(labels, vec!["input", "wake", "input"]);
        assert!(p.exited());
        assert_eq!(p.calls().last(), Some(&PlatformCall::Exit));
    }

    #[test]
    fn run_until_stops_the_clock_there() {
        let mut p = HeadlessPlatform::new();
        p.push_input(Time(1_000), WindowId(1), key(Key::A));
        let mut app = Recorder::default();
        p.run_until(&mut app, Some(Time(120)));
        assert_eq!(p.virtual_clock().now(), Time(120));
        assert_eq!(app.seen.len(), 1);
        p.run(&mut app);
        assert_eq!(app.seen.len(), 3);
    }

    #[test]
    fn quit_can_be_vetoed() {
        let mut p = HeadlessPlatform::new();
        p.push(Time(10), Scripted::QuitRequested);
        let mut app = Recorder { veto_quit: true, ..Default::default() };
        p.run_until(&mut app, Some(Time(20)));
        assert!(!p.exited());
        p.push(Time(30), Scripted::QuitRequested);
        app.veto_quit = false;
        p.run(&mut app);
        assert!(p.exited());
    }

    #[test]
    fn resize_events_update_the_window_record() {
        let mut p = HeadlessPlatform::new();
        p.push_window_event(Time(5), WindowId(1), WindowEvent::Resized { size: DeviceSize { w: 10, h: 20 }, scale: Scale(2.0) });
        let mut app = Recorder::default();
        p.run_until(&mut app, Some(Time(6)));
        let w = &p.windows()[&WindowId(1)];
        assert_eq!((w.size, w.scale), (DeviceSize { w: 10, h: 20 }, Scale(2.0)));
        assert_eq!(p.cx().window_scale(WindowId(1)), Some(Scale(2.0)));
    }
}
