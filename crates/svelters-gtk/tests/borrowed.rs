//! Borrowed handles on the GTK backend (plans/milestones/X1-borrowed-gi.md): stable identity,
//! weak ownership, death reasons and the insert/remove guard. Needs a display (xvfb-run).

use gtk::prelude::*;
use svelters_abi::{SVL_GONE_CLOSED, SVL_GONE_OWNER, SVL_GONE_PARENT, SVL_LIFE_BORROWED, SVL_LIFE_OWNED};
use svelters_gtk::testing::{on_gtk, pump, Harness, V};

fn handle(v: V) -> u64 {
    match v {
        V::Handle(h) => h,
        other => panic!("expected a handle, got {other:?}"),
    }
}

/// window > box > entry; the entry's GtkText child is native-made.
fn tree(h: &Harness) -> (u64, u64, u64) {
    let win = h.create("window");
    let bx = h.create("box");
    let entry = h.create("entry");
    h.insert(bx, entry, None, None).unwrap();
    h.insert(win, bx, None, None).unwrap();
    h.call(win, "present").unwrap();
    pump(100);
    (win, bx, entry)
}

#[test]
fn two_getters_return_the_same_borrowed_handle() {
    on_gtk(|| {
        let h = Harness::new();
        let (win, bx, entry) = tree(&h);

        let text = handle(h.call_args(entry, "getFirstChild", &[]).unwrap());
        assert!(![win, bx, entry].contains(&text));
        let info = h.handle_info(text).unwrap();
        assert_eq!(info.lifetime, SVL_LIFE_BORROWED);
        assert_eq!(info.kind, h.kind_id("Text"), "kind comes from the object's own type");

        // Same object through another getter, and again through the same one.
        assert_eq!(handle(h.call_args(entry, "get_first_child", &[]).unwrap()), text);
        assert_eq!(handle(h.call_args(entry, "get_last_child", &[]).unwrap()), text, "the entry has one native child");

        // Objects we created keep their own handles, whatever path reaches them.
        assert_eq!(handle(h.call_args(text, "getParent", &[]).unwrap()), entry);
        assert_eq!(handle(h.call_args(entry, "get_parent", &[]).unwrap()), bx);
        assert_eq!(handle(h.call_args(text, "getRoot", &[]).unwrap()), win, "the root is our window's TOPLEVEL handle");
        assert_eq!(h.handle_info(entry).unwrap().lifetime, SVL_LIFE_OWNED);

        // Borrowed handles take the same calls and properties as any other.
        assert_eq!(h.call_args(text, "get_visible", &[]).unwrap(), V::Bool(true));
        h.set_str(entry, "text", "typed").unwrap();
        assert_eq!(h.get(text, "text"), V::Str("typed".into()));
    });
}

#[test]
fn borrowed_handles_cannot_enter_the_tree() {
    on_gtk(|| {
        let h = Harness::new();
        let (_win, bx, entry) = tree(&h);
        let text = handle(h.call_args(entry, "getFirstChild", &[]).unwrap());
        let other = h.create("box");
        assert!(h.insert(bx, text, None, None).unwrap_err().contains("borrowed"));
        assert!(h.insert(text, other, None, None).unwrap_err().contains("borrowed"));
        assert!(h.remove(entry, text).unwrap_err().contains("borrowed"));
        assert!(h.take_gone().is_empty());
    });
}

#[test]
fn destroy_on_a_borrowed_handle_only_forgets_it() {
    on_gtk(|| {
        let h = Harness::new();
        let (_win, _bx, entry) = tree(&h);
        let text = handle(h.call_args(entry, "getFirstChild", &[]).unwrap());
        let weak = h.obj(text).downgrade();

        h.destroy(text);
        h.destroy(text); // idempotent
        pump(50);
        assert!(h.handle_info(text).is_err(), "stale as soon as destroy returns");
        assert!(h.call_args(text, "get_visible", &[]).is_err());
        assert!(weak.upgrade().is_some(), "the native owner still has it");
        assert!(h.take_gone().is_empty(), "the app's own destroy is silent");

        // The object is still reachable: a new handle, never the old number.
        let again = handle(h.call_args(entry, "getFirstChild", &[]).unwrap());
        assert!(again > text);
        assert_eq!(h.handle_info(again).unwrap().lifetime, SVL_LIFE_BORROWED);
    });
}

#[test]
fn native_death_of_the_owner_reports_owner() {
    on_gtk(|| {
        let h = Harness::new();
        let (_win, bx, entry) = tree(&h);
        let text = handle(h.call_args(entry, "getFirstChild", &[]).unwrap());
        let weak = h.obj(text).downgrade();

        // Destroying the entry (the renderer's `destroy` after a removal) disposes its internals.
        h.remove(bx, entry).unwrap();
        h.destroy(entry);
        pump(200);
        assert!(weak.upgrade().is_none());
        assert_eq!(h.take_gone(), vec![(text, SVL_GONE_OWNER)]);
        assert!(h.handle_info(text).is_err());
        h.destroy(text); // late release: harmless
        assert!(h.take_gone().is_empty(), "at most once");
    });
}

#[test]
fn closing_the_window_cascades_to_borrowed_children() {
    on_gtk(|| {
        let h = Harness::new();
        let (win, bx, entry) = tree(&h);
        let text = handle(h.call_args(entry, "getFirstChild", &[]).unwrap());

        h.widget(win).downcast::<gtk::Window>().unwrap().close();
        pump(100);
        // Window first, then everything below it in pre-order, the borrowed child last.
        assert_eq!(h.take_gone(), vec![(win, SVL_GONE_CLOSED), (bx, SVL_GONE_PARENT), (entry, SVL_GONE_PARENT), (text, SVL_GONE_PARENT)]);
        assert!(h.handle_info(text).is_err());
        pump(100);
        assert!(h.take_gone().is_empty(), "each handle is reported once; the weak notify stays quiet");
    });
}

#[test]
fn minting_does_not_keep_objects_alive() {
    on_gtk(|| {
        let h = Harness::new();
        let (_win, bx, entry) = tree(&h);
        let obj = h.obj(entry).downcast::<gtk::Widget>().unwrap().first_child().unwrap();
        let before = obj.ref_count();
        let text = handle(h.call_args(entry, "getFirstChild", &[]).unwrap());
        drop(h.obj(text));
        assert_eq!(obj.ref_count(), before, "a borrowed handle holds no strong reference");
        let weak = obj.downgrade();
        drop(obj);

        h.remove(bx, entry).unwrap();
        h.destroy(entry);
        pump(200);
        assert!(weak.upgrade().is_none(), "the object died with its owner although a handle for it existed");
    });
}

#[test]
fn objects_that_are_not_widgets_travel_as_shared_handles() {
    on_gtk(|| {
        let h = Harness::new();
        let (_win, _bx, entry) = tree(&h);
        // gtk_widget_create_pango_layout returns a new PangoLayout (transfer full): every GObject is a
        // handle, SHARED when it is not a widget, released with `destroy`.
        let V::Handle(layout) = h.call_args(entry, "create_pango_layout", &[V::Str("hello".into())]).unwrap() else { panic!("handle") };
        assert_eq!(h.handle_info(layout).unwrap().lifetime, svelters_abi::SVL_LIFE_SHARED, "a non-widget object is SHARED");
        // It has a kind and methods now.
        assert_eq!(h.call_args(layout, "get_text", &[]).unwrap(), V::Str("hello".into()));
        h.destroy(layout);
        assert!(h.handle_info(layout).is_err(), "released: stale");
        h.destroy(layout);
        assert!(h.take_gone().is_empty());
    });
}

/// A1 gap G3/G2 repro: `text-view.getProp('buffer')` is a handle that takes method calls with
/// arguments. A buffer is not a widget, so it is SHARED (a strong reference): it outlives its view.
#[test]
fn text_buffer_is_a_shared_handle_with_working_methods() {
    on_gtk(|| {
        let h = Harness::new();
        let win = h.create("window");
        let tv = h.create("text-view");
        h.insert(win, tv, None, None).unwrap();
        let buf = handle(h.get(tv, "buffer"));
        assert_eq!(h.handle_info(buf).unwrap().lifetime, svelters_abi::SVL_LIFE_SHARED);
        assert_eq!(handle(h.get(tv, "buffer")), buf, "stable identity");
        h.call_args(buf, "set_text", &[V::Str("hello".into()), V::I64(-1)]).unwrap();
        assert_eq!(h.call_args(buf, "get_char_count", &[]).unwrap(), V::I64(5));
        h.call_args(buf, "insert_at_cursor", &[V::Str("!!".into()), V::I64(-1)]).unwrap();
        assert_eq!(h.call_args(buf, "get_char_count", &[]).unwrap(), V::I64(7));
        h.remove(win, tv).unwrap();
        h.destroy(tv);
        pump(200);
        assert!(h.take_gone().is_empty(), "a SHARED object does not die with its owner: we hold it");
        assert_eq!(h.call_args(buf, "get_char_count", &[]).unwrap(), V::I64(7));
        h.destroy(buf);
        assert!(h.handle_info(buf).is_err());
    });
}
