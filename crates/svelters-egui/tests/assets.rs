//! AS1 (plans/research/ASSETS.md §2.3, §3): the egui loaders for `asset://` refs and `data:` uris,
//! `icon source`, and an asset edit that reloads no JS. Headless.

mod common;

use common::Rig;
use egui::load::{BytesPoll, TexturePoll};
use egui::{TextureOptions, vec2};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use svelters_abi::*;
use svelters_modules::{AssetRef, Assets};

fn s(v: &str) -> Value {
    Value::Str(v.into())
}

fn svg(side: u32) -> String {
    format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="{side}" height="{side}" viewBox="0 0 16 16"><rect width="16" height="16" fill="currentColor"/></svg>"##)
}

/// A scratch app dir holding `icon.svg` of `side` points, its registry and the ref of the file.
fn app(name: &str, side: u32) -> (PathBuf, Arc<Assets>, AssetRef) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/as1").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let dir = dir.canonicalize().unwrap();
    std::fs::write(dir.join("icon.svg"), svg(side)).unwrap();
    let assets = Assets::for_app(&dir);
    let r = assets.register("app", &dir.join("icon.svg")).unwrap();
    (dir, assets, r)
}

fn texture_size(ctx: &egui::Context, uri: &str) -> Option<egui::Vec2> {
    match ctx.try_load_texture(uri, TextureOptions::default(), egui::load::SizeHint::default()) {
        Ok(TexturePoll::Ready { texture }) => Some(texture.size),
        _ => None,
    }
}

#[test]
fn bytes_loader_serves_asset_and_data_uris() {
    let (_dir, assets, r) = app("bytes", 16);
    let ctx = egui::Context::default();
    svelters_egui::assets::install(&ctx, assets.clone());
    svelters_egui::assets::install(&ctx, assets); // idempotent

    let BytesPoll::Ready { bytes, mime, .. } = ctx.try_load_bytes(r.as_uri()).unwrap() else { panic!("pending") };
    assert_eq!(&*bytes, svg(16).as_bytes());
    assert_eq!(mime.as_deref(), Some("image/svg+xml"));

    // "<svg/>" in base64, and plain percent-encoded text
    let BytesPoll::Ready { bytes, mime, .. } = ctx.try_load_bytes("data:image/svg+xml;base64,PHN2Zy8+").unwrap() else { panic!("pending") };
    assert_eq!((&*bytes, mime.as_deref()), (&b"<svg/>"[..], Some("image/svg+xml")));
    let BytesPoll::Ready { bytes, .. } = ctx.try_load_bytes("data:text/plain,a%20b").unwrap() else { panic!("pending") };
    assert_eq!(&*bytes, b"a b");

    assert!(ctx.try_load_bytes("asset://app/missing.svg").is_err());
    assert!(ctx.try_load_bytes("data:;base64,@@@").is_err());
    assert!(matches!(ctx.try_load_bytes("nope://x"), Err(egui::load::LoadError::NoMatchingBytesLoader)));
    // a data uri svg rasterises through our svg loader at its own size
    assert_eq!(texture_size(&ctx, "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIyMCIgaGVpZ2h0PSIxMCI+PC9zdmc+"), Some(vec2(20.0, 10.0)));
}

#[test]
fn svg_ignores_external_hrefs() {
    let (dir, assets, _r) = app("href", 16);
    std::fs::write(dir.join("secret.png"), b"not an image").unwrap();
    std::fs::write(
        dir.join("ref.svg"),
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="8" height="8"><image xlink:href="secret.png" width="8" height="8"/></svg>"#,
    )
    .unwrap();
    let r = assets.register("app", &dir.join("ref.svg")).unwrap();
    let ctx = egui::Context::default();
    svelters_egui::assets::install(&ctx, assets);
    assert_eq!(texture_size(&ctx, r.as_uri()), Some(vec2(8.0, 8.0)), "renders, the href is dropped");
}

#[test]
fn icon_with_a_source_is_measured_square_and_loads_the_asset() {
    let (_dir, assets, r) = app("icon", 16);
    let mut rig = Rig::new();
    svelters_egui::assets::install(&rig.harness.ctx, assets);
    let (_w, col) = rig.window();
    let a = rig.leaf(col, "icon", &[("source", s(r.as_uri())), ("size", Value::F64(20.0)), ("align-self", s("start"))]);
    let b = rig.leaf(col, "icon", &[("source", s(r.as_uri())), ("tint", Value::Bool(false)), ("align-self", s("start"))]);
    let missing = rig.leaf(col, "icon", &[("source", s("asset://app/none.svg")), ("size", Value::F64(12.0)), ("align-self", s("start"))]);
    rig.run();
    assert_eq!(rig.drawn(a).size(), vec2(20.0, 20.0));
    let body = egui::TextStyle::Body.resolve(&rig.harness.ctx.global_style()).size;
    assert_eq!(rig.drawn(b).size(), vec2(body, body), "no size: the body size");
    assert_eq!(rig.drawn(missing).size(), vec2(12.0, 12.0), "the placeholder keeps the square");
    assert_eq!(rig.get(a, "tint"), Value::Bool(true));
    assert_eq!(rig.get(b, "tint"), Value::Bool(false));
    assert_eq!(rig.get(a, "source"), s(r.as_uri()));
    assert_eq!(rig.get(rig.leaf(col, "icon", &[]), "source"), Value::Null);
    // the SVG was rasterised for the icon (a texture of the asset exists)
    assert!(texture_size(&rig.harness.ctx, r.as_uri()).is_some());
    rig.set(a, "source", Value::Null);
    rig.run();
    assert!(rig.drawn(a).size().x > 0.0, "a glyph icon again");
}

#[test]
fn changing_the_file_forgets_the_image_and_relayouts_with_no_js() {
    let (dir, assets, r) = app("change", 16);
    let mut rig = Rig::new();
    svelters_egui::assets::install(&rig.harness.ctx, assets.clone());
    let (_w, col) = rig.window();
    let img = rig.leaf(col, "image", &[("source", s(r.as_uri())), ("align-self", s("start"))]);
    rig.run();
    assert_eq!(rig.drawn(img).size(), vec2(16.0, 16.0));
    let v = assets.version(&r);

    let seen = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let seen2 = seen.clone();
    assets.on_change(move |_| {
        seen2.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }).forget();

    std::fs::write(dir.join("icon.svg"), svg(32)).unwrap();
    let refs = assets.changed(&[dir.join("icon.svg")]);
    assert_eq!(refs, vec![r.clone()]);
    assert!(assets.version(&r) > v);
    assert_eq!(seen.load(std::sync::atomic::Ordering::SeqCst), 1);
    rig.run();
    assert_eq!(rig.drawn(img).size(), vec2(32.0, 32.0), "the new file, measured in the same frames");
}

#[test]
fn texture_is_cached_by_version_and_size() {
    let (dir, assets, r) = app("texture", 16);
    let ctx = egui::Context::default();
    svelters_egui::assets::install(&ctx, assets.clone());
    let a = svelters_egui::assets::texture(&ctx, &r, vec2(16.0, 16.0)).unwrap();
    let b = svelters_egui::assets::texture(&ctx, &r, vec2(16.0, 16.0)).unwrap();
    assert_eq!(a.id(), b.id());
    assert_eq!(a.size(), [16, 16]);
    let c = svelters_egui::assets::texture(&ctx, &r, vec2(32.0, 32.0)).unwrap();
    assert_eq!(c.size(), [32, 32]);
    std::fs::write(dir.join("icon.svg"), svg(8)).unwrap();
    assets.changed(&[dir.join("icon.svg")]);
    let d = svelters_egui::assets::texture(&ctx, &r, vec2(16.0, 16.0)).unwrap();
    assert_ne!(a.id(), d.id(), "a new version, a new texture");
    let img = svelters_egui::assets::color_image(&assets, &r, [4, 4]).unwrap();
    assert_eq!(img.size, [4, 4]);
}

/// End to end from source: `import logo from './logo.svg'` in a component, `<image source={logo}>`;
/// editing the file reloads no JS (`hot == Some([])`) and the image has its new size.
#[test]
fn imported_svg_updates_on_edit_without_running_js() {
    use egui_kittest::kittest::Queryable;
    use svelters_egui::embed::{EmbedConfig, MountSpec, SlotOptions};
    use svelters_egui::harness::EmbedHarness;
    use svelters_host::HostConfig;
    use svelters_host::modules::{SourceConfig, SourceProvider};

    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/embed-egui").canonicalize().unwrap();
    assert!(fixture.join("node_modules").exists(), "run `pnpm install` first");
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/as1/e2e");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("package.json"), r#"{ "name": "as1", "private": true, "type": "module", "svelters": { "toolkit": "egui", "embed": { "Pic": "src/Pic.svelte" } } }"#).unwrap();
    std::fs::write(
        dir.join("src/Pic.svelte"),
        "<script>\n  import logo from './logo.svg';\n</script>\n<box>\n  <image source={logo} accessible-name=\"Pic\" align-self=\"start\" />\n  <icon source={logo} size={20} accessible-name=\"Ico\" />\n</box>\n",
    )
    .unwrap();
    std::fs::write(dir.join("src/logo.svg"), svg(16)).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(fixture.join("node_modules"), dir.join("node_modules")).unwrap();
    let dir = dir.canonicalize().unwrap();

    let mut cfg = SourceConfig::for_path(&dir).unwrap();
    cfg.dev = true;
    cfg.hmr = true;
    let provider = std::rc::Rc::new(SourceProvider::new(cfg).unwrap());
    let ecfg = EmbedConfig {
        id_salt: "as1",
        host: HostConfig { echo_console: false, echo_errors: false, quit_on_last_window: false, ..HostConfig::default() },
        ..EmbedConfig::default()
    };
    let mut eh = EmbedHarness::new(egui::vec2(400.0, 300.0), ecfg).unwrap();
    let e = eh.embed.clone();
    let entry = e.load_from(provider).unwrap();
    assert!(e.assets().is_some(), "the embed adopts its provider's registry");
    let slot = e.create_slot("a", SlotOptions::fill());
    eh.set_ui(move |e, ui| {
        e.show_slot(ui, slot);
    });
    eh.run();
    e.mount(MountSpec::new(entry, "Pic", slot, "p")).unwrap();
    eh.run();
    assert_eq!(eh.harness.get_by_label("Pic").rect().size(), vec2(16.0, 16.0));

    std::fs::write(dir.join("src/logo.svg"), svg(32)).unwrap();
    let report = e.host().apply_changes(vec![dir.join("src/logo.svg")]).expect("a reload ran");
    assert!(report.ok && report.hot == Some(vec![]), "{report:?}");
    assert_eq!(report.assets.len(), 1);
    eh.run();
    assert_eq!(eh.harness.get_by_label("Pic").rect().size(), vec2(32.0, 32.0));
}
