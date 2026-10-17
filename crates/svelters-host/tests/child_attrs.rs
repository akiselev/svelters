//! X3: child attributes (`set_child_attr`, `get_child_attr`) and listener phases (`listen_ex`,
//! flags in the signal id of `post_event`/`call_sync`) over the log backend.

use std::path::PathBuf;
use svelters_backend_log::parse_script;
use svelters_host::testing::{fixture_config, run_bundle};

fn js(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/js").join(name)
}

const HEAD: &str = "create window #1\ncreate grid #2\ncreate label #3\ncreate label #4\ninsert #1 #2 before=end slot=default\ninsert #2 #3 before=end slot=default\nattr #2 #3 column 2\nattr #2 #3 row \"x\"\nattr #2 #3 column null\nremove #2 #3\ninsert #2 #3 before=end slot=default\nset #1 title \"[2,\\\"x\\\",null,true,null]\"\nlisten #1 close-request on capture\nlisten #1 close-request on after\nlisten #1 close-request off after\nlisten #1 close-request on\n";

#[test]
fn child_attrs_reach_the_backend_and_are_cleared_on_remove() {
    let (cfg, opts) = fixture_config();
    let r = run_bundle(&js("child_attrs.js"), parse_script("[{\"checkpoint\":true}]").unwrap(), cfg, opts).unwrap();
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    assert_eq!(r.oplog, HEAD);
}

#[test]
fn listener_flags_ride_in_the_signal_id_and_reach_the_matching_dispatch() {
    let (cfg, opts) = fixture_config();
    let steps = parse_script(
        r#"[{"sync":[1,"close-request",[],"capture"]},
            {"sync":[1,"close-request",[]]},
            {"event":[1,"close-request",[],"after"]},
            {"event":[1,"close-request",[],"after+capture"]},
            {"checkpoint":true}]"#,
    )
    .unwrap();
    let r = run_bundle(&js("child_attrs.js"), steps, cfg, opts).unwrap();
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    let tail = r.oplog.strip_prefix(HEAD).expect("head");
    assert_eq!(
        tail,
        "sync #1 close-request [] capture\nset #1 title \"close-request/2\"\nresult #1 null\n\
         sync #1 close-request []\nset #1 title \"close-request/0\"\nresult #1 null\n\
         event #1 close-request [] after\nset #1 title \"close-request/false/1\"\n\
         event #1 close-request [] after+capture\nset #1 title \"close-request/false/3\"\n"
    );
}
