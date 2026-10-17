//! X7 (G12) end to end on GTK: `bind:text` on a text view goes through its buffer and `bind:value`
//! on a scale through its adjustment. The real host runs the `fixtures/buffer-bind` bundle; native
//! edits reach the app state and the app's writes reach the buffer and the adjustment. Needs a
//! display (xvfb-run) and the built fixture (`pnpm -r build`); skipped without the bundle.

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
const VIEW: u64 = 3;
const TEXT_LABEL: u64 = 4;
const SCALE: u64 = 5;
const VALUE_LABEL: u64 = 6;
const BUTTON: u64 = 7;

type Snapshot = (String, String, f64, String);

fn snapshot(b: &GtkBackend) -> Snapshot {
    let view = b.object(VIEW).unwrap().downcast::<gtk::TextView>().unwrap();
    let buf = view.buffer();
    let text = buf.text(&buf.start_iter(), &buf.end_iter(), false).to_string();
    let label = |h| b.object(h).unwrap().downcast::<gtk::Label>().unwrap().label().to_string();
    let scale = b.object(SCALE).unwrap().downcast::<gtk::Scale>().unwrap();
    (text, label(TEXT_LABEL), scale.value(), label(VALUE_LABEL))
}

#[test]
fn text_view_text_and_scale_value_are_two_way_bound_through_buffer_and_adjustment() {
    let bundle = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/buffer-bind/dist/app.js");
    if !bundle.exists() {
        eprintln!("skipped: {} is not built", bundle.display());
        return;
    }
    on_gtk(move || {
        let backend = Rc::new(GtkBackend::new().expect("gtk"));
        let raw = unsafe { BackendRef::from_raw(backend.as_raw()) }.unwrap();
        let cfg = HostConfig { echo_console: false, echo_errors: false, ..HostConfig::default() };
        let host = Host::new(raw, cfg).unwrap();
        host.load_bundle(bundle).unwrap();

        let seen: Rc<RefCell<Vec<Snapshot>>> = Rc::default();
        let (b, s) = (backend.clone(), seen.clone());
        glib::timeout_add_local_once(Duration::from_millis(300), move || {
            s.borrow_mut().push(snapshot(&b));
            // the user types into the buffer and drags the slider
            let view = b.object(VIEW).unwrap().downcast::<gtk::TextView>().unwrap();
            view.buffer().set_text("typed");
            b.object(SCALE).unwrap().downcast::<gtk::Range>().unwrap().adjustment().set_value(40.0);
        });
        let (b, s) = (backend.clone(), seen.clone());
        glib::timeout_add_local_once(Duration::from_millis(600), move || {
            s.borrow_mut().push(snapshot(&b));
            b.object(BUTTON).unwrap().downcast::<gtk::Button>().unwrap().emit_clicked();
        });
        let (b, s) = (backend.clone(), seen.clone());
        glib::timeout_add_local_once(Duration::from_millis(900), move || {
            s.borrow_mut().push(snapshot(&b));
            b.object(WIN).unwrap().downcast::<gtk::Window>().unwrap().close();
        });
        let raw2 = raw;
        glib::timeout_add_local_once(Duration::from_millis(8000), move || raw2.quit());

        host.run().unwrap();
        let errs: Vec<String> = host.take_errors().into_iter().map(|e| e.message).collect();
        assert!(errs.is_empty(), "{errs:?}");
        let seen = seen.borrow();
        let want = |a: &str, b: &str, c: f64, d: &str| (a.to_string(), b.to_string(), c, d.to_string());
        assert_eq!(seen[0], want("start", "text=start", 10.0, "value=10"), "the app's initial state reached the widgets");
        assert_eq!(seen[1], want("typed", "text=typed", 40.0, "value=40"), "native edits reached the app state");
        assert_eq!(seen[2], want("from js", "text=from js", 55.0, "value=55"), "the app's writes reached the widgets");
    });
}
