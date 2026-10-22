//! The frame all runners share ([`EguiBackend::ui`]), plus what runs outside the walk: native
//! closes, the checkpoint, rAF, remeasure, a synchronous layout for `bounds`, repaint requests.
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
//!
//! EM1 splits it into `logic_pass` (JS; `Embed::logic` calls it after its queued commands) and
//! `show_windows` (the walk); slots are drawn by `Embed::show_slot` (ARCH-EMBED §5). Phases
//! (ARCH-SCENE R17): `Logic` from the start of `logic_pass`, `Ui` from the end of logic to the end of
//! the egui pass (the legacy [`EguiBackend::ui`] ends it when it returns). Writer lane: EMBED.

use crate::backend::EguiBackend;
use crate::kinds;
use crate::layout;
use crate::scene::{Handle, Phase, RootIx};
use crate::walk::Out;
use std::sync::atomic::Ordering;
use std::time::Instant;
use svelters_abi::*;

impl EguiBackend {
    /// A host op changed scene root `r` (EM2, ARCH-EMBED §5.5). During a frame the root is recorded
    /// and `route_repaints` asks for its viewport after logic; outside one, its viewport is asked
    /// now. A hidden slot's root asks for nothing (it is not drawn); a detached node neither.
    pub(crate) fn touched(&self, r: Option<RootIx>) {
        let Some(r) = r.filter(|r| !r.is_detached()) else { return };
        if self.0.in_frame.get() {
            let mut t = self.0.touched.borrow_mut();
            if !t.contains(&r) {
                t.push(r);
            }
        } else if let Some(vp) = self.viewport_of(r)
            && let Some(ctx) = self.ctx()
        {
            ctx.request_repaint_of(vp);
        }
    }

    /// The scene root of handle `h`, for `touched`.
    pub(crate) fn root_of(&self, h: Handle) -> Option<RootIx> {
        let scene = self.0.scene.borrow();
        scene.live(h).ok().map(|id| scene.core[scene.ix(id)].root)
    }

    /// The viewport root `r` is drawn in: its slot's (`None` while hidden), else the root viewport.
    pub(crate) fn viewport_of(&self, r: RootIx) -> Option<egui::ViewportId> {
        self.0.root_vp.borrow().get(&r).copied().unwrap_or(Some(egui::ViewportId::ROOT))
    }

    /// Step 7 of `logic` (ARCH-EMBED §5.1): every viewport showing a root that host ops changed
    /// since the last call gets a repaint, except `drawing` (the viewport whose pass runs now: its
    /// ui draws the change in this pass). Returns the viewports asked.
    pub(crate) fn route_repaints(&self, ctx: &egui::Context, drawing: Option<egui::ViewportId>) -> Vec<egui::ViewportId> {
        let touched = std::mem::take(&mut *self.0.touched.borrow_mut());
        let mut vps: Vec<egui::ViewportId> = Vec::new();
        for r in touched {
            if let Some(vp) = self.viewport_of(r)
                && Some(vp) != drawing
                && !vps.contains(&vp)
            {
                vps.push(vp);
            }
        }
        for &vp in &vps {
            ctx.request_repaint_of(vp);
        }
        vps
    }

    pub(crate) fn checkpoint(&self) {
        self.assert_no_pass("checkpoint");
        if let Some(host) = self.host() {
            self.bump(|s| s.checkpoints += 1);
            host.checkpoint();
        }
    }

    /// Runs the pending layout of `h`'s root now (`bounds`), when the root has been laid out
    /// before (its size is known) and egui's fonts are available. Other roots keep their queues.
    pub(crate) fn layout_now(&self, h: Handle) {
        let Some(ctx) = self.ctx() else { return };
        let mut scene = self.0.scene.borrow_mut();
        let Ok(id) = scene.live(h) else { return };
        let r = scene.core[scene.ix(id)].root;
        let Some(size) = scene.roots.get(r).and_then(|e| e.size) else { return };
        let m = layout::Metrics::new(&ctx);
        layout::remeasure(&mut scene, &m, r);
        if let Some(run) = layout::run_root(&mut scene, &m, r, size) {
            drop(scene);
            self.count_layout(run);
        }
    }

    /// Adds a layout run to the backend's counters (one run per root served).
    pub(crate) fn count_layout(&self, run: layout::LayoutRun) {
        self.bump(|s| {
            s.layouts += 1;
            s.measured += run.leaf_layouts;
        });
    }

    /// Delivers the prop changes Rust writes announced (R18) and asks for the checkpoint that
    /// lets the runtime read them.
    fn flush_announcements(&self) {
        let a = self.0.scene.take_announcements();
        if a.is_empty() {
            return;
        }
        if let Some(host) = self.host() {
            for (h, prop) in a {
                self.bump(|s| s.changes += 1);
                host.prop_changed(h, prop);
            }
            self.0.checkpoint_pending.set(true);
        }
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
        crate::events::install(self, &ctx); // EV1: the input hook (once per context)
        if ctx.current_pass_index() == 0 {
            self.logic_pass(&ctx);
        }
        // the presented windows are all drawn in this pass (root and immediate viewports)
        self.0.touched.borrow_mut().clear();
        // R17: from the end of logic to the end of the pass, Rust writes are deferred.
        self.0.scene.set_phase(Phase::Ui);
        self.show_windows(ui);
        // This call is the whole frame of the legacy backend: the pass ends here. (EM1's `Embed`
        // keeps `Ui` until egui's end of pass, so the app's own ui between slots is covered.)
        self.0.scene.set_phase(Phase::Idle);
    }

    /// The part of a frame that runs JS (EM1: `Embed::logic` calls it, after its queued commands):
    /// counts the frame, then (phase `Logic`) deferred scene writes, announcements, closes,
    /// checkpoint, rAF and remeasure. First pass only. The caller sets `Ui` after it.
    pub(crate) fn logic_pass(&self, ctx: &egui::Context) {
        self.0.in_frame.set(true);
        self.0.scene.set_phase(Phase::Logic);
        self.bump(|s| s.frames += 1);
        self.0.shared.frames.fetch_add(1, Ordering::Relaxed);
        self.before_pass(ctx);
        self.0.in_frame.set(false);
    }

    /// The presented windows (the desktop order: the root window in `ui`, the others as
    /// immediate viewports), then the outbox. Never runs JS.
    pub(crate) fn show_windows(&self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.0.in_frame.set(true);
        let out = self.walk(ui);
        self.after_pass(&ctx, out);
        self.0.in_frame.set(false);
    }

    /// Makes `ctx` the context wakes and host ops repaint (an embedder may create the backend
    /// before the context it is drawn in exists).
    pub(crate) fn bind_ctx(&self, ctx: &egui::Context) {
        *self.0.shared.ctx.lock().unwrap() = Some(ctx.clone());
        crate::events::install(self, ctx); // EV1: the input hook (once per context)
    }

    fn before_pass(&self, ctx: &egui::Context) {
        // ARCH-SCENE §7.2 step 2b: mutations deferred during the last frame (the runtime sees them
        // in this logic's checkpoint), then the prop changes Rust writes announced.
        self.0.scene.run_deferred();
        // AS1: images whose asset changed (no JS ran): their nodes remeasure below
        let changed = crate::assets::take_changed(ctx);
        if !changed.is_empty() {
            crate::assets::mark_nodes(&mut self.0.scene.borrow_mut(), &changed);
        }
        self.flush_announcements();
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
        // all four are consumed (a short-circuit left `checkpoint_pending` set after a wake: one
        // more frame per interaction)
        let pending = self.0.checkpoint_pending.replace(false);
        if first || wake || due || pending {
            self.checkpoint();
        }
        if self.0.frame_pending.replace(false)
            && let Some(host) = self.host()
        {
            host.frame();
        }
        // LS1: lists bind the rows this frame draws (call_sync), at this frame's offset (R22)
        self.realize_lists(ctx);
        // WG5a: table cell editors bind and commit (call_sync), outside the pass
        self.realize_edits();
        // ARCH-SCENE §7.2 step 6 (SG4): state providers that want to run, each one batch. What they
        // changed repaints the visible roots once (route_repaints); one still wanting asks for
        // another logic (R16).
        if crate::provider::pass::run_state_providers(&self.0.scene, Some(ctx)) {
            let roots: Vec<RootIx> = self.0.scene.borrow().roots.iter().filter(|(_, e)| e.visible).map(|(r, _)| r).collect();
            for r in roots {
                self.touched(Some(r));
            }
        }
        if crate::provider::pass::providers_wanting(&self.0.scene) {
            ctx.request_repaint_of(self.logic_vp());
        }
        // Writes the checkpoint's handlers and the state providers made through `SceneHandle` are
        // announced now.
        self.flush_announcements();
        // DS1: a theme installed since the last logic reaches the context's Style.
        self.apply_pending_theme(ctx);
        // ARCH-SCENE §7.2 step 7 (CS5e): restyle what this logic's changes reach.
        self.restyle_pending(ctx);
        self.transitions_step(ctx);
        self.animate_step(ctx);
        // Remeasure leaves whose text changed, per root; only a size change queues a relayout.
        let mut scene = self.0.scene.borrow_mut();
        let roots: Vec<_> = scene.roots.iter().filter(|(_, e)| !e.dirty.remeasure.is_empty()).map(|(r, _)| r).collect();
        if !roots.is_empty() {
            let m = layout::Metrics::new(ctx);
            for r in roots {
                layout::remeasure(&mut scene, &m, r);
            }
        }
    }

    /// TR2: samples the native animations at egui's time (kittest's virtual clock in tests), applies
    /// the composed visuals, and reports each finished animation with `animationend` (the
    /// `outro_done` of an exiting node) when the runtime listens to it. Runs no JS: the event is
    /// queued and the next checkpoint delivers it.
    fn animate_step(&self, ctx: &egui::Context) {
        if self.0.anim.borrow().is_empty() {
            return;
        }
        let step = self.0.anim.borrow_mut().step(ctx.input(|i| i.time));
        let mut events = Vec::new();
        {
            let mut scene = self.0.scene.borrow_mut();
            for (h, v) in step.visuals {
                match scene.live(h) {
                    Ok(id) => {
                        scene.set_visual(id, v);
                    }
                    Err(_) => self.0.anim.borrow_mut().forget(h),
                }
            }
            for (h, aid) in step.done {
                let Ok(id) = scene.live(h) else { continue };
                let i = scene.ix(id);
                let sig = kinds::signal_id(scene.core[i].kind as usize, "animationend");
                if sig != 0 && scene.listens(i, sig) {
                    events.push((h, sig, aid));
                }
            }
        }
        if let Some(host) = self.host() {
            for (h, sig, aid) in events {
                self.bump(|s| s.events += 1);
                host.post_event(h, sig, &[Value::I64(aid as i64)]);
                self.0.checkpoint_pending.set(true);
            }
        }
    }

    /// OW-CSS: steps the CSS transitions at egui's time (kittest's virtual clock in tests): ended
    /// ones drop and running ones are sampled for this frame's paint.
    fn transitions_step(&self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        self.0.scene.borrow_mut().style.step_transitions(now);
    }

    /// OW-CSS: the viewports of the roots with a running CSS transition are asked for the next
    /// frame, and the logic viewport (which steps them); as [`Self::animate_demand`].
    fn transitions_demand(&self, ctx: &egui::Context) {
        let roots: Vec<RootIx> = {
            let scene = self.0.scene.borrow();
            if !scene.style.transitions_running() {
                return;
            }
            let mut v: Vec<RootIx> = Vec::new();
            for r in scene.style.transitioning_nodes().filter_map(|id| scene.idx(id).map(|i| scene.core[i].root)) {
                if !v.contains(&r) {
                    v.push(r);
                }
            }
            v
        };
        ctx.request_repaint_of(self.logic_vp());
        for r in roots {
            if !r.is_detached()
                && let Some(vp) = self.viewport_of(r)
            {
                ctx.request_repaint_of(vp);
            }
        }
    }

    /// SV37: the viewports of the roots with a running animation are asked for the next frame, and
    /// the logic viewport (which steps the animator); no other viewport is woken.
    fn animate_demand(&self, ctx: &egui::Context) {
        let roots: Vec<RootIx> = {
            let scene = self.0.scene.borrow();
            let anim = self.0.anim.borrow();
            let mut v: Vec<RootIx> = Vec::new();
            for r in anim.running().filter_map(|h| scene.live(h).ok().map(|id| scene.core[scene.ix(id)].root)) {
                if !v.contains(&r) {
                    v.push(r);
                }
            }
            v
        };
        if roots.is_empty() {
            return;
        }
        ctx.request_repaint_of(self.logic_vp());
        for r in roots {
            if !r.is_detached()
                && let Some(vp) = self.viewport_of(r)
            {
                ctx.request_repaint_of(vp);
            }
        }
    }

    /// Runs the sync `close-request` of window `w`; true if a handler kept the window open.
    fn close_vetoed(&self, w: Handle) -> bool {
        let sig = {
            let scene = self.0.scene.borrow();
            let Ok(id) = scene.live(w) else { return false };
            let i = scene.ix(id);
            let sig = kinds::signal_id(scene.core[i].kind as usize, "close-request");
            if !scene.listens(i, sig) {
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
        let dead = {
            let mut scene = self.0.scene.borrow_mut();
            match scene.live(w) {
                Ok(id) => scene.free_subtree(id),
                Err(_) => return,
            }
        };
        if dead.is_empty() {
            return;
        }
        self.0.windows.borrow_mut().retain(|&x| x != w);
        self.0.titles.borrow_mut().remove(&w);
        if let Some(host) = self.host() {
            for (i, &h) in dead.iter().enumerate() {
                host.on_destroyed(h.to_raw(), if i == 0 { SVL_GONE_CLOSED } else { SVL_GONE_PARENT });
            }
        }
    }

    /// Flushes `out` to the host (queues only: `post_event`/`prop_changed` run no JS) and requests
    /// the repaints pending work needs. Also called by `Embed` after its logic and each slot.
    pub(crate) fn after_pass(&self, ctx: &egui::Context, out: Vec<Out>) {
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
        // EM2: pending JS work asks for a pass of the viewport that runs logic next (`logic_vp`:
        // the root viewport, or this deferred viewport with the begin-pass plugin), never the
        // viewport that merely drew it (ARCH-EMBED §5.2).
        let vp = self.logic_vp();
        if self.0.checkpoint_pending.get() || self.0.frame_pending.get() || self.0.shared.wake.load(Ordering::Acquire) {
            ctx.request_repaint_of(vp);
        }
        if !self.0.close_requests.borrow().is_empty() || !self.0.closes.borrow().is_empty() {
            ctx.request_repaint_of(egui::ViewportId::ROOT);
        }
        self.animate_demand(ctx);
        self.transitions_demand(ctx);
        // CS5e: a walk-time restyle that shows next frame (an ancestor, a layout change)
        if std::mem::take(&mut self.0.scene.borrow_mut().style.repaint) {
            ctx.request_repaint_of(vp);
        }
        // R16: deferred scene writes are pending work; the next logic runs them.
        if self.0.scene.deferred_len() > 0 {
            ctx.request_repaint_of(vp);
        }
        // timers: the root viewport, which always runs logic
        if let Some(d) = self.0.deadline.get() {
            ctx.request_repaint_after_for(d.saturating_duration_since(Instant::now()), egui::ViewportId::ROOT);
        }
    }
}
