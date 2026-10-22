//! RA1 acceptance (ARCH-EMBED §10): this crate registers `<quantity-field>` with
//! `#[svelters::widget]`; the registry gives it a pack-62 id; the compiler, given the dumped spec
//! fragment, rejects an unknown prop on it and accepts `bind:value` (`exact`); the checked-in
//! `kinds.json`/typings match this binary; and kittest drives the field by its label.
//! (svelte-check against the dumped typings runs in `app/`'s `pnpm test`.)

use egui_kittest::kittest::Queryable;
use std::path::PathBuf;
use svelters_compiler::compile::{CompileOptions, CompileResult, compile};
use svelters_compiler::diag::Severity;
use svelters_compiler::semantics::SpecBackend;
use svelters_egui::egui;
use svelters_egui::egui::accesskit::Role;
use svelters_egui::harness::TestApp;
use svelters_egui::kinds::{self, KindRegistry, PROVIDER_PACK};

fn compile_egui(src: &str) -> CompileResult {
    egui_kinds_demo::link();
    let b = SpecBackend::builtin("egui")
        .unwrap()
        .merge_fragment(&svelters_egui::spec::fragment_text())
        .unwrap();
    compile(src, &CompileOptions::new("App.svelte", &b))
}

fn errors(r: &CompileResult) -> Vec<String> {
    r.diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| format!("{}: {}", d.code, d.message))
        .collect()
}

#[test]
fn registry_gives_the_kind_a_pack_62_id() {
    egui_kinds_demo::link();
    let reg = KindRegistry::global();
    let k = reg
        .tag_index("quantity-field")
        .expect("quantity-field is registered");
    let p = reg.provider(k).expect("a provider kind");
    assert_eq!(p.entry.id, "cad.QuantityField");
    let id = reg.kind_id(k);
    assert_eq!(id & (0x3F << 22), PROVIDER_PACK);
    assert_eq!(reg.kind_of_id(id), Some(k));
    // own props and the Widget base's resolve, and ids round-trip to the same definitions
    for name in [
        "value",
        "dim",
        "units-shown",
        "label",
        "problem",
        "enabled",
        "accessible-name",
    ] {
        let (pid, def) = kinds::find_prop(k, name).unwrap_or_else(|| panic!("{name}"));
        assert!(
            std::ptr::eq(kinds::prop_def(k, pid).unwrap(), def),
            "{name}"
        );
    }
    assert!(
        kinds::find_signal(k, "accepted").is_some() && kinds::find_signal(k, "edited").is_some()
    );
    // provider data on any node through a typed key (ARCH-EMBED §8.4)
    static NOTE: svelters_egui::provider::ExtKey<String> =
        svelters_egui::provider::ExtKey::new("cad.note");
    // (through the scene API, SG1: `SceneHandle::write`/`read`, `NodeMut::ext_mut`, `NodeRef::ext`)
    use svelters_egui::scene::{KindId, ProviderId, SceneHandle};
    let scene = SceneHandle::default();
    let app = ProviderId::APP;
    let n = scene.write(app, |s| s.create(KindId(id)).unwrap()).unwrap();
    scene
        .write(app, |s| {
            let mut m = s.node(n).unwrap();
            assert!(m.read().ext(&NOTE).is_none());
            m.ext_mut(&NOTE).set("pending".to_string());
        })
        .unwrap();
    scene.read(|s| {
        let n = s.get(n).unwrap();
        assert_eq!(n.ext(&NOTE).map(String::as_str), Some("pending"));
        assert_eq!(n.ext_block().names().collect::<Vec<_>>(), ["cad.note"]);
        // the node holds the widget struct with its defaults, typed
        assert_eq!(
            n.widget(|w: &egui_kinds_demo::QuantityField| w.units_shown()),
            Some(true)
        );
    });
    // a provider prop id is not accepted on a built-in kind
    let (value, _) = kinds::find_prop(k, "value").unwrap();
    assert!(kinds::prop_def(kinds::tag_index("label").unwrap(), value).is_none());
}

#[test]
fn compiler_rejects_unknown_props_and_accepts_bind_value() {
    let r = compile_egui(r#"<window><quantity-field label="W" dim="length" bogus={1} /></window>"#);
    let e = errors(&r);
    assert!(e.iter().any(|m| m.contains("bogus")), "{e:?}");

    let r = compile_egui(r#"<window><quantity-field label="W" dim="mass" /></window>"#);
    assert!(
        !errors(&r).is_empty(),
        "a static enum value outside the Rust enum is refused"
    );

    let r = compile_egui(
        r#"<script>let w = $state(0.01); let a = $state(0);</script>
<window><quantity-field label="W" dim="angle" units-shown={false} bind:value={w} onaccepted={(v) => (a = v)} /></window>"#,
    );
    assert!(errors(&r).is_empty(), "{:?}", errors(&r));
    let js = r.js.unwrap();
    assert!(
        js.contains(r#"$n.toplevel("window", "present")"#) || js.contains("quantity-field"),
        "{js}"
    );
    assert!(
        js.contains(r#""value""#),
        "bind:value lowers onto observation of `value`: {js}"
    );
    // no warnings about stale observation: the prop is exact/delivery by construction
    let warns: Vec<&str> = r
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Warning)
        .map(|d| d.code)
        .collect();
    assert!(!warns.iter().any(|c| c.contains("bind")), "{warns:?}");
}

fn dump_dir() -> PathBuf {
    egui_kinds_demo::dump_dir()
}

/// The checked-in `app/.svelters/app` is what this binary dumps
/// (`SVELTERS_UPDATE=1 cargo test -p egui-kinds-example --test kinds` rewrites it).
#[test]
fn checked_in_kinds_json_and_typings_are_current() {
    egui_kinds_demo::link();
    if std::env::var("SVELTERS_UPDATE").as_deref() == Ok("1") {
        svelters_egui::dump(&dump_dir()).unwrap();
        return;
    }
    let tmp = std::env::temp_dir().join(format!("svelters-egui-kinds-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    svelters_egui::dump(&tmp).unwrap();
    // every dumped file, also under `behaviors/` (EV2b)
    let mut dirs = vec![PathBuf::new()];
    while let Some(rel) = dirs.pop() {
        for e in std::fs::read_dir(tmp.join(&rel)).unwrap().flatten() {
            let name = rel.join(e.file_name());
            if e.path().is_dir() {
                dirs.push(name);
                continue;
            }
            let want = std::fs::read_to_string(e.path()).unwrap();
            let have = std::fs::read_to_string(dump_dir().join(&name)).unwrap_or_default();
            assert!(
                have == want,
                "app/.svelters/app/{} is stale: run SVELTERS_UPDATE=1 cargo test -p egui-kinds-example --test kinds",
                name.display()
            );
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
}

fn bundle() -> PathBuf {
    let p = egui_kinds_demo::bundle();
    assert!(
        p.exists(),
        "{} is missing: run `pnpm build` (needs `cargo build -p svelters-compiler` first)",
        p.display()
    );
    p
}

fn has(app: &TestApp, label: &str) -> bool {
    app.harness.query_by_label(label).is_some()
}

#[test]
fn kittest_drives_the_field_by_label() {
    egui_kinds_demo::link();
    let mut app = TestApp::load(bundle()).unwrap();
    app.run();
    assert!(
        has(&app, "width 0.012 accepted 0"),
        "initial bound value reached the label"
    );
    assert!(has(&app, "edits none"));
    // the field shows the bound SI value in the display unit
    // the field's TextInput carries the label (Widget::a11y); the inline caption is a separate Label
    let field = app.harness.get_by_role_and_label(Role::TextInput, "Width");
    assert_eq!(field.value().as_deref(), Some("12 mm"));

    field.focus();
    app.run();
    app.harness
        .key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    app.run();
    app.harness
        .get_by_role_and_label(Role::TextInput, "Width")
        .type_text("25 mm");
    app.run();
    assert!(has(&app, "edits some"), "onedited reached JS");
    app.harness.key_press(egui::Key::Enter);
    app.run();
    app.run();
    // Enter committed: cx.set announced `value` (bind:value), onaccepted carried the SI value
    assert!(
        has(&app, "width 0.025 accepted 0.025"),
        "{:#?}",
        app.harness.root()
    );

    // an entry that does not parse sets the provider state bit `cad.invalid` (state_bits!) and
    // commits nothing
    app.harness
        .get_by_role_and_label(Role::TextInput, "Width")
        .focus();
    app.run();
    app.harness
        .key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    app.harness
        .get_by_role_and_label(Role::TextInput, "Width")
        .type_text("abc");
    app.run();
    let h = app.backend.handles_of("quantity-field")[0];
    let invalid = egui_kinds_demo::cad::INVALID.bits();
    assert!(
        app.backend.with_scene(|s| s.n(h).state().contains(invalid)),
        "cad.invalid is set"
    );
    assert_eq!(
        svelters_egui::provider::state_by_name("cad.invalid"),
        Some(invalid)
    );
    app.harness.key_press(egui::Key::Enter);
    app.run();
    assert!(has(&app, "width 0.025 accepted 0.025"));

    let errors = app.errors();
    assert!(errors.is_empty(), "{errors:?}");
}
