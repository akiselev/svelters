//! [`Harness`]: kittest's verbs on presto (architecture §16.1). It runs a real [`presto::App`] on
//! `presto_platform::HeadlessPlatform` and the [`HeadlessCompositor`], on the platform's virtual clock.
//! Input is scripted into the headless platform, which delivers it as an OS backend would
//! (`AppHandler::input`, `a11y_event`), so tests take the production dispatch path; queries read the
//! AccessKit snapshot folded from the app's `update_a11y` calls.
//!
//! - `step` = one vsync on the virtual clock: queued input is delivered, the clock moves one interval,
//!   the compositor applies commits and presents, the UI answers `BeginFrame`.
//! - `run` = step until settled (nothing queued, no frame wanted, no commit in flight, no animation
//!   running), failing after `max_steps`.

use crate::a11y::{A11yTree, By, NodeInfo};
use crate::egui_event::{EguiAdapter, Injected};
use crate::headless::HeadlessCompositor;
use crate::render::{GpuRender, Image, RenderError};
use presto::platform::{
    A11yEvent, ClipboardKind, HeadlessPlatform, PlatformCall, PlatformCx, Scripted, VirtualClock,
    WindowDesc,
};
use presto::seams::DocumentApi;
use presto::{App, AppConfig};
use presto_base::accesskit::{Action, ActionRequest, NodeId, Role, TreeId};
use presto_base::color::Color;
use presto_base::event::{
    ImeEvent, InputEvent, KeyEvent, KeyLocation, KeyState, Modifiers, MouseButton, PointerEvent,
    PointerPhase, PointerType, ScrollDelta, WheelEvent, WindowEvent,
};
use presto_base::geom::{DeviceSize, Point, Rect, Scale, Size, Vec2};
use presto_base::ids::WindowId;
use presto_base::key::Key;
use presto_base::smallvec::SmallVec;
use presto_base::text::GlyphRasterizer;
use presto_base::time::{Clock, Span, Time};
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

pub struct HarnessBuilder {
    size: Size,
    scale: f32,
    interval: Span,
    max_steps: u64,
    start: Time,
    config: AppConfig,
    glyphs: Option<Arc<dyn GlyphRasterizer>>,
    clear: Color,
}

impl Default for HarnessBuilder {
    fn default() -> Self {
        Self {
            size: Size::new(800.0, 600.0),
            scale: 1.0,
            interval: Span::from_micros(16_667),
            max_steps: 64,
            start: Time(1_000_000_000),
            config: AppConfig::default(),
            glyphs: None,
            clear: Color::WHITE,
        }
    }
}

impl HarnessBuilder {
    pub fn new() -> Self {
        Self::default()
    }
    /// The window's inner size in points.
    pub fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
    pub fn with_scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }
    /// The simulated display interval (one `step`).
    pub fn with_interval(mut self, interval: Span) -> Self {
        self.interval = interval;
        self
    }
    pub fn with_max_steps(mut self, n: u64) -> Self {
        self.max_steps = n;
        self
    }
    /// The glyph rasteriser `render` draws text with (default: presto-text over its bundled test
    /// fonts, `FontConfig::testing()`).
    pub fn with_glyphs(mut self, glyphs: Arc<dyn GlyphRasterizer>) -> Self {
        self.glyphs = Some(glyphs);
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

    /// Build the harness around `doc` and run it until settled.
    #[track_caller]
    pub fn build<D: DocumentApi + 'static>(self, doc: D) -> Harness<D> {
        let mut h = self.build_unsettled(doc);
        h.run();
        h
    }

    /// Build without running (to assert on the first frames).
    pub fn build_unsettled<D: DocumentApi + 'static>(self, doc: D) -> Harness<D> {
        let mut platform = HeadlessPlatform::new();
        platform.cx().default_scale = Scale(self.scale);
        let clock = platform.virtual_clock();
        clock.set(self.start);
        let mut config = self.config;
        config.windows = vec![WindowDesc::new("presto-test", self.size)];
        let slot: Rc<RefCell<Option<HeadlessCompositor>>> = Rc::default();
        let keep = slot.clone();
        let spawn = Box::new(move |sink, _clock| {
            let c = HeadlessCompositor::new(sink);
            *keep.borrow_mut() = Some(c.clone());
            c
        });
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
            egui: EguiAdapter::default(),
            a11y: A11yTree::default(),
            calls: Vec::new(),
            steps: 0,
            glyphs: self.glyphs,
            clear: self.clear,
            gpu: None,
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
    egui: EguiAdapter,
    a11y: A11yTree,
    /// Every platform call the app made, in order.
    calls: Vec<PlatformCall>,
    steps: u64,
    glyphs: Option<Arc<dyn GlyphRasterizer>>,
    clear: Color,
    /// Opened by the first `render`.
    gpu: Option<GpuRender>,
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
    /// Every platform call the app made so far (windows, cursors, IME, a11y updates, exit).
    pub fn calls(&self) -> &[PlatformCall] {
        &self.calls
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
    pub fn steps(&self) -> u64 {
        self.steps
    }
    pub fn a11y(&self) -> &A11yTree {
        &self.a11y
    }

    // ---- running --------------------------------------------------------------------------------

    /// One vsync on the virtual clock.
    pub fn step(&mut self) {
        self.deliver();
        self.clock.advance(self.interval);
        let now = self.now();
        self.compositor.vsync(now, self.interval);
        // The vsync's messages woke the UI thread; the platform delivers the wake.
        self.platform.run_until(&mut self.app, Some(now));
        self.collect_calls();
        self.steps += 1;
    }

    fn collect_calls(&mut self) {
        for c in self.platform.take_calls() {
            if let PlatformCall::UpdateA11y(w, u) = &c
                && *w == self.window
            {
                self.a11y.apply(u);
            }
            self.calls.push(c);
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
                }
            }
        }
        self.platform.run_until(&mut self.app, Some(now));
        self.collect_calls();
    }

    /// The window as the compositor has it now (every applied commit), drawn by presto-render on the
    /// environment's wgpu adapter. Opens the device on first use; `Err` when there is none.
    pub fn render(&mut self) -> Result<Image, RenderError> {
        if self.gpu.is_none() {
            let glyphs = match &self.glyphs {
                Some(g) => g.clone(),
                None => {
                    let fonts = presto_text::FontStore::new(presto_text::FontConfig::testing());
                    Arc::new(presto_text::FontRasterizer::new(fonts)) as Arc<dyn GlyphRasterizer>
                }
            };
            self.gpu = Some(GpuRender::new(glyphs)?);
        }
        let gpu = self.gpu.as_mut().expect("opened above");
        self.compositor.render(self.window, gpu, self.clear)
    }

    /// Why the harness is not settled, or `None` when it is (or the app exited).
    pub fn busy(&self) -> Option<String> {
        if self.exited() {
            return None;
        }
        if !self.queue.borrow().is_empty() {
            return Some("queued input".into());
        }
        if !self.app.is_idle() {
            return Some("the app has a frame to run".into());
        }
        if !self.compositor.is_idle() {
            let endless = self.compositor.endless_animations();
            if !endless.is_empty() {
                return Some(format!(
                    "animations {endless:?} have no end the headless compositor can compute (physical springs need presto-anim)"
                ));
            }
            return Some("the compositor has a commit, frame or animation in flight".into());
        }
        None
    }

    /// Step until settled; `Err` after `max_steps`.
    pub fn try_run(&mut self) -> Result<u64, NotSettled> {
        self.run_until_settled(self.max_steps)
    }

    /// Step until settled, at most `max` steps. Always steps at least once.
    pub fn run_until_settled(&mut self, max: u64) -> Result<u64, NotSettled> {
        let mut n = 0;
        loop {
            self.step();
            n += 1;
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

    // ---- input ----------------------------------------------------------------------------------

    /// Queue a presto input event for the harness window (delivered by the next `step`).
    pub fn event(&self, ev: InputEvent) {
        self.queue.borrow_mut().push(Queued::Input(self.window, ev));
    }

    /// Queue an egui event, translated as the platform would deliver it.
    pub fn event_egui(&mut self, ev: egui::Event) {
        for i in self.egui.translate(&ev) {
            let q = match i {
                Injected::Input(e) => Queued::Input(self.window, e),
                Injected::A11y(r) => Queued::A11y(self.window, r),
                Injected::Focused(f) => Queued::Window(self.window, WindowEvent::Focused(f)),
                Injected::SetClipboard(t) => Queued::Clipboard(t),
            };
            self.queue.borrow_mut().push(q);
        }
    }

    pub fn window_event(&self, ev: WindowEvent) {
        self.queue
            .borrow_mut()
            .push(Queued::Window(self.window, ev));
    }

    /// Resize the window (points), through the resize handshake.
    pub fn set_size(&self, size: Size) {
        let scale = self.app.window_size(self.window).map_or(1.0, |(_, s)| s.0);
        let device = DeviceSize {
            w: (size.w * scale).round() as u32,
            h: (size.h * scale).round() as u32,
        };
        self.window_event(WindowEvent::Resized {
            size: device,
            scale: Scale(scale),
        });
    }

    /// A native menu item chosen now (`AppHandler::menu_activated`), as the macOS backend reports it.
    pub fn menu_activated(&mut self, id: &str) {
        self.deliver();
        presto::platform::AppHandler::menu_activated(&mut self.app, self.platform.cx(), id);
        self.collect_calls();
    }

    /// Modifiers held for the following `key_press`, `click` and wheel input.
    pub fn set_modifiers(&mut self, m: Modifiers) {
        self.modifiers = m;
        self.event(InputEvent::ModifiersChanged(m));
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

    pub fn key_down(&self, key: Key) {
        self.key(key, KeyState::Pressed, self.modifiers);
    }
    pub fn key_up(&self, key: Key) {
        self.key(key, KeyState::Released, self.modifiers);
    }
    /// Press and release `key` with the held modifiers.
    pub fn key_press(&self, key: Key) {
        self.key_press_modifiers(self.modifiers, key);
    }
    pub fn key_press_modifiers(&self, m: Modifiers, key: Key) {
        self.key(key, KeyState::Pressed, m);
        self.key(key, KeyState::Released, m);
    }
    /// Type text into the focused node (an IME commit, as egui's `Event::Text`).
    pub fn type_text(&self, text: &str) {
        self.event(InputEvent::Ime(ImeEvent::Commit(text.to_owned())));
    }

    fn pointer(
        &self,
        phase: PointerPhase,
        pos: Point,
        button: Option<MouseButton>,
        buttons: u32,
    ) -> InputEvent {
        InputEvent::Pointer(PointerEvent {
            phase,
            pointer: 0,
            kind: PointerType::Mouse,
            button,
            buttons,
            pos,
            modifiers: self.modifiers,
            pressure: if buttons != 0 { 0.5 } else { 0.0 },
            tilt: Vec2::ZERO,
            history: SmallVec::new(),
        })
    }

    pub fn hover_at(&self, pos: Point) {
        self.event(self.pointer(PointerPhase::Move, pos, None, 0));
    }
    /// Move, press and release the primary button at `pos`.
    pub fn click_at(&self, pos: Point) {
        self.click_button_at(pos, MouseButton::Primary);
    }
    pub fn click_button_at(&self, pos: Point, b: MouseButton) {
        let bit = match b {
            MouseButton::Primary => 1,
            MouseButton::Secondary => 2,
            MouseButton::Middle => 4,
            _ => 0,
        };
        self.event(self.pointer(PointerPhase::Move, pos, None, 0));
        self.event(self.pointer(PointerPhase::Down, pos, Some(b), bit));
        self.event(self.pointer(PointerPhase::Up, pos, Some(b), 0));
    }
    /// A primary-button drag from `from` to `to` in `steps` moves.
    pub fn drag(&self, from: Point, to: Point, steps: u32) {
        self.event(self.pointer(PointerPhase::Move, from, None, 0));
        self.event(self.pointer(PointerPhase::Down, from, Some(MouseButton::Primary), 1));
        let n = steps.max(1);
        for i in 1..=n {
            let t = i as f32 / n as f32;
            let p = Point::new(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t);
            self.event(self.pointer(PointerPhase::Move, p, None, 1));
        }
        self.event(self.pointer(PointerPhase::Up, to, Some(MouseButton::Primary), 0));
    }
    pub fn wheel_at(&self, pos: Point, delta: ScrollDelta) {
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

    pub fn query_all(&self, by: By<'_>) -> Vec<Node<'_, D>> {
        self.a11y
            .walk()
            .into_iter()
            .filter(|n| by.matches(n))
            .map(|n| self.node_ref(n))
            .collect()
    }

    /// The one node matching `by`, if any; panics when several match.
    #[track_caller]
    pub fn query(&self, by: By<'_>) -> Option<Node<'_, D>> {
        let mut all = self.query_all(by.clone());
        match all.len() {
            0 => None,
            1 => all.pop(),
            n => panic!("{n} nodes match {by}; expected at most one"),
        }
    }

    /// The one node matching `by`; panics when none or several match (kittest's `get_by`).
    #[track_caller]
    pub fn get(&self, by: By<'_>) -> Node<'_, D> {
        match self.query(by.clone()) {
            Some(n) => n,
            None => panic!("no node matches {by}\n{}", self.a11y_dump()),
        }
    }

    #[track_caller]
    pub fn get_by_label(&self, label: &str) -> Node<'_, D> {
        self.get(By::new().label(label))
    }
    #[track_caller]
    pub fn query_by_label(&self, label: &str) -> Option<Node<'_, D>> {
        self.query(By::new().label(label))
    }
    pub fn get_all_by_label(&self, label: &str) -> Vec<Node<'_, D>> {
        self.query_all(By::new().label(label))
    }
    #[track_caller]
    pub fn get_by_label_contains(&self, label: &str) -> Node<'_, D> {
        self.get(By::new().label_contains(label))
    }
    #[track_caller]
    pub fn get_by_role(&self, role: Role) -> Node<'_, D> {
        self.get(By::new().role(role))
    }
    #[track_caller]
    pub fn query_by_role(&self, role: Role) -> Option<Node<'_, D>> {
        self.query(By::new().role(role))
    }
    #[track_caller]
    pub fn get_by_role_and_label(&self, role: Role, label: &str) -> Node<'_, D> {
        self.get(By::new().role(role).label(label))
    }
    #[track_caller]
    pub fn query_by_role_and_label(&self, role: Role, label: &str) -> Option<Node<'_, D>> {
        self.query(By::new().role(role).label(label))
    }
    #[track_caller]
    pub fn get_by_value(&self, value: &str) -> Node<'_, D> {
        self.get(By::new().value(value))
    }
    #[track_caller]
    pub fn query_by_value(&self, value: &str) -> Option<Node<'_, D>> {
        self.query(By::new().value(value))
    }
    /// By author id (EM1), the stable locator agents and tests use.
    #[track_caller]
    pub fn get_by_author_id(&self, id: &str) -> Node<'_, D> {
        self.get(By::new().author_id(id))
    }
    #[track_caller]
    pub fn query_by_author_id(&self, id: &str) -> Option<Node<'_, D>> {
        self.query(By::new().author_id(id))
    }

    /// The focused node, when focus is on something other than the root.
    pub fn focused(&self) -> Option<Node<'_, D>> {
        let f = self.a11y.focus()?;
        if Some(f) == self.a11y.root() {
            return None;
        }
        self.a11y
            .walk()
            .into_iter()
            .find(|n| n.id == f)
            .map(|n| self.node_ref(n))
    }

    /// The snapshot as an indented outline (role, label, value, author id, bounds), for failure messages.
    pub fn a11y_dump(&self) -> String {
        let walk = self.a11y.walk();
        let mut depth = std::collections::HashMap::<NodeId, usize>::new();
        let mut out = String::new();
        for n in &walk {
            let d = n
                .parent
                .and_then(|p| depth.get(&p).map(|d| d + 1))
                .unwrap_or(0);
            depth.insert(n.id, d);
            out.push_str(&"  ".repeat(d));
            out.push_str(&format!("{:?}", n.role()));
            if let Some(l) = n.label() {
                out.push_str(&format!(" {l:?}"));
            }
            if let Some(v) = n.value() {
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
}

/// A node of the snapshot, with kittest's actions. Actions queue input; the next `step`/`run`
/// delivers it.
pub struct Node<'h, D: DocumentApi + 'static> {
    harness: &'h Harness<D>,
    info: NodeInfo<'h>,
}

impl<D: DocumentApi + 'static> Node<'_, D> {
    pub fn id(&self) -> NodeId {
        self.info.id
    }
    pub fn info(&self) -> &NodeInfo<'_> {
        &self.info
    }
    pub fn label(&self) -> Option<&str> {
        self.info.label()
    }
    pub fn value(&self) -> Option<&str> {
        self.info.value()
    }
    pub fn role(&self) -> Role {
        self.info.role()
    }
    pub fn author_id(&self) -> Option<&str> {
        self.info.author_id()
    }
    pub fn toggled(&self) -> Option<presto_base::accesskit::Toggled> {
        self.info.toggled()
    }
    pub fn is_disabled(&self) -> bool {
        self.info.is_disabled()
    }
    pub fn is_focused(&self) -> bool {
        self.harness.a11y.focus() == Some(self.info.id)
    }
    /// Window-space bounds (points).
    pub fn rect(&self) -> Option<Rect> {
        self.info.bounds
    }

    #[track_caller]
    fn center(&self) -> Point {
        self.info
            .center()
            .unwrap_or_else(|| panic!("node {:?} ({:?}) has no bounds", self.info.id, self.label()))
    }

    /// Move, press and release the primary button at the node's centre.
    #[track_caller]
    pub fn click(&self) {
        self.harness.click_at(self.center());
    }
    #[track_caller]
    pub fn click_secondary(&self) {
        self.harness
            .click_button_at(self.center(), MouseButton::Secondary);
    }
    #[track_caller]
    pub fn hover(&self) {
        self.harness.hover_at(self.center());
    }
    /// An AccessKit action on this node.
    pub fn action(&self, action: Action) {
        let req = ActionRequest {
            action,
            target_tree: TreeId::ROOT,
            target_node: self.info.id,
            data: None,
        };
        self.harness
            .queue
            .borrow_mut()
            .push(Queued::A11y(self.harness.window, req));
    }
    /// The AccessKit `Click` action (no pointer).
    pub fn click_accesskit(&self) {
        self.action(Action::Click);
    }
    pub fn focus(&self) {
        self.action(Action::Focus);
    }
    /// Focus the node, then type `text` into it.
    pub fn type_text(&self, text: &str) {
        self.focus();
        self.harness.type_text(text);
    }
    pub fn key_press(&self, key: Key) {
        self.focus();
        self.harness.key_press(key);
    }
}
