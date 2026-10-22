//! The eframe runners: a native window (glow) whose root viewport draws the app, returning when it
//! closes. [`run_desktop`] is the desktop order on an [`Embed`] (`run_app`); [`run_eframe`] is
//! `svl_backend.run` for a host that owns the backend itself. Without the `eframe` feature, both
//! fail with a pointer to embedding.

use crate::backend::EguiBackend;
#[cfg(feature = "eframe")]
use crate::embed::Embed;
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
    let app = EframeApp { b: b.clone() };
    let b = b.clone();
    run_native_with(title, size, move |ctx| {
        // VIS1: the default look (VS Code Dark/Light Modern)
        crate::vscode_look::apply(&b, ctx);
        Box::new(app)
    })
}

/// `make` gets eframe's context before the first frame (an embed attaches to it there).
#[cfg(feature = "eframe")]
fn run_native_with(
    title: String,
    size: egui::Vec2,
    make: impl FnOnce(&egui::Context) -> Box<dyn ::eframe::App> + 'static,
) -> Result<(), String> {
    // W5: the system primary selection for the text kinds (Linux; a no-op elsewhere)
    crate::primary::use_system();
    let opts = ::eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_title(title.clone()).with_inner_size(size),
        renderer: ::eframe::Renderer::Glow,
        ..Default::default()
    };
    ::eframe::run_native(&title, opts, Box::new(move |cc| Ok(make(&cc.egui_ctx)))).map_err(|e| format!("eframe: {e}"))
}

/// The desktop app on an [`Embed`]: `logic` from `App::logic`, the presented windows from `App::ui`.
#[cfg(feature = "eframe")]
struct DesktopApp {
    embed: Embed,
}

#[cfg(feature = "eframe")]
impl ::eframe::App for DesktopApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut ::eframe::Frame) {
        self.embed.logic(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut ::eframe::Frame) {
        self.embed.show_windows(ui);
    }
}

/// Runs an embed whose app bundle is loaded (`Embed::load_app`) in an eframe window until it closes.
#[cfg(feature = "eframe")]
pub(crate) fn run_desktop(embed: Embed) -> Result<(), String> {
    let (title, size) = root_props(embed.backend());
    // the pass callbacks (R17 phases, the begin-pass plugin) are on eframe's context from frame 1
    run_native_with(title, size, move |ctx| {
        embed.attach(ctx);
        // VIS1: the default look (VS Code Dark/Light Modern)
        crate::vscode_look::apply(embed.backend(), ctx);
        Box::new(DesktopApp { embed })
    })
}

#[cfg(not(feature = "eframe"))]
pub(crate) fn run_eframe(_b: &EguiBackend) -> Result<(), String> {
    let _ = root_props;
    Err("svelters-egui was built without the `eframe` feature: embed it (EguiBackend::ui) instead".into())
}

#[cfg(not(feature = "eframe"))]
pub(crate) fn run_desktop(_embed: crate::embed::Embed) -> Result<(), String> {
    Err("svelters-egui was built without the `eframe` feature: embed it (Embed) instead".into())
}
