//! The `window` kind as a viewport: the presented windows are walked each pass, the first live one
//! in the root viewport, the others with `show_viewport_immediate`; their titles and sizes follow
//! the window props, and a user close becomes a `close-request` decided before the next pass.
//!
//! OW2: the window's inner chrome (its background) is svelters' own: the window node's computed
//! style over [`WINDOW_UA`] (`bg`, VS Code's chrome), painted by the box primitive. No egui
//! `CentralPanel` or `Frame`.
//!
//! For the desktop runner and kittest; after EM1 an embedding host draws slots instead
//! (ARCH-EMBED §9). Writer lane: EMBED.

use crate::backend::EguiBackend;
use crate::scene::{Handle, W};
use crate::walk::Out;

/// OW2: the window's background where no CSS sets one (VS Code's chrome, `bg`).
pub const WINDOW_UA: crate::paint::Ua = crate::paint::Ua(&[crate::paint::UaRule::base(&[("background-color", crate::paint::UaValue::Token("bg"))])]);

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
                self.window_chrome_ui(ui, w, &mut out);
                // EV1 (SV7): overlays and viewport passes after the window's walk
                crate::provider::pass::run_viewport_passes(&self.0.scene, ui);
            } else {
                let vid = egui::ViewportId::from_hash_of(("svl-window", w));
                let (title, size) = self.window_props(w);
                let builder = egui::ViewportBuilder::default().with_title(title).with_inner_size(size);
                ctx.show_viewport_immediate(vid, builder, |ui, _class| {
                    if ui.ctx().input(|inp| inp.viewport().close_requested()) {
                        self.on_close_requested(ui.ctx(), vid, w);
                    }
                    self.window_chrome_ui(ui, w, &mut out);
                    crate::provider::pass::run_viewport_passes(&self.0.scene, ui);
                });
            }
        }
        self.0.pass_active.set(false);
        out
    }

    /// OW2: window `w`'s content in `ui`'s whole rect on its own background.
    fn window_chrome_ui(&self, ui: &mut egui::Ui, w: Handle, out: &mut Vec<Out>) {
        // the space left in `ui` (what egui's `CentralPanel` took): a host may draw above it
        let rect = ui.available_rect_before_wrap();
        {
            let scene = self.0.scene.borrow();
            if let Ok(id) = scene.live(w) {
                let css = scene.style.computed(id).map(|c| &**c);
                let dark = ui.ctx().theme() == egui::Theme::Dark;
                let st = crate::paint::StyleView::new(css, WINDOW_UA, crate::provider::StateBits::NONE, dark);
                let mut pc = crate::paint::PaintCx::new(ui.painter());
                pc.target = (w, None);
                crate::paint::paint_box(&mut pc, &st, rect);
            }
        }
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).id_salt(("svl-window", w)).layout(egui::Layout::top_down(egui::Align::Min)));
        self.window_ui(&mut child, w, out);
        // the window takes that space in `ui` (as the panel did), so a frame around `ui` fits it
        ui.advance_cursor_after_rect(rect);
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
