//! SCHED1: hot reload under the deterministic scheduler. An app loaded from source (plain JS modules,
//! so no `pnpm install` is needed) has a dynamic `import()` of a module whose top-level await keeps
//! the import pending for 3 ms of simulated time. Edits land at seeded random times through the
//! provider's overlay and `Host::apply_changes`: one swaps a self-accepting module in place (H1), the
//! other forces a full reload. Whatever the order, the app must end on the new versions, with the
//! pending import settled exactly once in the runtime that survived.
//!
//! `ITERATIONS=50` runs fifty schedules; `SEED=n` replays the one a failure printed.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;
use svelters_host::modules::{SourceConfig, SourceProvider};
use svelters_host::svelters_sched::run_seeds;
use svelters_host::testing::SchedHost;
use svelters_host::{HostConfig, HostSched, ReloadReport};

fn quiet() -> HostConfig {
    HostConfig { echo_console: false, echo_errors: false, ..HostConfig::default() }
}

const MAIN: &str = "import { version } from './dep.js';
console.log(`main ${version}`);
import('./lazy.js').then((m) => console.log(`lazy ${m.value}`), (e) => console.log(`lazy failed ${e}`));
";

fn dep(v: &str) -> String {
    format!("export const version = '{v}';\nif (import.meta.hot) import.meta.hot.accept((m) => console.log(`accepted dep ${{m && m.version}}`));\n")
}

fn lazy(v: &str) -> String {
    format!("await new Promise((r) => setTimeout(r, 3));\nexport const value = '{v}';\n")
}

fn app(seed: u64) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("svelters-sched-hmr-{}-{seed}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("package.json"), "{\"name\": \"sched-hmr\", \"private\": true}\n").unwrap();
    std::fs::write(dir.join("src/main.js"), MAIN).unwrap();
    std::fs::write(dir.join("src/dep.js"), dep("v1")).unwrap();
    std::fs::write(dir.join("src/lazy.js"), lazy("L1")).unwrap();
    dir.canonicalize().unwrap()
}

/// An edit through the overlay, applied as an editor would after saving.
fn edit(h: &Rc<svelters_host::Host<'static>>, provider: &SourceProvider, path: &Path, text: String) -> impl FnOnce() + 'static {
    let (h, overlay, path) = (h.clone(), provider.overlay().clone(), path.to_path_buf());
    move || {
        overlay.set(&path, text);
        h.apply_changes(vec![path]);
    }
}

#[test]
fn hmr_reload_races_a_pending_import() {
    let mut seen: Vec<(bool, bool)> = Vec::new();
    run_seeds("hmr_reload_races_a_pending_import", 12, |s| {
        s.set_turn_time(Duration::from_millis(1));
        let dir = app(s.seed());
        let mut cfg = SourceConfig::for_path(&dir.join("src/main.js")).unwrap();
        cfg.dev = true;
        cfg.hmr = true;
        let provider = Rc::new(SourceProvider::new(cfg).unwrap());
        let h = SchedHost::new(s, quiet()).unwrap();
        let reports: Rc<RefCell<Vec<ReloadReport>>> = Rc::default();
        let r = reports.clone();
        h.host.on_reload(move |rep| r.borrow_mut().push(rep.clone()));
        h.host.load_from(provider.clone()).unwrap();

        let (edit_dep, edit_lazy) = (s.random_bool(0.8), s.random_bool(0.8));
        if edit_dep {
            let at = Duration::from_micros(s.random_below(7000));
            s.spawn_local_after(at, "edit dep.js", edit(&h.host, &provider, &dir.join("src/dep.js"), dep("v2")));
        }
        if edit_lazy {
            let at = Duration::from_micros(s.random_below(7000));
            s.spawn_local_after(at, "edit lazy.js", edit(&h.host, &provider, &dir.join("src/lazy.js"), lazy("L2")));
        }
        s.run_until_idle();

        let lines = h.console();
        let errors = h.errors();
        assert!(errors.is_empty(), "{errors:#?}\n{lines:#?}\n{:#?}", reports.borrow());
        assert!(reports.borrow().iter().all(|r| r.ok), "{:#?}", reports.borrow());
        assert!(!lines.iter().any(|l| l.starts_with("lazy failed")), "{lines:#?}");
        let last_main = lines.iter().rposition(|l| l.starts_with("main ")).unwrap_or_else(|| panic!("{lines:#?}"));
        let tail = &lines[last_main + 1..];
        // the pending import of the surviving runtime settled exactly once, on the newest lazy.js
        let lazies: Vec<&String> = tail.iter().filter(|l| l.starts_with("lazy ")).collect();
        let want_lazy = if edit_lazy { "lazy L2" } else { "lazy L1" };
        assert_eq!(lazies, [want_lazy], "{lines:#?}\n{:#?}", reports.borrow());
        // dep.js ends on its newest version: re-evaluated by a full reload, or swapped in place
        let dep_now = if lines[last_main] == "main v2" || tail.iter().any(|l| l == "accepted dep v2") { "v2" } else { "v1" };
        assert_eq!(dep_now, if edit_dep { "v2" } else { "v1" }, "{lines:#?}\n{:#?}", reports.borrow());
        // Coverage: a full reload replaced the first runtime while its import was pending (no `lazy`
        // line before the second `main`), and a swap landed while one was pending (`accepted` then
        // `lazy` with no `main` between).
        let mains: Vec<usize> = lines.iter().enumerate().filter(|(_, l)| l.starts_with("main ")).map(|(i, _)| i).collect();
        let full_while_pending = mains.len() >= 2 && !lines[..mains[1]].iter().any(|l| l.starts_with("lazy "));
        let hot_while_pending = lines.iter().position(|l| l == "accepted dep v2").is_some_and(|a| {
            lines[a..].iter().take_while(|l| !l.starts_with("main ")).any(|l| l.starts_with("lazy "))
        });
        seen.push((full_while_pending, hot_while_pending));
        assert_eq!(h.host.ops_in_flight(), 0);
    });
    if seen.len() >= 12 {
        assert!(seen.iter().any(|s| s.0) && seen.iter().any(|s| s.1), "a full reload and an in-place swap should each land while the import is pending: {seen:?}");
    }
}

#[test]
fn a_scheduler_clock_excludes_scripted_mode() {
    let s = svelters_host::svelters_sched::TestScheduler::new(0);
    let fg = s.foreground("host");
    let log = svelters_backend_log::LogBackend::new(Vec::new());
    let cfg = HostConfig { scripted: true, sched: HostSched::test(&s, fg), ..quiet() };
    let err = svelters_host::Host::new_owned(log, cfg).err().expect("refused");
    assert!(err.to_string().contains("scripted"), "{err}");
}
