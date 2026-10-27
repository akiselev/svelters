//! SV81: a dropped host frees its QuickJS runtime, and a full reload frees the old one. The JS
//! closures the runtime owns hold the host's callback table; that table must not keep the runtime.

use std::path::{Path, PathBuf};
use std::rc::Rc;
use svelters_host::modules::{SourceConfig, SourceProvider};
use svelters_host::{Host, HostConfig};

fn quiet() -> HostConfig {
    HostConfig { echo_console: false, echo_errors: false, ..HostConfig::default() }
}

fn app(tag: &str, main: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("svelters-teardown-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("package.json"), "{\"name\": \"teardown\", \"private\": true}\n").unwrap();
    std::fs::write(dir.join("src/main.js"), main).unwrap();
    dir.canonicalize().unwrap()
}

fn provider(dir: &Path) -> Rc<SourceProvider> {
    let src = SourceConfig::for_path(&dir.join("src/main.js")).unwrap();
    Rc::new(SourceProvider::new(src).unwrap())
}

#[test]
fn sv81_a_dropped_host_frees_its_runtime() {
    for i in 0..20u64 {
        let before = Host::freed_runtimes();
        let host = Host::new_owned(svelters_backend_log::LogBackend::new(Vec::new()), quiet()).unwrap();
        drop(host);
        assert_eq!(Host::freed_runtimes(), before + 1, "bare host #{i} kept its runtime");
    }
}

#[test]
fn sv81_a_host_that_ran_an_app_frees_its_runtime() {
    let dir = app("ran", "globalThis.keep = [() => 1, { a: 1 }];\nsetTimeout(() => {}, 1000);\nconsole.log('up');\n");
    for i in 0..5u64 {
        let before = Host::freed_runtimes();
        let host = Host::new_owned(svelters_backend_log::LogBackend::new(Vec::new()), quiet()).unwrap();
        host.load_from(provider(&dir)).unwrap();
        assert!(host.take_errors().is_empty());
        drop(host);
        assert_eq!(Host::freed_runtimes(), before + 1, "host #{i} kept its runtime");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn sv81_a_full_reload_frees_the_old_runtime() {
    let dir = app("reload", "console.log('v1');\n");
    let host = Host::new_owned(svelters_backend_log::LogBackend::new(Vec::new()), quiet()).unwrap();
    host.load_from(provider(&dir)).unwrap();
    for v in 2..5 {
        let before = Host::freed_runtimes();
        let main = dir.join("src/main.js");
        std::fs::write(&main, format!("console.log('v{v}');\n")).unwrap();
        let report = host.apply_changes(vec![main]).expect("a reload");
        assert!(report.ok, "{report:?}");
        assert_eq!(Host::freed_runtimes(), before + 1, "reload to v{v} kept the old runtime");
    }
    let lines: Vec<String> = host.take_console().into_iter().map(|l| l.text).collect();
    assert_eq!(lines.last().map(String::as_str), Some("v4"), "{lines:?}");
    drop(host);
    let _ = std::fs::remove_dir_all(&dir);
}
