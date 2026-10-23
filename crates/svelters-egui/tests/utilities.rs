//! U3 (plans/research/UTILITY-CSS.md §4.1, §4.6, §7 "U3"): utility classes on the egui scene,
//! driven through the `style` host op and the Rust API, headless under kittest (the recording host
//! of `common`, no JS). Values are the theme's tokens, so each expectation is what a hand-written
//! declaration of the same `var(--sv-*)` computes to on a probe node.

mod common;

use common::Rig;
use svelters_abi::*;
use svelters_egui::Utilities;
use svelters_egui::imm::El;
use svelters_egui::scene::{NodeId, StateBits};
use svelters_egui::theme::Theme;
use svelters_style::{Color, ComputedValue};

fn sheet(rig: &Rig, id: &str, css: &str) {
    rig.br().style(SVL_STYLE_SHEET, 0, Some(&format!("{id}{SVL_STYLE_SEP}{css}"))).unwrap();
}

fn classes(rig: &Rig, h: svl_handle, c: &str) {
    rig.br().style(SVL_STYLE_CLASSES, h, Some(c)).unwrap();
}

fn get(rig: &Rig, h: svl_handle, prop: &str) -> Option<ComputedValue> {
    rig.backend.computed_style(h)?.get(prop).cloned()
}

fn rgb(r: u8, g: u8, b: u8) -> Option<ComputedValue> {
    Some(ComputedValue::Color(Color { r: r as f32 / 255.0, g: g as f32 / 255.0, b: b as f32 / 255.0, a: 1.0 }))
}

/// What `decl` (one declaration) computes `prop` to on the probe node: the expected value.
fn probe(rig: &mut Rig, p: svl_handle, decl: &str, prop: &str) -> Option<ComputedValue> {
    rig.backend.set_inline(p, Some(decl)).unwrap();
    rig.run();
    let v = get(rig, p, prop);
    assert!(v.is_some(), "the probe computes {prop} from '{decl}'");
    v
}

fn state(rig: &Rig, h: svl_handle) -> StateBits {
    rig.backend.scene().read(|s| s.get(NodeId::from_raw(h).unwrap()).unwrap().state())
}

/// The left inset of `child` in `parent` (the parent's padding).
fn inset(rig: &Rig, parent: svl_handle, child: svl_handle) -> f32 {
    let (p, c) = rig.backend.with_scene(|s| (s.n(parent).rect, s.n(child).rect));
    c.min.x - p.min.x
}

/// A themed rig with utilities on, a window column, and a probe box.
fn rig(u: Utilities) -> (Rig, svl_handle, svl_handle) {
    let mut rig = Rig::new();
    rig.backend.set_theme(&Theme::dark_modern());
    rig.backend.set_utilities(&u);
    let (_w, col) = rig.window();
    let p = rig.leaf(col, "box", &[]);
    rig.run();
    (rig, col, p)
}

fn text(s: &str) -> (&'static str, Value) {
    ("text", Value::Str(s.into()))
}

/// `<box class="p-4 bg-surface rounded-md">`: the computed values are the theme tokens', the
/// padding lays out, and a scheme switch recolours.
#[test]
fn utilities_compute_the_theme_tokens_and_follow_the_scheme() {
    let (mut rig, col, p) = rig(Utilities::on());
    let b = rig.leaf(col, "box", &[]);
    let inner = rig.leaf(b, "label", &[text("inside")]);
    classes(&rig, b, "p-4 bg-surface rounded-md");
    rig.run();
    let surface = probe(&mut rig, p, "background-color: var(--sv-surface)", "background-color");
    assert_eq!(get(&rig, b, "background-color"), surface);
    assert_eq!(get(&rig, b, "border-radius"), probe(&mut rig, p, "border-radius: var(--sv-radius-md)", "border-radius"));
    let space4 = Theme::dark_modern().px("--sv-space-4").unwrap();
    assert!((inset(&rig, b, inner) - space4).abs() < 0.5, "p-4 = --sv-space-4 ({space4}px): {}", inset(&rig, b, inner));
    let stats = rig.backend.utility_stats();
    assert_eq!((stats.generated, stats.live), (3, 3), "{stats:?}");

    rig.backend.set_theme(&Theme::light_modern());
    rig.run();
    let light = probe(&mut rig, p, "background-color: var(--sv-surface)", "background-color");
    assert_ne!(light, surface, "the light surface differs");
    assert_eq!(get(&rig, b, "background-color"), light, "the scheme switch recoloured the utility");
    assert_eq!(rig.backend.utility_stats().generated, 3, "a theme switch generates nothing");
}

/// `hover:bg-list-hover` restyles in the frame that hovers (as `style.rs`'s
/// `hover_restyles_in_the_same_frame`): the new rule's state dependency is in the maps.
#[test]
fn hover_utility_restyles_in_the_same_frame() {
    let (mut rig, col, p) = rig(Utilities::on());
    let btn = rig.leaf(col, "button", &[text("Hover me")]);
    classes(&rig, btn, "hover:bg-list-hover");
    let want = probe(&mut rig, p, "background-color: var(--sv-list-hover)", "background-color");
    assert_ne!(get(&rig, btn, "background-color"), want);
    let checkpoints = rig.log.borrow().checkpoints;
    rig.harness.hover_at(rig.drawn(btn).center());
    rig.harness.step(); // one frame
    assert!(state(&rig, btn).contains(StateBits::HOVERED));
    assert_eq!(get(&rig, btn, "background-color"), want, "restyled before it painted");
    assert_eq!(rig.log.borrow().checkpoints, checkpoints, "no JS for a hover");
}

/// `part-thumb:bg-accent` reaches the slider's thumb (a `::part(thumb)` of the host).
#[test]
fn part_variant_reaches_the_slider_thumb() {
    let (mut rig, col, p) = rig(Utilities::on());
    let s = rig.leaf(col, "slider", &[("accessible-name", Value::Str("Gain".into()))]);
    classes(&rig, s, "part-thumb:bg-accent");
    rig.run();
    let want = probe(&mut rig, p, "background-color: var(--sv-accent)", "background-color");
    let thumb = rig.backend.computed_part_style(s, "thumb").expect("a rule reaches ::part(thumb)");
    assert_eq!(thumb.get("background-color").cloned(), want);
    assert_ne!(get(&rig, s, "background-color"), want, "the host itself is not coloured");
}

/// Cascade position (UQ3): a utility beats the theme's `base.css`; a component's own unlayered
/// rule beats a utility; `!` beats it back.
#[test]
fn utilities_sit_between_the_theme_and_component_css() {
    let (mut rig, col, p) = rig(Utilities::on());
    let btn = rig.leaf(col, "button", &[text("base")]);
    let card = rig.leaf(col, "box", &[]);
    sheet(&rig, "app", ".card { background-color: #ff0000 }");
    classes(&rig, btn, "bg-accent");
    classes(&rig, card, "card bg-surface");
    rig.run();
    let accent = probe(&mut rig, p, "background-color: var(--sv-accent)", "background-color");
    let surface = probe(&mut rig, p, "background-color: var(--sv-surface)", "background-color");
    assert_eq!(get(&rig, btn, "background-color"), accent, "a utility beats base.css's button rule");
    assert_eq!(get(&rig, card, "background-color"), rgb(255, 0, 0), "the component rule beats the utility");
    classes(&rig, card, "card bg-surface!");
    rig.run();
    assert_eq!(get(&rig, card, "background-color"), surface, "`!` beats the component rule");
}

/// A dynamic class (`class="p-{n}"`) follows its value: each new value is generated on first use,
/// and the node's references move with it.
#[test]
fn dynamic_classes_follow_their_value() {
    let (mut rig, col, _p) = rig(Utilities::on());
    let b = rig.leaf(col, "box", &[]);
    let inner = rig.leaf(b, "label", &[text("n")]);
    let t = Theme::dark_modern();
    for n in [1, 2, 3, 2, 1] {
        classes(&rig, b, &format!("p-{n}"));
        rig.run();
        let want = t.px(&format!("--sv-space-{n}")).unwrap();
        assert!((inset(&rig, b, inner) - want).abs() < 0.5, "p-{n}: {} vs {want}", inset(&rig, b, inner));
    }
    assert_eq!(rig.backend.utility_stats().generated, 3, "each value generated once");
}

/// Opt-in (UQ8): with utilities off, `flex` and `p-4` are plain classes; with the prefix `tw`, only
/// `tw:`-prefixed classes are utilities. Turning them on later applies to live nodes.
#[test]
fn opt_in_and_prefix() {
    let (mut rig, col, _p) = rig(Utilities::off());
    let a = rig.leaf(col, "box", &[]);
    let a_in = rig.leaf(a, "label", &[text("a")]);
    let b = rig.leaf(col, "box", &[]);
    let b_in = rig.leaf(b, "label", &[text("b")]);
    sheet(&rig, "app", ".flex { color: #ff0000 }");
    classes(&rig, a, "flex p-4");
    classes(&rig, b, "tw:p-4");
    rig.run();
    assert_eq!(get(&rig, a, "color"), rgb(255, 0, 0), "a semantic class keeps its rule");
    assert_eq!(get(&rig, a, "display"), None, "`flex` is not a utility while they are off");
    assert!(inset(&rig, a, a_in).abs() < 0.5);
    assert_eq!(rig.backend.utility_stats().generated, 0);

    let space4 = Theme::dark_modern().px("--sv-space-4").unwrap();
    rig.backend.set_utilities(&Utilities::on().prefix("tw"));
    rig.run();
    assert!(inset(&rig, a, a_in).abs() < 0.5, "unprefixed p-4 stays plain");
    assert!((inset(&rig, b, b_in) - space4).abs() < 0.5, "tw:p-4 applies to a live node: {}", inset(&rig, b, b_in));
    assert_eq!(rig.backend.utilities().prefix.as_deref(), Some("tw"));

    rig.backend.set_utilities(&Utilities::on());
    rig.run();
    assert!((inset(&rig, a, a_in) - space4).abs() < 0.5, "without the prefix p-4 applies");
    assert!(inset(&rig, b, b_in).abs() < 0.5, "and tw:p-4 is a plain class again");
}

/// App tokens (SV5 `set_tokens`, or a theme override) are utility values: `bg-cad-panel` once
/// `--sv-cad-panel` exists, including on a node that carried the class before the token did.
#[test]
fn app_tokens_are_utility_values() {
    let (mut rig, col, _p) = rig(Utilities::on());
    let early = rig.leaf(col, "box", &[]);
    classes(&rig, early, "bg-cad-panel");
    rig.run();
    assert_eq!(get(&rig, early, "background-color"), None, "no such token yet: a plain class");
    rig.backend.set_tokens(None, &[("--sv-cad-panel", "#123456")]).unwrap();
    let late = rig.leaf(col, "box", &[]);
    classes(&rig, late, "bg-cad-panel");
    rig.run();
    assert_eq!(get(&rig, late, "background-color"), rgb(0x12, 0x34, 0x56));
    assert_eq!(get(&rig, early, "background-color"), rgb(0x12, 0x34, 0x56), "resolved again when the token arrived");

    rig.backend.set_theme(&Theme::dark_modern().with([("--sv-cad-rail", "#654321")]));
    let themed = rig.leaf(col, "box", &[]);
    classes(&rig, themed, "bg-cad-rail");
    rig.run();
    assert_eq!(get(&rig, themed, "background-color"), rgb(0x65, 0x43, 0x21), "a theme override is a value too");
}

/// OW-RUST: an `imm` virtual element with utility classes computes what the Svelte node does.
#[test]
fn imm_elements_take_utility_classes() {
    let (mut rig, col, _p) = rig(Utilities::on());
    let sv = rig.backend.imm();
    let el = El::new("button").class("bg-accent rounded-md px-2");
    // first use from Rust: generated by the imm computation itself
    let v = sv.computed(&el, StateBits::NONE).expect("the cascade runs");
    let btn = rig.leaf(col, "button", &[text("node")]);
    classes(&rig, btn, "bg-accent rounded-md px-2");
    rig.run();
    for prop in ["background-color", "border-radius", "padding-left", "padding-right"] {
        assert_eq!(v.get(prop), get(&rig, btn, prop).as_ref(), "{prop}");
    }
    assert!(v.get("background-color").is_some());
    let hover = sv.computed(&El::new("button").class("hover:bg-list-hover"), StateBits::HOVERED).unwrap();
    let plain = sv.computed(&El::new("button").class("hover:bg-list-hover"), StateBits::NONE).unwrap();
    assert_ne!(hover.get("background-color"), plain.get("background-color"), "a state variant follows the virtual state");
}

const APP: &str = r#"<script>
  let n = $state(1);
</script>

<window title="u3">
  <box class="p-{n} bg-surface rounded-md">
    <label class="text-fg">pad {n}</label>
    <button class="hover:bg-list-hover px-2" onclicked={() => n++}>More</button>
  </box>
  <box class="card bg-accent"><label>card</label></box>
</window>

<style>
  .card { background-color: #ff0000; }
</style>
"#;

/// The app as a scratch dir linked to `fixtures/embed-egui`'s node_modules (as LD2's tests do;
/// needs `pnpm install`).
#[cfg(feature = "source")]
fn scratch(name: &str) -> std::path::PathBuf {
    use std::path::Path;
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/embed-egui").canonicalize().unwrap();
    assert!(fixture.join("node_modules").exists(), "run `pnpm install` first");
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/u3").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("package.json"), r#"{ "name": "u3", "private": true, "type": "module", "svelters": { "toolkit": "egui" } }"#).unwrap();
    std::fs::write(dir.join("src/App.svelte"), APP).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(fixture.join("node_modules"), dir.join("node_modules")).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(fixture.join("node_modules"), dir.join("node_modules")).unwrap();
    dir.canonicalize().unwrap()
}

/// A release `.svb` bundle carries nothing for utilities (the host generates them on first use):
/// the app styles the same from it as from source, before and after a dynamic `p-{n}` changes,
/// with the component's own `<style>` still beating `bg-accent`.
#[cfg(feature = "source")]
#[test]
fn a_svb_bundle_styles_like_source() {
    use egui_kittest::kittest::Queryable;
    use std::rc::Rc;
    use std::sync::Arc;
    use svelters_egui::harness::TestApp;
    use svelters_host::modules::svb::{self, BuildOptions, Svb, SvbProvider};
    use svelters_host::modules::{SourceConfig, SourceProvider};

    let dir = scratch("svb");
    let source_cfg = || {
        let mut c = SourceConfig::for_path(&dir).unwrap();
        c.backend = "egui".into();
        c
    };
    let bytes = svb::build(source_cfg(), &BuildOptions::default()).unwrap();
    let quiet = svelters_host::HostConfig { echo_console: false, echo_errors: true, ..svelters_host::HostConfig::default() };
    let launch = |provider: Rc<dyn svelters_host::ModuleProvider>| {
        let mut app = TestApp::with_loader(quiet.clone(), |embed| {
            embed.backend().set_theme(&Theme::dark_modern());
            embed.backend().set_utilities(&Utilities::on());
            embed.load_app_from(provider)
        })
        .unwrap();
        app.run();
        assert!(app.errors().is_empty(), "{:?}", app.errors());
        app
    };
    let mut from_source = launch(Rc::new(SourceProvider::new(source_cfg()).unwrap()));
    let mut from_svb = launch(Rc::new(SvbProvider::new(Arc::new(Svb::from_bytes(bytes).unwrap())).unwrap()));

    let props = ["background-color", "padding", "padding-left", "padding-right", "border-radius", "color"];
    let snapshot = |app: &TestApp| -> Vec<Vec<Option<ComputedValue>>> {
        ["box", "label", "button"]
            .iter()
            .flat_map(|tag| app.backend.handles_of(tag))
            .map(|h| {
                let c = app.backend.computed_style(h);
                props.iter().map(|p| c.as_ref().and_then(|c| c.get(p).cloned())).collect()
            })
            .collect()
    };
    let before = snapshot(&from_source);
    assert_eq!(before, snapshot(&from_svb), "the same computed styles");
    let red = from_source.backend.handles_of("box").into_iter().any(|h| from_source.backend.computed_style(h).and_then(|c| c.get("background-color").cloned()) == rgb(255, 0, 0));
    assert!(red, "the component's <style> beats bg-accent");
    let generated = from_svb.backend.utility_stats().generated;
    assert!(generated >= 6, "{generated}");

    for app in [&mut from_source, &mut from_svb] {
        app.harness.get_by_role_and_label(egui::accesskit::Role::Button, "More").click();
        app.run();
        assert!(app.harness.query_by_label("pad 2").is_some());
    }
    assert_eq!(snapshot(&from_source), snapshot(&from_svb), "the same after p-1 became p-2");
    assert_ne!(snapshot(&from_svb), before, "p-2 restyled the box");
    assert_eq!(from_svb.backend.utility_stats().generated, generated + 1, "p-2 generated on first use");
}
