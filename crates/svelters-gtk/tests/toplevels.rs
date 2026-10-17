//! X7 (G9): a TOPLEVEL child inserted under a parent is transient for the parent's window
//! (plans/architecture/backend-abi.md, "Toplevels in the tree"). Needs a display (xvfb-run).

use gtk::prelude::*;
use svelters_abi::{SVL_GONE_CLOSED, SVL_GONE_PARENT};
use svelters_gtk::testing::{on_gtk, pump, Harness};

fn transient_of(h: &Harness, dialog: u64) -> Option<gtk::Window> {
    h.widget(dialog).downcast::<gtk::Window>().unwrap().transient_for()
}

#[test]
fn a_toplevel_child_is_transient_for_the_window_of_its_parent() {
    on_gtk(|| {
        let h = Harness::new();
        let win = h.create("window");
        let bx = h.create("box");
        let about = h.create("aboutdialog");
        h.insert(win, bx, None, None).unwrap();
        // under a plain widget: the window that contains it
        h.insert(bx, about, None, None).unwrap();
        assert_eq!(transient_of(&h, about), Some(h.widget(win).downcast::<gtk::Window>().unwrap()));
        assert!(h.insert(win, about, None, None).is_err(), "already inserted");
        h.remove(bx, about).unwrap();
        assert_eq!(transient_of(&h, about), None);
        assert!(h.remove(bx, about).is_err(), "not inserted any more");
        h.insert(win, about, None, None).unwrap();
        assert_eq!(transient_of(&h, about), Some(h.widget(win).downcast::<gtk::Window>().unwrap()));
        h.remove(win, about).unwrap();
        h.destroy(about);
        h.destroy(win);
        pump(100);
    });
}

#[test]
fn a_window_cannot_be_transient_for_itself_and_the_parent_must_be_in_a_window() {
    on_gtk(|| {
        let h = Harness::new();
        let win = h.create("window");
        let loose = h.create("box");
        let about = h.create("aboutdialog");
        assert!(h.insert(win, win, None, None).is_err());
        assert!(h.insert(loose, about, None, None).unwrap_err().contains("not inside a window"));
        h.destroy(about);
        h.destroy(loose);
        h.destroy(win);
        pump(100);
    });
}

#[test]
fn a_closed_parent_takes_its_transient_dialogs_with_it() {
    on_gtk(|| {
        let h = Harness::new();
        let win = h.create("window");
        let about = h.create("aboutdialog");
        h.insert(win, about, None, None).unwrap();
        h.call(win, "present").unwrap();
        h.set_bool(about, "modal", true).unwrap();
        h.call(about, "present").unwrap();
        pump(100);
        assert!(h.widget(about).is_visible());
        let weak = h.widget(about).downgrade();
        h.widget(win).downcast::<gtk::Window>().unwrap().destroy();
        pump(200);
        let gone = h.take_gone();
        assert_eq!(gone, vec![(win, SVL_GONE_CLOSED), (about, SVL_GONE_PARENT)]);
        assert!(weak.upgrade().is_none_or(|w| !w.is_visible()), "the dialog is gone with its parent");
    });
}

#[test]
fn a_dialog_closed_on_its_own_is_reported_closed() {
    on_gtk(|| {
        let h = Harness::new();
        let win = h.create("window");
        let about = h.create("aboutdialog");
        h.insert(win, about, None, None).unwrap();
        h.call(win, "present").unwrap();
        h.call(about, "present").unwrap();
        pump(100);
        h.widget(about).downcast::<gtk::Window>().unwrap().close();
        pump(200);
        assert_eq!(h.take_gone(), vec![(about, SVL_GONE_CLOSED)]);
        assert!(h.handle_info(win).is_ok(), "the parent lives on");
        h.destroy(win);
        pump(100);
    });
}
