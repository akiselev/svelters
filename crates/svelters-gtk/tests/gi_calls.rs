//! Dynamic GIRepository method calls on the GTK backend, one test per argument/return type
//! (plans/milestones/X1-borrowed-gi.md). Needs a display (xvfb-run).

use gtk::prelude::*;
use svelters_gtk::testing::{on_gtk, pump, Harness, V};

fn s(x: &str) -> V {
    V::Str(x.into())
}

#[test]
fn bool_args_and_returns() {
    on_gtk(|| {
        let h = Harness::new();
        let label = h.create("label");
        assert_eq!(h.call_args(label, "get_visible", &[]).unwrap(), V::Bool(true));
        assert_eq!(h.call_args(label, "set_visible", &[V::Bool(false)]).unwrap(), V::Null);
        assert_eq!(h.call_args(label, "get_visible", &[]).unwrap(), V::Bool(false));
        // JS booleans arrive as bools; integers are accepted as truthy/falsy.
        h.call_args(label, "setVisible", &[V::I64(1)]).unwrap();
        assert_eq!(h.call_args(label, "getVisible", &[]).unwrap(), V::Bool(true));
        assert!(h.call_args(label, "set_visible", &[s("yes")]).unwrap_err().contains("expected a boolean"));
    });
}

#[test]
fn integer_args_and_returns() {
    on_gtk(|| {
        let h = Harness::new();
        let bx = h.create("box");
        h.call_args(bx, "set_spacing", &[V::I64(7)]).unwrap();
        assert_eq!(h.call_args(bx, "get_spacing", &[]).unwrap(), V::I64(7));
        assert!(h.call_args(bx, "set_spacing", &[V::I64(1 << 40)]).unwrap_err().contains("out of range"));
        assert!(h.call_args(bx, "set_spacing", &[V::F64(1.5)]).unwrap_err().contains("not an integer"));
        assert!(h.call_args(bx, "set_spacing", &[s("3")]).unwrap_err().contains("expected an integer"));
        // guint
        let spin = h.create("spin-button");
        h.call_args(spin, "set_digits", &[V::I64(3)]).unwrap();
        assert_eq!(h.call_args(spin, "get_digits", &[]).unwrap(), V::I64(3));
        assert!(h.call_args(spin, "set_digits", &[V::I64(-1)]).unwrap_err().contains("out of range"));
    });
}

#[test]
fn floating_point_args_and_returns() {
    on_gtk(|| {
        let h = Harness::new();
        let label = h.create("label");
        // double
        h.call_args(label, "set_opacity", &[V::F64(0.0)]).unwrap();
        assert_eq!(h.call_args(label, "get_opacity", &[]).unwrap(), V::F64(0.0)); // GTK quantises opacity to 8 bits
        h.call_args(label, "set_opacity", &[V::I64(1)]).unwrap();
        assert_eq!(h.call_args(label, "get_opacity", &[]).unwrap(), V::F64(1.0));
        // float
        h.call_args(label, "set_xalign", &[V::F64(0.5)]).unwrap();
        assert_eq!(h.call_args(label, "get_xalign", &[]).unwrap(), V::F64(0.5));
        assert!(h.call_args(label, "set_xalign", &[s("x")]).unwrap_err().contains("expected a number"));
    });
}

#[test]
fn string_args_and_returns_including_null() {
    on_gtk(|| {
        let h = Harness::new();
        let label = h.create("label");
        h.call_args(label, "set_text", &[s("héllo")]).unwrap();
        assert_eq!(h.call_args(label, "get_text", &[]).unwrap(), s("héllo"));
        // nullable in, nullable out
        assert_eq!(h.call_args(label, "get_tooltip_text", &[]).unwrap(), V::Null);
        h.call_args(label, "set_tooltip_text", &[s("tip")]).unwrap();
        assert_eq!(h.call_args(label, "get_tooltip_text", &[]).unwrap(), s("tip"));
        h.call_args(label, "set_tooltip_text", &[V::Null]).unwrap();
        assert_eq!(h.call_args(label, "get_tooltip_text", &[]).unwrap(), V::Null);
        // not nullable
        assert!(h.call_args(label, "set_text", &[V::Null]).unwrap_err().contains("null is not allowed"));
        assert!(h.call_args(label, "set_text", &[V::I64(3)]).unwrap_err().contains("expected a string"));
        assert!(h.call_args(label, "set_text", &[s("a\0b")]).unwrap_err().contains("NUL"));
        // transfer full string return (freed by the caller side)
        assert_eq!(h.call_args(label, "get_css_name", &[]).unwrap(), s("label"));
    });
}

#[test]
fn enum_args_and_returns() {
    on_gtk(|| {
        let h = Harness::new();
        let label = h.create("label");
        // by nick
        h.call_args(label, "set_halign", &[s("center")]).unwrap();
        assert_eq!(h.call_args(label, "get_halign", &[]).unwrap(), s("center"));
        // by number (GTK_ALIGN_END = 2); returns are always nicks, like properties
        h.call_args(label, "set_halign", &[V::I64(2)]).unwrap();
        assert_eq!(h.call_args(label, "get_halign", &[]).unwrap(), s("end"));
        // an enum from another namespace (PangoEllipsizeMode)
        h.call_args(label, "set_ellipsize", &[s("middle")]).unwrap();
        assert_eq!(h.call_args(label, "get_ellipsize", &[]).unwrap(), s("middle"));
        assert!(h.call_args(label, "set_halign", &[s("sideways")]).unwrap_err().contains("not a member"));
        assert!(h.call_args(label, "set_halign", &[V::I64(99)]).is_err());
        // The host sees typed slots: the enum travels by name through its type descriptor.
        let d = h.method_desc(label, "set_halign").expect("descriptor");
        assert_eq!(d.params[0].tag, svelters_abi::SVL_ENUM);
    });
}

#[test]
fn flags_args_and_returns() {
    on_gtk(|| {
        let h = Harness::new();
        let label = h.create("label");
        let has = |h: &Harness, name: &str| match h.call_args(label, "get_state_flags", &[]).unwrap() {
            V::List(l) => l.contains(&s(name)),
            other => panic!("flags are a list of names, got {other:?}"),
        };
        // by name, as a set of names, and by number
        h.call_args(label, "set_state_flags", &[s("prelight"), V::Bool(false)]).unwrap();
        assert!(has(&h, "prelight"));
        h.call_args(label, "set_state_flags", &[V::List(vec![s("active")]), V::Bool(false)]).unwrap();
        assert!(has(&h, "prelight") && has(&h, "active"));
        h.call_args(label, "unset_state_flags", &[s("prelight|active")]).unwrap();
        assert!(!has(&h, "prelight") && !has(&h, "active"));
        h.call_args(label, "set_state_flags", &[V::I64(1 << 1), V::Bool(false)]).unwrap();
        assert!(has(&h, "prelight"));
        let d = h.method_desc(label, "set_state_flags").expect("descriptor");
        assert_eq!(d.params[0].tag, svelters_abi::SVL_FLAGS);
        assert!(h.call_args(label, "set_state_flags", &[s("bogus"), V::Bool(false)]).unwrap_err().contains("not a member"));
    });
}

#[test]
fn object_args_and_returns_including_null() {
    on_gtk(|| {
        let h = Harness::new();
        let win = h.create("window");
        let bx = h.create("box");
        let label = h.create("label");
        let entry = h.create("entry");
        h.insert(bx, label, None, None).unwrap();
        h.insert(bx, entry, None, None).unwrap();
        h.insert(win, bx, None, None).unwrap();

        // nullable object in, object out (an existing OWNED handle)
        assert_eq!(h.call_args(label, "get_mnemonic_widget", &[]).unwrap(), V::Null);
        h.call_args(label, "set_mnemonic_widget", &[V::Handle(entry)]).unwrap();
        assert_eq!(h.call_args(label, "get_mnemonic_widget", &[]).unwrap(), V::Handle(entry));
        h.call_args(label, "set_mnemonic_widget", &[V::Null]).unwrap();
        assert_eq!(h.call_args(label, "get_mnemonic_widget", &[]).unwrap(), V::Null);

        // object out through a plain getter; null at the end of the chain
        assert_eq!(h.call_args(bx, "get_first_child", &[]).unwrap(), V::Handle(label));
        assert_eq!(h.call_args(label, "get_next_sibling", &[]).unwrap(), V::Handle(entry));
        assert_eq!(h.call_args(entry, "get_next_sibling", &[]).unwrap(), V::Null);
    });
}

#[test]
fn object_arguments_are_checked() {
    on_gtk(|| {
        let h = Harness::new();
        let label = h.create("label");
        let entry = h.create("entry");
        let ctl = h.create("EventControllerKey");
        // a controller is not a widget
        assert!(h.call_args(label, "set_mnemonic_widget", &[V::Handle(ctl)]).unwrap_err().contains("GtkWidget"));
        assert!(h.call_args(label, "set_mnemonic_widget", &[V::Handle(999_999)]).unwrap_err().contains("stale"));
        assert!(h.call_args(label, "set_mnemonic_widget", &[V::I64(entry as i64)]).is_err());
    });
}

#[test]
fn transfer_full_arguments_get_their_own_reference() {
    on_gtk(|| {
        let h = Harness::new();
        let entry = h.create("entry");
        let ctl = h.create("EventControllerKey");
        let weak = h.obj(ctl).downgrade();
        // gtk_widget_add_controller takes the caller's reference.
        h.call_args(entry, "add_controller", &[V::Handle(ctl)]).unwrap();
        h.destroy(ctl);
        pump(100);
        assert!(weak.upgrade().is_some(), "the widget keeps the controller alive after the table let go");
        h.destroy(entry);
        pump(200);
        assert!(weak.upgrade().is_none(), "and nothing leaked a reference");
    });
}

#[test]
fn methods_of_interfaces_and_ancestors_resolve() {
    on_gtk(|| {
        let h = Harness::new();
        let entry = h.create("entry");
        // GtkEditable (interface) and GtkWidget (ancestor) methods on a GtkEntry
        h.call_args(entry, "set_text", &[s("abc")]).unwrap();
        assert_eq!(h.call_args(entry, "get_text", &[]).unwrap(), s("abc"));
        assert_eq!(h.call_args(entry, "get_position", &[]).unwrap(), V::I64(0));
        h.call_args(entry, "set_position", &[V::I64(2)]).unwrap();
        assert_eq!(h.call_args(entry, "get_position", &[]).unwrap(), V::I64(2));
        assert_eq!(h.call_args(entry, "get_visible", &[]).unwrap(), V::Bool(true));
        assert_ne!(h.resolve_method(entry, "get_visible"), 0);
        assert_eq!(h.resolve_method(entry, "no_such_method"), 0);
        // the annotated, generated method still wins for its own name
        let win = h.create("window");
        let generated = h.resolve_method(win, "present");
        assert!(generated != 0 && generated < h.resolve_method(win, "get_visible"), "generated ids come before dynamic ones");
    });
}

#[test]
fn unsupported_features_are_rejected_with_clear_errors() {
    on_gtk(|| {
        let h = Harness::new();
        let label = h.create("label");
        let win = h.create("window");
        // callbacks (with their user_data and notify)
        let e = h.call_args(label, "add_tick_callback", &[V::Null, V::Null, V::Null]).unwrap_err();
        assert!(e.contains("callback"), "{e}");
        // out parameters are supported since NT0: measure answers [ret, minimum, natural, ...]
        let V::List(m) = h.call_args(label, "measure", &[s("horizontal"), V::I64(-1)]).unwrap() else { panic!("outs") };
        assert_eq!(m.len(), 5);
        let e = h.call_args(label, "get_allocation_not_a_method", &[]).unwrap_err();
        assert!(e.contains("unknown method"), "{e}");
        // static functions are not methods
        let e = h.call_args(win, "new", &[]).unwrap_err();
        assert!(e.contains("static function or constructor"), "{e}");
        // arity
        let e = h.call_args(label, "set_text", &[]).unwrap_err();
        assert!(e.contains("takes 1 argument"), "{e}");
        let e = h.call_args(label, "get_text", &[s("x")]).unwrap_err();
        assert!(e.contains("takes 0 argument"), "{e}");
    });
}

#[test]
fn string_arrays_and_boxed_values() {
    on_gtk(|| {
        let h = Harness::new();
        let label = h.create("label");
        // GStrv in and out
        h.call_args(label, "set_css_classes", &[V::List(vec![s("a"), s("b")])]).unwrap();
        assert_eq!(h.call_args(label, "get_css_classes", &[]).unwrap(), V::List(vec![s("a"), s("b")]));
        assert!(h.call_args(label, "set_css_classes", &[V::List(vec![V::I64(1)])]).is_err());
        // a nullable boxed argument (PangoAttrList)
        assert_eq!(h.call_args(label, "set_attributes", &[V::Null]).unwrap(), V::Null);
        assert_eq!(h.call_args(label, "get_attributes", &[]).unwrap(), V::Null);
        // an object that is not a widget comes back as a SHARED handle
        let layout = h.call_args(label, "create_pango_layout", &[s("hi")]).unwrap();
        assert!(matches!(layout, V::Handle(_)), "{layout:?}");
    });
}
