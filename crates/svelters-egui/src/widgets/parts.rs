//! OW1a: helpers shared by the toggles and indicators (checkbox, switch, toggle-button,
//! progress-bar, spinner): paint parts with their own UA table, and the path glyphs.
//!
//! **Paint parts until OW-CSS lands `::part()`.** A paint part (`checkbox::part(indicator)`,
//! `::part(mark)`, `switch::part(track)`, `progress-bar::part(fill)`, …) is painted with a
//! [`StyleView`] over its own UA table in the host's state. Until the engine computes part styles,
//! a part reads the **host's** computed style for the properties the theme's base sheet already
//! sets on the host for that part (the `checkbox` rule of `theme/base.css` styles the box), and
//! its UA table for everything else ([`part_view`]). When OW-CSS lands, `part_view` takes the
//! part's computed style instead and no widget changes.
//!
//! **Glyphs.** ARCH-WIDGETS decides glyphs are icons from the registry (`content: icon(check)`,
//! WIDGET-PARITY P2). svelters-egui bundles no icon font today (ICR1 adds the registry), so the
//! check, dash and radio dot are drawn as paths here, sized from the part's box, and the switch
//! from P2 on will read `content` first. Writer lane: OW1a.

use crate::paint::{PaintCx, StyleView, TraceKind, Ua};
use egui::{Color32, Rect, Stroke, pos2};

/// A part's view: the host's computed style (`host_css`: true for parts the base sheet styles
/// through the host rule) over the part's UA table, in the host's state.
pub(crate) fn part_view<'a>(host: &StyleView<'a>, ua: Ua, host_css: bool) -> StyleView<'a> {
    StyleView::new(if host_css { host.computed() } else { None }, ua, host.state(), host.is_dark())
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

/// The host's `gap` (between an indicator and the caption).
pub(crate) fn gap(s: &StyleView<'_>, default: f32) -> f32 {
    s.px("gap").or_else(|| s.px("column-gap")).unwrap_or(default)
}

/// `prefers-reduced-motion`: animations (the busy sweep, the spinner, the knob) are static.
pub(crate) fn reduced_motion(ctx: &egui::Context) -> bool {
    ctx.global_style().animation_time <= 0.0
}
