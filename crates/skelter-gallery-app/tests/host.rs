//! A host's extensions in the app: its kind compiles and mounts in a live cell (spec fragment,
//! `Kinds::with_app`), its family and token set show, and inspect describes its node.

use skelter_base::accesskit::Role;
use skelter_base::change::ChangeClass;
use skelter_core::{Kind, KindId, KindRegistry, PropDefault, PropSpec, RecordCx, ValueType};
use skelter_gallery::Extensions;
use skelter_gallery_app::GalleryDocument;
use skelter_test::Harness;
use skelter_widgets::theme::TokenMap;
use std::path::Path;

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

#[test]
fn a_hosts_kind_family_and_theme_work_in_the_app() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/host");
    let mut tokens = TokenMap::new();
    tokens.insert("--sv-accent", "#ff00ff");
    let ext = Extensions::new()
        .story_root(&root)
        .family("host")
        .kinds(|reg: &mut KindRegistry| reg.register::<Swatch>().map(|_| ()).map_err(|e| e.to_string()))
        .theme("host-magenta", tokens);
    let fonts = skelter_text::FontStore::new(skelter_text::FontConfig::testing());
    let doc = skelter_gallery_app::app::document(ext, fonts).unwrap();
    let mut h = Harness::<GalleryDocument>::builder().with_size(skelter_base::geom::Size::new(1500.0, 950.0)).build(doc);
    h.run();
    h.get_by_author_id("story:swatch").click();
    h.run();
    assert!(h.state().gallery.live().unwrap().errors().is_empty(), "{:?}", h.state().gallery.live().unwrap().errors());
    // 2 states × the four built-in themes (the default), then the host's set too
    assert_eq!(h.state().gallery.cells().count(), 8);
    h.get_by_author_id("theme:host-magenta").click();
    h.run();
    let g = &h.state().gallery;
    assert_eq!(g.cells().count(), 10);
    let magenta = g.cells().find(|(c, _)| c.theme.name() == "host-magenta").unwrap().1;
    let d = g.document().borrow();
    let n = d.query().within(magenta).kind(KindId::of("gallery-swatch")).first().unwrap();
    assert_eq!(d.view(n).and_then(|v| v.css()?.custom("--sv-accent").map(str::to_string)).as_deref(), Some("#ff00ff"));
    drop(d);
    let d = g.document().borrow();
    for (c, frame) in g.cells() {
        assert!(d.query().within(frame).kind(KindId::of("gallery-swatch")).first().is_some(), "{} has its swatch", c.label());
    }
    drop(d);
    let j = g.inspect_live(0, "gallery-swatch");
    assert_eq!(j["nodes"][0]["kind"], "gallery-swatch", "{j:#}");
}
