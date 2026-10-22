//! [`Viewports`]: a headless multi-viewport runner for tests (EM2, DOCK-W2/W4). It does what eframe
//! does with deferred viewports, without windows: the root viewport's ui is the test's (`App::logic`
//! + `App::ui`), every deferred viewport shown by it gets its own passes, and a viewport runs a pass
//! only when it asked for a repaint (`repaint_delay == 0`), got input, or just appeared. Passes are
//! counted per viewport, so a test can say "a change in window 2 repaints only window 2".
//!
//! Input goes to one viewport, one event per pass (as kittest does). Positions are in that
//! viewport's points; every viewport is `size` large at 1 point per pixel. Immediate viewports are
//! not supported (egui would draw them through an immediate renderer this runner does not set).
//!
//! Writer lane: EMBED.

use egui::{DeferredViewportUiCallback, Event, Pos2, RawInput, Rect, Vec2, ViewportClass, ViewportId, ViewportInfo};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

struct Vp {
    id: ViewportId,
    /// The deferred viewport's ui (`None` for the root viewport).
    cb: Option<Arc<DeferredViewportUiCallback>>,
    due: bool,
    /// The last `repaint_delay` egui reported for it.
    delay: Duration,
    events: VecDeque<Event>,
    passes: u64,
}

type RootUi = Box<dyn FnMut(&mut egui::Ui)>;

/// See the module doc.
pub struct Viewports {
    pub ctx: egui::Context,
    ui: RootUi,
    size: Vec2,
    vps: Vec<Vp>,
    time: f64,
    /// Passes run, all viewports.
    pub total: u64,
    asked: Asked,
}

type Asked = Arc<std::sync::Mutex<Vec<ViewportId>>>;

impl Viewports {
    /// A runner with a new context (multi-viewport: `embed_viewports` off). Set the root ui with
    /// [`Viewports::set_ui`] (after creating what it captures on `ctx`, e.g. an `Embed`).
    pub fn new(size: Vec2) -> Viewports {
        let ctx = egui::Context::default();
        ctx.set_embed_viewports(false);
        ctx.all_styles_mut(|s| {
            s.animation_time = 0.0;
            s.visuals.text_cursor.blink = false; // as kittest: no timer repaints from a caret
        });
        // repaint requests made between passes (a host op, a wake), as eframe gets them
        let asked: Asked = Arc::default();
        let a = asked.clone();
        ctx.set_request_repaint_callback(move |info| {
            if info.delay == Duration::ZERO {
                a.lock().unwrap().push(info.viewport_id);
            }
        });
        let root = Vp { id: ViewportId::ROOT, cb: None, due: true, delay: Duration::ZERO, events: VecDeque::new(), passes: 0 };
        Viewports { ctx, ui: Box::new(|_| {}), size, vps: vec![root], time: 0.0, total: 0, asked }
    }

    /// The root viewport's ui, run in each of its passes.
    pub fn set_ui(&mut self, ui: impl FnMut(&mut egui::Ui) + 'static) {
        self.ui = Box::new(ui);
        self.request(ViewportId::ROOT);
    }

    fn vp(&mut self, id: ViewportId) -> Option<&mut Vp> {
        self.vps.iter_mut().find(|v| v.id == id)
    }

    /// The viewports egui knows (the root one first, the deferred ones in order of appearance).
    pub fn ids(&self) -> Vec<ViewportId> {
        self.vps.iter().map(|v| v.id).collect()
    }

    /// Passes viewport `id` ran since the last [`Viewports::reset_passes`].
    pub fn passes(&self, id: ViewportId) -> u64 {
        self.vps.iter().find(|v| v.id == id).map_or(0, |v| v.passes)
    }

    pub fn reset_passes(&mut self) {
        for v in &mut self.vps {
            v.passes = 0;
        }
        self.total = 0;
    }

    /// Asks for a pass of `id`, as a window system would after an expose.
    pub fn request(&mut self, id: ViewportId) {
        if let Some(v) = self.vp(id) {
            v.due = true;
        }
    }

    /// Marks the viewports egui's repaint callback named since the last look.
    fn take_asked(&mut self) {
        let asked = std::mem::take(&mut *self.asked.lock().unwrap());
        for id in asked {
            if let Some(v) = self.vp(id) {
                v.due = true;
            }
        }
    }

    /// The viewports that will run a pass at the next [`Viewports::step`] (asked for a repaint now,
    /// or have input).
    pub fn due(&mut self) -> Vec<ViewportId> {
        self.take_asked();
        self.vps.iter().filter(|v| v.due || !v.events.is_empty()).map(|v| v.id).collect()
    }

    /// The viewports egui asked to repaint after a delay (timers): `(id, delay)`.
    pub fn delayed(&self) -> Vec<(ViewportId, Duration)> {
        self.vps.iter().filter(|v| !v.due && v.delay < Duration::MAX).map(|v| (v.id, v.delay)).collect()
    }

    /// Queues `ev` for viewport `id` (one event per pass).
    pub fn event(&mut self, id: ViewportId, ev: Event) {
        if let Some(v) = self.vp(id) {
            v.events.push_back(ev);
        }
    }

    /// A primary click at `pos` in viewport `id`: move, press, release (three passes).
    pub fn click(&mut self, id: ViewportId, pos: Pos2) {
        let b = egui::PointerButton::Primary;
        self.event(id, Event::PointerMoved(pos));
        self.event(id, Event::PointerButton { pos, button: b, pressed: true, modifiers: Default::default() });
        self.event(id, Event::PointerButton { pos, button: b, pressed: false, modifiers: Default::default() });
    }

    /// Text typed into viewport `id` (one pass).
    pub fn type_text(&mut self, id: ViewportId, text: &str) {
        self.event(id, Event::Text(text.to_string()));
    }

    /// Runs one pass of the first viewport that is due (root first). False: none is.
    pub fn step(&mut self) -> bool {
        self.take_asked();
        let Some(id) = self.vps.iter().find(|v| v.due || !v.events.is_empty()).map(|v| v.id) else { return false };
        self.run_pass(id);
        true
    }

    /// Steps until no viewport is due; returns the passes run. Panics after `max` passes (a
    /// viewport that never stops repainting).
    pub fn run(&mut self, max: u64) -> u64 {
        let mut n = 0;
        while self.step() {
            n += 1;
            assert!(n <= max, "Viewports::run: still repainting after {max} passes (due: {:?})", self.due());
        }
        n
    }

    /// Runs one pass of viewport `id` now (with its next queued event, if any).
    pub fn run_pass(&mut self, id: ViewportId) {
        let size = self.size;
        let Some(v) = self.vp(id) else { return };
        v.due = false;
        v.passes += 1;
        let ev = v.events.pop_front();
        let cb = v.cb.clone();
        self.total += 1;
        self.time += 1.0 / 60.0;
        let rect = Rect::from_min_size(Pos2::ZERO, size);
        let mut input = RawInput {
            viewport_id: id,
            screen_rect: Some(rect),
            time: Some(self.time),
            predicted_dt: 1.0 / 60.0,
            focused: true,
            events: ev.into_iter().collect(),
            ..RawInput::default()
        };
        input.viewports.insert(
            id,
            ViewportInfo { native_pixels_per_point: Some(1.0), inner_rect: Some(rect), focused: Some(true), ..ViewportInfo::default() },
        );
        // `SVELTERS_VIEWPORTS_LOG=1`: each pass and why it ran (egui's repaint causes)
        let log = std::env::var_os("SVELTERS_VIEWPORTS_LOG").is_some();
        let why = |u: &egui::Ui| {
            if log {
                eprintln!("[viewports] pass {:?} (event: {}) causes {:?}", u.ctx().viewport_id(), !u.ctx().input(|i| i.events.is_empty()), u.ctx().repaint_causes());
            }
        };
        let mut out = match cb {
            None => {
                let ui = &mut self.ui;
                self.ctx.run_ui(input, |u| {
                    why(u);
                    ui(u)
                })
            }
            Some(cb) => self.ctx.run_ui(input, |u| {
                why(u);
                cb(u)
            }),
        };
        out.textures_delta.clear(); // no renderer

        // what every viewport asked for, and the deferred viewports the root ui shows
        let mut alive = vec![ViewportId::ROOT];
        for (vid, vo) in out.viewport_output {
            alive.push(vid);
            let zero = vo.repaint_delay == Duration::ZERO;
            match self.vp(vid) {
                Some(v) => {
                    if vo.viewport_ui_cb.is_some() {
                        v.cb = vo.viewport_ui_cb;
                    }
                    v.delay = vo.repaint_delay;
                    v.due |= zero;
                }
                None if vo.class == ViewportClass::Deferred => self.vps.push(Vp {
                    id: vid,
                    cb: vo.viewport_ui_cb,
                    due: true, // a new window paints once
                    delay: vo.repaint_delay,
                    events: VecDeque::new(),
                    passes: 0,
                }),
                None => {}
            }
        }
        // a deferred viewport the root ui stopped showing is gone (egui drops it at the end of the
        // root's pass)
        if id == ViewportId::ROOT {
            self.vps.retain(|v| alive.contains(&v.id));
        }
    }
}
