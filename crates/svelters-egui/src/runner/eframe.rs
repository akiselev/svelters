//! The eframe runner (`svl_backend.run`): a native window (glow) whose root viewport draws the
//! backend, returning when it closes. Without the `eframe` feature, `run` fails with a pointer to
//! embedding.

use crate::backend::EguiBackend;
use crate::scene::W;

/// Title and inner size of the root window, if one is presented already (desktop order).
fn root_props(b: &EguiBackend) -> (String, egui::Vec2) {
    b.root_window()
        .and_then(|w| {
            b.with_scene(|s| match &s.n(w).w {
                W::Window { title, default_w, default_h } => Some((title.clone(), egui::vec2(*default_w as f32, *default_h as f32))),
                _ => None,
            })
        })
        .unwrap_or_else(|| ("svelters".to_string(), egui::vec2(480.0, 360.0)))
}

#[cfg(feature = "eframe")]
struct EframeApp {
    b: EguiBackend,
}

#[cfg(feature = "eframe")]
impl ::eframe::App for EframeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut ::eframe::Frame) {
        self.b.ui(ui);
    }
}

/// `svl_backend.run`: an eframe window (glow), returning when it closes.
#[cfg(feature = "eframe")]
pub(crate) fn run_eframe(b: &EguiBackend) -> Result<(), String> {
    let (title, size) = root_props(b);
    let opts = ::eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_title(title.clone()).with_inner_size(size),
        renderer: ::eframe::Renderer::Glow,
        ..Default::default()
    };
    let app = EframeApp { b: b.clone() };
    ::eframe::run_native(&title, opts, Box::new(move |_cc| Ok(Box::new(app)))).map_err(|e| format!("eframe: {e}"))
}

#[cfg(not(feature = "eframe"))]
pub(crate) fn run_eframe(_b: &EguiBackend) -> Result<(), String> {
    let _ = root_props;
    Err("svelters-egui was built without the `eframe` feature: embed it (EguiBackend::ui) instead".into())
}
