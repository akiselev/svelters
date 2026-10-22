//! Paint-only chrome, for a foreign painter that keeps its own interaction (berth's
//! `ChromePainter`: a tab is dragged, so berth calls `ui.interact` with its own sense): the
//! state bits come from the same units run on the caller's response ([`state`]), and the look
//! from the cascade for a virtual element ([`Sv::styled`]) through the paint primitives
//! ([`surface`], [`label`], [`ring`]). A part of the chrome that is a plain control (a tab's close
//! button, the strip's scroll arrows, pin and float buttons) is a full widget instead
//! ([`Button`](super::Button) with `show_at`).

use super::{Styled, Sv};
use crate::interact::{FocusParams, Focusable, Hoverable, Intents, PartResponses, PressParams, Pressable, Unit};
use crate::paint::{self, Align, BoxGeom, PaintCx};
use crate::provider::StateBits;
use std::sync::{Arc, Mutex};

#[derive(Debug, Default)]
struct Units {
    hover: Hoverable,
    focus: Focusable,
    press: Pressable,
}

/// The state bits of a chrome element whose response the caller made (`resp`, any sense): the
/// Hoverable, Pressable and (when `resp` senses focus) Focusable units run on it, so `:hover`,
/// `:active`, `:focus`, `:focus-visible` follow the same rules as a Svelte widget's (same frame,
/// keyboard-only ring, R9 hover while disabled). `props` are the caller's mirrored bits
/// (`StateBits::SELECTED` for the active tab, `CURRENT`, `DROP_TARGET`, …). The units keep their
/// state in egui memory under `resp.id`.
pub fn state(ui: &egui::Ui, resp: &egui::Response, props: StateBits) -> StateBits {
    let units = ui.ctx().data_mut(|d| d.get_temp_mut_or_insert_with(resp.id.with("svelters-imm-chrome"), || Arc::new(Mutex::new((Units::default(), StateBits::NONE)))).clone());
    let mut g = units.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let (u, state) = &mut *g;
    let keep = StateBits(Hoverable::OWNS.0 | Focusable::OWNS.0 | Pressable::OWNS.0);
    *state = StateBits(state.0 & keep.0) | props;
    state.set(StateBits::DISABLED, !ui.is_enabled());
    let focusable = resp.sense.is_focusable();
    crate::interact::run(ui, resp, state, &PartResponses::default(), |cx| {
        u.hover.input(&(), cx, &mut Intents::default());
        if focusable {
            u.focus.input(&FocusParams::default(), cx, &mut Intents::default());
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
/// and content boxes, for the caller's layout).
pub fn surface(painter: &egui::Painter, id: egui::Id, st: &Styled, rect: egui::Rect) -> BoxGeom {
    paint::paint_box(&mut cx(painter, id, None), &st.view(), rect)
}

/// Paints `text` in `rect` under `st` (font, colour, `text-align` over `align`), traced as part
/// `label`.
pub fn label(painter: &egui::Painter, id: egui::Id, st: &Styled, text: &str, rect: egui::Rect, align: Align) {
    let v = st.view();
    let g = paint::text_galley(painter.ctx(), &v, text, None);
    paint::paint_text(&mut cx(painter, id, Some("label")), &v, &g, rect, Align::of(&v, align));
}

/// Paints `st`'s outline (the focus ring, when a `:focus-visible` rule or the UA table sets one)
/// around `rect`, last.
pub fn ring(painter: &egui::Painter, id: egui::Id, st: &Styled, rect: egui::Rect) {
    paint::paint_outline(&mut cx(painter, id, None), &st.view(), rect);
}

impl Sv {
    /// [`state`] and [`Sv::styled`] in one call: the element `el` of a foreign painter in the
    /// state its response and `props` give, over `ua` (`Ua::NONE` for chrome with no kind yet:
    /// the theme's CSS alone).
    pub fn chrome(&self, ui: &egui::Ui, resp: &egui::Response, el: &super::El, ua: crate::paint::Ua, props: StateBits) -> Styled {
        let s = state(ui, resp, props);
        self.styled(ui.ctx(), el, ua, s)
    }
}
