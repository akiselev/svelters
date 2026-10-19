//! NT0 end to end on GTK: the real host runs the `fixtures/gtk-types` Svelte bundle, which imports
//! `svelters:gtk/gtk` and `svelters:gtk/gio`: one native TextBuffer shared by two views (its `changed`
//! signal drives a label), a StringList built and extended from JS, and a FileIcon from a path.
//! Needs a display (xvfb-run) and the built fixture (`pnpm -r build`).

use gtk::glib;
use gtk::prelude::*;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;
use svelters_abi::BackendRef;
use svelters_gtk::testing::on_gtk;
use svelters_gtk::GtkBackend;
use svelters_host::{Host, HostConfig};

fn walk(w: &gtk::Widget, out: &mut Vec<gtk::Widget>) {
    out.push(w.clone());
    let mut c = w.first_child();
    while let Some(x) = c {
        walk(&x, out);
        c = x.next_sibling();
    }
}

fn all_widgets() -> Vec<gtk::Widget> {
    let mut v = Vec::new();
    for t in gtk::Window::list_toplevels() {
        walk(&t, &mut v);
    }
    v
}

fn label_with(suffix: &str) -> Option<String> {
    all_widgets().iter().filter_map(|w| w.downcast_ref::<gtk::Label>()).map(|l| l.text().to_string()).find(|t| t.ends_with(suffix))
}

#[test]
fn the_gtk_types_fixture_runs_on_the_real_host() {
    let bundle = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/gtk-types/dist/app.js");
    if !bundle.exists() {
        eprintln!("skipped: {} is not built", bundle.display());
        return;
    }
    on_gtk(move || {
        let backend = Rc::new(GtkBackend::new().expect("gtk"));
        let raw = unsafe { BackendRef::from_raw(backend.as_raw()) }.unwrap();
        let cfg = HostConfig { echo_console: false, echo_errors: false, ..HostConfig::default() };
        let host = Host::new(raw, cfg).unwrap();
        host.load_bundle(bundle).unwrap();

        let seen: Rc<std::cell::RefCell<Vec<String>>> = Rc::default();
        let s = seen.clone();
        glib::timeout_add_local_once(Duration::from_millis(400), move || {
            let views: Vec<gtk::TextView> = all_widgets().into_iter().filter_map(|w| w.downcast::<gtk::TextView>().ok()).collect();
            let mut log = s.borrow_mut();
            log.push(format!("views={}", views.len()));
            // One native buffer behind both views, holding the constructor's text.
            let buffer = views[0].buffer();
            log.push(format!("shared={}", views[0].buffer() == views[1].buffer()));
            log.push(format!("text={}", buffer.text(&buffer.start_iter(), &buffer.end_iter(), false)));
            log.push(format!("label={:?}", label_with(" characters")));
            // The image shows the FileIcon built from a path.
            let image = all_widgets().into_iter().find_map(|w| w.downcast::<gtk::Image>().ok()).unwrap();
            let icon = image.gicon().and_then(|i| i.downcast::<gtk::gio::FileIcon>().ok());
            log.push(format!("icon={:?}", icon.map(|i| i.file().path().unwrap().to_string_lossy().into_owned())));
            // Typing into the buffer reaches JS through the native `changed` signal.
            buffer.set_text("Hello, native world");
        });
        let s = seen.clone();
        glib::timeout_add_local_once(Duration::from_millis(800), move || {
            s.borrow_mut().push(format!("label2={:?}", label_with(" characters")));
            gtk::Window::list_toplevels().iter().for_each(|w| {
                if let Some(w) = w.downcast_ref::<gtk::Window>() {
                    w.close();
                }
            });
        });
        let raw2 = raw;
        glib::timeout_add_local_once(Duration::from_millis(3000), move || raw2.quit());

        host.run().unwrap();
        let errs: Vec<String> = host.take_errors().into_iter().map(|e| e.message).collect();
        assert!(errs.is_empty(), "{errs:?}");
        let seen = seen.borrow();
        assert_eq!(seen[0], "views=2", "{seen:?}");
        assert_eq!(seen[1], "shared=true", "{seen:?}");
        assert_eq!(seen[2], "text=Hello", "{seen:?}");
        assert_eq!(seen[3], "label=Some(\"5 characters\")", "{seen:?}");
        assert_eq!(seen[4], "icon=Some(\"/usr/share/pixmaps/svelters.svg\")", "{seen:?}");
        assert_eq!(seen[5], "label2=Some(\"19 characters\")", "{seen:?}");
    });
}

/// SHARED handles are released when the JS engine collects their wrappers: 80 objects created and
/// dropped in a loop leave only what the app still holds.
#[test]
fn dropped_wrappers_release_their_shared_handles() {
    let bundle = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/gtk-types-gc/dist/app.js");
    if !bundle.exists() {
        eprintln!("skipped: {} is not built", bundle.display());
        return;
    }
    on_gtk(move || {
        let backend = Rc::new(GtkBackend::new().expect("gtk"));
        let raw = unsafe { BackendRef::from_raw(backend.as_raw()) }.unwrap();
        let cfg = HostConfig { echo_console: false, echo_errors: false, ..HostConfig::default() };
        let host = Host::new(raw, cfg).unwrap();
        unsafe { backend.attach_host(host.host_callbacks()) };
        host.load_bundle(bundle).unwrap();
        host.checkpoint();
        let baseline = backend.live_handles();
        // "make" fills a pool with 40 lists and 40 files, "drop" empties it.
        let buttons: Vec<gtk::Button> = all_widgets().into_iter().filter_map(|w| w.downcast::<gtk::Button>().ok()).collect();
        let (make, drop_pool) = (&buttons[0], &buttons[1]);
        make.emit_clicked();
        host.checkpoint();
        let before = backend.live_handles();
        assert!(before >= baseline + 80, "the handler minted handles: {baseline} -> {before}");
        drop_pool.emit_clicked();
        host.checkpoint();
        for _ in 0..5 {
            host.collect_garbage();
            host.drain_jobs();
            host.checkpoint();
            svelters_gtk::testing::pump(50);
        }
        let after = backend.live_handles();
        assert!(after <= baseline + 5, "garbage collection released the dropped wrappers: {baseline} -> {before} -> {after}");
        assert!(after >= baseline, "the kept list and the window are still alive: {after}");
        let errs: Vec<String> = host.take_errors().into_iter().map(|e| e.message).collect();
        assert!(errs.is_empty(), "{errs:?}");
    });
}
