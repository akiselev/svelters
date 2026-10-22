//! The egui runner: the case's bundle on [`TestApp`] (the real QuickJS host, the egui backend and a
//! kittest harness, headless). The view is the AccessKit tree; actions are kittest input on the
//! node found by its label, as a user would do them. With the `snapshots` feature, `snapshot` steps
//! render through wgpu (lavapipe in CI) and compare with `<cases>/snapshots/<case>-<name>.png`.

use crate::case::Case;
use crate::run::{Outcome, Target, run_case};
use crate::view::ViewNode;
use egui_kittest::kittest::{By, NodeT, Queryable};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use svelters_egui::egui;
use svelters_egui::harness::TestApp;

/// The backend name.
pub const BACKEND: &str = "egui";

/// How long `settle` waits for animations, timers and the host to go idle.
const SETTLE_LIMIT: Duration = Duration::from_secs(10);

pub struct EguiTarget {
    pub app: TestApp,
    /// Where `snapshot` steps keep their PNGs, and the case's name (the file prefix).
    pub snapshots: Option<(PathBuf, String)>,
    /// Snapshots written because none existed yet.
    pub recorded: Vec<PathBuf>,
}

impl EguiTarget {
    pub fn load(bundle: &Path) -> Result<EguiTarget, String> {
        Ok(EguiTarget { app: TestApp::load(bundle)?, snapshots: None, recorded: Vec::new() })
    }
}

fn role_is(n: &egui_kittest::Node<'_>, roles: &[egui::accesskit::Role]) -> bool {
    roles.contains(&n.accesskit_node().role())
}

impl Target for EguiTarget {
    fn settle(&mut self) -> Result<(), String> {
        let start = Instant::now();
        loop {
            // real time: transitions and timers advance with the wall clock
            let _ = self.app.harness.try_run_realtime();
            if self.app.embed.settled().is_settled() && !self.app.harness.ctx.has_requested_repaint() {
                return Ok(());
            }
            if start.elapsed() > SETTLE_LIMIT {
                return Err(format!("not settled after {SETTLE_LIMIT:?}: {:?}", self.app.embed.settled()));
            }
            std::thread::sleep(Duration::from_millis(5));
            self.app.harness.step();
        }
    }

    fn view(&mut self) -> Result<Vec<ViewNode>, String> {
        fn walk(n: egui_kittest::Node<'_>, depth: u32, out: &mut Vec<ViewNode>) {
            use egui::accesskit::Role;
            let a = n.accesskit_node();
            // a text input shows its content (its label is the hint text, which names it)
            let text = match a.role() {
                Role::TextInput | Role::MultilineTextInput => a.value().unwrap_or_default(),
                _ => a.label().or_else(|| a.value()).unwrap_or_default(),
            };
            out.push(ViewNode { depth, kind: format!("{:?}", a.role()), text });
            for c in n.children() {
                walk(c, depth + 1, out);
            }
        }
        let mut out = Vec::new();
        walk(self.app.harness.root(), 0, &mut out);
        Ok(out)
    }

    fn click(&mut self, text: &str) -> Result<(), String> {
        use egui::accesskit::Role;
        let hits: Vec<_> = self.app.harness.query_all(By::new().label(text)).filter(|n| role_is(n, &[Role::Button, Role::CheckBox])).collect();
        match hits.as_slice() {
            [n] => n.click(),
            [] => return Err(format!("no button showing {text:?}")),
            many => return Err(format!("{} buttons show {text:?}", many.len())),
        }
        let _ = self.app.harness.try_run();
        Ok(())
    }

    fn type_text(&mut self, field: &str, text: &str) -> Result<(), String> {
        use egui::accesskit::Role;
        let find = |app: &TestApp| -> Result<(), String> {
            let n = app.harness.query_all(By::new().label(field)).filter(|n| role_is(n, &[Role::TextInput, Role::MultilineTextInput])).count();
            if n == 1 { Ok(()) } else { Err(format!("{n} text inputs named {field:?}")) }
        };
        find(&self.app)?;
        let by = || By::new().label(field).predicate(|n| matches!(n.role(), Role::TextInput | Role::MultilineTextInput));
        self.app.harness.get(by()).click();
        let _ = self.app.harness.try_run();
        // replace the content, as the GTK and Qt drivers do
        self.app.harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        let _ = self.app.harness.try_run();
        self.app.harness.get(by()).type_text(text);
        let _ = self.app.harness.try_run();
        Ok(())
    }

    fn wait(&mut self, ms: u64) -> Result<(), String> {
        let end = Instant::now() + Duration::from_millis(ms);
        while Instant::now() < end {
            self.app.harness.step();
            std::thread::sleep(Duration::from_millis(16));
        }
        Ok(())
    }

    #[cfg(feature = "snapshots")]
    fn snapshot(&mut self, name: &str) -> Result<bool, String> {
        let Some((dir, case)) = &self.snapshots else { return Ok(false) };
        let file = format!("{case}-{name}");
        let path = dir.join(format!("{file}.png"));
        let image = self.app.harness.render()?;
        if !path.is_file() {
            // the first run records it; commit the PNG (CI uploads the snapshots it records)
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            image.save(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            self.recorded.push(path);
            return Ok(true);
        }
        // kittest's default per-pixel threshold (enough across wgpu backends) and a few pixels of
        // slack; the committed PNGs are lavapipe renderings (CI)
        let opts = egui_kittest::SnapshotOptions::new().output_path(dir).max_failed_pixels(8);
        egui_kittest::try_image_snapshot_options(&image, file, &opts).map_err(|e| e.to_string())?;
        Ok(true)
    }

    fn errors(&mut self) -> Vec<String> {
        self.app.errors()
    }
}

/// Runs `case` on egui. `snapshots`: the directory of the case's PNGs (used with the `snapshots`
/// feature only).
pub fn run(case: &Case, bundle: &Path, snapshots: Option<&Path>) -> (Outcome, Vec<PathBuf>) {
    if !bundle.is_file() {
        return (Outcome::fail(format!("{} not built (pnpm --filter ./tests/cases build)", bundle.display())), Vec::new());
    }
    let mut t = match EguiTarget::load(bundle) {
        Ok(t) => t,
        Err(e) => return (Outcome::fail(format!("load: {e}")), Vec::new()),
    };
    t.snapshots = snapshots.map(|d| (d.to_path_buf(), case.name.clone()));
    let out = run_case(case, &mut t);
    (out, std::mem::take(&mut t.recorded))
}
