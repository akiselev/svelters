//! ABI v2.6 on the QuickJS host over the log backend: `host.observe` is refcounted per (handle,
//! prop) and reaches the backend on transitions only; `prop_changed` is delivered to
//! `__svelters_prop_changed(handle, prop)` at the next checkpoint while observed and dropped
//! otherwise; destroy forgets the observations of a handle.

use std::path::PathBuf;
use svelters_backend_log::parse_script;
use svelters_host::testing::{fixture_config, run_bundle};

fn js(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/js").join(name)
}

#[test]
fn observe_is_refcounted_and_prop_changed_reaches_js_while_observed() {
    let (cfg, opts) = fixture_config();
    let steps = parse_script(
        r#"[{"changed":[3,"label"]},{"changed":[3,"label"]},{"changed":[3,"label"]},{"changed":[3,"tooltip-text"]},{"checkpoint":true}]"#,
    )
    .unwrap();
    let r = run_bundle(&js("observe.js"), steps, cfg, opts).unwrap();
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    assert_eq!(
        r.oplog,
        "create window #1\n\
         create label #2\n\
         create button #3\n\
         insert #1 #2 before=end slot=default\n\
         observe #3 label on\n\
         set #3 label \"x\"\n\
         create label #4\n\
         observe #4 label on\n\
         destroy #4\n\
         changed #3 label\n\
         set #2 label \"1: #btn label = \\\"x\\\"\"\n\
         changed #3 label\n\
         set #2 label \"2: #btn label = \\\"x\\\"\"\n\
         observe #3 label off\n\
         changed #3 label\n\
         changed #3 tooltip-text\n"
    );
    let console: Vec<String> = r.console.iter().map(|c| c.text.clone()).collect();
    assert_eq!(
        console,
        [
            "on -> true",
            "nested on -> true",
            "off unobserved -> false",
            "off dead -> false",
            "on dead -> handle #4 was destroyed (destroyed by the app)",
            "off -> true",
            "off -> false",
        ]
    );
}
