//! GAL1 item 6: a host's extensions: a kind of its own, a story root, a family, a setup hook and
//! a token set.

use skelter_base::accesskit::Role;
use skelter_base::change::ChangeClass;
use skelter_core::{Kind, KindRegistry, PropDefault, PropSpec, RecordCx, ValueType};
use skelter_gallery::{Cell, Extensions, Renderer, Theme};
use skelter_widgets::theme::TokenMap;
use std::cell::Cell as StdCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

static SWATCH: skelter_core::KindSpec = skelter_core::KindSpec {
    props: &[PropSpec::new("label", ValueType::Str, ChangeClass::Paint).default(PropDefault::Str(""))],
    role: Role::Image,
    doc: "A test kind: a box the host registers.",
    ..skelter_core::KindSpec::new("gallery-swatch")
};

#[derive(Default)]
struct Swatch;

impl Kind for Swatch {
    fn spec() -> &'static skelter_core::KindSpec {
        &SWATCH
    }
    fn record(&self, cx: &mut RecordCx) {
        cx.draw_box();
    }
}

fn register(reg: &mut KindRegistry) -> Result<(), String> {
    reg.register::<Swatch>().map(|_| ()).map_err(|e| e.to_string())
}

fn ext(calls: Rc<StdCell<u32>>) -> Extensions {
    let mut red = TokenMap::new();
    red.insert("--sv-bg", "#ff0000");
    Extensions::new()
        .story_root(fixture("ext"))
        .family("host")
        .kinds(register)
        .theme("test-red", red)
        .on_setup(move |cx| {
            assert_eq!(cx.story.id.as_str(), "swatch");
            calls.set(calls.get() + 1);
        })
}

#[test]
fn coverage_and_themes_see_the_host() {
    let e = ext(Rc::default());
    assert_eq!(e.theme_named("test-red"), Some(Theme::Extra("test-red".into())));
    assert_eq!(e.theme_named("solarized"), None);
    let reg = skelter_gallery::cli::registry(&e).unwrap();
    let cat = skelter_gallery::discover_with(&e.story_roots, &e.families).unwrap();
    let cov = cat.coverage(&reg);
    assert!(cov.covered.contains(&"gallery-swatch".to_string()), "{cov:?}");
    assert!(skelter_gallery::discover(&e.story_roots).is_err(), "family `host` needs the extension");
}

#[test]
fn the_host_kind_renders_with_its_setup_and_theme() {
    if std::env::var("SKELTER_GPU_TESTS").as_deref() != Ok("1") {
        eprintln!("skipped: set SKELTER_GPU_TESTS=1 to run GPU tests");
        return;
    }
    let calls: Rc<StdCell<u32>> = Rc::default();
    let mut r = Renderer::new(&ext(calls.clone())).unwrap();
    let s = r.catalog().get("swatch").unwrap().clone();
    let mut c = Cell::new(&s, &s.variants[0]);
    let shot = r.shot(&c).unwrap();
    assert_eq!(calls.get(), 1, "the setup hook ran before the mount");
    assert!(shot.warnings.is_empty(), "{:?}", shot.warnings);
    let v = r.inspect(&c, "gallery-swatch").unwrap();
    assert_eq!(shot.image.pixel(30, 20), [0, 255, 0, 255], "the swatch drew its background");
    assert_eq!(v["nodes"][0]["kind"], "gallery-swatch");
    // the host's token set: the cell's background is its `--sv-bg`
    c.theme = Theme::Extra("test-red".into());
    let red = r.shot(&c).unwrap();
    assert_eq!(red.image.pixel(2, 2), [255, 0, 0, 255]);
    let rep = r.check();
    assert!(rep.ok(), "{:#?}", rep.problems);
}
