//! [`Harness`]: kittest's verbs on skelter (architecture §16.1). It runs a real [`skelter::App`] on
//! `skelter_platform::HeadlessPlatform` and the [`HeadlessCompositor`], on the platform's virtual clock.
//! Input is scripted into the headless platform, which delivers it as an OS backend would
//! (`AppHandler::input`, `a11y_event`), so tests take the production dispatch path; queries read the
//! AccessKit snapshot folded from the app's `update_a11y` calls (grafted subtrees, egui islands', included:
//! `Commit::a11y_subtrees`).
//!
//! - `step` = one vsync on the virtual clock: queued input is delivered, the clock moves one interval,
//!   the compositor applies commits and presents, the UI answers `BeginFrame`.
//! - `run` = step until settled (nothing queued, no frame wanted, no commit in flight, no animation
//!   running), failing after `max_steps`; then kittest's accessibility check, when it is on.
//!
//! Verbs take skelter's or egui's argument types ([`crate::convert`]); `event` takes a skelter
//! `InputEvent` or an `egui::Event` (translated by [`EguiAdapter`]).

use crate::a11y::{A11yTree, AkNode, By, NodeInfo, NodeKey};
use crate::consumer::Consumer;
use crate::convert::{IntoButton, IntoKey, IntoModifiers, IntoPoint, IntoSize, TestEvent};
use crate::egui_event::{EguiAdapter, Injected};
use crate::headless::HeadlessCompositor;
use crate::render::{GpuRender, Image, RenderError};
use crate::snapshot::{SnapshotError, SnapshotOptions};
use skelter::platform::{
    A11yEvent, ClipboardKind, HeadlessPlatform, PlatformCall, PlatformCx, Scripted, VirtualClock,
    WindowDesc,
};
use skelter::seams::DocumentApi;
use skelter::{App, AppConfig};
use skelter_base::accesskit::{Action, ActionData, ActionRequest, NodeId, Role, TreeId};
use skelter_base::color::Color;
use skelter_base::event::{
    ImeEvent, InputEvent, KeyEvent, KeyLocation, KeyState, Modifiers, MouseButton, PointerEvent,
    PointerPhase, PointerType, ScrollDelta, WheelEvent, WindowEvent,
};
use skelter_base::geom::{DeviceSize, Point, Rect, Scale, Size, Vec2};
use skelter_base::ids::WindowId;
use skelter_base::key::Key;
use skelter_base::smallvec::SmallVec;
use skelter_base::text::GlyphRasterizer;
use skelter_base::time::{Clock, Span, Time};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

/// The harness did not settle within its step budget.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotSettled {
    pub steps: u64,
    pub reason: String,
}

impl std::fmt::Display for NotSettled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "not settled after {} steps: {}", self.steps, self.reason)
    }
}

impl std::error::Error for NotSettled {}

/// Document-specific work the harness does around the production path (set by [`crate::island`]).
pub(crate) struct Hooks<D> {
    /// The clipboard text a paste puts on the platform clipboard, for clipboards the document owns.
    pub clipboard: Option<Box<dyn FnMut(&mut D, &str)>>,
    /// Before `render`: draw the document's own textures (an island's) on the harness's device.
    pub textures: Option<Box<dyn FnMut(&mut D, &mut GpuRender)>>,
    /// At the start of each step (an egui island's parity mode: one pass per step, as kittest).
    pub before_step: Option<Box<dyn FnMut(&mut D)>>,
    /// Replaces the settle rule after queued input (kittest's: the last pass requested no repaint),
    /// given the next frame's target time; commits in flight then don't count, and `render` applies
    /// them first.
    pub settled: Option<Box<dyn Fn(&D, Time) -> bool>>,
    /// `step` delivers queued input one event per step (kittest: one egui pass per event, so a
    /// modifier change is seen by the press after it).
    pub one_event_per_step: bool,
    /// `type_text` sends key presses carrying the text (an egui island maps them to
    /// `egui::Event::Text`, as kittest types) instead of an IME commit.
    pub text_as_keys: bool,
}

impl<D> Default for Hooks<D> {
    fn default() -> Self {
        Hooks {
            clipboard: None,
            textures: None,
            before_step: None,
            settled: None,
            one_event_per_step: false,
            text_as_keys: false,
        }
    }
}

pub struct HarnessBuilder {
    pub(crate) size: Size,
    pub(crate) scale: f32,
    pub(crate) interval: Span,
    pub(crate) max_steps: u64,
    pub(crate) start: Time,
    pub(crate) config: AppConfig,
    pub(crate) glyphs: Option<Arc<dyn GlyphRasterizer>>,
    pub(crate) clear: Color,
    pub(crate) render_every_step: bool,
    pub(crate) check_accessibility: bool,
    pub(crate) snapshot: SnapshotOptions,
    pub(crate) native_menu_bar: bool,
    pub(crate) calls_cap: Option<usize>,
    pub(crate) consumer: bool,
}

impl Default for HarnessBuilder {
    fn default() -> Self {
        Self {
            size: Size::new(800.0, 600.0),
            scale: 1.0,
            interval: Span::from_micros(16_667),
            max_steps: 64,
            calls_cap: None,
            start: Time(1_000_000_000),
            config: AppConfig::default(),
            glyphs: None,
            clear: Color::WHITE,
            render_every_step: false,
            check_accessibility: true,
            snapshot: SnapshotOptions::default(),
            native_menu_bar: false,
            consumer: false,
        }
    }
}

impl HarnessBuilder {
    pub fn new() -> Self {
        Self::default()
    }
    /// The window's inner size in points (skelter `Size` or `egui::Vec2`).
    pub fn with_size(mut self, size: impl IntoSize) -> Self {
        self.size = size.into_size();
        self
    }
    pub fn with_scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }
    /// kittest's name for [`HarnessBuilder::with_scale`].
    pub fn with_pixels_per_point(self, ppp: f32) -> Self {
        self.with_scale(ppp)
    }
    /// The simulated display interval (one `step`).
    pub fn with_interval(mut self, interval: Span) -> Self {
        self.interval = interval;
        self
    }
    /// kittest's name for [`HarnessBuilder::with_interval`], in seconds.
    pub fn with_step_dt(self, dt: f32) -> Self {
        self.with_interval(Span::from_nanos((f64::from(dt) * 1e9).round() as i64))
    }
    pub fn with_max_steps(mut self, n: u64) -> Self {
        self.max_steps = n;
        self
    }
    /// The glyph rasteriser `render` draws text with (default: skelter-text over its bundled test
    /// fonts, `FontConfig::testing()`).
    pub fn with_glyphs(mut self, glyphs: Arc<dyn GlyphRasterizer>) -> Self {
        self.glyphs = Some(glyphs);
        self
    }
    /// Whether the headless platform shows menu bars natively, as macOS does (`set_menu_bar` answers
    /// it; default false).
    pub fn with_native_menu_bar(mut self, native: bool) -> Self {
        self.native_menu_bar = native;
        self
    }
    /// What `render` clears to (default white).
    pub fn with_clear(mut self, clear: Color) -> Self {
        self.clear = clear;
        self
    }
    /// The app's configuration (its window list is replaced by the harness window).
    pub fn with_config(mut self, config: AppConfig) -> Self {
        self.config = config;
        self
    }
    /// Render after every step (kittest's; paint callbacks then run each step). Needs an adapter.
    pub fn with_render_every_step(mut self, on: bool) -> Self {
        self.render_every_step = on;
        self
    }
    /// skelter's `render` never draws a pointer cursor; only `false` is accepted.
    #[track_caller]
    pub fn with_render_cursor(self, on: bool) -> Self {
        assert!(
            !on,
            "skelter-test renders the committed tree; the OS cursor is not part of it"
        );
        self
    }
    /// Keep only the last `n` platform calls ([`Harness::calls`]); a long run (a tour of thousands of
    /// frames, each sending an AccessKit tree) otherwise grows without bound. Default: all.
    pub fn with_calls_cap(mut self, n: usize) -> Self {
        self.calls_cap = Some(n);
        self
    }
    /// Also fold the app's AccessKit updates into AccessKit's own consumers ([`crate::consumer`]: an
    /// `accesskit_consumer::Tree`, and on Linux an AT-SPI adapter), read with [`Harness::consumer`].
    /// Default off: the consumers panic on trees a platform adapter would reject.
    pub fn with_consumer(mut self, on: bool) -> Self {
        self.consumer = on;
        self
    }
    /// kittest's check that every input widget has an accessible name, after each `run` (default on).
    pub fn with_accessibility_check(mut self, on: bool) -> Self {
        self.check_accessibility = on;
        self
    }
    /// The options `snapshot` and `try_snapshot` use.
    pub fn with_options(mut self, options: SnapshotOptions) -> Self {
        self.snapshot = options;
        self
    }

    /// Build the harness around `doc` and run it until settled.
    #[track_caller]
    pub fn build<D: DocumentApi + 'static>(self, doc: D) -> Harness<D> {
        let mut h = self.build_unsettled(doc);
        h.run();
        h
    }

    /// Build without running (to assert on the first frames).
    pub fn build_unsettled<D: DocumentApi + 'static>(self, doc: D) -> Harness<D> {
        self.build_with_hooks(doc, Hooks::default())
    }

    pub(crate) fn build_with_hooks<D: DocumentApi + 'static>(
        self,
        doc: D,
        hooks: Hooks<D>,
    ) -> Harness<D> {
        self.build_with(|_, _| doc, hooks, None)
    }

    /// The glyph rasteriser `render` uses.
    pub(crate) fn default_glyphs(&self) -> Arc<dyn GlyphRasterizer> {
        match &self.glyphs {
            Some(g) => g.clone(),
            None => {
                let fonts = skelter_text::FontStore::new(skelter_text::FontConfig::testing());
                Arc::new(skelter_text::FontRasterizer::new(fonts))
            }
        }
    }

    /// Build around the document `make_doc` makes with the platform's clock (and the GPU, when one is
    /// opened up front for it).
    pub(crate) fn build_with<D: DocumentApi + 'static>(
        self,
        make_doc: impl FnOnce(Arc<dyn Clock>, Option<&GpuRender>) -> D,
        hooks: Hooks<D>,
        gpu: Option<GpuRender>,
    ) -> Harness<D> {
        let mut platform = HeadlessPlatform::new();
        platform.cx().default_scale = Scale(self.scale);
        platform.cx().native_menu_bar = self.native_menu_bar;
        let clock = platform.virtual_clock();
        clock.set(self.start);
        let doc = make_doc(clock.clone(), gpu.as_ref());
        let mut config = self.config;
        config.windows = vec![WindowDesc::new("skelter-test", self.size)];
        let slot: Rc<RefCell<Option<HeadlessCompositor>>> = Rc::default();
        let keep = slot.clone();
        let spawn = Box::new(
            move |sink, _clock, _monitors: &[skelter::platform::Monitor]| {
                let c = HeadlessCompositor::new(sink);
                *keep.borrow_mut() = Some(c.clone());
                c
            },
        );
        let mut app = App::new(config, doc, spawn);
        platform.run_until(&mut app, Some(self.start));
        let window = app.windows().next().expect("the harness window opened");
        let compositor = slot
            .borrow_mut()
            .take()
            .expect("init spawned the compositor");
        let mut h = Harness {
            app,
            platform,
            compositor,
            clock,
            window,
            interval: self.interval,
            max_steps: self.max_steps,
            queue: RefCell::new(Vec::new()),
            modifiers: Modifiers::default(),
            egui: RefCell::new(EguiAdapter::default()),
            a11y: A11yTree::default(),
            consumer: self.consumer.then(|| {
                let mut c = Consumer::new();
                let (w, h) = (f64::from(self.size.w * self.scale), f64::from(self.size.h * self.scale));
                let r = skelter_base::accesskit::Rect { x0: 0.0, y0: 0.0, x1: w, y1: h };
                c.set_window_bounds(r, r);
                c
            }),
            calls: Vec::new(),
            calls_cap: self.calls_cap,
            steps: 0,
            glyphs: self.glyphs,
            clear: self.clear,
            gpu,
            render_every_step: self.render_every_step,
            last_render: None,
            check_accessibility: self.check_accessibility,
            snapshot: self.snapshot,
            snapshot_failures: Vec::new(),
            hooks,
        };
        h.collect_calls();
        // The platform reports the window's size once it exists, as an OS backend does.
        let device = DeviceSize {
            w: (self.size.w * self.scale).round() as u32,
            h: (self.size.h * self.scale).round() as u32,
        };
        h.window_event(WindowEvent::Resized {
            size: device,
            scale: Scale(self.scale),
        });
        h.deliver();
        h
    }
}

/// What a node handle queues.
#[derive(Clone, Debug)]
enum Queued {
    Input(WindowId, InputEvent),
    A11y(WindowId, ActionRequest),
    Window(WindowId, WindowEvent),
    Clipboard(String),
}

pub struct Harness<D: DocumentApi + 'static> {
    app: App<D, HeadlessCompositor>,
    platform: HeadlessPlatform,
    /// The harness's handle on the compositor's state (it outlives the app's on exit).
    compositor: HeadlessCompositor,
    clock: Arc<VirtualClock>,
    window: WindowId,
    interval: Span,
    max_steps: u64,
    /// Interior mutability so node handles (borrowing the harness) can queue input, as kittest's do.
    queue: RefCell<Vec<Queued>>,
    modifiers: Modifiers,
    egui: RefCell<EguiAdapter>,
    a11y: A11yTree,
    /// AccessKit's own consumers on the same updates ([`HarnessBuilder::with_consumer`]).
    consumer: Option<Consumer>,
    /// Every platform call the app made, in order.
    calls: Vec<PlatformCall>,
    calls_cap: Option<usize>,
    steps: u64,
    glyphs: Option<Arc<dyn GlyphRasterizer>>,
    clear: Color,
    /// Opened by the first `render`.
    gpu: Option<GpuRender>,
    render_every_step: bool,
    last_render: Option<Image>,
    pub(crate) check_accessibility: bool,
    snapshot: SnapshotOptions,
    /// `snapshot` failures, reported when the harness drops (kittest's `SnapshotResults`).
    snapshot_failures: Vec<SnapshotError>,
    hooks: Hooks<D>,
}

impl<D: DocumentApi + 'static> Drop for Harness<D> {
    fn drop(&mut self) {
        if !self.snapshot_failures.is_empty() && !std::thread::panicking() {
            let all: Vec<String> = self.snapshot_failures.iter().map(|e| e.to_string()).collect();
            panic!("{} snapshot(s) failed:\n{}", all.len(), all.join("\n"));
        }
    }
}

impl<D: DocumentApi + 'static> Harness<D> {
    pub fn builder() -> HarnessBuilder {
        HarnessBuilder::default()
    }

    /// Build with the defaults (800×600 points, 60 Hz) and run until settled.
    #[track_caller]
    pub fn new(doc: D) -> Self {
        HarnessBuilder::default().build(doc)
    }

    // ---- state ----------------------------------------------------------------------------------

    /// The document (kittest's `state`).
    pub fn state(&self) -> &D {
        self.app.doc()
    }
    pub fn state_mut(&mut self) -> &mut D {
        self.app.doc_mut()
    }
    pub fn app(&self) -> &App<D, HeadlessCompositor> {
        &self.app
    }
    pub fn app_mut(&mut self) -> &mut App<D, HeadlessCompositor> {
        &mut self.app
    }
    pub fn compositor(&self) -> &HeadlessCompositor {
        &self.compositor
    }
    pub fn platform(&self) -> &HeadlessPlatform {
        &self.platform
    }
    pub fn platform_mut(&mut self) -> &mut HeadlessPlatform {
        &mut self.platform
    }
    /// Every platform call the app made so far (windows, cursors, IME, a11y updates, exit).
    pub fn calls(&self) -> &[PlatformCall] {
        &self.calls
    }
    /// Draws the document's own textures (its egui islands, offscreen viewports) on the harness's
    /// device before every [`render`](Self::render), replacing any earlier hook: with
    /// [`crate::Registrations`] and [`crate::register_textures`] for the skelter-render side.
    pub fn set_textures(&mut self, f: impl FnMut(&mut D, &mut GpuRender) + 'static) {
        self.hooks.textures = Some(Box::new(f));
    }
    /// [`set_textures`](Self::set_textures), builder style.
    pub fn with_textures(mut self, f: impl FnMut(&mut D, &mut GpuRender) + 'static) -> Self {
        self.set_textures(f);
        self
    }
    /// The platform calls made since the last take, removed from the harness.
    pub fn take_calls(&mut self) -> Vec<PlatformCall> {
        std::mem::take(&mut self.calls)
    }
    pub fn exited(&self) -> bool {
        self.platform.exited()
    }
    pub fn window(&self) -> WindowId {
        self.window
    }
    pub fn now(&self) -> Time {
        self.clock.now()
    }
    pub fn clock(&self) -> &Arc<VirtualClock> {
        &self.clock
    }
    /// One step's virtual time.
    pub fn interval(&self) -> Span {
        self.interval
    }
    pub fn steps(&self) -> u64 {
        self.steps
    }
    pub fn a11y(&self) -> &A11yTree {
        &self.a11y
    }
    /// AccessKit's consumer tree (and on Linux the AT-SPI adapter) on the app's updates, when built
    /// [`with_consumer`](HarnessBuilder::with_consumer).
    #[track_caller]
    pub fn consumer(&self) -> &Consumer {
        self.consumer.as_ref().expect("build the harness with_consumer(true)")
    }
    #[track_caller]
    pub fn consumer_mut(&mut self) -> &mut Consumer {
        self.consumer.as_mut().expect("build the harness with_consumer(true)")
    }
    /// The image of the last step when built `with_render_every_step(true)`.
    pub fn last_render(&self) -> Option<&Image> {
        self.last_render.as_ref()
    }

    // ---- running --------------------------------------------------------------------------------

    /// One vsync on the virtual clock.
    #[track_caller]
    pub fn step(&mut self) {
        if self.hooks.one_event_per_step && self.queue.borrow().len() > 1 {
            let queued = std::mem::take(&mut *self.queue.borrow_mut());
            for q in queued {
                self.queue.borrow_mut().push(q);
                self.step_one();
            }
            return;
        }
        self.step_one();
    }

    fn step_one(&mut self) {
        if let Some(f) = &mut self.hooks.before_step {
            f(self.app.doc_mut());
        }
        self.deliver();
        // The test may have changed the document's state between steps (`state_mut`): it gets its frame.
        self.app.request_dirty_frames();
        self.clock.advance(self.interval);
        let now = self.now();
        self.compositor.vsync(now, self.interval);
        // The vsync's messages woke the UI thread; the platform delivers the wake.
        self.platform.run_until(&mut self.app, Some(now));
        self.collect_calls();
        self.steps += 1;
        if self.render_every_step {
            let img = self
                .render()
                .unwrap_or_else(|e| panic!("render_every_step: {e}"));
            self.last_render = Some(img);
        }
    }

    /// `n` steps (kittest's `run_steps`).
    #[track_caller]
    pub fn run_steps(&mut self, n: usize) {
        for _ in 0..n {
            self.step();
        }
    }

    fn collect_calls(&mut self) {
        for c in self.platform.take_calls() {
            if let PlatformCall::UpdateA11y(w, u) = &c
                && *w == self.window
            {
                self.a11y.apply(u);
                if let Some(c) = &mut self.consumer {
                    c.apply(u);
                }
            }
            self.calls.push(c);
        }
        if let Some(cap) = self.calls_cap
            && self.calls.len() > cap
        {
            let over = self.calls.len() - cap;
            self.calls.drain(..over);
        }
    }

    /// Script the queued input into the headless platform at the current time and let it deliver.
    fn deliver(&mut self) {
        let queued = std::mem::take(&mut *self.queue.borrow_mut());
        let now = self.now();
        for q in queued {
            match q {
                Queued::Input(w, event) => self.platform.push_input(now, w, event),
                Queued::A11y(w, req) => self.platform.push(
                    now,
                    Scripted::A11y {
                        window: w,
                        event: A11yEvent::Action(req),
                    },
                ),
                Queued::Window(w, ev) => self.platform.push_window_event(now, w, ev),
                Queued::Clipboard(text) => {
                    // Deliver what came before, then set the clipboard for the paste chord after it.
                    self.platform.run_until(&mut self.app, Some(now));
                    self.platform
                        .cx()
                        .clipboard()
                        .set_text(ClipboardKind::Clipboard, &text);
                    if let Some(f) = &mut self.hooks.clipboard {
                        f(self.app.doc_mut(), &text);
                    }
                }
            }
        }
        self.platform.run_until(&mut self.app, Some(now));
        self.collect_calls();
    }

    /// The window as the compositor has it now (every applied commit), drawn by skelter-render on the
    /// environment's wgpu adapter. Opens the device on first use; `Err` when there is none.
    pub fn render(&mut self) -> Result<Image, RenderError> {
        if self.gpu.is_none() {
            let glyphs = match &self.glyphs {
                Some(g) => g.clone(),
                None => {
                    let fonts = skelter_text::FontStore::new(skelter_text::FontConfig::testing());
                    Arc::new(skelter_text::FontRasterizer::new(fonts)) as Arc<dyn GlyphRasterizer>
                }
            };
            self.gpu = Some(GpuRender::new(glyphs)?);
        }
        if self.hooks.settled.is_some() {
            // the last step's commit is still in flight under the parity rule: apply it
            let now = self.now();
            self.compositor.vsync(now, self.interval);
            self.platform.run_until(&mut self.app, Some(now));
            self.collect_calls();
        }
        let gpu = self.gpu.as_mut().expect("opened above");
        if let Some(f) = &mut self.hooks.textures {
            f(self.app.doc_mut(), gpu);
        }
        self.compositor.render(self.window, gpu, self.clear)
    }

    /// The harness's GPU, once `render` opened it.
    pub fn gpu(&mut self) -> Option<&mut GpuRender> {
        self.gpu.as_mut()
    }

    /// Why the harness is not settled, or `None` when it is (or the app exited).
    pub fn busy(&self) -> Option<String> {
        if self.exited() {
            return None;
        }
        if !self.queue.borrow().is_empty() {
            return Some("queued input".into());
        }
        if let Some(settled) = &self.hooks.settled {
            // the next pass runs at the next frame's target: a repaint requested for it (egui's
            // delay 0 after input) is not settled
            return (!settled(self.app.doc(), self.now() + self.interval)).then(|| "the last pass requested a repaint".into());
        }
        if !self.app.is_idle() {
            return Some("the app has a frame to run".into());
        }
        if !self.compositor.is_idle() {
            let endless = self.compositor.endless_animations();
            if !endless.is_empty() {
                return Some(format!(
                    "animations {endless:?} have no end the headless compositor can compute (physical springs need skelter-anim)"
                ));
            }
            return Some("the compositor has a commit, frame or animation in flight".into());
        }
        None
    }

    /// Step until settled; `Err` after `max_steps`. Runs kittest's accessibility check when settled.
    #[track_caller]
    pub fn try_run(&mut self) -> Result<u64, NotSettled> {
        let n = self.run_until_settled(self.max_steps)?;
        if self.check_accessibility {
            self.check_accessibility();
        }
        Ok(n)
    }

    /// Step until settled; `None` after `max_steps` (kittest's `run_ok`: no accessibility check).
    pub fn run_ok(&mut self) -> Option<u64> {
        self.run_until_settled(self.max_steps).ok()
    }

    /// Step until settled, at most `max` steps. Always steps at least once.
    #[track_caller]
    pub fn run_until_settled(&mut self, max: u64) -> Result<u64, NotSettled> {
        // counted in vsyncs: a step that delivers one event per vsync takes several
        let start = self.steps;
        loop {
            self.step();
            let n = self.steps - start;
            match self.busy() {
                None => return Ok(n),
                Some(reason) if n >= max => return Err(NotSettled { steps: n, reason }),
                Some(_) => {}
            }
        }
    }

    /// Step until settled (kittest's `run`); panics after `max_steps`.
    #[track_caller]
    pub fn run(&mut self) -> u64 {
        match self.try_run() {
            Ok(n) => n,
            Err(e) => panic!("{e}"),
        }
    }

    /// Advance the clock by `span` in steps (timers, animations), then settle.
    #[track_caller]
    pub fn run_for(&mut self, span: Span) {
        let end = self.now() + span;
        while self.now() < end {
            self.step();
        }
        self.run();
    }

    // ---- accessibility --------------------------------------------------------------------------

    /// Input widgets (egui's `is_input` roles) that are not hidden, not read-only and have neither a
    /// label nor a placeholder, described as kittest describes them.
    pub fn unnamed_widgets(&self) -> Vec<String> {
        let walk = self.a11y.walk();
        walk.iter()
            .filter(|n| egui::accessibility::is_input(n.role()) && !n.is_hidden())
            .filter(|n| !n.node.is_read_only())
            .filter(|n| {
                let ak = AkNode { info: **n, tree: &self.a11y };
                let named = ak.label().is_some_and(|l| !l.trim().is_empty());
                let placeholder = n.node.placeholder().is_some_and(|p| !p.trim().is_empty());
                !named && !placeholder
            })
            .map(|n| {
                let inside = std::iter::successors(n.parent, |p| {
                    walk.iter().find(|w| w.key() == *p).and_then(|w| w.parent)
                })
                .find_map(|p| self.a11y.node(p).and_then(|x| x.label()).map(str::to_owned))
                .map_or_else(String::new, |l| format!(" inside {l:?}"));
                format!("{:?} at {:?}{inside}", n.role(), n.node.bounds())
            })
            .collect()
    }

    /// kittest's check: every input widget has an accessible name.
    #[track_caller]
    pub fn check_accessibility(&self) {
        let unnamed = self.unnamed_widgets();
        assert!(
            unnamed.is_empty(),
            "{} widget(s) lack accessible name. You can disable this check with HarnessBuilder::with_accessibility_check(false). The widgets:\n{}",
            unnamed.len(),
            unnamed.join("\n")
        );
    }

    // ---- input ----------------------------------------------------------------------------------

    fn push(&self, q: Queued) {
        self.queue.borrow_mut().push(q);
    }

    /// Queue an event for the harness window (delivered by the next `step`): a skelter `InputEvent`,
    /// or an `egui::Event` translated as the platform would deliver it.
    pub fn event(&self, ev: impl Into<TestEvent>) {
        match ev.into() {
            TestEvent::Skelter(e) => self.push(Queued::Input(self.window, e)),
            TestEvent::Egui(e) => {
                let injected = self.egui.borrow_mut().translate(&e);
                for i in injected {
                    self.push(match i {
                        Injected::Input(e) => Queued::Input(self.window, e),
                        Injected::A11y(r) => Queued::A11y(self.window, r),
                        Injected::Focused(f) => Queued::Window(self.window, WindowEvent::Focused(f)),
                        Injected::SetClipboard(t) => Queued::Clipboard(t),
                    });
                }
            }
        }
    }

    /// An egui event (kept for tests written before `event` took both).
    pub fn event_egui(&self, ev: egui::Event) {
        self.event(ev);
    }

    /// `ev` with `m` held: the modifiers change before and are released after (kittest's).
    pub fn event_modifiers(&self, ev: impl Into<TestEvent>, m: impl IntoModifiers) {
        let m = m.into_modifiers(self.macos());
        self.event(InputEvent::ModifiersChanged(m));
        self.event(ev);
        self.event(InputEvent::ModifiersChanged(self.modifiers));
    }

    /// An AccessKit action request, delivered as the platform adapter delivers one.
    pub fn accesskit_action(&self, req: ActionRequest) {
        self.push(Queued::A11y(self.window, req));
    }

    pub fn window_event(&self, ev: WindowEvent) {
        self.push(Queued::Window(self.window, ev));
    }

    /// Set the window's UI zoom ([`App::set_zoom`]), after delivering what is queued.
    pub fn set_zoom(&mut self, factor: f32) {
        let now = self.now();
        self.platform.run_until(&mut self.app, Some(now));
        self.app.set_zoom(self.platform.cx(), self.window, factor);
    }

    /// Resize the window (points: skelter `Size` or `egui::Vec2`), through the resize handshake.
    pub fn set_size(&self, size: impl IntoSize) {
        let size = size.into_size();
        let scale = self.scale();
        let device = DeviceSize {
            w: (size.w * scale).round() as u32,
            h: (size.h * scale).round() as u32,
        };
        self.window_event(WindowEvent::Resized {
            size: device,
            scale: Scale(scale),
        });
    }

    /// Change the window's scale, keeping its size in points.
    pub fn set_pixels_per_point(&self, ppp: f32) {
        let pts = self.size();
        self.window_event(WindowEvent::Resized {
            size: DeviceSize {
                w: (pts.w * ppp).round() as u32,
                h: (pts.h * ppp).round() as u32,
            },
            scale: Scale(ppp),
        });
    }

    pub fn scale(&self) -> f32 {
        self.app.window_size(self.window).map_or(1.0, |(_, s)| s.0)
    }

    /// The window's size in points.
    pub fn size(&self) -> Size {
        self.app
            .window_size(self.window)
            .map_or(Size::ZERO, |(d, s)| Size::new(d.w as f32 / s.0, d.h as f32 / s.0))
    }

    fn macos(&self) -> bool {
        self.egui.borrow().macos
    }

    /// A native menu item chosen now (`AppHandler::menu_activated`), as the macOS backend reports it.
    pub fn menu_activated(&mut self, id: &str) {
        self.deliver();
        skelter::platform::AppHandler::menu_activated(&mut self.app, self.platform.cx(), id);
        self.collect_calls();
    }

    /// Modifiers held for the following keys, clicks and wheel input.
    pub fn set_modifiers(&mut self, m: impl IntoModifiers) {
        self.modifiers = m.into_modifiers(self.macos());
        self.event(InputEvent::ModifiersChanged(self.modifiers));
    }

    fn key(&self, key: Key, state: KeyState, m: Modifiers) {
        self.event(InputEvent::Key(KeyEvent {
            key: Some(key),
            physical: 0,
            text: None,
            state,
            repeat: false,
            location: KeyLocation::Standard,
            modifiers: m,
        }));
    }

    pub fn key_down(&self, key: impl IntoKey) {
        self.key(key.into_key(), KeyState::Pressed, self.modifiers);
    }
    pub fn key_up(&self, key: impl IntoKey) {
        self.key(key.into_key(), KeyState::Released, self.modifiers);
    }
    pub fn key_down_modifiers(&self, m: impl IntoModifiers, key: impl IntoKey) {
        let m = m.into_modifiers(self.macos());
        self.event(InputEvent::ModifiersChanged(m));
        self.key(key.into_key(), KeyState::Pressed, m);
        self.event(InputEvent::ModifiersChanged(self.modifiers));
    }
    pub fn key_up_modifiers(&self, m: impl IntoModifiers, key: impl IntoKey) {
        let m = m.into_modifiers(self.macos());
        self.event(InputEvent::ModifiersChanged(m));
        self.key(key.into_key(), KeyState::Released, m);
        self.event(InputEvent::ModifiersChanged(self.modifiers));
    }
    /// Press `keys` in order, release them in reverse (kittest's `key_combination`).
    pub fn key_combination<K: IntoKey + Copy>(&self, keys: &[K]) {
        for k in keys {
            self.key_down(*k);
        }
        for k in keys.iter().rev() {
            self.key_up(*k);
        }
    }
    /// With `m` held: every key down, then every key up (kittest's `key_combination_modifiers`).
    pub fn key_combination_modifiers<K: IntoKey + Copy>(&self, m: impl IntoModifiers, keys: &[K]) {
        let m = m.into_modifiers(self.macos());
        self.event(InputEvent::ModifiersChanged(m));
        for state in [KeyState::Pressed, KeyState::Released] {
            for k in keys {
                self.key(k.into_key(), state, m);
            }
        }
        self.event(InputEvent::ModifiersChanged(self.modifiers));
    }
    /// Press and release `key` with the held modifiers.
    pub fn key_press(&self, key: impl IntoKey) {
        self.key_combination(&[key.into_key()]);
    }
    pub fn key_press_modifiers(&self, m: impl IntoModifiers, key: impl IntoKey) {
        self.key_combination_modifiers(m, &[key.into_key()]);
    }
    /// Type text into the focused node: an IME commit, or in an egui island key presses carrying the
    /// text (egui's `Event::Text`, as kittest types).
    pub fn type_text(&self, text: &str) {
        if self.hooks.text_as_keys {
            self.event(InputEvent::Key(KeyEvent {
                key: None,
                physical: 0,
                text: Some(text.to_owned()),
                state: KeyState::Pressed,
                repeat: false,
                location: KeyLocation::Standard,
                modifiers: Modifiers::default(),
            }));
            return;
        }
        self.event(InputEvent::Ime(ImeEvent::Commit(text.to_owned())));
    }

    fn pointer(
        &self,
        phase: PointerPhase,
        pos: Point,
        button: Option<MouseButton>,
        buttons: u32,
        modifiers: Modifiers,
    ) -> InputEvent {
        InputEvent::Pointer(PointerEvent {
            phase,
            pointer: 0,
            kind: PointerType::Mouse,
            button,
            buttons,
            pos,
            modifiers,
            pressure: if buttons != 0 { 0.5 } else { 0.0 },
            tilt: Vec2::ZERO,
            history: SmallVec::new(),
        })
    }

    pub fn hover_at(&self, pos: impl IntoPoint) {
        let pos = pos.into_point();
        self.egui.borrow_mut().pointer = pos;
        self.event(self.pointer(PointerPhase::Move, pos, None, 0, self.modifiers));
    }
    /// Move, press and release the primary button at `pos`.
    pub fn click_at(&self, pos: impl IntoPoint) {
        self.click_button_at(pos, MouseButton::Primary);
    }
    pub fn click_button_at(&self, pos: impl IntoPoint, b: impl IntoButton) {
        self.click_button_modifiers_at(pos, b, self.modifiers);
    }
    /// Move, then press and release `b` with `m` held (they change before and are released after).
    pub fn click_button_modifiers_at(
        &self,
        pos: impl IntoPoint,
        b: impl IntoButton,
        m: impl IntoModifiers,
    ) {
        let (pos, b, m) = (pos.into_point(), b.into_button(), m.into_modifiers(self.macos()));
        let bit = button_bit(b);
        self.hover_at(pos);
        if m != self.modifiers {
            self.event(InputEvent::ModifiersChanged(m));
        }
        self.event(self.pointer(PointerPhase::Down, pos, Some(b), bit, m));
        self.event(self.pointer(PointerPhase::Up, pos, Some(b), 0, m));
        if m != self.modifiers {
            self.event(InputEvent::ModifiersChanged(self.modifiers));
        }
    }
    /// Press the primary button at `pos` (kittest's `drag_at`: the start of a drag; moves follow with
    /// `hover_at`, the release with `drop_at`).
    pub fn drag_at(&self, pos: impl IntoPoint) {
        let pos = pos.into_point();
        self.egui.borrow_mut().pointer = pos;
        self.event(self.pointer(PointerPhase::Down, pos, Some(MouseButton::Primary), 1, self.modifiers));
    }
    /// Release the primary button at `pos`, then the pointer leaves (kittest's `drop_at`).
    pub fn drop_at(&self, pos: impl IntoPoint) {
        let pos = pos.into_point();
        self.egui.borrow_mut().pointer = pos;
        self.event(self.pointer(PointerPhase::Up, pos, Some(MouseButton::Primary), 0, self.modifiers));
        self.remove_cursor();
    }
    /// The pointer leaves the window (kittest's `remove_cursor`).
    pub fn remove_cursor(&self) {
        let pos = self.egui.borrow().pointer;
        self.event(self.pointer(PointerPhase::Leave, pos, None, 0, self.modifiers));
    }
    /// A primary-button drag from `from` to `to` in `steps` moves.
    pub fn drag(&self, from: impl IntoPoint, to: impl IntoPoint, steps: u32) {
        let (from, to) = (from.into_point(), to.into_point());
        let m = self.modifiers;
        self.event(self.pointer(PointerPhase::Move, from, None, 0, m));
        self.event(self.pointer(PointerPhase::Down, from, Some(MouseButton::Primary), 1, m));
        let n = steps.max(1);
        for i in 1..=n {
            let t = i as f32 / n as f32;
            let p = Point::new(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t);
            self.event(self.pointer(PointerPhase::Move, p, None, 1, m));
        }
        self.event(self.pointer(PointerPhase::Up, to, Some(MouseButton::Primary), 0, m));
        self.egui.borrow_mut().pointer = to;
    }
    pub fn wheel_at(&self, pos: impl IntoPoint, delta: ScrollDelta) {
        let pos = pos.into_point();
        self.egui.borrow_mut().pointer = pos;
        self.event(InputEvent::Wheel(WheelEvent {
            pos,
            delta,
            phase: None,
            momentum: None,
            modifiers: self.modifiers,
        }));
    }

    // ---- queries --------------------------------------------------------------------------------

    fn node_ref<'h>(&'h self, info: NodeInfo<'h>) -> Node<'h, D> {
        Node {
            harness: self,
            info,
        }
    }

    fn find(&self, scope: Option<NodeKey>, by: &By<'_>) -> Vec<Node<'_, D>> {
        self.a11y
            .query(scope, by)
            .into_iter()
            .map(|n| self.node_ref(n))
            .collect()
    }

    /// The root node (the window); queries on it search the whole tree.
    #[track_caller]
    pub fn root(&self) -> Node<'_, D> {
        let root = self
            .a11y
            .root_key()
            .unwrap_or_else(|| panic!("no AccessKit tree yet"));
        self.node_at(root)
            .expect("the root is in the walk")
    }

    fn node_at(&self, key: NodeKey) -> Option<Node<'_, D>> {
        self.a11y
            .walk()
            .into_iter()
            .find(|n| n.key() == key)
            .map(|n| self.node_ref(n))
    }

    /// The focused node, when focus is on something other than the root.
    pub fn focused(&self) -> Option<Node<'_, D>> {
        let f = self.a11y.focus()?;
        if Some(f) == self.a11y.root_key() {
            return None;
        }
        self.node_at(f)
    }

    /// The snapshot as an indented outline (role, label, value, author id, bounds), for failure messages.
    pub fn a11y_dump(&self) -> String {
        let mut out = String::new();
        for n in &self.a11y.walk() {
            out.push_str(&"  ".repeat(n.depth));
            out.push_str(&format!("{:?}", n.role()));
            if let Some(l) = n.match_label() {
                out.push_str(&format!(" {l:?}"));
            }
            if let Some(v) = n.value()
                && n.role() != Role::Label
            {
                out.push_str(&format!(" value={v:?}"));
            }
            if let Some(a) = n.author_id() {
                out.push_str(&format!(" #{a}"));
            }
            if let Some(b) = n.bounds {
                out.push_str(&format!(
                    " [{:.0},{:.0} {:.0}×{:.0}]",
                    b.min.x,
                    b.min.y,
                    b.width(),
                    b.height()
                ));
            }
            out.push('\n');
        }
        out
    }

    // ---- snapshots ------------------------------------------------------------------------------

    /// Render and compare with `{output_path}/{name}.png` under the harness's options (kittest's
    /// `try_snapshot`; see [`crate::snapshot`]). Runs the accessibility check first when it is on.
    pub fn try_snapshot(&mut self, name: impl Into<String>) -> Result<(), SnapshotError> {
        if self.check_accessibility {
            self.check_accessibility();
        }
        let options = self.snapshot.clone();
        self.try_snapshot_options(name, &options)
    }

    pub fn try_snapshot_options(
        &mut self,
        name: impl Into<String>,
        options: &SnapshotOptions,
    ) -> Result<(), SnapshotError> {
        let img = self.render().map_err(SnapshotError::Render)?;
        crate::snapshot::try_image_snapshot_options(&img, name, options)
    }

    /// Like `try_snapshot`; a failure is reported when the harness drops (kittest's `snapshot`).
    pub fn snapshot(&mut self, name: impl Into<String>) {
        if let Err(e) = self.try_snapshot(name) {
            self.snapshot_failures.push(e);
        }
    }

    pub fn snapshot_options(&mut self, name: impl Into<String>, options: &SnapshotOptions) {
        if let Err(e) = self.try_snapshot_options(name, options) {
            self.snapshot_failures.push(e);
        }
    }

    /// The snapshot failures so far, taken (so the drop does not report them).
    pub fn take_snapshot_results(&mut self) -> Vec<SnapshotError> {
        std::mem::take(&mut self.snapshot_failures)
    }
}

fn button_bit(b: MouseButton) -> u32 {
    match b {
        MouseButton::Primary => 1,
        MouseButton::Secondary => 2,
        MouseButton::Middle => 4,
        MouseButton::Back => 8,
        MouseButton::Forward => 16,
        MouseButton::Other(_) => 0,
    }
}

/// kittest's queries (`Queryable`) on a scope: the harness (the whole tree) or a node (its subtree).
/// `get_*` panics on none or several matches, `get_all_*` on none; `query_*` panics on several.
macro_rules! queries {
    ($h:lifetime) => {
        /// Every node matching `by` (kittest's `query_all`).
        pub fn query_all(&self, by: By<'_>) -> std::vec::IntoIter<Node<$h, D>> {
            self.harness_ref().find(self.scope(), &by).into_iter()
        }
        #[track_caller]
        pub fn get_all(&self, by: By<'_>) -> std::vec::IntoIter<Node<$h, D>> {
            let all = self.harness_ref().find(self.scope(), &by);
            if all.is_empty() {
                panic!("no node matches {by}\n{}", self.harness_ref().a11y_dump());
            }
            all.into_iter()
        }
        /// The one node matching `by`, if any; panics when several match.
        #[track_caller]
        pub fn query(&self, by: By<'_>) -> Option<Node<$h, D>> {
            let mut all = self.harness_ref().find(self.scope(), &by);
            match all.len() {
                0 => None,
                1 => all.pop(),
                n => panic!(
                    "{n} nodes match {by}; expected at most one (use query_all for several)\n{}",
                    self.harness_ref().a11y_dump()
                ),
            }
        }
        /// The one node matching `by`; panics when none or several match.
        #[track_caller]
        pub fn get(&self, by: By<'_>) -> Node<$h, D> {
            match self.query(by.clone()) {
                Some(n) => n,
                None => panic!("no node matches {by}\n{}", self.harness_ref().a11y_dump()),
            }
        }

        pub fn query_all_by_label(&self, label: &str) -> std::vec::IntoIter<Node<$h, D>> {
            self.query_all(By::new().label(label))
        }
        #[track_caller]
        pub fn get_all_by_label(&self, label: &str) -> std::vec::IntoIter<Node<$h, D>> {
            self.get_all(By::new().label(label))
        }
        #[track_caller]
        pub fn query_by_label(&self, label: &str) -> Option<Node<$h, D>> {
            self.query(By::new().label(label))
        }
        #[track_caller]
        pub fn get_by_label(&self, label: &str) -> Node<$h, D> {
            self.get(By::new().label(label))
        }

        pub fn query_all_by_label_contains(&self, label: &str) -> std::vec::IntoIter<Node<$h, D>> {
            self.query_all(By::new().label_contains(label))
        }
        #[track_caller]
        pub fn get_all_by_label_contains(&self, label: &str) -> std::vec::IntoIter<Node<$h, D>> {
            self.get_all(By::new().label_contains(label))
        }
        #[track_caller]
        pub fn query_by_label_contains(&self, label: &str) -> Option<Node<$h, D>> {
            self.query(By::new().label_contains(label))
        }
        #[track_caller]
        pub fn get_by_label_contains(&self, label: &str) -> Node<$h, D> {
            self.get(By::new().label_contains(label))
        }

        pub fn query_all_by_role_and_label(
            &self,
            role: Role,
            label: &str,
        ) -> std::vec::IntoIter<Node<$h, D>> {
            self.query_all(By::new().role(role).label(label))
        }
        #[track_caller]
        pub fn get_all_by_role_and_label(
            &self,
            role: Role,
            label: &str,
        ) -> std::vec::IntoIter<Node<$h, D>> {
            self.get_all(By::new().role(role).label(label))
        }
        #[track_caller]
        pub fn query_by_role_and_label(&self, role: Role, label: &str) -> Option<Node<$h, D>> {
            self.query(By::new().role(role).label(label))
        }
        #[track_caller]
        pub fn get_by_role_and_label(&self, role: Role, label: &str) -> Node<$h, D> {
            self.get(By::new().role(role).label(label))
        }

        pub fn query_all_by_role(&self, role: Role) -> std::vec::IntoIter<Node<$h, D>> {
            self.query_all(By::new().role(role))
        }
        #[track_caller]
        pub fn get_all_by_role(&self, role: Role) -> std::vec::IntoIter<Node<$h, D>> {
            self.get_all(By::new().role(role))
        }
        #[track_caller]
        pub fn query_by_role(&self, role: Role) -> Option<Node<$h, D>> {
            self.query(By::new().role(role))
        }
        #[track_caller]
        pub fn get_by_role(&self, role: Role) -> Node<$h, D> {
            self.get(By::new().role(role))
        }

        pub fn query_all_by_value(&self, value: &str) -> std::vec::IntoIter<Node<$h, D>> {
            self.query_all(By::new().value(value))
        }
        #[track_caller]
        pub fn get_all_by_value(&self, value: &str) -> std::vec::IntoIter<Node<$h, D>> {
            self.get_all(By::new().value(value))
        }
        #[track_caller]
        pub fn query_by_value(&self, value: &str) -> Option<Node<$h, D>> {
            self.query(By::new().value(value))
        }
        #[track_caller]
        pub fn get_by_value(&self, value: &str) -> Node<$h, D> {
            self.get(By::new().value(value))
        }

        /// By predicate over the node (kittest's `*_by`).
        pub fn query_all_by(
            &self,
            f: impl Fn(&AkNode<'_>) -> bool,
        ) -> std::vec::IntoIter<Node<$h, D>> {
            self.query_all(By::new().predicate(f))
        }
        #[track_caller]
        pub fn get_all_by(&self, f: impl Fn(&AkNode<'_>) -> bool) -> std::vec::IntoIter<Node<$h, D>> {
            self.get_all(By::new().predicate(f))
        }
        #[track_caller]
        pub fn query_by(&self, f: impl Fn(&AkNode<'_>) -> bool) -> Option<Node<$h, D>> {
            self.query(By::new().predicate(f))
        }
        #[track_caller]
        pub fn get_by(&self, f: impl Fn(&AkNode<'_>) -> bool) -> Node<$h, D> {
            self.get(By::new().predicate(f))
        }

        /// By author id (EM1), the stable locator agents and tests use.
        pub fn query_all_by_author_id(&self, id: &str) -> std::vec::IntoIter<Node<$h, D>> {
            self.query_all(By::new().author_id(id))
        }
        #[track_caller]
        pub fn query_by_author_id(&self, id: &str) -> Option<Node<$h, D>> {
            self.query(By::new().author_id(id))
        }
        #[track_caller]
        pub fn get_by_author_id(&self, id: &str) -> Node<$h, D> {
            self.get(By::new().author_id(id))
        }
    };
}

impl<D: DocumentApi + 'static> Harness<D> {
    fn harness_ref(&self) -> &Self {
        self
    }
    fn scope(&self) -> Option<NodeKey> {
        None
    }
    queries!('_);
}

/// A node of the snapshot, with kittest's actions. Actions queue input; the next `step`/`run`
/// delivers it.
pub struct Node<'h, D: DocumentApi + 'static> {
    harness: &'h Harness<D>,
    info: NodeInfo<'h>,
}

impl<D: DocumentApi + 'static> Clone for Node<'_, D> {
    fn clone(&self) -> Self {
        Node {
            harness: self.harness,
            info: self.info,
        }
    }
}

impl<D: DocumentApi + 'static> std::fmt::Debug for Node<'_, D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.accesskit_node().fmt(f)
    }
}

impl<'h, D: DocumentApi + 'static> Node<'h, D> {
    fn harness_ref(&self) -> &'h Harness<D> {
        self.harness
    }
    fn scope(&self) -> Option<NodeKey> {
        Some(self.info.key())
    }
    queries!('h);

    pub fn id(&self) -> NodeId {
        self.info.id
    }
    pub fn tree(&self) -> TreeId {
        self.info.tree
    }
    pub fn key(&self) -> NodeKey {
        self.info.key()
    }
    pub fn info(&self) -> &NodeInfo<'h> {
        &self.info
    }
    /// The node as kittest's `accesskit_node()` shows it.
    pub fn accesskit_node(&self) -> AkNode<'h> {
        AkNode {
            info: self.info,
            tree: &self.harness.a11y,
        }
    }
    /// The accessible name (the label, else the labelling nodes' text).
    pub fn label(&self) -> Option<String> {
        self.accesskit_node().label()
    }
    pub fn value(&self) -> Option<String> {
        self.info.value().map(str::to_owned)
    }
    pub fn role(&self) -> Role {
        self.info.role()
    }
    pub fn author_id(&self) -> Option<&'h str> {
        self.info.node.author_id()
    }
    pub fn toggled(&self) -> Option<skelter_base::accesskit::Toggled> {
        self.info.toggled()
    }
    pub fn is_disabled(&self) -> bool {
        self.info.is_disabled()
    }
    pub fn is_focused(&self) -> bool {
        self.harness.a11y.focus() == Some(self.info.key())
    }
    /// Window-space bounds (points).
    pub fn bounds(&self) -> Option<Rect> {
        self.info.bounds
    }
    /// Window-space bounds in points as an `egui::Rect` (kittest's `rect`); panics without bounds.
    #[track_caller]
    pub fn rect(&self) -> egui::Rect {
        let b = self.info.bounds.unwrap_or_else(|| {
            panic!("node {:?} ({:?}) has no bounds", self.info.id, self.label())
        });
        egui::Rect::from_min_max(egui::pos2(b.min.x, b.min.y), egui::pos2(b.max.x, b.max.y))
    }

    pub fn parent(&self) -> Option<Node<'h, D>> {
        self.harness.node_at(self.info.parent?)
    }
    /// Direct children (a graft node's: its subtree's root).
    pub fn children(&self) -> std::vec::IntoIter<Node<'h, D>> {
        self.query_all(By::new().recursive(false))
    }
    /// Every descendant, in tree order.
    pub fn children_recursive(&self) -> std::vec::IntoIter<Node<'h, D>> {
        self.query_all(By::new())
    }

    #[track_caller]
    fn center(&self) -> Point {
        self.info
            .center()
            .unwrap_or_else(|| panic!("node {:?} ({:?}) has no bounds", self.info.id, self.label()))
    }

    /// Move to the node's centre.
    #[track_caller]
    pub fn hover(&self) {
        self.harness.hover_at(self.center());
    }
    /// Move, press and release the primary button at the node's centre.
    #[track_caller]
    pub fn click(&self) {
        self.click_button(MouseButton::Primary);
    }
    #[track_caller]
    pub fn click_secondary(&self) {
        self.click_button(MouseButton::Secondary);
    }
    #[track_caller]
    pub fn click_button(&self, b: impl IntoButton) {
        self.harness
            .click_button_modifiers_at(self.center(), b, self.harness.modifiers);
    }
    /// Click with `m` held (they change before and are released after).
    #[track_caller]
    pub fn click_modifiers(&self, m: impl IntoModifiers) {
        self.click_button_modifiers(MouseButton::Primary, m);
    }
    #[track_caller]
    pub fn click_button_modifiers(&self, b: impl IntoButton, m: impl IntoModifiers) {
        self.harness.click_button_modifiers_at(self.center(), b, m);
    }

    /// An AccessKit action on this node.
    pub fn action(&self, action: Action) {
        self.action_with(action, None);
    }
    pub fn action_with(&self, action: Action, data: Option<ActionData>) {
        self.harness.accesskit_action(ActionRequest {
            action,
            target_tree: self.info.tree,
            target_node: self.info.id,
            data,
        });
    }
    /// The AccessKit `Click` action (no pointer).
    pub fn click_accesskit(&self) {
        self.action(Action::Click);
    }
    pub fn increment_accesskit(&self) {
        self.action(Action::Increment);
    }
    pub fn decrement_accesskit(&self) {
        self.action(Action::Decrement);
    }
    pub fn set_value_accesskit(&self, value: f64) {
        self.action_with(Action::SetValue, Some(ActionData::NumericValue(value)));
    }
    pub fn set_text_value_accesskit(&self, value: &str) {
        self.action_with(Action::SetValue, Some(ActionData::Value(value.into())));
    }
    pub fn replace_selected_text_accesskit(&self, text: &str) {
        self.action_with(Action::ReplaceSelectedText, Some(ActionData::Value(text.into())));
    }
    pub fn scroll_to_me(&self) {
        self.action(Action::ScrollIntoView);
    }
    pub fn scroll_down(&self) {
        self.action(Action::ScrollDown);
    }
    pub fn scroll_up(&self) {
        self.action(Action::ScrollUp);
    }
    pub fn scroll_left(&self) {
        self.action(Action::ScrollLeft);
    }
    pub fn scroll_right(&self) {
        self.action(Action::ScrollRight);
    }
    /// The AccessKit `Focus` action.
    pub fn focus(&self) {
        self.action(Action::Focus);
    }
    /// Type `text` into the focused node (kittest's: no focus change; call `focus` first).
    pub fn type_text(&self, text: &str) {
        self.harness.type_text(text);
    }
    /// Focus the node, then press `key`.
    pub fn key_press(&self, key: impl IntoKey) {
        self.focus();
        self.harness.key_press(key);
    }
}
