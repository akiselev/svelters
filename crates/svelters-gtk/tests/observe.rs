//! ABI v2.6 `observe` / `prop_changed` on GTK: `notify::<prop>` on widgets (generated ids), on
//! non-widget objects (reflected ids) and on forwarded (`via`) properties; nested on/off; destroy and
//! native death disconnect; an observation never keeps the object alive. Mechanics only: no value,
//! no comparison (plans/research/REACTIVITY.md §12 decides the rest). Needs a display (xvfb-run).

use gtk::prelude::*;
use svelters_abi::*;
use svelters_gtk::testing::{on_gtk, pump, Harness, V};

fn s(x: &str) -> V {
    V::Str(x.into())
}

fn changed_of(h: &Harness, handle: u64, prop: u32) -> usize {
    h.take_changed().into_iter().filter(|c| *c == (handle, prop)).count()
}

#[test]
fn a_widget_property_notifies_while_observed_and_calls_nest() {
    on_gtk(|| {
        let h = Harness::new();
        let l = h.create("label");
        let pid = h.resolve_prop("label", "label");
        let label: gtk::Label = h.obj(l).downcast().unwrap();

        // Not observed: nothing reaches the host.
        label.set_label("a");
        assert!(h.take_changed().is_empty());

        h.observe(l, "label", true).unwrap();
        label.set_label("b");
        assert_eq!(h.take_changed(), vec![(l, pid)], "a native change is announced synchronously, without a value");
        // A write through the ABI notifies too (echo handling is the host's business, §12).
        h.set_str(l, "label", "c").unwrap();
        assert_eq!(changed_of(&h, l, pid), 1);
        // Another property of the same widget is not observed.
        label.set_selectable(true);
        assert!(h.take_changed().is_empty());

        // Nested: two `on`, one `off` keeps the connection.
        h.observe(l, "label", true).unwrap();
        h.observe(l, "label", false).unwrap();
        label.set_label("d");
        assert_eq!(changed_of(&h, l, pid), 1);
        h.observe(l, "label", false).unwrap();
        label.set_label("e");
        assert!(h.take_changed().is_empty(), "the matching last off disconnects");
        // `off` without `on` is not an error.
        h.observe(l, "label", false).unwrap();
    });
}

#[test]
fn a_non_widget_property_notifies_on_native_edits() {
    on_gtk(|| {
        let h = Harness::new();
        let buf = h.construct("Gtk.TextBuffer", &[("text", s("hello"))]).unwrap();
        assert_eq!(h.handle_info(buf).unwrap().lifetime, SVL_LIFE_SHARED);
        let text = h.resolve_prop("Gtk.TextBuffer", "text");
        let cursor = h.resolve_prop("Gtk.TextBuffer", "cursor-position");
        h.observe(buf, "text", true).unwrap();
        h.observe(buf, "cursor-position", true).unwrap();

        let b: gtk::TextBuffer = h.obj(buf).downcast().unwrap();
        b.insert_at_cursor("!");
        let got = h.take_changed();
        assert!(got.contains(&(buf, text)) && got.contains(&(buf, cursor)), "{got:?}");
        // The host reads the value itself when it wants it.
        assert_eq!(h.get(buf, "text"), s("hello!"));

        h.observe(buf, "cursor-position", false).unwrap();
        // set_text is a delete and an insert: one notify each, no coalescing (that is §12's).
        b.set_text("x");
        let got = h.take_changed();
        assert!(!got.is_empty() && got.iter().all(|c| *c == (buf, text)), "{got:?}");
        h.observe(buf, "text", false).unwrap();
    });
}

#[test]
fn observe_and_a_notify_listener_are_independent_connections() {
    on_gtk(|| {
        let h = Harness::new();
        let store = h.construct("Gio.ListStore", &[("item-type", s("Gio.File"))]).unwrap();
        if !h.has_prop(store, "n-items") {
            eprintln!("SKIP: GListStore:n-items needs GLib 2.74");
            return;
        }
        let n = h.resolve_prop("Gio.ListStore", "n-items");
        h.listen(store, "notify::nItems", true).unwrap();
        h.observe(store, "n-items", true).unwrap();
        let file = h.call_static("Gio.File", "new_for_path", &[s("/tmp")]).unwrap();
        h.call_args(store, "append", &[file.clone()]).unwrap();
        assert_eq!(changed_of(&h, store, n), 1);
        let ev: Vec<String> = h.take_events().into_iter().map(|(_, s)| s).collect();
        assert_eq!(ev.len(), 1, "{ev:?}");
        // Dropping the observation leaves the listener alone, and the other way round.
        h.observe(store, "n-items", false).unwrap();
        h.call_args(store, "append", &[file.clone()]).unwrap();
        assert!(h.take_changed().is_empty());
        assert_eq!(h.take_events().len(), 1);
        h.listen(store, "notify::nItems", false).unwrap();
        h.observe(store, "n-items", true).unwrap();
        h.call_args(store, "append", &[file]).unwrap();
        assert_eq!(changed_of(&h, store, n), 1);
        assert!(h.take_events().is_empty());
    });
}

#[test]
fn a_reflected_property_of_a_type_without_tables_notifies() {
    on_gtk(|| {
        let h = Harness::new();
        let store = h.construct("Gio.ListStore", &[("item-type", s("Gio.File"))]).unwrap();
        if !h.has_prop(store, "n-items") {
            eprintln!("SKIP: GListStore:n-items needs GLib 2.74");
            return;
        }
        let n = h.resolve_prop("Gio.ListStore", "n-items");
        assert!(n >= 0x0100_0000, "a reflected property id");
        h.observe(store, "n-items", true).unwrap();
        let file = h.call_static("Gio.File", "new_for_path", &[s("/tmp")]).unwrap();
        h.call_args(store, "append", &[file]).unwrap();
        assert_eq!(changed_of(&h, store, n), 1);
        h.observe(store, "n-items", false).unwrap();
        h.call_args(store, "remove_all", &[]).unwrap();
        assert!(h.take_changed().is_empty());
    });
}

#[test]
fn a_forwarded_property_is_watched_on_the_object_it_lives_on() {
    on_gtk(|| {
        let h = Harness::new();
        let sc = h.create("scale");
        let pid = h.resolve_prop("scale", "value");
        assert_ne!(pid, 0);
        h.set_value(sc, "upper", &Value::F64(100.0)).unwrap();
        h.observe(sc, "value", true).unwrap();
        let range: gtk::Range = h.obj(sc).downcast().unwrap();
        range.adjustment().set_value(42.0);
        assert_eq!(changed_of(&h, sc, pid), 1);
        h.observe(sc, "value", false).unwrap();
        range.adjustment().set_value(7.0);
        assert!(h.take_changed().is_empty());
    });
}

#[test]
fn destroy_drops_the_observation_and_the_object() {
    on_gtk(|| {
        let h = Harness::new();
        let buf = h.construct("Gtk.TextBuffer", &[]).unwrap();
        let text = h.resolve_prop("Gtk.TextBuffer", "text");
        h.observe(buf, "text", true).unwrap();
        h.observe(buf, "text", true).unwrap();
        let obj: gtk::TextBuffer = h.obj(buf).downcast().unwrap();
        let weak = obj.downgrade();
        h.destroy(buf);
        // Our own strong reference keeps the buffer here: its notify must not reach the host any more.
        obj.set_text("after destroy");
        assert!(h.take_changed().is_empty(), "destroy disconnected the nested observation");
        drop(obj);
        pump(50);
        assert!(weak.upgrade().is_none(), "nothing else holds the buffer: observation kept no reference");
        // Observing a dead handle fails; `off` on it is a no-op.
        let (rc, msg) = h.observe_id(buf, text, true).unwrap_err();
        assert_eq!(rc, -1, "{msg}");
        h.observe_id(buf, text, false).unwrap();
    });
}

#[test]
fn an_observed_object_from_a_getter_dies_when_replaced_and_released() {
    on_gtk(|| {
        let h = Harness::new();
        let tv = h.create("text-view");
        let b = match h.call_args(tv, "get_buffer", &[]).unwrap() {
            V::Handle(b) => b,
            other => panic!("{other:?}"),
        };
        assert_eq!(h.handle_info(b).unwrap().lifetime, SVL_LIFE_SHARED);
        h.observe(b, "text", true).unwrap();
        let weak = h.obj(b).downgrade();
        // Replace the view's buffer natively, then let JS's handle go: the observation must not be
        // what keeps the old buffer alive.
        let view: gtk::TextView = h.obj(tv).downcast().unwrap();
        view.set_buffer(Some(&gtk::TextBuffer::new(None)));
        h.destroy(b);
        pump(50);
        assert!(weak.upgrade().is_none(), "the replaced buffer was finalized");
        assert!(h.take_changed().is_empty());
    });
}

#[test]
fn a_widget_observed_inside_a_window_is_released_when_the_window_closes() {
    on_gtk(|| {
        let h = Harness::new();
        let win = h.create("window");
        let l = h.create("label");
        h.insert(win, l, None, None).unwrap();
        h.observe(l, "label", true).unwrap();
        h.observe(win, "title", true).unwrap();
        let label: gtk::Label = h.obj(l).downcast().unwrap();
        let weak = label.downgrade();
        h.call(win, "present").unwrap();
        pump(150);
        h.widget(win).downcast::<gtk::Window>().unwrap().close();
        pump(100);
        let gone: Vec<u64> = h.take_destroyed();
        assert!(gone.contains(&win) && gone.contains(&l), "{gone:?}");
        // The label is alive while we hold it, but its notify reaches nobody.
        label.set_label("late");
        assert!(h.take_changed().is_empty());
        drop(label);
        pump(50);
        assert!(weak.upgrade().is_none());
    });
}

#[test]
fn refusals_say_why() {
    on_gtk(|| {
        let h = Harness::new();
        // A virtual (svelters-computed) property has no notify: unsupported, not an error.
        let hb = h.create("header-bar");
        let (rc, msg) = h.observe(hb, "title", true).unwrap_err();
        assert_eq!(rc, SVL_OBSERVE_UNSUPPORTED, "{msg}");
        // A construct-only property never changes.
        let buf = h.construct("Gtk.TextBuffer", &[]).unwrap();
        let (rc, msg) = h.observe(buf, "tag-table", true).unwrap_err();
        assert_eq!(rc, -1, "{msg}");
        assert!(msg.contains("construct-only"), "{msg}");
        // A property of another kind.
        let l = h.create("label");
        let other = h.resolve_prop("button", "label");
        let lbl = h.resolve_prop("label", "label");
        if other != lbl {
            assert_eq!(h.observe_id(l, other, true).unwrap_err().0, -1);
        }
    });
}

#[test]
fn descriptors_flag_observable_properties() {
    on_gtk(|| {
        let h = Harness::new();
        let b = h.backend_ref();
        let flags = |kind: &str, prop: &str| {
            let k = h.kind_id(kind);
            b.describe(NameKind::Prop, k, h.resolve_prop(kind, prop)).map(|d| d.flags).unwrap_or(0)
        };
        assert_ne!(flags("label", "label") & SVL_M_OBSERVABLE, 0);
        assert_ne!(flags("scale", "value") & SVL_M_OBSERVABLE, 0);
        assert_eq!(flags("header-bar", "title") & SVL_M_OBSERVABLE, 0, "virtual");
        assert_ne!(flags("Gtk.TextBuffer", "text") & SVL_M_OBSERVABLE, 0);
        assert_eq!(flags("Gtk.TextBuffer", "tag-table") & SVL_M_OBSERVABLE, 0, "construct-only");
    });
}
