//! OW6: what `date-picker` and `color-picker` share: the drop button's open/close (a click,
//! Enter/Space, F4 or Alt+Down open it; Escape, a press outside or a pick close it), the popup
//! surface (`::part(popup)`, OW3's `popup::SURFACE` look, R7) placed by OW3's Anchored unit
//! below the button (flipped above at the bottom edge) at `Order::Foreground`, the keyboard
//! grab while open (OW3's `Grab`: focus returns to the button), and the Save/Cancel footer of
//! `commit-mode="confirm"`. Writer lane: OW6.

use super::popup::{self, Grab};
use crate::interact::anchor::{self, AnchorParams, Placement};
use crate::interact::dismiss::{DismissParams, Outside};
use crate::paint::{self, Align, StyleView, Ua, UaRule, UaValue::*};
use crate::provider::{StateBits, WidgetCx};
use egui::{Id, Key, Modifiers, Order, Rect, Vec2, accesskit, pos2, vec2};

/// `::part(popup)`: OW3's popup surface.
pub const POPUP_UA: Ua = Ua(&[UaRule::base(popup::SURFACE), UaRule::base(&[("padding", Px(8.0)), ("gap", Px(8.0))])]);

/// The closed button (DESIGN R4 field look, as OW3's select): `control-bg`, `control-border`,
/// radius `input-radius`, R8's field ring, the `focus` border while open, R10 pointer.
pub const BUTTON_UA: Ua = Ua(&[
    UaRule::base(&[
        ("background-color", Token("control-bg")),
        ("color", Token("control-fg")),
        ("border-color", Token("control-border")),
        ("border-width", Token("stroke")),
        ("border-radius", Token("input-radius")),
        ("padding-left", Px(6.0)),
        ("padding-right", Px(6.0)),
        ("padding-top", Px(3.0)),
        ("padding-bottom", Px(3.0)),
        ("line-height", Px(18.0)),
        ("font-size", Token("font-size")),
        ("gap", Px(4.0)),
        ("cursor", Keyword("pointer")),
        ("outline-style", Keyword("none")),
        ("outline-color", Token("focus")),
        ("outline-width", Px(1.0)),
        ("outline-offset", Px(-1.0)),
    ]),
    UaRule::when(StateBits::OPEN, &[("border-color", Token("focus"))]),
    UaRule::when(StateBits::FOCUS_VISIBLE, &[("outline-style", Keyword("solid"))]),
    UaRule::when(StateBits::DISABLED, &[("opacity", Number(0.4)), ("cursor", Keyword("not-allowed"))]),
]);

/// `::part(chevron)`: R14's drop chevron.
pub const CHEVRON_UA: Ua = Ua(&[UaRule::base(&[("width", Px(16.0)), ("font-size", Px(16.0)), ("color", Token("icon"))])]);

/// `::part(save)`: the primary footer button (`button-bg` in accent, DESIGN R1).
pub const SAVE_UA: Ua = Ua(&[
    UaRule::base(&[
        ("background-color", Token("accent")),
        ("color", Token("accent-fg")),
        ("border-radius", Token("button-radius")),
        ("padding-left", Px(10.0)),
        ("padding-right", Px(10.0)),
        ("height", Token("control-h")),
        ("cursor", Keyword("pointer")),
        ("outline-style", Keyword("none")),
        ("outline-color", Token("focus")),
        ("outline-width", Px(2.0)),
        ("outline-offset", Px(1.0)),
    ]),
    UaRule::when(StateBits::HOVERED, &[("background-color", Token("accent-hover"))]),
    UaRule::when(StateBits::FOCUS_VISIBLE, &[("outline-style", Keyword("solid"))]),
]);

/// `::part(cancel)`: the secondary footer button.
pub const CANCEL_UA: Ua = Ua(&[
    UaRule::base(&[
        ("background-color", Token("button-bg")),
        ("color", Token("button-fg")),
        ("border-color", Token("button-border-color")),
        ("border-width", Token("stroke")),
        ("border-radius", Token("button-radius")),
        ("padding-left", Px(10.0)),
        ("padding-right", Px(10.0)),
        ("height", Token("control-h")),
        ("cursor", Keyword("pointer")),
        ("outline-style", Keyword("none")),
        ("outline-color", Token("focus")),
        ("outline-width", Px(2.0)),
        ("outline-offset", Px(1.0)),
    ]),
    UaRule::when(StateBits::HOVERED, &[("background-color", Token("button-bg-hover"))]),
    UaRule::when(StateBits::FOCUS_VISIBLE, &[("outline-style", Keyword("solid"))]),
]);

/// What the footer did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Footer {
    Save,
    Cancel,
}

/// A drop button's popup state.
#[derive(Debug, Default)]
pub struct Drop {
    grab: Grab,
    was_open: bool,
    /// The pass it opened in (its own press is not "outside").
    opened: u64,
    /// It was opened from the keyboard (the body shows its focus ring at once).
    pub keyboard: bool,
}

impl Drop {
    /// The popup's id (the keyboard grab's focus target).
    pub fn id(host: Id) -> Id {
        host.with("ow6-popup")
    }

    /// Before the units run: notes a press (where focus goes back to).
    pub fn begin(&mut self, ctx: &egui::Context) {
        self.grab.note_press(ctx);
    }

    /// The keys that open the closed, focused button (F4, Alt+Down; Enter and Space come as
    /// clicks).
    pub fn open_keys(ctx: &egui::Context) -> bool {
        ctx.input(|i| i.key_pressed(Key::F4) || i.events.iter().any(|e| matches!(e, egui::Event::Key { key: Key::ArrowDown, pressed: true, modifiers, .. } if modifiers.alt)))
    }

    /// Records an open (`keyboard`: from a key).
    pub fn opening(&mut self, ctx: &egui::Context, keyboard: bool) {
        self.opened = ctx.cumulative_pass_nr();
        self.keyboard = keyboard;
    }

    /// It opened this pass (a host write or a click): the body resets.
    pub fn just_opened(&self, open: bool) -> bool {
        open && !self.was_open
    }

    /// While open: takes the keyboard. Returns whether the popup holds focus and Escape was
    /// pressed (the caller closes).
    pub fn hold(&mut self, ctx: &egui::Context, host: Id) -> bool {
        let id = Drop::id(host);
        if !self.was_open || !self.grab.active {
            self.grab.take(ctx, id);
        }
        let esc = self.was_open && ctx.input(|i| i.key_pressed(Key::Escape));
        if esc {
            ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
        }
        esc
    }

    /// Whether the popup holds the keyboard (its body takes the keys).
    pub fn has_keys(&self, ctx: &egui::Context, host: Id) -> bool {
        self.was_open && ctx.memory(|m| m.has_focus(Drop::id(host)))
    }

    /// Places the popup (`size`: its border box) below `anchor`, flipped above when it does not
    /// fit; `gap` from the surface's `margin`.
    pub fn frame(ctx: &egui::Context, surf: &StyleView<'_>, anchor: Rect, size: Vec2) -> Rect {
        let placement = Placement::BOTTOM_START;
        anchor::place(&AnchorParams { anchor, size, bounds: anchor::bounds(ctx), placement, gap: popup::gap(surf, placement.side), ..AnchorParams::default() }).rect
    }

    /// A press outside the popup and the button closes it (not the press that opened it).
    pub fn dismissed(&self, ctx: &egui::Context, frame: Rect, anchor: Rect) -> bool {
        self.opened != ctx.cumulative_pass_nr() && popup::dismissed(ctx, &DismissParams { inside: vec![frame, anchor], on_outside: Outside::Close, escape: false }).is_some()
    }

    /// Shows the popup surface at `frame` in its own foreground layer and runs `body` in it.
    pub fn show<R>(&mut self, ctx: &egui::Context, host: Id, frame: Rect, body: impl FnOnce(&mut egui::Ui) -> R) -> Option<R> {
        let id = Drop::id(host);
        let mut out = None;
        let grab = &self.grab;
        popup::area(ctx, id.with("area"), Order::Foreground, frame, |ui| {
            grab.keep(ui, id, frame);
            out = Some(body(ui));
        });
        out
    }

    /// After the popup: releases the grab when closed; remembers the state.
    pub fn end(&mut self, ctx: &egui::Context, host: Id, open: bool) {
        if !open && self.grab.active {
            self.grab.release(ctx, Drop::id(host));
        }
        self.was_open = open;
        if !open {
            self.keyboard = false;
        }
    }
}

/// The footer's height (`control-h`).
pub(crate) fn footer_height(dark: bool) -> f32 {
    StyleView::ua_only(SAVE_UA, StateBits::NONE, dark).px("height").unwrap_or(26.0)
}

/// Paints the Save/Cancel footer in `r` (right-aligned, the platform's button order: OK last on
/// Windows/Linux, FM1 QW11 `Policy::button_order` when it lands) and returns what was pressed.
pub(crate) fn footer(ui: &egui::Ui, cx: &WidgetCx<'_>, host: &StyleView<'_>, id: Id, r: Rect) -> Option<Footer> {
    let mut hit = None;
    let mut x = r.right();
    for (name, ua, label, f) in [("save", SAVE_UA, "Save", Footer::Save), ("cancel", CANCEL_UA, "Cancel", Footer::Cancel)] {
        let base = cx.part(host, name, ua);
        let g = paint::text_galley(ui.ctx(), &base, label, None);
        let w = g.size().x + paint::box_extra(&base).x;
        let br = Rect::from_min_max(pos2(x - w, r.top()), pos2(x, r.top() + r.height()));
        x -= w + 6.0;
        let bid = id.with(name);
        let resp = ui.interact(br, bid, egui::Sense::click());
        let mut bits = StateBits::NONE;
        bits.set(StateBits::HOVERED, resp.hovered());
        bits.set(StateBits::PRESSED, resp.is_pointer_button_down_on());
        let v = base.in_state(bits);
        let mut pc = cx.paint_cx(ui);
        let mut bp = pc.part(name);
        let geom = paint::paint_box(&mut bp, &v, br);
        let g = paint::text_galley(ui.ctx(), &v, label, None);
        paint::paint_text(&mut bp, &v, &g, geom.content, Align::Center);
        if resp.hovered()
            && let Some(c) = v.cursor()
        {
            ui.ctx().set_cursor_icon(c);
        }
        let ak = ui.input(|i| i.has_accesskit_action_request(bid, accesskit::Action::Click));
        if resp.clicked() || ak {
            hit = Some(f);
        }
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
    }
    hit
}

/// The closed button's chevron rect at the end of `content` and the label's rect before it.
pub(crate) fn split_chevron(host: &StyleView<'_>, chevron: &StyleView<'_>, content: Rect) -> (Rect, Rect) {
    let cw = chevron.px("width").unwrap_or(16.0);
    let cr = Rect::from_min_max(pos2(content.max.x - cw, content.min.y), content.max);
    let lr = Rect::from_min_max(content.min, pos2(cr.min.x - super::picker_glyphs::gap(host, 4.0), content.max.y));
    (cr, lr)
}

/// The popup surface's border box for a body of `body` size (plus a footer row when `footer`).
pub(crate) fn popup_size(surf: &StyleView<'_>, body: Vec2, footer: bool) -> Vec2 {
    let gap = super::picker_glyphs::gap(surf, 8.0);
    let f = if footer { gap + footer_height(surf.is_dark()) } else { 0.0 };
    vec2(body.x, body.y + f) + paint::box_extra(surf)
}
