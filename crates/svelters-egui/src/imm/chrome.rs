//! Paint-only chrome, for a foreign painter that owns ids, senses, input, focus and AccessKit
//! (berth's `ChromePainter`, BD7): berth builds every `Response` itself (tabs, strips, and its own
//! buttons: tab close, scroll arrows, group close and float, region pin and collapse, float dock
//! and close, window maximize and close) and calls the painter with it.
//!
//! - [`state`]: the state bits, from the same units run on **the caller's response**, observing
//!   only (`interact::observe`: no focus request, no Escape rule): `:hover`, `:active`, `:focus`,
//!   `:focus-visible` (keyboard-only, R8) as a Svelte widget gets them; `:disabled` when the caller
//!   says so (`props` holds `StateBits::DISABLED`: berth's non-closable close button, hover-only
//!   on an enabled `Ui`), still hoverable (R9).
//! - [`Sv::chrome`]: those bits and the cascade for a virtual element ([`El`](super::El)), as a
//!   [`Styled`].
//! - [`surface`], [`ring`]: box and focus ring; [`icon`]: an icon by key in a rect; [`label`]:
//!   text laid out from the style; [`galley`]: a galley the caller laid out (berth's `tab_font`),
//!   painted in the style's colour; [`text_color`] and [`font`] for the caller's layout.
//!
//! Every shape is traced under the caller's id (PaintTrace, debug builds), as `imm` widgets.

use super::{Styled, Sv};
use crate::interact::{FocusParams, Focusable, Hoverable, Intents, PartResponses, PressParams, Pressable, Unit};
use crate::paint::{self, Align, BoxGeom, PaintCx, TraceKind};
use crate::provider::StateBits;
use std::sync::{Arc, Mutex};

/// The UA table of a chrome button (berth's close, pin, float, …): the push button's (R1–R3, R8
/// ring at +1, R9, R10). A frameless chrome button sets `background-color`/`border` in CSS, or
/// paints only [`icon`] and [`ring`].
pub const BUTTON_UA: crate::paint::Ua = crate::widgets::button::UA;

#[derive(Debug, Default)]
struct Units {
    hover: Hoverable,
    focus: Focusable,
    press: Pressable,
}

/// The state bits of a chrome element whose response the caller made (`resp`, any sense):
/// Hoverable, Pressable and (when `resp` senses focus) Focusable observe it. `props` are the
/// caller's mirrored bits (`SELECTED` for the active tab, `CURRENT`, `DROP_TARGET`, `DRAGGING`,
/// and `DISABLED` for a control that is disabled while its `Ui` is not). The units keep their
/// state in egui memory under `resp.id`. Nothing in egui changes.
pub fn state(ui: &egui::Ui, resp: &egui::Response, props: StateBits) -> StateBits {
    let units = ui.ctx().data_mut(|d| d.get_temp_mut_or_insert_with(resp.id.with("svelters-imm-chrome"), || Arc::new(Mutex::new((Units::default(), StateBits::NONE)))).clone());
    let mut g = units.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let (u, state) = &mut *g;
    let keep = StateBits(Hoverable::OWNS.0 | Focusable::OWNS.0 | Pressable::OWNS.0);
    let enabled = ui.is_enabled() && !props.contains(StateBits::DISABLED);
    *state = StateBits(state.0 & keep.0) | props;
    state.set(StateBits::DISABLED, !enabled);
    let focusable = resp.sense.is_focusable();
    crate::interact::observe(ui, resp, enabled, state, &PartResponses::default(), |cx| {
        u.hover.input(&(), cx, &mut Intents::default());
        if focusable {
            u.focus.input(&FocusParams { focus_on_click: Some(false), ..FocusParams::default() }, cx, &mut Intents::default());
        } else {
            cx.set(Focusable::OWNS, false);
        }
        u.press.input(&PressParams::default(), cx, &mut Intents::default());
    });
    *state
}

/// A paint context on `painter` traced as `id` (debug builds, when the trace is on).
fn cx<'a>(painter: &'a egui::Painter, id: egui::Id, part: Option<&'static str>) -> PaintCx<'a> {
    let mut p = PaintCx::for_node(painter, id.value());
    p.target.1 = part;
    p
}

/// Paints `st`'s box (shadows, background, border) at `rect`; returns its geometry (the padding
/// and content boxes, for the caller's layout). R9: a style with `opacity` < 1 (a disabled button)
/// paints faded.
pub fn surface(painter: &egui::Painter, id: egui::Id, st: &Styled, rect: egui::Rect) -> BoxGeom {
    let v = st.view();
    let p = faded(painter, &v);
    paint::paint_box(&mut cx(&p, id, None), &v, rect)
}

/// `painter` at the style's `opacity`.
fn faded(painter: &egui::Painter, v: &paint::StyleView) -> egui::Painter {
    let mut p = painter.clone();
    if v.opacity() < 1.0 {
        p.multiply_opacity(v.opacity());
    }
    p
}

/// Paints `text` in `rect` under `st` (font, colour, `text-align` over `align`), traced as part
/// `label`.
pub fn label(painter: &egui::Painter, id: egui::Id, st: &Styled, text: &str, rect: egui::Rect, align: Align) {
    let v = st.view();
    let p = faded(painter, &v);
    let g = paint::text_galley(painter.ctx(), &v, text, None);
    paint::paint_text(&mut cx(&p, id, Some("label")), &v, &g, rect, Align::of(&v, align));
}

/// Paints a galley the caller laid out (berth's tab label in its `tab_font`) at `pos`, in `st`'s
/// `color` (over the galley's own), traced as part `label`.
pub fn galley(painter: &egui::Painter, id: egui::Id, st: &Styled, g: &Arc<egui::Galley>, pos: egui::Pos2) {
    let v = st.view();
    let p = faded(painter, &v);
    let color = text_color(st);
    p.galley_with_override_text_color(pos, g.clone(), color);
    cx(&p, id, Some("label")).record(TraceKind::Text, egui::Rect::from_min_size(pos, g.size()), color, egui::Stroke::NONE, v.font_size());
}

/// `st`'s text colour (`color`), for a caller that lays text out itself.
pub fn text_color(st: &Styled) -> egui::Color32 {
    st.view().color("color").unwrap_or(egui::Color32::PLACEHOLDER)
}

/// `st`'s font (`font-family`, `font-size`, `font-weight`) as egui's, for a caller that lays text
/// out itself (berth's `ChromeMetrics::tab_font`).
pub fn font(ctx: &egui::Context, st: &Styled) -> egui::FontId {
    paint::text_font(ctx, &st.view(), "")
}

/// Paints the icon `key` (`name`, or `family:name`) centred in `rect`, tinted by `st`'s `color`
/// and sized by its `font-size`, traced as part `icon`. ICR1 re-points it at the icon registry.
pub fn icon(painter: &egui::Painter, id: egui::Id, st: &Styled, key: &str, rect: egui::Rect) {
    let v = st.view();
    let p = faded(painter, &v);
    super::paint_icon_key(&mut cx(&p, id, Some("icon")), &v, key, rect);
}

/// Paints `st`'s outline (the focus ring, when a `:focus-visible` rule or the UA table sets one,
/// R8) around `rect`, last.
pub fn ring(painter: &egui::Painter, id: egui::Id, st: &Styled, rect: egui::Rect) {
    paint::paint_outline(&mut cx(painter, id, None), &st.view(), rect);
}

impl Sv {
    /// [`state`] and [`Sv::styled`] in one call: the element `el` of a foreign painter in the
    /// state its response and `props` give, over `ua` (`Ua::NONE` for chrome with no kind yet:
    /// the theme's CSS alone; [`BUTTON_UA`] for chrome buttons).
    pub fn chrome(&self, ui: &egui::Ui, resp: &egui::Response, el: &super::El, ua: crate::paint::Ua, props: StateBits) -> Styled {
        let s = state(ui, resp, props);
        self.styled(ui.ctx(), el, ua, s)
    }
}
