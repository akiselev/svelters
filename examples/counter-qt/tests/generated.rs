//! Runs the generated verification tests (one per annotated slot/ownership/signal/property fact).
//! Needs a display: `xvfb-run -a cargo test -p counter-qt` (QT_QPA_PLATFORM=xcb) or offscreen.

unsafe extern "C" {
    fn svl_qt_verify_run() -> i32;
}

fn main() {
    // Make sure the backend is linked in and initialised through the public entry point.
    let _ = counter_qt::backend();
    let failures = unsafe { svl_qt_verify_run() };
    if failures != 0 {
        eprintln!("{failures} generated Qt verification test(s) failed");
        std::process::exit(1);
    }
}
