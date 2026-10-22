//! SCHED1 on egui: a [`TestApp`] (the real QuickJS host, the egui backend, a kittest harness) driven by
//! a deterministic [`TestScheduler`] (`svelters-sched`, re-exported here).
//!
//! The host gets every [`svelters_host::HostSched`] hook from the scheduler: offloaded export jobs are
//! scheduler tasks, timers and `performance.now()` run on its clock, and each host wake (a completion,
//! a due timer, a queued event) wakes the egui foreground. A turn of that foreground is one
//! `harness.step()`: the frame's logic pass runs the checkpoint as eframe would. The foreground also
//! wants a turn while the last frame asked for an immediate repaint (kittest's own settle rule).
//!
//! ```ignore
//! run_seeds("my_egui_test", 12, |s| {
//!     let app = EguiSched::load(s, "main.js", quiet()).unwrap();
//!     s.run_until_parked();                       // first frames: the window is up
//!     app.click_after(s, Duration::from_millis(3), "Start");
//!     s.run_until_idle();
//!     assert!(app.has_label("done"));
//! });
//! ```
//!
//! egui's own clock (`input.time`, which native animations read) still advances by kittest's
//! `step_dt` per frame, not by the scheduler's clock: a follow-up.

pub use svelters_host::svelters_sched::*;

use egui_kittest::kittest::Queryable;
use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;
use svelters_egui::harness::TestApp;
use svelters_host::{HostConfig, HostSched};

/// A [`TestApp`] whose frames are a scheduler foreground. Cheap to clone (shared state).
#[derive(Clone)]
pub struct EguiSched {
    pub app: Rc<RefCell<TestApp>>,
    pub fg: Foreground,
    /// The last frame asked for an immediate repaint, or input is queued: the foreground wants a turn.
    wants: Rc<Cell<bool>>,
}

impl EguiSched {
    /// Loads `bundle` (an app bundle or a plain ES module) on a [`TestApp`] with `cfg` plus the
    /// scheduler's hooks (`scripted: false`).
    pub fn load(s: &TestScheduler, bundle: impl AsRef<Path>, cfg: HostConfig) -> Result<EguiSched, String> {
        let fg = s.foreground("egui");
        let cfg = HostConfig { scripted: false, sched: HostSched::test(s, fg), ..cfg };
        let app = Rc::new(RefCell::new(TestApp::with_config(bundle, cfg)?));
        let wants = Rc::new(Cell::new(true));
        let (a, w) = (app.clone(), wants.clone());
        s.set_runner(fg, move || {
            let mut app = a.borrow_mut();
            app.harness.step();
            w.set(app.harness.output().viewport_output.values().any(|v| v.repaint_delay.is_zero()));
        });
        let w = wants.clone();
        s.set_poll(fg, move || w.get());
        Ok(EguiSched { app, fg, wants })
    }

    /// Clicks the node labelled `label` now (kittest input) and asks for the frame that delivers it.
    /// Panics if no such node is shown.
    pub fn click(&self, label: &str) {
        self.app.borrow().harness.get_by_label(label).click();
        self.wants.set(true);
    }

    /// [`EguiSched::click`] as a test action at `now + d`, ordered by the scheduler against everything
    /// else due then.
    pub fn click_after(&self, s: &TestScheduler, d: Duration, label: &str) {
        let (me, l) = (self.clone(), label.to_string());
        s.spawn_local_after(d, &format!("click {label}"), move || me.click(&l));
    }

    /// A node labelled `label` is shown.
    pub fn has_label(&self, label: &str) -> bool {
        self.app.borrow().harness.query_by_label(label).is_some()
    }

    pub fn console(&self) -> Vec<String> {
        self.app.borrow().host().take_console().into_iter().map(|l| l.text).collect()
    }

    pub fn errors(&self) -> Vec<String> {
        self.app.borrow().errors()
    }
}
