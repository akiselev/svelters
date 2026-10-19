//! Shared by the N1 host tests: run a JS module on the real host and GTK backend, with the generated
//! `svelters:gtk/*` classes staged next to it (`./types/gtk/<ns>/<Class>.js`).

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use svelters_abi::BackendRef;
use svelters_gtk::testing::pump;
use svelters_gtk::GtkBackend;
use svelters_host::{Host, HostConfig};

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let p = e.path();
        if p.is_dir() {
            copy_dir(&p, &to.join(e.file_name()));
        } else {
            std::fs::copy(&p, to.join(e.file_name())).unwrap();
        }
    }
}

/// A scratch directory with `main.js` (the template with `__DIR__` replaced by the directory) and a
/// copy of `packages/types` (`src/` and `gtk/`) under `types/`.
pub fn stage(name: &str, template: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("svelters-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let pkg = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages/types");
    copy_dir(&pkg.join("src"), &dir.join("types/src"));
    copy_dir(&pkg.join("gtk"), &dir.join("types/gtk"));
    for (n, c) in files {
        std::fs::write(dir.join(n), c).unwrap();
    }
    let tpl = std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/js").join(template)).unwrap();
    std::fs::write(dir.join("main.js"), tpl.replace("__DIR__", dir.to_str().unwrap())).unwrap();
    dir
}

pub struct Run {
    pub lines: Vec<String>,
    pub errors: Vec<String>,
    pub live_callbacks: usize,
}

/// Loads `dir/main.js` and pumps until a line `ALL DONE` or `FAILED...` (or the deadline). `on_line`
/// sees every console line as it arrives (the test reacts to markers such as `MONITORING`).
pub fn run(dir: &Path, secs: u64, mut on_line: impl FnMut(&str)) -> Run {
    let backend = GtkBackend::new().expect("backend");
    let br = unsafe { BackendRef::from_raw(backend.as_raw()) }.unwrap();
    let host = Host::new(br, HostConfig { echo_console: false, echo_errors: false, ..HostConfig::default() }).unwrap();
    unsafe { backend.attach_host(host.host_callbacks()) };
    host.load_bundle(&dir.join("main.js")).unwrap();
    let mut lines: Vec<String> = Vec::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(secs);
    loop {
        pump(10);
        host.checkpoint();
        for l in host.take_console() {
            on_line(&l.text);
            lines.push(l.text);
        }
        if lines.iter().any(|l| l == "ALL DONE" || l.starts_with("FAILED")) || std::time::Instant::now() > deadline {
            break;
        }
    }
    // Let deferred frees (idle) and releases (checkpoint) run.
    for _ in 0..5 {
        pump(20);
        host.checkpoint();
    }
    lines.extend(host.take_console().into_iter().map(|l| l.text));
    let errors = host.take_errors().into_iter().map(|e| e.message).collect();
    let live_callbacks = host.live_callbacks();
    Run { lines, errors, live_callbacks }
}
