//! Component tests in plain `cargo test`: a compiled Svelte bundle on the real QuickJS host and the
//! egui backend, driven by `egui_kittest` (headless, no renderer: no GPU and no display).
//!
//! ```ignore
//! let mut app = TestApp::load("fixtures/counter-egui/dist/app.js")?;
//! app.harness.get_by_label("Increment").click();
//! app.run();
//! assert!(app.harness.query_by_label("Count: 1").is_some());
//! ```
//!
//! [`TestApp`] runs an app bundle (its `App` mounts its windows) on an [`Embed`]; [`EmbedHarness`]
//! hosts embed bundles in a `Ui` of the test's own, the way an application embeds svelters-egui.

use crate::backend::EguiBackend;
use crate::embed::{Busy, Embed, EmbedConfig, EmbedError};
use egui_kittest::Harness;
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use svelters_host::{Host, HostConfig};

pub use crate::runner::dock;
pub use crate::runner::viewports::Viewports;
pub use egui_kittest;
pub use egui_kittest::kittest::{NodeT, Queryable};

/// An app bundle on an [`Embed`] (the host attached through ABI v2.7 `start`; D1: the harness drives
/// the frames), with a kittest [`Harness`] whose frames run `logic` and draw the presented windows.
pub struct TestApp {
    pub harness: Harness<'static, Embed>,
    pub embed: Embed,
    pub backend: EguiBackend,
}

impl TestApp {
    /// Loads `bundle` with a quiet host config (console and errors are captured, not echoed).
    pub fn load(bundle: impl AsRef<Path>) -> Result<TestApp, String> {
        TestApp::with_config(bundle, HostConfig { echo_console: false, echo_errors: false, ..HostConfig::default() })
    }

    pub fn with_config(bundle: impl AsRef<Path>, cfg: HostConfig) -> Result<TestApp, String> {
        // The harness's context does not exist yet: `logic` binds it at the first frame.
        let embed = Embed::new(&egui::Context::default(), EmbedConfig::app(cfg)).map_err(|e| e.to_string())?;
        embed.load_app(bundle).map_err(|e| e.to_string())?;
        let backend = embed.backend().clone();
        let size = backend
            .root_window()
            .and_then(|w| match backend.prop(w, "default-width").ok().zip(backend.prop(w, "default-height").ok()) {
                Some((svelters_abi::Value::F64(w), svelters_abi::Value::F64(h))) => Some(egui::vec2(w as f32, h as f32)),
                _ => None,
            })
            .unwrap_or(egui::vec2(480.0, 360.0));
        // EM2 (the SG1 gap): the pass callbacks are on the harness's context before its first frame
        let attach = embed.clone();
        let harness = Harness::builder().with_size(size).with_context_setup(move |ctx| attach.attach(ctx)).build_ui_state(
            |ui, e: &mut Embed| {
                e.logic(ui.ctx());
                e.show_windows(ui);
            },
            embed.clone(),
        );
        Ok(TestApp { harness, embed, backend })
    }

    pub fn host(&self) -> &Host<'static> {
        self.embed.host()
    }

    /// Runs frames until nothing asks for another one; returns how many ran.
    pub fn run(&mut self) -> u64 {
        self.harness.run()
    }

    /// Errors JS reported so far (uncaught exceptions, rejected promises).
    pub fn errors(&self) -> Vec<String> {
        self.host().take_errors().into_iter().map(|e| e.to_string()).collect()
    }
}

type UiFn = Rc<RefCell<Box<dyn FnMut(&mut egui::Ui)>>>;

/// The harness state: the test's own ui closure, set after the embed exists.
pub struct HostUi(UiFn);

/// An [`Embed`] in a kittest harness whose ui is the test's (`set_ui`): the host app's panels call
/// `embed.show_slot`, as cadatomic's panes do. Each frame calls `embed.logic` first.
pub struct EmbedHarness {
    pub harness: Harness<'static, HostUi>,
    pub embed: Embed,
}

/// `run_until_settled` gave up: the busy reasons of each iteration (a non-converging loop shows its
/// chain).
#[derive(Clone, Debug)]
pub struct NotSettled {
    pub chain: Vec<Busy>,
}

impl EmbedHarness {
    pub fn new(size: egui::Vec2, cfg: EmbedConfig) -> Result<EmbedHarness, EmbedError> {
        let f: UiFn = Rc::new(RefCell::new(Box::new(|_ui: &mut egui::Ui| {})));
        let harness = Harness::builder().with_size(size).build_ui_state(
            |ui, s: &mut HostUi| {
                let f = s.0.clone();
                (f.borrow_mut())(ui);
            },
            HostUi(f),
        );
        let embed = Embed::new(&harness.ctx, cfg)?;
        Ok(EmbedHarness { harness, embed })
    }

    /// Sets the host's ui; `embed.logic` runs before it every frame.
    pub fn set_ui(&mut self, mut f: impl FnMut(&Embed, &mut egui::Ui) + 'static) {
        let e = self.embed.clone();
        *self.harness.state().0.borrow_mut() = Box::new(move |ui| {
            e.logic(ui.ctx());
            f(&e, ui);
        });
    }

    /// Runs frames until nothing asks for another one; returns how many ran.
    pub fn run(&mut self) -> u64 {
        self.harness.run()
    }

    pub fn step(&mut self) {
        self.harness.step();
    }

    /// Steps frames until [`Embed::settled`] is empty, at most `max_iters` (slint's
    /// `flush_until_settled`, cap 10 by convention). Returns the iterations it took.
    pub fn run_until_settled(&mut self, max_iters: u32) -> Result<u32, NotSettled> {
        let mut chain = Vec::new();
        for i in 1..=max_iters {
            self.harness.step();
            let s = self.embed.settled();
            chain.push(s.busy);
            if s.is_settled() {
                return Ok(i);
            }
        }
        Err(NotSettled { chain })
    }
}
