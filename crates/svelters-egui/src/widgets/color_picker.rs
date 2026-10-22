//! `color-picker`, svelters' own (OW6; ARCH-WIDGETS §6.1, WIDGET-PARITY §2.20 `QColorDialog`): a
//! drop button showing the colour (`::part(swatch)`, a checkerboard under alpha) with R14's
//! chevron, opening the colour editor ([`ColorBody`], the `color-editor` kind's parts) in a popup
//! (`::part(popup)`, OW3's Anchored and Dismissable). `commit-mode`: `live` (the default, as the
//! egui wrapper: each edit changes `value`; closing commits) or `confirm` (Qt's dialog: edits
//! are pending until Save; Cancel or Escape drops them).
//!
//! States: `:hover`, `:active`, `:focus-visible`, `:open`, `:disabled`. Keys, closed: Enter,
//! Space, F4 or Alt+Down open it; open: Tab moves between the editor's sliders and boxes inside
//! the popup, Escape closes it and focus returns to the button. AccessKit: a `ColorWell` named
//! "Color" with the hex as its value, `expanded` and `has_popup`. Writer lane: OW6.

use super::color_editor::{self, COLOR_COMMIT_MODE, ColorBody, ColorCfg, HexColor};
use super::picker_glyphs::{self, Dir};
use super::picker_popup::{self, BUTTON_UA, CHEVRON_UA, Drop, Footer, POPUP_UA};
use crate::interact::{Focusable, Hoverable, Pressable};
use crate::paint::{self, TraceKind, Ua, UaRule, UaValue::*};
use crate::provider::{A11yCx, AvailableSpace, Event, MeasureCx, Part, StateBits, Widget, WidgetCx};
use egui::{Color32, Key, Modifiers, Rect, Stroke, Vec2, accesskit, pos2, vec2};

pub const UA: Ua = BUTTON_UA;

/// `::part(swatch)`: the colour on the closed button.
pub const SWATCH_UA: Ua = Ua(&[UaRule::base(&[("width", Px(28.0)), ("height", Px(16.0)), ("border-radius", Px(2.0)), ("border-color", Token("control-border")), ("border-width", Token("stroke"))])]);

/// A colour drop button with the colour editor in its popup.
// the `#[part]` fields name the kind's `::part()`s; nothing reads them (paint parts)
#[allow(dead_code)]
#[svelters::widget(namespace = "sv", replaces = "color-picker", caps = FOCUSABLE | PAINTS_BOX)]
pub struct ColorPicker {
    #[prop(bind)]
    value: HexColor,
    #[prop(default = "opaque", measure)]
    alpha: String,
    #[prop(default = "hex rgb")]
    fields: String,
    #[prop]
    swatches: Vec<String>,
    #[prop(bind)]
    custom_colors: Vec<String>,
    #[prop(default = "live")]
    commit_mode: String,
    #[event]
    changed: Event<String>,
    #[event]
    commit: Event<String>,
    #[behavior]
    hover: Hoverable,
    #[behavior]
    focus: Focusable,
    #[behavior]
    pressable: Pressable,
    #[state]
    body: ColorBody,
    #[state]
    drop: Drop,
    #[state]
    open: bool,
    /// `live`: an edit happened since it opened (closing commits it).
    #[state]
    edited: bool,
    #[part]
    swatch: Part,
    #[part]
    chevron: Part,
    #[part]
    popup: Part,
    #[part]
    square: Part,
    #[part]
    hue: Part,
    #[part(name = "alpha")]
    alpha_part: Part,
    #[part]
    handle: Part,
    #[part(name = "fields")]
    fields_part: Part,
    #[part]
    field: Part,
    #[part(name = "swatches")]
    swatches_part: Part,
    #[part(name = "swatch-cell")]
    swatch_cell: Part,
    #[part]
    save: Part,
    #[part]
    cancel: Part,
}

impl ColorPicker {
    fn live(&self) -> bool {
        super::pick(&self.commit_mode, COLOR_COMMIT_MODE, "live") == "live"
    }

    fn has_alpha(&self) -> bool {
        super::pick(&self.alpha, crate::kinds::pickers::ALPHA, "opaque") != "opaque"
    }

    fn set_open(&mut self, cx: &mut WidgetCx<'_>, ctx: &egui::Context, on: bool, keyboard: bool) {
        if on && !self.open {
            self.drop.opening(ctx, keyboard);
            self.edited = false;
        }
        if !on && self.open && self.live() && self.edited {
            // live: closing is the commit point of what was edited
            cx.dispatch(&self.commit, self.value.0.clone());
        }
        self.open = on;
        cx.set_state(StateBits::OPEN, on);
    }
}

impl Widget for ColorPicker {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, _avail: AvailableSpace) -> Vec2 {
        let s = cx.style(UA);
        let sw = s.part(cx.part_css("swatch"), SWATCH_UA);
        let c = s.part(cx.part_css("chevron"), CHEVRON_UA);
        let sz = vec2(sw.px("width").unwrap_or(28.0), sw.px("height").unwrap_or(16.0));
        let lh = s.px("line-height").unwrap_or(sz.y);
        vec2(sz.x + super::parts::gap(&s, 4.0) + c.px("width").unwrap_or(16.0), sz.y.max(lh)) + paint::box_extra(&s)
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        let ctx = ui.ctx().clone();
        self.drop.begin(&ctx);
        cx.set_state(StateBits::OPEN, self.open);
        let resp = cx.interact(ui, self.units_sense());
        let fired = self.run_units(cx, ui, &resp);
        let enabled = cx.enabled();
        let host_id = resp.id;
        if enabled && fired.pressable.iter().any(|p| p.is_click()) {
            let kb = ctx.input(|i| i.key_pressed(Key::Enter) || i.key_pressed(Key::Space));
            let on = !self.open;
            self.set_open(cx, &ctx, on, kb);
        } else if enabled && !self.open && resp.has_focus() && Drop::open_keys(&ctx) {
            self.set_open(cx, &ctx, true, true);
        }
        if !enabled && self.open {
            self.set_open(cx, &ctx, false, false);
        }
        if self.drop.just_opened(self.open) {
            self.body = ColorBody::default();
        }

        // ---- the popup ----
        if self.open && self.drop.hold(&ctx, host_id) {
            self.set_open(cx, &ctx, false, false);
            self.body = ColorBody::default();
        }
        if self.open {
            let live = self.live();
            let (hex, rgb, hsv) = ColorCfg::fields_of(&self.fields);
            let swatches = std::mem::take(&mut self.swatches);
            let custom = self.custom_colors.clone();
            let cfg = ColorCfg { alpha: self.has_alpha(), hex, rgb, hsv, swatches: &swatches, custom: &custom };
            let (frame, body_size) = {
                let host = cx.style(UA);
                let surf = cx.part(&host, "popup", POPUP_UA);
                let body_size = ColorBody::measure(&surf, &cfg);
                (Drop::frame(&ctx, &surf, resp.rect, picker_popup::popup_size(&surf, body_size, !live)), body_size)
            };
            if self.drop.dismissed(&ctx, frame, resp.rect) {
                self.set_open(cx, &ctx, false, false);
            } else {
                let pid = Drop::id(host_id);
                let (body, value, custom_colors, changed, commit) = (&mut self.body, &mut self.value, &mut self.custom_colors, &self.changed, &self.commit);
                let res = self.drop.show(&ctx, host_id, frame, |pui| {
                    let host = cx.style(UA);
                    let surf = cx.part(&host, "popup", POPUP_UA);
                    let geom = paint::box_geom(&surf, frame);
                    paint::paint_box(&mut cx.paint_cx(pui).part("popup"), &surf, frame);
                    let foot_h = picker_popup::footer_height(surf.is_dark());
                    let br = Rect::from_min_size(geom.content.min, body_size);
                    let fr = Rect::from_min_max(pos2(geom.content.left(), geom.content.bottom() - foot_h), geom.content.max);
                    let before = value.0.clone();
                    color_editor::run_body(body, pui, cx, POPUP_UA, &cfg, value, custom_colors, br, pid, live, changed, commit);
                    let foot = if live { None } else { picker_popup::footer(pui, cx, &cx.part(&cx.style(UA), "popup", POPUP_UA), pid, fr) };
                    (before != value.0, foot)
                });
                if let Some((edited, foot)) = res {
                    self.edited |= edited;
                    match foot {
                        Some(Footer::Save) => {
                            let mut h = self.body.hsv(self.value.rgba());
                            if !self.has_alpha() {
                                h.a = 1.0;
                            }
                            let hex = color_editor::color_hex(h.to_srgba());
                            self.set_open(cx, &ctx, false, false);
                            if hex != self.value.0 && !cx.dispatch(&self.changed, hex.clone()).prevented() {
                                cx.set(&mut self.value, HexColor(hex));
                            }
                            cx.dispatch(&self.commit, self.value.0.clone());
                        }
                        Some(Footer::Cancel) => {
                            self.set_open(cx, &ctx, false, false);
                            self.body = ColorBody::default();
                        }
                        None => {}
                    }
                }
            }
            self.swatches = swatches;
        }
        self.drop.end(&ctx, host_id, self.open);
        cx.set_state(StateBits::OPEN, self.open);
        cx.commit_state();

        // ---- the closed button ----
        let host = cx.style(UA);
        if host.state().contains(StateBits::HOVERED)
            && let Some(c) = host.cursor()
        {
            ctx.set_cursor_icon(c);
        }
        let hexv = self.value.0.clone();
        if ui.is_rect_visible(cx.rect()) {
            let g = cx.paint_box(ui, UA);
            let c = cx.part(&host, "chevron", CHEVRON_UA);
            let (cr, lr) = picker_popup::split_chevron(&host, &c, g.content);
            let sw = cx.part(&host, "swatch", SWATCH_UA);
            let sz = vec2(sw.px("width").unwrap_or(28.0).min(lr.width()), sw.px("height").unwrap_or(16.0).min(lr.height()));
            let sr = Rect::from_min_size(pos2(lr.left(), lr.center().y - sz.y / 2.0), sz);
            let mut pc = cx.paint_cx(ui);
            {
                let mut sp = pc.part("swatch");
                let [r, gg, b, a] = self.value.rgba();
                let rr = sp.snap_rect(sr);
                if a < 255 {
                    // the checkerboard under alpha (as the editor's strip)
                    let half = Rect::from_min_max(pos2(rr.center().x, rr.top()), rr.max);
                    sp.painter.rect_filled(rr, 0.0, Color32::from_gray(204));
                    sp.painter.rect_filled(Rect::from_min_max(rr.min, pos2(rr.center().x, rr.center().y)), 0.0, Color32::from_gray(128));
                    sp.painter.rect_filled(Rect::from_min_max(pos2(half.left(), rr.center().y), rr.max), 0.0, Color32::from_gray(128));
                }
                let col = Color32::from_rgba_unmultiplied(r, gg, b, a);
                sp.painter.rect_filled(rr, sw.radii()[0], col);
                sp.record(TraceKind::Box, rr, col, Stroke::NONE, 0.0);
                paint::paint_box(&mut sp, &sw.in_state(StateBits::NONE), sr);
            }
            picker_glyphs::chevron(&mut pc.part("chevron"), &c, Dir::Down, cr);
            cx.paint_outline(ui, UA);
        }
        let open = self.open;
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::ColorWell, enabled, "Color"));
        ctx.accesskit_node_builder(resp.id, |n| {
            n.set_value(hexv.clone());
            n.set_expanded(open);
            n.set_has_popup(accesskit::HasPopup::Dialog);
        });
        resp
    }

    fn a11y(&self, _cx: &A11yCx<'_>, node: &mut accesskit::Node) {
        node.set_role(accesskit::Role::ColorWell);
        self.units_a11y(node);
    }

    fn consume_key(&self, key: Key, m: Modifiers) -> bool {
        self.units_claims(key, m) || (!m.any() && key == Key::F4) || (m.alt && key == Key::ArrowDown) || (self.open && (key == Key::Escape || ColorBody::claims(key, m)))
    }
}
