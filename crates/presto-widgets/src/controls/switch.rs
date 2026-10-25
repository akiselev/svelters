//! `switch` (ARCH-WIDGETS §6.1, ported from svelters-egui `widgets/switch.rs`): box, text, path;
//! Hoverable, Focusable, Pressable, Toggleable. Paint parts `track` and `knob`, and the caption
//! (`label`).
//!
//! VS Code has no switch, so its look follows the user's Q4 decision: DESIGN R1–R18 with Fluent 2
//! metrics on the VS Code tokens (40×20 track, radius 10, 12 px knob that grows to 14 on hover).
//! R17: the state shows by fill **and** knob position, not colour alone: off is a bordered track
//! with an `fg` knob on the left, on is the accent fill with an `accent-fg` knob on the right.
//! The knob is drawn at its end position (as under reduced motion); its slide is a CSS
//! `transition` on the part once presto's animation engine runs part transitions.
//! Signals: `changed(active)`, `clicked`.

use crate::controls::checkbox::{self, Indicator};
use crate::controls::common::{self, Units};
use crate::interact::{Focusable, Hoverable, Intents, Mods, PressParams, Pressable, Sense, StateBits, Toggle, ToggleParams, Toggleable, Unit};
use crate::kind_props;
use crate::paint::{PaintCx, StyleView, Ua, UaRule, UaValue::*};
use crate::shim::core::{A11yCx, Event, EventCx, Handled, Kind, KindSpec, MeasureCx, NameFrom, PropError, PropId, RecordCx, SignalSpec, Value, ValueType};
use presto_base::accesskit;
use presto_base::color::Color;
use presto_base::geom::{Point, Rect};
use presto_base::key::Key;
use svelters_style::ComputedStyle;

/// The host: caption, gap, focus ring (around the track, R8: standalone controls 1 px at +1).
pub const UA: Ua = Ua(&[
    UaRule::base(&[
        ("gap", Px(8.0)),
        ("outline-style", Keyword("none")),
        ("outline-color", Token("focus")),
        ("outline-width", Px(1.0)),
        ("outline-offset", Px(1.0)),
    ]),
    UaRule::when(StateBits::FOCUS_VISIBLE, &[("outline-style", Keyword("solid"))]),
]);

/// `::part(track)`.
pub const TRACK: Ua = Ua(&[
    UaRule::base(&[
        ("width", Px(40.0)),
        ("height", Px(20.0)),
        ("background-color", Token("control-bg")),
        ("border-color", Token("control-border")),
        ("border-width", Token("stroke")),
        ("border-radius", Px(10.0)),
    ]),
    UaRule::when(StateBits::HOVERED, &[("background-color", Token("button-bg-hover"))]),
    UaRule::when(StateBits::CHECKED, &[("background-color", Token("accent")), ("border-color", Token("accent"))]),
    UaRule::when(StateBits(StateBits::CHECKED.0 | StateBits::HOVERED.0), &[("background-color", Token("accent-hover")), ("border-color", Token("accent-hover"))]),
]);

/// `::part(knob)`: its colour and size.
pub const KNOB: Ua = Ua(&[
    UaRule::base(&[("color", Token("fg")), ("width", Px(12.0)), ("height", Px(12.0))]),
    UaRule::when(StateBits::HOVERED, &[("width", Px(14.0)), ("height", Px(14.0))]),
    UaRule::when(StateBits::CHECKED, &[("color", Token("accent-fg"))]),
    UaRule::when(StateBits::DISABLED, &[("color", Token("disabled-fg"))]),
]);

kind_props! {
    /// The switch's props.
    pub struct SwitchProps;
    /// The caption (right of the track).
    text "text": String = Str("") ; Str, Layout;
    /// On (`bind:active`); the user toggles it.
    active "active": bool = Bool(false) ; Bool, Paint, bindable;
    disabled "disabled": bool = Bool(false) ; Bool, Paint;
}

/// The signals, in spec order.
pub const SIGNALS: &[SignalSpec] = &[
    SignalSpec::new("changed", &[("active", ValueType::Bool)], "The user toggled it; carries the new state. Preventing it keeps the state.").cancelable(),
    SignalSpec::new("clicked", &[], "Pressed and released (or Space, Enter, AccessKit `Click`)."),
];

pub static SPEC: KindSpec = KindSpec { tag: "switch", props: SwitchProps::SPECS, signals: SIGNALS, slots: &[], children: false, role: accesskit::Role::Switch, name_from: NameFrom::Prop("text"), text_prop: Some("text"), focusable: true };

const IND: Indicator = Indicator { name: "track", ua: TRACK, host_css: false };

/// Paints the switch at `rect` under `host` (its style in this state, `:checked` when on) with its
/// parts' computed styles: track, knob at the `active` end, caption, the focus ring around the track.
/// For foreign painters too.
pub fn paint(pc: &mut PaintCx, host: &StyleView<'_>, parts: &[(&'static str, &ComputedStyle)], rect: Rect, text: &str, active: bool) {
    let knob_css = checkbox::part_css(parts, "knob");
    checkbox::paint_indicator(pc, host, parts, rect, IND, text, "", |pc, host, tr| {
        let knob = checkbox::part_with(host, knob_css, KNOB, false);
        let d = checkbox::part_size(host, KNOB, 12.0).x.min(tr.height());
        let t = if active { 1.0 } else { 0.0 };
        let inset = tr.height() / 2.0;
        let (x0, x1) = (tr.min.x + inset, tr.max.x - inset);
        let x = x0 + (x1 - x0) * t;
        let color = knob.color("color").unwrap_or(Color::BLACK);
        checkbox::paint_dot(pc, Point::new(x, (tr.min.y + tr.max.y) / 2.0), d / 2.0, color);
    });
}

/// An on/off switch with a caption.
#[derive(Debug, Default)]
pub struct Switch {
    pub props: SwitchProps,
    units: Units,
    hover: Hoverable,
    focus: Focusable,
    press: Pressable,
    toggle: Toggleable,
}

impl Switch {
    fn toggle_params(&self) -> ToggleParams {
        ToggleParams { checked: self.props.active, ..ToggleParams::default() }
    }

    /// `state` with the prop mirrors (`:checked` when on, `:disabled`).
    fn mirror(&self, mut state: StateBits) -> StateBits {
        state.set(StateBits::CHECKED, self.props.active);
        state.set(StateBits::DISABLED, self.props.disabled);
        state
    }
}

impl Kind for Switch {
    fn parts() -> &'static [&'static str] {
        &["track", "knob", "label"]
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
        self.props.set(prop, v)
    }
    fn get_prop(&self, prop: PropId) -> Option<Value> {
        self.props.get(prop)
    }

    fn measure(&self, cx: &mut MeasureCx, _known: taffy::Size<Option<f32>>, _avail: taffy::Size<taffy::AvailableSpace>) -> taffy::Size<f32> {
        let host = checkbox::host_in(&cx.style, UA, self.mirror(cx.style.state));
        let s = checkbox::measure_indicator(cx.text, &host, cx.style.parts, IND, &self.props.text, "");
        taffy::Size { width: s.x, height: s.y }
    }

    fn record(&self, cx: &mut RecordCx) {
        let host = checkbox::host_in(&cx.style, UA, self.mirror(cx.style.state));
        let parts = cx.style.parts;
        let rect = cx.rect();
        paint(&mut PaintCx::of(cx), &host, parts, rect, &self.props.text, self.props.active);
    }

    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        *cx.state = self.mirror(*cx.state);
        cx.enabled = !self.props.disabled;
        let tp = self.toggle_params();
        let pp = PressParams::default();
        let sense = Sense::HOVER.union(self.focus.sense(&Default::default())).union(self.press.sense(&pp)).union(self.toggle.sense(&tp));
        let (mut pressed, mut toggled) = (Intents::default(), Intents::default());
        let (hover, focus, press, toggle) = (&mut self.hover, &mut self.focus, &mut self.press, &mut self.toggle);
        let pass = self.units.run(cx, ev, sense, |icx| {
            hover.input(&(), icx, &mut Intents::default());
            focus.input(&Default::default(), icx, &mut Intents::default());
            press.input(&pp, icx, &mut pressed);
            toggle.input(&tp, icx, &mut toggled);
        });
        if pressed.iter().any(|p| p.is_click()) {
            common::emit(cx, &SPEC, "clicked", Vec::new());
        }
        for Toggle::Toggled { checked, .. } in toggled.iter().copied() {
            // EV1: the signal goes out before the default action (the prop write)
            if common::emit_sync(cx, &SPEC, "changed", vec![Value::Bool(checked)]) {
                continue;
            }
            if self.props.active != checked {
                self.props.active = checked;
                cx.prop_changed(P::active.id());
            }
        }
        // a prevented toggle leaves the unit's bits at the new value: back to the props
        *cx.state = self.mirror(*cx.state);
        if pass.resp.hovered || pass.resp.down_on || !pressed.is_empty() || !toggled.is_empty() { Handled::Yes } else { Handled::No }
    }

    fn a11y(&self, cx: &mut A11yCx) {
        cx.node.set_role(accesskit::Role::Switch);
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

    fn claims_key(&self, key: Key, m: presto_base::event::Modifiers) -> bool {
        self.press.claims(&PressParams::default(), key, Mods::of(m, false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::testkit::{self, Driver};
    use crate::paint::ua_token;
    use presto_base::change::ChangeClass;
    use presto_base::color::Fill;
    use presto_base::display::{Prim, Quad};
    use presto_base::geom::Size;

    const SIZE: Size = Size { w: 100.0, h: 20.0 };

    fn switch(text: &str) -> Switch {
        let mut s = Switch::default();
        s.props.text = text.into();
        s
    }

    fn quad(p: &Prim) -> &Quad {
        match p {
            Prim::Quad(q) => q,
            other => panic!("not a quad: {other:?}"),
        }
    }

    #[test]
    fn spec() {
        let s = Switch::spec();
        assert_eq!(s.tag, "switch");
        assert_eq!(s.role, accesskit::Role::Switch);
        let active = s.prop_spec(s.prop("active").unwrap()).unwrap();
        assert!(active.bindable && active.change == ChangeClass::Paint);
        assert_eq!(s.prop_spec(s.prop("text").unwrap()).unwrap().change, ChangeClass::Layout);
        assert_eq!(s.signals[s.signal("changed").unwrap().0 as usize].args, &[("active", ValueType::Bool)]);
        assert!(s.signal("clicked").is_some());
    }

    #[test]
    fn measures_track_gap_and_caption() {
        let adv = crate::shim::testing::MonoShaper::advance(13.0);
        assert_eq!(testkit::measure(&switch("On"), StateBits::NONE), Size::new(40.0 + 8.0 + 2.0 * adv, 20.0));
    }

    #[test]
    fn records_off_on_and_states() {
        let mut s = switch("Wi-Fi");
        let (list, _) = testkit::record(&s, SIZE, StateBits::NONE);
        assert_eq!(testkit::kinds(&list), ["quad", "glyphs", "quad"], "track, caption, knob");
        let track = quad(&list.items[0]);
        assert_eq!((track.rect, track.radii.tl), (Rect::new(0.0, 0.0, 40.0, 20.0), 10.0));
        assert_eq!(track.fill, Fill::Solid(ua_token(true, "control-bg").unwrap()));
        assert_eq!(track.border_color, ua_token(true, "control-border").unwrap());
        let knob = quad(&list.items[2]);
        assert_eq!(knob.rect, Rect::new(4.0, 4.0, 16.0, 16.0), "12px knob on the left");
        assert_eq!(knob.fill, Fill::Solid(ua_token(true, "fg").unwrap()));

        s.props.active = true;
        let (list, _) = testkit::record(&s, SIZE, StateBits::NONE);
        assert_eq!(quad(&list.items[0]).fill, Fill::Solid(ua_token(true, "accent").unwrap()));
        let knob = quad(&list.items[2]);
        assert_eq!(knob.rect, Rect::new(24.0, 4.0, 36.0, 16.0), "R17: on the right");
        assert_eq!(knob.fill, Fill::Solid(ua_token(true, "accent-fg").unwrap()));

        let (list, _) = testkit::record(&s, SIZE, StateBits::HOVERED);
        assert_eq!(quad(&list.items[0]).fill, Fill::Solid(ua_token(true, "accent-hover").unwrap()));
        assert_eq!(quad(&list.items[2]).rect, Rect::new(23.0, 3.0, 37.0, 17.0), "the knob grows to 14 on hover");

        s.props.active = false;
        let (list, _) = testkit::record(&s, SIZE, StateBits::HOVERED);
        assert_eq!(quad(&list.items[0]).fill, Fill::Solid(ua_token(true, "button-bg-hover").unwrap()));

        let (list, _) = testkit::record(&s, SIZE, StateBits::FOCUSED | StateBits::FOCUS_VISIBLE);
        assert_eq!(testkit::kinds(&list), ["quad", "glyphs", "quad", "quad"], "the ring last");
        assert_eq!(quad(list.items.last().unwrap()).rect, Rect::new(-2.0, -2.0, 42.0, 22.0), "around the track");

        s.props.disabled = true;
        s.props.active = true;
        let (list, _) = testkit::record(&s, SIZE, StateBits::NONE);
        assert_eq!(quad(&list.items[2]).fill, Fill::Solid(ua_token(true, "disabled-fg").unwrap()));
        let Prim::Glyphs(g) = &list.items[1] else { unreachable!() };
        assert_eq!(g.color, ua_token(true, "disabled-fg").unwrap());
    }

    #[test]
    fn click_toggles_and_emits() {
        let mut d = Driver::new(switch("A"), SIZE);
        d.click();
        assert_eq!(d.signals, [("clicked", vec![]), ("changed", vec![Value::Bool(true)])]);
        assert_eq!(d.changed, ["active"]);
        assert_eq!(d.get("active"), Some(Value::Bool(true)));
        assert!(d.state.contains(StateBits::CHECKED));
        d.signals.clear();
        d.click();
        assert_eq!(d.signals, [("clicked", vec![]), ("changed", vec![Value::Bool(false)])]);
        assert!(!d.state.contains(StateBits::CHECKED));
    }

    #[test]
    fn space_and_accesskit_toggle() {
        let mut d = Driver::new(switch("A"), SIZE);
        d.focus(true);
        d.key(Key::Space);
        assert_eq!(d.take(), ["clicked", "changed"]);
        assert_eq!(d.get("active"), Some(Value::Bool(true)));
        d.a11y_action(accesskit::Action::Click);
        assert_eq!(d.take(), ["clicked", "changed"]);
        assert_eq!(d.get("active"), Some(Value::Bool(false)));
        assert!(d.kind.claims_key(Key::Space, Default::default()));
    }

    #[test]
    fn disabled_does_not_toggle() {
        let mut d = Driver::new(switch("A"), SIZE);
        d.set("disabled", Value::Bool(true));
        d.click();
        d.focus(true);
        d.key(Key::Space);
        d.a11y_action(accesskit::Action::Click);
        assert!(d.take().is_empty());
        assert_eq!(d.get("active"), Some(Value::Bool(false)));
    }

    #[test]
    fn a11y_node() {
        let mut s = switch("Wi-Fi");
        let n = testkit::a11y(&s, StateBits::NONE, SIZE);
        assert_eq!(n.role(), accesskit::Role::Switch);
        assert_eq!(n.label(), Some("Wi-Fi"));
        assert_eq!(n.toggled(), Some(accesskit::Toggled::False));
        s.props.active = true;
        assert_eq!(testkit::a11y(&s, StateBits::NONE, SIZE).toggled(), Some(accesskit::Toggled::True));
    }

    #[test]
    fn a_prevented_changed_keeps_the_state() {
        let mut d = Driver::new(Switch::default(), presto_base::geom::Size::new(80.0, 20.0));
        d.prevent = vec!["changed"];
        d.click();
        assert!(d.names().contains(&"changed"));
        assert!(!d.kind.props.active && !d.state.contains(StateBits::CHECKED));
    }
}
