//! `label`, svelters' own (OW1a, ARCH-WIDGETS §6.1): text; Hoverable, and Pressable only while a
//! runtime listens to `clicked` (so a label never steals clicks from what is under it).
//! The default `label` since OW1a3 (`DEFAULT_OWN`); `SVELTERS_OWN_WIDGETS=none` gives the
//! `egui::Label` wrapper until VIS1c deletes it.
//!
//! `wrap-mode`: `wrap` (height for width), `truncate` (one line, cut at the end with `…`),
//! `extend` (one line, never wrapped). OW1a2: `elide` (`start`, `middle`, `end`; WG6 QW13) cuts one
//! line to the width wherever `wrap-mode` says, and so does CSS `text-overflow: ellipsis`; an
//! elided label shows its full text as the hover text (without a `tooltip`) and accessible name.
//! `for` (Qt's buddy, WIDGET-PARITY §2.10) names a target's `anchor`: a click on the label focuses
//! it, and the label names it in AccessKit (`labelled_by`).
//!
//! OW1a3:
//! - **Rich text** (RT2, WIDGET-PARITY Q4): with run children (`span`, `b`, `i`, `code`, `a`) the
//!   label is one paragraph laid out by [`crate::rich::layout_job_at`] at the label's CSS font
//!   size. [`crate::rich::refresh`] keeps the runs for measuring (and the text mirror, so
//!   `Node::text` and AccessKit read all runs); the draw collects them again for this frame's
//!   hover and focus styles, and every `a` run is a link (`crate::rich::link_pass`).
//! - **Selection** (`selectable`, WIDGET-PARITY §2.10 `textInteractionFlags`, ARCH §6.1: a
//!   read-only TextInput): off by default (app chrome; the theme also sets egui's
//!   `selectable_labels` off). A selectable label is `:read-only`, shows the I-beam (R5), selects
//!   by pointer (drag, double-click a word, triple-click a line, Shift-click extends) and, once
//!   clicked, by keyboard (Shift+arrows, Ctrl+A), copies with Ctrl+C, and gets AccessKit text runs
//!   with the selection. `bind:selection-start` / `bind:selection-end` are the selection in chars
//!   of the shown text (Qt's `selectionStart`, `setSelection`; equal means none, a negative end is
//!   the end of the text).

use crate::interact::{Hoverable, Pressable};
use crate::paint::{self, Align, Elide, Ua, UaRule, UaValue::*};
use crate::provider::{A11yCx, AvailableSpace, Event, MeasureCx, Widget, WidgetCx};
use crate::rich::Rich;
use egui::text::CCursor;
use egui::text_selection::{CCursorRange, TextCursorState};

pub const UA: Ua = Ua(&[UaRule::base(&[("text-align", Keyword("start"))])]);

/// Static text.
#[svelters::widget(namespace = "sv", replaces = "label", caps = HEIGHT_FOR_WIDTH)]
pub struct Label {
    #[prop(measure)]
    text: String,
    /// `wrap`, `truncate` or `extend`.
    #[prop(measure, default = "wrap".to_string())]
    wrap_mode: String,
    /// The text can be selected and copied (a read-only TextInput); off by default.
    #[prop]
    selectable: bool,
    /// The selection's start, in chars of the shown text (`bind:`).
    #[prop(bind)]
    selection_start: i64,
    /// The selection's end, in chars (`bind:`); a negative end is the end of the text.
    #[prop(bind)]
    selection_end: i64,
    #[prop(measure, default = 400)]
    font_weight: i64,
    /// WG6 QW13: cut one line with `…` to the width: `none`, `start`, `middle`, `end`.
    #[prop(measure, enumeration = "none|start|middle|end", default = "none".to_string())]
    elide: String,
    /// The `anchor` of the control this label names (Qt's buddy): a click focuses it.
    #[prop(rename = "for")]
    for_anchor: String,
    #[event]
    clicked: Event<()>,
    #[behavior]
    hover: Hoverable,
    #[behavior]
    press: Pressable,
    /// RT2: the runs, as last collected for measuring (`crate::rich::refresh`).
    #[state]
    rich: Option<Box<Rich>>,
    /// The selection (`selectable`).
    #[state]
    cursor: TextCursorState,
    /// The selection props as last synced with `cursor` (a JS write differs from it).
    #[state]
    synced: (i64, i64),
}

impl Label {
    /// The cut, if any: `elide`, `wrap-mode="truncate"` (an end cut) or `text-overflow: ellipsis`.
    fn elision(&self, s: &paint::StyleView<'_>) -> Option<Elide> {
        Elide::parse(&self.elide).or_else(|| (self.wrap_mode == "truncate").then_some(Elide::End)).or_else(|| Elide::of_style(s))
    }

    /// The plain galley at `width`.
    fn galley(&self, ctx: &egui::Context, s: &paint::StyleView<'_>, width: Option<f32>) -> std::sync::Arc<egui::Galley> {
        match (self.elision(s), self.wrap_mode.as_str(), width) {
            (Some(mode), _, Some(w)) => paint::text_galley(ctx, s, &paint::elide(ctx, s, &self.text, mode, w), None),
            (None, "wrap", Some(w)) => paint::text_galley(ctx, s, &self.text, Some(w)),
            _ => paint::text_galley(ctx, s, &self.text, None),
        }
    }

    #[allow(clippy::too_many_arguments)]
    /// RT2: the rich galley at `width` (`wrap` wraps, `truncate` cuts the paragraph at the end).
    fn rich_galley(&self, ctx: &egui::Context, style: &egui::Style, s: &paint::StyleView<'_>, rich: &Rich, colors: crate::rich::Colors, width: Option<f32>, lit: Option<crate::scene::NodeId>) -> std::sync::Arc<egui::Galley> {
        let wrap = (self.wrap_mode == "wrap").then_some(width).flatten();
        let mut job = crate::rich::layout_job_at(ctx, style, s.font_size(), rich, colors, wrap, lit);
        if let (Some(w), true) = (width, self.wrap_mode == "truncate" || self.elision(s).is_some()) {
            job.wrap = egui::text::TextWrapping::truncate_at_width(w);
        }
        ctx.fonts_mut(|f| f.layout_job(job))
    }

    /// The own label's text and weight (`crate::rich::collect`).
    pub(crate) fn text_weight(&self) -> (String, i64) {
        (self.text.clone(), self.font_weight)
    }

    /// Stores the runs collected for measuring; gives the text the label now shows.
    pub(crate) fn set_rich(&mut self, rich: Option<Box<Rich>>) -> String {
        self.rich = rich;
        self.rich.as_ref().map_or_else(|| self.text.clone(), |r| r.plain.clone())
    }

    /// RT2: the runs as last collected (`None`: plain text).
    pub fn rich(&self) -> Option<&Rich> {
        self.rich.as_deref()
    }

    /// The selection props as a char range of `galley`, if they select anything.
    fn prop_range(&self, galley: &egui::Galley) -> Option<CCursorRange> {
        let n = galley.text().chars().count() as i64;
        let end = if self.selection_end < 0 { n } else { self.selection_end.min(n) };
        let start = self.selection_start.clamp(0, n);
        (start != end).then(|| CCursorRange::two(CCursor::new(start as usize), CCursor::new(end as usize)))
    }

    /// Pointer and keyboard selection over `galley` drawn at `pos` (`selectable`). Returns the
    /// range to paint.
    fn select(&mut self, ui: &egui::Ui, cx: &mut WidgetCx<'_>, resp: &egui::Response, galley: &egui::Galley, pos: egui::Pos2) -> Option<CCursorRange> {
        // a JS write of the props moves the selection
        if (self.selection_start, self.selection_end) != self.synced {
            self.cursor.set_char_range(self.prop_range(galley));
        }
        if let Some(p) = resp.interact_pointer_pos().or_else(|| resp.hover_pos()).filter(|_| resp.is_pointer_button_down_on() || resp.clicked() || resp.drag_stopped()) {
            let at = galley.cursor_from_pos(p - pos);
            if self.cursor.pointer_interaction(ui, resp, at, galley, resp.dragged()) {
                resp.request_focus();
            }
        }
        if resp.has_focus() {
            let os = ui.ctx().os();
            let events = ui.input(|i| i.events.clone());
            let mut range = self.cursor.range(galley).unwrap_or_else(|| CCursorRange::one(CCursor::new(0)));
            let mut moved = false;
            for e in &events {
                match e {
                    egui::Event::Copy => {
                        let t = range.slice_str(galley.text());
                        if !t.is_empty() {
                            ui.ctx().copy_text(t.to_string());
                        }
                    }
                    e => moved |= range.on_event(os, e, galley, resp.id),
                }
            }
            if moved {
                self.cursor.set_char_range(Some(range));
            }
        } else if resp.ctx.input(|i| i.pointer.any_pressed()) && !resp.is_pointer_button_down_on() && !resp.hovered() {
            // a press elsewhere ends the selection (as egui's labels)
            self.cursor.set_char_range(None);
        }
        let range = self.cursor.range(galley).filter(|r| !r.is_empty());
        let (start, end) = range.map_or((0, 0), |r| {
            let r = r.as_sorted_char_range();
            (r.start.0 as i64, r.end.0 as i64)
        });
        if (start, end) != self.synced {
            // the user's selection (announced to `bind:`), or a JS write applied (unchanged)
            cx.set(&mut self.selection_start, start);
            cx.set(&mut self.selection_end, end);
        }
        self.synced = (start, end);
        range
    }
}

impl Widget for Label {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, avail: AvailableSpace) -> egui::Vec2 {
        let s = cx.style(UA).with_font_weight(Some(self.font_weight));
        if let Some(rich) = &self.rich {
            let colors = crate::rich::Colors { text: egui::Color32::WHITE, link: egui::Color32::WHITE };
            let w = match avail {
                AvailableSpace::Definite(w) if self.wrap_mode == "wrap" => Some(w),
                _ => None,
            };
            let job = crate::rich::layout_job_at(cx.ctx, cx.style, s.font_size(), rich, colors, w, None);
            return cx.ctx.fonts_mut(|f| f.layout_job(job)).size();
        }
        match (self.wrap_mode.as_str(), avail) {
            ("wrap", AvailableSpace::Definite(w)) if self.elision(&s).is_none() => paint::measure_text(cx.ctx, &s, &self.text, AvailableSpace::Definite(w)),
            _ => paint::measure_text(cx.ctx, &s, &self.text, AvailableSpace::MaxContent),
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        let target = (!self.for_anchor.is_empty()).then(|| {
            let sc = cx.scene();
            let root = cx.node().root();
            sc.by_anchor(root, &self.for_anchor).map(|n| (n.id(), n.egui_id()))
        });
        let target = target.flatten();
        let clickable = cx.listens(&self.clicked) || target.is_some();
        cx.set_state(crate::provider::StateBits::READ_ONLY, self.selectable);
        // RT2: this frame's runs (hover and focus styles of the links)
        let me = cx.index();
        let rich = crate::rich::collect_runs(cx.scene_mut(), me, &self.text, self.font_weight);
        let links = rich.as_ref().map(|r| r.links()).unwrap_or_default();
        let lit = crate::rich::lit_link(cx.scene_mut(), &links, ui.ctx());
        let colors = {
            let st = cx.style(UA);
            crate::rich::Colors { text: st.color("color").unwrap_or(ui.visuals().text_color()), link: ui.visuals().hyperlink_color }
        };
        // the galley first: the label's hit area and drawn rect are its text (as egui's label)
        let (mut g, text_rect) = {
            let st = cx.style(UA).with_font_weight(Some(self.font_weight));
            let w = (self.wrap_mode != "extend" || self.elision(&st).is_some()).then(|| cx.rect().width());
            let g = match &rich {
                Some(r) => self.rich_galley(ui.ctx(), ui.style(), &st, r, colors, w, lit),
                None => self.galley(ui.ctx(), &st, w),
            };
            let (r, size) = (cx.rect(), g.size());
            let x = match Align::of(&st, Align::Start) {
                Align::Start => r.left(),
                Align::Center => r.center().x - size.x / 2.0,
                Align::End => r.right() - size.x,
            };
            let ppp = ui.ctx().pixels_per_point();
            let min = egui::pos2(x, r.center().y - size.y / 2.0);
            (g, egui::Rect::from_min_size(egui::pos2((min.x * ppp).round() / ppp, (min.y * ppp).round() / ppp), size))
        };
        let shown = rich.as_ref().map_or_else(|| self.text.clone(), |r| r.plain.clone());
        let elided = g.job.text != shown;
        let mut sense = if clickable { self.units_sense() } else { egui::Sense::hover() };
        if self.selectable {
            // focusable: keyboard selection and Ctrl+C once clicked (Qt's TextSelectableByKeyboard)
            sense |= egui::Sense::CLICK | egui::Sense::DRAG | egui::Sense::FOCUSABLE;
        }
        let resp = cx.interact_at(ui, text_rect, sense);
        let resp = if self.selectable { resp.on_hover_cursor(egui::CursorIcon::Text) } else { resp };
        let fired = self.run_units(cx, ui, &resp);
        if clickable && fired.press.iter().any(|p| p.is_click()) {
            if cx.listens(&self.clicked) {
                cx.dispatch(&self.clicked, ());
            }
            if let Some((t, _)) = target {
                cx.focus_node(t);
            }
        }
        // RT2: one interaction per link, over the label's (links are on top)
        if let Some(r) = &rich {
            let ring = ui.visuals().selection.stroke;
            let (activated, lit_now) = crate::rich::link_pass(cx.scene_mut(), ui, r, &links, &g, text_rect.min, ring);
            if lit_now != lit {
                // a hover or focus that began this frame restyles the link before it is drawn
                let rich = crate::rich::collect_runs(cx.scene_mut(), me, &self.text, self.font_weight);
                if let Some(r) = &rich {
                    let st = cx.style(UA).with_font_weight(Some(self.font_weight));
                    let w = (self.wrap_mode != "extend" || self.elision(&st).is_some()).then(|| cx.rect().width());
                    g = self.rich_galley(ui.ctx(), ui.style(), &st, r, colors, w, lit_now);
                }
            }
            let (s, out) = cx.scene_out();
            crate::rich::activate(s, ui.ctx(), out, activated);
        }
        let selection = if self.selectable { self.select(ui, cx, &resp, &g, text_rect.min) } else { None };
        if let Some(range) = &selection {
            egui::text_selection::visuals::paint_text_selection(&mut g, ui.visuals(), range, None);
        }
        if ui.is_rect_visible(cx.rect()) {
            let st = cx.style(UA).with_font_weight(Some(self.font_weight));
            paint::paint_text(&mut cx.paint_cx(ui), &st, &g, cx.rect(), Align::of(&st, Align::Start));
        }
        if let Some((_, eid)) = target {
            // AccessKit: the label names its buddy (`labelled_by` on the target's node)
            let label = resp.id.accesskit_id();
            ui.ctx().accesskit_node_builder(eid, |n| n.push_labelled_by(label));
        }
        let enabled = cx.enabled();
        let text = shown;
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, enabled, &text));
        if self.selectable {
            // a read-only TextInput's text runs and selection (only when selectable)
            let to_global = egui::emath::TSTransform::from_translation(text_rect.min.to_vec2());
            egui::text_selection::accesskit_text::update_accesskit_for_text_widget(ui.ctx(), resp.id, selection, to_global, &g);
        }
        if elided && cx.node().common().tooltip.is_empty() {
            return resp.on_hover_text(text);
        }
        resp
    }

    fn a11y(&self, _cx: &A11yCx<'_>, node: &mut egui::accesskit::Node) {
        node.set_role(egui::accesskit::Role::Label);
        let text = self.rich.as_ref().map_or(self.text.as_str(), |r| r.plain.as_str());
        node.set_value(text);
    }
}
