//! The eframe runners: a native window (glow) whose root viewport draws the app, returning when it
//! closes. [`run_desktop`] is the desktop order on an [`Embed`] (`run_app`); [`run_eframe`] is
//! `svl_backend.run` for a host that owns the backend itself. Without the `eframe` feature, both
//! fail with a pointer to embedding.

use crate::backend::EguiBackend;
#[cfg(feature = "eframe")]
use crate::embed::Embed;
#[cfg(feature = "eframe")]
use svelters_host::dx::FrameMonitor;
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
    /// ZU3: hang watchdog and frame times (`svelters_host::dx::frames`); `None` when off.
    frames: Option<FrameMonitor>,
}

#[cfg(feature = "eframe")]
impl ::eframe::App for EframeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut ::eframe::Frame) {
        if let Some(m) = &mut self.frames {
            m.begin();
        }
        self.b.ui(ui);
        if let Some(m) = &mut self.frames {
            m.end();
        }
    }
}

/// `svl_backend.run`: an eframe window (glow), returning when it closes.
#[cfg(feature = "eframe")]
pub(crate) fn run_eframe(b: &EguiBackend) -> Result<(), String> {
    let (title, size) = root_props(b);
    // created here, on the thread that runs the event loop (the UI thread)
    let app = EframeApp { b: b.clone(), frames: FrameMonitor::from_env() };
    run_native(title, size, Box::new(app))
}

#[cfg(feature = "eframe")]
fn run_native(title: String, size: egui::Vec2, app: Box<dyn ::eframe::App>) -> Result<(), String> {
    run_native_with(title, size, move |_ctx| app)
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
    /// ZU3: a frame is `logic` then `ui`; `None` when off.
    frames: Option<FrameMonitor>,
}

#[cfg(feature = "eframe")]
impl ::eframe::App for DesktopApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut ::eframe::Frame) {
        if let Some(m) = &mut self.frames {
            m.begin();
        }
        self.embed.logic(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut ::eframe::Frame) {
        if let Some(m) = &mut self.frames {
            m.begin(); // no-op after `logic`
        }
        self.embed.show_windows(ui);
        if let Some(m) = &mut self.frames {
            m.end();
        }
    }
}

/// Runs an embed whose app bundle is loaded (`Embed::load_app`) in an eframe window until it closes.
#[cfg(feature = "eframe")]
pub(crate) fn run_desktop(embed: Embed) -> Result<(), String> {
    let (title, size) = root_props(embed.backend());
    // the pass callbacks (R17 phases, the begin-pass plugin) are on eframe's context from frame 1
    run_native_with(title, size, move |ctx| {
        embed.attach(ctx);
        Box::new(DesktopApp { embed, frames: FrameMonitor::from_env() })
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
