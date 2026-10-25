//! What svelters-presto reads from the registry and the spec: wire id uniqueness, methods with typed
//! parameters, named signal arguments, nullable props, accessible name sources, and JS-created windows.

use super::*;
use crate::cx::{EventCx, RecordCx};
use crate::error::CallError;
use crate::kind::{Handled, Kind, KindSpec, MethodSpec, NameFrom, ParamSpec, PropSpec, SignalFlags, SignalSpec};
use crate::registry::{MemberKind, RegistryError};
use crate::value::ValueType;
use crate::{KindRegistry, ListenFlags, MethodId, SignalId, Value, common};
use presto_base::change::ChangeClass;
use std::sync::Arc;

static PANEL: KindSpec = KindSpec {
    doc: "A panel with a caption.",
    props: &[
        PropSpec::new("caption", ValueType::Str, ChangeClass::Layout).nullable().doc("The caption."),
        PropSpec::new("level", ValueType::I64, ChangeClass::Paint),
    ],
    signals: &[
        SignalSpec::new("resized", None, SignalFlags::NONE).args(&[ParamSpec::new("width", ValueType::F64), ParamSpec::new("height", ValueType::F64)]),
        SignalSpec::new("picked", None, SignalFlags::NONE).args(&[ParamSpec::new("index", ValueType::I64)]),
    ],
    methods: &[
        MethodSpec::new("grow").params(&[ParamSpec::new("by", ValueType::I64), ParamSpec::new("why", ValueType::Str).nullable()]).returns(ValueType::I64, false),
    ],
    name_from: &[NameFrom::Prop("caption"), NameFrom::Contents],
    name_required: true,
    container: true,
    role: accesskit::Role::Group,
    ..KindSpec::new("panel")
};

const PANEL_ID: crate::KindId = crate::KindId::of("panel");
const RESIZED: SignalId = SignalId::of("panel", "resized", "record");
const PICKED: SignalId = SignalId::of("panel", "picked", "int");

fn grow() -> MethodId {
    PANEL.method_named("grow").unwrap()
}

#[derive(Default)]
struct Panel {
    level: i64,
}

impl Kind for Panel {
    fn spec() -> &'static KindSpec {
        &PANEL
    }
    fn record(&self, cx: &mut RecordCx) {
        cx.draw_box();
    }
    fn call(&mut self, cx: &mut EventCx, method: MethodId, args: &[Option<Value>]) -> Result<Option<Value>, CallError> {
        if method != grow() {
            return Err(CallError::Unknown(method));
        }
        self.level += args[0].as_ref().and_then(Value::as_i64).unwrap_or(0);
        cx.emit_args(RESIZED, vec![Value::F64(1.0), Value::F64(2.0)]);
        cx.emit_args(PICKED, vec![Value::I64(self.level)]);
        Ok(Some(Value::I64(self.level)))
    }
    fn event(&mut self, _cx: &mut EventCx, _ev: &crate::Event) -> Handled {
        Handled::No
    }
}

fn pdoc() -> (Document, NodeId) {
    let mut r = KindRegistry::new();
    r.register::<testkinds::BoxKind>().unwrap();
    r.register::<testkinds::TextKind>().unwrap();
    r.register::<Panel>().unwrap();
    let mut d = Document::new(Arc::new(r), Box::new(FixedText));
    let root = d.create_window(W, BOX, Size::new(200.0, 100.0), Scale(1.0));
    (d, root)
}

#[test]
fn spec_fields_reach_the_registry() {
    let (d, _) = pdoc();
    let e = d.registry().by_name("panel").unwrap();
    assert_eq!(e.spec.doc, "A panel with a caption.");
    assert!(e.prop(PANEL.prop_named("caption").unwrap()).unwrap().nullable);
    let s = e.signal(RESIZED).expect("a two-argument signal hashes as a record");
    assert_eq!(s.arg, Some(ValueType::Record));
    assert_eq!(s.args.iter().map(|a| a.name).collect::<Vec<_>>(), ["width", "height"]);
    assert_eq!(e.signal(PICKED).unwrap().arg, Some(ValueType::I64));
    let m = e.method(grow()).unwrap();
    assert_eq!(m.signature(), "(int,string?)->int");
    assert_eq!(grow(), MethodId::of("panel", "grow", "(int,string?)->int"));
    assert_eq!(e.method_ids(), [grow()]);
}

#[test]
fn every_wire_id_is_known_and_unique() {
    let (d, _) = pdoc();
    let reg = d.registry();
    let m = reg.member(grow().0).unwrap();
    assert_eq!((m.what, m.kind, m.name), (MemberKind::Method, "panel", "grow"));
    assert_eq!(reg.member(PANEL_ID.0).unwrap().what, MemberKind::Kind);
    assert_eq!(reg.member(common::CLICK.0).unwrap().kind, "*");
    assert_eq!(reg.member(common::FOCUS_METHOD.0).unwrap().what, MemberKind::Method);
    // order independent
    let mut r2 = KindRegistry::new();
    r2.register::<Panel>().unwrap();
    r2.register::<testkinds::TextKind>().unwrap();
    r2.register::<testkinds::BoxKind>().unwrap();
    assert_eq!(r2.spec_hash(), reg.spec_hash());
    let mut r3 = KindRegistry::new();
    r3.register::<Panel>().unwrap();
    assert_ne!(r3.spec_hash(), reg.spec_hash());
}

/// A kind whose name hashes like another registered member's id: forced by registering a spec whose
/// kind id equals one already taken (the same name under a different spec is a duplicate kind; a
/// real FNV collision needs a found pair, so the collision path is driven through `member` ids).
#[test]
fn collisions_are_rejected_with_both_names() {
    static A: KindSpec = KindSpec { props: &[PropSpec::new("x", ValueType::Bool, ChangeClass::Paint)], ..KindSpec::new("ka") };
    static DUP: KindSpec = KindSpec { ..KindSpec::new("ka") };
    let mut r = KindRegistry::new();
    r.register_with(&A, || Box::new(testkinds::BoxKind)).unwrap();
    let e = r.register_with(&DUP, || Box::new(testkinds::BoxKind)).unwrap_err();
    assert!(matches!(e, RegistryError::DuplicateKind { .. }));
    // a member that shadows a common one (same name and type as `*`'s would still differ by kind
    // name, so a collision within a kind is the reachable case)
    static TWICE: KindSpec = KindSpec {
        props: &[PropSpec::new("y", ValueType::Bool, ChangeClass::Paint), PropSpec::new("y", ValueType::Bool, ChangeClass::Paint)],
        ..KindSpec::new("kb")
    };
    let e = r.register_with(&TWICE, || Box::new(testkinds::BoxKind)).unwrap_err();
    assert!(matches!(e, RegistryError::MemberCollision { .. }));
    assert!(e.to_string().contains("`y`"));
    // nothing of a rejected kind stays registered
    assert!(r.by_name("kb").is_none());
    assert!(r.member(PropId::of("kb", "y", "bool").0).is_none());
    // the cross-kind error names both owners
    let err = RegistryError::IdCollision {
        id: 7,
        existing: crate::Member { what: MemberKind::Prop, kind: "ka", name: "x" },
        new: crate::Member { what: MemberKind::Signal, kind: "kc", name: "z" },
    };
    let msg = err.to_string();
    assert!(msg.contains("prop `x` of kind `ka`") && msg.contains("signal `z` of kind `kc`"), "{msg}");
    // unknown accessible-name props are caught
    static BADNAME: KindSpec = KindSpec { name_from: &[NameFrom::Prop("nope")], ..KindSpec::new("kd") };
    assert!(matches!(r.register_with(&BADNAME, || Box::new(testkinds::BoxKind)), Err(RegistryError::UnknownNameProp { .. })));
}

use crate::PropId;

#[test]
fn methods_check_arguments_and_queue_signals() {
    let (mut d, root) = pdoc();
    let p = child(&mut d, root, PANEL_ID, "");
    d.listen(p, RESIZED, ListenFlags::default());
    d.listen(p, PICKED, ListenFlags::default());
    assert_eq!(d.call(p, grow(), &[Some(Value::I64(2))]), Ok(Some(Value::I64(2))));
    assert_eq!(d.call(p, grow(), &[Some(Value::F64(3.0)), Some(Value::from("x"))]), Ok(Some(Value::I64(5))));
    assert_eq!(d.call(p, grow(), &[]), Err(CallError::BadArg { method: grow(), index: 0 }));
    assert_eq!(d.call(p, grow(), &[Some(Value::from("x"))]), Err(CallError::BadArg { method: grow(), index: 0 }));
    assert_eq!(d.call(p, grow(), &[Some(Value::I64(1)), None, None]), Err(CallError::BadArg { method: grow(), index: 2 }));
    let unknown = MethodId::of("panel", "nope", "()->void");
    assert_eq!(d.call(p, unknown, &[]), Err(CallError::Unknown(unknown)));
    let ev = d.take_events();
    let resized: Vec<_> = ev.iter().filter(|e| e.signal == RESIZED).collect();
    assert_eq!(resized.len(), 2);
    let arg = resized[0].arg.as_ref().unwrap();
    assert_eq!(arg.field("width"), Some(&Value::F64(1.0)));
    assert_eq!(arg.field("height"), Some(&Value::F64(2.0)));
    assert_eq!(ev.iter().filter(|e| e.signal == PICKED).last().unwrap().arg, Some(Value::I64(5)));
}

#[test]
fn common_methods() {
    let (mut d, root) = pdoc();
    let t = child(&mut d, root, BOX, "width: 30px; height: 10px; margin-top: 5px");
    d.set_prop(t, common::TABINDEX, Value::I64(0)).unwrap();
    frame(&mut d);
    assert_eq!(d.call(t, common::FOCUS_METHOD, &[]), Ok(None));
    assert_eq!(d.focused(W), Some(t));
    d.call(root, common::BLUR_METHOD, &[]).unwrap();
    assert_eq!(d.focused(W), None, "blur on an ancestor of the focused node clears it");
    let b = d.call(t, common::BOUNDS, &[]).unwrap().unwrap();
    assert_eq!(b.field("y"), Some(&Value::F64(5.0)));
    assert_eq!(b.field("width"), Some(&Value::F64(30.0)));
    let loose = d.create(BOX);
    assert_eq!(d.call(loose, common::BOUNDS, &[]), Ok(None));
    assert_eq!(d.call(t, common::SCROLL_INTO_VIEW, &[]), Ok(None));
}

#[test]
fn names_follow_the_sources() {
    let (mut d, root) = pdoc();
    let p = child(&mut d, root, PANEL_ID, "");
    let t = child(&mut d, p, TEXT, "");
    d.set_prop(t, testkinds::TEXT, Value::from("Inner")).unwrap();
    let label = |d: &mut Document| {
        let u = frame(d).unwrap().a11y.unwrap();
        u.nodes.iter().find(|(id, _)| id.0 == p.0).map(|(_, n)| n.label().map(str::to_owned)).unwrap()
    };
    assert_eq!(label(&mut d).as_deref(), Some("Inner"), "contents when the caption is unset");
    d.set_prop(p, PANEL.prop_named("caption").unwrap(), Value::from("Caption")).unwrap();
    assert_eq!(label(&mut d).as_deref(), Some("Caption"));
    d.set_prop(p, common::ARIA_LABEL, Value::from("Aria")).unwrap();
    assert_eq!(label(&mut d).as_deref(), Some("Aria"));
}

#[test]
fn js_created_windows_attach_and_detach() {
    let (mut d, root) = pdoc();
    let w2 = presto_base::ids::WindowId(2);
    let win = d.create(BOX);
    let inner = d.create(BOX);
    d.insert(win, inner, None, None).unwrap();
    // a child cannot be a root
    assert_eq!(d.attach_window(inner, w2, Size::new(50.0, 50.0), Scale(1.0)), Err(crate::TreeError::HasParent(inner)));
    d.attach_window(win, w2, Size::new(50.0, 50.0), Scale(1.0)).unwrap();
    assert_eq!(d.window_root(w2), Some(win));
    assert_eq!(d.window_of(inner), Some(w2));
    assert!(d.frame(w2, Time(0)).is_some(), "the window presents");
    assert_eq!(d.insert(root, win, None, None), Err(crate::TreeError::IsRoot(win)));
    // detach keeps the nodes alive
    assert_eq!(d.detach_window(w2), Some(win));
    assert!(d.contains(win) && d.contains(inner));
    assert_eq!(d.window_of(inner), None);
    assert_eq!(d.window_root(w2), None);
    // and it can be inserted elsewhere now
    d.insert(root, win, None, None).unwrap();
    assert_eq!(d.window_of(inner), Some(W));
}
