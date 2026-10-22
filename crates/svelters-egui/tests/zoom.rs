//! ZM1 (plans/milestones/ZM1-zoom.md): UI zoom. The layout stays in CSS px (points) while the
//! frame, the font atlas, SVG rasters and AccessKit bounds follow `pixels_per_point`; the zoom keys
//! are the runner's (steps of 1.2) or the embedder's. Headless.

mod common;

use common::Rig;
use egui::{Key, Modifiers, vec2};
use egui_kittest::kittest::{NodeT, Queryable};
use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use svelters_abi::*;
use svelters_egui::embed::EmbedConfig;
use svelters_egui::harness::EmbedHarness;
use svelters_egui::zoom::{self, ZoomKeys};
use svelters_host::HostConfig;

fn s(v: &str) -> Value {
    Value::Str(v.into())
}

fn close(a: f32, b: f32, eps: f32) -> bool {
    (a - b).abs() <= eps
}

/// The AccessKit bounds of the button labelled `label`, in physical pixels (the root transform
/// applied).
fn physical_bounds(rig: &Rig, label: &str) -> egui::Rect {
    let n = rig.harness.get_by_role_and_label(egui::accesskit::Role::Button, label);
    let b = n.accesskit_node().bounding_box().expect("bounds");
    egui::Rect::from_min_max(egui::pos2(b.x0 as f32, b.y0 as f32), egui::pos2(b.x1 as f32, b.y1 as f32))
}

fn clicks(rig: &Rig, h: svl_handle) -> usize {
    let sig = rig.signal_id(h, "clicked");
    rig.log.borrow().events.iter().filter(|(x, s, _)| *x == h && *s == sig).count()
}

#[test]
fn zoom_scales_physical_pixels_and_keeps_the_layout_in_css_px() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let btn = rig.leaf(col, "button", &[("text", s("Zoom me")), ("align-self", s("start"))]);
    rig.listen(btn, "clicked");
    rig.run();
    let ctx = rig.harness.ctx.clone();
    assert_eq!((ctx.zoom_factor(), ctx.pixels_per_point()), (1.0, 1.0));
    let pt1 = rig.drawn(btn);
    let px1 = physical_bounds(&rig, "Zoom me");
    let measured = rig.backend.scene().read(|s| s.stats().leaves_measured);

    rig.backend.set_zoom(1.5);
    assert_eq!(rig.backend.zoom(), 1.5, "the asked-for zoom, before egui applies it");
    rig.run();
    assert_eq!((ctx.zoom_factor(), ctx.pixels_per_point()), (1.5, 1.5));
    assert_eq!(rig.backend.zoom(), 1.5);

    // the layout is in CSS px (points): the button keeps its size (text snaps to the new pixel grid)
    let pt2 = rig.drawn(btn);
    assert!(close(pt2.width(), pt1.width(), 1.0) && close(pt2.height(), pt1.height(), 1.0), "{pt1:?} -> {pt2:?}");
    // in physical px it is 1.5 times as large, and AccessKit reports exactly the drawn rect
    let px2 = physical_bounds(&rig, "Zoom me");
    assert!(close(px2.width(), 1.5 * px1.width(), 1.5) && close(px2.height(), 1.5 * px1.height(), 1.5), "{px1:?} -> {px2:?}");
    assert!(close(px2.min.x, pt2.min.x * 1.5, 0.01) && close(px2.max.y, pt2.max.y * 1.5, 0.01), "AccessKit {px2:?} vs drawn {pt2:?} x 1.5");
    // the scale change remeasured the leaves once
    let remeasured = rig.backend.scene().read(|s| s.stats().leaves_measured) - measured;
    assert!(remeasured >= 1, "the leaves were remeasured at the new scale");
    rig.harness.step();
    assert_eq!(rig.backend.scene().read(|s| s.stats().leaves_measured) - measured, remeasured, "only once");

    // hit testing follows: a press at the drawn centre is a click on the button
    rig.click(btn);
    assert_eq!(clicks(&rig, btn), 1);

    // out of range and nonsense
    rig.backend.set_zoom(100.0);
    assert_eq!(rig.backend.zoom(), zoom::MAX_ZOOM);
    rig.backend.set_zoom(f32::NAN);
    rig.backend.set_zoom(0.0);
    assert_eq!(rig.backend.zoom(), zoom::MAX_ZOOM);
}

/// Every glyph of the last frame's text shapes showing `text`: (texel width in the font atlas,
/// screen width in points).
fn glyphs(rig: &Rig, text: &str) -> Vec<(char, f32, f32)> {
    fn walk(shape: &egui::Shape, text: &str, out: &mut Vec<(char, f32, f32)>) {
        match shape {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, text, out)),
            egui::Shape::Text(t) if t.galley.text() == text => {
                for row in &t.galley.rows {
                    for g in &row.glyphs {
                        if !g.uv_rect.is_nothing() {
                            out.push((g.chr, f32::from(g.uv_rect.max[0] - g.uv_rect.min[0]), g.uv_rect.size.x));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    for c in &rig.harness.output().shapes {
        walk(&c.shape, text, &mut out);
    }
    out
}

#[test]
fn text_is_rasterised_again_at_the_zoom() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    rig.leaf(col, "label", &[("text", s("Crisp W")), ("align-self", s("start"))]);
    rig.run();
    let at1 = glyphs(&rig, "Crisp W");
    assert!(!at1.is_empty(), "the label's galley is in the frame");
    rig.backend.set_zoom(1.5);
    rig.run();
    let at15 = glyphs(&rig, "Crisp W");
    assert_eq!(at1.len(), at15.len());
    for ((c, t1, w1), (_, t15, w15)) in at1.iter().zip(&at15) {
        // the atlas holds the glyph at 1.5 times as many texels (re-rasterised, not a scaled
        // texture), each texel one physical pixel; its size in points is about the same
        assert!(close(*t15, 1.5 * t1, 1.5), "{c:?}: {t1} texels at zoom 1, {t15} at 1.5");
        assert!(close(*t15, w15 * 1.5, 0.01), "{c:?}: one texel per physical pixel");
        assert!(close(*w15, *w1, 1.0), "{c:?}: {w1} pt -> {w15} pt");
    }
}

fn svg(side: u32) -> String {
    format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="{side}" height="{side}" viewBox="0 0 16 16"><rect width="16" height="16" fill="currentColor"/></svg>"##)
}

/// A scratch app dir holding `icon.svg`, its registry and the ref of the file.
fn app(name: &str) -> (PathBuf, Arc<svelters_modules::Assets>, svelters_modules::AssetRef) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/zm1").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let dir = dir.canonicalize().unwrap();
    std::fs::write(dir.join("icon.svg"), svg(16)).unwrap();
    let assets = svelters_modules::Assets::for_app(&dir);
    let r = assets.register("app", &dir.join("icon.svg")).unwrap();
    (dir, assets, r)
}

/// The sizes of the live textures named `uri`.
fn texture_sizes(ctx: &egui::Context, uri: &str) -> Vec<[usize; 2]> {
    let mut v: Vec<[usize; 2]> = ctx.tex_manager().read().allocated().filter(|(_, m)| m.name == uri).map(|(_, m)| m.size).collect();
    v.sort();
    v
}

#[test]
fn svg_icons_and_images_render_again_at_device_resolution() {
    let (_dir, assets, r) = app("svg");
    let data = format!("data:image/svg+xml,{}", svg(16).replace('#', "%23"));
    let mut rig = Rig::new();
    svelters_egui::assets::install(&rig.harness.ctx, assets);
    let (_w, col) = rig.window();
    rig.leaf(col, "icon", &[("source", s(r.as_uri())), ("size", Value::F64(20.0)), ("align-self", s("start"))]);
    rig.leaf(col, "icon", &[("source", s(&data)), ("size", Value::F64(20.0)), ("align-self", s("start"))]);
    let img = rig.leaf(col, "image", &[("source", s(r.as_uri())), ("align-self", s("start"))]);
    rig.run();
    let ctx = rig.harness.ctx.clone();
    assert_eq!(rig.drawn(img).size(), vec2(16.0, 16.0));
    assert_eq!(texture_sizes(&ctx, r.as_uri()), vec![[16, 16], [20, 20]], "the icon at 20 px, the image at 16");
    assert_eq!(texture_sizes(&ctx, &data), vec![[20, 20]]);

    rig.backend.set_zoom(1.5);
    rig.run();
    for _ in 0..3 {
        rig.harness.step();
    }
    assert_eq!(rig.drawn(img).size(), vec2(16.0, 16.0), "measured in points");
    // rendered again at 1.5 times the pixels; the rasters of the old scale are dropped
    assert_eq!(texture_sizes(&ctx, r.as_uri()), vec![[24, 24], [30, 30]]);
    assert_eq!(texture_sizes(&ctx, &data), vec![[30, 30]], "a data: svg too (egui keys only .svg uris on size)");

    // `assets::texture` (Rust panels) re-rasterises at the new scale and drops the old one
    let t = svelters_egui::assets::texture(&ctx, &r, vec2(10.0, 10.0)).unwrap();
    assert_eq!(t.size(), [15, 15]);
}

#[test]
fn the_runner_steps_zoom_by_1_2_on_ctrl_plus_minus_zero() {
    assert_eq!(EmbedConfig::app(HostConfig::default()).zoom_keys, ZoomKeys::Embed, "svelters-egui-run and TestApp");
    let cfg = EmbedConfig { id_salt: "zm1-run", zoom_keys: ZoomKeys::Embed, host: HostConfig { echo_console: false, echo_errors: false, quit_on_last_window: false, ..HostConfig::default() }, ..EmbedConfig::default() };
    let mut eh = EmbedHarness::new(vec2(300.0, 200.0), cfg).unwrap();
    let seen = Rc::new(Cell::new(0));
    let seen2 = seen.clone();
    eh.set_ui(move |_e, ui| {
        if ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Key { pressed: true, key: Key::Equals | Key::Minus | Key::Num0, .. }))) {
            seen2.set(seen2.get() + 1);
        }
    });
    eh.run();
    let ctx = eh.harness.ctx.clone();
    assert!(!ctx.options(|o| o.zoom_with_keyboard), "egui's own handler (steps of 0.1) is off");
    let press = |eh: &mut EmbedHarness, key: Key| {
        eh.harness.key_press_modifiers(Modifiers::COMMAND, key);
        eh.run();
    };
    press(&mut eh, Key::Equals);
    assert!(close(ctx.zoom_factor(), 1.2, 1e-5), "{}", ctx.zoom_factor());
    assert!(close(ctx.pixels_per_point(), 1.2, 1e-5));
    press(&mut eh, Key::Equals);
    assert!(close(eh.embed.zoom(), 1.44, 1e-5), "{}", eh.embed.zoom());
    press(&mut eh, Key::Minus);
    assert!(close(eh.embed.zoom(), 1.2, 1e-5));
    press(&mut eh, Key::Minus);
    press(&mut eh, Key::Minus);
    assert!(close(eh.embed.zoom(), 1.0 / 1.2, 1e-5), "{}", eh.embed.zoom());
    press(&mut eh, Key::Num0);
    assert_eq!(ctx.zoom_factor(), 1.0);
    assert_eq!(seen.get(), 0, "the embed consumed the zoom keys");
}

#[test]
fn an_embedder_owns_the_zoom_keys() {
    let cfg = EmbedConfig { id_salt: "zm1-host", host: HostConfig { echo_console: false, echo_errors: false, quit_on_last_window: false, ..HostConfig::default() }, ..EmbedConfig::default() };
    assert_eq!(cfg.zoom_keys, ZoomKeys::Host);
    let mut eh = EmbedHarness::new(vec2(300.0, 200.0), cfg).unwrap();
    let ctx = eh.harness.ctx.clone();
    assert!(!ctx.options(|o| o.zoom_with_keyboard), "Embed::new turns egui's own handler off");
    // the host's keymap (cadatomic): Ctrl+= reaches its ui and it calls `set_zoom` with its level
    let level = Rc::new(Cell::new(0));
    let level2 = level.clone();
    eh.set_ui(move |e, ui| {
        if ui.input_mut(|i| i.consume_shortcut(&egui::gui_zoom::kb_shortcuts::ZOOM_IN_SECONDARY)) {
            level2.set(level2.get() + 1);
            e.set_zoom(zoom::level_factor(level2.get()));
        }
    });
    eh.run();
    eh.harness.key_press_modifiers(Modifiers::COMMAND, Key::Equals);
    eh.run();
    assert_eq!(level.get(), 1, "Ctrl+= was not consumed by svelters or egui");
    assert!(close(ctx.zoom_factor(), 1.2, 1e-5), "{}", ctx.zoom_factor());
    eh.harness.key_press_modifiers(Modifiers::COMMAND, Key::Minus);
    eh.run();
    assert!(close(ctx.zoom_factor(), 1.2, 1e-5), "Ctrl+- is the host's too: nobody zoomed");
}
