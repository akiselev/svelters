//! X2 end to end on GTK: the real host runs a JS bundle whose handlers decide sync signals
//! (`close-request`, a key controller's `key-pressed`) and use `requestAnimationFrame`. A glib
//! timer chain plays the user. Needs a display (xvfb-run); `GSK_RENDERER=cairo` avoids the GPU path.

use gtk::glib;
use gtk::prelude::*;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;
use svelters_abi::BackendRef;
use svelters_gtk::testing::on_gtk;
use svelters_gtk::GtkBackend;
use svelters_host::{Host, HostConfig};

const WIN: u64 = 1;
const ENTRY: u64 = 3;
const LABEL: u64 = 4;
const KEY: u64 = 5;

#[derive(Default)]
struct Seen {
    key: Vec<bool>,
    nested: Vec<bool>,
    entry_text: String,
    label_before_close: String,
    still_open_after_veto: bool,
    no_gone_yet: bool,
    window_gone_at_end: bool,
}

fn emit_key(ctrl: &gtk::EventController, keyval: u32) -> bool {
    ctrl.emit_by_name::<bool>("key-pressed", &[&keyval, &38u32, &gtk::gdk::ModifierType::empty()])
}

fn after(ms: u64, f: impl FnOnce() + 'static) {
    glib::timeout_add_local_once(Duration::from_millis(ms), f);
}

#[test]
fn js_handlers_decide_sync_signals_and_raf_uses_the_frame_clock() {
    on_gtk(|| {
        let backend = Rc::new(GtkBackend::new().expect("gtk"));
        let raw = unsafe { BackendRef::from_raw(backend.as_raw()) }.unwrap();
        let cfg = HostConfig { echo_console: false, echo_errors: false, ..HostConfig::default() };
        let host = Host::new(raw, cfg).unwrap();
        host.load_bundle(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/js/sync_e2e.js")).unwrap();

        let seen = Rc::new(RefCell::new(Seen::default()));
        let b = backend.clone();
        let s = seen.clone();
        // Keys: 97 is handled, 98 undecided, 99 throws, 100 raises a nested key press.
        after(300, move || {
            let ctrl = b.object(KEY).unwrap().downcast::<gtk::EventController>().unwrap();
            let mut st = s.borrow_mut();
            for k in [97, 98, 99] {
                st.key.push(emit_key(&ctrl, k));
            }
            let entry = b.object(ENTRY).unwrap().downcast::<gtk::Editable>().unwrap();
            let nested = Rc::new(RefCell::new(Vec::new()));
            let (n2, c2) = (nested.clone(), ctrl.clone());
            entry.connect_changed(move |_| n2.borrow_mut().push(emit_key(&c2, 120)));
            st.key.push(emit_key(&ctrl, 100));
            st.nested = nested.borrow().clone();
            st.entry_text = entry.text().to_string();
        });
        let b = backend.clone();
        let s = seen.clone();
        // Close: vetoed while the app says so, then closes natively once the handler stops vetoing.
        after(700, move || {
            let win = b.object(WIN).unwrap().downcast::<gtk::Window>().unwrap();
            win.close();
            let mut st = s.borrow_mut();
            st.still_open_after_veto = b.object(WIN).is_some();
            let ctrl = b.object(KEY).unwrap().downcast::<gtk::EventController>().unwrap();
            emit_key(&ctrl, 101);
            st.label_before_close = b.object(LABEL).unwrap().downcast::<gtk::Label>().unwrap().label().to_string();
        });
        let b = backend.clone();
        after(900, move || b.object(WIN).unwrap().downcast::<gtk::Window>().unwrap().close());
        let raw2 = raw;
        after(8000, move || raw2.quit()); // safety net: the host quits itself after the last window

        host.run().unwrap();
        seen.borrow_mut().window_gone_at_end = backend.object(WIN).is_none();
        seen.borrow_mut().no_gone_yet = true;

        let errs: Vec<String> = host.take_errors().into_iter().map(|e| e.message).collect();
        let st = seen.borrow();
        assert_eq!(st.key, [true, false, false, true], "handled / undecided / thrown (default) / handled");
        assert_eq!(st.nested, [false], "a sync signal raised inside a sync handler came back as the default");
        assert_eq!(st.entry_text, "typed", "ops from a sync handler apply at once");
        assert!(st.still_open_after_veto, "e.preventDefault() on close vetoed the native close");
        assert_eq!(
            st.label_before_close,
            "key 97|key 98|key 99|key 100|async key-pressed deferred=true|close veto=true|key 101",
            "handlers ran in order; the nested signal was delivered late with deferred=true"
        );
        assert!(st.window_gone_at_end, "the second close was left to the toolkit and the window died natively");
        assert_eq!(errs.len(), 1, "{errs:?}");
        assert!(errs[0].contains("key boom"), "{errs:?}");
        // The first frame is requested while the window is not mapped yet (fallback timer); the
        // two it chains after that come from the GDK frame clock.
        let threads: Vec<String> = std::fs::read_dir("/proc/self/task")
            .unwrap()
            .filter_map(|e| std::fs::read_to_string(e.unwrap().path().join("comm")).ok())
            .map(|s| s.trim().to_string())
            .collect();
        assert!(threads.iter().all(|t| !t.starts_with("svelters")), "the host started a thread: {threads:?}");
        assert!(backend.frames_by_clock() >= 2, "requestAnimationFrame ran on the GDK frame clock, got {}", backend.frames_by_clock());
    });
}
