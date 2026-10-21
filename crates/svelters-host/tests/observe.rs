//! ABI v2.6 on the QuickJS host over the log backend: `host.observe` is refcounted per (handle,
//! prop) and reaches the backend on transitions only; a scripted native change (`changed` with a
//! value) is delivered in a `__svelters_changed` batch while observed and when the value differs from
//! the last one (our own write included), and dropped otherwise; destroy forgets the observations.

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
        r#"[{"changed":[3,"label","x"]},{"changed":[3,"label","y"]},{"changed":[3,"label","y"]},{"changed":[3,"label","z"]},{"changed":[3,"label","w"]},{"changed":[3,"tooltip-text","t"]},{"checkpoint":true}]"#,
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
         changed #3 label \"x\"\n\
         changed #3 label \"y\"\n\
         set #2 label \"1: #btn label = \\\"y\\\" (live \\\"y\\\")\"\n\
         changed #3 label \"y\"\n\
         changed #3 label \"z\"\n\
         set #2 label \"2: #btn label = \\\"z\\\" (live \\\"z\\\")\"\n\
         observe #3 label off\n\
         changed #3 label \"w\"\n\
         changed #3 tooltip-text \"t\"\n"
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
