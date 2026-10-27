//! POL1-GALLERY items 1 and 2: the bundled fonts cover Arabic, CJK and symbols; the demo icon set
//! and a story's imported SVGs draw. Font coverage needs no GPU; the shot needs `SKELTER_GPU_TESTS=1`.

use skelter_gallery::fonts::{self, FontSet};
use skelter_gallery::{Cell, Extensions, Renderer};
use skelter_text::{RichText, TextStyle, TextSystem};
use skelter_widgets::paint::icon::registry::IconRegistry;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

/// The glyph ids `s` shapes to with the gallery's fonts, and the byte lengths of the faces used.
fn shape(text: &mut TextSystem, store: &skelter_text::FontStore, s: &str, family: &str) -> (Vec<u32>, Vec<usize>) {
    let style = TextStyle { family: family.into(), wrap: false, ..TextStyle::default() };
    let l = text.layout(&RichText::plain(s, style), None);
    let mut ids = Vec::new();
    let mut faces = Vec::new();
    for pr in l.runs() {
        ids.extend(pr.run.glyphs.iter().map(|g| u32::from(g.id)));
        faces.push(store.face(pr.run.font).expect("an interned face").data.data().len());
    }
    (ids, faces)
}

#[test]
fn the_bundled_fonts_cover_arabic_cjk_and_symbols() {
    let store = fonts::store(FontSet::Bundled);
    let mut text = TextSystem::new(store.clone());
    let theme_family = "Inter, system-ui, sans-serif";
    for (what, s) in [
        ("Arabic", "مرحبا بالعالم"),
        ("Hebrew", "שלום עולם"),
        ("Chinese", "文件 编辑 设置"),
        ("Japanese", "ファイル 日本語"),
        ("Korean", "파일 한국어"),
        ("symbols", "★ ⚙ ◀ ▶ ✎ ✂ ⌫ ⌘ ⋯ ✓"),
        ("Latin", "Project settings — 123"),
    ] {
        let (ids, _) = shape(&mut text, &store, s, theme_family);
        assert!(!ids.is_empty(), "{what}: nothing shaped");
        assert!(ids.iter().all(|&g| g != 0), "{what}: {s:?} has .notdef glyphs (tofu): {ids:?}");
    }
    // the theme's body font is Inter, its monospace JetBrains Mono: both bundled, both found by name
    let inter = fonts::BUNDLED[1].len();
    let (_, faces) = shape(&mut text, &store, "Settings", theme_family);
    assert_eq!(faces, [inter], "Latin draws with Inter Regular");
    let (_, faces) = shape(&mut text, &store, "Settings", "");
    assert_eq!(faces, [inter], "the default family is Inter");
    let (ids, faces) = shape(&mut text, &store, "make check", "\"JetBrains Mono\", monospace");
    assert!(ids.iter().all(|&g| g != 0));
    assert_eq!(faces, [fonts::BUNDLED[4].len()], "monospace draws with JetBrains Mono");
    // the symbol chain is the gallery's on every OS
    let chain: Vec<String> = store.symbol_chain().iter().map(|s| s.to_string()).collect();
    assert_eq!(chain, fonts::SYMBOL_CHAIN);
}

#[test]
fn the_demo_icons_and_story_assets_register() {
    skelter_gallery::icons::register_demo();
    let reg = IconRegistry::global();
    for (key, _) in skelter_gallery::icons::DEMO {
        assert!(reg.contains(key), "{key}");
        assert!(reg.resolve(key, 16.0, 1.0).is_some(), "{key} resolves to a master");
    }
    let root = fixture("media");
    let assets = svelters_host::modules::Assets::for_app(&root);
    let (ok, failed) = skelter_gallery::icons::register_assets(&assets, &root.join("stories/controls/media"));
    assert!(failed.is_empty(), "{failed:?}");
    assert_eq!(ok, ["asset://app/stories/controls/media/red.svg"]);
    assert!(skelter_widgets::controls::image::load(&ok[0]).is_ok(), "the ref loads as an image source");
}

#[test]
fn an_imported_svg_and_a_registry_icon_draw() {
    if std::env::var("SKELTER_GPU_TESTS").as_deref() != Ok("1") {
        eprintln!("skipped: set SKELTER_GPU_TESTS=1 to run GPU tests");
        return;
    }
    let mut r = Renderer::new(&Extensions::new().story_root(fixture("media"))).unwrap();
    let story = r.catalog().get("media").unwrap().clone();
    let svg = r.shot(&Cell::new(&story, story.variant("svg").unwrap())).unwrap();
    assert!(svg.warnings.is_empty(), "{:?}", svg.warnings);
    let red = svg.image.rgba.chunks_exact(4).filter(|p| p[0] > 230 && p[1] < 30 && p[2] < 30 && p[3] == 255).count();
    assert!(red >= 30 * 30, "the imported red square draws (found {red} red pixels); the missing-image placeholder has none");
    // a registry icon draws as its SVG, not its key as text: its pixels are the icon's, which
    // differ from the same cell with an unregistered key
    let icon = r.shot(&Cell::new(&story, story.variant("icon").unwrap())).unwrap();
    let bg = &icon.image.rgba[..4];
    let drawn = icon.image.rgba.chunks_exact(4).filter(|p| *p != bg).count();
    assert!(drawn > 40, "file.save draws ({drawn} pixels)");
}
