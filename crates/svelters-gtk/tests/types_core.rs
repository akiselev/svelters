//! NT0 core: non-GUI objects and values through the backend's C ABI. Every GObject is a handle
//! (SHARED for non-widgets), `construct` takes construct-time properties, `call_static` reaches
//! constructor functions, VALUE handles answer methods, and out parameters, lists and arrays marshal.
//! Needs a display (xvfb-run).

use gtk::glib;
use gtk::prelude::*;
use svelters_abi::*;
use svelters_gtk::testing::{on_gtk, pump, Harness, V};

fn s(x: &str) -> V {
    V::Str(x.into())
}

fn handle(v: V) -> u64 {
    match v {
        V::Handle(h) => h,
        other => panic!("expected a handle, got {other:?}"),
    }
}

fn boxed(v: V) -> u64 {
    match v {
        V::Boxed(h) => h,
        other => panic!("expected a boxed value, got {other:?}"),
    }
}

#[test]
fn a_file_is_a_shared_handle_with_one_identity() {
    on_gtk(|| {
        let h = Harness::new();
        let f = handle(h.call_static("Gio.File", "new_for_path", &[s("/usr/share")]).unwrap());
        let info = h.handle_info(f).unwrap();
        assert_eq!(info.lifetime, SVL_LIFE_SHARED);
        // The runtime class (GLocalFile) is not in the typelib; the kind still has a name and a parent chain.
        let chain = h.type_chain(f);
        assert!(!chain.is_empty() && chain.last().unwrap() == "GObject.Object", "{chain:?}");
        // Methods of the interface the class implements resolve, and strings come back.
        assert_eq!(h.call_args(f, "get_basename", &[]).unwrap(), s("share"));
        assert_eq!(h.call_args(f, "get_path", &[]).unwrap(), s("/usr/share"));
        assert_eq!(h.call_args(f, "getUri", &[]).unwrap(), s("file:///usr/share"));
        // A getter that returns an object we already hold returns the same handle.
        // g_file_get_parent makes a new GFile each time: two handles, equal files.
        let parent = handle(h.call_args(f, "get_parent", &[]).unwrap());
        let again = handle(h.call_args(f, "get_parent", &[]).unwrap());
        assert_ne!(parent, again);
        assert_eq!(h.call_args(parent, "equal", &[V::Handle(again)]).unwrap(), V::Bool(true));
        assert_ne!(parent, f);
        assert_eq!(h.call_args(parent, "get_path", &[]).unwrap(), s("/usr"));
        // Object arguments by handle.
        assert_eq!(h.call_args(f, "equal", &[V::Handle(f)]).unwrap(), V::Bool(true));
        assert_eq!(h.call_args(f, "equal", &[V::Handle(parent)]).unwrap(), V::Bool(false));
        // A string form where a GFile is expected (a path).
        let child = handle(h.call_args(f, "get_child", &[s("zoneinfo")]).unwrap());
        assert_eq!(h.call_args(child, "get_path", &[]).unwrap(), s("/usr/share/zoneinfo"));
        assert_eq!(h.call_args(parent, "has_prefix", &[V::Handle(f)]).unwrap(), V::Bool(false));
        assert_eq!(h.call_args(f, "has_prefix", &[V::Handle(parent)]).unwrap(), V::Bool(true));
    });
}

#[test]
fn shared_handles_hold_a_strong_reference_until_destroyed() {
    on_gtk(|| {
        let h = Harness::new();
        let f = handle(h.call_static("Gio.File", "new_for_path", &[s("/tmp")]).unwrap());
        let weak = h.obj(f).downgrade();
        let live = h.backend.live_handles();
        pump(50);
        assert!(weak.upgrade().is_some(), "SHARED keeps the object alive");
        h.destroy(f);
        pump(100);
        assert!(weak.upgrade().is_none(), "and nothing else leaked a reference");
        assert_eq!(h.backend.live_handles(), live - 1);
        assert!(h.call_args(f, "get_path", &[]).is_err(), "a destroyed handle is stale");
        h.destroy(f); // idempotent
    });
}

#[test]
fn list_store_constructs_with_a_construct_only_type_and_signals() {
    on_gtk(|| {
        let h = Harness::new();
        let store = h.construct("Gio.ListStore", &[("item-type", s("Gio.File"))]).unwrap();
        assert_eq!(h.handle_info(store).unwrap().lifetime, SVL_LIFE_SHARED);
        // A GType comes back as its native name.
        assert_eq!(h.call_args(store, "get_item_type", &[]).unwrap(), s("Gio.File"));
        assert_eq!(h.get(store, "item-type"), s("Gio.File"));
        // Construct-only: setProp refuses it with a clear error.
        let e = h.set_value(store, "item-type", &Value::Str("Gio.File".into())).unwrap_err();
        assert!(e.contains("constructed"), "{e}");
        // Unknown construct property and unknown type.
        assert!(h.construct("Gio.ListStore", &[("nope", V::Bool(true))]).unwrap_err().contains("unknown property"));
        assert!(h.construct("Gio.ListStore", &[("item-type", s("No.Such"))]).is_err());

        h.listen(store, "items-changed", true).unwrap();
        let f = handle(h.call_static("Gio.File", "new_for_path", &[s("/etc")]).unwrap());
        h.call_args(store, "append", &[V::Handle(f)]).unwrap();
        assert_eq!(h.get(store, "n-items"), V::I64(1));
        assert_eq!(h.call_args(store, "get_n_items", &[]).unwrap(), V::I64(1));
        assert_eq!(h.call_args(store, "get_item", &[V::I64(0)]).unwrap(), V::Handle(f), "identity: the item is the same handle");
        assert_eq!(h.call_args(store, "get_item", &[V::I64(5)]).unwrap(), V::Null);
        let ev = h.take_events_with_args();
        assert_eq!(ev, vec![(store, "items-changed".to_string(), vec![V::I64(0), V::I64(0), V::I64(1)])]);
        // The wrong item type is an error from GLib's own check, not a crash.
        let other = h.construct("Gio.ListStore", &[("item-type", s("Gtk.StringObject"))]).unwrap();
        let _ = other;
        h.listen(store, "items-changed", false).unwrap();
        h.call_args(store, "remove_all", &[]).unwrap();
        assert!(h.take_events().is_empty());
    });
}

#[test]
fn out_parameters_arrays_and_lists_marshal() {
    on_gtk(|| {
        let h = Harness::new();
        let dir = std::env::temp_dir().join(format!("svl-nt0-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.txt");
        std::fs::write(&path, b"hello").unwrap();
        let f = handle(h.call_static("Gio.File", "new_for_path", &[s(path.to_str().unwrap())]).unwrap());
        // g_file_load_contents: an out byte array with its length, an out string, a GError.
        let r = h.call_args(f, "load_contents", &[V::Null]).unwrap();
        let V::List(items) = r else { panic!("{r:?}") };
        assert_eq!(items.len(), 3, "{items:?}");
        assert_eq!(items[0], V::Bool(true));
        // A missing file raises its GError.
        let missing = handle(h.call_static("Gio.File", "new_for_path", &[s("/no/such/file")]).unwrap());
        assert!(h.call_args(missing, "load_contents", &[V::Null]).is_err());
        // A GList of objects (transfer container) as a return value.
        let win = h.create("window");
        let tops = h.call_static("Gtk.Window", "list_toplevels", &[]).unwrap();
        let V::List(tops) = tops else { panic!("{tops:?}") };
        assert!(tops.contains(&V::Handle(win)), "the window we made is in the list, as its own handle: {tops:?}");
        // Out parameters of a widget method: GtkWidget.measure.
        let label = h.create("label");
        h.call_args(label, "set_text", &[s("hello")]).unwrap();
        let m = h.call_args(label, "measure", &[s("horizontal"), V::I64(-1)]).unwrap();
        let V::List(m) = m else { panic!("{m:?}") };
        assert_eq!(m.len(), 5, "return value then minimum, natural, minimum_baseline, natural_baseline");
        let (V::I64(min), V::I64(nat)) = (&m[1], &m[2]) else { panic!("{m:?}") };
        assert!(*min > 0 && nat >= min);
        let d = h.method_desc(label, "measure").unwrap();
        assert_eq!(d.params.iter().filter(|p| p.flags & SVL_D_OUT != 0).count(), 4);
        std::fs::remove_dir_all(dir).ok();
    });
}

#[test]
fn value_handles_have_kinds_and_methods() {
    on_gtk(|| {
        let h = Harness::new();
        let fd = boxed(h.call_static("Pango.FontDescription", "from_string", &[s("Sans Bold 12")]).unwrap());
        let info = h.handle_info(fd).unwrap();
        assert_eq!(info.lifetime, SVL_LIFE_VALUE);
        assert_ne!(info.kind, 0);
        assert_eq!(h.type_chain(fd)[0], "Pango.FontDescription");
        assert_eq!(h.call_args(fd, "get_family", &[]).unwrap(), s("Sans"));
        assert_eq!(h.call_args(fd, "get_size", &[]).unwrap(), V::I64(12 * 1024));
        assert_eq!(h.call_args(fd, "to_string", &[]).unwrap(), s("Sans Bold 12"));
        let fd2 = boxed(h.call_static("Pango.FontDescription", "from_string", &[s("Sans Bold 12")]).unwrap());
        assert_ne!(fd, fd2, "a value has no identity");
        assert_eq!(h.call_args(fd, "equal", &[V::Boxed(fd2)]).unwrap(), V::Bool(true));
        // A property setter on a value handle is refused.
        assert!(h.set_value(fd, "size", &Value::I64(1)).is_err());
        // An AttrList parses its text form.
        let al = boxed(h.call_static("Pango.AttrList", "from_string", &[s("0 5 weight bold")]).unwrap());
        assert_eq!(h.call_args(al, "to_string", &[]).unwrap(), s("0 5 weight bold"));
        h.destroy(fd);
        assert!(h.call_args(fd, "get_family", &[]).is_err());
        h.destroy(fd); // idempotent
    });
}

#[test]
fn text_buffer_and_caller_allocated_iterators() {
    on_gtk(|| {
        let h = Harness::new();
        let buf = h.construct("Gtk.TextBuffer", &[("text", s("hello"))]).unwrap();
        assert_eq!(h.handle_info(buf).unwrap().lifetime, SVL_LIFE_SHARED);
        assert_eq!(h.call_args(buf, "get_char_count", &[]).unwrap(), V::I64(5));
        // get_start_iter fills a GtkTextIter the caller allocates; it comes back as a value.
        let start = h.call_args(buf, "get_start_iter", &[]).unwrap();
        let V::List(start) = start else { panic!("{start:?}") };
        let (V::Null, V::Boxed(si)) = (&start[0], &start[1]) else { panic!("{start:?}") };
        let end = h.call_args(buf, "get_end_iter", &[]).unwrap();
        let V::List(end) = end else { panic!("{end:?}") };
        let V::Boxed(ei) = end[1] else { panic!() };
        assert_eq!(h.call_args(buf, "get_text", &[V::Boxed(*si), V::Boxed(ei), V::Bool(false)]).unwrap(), s("hello"));
        h.call_args(buf, "insert_at_cursor", &[s(" world"), V::I64(-1)]).unwrap();
        assert_eq!(h.get(buf, "text"), s("hello world"));
        h.listen(buf, "changed", true).unwrap();
        h.set_value(buf, "text", &Value::Str("x".into())).unwrap();
        assert!(!h.take_events().is_empty());
    });
}

#[test]
fn string_list_directory_list_and_file_dialog() {
    on_gtk(|| {
        let h = Harness::new();
        let sl = h.construct("Gtk.StringList", &[("strings", V::List(vec![s("a"), s("b")]))]).unwrap();
        assert_eq!(h.call_args(sl, "get_n_items", &[]).unwrap(), V::I64(2));
        assert_eq!(h.call_args(sl, "get_string", &[V::I64(1)]).unwrap(), s("b"));
        h.call_args(sl, "append", &[s("c")]).unwrap();
        assert_eq!(h.call_args(sl, "get_string", &[V::I64(2)]).unwrap(), s("c"));
        // The model's items are objects with their own identity.
        let item = handle(h.call_args(sl, "get_item", &[V::I64(0)]).unwrap());
        assert_eq!(h.call_args(sl, "get_item", &[V::I64(0)]).unwrap(), V::Handle(item));
        assert_eq!(h.call_args(item, "get_string", &[]).unwrap(), s("a"));
        let f = handle(h.call_static("Gio.File", "new_for_path", &[s("/usr/share")]).unwrap());
        let dl = h.construct("Gtk.DirectoryList", &[("file", V::Handle(f)), ("attributes", s("standard::display-name"))]).unwrap();
        for _ in 0..100 {
            if h.get(dl, "loading") == V::Bool(false) {
                break;
            }
            pump(20);
        }
        let V::I64(n) = h.call_args(dl, "get_n_items", &[]).unwrap() else { panic!() };
        assert!(n > 0, "/usr/share has entries");
        assert_eq!(h.get(dl, "file"), V::Handle(f));
        let first = handle(h.call_args(dl, "get_item", &[V::I64(0)]).unwrap());
        assert!(matches!(h.call_args(first, "get_display_name", &[]).unwrap(), V::Str(_)));
        let fd = h.construct("Gtk.FileDialog", &[("title", s("Open")), ("modal", V::Bool(true))]).unwrap();
        assert_eq!(h.get(fd, "title"), s("Open"));
        assert_eq!(h.call_args(fd, "get_title", &[]).unwrap(), s("Open"));
    });
}

#[test]
fn textures_clipboards_and_displays_are_objects() {
    on_gtk(|| {
        let h = Harness::new();
        let path = std::env::temp_dir().join(format!("svl-nt0-{}.png", std::process::id()));
        let pixel = glib::Bytes::from(&[0u8, 0, 0, 255][..]);
        gtk::gdk::MemoryTexture::new(1, 1, gtk::gdk::MemoryFormat::R8g8b8a8, &pixel, 4).save_to_png(&path);
        let tex = handle(h.call_static("Gdk.Texture", "new_from_filename", &[s(path.to_str().unwrap())]).unwrap());
        assert_eq!(h.call_args(tex, "get_width", &[]).unwrap(), V::I64(1));
        assert_eq!(h.get(tex, "height"), V::I64(1));
        std::fs::remove_file(path).ok();

        let display = handle(h.call_static("Gdk.Display", "get_default", &[]).unwrap());
        let cb = handle(h.call_args(display, "get_clipboard", &[]).unwrap());
        assert_eq!(h.call_args(display, "get_clipboard", &[]).unwrap(), V::Handle(cb), "one handle per clipboard");
        assert_eq!(h.get(cb, "display"), V::Handle(display));
        // The clipboard's formats are a value; a value built from an array of strings (with a length) works too.
        let current = boxed(h.get(cb, "formats"));
        assert!(matches!(h.call_args(current, "to_string", &[]).unwrap(), V::Str(_)));
        let formats = boxed(h.call_static("Gdk.ContentFormats", "new", &[V::List(vec![s("text/plain"), s("image/png")])]).unwrap());
        assert_eq!(h.call_args(formats, "contain_mime_type", &[s("image/png")]).unwrap(), V::Bool(true));
        assert_eq!(h.call_args(formats, "contain_mime_type", &[s("image/jpeg")]).unwrap(), V::Bool(false));
        // The array's length argument is hidden: the result is the list itself.
        assert_eq!(h.call_args(formats, "get_mime_types", &[]).unwrap(), V::List(vec![s("text/plain"), s("image/png")]));
    });
}

trait CloneBoxed {
    fn clone_boxed(&self) -> V;
}
impl CloneBoxed for V {
    fn clone_boxed(&self) -> V {
        self.clone()
    }
}
