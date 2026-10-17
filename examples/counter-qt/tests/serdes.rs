//! Round-trip tests for the ABI v3 value model on the Qt backend: a value goes in through the C ABI
//! (exactly as the host sends it), the test reads what the toolkit really holds (`QObject::property`
//! through a test hook) and reads the value back through the ABI.
//!
//! One nextest test per case (`harness = false`: Qt needs the main thread). Needs a display:
//! `xvfb-run -a cargo nextest run -p counter-qt`.

use counter_qt::hooks::native_prop;
use svelters_abi::*;

fn be() -> BackendRef<'static> {
    // SAFETY: the generated backend is a static vtable.
    unsafe { BackendRef::from_raw(counter_qt::backend()) }.expect("abi version")
}

struct Obj {
    h: svl_handle,
    kind: svl_id,
}

fn mk(tag: &str) -> Obj {
    let b = be();
    let kind = b.resolve(NameKind::Kind, 0, tag);
    assert_ne!(kind, 0, "unknown kind {tag}");
    Obj { h: b.create(kind).expect("create"), kind }
}

impl Obj {
    fn pid(&self, name: &str) -> svl_id {
        let id = be().resolve(NameKind::Prop, self.kind, name);
        assert_ne!(id, 0, "unknown prop {name}");
        id
    }
    fn mid(&self, name: &str) -> svl_id {
        let id = be().resolve(NameKind::Method, self.kind, name);
        assert_ne!(id, 0, "unknown method {name}");
        id
    }
    fn set(&self, prop: &str, v: &Value) -> Result<(), String> {
        be().set_prop(self.h, self.pid(prop), v).map_err(|e| e.message)
    }
    fn get(&self, prop: &str) -> Value {
        be().get_prop(self.h, self.pid(prop)).expect("get_prop")
    }
    fn call(&self, m: &str, args: &[Value]) -> Result<Value, String> {
        be().call(self.h, self.mid(m), args).map_err(|e| e.message)
    }
    fn native(&self, name: &str) -> String {
        native_prop(self.h, name, false)
    }
    fn desc(&self, prop: &str) -> MemberDesc {
        be().describe(NameKind::Prop, self.kind, self.pid(prop)).expect("descriptor")
    }
    fn tdesc(&self, prop: &str) -> TypeDesc {
        let d = self.desc(prop);
        let slot = d.params.first().unwrap_or(&d.ret);
        be().describe_type(slot.ty).expect("type descriptor")
    }
    /// Enum or flags value by member names, as the host would encode it.
    fn named(&self, prop: &str, names: &[&str]) -> Value {
        let slot = self.desc(prop).params[0].clone();
        let t = self.tdesc(prop);
        let mut bits = 0i64;
        for n in names {
            bits |= t.members.iter().find(|m| m.name == *n).unwrap_or_else(|| panic!("no member {n}: {:?}", t.members)).value;
        }
        match slot.tag {
            SVL_ENUM => Value::Enum { ty: slot.ty, value: bits },
            SVL_FLAGS => Value::Flags { ty: slot.ty, mask: bits as u64 },
            t => panic!("tag {t}"),
        }
    }
}

fn names_of(v: &Value) -> Vec<String> {
    let (ty, bits): (u32, u64) = match v {
        Value::Enum { ty, value } => (*ty, *value as u64),
        Value::Flags { ty, mask } => (*ty, *mask),
        other => panic!("not enum/flags: {other:?}"),
    };
    let t = be().describe_type(ty).expect("type descriptor");
    let enum_like = matches!(v, Value::Enum { .. });
    t.members
        .iter()
        .filter(|m| if enum_like { m.value as u64 == bits } else { m.value != 0 && bits & m.value as u64 == m.value as u64 })
        .map(|m| m.name.clone())
        .collect()
}

fn rec(ty_name: &str, ty: svl_id, fields: Vec<Value>) -> Value {
    let t = be().describe_type(ty).unwrap_or_else(|| panic!("record type {ty}"));
    assert_eq!(t.name, ty_name);
    assert_eq!(t.fields.len(), fields.len());
    Value::Record { ty, fields }
}

fn record_of(o: &Obj, prop: &str, fields: &[Value]) -> Value {
    let d = o.desc(prop);
    let ty = d.params.first().unwrap_or(&d.ret).ty;
    rec(&be().describe_type(ty).unwrap().name, ty, fields.to_vec())
}

fn f(x: f64) -> Value {
    Value::F64(x)
}
fn i(x: i64) -> Value {
    Value::I64(x)
}

fn field_names(o: &Obj, prop: &str) -> Vec<String> {
    o.tdesc(prop).fields.iter().map(|f| f.name.clone().unwrap()).collect()
}

fn scalars() {
    let w = mk("box-layout");
    w.set("enabled", &Value::Bool(false)).unwrap();
    assert!(w.native("enabled").contains("false"), "{}", w.native("enabled"));
    assert_eq!(w.get("enabled"), Value::Bool(false));
    // Integers are range checked, floats are rejected where an integer is expected.
    let s = mk("spin-box");
    s.set("maximum", &i(1000)).unwrap();
    s.set("value", &i(42)).unwrap();
    assert!(s.native("value").contains("42"));
    assert_eq!(s.get("value"), Value::I64(42));
    let e = s.set("value", &i(1 << 40)).unwrap_err();
    assert!(e.contains("out of range"), "{e}");
    let e = s.set("value", &f(1.5)).unwrap_err();
    assert!(e.contains("expected integer"), "{e}");
    assert_eq!(s.set("value", &f(7.0)), Ok(()), "an integral float is an integer");
    assert_eq!(s.get("value"), Value::I64(7));
    assert!(s.set("value", &Value::Str("7".into())).is_err(), "strings are the host's to coerce");
    // The descriptor carries the range of an int property.
    let slot = &s.desc("value").params[0];
    assert_eq!((slot.tag, slot.min, slot.max), (SVL_I64, i32::MIN as f64, i32::MAX as f64));
    // f64
    let d = mk("double-spin-box");
    d.set("decimals", &i(3)).unwrap();
    d.set("value", &f(2.5)).unwrap();
    assert_eq!(d.get("value"), Value::F64(2.5));
    d.set("value", &i(3)).unwrap();
    assert_eq!(d.get("value"), Value::F64(3.0));
}

fn strings() {
    let w = mk("box-layout");
    // UTF-8, with a non-Latin-1 character.
    let s = "h\u{e9}llo \u{2713} \u{1F600}";
    w.set("tool-tip", &Value::Str(s.into())).unwrap();
    assert_eq!(w.get("tool-tip"), Value::Str(s.into()));
    // UTF-16 straight from the engine: no transcoding on the way in.
    let u16s: Vec<u16> = "w\u{f6}rld \u{2603}".encode_utf16().collect();
    let mut v = svl_value::new(
        SVL_STR,
        svl_value_u { s: svl_str { ptr: u16s.as_ptr().cast(), len: u16s.len() } },
    );
    v.flags = SVL_F_UTF16;
    be().set_prop_raw(w.h, w.pid("tool-tip"), &v).unwrap();
    assert_eq!(w.get("tool-tip"), Value::Str("w\u{f6}rld \u{2603}".into()));
    // Embedded NULs survive: lengths are explicit.
    w.set("tool-tip", &Value::Str("a\0b".into())).unwrap();
    assert_eq!(w.get("tool-tip"), Value::Str("a\0b".into()));
    // null clears; a number is not a string.
    w.set("tool-tip", &Value::Null).unwrap();
    assert_eq!(w.get("tool-tip"), Value::Str(String::new()));
    let e = w.set("tool-tip", &i(3)).unwrap_err();
    assert!(e.contains("expected string"), "{e}");
    assert_ne!(be().caps() & SVL_CAP_UTF16_IN, 0);
}

fn enums() {
    let l = mk("label");
    let t = l.tdesc("text-format");
    assert_eq!(t.kind, SVL_ENUM);
    let names: Vec<_> = t.members.iter().map(|m| m.name.as_str()).collect();
    assert!(names.contains(&"rich-text") && names.contains(&"plain-text"), "{names:?}");
    l.set("text-format", &l.named("text-format", &["rich-text"])).unwrap();
    assert!(l.native("textFormat").contains("RichText"), "{}", l.native("textFormat"));
    let back = l.get("text-format");
    assert_eq!(names_of(&back), vec!["rich-text".to_string()]);
    // The type id on the wire matches the descriptor; a foreign or unknown value is refused.
    let slot_ty = l.desc("text-format").params[0].ty;
    assert!(matches!(back, Value::Enum { ty, .. } if ty == slot_ty));
    let e = l.set("text-format", &Value::Enum { ty: slot_ty, value: 9999 }).unwrap_err();
    assert!(e.contains("not a Qt::TextFormat value"), "{e}");
    // Class-scoped enums (QLineEdit::EchoMode, QBoxLayout::Direction).
    let en = mk("line-edit");
    en.set("echo-mode", &en.named("echo-mode", &["password-echo-on-edit"])).unwrap();
    assert!(en.native("echoMode").contains("PasswordEchoOnEdit"));
    assert_eq!(names_of(&en.get("echo-mode")), vec!["password-echo-on-edit".to_string()]);
    let bx = mk("box-layout");
    assert_eq!(bx.tdesc("direction").members.iter().map(|m| m.name.clone()).collect::<Vec<_>>(), vec!["bottom-to-top", "left-to-right", "right-to-left", "top-to-bottom"]);
    bx.set("direction", &bx.named("direction", &["top-to-bottom"])).unwrap();
    assert_eq!(names_of(&bx.get("direction")), vec!["top-to-bottom".to_string()]);
    // Names are also accepted by the backend (the host normally resolves them).
    en.set("echo-mode", &Value::Str("no-echo".into())).unwrap();
    assert_eq!(names_of(&en.get("echo-mode")), vec!["no-echo".to_string()]);
    assert!(en.set("echo-mode", &Value::Str("bogus".into())).unwrap_err().contains("unknown"));
}

fn flags() {
    let l = mk("label");
    let t = l.tdesc("alignment");
    assert_eq!(t.kind, SVL_FLAGS);
    l.set("alignment", &l.named("alignment", &["right", "vcenter"])).unwrap();
    let native = l.native("alignment");
    assert!(native.contains("AlignTrailing") && native.contains("AlignVCenter"), "{native}");
    let mut got = names_of(&l.get("alignment"));
    got.sort();
    assert!(got.contains(&"right".to_string()) && got.contains(&"vcenter".to_string()), "{got:?}");
    let ty = l.desc("alignment").params[0].ty;
    let e = l.set("alignment", &Value::Flags { ty, mask: 1 << 30 }).unwrap_err();
    assert!(e.contains("unknown bits"), "{e}");
    // An empty set is a valid value.
    l.set("alignment", &Value::Flags { ty, mask: 0 }).unwrap();
    assert!(matches!(l.get("alignment"), Value::Flags { mask: 0, .. }));
    // Flags of a class-scoped enum.
    let c = mk("color-dialog");
    c.set("options", &c.named("options", &["show-alpha-channel", "no-buttons"])).unwrap();
    let mut got = names_of(&c.get("options"));
    got.sort();
    assert_eq!(got, vec!["no-buttons".to_string(), "show-alpha-channel".to_string()]);
}

fn records() {
    let w = mk("box-layout");
    assert_eq!(field_names(&w, "geometry"), ["x", "y", "width", "height"]);
    assert_eq!(field_names(&w, "minimum-size"), ["width", "height"]);
    assert_eq!(field_names(&w, "pos"), ["x", "y"]);
    // Rect (QRect: integral fields).
    w.set("geometry", &record_of(&w, "geometry", &[i(10), i(20), i(300), i(400)])).unwrap();
    assert!(w.native("geometry").contains("10,20 300x400"), "{}", w.native("geometry"));
    assert_eq!(w.get("geometry"), record_of(&w, "geometry", &[i(10), i(20), i(300), i(400)]));
    // Integral floats are accepted for integer fields, fractions are not.
    w.set("geometry", &record_of(&w, "geometry", &[f(1.0), f(2.0), f(3.0), f(4.0)])).unwrap();
    let e = w.set("geometry", &record_of(&w, "geometry", &[f(1.5), i(2), i(3), i(4)])).unwrap_err();
    assert!(e.contains("expected integer"), "{e}");
    // A record with the wrong number of fields, or a wrong tag, is refused.
    let ty = w.desc("geometry").params[0].ty;
    assert!(w.set("geometry", &Value::Record { ty, fields: vec![i(1)] }).unwrap_err().contains("4 fields"));
    assert!(w.set("geometry", &i(1)).unwrap_err().contains("Rect record"));
    // Size
    w.set("minimum-size", &record_of(&w, "minimum-size", &[i(64), i(48)])).unwrap();
    assert!(w.native("minimumSize").contains("64, 48"), "{}", w.native("minimumSize"));
    assert_eq!(w.get("minimum-size"), record_of(&w, "minimum-size", &[i(64), i(48)]));
    // Insets live on the layout.
    let ty = w.desc("contents-margins").params[0].ty;
    let m = Value::Record { ty, fields: vec![i(1), i(2), i(3), i(4)] };
    w.set("contents-margins", &m).unwrap();
    let margins = native_prop(w.h, "contentsMargins", true);
    assert!(margins.contains("1") && !margins.is_empty(), "{margins}");
    assert_eq!(w.get("contents-margins"), m, "top, right, bottom, left");
    // Point and the read-only size/pos.
    w.set("geometry", &record_of(&w, "geometry", &[i(7), i(9), i(100), i(80)])).unwrap();
    assert_eq!(w.get("pos"), record_of(&w, "pos", &[i(7), i(9)]));
    assert_eq!(w.get("size"), record_of(&w, "size", &[i(100), i(80)]));
    assert!(w.set("pos", &record_of(&w, "pos", &[i(0), i(0)])).unwrap_err().contains("read-only"));
    // A method taking a record.
    w.call("resizeTo", &[record_of(&w, "size", &[i(120), i(90)])]).unwrap();
    assert_eq!(w.get("size"), record_of(&w, "size", &[i(120), i(90)]));
    // Type ids differ for the integral and floating variants only by descriptor, never by name.
    assert_eq!(be().describe_type(ty).unwrap().name, "Insets");
}

fn color() {
    let c = mk("color-dialog");
    assert_eq!(field_names(&c, "current-color"), ["red", "green", "blue", "alpha"]);
    let slot = c.desc("current-color").params[0].clone();
    assert!(slot.has(SVL_D_STRING_FORM));
    let ty = slot.ty;
    c.set("current-color", &Value::Record { ty, fields: vec![f(1.0), f(0.0), f(0.0), f(1.0)] }).unwrap();
    let Value::Record { fields, .. } = c.get("current-color") else { panic!() };
    assert_eq!(fields, vec![f(1.0), f(0.0), f(0.0), f(1.0)]);
    // The CSS-ish string form is parsed by the backend.
    c.set("current-color", &Value::Str("#00ff00".into())).unwrap();
    let Value::Record { fields, .. } = c.get("current-color") else { panic!() };
    assert_eq!(fields, vec![f(0.0), f(1.0), f(0.0), f(1.0)]);
    c.set("current-color", &Value::Str("purple".into())).unwrap();
    let e = c.set("current-color", &Value::Str("not-a-color".into())).unwrap_err();
    assert!(e.contains("cannot parse color"), "{e}");
    let e = c.set("current-color", &Value::Record { ty, fields: vec![f(2.0), f(0.0), f(0.0), f(1.0)] }).unwrap_err();
    assert!(e.contains("0..1"), "{e}");
}

fn handles_and_nullable() {
    let label = mk("label");
    let entry = mk("line-edit");
    assert_eq!(label.call("buddy", &[]).unwrap(), Value::Null);
    label.call("setBuddy", &[Value::Handle(entry.h)]).unwrap();
    assert_eq!(label.call("buddy", &[]).unwrap(), Value::Handle(entry.h));
    // null clears it.
    label.call("setBuddy", &[Value::Null]).unwrap();
    assert_eq!(label.call("buddy", &[]).unwrap(), Value::Null);
    // A stale handle is an error, not a crash.
    be().destroy(entry.h);
    let e = label.call("setBuddy", &[Value::Handle(entry.h)]).unwrap_err();
    assert!(e.contains("stale or unknown handle"), "{e}");
    assert!(label.call("setBuddy", &[i(3)]).unwrap_err().contains("expected handle"));
}

fn boxed_values() {
    let b1 = mk("push-button");
    let b2 = mk("push-button");
    // A string form builds the value; reading it back gives an opaque handle.
    b1.set("icon", &Value::Str("document-new".into())).unwrap();
    let Value::Boxed { ty, handle } = b1.get("icon") else { panic!("boxed") };
    let td = be().describe_type(ty).unwrap();
    assert_eq!((td.kind, td.name.as_str()), (SVL_BOXED, "QIcon"));
    assert!(b1.desc("icon").params[0].has(SVL_D_STRING_FORM));
    // Passing the handle to another widget copies the (implicitly shared) value.
    b2.set("icon", &Value::Boxed { ty, handle }).unwrap();
    // A boxed value is a VALUE-class handle.
    assert_eq!(be().handle_info(handle).unwrap().lifetime, SVL_LIFE_VALUE);
    // Release is explicit and idempotent; the handle is stale afterwards.
    be().destroy(handle);
    assert!(be().handle_info(handle).is_err());
    be().destroy(handle);
    let e = b2.set("icon", &Value::Boxed { ty, handle }).unwrap_err();
    assert!(e.contains("stale boxed handle"), "{e}");
    // A boxed handle of another type is refused.
    let l = mk("label");
    let Value::Boxed { ty: pty, handle: ph } = l.get("pixmap") else { panic!("boxed pixmap") };
    assert_ne!(pty, ty);
    let e = b2.set("icon", &Value::Boxed { ty: pty, handle: ph }).unwrap_err();
    assert!(e.contains("is not a QIcon"), "{e}");
    be().destroy(ph);
    // Other string forms.
    b1.set("shortcut", &Value::Str("Ctrl+S".into())).unwrap();
    assert!(b1.native("shortcut").contains("Ctrl+S"), "{}", b1.native("shortcut"));
    let Value::Boxed { handle: sh, .. } = b1.get("shortcut") else { panic!() };
    be().destroy(sh);
    // Null boxed is a null value.
    b2.set("icon", &Value::Null).unwrap();
    // Size record next to it.
    let ty = b1.desc("icon-size").params[0].ty;
    b1.set("icon-size", &Value::Record { ty, fields: vec![i(32), i(32)] }).unwrap();
    assert!(b1.native("iconSize").contains("32, 32"), "{}", b1.native("iconSize"));
}

fn bytes() {
    let w = mk("box-layout");
    let Value::Bytes(saved) = w.call("saveGeometry", &[]).unwrap() else { panic!("bytes") };
    assert!(!saved.is_empty());
    // BYTES in: borrowed for the call, copied by the backend.
    let r = w.call("restoreGeometry", &[Value::Bytes(saved.clone())]).unwrap();
    assert!(matches!(r, Value::Bool(_)));
    let r = w.call("restoreGeometry", &[Value::Bytes(vec![1, 2, 3])]).unwrap();
    assert_eq!(r, Value::Bool(false), "garbage is not a geometry");
    assert!(w.call("restoreGeometry", &[Value::Str("x".into())]).unwrap_err().contains("expected bytes"));
    assert_eq!(w.call("restoreGeometry", &[Value::Null]).unwrap(), Value::Bool(false));
}

fn dates_and_times() {
    let d = mk("date-edit");
    let ty = d.desc("date").params[0].ty;
    let td = be().describe_type(ty).unwrap();
    assert_eq!(td.fields.iter().map(|f| f.name.clone().unwrap()).collect::<Vec<_>>(), ["year", "month", "day"]);
    d.set("date", &Value::Record { ty, fields: vec![i(2024), i(2), i(29)] }).unwrap();
    assert!(d.native("date").contains("2024-02-29"), "{}", d.native("date"));
    assert_eq!(d.get("date"), Value::Record { ty, fields: vec![i(2024), i(2), i(29)] });
    let e = d.set("date", &Value::Record { ty, fields: vec![i(2023), i(2), i(29)] }).unwrap_err();
    assert!(e.contains("invalid date"), "{e}");
    let t = mk("time-edit");
    let ty = t.desc("time").params[0].ty;
    t.set("time", &Value::Record { ty, fields: vec![i(13), i(5), i(9), i(250)] }).unwrap();
    assert_eq!(t.get("time"), Value::Record { ty, fields: vec![i(13), i(5), i(9), i(250)] });
    assert!(t.set("time", &Value::Record { ty, fields: vec![i(25), i(0), i(0), i(0)] }).is_err());
    // A datetime is epoch milliseconds, typed so JS can make it a Date.
    let dt = mk("date-time-edit");
    let ms = 1_700_000_000_123.0;
    let mut v = svl_value::typed(SVL_F64, SVL_T_DATETIME, svl_value_u { f: ms });
    v.flags = 0;
    be().set_prop_raw(dt.h, dt.pid("date-time"), &v).unwrap();
    let raw = be().get_prop_raw(dt.h, dt.pid("date-time")).unwrap();
    assert_eq!((raw.tag, raw.type_), (SVL_F64, SVL_T_DATETIME));
    assert_eq!(unsafe { raw.u.f }, ms);
}

fn variants() {
    let c = mk("combo-box");
    let tree = Value::Map(vec![
        ("name".into(), Value::Str("caf\u{e9}".into())),
        ("n".into(), i(7)),
        ("ratio".into(), f(0.5)),
        ("when".into(), Value::DateTime(1_700_000_000_000.0)),
        ("tags".into(), Value::List(vec![Value::Str("a".into()), Value::Bool(true), Value::Null, Value::Bytes(vec![1, 2])])),
    ]);
    // addItem(text, QVariant): a tree goes in; itemData(index, role) gives it back.
    c.call("addItem", &[Value::Str("first".into()), tree.clone()]).unwrap();
    c.call("addItem", &[Value::Str("second".into()), Value::Str("just a string".into())]).unwrap();
    c.call("addItem", &[Value::Str("third".into()), Value::Null]).unwrap();
    assert_eq!(c.get("count"), Value::I64(3));
    let Value::Map(mut got) = c.call("itemData", &[i(0), i(256)]).unwrap() else { panic!("map") };
    got.sort_by(|a, b| a.0.cmp(&b.0));
    let Value::Map(mut want) = tree else { unreachable!() };
    want.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(got, want);
    assert_eq!(c.call("itemData", &[i(1), i(256)]).unwrap(), Value::Str("just a string".into()));
    assert_eq!(c.call("itemData", &[i(2), i(256)]).unwrap(), Value::Null);
    // The read-only variant property follows the current item (the first one).
    let cd = c.get("current-data");
    let Value::Map(cur) = cd else { panic!("map, got {cd:?}") };
    assert_eq!(cur.len(), 5);
    assert!(c.set("current-data", &Value::Null).is_err(), "currentData has no setter");
    // variant(signature, value): a basic signature fixes the QMetaType and range, the rest is the value.
    let typed = |sig: &str, v: Value| Value::Record { ty: SVL_T_TYPED_VARIANT, fields: vec![Value::Str(sig.into()), v] };
    c.call("addItem", &[Value::Str("u".into()), typed("u", i(5))]).unwrap();
    assert_eq!(c.call("itemData", &[i(3), i(256)]).unwrap(), i(5));
    c.call("addItem", &[Value::Str("d".into()), typed("d", i(2))]).unwrap();
    assert_eq!(c.call("itemData", &[i(4), i(256)]).unwrap(), f(2.0));
    c.call("addItem", &[Value::Str("pair".into()), typed("(is)", Value::List(vec![i(1), Value::Str("x".into())]))]).unwrap();
    assert_eq!(c.call("itemData", &[i(5), i(256)]).unwrap(), Value::List(vec![i(1), Value::Str("x".into())]));
    let e = c.call("addItem", &[Value::Str("x".into()), typed("y", i(300))]).unwrap_err();
    assert!(e.contains("300 does not fit the type"), "{e}");
    let e = c.call("addItem", &[Value::Str("x".into()), typed("s", i(1))]).unwrap_err();
    assert!(e.contains("does not match the signature"), "{e}");
    // A typed record cannot be nested in a variant.
    let ty = mk("box-layout").desc("geometry").params[0].ty;
    let rec = Value::Record { ty, fields: vec![i(1), i(2), i(3), i(4)] };
    let e = c.call("addItem", &[Value::Str("x".into()), rec]).unwrap_err();
    assert!(e.contains("cannot be held in a variant"), "{e}");
}

fn errors_cross_the_boundary_as_values() {
    let w = mk("box-layout");
    // A C++ exception never unwinds into the host: it becomes an ERROR out value (domain "c++").
    counter_qt::hooks::test_throw_next();
    let err = be().call(w.h, w.mid("saveGeometry"), &[]).unwrap_err();
    assert!(err.message.contains("test exception"), "{err:?}");
    let n = err.native.expect("native error");
    assert_eq!((n.domain.as_str(), n.message.as_str(), &n.code), ("c++", "test exception", &Value::Null));
    // The backend keeps working afterwards.
    assert!(matches!(w.call("saveGeometry", &[]).unwrap(), Value::Bytes(_)));
    // Conversion failures are plain errors (rc < 0 + last_error), not native errors.
    let err = be().set_prop(w.h, w.pid("enabled"), &Value::Str("x".into())).unwrap_err();
    assert!(err.native.is_none() && err.message.contains("expected boolean"), "{err:?}");
}

fn descriptors() {
    let w = mk("box-layout");
    // Every typed prop has a descriptor; the unknown ones do not.
    let d = w.desc("geometry");
    assert_eq!((d.params.len(), d.params[0].tag, d.ret.tag), (1, SVL_RECORD, SVL_RECORD));
    assert_eq!(d.params[0].name.as_deref(), Some("geometry"));
    assert!(be().describe(NameKind::Prop, w.kind, 99_999).is_none());
    assert!(be().describe_type(123_456_789).is_none());
    // Read-only: no parameter.
    assert_eq!(w.desc("pos").params.len(), 0);
    // Methods: parameters and return.
    let m = be().describe(NameKind::Method, w.kind, w.mid("restoreGeometry")).unwrap();
    assert_eq!((m.params[0].tag, m.ret.tag), (SVL_BYTES, SVL_BOOL));
    let m = be().describe(NameKind::Method, w.kind, w.mid("resizeTo")).unwrap();
    assert_eq!(m.params[0].tag, SVL_RECORD);
}

fn out_and_inout_params() {
    // Out only: the backend is called without arguments and answers [ret, outs...] (ret is null for void).
    let e = mk("box-layout");
    let d = be().describe(NameKind::Method, e.kind, e.mid("getContentsMargins")).unwrap();
    let flags: Vec<u16> = d.params.iter().map(|p| p.flags & SVL_D_OUT).collect();
    assert_eq!(flags, [SVL_D_OUT; 4]);
    assert!(d.ret.is_void());
    let Value::List(items) = e.call("getContentsMargins", &[]).unwrap() else { panic!("list") };
    assert_eq!(items.len(), 5);
    assert_eq!(items[0], Value::Null);
    e.set("contents-margins", &record_of(&e, "contents-margins", &[i(1), i(2), i(3), i(4)])).unwrap();
    let Value::List(items) = e.call("getContentsMargins", &[]).unwrap() else { panic!("list") };
    assert_eq!(items[1..], [Value::I64(4), Value::I64(1), Value::I64(2), Value::I64(3)], "left, top, right, bottom");
    assert!(e.call("getContentsMargins", &[i(1)]).unwrap_err().contains("expected 0 argument"));
    // In-out: QAbstractSpinBox::validate(QString &input, int &pos) -> State.
    let s = mk("spin-box");
    let d = be().describe(NameKind::Method, s.kind, s.mid("validate")).unwrap();
    assert_eq!(d.params.iter().map(|p| p.flags & SVL_D_INOUT).collect::<Vec<_>>(), [SVL_D_INOUT; 2]);
    assert_eq!(d.ret.tag, SVL_ENUM);
    s.set("maximum", &i(100)).unwrap();
    let Value::List(items) = s.call("validate", &[Value::Str("42".into()), i(2)]).unwrap() else { panic!("list") };
    assert_eq!(items.len(), 3);
    assert!(matches!(items[0], Value::Enum { value: 2, .. }), "Acceptable: {:?}", items[0]);
    assert_eq!((&items[1], &items[2]), (&Value::Str("42".into()), &Value::I64(2)));
    let Value::List(items) = s.call("validate", &[Value::Str("abc".into()), i(3)]).unwrap() else { panic!("list") };
    assert!(matches!(items[0], Value::Enum { value: 0, .. }), "Invalid: {:?}", items[0]);
}

/// A host that records every callback invocation and lets the test choose the answer.
struct Recorder {
    calls: std::rc::Rc<std::cell::RefCell<Vec<(u64, Vec<Value>)>>>,
    answer: std::rc::Rc<std::cell::RefCell<Result<Value, String>>>,
}

impl HostCallbacks for Recorder {
    fn post_event(&self, _: svl_handle, _: svl_id, _: &[Value]) {}
    fn checkpoint(&self) {}
    fn call_sync(&self, _: svl_handle, _: svl_id, _: &[Value]) -> Result<Value, SyncError> {
        Err(SyncError::Failed("no sync handlers".into()))
    }
    fn on_destroyed(&self, _: svl_handle, _: u32) {}
    fn frame(&self) {}
    fn invoke_callback(&self, callback: u64, args: &[Value]) -> StrResult<Value> {
        self.calls.borrow_mut().push((callback, args.to_vec()));
        self.answer.borrow().clone()
    }
}

type Calls = std::rc::Rc<std::cell::RefCell<Vec<(u64, Vec<Value>)>>>;
type Answer = std::rc::Rc<std::cell::RefCell<Result<Value, String>>>;

fn install_host() -> (Calls, Answer) {
    let calls = Calls::default();
    let answer: Answer = std::rc::Rc::new(std::cell::RefCell::new(Ok(Value::Null)));
    let vt = Box::leak(Box::new(HostVtable::new(Recorder { calls: calls.clone(), answer: answer.clone() })));
    // SAFETY: leaked above, so it outlives every call the backend can make.
    unsafe { counter_qt::set_host(vt.as_raw()) };
    (calls, answer)
}

fn callbacks_run_js_functions() {
    let (calls, answer) = install_host();
    let menu = mk("menu");
    let d = be().describe(NameKind::Method, menu.kind, menu.mid("addItem")).unwrap();
    assert_eq!(d.params.iter().map(|p| p.tag).collect::<Vec<_>>(), [SVL_STR, SVL_CALLBACK, SVL_BOXED]);
    assert!(d.ret.is_void(), "the QAction the C++ call returns is not exposed");
    let t = be().describe_type(d.params[1].ty).expect("callback type");
    assert_eq!((t.kind, t.scope, t.name.as_str()), (SVL_CALLBACK, SVL_CB_NOTIFIED, "(checked: boolean) => void"));
    assert_eq!(t.fields.len(), 1);
    assert_eq!((t.fields[0].tag, t.fields[0].name.as_deref()), (SVL_BOOL, Some("checked")));
    assert!(t.ret.is_void());

    let cb = |id: u64| Value::Callback { id, ty: d.params[1].ty };
    assert_eq!(menu.call("addItem", &[Value::Str("Open".into()), cb(7), Value::Str("Ctrl+O".into())]).unwrap(), Value::Null);
    menu.call("addItem", &[Value::Str("Save".into()), cb(8), Value::Str("".into())]).unwrap();
    // Triggering the actions (as a click would) runs each JS function with the signal's argument.
    assert_eq!(counter_qt::hooks::trigger_actions(menu.h).unwrap(), 2);
    assert_eq!(*calls.borrow(), [(7, vec![Value::Bool(false)]), (8, vec![Value::Bool(false)])]);
    // A function the host refuses (threw, released, owner gone) is the host's business: Qt carries on.
    *answer.borrow_mut() = Err("it threw".into());
    assert_eq!(counter_qt::hooks::trigger_actions(menu.h).unwrap(), 2);
    assert_eq!(calls.borrow().len(), 4);
    // Validation: the slot must be a function.
    let err = menu.call("addItem", &[Value::Str("Bad".into()), Value::Str("nope".into()), Value::Null]).unwrap_err();
    assert!(err.contains("addItem(functor): expected a function"), "{err}");
}

fn async_methods_complete_through_signals() {
    let (calls, _) = install_host();
    let dlg = mk("color-dialog");
    let d = be().describe(NameKind::Method, dlg.kind, dlg.mid("open")).unwrap();
    assert_eq!(d.flags & SVL_M_ASYNC, SVL_M_ASYNC);
    let t = be().describe_type(d.params.last().unwrap().ty).unwrap();
    assert_eq!((t.kind, t.scope, t.name.as_str()), (SVL_CALLBACK, SVL_CB_ASYNC, "(result: number) => void"));
    let done = |id: u64| Value::Callback { id, ty: d.params[0].ty };

    // accept() -> finished(1) -> the completion callback gets the result once.
    dlg.call("open", &[done(9)]).unwrap();
    assert!(calls.borrow().is_empty(), "nothing completes before the dialog finishes");
    dlg.call("accept", &[]).unwrap();
    assert_eq!(*calls.borrow(), [(9, vec![Value::I64(1)])]);
    dlg.call("open", &[done(10)]).unwrap();
    dlg.call("reject", &[]).unwrap();
    assert_eq!(calls.borrow()[1], (10, vec![Value::I64(0)]));
    dlg.call("accept", &[]).unwrap();
    assert_eq!(calls.borrow().len(), 2, "a finished operation does not complete twice");

    // Abort: backend.cancel(id) closes the dialog and completes with a NativeError, once.
    dlg.call("open", &[done(11)]).unwrap();
    assert!(be().cancel(11));
    let calls = calls.borrow();
    assert_eq!(calls.len(), 3);
    let Value::Error(e) = &calls[2].1[0] else { panic!("error expected, got {:?}", calls[2]) };
    assert_eq!((calls[2].0, e.domain.as_str(), e.message.as_str()), (11, "qt", "operation cancelled"));
    drop(calls);
    assert!(!be().cancel(11), "already settled");
    assert!(!be().cancel(999), "unknown operation");
    // Wrong arity: the completion callback is mandatory.
    assert!(dlg.call("open", &[]).unwrap_err().contains("expected 1 argument"));
}

fn main() {
    // The AT-SPI bridge talks to a session bus that does not exist in CI.
    // SAFETY: single-threaded at this point.
    unsafe {
        std::env::set_var("QT_ACCESSIBILITY", "0");
        // Tests must not talk to the developer's session bus (QtDBus aborts on a half-working one).
        std::env::set_var("DBUS_SESSION_BUS_ADDRESS", "unix:path=/nonexistent");
    }
    counter_qt::run_cases(&[
        ("scalars", scalars),
        ("strings", strings),
        ("enums", enums),
        ("flags", flags),
        ("records", records),
        ("color", color),
        ("handles_and_nullable", handles_and_nullable),
        ("boxed_values", boxed_values),
        ("bytes", bytes),
        ("dates_and_times", dates_and_times),
        ("variants", variants),
        ("errors_cross_the_boundary_as_values", errors_cross_the_boundary_as_values),
        ("descriptors", descriptors),
        ("out_and_inout_params", out_and_inout_params),
        ("callbacks_run_js_functions", callbacks_run_js_functions),
        ("async_methods_complete_through_signals", async_methods_complete_through_signals),
    ]);
}
