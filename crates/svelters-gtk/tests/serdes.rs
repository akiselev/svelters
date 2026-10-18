//! Round-trip tests for the ABI v3 value model on the GTK backend: a value goes in through the C ABI
//! exactly as the host sends it, the test reads what GTK holds (through the GObject API) and reads the
//! value back through the ABI. Run under a display: `xvfb-run -a cargo nextest run -p svelters-gtk`.

use gtk::prelude::*;
use gtk::{gdk, glib};
use svelters_abi::*;
use svelters_gtk::testing::{on_gtk, Harness, V};

fn i(x: i64) -> Value {
    Value::I64(x)
}
fn f(x: f64) -> Value {
    Value::F64(x)
}

fn slot_ty(h: &Harness, obj: u64, prop: &str) -> u32 {
    let d = h.prop_desc(obj, prop).expect("descriptor");
    d.params.first().unwrap_or(&d.ret).ty
}

#[test]
fn scalars_bool_ints_floats() {
    on_gtk(|| {
        let h = Harness::new();
        let bx = h.create("Box");
        h.set_value(bx, "spacing", &i(12)).unwrap();
        assert_eq!(h.obj(bx).property::<i32>("spacing"), 12);
        assert_eq!(h.get_value(bx, "spacing"), i(12));
        // Range checks come from the descriptor and from the conversion.
        let slot = h.prop_desc(bx, "spacing").unwrap().params[0].clone();
        assert_eq!((slot.tag, slot.min, slot.max), (SVL_I64, 0.0, i32::MAX as f64));
        assert!(h.set_value(bx, "spacing", &i(1 << 40)).unwrap_err().contains("out of range"));
        assert!(h.set_value(bx, "spacing", &f(1.5)).unwrap_err().contains("not an integer"));
        h.set_value(bx, "spacing", &f(3.0)).unwrap();
        assert_eq!(h.obj(bx).property::<i32>("spacing"), 3);
        // bool
        let l = h.create("Label");
        h.set_value(l, "wrap", &Value::Bool(true)).unwrap();
        assert!(h.obj(l).property::<bool>("wrap"));
        assert_eq!(h.get_value(l, "wrap"), Value::Bool(true));
        // f64 with a descriptor range (opacity is 0..1)
        h.set_value(l, "opacity", &f(0.25)).unwrap();
        let Value::F64(o) = h.get_value(l, "opacity") else { panic!() };
        assert!((o - 0.25).abs() < 0.01, "GTK quantises opacity to 8 bits: {o}");
        let slot = h.prop_desc(l, "opacity").unwrap().params[0].clone();
        assert_eq!((slot.tag, slot.min, slot.max), (SVL_F64, 0.0, 1.0));
        // f64 properties of a plain object, an integral value is accepted where a float is expected
        let adj = h.create("Adjustment");
        h.set_value(adj, "upper", &f(200.0)).unwrap();
        h.set_value(adj, "value", &i(50)).unwrap();
        assert_eq!(h.obj(adj).property::<f64>("value"), 50.0);
        assert_eq!(h.get_value(adj, "value"), f(50.0));
    });
}

#[test]
fn strings_utf8_and_utf16() {
    on_gtk(|| {
        let h = Harness::new();
        let l = h.create("Label");
        let s = "h\u{e9}llo \u{2713} \u{1F600}";
        h.set_value(l, "label", &Value::Str(s.into())).unwrap();
        assert_eq!(h.obj(l).property::<String>("label"), s);
        assert_eq!(h.get_value(l, "label"), Value::Str(s.into()));
        // UTF-16 is accepted too, although GTK does not advertise it (the host sends UTF-8).
        assert_eq!(h.backend_ref().caps() & SVL_CAP_UTF16_IN, 0);
        let u: Vec<u16> = "w\u{f6}rld".encode_utf16().collect();
        let mut v = svl_value::new(SVL_STR, svl_value_u { s: svl_str { ptr: u.as_ptr().cast(), len: u.len() } });
        v.flags = SVL_F_UTF16;
        let id = h.backend_ref().resolve(NameKind::Prop, h.kind_id("Label"), "label");
        h.backend_ref().set_prop_raw(l, id, &v).unwrap();
        assert_eq!(h.obj(l).property::<String>("label"), "w\u{f6}rld");
        // null clears a nullable string; other types are refused.
        h.set_value(l, "label", &Value::Null).unwrap();
        assert!(h.set_value(l, "label", &i(5)).is_err());
    });
}

#[test]
fn enums_by_native_value_and_name() {
    on_gtk(|| {
        let h = Harness::new();
        let bx = h.create("Box");
        let t = h.type_desc(slot_ty(&h, bx, "orientation")).unwrap();
        assert_eq!(t.kind, SVL_ENUM);
        assert_eq!(t.members.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), ["horizontal", "vertical"]);
        let v = h.named(bx, "orientation", &["vertical"]).unwrap();
        h.set_value(bx, "orientation", &v).unwrap();
        assert_eq!(h.obj(bx).property::<gtk::Orientation>("orientation"), gtk::Orientation::Vertical);
        assert_eq!(h.get_value(bx, "orientation"), v, "enums leave as their native value with the type id");
        assert_eq!(h.get(bx, "orientation"), V::Str("vertical".into()));
        // A bad value is refused. Names are still accepted as a convenience (nicks).
        let ty = slot_ty(&h, bx, "orientation");
        assert!(h.set_value(bx, "orientation", &Value::Enum { ty, value: 77 }).is_err());
        h.set_value(bx, "orientation", &Value::Str("horizontal".into())).unwrap();
        assert_eq!(h.obj(bx).property::<gtk::Orientation>("orientation"), gtk::Orientation::Horizontal);
        // A different toolkit enum (Pango) with a name that GTK spells with dashes elsewhere.
        let l = h.create("Label");
        let v = h.named(l, "ellipsize", &["middle"]).unwrap();
        h.set_value(l, "ellipsize", &v).unwrap();
        assert_eq!(h.get(l, "ellipsize"), V::Str("middle".into()));
    });
}

#[test]
fn flags_and_string_lists() {
    on_gtk(|| {
        let h = Harness::new();
        let e = h.create("Entry");
        let t = h.type_desc(slot_ty(&h, e, "input-hints")).unwrap();
        assert_eq!(t.kind, SVL_FLAGS);
        let v = h.named(e, "input-hints", &["spellcheck", "lowercase"]).unwrap();
        h.set_value(e, "input-hints", &v).unwrap();
        let hints = h.obj(e).property::<gtk::InputHints>("input-hints");
        assert!(hints.contains(gtk::InputHints::SPELLCHECK) && hints.contains(gtk::InputHints::LOWERCASE));
        assert_eq!(h.get_value(e, "input-hints"), v);
        let V::List(mut names) = h.get(e, "input-hints") else { panic!() };
        names.sort_by_key(|n| format!("{n:?}"));
        assert_eq!(names, vec![V::Str("lowercase".into()), V::Str("spellcheck".into())]);
        // Flags may also be given as nick text (attribute form).
        h.set_value(e, "input-hints", &Value::Str("uppercase-chars".into())).unwrap();
        assert!(h.obj(e).property::<gtk::InputHints>("input-hints").contains(gtk::InputHints::UPPERCASE_CHARS));
        // GStrv: css-classes
        let w = h.create("Button");
        h.set_value(w, "css-classes", &Value::List(vec![Value::Str("suggested-action".into()), Value::Str("big".into())])).unwrap();
        let classes: Vec<String> = h.widget(w).css_classes().iter().map(|s| s.to_string()).collect();
        assert!(classes.contains(&"suggested-action".to_string()) && classes.contains(&"big".to_string()), "{classes:?}");
        assert_eq!(h.get_value(w, "css-classes"), Value::List(classes.iter().map(|s| Value::Str(s.clone())).collect()));
        assert!(h.set_value(w, "css-classes", &Value::List(vec![i(1)])).is_err());
        h.set_value(w, "css-classes", &Value::List(vec![])).unwrap();
        assert!(h.widget(w).css_classes().is_empty());
    });
}

#[test]
fn records_rect_and_color() {
    on_gtk(|| {
        let h = Harness::new();
        // GdkRectangle (integral fields): Popover.pointing-to
        let p = h.create("Popover");
        let d = h.prop_desc(p, "pointing-to").unwrap();
        let ty = d.params[0].ty;
        let td = h.type_desc(ty).unwrap();
        assert_eq!((td.kind, td.name.as_str()), (SVL_RECORD, "Rect"));
        assert_eq!(td.fields.iter().map(|f| f.name.clone().unwrap()).collect::<Vec<_>>(), ["x", "y", "width", "height"]);
        let r = Value::Record { ty, fields: vec![i(4), i(5), i(60), i(70)] };
        h.set_value(p, "pointing-to", &r).unwrap();
        let (ok, rect) = h.obj(p).downcast::<gtk::Popover>().unwrap().pointing_to();
        assert!(ok);
        assert_eq!((rect.x(), rect.y(), rect.width(), rect.height()), (4, 5, 60, 70));
        assert_eq!(h.get_value(p, "pointing-to"), r);
        assert!(h.set_value(p, "pointing-to", &Value::Record { ty, fields: vec![f(1.5), i(0), i(1), i(1)] }).is_err());
        assert!(h.set_value(p, "pointing-to", &Value::Record { ty, fields: vec![i(1)] }).unwrap_err().contains("4 fields"));
        // GdkRGBA: ColorDialogButton.rgba
        let c = h.create("ColorDialogButton");
        let ty = slot_ty(&h, c, "rgba");
        assert_eq!(ty, SVL_T_COLOR);
        assert!(h.prop_desc(c, "rgba").unwrap().params[0].has(SVL_D_STRING_FORM));
        let red = Value::Record { ty, fields: vec![f(1.0), f(0.0), f(0.0), f(1.0)] };
        h.set_value(c, "rgba", &red).unwrap();
        let rgba = h.obj(c).property::<gdk::RGBA>("rgba");
        assert_eq!((rgba.red(), rgba.green(), rgba.blue(), rgba.alpha()), (1.0, 0.0, 0.0, 1.0));
        assert_eq!(h.get_value(c, "rgba"), red);
        // the CSS string form is parsed by the backend
        h.set_value(c, "rgba", &Value::Str("rgb(0, 128, 255)".into())).unwrap();
        let rgba = h.obj(c).property::<gdk::RGBA>("rgba");
        assert!((rgba.green() - 128.0 / 255.0).abs() < 1e-6 && rgba.blue() == 1.0);
        assert!(h.set_value(c, "rgba", &Value::Str("not a color".into())).unwrap_err().contains("cannot parse color"));
    });
}

#[test]
fn handles_nullable_and_boxed() {
    on_gtk(|| {
        let h = Harness::new();
        // object handle, nullable
        let label = h.create("Label");
        let entry = h.create("Entry");
        h.set_handle(label, "mnemonic-widget", entry).unwrap();
        assert_eq!(h.get(label, "mnemonic-widget"), V::Handle(entry));
        h.set_value(label, "mnemonic-widget", &Value::Null).unwrap();
        assert_eq!(h.get(label, "mnemonic-widget"), V::Null);
        assert!(h.set_handle(label, "mnemonic-widget", 999_999).unwrap_err().contains("stale"));
        let bx = h.create("Box");
        assert!(h.set_handle(label, "mnemonic-widget", bx).is_ok(), "any widget is acceptable");
        // A GIcon object has no kind: it travels as a BOXED handle; a string is its canonical form.
        let img = h.create("Image");
        assert!(h.prop_desc(img, "gicon").unwrap().params[0].has(SVL_D_STRING_FORM));
        h.set_value(img, "gicon", &Value::Str("document-new".into())).unwrap();
        let icon = h.obj(img).property::<Option<gtk::gio::Icon>>("gicon").expect("icon set");
        assert!(icon.downcast_ref::<gtk::gio::ThemedIcon>().is_some());
        let Value::Boxed { ty, handle } = h.get_value(img, "gicon") else { panic!("boxed") };
        assert_eq!(h.type_desc(ty).unwrap().kind, SVL_BOXED);
        // Handing the boxed handle to another image copies the reference.
        let img2 = h.create("Image");
        h.set_value(img2, "gicon", &Value::Boxed { ty, handle }).unwrap();
        assert!(h.obj(img2).property::<Option<gtk::gio::Icon>>("gicon").is_some());
        // A boxed value is a VALUE-class handle; releasing it makes the handle stale.
        assert_eq!(h.handle_info(handle).unwrap().lifetime, SVL_LIFE_VALUE);
        // Release: explicit, idempotent, stale afterwards.
        h.destroy(handle);
        assert!(h.handle_info(handle).is_err());
        h.destroy(handle);
        assert!(h.set_value(img2, "gicon", &Value::Boxed { ty, handle }).unwrap_err().contains("stale boxed handle"));
        // A boxed value of the wrong type is refused.
        let fb = h.create("FontDialogButton");
        h.set_value(fb, "font-desc", &Value::Str("Sans Bold 12".into())).unwrap();
        let fd = h.obj(fb).property::<Option<gtk::pango::FontDescription>>("font-desc").unwrap();
        assert_eq!(fd.size(), 12 * gtk::pango::SCALE);
        let Value::Boxed { ty: fty, handle: fh } = h.get_value(fb, "font-desc") else { panic!("boxed font") };
        assert_ne!(fty, ty);
        assert!(h.set_value(img2, "gicon", &Value::Boxed { ty: fty, handle: fh }).unwrap_err().contains("is not a"));
        h.destroy(fh);
        h.set_value(img2, "gicon", &Value::Null).unwrap();
        assert!(h.obj(img2).property::<Option<gtk::gio::Icon>>("gicon").is_none());
    });
}

#[test]
fn variants_dates_and_string_forms() {
    on_gtk(|| {
        let h = Harness::new();
        // GVariant <-> JSON-like trees: GtkActionable:action-target
        let b = h.create("Button");
        let d = h.prop_desc(b, "action-target").unwrap();
        assert_eq!((d.params[0].tag, d.params[0].ty), (SVL_NULL, SVL_T_VARIANT));
        let tree = Value::Map(vec![
            ("name".into(), Value::Str("x".into())),
            ("n".into(), i(7)),
            ("list".into(), Value::List(vec![Value::Bool(true), f(0.5), Value::Bytes(vec![1, 2])])),
        ]);
        h.set_value(b, "action-target", &tree).unwrap();
        let v = h.obj(b).property::<Option<glib::Variant>>("action-target").expect("variant set");
        assert_eq!(v.type_().as_str(), "a{sv}");
        let Value::Map(mut got) = h.get_value(b, "action-target") else { panic!("map") };
        got.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(
            got,
            vec![
                ("list".into(), Value::List(vec![Value::Bool(true), f(0.5), Value::Bytes(vec![1, 2])])),
                ("n".into(), i(7)),
                ("name".into(), Value::Str("x".into())),
            ]
        );
        h.set_value(b, "action-target", &Value::Str("plain".into())).unwrap();
        assert_eq!(h.get_value(b, "action-target"), Value::Str("plain".into()));
        h.set_value(b, "action-target", &Value::Null).unwrap();
        assert_eq!(h.get_value(b, "action-target"), Value::Null);
        assert!(h.set_value(b, "action-target", &Value::List(vec![Value::Null])).unwrap_err().contains("null cannot be nested"));

        // Exact types: `variant(signature, value)` sends a typed-variant record.
        let typed = |sig: &str, v: Value| Value::Record { ty: SVL_T_TYPED_VARIANT, fields: vec![Value::Str(sig.into()), v] };
        let ty_after = |h: &Harness, v: Value| -> Result<String, String> {
            h.set_value(b, "action-target", &v)?;
            Ok(h.obj(b).property::<Option<glib::Variant>>("action-target").map(|v| v.print(true).to_string()).unwrap_or_default())
        };
        assert_eq!(ty_after(&h, typed("(is)", Value::List(vec![i(3), Value::Str("x".into())]))).unwrap(), "(3, 'x')");
        assert_eq!(h.obj(b).property::<Option<glib::Variant>>("action-target").unwrap().type_().as_str(), "(is)");
        assert_eq!(ty_after(&h, typed("u", i(5))).unwrap(), "uint32 5");
        assert_eq!(ty_after(&h, typed("y", i(255))).unwrap(), "byte 0xff");
        assert_eq!(ty_after(&h, typed("d", i(2))).unwrap(), "2.0");
        assert_eq!(ty_after(&h, typed("ai", Value::List(vec![i(1), i(2)]))).unwrap(), "[1, 2]");
        assert_eq!(ty_after(&h, typed("ay", Value::Bytes(vec![1, 2]))).unwrap(), "[byte 0x01, 0x02]");
        assert_eq!(ty_after(&h, typed("a{ss}", Value::Map(vec![("k".into(), Value::Str("v".into()))]))).unwrap(), "{'k': 'v'}");
        assert_eq!(ty_after(&h, typed("a{sv}", Value::Map(vec![("n".into(), typed("q", i(9)))]))).unwrap(), "{'n': <uint16 9>}");
        assert_eq!(ty_after(&h, typed("ms", Value::Null)).unwrap(), "@ms nothing");
        assert_eq!(ty_after(&h, typed("ms", Value::Str("a".into()))).unwrap(), "@ms 'a'");
        let e = ty_after(&h, typed("y", i(300))).unwrap_err();
        assert!(e.contains("300 does not fit type y"), "{e}");
        let e = ty_after(&h, typed("(ii)", Value::List(vec![i(1)]))).unwrap_err();
        assert!(e.contains("expected a list of 2 item(s)"), "{e}");
        let e = ty_after(&h, typed("a{is}", Value::Map(vec![("k".into(), Value::Str("v".into()))]))).unwrap_err();
        assert!(e.contains("only with string keys"), "{e}");
        let e = ty_after(&h, typed("(", Value::Null)).unwrap_err();
        assert!(e.contains("invalid GVariant type"), "{e}");

        // GDateTime <-> a Date (epoch milliseconds typed DATETIME): GtkCalendar:date
        let c = h.create("Calendar");
        let ms = 1_700_000_123_456.0;
        if svelters_gtk::compat::has((4, 16)) {
            let slot = h.prop_desc(c, "date").unwrap().params[0].clone();
            assert_eq!((slot.tag, slot.ty), (SVL_F64, SVL_T_DATETIME));
            h.set_value(c, "date", &Value::DateTime(ms)).unwrap();
            let dt = h.obj(c).property::<glib::DateTime>("date");
            assert_eq!(dt.to_unix(), 1_700_000_123);
            let Value::DateTime(back) = h.get_value(c, "date") else { panic!("datetime") };
            assert!((back - ms).abs() < 1.0, "{back} vs {ms}");
        } else {
            // GtkCalendar:date is GTK 4.16: undescribed, and refused with a version error.
            assert!(h.prop_desc(c, "date").is_none());
            let e = h.set_value(c, "date", &Value::DateTime(ms)).unwrap_err();
            assert!(e.contains("property GtkCalendar:date needs GTK 4.16"), "{e}");
        }

        // string forms: a shortcut trigger and a GFile (path or URI)
        let sc = h.create("Shortcut");
        assert!(h.prop_desc(sc, "trigger").unwrap().params[0].has(SVL_D_STRING_FORM));
        h.set_value(sc, "trigger", &Value::Str("<Control>s".into())).unwrap();
        let trig = h.obj(sc).property::<Option<gtk::ShortcutTrigger>>("trigger").unwrap();
        assert!(trig.to_str().contains("s"), "{}", trig.to_str());
        // An unknown key name (GTK 4.14 accepts "<<nonsense" as an empty trigger; 4.22 refuses it).
        assert!(h.set_value(sc, "trigger", &Value::Str("<Control>nosuchkey".into())).unwrap_err().contains("cannot parse shortcut"));
        let fl = h.create("FileLauncher");
        h.set_value(fl, "file", &Value::Str("/tmp/some file.txt".into())).unwrap();
        let file = h.obj(fl).property::<Option<gtk::gio::File>>("file").unwrap();
        assert_eq!(file.path().unwrap().to_str().unwrap(), "/tmp/some file.txt");
        h.set_value(fl, "file", &Value::Str("file:///tmp/other.txt".into())).unwrap();
        let file = h.obj(fl).property::<Option<gtk::gio::File>>("file").unwrap();
        assert_eq!(file.uri().as_str(), "file:///tmp/other.txt");
    });
}

#[test]
fn descriptors_cover_properties_and_types() {
    on_gtk(|| {
        let h = Harness::new();
        let w = h.create("Label");
        let d = h.prop_desc(w, "label").unwrap();
        assert_eq!((d.params[0].tag, d.ret.tag), (SVL_STR, SVL_STR));
        assert!(d.params[0].has(SVL_D_NULLABLE));
        // Read-only properties publish no parameter.
        let ro = h.prop_desc(w, "scale-factor").unwrap();
        assert!(ro.params.is_empty(), "{ro:?}");
        assert!(h.backend_ref().describe_type(0xdead_beef).is_none());
        assert!(h.backend_ref().describe(NameKind::Prop, h.kind_id("Label"), 999_999).is_none());
        // Type ids are stable for the same GType.
        let a = slot_ty(&h, w, "ellipsize");
        let b = slot_ty(&h, h.create("Inscription"), "text-overflow");
        assert_ne!(a, b);
        assert_eq!(slot_ty(&h, w, "ellipsize"), a);
    });
}

#[test]
fn glib_error_values_do_not_cross_as_garbage() {
    // A property the backend cannot convert is a clean error, never a crash.
    on_gtk(|| {
        let h = Harness::new();
        let l = h.create("Label");
        let e = h.set_value(l, "attributes", &i(3)).unwrap_err();
        assert!(e.contains("attributes"), "{e}");
        let _ = glib::Type::OBJECT;
    });
}
