//! G0 (ABI v2.7) on GTK: `Host::start_with` attaches the host without running the loop and mounts
//! the bundle in the started hook while the test pumps the main context; `Host::run_with` mounts in
//! the hook of a real `run`; and the desktop order (`load_bundle`, no `attach_host`) has the host
//! attached during mount, so a CustomFilter built at startup calls its JS function. Needs a display
//! (tools/dev/headless).

mod common;

use gtk::glib;
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};
use svelters_abi::BackendRef;
use svelters_gtk::GtkBackend;
use svelters_gtk::testing::{on_gtk, pump};
use svelters_host::{Host, HostConfig};

fn quiet() -> HostConfig {
    HostConfig { echo_console: false, echo_errors: false, ..HostConfig::default() }
}

/// Pumps and checkpoints until `ALL DONE` (or 10 s); returns the console lines.
fn pump_until_done(host: &Host<'_>) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !lines.iter().any(|l| l == "ALL DONE") && Instant::now() < deadline {
        pump(10);
        host.checkpoint();
        lines.extend(host.take_console().into_iter().map(|l| l.text));
    }
    lines
}

fn label_of(backend: &GtkBackend, lines: &[String]) -> String {
    let h: u64 = lines.iter().find_map(|l| l.strip_prefix("label ")).expect("label line").parse().unwrap();
    backend.object(h).expect("label object").downcast::<gtk::Label>().expect("a GtkLabel").label().to_string()
}

fn main_js(dir: &Path) -> PathBuf {
    dir.join("main.js")
}

#[test]
fn start_with_mounts_in_the_hook_while_the_test_pumps() {
    on_gtk(|| {
        let dir = common::stage("host-start", "host_start.js", &[]);
        let backend = GtkBackend::new().expect("gtk");
        let br = unsafe { BackendRef::from_raw(backend.as_raw()) }.unwrap();
        assert!(br.has_start());
        let host = Host::new(br, quiet()).unwrap();
        let path = main_js(&dir);
        host.start_with(move |h| h.load_bundle(&path).unwrap()).unwrap();
        assert!(host.is_started());
        let lines = pump_until_done(&host);
        assert!(lines.iter().any(|l| l == "mount items 1"), "{lines:?}");
        assert!(lines.iter().any(|l| l == "ALL DONE"), "{lines:?}");
        assert_eq!(label_of(&backend, &lines), "hello");
        let errors = host.take_errors();
        assert!(errors.is_empty(), "{errors:?}");
        host.stop();
        assert!(!host.is_started());
        // Detached: a wake schedules nothing that reaches the host.
        let before = host.checkpoint_count();
        br.wake();
        pump(30);
        assert_eq!(host.checkpoint_count(), before);
        drop(host);
        let _ = std::fs::remove_dir_all(&dir);
    });
}

#[test]
fn run_with_mounts_in_the_hook_of_the_loop() {
    on_gtk(|| {
        let dir = common::stage("host-run-with", "host_start.js", &[]);
        let backend = GtkBackend::new().expect("gtk");
        let br = unsafe { BackendRef::from_raw(backend.as_raw()) }.unwrap();
        let host = Host::new(br, quiet()).unwrap();
        let path = main_js(&dir);
        let lines: Rc<RefCell<Vec<String>>> = Rc::default();
        let fired = Rc::new(Cell::new(0));
        let (l, f) = (lines.clone(), fired.clone());
        host.run_with(move |h| {
            f.set(f.get() + 1);
            assert!(h.is_started());
            h.load_bundle(&path).unwrap();
            // Poll the console from the loop and quit once the bundle is done.
            let hp: *const Host<'_> = h;
            let deadline = Instant::now() + Duration::from_secs(10);
            glib::timeout_add_local(Duration::from_millis(10), move || {
                // SAFETY: `run` is on the stack until this source quits it, and the source ends then.
                let h = unsafe { &*hp };
                l.borrow_mut().extend(h.take_console().into_iter().map(|c| c.text));
                if l.borrow().iter().any(|x| x == "ALL DONE") || Instant::now() > deadline {
                    br.quit();
                    return glib::ControlFlow::Break;
                }
                glib::ControlFlow::Continue
            });
        })
        .unwrap();
        assert_eq!(fired.get(), 1);
        assert!(!host.is_started());
        let lines = lines.borrow().clone();
        assert!(lines.iter().any(|l| l == "mount items 1"), "{lines:?}");
        assert!(lines.iter().any(|l| l == "ALL DONE"), "{lines:?}");
        assert_eq!(label_of(&backend, &lines), "hello");
        let errors = host.take_errors();
        assert!(errors.is_empty(), "{errors:?}");
        drop(host);
        let _ = std::fs::remove_dir_all(&dir);
    });
}

#[test]
fn desktop_order_mount_has_the_host_attached() {
    on_gtk(|| {
        let dir = common::stage("host-preattach", "host_start.js", &[]);
        let backend = GtkBackend::new().expect("gtk");
        let br = unsafe { BackendRef::from_raw(backend.as_raw()) }.unwrap();
        let host = Host::new(br, quiet()).unwrap();
        // No attach_host: load_bundle attaches through `start` before mount.
        host.load_bundle(main_js(&dir)).unwrap();
        assert!(!host.is_started());
        let first: Vec<String> = host.take_console().into_iter().map(|l| l.text).collect();
        assert!(first.iter().any(|l| l == "mount items 1"), "the filter ran during mount: {first:?}");
        // A wake before any loop reaches the host through the main context.
        let before = host.checkpoint_count();
        br.wake();
        pump(30);
        assert!(host.checkpoint_count() > before, "wake before run reaches the host");
        let errors = host.take_errors();
        assert!(errors.is_empty(), "{errors:?}");
        drop(host);
        let _ = std::fs::remove_dir_all(&dir);
    });
}
