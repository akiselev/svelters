//! W-H1: `host.dispatch_now` (ABI v2.9) delivers a signal at event time and drains the jobs its
//! handler queued, so the handler's microtasks (Svelte's flush) have run when it returns, unlike
//! `post_event`, whose delivery waits for the next checkpoint.

use std::path::PathBuf;
use svelters_backend_log::parse_script;
use svelters_host::testing::{fixture_config, run_bundle};

fn js(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/js").join(name)
}

const HEAD: &str = "create window #1\ncreate button #2\ninsert #1 #2 before=end slot=default\ncreate label #3\n\
                    insert #1 #3 before=end slot=default\nlisten #2 clicked on\n";

#[test]
fn dispatch_now_runs_the_handler_and_its_microtasks_before_returning() {
    let (cfg, opts) = fixture_config();
    let steps = parse_script(r#"[{"checkpoint":true},{"now":[2,"clicked",[]]},{"now":[2,"clicked",[]]}]"#).unwrap();
    let r = run_bundle(&js("dispatch_now.js"), steps, cfg, opts).unwrap();
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    assert_eq!(
        r.oplog,
        format!(
            "{HEAD}now #2 clicked []\nset #3 label \"clicked 1\"\nset #3 label \"flushed 1\"\nresult #2 ok\n\
             now #2 clicked []\nset #3 label \"clicked 2\"\nset #3 label \"flushed 2\"\nresult #2 ok\n"
        )
    );
}

#[test]
fn dispatch_now_without_a_listener_fails() {
    let (cfg, opts) = fixture_config();
    let steps = parse_script(r#"[{"checkpoint":true},{"now":[3,"clicked",[]]}]"#).unwrap();
    let r = run_bundle(&js("dispatch_now.js"), steps, cfg, opts).unwrap();
    assert_eq!(r.oplog, format!("{HEAD}now #3 clicked []\nresult #3 error\n"));
}

#[test]
fn jobs_an_earlier_event_left_run_in_the_dispatch_now_drain() {
    // Scripted: the `event` step's checkpoint only delivers it, so its microtask is still queued
    // when the `now` step comes; dispatch_now's drain runs it after the second handler.
    let (cfg, opts) = fixture_config();
    let steps = parse_script(r#"[{"checkpoint":true},{"event":[2,"clicked",[]]},{"now":[2,"clicked",[]]}]"#).unwrap();
    let r = run_bundle(&js("dispatch_now.js"), steps, cfg, opts).unwrap();
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    assert!(
        r.oplog.ends_with(
            "event #2 clicked []\nset #3 label \"clicked 1\"\nnow #2 clicked []\nset #3 label \"clicked 2\"\n\
             set #3 label \"flushed 1\"\nset #3 label \"flushed 2\"\nresult #2 ok\n"
        ),
        "{}",
        r.oplog
    );
}
