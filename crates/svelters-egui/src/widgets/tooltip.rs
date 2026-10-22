//! OW3: svelters' own tooltips (WIDGET-PARITY §2.15), the common `tooltip` prop of every kind.
//!
//! [`attach`] runs [`Tooltipable`] on a host's response: the wake-up delay
//! (`Policy::tooltip_delay_ms`), the follow-on window (`Policy::tooltip_follow_on_ms`: the next
//! tooltip shows at once), hidden by a press until the pointer leaves, shown on disabled hosts too
//! (R9: the reason). The text also becomes the host's AccessKit description, so it is read with
//! the host whether or not it shows. Hosts ask during the walk; at the end of the pass the
//! innermost one (the smallest rect) is painted, once, in an `egui::Area` at `Order::Tooltip`
//! that takes no pointer input, below the pointer ([`Anchored`](crate::interact::Anchored), flipped
//! above it at the bottom edge). [`show`] and [`hide`] are QToolTip's `showText`/`hideText`.
//!
//! Look: [`TOOLTIP_UA`] (R7: `tooltip-bg`, `menu-border-color` stroke, `popup-shadow`), text
//! wrapped at 400 (DESIGN's `tooltip_width`).
//!
//! The switch: `tooltip` in the own-widget [`KindSet`](super::KindSet) (no tag of its own).
//! Writer lane: OW3.

use super::popup::{self, SURFACE};
use crate::interact::anchor::{self, AnchorParams, Placement};
use crate::interact::tooltip::{Tip, TooltipParams, Tooltipable};
use crate::interact::Intents;
use crate::paint::{self, Align, StyleView, Ua, UaRule, UaValue::*};
use crate::provider::StateBits;
use egui::{Context, Id, Order, Pos2, Rect, Ui, Vec2, vec2};

/// The tooltip's surface: [`popup::SURFACE`] on `tooltip-bg`, 4/8 padding, radius 4, 12px text
/// wrapped at 400, 18px under the pointer.
pub const TOOLTIP_UA: Ua = Ua(&[
    UaRule::base(SURFACE),
    UaRule::base(&[
        ("background-color", Token("tooltip-bg")),
        ("border-radius", Px(4.0)),
        ("padding-left", Px(8.0)),
        ("padding-right", Px(8.0)),
        ("padding-top", Px(4.0)),
        ("padding-bottom", Px(4.0)),
        ("font-size", Token("font-size-sm")),
        ("max-width", Px(400.0)),
        ("margin", Px(18.0)),
    ]),
]);

#[derive(Clone, Default)]
struct Host {
    unit: Tooltipable,
    at: Option<Pos2>,
}

/// One pass's request: the innermost host wins.
#[derive(Clone)]
struct Pending {
    pass: u64,
    area: f32,
    text: String,
    at: Pos2,
    host: Id,
}

#[derive(Clone, Default)]
struct Shared {
    pending: Option<Pending>,
    /// [`show`]'s tooltip: shown until [`hide`].
    fixed: Option<(String, Pos2)>,
    /// When the last tooltip hid (the follow-on window).
    hid_at: f64,
    shown: Option<Id>,
}

fn shared_id() -> Id {
    Id::unique("svelters-egui/ow3-tooltip")
}

/// Shows `text` (a non-empty tooltip) for host `resp` the svelters way; with `own` false, egui's
/// `on_hover_text` (the wrapper, `SVELTERS_OWN_WIDGETS=none`).
pub(crate) fn attach(ui: &Ui, resp: &egui::Response, text: &str, own: bool) -> egui::Response {
    if !own {
        return resp.clone().on_hover_text(text);
    }
    let ctx = ui.ctx().clone();
    install(&ctx);
    let key = resp.id.with("ow3-tip");
    let mut host: Host = ctx.data(|d| d.get_temp(key)).unwrap_or_default();
    let policy = crate::native::policy_of(&ctx);
    let (now, ptr, pressed) = ctx.input(|i| (i.time, i.pointer.latest_pos(), i.pointer.any_pressed()));
    // a disabled host senses nothing; R9 keeps its tooltip (the pointer over it, not covered)
    let over = resp.hovered() || (!resp.enabled() && ptr.is_some_and(|p| resp.rect.contains(p) && ctx.layer_id_at(p) == Some(resp.layer_id)));
    let sh: Shared = ctx.data(|d| d.get_temp(shared_id())).unwrap_or_default();
    let warm = sh.shown.is_some_and(|s| s != resp.id) || now - sh.hid_at <= policy.tooltip_follow_on_ms as f64 / 1000.0;
    let p = TooltipParams { has_tip: !text.is_empty(), delay: policy.tooltip_delay_ms as f64 / 1000.0, warm, duration: 0.0 };
    let mut out = Intents::default();
    host.unit.step(&p, over, pressed, now, &mut out);
    for t in out {
        match t {
            Tip::Show => host.at = ptr,
            Tip::Hide => host.at = None,
        }
    }
    if let Some(w) = crate::interact::Unit::next_frame(&host.unit) {
        ctx.request_repaint_after(w);
    }
    if host.unit.shown()
        && let Some(at) = host.at
    {
        let area = resp.rect.area();
        ctx.data_mut(|d| {
            let s: &mut Shared = d.get_temp_mut_or_default(shared_id());
            let pass = ctx.cumulative_pass_nr();
            if s.pending.as_ref().is_none_or(|p| p.pass != pass || area <= p.area) {
                s.pending = Some(Pending { pass, area, text: text.to_string(), at, host: resp.id });
            }
        });
    }
    ctx.data_mut(|d| d.insert_temp(key, host));
    // AccessKit: the host is described by its tooltip, shown or not
    ctx.accesskit_node_builder(resp.id, |n| n.set_description(text.to_string()));
    resp.clone()
}

/// QToolTip `showText`: shows `text` at `at` (screen) until [`hide`].
pub fn show(ctx: &Context, text: &str, at: Pos2) {
    install(ctx);
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Shared>(shared_id()).fixed = Some((text.to_string(), at)));
    ctx.request_repaint();
}

/// QToolTip `hideText`.
pub fn hide(ctx: &Context) {
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Shared>(shared_id()).fixed = None);
    ctx.request_repaint();
}

/// Whether a tooltip showed in the last pass (tests, automation).
pub fn is_shown(ctx: &Context) -> bool {
    ctx.data(|d| d.get_temp::<Shared>(shared_id())).is_some_and(|s| s.shown.is_some())
}

fn install(ctx: &Context) {
    let key = Id::unique("svelters-egui/ow3-tooltip/installed");
    if ctx.data(|d| d.get_temp::<bool>(key).is_some()) {
        return;
    }
    ctx.data_mut(|d| d.insert_temp(key, true));
    ctx.on_end_pass("svelters-ow3-tooltip", std::sync::Arc::new(|ui: &mut Ui| end_pass(ui.ctx())));
}

/// Paints the pass's tooltip (the innermost host's, else [`show`]'s).
fn end_pass(ctx: &Context) {
    let pass = ctx.cumulative_pass_nr();
    let now = ctx.input(|i| i.time);
    let (req, was) = ctx.data_mut(|d| {
        let s: &mut Shared = d.get_temp_mut_or_default(shared_id());
        let req = match s.pending.take().filter(|p| p.pass == pass) {
            Some(p) => Some((p.text, p.at, p.host)),
            None => s.fixed.clone().map(|(t, at)| (t, at, Id::NULL)),
        };
        (req, s.shown)
    });
    let shown = req.as_ref().map(|r| r.2);
    if was.is_some() && shown.is_none() {
        ctx.data_mut(|d| d.get_temp_mut_or_default::<Shared>(shared_id()).hid_at = now);
    }
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Shared>(shared_id()).shown = shown);
    let Some((text, at, _)) = req else { return };
    paint(ctx, &text, at);
}

fn paint(ctx: &Context, text: &str, at: Pos2) {
    let s = StyleView::ua_only(TOOLTIP_UA, StateBits::NONE, popup::dark(ctx));
    let wrap = s.px("max-width").map(|w| w - paint::box_extra(&s).x);
    let g = paint::text_galley(ctx, &s, text, wrap);
    let size = g.size() + paint::box_extra(&s);
    let anchor = Rect::from_min_size(at, Vec2::ZERO);
    let placement = Placement::BOTTOM_START;
    let placed = anchor::place(&AnchorParams { anchor, size, bounds: anchor::bounds(ctx), placement, gap: popup::gap(&s, placement.side), ..AnchorParams::default() });
    let frame = placed.rect;
    let id = Id::unique("svelters-egui/ow3-tooltip/area");
    egui::Area::new(id).order(Order::Tooltip).fixed_pos(frame.min).constrain(false).interactable(false).show(ctx, |ui| {
        let mut pc = paint::PaintCx::new(ui.painter());
        pc.tag = "tooltip";
        let geom = paint::paint_box(&mut pc, &s, frame);
        paint::paint_text(&mut pc, &s, &g, geom.content, Align::Start);
        let r = ui.allocate_rect(frame, egui::Sense::hover());
        let label = text.to_string();
        ui.ctx().accesskit_node_builder(r.id, |n| {
            n.set_role(egui::accesskit::Role::Tooltip);
            n.set_label(label);
        });
        let _ = vec2(0.0, 0.0);
    });
}
