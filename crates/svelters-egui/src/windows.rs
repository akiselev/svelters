//! The `window` kind as a viewport: the presented windows are walked each pass, the first live one
//! in the root viewport's `CentralPanel`, the others with `show_viewport_immediate`; their titles
//! and sizes follow the window props, and a user close becomes a `close-request` decided before the
//! next pass.
//!
//! For the desktop runner and kittest; after EM1 an embedding host draws slots instead
//! (ARCH-EMBED §9). Writer lane: EMBED.

use crate::backend::EguiBackend;
use crate::scene::{Handle, W};
use crate::walk::Out;

impl EguiBackend {
    /// The window shown in the root viewport: the first presented one still alive.
    pub fn root_window(&self) -> Option<Handle> {
        let scene = self.0.scene.borrow();
        self.0.windows.borrow().iter().copied().find(|&w| scene.contains(w))
    }

    pub(crate) fn walk(&self, ui: &mut egui::Ui) -> Vec<Out> {
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
}
