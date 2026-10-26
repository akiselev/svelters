//! cadatomic's panels-ui requests (FQ-74, FQ-75): an app kind's record props and method arguments
//! reach the kind with their shape from compiled Svelte through the real svelters-host, and a
//! kind method's coded error reaches JS with `.code`.

use skelter_base::change::ChangeClass;
use skelter_core::{self as core, ArgSpec, EventCx, Kind, KindId, KindRegistry, MethodId, MethodSpec, PropId, PropSpec, RecordCx, Value as CValue, ValueType};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use svelters_skelter::embed::{Embed, EmbedConfig, EmbedEvent, MountSpec};
use svelters_skelter::{Kinds, SkelterBackend, spec};

const RANGE: PropId = PropId::of("probe", "range", "record");
const LAST: PropId = PropId::of("probe", "last", "string");

static PROBE: core::KindSpec = core::KindSpec {
    props: &[
        PropSpec::new("range", ValueType::Record, ChangeClass::Paint).doc("A selection range: `{start, end: {line, tags}}`."),
        PropSpec::new("last", ValueType::Str, ChangeClass::Paint).doc("What the last call got."),
    ],
    methods: &[
        MethodSpec::new("echo", &[ArgSpec::new("at", ValueType::Record), ArgSpec::new("ids", ValueType::List), ArgSpec::new("note", ValueType::Str).nullable()], Some(ArgSpec::new("value", ValueType::Record))).doc("Returns its arguments as one record."),
        MethodSpec::new("fail", &[], None).doc("Refuses with the code `E_RANGE`."),
    ],
    ..core::KindSpec::new("probe")
};

/// Stands in for cadatomic's code editor: takes a record prop, echoes record and list arguments.
#[derive(Default)]
struct Probe;

impl Kind for Probe {
    fn spec() -> &'static core::KindSpec {
        &PROBE
    }
    fn record(&self, _cx: &mut RecordCx) {}
    fn call(&mut self, _cx: &mut EventCx, method: MethodId, args: &[CValue]) -> Result<Option<CValue>, core::PropError> {
        if method == MethodId::of("probe", "fail", "void") {
            return Err(core::PropError::coded("E_RANGE", "line 99 is past the end"));
        }
        assert_eq!(method, MethodId::of("probe", "echo", "record"));
        let rec = vec![(Arc::from("at"), args[0].clone()), (Arc::from("ids"), args[1].clone()), (Arc::from("n"), CValue::I64(args.len() as i64))];
        Ok(Some(CValue::Record(rec.into())))
    }
}

fn kinds() -> Arc<Kinds> {
    Arc::new(Kinds::with_app(svelters_skelter::surface::tier0::ALL, |reg: &mut KindRegistry| reg.register::<Probe>().map(|_| ())).unwrap())
}

/// A scratch app with the app kinds' spec fragment (as cadatomic's build passes it).
fn scratch(name: &str, kinds: &Kinds, components: &[(&str, &str)]) -> PathBuf {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/embed-egui").canonicalize().unwrap();
    assert!(fixture.join("node_modules").exists(), "run `pnpm install` first");
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/svelters-skelter-host").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    let embed: Vec<String> = components.iter().map(|(f, _)| format!("\"{}\": \"src/{f}\"", f.trim_end_matches(".svelte"))).collect();
    std::fs::write(dir.join("package.json"), format!(r#"{{ "name": "app-values", "private": true, "type": "module", "svelters": {{ "toolkit": "skelter", "embed": {{ {} }} }} }}"#, embed.join(", "))).unwrap();
    for (f, src) in components {
        std::fs::write(dir.join("src").join(f), src).unwrap();
    }
    std::fs::write(dir.join("app.skelter.json"), spec::fragment_text(kinds)).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(fixture.join("node_modules"), dir.join("node_modules")).unwrap();
    dir.canonicalize().unwrap()
}

/// Mounts `component` of `src` into a box of a fresh backend on `kinds`; returns the backend, the
/// box and the errors JS reported.
fn mount(name: &str, component: &'static str, src: &str) -> (SkelterBackend, skelter_base::ids::NodeId, Embed, Vec<EmbedEvent>) {
    let kinds = kinds();
    let backend = SkelterBackend::new(kinds.clone());
    let pane = backend.document().borrow_mut().create(KindId::of("box"));
    let host = svelters_host::HostConfig { echo_console: false, echo_errors: true, quit_on_last_window: false, ..svelters_host::HostConfig::default() };
    let embed = Embed::with_backend(backend.clone(), EmbedConfig { host, ..EmbedConfig::default() }).unwrap();
    let dir = scratch(name, &kinds, &[(&format!("{component}.svelte"), src)]);
    let mut cfg = svelters_host::modules::SourceConfig::for_path(&dir).unwrap();
    cfg.backend = "skelter".into();
    cfg.spec_fragments = vec![dir.join("app.skelter.json")];
    let entry = embed.load_from(Rc::new(svelters_host::modules::SourceProvider::new(cfg).unwrap())).unwrap();
    let slot = embed.adopt_slot("panel", pane).unwrap();
    embed.mount(MountSpec::new(entry, component, slot, "panel")).unwrap();
    embed.logic();
    let errors = embed.take_events().into_iter().filter(|e| matches!(e, EmbedEvent::Error { .. } | EmbedEvent::Faulted { .. })).collect();
    (backend, pane, embed, errors)
}

const RECORD_PROP: &str = r#"<script>
  let line = $state(2);
</script>
<probe range={{ start: 1, end: { line, tags: ['a', 'b'] } }} />
"#;

/// FQ-74: a record prop set from Svelte reaches the kind as a record, nested fields included.
#[test]
fn a_record_prop_reaches_the_kind_with_its_shape() {
    let (backend, pane, _embed, errors) = mount("record-prop", "Range", RECORD_PROP);
    assert!(errors.is_empty(), "{errors:?}");
    let doc = backend.document();
    let d = doc.borrow();
    let probe = d.children(pane)[0];
    let r = d.get_prop(probe, RANGE).expect("set");
    assert_eq!(r.field("start"), Some(&CValue::I64(1)));
    let end = r.field("end").expect("a nested record");
    assert_eq!(end.field("line"), Some(&CValue::I64(2)));
    assert_eq!(end.field("tags"), Some(&CValue::List(vec![CValue::from("a"), CValue::from("b")].into())));
}

const METHODS: &str = r#"<script>
  let el;
  let out = $state('');
  $effect(() => {
    const r = el.call('echo', { line: 3, col: { x: 1 } }, [1, 2]);
    let caught = 'none';
    try {
      el.call('fail');
    } catch (e) {
      caught = `${e.code}|${e.message}|${e instanceof Error}`;
    }
    out = JSON.stringify(r) + ' ' + caught;
  });
</script>
<probe bind:this={el} last={out} />
"#;

/// FQ-75: an app kind's method takes record and list arguments (described from its core
/// `MethodSpec`; a nullable one may be left out) and returns a record; a coded refusal
/// (`PropError::Coded`) is a JS error with `.code`.
#[test]
fn methods_take_records_and_raise_coded_errors() {
    let (backend, pane, _embed, errors) = mount("methods", "Methods", METHODS);
    assert!(errors.is_empty(), "{errors:?}");
    let doc = backend.document();
    let d = doc.borrow();
    let probe = d.children(pane)[0];
    let last = d.get_prop(probe, LAST).expect("set");
    assert_eq!(last.as_str(), Some(r#"{"at":{"line":3,"col":{"x":1}},"ids":[1,2],"n":2} E_RANGE|line 99 is past the end|true"#));
}
