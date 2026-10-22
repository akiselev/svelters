//! `icon` and `image`, svelters' own (OW1a2, ARCH-WIDGETS §6.1): an icon-font glyph or an AS1
//! image `source` (tinted with `color` when `tint`), and a fitted image. Replace the
//! `egui::Label`/`egui::Button` icon wrapper (`leaf.rs`) and the self-painted image
//! (`elements/media.rs`). ICR1's `IconRegistry` re-points `icon` later behind the same props.
//!
//! `icon`: with `frame` it is an icon button (the button's UA table: R1–R3 fills, R8 ring, R9
//! disabled, R10 pointer; focusable, role `Button`); without, an image (role `Image`, or `Button`
//! while `clicked` is listened). Named by `accessible-name`, else `tooltip`, else `name`. A family,
//! name or glyph the fonts cannot draw paints an outlined square (`fonts::icon_look` reports it).
//! `image`: `fit` maps to `object-fit` (`contain`, `cover`, `fill`, `none`, `scale-down`); a source
//! that is not loaded (yet, or an error) paints a crossed frame; clicks only while listened.

use super::button;
use crate::interact::{FocusParams, Focusable, Hoverable, Press, Pressable};
use crate::paint::{self, TraceKind, Ua, UaRule, UaValue::*};
use crate::provider::{A11yCx, AvailableSpace, Event, MeasureCx, StateBits, Widget, WidgetCx};
use egui::{Rect, Stroke, Vec2, pos2};

/// An unframed icon: its tint (the theme's `icon` colour) and the disabled look (R9).
pub const ICON_UA: Ua = Ua(&[
    UaRule::base(&[("color", Token("icon"))]),
    UaRule::when(StateBits::DISABLED, &[("opacity", Number(0.4)), ("cursor", Keyword("not-allowed"))]),
]);

/// The colour of a placeholder or a not-loaded image's frame.
pub const PLACEHOLDER_UA: Ua = Ua(&[UaRule::base(&[("color", Token("border-widget")), ("border-radius", Px(2.0))]), UaRule::when(StateBits::DISABLED, &[("opacity", Number(0.4))])]);

/// The clicks of a pressable host, as the button reports them: (clicked, double, secondary).
fn clicks(fired: &[Press]) -> (bool, bool, bool) {
    let mut c = (false, false, false);
    for p in fired {
        match p {
            Press::Click { count: 1, button: egui::PointerButton::Primary, .. } => c.0 = true,
            Press::Click { count: 2, button: egui::PointerButton::Primary, .. } => c.1 = true,
            Press::Click { button: egui::PointerButton::Secondary, .. } => c.2 = true,
            _ => {}
        }
    }
    c
}

/// An outlined square of `side` centred in `r` (an icon or image that cannot be drawn).
fn placeholder(pc: &mut paint::PaintCx<'_>, r: Rect, side: f32, color: egui::Color32, cross: bool) {
    let sq = if cross { r } else { Rect::from_center_size(r.center(), Vec2::splat(side)) };
    let stroke = Stroke::new(pc.snap_width(1.0), color);
    pc.painter.rect_stroke(sq, 1.0, stroke, egui::StrokeKind::Inside);
    let x = sq.shrink(3.0);
    if cross && x.is_positive() {
        pc.painter.line_segment([x.left_top(), x.right_bottom()], stroke);
        pc.painter.line_segment([x.right_top(), x.left_bottom()], stroke);
    }
    pc.record(TraceKind::Path, sq, egui::Color32::TRANSPARENT, stroke, 0.0);
}

/// `source`'s texture fitted in `r` by `fit`, tinted (`None`: not loaded).
fn paint_source(pc: &mut paint::PaintCx<'_>, ctx: &egui::Context, source: &str, fit: &str, r: Rect, tint: egui::Color32) -> bool {
    use egui::load::{SizeHint, TexturePoll};
    crate::elements::images::install(ctx);
    match ctx.try_load_texture(&crate::elements::images::uri_of(source), egui::TextureOptions::default(), SizeHint::default()) {
        Ok(TexturePoll::Ready { texture }) => {
            let (dest, uv) = crate::elements::media::fit_rect(fit, r, texture.size);
            pc.painter.with_clip_rect(pc.painter.clip_rect().intersect(r)).image(texture.id, dest, uv, tint);
            pc.record(TraceKind::Image, dest, tint, Stroke::NONE, 0.0);
            true
        }
        _ => false,
    }
}

/// An icon-font glyph or a small image (an icon button with `frame`).
#[svelters::widget(namespace = "sv", replaces = "icon", caps = FOCUSABLE | PAINTS_BOX)]
pub struct Icon {
    /// A name in the family's table.
    #[prop(measure)]
    name: String,
    /// The glyph itself, when `name` is empty or unknown.
    #[prop(measure)]
    glyph: String,
    /// The registered icon font.
    #[prop(measure)]
    family: String,
    /// Points (else the CSS `font-size`).
    #[prop(measure)]
    size: Option<f64>,
    #[prop(measure, default = 400)]
    font_weight: i64,
    /// AS1: an image instead of a glyph.
    #[prop(measure)]
    source: String,
    /// With `source`: tinted with the colour.
    #[prop(default = true)]
    tint: bool,
    /// An icon button.
    #[prop(measure)]
    frame: bool,
    /// With `frame`: the selected (toggle) look.
    #[prop]
    selected: bool,
    #[event]
    clicked: Event<()>,
    #[event]
    double_clicked: Event<()>,
    #[event]
    secondary_clicked: Event<()>,
    #[behavior]
    hover: Hoverable,
    #[behavior(params = focus_params)]
    focus: Focusable,
    #[behavior]
    press: Pressable,
    /// This frame's tooltip (its accessible name without `accessible-name`).
    tip: String,
    /// It takes clicks (framed, or listened).
    clickable: bool,
}

impl Icon {
    fn focus_params(&self) -> FocusParams {
        FocusParams { tab_stop: self.frame, ..FocusParams::default() }
    }
    fn ua(&self) -> Ua {
        if self.frame { button::UA } else { ICON_UA }
    }
    /// The glyph's side or text size under `s`.
    fn content(&self, ctx: &egui::Context, s: &paint::StyleView<'_>) -> (Vec2, Option<(egui::FontId, String)>) {
        let size = self.size.filter(|v| v.is_finite() && *v > 0.0).or(Some(f64::from(s.font_size())));
        if !self.source.is_empty() {
            return (Vec2::splat(size.unwrap_or(16.0) as f32), None);
        }
        match crate::fonts::icon_look(ctx, &ctx.global_style(), &self.family, &self.name, &self.glyph, self.font_weight, size) {
            crate::fonts::IconLook::Glyph(font, text) => {
                let g = ctx.fonts_mut(|f| f.layout_no_wrap(text.clone(), font.clone(), egui::Color32::WHITE));
                (g.size(), Some((font, text)))
            }
            crate::fonts::IconLook::Placeholder(side) => (Vec2::splat(side), None),
        }
    }
}

impl Widget for Icon {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, _avail: AvailableSpace) -> egui::Vec2 {
        let s = cx.style(self.ua());
        let (c, _) = self.content(cx.ctx, &s);
        if self.frame { c + paint::box_extra(&s) } else { c }
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        self.tip = cx.node().common().tooltip.clone();
        self.clickable = self.frame || cx.listens(&self.clicked) || cx.listens(&self.double_clicked) || cx.listens(&self.secondary_clicked);
        cx.set_state(StateBits::SELECTED, self.frame && self.selected);
        let sense = if self.clickable { self.units_sense() } else { egui::Sense::hover() };
        let resp = cx.interact(ui, sense);
        let fired = self.run_units(cx, ui, &resp);
        if self.clickable {
            let (c, d, s) = clicks(&fired.press.0);
            if c || crate::focus::take_activation(ui, cx.id()) {
                cx.dispatch(&self.clicked, ());
            }
            if d {
                cx.dispatch(&self.double_clicked, ());
            }
            if s {
                cx.dispatch(&self.secondary_clicked, ());
            }
        }
        let ua = self.ua();
        let host = cx.style(ua);
        if self.clickable
            && host.state().contains(StateBits::HOVERED)
            && let Some(c) = host.cursor()
        {
            ui.ctx().set_cursor_icon(c);
        }
        if host.opacity() < 1.0 {
            ui.multiply_opacity(host.opacity());
        }
        if ui.is_rect_visible(cx.rect()) {
            let content = if self.frame { cx.paint_box(ui, ua).content } else { cx.rect() };
            let host = cx.style(ua);
            let color = host.color("color").unwrap_or(egui::Color32::PLACEHOLDER);
            let (csize, glyph) = self.content(ui.ctx(), &host);
            let mut pc = cx.paint_cx(ui);
            let r = Rect::from_center_size(content.center(), csize.min(content.size().max(Vec2::ZERO)).max(Vec2::splat(1.0)));
            if !self.source.is_empty() {
                let tint = if self.tint { color } else { egui::Color32::WHITE };
                if !paint_source(&mut pc, ui.ctx(), &self.source, "contain", r, tint) && crate::elements::media::image_size(ui.ctx(), &self.source).is_none() {
                    let pv = paint::StyleView::ua_only(PLACEHOLDER_UA, host.state(), host.is_dark());
                    placeholder(&mut pc, r, r.width().min(r.height()), pv.color("color").unwrap_or(color), false);
                }
            } else if let Some((font, text)) = glyph {
                let g = ui.ctx().fonts_mut(|f| f.layout_no_wrap(text, font.clone(), color));
                let pos = pos2(((content.center().x - g.size().x / 2.0) * pc.ppp).round() / pc.ppp, ((content.center().y - g.size().y / 2.0) * pc.ppp).round() / pc.ppp);
                pc.painter.galley(pos, g.clone(), color);
                pc.record(TraceKind::Icon, Rect::from_min_size(pos, g.size()), color, Stroke::NONE, font.size);
            } else {
                placeholder(&mut pc, r, csize.x, color, false);
            }
            if self.frame {
                cx.paint_outline(ui, ua);
            }
        }
        let role = if self.clickable { egui::Role::Button } else { egui::Role::Image };
        let (enabled, selected) = (cx.enabled(), self.frame && self.selected);
        let name = if self.tip.is_empty() { self.name.clone() } else { self.tip.clone() };
        resp.widget_info(|| egui::WidgetInfo::selected(role, enabled, selected, &name));
        resp
    }

    fn a11y(&self, cx: &A11yCx<'_>, node: &mut egui::accesskit::Node) {
        node.set_role(if self.clickable { egui::accesskit::Role::Button } else { egui::accesskit::Role::Image });
        node.clear_value();
        if cx.name.is_none() {
            let name = if self.tip.is_empty() { &self.name } else { &self.tip };
            if !name.is_empty() {
                node.set_label(name.as_str());
            }
        }
        if self.clickable {
            self.units_a11y(node);
        }
    }

    fn consume_key(&self, key: egui::Key, m: egui::Modifiers) -> bool {
        self.frame && self.units_claims(key, m)
    }
}

/// An image from a `source` (a path, an asset or a URI), fitted by `fit`.
#[svelters::widget(namespace = "sv", replaces = "image")]
pub struct Image {
    #[prop(measure)]
    source: String,
    /// `object-fit`: `contain`, `cover`, `fill`, `none`, `scale-down`.
    #[prop(enumeration = "contain|cover|fill|none|scale-down", default = "contain".to_string())]
    fit: String,
    #[event]
    clicked: Event<()>,
    #[behavior]
    press: Pressable,
    /// The size it was measured at (a load that finishes later measures again).
    measured: Option<Vec2>,
}

impl Widget for Image {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, _avail: AvailableSpace) -> egui::Vec2 {
        let loaded = crate::elements::media::image_size(cx.ctx, &self.source);
        self.measured = loaded;
        match loaded {
            Some(s) => s,
            None if self.source.is_empty() => Vec2::ZERO,
            None => Vec2::splat(crate::elements::media::PLACEHOLDER),
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        let clickable = cx.listens(&self.clicked);
        let resp = cx.interact(ui, if clickable { self.units_sense() } else { egui::Sense::hover() });
        let fired = self.run_units(cx, ui, &resp);
        if clickable && clicks(&fired.press.0).0 {
            cx.dispatch(&self.clicked, ());
        }
        let r = cx.rect();
        let host = cx.style(PLACEHOLDER_UA);
        let loaded = crate::elements::media::image_size(ui.ctx(), &self.source);
        if ui.is_rect_visible(r) && !self.source.is_empty() {
            let mut pc = cx.paint_cx(ui);
            let tint = egui::Color32::WHITE.gamma_multiply(host.opacity());
            if !paint_source(&mut pc, ui.ctx(), &self.source, &self.fit, r, tint) && loaded.is_none() {
                placeholder(&mut pc, r, 0.0, host.color("color").unwrap_or(egui::Color32::GRAY), true);
            }
        }
        // measured before it loaded (an async loader): measure again
        if loaded.is_some() && loaded != self.measured {
            cx.request_measure();
        }
        let enabled = cx.enabled();
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Image, enabled, ""));
        resp
    }

    fn a11y(&self, _cx: &A11yCx<'_>, node: &mut egui::accesskit::Node) {
        node.set_role(egui::accesskit::Role::Image);
    }
}
