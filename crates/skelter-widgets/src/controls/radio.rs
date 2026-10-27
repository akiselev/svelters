//! `radio-button` (ARCH-WIDGETS §6.1, WIDGET-PARITY §2.3, ported from svelters-egui
//! `widgets/radio.rs`): box, text, path (the dot); Hoverable, Focusable, Pressable, Toggleable (radio:
//! a click only checks), Roving. Paint parts `indicator` (the round box), `dot`, `icon` and `label`.
//!
//! **Exclusivity**: the radio buttons of one parent are a peer group
//! ([`PeerGroup`](crate::shim::core::PeerGroup)). A click checks this button, writes its `value` into
//! `group`, and asks the siblings to follow ([`PeerMsg::Checked`]): each takes the group value (when
//! this one has a value) and stays checked only if it stands for it, announcing
//! `changed(checked, group)` when either changed. A button with a `value` follows `group == value`
//! when the host writes either one (unannounced).
//!
//! **Roving**: the siblings are one Tab stop (the checked one, or the first; skelter-core's Tab order
//! from the peer group), and the arrow keys move focus and the check within them (Qt, GTK): the move
//! is a [`PeerTarget::Rove`] request, and the member it lands on checks itself ([`PeerMsg::Roved`]).
//!
//! States: `:hover`, `:active`, `:focus-visible` (the ring around the indicator, DESIGN R8),
//! `:checked` (R17: the dot plus the on-fill), `:disabled`.

use crate::controls::checkbox::{self, Indicator};
use crate::controls::common::{self, Units};
use crate::interact::{Focusable, Hoverable, Intents, Mods, PressParams, Pressable, Rove, Roving, RovingParams, Sense, StateBits, Toggle, ToggleParams, Toggleable, Unit};
use crate::kind_props;
use crate::paint::{PaintCx, StyleView, Ua, UaRule, UaValue::*};
use crate::shim::core::{A11yCx, Event, EventCx, Handled, Kind, KindSpec, MeasureCx, NameFrom, PeerGroup, PeerMsg, PeerTarget, PropError, PropId, RecordCx, SignalSpec, Value, ValueType};
use skelter_base::accesskit;
use skelter_base::color::Color;
use skelter_base::geom::{Point, Rect};
use skelter_base::key::Key;
use svelters_style::ComputedStyle;

/// `::part(indicator)`: the round box (VS Code has no radio; DESIGN §1.3 sizes it as the checkbox).
pub const INDICATOR: Ua = Ua(&[
    UaRule::base(&[
        ("width", Px(18.0)),
        ("height", Px(18.0)),
        ("background-color", Token("checkbox-bg")),
        ("border-color", Token("checkbox-border-color")),
        ("border-width", Token("stroke")),
        ("border-radius", Px(9.0)),
    ]),
    // R3: hover steps the fill and the edge (the fill step alone is faint in Dark Modern)
    UaRule::when(StateBits::HOVERED, &[("background-color", Token("button-bg-hover")), ("border-color", Token("muted"))]),
    UaRule::when(StateBits::CHECKED, &[("background-color", Token("toggle-on-bg")), ("border-color", Token("toggle-on-border"))]),
    UaRule::when(StateBits::PRESSED, &[("background-color", Token("button-bg-active"))]),
]);

/// `::part(dot)`: the checked dot's colour and diameter.
pub const DOT: Ua = Ua(&[UaRule::base(&[("color", Token("fg")), ("width", Px(8.0))]), UaRule::when(StateBits::DISABLED, &[("color", Token("disabled-fg"))])]);

kind_props! {
    /// The radio button's props.
    pub struct RadioProps;
    /// The caption.
    text "text": String = Str("") ; Str, Layout;
    /// Checked (`bind:checked`); a click checks it, a sibling's check unchecks it.
    checked "checked": bool = Bool(false) ; Bool, Paint, bindable;
    /// What this choice stands for: a click writes it into `group`, and `checked` follows
    /// `group == value`.
    value "value": String = Str("") ; Str, Paint;
    /// The group's value (`bind:group`).
    group "group": String = Str("") ; Str, Paint, bindable;
    /// WIDGET-PARITY §2.3: an icon before the caption (a key, or `family:name`).
    icon "icon": String = Str("") ; Str, Layout;
    disabled "disabled": bool = Bool(false) ; Bool, Paint;
}

/// The signals, in spec order.
pub const SIGNALS: &[SignalSpec] = &[
    SignalSpec::new("changed", &[("checked", ValueType::Bool), ("group", ValueType::Str)], "The user's click (or an arrow key) changed it, or a sibling's unchecked it. Preventing the user's check keeps the group as it was.").cancelable(),
    SignalSpec::new("clicked", &[], "Pressed and released (or Space, Enter, AccessKit `Click`)."),
];

pub static SPEC: KindSpec = KindSpec { tag: "radio-button", props: RadioProps::SPECS, signals: SIGNALS, slots: &[], children: false, role: accesskit::Role::RadioButton, name_from: NameFrom::Prop("text"), text_prop: Some("text"), focusable: true };

const IND: Indicator = Indicator { name: "indicator", ua: INDICATOR, host_css: false };

/// Paints the radio button at `rect` under `host` (its style in this state, `:checked` included) with
/// its parts' computed styles: indicator, dot when checked, icon, caption, focus ring. For foreign
/// painters too.
pub fn paint(pc: &mut PaintCx, host: &StyleView<'_>, parts: &[(&'static str, &ComputedStyle)], rect: Rect, text: &str, icon: &str) {
    let dot_css = checkbox::part_css(parts, "dot");
    checkbox::paint_indicator(pc, host, parts, rect, IND, text, icon, |pc, host, ir| {
        if host.state().contains(StateBits::CHECKED) {
            let dot = checkbox::part_with(host, dot_css, DOT, false);
            let d = dot.px("width").unwrap_or(8.0);
            let color = dot.color("color").unwrap_or(Color::BLACK);
            let c = Point::new((ir.min.x + ir.max.x) / 2.0, (ir.min.y + ir.max.y) / 2.0);
            checkbox::paint_dot(pc, c, d / 2.0, color);
        }
    });
}

/// One choice of several.
#[derive(Debug, Default)]
pub struct RadioButton {
    pub props: RadioProps,
    units: Units,
    hover: Hoverable,
    focus: Focusable,
    press: Pressable,
    toggle: Toggleable,
    roving: Roving,
}

impl RadioButton {
    fn toggle_params(&self) -> ToggleParams {
        ToggleParams { checked: self.props.checked, radio: true, ..ToggleParams::default() }
    }
    const ROVING: RovingParams = RovingParams { on: true };

    fn changed_args(checked: bool, group: &str) -> Vec<Value> {
        vec![Value::Bool(checked), Value::str(group)]
    }

    /// A host write of `group` or `value` re-derives `checked` (unannounced).
    fn derive(&mut self) {
        if !self.props.value.is_empty() {
            self.props.checked = self.props.group == self.props.value;
        }
    }

    /// `state` with the prop mirrors (`:checked`, `:disabled`).
    fn mirror(&self, mut state: StateBits) -> StateBits {
        state.set(StateBits::CHECKED, self.props.checked);
        state.set(StateBits::DISABLED, self.props.disabled);
        state
    }

    /// Checks this button (a click, or a roving move landing here): announces it, writes `checked`
    /// and the group value, and asks the siblings to follow. Nothing when it already is.
    fn check(&mut self, cx: &mut EventCx) {
        let p = &mut self.props;
        let new_group = if p.value.is_empty() { p.group.clone() } else { p.value.clone() };
        if p.checked && new_group == p.group {
            return;
        }
        // EV1: the signal goes out before the default action (the prop writes)
        if common::emit_sync(cx, &SPEC, "changed", Self::changed_args(true, &new_group)) {
            return;
        }
        let p = &mut self.props;
        if !p.checked {
            p.checked = true;
            cx.prop_changed(P::checked.id());
        }
        if p.group != new_group {
            p.group = new_group;
            cx.prop_changed(P::group.id());
        }
        let writes_group = !p.value.is_empty();
        cx.peer_request(PeerTarget::Others, PeerMsg::Checked(vec![Value::str(&p.group), Value::Bool(writes_group)]));
    }

    /// A sibling was checked with group value `group`: this button takes the value (when the
    /// sibling has a value) and is checked only if it stands for it.
    fn follow(&mut self, cx: &mut EventCx, group: &str, writes_group: bool) {
        let p = &mut self.props;
        let (was, g0) = (p.checked, p.group.clone());
        if writes_group && p.group != group {
            p.group = group.to_owned();
            cx.prop_changed(P::group.id());
        }
        let now = !p.value.is_empty() && p.group == p.value;
        if p.checked != now {
            p.checked = now;
            cx.prop_changed(P::checked.id());
        }
        if was != p.checked || g0 != p.group {
            common::emit(cx, &SPEC, "changed", Self::changed_args(p.checked, &p.group));
        }
    }
}

impl Kind for RadioButton {
    fn parts() -> &'static [&'static str] {
        &["indicator", "mark", "icon", "label"]
    }
    fn prop_state(&self) -> crate::interact::StateBits {
        self.mirror(crate::interact::StateBits::NONE)
    }
    fn prop_states() -> crate::interact::StateBits {
        crate::interact::StateBits::CHECKED
    }
    fn spec() -> &'static KindSpec {
        &SPEC
    }
    fn kind_spec(&self) -> &'static KindSpec {
        &SPEC
    }
    fn set_prop(&mut self, prop: PropId, v: &Value) -> Result<(), PropError> {
        self.props.set(prop, v)?;
        self.derive();
        Ok(())
    }
    fn get_prop(&self, prop: PropId) -> Option<Value> {
        self.props.get(prop)
    }

    fn measure(&self, cx: &mut MeasureCx, _known: taffy::Size<Option<f32>>, avail: taffy::Size<taffy::AvailableSpace>) -> taffy::Size<f32> {
        let host = checkbox::host_in(&cx.style, checkbox::UA, self.mirror(cx.style.state));
        let s = checkbox::measure_indicator(cx.text, &host, cx.style.parts, IND, &self.props.text, &self.props.icon);
        // a definite width caps the caption (CSS fit-content; it elides past it)
        taffy::Size { width: common::fit_width(s.x, avail.width), height: s.y }
    }

    fn record(&self, cx: &mut RecordCx) {
        let host = checkbox::host_in(&cx.style, checkbox::UA, self.mirror(cx.style.state));
        let parts = cx.style.parts;
        let rect = cx.rect();
        paint(&mut PaintCx::of(cx), &host, parts, rect, &self.props.text, &self.props.icon);
    }

    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        *cx.state = self.mirror(*cx.state);
        cx.enabled = !self.props.disabled;
        let tp = self.toggle_params();
        let pp = PressParams::default();
        let sense = Sense::HOVER.union(self.focus.sense(&Default::default())).union(self.press.sense(&pp)).union(self.toggle.sense(&tp));
        let (mut pressed, mut toggled, mut roved) = (Intents::default(), Intents::default(), Intents::<Rove>::default());
        let (hover, focus, press, toggle, roving) = (&mut self.hover, &mut self.focus, &mut self.press, &mut self.toggle, &mut self.roving);
        let pass = self.units.run(cx, ev, sense, |icx| {
            hover.input(&(), icx, &mut Intents::default());
            focus.input(&Default::default(), icx, &mut Intents::default());
            press.input(&pp, icx, &mut pressed);
            toggle.input(&tp, icx, &mut toggled);
            roving.input(&Self::ROVING, icx, &mut roved);
        });
        if pressed.iter().any(|p| p.is_click()) {
            common::emit(cx, &SPEC, "clicked", Vec::new());
        }
        // Roving: the member the arrow lands on takes focus and checks itself; the rest follow it
        if let Some(r) = roved.iter().last().copied() {
            cx.peer_request(PeerTarget::Rove(r), PeerMsg::Roved);
        }
        if toggled.iter().any(|t| matches!(t, Toggle::Toggled { checked: true, .. })) {
            self.check(cx);
        }
        cx.state.set(StateBits::CHECKED, self.props.checked);
        if (ev.is_pointer() && (pass.resp.hovered || pass.resp.down_on)) || !pressed.is_empty() || !toggled.is_empty() || !roved.is_empty() { Handled::Yes } else { Handled::No }
    }

    fn a11y(&self, cx: &mut A11yCx) {
        cx.node.set_role(accesskit::Role::RadioButton);
        if !self.props.text.is_empty() {
            cx.node.set_label(self.props.text.clone());
        }
        if self.props.disabled {
            cx.node.set_disabled();
        }
        self.focus.a11y(&Default::default(), cx.node);
        self.press.a11y(&PressParams::default(), cx.node);
        self.toggle.a11y(&self.toggle_params(), cx.node);
    }

    fn claims_key(&self, key: Key, m: skelter_base::event::Modifiers) -> bool {
        let m = Mods::of(m, false);
        self.press.claims(&PressParams::default(), key, m) || self.roving.claims(&Self::ROVING, key, m)
    }

    fn peer_group(&self) -> Option<PeerGroup> {
        Some(PeerGroup { tag: SPEC.tag, key: None, stop: "checked" })
    }

    fn peer(&mut self, cx: &mut EventCx, msg: &PeerMsg) -> Handled {
        *cx.state = self.mirror(*cx.state);
        match msg {
            PeerMsg::Roved => self.check(cx),
            PeerMsg::Checked(args) => {
                let group = args.first().and_then(Value::as_str).unwrap_or("").to_owned();
                let writes = args.get(1).and_then(Value::as_bool).unwrap_or(false);
                self.follow(cx, &group, writes);
            }
        }
        cx.state.set(StateBits::CHECKED, self.props.checked);
        Handled::Yes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::testkit;
    use crate::interact::roving::group::Group;
    use crate::paint::ua_token;
    use skelter_base::change::ChangeClass;
    use skelter_base::color::Fill;
    use skelter_base::display::{Prim, Quad};
    use skelter_base::geom::Size;

    const SIZE: Size = Size { w: 90.0, h: 18.0 };

    fn radio(text: &str, value: &str) -> RadioButton {
        let mut r = RadioButton::default();
        r.props.text = text.into();
        r.props.value = value.into();
        r
    }

    fn three() -> Group<RadioButton> {
        Group::new(vec![radio("A", ""), radio("B", ""), radio("C", "")], SIZE)
    }

    fn changed(i: usize, checked: bool, group: &str) -> (usize, &'static str, Vec<Value>) {
        (i, "changed", RadioButton::changed_args(checked, group))
    }

    #[test]
    fn spec() {
        let s = RadioButton::spec();
        assert_eq!(s.tag, "radio-button");
        assert_eq!(s.role, accesskit::Role::RadioButton);
        assert!(s.prop_spec(s.prop("checked").unwrap()).unwrap().bindable);
        assert!(s.prop_spec(s.prop("group").unwrap()).unwrap().bindable);
        assert!(!s.prop_spec(s.prop("value").unwrap()).unwrap().bindable);
        assert_eq!(s.prop_spec(s.prop("icon").unwrap()).unwrap().change, ChangeClass::Layout);
        assert_eq!(s.signals[s.signal("changed").unwrap().0 as usize].args, &[("checked", ValueType::Bool), ("group", ValueType::Str)]);
        assert!(s.signal("clicked").is_some());
        assert_eq!(RadioButton::default().peer_group(), Some(PeerGroup { tag: "radio-button", key: None, stop: "checked" }));
    }

    #[test]
    fn records_unchecked_checked_and_states() {
        let mut r = radio("One", "");
        let (list, _) = testkit::record(&r, SIZE, StateBits::NONE);
        assert_eq!(testkit::kinds(&list), ["quad", "glyphs"]);
        let Prim::Quad(q) = &list.items[0] else { unreachable!() };
        assert_eq!((q.rect, q.radii.tl), (Rect::new(0.0, 0.0, 18.0, 18.0), 9.0), "the round box");
        assert_eq!(q.fill, Fill::Solid(ua_token(true, "checkbox-bg").unwrap()));

        r.props.checked = true;
        let (list, _) = testkit::record(&r, SIZE, StateBits::NONE);
        assert_eq!(testkit::kinds(&list), ["quad", "glyphs", "quad"], "the dot after the caption");
        let Prim::Quad(q) = &list.items[0] else { unreachable!() };
        assert_eq!(q.fill, Fill::Solid(ua_token(true, "toggle-on-bg").unwrap()), "R17: dot plus on-fill");
        let Prim::Quad(dot) = &list.items[2] else { unreachable!() };
        assert_eq!((dot.rect, dot.radii.tl), (Rect::new(5.0, 5.0, 13.0, 13.0), 4.0));
        assert_eq!(dot.fill, Fill::Solid(ua_token(true, "fg").unwrap()));

        let (list, _) = testkit::record(&r, SIZE, StateBits::HOVERED);
        let Prim::Quad(q) = &list.items[0] else { unreachable!() };
        assert_eq!(q.fill, Fill::Solid(ua_token(true, "toggle-on-bg").unwrap()), "checked wins over hover (later rule)");
        r.props.checked = false;
        let (list, _) = testkit::record(&r, SIZE, StateBits::HOVERED);
        let Prim::Quad(q) = &list.items[0] else { unreachable!() };
        assert_eq!(q.fill, Fill::Solid(ua_token(true, "button-bg-hover").unwrap()));

        let (list, _) = testkit::record(&r, SIZE, StateBits::FOCUSED | StateBits::FOCUS_VISIBLE);
        assert_eq!(testkit::kinds(&list), ["quad", "glyphs", "quad"]);
        let Some(Prim::Quad(Quad { rect, .. })) = list.items.last() else { unreachable!() };
        assert_eq!(*rect, Rect::new(-3.0, -3.0, 21.0, 21.0), "the ring around the indicator, 2px at +1");

        r.props.checked = true;
        r.props.disabled = true;
        let (list, _) = testkit::record(&r, SIZE, StateBits::NONE);
        let Prim::Quad(dot) = &list.items[2] else { unreachable!() };
        assert_eq!(dot.fill, Fill::Solid(ua_token(true, "disabled-fg").unwrap()));
    }

    #[test]
    fn click_checks_and_unchecks_the_siblings() {
        let mut g = three();
        g.click(1);
        assert_eq!(g.bools("checked"), [false, true, false]);
        assert_eq!(g.take(), [(1, "clicked", vec![]), changed(1, true, "")]);
        assert!(g.states[1].contains(StateBits::CHECKED));
        g.click(2);
        assert_eq!(g.bools("checked"), [false, false, true]);
        assert_eq!(g.take(), [(2, "clicked", vec![]), changed(2, true, ""), changed(1, false, "")]);
        assert!(!g.states[1].contains(StateBits::CHECKED));
        assert_eq!(g.changed, [(1, "checked"), (2, "checked"), (1, "checked")]);
        assert_eq!(g.tab_stop(0), 2, "the checked member is the group's Tab stop");
    }

    #[test]
    fn a_checked_radio_stays_checked() {
        let mut g = three();
        g.click(0);
        g.take();
        g.click(0);
        assert_eq!(g.bools("checked"), [true, false, false]);
        assert_eq!(g.take(), [(0, "clicked", vec![])], "no change to announce");
    }

    #[test]
    fn space_and_accesskit_check() {
        let mut g = three();
        g.focus(1);
        assert!(g.states[1].contains(StateBits::FOCUS_VISIBLE));
        g.key(Key::Space);
        assert_eq!(g.bools("checked"), [false, true, false]);
        g.take();
        g.a11y_click(2);
        assert_eq!(g.bools("checked"), [false, false, true]);
        assert_eq!(g.take(), [(2, "clicked", vec![]), changed(2, true, ""), changed(1, false, "")]);
    }

    #[test]
    fn arrows_move_focus_and_the_check() {
        let mut g = three();
        g.focus(0);
        g.key(Key::Space);
        g.take();
        g.key(Key::ArrowDown);
        assert_eq!(g.focused, Some(1));
        assert_eq!(g.bools("checked"), [false, true, false]);
        assert_eq!(g.take(), [changed(1, true, ""), changed(0, false, "")], "no `clicked` on a roving move");
        assert!(g.states[1].contains(StateBits::FOCUS_VISIBLE) && !g.states[0].contains(StateBits::FOCUSED));
        g.key(Key::End);
        assert_eq!((g.focused, g.bools("checked")), (Some(2), vec![false, false, true]));
        g.key(Key::ArrowRight);
        assert_eq!((g.focused, g.bools("checked")), (Some(0), vec![true, false, false]), "wraps");
        g.key(Key::ArrowUp);
        assert_eq!((g.focused, g.bools("checked")), (Some(2), vec![false, false, true]));
        g.key(Key::Home);
        assert_eq!((g.focused, g.bools("checked")), (Some(0), vec![true, false, false]));
        assert!(g.nodes[0].claims_key(Key::ArrowDown, Default::default()));
    }

    #[test]
    fn values_write_the_group() {
        let mut g = Group::new(vec![radio("A", "a"), radio("B", "b"), radio("C", "c")], SIZE);
        g.click(1);
        assert_eq!(g.bools("checked"), [false, true, false]);
        assert_eq!((0..3).map(|i| g.get(i, "group")).collect::<Vec<_>>(), [Value::str("b"), Value::str("b"), Value::str("b")]);
        assert_eq!(g.take(), [(1, "clicked", vec![]), changed(1, true, "b"), changed(0, false, "b"), changed(2, false, "b")]);
        // the host writes `group`: `checked` follows, unannounced
        for i in 0..3 {
            g.set(i, "group", Value::str("c"));
        }
        assert_eq!(g.bools("checked"), [false, false, true]);
        assert!(g.take().is_empty());
    }

    #[test]
    fn disabled_does_not_check() {
        let mut g = three();
        g.set(0, "disabled", Value::Bool(true));
        g.click(0);
        g.a11y_click(0);
        assert_eq!(g.bools("checked"), [false, false, false]);
        assert!(g.take().is_empty());
    }

    #[test]
    fn a11y_node() {
        let mut r = radio("One", "");
        let n = testkit::a11y(&r, StateBits::NONE, SIZE);
        assert_eq!(n.role(), accesskit::Role::RadioButton);
        assert_eq!(n.label(), Some("One"));
        assert_eq!(n.toggled(), Some(accesskit::Toggled::False));
        assert!(n.supports_action(accesskit::Action::Click));
        r.props.checked = true;
        assert_eq!(testkit::a11y(&r, StateBits::NONE, SIZE).toggled(), Some(accesskit::Toggled::True));
    }

    #[test]
    fn a_prevented_changed_keeps_the_group() {
        let mut r = RadioButton::default();
        r.props.value = "a".into();
        let mut d = testkit::Driver::new(r, skelter_base::geom::Size::new(80.0, 20.0));
        d.prevent = vec!["changed"];
        d.click();
        assert!(d.names().contains(&"changed"));
        assert!(!d.kind.props.checked && d.kind.props.group.is_empty());
    }
}
