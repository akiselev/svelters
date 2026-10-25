//! presto `css_precompiled`: a release build for a backend with the capability gets its scoped
//! sheet as `svelters_style::binary` bytes, with Svelte's pruning turned into rule hints per
//! element. The precompiled sheet must style like the text one: same rules, selectors and
//! declarations as the engine parses from `css.code`.

use svelters_compiler::compile::{CompileOptions, compile};
use svelters_compiler::semantics::SpecBackend;
use svelters_style::binary::{decode, from_base64, to_base64};
use svelters_style::{ParseOptions, Stylesheet};

const EGUI_JSON: &str = include_str!("../../../metadata/semantics/egui.json");

fn backend(caps: &[&str]) -> SpecBackend {
    let mut spec: serde_json::Value = serde_json::from_str(EGUI_JSON).unwrap();
    spec["capabilities"] = serde_json::json!(caps);
    SpecBackend::from_json(&spec.to_string()).unwrap()
}

const APP: &str = r#"<script>let on = $state(false);</script>
<window><box class="card">
  <label class="title">a</label>
  <label class:on>b</label>
  <button id="ok">c</button>
</box></window>
<style>
  .card .title { color: red; }
  .title { padding: 2px; }
  label { gap: 1px; }
  #ok { gap: 3px; }
  .unused { gap: 6px; }
</style>"#;

fn compiled(caps: &[&str], dev: bool) -> svelters_compiler::scoped_css::ScopedCss {
    let b = backend(caps);
    let mut opts = CompileOptions::new("App.svelte", &b);
    opts.scoped_css = true;
    opts.dev = dev;
    let res = compile(APP, &opts);
    assert!(!res.has_errors(), "{:#?}", res.diagnostics);
    res.css.expect("a scoped sheet")
}

/// The index of the rule whose selector text is `sel`.
fn rule(sheet: &Stylesheet, sel: &str) -> u32 {
    sheet.rules.iter().position(|r| r.selector_text() == sel).unwrap_or_else(|| panic!("no rule {sel}: {:?}", sheet.rules.iter().map(|r| r.selector_text()).collect::<Vec<_>>())) as u32
}

#[test]
fn release_builds_get_a_precompiled_sheet_with_hints() {
    let css = compiled(&["css_precompiled"], false);
    let bytes = css.precompiled.clone().expect("precompiled");
    assert_eq!(from_base64(&css.precompiled_base64().unwrap()).unwrap(), bytes);
    let p = decode(&bytes, &ParseOptions::default()).unwrap();

    // the same rules as the engine parses from the text
    let text = Stylesheet::parse(&css.code, &ParseOptions::default());
    assert_eq!(p.sheet.rules.len(), text.rules.len());
    for (a, b) in p.sheet.rules.iter().zip(&text.rules) {
        assert_eq!(a.selector_text(), b.selector_text());
        let key = |d: &svelters_style::Declaration| (d.name.clone(), d.value.clone(), d.important);
        assert_eq!(a.block.decls.iter().map(key).collect::<Vec<_>>(), b.block.decls.iter().map(key).collect::<Vec<_>>());
        assert_eq!(a.media.len(), b.media.len());
    }

    let h = &css.hash;
    let title_desc = rule(&p.sheet, &format!(".card.{h} .title:where(.{h})"));
    let title = rule(&p.sheet, &format!(".title.{h}"));
    let label = rule(&p.sheet, &format!("label.{h}"));
    let ok = rule(&p.sheet, &format!("#ok.{h}"));
    assert!(p.global_rules.is_empty(), "{:?}", p.global_rules);

    let at = |needle: &str| APP.find(needle).unwrap() as u32;
    let hints = |needle: &str| p.hints.iter().find(|x| x.element == at(needle)).unwrap_or_else(|| panic!("no hints for {needle}: {:?}", p.hints));

    let first = hints(r#"<label class="title">"#);
    for r in [title_desc, title, label] {
        assert!(first.candidates.contains(&r), "{r} in {first:?}");
    }
    assert!(!first.candidates.contains(&ok));
    // `.title` and `label` match whatever the context; `.card .title` depends on the ancestor
    assert!(first.always.contains(&title) && first.always.contains(&label), "{first:?}");
    assert!(!first.always.contains(&title_desc));

    // `class:on`: the classes can change, nothing is static
    let second = hints("<label class:on>");
    assert!(second.candidates.contains(&label) && second.always.is_empty(), "{second:?}");

    let button = hints(r#"<button id="ok">"#);
    assert_eq!(button.candidates, vec![ok]);
    assert_eq!(button.always, vec![ok]);
}

#[test]
fn dev_builds_and_other_backends_keep_the_text_path() {
    assert!(compiled(&["css_precompiled"], true).precompiled.is_none(), "dev");
    assert!(compiled(&[], false).precompiled.is_none(), "no capability");
}

#[test]
fn base64_round_trips() {
    for n in 0..20u8 {
        let bytes: Vec<u8> = (0..n).map(|i| i.wrapping_mul(37).wrapping_add(n)).collect();
        assert_eq!(from_base64(&to_base64(&bytes)).unwrap(), bytes);
    }
    assert_eq!(to_base64(b"svelters"), "c3ZlbHRlcnM=");
}
