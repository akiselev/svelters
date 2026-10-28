//! GAL1 item 2: shots. GPU tests run only with `SKELTER_GPU_TESTS=1` (skelter's rule: CI sets it,
//! lavapipe on Linux, WARP on Windows, Metal on macOS). `GALLERY_SHOTS=<dir>` writes the PNGs for
//! a look (never compared: no goldens).

use skelter_gallery::{Cell, Extensions, Renderer, State, Theme};
use std::path::{Path, PathBuf};

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

fn keep(name: &str, shot: &skelter_gallery::Shot) {
    if let Some(dir) = std::env::var_os("GALLERY_SHOTS") {
        skelter_gallery::save_png(&shot.image, &Path::new(&dir).join(format!("{name}.png"))).unwrap();
    }
}

fn renderer() -> Renderer {
    Renderer::new(&Extensions::new().story_root(fixture("gallery"))).unwrap()
}

fn cell(r: &Renderer, story: &str, variant: &str) -> Cell {
    let s = r.catalog().get(story).unwrap();
    Cell::new(s, s.variant(variant).unwrap())
}

#[test]
fn a_shot_has_the_cell_size_and_is_deterministic() {
    if !gpu() {
        return;
    }
    let mut r = renderer();
    let c = cell(&r, "button", "primary");
    let a = r.shot(&c).unwrap();
    keep("button-primary", &a);
    assert_eq!((a.image.width, a.image.height), (160, 48));
    assert!(a.warnings.is_empty(), "{:?}", a.warnings);
    // something drew: more than one colour
    let first = &a.image.rgba[..4];
    assert!(a.image.rgba.chunks_exact(4).any(|p| p != first), "a blank shot");
    // another cell, then the first again: the same bytes
    let mut other = c.clone();
    other.theme = Theme::HcLight;
    other.zoom = 2.0;
    let b = r.shot(&other).unwrap();
    keep("button-primary-hc-light-x2", &b);
    assert_eq!((b.image.width, b.image.height), (320, 96), "zoom 2: twice the device pixels");
    let again = r.shot(&c).unwrap();
    assert!(again.image == a.image, "the same cell twice gives the same pixels");
}

#[test]
fn disabled_goes_through_the_prop_and_is_forced_otherwise() {
    if !gpu() {
        return;
    }
    let mut r = renderer();
    let mut c = cell(&r, "button", "primary");
    c.state = State::Disabled;
    let m = r.mount(&c).unwrap();
    assert_eq!(m.applied.as_deref(), Some("disabled prop"));
    // the button's real disabled state: its `enabled` prop is false, nothing is forced
    let b = m.subjects[0];
    let forced = r.backend().forced_state(b);
    assert!(forced.is_empty(), "{forced:?}");
    let disabled = r.backend().with_doc(|d| {
        let n = d.query().anchor("subject").collect();
        d.state(n[0]).contains(svelters_style::StateMask::DISABLED)
    });
    assert!(disabled, "the button is disabled for real");
    keep("button-primary-disabled", &r.shot(&c).unwrap());

    let mut p = cell(&r, "button", "plain");
    p.state = State::Disabled;
    let m = r.mount(&p).unwrap();
    assert!(m.applied.as_deref().unwrap().starts_with("forced :disabled on 1 subject"), "{:?}", m.applied);
    assert!(r.backend().forced_state(m.subjects[0]).contains(svelters_style::StateMask::DISABLED));
}

#[test]
fn forced_hover_lands_on_the_subject() {
    if !gpu() {
        return;
    }
    let mut r = renderer();
    let mut c = cell(&r, "button", "primary");
    c.state = State::Hover;
    let m = r.mount(&c).unwrap();
    assert!(m.has_subject);
    assert_eq!(m.subjects.len(), 1);
    // POL2-REGISTER: the pointer rests on the subject too (a `tooltip` shows)
    assert!(m.applied.as_deref().is_some_and(|a| a.starts_with("forced :hover on 1 subject, pointer over it for")), "{:?}", m.applied);
    assert!(r.backend().forced_state(m.subjects[0]).contains(svelters_style::StateMask::HOVERED));
    keep("button-primary-hover", &r.shot(&c).unwrap());
    // the next cell starts clean: a fresh mount, nothing forced
    c.state = State::Normal;
    let m = r.mount(&c).unwrap();
    assert!(r.backend().forced_state(m.subjects[0]).is_empty());
}

#[test]
fn a_sheet_places_every_cell_and_indexes_it() {
    if !gpu() {
        return;
    }
    let mut r = renderer();
    let story = r.catalog().get("button").unwrap().clone();
    let axes = skelter_gallery::Axes { variants: Some(vec!["plain".into()]), states: Some(vec![State::Normal, State::Hover]), themes: vec![Theme::DarkModern, Theme::LightModern], ..Default::default() };
    let m = skelter_gallery::Matrix::new(&story, &axes).unwrap();
    let sheet = r.sheet(&m, skelter_gallery::SheetLayout::default()).unwrap();
    assert_eq!(sheet.cells.len(), 4);
    assert_eq!(sheet.errors().count(), 0);
    for c in &sheet.cells {
        assert_eq!((c.rect.w, c.rect.h), (160, 48));
        assert!(c.rect.x + c.rect.w <= sheet.image.width && c.rect.y + c.rect.h <= sheet.image.height);
        assert!(c.header.y + c.header.h == c.rect.y, "the header sits on its image");
    }
    // rows are states, columns themes
    assert_eq!(sheet.cells[0].rect.y, sheet.cells[1].rect.y);
    assert!(sheet.cells[2].rect.y > sheet.cells[0].rect.y);
    assert!(sheet.cells[1].rect.x > sheet.cells[0].rect.x);
    let index = sheet.index(&m, None);
    assert_eq!(index["cells"][3]["cell"]["state"], "hover");
    assert_eq!(index["cells"][3]["cell"]["theme"], "light-modern");
    let again = r.sheet(&m, skelter_gallery::SheetLayout::default()).unwrap();
    assert!(again.image == sheet.image, "deterministic within a run");
    if let Some(dir) = std::env::var_os("GALLERY_SHOTS") {
        skelter_gallery::save_png(&sheet.image, &Path::new(&dir).join("sheet.png")).unwrap();
    }
}

#[test]
fn inspect_shows_a_forced_hover_in_the_matched_rules() {
    if !gpu() {
        return;
    }
    let mut r = renderer();
    let mut c = cell(&r, "button", "primary");
    let normal = r.inspect(&c, "#subject").unwrap();
    c.state = State::Hover;
    let hover = r.inspect(&c, "#subject").unwrap();
    let n = &hover["nodes"][0];
    assert_eq!(n["kind"], "button");
    assert_eq!(n["anchor"], "subject");
    assert_eq!(n["state"]["forced"], serde_json::json!(["hover"]));
    assert_eq!(n["box"]["h"].as_f64().map(|h| h > 0.0), Some(true), "{n}");
    assert_eq!(n["accesskit"]["role"], "Button", "{n}");
    assert_eq!(n["accesskit"]["name"], "Run small");
    let selectors = |v: &serde_json::Value| -> Vec<String> { v["nodes"][0]["matched_rules"].as_array().unwrap().iter().map(|r| r["selector"].as_str().unwrap().to_string()).collect() };
    let (before, after) = (selectors(&normal), selectors(&hover));
    let hover_rule = after.iter().find(|s| s.starts_with(".b.") && s.ends_with(":hover")).unwrap_or_else(|| panic!("no :hover rule matched: {after:?}"));
    assert!(!before.contains(hover_rule), "the hover rule matched only under the forced state: {before:?}");
    // the component's rule points at its <style> in the variant file
    let rule = hover["nodes"][0]["matched_rules"].as_array().unwrap().iter().find(|r| r["selector"].as_str() == Some(hover_rule)).unwrap();
    assert!(rule["source"]["file"].as_str().is_some_and(|f| f.ends_with("primary.svelte")), "{rule}");
    assert_eq!(rule["source"]["line"], 11, "{rule}");
    assert!(rule["layer"].is_null(), "an unlayered component rule: {rule}");
    // base.css's kind rules carry their @layer (R1)
    let rules = hover["nodes"][0]["matched_rules"].as_array().unwrap();
    assert!(rules.iter().any(|r| r["layer"] == "sv.base" && r["selector"] == "button"), "{rules:?}");
    assert_eq!(n["computed"]["background"], "#ff0000", "{n}");
    // a kind name picks too
    assert_eq!(r.inspect(&c, "button").unwrap()["nodes"].as_array().unwrap().len(), 1);
    assert!(r.inspect(&c, "#nope").is_err());
}
