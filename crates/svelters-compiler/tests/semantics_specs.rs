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
        for tag in ["window", "box", "label", "button", "entry", "scrolledwindow"] {
            let e = el(&b, tag);
            assert!(b.is_a(&e, tag));
        }
    }
    let gtk = load("gtk");
    assert_eq!(el(&gtk, "window").kind, "Gtk.ApplicationWindow");
    assert_eq!(el(&gtk, "headerbar").kind, "Gtk.HeaderBar");
    assert!(gtk.is_a(&el(&gtk, "window"), "Gtk.Window"));
    assert!(gtk.is_a(&el(&gtk, "window"), "GtkWindow"));
    assert!(gtk.is_a(&el(&gtk, "entry"), "Gtk.Editable"), "interfaces are in the chain");
    assert!(!gtk.is_a(&el(&gtk, "label"), "Gtk.Button"));
    let qt = load("qt");
    assert_eq!(el(&qt, "button").kind, "button");
    assert!(qt.is_a(&el(&qt, "button"), "QAbstractButton"));
    assert!(qt.is_a(&el(&qt, "window"), "QMainWindow"));
}

#[test]
fn lifetime_and_present() {
    for b in both() {
        let w = b.lifetime(&el(&b, "window"));
        assert_eq!(w, LifetimePlan { lifetime: Lifetime::Toplevel, present: Some("present".into()) }, "{}", b.backend());
        assert_eq!(b.lifetime(&el(&b, "label")).lifetime, Lifetime::Owned);
        assert_eq!(b.lifetime(&el(&b, "box")).present, None);
    }
}

#[test]
fn unknown_element_on_qt_suggests_and_hints() {
    let qt = load("qt");
    let e = qt.resolve_element("headerbar").unwrap_err();
    assert_eq!(e.code, codes::UNKNOWN_ELEMENT);
    assert!(e.suggestions.contains(&"menubar".to_string()), "{:?}", e.suggestions);
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
    assert_eq!((p.js.as_str(), p.ty), ("placeholder", PropType::String));
    assert_eq!(gtk.property(&entry, "placeholder").unwrap().native, "placeholder-text");
    assert_eq!(gtk.property(&entry, "text").unwrap().owner, "Gtk.Editable");
    let bx = el(&gtk, "box");
    let o = gtk.property(&bx, "orientation").unwrap();
    assert_eq!(o.ty, PropType::Enum);
    assert_eq!(o.values.as_deref().unwrap(), ["horizontal", "vertical"]);
    assert_eq!(o.default, Some(serde_json::json!("horizontal")));
    let css = gtk.property(&el(&gtk, "button"), "css-classes").unwrap();
    assert_eq!((css.js.as_str(), css.ty, css.of.as_deref()), ("cssClasses", PropType::List, Some("string")));
    let e = gtk.property(&el(&gtk, "label"), "lable").unwrap_err();
    assert_eq!(e.code, codes::UNKNOWN_PROPERTY);
    assert!(e.suggestions.contains(&"label".to_string()));
    let ro = gtk.writable_property(&entry, "cursor-position").unwrap_err();
    assert_eq!(ro.code, codes::READ_ONLY_PROPERTY);

    let qt = load("qt");
    let w = el(&qt, "window");
    assert_eq!(qt.property(&w, "windowTitle").unwrap().js, "title");
    assert_eq!(qt.property(&el(&qt, "box"), "orientation").unwrap().default, Some(serde_json::json!("vertical")));
    let e = qt.property(&el(&qt, "label"), "wordWrap").unwrap_err();
    assert!(e.message.contains("only annotated"), "{}", e.message);
}

#[test]
fn text_content() {
    for b in both() {
        assert_eq!(b.text_content(&el(&b, "label")).unwrap().js, "label");
        assert_eq!(b.text_content(&el(&b, "button")).unwrap().js, "label");
        assert!(b.text_content(&el(&b, "box")).is_none());
    }
    let gtk = load("gtk");
    assert_eq!(gtk.text_content(&el(&gtk, "togglebutton")).unwrap().js, "label", "inherited from Gtk.Button");
}

#[test]
fn events_aliases_and_signals() {
    for b in both() {
        let button = el(&b, "button");
        let c = b.event(&button, "onclick").unwrap();
        assert_eq!((c.signal.as_str(), c.via_alias.as_deref()), ("clicked", Some("click")));
        assert_eq!(b.event(&button, "onclicked").unwrap().signal, "clicked");
        let ch = b.event(&el(&b, "entry"), "onchanged").unwrap();
        assert_eq!(ch.args, vec![("text".to_string(), "string".to_string())], "{}", b.backend());
        assert_eq!(b.event(&el(&b, "entry"), "onchange").unwrap().signal, "changed");
        let close = b.event(&el(&b, "window"), "onclose").unwrap();
        assert_eq!(close.signal, "close-request");
        assert!(close.disconnect_on_drop);
        let e = b.event(&button, "onclik").unwrap_err();
        assert_eq!(e.code, codes::UNKNOWN_EVENT);
        assert!(e.suggestions.contains(&"onclick".to_string()) || e.suggestions.contains(&"onclicked".to_string()), "{:?}", e.suggestions);
    }
    let gtk = load("gtk");
    let close = gtk.event(&el(&gtk, "window"), "onclose-request").unwrap();
    assert_eq!((close.owner.as_str(), close.handler_result), ("Gtk.Window", HandlerResult::BoolStop));
    let qt = load("qt");
    let close = qt.event(&el(&qt, "window"), "onclose").unwrap();
    assert_eq!((close.handler_result, close.sync_required), (HandlerResult::BoolStop, true), "synthetic close filter is sync");
    let key = qt.event(&el(&qt, "entry"), "onkey-press").unwrap();
    assert_eq!((key.signal.as_str(), key.sync_required), ("key-press", true));
    assert!(qt.event(&el(&qt, "label"), "onclick").is_err(), "QLabel has no annotated signal");
}

#[test]
fn bind_text_on_entry() {
    for b in both() {
        let plan = b.plan_binding(&el(&b, "entry"), "text").unwrap();
        let sync: Vec<String> = if b.backend() == "gtk" { vec!["changed".into(), "notify::text".into()] } else { vec!["textChanged(QString)".into()] };
        assert_eq!(
            plan,
            BindingPlan {
                prop: "text".into(),
                prop_type: PropType::String,
                signal: "changed".into(),
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
        let k = &b.spec().kinds[&el(&b, "entry").kind];
        let spec = k.bindings.get("text").or_else(|| b.spec().kinds["Gtk.Entry"].bindings.get("text")).unwrap();
        assert!(spec.provenance.contains("signals-and-slots-semantics.md"), "{}", spec.provenance);
        assert_eq!(spec.lifetime["on_native_death"], "end_binding");
    }
}

#[test]
fn no_binding_for_label() {
    let gtk = load("gtk");
    // Gtk.Text is a Gtk.Editable too, but only Gtk.Entry annotates bind:text.
    assert_eq!(gtk.plan_binding(&el(&gtk, "text"), "text").unwrap_err().code, codes::BIND_NO_PLAN);
    let e = gtk.plan_binding(&el(&gtk, "label"), "label").unwrap_err();
    assert_eq!(e.code, codes::BIND_NO_PLAN);
    assert!(e.message.contains("notify::label"), "{}", e.message);
    let qt = load("qt");
    let e = qt.plan_binding(&el(&qt, "label"), "label").unwrap_err();
    assert_eq!(e.code, codes::BIND_NO_PLAN);
    assert!(e.message.contains("QLabel::text has no change notification"), "{}", e.message);
    // Read-only and unknown properties are reported as such.
    assert_eq!(gtk.plan_binding(&el(&gtk, "entry"), "cursorPosition").unwrap_err().code, codes::READ_ONLY_PROPERTY);
    assert_eq!(qt.plan_binding(&el(&qt, "entry"), "nope").unwrap_err().code, codes::UNKNOWN_PROPERTY);
}

#[test]
fn gtk_slots() {
    let gtk = load("gtk");
    let (hb, button, label, win) = (el(&gtk, "headerbar"), el(&gtk, "button"), el(&gtk, "label"), el(&gtk, "window"));
    let p = gtk.place_child(&hb, &button, None).unwrap();
    assert_eq!((p.slot.as_str(), p.is_default, p.routing, p.many, p.order), ("start", true, Routing::Default, true, SlotOrder::AppendOnly));
    let p = gtk.place_child(&hb, &button, Some("end")).unwrap();
    assert_eq!((p.slot.as_str(), p.is_default, p.routing), ("end", false, Routing::Explicit));
    let p = gtk.place_child(&hb, &label, Some("title")).unwrap();
    assert_eq!((p.many, p.order), (false, SlotOrder::None));
    // window (Gtk.ApplicationWindow) inherits Gtk.Window's slots.
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
    let (w, bx) = (el(&qt, "window"), el(&qt, "box"));
    let p = qt.place_child(&w, &bx, None).unwrap();
    assert_eq!((p.slot.as_str(), p.is_default, p.routing), ("central", true, Routing::Default));
    let p = qt.place_child(&w, &el(&qt, "statusbar"), None).unwrap();
    assert_eq!((p.slot.as_str(), p.is_default, p.routing), ("statusbar", false, Routing::ByKind));
    let p = qt.place_child(&w, &el(&qt, "menubar"), None).unwrap();
    assert_eq!((p.slot.as_str(), p.routing), ("menubar", Routing::ByKind));
    let p = qt.place_child(&w, &el(&qt, "menubar"), Some("menubar")).unwrap();
    assert_eq!(p.routing, Routing::Explicit);
    let e = qt.place_child(&w, &el(&qt, "button"), Some("statusbar")).unwrap_err();
    assert_eq!(e.code, codes::SLOT_REJECTS_CHILD);
    assert!(e.message.contains("QStatusBar"), "{}", e.message);
    assert_eq!(e.suggestions, vec!["central"]);
    let e = qt.place_child(&w, &bx, Some("titlebar")).unwrap_err();
    assert_eq!(e.code, codes::UNKNOWN_SLOT);
    assert!(!e.suggestions.is_empty());
    let p = qt.place_child(&bx, &el(&qt, "label"), None).unwrap();
    assert_eq!((p.many, p.order), (true, SlotOrder::NativeIndex));
    let p = qt.place_child(&el(&qt, "scrolledwindow"), &bx, None).unwrap();
    assert_eq!((p.slot.as_str(), p.many), ("child", false));
}

#[test]
fn style_common_declarations() {
    for b in both() {
        let bx = el(&b, "box");
        let set = |prop: &str, value: StaticValue| StyleLowering::SetProp { prop: prop.into(), value };
        assert_eq!(b.style_declaration(&bx, "orientation", &ident("vertical")).unwrap(), set("orientation", StaticValue::Enum("vertical".into())));
        assert_eq!(b.style_declaration(&bx, "spacing", &len(8.0, Some("dp"))).unwrap(), set("spacing", StaticValue::Int(8)), "{}", b.backend());
        assert_eq!(b.style_declaration(&bx, "spacing", &len(0.0, None)).unwrap(), set("spacing", StaticValue::Int(0)));
        let e = b.style_declaration(&bx, "spacing", &len(8.0, Some("px"))).unwrap_err();
        assert_eq!(e.code, codes::STYLE_BAD_VALUE);
        assert_eq!(e.suggestions, vec!["dp"]);
        let e = b.style_declaration(&bx, "spacing", &len(8.0, None)).unwrap_err();
        assert_eq!((e.code, e.suggestions.as_slice()), (codes::STYLE_BAD_VALUE, ["8dp".to_string()].as_slice()));
        let e = b.style_declaration(&bx, "spacing", &len(1.5, Some("dp"))).unwrap_err();
        assert!(e.message.contains("whole pixels"), "{}", e.message);
        let e = b.style_declaration(&bx, "orientation", &ident("vertcal")).unwrap_err();
        assert_eq!((e.code, e.suggestions.as_slice()), (codes::STYLE_BAD_VALUE, ["vertical".to_string()].as_slice()));
        let e = b.style_declaration(&bx, "orientation", &len(1.0, None)).unwrap_err();
        assert_eq!(e.code, codes::STYLE_BAD_VALUE);
        let e = b.style_declaration(&bx, "spacng", &len(1.0, Some("dp"))).unwrap_err();
        assert_eq!((e.code, e.suggestions.as_slice()), (codes::STYLE_UNKNOWN_DECLARATION, ["spacing".to_string()].as_slice()));
        let e = b.style_declaration(&el(&b, "label"), "spacing", &len(1.0, Some("dp"))).unwrap_err();
        assert_eq!(e.code, codes::STYLE_NOT_APPLICABLE);
        let mh = b.style_declaration(&el(&b, "button"), "min-height", &len(32.0, Some("dp"))).unwrap();
        let prop = if b.backend() == "gtk" { "heightRequest" } else { "minimumHeight" };
        assert_eq!(mh, set(prop, StaticValue::Int(32)));
    }
    // Qt: min-height applies to every QWidget, but only button and entry annotate minimumHeight.
    let qt = load("qt");
    let e = qt.style_declaration(&el(&qt, "label"), "min-height", &len(4.0, Some("dp"))).unwrap_err();
    assert_eq!(e.code, codes::STYLE_NOT_APPLICABLE);
    assert!(e.message.contains("minimumHeight"), "{}", e.message);
    // GTK-only common declarations.
    let gtk = load("gtk");
    assert_eq!(
        gtk.style_declaration(&el(&gtk, "entry"), "enabled", &ident("false")).unwrap(),
        StyleLowering::SetProp { prop: "sensitive".into(), value: StaticValue::Bool(false) }
    );
    assert_eq!(gtk.style_declaration(&el(&gtk, "entry"), "enabled", &ident("no")).unwrap_err().code, codes::STYLE_BAD_VALUE);
    assert_eq!(qt.style_declaration(&el(&qt, "entry"), "enabled", &ident("false")).unwrap_err().code, codes::STYLE_UNKNOWN_DECLARATION);
    // Range: width-request >= -1.
    assert_eq!(gtk.style_declaration(&el(&gtk, "box"), "min-width", &len(-5.0, Some("dp"))).unwrap_err().code, codes::STYLE_BAD_VALUE);
}

#[test]
fn style_emphasis_intents() {
    let gtk = load("gtk");
    let qt = load("qt");
    let intent = |intent: &str, actions| StyleLowering::Intent { declaration: "emphasis".into(), intent: intent.into(), actions };
    assert_eq!(
        gtk.style_declaration(&el(&gtk, "button"), "emphasis", &ident("accent")).unwrap(),
        intent("accent", vec![StyleLowering::AddToken { prop: "cssClasses".into(), token: "suggested-action".into() }])
    );
    assert_eq!(
        gtk.style_declaration(&el(&gtk, "button"), "emphasis", &ident("destructive")).unwrap(),
        intent("destructive", vec![StyleLowering::AddToken { prop: "cssClasses".into(), token: "destructive-action".into() }])
    );
    assert_eq!(gtk.style_declaration(&el(&gtk, "button"), "emphasis", &ident("normal")).unwrap(), intent("normal", vec![]));
    assert_eq!(
        qt.style_declaration(&el(&qt, "button"), "emphasis", &ident("accent")).unwrap(),
        intent("accent", vec![StyleLowering::SetProp { prop: "default".into(), value: StaticValue::Bool(true) }])
    );
    let e = qt.style_declaration(&el(&qt, "button"), "emphasis", &ident("destructive")).unwrap_err();
    assert_eq!(e.code, codes::STYLE_INTENT_UNSUPPORTED);
    assert!(e.message.contains("no native equivalent on qt"), "{}", e.message);
    assert_eq!(e.suggestions, vec!["accent", "normal"]);
    let e = gtk.style_declaration(&el(&gtk, "button"), "emphasis", &ident("acent")).unwrap_err();
    assert_eq!((e.code, e.suggestions.as_slice()), (codes::STYLE_BAD_VALUE, ["accent".to_string()].as_slice()));
    for b in [&gtk, &qt] {
        assert_eq!(b.style_declaration(&el(b, "label"), "emphasis", &ident("accent")).unwrap_err().code, codes::STYLE_NOT_APPLICABLE);
    }
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
    let qbx = el(&qt, "box");
    assert_eq!(qt.coerce_static(qt.property(&qbx, "spacing").unwrap(), Some("8")), Ok(StaticValue::Int(8)));
    assert!(qt.coerce_static(qt.property(&qbx, "orientation").unwrap(), Some("diagonal")).is_err());
    let def = qt.property(&el(&qt, "button"), "default").unwrap();
    assert_eq!(qt.coerce_static(def, None), Ok(StaticValue::Bool(true)));
}

#[test]
fn abi_v3_value_types() {
    let gtk = load("gtk");
    let button = el(&gtk, "button");
    let css = gtk.property(&button, "cssClasses").unwrap();
    assert_eq!((css.ty, css.of.as_deref(), css.type_ref.as_deref()), (PropType::List, Some("string"), Some("list<string>")));
    let v = gtk.coerce_static(css, Some("pill  flat")).unwrap();
    assert_eq!(v, StaticValue::StrList(vec!["pill".into(), "flat".into()]));
    assert_eq!(v.to_json(), serde_json::json!(["pill", "flat"]));
    // AddToken still targets the list prop.
    match gtk.style_declaration(&button, "emphasis", &ident("accent")).unwrap() {
        StyleLowering::Intent { actions, .. } => {
            assert_eq!(actions, vec![StyleLowering::AddToken { prop: "cssClasses".into(), token: "suggested-action".into() }])
        }
        other => panic!("{other:?}"),
    }

    let qt = load("qt");
    let geo = qt.property(&el(&qt, "box"), "geometry").unwrap();
    assert_eq!((geo.ty, geo.of.as_deref(), geo.type_ref.as_deref()), (PropType::Record, Some("Rect"), Some("record<Rect>")));
    assert!(qt.coerce_static(geo, Some("0 0 10 10")).unwrap_err().contains("record<Rect>"));
    let pos = qt.property(&el(&qt, "box"), "pos").unwrap();
    assert!(!pos.writable && pos.readable);
    let icon = qt.property(&el(&qt, "button"), "icon").unwrap();
    assert_eq!((icon.ty, icon.of.as_deref()), (PropType::Boxed, Some("QIcon")));
    let date = qt.property(&el(&qt, "dateedit"), "date").unwrap();
    assert_eq!((date.ty, date.type_ref.as_deref()), (PropType::Other, Some("date")));
    let align = qt.property(&el(&qt, "entry"), "alignment").unwrap();
    assert_eq!(align.ty, PropType::Flags);
    let v = qt.coerce_static(align, Some("left vcenter")).unwrap();
    assert_eq!(v, StaticValue::Flags { nicks: vec!["left".into(), "vcenter".into()], bits: None });
    assert_eq!(v.to_json(), serde_json::json!(["left", "vcenter"]));
    assert!(qt.coerce_static(align, Some("middle")).is_err());
    let echo = qt.property(&el(&qt, "entry"), "echoMode").unwrap();
    assert_eq!(qt.coerce_static(echo, Some("password")), Ok(StaticValue::Enum("password".into())));
}
