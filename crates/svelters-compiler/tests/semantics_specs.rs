//! BackendSemantics over the real generated specs (metadata/semantics/{gtk,qt}.json): the
//! elements of the counter / if-toggle / keyed-list / native-close fixtures, bindings, slots and
//! native style lowering on both backends.

use svelters_compiler::diag::Diagnostics;
use svelters_compiler::semantics::*;
use svelters_compiler::style::ValueToken;

fn load(backend: &str) -> SpecBackend {
    let path = format!("{}/../../metadata/semantics/{backend}.json", env!("CARGO_MANIFEST_DIR"));
    SpecBackend::from_path(path).unwrap()
}

fn both() -> [SpecBackend; 2] {
    [load("gtk"), load("qt")]
}

fn el(b: &SpecBackend, tag: &str) -> ElementRef {
    b.resolve_element(tag).unwrap_or_else(|e| panic!("{}: {e}", b.backend()))
}

/// `gtk` on GTK, `qt` on Qt: each backend has its own element and member names.
fn t<T>(b: &SpecBackend, gtk: T, qt: T) -> T {
    if b.backend() == "qt" { qt } else { gtk }
}

fn ident(s: &str) -> Vec<ValueToken> {
    vec![ValueToken::Ident(s.into())]
}

fn len(v: f64, unit: Option<&str>) -> Vec<ValueToken> {
    vec![ValueToken::Number { value: v, unit: unit.map(str::to_string) }]
}

#[test]
fn builtin_specs_match_the_checked_in_files() {
    for name in ["gtk", "qt"] {
        let a = SpecBackend::builtin(name).unwrap();
        let b = load(name);
        assert_eq!(a.backend(), name);
        assert_eq!(a.spec().kinds.len(), b.spec().kinds.len());
    }
    assert!(SpecBackend::builtin("win32").is_err());
    assert!(SpecBackend::from_json(r#"{"schema":"other"}"#).is_err());
}

#[test]
fn fixture_elements_resolve_on_both_backends() {
    for b in both() {
        let tags = if b.backend() == "qt" {
            ["main-window", "box-layout", "label", "push-button", "line-edit", "scroll-area"]
        } else {
            ["window", "box", "label", "button", "entry", "scrolled-window"]
        };
        for tag in tags {
            let e = el(&b, tag);
            assert!(b.is_a(&e, tag));
        }
    }
    let gtk = load("gtk");
    assert_eq!(el(&gtk, "window").kind, "Gtk.Window");
    assert_eq!(el(&gtk, "application-window").kind, "Gtk.ApplicationWindow");
    assert_eq!(el(&gtk, "header-bar").kind, "Gtk.HeaderBar");
    assert!(gtk.is_a(&el(&gtk, "application-window"), "Gtk.Window"));
    assert!(gtk.is_a(&el(&gtk, "window"), "GtkWindow"));
    assert!(gtk.is_a(&el(&gtk, "entry"), "Gtk.Editable"), "interfaces are in the chain");
    assert!(!gtk.is_a(&el(&gtk, "label"), "Gtk.Button"));
    let qt = load("qt");
    assert_eq!(el(&qt, "push-button").kind, "push-button");
    assert!(qt.is_a(&el(&qt, "push-button"), "QAbstractButton"));
    assert!(qt.is_a(&el(&qt, "main-window"), "QMainWindow"));
}

#[test]
fn lifetime_and_present() {
    for b in both() {
        let w = b.lifetime(&el(&b, t(&b, "window", "main-window")));
        assert_eq!(w, LifetimePlan { lifetime: Lifetime::Toplevel, present: Some("present".into()) }, "{}", b.backend());
        assert_eq!(b.lifetime(&el(&b, "label")).lifetime, Lifetime::Owned);
        assert_eq!(b.lifetime(&el(&b, t(&b, "box", "box-layout"))).present, None);
    }
}

#[test]
fn unknown_element_on_qt_suggests_and_hints() {
    let qt = load("qt");
    let e = qt.resolve_element("header-bar").unwrap_err();
    assert_eq!(e.code, codes::UNKNOWN_ELEMENT);
    assert!(e.suggestions.contains(&"menu-bar".to_string()), "{:?}", e.suggestions);
    assert!(e.help.as_deref().unwrap().contains("header bar"));
    let e = qt.resolve_element("lable").unwrap_err();
    assert_eq!(e.suggestions, vec!["label"]);
    // Reported as a backend diagnostic with the hint folded into help.
    let mut d = Diagnostics::for_backend("qt");
    e.report(&mut d, (3, 8));
    assert_eq!(d.list[0].code, "unknown_element");
    assert!(d.list[0].help.as_deref().unwrap().contains("`label`"));
}

#[test]
fn properties_by_js_or_native_name() {
    let gtk = load("gtk");
    let entry = el(&gtk, "entry");
    let p = gtk.property(&entry, "placeholder-text").unwrap();
    assert_eq!((p.js.as_str(), p.native.as_str(), p.ty), ("placeholder-text", "placeholder-text", PropType::String));
    assert!(gtk.property(&entry, "placeholder").is_err(), "no aliases");
    assert_eq!(gtk.property(&entry, "text").unwrap().owner, "Gtk.Editable");
    let bx = el(&gtk, "box");
    let o = gtk.property(&bx, "orientation").unwrap();
    assert_eq!(o.ty, PropType::Enum);
    assert_eq!(o.values.as_deref().unwrap(), ["horizontal", "vertical"]);
    assert_eq!(o.default, Some(serde_json::json!("horizontal")));
    let css = gtk.property(&el(&gtk, "button"), "css-classes").unwrap();
    assert_eq!((css.js.as_str(), css.ty, css.of.as_deref()), ("css-classes", PropType::List, Some("string")));
    let e = gtk.property(&el(&gtk, "label"), "lable").unwrap_err();
    assert_eq!(e.code, codes::UNKNOWN_PROPERTY);
    assert!(e.suggestions.contains(&"label".to_string()));
    let ro = gtk.writable_property(&entry, "cursor-position").unwrap_err();
    assert_eq!(ro.code, codes::READ_ONLY_PROPERTY);

    let qt = load("qt");
    let w = el(&qt, "main-window");
    assert_eq!(qt.property(&w, "windowTitle").unwrap().js, "window-title");
    assert_eq!(qt.property(&el(&qt, "box-layout"), "direction").unwrap().default, Some(serde_json::json!("top-to-bottom")));
    let e = qt.property(&el(&qt, "label"), "buddy").unwrap_err();
    assert!(e.message.contains("only annotated"), "{}", e.message);
}

#[test]
fn text_content() {
    for b in both() {
        let text = t(&b, "label", "text");
        assert_eq!(b.text_content(&el(&b, "label")).unwrap().js, text);
        assert_eq!(b.text_content(&el(&b, t(&b, "button", "push-button"))).unwrap().js, text);
        assert!(b.text_content(&el(&b, t(&b, "box", "box-layout"))).is_none());
    }
    let gtk = load("gtk");
    assert_eq!(gtk.text_content(&el(&gtk, "toggle-button")).unwrap().js, "label", "inherited from Gtk.Button");
}

#[test]
fn events_and_signals() {
    for b in both() {
        let button = el(&b, t(&b, "button", "push-button"));
        let c = b.event(&button, "onclicked").unwrap();
        assert_eq!((c.signal.as_str(), c.via_alias.as_deref()), ("clicked", None));
        assert_eq!(b.event(&button, "onclick").unwrap_err().code, codes::UNKNOWN_EVENT, "no DOM-style aliases");
        let entry = el(&b, t(&b, "entry", "line-edit"));
        let ch = b.event(&entry, t(&b, "onchanged", "ontext-changed")).unwrap();
        assert_eq!(ch.args, vec![("text".to_string(), "string".to_string())], "{}", b.backend());
        assert!(b.event(&entry, "onchange").is_err());
        let close = b.event(&el(&b, t(&b, "window", "main-window")), t(&b, "onclose-request", "onclose")).unwrap();
        assert_eq!(close.signal, t(&b, "close-request", "close"));
        assert!(close.disconnect_on_drop);
        let e = b.event(&button, "onclickd").unwrap_err();
        assert_eq!(e.code, codes::UNKNOWN_EVENT);
        assert!(e.suggestions.contains(&"onclicked".to_string()), "{:?}", e.suggestions);
    }
    let gtk = load("gtk");
    let close = gtk.event(&el(&gtk, "window"), "onclose-request").unwrap();
    assert_eq!((close.owner.as_str(), close.handler_result), ("Gtk.Window", HandlerResult::BoolStop));
    let qt = load("qt");
    let close = qt.event(&el(&qt, "main-window"), "onclose").unwrap();
    assert_eq!((close.handler_result, close.sync_required), (HandlerResult::BoolStop, true), "synthetic close filter is sync");
    let key = qt.event(&el(&qt, "line-edit"), "onkey-press").unwrap();
    assert_eq!((key.signal.as_str(), key.sync_required), ("key-press", true));
    assert!(qt.event(&el(&qt, "label"), "onclicked").is_err(), "QLabel has no clicked signal");
}

#[test]
fn bind_text_on_entry() {
    for b in both() {
        let entry = el(&b, t(&b, "entry", "line-edit"));
        let plan = b.plan_binding(&entry, "text").unwrap();
        let sync: Vec<String> = if b.backend() == "gtk" { vec!["changed".into(), "notify::text".into()] } else { vec!["textChanged(QString)".into()] };
        assert_eq!(
            plan,
            BindingPlan {
                prop: "text".into(),
                prop_type: PropType::String,
                signal: t(&b, "changed", "text-changed").into(),
                value: BindingValue::Arg(0),
                echo: Echo::BackendBlocks,
                direction: BindingDirection::TwoWay,
                compare_before_set: true,
                read_back: false,
                natively_normalizes: false,
                sync_signals_on_set: sync,
            },
            "{}",
            b.backend()
        );
        assert_eq!(plan.arg_operand(), 0);
        // The binding comes from the `bindings` annotation, not the hand-written rules.
        let k = &b.spec().kinds[&entry.kind];
        let spec = k.bindings.get("text").or_else(|| b.spec().kinds["Gtk.Entry"].bindings.get("text")).unwrap();
        assert!(spec.provenance.contains("signals-and-slots-semantics.md"), "{}", spec.provenance);
        assert_eq!(spec.lifetime["on_native_death"], "end_binding");
    }
}

#[test]
fn no_binding_for_label() {
    let gtk = load("gtk");
    // Gtk.Inscription has a `text` property but no binding annotation.
    // (GtkLabel:label is bindable since AI1 inferred the generic `bind:<prop>` facts.)
    let e = gtk.plan_binding(&el(&gtk, "inscription"), "text").unwrap_err();
    assert_eq!(e.code, codes::BIND_NO_PLAN);
    assert!(e.message.contains("notify::text"), "{}", e.message);
    assert!(gtk.plan_binding(&el(&gtk, "label"), "label").is_ok());
    let qt = load("qt");
    let e = qt.plan_binding(&el(&qt, "label"), "text").unwrap_err();
    assert_eq!(e.code, codes::BIND_NO_PLAN);
    assert!(e.message.contains("QLabel::text has no change notification"), "{}", e.message);
    // Read-only and unknown properties are reported as such.
    assert_eq!(gtk.plan_binding(&el(&gtk, "entry"), "cursor-position").unwrap_err().code, codes::READ_ONLY_PROPERTY);
    assert_eq!(qt.plan_binding(&el(&qt, "line-edit"), "nope").unwrap_err().code, codes::UNKNOWN_PROPERTY);
}

#[test]
fn gtk_slots() {
    let gtk = load("gtk");
    let (hb, button, label, win) = (el(&gtk, "header-bar"), el(&gtk, "button"), el(&gtk, "label"), el(&gtk, "window"));
    let p = gtk.place_child(&hb, &button, None).unwrap();
    assert_eq!((p.slot.as_str(), p.is_default, p.routing, p.many, p.order), ("start", true, Routing::Default, true, SlotOrder::AppendOnly));
    let p = gtk.place_child(&hb, &button, Some("end")).unwrap();
    assert_eq!((p.slot.as_str(), p.is_default, p.routing), ("end", false, Routing::Explicit));
    let p = gtk.place_child(&hb, &label, Some("title")).unwrap();
    assert_eq!((p.many, p.order), (false, SlotOrder::None));
    // window is Gtk.Window, which declares the titlebar slot.
    let p = gtk.place_child(&win, &hb, Some("titlebar")).unwrap();
    assert_eq!((p.slot.as_str(), p.owner.as_str()), ("titlebar", "Gtk.Window"));
    let p = gtk.place_child(&win, &el(&gtk, "box"), None).unwrap();
    assert_eq!((p.slot.as_str(), p.is_default), ("child", true));
    let p = gtk.place_child(&el(&gtk, "box"), &label, None).unwrap();
    assert_eq!((p.slot.as_str(), p.order), ("children", SlotOrder::BeforeSibling));
    let e = gtk.place_child(&win, &hb, Some("titelbar")).unwrap_err();
    assert_eq!((e.code, e.suggestions.as_slice()), (codes::UNKNOWN_SLOT, ["titlebar".to_string()].as_slice()));
    assert_eq!(gtk.place_child(&label, &button, None).unwrap_err().code, codes::CHILDREN_NOT_ALLOWED);
}

#[test]
fn qt_slots_and_kind_routing() {
    let qt = load("qt");
    let (w, bx) = (el(&qt, "main-window"), el(&qt, "box-layout"));
    let p = qt.place_child(&w, &bx, None).unwrap();
    assert_eq!((p.slot.as_str(), p.is_default, p.routing), ("central", true, Routing::Default));
    let p = qt.place_child(&w, &el(&qt, "status-bar"), None).unwrap();
    assert_eq!((p.slot.as_str(), p.is_default, p.routing), ("statusbar", false, Routing::ByKind));
    let p = qt.place_child(&w, &el(&qt, "menu-bar"), None).unwrap();
    assert_eq!((p.slot.as_str(), p.routing), ("menubar", Routing::ByKind));
    let p = qt.place_child(&w, &el(&qt, "menu-bar"), Some("menubar")).unwrap();
    assert_eq!(p.routing, Routing::Explicit);
    let e = qt.place_child(&w, &el(&qt, "push-button"), Some("statusbar")).unwrap_err();
    assert_eq!(e.code, codes::SLOT_REJECTS_CHILD);
    assert!(e.message.contains("QStatusBar"), "{}", e.message);
    assert_eq!(e.suggestions, vec!["central"]);
    let e = qt.place_child(&w, &bx, Some("titlebar")).unwrap_err();
    assert_eq!(e.code, codes::UNKNOWN_SLOT);
    assert!(!e.suggestions.is_empty());
    let p = qt.place_child(&bx, &el(&qt, "label"), None).unwrap();
    assert_eq!((p.many, p.order), (true, SlotOrder::NativeIndex));
    let p = qt.place_child(&el(&qt, "scroll-area"), &bx, None).unwrap();
    assert_eq!((p.slot.as_str(), p.many), ("child", false));
}

#[test]
fn style_setter_declarations() {
    for b in both() {
        let bx = el(&b, t(&b, "box", "box-layout"));
        let set = |prop: &str, value: StaticValue| Some(StyleLowering::SetProp { prop: prop.into(), value });
        // the box's direction: GTK orientation, Qt QBoxLayout direction
        let (dir, value, typo) = t(&b, ("orientation", "vertical", "vertcal"), ("direction", "top-to-bottom", "top-to-botom"));
        assert_eq!(b.style_declaration(&bx, None, dir, &ident(value)).unwrap(), set(dir, StaticValue::Enum(value.into())));
        assert_eq!(b.style_declaration(&bx, None, "spacing", &len(8.0, Some("px"))).unwrap(), set("spacing", StaticValue::Int(8)), "{}", b.backend());
        assert_eq!(b.style_declaration(&bx, None, "spacing", &len(8.0, None)).unwrap(), set("spacing", StaticValue::Int(8)), "unitless lengths are pixels");
        let e = b.style_declaration(&bx, None, "spacing", &len(8.0, Some("dp"))).unwrap_err();
        assert_eq!((e.code, e.suggestions.as_slice()), (codes::STYLE_BAD_VALUE, ["8px".to_string()].as_slice()));
        let e = b.style_declaration(&bx, None, "spacing", &len(1.5, Some("px"))).unwrap_err();
        assert!(e.message.contains("whole pixels"), "{}", e.message);
        let e = b.style_declaration(&bx, None, dir, &ident(typo)).unwrap_err();
        assert_eq!((e.code, e.suggestions.as_slice()), (codes::STYLE_BAD_VALUE, [value.to_string()].as_slice()));
        let e = b.style_declaration(&bx, None, dir, &len(1.0, None)).unwrap_err();
        assert_eq!(e.code, codes::STYLE_BAD_VALUE);
        // CSS properties and unknown names are not the compiler's
        assert_eq!(b.style_declaration(&bx, None, "color", &ident("red")).unwrap(), None);
        assert_eq!(b.style_declaration(&bx, None, "spacng", &len(1.0, None)).unwrap(), None);
        assert!(!b.is_setter_property("color") && b.is_setter_property("spacing"));
        let e = b.style_declaration(&el(&b, "label"), None, "spacing", &len(1.0, None)).unwrap_err();
        assert_eq!(e.code, codes::STYLE_NOT_APPLICABLE);
        assert!(e.message.contains("not <label>"), "{}", e.message);
    }
    // names are native: GTK's size request, Qt's minimum size
    let gtk = load("gtk");
    let qt = load("qt");
    assert_eq!(
        gtk.style_declaration(&el(&gtk, "button"), None, "height-request", &len(32.0, None)).unwrap(),
        Some(StyleLowering::SetProp { prop: "height-request".into(), value: StaticValue::Int(32) })
    );
    assert_eq!(
        qt.style_declaration(&el(&qt, "label"), None, "minimum-height", &len(4.0, Some("px"))).unwrap(),
        Some(StyleLowering::SetProp { prop: "minimum-height".into(), value: StaticValue::Int(4) })
    );
    assert_eq!(gtk.style_declaration(&el(&gtk, "button"), None, "minimum-height", &len(4.0, None)).unwrap(), None);
    assert_eq!(
        gtk.style_declaration(&el(&gtk, "entry"), None, "sensitive", &ident("false")).unwrap(),
        Some(StyleLowering::SetProp { prop: "sensitive".into(), value: StaticValue::Bool(false) })
    );
    assert_eq!(gtk.style_declaration(&el(&gtk, "entry"), None, "sensitive", &ident("no")).unwrap_err().code, codes::STYLE_BAD_VALUE);
    // Range: width-request >= -1.
    assert_eq!(gtk.style_declaration(&el(&gtk, "box"), None, "width-request", &len(-5.0, None)).unwrap_err().code, codes::STYLE_BAD_VALUE);
    // min-width is GTK CSS, not a setter
    assert_eq!(gtk.style_declaration(&el(&gtk, "box"), None, "min-width", &len(5.0, Some("px"))).unwrap(), None);
    // insets, number
    assert_eq!(
        qt.style_declaration(&el(&qt, "box-layout"), None, "contents-margins", &[len(4.0, None), len(8.0, Some("px"))].concat()).unwrap(),
        Some(StyleLowering::SetProp { prop: "contents-margins".into(), value: StaticValue::Record(serde_json::json!({"top": 4, "right": 8, "bottom": 4, "left": 8})) })
    );
    assert_eq!(
        gtk.style_declaration(&el(&gtk, "label"), None, "xalign", &len(0.5, None)).unwrap(),
        Some(StyleLowering::SetProp { prop: "xalign".into(), value: StaticValue::Float(0.5) })
    );
}

#[test]
fn style_child_attributes() {
    let qt = load("qt");
    let bx = el(&qt, "box-layout");
    let button = el(&qt, "push-button");
    assert_eq!(
        qt.style_declaration(&button, Some(&bx), "stretch", &len(2.0, None)).unwrap(),
        Some(StyleLowering::ChildAttr { attr: "stretch".into(), value: StaticValue::Int(2) })
    );
    assert_eq!(
        qt.style_declaration(&button, Some(&bx), "alignment", &ident("left")).unwrap(),
        Some(StyleLowering::ChildAttr { attr: "alignment".into(), value: StaticValue::Enum("left".into()) })
    );
    let e = qt.style_declaration(&button, None, "stretch", &len(2.0, None)).unwrap_err();
    assert_eq!(e.code, codes::STYLE_NOT_APPLICABLE);
    let e = qt.style_declaration(&bx, Some(&el(&qt, "main-window")), "stretch", &len(2.0, None)).unwrap_err();
    assert!(e.message.contains("is in a <main-window>"), "{}", e.message);
}

#[test]
fn coerce_static_attributes() {
    let gtk = load("gtk");
    let bx = el(&gtk, "box");
    let spacing = gtk.property(&bx, "spacing").unwrap();
    assert_eq!(gtk.coerce_static(spacing, Some("8")), Ok(StaticValue::Int(8)));
    assert!(gtk.coerce_static(spacing, Some("-3")).is_err(), "GtkBox:spacing >= 0");
    assert!(gtk.coerce_static(spacing, Some("eight")).is_err());
    assert!(gtk.coerce_static(spacing, None).is_err());
    let o = gtk.property(&bx, "orientation").unwrap();
    assert_eq!(gtk.coerce_static(o, Some("vertical")), Ok(StaticValue::Enum("vertical".into())));
    let e = gtk.coerce_static(o, Some("verticle")).unwrap_err();
    assert!(e.contains("did you mean \"vertical\""), "{e}");
    let sens = gtk.property(&bx, "sensitive").unwrap();
    assert_eq!(gtk.coerce_static(sens, None), Ok(StaticValue::Bool(true)));
    assert_eq!(gtk.coerce_static(sens, Some("false")), Ok(StaticValue::Bool(false)));
    assert!(gtk.coerce_static(sens, Some("yes")).is_err());
    let op = gtk.property(&bx, "opacity").unwrap();
    assert_eq!(gtk.coerce_static(op, Some("0.5")), Ok(StaticValue::Float(0.5)));
    assert!(gtk.coerce_static(op, Some("2")).is_err(), "opacity is 0..1");
    let title = gtk.property(&el(&gtk, "window"), "title").unwrap();
    assert_eq!(gtk.coerce_static(title, Some("Counter")).unwrap().to_json(), serde_json::json!("Counter"));
    let child = gtk.property(&el(&gtk, "window"), "child").unwrap();
    assert!(gtk.coerce_static(child, Some("x")).is_err());
    let hints = gtk.property(&el(&gtk, "entry"), "input-hints").unwrap();
    match gtk.coerce_static(hints, Some("spellcheck | lowercase")).unwrap() {
        StaticValue::Flags { nicks, bits } => {
            assert_eq!(nicks, ["spellcheck", "lowercase"]);
            assert_eq!(bits, Some(hints.bits.as_ref().unwrap()["spellcheck"] | hints.bits.as_ref().unwrap()["lowercase"]));
        }
        other => panic!("{other:?}"),
    }

    let qt = load("qt");
    let qbx = el(&qt, "box-layout");
    assert_eq!(qt.coerce_static(qt.property(&qbx, "spacing").unwrap(), Some("8")), Ok(StaticValue::Int(8)));
    assert!(qt.coerce_static(qt.property(&qbx, "direction").unwrap(), Some("diagonal")).is_err());
    let def = qt.property(&el(&qt, "push-button"), "default").unwrap();
    assert_eq!(qt.coerce_static(def, None), Ok(StaticValue::Bool(true)));
}

#[test]
fn abi_v3_value_types() {
    let gtk = load("gtk");
    let button = el(&gtk, "button");
    let css = gtk.property(&button, "css-classes").unwrap();
    assert_eq!((css.ty, css.of.as_deref(), css.type_ref.as_deref()), (PropType::List, Some("string"), Some("list<string>")));
    let v = gtk.coerce_static(css, Some("pill  flat")).unwrap();
    assert_eq!(v, StaticValue::StrList(vec!["pill".into(), "flat".into()]));
    assert_eq!(v.to_json(), serde_json::json!(["pill", "flat"]));

    let qt = load("qt");
    let geo = qt.property(&el(&qt, "box-layout"), "geometry").unwrap();
    assert_eq!((geo.ty, geo.of.as_deref(), geo.type_ref.as_deref()), (PropType::Record, Some("Rect"), Some("record<Rect>")));
    assert!(qt.coerce_static(geo, Some("0 0 10 10")).unwrap_err().contains("record<Rect>"));
    let pos = qt.property(&el(&qt, "box-layout"), "pos").unwrap();
    assert!(!pos.writable && pos.readable);
    let icon = qt.property(&el(&qt, "push-button"), "icon").unwrap();
    assert_eq!((icon.ty, icon.of.as_deref()), (PropType::Boxed, Some("QIcon")));
    let date = qt.property(&el(&qt, "date-edit"), "date").unwrap();
    assert_eq!((date.ty, date.type_ref.as_deref()), (PropType::Other, Some("date")));
    let align = qt.property(&el(&qt, "line-edit"), "alignment").unwrap();
    assert_eq!(align.ty, PropType::Flags);
    let v = qt.coerce_static(align, Some("left vcenter")).unwrap();
    assert_eq!(v, StaticValue::Flags { nicks: vec!["left".into(), "vcenter".into()], bits: None });
    assert_eq!(v.to_json(), serde_json::json!(["left", "vcenter"]));
    assert!(qt.coerce_static(align, Some("middle")).is_err());
    let echo = qt.property(&el(&qt, "line-edit"), "echo-mode").unwrap();
    assert_eq!(qt.coerce_static(echo, Some("password")), Ok(StaticValue::Enum("password".into())));
}

#[test]
fn child_attrs_per_slot() {
    for b in both() {
        let grid = el(&b, t(&b, "grid", "grid-layout"));
        let col = b.child_attr(&grid, "cells", "column").unwrap();
        assert_eq!((col.spec.ty, col.spec.default.clone()), (AttrType::I32, Some(serde_json::json!(0))), "{}", b.backend());
        assert_eq!(b.child_attr(&grid, "cells", "row-span").unwrap().spec.default, Some(serde_json::json!(1)));
        let e = b.child_attr(&grid, "cells", "colum").unwrap_err();
        assert_eq!(e.code, codes::UNKNOWN_CHILD_ATTR);
        assert_eq!(e.suggestions[0], "parent:column");
        assert!(b.coerce_child_attr(&col, Some("x")).is_err());
        assert_eq!(b.coerce_child_attr(&col, Some("3")).unwrap(), StaticValue::Int(3));
    }
    let gtk = load("gtk");
    assert_eq!(gtk.child_attr(&el(&gtk, "box"), "children", "row").unwrap_err().code, codes::UNKNOWN_CHILD_ATTR);
    let tab = gtk.child_attr(&el(&gtk, "notebook"), "pages", "tab-label").unwrap();
    assert_eq!(tab.spec.ty, AttrType::String);
    let qt = load("qt");
    let align = qt.child_attr(&el(&qt, "box-layout"), "children", "alignment").unwrap();
    assert_eq!(align.spec.ty, AttrType::Enum);
    assert!(align.spec.values.as_ref().unwrap().contains(&"center".to_string()));
    assert_eq!(qt.coerce_child_attr(&align, Some("center")).unwrap(), StaticValue::Enum("center".into()));
}

#[test]
fn sync_and_phases() {
    let gtk = load("gtk");
    let close = gtk.event(&el(&gtk, "window"), "onclose-request").unwrap();
    assert!(close.sync);
    assert_eq!(close.phases, vec!["after"]);
    let clicked = gtk.event(&el(&gtk, "button"), "onclicked").unwrap();
    assert!(!clicked.sync);
    assert_eq!(clicked.phases, vec!["after"]);
    let k = &gtk.spec().kinds["Gtk.EventControllerKey"].signals["key-pressed"];
    assert_eq!(k.phases, vec!["after", "capture"]);
    let qt = load("qt");
    let key = qt.event(&el(&qt, "line-edit"), "onkey-press").unwrap();
    assert!(key.sync);
    assert_eq!(key.phases, vec!["capture"]);
    let close = qt.event(&el(&qt, "main-window"), "onclose").unwrap();
    assert!(close.sync && close.phases.is_empty());
    assert_eq!(qt.event(&el(&qt, "push-button"), "onclicked").unwrap().phases, vec!["after"]);
}

#[test]
fn typed_methods() {
    let gtk = load("gtk");
    assert_eq!(gtk.spec().method_lookup, "typelib");
    let m = gtk.method(&el(&gtk, "scale"), "set_range").unwrap();
    assert_eq!(m.dispatch, "dynamic");
    let params = m.params.as_ref().unwrap();
    assert_eq!(params.iter().map(|p| (p.name.as_str(), p.ty.ty.as_str())).collect::<Vec<_>>(), vec![("min", "f64"), ("max", "f64")]);
    assert_eq!(gtk.method(&el(&gtk, "window"), "present").unwrap().dispatch, "zero_arg");
    let e = gtk.method(&el(&gtk, "window"), "presnt").unwrap_err();
    assert_eq!(e.code, codes::UNKNOWN_METHOD);
    assert!(e.suggestions.contains(&"present".to_string()));
    assert!(e.help.unwrap().contains("GIRepository"));
    let qt = load("qt");
    assert_eq!(qt.spec().method_lookup, "annotated");
    let p = qt.method(&el(&qt, "main-window"), "present").unwrap();
    assert_eq!(p.returns.as_ref().unwrap().ty, "void");
}
