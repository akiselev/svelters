//! ABI v2 lifetime behaviour of the GTK backend: handle info, idempotent destroy, deferred
//! teardown, onRequestClose and the cascade of a native close. Needs a display (xvfb-run).

use gtk::prelude::*;
use svelters_abi::{SVL_GONE_CLOSED, SVL_GONE_PARENT, SVL_LIFE_OWNED, SVL_LIFE_TOPLEVEL};
use svelters_gtk::testing::{on_gtk, pump, Harness};

#[test]
fn handle_info_and_idempotent_destroy() {
    on_gtk(|| {
        let h = Harness::new();
        let win = h.create("window");
        let label = h.create("label");
        assert!(win < 1 << 53 && label < 1 << 53);
        let wi = h.handle_info(win).unwrap();
        assert_eq!((wi.kind, wi.lifetime), (h.kind_id("window"), SVL_LIFE_TOPLEVEL));
        let li = h.handle_info(label).unwrap();
        assert_eq!((li.kind, li.lifetime), (h.kind_id("label"), SVL_LIFE_OWNED));

        h.destroy(label);
        h.destroy(label); // idempotent
        h.destroy(424_242); // never issued: harmless
        assert!(h.handle_info(label).is_err(), "stale as soon as destroy returns");
        assert!(h.set_str(label, "label", "x").is_err());
        assert!(h.take_gone().is_empty(), "the app's own destroy is silent");

        // Handles are never reused.
        let again = h.create("label");
        assert!(again > label);

        h.destroy(win);
        h.destroy(win);
        pump(100);
        assert!(h.take_gone().is_empty(), "a destroyed window does not report itself");
    });
}

#[test]
fn destroy_inside_a_signal_is_deferred() {
    on_gtk(|| {
        let h = Harness::new();
        let win = h.create("window");
        let bx = h.create("box");
        let button = h.create("button");
        h.insert(win, bx, None, None).unwrap();
        h.insert(bx, button, None, None).unwrap();
        h.listen(button, "clicked", true).unwrap();
        h.call(win, "present").unwrap();
        pump(100);

        let weak = h.widget(button).downgrade();
        let btn = h.widget(button).downcast::<gtk::Button>().unwrap();
        // A second handler on the same emission checks that the button is still alive while the
        // first one (the backend's) has already seen the app destroy it.
        let alive_in_signal = std::rc::Rc::new(std::cell::Cell::new(false));
        let flag = alive_in_signal.clone();
        let wk = weak.clone();
        btn.connect_clicked(move |_| flag.set(wk.upgrade().is_some()));
        drop(btn);
        h.widget(button).downcast::<gtk::Button>().unwrap().emit_clicked();
        h.remove(bx, button).unwrap();
        h.destroy(button); // as the renderer does after a keyed removal, possibly from a handler
        assert!(alive_in_signal.get());
        pump(300);
        assert!(weak.upgrade().is_none(), "finalized from an idle after destroy");
        assert!(h.handle_info(button).is_err());
    });
}

#[test]
fn close_request_is_vetoed_while_listened_and_destroys_otherwise() {
    on_gtk(|| {
        let h = Harness::new();
        let win = h.create("window");
        let label = h.create("label");
        h.insert(win, label, None, None).unwrap();
        h.call(win, "present").unwrap();
        pump(150);

        h.listen(win, "close-request", true).unwrap();
        h.widget(win).downcast::<gtk::Window>().unwrap().close();
        pump(100);
        assert_eq!(h.take_events(), vec![(win, "close-request".to_string())]);
        assert!(h.take_gone().is_empty());
        assert!(h.handle_info(win).is_ok(), "vetoed: the window is still there");

        h.listen(win, "close-request", false).unwrap();
        h.widget(win).downcast::<gtk::Window>().unwrap().close();
        pump(100);
        // Window first, then everything inserted below it; each exactly once.
        assert_eq!(h.take_gone(), vec![(win, SVL_GONE_CLOSED), (label, SVL_GONE_PARENT)]);
        assert!(h.handle_info(win).is_err());
        assert!(h.handle_info(label).is_err());
        h.destroy(win); // stale: ignored
        pump(50);
        assert!(h.take_gone().is_empty(), "at most once");
    });
}
