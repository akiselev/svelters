//! `hyperlink`, svelters' own (OW1a, ARCH-WIDGETS §6.1): text; Hoverable, Focusable, Pressable.
//! Replaces the `egui::Link`/`Hyperlink` wrapper behind the own-widget switch.
//!
//! Look (DESIGN R18, R10): the `link` colour, underlined while hovered, the pointing-hand cursor,
//! and the keyboard-only focus ring (R8). A click opens `url` (through egui's `OpenUrl`, which the
//! host routes to `svelters:shell`) and sends `clicked`; with no `url` it only sends `clicked`.

use crate::interact::{Focusable, Hoverable, Pressable};
use crate::paint::{self, Align, Ua, UaRule, UaValue::*};
use crate::provider::{A11yCx, AvailableSpace, Event, MeasureCx, StateBits, Widget, WidgetCx};

pub const UA: Ua = Ua(&[
    UaRule::base(&[
        ("color", Token("link")),
        ("outline-style", Keyword("none")),
        ("outline-color", Token("focus")),
        ("outline-width", Px(1.0)),
        ("outline-offset", Px(1.0)),
        ("text-decoration-line", Keyword("none")),
    ]),
    UaRule::when(StateBits::HOVERED, &[("text-decoration-line", Keyword("underline"))]),
    UaRule::when(StateBits::FOCUS_VISIBLE, &[("outline-style", Keyword("solid"))]),
    UaRule::when(StateBits::DISABLED, &[("color", Token("disabled-fg"))]),
]);

/// A clickable link.
#[svelters::widget(namespace = "sv", replaces = "hyperlink", caps = FOCUSABLE)]
pub struct Hyperlink {
    /// The shown text (the `url` when empty).
    #[prop(measure)]
    text: String,
    #[prop(measure)]
    url: String,
    /// Opens in a new tab or window where the host has them.
    #[prop]
    new_tab: bool,
    #[event]
    clicked: Event<()>,
    #[event]
    secondary_clicked: Event<()>,
    #[behavior]
    hover: Hoverable,
    #[behavior]
    focus: Focusable,
    #[behavior]
    press: Pressable,
}

impl Hyperlink {
    fn shown(&self) -> &str {
        if self.text.is_empty() { &self.url } else { &self.text }
    }
}

impl Widget for Hyperlink {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, _avail: AvailableSpace) -> egui::Vec2 {
        paint::measure_text(cx.ctx, &cx.style(UA), self.shown(), AvailableSpace::MaxContent)
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        let resp = cx.interact(ui, self.units_sense());
        let fired = self.run_units(cx, ui, &resp);
        for p in fired.press.iter() {
            match p {
                p if p.is_click() => {
                    if !cx.dispatch(&self.clicked, ()).prevented && !self.url.is_empty() {
                        ui.ctx().open_url(egui::OpenUrl { url: self.url.clone(), new_tab: self.new_tab || p_mods_new_tab(p) });
                    }
                }
                crate::interact::Press::Click { button: egui::PointerButton::Secondary, .. } => {
                    cx.dispatch(&self.secondary_clicked, ());
                }
                _ => {}
            }
        }
        if resp.hovered() && cx.enabled() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if ui.is_rect_visible(cx.rect()) {
            let s = cx.style(UA);
            let g = paint::text_galley(ui.ctx(), &s, self.shown(), None);
            let mut pc = cx.paint_cx(ui);
            paint::paint_text(&mut pc, &s, &g, cx.rect(), Align::Start);
            if s.keyword("text-decoration-line") == Some("underline") {
                let color = s.color("color").unwrap_or(egui::Color32::PLACEHOLDER);
                let r = cx.rect();
                let x0 = r.left();
                let y = r.center().y + g.size().y / 2.0 - 1.0;
                let w = pc.snap_width(1.0);
                pc.painter.line_segment([egui::pos2(x0, y), egui::pos2(x0 + g.size().x, y)], egui::Stroke::new(w, color));
            }
            paint::paint_outline(&mut pc, &s, cx.rect());
        }
        let (enabled, text) = (cx.enabled(), self.shown().to_string());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Link, enabled, &text));
        resp
    }

    fn a11y(&self, _cx: &A11yCx<'_>, node: &mut egui::accesskit::Node) {
        node.set_role(egui::accesskit::Role::Link);
        if !self.url.is_empty() {
            node.set_url(self.url.as_str());
        }
        self.units_a11y(node);
    }

    fn consume_key(&self, key: egui::Key, m: egui::Modifiers) -> bool {
        self.units_claims(key, m)
    }
}

/// Ctrl/Cmd-click opens in a new tab (as egui's `Hyperlink` and browsers do).
fn p_mods_new_tab(p: &crate::interact::Press) -> bool {
    matches!(p, crate::interact::Press::Click { mods, .. } if mods.command)
}
