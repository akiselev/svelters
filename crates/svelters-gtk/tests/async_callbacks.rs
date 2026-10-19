//! N1 hand-written lifetime tests of GI closures and folded async calls (tests/js/async_callbacks.js):
//! a call-scope closure ends with its call, a notified-scope closure lives until its GDestroyNotify, a
//! callback that outlives its owner answers `false` instead of crashing, 40 concurrent and 20 cancelled
//! async calls each settle once, an object disposed mid-flight rejects with `disposed`, an async
//! static constructor resolves, and a non-GIO error domain (GtkDialogError) is named. Afterwards no
//! native closure, async operation or host callback is left. Needs a display (tools/dev/headless).

mod common;

use svelters_gtk::testing::{on_gtk, pump};

#[test]
fn closures_and_async_calls_are_released_by_scope() {
    on_gtk(|| {
        let dir = common::stage("async-cb", "async_callbacks.js", &[("a.txt", "alpha")]);
        let base = svelters_gtk::closures::live_closures();
        let run = common::run(&dir, 30, |_| {});
        // the finalized filter's destroy notify frees its closure from an idle
        pump(100);
        let _ = std::fs::remove_dir_all(&dir);
        let lines: Vec<&str> = run.lines.iter().map(|s| s.as_str()).collect();
        assert_eq!(
            lines,
            [
                "call scope [\"a\",\"b\",\"c\"] true",
                "notified 1 true",
                "notified after append 2",
                // "banana split" would match: the released function answers false instead
                "outlived owner 2",
                "concurrent 40 true",
                "cancel many 20 true",
                "pre-aborted cancelled",
                "disposed mid-flight svelters disposed",
                "static async File FileIOStream true",
                "dialog gtk-dialog-error-quark cancelled",
                "ALL DONE",
            ],
            "errors: {:?}",
            run.errors
        );
        assert_eq!(run.live_callbacks, 0, "every host callback was released");
        assert_eq!(svelters_gtk::closures::live_async_ops(), 0, "every async operation completed natively");
        assert_eq!(svelters_gtk::closures::live_closures(), base, "call-scope and notified closures were freed");
    });
}
