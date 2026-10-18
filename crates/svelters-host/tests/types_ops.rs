//! ABI v2.5 host ops `construct`, `callStatic` and `typeChain` over the log backend: the op log lines,
//! the handles they return, and the errors for bad arguments.

use std::path::PathBuf;
use svelters_backend_log::parse_script;
use svelters_host::testing::{fixture_config, run_bundle};

#[test]
fn construct_and_static_calls_are_logged_and_return_handles() {
    let bundle = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/js/types_ops.js");
    let (cfg, opts) = fixture_config();
    let r = run_bundle(&bundle, parse_script(r#"[{"checkpoint":true}]"#).unwrap(), cfg, opts).unwrap();
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    assert_eq!(
        r.oplog,
        "construct #1 Gio.ListStore {\"item-type\":\"Gio.File\"}\n\
         static Gio.File.new_for_path [\"/usr/share\"] #2\n\
         static Gio.File.parse_name [\"x\"]\n\
         call #1 append [2]\n\
         set #1 note 1\n"
    );
    let lines: Vec<String> = r.console.iter().map(|l| l.text.clone()).collect();
    assert_eq!(lines[0], "{\"store\":1,\"file\":2,\"none\":null,\"chain\":[]}");
    assert!(lines[1].contains("construct properties must be an object"), "{lines:?}");
    assert!(lines[2].contains("args must be an array"), "{lines:?}");
}
