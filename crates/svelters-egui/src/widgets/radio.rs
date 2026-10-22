//! `radio-button`, svelters' own (OW1a2, ARCH-WIDGETS §6.1, WIDGET-PARITY §2.3): box, text, path
//! (the dot); Hoverable, Focusable, Pressable, Toggleable (radio: a click only checks), Roving.
//! Paint parts `indicator` (the round box), `dot`, `icon` and `label`. Replaces the
//! `egui::RadioButton` wrapper.
//!
//! **Exclusivity** follows the wrapper: a click checks this button, writes its `value` into
//! `group`, and unchecks the sibling radio buttons of the same parent (writing their `group` too
//! when this one has a value), each announcing `changed(checked, group)`. A button with a `value`
//! follows `group == value` when the host writes either one (unannounced).
//!
//! **Roving**: the siblings are one Tab stop (the checked one, or the first), and the arrow keys
//! move focus and the check within them (Qt, GTK).
//!
//! States: `:hover`, `:active`, `:focus-visible` (the ring around the indicator, DESIGN R8),
//! `:checked` (R17: the dot plus the on-fill), `:disabled`.

use super::parts;
use crate::interact::{FocusParams, Focusable, Hoverable, Pressable, Roving, RovingParams, Toggle, ToggleParams, Toggleable};
use crate::paint::{Ua, UaRule, UaValue::*};
use crate::provider::{A11yCx, Args, AvailableSpace, Event, MeasureCx, Part, StateBits, Widget, WidgetCx};
use crate::scene::NodeId;
use svelters_host::exports::Value;

/// `::part(indicator)`: the round box (VS Code has no radio; DESIGN §1.3 sizes it as the checkbox).
// TODO(OW-CSS ::part): the part's computed style replaces this table where a sheet sets it
pub const INDICATOR: Ua = Ua(&[
    UaRule::base(&[
        ("width", Px(18.0)),
        ("height", Px(18.0)),
        ("background-color", Token("checkbox-bg")),
        ("border-color", Token("checkbox-border-color")),
        ("border-width", Token("stroke")),
        ("border-radius", Px(9.0)),
    ]),
    UaRule::when(StateBits::HOVERED, &[("background-color", Token("button-bg-hover"))]),
    UaRule::when(StateBits::CHECKED, &[("background-color", Token("toggle-on-bg")), ("border-color", Token("toggle-on-border"))]),
    UaRule::when(StateBits::PRESSED, &[("background-color", Token("button-bg-active"))]),
]);

/// `::part(dot)`: the checked dot's colour and diameter.
pub const DOT: Ua = Ua(&[UaRule::base(&[("color", Token("fg")), ("width", Px(8.0))]), UaRule::when(StateBits::DISABLED, &[("color", Token("disabled-fg"))])]);

/// One choice of several.
// the `#[part]` fields name the kind's `::part()`s; nothing reads them (paint parts)
#[allow(dead_code)]
#[svelters::widget(namespace = "sv", replaces = "radio-button", caps = FOCUSABLE | PAINTS_BOX)]
pub struct RadioButton {
    #[prop(measure)]
    text: String,
    #[prop(bind)]
    checked: bool,
    #[prop]
    value: String,
    #[prop(bind)]
    group: String,
    /// WIDGET-PARITY §2.3: an icon before the caption (a key, or `family:name`).
    #[prop(measure)]
    icon: String,
    /// The user's click changed it (or a sibling's unchecked it): `(checked, group)`.
    #[event]
    changed: Event<Args>,
    #[event]
    clicked: Event<()>,
    #[behavior]
    hover: Hoverable,
    #[behavior(params = focus_params)]
    focus: Focusable,
    #[behavior]
    press: Pressable,
    #[behavior(params = toggle_params)]
    toggle: Toggleable,
    #[behavior(params = roving_params)]
    roving: Roving,
    #[part]
    indicator: Part,
    #[part]
    dot: Part,
    #[part(name = "icon")]
    icon_part: Part,
    #[part(name = "label")]
    label_part: Part,
    /// This frame: the group is one Tab stop and this button is not it.
    roving_member: bool,
}

impl RadioButton {
    fn toggle_params(&self) -> ToggleParams {
        ToggleParams { checked: self.checked, radio: true, ..ToggleParams::default() }
    }
    fn focus_params(&self) -> FocusParams {
        FocusParams { tab_stop: !self.roving_member, ..FocusParams::default() }
    }
    fn roving_params(&self) -> RovingParams {
        RovingParams { on: true }
    }
    fn changed_args(checked: bool, group: &str) -> Args {
        Args(vec![Value::Bool(checked), Value::Str(group.to_string())])
    }
}

/// The radio buttons among this node's siblings (itself included), in order, with `checked`.
fn members(cx: &mut WidgetCx<'_>, me: NodeId, my_checked: bool) -> Vec<(NodeId, bool)> {
    let mut v = Vec::new();
    for id in cx.sibling_ids() {
        if id == me {
            v.push((id, my_checked));
        } else if let Some(c) = cx.with_peer::<RadioButton, _>(id, |r, _| r.checked) {
            v.push((id, c));
        }
    }
    v
}

impl Widget for RadioButton {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, _avail: AvailableSpace) -> egui::Vec2 {
        super::checkbox::measure_with_indicator(cx, super::checkbox::UA, "indicator", INDICATOR, &self.text, &self.icon, false)
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        // a host write of `group` or `value` re-derives `checked` (unannounced, as the wrapper)
        if !self.value.is_empty() {
            self.checked = self.group == self.value;
        }
        let me = cx.node().id();
        let group = members(cx, me, self.checked);
        let stop = group.iter().find(|m| m.1).or(group.first()).map(|m| m.0);
        self.roving_member = group.len() > 1 && stop != Some(me);
        let mut sense = self.units_sense();
        if self.roving_member {
            sense -= egui::Sense::FOCUSABLE;
        }
        let resp = cx.interact(ui, sense);
        let fired = self.run_units(cx, ui, &resp);
        if fired.press.iter().any(|p| p.is_click()) {
            cx.dispatch(&self.clicked, ());
        }
        let check = fired.toggle.iter().any(|t| matches!(t, Toggle::Toggled { checked: true, .. }));
        // Roving: focus and check the member the arrow lands on; the rest follow it
        if let Some(r) = fired.roving.iter().last().copied().filter(|_| group.len() > 1) {
            let at = group.iter().position(|m| m.0 == me).unwrap_or(0);
            let to = group[r.target(at, group.len())].0;
            if to != me
                && let Some(Some((g, writes))) = cx.with_peer::<RadioButton, _>(to, |r, p| {
                    p.focus();
                    r.check(p)
                })
            {
                self.follow(cx, &g, writes);
                self.uncheck_siblings_except(cx, &[me, to], &g, writes);
            }
        }
        if check {
            let new_group = if self.value.is_empty() { self.group.clone() } else { self.value.clone() };
            let changes = !self.checked || new_group != self.group;
            // EV1: a filter that prevents the default action leaves this button and its siblings
            if !changes || !cx.dispatch(&self.changed, Self::changed_args(true, &new_group)).prevented() {
                cx.set(&mut self.checked, true);
                cx.set(&mut self.group, new_group.clone());
                self.uncheck_siblings(cx, me, &new_group);
            }
        }
        cx.set_state(StateBits::CHECKED, self.checked);
        if ui.is_rect_visible(cx.rect()) {
            super::checkbox::paint_indicator_kind(ui, cx, super::checkbox::UA, "indicator", INDICATOR, false, &self.text, &self.icon, |pc, host, ir| {
                if host.state().contains(StateBits::CHECKED) {
                    let dot = parts::part_view(host, DOT, false);
                    let d = dot.px("width").unwrap_or(8.0);
                    let color = dot.color("color").unwrap_or(egui::Color32::PLACEHOLDER);
                    parts::paint_dot(&mut pc.part("dot"), ir.center(), d / 2.0, color, egui::Stroke::NONE);
                }
            });
        }
        let (enabled, checked, text) = (cx.enabled(), self.checked, &self.text);
        resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::RadioButton, enabled, checked, text));
        resp
    }

    fn a11y(&self, _cx: &A11yCx<'_>, node: &mut egui::accesskit::Node) {
        node.set_role(egui::accesskit::Role::RadioButton);
        self.units_a11y(node);
    }

    fn consume_key(&self, key: egui::Key, m: egui::Modifiers) -> bool {
        self.units_claims(key, m)
    }
}

impl RadioButton {
    /// Roving onto this peer: checks it like a click would. Returns the group value and whether
    /// the siblings take it (this button has a value), if anything changed.
    fn check(&mut self, p: &mut crate::provider::PeerCx) -> Option<(String, bool)> {
        let new_group = if self.value.is_empty() { self.group.clone() } else { self.value.clone() };
        if self.checked && new_group == self.group {
            return None;
        }
        p.emit(&self.changed, Self::changed_args(true, &new_group));
        p.set(&mut self.checked, true);
        p.set(&mut self.group, new_group.clone());
        Some((new_group, !self.value.is_empty()))
    }

    /// This button after a sibling was checked (the same rule as the sibling pass).
    fn follow(&mut self, cx: &mut WidgetCx<'_>, group: &str, writes_group: bool) {
        let (was, g0) = (self.checked, self.group.clone());
        if writes_group && self.group != group {
            cx.set(&mut self.group, group.to_string());
        }
        let now = !self.value.is_empty() && self.group == self.value;
        cx.set(&mut self.checked, now);
        if was != now || g0 != self.group {
            cx.dispatch(&self.changed, Self::changed_args(now, &self.group));
        }
    }

    fn uncheck_siblings(&mut self, cx: &mut WidgetCx<'_>, me: NodeId, group: &str) {
        let writes_group = !self.value.is_empty();
        self.uncheck_siblings_except(cx, &[me], group, writes_group);
    }

    /// The wrapper's sibling pass: a sibling takes the group value (when the checked button has a
    /// value) and is checked only if it stands for it.
    fn uncheck_siblings_except(&self, cx: &mut WidgetCx<'_>, skip: &[NodeId], group: &str, writes_group: bool) {
        for id in cx.sibling_ids() {
            if skip.contains(&id) {
                continue;
            }
            cx.with_peer::<RadioButton, _>(id, |r, p| {
                let (was, g0) = (r.checked, r.group.clone());
                if writes_group && r.group != group {
                    p.set(&mut r.group, group.to_string());
                }
                let now = !r.value.is_empty() && r.group == r.value;
                if r.checked != now {
                    p.set(&mut r.checked, now);
                }
                if was != r.checked || g0 != r.group {
                    p.emit(&r.changed, Self::changed_args(r.checked, &r.group));
                }
            });
        }
    }
}
