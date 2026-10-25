//! W-J1 and W-C2 in the host: the raw fast ops build the same tree as the general ops (same op
//! log, same errors), node wrappers holding handlers that capture them are collected once their
//! node is destroyed, and a template instantiates to exactly the ops the runtime would issue.

use std::path::PathBuf;
use svelters_backend_log::{LogBackend, parse_script};
use svelters_host::testing::{fixture_config, run_bundle};
use svelters_host::{Host, HostConfig};

fn js(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/js").join(name)
}

fn run(name: &str) -> svelters_host::testing::RunReport {
    let (cfg, opts) = fixture_config();
    let steps = parse_script(r#"[{"checkpoint":true}]"#).unwrap();
    run_bundle(&js(name), steps, cfg, opts).unwrap()
}

fn console(r: &svelters_host::testing::RunReport) -> Vec<String> {
    r.console.iter().map(|l| l.text.clone()).collect()
}

#[test]
fn fast_ops_log_and_fail_exactly_as_the_general_ops() {
    let fast = run("fast_ops.js");
    let slow = run("slow_ops.js");
    assert!(fast.errors.is_empty(), "{:?}", fast.errors);
    assert_eq!(fast.oplog, slow.oplog);
    assert!(fast.oplog.contains("set #3 label \"héllo wörld\"\n"), "{}", fast.oplog);
    assert!(fast.oplog.contains("listen #4 clicked off\nlisten #4 clicked on\n"), "{}", fast.oplog);
    assert_eq!(console(&fast), console(&slow));
}

#[test]
fn a_wrapper_whose_handler_captures_it_is_collected_after_destroy() {
    let (_, opts) = fixture_config();
    let log = LogBackend::with_options(Vec::new(), opts);
    let vt = log.vtable();
    let cfg = HostConfig { echo_console: false, echo_errors: false, ..HostConfig::default() };
    let host = Host::new(vt.backend_ref(), cfg).unwrap();
    host.load_bundle(js("wrappers.js")).unwrap();
    assert!(host.take_errors().is_empty());
    let lines: Vec<String> = host.take_console().into_iter().map(|l| l.text).collect();
    assert_eq!(lines[..3], ["same wrapper true", "handle true", "after destroy null"]);
    assert!(lines[3].starts_with("wrap dead: "), "{lines:?}");
    assert_eq!(host.live_node_wrappers(), 2);
    // the destroyed node's wrapper is let go at the checkpoint, then only the GC holds the cycle
    host.checkpoint();
    host.collect_garbage();
    assert_eq!(host.live_node_wrappers(), 1, "the kept node's wrapper stays, the dropped one is collected");
}

#[test]
fn a_template_expands_to_the_ops_the_runtime_would_issue() {
    let tpl = run("templates.js");
    let ops = run("templates_ops.js");
    assert!(tpl.errors.is_empty(), "{:?}", tpl.errors);
    assert_eq!(tpl.oplog, ops.oplog);
}
