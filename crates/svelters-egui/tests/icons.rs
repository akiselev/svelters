//! ICR1 (plans/milestones/ICR1-icon-registry.md): the icon registry, its keys and namespaces,
//! ICON-STYLE's master choice and whole-pixel boxes, `color` tint and the colour slots (checked on
//! the raster), `content: icon()` and the button's icon part through the registry, and `<icon
//! name>` under kittest.
//!
//! The SVGs are IC1's build output of IC0's samples (`tests/icons/build`, see `icon_loader.rs`).

mod common;

use common::Rig;
use egui::Color32;
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use std::sync::Arc;
use svelters_abi::*;
use svelters_egui::icons::{self, Fit, Icon, IconError, IconRegistry, IconStyle, Owner, Slots};
use svelters_egui::paint::{self, IconKey, PaintCx, PaintTrace, StyleView, TraceKind, Ua};
use svelters_egui::provider::StateBits;

const WARNING_16: &[u8] = include_bytes!("icons/build/16/chrome.warning.svg");
const EXTRUDE_16: &[u8] = include_bytes!("icons/build/16/mcad.extrude.svg");
const EXTRUDE_20: &[u8] = include_bytes!("icons/build/20/mcad.extrude.svg");
const EXTRUDE_24: &[u8] = include_bytes!("icons/build/24/mcad.extrude.svg");

const GREY: Color32 = Color32::from_rgb(0xCC, 0xCC, 0xCC);
const YELLOW: Color32 = Color32::from_rgb(0xCC, 0xA7, 0x00);
const BLUE: Color32 = Color32::from_rgb(0x37, 0x94, 0xFF);

fn s(v: &str) -> Value {
    Value::Str(v.into())
}

fn extrude() -> Icon {
    Icon::svg(16, EXTRUDE_16).master(20, EXTRUDE_20).master(24, EXTRUDE_24)
}

fn registry() -> Arc<IconRegistry> {
    let r = IconRegistry::new();
    r.register(Owner::Host, "chrome.warning", Icon::svg(16, WARNING_16)).unwrap();
    r.register(Owner::Host, "mcad.extrude", extrude()).unwrap();
    r
}

/// Unmultiplied RGBA of every pixel.
fn pixels(img: &egui::ColorImage) -> Vec<[u8; 4]> {
    img.pixels.iter().map(|p| p.to_srgba_unmultiplied()).collect()
}

fn opaque(img: &egui::ColorImage) -> Vec<Color32> {
    pixels(img).into_iter().filter(|p| p[3] == 255).map(|[r, g, b, _]| Color32::from_rgb(r, g, b)).collect()
}

#[test]
fn keys_and_extension_namespaces() {
    let r = IconRegistry::new();
    let svg = || Icon::svg(16, WARNING_16);
    let _ = icons::take_diagnostics();
    assert!(r.register(Owner::Host, "file.save", svg()).is_ok());
    assert!(r.register(Owner::Host, "debug.step-over", svg()).is_ok());
    assert!(r.register(Owner::Host, "mcad.extrude+remove", svg()).is_ok());
    assert!(matches!(r.register(Owner::Host, "File.Save", svg()), Err(IconError::BadKey { .. })));
    assert!(matches!(r.register(Owner::Host, "file_save", svg()), Err(IconError::BadKey { .. })));
    assert!(matches!(r.register(Owner::Host, "ext.acme.gear", svg()), Err(IconError::Namespace { .. })), "ext.* is the extensions'");
    let acme = Owner::Extension("acme".into());
    assert!(r.register(acme.clone(), "ext.acme.gear", svg()).is_ok());
    assert!(matches!(r.register(acme.clone(), "ext.other.gear", svg()), Err(IconError::Namespace { .. })));
    assert!(matches!(r.register(acme.clone(), "file.save", svg()), Err(IconError::Namespace { .. })), "an extension cannot shadow a host key");
    assert!(matches!(r.register(Owner::Host, "x.empty", Icon::Svg { masters: Default::default(), hc: Default::default() }), Err(IconError::BadIcon { .. })));
    assert_eq!(r.keys(), ["debug.step-over", "ext.acme.gear", "file.save", "mcad.extrude+remove"]);
    // every refusal is a register-time diagnostic
    let d = icons::take_diagnostics();
    assert_eq!(d.len(), 6, "{d:?}");
    assert!(d[2].to_string().contains("ext.acme.gear"), "{}", d[2]);
    // an extension can remove only its own icons
    assert!(!r.unregister(&acme, "file.save"));
    assert_eq!(r.unregister_owner(&acme), 1);
    assert!(!r.contains("ext.acme.gear"));
}

#[test]
fn master_choice_by_physical_size() {
    let r = registry();
    let c = |pt: f32, ppp: f32| r.resolve("mcad.extrude", pt, ppp).map(|c| (c.master, c.px, c.fit));
    assert_eq!(c(16.0, 1.0), Some((16, 16, Fit::Exact)));
    assert_eq!(c(16.0, 1.25), Some((20, 20, Fit::Exact)), "the 20 master serves 1.25×");
    assert_eq!(c(16.0, 1.5), Some((24, 24, Fit::Exact)), "the 24 master takes over at 1.5×");
    assert_eq!(c(16.0, 2.0), Some((16, 32, Fit::Double)));
    assert_eq!(c(16.0, 1.1), Some((16, 16, Fit::Padded)));
    assert_eq!(r.resolve("chrome.warning", 24.0, 1.0).map(|c| (c.master, c.fit)), Some((16, Fit::Scaled)));
    assert_eq!(r.resolve("nope", 16.0, 1.0), None);
}

#[test]
fn tint_without_slots_and_real_colours_with_them() {
    // no slot: white, to be multiplied by `color`
    let img = icons::rasterize(WARNING_16, 16, GREY, &Slots::default()).unwrap();
    let o = opaque(&img);
    assert!(!o.is_empty() && o.iter().all(|c| *c == Color32::WHITE), "white for the tint");
    // accent set: the warning's own yellow, not yellow × grey
    let slots = Slots { accent: Some(YELLOW), ..Slots::default() };
    let img = icons::rasterize(WARNING_16, 16, GREY, &slots).unwrap();
    let o = opaque(&img);
    assert!(!o.is_empty() && o.iter().all(|c| *c == YELLOW), "every opaque pixel of the accent group is the slot colour: {o:?}");
    // the triangle's stroke is inherited from the root: it follows the slot too (`slot_svg`)
    let stroke_px = pixels(&img)[13 * 16 + 4]; // the base line at y 13.5
    assert_eq!(stroke_px, [0xCC, 0xA7, 0x00, 255], "the inherited stroke takes the slot");
}

#[test]
fn subject_slot_colours_the_tone_and_its_outline_only() {
    let slots = Slots { subject: Some(BLUE), ..Slots::default() };
    let img = icons::rasterize(EXTRUDE_16, 16, GREY, &slots).unwrap();
    let px = pixels(&img);
    let at = |x: usize, y: usize| px[y * 16 + x];
    // the arrow shaft (unclassed, x 7.5) keeps `color`
    assert_eq!(at(7, 4), [0xCC, 0xCC, 0xCC, 255], "primary ink keeps color");
    // inside the top face: the subject tone, blue at .4 (plus its outline's antialiasing)
    let face = at(4, 9);
    assert!(face[2] > face[0] + 100 && face[3] < 200, "subject tone is the slot colour, translucent: {face:?}");
    // the subject's own (inherited) outline on its top-right edge is blue too, not grey
    let edge = at(11, 8);
    assert!(edge[2] > edge[0] + 100, "the subject's outline takes the slot: {edge:?}");
    // the same icon without the slot is all white (tinted later)
    let img = icons::rasterize(EXTRUDE_16, 16, GREY, &Slots::default()).unwrap();
    assert!(pixels(&img).iter().filter(|p| p[3] > 0).all(|p| p[0] == 255 && p[1] == 255 && p[2] == 255));
    // slots come from CSS custom property text; anything but a colour is ignored
    assert_eq!(Slots::from_css(Some("#3794ff"), Some("var(--x)"), None), Slots { subject: Some(BLUE), ..Slots::default() });
    assert!(icons::slot_style_sheet(GREY, &slots).starts_with("svg{color:rgba(204,204,204,1.000)}.subject{color:rgba(55,148,255,1.000)}"));
}

#[test]
fn paint_snaps_the_box_to_physical_pixels() {
    let reg = registry();
    let ctx = egui::Context::default();
    let mut painted = Vec::new();
    for ppp in [1.0, 1.25, 1.5] {
        ctx.set_pixels_per_point(ppp);
        let mut out = ctx.run_ui(egui::RawInput::default(), |_| {});
        out.textures_delta.clear();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            assert_eq!(ui.painter().pixels_per_point(), ppp);
            // a rect at a fractional offset
            let rect = egui::Rect::from_min_size(egui::pos2(10.3, 7.7), egui::vec2(20.0, 20.0));
            painted.push((ppp, icons::paint(ui.painter(), &reg, "mcad.extrude", rect, &IconStyle::new(16.0, GREY)).unwrap()));
            assert!(icons::paint(ui.painter(), &reg, "nope", rect, &IconStyle::new(16.0, GREY)).is_none());
        });
        out.textures_delta.clear();
    }
    for (ppp, p) in painted {
        let c = p.choice.unwrap();
        let min = p.rect.min.to_vec2() * ppp;
        assert_eq!(min, min.round(), "Z1: the raster's origin is on a physical pixel at {ppp}");
        assert_eq!(p.rect.width() * ppp, c.px as f32, "painted 1:1 at {ppp}");
        assert_eq!(p.tint, GREY, "tinted by color");
        assert_eq!(c.master, match ppp { 1.0 => 16, 1.25 => 20, _ => 24 });
    }
    assert!(icons::take_diagnostics().contains(&IconError::Missing { key: "nope".into() }));
}

/// `content: icon(<key>)` and the slots in CSS resolve through the registry; the own button's icon
/// part draws from it.
#[test]
fn content_icon_and_button_icon_part_use_the_registry() {
    let mut rig = Rig::new();
    icons::set_registry(&rig.harness.ctx, registry());
    paint::trace::enable(&rig.harness.ctx, true);
    let (_w, col) = rig.window();
    let b = rig.leaf(col, "box", &[]);
    let btn = rig.leaf(col, "button", &[("text", s("Extrude")), ("icon", s("mcad.extrude"))]);
    rig.br().style(SVL_STYLE_CLASSES, b, Some("x")).unwrap();
    rig.br()
        .style(SVL_STYLE_SHEET, 0, Some(&format!("app{SVL_STYLE_SEP}.x {{ content: icon(chrome.warning); --icon-accent: #cca700; color: #cccccc; }}")))
        .unwrap();
    rig.run();
    let css = rig.backend.computed_style(b).expect("computed");
    let sv = StyleView::new(Some(&css), Ua::NONE, StateBits::NONE, true);
    assert_eq!(paint::content_icon(&sv, &IconKey::key("chrome.file")), Some(IconKey::key("chrome.warning")));
    assert_eq!(paint::icon_slots(&sv), Slots { accent: Some(YELLOW), ..Slots::default() });
    assert_eq!(paint::icon_size(&rig.harness.ctx, &sv, &IconKey::key("chrome.warning")), egui::Vec2::splat(sv.font_size()));
    // painting: the registry draws it, untinted because a slot is set
    let ctx = rig.harness.ctx.clone();
    let mut trace = PaintTrace::default();
    let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
        let painter = ui.painter().clone();
        let mut cx = PaintCx::new(&painter);
        cx.trace = Some(&mut trace);
        let r = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(32.0, 32.0));
        paint::paint_icon(&mut cx, &sv, &IconKey::key("chrome.file"), r);
    });
    out.textures_delta.clear();
    let e = trace.entries.iter().find(|e| e.kind == TraceKind::Icon).expect("an icon was painted");
    assert_eq!(e.fill, Color32::WHITE, "slots set: the raster carries the colours, no multiply");
    let textured = out.shapes.iter().any(|c| match &c.shape {
        egui::Shape::Rect(r) => r.brush.is_some(),
        egui::Shape::Mesh(m) => m.texture_id != egui::TextureId::default(),
        _ => false,
    });
    assert!(textured, "a textured rect, not a glyph");
    // the own button's icon part
    let t = paint::trace::last(&rig.harness.ctx);
    let node = rig.backend.with_scene(|s| s.n(btn).handle.to_raw());
    let icon = t.find(node, Some("icon"), TraceKind::Icon).expect("the button's icon part");
    assert_eq!(icon.rect.size(), egui::Vec2::splat(16.0), "16px, the icon part's UA size");
    assert_ne!(icon.fill, Color32::TRANSPARENT, "drawn from the registry, not the placeholder box");
}

/// `<icon name="…">` from the registry: square of `size` (else 16), an image to AccessKit, and an
/// unregistered name still takes the font path.
#[test]
fn icon_kind_draws_registry_keys() {
    let mut rig = Rig::new();
    icons::set_registry(&rig.harness.ctx, registry());
    let (_w, col) = rig.window();
    let a = rig.leaf(col, "icon", &[("name", s("mcad.extrude")), ("accessible-name", s("Extrude")), ("align-self", s("start"))]);
    let b = rig.leaf(col, "icon", &[("name", s("chrome.warning")), ("size", Value::F64(24.0)), ("align-self", s("start"))]);
    let c = rig.leaf(col, "icon", &[("name", s("chrome.warning")), ("frame", Value::Bool(true)), ("tooltip", s("Warn")), ("align-self", s("start"))]);
    rig.br().style(SVL_STYLE_CLASSES, b, Some("warn")).unwrap();
    rig.br().style(SVL_STYLE_SHEET, 0, Some(&format!("app{SVL_STYLE_SEP}.warn {{ --icon-accent: #cca700; }}"))).unwrap();
    rig.run();
    assert_eq!(rig.drawn(a).size(), egui::Vec2::splat(16.0));
    assert_eq!(rig.drawn(b).size(), egui::Vec2::splat(24.0));
    rig.harness.get_by_role_and_label(Role::Image, "Extrude");
    rig.harness.get_by_role_and_label(Role::Button, "Warn").click();
    rig.run();
    assert!(rig.drawn(c).width() >= 16.0, "a framed registry icon is an icon button");
    rig.run();
    let d = icons::take_diagnostics();
    assert!(!d.iter().any(|e| matches!(e, IconError::Missing { .. } | IconError::Load { .. })), "{d:?}");
    // renaming to a key outside the registry falls back to the font path (a placeholder box here)
    rig.set(a, "name", s("not.registered"));
    rig.run();
    assert!(rig.drawn(a).width() > 0.0);
}

/// Zooming through many levels keeps at most `RASTERS_PER_KEY` rasters of a key: the least
/// recently drawn (earlier zoom levels) are evicted.
#[test]
fn rasters_of_earlier_zoom_levels_are_evicted() {
    let reg = registry();
    let ctx = egui::Context::default();
    let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(40.0, 40.0));
    let zooms = [1.0, 1.1, 1.2, 1.25, 1.3, 1.4, 1.5, 1.6, 1.7, 1.8, 2.0];
    for ppp in zooms {
        ctx.set_pixels_per_point(ppp);
        let mut out = ctx.run_ui(egui::RawInput::default(), |_| {});
        out.textures_delta.clear();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            icons::paint(ui.painter(), &reg, "mcad.extrude", rect, &IconStyle::new(16.0, GREY)).unwrap();
        });
        out.textures_delta.clear();
        assert!(icons::cached_rasters(&ctx, "mcad.extrude") <= icons::RASTERS_PER_KEY);
    }
    assert_eq!(icons::cached_rasters(&ctx, "mcad.extrude"), icons::RASTERS_PER_KEY);
    assert_eq!(icons::cached_rasters(&ctx, "chrome.warning"), 0);
}
