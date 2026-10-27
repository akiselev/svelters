//! `svelters_skelter::dump`: an app on skelter alone regenerates its `.svelters/app` (exports,
//! `kinds.json`, `kinds.d.ts`, behaviour modules and typings) from its `Kinds` and behaviours.

use skelter_base::accesskit::Role;
use skelter_base::change::ChangeClass;
use skelter_core::{self as core, ArgSpec, Kind, KindRegistry, MethodSpec, PropDefault, PropSpec, RecordCx, SignalFlags, SignalSpec, ValueType};
use std::sync::Arc;
use svelters_skelter::behavior::{Behavior, BehaviorCx, Props};
use svelters_skelter::spec::BindSource;
use svelters_skelter::{Kinds, SkelterBackend};

static GAUGE: core::KindSpec = core::KindSpec {
    props: &[
        PropSpec::new("level", ValueType::F64, ChangeClass::Paint).default(PropDefault::F64(0.0)).native().doc("The level (`bind:level`)."),
        PropSpec::new("label", ValueType::Str, ChangeClass::Layout).doc("The caption."),
    ],
    signals: &[SignalSpec::new("changed", Some(ValueType::F64), SignalFlags::NONE).args(&[ArgSpec::new("level", ValueType::F64)]).doc("The user moved the level.")],
    methods: &[MethodSpec::new("reset", &[ArgSpec::new("to", ValueType::F64)], Some(ArgSpec::new("level", ValueType::F64))).doc("Sets the level to `to`.")],
    role: Role::ProgressIndicator,
    doc: "A gauge (the dump test's app kind).",
    ..core::KindSpec::new("app-gauge")
};

#[derive(Default)]
struct Gauge;

impl Kind for Gauge {
    fn spec() -> &'static core::KindSpec {
        &GAUGE
    }
    fn record(&self, _cx: &mut RecordCx) {}
}

struct Nop;

impl Behavior for Nop {
    fn attach(&mut self, _cx: &mut BehaviorCx<'_>, _props: &Props) {}
}

#[test]
fn dump_writes_the_app_kinds_and_behaviours() {
    let kinds = Kinds::with_app(&svelters_skelter::surface::builtin(), |reg: &mut KindRegistry| reg.register::<Gauge>().map(|_| ())).expect("the app kind registers");
    let backend = SkelterBackend::new(Arc::new(kinds));
    backend.register_behavior("cad", "pick", || Box::new(Nop) as Box<dyn Behavior>);
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("svelters-skelter-dump");
    let _ = std::fs::remove_dir_all(&dir);
    let sources = [BindSource::draft("svelters:app/cad", "slot", "draftValue", "setDraft")];
    let report = backend.dump(&dir, &sources).expect("dump");
    for f in ["exports.json", "app.d.ts", "kinds.json", "kinds.d.ts", "behaviors.d.ts", "behaviors/cad.js"] {
        assert!(dir.join(f).is_file(), "{f}");
        assert!(report.written.contains(&dir.join(f)), "{f} written");
    }

    let dts = std::fs::read_to_string(dir.join("kinds.d.ts")).unwrap();
    assert!(dts.contains("'app-gauge': EguiAttrs<EguiWidgetProps & {"), "{dts}");
    assert!(dts.contains("level?: number;") && dts.contains("'bind:level'?: number;"), "prop + binding: {dts}");
    assert!(dts.contains("label?: string | null;"), "a prop with no default is nullable: {dts}");
    assert!(dts.contains("onchanged?: (level: number) => void;"), "event: {dts}");
    assert!(dts.contains("export interface AppGaugeElement extends NativeElement {") && dts.contains("reset(to: number): number;"), "method: {dts}");
    assert!(dts.contains("'app-gauge': AppGaugeElement;"), "bind:this: {dts}");
    assert!(!dts.contains("'button'"), "built-in kinds stay in types/egui: {dts}");

    let text = std::fs::read_to_string(dir.join("kinds.json")).unwrap();
    let merged = svelters_compiler::semantics::SpecBackend::builtin("skelter").unwrap().merge_fragment(&text).expect("the compiler merges kinds.json");
    assert!(merged.spec().tags.contains_key("app-gauge"));
    let frag: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(frag["tags"]["app-gauge"], "AppGauge");
    assert_eq!(frag["kinds"]["AppGauge"]["methods"]["reset"]["returns"]["type"], "f64");
    assert_eq!(frag["behaviors"]["cad.pick"]["module"], "svelters:behaviors/cad");
    assert_eq!(frag["bind_sources"][0]["lowering"], "draft");

    let bdts = std::fs::read_to_string(dir.join("behaviors.d.ts")).unwrap();
    assert!(bdts.contains("declare module 'svelters:behaviors/cad'") && bdts.contains("export const pick: Action<"), "{bdts}");
    assert!(std::fs::read_to_string(dir.join("behaviors/cad.js")).unwrap().contains("export const pick = nativeBehavior('cad.pick', []);"));

    // a second dump rewrites nothing
    let again = backend.dump(&dir, &sources).unwrap();
    assert!(again.written.is_empty(), "{:?}", again.written);
    assert_eq!(again.unchanged, report.written.len());
}
