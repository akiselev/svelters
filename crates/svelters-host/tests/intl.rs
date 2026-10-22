//! IN1: `Intl` in QuickJS through the host (tests/js/intl.js), and a runtime locale switch seen
//! by JS.
#![cfg(feature = "intl")]

use std::path::PathBuf;
use std::time::Duration;
use svelters_backend_log::{LogBackend, LogOptions};
use svelters_host::{Host, HostConfig};

fn host(locale: &str) -> Host<'static> {
    let log = LogBackend::with_options(Vec::new(), LogOptions { settle: Duration::from_millis(20) });
    let log = Box::leak(Box::new(log));
    let vt = Box::leak(Box::new(log.vtable()));
    let cfg = HostConfig { echo_console: false, echo_errors: false, locale: Some(locale.into()), ..HostConfig::default() };
    Host::new(vt.backend_ref(), cfg).unwrap()
}

fn console(h: &Host<'_>) -> Vec<String> {
    h.take_console().into_iter().map(|c| c.text).collect()
}

#[test]
fn intl_in_quickjs() {
    let h = host("en-US");
    h.load_bundle(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/js/intl.js")).unwrap();
    assert!(h.take_errors().is_empty());
    let lines = console(&h);
    let failed: Vec<&String> = lines.iter().filter(|l| !l.starts_with("ok ") && !l.starts_with("locale ")).collect();
    assert!(failed.is_empty(), "{failed:#?}");
    assert!(lines.iter().filter(|l| l.starts_with("ok ")).count() >= 50, "{lines:#?}");
    assert_eq!(lines.last().unwrap(), "locale en-US 1,234.5 other");

    // the host switches language: subscribers run, the defaults follow
    assert_eq!(h.set_locale("de_DE.UTF-8").unwrap(), "de-DE");
    assert_eq!(h.locale(), "de-DE");
    assert_eq!(console(&h), ["locale de-DE 1.234,5 other"]);
    assert_eq!(h.set_locale("pl").unwrap(), "pl");
    assert_eq!(console(&h), ["locale pl 1234,5 few"]);
    assert!(h.set_locale("not a tag!").is_err());
    assert_eq!(h.locale(), "pl");
}
