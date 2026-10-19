//! N1 acceptance: `tests/js/gio_services.js` on the generic path. The generated `Gio.File` class
//! (packages/types) drives the GI invoker through the real host: sync calls with out parameters, a
//! `GError` as `NativeError`, `_async`/`_finish` folded into Promises (resolve, reject, abort through
//! `GCancellable`), a call-scope callback, enumeration and a monitor signal. The hand-written `gio-file`
//! service this test used to exercise is retired. Run under a display (tools/dev/headless).

mod common;

use svelters_gtk::testing::on_gtk;

#[test]
fn gio_file_on_the_generic_path() {
    on_gtk(|| {
        let dir = common::stage("gio", "gio_services.js", &[("a.txt", "alpha"), ("b.txt", "bravo"), ("c.txt", "charlie")]);
        let base_closures = svelters_gtk::closures::live_closures();
        let mut created = false;
        let d2 = dir.clone();
        let run = common::run(&dir, 20, |l| {
            if l == "MONITORING" && !created {
                std::fs::write(d2.join("new.txt"), "x").unwrap();
                created = true;
            }
        });
        let _ = std::fs::remove_dir_all(&dir);
        let lines: Vec<&str> = run.lines.iter().map(|s| s.as_str()).collect();
        assert_eq!(
            lines,
            [
                "path true true",
                "exists true",
                "loadContents ret,contents,etagOut true true \"alpha\" true",
                "exists missing false",
                "sync error NativeError g-io-error-quark not-found true true",
                "async returns a promise true",
                "async result ret,contents,etagOut \"bravo\"",
                "async rejection NativeError g-io-error-quark not-found",
                "abort NativeError g-io-error-quark cancelled",
                "queryInfoAsync FileInfo 5 a.txt",
                "enumerate 5 [\"a.txt\",\"b.txt\",\"c.txt\"]",
                "measureDiskUsage true true true",
                "MONITORING",
                "EVENT created true",
                "ALL DONE",
            ],
            "errors: {:?}",
            run.errors
        );
        assert!(run.errors.is_empty(), "{:?}", run.errors);
        assert_eq!(run.live_callbacks, 0, "every completion and call-scope callback was released");
        assert_eq!(svelters_gtk::closures::live_closures(), base_closures, "the call-scope closure was freed");
        assert_eq!(svelters_gtk::closures::live_async_ops(), 0, "every async operation completed natively");
    });
}
