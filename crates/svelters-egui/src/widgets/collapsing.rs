//! `collapsing-header`, svelters' own (OW1a2, ARCH-WIDGETS §6.1, WIDGET-PARITY §2.16): a header
//! row (the `header` paint part: the `twistie` chevron, DESIGN R14, and the `label`) above its
//! children, which are laid out only while `open` (`Widget::layout`; closed, the node is the header
//! row and the children get the hidden layout). The body is indented to the caption, past the
//! twistie. Replaces the self-painted wrapper and `layout.rs`'s `Tree::collapsing`.
//!
//! Units: Hoverable, Focusable, Pressable, Expandable (click on the header, Space/Enter, Right and
//! Left, AccessKit Expand/Collapse). States: `:expanded`, `:hover` (the header's `list-hover`
//! fill; section headers stay flat, DESIGN §2.3), `:active`, `:focus-visible` (1px ring inside the
//! header, R8 rows). AccessKit `Role::DisclosureTriangle` named by `text`, `expanded`.
//!
//! OW1a4:
//! - `accordion`: sibling headers with the same non-empty `accordion` name are one group, and a
//!   user opening one closes the others (`with_peer`, as radio's exclusivity), each announcing
//!   `changed(false)`;
//! - header actions: children inserted with `slot="end"` (the kind's second named slot,
//!   `kinds::extra_slots`) are real child nodes laid out right-aligned in the header row, out of
//!   the body's flow, drawn while the header is hovered or holds focus (`:hover`,
//!   `:focus-within`, VS Code's view title actions), open or closed; the caption elides before them;
//! - `icon` (a key, or `family:name`) between the twistie and the caption, `::part(icon)`, painted
//!   through ICR1's registry (`paint::paint_icon`).

use super::parts;
use crate::interact::{Expand, ExpandParams, Expandable, Focusable, Hoverable, Pressable};
use crate::paint::{self, Align, StyleView, Ua, UaRule, UaValue::*};
use crate::provider::{A11yCx, AvailableSpace, Event, LayoutCx, MeasureCx, Part, StateBits, Widget, WidgetCx};
use crate::scene::{NodeId, SceneRef};
use crate::taffy::{LayoutInput, LayoutOutput, RunMode};

/// The host: the body's gap below the header.
pub const UA: Ua = Ua(&[UaRule::base(&[("gap", Px(2.0))])]);

/// `::part(header)`: the row (VS Code pane header metrics, flat).
// The part's UA layer: `::part()` rules in a sheet override it (OW1a3, `parts::part_of`).
pub const HEADER: Ua = Ua(&[
    UaRule::base(&[
        ("height", Token("header-h")),
        ("gap", Px(4.0)),
        ("padding-right", Px(4.0)),
        ("outline-style", Keyword("none")),
        ("outline-color", Token("focus")),
        ("outline-width", Px(1.0)),
        ("outline-offset", Px(-1.0)),
        ("cursor", Keyword("pointer")),
    ]),
    UaRule::when(StateBits::HOVERED, &[("background-color", Token("list-hover"))]),
    UaRule::when(StateBits::FOCUS_VISIBLE, &[("outline-style", Keyword("solid"))]),
]);

/// `::part(twistie)`: the chevron's box and colour.
pub const TWISTIE: Ua = Ua(&[UaRule::base(&[("width", Token("twistie-w")), ("color", Token("icon"))]), UaRule::when(StateBits::DISABLED, &[("color", Token("disabled-fg"))])]);

/// `::part(icon)`: the header icon (16px, the icon colour).
pub const ICON: Ua = Ua(&[UaRule::base(&[("width", Px(16.0)), ("height", Px(16.0)), ("color", Token("icon"))]), UaRule::when(StateBits::DISABLED, &[("color", Token("disabled-fg"))])]);

/// The children in `slot="end"` (header actions), in order.
fn end_ids(s: SceneRef<'_>, ids: &[NodeId]) -> Vec<NodeId> {
    ids.iter().copied().filter(|&c| s.get(c).and_then(|n| n.ext(&crate::kinds::SLOT)).is_some_and(|n| *n == "end")).collect()
}

/// A header that shows or hides its children.
// the `#[part]` fields name the kind's `::part()`s; nothing reads them (paint parts)
#[allow(dead_code)]
#[svelters::widget(namespace = "sv", replaces = "collapsing-header", caps = CONTAINER | CUSTOM_LAYOUT | PAINTS_BOX | FOCUSABLE)]
pub struct CollapsingHeader {
    /// The header's caption.
    #[prop(measure)]
    text: String,
    /// Shows the children (`bind:open`). A host write lays it out again (`measure`).
    #[prop(bind, measure)]
    open: bool,
    /// The user opened or closed it.
    #[event]
    changed: Event<bool>,
    /// The group of sibling headers of which one is open at a time (QToolBox); empty: none.
    #[prop]
    accordion: String,
    /// An icon before the caption (a key, or `family:name`).
    #[prop(measure)]
    icon: String,
    /// The width the header actions take, from layout (0 without any).
    #[state]
    actions_w: f32,
    #[behavior]
    hover: Hoverable,
    #[behavior]
    focus: Focusable,
    #[behavior]
    press: Pressable,
    #[behavior(params = expand_params)]
    expand: Expandable,
    #[part]
    header: Part,
    #[part]
    twistie: Part,
    #[part(name = "label")]
    label_part: Part,
    #[part(name = "icon")]
    icon_part: Part,
}

impl CollapsingHeader {
    fn expand_params(&self) -> ExpandParams {
        ExpandParams { expanded: self.open, on_click: false }
    }

    /// The header row's size and the body's indent (the twistie plus the gap: the caption's x).
    fn header(&self, ctx: &egui::Context, host: &StyleView<'_>, head: &StyleView<'_>, tw: &StyleView<'_>, ic: &StyleView<'_>) -> (egui::Vec2, f32) {
        let t = paint::measure_text(ctx, host, &self.text, AvailableSpace::MaxContent);
        let twistie = tw.px("width").unwrap_or(16.0);
        let gap = head.px("gap").unwrap_or(4.0);
        let icon = self.icon_key(ic).map_or(0.0, |k| paint::icon_size(ctx, ic, &k).x + gap);
        let [pl, pr, pt, pb] = head.sides("padding");
        let h = head.px("height").unwrap_or(0.0).max(t.y + pt + pb);
        (egui::vec2(pl + twistie + gap + icon + t.x + pr, h), pl + twistie + gap)
    }

    /// The `icon` prop's key, after the part's `content` (`none` hides it).
    fn icon_key(&self, ic: &StyleView<'_>) -> Option<paint::IconKey> {
        super::checkbox::icon_key(&self.icon).and_then(|k| paint::content_icon(ic, &k))
    }

    /// The header row's size, the body's indent, the host's gap, the header's gap and right padding.
    fn header_of(&self, cx: &MeasureCx<'_>) -> (egui::Vec2, f32, f32, f32, f32) {
        let host = cx.style(UA);
        let head = parts::measured_part(cx, &host, "header", HEADER, false);
        let tw = parts::measured_part(cx, &host, "twistie", TWISTIE, false);
        let ic = parts::measured_part(cx, &host, "icon", ICON, false);
        let (s, indent) = self.header(cx.ctx, &host, &head, &tw, &ic);
        (s, indent, host.px("gap").unwrap_or(0.0), head.px("gap").unwrap_or(4.0), head.sides("padding")[1])
    }

    /// Accordion: closes the open sibling headers of the same group.
    fn close_group(&self, cx: &mut WidgetCx<'_>) {
        if self.accordion.is_empty() {
            return;
        }
        for id in cx.sibling_ids() {
            cx.with_peer::<CollapsingHeader, _>(id, |h, p| {
                if h.open && h.accordion == self.accordion {
                    p.set(&mut h.open, false);
                    p.emit(&h.changed, false);
                }
            });
        }
    }
}

impl Widget for CollapsingHeader {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, _avail: AvailableSpace) -> egui::Vec2 {
        self.header_of(cx).0
    }

    fn layout(&mut self, cx: &mut LayoutCx<'_>, inputs: LayoutInput) -> LayoutOutput {
        let (head, indent, gap, hgap, pr) = cx.with_measure(|m| self.header_of(m));
        let ends = end_ids(cx.scene(), cx.children());
        let open = self.open;
        let out = cx.out_of_flow(&ends, |cx| if open { cx.padded_layout(inputs, [indent, 0.0, head.y + gap, 0.0]) } else { cx.collapsed(inputs, head) });
        if inputs.run_mode == RunMode::PerformLayout {
            // the actions, right-aligned in the header row and centred in it
            let right = out.size.width - pr;
            let mut x = right;
            for &e in ends.iter().rev() {
                let sz = cx.measure(e, egui::vec2(out.size.width, head.y));
                x -= sz.x;
                cx.place(e, egui::pos2(x, ((head.y - sz.y) / 2.0).max(0.0)), sz);
                x -= hgap;
            }
            self.actions_w = if ends.is_empty() { 0.0 } else { right - x };
        }
        out
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        let resp = cx.interact(ui, self.units_sense());
        let fired = self.run_units(cx, ui, &resp);
        let hs = {
            let host = cx.style(UA);
            let head_v = parts::part_of(cx, &host, "header", HEADER, false);
            let tw = parts::part_of(cx, &host, "twistie", TWISTIE, false);
            let ic = parts::part_of(cx, &host, "icon", ICON, false);
            self.header(ui.ctx(), &host, &head_v, &tw, &ic).0
        };
        let row = egui::Rect::from_min_size(cx.rect().min, egui::vec2(cx.rect().width(), hs.y));
        // a click on the header row toggles (not one on the body's empty space)
        let mut set = fired.expand.iter().map(|Expand::Set(v)| *v).next_back();
        if fired.press.iter().any(|p| p.is_click()) && cx.enabled() && resp.interact_pointer_pos().is_none_or(|p| row.contains(p)) {
            set = Some(!self.open);
        }
        if let Some(v) = set.filter(|v| *v != self.open)
            && !cx.dispatch(&self.changed, v).prevented()
        {
            cx.set(&mut self.open, v);
            cx.request_measure();
            if v {
                self.close_group(cx);
            }
        }
        cx.set_state(StateBits::EXPANDED, self.open);
        if ui.is_rect_visible(row) {
            let host = cx.style(UA);
            let head_v = parts::part_of(cx, &host, "header", HEADER, false);
            let tw = parts::part_of(cx, &host, "twistie", TWISTIE, false);
            if host.state().contains(StateBits::HOVERED)
                && let Some(c) = head_v.cursor()
            {
                ui.ctx().set_cursor_icon(c);
            }
            let mut pc = cx.paint_cx(ui);
            let g = paint::paint_box(&mut pc.part("header"), &head_v, row);
            let twidth = tw.px("width").unwrap_or(16.0);
            let tr = egui::Rect::from_min_size(egui::pos2(g.content.left(), row.top()), egui::vec2(twidth, row.height()));
            let color = tw.color("color").unwrap_or(egui::Color32::PLACEHOLDER);
            let sq = egui::Rect::from_center_size(tr.center(), egui::Vec2::splat(twidth.min(row.height())));
            match tw.keyword("content") {
                Some(_) => {
                    let def = paint::IconKey::new("codicons", if self.open { "chevron-down" } else { "chevron-right" });
                    if let Some(k) = paint::content_icon(&tw, &def) {
                        paint::paint_icon(&mut pc.part("twistie"), &tw, &k, sq);
                    }
                }
                None => parts::paint_chevron(&mut pc.part("twistie"), sq, color, self.open),
            }
            let hgap = head_v.px("gap").unwrap_or(4.0);
            let mut x = tr.right() + hgap;
            let ic = parts::part_of(cx, &host, "icon", ICON, false);
            if let Some(k) = self.icon_key(&ic) {
                let sz = paint::icon_size(ui.ctx(), &ic, &k);
                let ir = egui::Rect::from_min_size(egui::pos2(x, row.center().y - sz.y / 2.0), sz);
                paint::paint_icon(&mut pc.part("icon"), &ic, &k, ir);
                x = ir.right() + hgap;
            }
            if !self.text.is_empty() {
                let end = if self.actions_w > 0.0 { g.content.right() - self.actions_w } else { g.content.right() };
                let lr = egui::Rect::from_min_max(egui::pos2(x, row.top()), egui::pos2(end.max(x), row.bottom()));
                let text = paint::elide(ui.ctx(), &host, &self.text, paint::Elide::End, lr.width());
                let gal = paint::text_galley(ui.ctx(), &host, &text, None);
                paint::paint_text(&mut pc.part("label"), &host, &gal, lr, Align::Start);
            }
            paint::paint_outline(&mut pc, &head_v, row);
        }
        // header actions show while the header is hovered or holds focus (VS Code view titles)
        let ends = end_ids(cx.scene(), cx.child_ids());
        let st = cx.style(UA).state();
        // (the pointer over an action is over the header too, though `:hover` is the action's then)
        let show = !ends.is_empty() && (resp.contains_pointer() || st.contains(StateBits::FOCUSED) || st.contains(StateBits::FOCUS_WITHIN));
        if ends.is_empty() && self.open {
            cx.children(ui);
        } else {
            for id in cx.child_ids().to_vec() {
                let end = ends.contains(&id);
                if (end && show) || (!end && self.open) {
                    cx.child(ui, id);
                }
            }
        }
        let (enabled, text) = (cx.enabled(), &self.text);
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::DisclosureTriangle, enabled, text));
        resp
    }

    fn a11y(&self, _cx: &A11yCx<'_>, node: &mut egui::accesskit::Node) {
        node.set_role(egui::accesskit::Role::DisclosureTriangle);
        if !self.text.is_empty() {
            node.set_label(self.text.as_str());
        }
        self.units_a11y(node);
    }

    fn consume_key(&self, key: egui::Key, m: egui::Modifiers) -> bool {
        self.units_claims(key, m)
    }
}
