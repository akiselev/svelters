//! A1: an app's own core kinds (cadatomic's `<property-grid>`, `<quantity-field>`, `<empty-state>`,
//! `<status-text>`, `<code-editor>`; stand-ins here, the real ones live in cadatomic) registered with
//! `Kinds::with_app`: creatable from JS through the backend with the props, methods and signals of
//! their core `KindSpec`, published in the generated spec, and merged into the compiler's
//! `skelter.json` as a fragment.

mod common;

use common::Rig;
use skelter_base::accesskit::Role;
use skelter_base::change::ChangeClass;
use skelter_core::{self as core, ArgSpec, EventCx, Kind, KindRegistry, MethodId, MethodSpec, PropDefault, PropId, PropSpec, RecordCx, SignalFlags, SignalId, SignalSpec, SlotSpec, Value as CValue, ValueType};
use std::sync::Arc;
use svelters_abi::*;
use svelters_skelter::kinds::PropTarget;
use svelters_skelter::{Kinds, SkelterBackend, spec};

const QTY_VALUE: PropId = PropId::of("quantity-field", "value", "number");
const QTY_CHANGED: SignalId = SignalId::of("quantity-field", "changed", "number");
const QTY_STEP: MethodId = MethodId::of("quantity-field", "step", "number");

static QUANTITY_FIELD: core::KindSpec = core::KindSpec {
    props: &[
        PropSpec::new("value", ValueType::F64, ChangeClass::Paint).default(PropDefault::F64(0.0)).native().doc("The quantity in its unit (`bind:value`)."),
        PropSpec::new("unit", ValueType::Str, ChangeClass::Layout).doc("The unit shown after the number (`mm`)."),
    ],
    signals: &[SignalSpec::new("changed", Some(ValueType::F64), SignalFlags::NONE).args(&[ArgSpec::new("value", ValueType::F64)]).doc("The user changed the value.")],
    methods: &[MethodSpec::new("step", &[ArgSpec::new("by", ValueType::F64)], Some(ArgSpec::new("value", ValueType::F64))).doc("Adds `by`; returns the new value.")],
    role: Role::SpinButton,
    focusable: true,
    doc: "A number with a unit (stand-in for cadatomic's).",
    ..core::KindSpec::new("quantity-field")
};

#[derive(Default)]
struct QuantityField;

impl Kind for QuantityField {
    fn spec() -> &'static core::KindSpec {
        &QUANTITY_FIELD
    }
    fn record(&self, _cx: &mut RecordCx) {}
    fn call(&mut self, cx: &mut EventCx, method: MethodId, args: &[CValue]) -> Result<Option<CValue>, core::PropError> {
        if method != QTY_STEP {
            return Err(core::PropError::Rejected(format!("{method:?}")));
        }
        let by = args.first().and_then(|v| v.as_f64()).unwrap_or(0.0);
        let v = cx.view(cx.node()).and_then(|n| n.prop_f64(QTY_VALUE)).unwrap_or(0.0) + by;
        cx.set_native(QTY_VALUE, CValue::F64(v))?;
        cx.emit(QTY_CHANGED, Some(CValue::F64(v)));
        Ok(Some(CValue::F64(v)))
    }
}

static PROPERTY_GRID: core::KindSpec = core::KindSpec {
    props: &[PropSpec::new("filter", ValueType::Str, ChangeClass::Layout).default(PropDefault::Str(""))],
    slots: &[SlotSpec { name: "header", single: true }],
    container: true,
    ..core::KindSpec::new("property-grid")
};

static EMPTY_STATE: core::KindSpec = core::KindSpec {
    props: &[PropSpec::new("message", ValueType::Str, ChangeClass::Layout).default(PropDefault::Str(""))],
    text_prop: Some("message"),
    ..core::KindSpec::new("empty-state")
};

static STATUS_TEXT: core::KindSpec = core::KindSpec {
    props: &[PropSpec::new("tone", ValueType::Enum, ChangeClass::Paint).default(PropDefault::Enum(0)).enum_values(&["info", "warning", "error"])],
    role: Role::Status,
    ..core::KindSpec::new("status-text")
};

static CODE_EDITOR: core::KindSpec = core::KindSpec {
    props: &[PropSpec::new("text", ValueType::Str, ChangeClass::Layout).default(PropDefault::Str("")).native()],
    signals: &[SignalSpec::new("changed", Some(ValueType::Str), SignalFlags::NONE).args(&[ArgSpec::new("text", ValueType::Str)])],
    role: Role::MultilineTextInput,
    focusable: true,
    ..core::KindSpec::new("code-editor")
};

/// A kind with no behaviour of its own.
#[derive(Default)]
struct Plain;

impl Kind for Plain {
    fn spec() -> &'static core::KindSpec {
        &EMPTY_STATE
    }
    fn record(&self, _cx: &mut RecordCx) {}
}

fn plain() -> Box<dyn Kind> {
    Box::new(Plain)
}

fn app_kinds() -> Arc<Kinds> {
    let kinds = Kinds::with_app(svelters_skelter::surface::tier0::ALL, |reg: &mut KindRegistry| {
        reg.register::<QuantityField>()?;
        for s in [&PROPERTY_GRID, &EMPTY_STATE, &STATUS_TEXT, &CODE_EDITOR] {
            reg.register_with(s, plain)?;
        }
        Ok(())
    });
    Arc::new(kinds.expect("the app's kinds register"))
}

#[test]
fn app_kinds_map_from_their_core_specs() {
    let kinds = app_kinds();
    let q = kinds.by_tag("quantity-field").expect("derived surface kind");
    assert!(q.app && q.hosted);
    assert_eq!(q.surface.name, "QuantityField");
    assert_eq!(q.prop_named("value").unwrap().target, PropTarget::Native(QTY_VALUE));
    assert_eq!(q.prop_named("value").unwrap().wire, QTY_VALUE.0, "own members keep their core ids");
    assert_eq!(q.signal_named("changed").unwrap().native, Some(QTY_CHANGED));
    assert!(q.prop_named("unit").unwrap().spec.nullable, "an unset default reads as null");
    // inherited `Widget` props reach core's common props
    assert_eq!(q.prop_named("enabled").unwrap().target, PropTarget::Inverted(core::common::DISABLED));
    let grid = kinds.by_tag("property-grid").unwrap();
    assert_eq!(grid.slots.iter().map(|s| s.spec.name).collect::<Vec<_>>(), ["children", "header"]);
    assert_eq!(grid.slot(grid.slots[1].wire), Some(PROPERTY_GRID.slot_named("header")));
    for t in ["empty-state", "status-text", "code-editor"] {
        assert!(kinds.by_tag(t).is_some_and(|m| m.app), "{t}");
    }
    // the built-ins are unchanged
    assert!(!kinds.by_tag("button").unwrap().app);
    assert!(!svelters_skelter::builtin_kinds().iter().any(|m| m.app));
}

#[test]
fn an_app_kind_is_created_and_driven_through_the_backend() {
    let r = Rig::with_backend(SkelterBackend::new(app_kinds()));
    let q = r.create("quantity-field");
    let bx = r.create("property-grid");
    r.insert(bx, q);
    // props from the KindSpec
    r.set(q, "unit", Value::Str("mm".into()));
    assert_eq!(r.get(q, "unit"), Value::Str("mm".into()));
    assert_eq!(r.get(q, "value"), Value::F64(0.0));
    r.set(r.create("status-text"), "tone", Value::Str("warning".into()));
    // a method from the KindSpec, emitting a signal from the KindSpec
    r.listen(q, "changed");
    let m = r.br().resolve(NameKind::Method, r.kind(q), "step");
    assert_ne!(m, 0);
    assert_eq!(r.br().call(q, m, &[Value::F64(2.5)]).unwrap(), Value::F64(2.5));
    r.backend.pump();
    let l = r.take();
    assert_eq!(l.events, [(q, r.signal_id(q, "changed"), vec![Value::F64(2.5)])]);
    assert_eq!(r.get(q, "value"), Value::F64(2.5));
}

#[test]
fn app_kinds_appear_in_the_spec_and_merge_into_the_compiler() {
    use svelters_compiler::check::{FileState, check};
    use svelters_compiler::semantics::SpecBackend;
    let kinds = app_kinds();
    let full = spec::semantics(&kinds);
    let q = &full["kinds"]["QuantityField"];
    assert_eq!(full["tags"]["quantity-field"], "QuantityField");
    assert_eq!(q["widget"], "app");
    assert_eq!(q["props"]["value"]["id"].as_u64(), Some(QTY_VALUE.0 as u64));
    assert_eq!(q["methods"]["step"]["id"].as_u64(), Some(QTY_STEP.0 as u64));
    assert_eq!(q["bindings"]["value"]["event"], "changed");
    assert_eq!(q["a11y"]["role"], "SpinButton");
    // the fragment holds the app's kinds alone
    let frag = spec::fragment_text(&kinds);
    let f: serde_json::Value = serde_json::from_str(&frag).unwrap();
    let mut tags: Vec<&str> = f["tags"].as_object().unwrap().keys().map(String::as_str).collect();
    tags.sort();
    assert_eq!(tags, ["code-editor", "empty-state", "property-grid", "quantity-field", "status-text"]);
    // merged into the built-in skelter.json, the compiler knows them
    let src = "<property-grid><quantity-field unit=\"mm\" onchanged={() => {}} /></property-grid>\n<empty-state message=\"none\" />\n<status-text tone=\"error\" />\n<code-editor text=\"\" />\n";
    let errors = |b: &SpecBackend| {
        let r = check("App.svelte", src, b, &mut FileState::new());
        r.diagnostics.iter().filter(|d| d.code.starts_with("unknown_")).map(|d| d.message.clone()).collect::<Vec<_>>()
    };
    let builtin = SpecBackend::builtin("skelter").unwrap();
    assert!(!errors(&builtin).is_empty(), "the built-in spec does not know the app's kinds");
    let with_app = builtin.merge_fragment(&frag).unwrap();
    assert_eq!(errors(&with_app), Vec::<String>::new());
}
