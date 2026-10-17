//! The Qt backend through the real host: a dialog as a Promise (resolve, reject, abort) and a menu
//! item that runs a JS function (`examples/counter-qt/tests/js/qt_services.js`), the Qt counterpart
//! of `svelters-gtk`'s `gio_services` test. Needs a display: `xvfb-run -a cargo nextest run -p counter-qt`.

use std::path::PathBuf;
use svelters_abi::BackendRef;
use svelters_host::{Host, HostConfig};

/// `SVL_BENCH=1`: getProp of strings through the host (see `tests/js/getprop_bench.js`).
fn bench() {
    if std::env::var_os("SVL_BENCH").is_none() {
        return;
    }
    let br = unsafe { BackendRef::from_raw(counter_qt::backend()) }.expect("abi version");
    let host = Host::new(br, HostConfig { echo_console: false, echo_errors: false, turn_budget: None, ..HostConfig::default() }).expect("host");
    unsafe { counter_qt::set_host(host.host_callbacks()) };
    host.load_bundle(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/js/getprop_bench.js")).expect("bundle");
    for l in host.take_console() {
        println!("{}", l.text);
    }
    for e in host.take_errors() {
        println!("error: {e}");
    }
}

fn main() {
    // SAFETY: single-threaded at this point. Tests must not touch the developer's session bus.
    unsafe {
        std::env::set_var("QT_ACCESSIBILITY", "0");
        std::env::set_var("DBUS_SESSION_BUS_ADDRESS", "unix:path=/nonexistent");
    }
    counter_qt::run_as_test("qt_getprop_bench", bench);
    counter_qt::run_as_test("qt_services", || {
        let br = unsafe { BackendRef::from_raw(counter_qt::backend()) }.expect("abi version");
        let cfg = HostConfig { echo_console: false, echo_errors: false, quit_on_last_window: true, ..HostConfig::default() };
        let host = Host::new(br, cfg).expect("host");
        unsafe { counter_qt::set_host(host.host_callbacks()) };
        let js = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/js/qt_services.js");
        host.load_bundle(js).expect("bundle");
        // Trigger every action of every handle shortly after the script registered its menu items.
        counter_qt::hooks::run_later(
            400,
            Box::new(|| {
                for h in 1..=16 {
                    let _ = counter_qt::hooks::trigger_actions(h);
                }
            }),
        );
        counter_qt::hooks::run_later(15_000, Box::new(|| unsafe { (counter_qt::backend().quit)(counter_qt::backend().self_) }));
        host.run().expect("run");
        host.drain_jobs();
        let out: Vec<String> = host.take_console().into_iter().map(|l| l.text).collect();
        let errors: Vec<String> = host.take_errors().into_iter().map(|e| e.to_string()).collect();
        let lines: Vec<&str> = out.iter().map(|s| s.as_str()).collect();
        assert_eq!(
            lines,
            [
                "open returns a promise true",
                "accepted 1",
                "rejected 0",
                "abort true qt operation cancelled",
                "arity TypeError <color-dialog>.open: expected 0 argument(s), got 1",
                "not a function TypeError <menu>.addItem(functor): expected function, got 42",
                "ready",
                "menu callbacks open:false",
                "done",
            ],
            "errors: {errors:?}"
        );
        assert!(errors.iter().any(|e| e.contains("boom")), "the throwing callback is reported: {errors:?}");
        assert_eq!(host.live_callbacks(), 0, "every callback is released: async after completion, notified with the menu");
    });
}
