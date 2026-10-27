//! cadatomic's gallery requests (POL1-GALLERY G1, G3–G6): per-frame hooks and busy probes, one
//! fragment file per renderer across threads, host fonts, host themes on their own base, and a
//! once-per-backend hook. GPU tests need `SKELTER_GPU_TESTS=1`.

use skelter_base::accesskit::Role;
use skelter_base::change::ChangeClass;
use skelter_core::{Kind, KindRegistry, PropDefault, PropSpec, RecordCx, ValueType};
use skelter_gallery::fonts::{self, FontSet};
use skelter_gallery::{Cell, Extensions, Renderer, Theme};
use skelter_text::{RichText, TextStyle, TextSystem};
use skelter_widgets::theme::{ColorScheme, Contrast, Density, TokenMap};
use std::cell::{Cell as StdCell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

fn gpu() -> bool {
    let on = std::env::var("SKELTER_GPU_TESTS").as_deref() == Ok("1");
    if !on {
        eprintln!("skipped: set SKELTER_GPU_TESTS=1 to run GPU tests");
    }
    on
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

/// The `ext` fixture (story `swatch`) with the host kind.
fn host() -> Extensions {
    Extensions::new().story_root(fixture("ext")).family("host").kinds(register)
}

fn swatch(r: &Renderer) -> Cell {
    let s = r.catalog().get("swatch").unwrap();
    Cell::new(s, &s.variants[0])
}

#[test]
fn g3_renderers_on_two_threads_keep_their_own_fragment() {
    if !gpu() {
        return;
    }
    // both renderers exist at once; the first is dropped before the second loads its variant
    let (made_tx, made_rx) = std::sync::mpsc::channel();
    let (go_tx, go_rx) = std::sync::mpsc::channel::<()>();
    let first = std::thread::spawn(move || {
        let r = Renderer::new(&host()).unwrap();
        made_tx.send(()).unwrap();
        go_rx.recv().unwrap();
        drop(r);
    });
    made_rx.recv().unwrap();
    let mut second = Renderer::new(&host()).unwrap();
    go_tx.send(()).unwrap();
    first.join().unwrap();
    let c = swatch(&second);
    second.mount(&c).expect("the second renderer's fragment survives the first's drop");
}

#[test]
fn g1_frame_hooks_run_each_step_and_busy_probes_hold_settle() {
    if !gpu() {
        return;
    }
    let frames: Rc<RefCell<Vec<(u64, f64)>>> = Rc::default();
    let busy_left = Rc::new(StdCell::new(0u32));
    let (f, b) = (frames.clone(), busy_left.clone());
    let ext = host()
        .on_frame(move |cx| {
            assert!(cx.with_doc(|d| d.len()) > 0, "the hook sees the document");
            f.borrow_mut().push((cx.frame, cx.now_ms));
        })
        .busy(move |_doc| {
            let n = b.get();
            (n > 0).then(|| {
                b.set(n - 1);
                format!("highlighting ({n} left)")
            })
        });
    let mut r = Renderer::new(&ext).unwrap();
    let c = swatch(&r);
    frames.borrow_mut().clear();
    busy_left.set(5);
    let m = r.mount(&c).unwrap();
    assert!(m.settled, "{:?}", m.warnings);
    assert!(m.steps >= 6, "settle waited for the busy probe ({} steps)", m.steps);
    let f = frames.borrow().clone();
    assert!(f.len() >= m.steps as usize, "one frame hook call per step: {} for {} steps", f.len(), m.steps);
    assert!(f.windows(2).all(|w| w[1].0 == w[0].0 + 1 && w[1].1 >= w[0].1), "frames count up, time runs forward: {f:?}");

    // a probe that never settles is reported
    busy_left.set(u32::MAX);
    let m = r.mount(&c).unwrap();
    assert!(!m.settled);
    assert!(m.warnings.iter().any(|w| w.contains("not settled") && w.contains("host busy: highlighting")), "{:?}", m.warnings);
}

#[test]
fn g4_host_fonts_join_the_hermetic_set() {
    let probe = std::fs::read(fixture("fonts/probe.ttf")).unwrap();
    let ext = Extensions::new().fonts([probe.clone()]);
    assert_eq!(ext.font_files.len(), 1);
    let store = fonts::store_with(FontSet::Bundled, &ext.font_files);
    let mut text = TextSystem::new(store.clone());
    let l = text.layout(&RichText::plain("A", TextStyle { family: "\"Gallery Probe\"".into(), ..TextStyle::default() }), None);
    let run = &l.runs().first().expect("a run").run;
    assert_ne!(u32::from(run.glyphs[0].id), 0);
    assert_eq!(store.face(run.font).unwrap().data.data().len(), probe.len(), "drawn with the host's font");
}

#[test]
fn g5_a_host_theme_has_its_own_scheme_and_contrast() {
    let mut tokens = TokenMap::new();
    tokens.insert("--cad-accent", "#0a7");
    let ext = host()
        .theme("cad-dark", TokenMap::new())
        .theme_with("cad-light", ColorScheme::Light, Contrast::Normal, tokens.clone())
        .theme_with("cad-hc-light", ColorScheme::Light, Contrast::High, tokens);
    let light = ext.resolve_theme(&Theme::Extra("cad-light".into()), Density::Default).unwrap();
    assert!(!light.is_dark());
    assert_eq!(light.contrast(), Contrast::Normal);
    assert_eq!(light.token("--cad-accent"), Some("#0a7"));
    let builtin = ext.resolve_theme(&Theme::LightModern, Density::Default).unwrap();
    assert_eq!(light.token("bg"), builtin.token("bg"), "Light Modern's tokens under the host's");
    assert_eq!(ext.resolve_theme(&Theme::Extra("cad-hc-light".into()), Density::Default).unwrap().contrast(), Contrast::High);
    assert!(ext.resolve_theme(&Theme::Extra("cad-dark".into()), Density::Default).unwrap().is_dark(), "`theme` stays Dark Modern");
    if !gpu() {
        return;
    }
    // the renderer installs the theme service from that base
    let mut r = Renderer::new(&ext).unwrap();
    let mut c = swatch(&r);
    c.theme = Theme::Extra("cad-light".into());
    r.mount(&c).unwrap();
    let svc = r.backend().with_doc(|d| d.service_ref::<skelter_widgets::theme::Theme>().cloned()).unwrap();
    assert!(!svc.is_dark(), "the theme service is light");
}

#[test]
fn g6_the_backend_hook_runs_once_before_any_variant_loads() {
    if !gpu() {
        return;
    }
    let calls = Rc::new(StdCell::new(0u32));
    let seen = calls.clone();
    let ext = host().on_backend(move |b| {
        seen.set(seen.get() + 1);
        assert_eq!(b.scene().read(|s| s.nodes().filter(|n| n.tag() == "gallery-swatch").count()), 0, "nothing mounted yet");
    });
    let mut r = Renderer::new(&ext).unwrap();
    assert_eq!(calls.get(), 1);
    let c = swatch(&r);
    r.mount(&c).unwrap();
    r.mount(&c).unwrap();
    assert_eq!(calls.get(), 1, "once per renderer, not per cell");
}
