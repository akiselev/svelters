//! `button` (ARCH-WIDGETS §6.1, ported from svelters-egui `widgets/button.rs`): box, text, icon;
//! Hoverable, Focusable, Pressable; paint parts `icon` and `label` (WIDGET-PARITY §2.1), laid out by
//! CSS (P3).
//!
//! Look (CSS, theme `sv.base`): `button`, `:hover`, `:active`, `:selected`, `:focus-visible` (the focus
//! ring is the outline), `:default`, `:disabled`. Where no rule sets a property the UA table [`UA`]
//! does, from the built-in VS Code theme's tokens. Signals: `clicked`, `double-clicked`,
//! `secondary-clicked`, `press`, `release`.
//!
//! `default` and `cancel` (EV3): Enter in the button's focus scope activates the scope's default
//! button, Escape its cancel button. Finding the scope's button is skelter-core's focus manager's; it
//! activates the button with an AccessKit `Click`, which the button takes as a click (`clicked`).

use crate::controls::common::{self, Units};
use crate::interact::{Focusable, Hoverable, Intents, Press, PressParams, Pressable, Sense, StateBits, Unit};
use crate::kind_props;
use crate::paint::{self, Align, IconKey, PaintCx, PartItem, StyleView, Ua, UaRule, UaValue::*};
use crate::shim::core::{A11yCx, Event, EventCx, Handled, Kind, KindSpec, MeasureCx, NameFrom, PropError, PropId, RecordCx, SignalSpec, Value};
use crate::shim::text::{TextLayout, TextShaper};
use skelter_base::accesskit;
use skelter_base::event::MouseButton;
use skelter_base::geom::{Rect, Vec2};
use skelter_base::key::Key;
use std::sync::Arc;

/// The button's UA sheet, with DESIGN's affordance rules on by default: VS Code's text button
/// metrics (padding 4/8, 16px lines, 1px border, radius 4: 26px tall; 12px text), the theme's
/// secondary-button roles, and
/// - R1 raised: `button-bg` fill, `button-border-color` boundary, radius 4, the 1px lip (`elev-1`);
/// - R2 pressed sinks: the lip goes, the fill steps to `button-bg-active`, an inset shadow, at once;
/// - R3 hover: `button-bg-hover` in the same frame, lip kept;
/// - R8 focus ring: keyboard-only (`:focus-visible`), 2px `focus` at offset 1;
/// - R9 disabled: opacity .4, still hoverable, `not-allowed` cursor;
/// - R10 cursor: `pointer`.
pub const UA: Ua = Ua(&[
    UaRule::base(&[
        ("background-color", Token("button-bg")),
        ("color", Token("button-fg")),
        ("border-color", Token("button-border-color")),
        ("border-width", Token("stroke")),
        ("border-radius", Token("button-radius")),
        ("box-shadow", Token("elev-1")),
        ("padding-left", Token("button-pad-x")),
        ("padding-right", Token("button-pad-x")),
        ("padding-top", Token("button-pad-y")),
        ("padding-bottom", Token("button-pad-y")),
        ("font-size", Token("font-size-sm")),
        ("line-height", Px(16.0)),
        ("text-align", Keyword("center")),
        ("justify-content", Keyword("center")),
        ("gap", Px(4.0)),
        ("cursor", Keyword("pointer")),
        ("outline-style", Keyword("none")),
        ("outline-color", Token("focus")),
        ("outline-width", Px(2.0)),
        ("outline-offset", Px(1.0)),
    ]),
    UaRule::when(StateBits::HOVERED, &[("background-color", Token("button-bg-hover"))]),
    UaRule::when(StateBits::SELECTED, &[("background-color", Token("button-bg-active"))]),
    UaRule::when(StateBits::PRESSED, &[("background-color", Token("button-bg-active")), ("box-shadow", Shadows(paint::PRESSED_INSET))]),
    UaRule::when(StateBits::FOCUS_VISIBLE, &[("outline-style", Keyword("solid"))]),
    UaRule::when(StateBits::DISABLED, &[("opacity", Number(0.4)), ("cursor", Keyword("not-allowed")), ("box-shadow", Shadows(&[]))]),
]);

/// `::part(icon)`'s UA sheet: a 16px glyph (codicon size).
pub const ICON_UA: Ua = Ua(&[UaRule::base(&[("font-size", Px(16.0))])]);

kind_props! {
    /// The button's props.
    pub struct ButtonProps;
    /// The caption.
    text "text": String = Str("") ; Str, Layout;
    /// Drawn as selected (a toggle look); clicking does not change it.
    selected "selected": bool = Bool(false) ; Bool, Paint;
    /// Draws the background and border (otherwise only while hovered, pressed or selected).
    frame "frame": bool = Bool(true) ; Bool, Paint;
    /// No vertical padding.
    small "small": bool = Bool(false) ; Bool, Layout;
    /// CSS font weight of the caption (a prop the runtime sets beats CSS).
    font_weight "font-weight": i64 = I64(400) ; I64, Layout;
    /// An icon before the caption: an icon key (`name` in the `icons` family, or `family:name`).
    icon "icon": String = Str("") ; Str, Layout;
    /// Held down, it activates again and again (`clicked` after 300 ms, then every 100 ms).
    repeat "repeat": bool = Bool(false) ; Bool, Property;
    disabled "disabled": bool = Bool(false) ; Bool, Paint;
    /// The default button of its focus scope: Enter there activates it, and it shows `:default`.
    default "default": bool = Bool(false) ; Bool, Paint;
    /// The cancel button of its focus scope: Escape there activates it.
    cancel "cancel": bool = Bool(false) ; Bool, Property;
}

/// The signals, in spec order.
pub const SIGNALS: &[SignalSpec] = &[
    SignalSpec::new("clicked", &[], "Pressed and released (or Space, Enter, AccessKit `Click`)."),
    SignalSpec::new("double-clicked", &[], ""),
    SignalSpec::new("secondary-clicked", &[], ""),
    SignalSpec::new("press", &[], "The primary button went down on it."),
    SignalSpec::new("release", &[], "It came up again, over the button or not."),
];

pub static SPEC: KindSpec = KindSpec { tag: "button", props: ButtonProps::SPECS, signals: SIGNALS, slots: &[], children: false, role: accesskit::Role::Button, name_from: NameFrom::Prop("text"), text_prop: Some("text"), focusable: true };

/// What a button shows, from its props: the input to [`measure`] and [`paint`] (also for foreign
/// painters that draw a button without a node).
#[derive(Clone, Copy, Debug)]
pub struct Look<'a> {
    pub text: &'a str,
    /// An icon key (`name` in the `icons` family, or `family:name`); empty for none.
    pub icon: &'a str,
    pub frame: bool,
    pub small: bool,
}

/// The parts' content: (icon key and size, label layout).
fn content(shaper: &mut dyn TextShaper, look: &Look, icon: &StyleView, label: &StyleView) -> (Option<(IconKey, Vec2)>, Option<Arc<TextLayout>>) {
    let icon = IconKey::parse(look.icon).and_then(|k| paint::content_icon(icon, &k)).map(|k| {
        let size = paint::icon_size(shaper, icon, &k);
        (k, size)
    });
    let label = (!look.text.is_empty()).then(|| paint::text_layout(shaper, label, look.text, None));
    (icon, label)
}

/// The height of a single label line under `s` (`line-height`, else the layout's).
fn line_size(s: &StyleView, l: &TextLayout) -> Vec2 {
    Vec2::new(l.size().w, s.px("line-height").filter(|l| *l > 0.0).unwrap_or(l.size().h))
}

/// The button's size under `host` and its parts' views (`::part(icon)` over [`ICON_UA`],
/// `::part(label)`).
pub fn measure(shaper: &mut dyn TextShaper, host: &StyleView, icon_v: &StyleView, label_v: &StyleView, look: &Look) -> Vec2 {
    let (icon, label) = content(shaper, look, icon_v, label_v);
    let mut items = Vec::new();
    if let Some((_, size)) = &icon {
        items.push(PartItem { name: "icon", style: icon_v, content: *size });
    }
    if let Some(l) = &label {
        items.push(PartItem { name: "label", style: label_v, content: line_size(label_v, l) });
    }
    let mut size = paint::measure_parts(host, &items);
    if items.is_empty() {
        // an empty button keeps one line's height
        size.y = host.px("line-height").unwrap_or(host.font_size());
    }
    let mut extra = paint::box_extra(host);
    if look.small {
        let [_, _, pt, pb] = host.sides("padding");
        extra.y -= pt + pb;
    }
    size + extra
}

/// Paints the button at `rect` under `host` (its style in this state) and its parts' views: box
/// (when framed), icon and label laid out by CSS (P3), the focus ring last.
pub fn paint(pc: &mut PaintCx, host: &StyleView, icon_v: &StyleView, label_v: &StyleView, rect: Rect, look: &Look) {
    let framed = look.frame || host.state().intersects(StateBits(StateBits::HOVERED.0 | StateBits::PRESSED.0 | StateBits::SELECTED.0));
    let geom = if framed { paint::paint_box(pc, host, rect) } else { paint::box_geom(host, rect) };
    let content_box = if look.small { geom.padding } else { geom.content };
    let (icon, label) = content(pc.text, look, icon_v, label_v);
    let mut items = Vec::new();
    if let Some((_, size)) = &icon {
        items.push(PartItem { name: "icon", style: icon_v, content: *size });
    }
    if let Some(l) = &label {
        items.push(PartItem { name: "label", style: label_v, content: line_size(label_v, l) });
    }
    let rects = paint::layout_parts(host, content_box, &items);
    for (item, r) in items.iter().zip(rects) {
        let Some(r) = r else { continue };
        match (item.name, &icon, &label) {
            ("icon", Some((k, _)), _) => paint::paint_icon(pc, item.style, k, r),
            ("label", _, Some(l)) => {
                let l = paint::fit_layout(pc.text, item.style, look.text, l, r.width());
                paint::paint_text(pc, item.style, &l, r, Align::Center);
            }
            _ => {}
        }
    }
    paint::paint_outline(pc, host, rect);
}

/// A push button.
#[derive(Debug, Default)]
pub struct Button {
    pub props: ButtonProps,
    units: Units,
    hover: Hoverable,
    focus: Focusable,
    press: Pressable,
}

impl Button {
    fn press_params(&self) -> PressParams {
        PressParams { repeat: self.props.repeat.then_some((0.3, 0.1)), ..PressParams::default() }
    }

    fn look(&self) -> Look<'_> {
        Look { text: &self.props.text, icon: &self.props.icon, frame: self.props.frame, small: self.props.small }
    }

    /// The node's state with the prop mirrors (`:selected`, `:disabled`).
    fn mirror(&self, state: &mut StateBits) {
        state.set(StateBits::SELECTED, self.props.selected);
        state.set(StateBits::DISABLED, self.props.disabled);
        state.set(StateBits::DEFAULT, self.props.default);
    }
}

impl Kind for Button {
    fn parts() -> &'static [&'static str] {
        &["icon", "label"]
    }
    fn prop_state(&self) -> crate::interact::StateBits {
        { let mut s = crate::interact::StateBits::NONE; self.mirror(&mut s); s }
    }
    fn prop_states() -> crate::interact::StateBits {
        crate::interact::StateBits(crate::interact::StateBits::SELECTED.0 | crate::interact::StateBits::DEFAULT.0)
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

    fn measure(&self, cx: &mut MeasureCx, _known: taffy::Size<Option<f32>>, avail: taffy::Size<taffy::AvailableSpace>) -> taffy::Size<f32> {
        let host = common::host_view(&cx.style, UA).with_font_weight(Some(self.props.font_weight));
        let icon_v = host.part(cx.style.part_css("icon"), ICON_UA);
        let label_v = host.part(cx.style.part_css("label"), Ua::NONE);
        let s = measure(cx.text, &host, &icon_v, &label_v, &self.look());
        // a definite width caps the caption (CSS fit-content; it elides past it)
        taffy::Size { width: common::fit_width(s.x, avail.width), height: s.y }
    }

    fn record(&self, cx: &mut RecordCx) {
        let host = common::host_view(&cx.style, UA).with_font_weight(Some(self.props.font_weight));
        let icon_v = host.part(cx.style.part_css("icon"), ICON_UA);
        let label_v = host.part(cx.style.part_css("label"), Ua::NONE);
        let rect = cx.rect();
        paint(&mut PaintCx::of(cx), &host, &icon_v, &label_v, rect, &self.look());
    }

    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        self.mirror(cx.state);
        cx.enabled = !self.props.disabled;
        let sense = Sense::HOVER.union(self.focus.sense(&Default::default())).union(self.press.sense(&self.press_params()));
        let params = self.press_params();
        let mut fired = Intents::default();
        let (hover, focus, press) = (&mut self.hover, &mut self.focus, &mut self.press);
        let pass = self.units.run(cx, ev, sense, |icx| {
            hover.input(&(), icx, &mut Intents::default());
            focus.input(&Default::default(), icx, &mut Intents::default());
            press.input(&params, icx, &mut fired);
        });
        if let Some(dt) = self.press.next_frame() {
            common::wake_in(cx, dt);
        }
        let (mut click, mut double, mut secondary, mut down, mut up) = (false, false, false, false, false);
        for p in fired.iter() {
            match p {
                Press::Click { count: 1, button: MouseButton::Primary, .. } | Press::Repeat => click = true,
                Press::Click { count: 2, button: MouseButton::Primary, .. } => double = true,
                Press::Click { button: MouseButton::Secondary, .. } => secondary = true,
                Press::Down => down = true,
                Press::Up { .. } | Press::Cancel => up = true,
                _ => {}
            }
        }
        // EV1: each signal is dispatched before a default action (a button has none)
        for (on, name) in [(down, "press"), (up, "release"), (click, "clicked"), (double, "double-clicked"), (secondary, "secondary-clicked")] {
            if on {
                common::emit(cx, &SPEC, name, Vec::new());
            }
        }
        common::cursor(cx, &UA);
        if (ev.is_pointer() && (pass.resp.hovered || pass.resp.down_on)) || !fired.is_empty() { Handled::Yes } else { Handled::No }
    }

    fn a11y(&self, cx: &mut A11yCx) {
        cx.node.set_role(accesskit::Role::Button);
        if !self.props.text.is_empty() {
            cx.node.set_label(self.props.text.clone());
        }
        if self.props.selected {
            cx.node.set_selected(true);
        }
        if self.props.disabled {
            cx.node.set_disabled();
        }
        self.focus.a11y(&Default::default(), cx.node);
        self.press.a11y(&self.press_params(), cx.node);
    }

    fn claims_key(&self, key: Key, m: skelter_base::event::Modifiers) -> bool {
        let m = crate::interact::Mods::of(m, false);
        self.press.claims(&self.press_params(), key, m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::testkit::{self, Driver};
    use skelter_base::change::ChangeClass;
    use skelter_base::display::Prim;
    use skelter_base::geom::Size;

    #[test]
    fn spec() {
        let s = Button::spec();
        assert_eq!(s.tag, "button");
        assert_eq!(s.role, accesskit::Role::Button);
        let text = s.prop("text").unwrap();
        assert_eq!(s.prop_spec(text).unwrap().change, ChangeClass::Layout);
        assert_eq!(s.prop_spec(s.prop("selected").unwrap()).unwrap().change, ChangeClass::Paint);
        assert_eq!(s.prop_spec(s.prop("font-weight").unwrap()).unwrap().default, crate::shim::core::Default::I64(400));
        assert!(s.signal("clicked").is_some() && s.signal("double-clicked").is_some());
        let mut b = Button::default();
        assert!(b.set_prop(text, &Value::Bool(true)).is_err(), "type-checked");
        b.set_prop(text, &Value::str("OK")).unwrap();
        assert_eq!(b.get_prop(text), Some(Value::str("OK")));
    }

    #[test]
    fn measures_vs_code_metrics() {
        let mut b = Button::default();
        b.props.text = "OK".into();
        // 12px text (font-size-sm), 16px lines, 4/8 padding, 1px border: 26 tall
        let s = testkit::measure(&b, StateBits::NONE);
        assert_eq!(s.h, 26.0);
        let adv = crate::shim::testing::MonoShaper::advance(12.0);
        assert_eq!(s.w, 2.0 * adv + 2.0 * 8.0 + 2.0);
    }

    #[test]
    fn records_box_label_and_ring() {
        let mut b = Button::default();
        b.props.text = "OK".into();
        let size = Size::new(60.0, 26.0);
        let (list, res) = testkit::record(&b, size, StateBits::NONE);
        // the 1px lip (elev-1) under the fill and border, then the caption
        assert_eq!(testkit::kinds(&list), ["shadow", "quad", "glyphs"]);
        assert_eq!(res.runs.len(), 1);
        let Prim::Quad(q) = &list.items[1] else { unreachable!() };
        assert_eq!(q.radii.tl, 4.0);
        assert!(q.border_widths.top == 1.0 && q.border_color.a > 0.0);
        // pressed: inset shadow instead of the lip
        let (list, _) = testkit::record(&b, size, StateBits::PRESSED);
        assert_eq!(testkit::kinds(&list), ["quad", "inset-shadow", "glyphs"]);
        // keyboard focus: the outline last
        let (list, _) = testkit::record(&b, size, StateBits::FOCUSED | StateBits::FOCUS_VISIBLE);
        assert_eq!(testkit::kinds(&list).last(), Some(&"quad"));
        assert!(list.bounds.min.x < 0.0, "the ring lies outside the box");
        // frameless: nothing but the caption until hovered
        b.props.frame = false;
        assert_eq!(testkit::kinds(&testkit::record(&b, size, StateBits::NONE).0), ["glyphs"]);
        assert_eq!(testkit::kinds(&testkit::record(&b, size, StateBits::HOVERED).0), ["shadow", "quad", "glyphs"]);
    }

    #[test]
    fn click_press_release() {
        let mut d = Driver::new(Button::default(), Size::new(60.0, 26.0));
        d.click();
        assert_eq!(d.take(), ["press", "release", "clicked"]);
        assert!(!d.state.contains(StateBits::PRESSED));
        d.down(5.0, 5.0);
        assert!(d.state.contains(StateBits::PRESSED), "`:active` while held");
        d.move_to(40.0, 5.0);
        d.up(40.0, 5.0);
        assert_eq!(d.take(), ["press", "release"], "moved too far: no click");
    }

    #[test]
    fn double_click_and_secondary() {
        let mut d = Driver::new(Button::default(), Size::new(60.0, 26.0));
        d.click();
        d.click();
        assert_eq!(d.take(), ["press", "release", "clicked", "press", "release", "clicked", "double-clicked"]);
        d.advance(1000);
        d.secondary_click(5.0, 5.0);
        // egui: any button held on the host is `down_on`, so a secondary press presses it too
        assert_eq!(d.take(), ["press", "release", "secondary-clicked"]);
    }

    #[test]
    fn keyboard_and_accesskit_activate() {
        let mut d = Driver::new(Button::default(), Size::new(60.0, 26.0));
        d.key(Key::Space);
        assert!(d.take().is_empty(), "not focused");
        d.focus(true);
        assert!(d.state.contains(StateBits::FOCUS_VISIBLE), "keyboard focus shows the ring");
        d.key(Key::Enter);
        assert_eq!(d.take(), ["clicked"]);
        d.a11y_action(accesskit::Action::Click);
        assert_eq!(d.take(), ["clicked"]);
        assert!(d.kind.claims_key(Key::Space, Default::default()));
    }

    #[test]
    fn disabled_does_not_click_but_hovers() {
        let mut d = Driver::new(Button::default(), Size::new(60.0, 26.0));
        d.set("disabled", Value::Bool(true));
        d.click();
        assert!(d.take().is_empty());
        assert!(d.state.contains(StateBits::DISABLED));
    }

    #[test]
    fn repeat_clicks_while_held() {
        let mut d = Driver::new(Button::default(), Size::new(60.0, 26.0));
        d.set("repeat", Value::Bool(true));
        d.hover(true);
        d.down(5.0, 5.0);
        assert!(d.wake.is_some(), "asks for a timer");
        d.advance(310);
        d.send(Event::Timer);
        assert_eq!(d.take(), ["press", "clicked"]);
        d.advance(110);
        d.send(Event::Timer);
        assert_eq!(d.take(), ["clicked"]);
    }

    #[test]
    fn default_and_cancel() {
        let s = Button::spec();
        assert!(s.prop("default").is_some() && s.prop("cancel").is_some());
        let mut d = Driver::new(Button::default(), Size::new(60.0, 26.0));
        d.set("default", Value::Bool(true));
        d.hover(true);
        assert!(d.state.contains(StateBits::DEFAULT), "`:default`");
        // the scope's Enter reaches the default button as an AccessKit click, focused or not
        d.a11y_action(accesskit::Action::Click);
        assert_eq!(d.take(), ["clicked"]);
    }

    #[test]
    fn a11y_node() {
        let mut b = Button::default();
        b.props.text = "Save".into();
        let n = testkit::a11y(&b, StateBits::NONE, Size::new(60.0, 26.0));
        assert_eq!(n.role(), accesskit::Role::Button);
        assert_eq!(n.label(), Some("Save"));
        assert!(n.supports_action(accesskit::Action::Click) && n.supports_action(accesskit::Action::Focus));
    }
}
