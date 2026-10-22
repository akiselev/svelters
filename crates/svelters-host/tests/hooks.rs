//! LD1: scheme hooks (SV22), the `svelters:app/*` visibility filter (SV25) and extra native
//! modules, through `Host::set_module_hooks`. The provider is in memory; no pnpm needed.

use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;
use svelters_backend_log::{LogBackend, LogOptions};
use svelters_host::modules::{SchemeHook, StaticModules, Visibility};
use svelters_host::{ErrorKind, Host, HostConfig, ModuleCode, ModuleHooks, ModuleProvider};

struct Mem(String);

impl ModuleProvider for Mem {
    fn entry(&self) -> String {
        "main.js".into()
    }
    fn resolve(&self, _: &str, s: &str) -> Result<String, String> {
        Err(format!("unexpected import {s}"))
    }
    fn load(&self, _: &str) -> Result<ModuleCode, String> {
        Ok(ModuleCode::Source { js: self.0.clone(), map: None })
    }
    fn watch_files(&self) -> Vec<PathBuf> {
        vec![]
    }
    fn invalidate(&self, _: &[PathBuf]) -> bool {
        false
    }
    fn describe(&self) -> String {
        "mem".into()
    }
}

struct Api;

impl SchemeHook for Api {
    fn scheme(&self) -> &str {
        "cadatomic"
    }
    fn load(&self, id: &str) -> Result<ModuleCode, String> {
        match id {
            "cadatomic:api/doc" => Ok(ModuleCode::Source { js: "export const answer = 42;".into(), map: None }),
            _ => Err(format!("no such module: `{id}`")),
        }
    }
}

fn host() -> Host<'static> {
    let log = LogBackend::with_options(Vec::new(), LogOptions { settle: Duration::from_millis(50) });
    let log = Box::leak(Box::new(log));
    let vt = Box::leak(Box::new(log.vtable()));
    Host::new(vt.backend_ref(), HostConfig { echo_console: false, echo_errors: false, ..HostConfig::default() }).unwrap()
}

fn extra() -> StaticModules {
    StaticModules(vec![("svelters:app/extra".into(), "export const v = 7;".into())])
}

const ENTRY: &str = "import { answer } from 'cadatomic:api/doc';\nimport { v } from 'svelters:app/extra';\nif (answer !== 42 || v !== 7) throw new Error('wrong values');\n";

#[test]
fn a_registered_scheme_and_an_extra_native_module_load() {
    let h = host();
    h.set_module_hooks(ModuleHooks::default().scheme(Rc::new(Api)).natives(extra()));
    h.load_from(Rc::new(Mem(ENTRY.into()))).unwrap();
}

#[test]
fn an_unregistered_scheme_is_a_load_error() {
    let h = host();
    h.set_module_hooks(ModuleHooks::default().natives(extra()));
    let err = h.load_from(Rc::new(Mem(ENTRY.into()))).expect_err("cadatomic: is not registered");
    assert_eq!(err.kind, ErrorKind::Load);
}

#[test]
fn the_visibility_filter_hides_app_modules() {
    let h = host();
    h.set_module_hooks(ModuleHooks::default().scheme(Rc::new(Api)).natives(extra()).visibility(Visibility::Deny(vec!["extra".into()])));
    let err = h.load_from(Rc::new(Mem(ENTRY.into()))).expect_err("extra is hidden");
    assert!(err.to_string().contains("no such module"), "{err}");
}

#[test]
fn a_reserved_scheme_is_refused() {
    struct Svl;
    impl SchemeHook for Svl {
        fn scheme(&self) -> &str {
            "svelters"
        }
        fn load(&self, _: &str) -> Result<ModuleCode, String> {
            Err(String::new())
        }
    }
    let h = host();
    h.set_module_hooks(ModuleHooks::default().scheme(Rc::new(Svl)));
    let err = h.load_from(Rc::new(Mem("".into()))).expect_err("reserved");
    assert_eq!(err.kind, ErrorKind::Host);
}
