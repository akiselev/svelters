//! cadatomic's requests in the app (POL1-GALLERY): G1 frame hooks and G6 the backend hook run in
//! the app's loop, G2 a host's own `code-editor` and the gallery's source pane (quire's editor under
//! `gallery-source`) work in one app, G5 a host theme's tokens come from its own base.

use skelter_base::accesskit::Role;
use skelter_core::{Kind, KindRegistry, Value};
use skelter_gallery::Extensions;
use skelter_gallery_app::GalleryDocument;
use skelter_test::Harness;
use skelter_widgets::theme::{ColorScheme, Contrast, Density, TokenMap};
use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// A copy of the fixture gallery for test `name` (each test its own: the tests run in parallel, and
/// Windows refuses to delete files another test's watcher holds open).
fn fixture(name: &str) -> PathBuf {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/gallery");
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/gallery-app-tests").join(format!("hooks-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    copy(&src, &dir);
    skelter_gallery::canonicalize(&dir).unwrap()
}

fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let p = e.unwrap().path();
        let dest = to.join(p.file_name().unwrap());
        if p.is_dir() { copy(&p, &dest) } else { std::fs::copy(&p, dest).map(|_| ()).unwrap() }
    }
}

fn gallery(ext: Extensions) -> Harness<GalleryDocument> {
    let fonts = skelter_text::FontStore::new(skelter_text::FontConfig::testing());
    let doc = skelter_gallery_app::app::document(ext, fonts).unwrap();
    let mut h = Harness::<GalleryDocument>::builder().with_size(skelter_base::geom::Size::new(1500.0, 950.0)).build(doc);
    h.run();
    h
}

/// A host kind that takes the `code-editor` tag (cadatomic's has `doc`/`value` props).
static HOST_EDITOR: skelter_core::KindSpec = skelter_core::KindSpec { role: Role::GenericContainer, doc: "A host's own code editor.", ..skelter_core::KindSpec::new("code-editor") };

#[derive(Default)]
struct HostEditor;

impl Kind for HostEditor {
    fn spec() -> &'static skelter_core::KindSpec {
        &HOST_EDITOR
    }
    fn record(&self, cx: &mut skelter_core::RecordCx) {
        cx.draw_box();
    }
}

fn host_editor(reg: &mut KindRegistry) -> Result<(), String> {
    reg.register::<HostEditor>().map(|_| ()).map_err(|e| e.to_string())
}

fn source_notice(h: &Harness<GalleryDocument>) -> Option<String> {
    let g = &h.state().gallery;
    let d = g.document().borrow();
    let n = d.query().anchor("source-errors").first()?;
    match d.get_prop(n, skelter_core::PropId::of("label", "text", "string")) {
        Some(Value::Str(s)) => Some(s.to_string()),
        _ => None,
    }
}

#[test]
fn g2_a_hosts_code_editor_and_the_source_pane_work_in_one_app() {
    let mut h = gallery(Extensions::new().story_root(fixture("g2-host")).kinds(host_editor));
    {
        let g = &h.state().gallery;
        let mut d = g.document().borrow_mut();
        let reg = d.registry().clone();
        assert!(reg.by_name("code-editor").is_some_and(|k| std::ptr::eq(k.spec, &HOST_EDITOR)), "the host's kind keeps its tag");
        assert!(reg.by_name("gallery-source").is_some(), "the source pane's kind is there too");
        let host = d.create(skelter_core::KindId::of("code-editor"));
        assert_eq!(d.view(host).map(|v| v.kind().spec.name), Some("code-editor"), "the host's editor mounts");
        let src = d.query().anchor("source").first().expect("the source pane");
        assert_eq!(d.view(src).map(|v| v.kind().spec.name), Some("gallery-source"));
    }
    assert_eq!(source_notice(&h).as_deref(), Some(""), "no notice");
    h.get_by_author_id("story:button").click();
    h.run();
    let g = &h.state().gallery;
    let d = g.document().borrow();
    let src = d.query().anchor("source").first().unwrap();
    let buffer = d.get_prop(src, skelter_core::PropId::of("gallery-source", "buffer", "string"));
    assert!(matches!(buffer, Some(Value::Str(ref s)) if !s.is_empty()), "the pane shows the variant's source: {buffer:?}");
}

#[test]
fn g1_g6_the_apps_loop_runs_the_frame_and_backend_hooks() {
    let (frames, backends) = (Rc::new(Cell::new(0u64)), Rc::new(Cell::new(0u32)));
    let (f, b) = (frames.clone(), backends.clone());
    let ext = Extensions::new().story_root(fixture("g1-g6")).on_frame(move |cx| f.set(cx.frame)).on_backend(move |_| b.set(b.get() + 1));
    let mut h = gallery(ext);
    h.get_by_author_id("story:button").click();
    h.run();
    assert_eq!(backends.get(), 1, "one backend for the opened story, hooked before it loaded");
    assert!(frames.get() > 0, "frame hooks ran with the story open");
}

#[test]
fn g5_a_host_themes_tokens_come_from_its_base() {
    let themes = skelter_gallery_app::theme::Themes {
        extra: vec![("cad-light".into(), TokenMap::new()), ("cad-dark".into(), TokenMap::new())],
        bases: vec![("cad-light".into(), ColorScheme::Light, Contrast::Normal)],
    };
    let light = themes.get("cad-light", Density::Default).unwrap();
    assert!(!light.is_dark());
    assert_eq!(light.token("bg"), themes.get("light-modern", Density::Default).unwrap().token("bg"));
    assert!(themes.get("cad-dark", Density::Default).unwrap().is_dark());
}
