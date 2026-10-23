//! OW7c: `status-bar` (WIDGET-PARITY §2.22, QStatusBar), the generic kind cadatomic builds its
//! status sections on (WIDGET-PARITY's Decided block): a flex row whose children are the sections,
//! placed by their `status-section` (Widget prop):
//!
//! - `start`: permanent, first;
//! - `temporary` (the default): after the start sections, hidden while a message shows (Qt's
//!   `addWidget`);
//! - `end`: permanent, pushed to the far end (Qt's `addPermanentWidget`).
//!
//! `flex-grow` stretches a section (Qt's `stretch`). Only taffy sees the section order
//! ([`order_sections`], `layout.rs` `children_laid`); paint and Tab order stay the tree's. The first
//! end section gets `margin-left: auto`, so the free space between the start and the end sections
//! is the message area, `::part(message)`.
//!
//! The message is `message` (bindable), `show-message(text, timeout)` (on egui's clock: kittest's
//! virtual one) and `clear-message`. A status tip wins over it: the `status-tip` (Widget prop) of
//! the innermost hovered node, else of the focused one, gathered by the walk ([`offer`]) and
//! resolved at the end of each pass, is shown by every status bar of the viewport and read from
//! Rust with [`status_tip`].

use crate::kinds::P;
use crate::paint::{self, UaRule, UaValue::*};
use crate::scene::{Arena, NodeId as SceneId, StateBits, W};
use crate::views::{arg, arity};
use crate::walk::Out;
use egui::{Context, Id, Rect, Ui, Vec2};
use std::sync::Arc;
use svelters_abi::{StrResult, Value};

/// A `status-bar` node's message and its sections in layout order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StatusBar {
    pub message: String,
    /// `show-message`'s timeout in seconds, started at the next paint (egui's clock).
    start: Option<f64>,
    /// egui time the message times out.
    until: Option<f64>,
    /// `changed(message)` is due at the next paint (a method or a timeout changed it).
    announce: bool,
    /// The children in section order (taffy's view), rebuilt before each layout of the bar.
    pub(crate) laid: Vec<SceneId>,
    /// The end section given `margin-left: auto` (restored when another one takes its place).
    pushed: Option<SceneId>,
}

impl StatusBar {
    /// Sets the message; `timeout` in seconds (`None` or 0: until changed). A JS prop write is
    /// not announced (`set_prop` never is); the methods are.
    pub(crate) fn set_message(&mut self, text: String, timeout: Option<f64>) {
        self.message = text;
        self.start = timeout.filter(|t| *t > 0.0 && !self.message.is_empty());
        self.until = None;
    }

    /// `show-message(text, timeout = 0)`.
    pub(crate) fn show_message(&mut self, args: &[Value]) -> StrResult<Value> {
        arity("show-message", args, 1, 2)?;
        let text = match arg(args, 0) {
            Value::Str(s) => s.clone(),
            Value::Null => String::new(),
            v => return Err(format!("show-message: text must be a string, got {v:?}")),
        };
        let timeout = match arg(args, 1) {
            Value::Null => None,
            Value::F64(t) => Some(*t),
            Value::I64(t) => Some(*t as f64),
            v => return Err(format!("show-message: timeout must be a number of seconds, got {v:?}")),
        };
        self.announce |= text != self.message;
        self.set_message(text, timeout);
        Ok(Value::Null)
    }

    /// `clear-message()`.
    pub(crate) fn clear_message(&mut self, args: &[Value]) -> StrResult<Value> {
        arity("clear-message", args, 0, 0)?;
        self.announce |= !self.message.is_empty();
        self.set_message(String::new(), None);
        Ok(Value::Null)
    }
}

/// The bar's look under its CSS (a container: `style::before_container` paints it): the panel
/// fill and a 1px line on top.
pub const STATUS_UA: paint::Ua = paint::Ua(&[UaRule::base(&[
    ("background-color", Token("panel-bg")),
    ("border-top-width", Px(1.0)),
    ("border-top-style", Keyword("solid")),
    ("border-top-color", Token("border")),
])]);
/// `::part(message)`: the message and status-tip text.
pub const MESSAGE_UA: paint::Ua = paint::Ua(&[UaRule::base(&[("color", Token("fg")), ("padding-left", Px(4.0))])]);

/// The bar's paint parts.
pub static PARTS: &[crate::provider::PartEntry] =
    &[crate::provider::PartEntry { name: "message", docs: "The message or status tip, in the free space after the start sections (one line, cut at the end sections).", node: false }];

fn section(arena: &Arena, c: usize) -> u8 {
    match arena.common(c).status_section {
        "start" => 0,
        "end" => 2,
        _ => 1,
    }
}

/// Rebuilds bar `i`'s layout order (start, temporary, end; stable within each) and pushes its
/// first end section to the end (`margin-left: auto`).
pub(crate) fn order_sections(arena: &mut Arena, i: usize) {
    let mut laid = arena.children[i].clone();
    laid.sort_by_key(|c| arena.idx(*c).map_or(1, |ci| section(arena, ci))); // stable
    let first_end = laid.iter().copied().find(|c| arena.idx(*c).is_some_and(|ci| section(arena, ci) == 2));
    let W::StatusBar(b) = arena.w_mut(i) else { return };
    b.laid = laid;
    let was = std::mem::replace(&mut b.pushed, first_end);
    if was != first_end
        && let Some(ci) = was.and_then(|c| arena.idx(c))
    {
        let m = arena.common(ci).margin as f32;
        set_margin_left(arena, ci, taffy::LengthPercentageAuto::length(m));
    }
    if let Some(ci) = first_end.and_then(|c| arena.idx(c)) {
        set_margin_left(arena, ci, taffy::LengthPercentageAuto::auto());
    }
}

/// Writes `margin-left` into child `c`'s taffy style (copied only when it changes: styles are shared).
fn set_margin_left(arena: &mut Arena, c: usize, v: taffy::LengthPercentageAuto) {
    if arena.layout[c].style.margin.left != v {
        std::sync::Arc::make_mut(&mut arena.layout[c].style).margin.left = v;
    }
}

/// Paints bar `i` at `r` (screen): ages its message, then its sections (the temporary ones only
/// while no message or tip shows), the message part, and its AccessKit status node.
pub(crate) fn paint(arena: &mut Arena, i: usize, r: Rect, origin: Vec2, ui: &mut Ui, out: &mut Vec<Out>) {
    let ctx = ui.ctx().clone();
    let now = ui.input(|inp| inp.time);
    let tip = arena.common(i).tooltip.clone();
    let resp = ui.interact(r, arena.core[i].egui_id.with("status"), egui::Sense::hover());
    if !tip.is_empty() {
        crate::widgets::tooltip::attach(ui, &resp, &tip, arena.own.contains("tooltip"));
    }
    // the message's clock
    let mut changed = None;
    if let W::StatusBar(b) = arena.w_mut(i) {
        if let Some(t) = b.start.take() {
            b.until = Some(now + t);
        }
        if b.until.is_some_and(|u| now >= u) {
            b.message.clear();
            b.until = None;
            b.announce = true;
        }
        if let Some(u) = b.until {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64((u - now).max(0.0)));
        }
        if std::mem::take(&mut b.announce) {
            changed = Some(b.message.clone());
        }
    }
    if let Some(m) = changed {
        crate::leaf::widget_changed(arena, i, &[P::Message], vec![Value::Str(m)], out);
    }
    let W::StatusBar(b) = arena.w(i) else { return };
    let text: Option<Arc<str>> = status_tip(&ctx).or_else(|| (!b.message.is_empty()).then(|| Arc::from(b.message.as_str())));
    let showing = text.is_some();
    let state = arena.core[i].state;
    if state.contains(StateBits::OPEN) != showing {
        let mut s = state;
        s.set(StateBits::OPEN, showing);
        crate::style::set_own_state(arena, i, s); // `:open` (not a walk bit: kept across passes)
        ctx.request_repaint(); // the bar's box was painted with the old state
    }
    // no computed style: the UA look (with one, `style::before_container` painted it)
    let node = arena.slots.id_at(i).expect("live node");
    let dk = crate::widgets::popup::dark(&ctx);
    if arena.style.computed(node).is_none() {
        let v = paint::StyleView::ua_only(STATUS_UA, StateBits::NONE, dk);
        paint::paint_box(&mut paint::PaintCx::for_node(ui.painter(), node.to_raw()), &v, r);
    }
    // the sections; the message area runs from the last start section to the first end one
    let (mut left, mut right) = (r.left(), r.right());
    for k in 0..arena.children[i].len() {
        let Some(c) = arena.idx(arena.children[i][k]) else { continue };
        let s = section(arena, c);
        let cr = arena.core[c].rect.translate(origin);
        if arena.common(c).visible && cr.is_positive() {
            match s {
                0 => left = left.max(cr.right()),
                2 => right = right.min(cr.left()),
                _ => {}
            }
        }
        if showing && s == 1 {
            continue;
        }
        crate::walk::paint_at(arena, c, origin, ui, out);
    }
    let label = text.as_deref().unwrap_or("");
    if let Some(t) = &text {
        let hcss = crate::widgets::popup::css(arena, node);
        let host = paint::StyleView::new(hcss.as_deref(), paint::Ua::ROOT, StateBits::NONE, dk);
        let mcss = crate::widgets::popup::part_css(arena, node, "message");
        let ms = host.part(mcss.as_deref(), MESSAGE_UA);
        let color = ms.color("color").unwrap_or(ui.visuals().text_color());
        let size = ms.px("font-size").unwrap_or(13.0);
        let pad = ms.sides("padding")[0];
        let area = Rect::from_x_y_ranges((left + pad)..=right.max(left + pad), r.y_range());
        let galley = ui.painter().layout_no_wrap(t.to_string(), egui::FontId::proportional(size), color);
        let pos = egui::pos2(area.left(), area.center().y - galley.size().y / 2.0);
        ui.painter().with_clip_rect(area.intersect(ui.clip_rect())).galley(pos, galley, color);
    }
    // AccessKit: a polite live status region carrying the message
    ctx.accesskit_node_builder(resp.id, |n| {
        n.set_role(egui::accesskit::Role::Status);
        n.set_live(egui::accesskit::Live::Polite);
        if !label.is_empty() {
            n.set_label(label);
        }
    });
}

// ---- status tips ----

/// The status-tip state of one viewport.
#[derive(Clone, Default)]
struct Tips {
    /// The pass `hovered` and `focused` were gathered in.
    pass: u64,
    /// The innermost hovered node's tip this pass (the walk goes parents first).
    hovered: Option<Arc<str>>,
    /// The focused node's tip this pass.
    focused: Option<Arc<str>>,
    /// The last complete pass's tip.
    shown: Option<Arc<str>>,
}

fn tips_id(ctx: &Context) -> Id {
    Id::unique("svelters-egui/ow7c-status-tip").with(ctx.viewport_id())
}

/// The status tip of the last complete pass of `ctx`'s viewport: the `status-tip` of the
/// innermost node under the pointer (enabled or not), else of the node with keyboard focus.
/// cadatomic's own status bar reads it here; `<status-bar>` shows it.
pub fn status_tip(ctx: &Context) -> Option<Arc<str>> {
    let id = tips_id(ctx); // outside the lock: `viewport_id` takes it too
    ctx.data(|d| d.get_temp::<Tips>(id)).and_then(|t| t.shown)
}

/// The walk's hook for node `i` at `r` (screen) with a `status-tip`: records it while the node is
/// hovered or focused.
pub(crate) fn offer(arena: &Arena, i: usize, r: Rect, ui: &Ui) {
    let Some(tip) = arena.common(i).status_tip.as_deref() else { return };
    let hovered = ui.rect_contains_pointer(r);
    let focused = !hovered && (arena.core[i].state.contains(StateBits::FOCUSED) || ui.ctx().memory(|m| m.focused()) == Some(arena.core[i].egui_id));
    if !hovered && !focused {
        return;
    }
    let ctx = ui.ctx();
    install(ctx);
    let (pass, id) = (ctx.cumulative_pass_nr(), tips_id(ctx));
    ctx.data_mut(|d| {
        let t: &mut Tips = d.get_temp_mut_or_default(id);
        if t.pass != pass {
            (t.pass, t.hovered, t.focused) = (pass, None, None);
        }
        let v = Some(Arc::from(tip));
        if hovered { t.hovered = v } else { t.focused = v }
    });
}

fn install(ctx: &Context) {
    let key = Id::unique("svelters-egui/ow7c-status-tip/installed");
    if ctx.data(|d| d.get_temp::<bool>(key).is_some()) {
        return;
    }
    ctx.data_mut(|d| d.insert_temp(key, true));
    ctx.on_end_pass("svelters-ow7c-status-tip", Arc::new(|ui: &mut Ui| end_pass(ui.ctx())));
}

/// Resolves the pass's tip; a change repaints, so the status bars (drawn before or after the
/// hovered node) show it at once.
fn end_pass(ctx: &Context) {
    let (pass, id) = (ctx.cumulative_pass_nr(), tips_id(ctx));
    let changed = ctx.data_mut(|d| {
        let t: &mut Tips = d.get_temp_mut_or_default(id);
        let now = if t.pass == pass { t.hovered.take().or_else(|| t.focused.take()) } else { None };
        let changed = now != t.shown;
        t.shown = now;
        changed
    });
    if changed {
        ctx.request_repaint();
    }
}
