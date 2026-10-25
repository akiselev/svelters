//! `toggle-button` (ARCH-WIDGETS §6.1, ported from svelters-egui `widgets/toggle_button.rs`): box,
//! text; Hoverable, Focusable, Pressable, Toggleable (radio: only turns on), Roving in a group.
//!
//! Look: the button's (DESIGN R1–R3, R8) plus R17's on state: `:checked` fills with `toggle-on-bg`
//! and a 1 px `toggle-on-border` (VS Code's `inputOption.active*`, VIS1 `toggle-on-fg`).
//!
//! **Groups** (WIDGET-PARITY §2.1, QButtonGroup): the sibling toggle buttons with the same non-empty
//! `group` are a peer group ([`PeerGroup`](crate::shim::core::PeerGroup)): checking one unchecks the
//! others ([`PeerMsg::Checked`], each announcing `changed(false)`), a click only turns a member on,
//! and the group is one Tab stop the arrow keys move in (a [`PeerTarget::Rove`] request; the member
//! it lands on checks itself, [`PeerMsg::Roved`]). Signals: `changed(checked)`, `clicked`.

use crate::controls::common::{self, Units};
use crate::interact::{Focusable, Hoverable, Intents, Mods, PressParams, Pressable, Rove, Roving, RovingParams, Sense, StateBits, Toggle, ToggleParams, Toggleable, Unit};
use crate::kind_props;
use crate::paint::{self, Align, PaintCx, StyleView, Ua, UaRule, UaValue::*};
use crate::shim::core::{A11yCx, Event, EventCx, Handled, Kind, KindSpec, MeasureCx, NameFrom, PeerGroup, PeerMsg, PeerTarget, PropError, PropId, RecordCx, SignalSpec, Value, ValueType};
use crate::shim::text::TextShaper;
use presto_base::accesskit;
use presto_base::geom::{Rect, Vec2};
use presto_base::key::Key;

/// The toggle button's UA sheet: the button's metrics and roles, R17's on state, R8's ring.
pub const UA: Ua = Ua(&[
    UaRule::base(&[
        ("background-color", Token("button-bg")),
        ("color", Token("button-fg")),
        ("border-color", Token("button-border-color")),
        ("border-width", Token("stroke")),
        ("border-radius", Token("button-radius")),
        ("padding-left", Token("button-pad-x")),
        ("padding-right", Token("button-pad-x")),
        ("padding-top", Token("button-pad-y")),
        ("padding-bottom", Token("button-pad-y")),
        ("text-align", Keyword("center")),
        ("outline-style", Keyword("none")),
        ("outline-color", Token("focus")),
        ("outline-width", Px(1.0)),
        ("outline-offset", Px(1.0)),
    ]),
    UaRule::when(StateBits::HOVERED, &[("background-color", Token("button-bg-hover"))]),
    UaRule::when(StateBits::CHECKED, &[("background-color", Token("toggle-on-bg")), ("border-color", Token("toggle-on-border")), ("color", Token("fg"))]),
    UaRule::when(StateBits::PRESSED, &[("background-color", Token("button-bg-active"))]),
    UaRule::when(StateBits::FOCUS_VISIBLE, &[("outline-style", Keyword("solid"))]),
    UaRule::when(StateBits::DISABLED, &[("color", Token("disabled-fg"))]),
]);

kind_props! {
    /// The toggle button's props.
    pub struct ToggleButtonProps;
    /// The caption.
    text "text": String = Str("") ; Str, Layout;
    /// Pressed in (`bind:checked`); the user toggles it.
    checked "checked": bool = Bool(false) ; Bool, Paint, bindable;
    /// A click only turns it on (one of a set of mutually exclusive choices).
    radio "radio": bool = Bool(false) ; Bool, Paint;
    /// Draws the background and border (otherwise only while hovered, pressed or checked).
    frame "frame": bool = Bool(true) ; Bool, Paint;
    /// CSS font weight of the caption (a prop the runtime sets beats CSS).
    font_weight "font-weight": i64 = I64(400) ; I64, Layout;
    /// The sibling toggle buttons with the same `group` form an exclusive set and one Tab stop.
    group "group": String = Str("") ; Str, Paint;
    disabled "disabled": bool = Bool(false) ; Bool, Paint;
}

/// The signals, in spec order.
pub const SIGNALS: &[SignalSpec] = &[
    SignalSpec::new("changed", &[("checked", ValueType::Bool)], "The user toggled it (or an arrow key moved the check), or a group member's check unchecked it."),
    SignalSpec::new("clicked", &[], "Pressed and released (or Space, Enter, AccessKit `Click`)."),
];

pub static SPEC: KindSpec = KindSpec { tag: "toggle-button", props: ToggleButtonProps::SPECS, signals: SIGNALS, slots: &[], children: false, role: accesskit::Role::Button, name_from: NameFrom::Prop("text"), text_prop: Some("text"), focusable: true };

/// Paints the toggle button at `rect` under `host` (its style in this state, without the
/// `font-weight` prop): box (when framed, hovered, pressed or checked), caption, focus ring. For
/// foreign painters too.
pub fn paint(pc: &mut PaintCx, host: &StyleView<'_>, rect: Rect, text: &str, frame: bool, font_weight: i64) {
    let framed = frame || host.state().intersects(StateBits(StateBits::HOVERED.0 | StateBits::PRESSED.0 | StateBits::CHECKED.0));
    let geom = if framed { paint::paint_box(pc, host, rect) } else { paint::box_geom(host, rect) };
    let s = host.with_font_weight(Some(font_weight));
    let l = paint::text_layout(pc.text, &s, text, None);
    paint::paint_text(pc, &s, &l, geom.content, Align::of(&s, Align::Center));
    paint::paint_outline(pc, host, rect);
}

/// Its size under `host` (with the `font-weight` prop).
pub fn measure(shaper: &mut dyn TextShaper, host: &StyleView<'_>, text: &str) -> Vec2 {
    let t = paint::measure_text(shaper, host, text, None);
    Vec2::new(t.w, t.h) + paint::box_extra(host)
}

/// A button that stays pressed (`checked`); with `radio` or in a `group`, a click only turns it on.
#[derive(Debug, Default)]
pub struct ToggleButton {
    pub props: ToggleButtonProps,
    units: Units,
    hover: Hoverable,
    focus: Focusable,
    press: Pressable,
    toggle: Toggleable,
    roving: Roving,
}

impl ToggleButton {
    fn grouped(&self) -> bool {
        !self.props.group.is_empty()
    }
    fn toggle_params(&self) -> ToggleParams {
        ToggleParams { checked: self.props.checked, radio: self.props.radio || self.grouped(), ..ToggleParams::default() }
    }
    fn roving_params(&self) -> RovingParams {
        RovingParams { on: self.grouped() }
    }

    /// `state` with the prop mirrors (`:checked`, `:disabled`).
    fn mirror(&self, mut state: StateBits) -> StateBits {
        state.set(StateBits::CHECKED, self.props.checked);
        state.set(StateBits::DISABLED, self.props.disabled);
        state
    }

    /// Writes `checked` and announces it, when it changes.
    fn set_checked(&mut self, cx: &mut EventCx, on: bool) {
        if self.props.checked != on {
            self.props.checked = on;
            cx.prop_changed(P::checked.id());
            common::emit(cx, &SPEC, "changed", vec![Value::Bool(on)]);
        }
        cx.state.set(StateBits::CHECKED, on);
    }
}

impl Kind for ToggleButton {
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
        let state = self.mirror(cx.style.state);
        let host = StyleView::new(cx.style.computed, UA, state, cx.style.dark).with_font_weight(Some(self.props.font_weight));
        let s = measure(cx.text, &host, &self.props.text);
        taffy::Size { width: s.x, height: s.y }
    }

    fn record(&self, cx: &mut RecordCx) {
        let state = self.mirror(cx.style.state);
        let host = StyleView::new(cx.style.computed, UA, state, cx.style.dark);
        let rect = cx.rect();
        paint(&mut PaintCx::of(cx), &host, rect, &self.props.text, self.props.frame, self.props.font_weight);
    }

    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        *cx.state = self.mirror(*cx.state);
        cx.enabled = !self.props.disabled;
        let (tp, rp, pp) = (self.toggle_params(), self.roving_params(), PressParams::default());
        let sense = Sense::HOVER.union(self.focus.sense(&Default::default())).union(self.press.sense(&pp)).union(self.toggle.sense(&tp));
        let (mut pressed, mut toggled, mut roved) = (Intents::default(), Intents::default(), Intents::<Rove>::default());
        let (hover, focus, press, toggle, roving) = (&mut self.hover, &mut self.focus, &mut self.press, &mut self.toggle, &mut self.roving);
        let pass = self.units.run(cx, ev, sense, |icx| {
            hover.input(&(), icx, &mut Intents::default());
            focus.input(&Default::default(), icx, &mut Intents::default());
            press.input(&pp, icx, &mut pressed);
            toggle.input(&tp, icx, &mut toggled);
            roving.input(&rp, icx, &mut roved);
        });
        if pressed.iter().any(|p| p.is_click()) {
            common::emit(cx, &SPEC, "clicked", Vec::new());
        }
        for Toggle::Toggled { checked, .. } in toggled.iter().copied() {
            // EV1: the signal goes out before the default action (the prop write)
            common::emit(cx, &SPEC, "changed", vec![Value::Bool(checked)]);
            if self.props.checked != checked {
                self.props.checked = checked;
                cx.prop_changed(P::checked.id());
            }
            if checked && self.grouped() {
                cx.peer_request(PeerTarget::Others, PeerMsg::Checked(Vec::new()));
            }
        }
        // Roving: the member the arrow lands on takes focus and checks itself; the others follow
        if let Some(r) = roved.iter().last().copied() {
            cx.peer_request(PeerTarget::Rove(r), PeerMsg::Roved);
        }
        if pass.resp.hovered || pass.resp.down_on || !pressed.is_empty() || !toggled.is_empty() || !roved.is_empty() { Handled::Yes } else { Handled::No }
    }

    fn a11y(&self, cx: &mut A11yCx) {
        let radio = self.props.radio || self.grouped();
        cx.node.set_role(if radio { accesskit::Role::RadioButton } else { accesskit::Role::Button });
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
        let m = Mods::of(m, false);
        self.press.claims(&PressParams::default(), key, m) || self.roving.claims(&self.roving_params(), key, m)
    }

    fn peer_group(&self) -> Option<PeerGroup> {
        self.grouped().then(|| PeerGroup { tag: SPEC.tag, key: Some(("group", Value::str(&self.props.group))), stop: "checked" })
    }

    fn peer(&mut self, cx: &mut EventCx, msg: &PeerMsg) -> Handled {
        *cx.state = self.mirror(*cx.state);
        match msg {
            PeerMsg::Roved => {
                self.set_checked(cx, true);
                cx.peer_request(PeerTarget::Others, PeerMsg::Checked(Vec::new()));
            }
            PeerMsg::Checked(_) => self.set_checked(cx, false),
        }
        Handled::Yes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::testkit::{self, Driver};
    use crate::interact::roving::group::Group;
    use crate::paint::ua_token;
    use presto_base::change::ChangeClass;
    use presto_base::color::Fill;
    use presto_base::display::{Prim, Quad};
    use presto_base::geom::Size;

    const SIZE: Size = Size { w: 60.0, h: 26.0 };

    fn tb(text: &str, group: &str) -> ToggleButton {
        let mut t = ToggleButton::default();
        t.props.text = text.into();
        t.props.group = group.into();
        t
    }

    fn quad(p: &Prim) -> &Quad {
        match p {
            Prim::Quad(q) => q,
            other => panic!("not a quad: {other:?}"),
        }
    }

    #[test]
    fn spec() {
        let s = ToggleButton::spec();
        assert_eq!(s.tag, "toggle-button");
        assert_eq!(s.role, accesskit::Role::Button);
        let checked = s.prop_spec(s.prop("checked").unwrap()).unwrap();
        assert!(checked.bindable && checked.change == ChangeClass::Paint);
        assert_eq!(s.prop_spec(s.prop("frame").unwrap()).unwrap().default, crate::shim::core::Default::Bool(true));
        assert_eq!(s.prop_spec(s.prop("font-weight").unwrap()).unwrap().change, ChangeClass::Layout);
        assert_eq!(s.signals[s.signal("changed").unwrap().0 as usize].args, &[("checked", ValueType::Bool)]);
        assert!(s.signal("clicked").is_some());
        assert_eq!(tb("A", "").peer_group(), None, "no group, no peers");
        assert_eq!(tb("A", "g").peer_group(), Some(PeerGroup { tag: "toggle-button", key: Some(("group", Value::str("g"))), stop: "checked" }));
    }

    #[test]
    fn measures_button_metrics() {
        let s = testkit::measure(&tb("Bold", ""), StateBits::NONE);
        let adv = crate::shim::testing::MonoShaper::advance(13.0);
        // 16px lines of 13px text, 4/8 padding, 1px border
        assert_eq!(s, Size::new(4.0 * adv + 2.0 * 8.0 + 2.0, 16.0 + 2.0 * 4.0 + 2.0));
    }

    #[test]
    fn records_off_on_and_states() {
        let mut t = tb("B", "");
        let (list, _) = testkit::record(&t, SIZE, StateBits::NONE);
        assert_eq!(testkit::kinds(&list), ["quad", "glyphs"]);
        assert_eq!(quad(&list.items[0]).fill, Fill::Solid(ua_token(true, "button-bg").unwrap()));
        let Prim::Glyphs(g) = &list.items[1] else { unreachable!() };
        assert_eq!(g.color, ua_token(true, "button-fg").unwrap());

        t.props.checked = true;
        let (list, _) = testkit::record(&t, SIZE, StateBits::NONE);
        let q = quad(&list.items[0]);
        assert_eq!(q.fill, Fill::Solid(ua_token(true, "toggle-on-bg").unwrap()), "R17 on state");
        assert_eq!(q.border_color, ua_token(true, "toggle-on-border").unwrap());
        let Prim::Glyphs(g) = &list.items[1] else { unreachable!() };
        assert_eq!(g.color, ua_token(true, "fg").unwrap());
        let (list, _) = testkit::record(&t, SIZE, StateBits::PRESSED);
        assert_eq!(quad(&list.items[0]).fill, Fill::Solid(ua_token(true, "button-bg-active").unwrap()));

        t.props.checked = false;
        let (list, _) = testkit::record(&t, SIZE, StateBits::HOVERED);
        assert_eq!(quad(&list.items[0]).fill, Fill::Solid(ua_token(true, "button-bg-hover").unwrap()));

        let (list, _) = testkit::record(&t, SIZE, StateBits::FOCUSED | StateBits::FOCUS_VISIBLE);
        assert_eq!(testkit::kinds(&list), ["quad", "glyphs", "quad"], "the ring last");
        assert_eq!(quad(list.items.last().unwrap()).rect, Rect::new(-2.0, -2.0, 62.0, 28.0));

        t.props.frame = false;
        assert_eq!(testkit::kinds(&testkit::record(&t, SIZE, StateBits::NONE).0), ["glyphs"], "frameless at rest");
        t.props.checked = true;
        assert_eq!(testkit::kinds(&testkit::record(&t, SIZE, StateBits::NONE).0), ["quad", "glyphs"], "framed while checked");

        t.props.disabled = true;
        let (list, _) = testkit::record(&t, SIZE, StateBits::NONE);
        let Prim::Glyphs(g) = &list.items[1] else { unreachable!() };
        assert_eq!(g.color, ua_token(true, "disabled-fg").unwrap());
    }

    #[test]
    fn click_toggles_and_emits() {
        let mut d = Driver::new(tb("B", ""), SIZE);
        d.click();
        assert_eq!(d.signals, [("clicked", vec![]), ("changed", vec![Value::Bool(true)])]);
        assert_eq!(d.changed, ["checked"]);
        assert!(d.state.contains(StateBits::CHECKED));
        d.signals.clear();
        d.click();
        assert_eq!(d.signals, [("clicked", vec![]), ("changed", vec![Value::Bool(false)])]);
        assert_eq!(d.get("checked"), Some(Value::Bool(false)));
    }

    #[test]
    fn radio_only_turns_on() {
        let mut d = Driver::new(tb("B", ""), SIZE);
        d.set("radio", Value::Bool(true));
        d.click();
        d.click();
        assert_eq!(d.take(), ["clicked", "changed", "clicked"]);
        assert_eq!(d.get("checked"), Some(Value::Bool(true)));
        assert_eq!(testkit::a11y(&d.kind, StateBits::NONE, SIZE).role(), accesskit::Role::RadioButton);
    }

    #[test]
    fn space_and_accesskit_toggle() {
        let mut d = Driver::new(tb("B", ""), SIZE);
        d.focus(true);
        d.key(Key::Space);
        assert_eq!(d.take(), ["clicked", "changed"]);
        d.a11y_action(accesskit::Action::Click);
        assert_eq!(d.take(), ["clicked", "changed"]);
        assert_eq!(d.get("checked"), Some(Value::Bool(false)));
        assert!(d.kind.claims_key(Key::Enter, Default::default()));
        assert!(!d.kind.claims_key(Key::ArrowRight, Default::default()), "no roving outside a group");
    }

    #[test]
    fn disabled_does_not_toggle() {
        let mut d = Driver::new(tb("B", ""), SIZE);
        d.set("disabled", Value::Bool(true));
        d.click();
        d.a11y_action(accesskit::Action::Click);
        assert!(d.take().is_empty());
        assert_eq!(d.get("checked"), Some(Value::Bool(false)));
    }

    fn grouped() -> Group<ToggleButton> {
        Group::new(vec![tb("L", "g"), tb("C", "g"), tb("R", "g"), tb("X", "h")], SIZE)
    }

    #[test]
    fn a_group_is_exclusive() {
        let mut g = grouped();
        g.click(0);
        g.click(1);
        assert_eq!(g.bools("checked"), [false, true, false, false]);
        let s = g.take();
        assert_eq!(&s[2..], [(1, "clicked", vec![]), (1, "changed", vec![Value::Bool(true)]), (0, "changed", vec![Value::Bool(false)])]);
        g.click(1);
        assert_eq!(g.bools("checked"), [false, true, false, false], "a member only turns on");
        assert_eq!(g.take(), [(1, "clicked", vec![])]);
        g.click(3);
        assert_eq!(g.bools("checked"), [false, true, false, true], "another group is not touched");
        assert_eq!(g.tab_stop(0), 1);
    }

    #[test]
    fn arrows_move_within_the_group() {
        let mut g = grouped();
        g.focus(1);
        g.key(Key::Space);
        g.take();
        g.key(Key::ArrowRight);
        assert_eq!(g.focused, Some(2));
        assert_eq!(g.bools("checked"), [false, false, true, false]);
        assert_eq!(g.take(), [(2, "changed", vec![Value::Bool(true)]), (1, "changed", vec![Value::Bool(false)])]);
        g.key(Key::ArrowRight);
        assert_eq!((g.focused, g.bools("checked")), (Some(0), vec![true, false, false, false]), "wraps within the group, past the other group");
        assert!(g.nodes[0].claims_key(Key::ArrowLeft, Default::default()));
    }

    #[test]
    fn a11y_node() {
        let mut t = tb("Bold", "");
        let n = testkit::a11y(&t, StateBits::NONE, SIZE);
        assert_eq!(n.role(), accesskit::Role::Button);
        assert_eq!(n.label(), Some("Bold"));
        assert_eq!(n.toggled(), Some(accesskit::Toggled::False));
        t.props.checked = true;
        assert_eq!(testkit::a11y(&t, StateBits::NONE, SIZE).toggled(), Some(accesskit::Toggled::True));
        t.props.group = "g".into();
        assert_eq!(testkit::a11y(&t, StateBits::NONE, SIZE).role(), accesskit::Role::RadioButton);
    }
}
