//! Off by default (E2E-INSPECTOR §6 Q1): nothing answers until the app enables automation, and
//! disabling it again stops every session. One test in its own binary: the switch is per process.

use svelters_automation::{Grants, Session};
use svelters_egui::harness::TestApp;

#[test]
fn automation_is_off_until_enabled() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/counter-egui");
    let app = TestApp::from_source(&dir).unwrap();
    assert!(!svelters_automation::is_enabled());
    let err = Session::new(app, Grants::ALL)
        .err()
        .expect("disabled by default");
    assert_eq!(err.code, "automation.disabled");

    svelters_automation::enable();
    let app = TestApp::from_source(&dir).unwrap();
    let mut s = Session::new(app, Grants::READ).unwrap();
    s.wait_settled(10_000.0).unwrap();
    assert_eq!(s.query("role=button", None).unwrap().len(), 1);
    // a read-only session cannot act
    let err = s.click("role=button", Default::default()).unwrap_err();
    assert_eq!(err.code, "grant.denied");
    assert_eq!(s.include_tooling(true).unwrap_err().code, "grant.denied");

    svelters_automation::disable();
    assert_eq!(
        s.query("role=button", None).unwrap_err().code,
        "automation.disabled"
    );
}
