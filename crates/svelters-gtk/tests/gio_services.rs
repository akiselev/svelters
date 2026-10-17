//! The value model's hardest cases on real GIO calls: a `GError` as `NativeError`, an async
//! `g_file_load_contents_async` as a Promise (with AbortSignal -> `GCancellable`), a call-scope
//! callback (`g_file_enumerate_children`) and a notified-scope callback (`g_file_monitor`), driven
//! from JS through the host. Run under a display: `xvfb-run -a cargo nextest run -p svelters-gtk`.

use std::path::PathBuf;
use svelters_abi::BackendRef;
use svelters_gtk::testing::{on_gtk, pump};
use svelters_gtk::GtkBackend;
use svelters_host::{Host, HostConfig};

#[test]
fn gio_file_callbacks_async_and_gerror() {
    on_gtk(|| {
        let dir = std::env::temp_dir().join(format!("svelters-gio-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for (n, c) in [("a.txt", "alpha"), ("b.txt", "bravo"), ("c.txt", "charlie")] {
            std::fs::write(dir.join(n), c).unwrap();
        }
        let tpl = std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/js/gio_services.js")).unwrap();
        let bundle = dir.join("main.js");
        std::fs::write(&bundle, tpl.replace("__DIR__", dir.to_str().unwrap())).unwrap();

        let backend = GtkBackend::new().expect("backend");
        let br = unsafe { BackendRef::from_raw(backend.as_raw()) }.unwrap();
        let host = Host::new(br, HostConfig { echo_console: false, echo_errors: false, ..HostConfig::default() }).unwrap();
        unsafe { backend.attach_host(host.host_callbacks()) };
        host.load_bundle(&bundle).unwrap();

        let mut seen: Vec<String> = Vec::new();
        let mut created = false;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        loop {
            pump(20);
            host.checkpoint();
            seen.extend(host.take_console().into_iter().map(|l| l.text));
            if !created && seen.iter().any(|l| l == "MONITORING") {
                std::fs::write(dir.join("new.txt"), "x").unwrap();
                created = true;
            }
            if seen.iter().any(|l| l == "ALL DONE" || l.starts_with("FAILED")) || std::time::Instant::now() > deadline {
                break;
            }
        }
        pump(50);
        host.checkpoint();
        let errors: Vec<String> = host.take_errors().into_iter().map(|e| e.message).collect();
        let live = host.live_callbacks();
        let _ = std::fs::remove_dir_all(&dir);

        let lines: Vec<&str> = seen.iter().map(|s| s.as_str()).collect();
        assert_eq!(
            lines,
            [
                "path true true",
                "exists true",
                "loadContents true \"alpha\"",
                "exists missing false",
                "sync error NativeError g-io-error-quark not-found true true",
                "async returns a promise true",
                "async result \"bravo\"",
                "async rejection NativeError g-io-error-quark not-found",
                "abort NativeError g-io-error-quark cancelled",
                "listNames 4 [\"a.txt\",\"b.txt\",\"c.txt\",\"main.js\"]",
                "listNames stops on false true",
                "MONITORING",
                "EVENT created true",
                "ALL DONE",
            ],
            "errors: {errors:?}"
        );
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(live, 0, "every callback was released: call scope after the call, async after completion, notified by unmonitor/destroy");
    });
}
