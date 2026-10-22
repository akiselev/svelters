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
//! EM1 turns this into `Embed::logic` (closes, commands, checkpoint, rAF, remeasure, repaint routing
//! per root; ARCH-EMBED §5). Writer lane: EMBED.

use crate::backend::EguiBackend;
use crate::kinds;
use crate::layout;
use crate::scene::{Handle, tree};
use crate::walk::Out;
use std::sync::atomic::Ordering;
use std::time::Instant;
use svelters_abi::*;

impl EguiBackend {
    /// Something changed outside a frame: draw again.
    pub(crate) fn touched(&self) {
        if !self.0.in_frame.get()
            && let Some(ctx) = self.ctx()
        {
            ctx.request_repaint();
        }
    }

    pub(crate) fn checkpoint(&self) {
        self.assert_no_pass("checkpoint");
        if let Some(host) = self.host() {
            self.bump(|s| s.checkpoints += 1);
            host.checkpoint();
        }
    }

    /// Runs a pending layout of `h`'s window now (`bounds`), when the window has been laid out
    /// before (its size is known) and egui's fonts are available.
    pub(crate) fn layout_now(&self, h: Handle) {
        let Some(ctx) = self.ctx() else { return };
        let mut scene = self.0.scene.borrow_mut();
        let mut root = h;
        while let Some(p) = scene.get(root).and_then(|n| n.parent) {
            root = p;
        }
        let Some(size) = self.0.dirty.layout_size.borrow().get(&root).copied() else { return };
        let m = layout::Metrics::new(&ctx);
        let leaves: Vec<Handle> = std::mem::take(&mut *self.0.dirty.remeasure.borrow_mut());
        if !leaves.is_empty() && layout::remeasure(&mut scene, &m, &leaves) {
            self.0.dirty.layout_dirty.set(true);
        }
        if !self.0.dirty.layout_dirty.get() && !scene.n(root).cache.is_empty() {
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
        if ctx.current_pass_index() == 0 {
            self.logic_pass(&ctx);
        }
        self.show_windows(ui);
    }

    /// The part of a frame that runs JS (EM1: `Embed::logic` calls it, after its queued commands):
    /// counts the frame, then closes, checkpoint, rAF and remeasure. First pass only.
    pub(crate) fn logic_pass(&self, ctx: &egui::Context) {
        self.0.in_frame.set(true);
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
        let leaves: Vec<Handle> = std::mem::take(&mut *self.0.dirty.remeasure.borrow_mut());
        if !leaves.is_empty() {
            let m = layout::Metrics::new(ctx);
            if layout::remeasure(&mut self.0.scene.borrow_mut(), &m, &leaves) {
                self.0.dirty.layout_dirty.set(true);
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
        let dead = tree::free_subtree(&mut self.0.scene.borrow_mut(), w);
        if dead.is_empty() {
            return;
        }
        self.0.windows.borrow_mut().retain(|&x| x != w);
        self.0.dirty.layout_size.borrow_mut().remove(&w);
        self.0.titles.borrow_mut().remove(&w);
        if let Some(host) = self.host() {
            for (i, &h) in dead.iter().enumerate() {
                host.on_destroyed(h, if i == 0 { SVL_GONE_CLOSED } else { SVL_GONE_PARENT });
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
}
