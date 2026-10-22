//! [`EguiBackend`]: the `svl_backend` over the scene, and the frame driver all runners share.
//!
//! The frame (plans/research/EGUI.md §1.2):
//!
//! ```text
//! before the pass  native closes (sync close-request), checkpoint (wake / timer / queued events),
//!                  frame (rAF), layout if dirty
//! the pass         walk the visible nodes → egui widgets at their rects; responses and widget
//!                  changes go to an outbox (no host call while the scene is borrowed)
//! after the pass   flush the outbox (post_event / prop_changed), repaint only if something is pending
//! idle             nothing requested → no frame → 0% CPU
//! ```
//!
//! JS never runs during the walk: `checkpoint` and `call_sync` are only called before it, and the
//! `pass_active` flag turns a violation into a panic.

use crate::kinds::{self, KINDS};
use crate::layout;
use crate::render::{self, Out};
use crate::scene::{self, Arena, Handle, W};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use svelters_abi::*;

/// The thread-safe half: `wake` may come from any thread.
#[derive(Default)]
pub(crate) struct Shared {
    pub wake: AtomicBool,
    /// Frames drawn, readable from any thread (idle measurements).
    pub frames: AtomicU64,
    pub ctx: Mutex<Option<egui::Context>>,
}

/// Counters for tests and measurements.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Frames the backend drew (first passes).
    pub frames: u64,
    /// Checkpoints it ran.
    pub checkpoints: u64,
    /// Layout runs, and leaves measured by them.
    pub layouts: u64,
    pub measured: u64,
    /// Events and prop changes delivered to the host.
    pub events: u64,
    pub changes: u64,
}

pub(crate) struct Inner {
    pub scene: RefCell<Arena>,
    host: Cell<*const svl_host>,
    pub shared: Arc<Shared>,
    deadline: Cell<Option<Instant>>,
    frame_pending: Cell<bool>,
    /// Events or prop changes were posted: the host needs a checkpoint.
    checkpoint_pending: Cell<bool>,
    in_frame: Cell<bool>,
    pass_active: Cell<bool>,
    first_frame_done: Cell<bool>,
    quitting: Cell<bool>,
    /// Presented windows in presentation order (the first live one is the root viewport).
    windows: RefCell<Vec<Handle>>,
    /// Leaves whose content changed since the last layout.
    remeasure: RefCell<Vec<Handle>>,
    /// Some window's layout must run again (style or structure changed).
    layout_dirty: Cell<bool>,
    /// Content size of each window's last layout.
    layout_size: RefCell<std::collections::HashMap<Handle, egui::Vec2>>,
    /// The user asked to close these windows (seen during a pass); decided before the next one.
    close_requests: RefCell<Vec<Handle>>,
    /// `close` was called on these windows.
    closes: RefCell<Vec<Handle>>,
    /// Title last sent per window viewport.
    titles: RefCell<std::collections::HashMap<Handle, String>>,
    stats: Cell<Stats>,
}

/// The egui backend. Cheap to clone (one shared state); the clone given to [`BackendVtable::new`]
/// and the ones kept by runners see the same scene.
#[derive(Clone)]
pub struct EguiBackend(pub(crate) Rc<Inner>);

impl Default for EguiBackend {
    fn default() -> Self {
        EguiBackend::new()
    }
}

impl EguiBackend {
    pub fn new() -> EguiBackend {
        EguiBackend(Rc::new(Inner {
            scene: RefCell::new(Arena::default()),
            host: Cell::new(std::ptr::null()),
            shared: Arc::new(Shared::default()),
            deadline: Cell::new(None),
            frame_pending: Cell::new(false),
            checkpoint_pending: Cell::new(false),
            in_frame: Cell::new(false),
            pass_active: Cell::new(false),
            first_frame_done: Cell::new(false),
            quitting: Cell::new(false),
            windows: RefCell::new(Vec::new()),
            remeasure: RefCell::new(Vec::new()),
            layout_dirty: Cell::new(true),
            layout_size: RefCell::new(Default::default()),
            close_requests: RefCell::new(Vec::new()),
            closes: RefCell::new(Vec::new()),
            titles: RefCell::new(Default::default()),
            stats: Cell::new(Stats::default()),
        }))
    }

    /// The `svl_backend` vtable over this backend (it keeps a clone).
    pub fn vtable(&self) -> BackendVtable {
        BackendVtable::new(self.clone())
    }

    /// A thread-safe counter of the frames drawn (for idle measurements from another thread).
    pub fn frame_counter(&self) -> Arc<dyn Fn() -> u64 + Send + Sync> {
        let shared = self.0.shared.clone();
        Arc::new(move || shared.frames.load(Ordering::Relaxed))
    }

    pub fn stats(&self) -> Stats {
        self.0.stats.get()
    }

    /// True once the host asked the app to quit (`quit`); runners that own no OS window (kittest,
    /// an embedding app) read it.
    pub fn quit_requested(&self) -> bool {
        self.0.quitting.get()
    }

    /// The window shown in the root viewport: the first presented one still alive.
    pub fn root_window(&self) -> Option<Handle> {
        let scene = self.0.scene.borrow();
        self.0.windows.borrow().iter().copied().find(|&w| scene.contains(w))
    }

    /// Reads the scene (tests, devtools).
    pub fn with_scene<R>(&self, f: impl FnOnce(&Arena) -> R) -> R {
        f(&self.0.scene.borrow())
    }

    /// A prop of a live node by name, as the host would read it.
    pub fn prop(&self, h: Handle, name: &str) -> Result<Value, String> {
        let scene = self.0.scene.borrow();
        let n = scene.get(h).ok_or_else(|| format!("stale or foreign handle {h}"))?;
        let (_, def) = kinds::find_prop(n.kind, name).ok_or_else(|| format!("<{}> has no prop {name}", n.tag()))?;
        scene::get_prop(n, def)
    }

    /// The live handles of kind `tag` in creation order of their slots (tests).
    pub fn handles_of(&self, tag: &str) -> Vec<Handle> {
        let scene = self.0.scene.borrow();
        let mut v: Vec<Handle> = scene.handles().into_iter().filter(|&h| scene.n(h).tag() == tag).collect();
        v.sort();
        v
    }

    fn host(&self) -> Option<HostRef<'static>> {
        let p = self.0.host.get();
        // SAFETY: the host attached through `run`/`start` outlives its attachment (it detaches
        // with `start(NULL)` before it is dropped).
        (!p.is_null()).then(|| unsafe { HostRef::from_raw(p) })
    }

    fn ctx(&self) -> Option<egui::Context> {
        self.0.shared.ctx.lock().unwrap().clone()
    }

    /// Something changed outside a frame: draw again.
    fn touched(&self) {
        if !self.0.in_frame.get()
            && let Some(ctx) = self.ctx()
        {
            ctx.request_repaint();
        }
    }

    fn assert_no_pass(&self, what: &str) {
        assert!(!self.0.pass_active.get(), "svelters-egui: {what} during the egui pass (JS must not run while the scene is drawn)");
    }

    fn bump(&self, f: impl FnOnce(&mut Stats)) {
        let mut s = self.0.stats.get();
        f(&mut s);
        self.0.stats.set(s);
    }

    fn checkpoint(&self) {
        self.assert_no_pass("checkpoint");
        if let Some(host) = self.host() {
            self.bump(|s| s.checkpoints += 1);
            host.checkpoint();
        }
    }

    // ---- TR1 visual channels ----

    fn visual_op(&self, op: u32, h: svl_handle, text: Option<&str>) -> StrResult<()> {
        fn nums<const N: usize>(text: &str, what: &str) -> StrResult<[f32; N]> {
            let v: Vec<f32> = text.split_whitespace().map(str::parse::<f32>).collect::<Result<_, _>>().map_err(|_| format!("{what}: '{text}' is not {N} numbers"))?;
            v.try_into().map_err(|_| format!("{what}: '{text}' is not {N} numbers").into())
        }
        let mut scene = self.0.scene.borrow_mut();
        let n = scene.get_mut(h).ok_or_else(|| format!("stale or foreign handle {h}"))?;
        let before = n.vis;
        match op {
            SVL_STYLE_OPACITY => {
                n.vis.opacity = match text {
                    None => None,
                    Some(t) => {
                        let t = t.trim();
                        let a = match t.strip_suffix('%') {
                            Some(p) => p.parse::<f32>().map(|x| x / 100.0),
                            None => t.parse::<f32>(),
                        }
                        .map_err(|_| format!("opacity: '{t}' is not a number"))?;
                        (a < 1.0).then_some(a.max(0.0))
                    }
                }
            }
            SVL_STYLE_TRANSFORM => n.vis.transform = text.map(|t| nums::<6>(t, "transform")).transpose()?,
            SVL_STYLE_CLIP => n.vis.clip = text.map(|t| nums::<2>(t, "clip")).transpose()?,
            _ => n.vis.inert = text.is_some(),
        }
        if n.vis == before {
            return Ok(());
        }
        if n.vis.clip != before.clip {
            layout::restyle(n);
            layout::invalidate(&mut scene, h);
            self.0.layout_dirty.set(true);
        }
        drop(scene);
        self.touched();
        Ok(())
    }

    /// Runs a pending layout of `h`'s window now (`bounds`), when the window has been laid out
    /// before (its size is known) and egui's fonts are available.
    fn layout_now(&self, h: Handle) {
        let Some(ctx) = self.ctx() else { return };
        let mut scene = self.0.scene.borrow_mut();
        let mut root = h;
        while let Some(p) = scene.get(root).and_then(|n| n.parent) {
            root = p;
        }
        let Some(size) = self.0.layout_size.borrow().get(&root).copied() else { return };
        let m = layout::Metrics::new(&ctx);
        let leaves: Vec<Handle> = std::mem::take(&mut *self.0.remeasure.borrow_mut());
        if !leaves.is_empty() && layout::remeasure(&mut scene, &m, &leaves) {
            self.0.layout_dirty.set(true);
        }
        if !self.0.layout_dirty.get() && !scene.n(root).cache.is_empty() {
            return;
        }
        let measured = layout::compute(&mut scene, &m, root, size);
        self.bump(|s| {
            s.layouts += 1;
            s.measured += measured as u64;
        });
        // other windows may be dirty too: the flag stays for the next pass (cached results make
        // the repeat cheap)
    }

    // ---- the frame ----

    /// One frame into `ui` (the root viewport's `Ui` in eframe and kittest, or a `Ui` of an
    /// embedding app): before the pass, the walk, after the pass. Call it every egui pass.
    pub fn ui(&self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        {
            let mut slot = self.0.shared.ctx.lock().unwrap();
            if slot.is_none() {
                *slot = Some(ctx.clone());
            }
        }
        self.0.in_frame.set(true);
        if ctx.current_pass_index() == 0 {
            self.bump(|s| s.frames += 1);
            self.0.shared.frames.fetch_add(1, Ordering::Relaxed);
            self.before_pass(&ctx);
        }
        let out = self.walk(ui);
        self.after_pass(&ctx, out);
        self.0.in_frame.set(false);
    }

    fn before_pass(&self, ctx: &egui::Context) {
        // Native closes: user requests (sync close-request decides) and `close` calls.
        let requests: Vec<Handle> = std::mem::take(&mut *self.0.close_requests.borrow_mut());
        for w in requests {
            if !self.0.scene.borrow().contains(w) {
                continue;
            }
            if !self.close_vetoed(w) {
                self.native_close(w);
            }
        }
        let closes: Vec<Handle> = std::mem::take(&mut *self.0.closes.borrow_mut());
        for w in closes {
            self.native_close(w);
        }
        let due = self.0.deadline.get().is_some_and(|d| Instant::now() >= d);
        if due {
            self.0.deadline.set(None);
        }
        let wake = self.0.shared.wake.swap(false, Ordering::AcqRel);
        let first = !self.0.first_frame_done.replace(true);
        if first || wake || due || self.0.checkpoint_pending.replace(false) {
            self.checkpoint();
        }
        if self.0.frame_pending.replace(false)
            && let Some(host) = self.host()
        {
            host.frame();
        }
        // Remeasure leaves whose text changed; only a size change dirties the layout.
        let leaves: Vec<Handle> = std::mem::take(&mut *self.0.remeasure.borrow_mut());
        if !leaves.is_empty() {
            let m = layout::Metrics::new(ctx);
            if layout::remeasure(&mut self.0.scene.borrow_mut(), &m, &leaves) {
                self.0.layout_dirty.set(true);
            }
        }
    }

    /// Runs the sync `close-request` of window `w`; true if a handler kept the window open.
    fn close_vetoed(&self, w: Handle) -> bool {
        let sig = {
            let scene = self.0.scene.borrow();
            let n = scene.n(w);
            let sig = kinds::signal_id(n.kind, "close-request");
            if !n.listens(sig) {
                return false;
            }
            sig
        };
        let Some(host) = self.host() else { return false };
        self.assert_no_pass("call_sync");
        matches!(host.call_sync(w, sig, &[]), Ok(Value::Bool(true)))
    }

    /// The window closes as if the user closed it: `SVL_GONE_CLOSED` for it, `SVL_GONE_PARENT`
    /// for every descendant (pre-order), all freed first.
    fn native_close(&self, w: Handle) {
        let mut dead = Vec::new();
        {
            let mut scene = self.0.scene.borrow_mut();
            if !scene.contains(w) {
                return;
            }
            fn collect(s: &Arena, h: Handle, out: &mut Vec<Handle>) {
                out.push(h);
                for &c in &s.n(h).children {
                    collect(s, c, out);
                }
            }
            collect(&scene, w, &mut dead);
            for &h in &dead {
                scene.free(h);
            }
        }
        self.0.windows.borrow_mut().retain(|&x| x != w);
        self.0.layout_size.borrow_mut().remove(&w);
        self.0.titles.borrow_mut().remove(&w);
        if let Some(host) = self.host() {
            for (i, &h) in dead.iter().enumerate() {
                host.on_destroyed(h, if i == 0 { SVL_GONE_CLOSED } else { SVL_GONE_PARENT });
            }
        }
    }

    fn walk(&self, ui: &mut egui::Ui) -> Vec<Out> {
        let ctx = ui.ctx().clone();
        let mut out = Vec::new();
        let windows: Vec<Handle> = {
            let scene = self.0.scene.borrow();
            self.0.windows.borrow().iter().copied().filter(|&w| scene.contains(w)).collect()
        };
        self.0.pass_active.set(true);
        for (i, &w) in windows.iter().enumerate() {
            if i == 0 {
                self.window_chrome(&ctx, w, egui::ViewportId::ROOT);
                if ctx.input(|inp| inp.viewport().close_requested()) {
                    self.on_close_requested(&ctx, egui::ViewportId::ROOT, w);
                }
                egui::CentralPanel::default()
                    .frame(egui::Frame::central_panel(ui.style()).inner_margin(0.0))
                    .show(ui, |ui| self.window_ui(ui, w, &mut out));
            } else {
                let vid = egui::ViewportId::from_hash_of(("svl-window", w));
                let (title, size) = self.window_props(w);
                let builder = egui::ViewportBuilder::default().with_title(title).with_inner_size(size);
                ctx.show_viewport_immediate(vid, builder, |ui, _class| {
                    if ui.ctx().input(|inp| inp.viewport().close_requested()) {
                        self.on_close_requested(ui.ctx(), vid, w);
                    }
                    egui::CentralPanel::default()
                        .frame(egui::Frame::central_panel(ui.style()).inner_margin(0.0))
                        .show(ui, |ui| self.window_ui(ui, w, &mut out));
                });
            }
        }
        self.0.pass_active.set(false);
        out
    }

    fn window_props(&self, w: Handle) -> (String, egui::Vec2) {
        let scene = self.0.scene.borrow();
        match &scene.n(w).w {
            W::Window { title, default_w, default_h } => (title.clone(), egui::vec2(*default_w as f32, *default_h as f32)),
            _ => unreachable!(),
        }
    }

    /// Root viewport title and initial size follow the window's props.
    fn window_chrome(&self, ctx: &egui::Context, w: Handle, vid: egui::ViewportId) {
        let (title, size) = self.window_props(w);
        let mut titles = self.0.titles.borrow_mut();
        let first = !titles.contains_key(&w);
        if titles.get(&w) != Some(&title) {
            ctx.send_viewport_cmd_to(vid, egui::ViewportCommand::Title(title.clone()));
            titles.insert(w, title);
        }
        if first {
            ctx.send_viewport_cmd_to(vid, egui::ViewportCommand::InnerSize(size));
        }
    }

    fn on_close_requested(&self, ctx: &egui::Context, vid: egui::ViewportId, w: Handle) {
        if self.0.quitting.get() {
            return; // the host decided to quit: let it close
        }
        ctx.send_viewport_cmd_to(vid, egui::ViewportCommand::CancelClose);
        let mut reqs = self.0.close_requests.borrow_mut();
        if !reqs.contains(&w) {
            reqs.push(w);
        }
        ctx.request_repaint();
    }

    /// Lays out (if needed) and paints window `w`'s content in `ui`'s available rect.
    fn window_ui(&self, ui: &mut egui::Ui, w: Handle, out: &mut Vec<Out>) {
        let rect = ui.available_rect_before_wrap();
        let size = rect.size();
        let mut scene = self.0.scene.borrow_mut();
        let stale = self.0.layout_dirty.get() || self.0.layout_size.borrow().get(&w) != Some(&size);
        if stale || scene.n(w).cache.is_empty() {
            let m = layout::Metrics::new(ui.ctx());
            let measured = layout::compute(&mut scene, &m, w, size);
            self.0.layout_size.borrow_mut().insert(w, size);
            self.bump(|s| {
                s.layouts += 1;
                s.measured += measured as u64;
            });
        }
        render::paint(&mut scene, w, rect.min.to_vec2(), ui, out);
        ui.allocate_rect(rect, egui::Sense::hover());
        drop(scene);
        // Every window shown this frame is laid out; a later window resets the flag too.
        self.0.layout_dirty.set(false);
    }

    fn after_pass(&self, ctx: &egui::Context, out: Vec<Out>) {
        if !out.is_empty() {
            if let Some(host) = self.host() {
                for o in out {
                    match o {
                        Out::Event(h, sig, args) => {
                            self.bump(|s| s.events += 1);
                            host.post_event(h, sig, &args);
                        }
                        Out::Changed(h, prop) => {
                            self.bump(|s| s.changes += 1);
                            host.prop_changed(h, prop);
                        }
                    }
                }
            }
            self.0.checkpoint_pending.set(true);
        }
        if self.0.checkpoint_pending.get() || self.0.frame_pending.get() || self.0.shared.wake.load(Ordering::Acquire) {
            ctx.request_repaint();
        }
        if !self.0.close_requests.borrow().is_empty() || !self.0.closes.borrow().is_empty() {
            ctx.request_repaint();
        }
        if let Some(d) = self.0.deadline.get() {
            ctx.request_repaint_after(d.saturating_duration_since(Instant::now()));
        }
    }

    // ---- ops helpers ----

    fn kind_of(&self, h: Handle) -> Result<usize, String> {
        self.0.scene.borrow().get(h).map(|n| n.kind).ok_or_else(|| format!("stale or foreign handle {h}"))
    }

    fn detach(&self, scene: &mut Arena, child: Handle) {
        if let Some(p) = scene.get(child).and_then(|n| n.parent) {
            if let Some(pn) = scene.get_mut(p) {
                pn.children.retain(|&c| c != child);
            }
            layout::invalidate(scene, p);
            scene.n_mut(child).parent = None;
        }
    }
}

impl Backend for EguiBackend {
    fn resolve(&self, what: NameKind, kind: svl_id, name: &str) -> svl_id {
        if what == NameKind::Kind {
            return kinds::tag_index(name).map(kinds::kind_id).unwrap_or(0);
        }
        let Some(k) = kinds::kind_of_id(kind) else { return 0 };
        match what {
            NameKind::Prop => kinds::find_prop(k, name).map(|(id, _)| id).unwrap_or(0),
            NameKind::Signal => kinds::find_signal(k, name).map(|(id, _)| id).unwrap_or(0),
            NameKind::Method => kinds::find_method(k, name).map(|(id, _)| id).unwrap_or(0),
            NameKind::Slot => (kinds::slot_of(k) == Some(name)) as u32,
            _ => 0,
        }
    }

    fn create(&self, kind: svl_id) -> StrResult<svl_handle> {
        let k = kinds::kind_of_id(kind).filter(|&k| KINDS[k].tag.is_some()).ok_or_else(|| format!("unknown kind id {kind}"))?;
        let h = self.0.scene.borrow_mut().alloc(|h| scene::new_node(k, h));
        Ok(h)
    }

    fn insert(&self, parent: svl_handle, child: svl_handle, before: Option<svl_handle>, slot: svl_id) -> StrResult<()> {
        let pk = self.kind_of(parent)?;
        let ck = self.kind_of(child)?;
        if kinds::slot_of(pk).is_none() {
            return Err(format!("<{}> takes no children", KINDS[pk].tag.unwrap_or("?")));
        }
        if slot > 1 {
            return Err(format!("unknown slot id {slot} on <{}>", KINDS[pk].tag.unwrap_or("?")));
        }
        if KINDS[ck].toplevel {
            return Err("a window cannot be inserted under another element".into());
        }
        if parent == child {
            return Err("cannot insert an element into itself".into());
        }
        let mut scene = self.0.scene.borrow_mut();
        // A parent inside the child's own subtree would make a cycle.
        let mut a = Some(parent);
        while let Some(x) = a {
            if x == child {
                return Err("cannot insert an element under its own descendant".into());
            }
            a = scene.n(x).parent;
        }
        if let Some(b) = before {
            if b == child {
                return Ok(()); // already in place
            }
            if scene.get(b).and_then(|n| n.parent) != Some(parent) {
                return Err(format!("handle #{b} is not a child of #{parent}"));
            }
        }
        self.detach(&mut scene, child);
        let pn = scene.n_mut(parent);
        let at = before.and_then(|b| pn.children.iter().position(|&c| c == b)).unwrap_or(pn.children.len());
        pn.children.insert(at, child);
        scene.n_mut(child).parent = Some(parent);
        layout::invalidate(&mut scene, parent);
        drop(scene);
        self.0.layout_dirty.set(true);
        self.touched();
        Ok(())
    }

    fn remove(&self, parent: svl_handle, child: svl_handle) -> StrResult<()> {
        let mut scene = self.0.scene.borrow_mut();
        if !scene.contains(parent) || !scene.contains(child) {
            return Err("stale or foreign handle".into());
        }
        if scene.n(child).parent != Some(parent) {
            return Err(format!("handle #{child} is not a child of #{parent}"));
        }
        self.detach(&mut scene, child);
        drop(scene);
        self.0.layout_dirty.set(true);
        self.touched();
        Ok(())
    }

    fn destroy(&self, h: svl_handle) {
        let mut scene = self.0.scene.borrow_mut();
        if !scene.contains(h) {
            return;
        }
        self.detach(&mut scene, h);
        // Children still attached are orphaned: the host destroys them itself.
        let kids = std::mem::take(&mut scene.n_mut(h).children);
        for c in kids {
            if let Some(cn) = scene.get_mut(c) {
                cn.parent = None;
            }
        }
        let n = scene.free(h);
        drop(scene);
        if let Some(n) = n {
            // Ephemeral widget state (cursor, scroll, animations) lives in egui memory under the node.
            if let Some(ctx) = self.ctx() {
                ctx.memory_mut(|m| m.data.remove::<egui::text_edit::TextEditState>(n.id));
            }
            if n.is_window() {
                self.0.windows.borrow_mut().retain(|&w| w != h);
            }
        }
        self.0.layout_dirty.set(true);
        self.touched();
    }

    fn set_prop(&self, h: svl_handle, prop: svl_id, v: &Value) -> StrResult<()> {
        let mut scene = self.0.scene.borrow_mut();
        let n = scene.get_mut(h).ok_or_else(|| format!("stale or foreign handle {h}"))?;
        let def = kinds::prop_def(n.kind, prop).ok_or_else(|| format!("unknown prop id {prop} on <{}>", n.tag()))?;
        // Host writes never announce a change: there is no echo to suppress (EGUI.md §1.4).
        let d = scene::set_prop(n, def, v).map_err(|e| format!("<{}> {}: {e}", n.tag(), def.name))?;
        if d.style {
            layout::invalidate(&mut scene, h);
            self.0.layout_dirty.set(true);
        }
        drop(scene);
        if d.measure {
            self.0.remeasure.borrow_mut().push(h);
        }
        self.touched();
        Ok(())
    }

    fn get_prop(&self, h: svl_handle, prop: svl_id) -> StrResult<Value> {
        let scene = self.0.scene.borrow();
        let n = scene.get(h).ok_or_else(|| format!("stale or foreign handle {h}"))?;
        let def = kinds::prop_def(n.kind, prop).ok_or_else(|| format!("unknown prop id {prop} on <{}>", n.tag()))?;
        scene::get_prop(n, def)
    }

    fn call(&self, h: svl_handle, method: svl_id, args: &[Value]) -> StrResult<Value> {
        let k = self.kind_of(h)?;
        let def = kinds::method_def(k, method).ok_or_else(|| format!("unknown method id {method}"))?;
        if !args.is_empty() {
            return Err(format!("{} takes no arguments", def.name));
        }
        match def.name {
            "present" => {
                let mut ws = self.0.windows.borrow_mut();
                if !ws.contains(&h) {
                    ws.push(h);
                }
                self.0.scene.borrow_mut().n_mut(h).presented = true;
                self.0.layout_dirty.set(true);
            }
            "close" => self.0.closes.borrow_mut().push(h),
            "request-focus" => self.0.scene.borrow_mut().n_mut(h).want_focus = true,
            other => return Err(format!("method {other} is not implemented")),
        }
        self.touched();
        Ok(Value::Null)
    }

    fn listen(&self, h: svl_handle, signal: svl_id, on: bool) -> StrResult<()> {
        let mut scene = self.0.scene.borrow_mut();
        let n = scene.get_mut(h).ok_or_else(|| format!("stale or foreign handle {h}"))?;
        kinds::signal_def(n.kind, signal).ok_or_else(|| format!("unknown signal id {signal} on <{}>", n.tag()))?;
        if on {
            if !n.listen.contains(&signal) {
                n.listen.push(signal);
            }
        } else {
            n.listen.retain(|&s| s != signal);
        }
        drop(scene);
        self.touched(); // a listener can change a widget's sense
        Ok(())
    }

    fn handle_info(&self, h: svl_handle) -> StrResult<svl_handle_info> {
        let k = self.kind_of(h)?;
        Ok(svl_handle_info {
            kind: kinds::kind_id(k),
            lifetime: if KINDS[k].toplevel { SVL_LIFE_TOPLEVEL } else { SVL_LIFE_OWNED },
            flags: 0,
        })
    }

    fn run(&self, host: &HostRef<'_>) -> StrResult<()> {
        self.0.host.set(host.as_raw());
        self.0.quitting.set(false);
        crate::runner::run_eframe(self)
    }

    fn waker(&self) -> Waker {
        let shared = self.0.shared.clone();
        Arc::new(move || {
            shared.wake.store(true, Ordering::Release);
            if let Some(ctx) = shared.ctx.lock().unwrap().as_ref() {
                ctx.request_repaint();
            }
        })
    }

    fn quit(&self) {
        self.0.quitting.set(true);
        if let Some(ctx) = self.ctx() {
            ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Close);
            ctx.request_repaint();
        }
    }

    fn schedule_wake(&self, ms: i64) {
        if ms < 0 {
            self.0.deadline.set(None);
            return;
        }
        self.0.deadline.set(Some(Instant::now() + Duration::from_millis(ms as u64)));
        if !self.0.in_frame.get()
            && let Some(ctx) = self.ctx()
        {
            ctx.request_repaint_after(Duration::from_millis(ms as u64));
        }
    }

    fn request_frame(&self) {
        self.0.frame_pending.set(true);
        if let Some(ctx) = self.ctx() {
            ctx.request_repaint();
        }
    }

    fn style(&self, op: u32, h: svl_handle, text: Option<&str>) -> StrResult<i32> {
        // G1: setter-only dialect (layout props are setters, D3); the cascade is G2.
        match op {
            SVL_STYLE_TARGET => Ok(SVL_STYLE_TARGET_NONE),
            SVL_STYLE_SHEET | SVL_STYLE_SCHEME => Ok(0),
            // TR1 visual channels (opacity since v2.3, the rest v2.8): paint state on the node
            SVL_STYLE_OPACITY | SVL_STYLE_TRANSFORM | SVL_STYLE_CLIP | SVL_STYLE_INERT => {
                self.visual_op(op, h, text)?;
                Ok(0)
            }
            _ => {
                self.kind_of(h)?;
                Ok(0)
            }
        }
    }

    fn supports_bounds(&self) -> bool {
        true
    }

    /// ABI v2.8: the node's rect in its window's content area. A dirty layout is brought up to date
    /// first (synchronously, as Qt's `QLayout::activate`), so FLIP sees the new place at once. A
    /// node outside a laid-out window reports 0, 0 and its measured size (leaves; 0 for boxes).
    fn bounds(&self, h: svl_handle) -> StrResult<[f64; 4]> {
        self.kind_of(h)?;
        self.layout_now(h);
        let scene = self.0.scene.borrow();
        let n = scene.n(h);
        if n.rect != egui::Rect::NOTHING {
            let r = n.rect;
            return Ok([r.min.x as f64, r.min.y as f64, r.width() as f64, r.height() as f64]);
        }
        let s = match (n.intrinsic, self.ctx()) {
            (Some(s), _) => s,
            (None, Some(ctx)) => layout::Metrics::new(&ctx).intrinsic(&n.w),
            (None, None) => egui::Vec2::ZERO,
        };
        Ok([0.0, 0.0, s.x as f64, s.y as f64])
    }

    fn supports_observe(&self) -> bool {
        true
    }

    fn observe(&self, h: svl_handle, prop: svl_id, on: bool) -> Result<(), ObserveError> {
        let mut scene = self.0.scene.borrow_mut();
        let n = scene.get_mut(h).ok_or_else(|| ObserveError::Failed(format!("stale or foreign handle {h}")))?;
        kinds::prop_def(n.kind, prop).ok_or_else(|| ObserveError::Failed(format!("unknown prop id {prop} on <{}>", n.tag())))?;
        // Every prop is exact: the scene is the only place a value changes.
        let c = n.observe.entry(prop).or_insert(0);
        if on {
            *c += 1;
        } else {
            *c = c.saturating_sub(1);
            if *c == 0 {
                n.observe.remove(&prop);
            }
        }
        Ok(())
    }

    fn supports_start(&self) -> bool {
        true
    }

    fn start(&self, host: Option<&HostRef<'_>>) -> StrResult<()> {
        // egui needs no global init: attaching is all; the caller's frames drive everything.
        self.0.host.set(host.map(|h| h.as_raw()).unwrap_or(std::ptr::null()));
        Ok(())
    }

    fn describe(&self, what: NameKind, kind: svl_id, member: svl_id) -> Option<MemberDesc> {
        if what != NameKind::Prop {
            return None;
        }
        let def = kinds::prop_def(kinds::kind_of_id(kind)?, member)?;
        let mut slot = SlotDesc::new(scene::tag_of(def.ty)).named(def.name);
        if def.nullable {
            slot = slot.flags(SVL_D_NULLABLE);
        }
        if def.ty == kinds::Ty::I64 {
            slot = slot.flags(SVL_D_INTEGRAL);
        }
        Some(MemberDesc { name: def.name.to_string(), params: vec![slot.clone()], ret: slot, flags: SVL_M_OBSERVABLE })
    }
}
