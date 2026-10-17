//! X8 end to end on GTK: a JS handler's typed result (`e.result = {newValue}`) fills the out-parameter of
//! `GtkSpinButton::input`, and the `output` handler displays the value as hex through `text`. A glib timer
//! chain plays the user. Needs a display (xvfb-run); `GSK_RENDERER=cairo` avoids the GPU path.

use gtk::glib;
use gtk::prelude::*;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;
use svelters_abi::BackendRef;
use svelters_gtk::testing::on_gtk;
use svelters_gtk::GtkBackend;
use svelters_host::{Host, HostConfig};

const WIN: u64 = 1;
const SPIN: u64 = 2;

#[derive(Default)]
struct Seen {
    text_after_set: String,
    value_after_input: f64,
    value_after_shorthand: f64,
    value_after_plain: f64,
    value_after_error: f64,
}

fn after(ms: u64, f: impl FnOnce() + 'static) {
    glib::timeout_add_local_once(Duration::from_millis(ms), f);
}

#[test]
fn js_results_fill_out_parameters_and_format_spin_button_text() {
    on_gtk(|| {
        let backend = Rc::new(GtkBackend::new().expect("gtk"));
        let raw = unsafe { BackendRef::from_raw(backend.as_raw()) }.unwrap();
        let cfg = HostConfig { echo_console: false, echo_errors: false, ..HostConfig::default() };
        let host = Host::new(raw, cfg).unwrap();
        host.load_bundle(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/js/typed_sync.js")).unwrap();

        let seen = Rc::new(RefCell::new(Seen::default()));
        let spin = {
            let b = backend.clone();
            move || b.object(SPIN).unwrap().downcast::<gtk::SpinButton>().unwrap()
        };
        let (sp, s) = (spin.clone(), seen.clone());
        after(300, move || {
            let sb = sp();
            sb.set_value(171.0); // output: JS writes "0xAB"
            s.borrow_mut().text_after_set = sb.text().to_string();
        });
        let (sp, s) = (spin.clone(), seen.clone());
        after(500, move || {
            let sb = sp();
            sb.set_text("0x10"); // input: JS answers {newValue: 16}
            let _ = sb.update();
            s.borrow_mut().value_after_input = sb.value();
            sb.set_text("num"); // shorthand: a bare number
            let _ = sb.update();
            s.borrow_mut().value_after_shorthand = sb.value();
            sb.set_text("plain");
            let _ = sb.update();
            s.borrow_mut().value_after_plain = sb.value();
            sb.set_value(100.0);
            sb.set_text("bad"); // ret -1: conversion failed
            let _ = sb.update();
            s.borrow_mut().value_after_error = sb.value();
        });
        let b = backend.clone();
        after(900, move || b.object(WIN).unwrap().downcast::<gtk::Window>().unwrap().close());
        let raw2 = raw;
        after(8000, move || raw2.quit());

        host.run().unwrap();
        let errs: Vec<String> = host.take_errors().into_iter().map(|e| e.message).collect();
        assert!(errs.is_empty(), "{errs:?}");
        let st = seen.borrow();
        assert_eq!(st.text_after_set, "0xAB", "the output handler displayed the value");
        assert_eq!(st.value_after_input, 16.0, "{{newValue: 16}} filled the out-parameter");
        assert_eq!(st.value_after_shorthand, 7.0, "a bare number is {{ret: 1, newValue: n}}");
        assert_eq!(st.value_after_plain, 7.0, "null leaves the conversion to GTK (\"plain\" is not a number: value kept)");
        assert_eq!(st.value_after_error, 100.0, "ret -1 is GTK_INPUT_ERROR: the value is kept");
    });
}
