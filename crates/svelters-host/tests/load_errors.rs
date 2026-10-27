//! GAL1 R3: a module-load error keeps the loader's whole message. QuickJS formats it into a fixed
//! 256-byte buffer (`Error loading module '<name>': <message>`), so a long module path used to cut
//! the message short (`…error[js_parse_error]: Un`).

use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;
use svelters_backend_log::{LogBackend, LogOptions};
use svelters_host::{ErrorKind, Host, HostConfig, ModuleCode, ModuleProvider};

/// The entry imports `dep`, a long path whose load fails with a long message.
struct Failing {
    dep: String,
    resolve_fails: bool,
}

const MESSAGE: &str = "error[js_parse_error]: Unexpected token, expected an expression after `=` (this sentence makes the message long enough to need more than QuickJS's buffer)";

impl ModuleProvider for Failing {
    fn entry(&self) -> String {
        "main.js".into()
    }
    fn resolve(&self, _: &str, s: &str) -> Result<String, String> {
        if self.resolve_fails { Err(format!("{s}: {MESSAGE}")) } else { Ok(self.dep.clone()) }
    }
    fn load(&self, id: &str) -> Result<ModuleCode, String> {
        if id == "main.js" {
            return Ok(ModuleCode::Source { js: format!("import x from '{}';\nexport default x;", self.dep), map: None });
        }
        Err(format!("{id}:2:11: {MESSAGE}"))
    }
    fn watch_files(&self) -> Vec<PathBuf> {
        vec![]
    }
    fn invalidate(&self, _: &[PathBuf]) -> bool {
        false
    }
    fn describe(&self) -> String {
        "failing".into()
    }
}

fn host() -> Host<'static> {
    let log = LogBackend::with_options(Vec::new(), LogOptions { settle: Duration::from_millis(50) });
    let log = Box::leak(Box::new(log));
    let vt = Box::leak(Box::new(log.vtable()));
    Host::new(vt.backend_ref(), HostConfig { echo_console: false, echo_errors: false, ..HostConfig::default() }).unwrap()
}

fn long_path() -> String {
    format!("/home/someone/work/{}/stories/controls/broken/syntax.svelte", "a-rather-deep-directory/".repeat(4))
}

#[test]
fn a_load_error_keeps_the_whole_message() {
    let dep = long_path();
    let err = host().load_from(Rc::new(Failing { dep: dep.clone(), resolve_fails: false })).expect_err("the import fails");
    assert_eq!(err.kind, ErrorKind::Load);
    assert_eq!(err.message, format!("Error loading module '{dep}': {dep}:2:11: {MESSAGE}"));
}

#[test]
fn a_resolve_error_keeps_the_whole_message() {
    let dep = long_path();
    let err = host().load_from(Rc::new(Failing { dep: dep.clone(), resolve_fails: true })).expect_err("the import fails");
    assert!(err.message.ends_with(&format!("{dep}: {MESSAGE}")), "{}", err.message);
}
