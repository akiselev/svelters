//! OW1a: helpers shared by the toggles and indicators (checkbox, switch, toggle-button,
//! progress-bar, spinner): paint parts with their own UA table, and the path glyphs.
//!
//! **Paint parts.** A paint part (`checkbox::part(indicator)`, `::part(mark)`,
//! `switch::part(track)`, `progress-bar::part(fill)`, …) is painted with a [`StyleView`] over its
//! own UA table in the host's state. OW1a3: when a sheet has a `::part()` rule for it, the part's
//! computed style (OW-CSS stage 2) comes first ([`part_of`]); otherwise the part reads the
//! **host's** computed style for the properties the theme's base sheet sets on the host for that
//! part (the `checkbox` rule of `theme/base.css` styles the box), and its UA table for everything
//! else ([`part_view`]).
//!
//! **Glyphs.** ARCH-WIDGETS decides glyphs are icons from the registry (`content: icon(check)`,
//! WIDGET-PARITY P2). svelters-egui bundles no icon font today (ICR1 adds the registry), so the
//! check, dash and radio dot are drawn as paths here, sized from the part's box, and the switch
//! from P2 on will read `content` first. Writer lane: OW1a.

use crate::paint::{PaintCx, StyleView, TraceKind, Ua};
use crate::provider::{MeasureCx, WidgetCx};
use egui::{Color32, Rect, Stroke, pos2};

/// A part's view with no `::part()` style: the host's computed style (`host_css`: true for parts
/// the base sheet styles through the host rule) over the part's UA table, in the host's state.
pub(crate) fn part_view<'a>(host: &StyleView<'a>, ua: Ua, host_css: bool) -> StyleView<'a> {
    StyleView::new(if host_css { host.computed() } else { None }, ua, host.state(), host.is_dark())
}

/// OW1a3: part `name`'s view while drawing: its `::part(name)` computed style (an author or theme
/// rule; with its running transitions) over the UA table `ua` when a sheet has a rule for it,
/// else [`part_view`] (the UA table is the UA layer).
pub(crate) fn part_of<'a>(cx: &'a WidgetCx<'_>, host: &'a StyleView<'a>, name: &str, ua: Ua, host_css: bool) -> StyleView<'a> {
    if cx.part_css(name).is_some() { cx.part(host, name, ua) } else { part_view(host, ua, host_css) }
}

/// OW1a3: part `name`'s view from its computed style `css` (the scene-free cores' form of
/// [`part_of`]: no transitions), else [`part_view`].
pub(crate) fn part_with<'a>(host: &'a StyleView<'a>, css: Option<&'a svelters_style::ComputedStyle>, ua: Ua, host_css: bool) -> StyleView<'a> {
    match css {
        Some(c) => host.part(Some(c), ua),
        None => part_view(host, ua, host_css),
    }
}

/// OW1a3: [`part_of`] while measuring.
pub(crate) fn measured_part<'a>(cx: &'a MeasureCx<'_>, host: &'a StyleView<'a>, name: &str, ua: Ua, host_css: bool) -> StyleView<'a> {
    match cx.part_css(name) {
        Some(c) => host.part(Some(c), ua),
        None => part_view(host, ua, host_css),
    }
}

/// A part's size from its UA table (`width`, `height`), never the host's CSS size.
pub(crate) fn part_size(host: &StyleView<'_>, ua: Ua, default: f32) -> egui::Vec2 {
    let s = StyleView::ua_only(ua, host.state(), host.is_dark());
    egui::vec2(s.px("width").unwrap_or(default), s.px("height").unwrap_or(default))
}

/// The check mark (`indeterminate`: the dash) inside `r`, in `color`.
pub(crate) fn paint_mark(cx: &mut PaintCx<'_>, r: Rect, color: Color32, indeterminate: bool) {
    let w = cx.snap_width((r.width() / 9.0).max(1.0));
    let stroke = Stroke::new(w, color);
    let inner = r.shrink(r.width() * 0.22);
    if indeterminate {
        let y = (inner.center().y * cx.ppp).round() / cx.ppp;
        cx.painter.line_segment([pos2(inner.left(), y), pos2(inner.right(), y)], stroke);
    } else {
        let pts = vec![
            pos2(inner.left(), inner.center().y + inner.height() * 0.02),
            pos2(inner.left() + inner.width() * 0.38, inner.bottom() - inner.height() * 0.12),
            pos2(inner.right(), inner.top() + inner.height() * 0.12),
        ];
        cx.painter.add(egui::Shape::line(pts, stroke));
    }
    cx.record(TraceKind::Path, inner, color, stroke, 0.0);
}

/// A filled circle (the radio dot, the switch knob).
pub(crate) fn paint_dot(cx: &mut PaintCx<'_>, center: egui::Pos2, radius: f32, fill: Color32, stroke: Stroke) {
    cx.painter.circle(center, radius, fill, stroke);
    cx.record(TraceKind::Path, Rect::from_center_size(center, egui::Vec2::splat(2.0 * radius)), fill, stroke, 0.0);
}

/// A filled rounded rect painted as a part (a bar's fill), traced as a path.
pub(crate) fn paint_fill(cx: &mut PaintCx<'_>, r: Rect, radius: f32, fill: Color32) {
    cx.painter.rect_filled(r, radius, fill);
    cx.record(TraceKind::Path, r, fill, Stroke::NONE, 0.0);
}

/// `prefers-reduced-motion`: animations (the busy sweep, the spinner, the knob) are static.
pub(crate) fn reduced_motion(ctx: &egui::Context) -> bool {
    ctx.global_style().animation_time <= 0.0
}

/// A chevron in `r` (DESIGN R14: what opens something shows one): pointing right, or down when
/// `open`. A path until ICR1 ships the codicon (`chevron-right`); a part's `content: icon(..)`
/// replaces it (P2).
pub(crate) fn paint_chevron(cx: &mut PaintCx<'_>, r: Rect, color: Color32, open: bool) {
    let w = cx.snap_width((r.width() / 12.0).max(1.0));
    let stroke = Stroke::new(w, color);
    let c = r.center();
    let h = r.width().min(r.height()) * 0.2;
    let pts = if open {
        vec![pos2(c.x - 2.0 * h, c.y - h), pos2(c.x, c.y + h), pos2(c.x + 2.0 * h, c.y - h)]
    } else {
        vec![pos2(c.x - h, c.y - 2.0 * h), pos2(c.x + h, c.y), pos2(c.x - h, c.y + 2.0 * h)]
    };
    cx.painter.add(egui::Shape::line(pts, stroke));
    cx.record(TraceKind::Path, r, color, stroke, 0.0);
}
