//! X2 end to end on Qt: the real host runs a JS bundle whose handlers decide sync signals
//! (`close-request`, an entry's `key-press`) and use `requestAnimationFrame`. Key presses and
//! window closes are native events (`hooks::type_text`, `hooks::close`). Needs a display
//! (xvfb-run) and the main thread, so no libtest harness.

use counter_qt::hooks;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use svelters_abi::BackendRef;
use svelters_host::{Host, HostConfig};

const WIN: u64 = 1;
const ENTRY: u64 = 3;

#[derive(Default)]
struct Seen {
    dump_before_close: String,
    toplevels_after_veto: usize,
    frames_by_update: u32,
}

fn main() {
    counter_qt::run_as_test("js_handlers_decide_sync_signals_and_raf_uses_the_frame_clock", body);
}

fn body() {
    let raw = unsafe { BackendRef::from_raw(counter_qt::backend()) }.unwrap();
    let cfg = HostConfig { echo_console: false, echo_errors: false, ..HostConfig::default() };
    let host = Host::new(raw, cfg).unwrap();
    host.load_bundle(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/js/sync_e2e.js")).unwrap();

    let seen = Rc::new(RefCell::new(Seen::default()));
    // Keys: `a` is consumed, `b` undecided, `c` throws, `d` closes the window from inside the handler.
    hooks::run_later(300, Box::new(|| {
        for t in ["a", "b", "c", "d"] {
            hooks::type_text(ENTRY, t).unwrap();
        }
    }));
    let s = seen.clone();
    // Close: vetoed while the app says so, then closes natively once the handler stops vetoing.
    hooks::run_later(700, Box::new(move || {
        let _ = hooks::close(WIN); // vetoed: close() reports false
        s.borrow_mut().toplevels_after_veto = hooks::toplevel_count();
        hooks::type_text(ENTRY, "e").unwrap();
        s.borrow_mut().dump_before_close = hooks::dump(WIN);
    }));
    hooks::run_later(900, Box::new(|| {
        let _ = hooks::close(WIN);
    }));
    let raw2 = raw;
    hooks::run_later(8000, Box::new(move || raw2.quit())); // safety net: the host quits after the last window

    host.run().unwrap();
    seen.borrow_mut().frames_by_update = unsafe { svl_qt_frames_by_update() };

    let errs: Vec<String> = host.take_errors().into_iter().map(|e| e.message).collect();
    let st = seen.borrow();
    let dump = &st.dump_before_close;
    // `a` never reached the entry, the others replaced its selection each time.
    assert!(dump.contains("\"class\":\"QLineEdit\",\"text\":\"e\""), "{dump}");
    assert!(
        dump.contains("key a|key b|key c|key d|async close-request deferred=true|close veto=true|key e"),
        "handlers ran in order; the close raised inside a handler came back late with deferred=true: {dump}"
    );
    assert!(!dump.contains("cancelled frame ran"), "{dump}");
    assert_eq!(st.toplevels_after_veto, 1, "e.preventDefault() on close vetoed the native close");
    assert_eq!(errs.len(), 1, "{errs:?}");
    assert!(errs[0].contains("key boom"), "{errs:?}");
    // The first frame is requested before any window is visible (fallback timer); the other
    // two come from QWindow::requestUpdate.
    assert!(st.frames_by_update >= 2, "requestAnimationFrame used the window's update requests, got {}", st.frames_by_update);
    let threads: Vec<String> = std::fs::read_dir("/proc/self/task")
        .unwrap()
        .filter_map(|e| std::fs::read_to_string(e.unwrap().path().join("comm")).ok())
        .map(|s| s.trim().to_string())
        .collect();
    assert!(threads.iter().all(|t| !t.starts_with("svelters")), "the host started a thread: {threads:?}");
    println!("host_sync ok");
}

unsafe extern "C" {
    fn svl_qt_frames_by_update() -> u32;
}
