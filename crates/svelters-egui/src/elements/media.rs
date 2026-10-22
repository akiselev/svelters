//! Paint and measure of `image`, `progress-bar`, `spinner` and `canvas`.

use super::{Painted, finite_or, images};
use crate::kinds;
use crate::layout::Metrics;
use crate::scene::{Arena, W};
use crate::walk::Out;
use egui::load::{SizeHint, TexturePoll};
use egui::{Rect, Role, Sense, TextureOptions, Ui, Vec2, WidgetInfo, pos2, vec2};
use svelters_abi::Value;

/// The side of the square an `image` whose source did not load is measured at (points).
pub(crate) const PLACEHOLDER: f32 = 16.0;

/// The loaded size of an image source, in points (`None`: not loaded, or no source).
pub(crate) fn image_size(ctx: &egui::Context, source: &str) -> Option<Vec2> {
    if source.is_empty() {
        return None;
    }
    images::install(ctx);
    match ctx.try_load_texture(&images::uri_of(source), TextureOptions::default(), SizeHint::default()) {
        Ok(TexturePoll::Ready { texture }) => Some(texture.size),
        _ => None,
    }
}

pub(super) fn intrinsic(m: &Metrics, w: &W) -> Vec2 {
    let sp = &m.style.spacing;
    match w {
        // not loaded (missing file, unknown uri, not decoded yet): the placeholder's size
        W::Image { source, .. } if source.is_empty() => Vec2::ZERO,
        W::Image { source, .. } => image_size(&m.ctx, source).unwrap_or(Vec2::splat(PLACEHOLDER)),
        W::ProgressBar { .. } => vec2(sp.slider_width, sp.interact_size.y),
        W::Spinner { size, .. } => Vec2::splat(size.map_or(sp.interact_size.y, |s| s as f32)),
        W::Canvas(c) => vec2(c.content_w as f32, c.content_h as f32),
        _ => Vec2::ZERO,
    }
}

/// Where an image of `img` size is drawn in `r` for `fit`, and the part of it shown (uv).
pub fn fit_rect(fit: &str, r: Rect, img: Vec2) -> (Rect, Rect) {
    let full = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    if img.x <= 0.0 || img.y <= 0.0 || r.width() <= 0.0 || r.height() <= 0.0 {
        return (r, full);
    }
    let contain = (r.width() / img.x).min(r.height() / img.y);
    let scale = match fit {
        "fill" => return (r, full),
        "none" => 1.0,
        "scale-down" => contain.min(1.0),
        "cover" => {
            // fill the rect, crop the image around its centre
            let s = (r.width() / img.x).max(r.height() / img.y);
            let shown = vec2(r.width() / (img.x * s), r.height() / (img.y * s));
            let uv = Rect::from_center_size(pos2(0.5, 0.5), shown);
            return (r, uv);
        }
        _ => contain,
    };
    (Rect::from_center_size(r.center(), img * scale), full)
}

#[allow(clippy::disallowed_types)] // OW: wrapper until OW1a
pub(super) fn paint(arena: &mut Arena, i: usize, ui: &mut Ui, out: &mut Vec<Out>, click_sense: bool) -> Painted {
    let r = ui.max_rect();
    let enabled = arena.common(i).enabled;
    let sense = if click_sense { Sense::click() } else { Sense::hover() };
    let resp = match arena.w(i) {
        W::Image { source, fit } => {
            let (source, fit) = (source.clone(), *fit);
            image(arena, i, ui, r, &source, fit, sense, enabled)
        }
        W::ProgressBar { fraction, text, show_percentage, animate } => {
            // NaN draws empty and +inf full (egui's own clamp keeps a NaN)
            let mut bar = egui::ProgressBar::new(finite_or(*fraction, if *fraction > 0.0 { 1.0 } else { 0.0 }).clamp(0.0, 1.0) as f32).desired_width(r.width()).desired_height(r.height()).animate(*animate);
            if !text.is_empty() {
                bar = bar.text(text.as_str());
            } else if *show_percentage {
                bar = bar.show_percentage();
            }
            ui.add(bar)
        }
        W::Spinner { spinning, size } => {
            let d = size.map_or(ui.spacing().interact_size.y, |s| s as f32).min(r.width().min(r.height()).max(1.0));
            let busy = *spinning;
            let resp = if busy { ui.add(egui::Spinner::new().size(d)) } else { ui.allocate_response(Vec2::splat(d), Sense::hover()) };
            resp.widget_info(|| WidgetInfo::labeled(Role::ProgressIndicator, enabled, ""));
            ui.ctx().accesskit_node_builder(resp.id, |b| {
                if busy {
                    b.set_busy();
                }
            });
            resp
        }
        W::Canvas(_) => canvas(arena, i, ui, r, sense, enabled, out),
        _ => unreachable!("not a media kind"),
    };
    Painted { resp }
}

#[allow(clippy::too_many_arguments)]
fn image(arena: &mut Arena, i: usize, ui: &mut Ui, r: Rect, source: &str, fit: &str, sense: Sense, enabled: bool) -> egui::Response {
    let resp = ui.allocate_rect(r, sense);
    let loaded = image_size(ui.ctx(), source);
    if let Some(size) = loaded {
        let (dest, uv) = fit_rect(fit, r, size);
        if let Ok(TexturePoll::Ready { texture }) = ui.ctx().try_load_texture(&images::uri_of(source), TextureOptions::default(), SizeHint::default()) {
            let tint = if enabled { egui::Color32::WHITE } else { egui::Color32::from_gray(128) };
            ui.painter().with_clip_rect(r.intersect(ui.clip_rect())).image(texture.id, dest, uv, tint);
        }
    } else if !source.is_empty() && ui.is_rect_visible(r) {
        // not loaded (yet, or a load error): a crossed-out frame
        let p = ui.painter().with_clip_rect(r.intersect(ui.clip_rect()));
        let stroke = ui.visuals().widgets.noninteractive.bg_stroke;
        p.rect_stroke(r, 2.0, stroke, egui::StrokeKind::Inside);
        let x = r.shrink(3.0);
        if x.is_positive() {
            p.line_segment([x.left_top(), x.right_bottom()], stroke);
            p.line_segment([x.right_top(), x.left_bottom()], stroke);
        }
    }
    resp.widget_info(|| WidgetInfo::labeled(Role::Image, enabled, ""));
    // measured before it loaded (an async loader): measure again
    if let Some(size) = loaded
        && arena.layout[i].intrinsic.is_some_and(|m| m != size)
    {
        arena.layout[i].intrinsic = None;
        let node = arena.slots.id_at(i).expect("live leaf");
        arena.invalidate_content(node);
        ui.ctx().request_repaint();
    }
    resp
}

fn canvas(arena: &mut Arena, i: usize, ui: &mut Ui, r: Rect, sense: Sense, enabled: bool, out: &mut Vec<Out>) -> egui::Response {
    let resp = ui.allocate_rect(r, sense);
    let W::Canvas(c) = arena.w(i) else { unreachable!() };
    if ui.is_rect_visible(r) {
        let clear = ui.visuals().panel_fill;
        for (clip, shape) in super::canvas::shapes(ui.ctx(), r.min, r.intersect(ui.clip_rect()), &c.list, clear) {
            ui.painter().with_clip_rect(clip).add(shape);
        }
    }
    resp.widget_info(|| WidgetInfo::labeled(Role::Canvas, enabled, ""));
    // `resize` after a layout that changed its size (and after the first)
    let size = (r.width(), r.height());
    if c.reported != Some(size) {
        if let W::Canvas(c) = arena.w_mut(i) {
            c.reported = Some(size);
        }
        let h = arena.slots.id_at(i).expect("live leaf").to_raw();
        let s = kinds::signal_id(arena.core[i].kind as usize, "resize");
        if arena.listens(i, s) {
            out.push(Out::Event(h, s, vec![Value::F64(size.0 as f64), Value::F64(size.1 as f64)]));
        }
    }
    resp
}
