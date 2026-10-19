//! ABI v2.6 end to end on GTK: the real host runs `tests/js/observe_e2e.js`, which observes a
//! TextBuffer's `text`. A native edit reaches `__svelters_prop_changed` at the next checkpoint; the
//! handler reads the text live and stops observing, so a second edit reaches nobody. Needs a
//! display (xvfb-run).

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

const BUF: u64 = 1;
const LABEL: u64 = 3;

fn after(ms: u64, f: impl FnOnce() + 'static) {
    glib::timeout_add_local_once(Duration::from_millis(ms), f);
}

#[test]
fn a_native_edit_of_an_observed_buffer_reaches_js() {
    on_gtk(|| {
        let backend = Rc::new(GtkBackend::new().expect("gtk"));
        let raw = unsafe { BackendRef::from_raw(backend.as_raw()) }.unwrap();
        let cfg = HostConfig { echo_console: false, echo_errors: false, ..HostConfig::default() };
        let host = Rc::new(Host::new(raw, cfg).unwrap());
        host.load_bundle(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/js/observe_e2e.js")).unwrap();
        assert_eq!(host.observed_props(), 1);

        let labels: Rc<RefCell<Vec<String>>> = Rc::default();
        let label_text = {
            let b = backend.clone();
            move || b.object(LABEL).unwrap().downcast::<gtk::Label>().unwrap().label().to_string()
        };
        let (b, l, t) = (backend.clone(), labels.clone(), label_text.clone());
        after(300, move || {
            l.borrow_mut().push(t());
            b.object(BUF).unwrap().downcast::<gtk::TextBuffer>().unwrap().insert_at_cursor("b");
        });
        let (b, l, t) = (backend.clone(), labels.clone(), label_text.clone());
        after(600, move || {
            l.borrow_mut().push(t());
            b.object(BUF).unwrap().downcast::<gtk::TextBuffer>().unwrap().insert_at_cursor("c");
        });
        let (l, t, hst) = (labels.clone(), label_text.clone(), host.clone());
        after(900, move || {
            l.borrow_mut().push(t());
            l.borrow_mut().push(format!("observed={}", hst.observed_props()));
            raw.quit();
        });
        host.run().unwrap();
        let errors = host.take_errors();
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(*labels.borrow(), ["observed=true", "#1 text=ab", "#1 text=ab", "observed=0"]);
    });
}
