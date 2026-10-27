//! `checkbox` (ARCH-WIDGETS §6.1, ported from svelters-egui `widgets/checkbox.rs`): box, text, path;
//! Hoverable, Focusable, Pressable, Toggleable. Paint parts `indicator` (the box) and `mark` (check or
//! dash), an optional `icon` and the caption (`label`, WIDGET-PARITY §2.2), laid out by CSS (P3).
//!
//! States: `:hover`, `:active`, `:focus-visible` (the ring around the indicator, DESIGN R8),
//! `:checked`, `:indeterminate`, `:disabled`. The base sheet's `checkbox` rule styles the indicator
//! (through the host's computed style, [`part_view`]); the UA tables below are VS Code's 18×18
//! radius-3 box (DESIGN §1.3) with R17's on-fill. Signals: `changed(checked)`, `clicked`.
//!
//! This module also holds what the indicator kinds share (`checkbox`, `radio-button`, `switch`): the
//! part views, measuring and painting an indicator with its icon and caption, and the path glyphs
//! (check mark, dash, dot) drawn as lines and round quads.

use crate::controls::common::{self, Units};
use crate::interact::{Focusable, Hoverable, Intents, Mods, PressParams, Pressable, Sense, StateBits, Toggle, ToggleParams, Toggleable, Unit};
use crate::kind_props;
use crate::paint::{self, Align, IconKey, PaintCx, PartItem, StyleView, Ua, UaRule, UaValue::*};
use crate::shim::core::{A11yCx, Event, EventCx, Handled, Kind, KindSpec, MeasureCx, NameFrom, PropError, PropId, RecordCx, SignalSpec, Value, ValueType};
use crate::shim::text::{TextLayout, TextShaper};
use skelter_base::accesskit;
use skelter_base::color::{Color, Fill};
use skelter_base::display::{BorderStyle, Line, LineCap, Prim, Quad};
use skelter_base::geom::{Corners, Insets, Point, Rect, Size, Vec2};
use skelter_base::key::Key;
use std::sync::Arc;
use svelters_style::ComputedStyle;

/// The host: the gap, the focus ring (drawn around the indicator).
pub const UA: Ua = Ua(&[
    UaRule::base(&[
        ("gap", Px(6.0)),
        ("align-items", Keyword("center")),
        ("outline-style", Keyword("none")),
        ("outline-color", Token("focus")),
        ("outline-width", Px(2.0)),
        ("outline-offset", Px(1.0)),
    ]),
    UaRule::when(StateBits::FOCUS_VISIBLE, &[("outline-style", Keyword("solid"))]),
]);

// The parts' UA layer: `::part()` rules in a sheet override it ([`part_with`]).
/// `::part(indicator)`: the box.
pub const INDICATOR: Ua = Ua(&[
    UaRule::base(&[
        ("width", Px(18.0)),
        ("height", Px(18.0)),
        ("background-color", Token("checkbox-bg")),
        ("border-color", Token("checkbox-border-color")),
        ("border-width", Token("stroke")),
        ("border-radius", Px(3.0)),
    ]),
    // R3: hover steps the fill and the edge (the fill step alone is faint in Dark Modern)
    UaRule::when(StateBits::HOVERED, &[("background-color", Token("button-bg-hover")), ("border-color", Token("muted"))]),
    UaRule::when(StateBits::CHECKED, &[("background-color", Token("toggle-on-bg")), ("border-color", Token("toggle-on-border"))]),
    UaRule::when(StateBits::INDETERMINATE, &[("background-color", Token("toggle-on-bg")), ("border-color", Token("toggle-on-border"))]),
    UaRule::when(StateBits::PRESSED, &[("background-color", Token("button-bg-active"))]),
]);

/// `::part(mark)`: the glyph's colour.
pub const MARK: Ua = Ua(&[UaRule::base(&[("color", Token("fg"))]), UaRule::when(StateBits::DISABLED, &[("color", Token("disabled-fg"))])]);

/// `::part(icon)` of the indicator kinds (16px, as the button's).
pub const ICON: Ua = Ua(&[UaRule::base(&[("font-size", Px(16.0))])]);

kind_props! {
    /// The check box's props.
    pub struct CheckboxProps;
    /// The caption.
    text "text": String = Str("") ; Str, Layout;
    /// Checked (`bind:checked`); the user toggles it.
    checked "checked": bool = Bool(false) ; Bool, Paint, bindable;
    /// WIDGET-PARITY §2.2: partially checked (`bind:indeterminate`), shown as a dash. The app writes
    /// it; a click clears it and checks the box (Qt, GTK).
    indeterminate "indeterminate": bool = Bool(false) ; Bool, Paint, bindable;
    /// The user cycles unchecked → checked → indeterminate (Qt's tristate).
    tristate "tristate": bool = Bool(false) ; Bool, Property;
    /// An icon before the caption (a key, or `family:name`).
    icon "icon": String = Str("") ; Str, Layout;
    disabled "disabled": bool = Bool(false) ; Bool, Paint;
}

/// The signals, in spec order.
pub const SIGNALS: &[SignalSpec] = &[
    SignalSpec::new("changed", &[("checked", ValueType::Bool)], "The user toggled it; carries the new `checked` (`indeterminate` is announced as a prop). Preventing it keeps the value.").cancelable(),
    SignalSpec::new("clicked", &[], "Pressed and released (or Space, Enter, AccessKit `Click`)."),
];

pub static SPEC: KindSpec = KindSpec { tag: "checkbox", props: CheckboxProps::SPECS, signals: SIGNALS, slots: &[], children: false, role: accesskit::Role::CheckBox, name_from: NameFrom::Prop("text"), text_prop: Some("text"), focusable: true };

// ---- what the indicator kinds share ----

/// An indicator kind's indicator part: its name, UA table, and whether it reads the host's computed
/// style when no `::part()` rule reaches it (the base sheet's `checkbox` rule styles the box).
#[derive(Clone, Copy, Debug)]
pub struct Indicator {
    pub name: &'static str,
    pub ua: Ua,
    pub host_css: bool,
}

/// The `::part(name)` computed style among a node's parts.
pub(crate) fn part_css<'c>(parts: &[(&'static str, &'c ComputedStyle)], name: &str) -> Option<&'c ComputedStyle> {
    parts.iter().find(|(n, _)| *n == name).map(|(_, c)| *c)
}

/// A part's view with no `::part()` style: the host's computed style (`host_css`: true for parts the
/// base sheet styles through the host rule) over the part's UA table, in the host's state.
pub(crate) fn part_view<'a>(host: &StyleView<'a>, ua: Ua, host_css: bool) -> StyleView<'a> {
    StyleView::new(if host_css { host.computed() } else { None }, ua, host.state(), host.is_dark()).with_theme(Some(host.theme()))
}

/// A part's view from its computed style `css` (a `::part()` rule over the UA table `ua`), else
/// [`part_view`].
pub(crate) fn part_with<'a>(host: &'a StyleView<'a>, css: Option<&'a ComputedStyle>, ua: Ua, host_css: bool) -> StyleView<'a> {
    match css {
        Some(c) => host.part(Some(c), ua),
        None => part_view(host, ua, host_css),
    }
}

/// A part's size from its UA table (`width`, `height`), never the host's CSS size.
pub(crate) fn part_size(host: &StyleView<'_>, ua: Ua, default: f32) -> Vec2 {
    let s = StyleView::ua_only(ua, host.state(), host.is_dark()).with_theme(Some(host.theme()));
    Vec2::new(s.px("width").unwrap_or(default), s.px("height").unwrap_or(default))
}

/// The icon and the caption layout of an indicator kind.
fn content(shaper: &mut dyn TextShaper, icon_v: &StyleView, label_v: &StyleView, text: &str, icon: &str) -> (Option<(IconKey, Vec2)>, Option<Arc<TextLayout>>) {
    let icon = IconKey::parse(icon).and_then(|k| paint::content_icon(icon_v, &k)).map(|k| {
        let size = paint::icon_size(shaper, icon_v, &k);
        (k, size)
    });
    let label = (!text.is_empty()).then(|| paint::text_layout(shaper, label_v, text, None));
    (icon, label)
}

/// The paint parts of an indicator kind, laid out by P3 in the host's content box: the indicator, an
/// optional `icon` and the caption `label`, spaced by `gap` (`flex-direction`, `order` and
/// `display: none` on the parts apply).
fn items<'v>(ind: Indicator, ind_v: &'v StyleView<'v>, icon_v: &'v StyleView<'v>, label_v: &'v StyleView<'v>, icon: &Option<(IconKey, Vec2)>, label: &Option<Arc<TextLayout>>) -> Vec<PartItem<'v>> {
    let isz = Vec2::new(ind_v.px("width").unwrap_or(16.0), ind_v.px("height").unwrap_or(16.0));
    let mut items = vec![PartItem { name: ind.name, style: ind_v, content: isz }];
    if let Some((_, size)) = icon {
        items.push(PartItem { name: "icon", style: icon_v, content: *size });
    }
    if let Some(l) = label {
        items.push(PartItem { name: "label", style: label_v, content: Vec2::new(l.size().w, l.size().h) });
    }
    items
}

/// What an indicator kind measures under `host` with its parts' computed styles `parts`: its parts
/// (P3) plus the host's padding and border.
pub(crate) fn measure_indicator(shaper: &mut dyn TextShaper, host: &StyleView<'_>, parts: &[(&'static str, &ComputedStyle)], ind: Indicator, text: &str, icon: &str) -> Vec2 {
    let ind_v = match part_css(parts, ind.name) {
        Some(c) => host.part(Some(c), ind.ua),
        None => part_view(host, ind.ua, ind.host_css),
    };
    let icon_v = host.part(part_css(parts, "icon"), ICON);
    let label_v = host.part(part_css(parts, "label"), Ua::NONE);
    let (ic, label) = content(shaper, &icon_v, &label_v, text, icon);
    let items = items(ind, &ind_v, &icon_v, &label_v, &ic, &label);
    paint::measure_parts(host, &items) + paint::box_extra(host)
}

/// Paints an indicator kind at `rect` under `host` (its style in this state) with its parts' computed
/// styles `parts`: the indicator box, `mark` inside it (the check, the dot, the knob: it gets the
/// host's view and the indicator's rect), the icon and the caption, then the focus ring around the
/// indicator (DESIGN R8).
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_indicator(pc: &mut PaintCx, host: &StyleView<'_>, parts: &[(&'static str, &ComputedStyle)], rect: Rect, ind: Indicator, text: &str, icon: &str, mark: impl FnOnce(&mut PaintCx, &StyleView<'_>, Rect)) {
    let ind_v = match part_css(parts, ind.name) {
        Some(c) => host.part(Some(c), ind.ua),
        None => part_view(host, ind.ua, ind.host_css),
    };
    let icon_v = host.part(part_css(parts, "icon"), ICON);
    let label_v = host.part(part_css(parts, "label"), Ua::NONE);
    let (ic, label) = content(pc.text, &icon_v, &label_v, text, icon);
    let items = items(ind, &ind_v, &icon_v, &label_v, &ic, &label);
    let content_box = paint::box_geom(host, rect).content;
    let rects = paint::layout_parts(host, content_box, &items);
    let mut ir = None;
    for (item, r) in items.iter().zip(rects) {
        let Some(r) = r else { continue };
        match item.name {
            "icon" => {
                if let Some((k, _)) = &ic {
                    paint::paint_icon(pc, item.style, k, r);
                }
            }
            "label" => {
                if let Some(l) = &label {
                    let l = paint::fit_layout(pc.text, item.style, text, l, r.width());
                    paint::paint_text(pc, item.style, &l, r, Align::Start);
                }
            }
            _ => {
                paint::paint_box(pc, item.style, r);
                ir = Some((r, item.style.px("border-radius").unwrap_or(0.0)));
            }
        }
    }
    if let Some((ir, radius)) = ir {
        mark(pc, host, ir);
        // the ring follows the indicator's corners (round for a radio or a switch's track)
        paint::paint_outline_radius(pc, host, ir, radius);
    }
}

fn center(r: Rect) -> Point {
    Point::new((r.min.x + r.max.x) / 2.0, (r.min.y + r.max.y) / 2.0)
}

/// The check mark (`indeterminate`: the dash) inside `r`, in `color`: a two-segment polyline with
/// round caps (so the joint closes), or one segment.
pub(crate) fn paint_mark(pc: &mut PaintCx, r: Rect, color: Color, indeterminate: bool) {
    let w = pc.snap_width((r.width() / 9.0).max(1.0));
    let inner = r.inset(Insets::all(r.width() * 0.22));
    let c = center(inner);
    let line = |from: Point, to: Point, cap: LineCap| Line { from, to, width: w, color, cap, dash: None, wavy: false };
    let ink = inner.inflate(w / 2.0);
    if indeterminate {
        let y = (c.y * pc.scale).round() / pc.scale;
        pc.push(Prim::Line(line(Point::new(inner.min.x, y), Point::new(inner.max.x, y), LineCap::Butt)), ink);
    } else {
        let (iw, ih) = (inner.width(), inner.height());
        let a = Point::new(inner.min.x, c.y + ih * 0.02);
        let b = Point::new(inner.min.x + iw * 0.38, inner.max.y - ih * 0.12);
        let e = Point::new(inner.max.x, inner.min.y + ih * 0.12);
        pc.push(Prim::Line(line(a, b, LineCap::Round)), ink);
        pc.push(Prim::Line(line(b, e, LineCap::Round)), ink);
    }
}

/// A filled circle (the radio dot, the switch knob): a quad with full radii.
pub(crate) fn paint_dot(pc: &mut PaintCx, at: Point, radius: f32, fill: Color) {
    let r = Rect::from_origin_size(Point::new(at.x - radius, at.y - radius), Size::new(2.0 * radius, 2.0 * radius));
    pc.push(Prim::Quad(Quad { rect: r, radii: Corners::all(radius), fill: Fill::Solid(fill), border_widths: Insets::ZERO, border_color: Color::TRANSPARENT, border_style: BorderStyle::Solid }), r);
}

/// The host's view of an indicator kind in `state` (the node's state with the kind's prop mirrors).
pub(crate) fn host_in<'a>(style: &crate::shim::core::Styles<'a>, ua: Ua, state: StateBits) -> StyleView<'a> {
    StyleView::new(style.computed, ua, state, style.dark).with_theme(style.theme).with_inherited(style.inherited)
}

// ---- the kind ----

/// Paints the check box at `rect` under `host` with its parts' computed styles: indicator, mark
/// (check or dash), icon, caption, focus ring. For foreign painters too.
#[allow(clippy::too_many_arguments)]
pub fn paint(pc: &mut PaintCx, host: &StyleView<'_>, parts: &[(&'static str, &ComputedStyle)], rect: Rect, text: &str, icon: &str, on: bool, partial: bool) {
    let mark_css = part_css(parts, "mark");
    paint_indicator(pc, host, parts, rect, Indicator { name: "indicator", ua: INDICATOR, host_css: true }, text, icon, |pc, host, ir| {
        if on || partial {
            let mark = part_with(host, mark_css, MARK, false);
            let color = mark.color("color").unwrap_or(Color::BLACK);
            paint_mark(pc, ir, color, partial);
        }
    });
}

/// A check box with a caption.
#[derive(Debug, Default)]
pub struct Checkbox {
    pub props: CheckboxProps,
    units: Units,
    hover: Hoverable,
    focus: Focusable,
    press: Pressable,
    toggle: Toggleable,
}

impl Checkbox {
    fn toggle_params(&self) -> ToggleParams {
        ToggleParams { checked: self.props.checked, indeterminate: self.props.indeterminate, tristate: self.props.tristate, radio: false }
    }

    /// `state` with the prop mirrors (`:checked`, `:indeterminate`, `:disabled`).
    fn mirror(&self, mut state: StateBits) -> StateBits {
        state.set(StateBits::CHECKED, self.props.checked && !self.props.indeterminate);
        state.set(StateBits::INDETERMINATE, self.props.indeterminate);
        state.set(StateBits::DISABLED, self.props.disabled);
        state
    }

    const IND: Indicator = Indicator { name: "indicator", ua: INDICATOR, host_css: true };
}

impl Kind for Checkbox {
    fn parts() -> &'static [&'static str] {
        &["indicator", "mark", "icon", "label"]
    }
    fn prop_state(&self) -> crate::interact::StateBits {
        self.mirror(crate::interact::StateBits::NONE)
    }
    fn prop_states() -> crate::interact::StateBits {
        crate::interact::StateBits(crate::interact::StateBits::CHECKED.0 | crate::interact::StateBits::INDETERMINATE.0)
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
        let host = host_in(&cx.style, UA, self.mirror(cx.style.state));
        let s = measure_indicator(cx.text, &host, cx.style.parts, Self::IND, &self.props.text, &self.props.icon);
        // a definite width caps the caption (CSS fit-content; it elides past it)
        taffy::Size { width: common::fit_width(s.x, avail.width), height: s.y }
    }

    fn record(&self, cx: &mut RecordCx) {
        let host = host_in(&cx.style, UA, self.mirror(cx.style.state));
        let parts = cx.style.parts;
        let rect = cx.rect();
        paint(&mut PaintCx::of(cx), &host, parts, rect, &self.props.text, &self.props.icon, self.props.checked, self.props.indeterminate);
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
        for Toggle::Toggled { checked, indeterminate } in toggled.iter().copied() {
            // EV1: the signal goes out before the default action (the prop writes)
            if common::emit_sync(cx, &SPEC, "changed", vec![Value::Bool(checked)]) {
                // prevented: the value stays (the toggle unit's bits are put back below)
                continue;
            }
            if self.props.checked != checked {
                self.props.checked = checked;
                cx.prop_changed(P::checked.id());
            }
            if self.props.indeterminate != indeterminate {
                self.props.indeterminate = indeterminate;
                cx.prop_changed(P::indeterminate.id());
            }
        }
        // a prevented toggle leaves the unit's bits at the new value: back to the props
        *cx.state = self.mirror(*cx.state);
        if (ev.is_pointer() && (pass.resp.hovered || pass.resp.down_on)) || !pressed.is_empty() || !toggled.is_empty() { Handled::Yes } else { Handled::No }
    }

    fn a11y(&self, cx: &mut A11yCx) {
        cx.node.set_role(accesskit::Role::CheckBox);
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
        self.press.claims(&PressParams::default(), key, Mods::of(m, false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::testkit::{self, Driver};
    use crate::paint::ua_token;
    use skelter_base::change::ChangeClass;

    const SIZE: Size = Size { w: 90.0, h: 18.0 };

    fn boxed(text: &str) -> Checkbox {
        let mut c = Checkbox::default();
        c.props.text = text.into();
        c
    }

    fn fill(p: &Prim) -> Color {
        match p {
            Prim::Quad(Quad { fill: Fill::Solid(c), .. }) => *c,
            other => panic!("not a solid quad: {other:?}"),
        }
    }

    #[test]
    fn spec() {
        let s = Checkbox::spec();
        assert_eq!(s.tag, "checkbox");
        assert_eq!(s.role, accesskit::Role::CheckBox);
        assert!(s.focusable);
        let checked = s.prop_spec(s.prop("checked").unwrap()).unwrap();
        assert!(checked.bindable && checked.change == ChangeClass::Paint);
        assert!(s.prop_spec(s.prop("indeterminate").unwrap()).unwrap().bindable);
        assert_eq!(s.prop_spec(s.prop("text").unwrap()).unwrap().change, ChangeClass::Layout);
        assert_eq!(s.prop_spec(s.prop("tristate").unwrap()).unwrap().change, ChangeClass::Property);
        let changed = &s.signals[s.signal("changed").unwrap().0 as usize];
        assert_eq!(changed.args, &[("checked", ValueType::Bool)]);
        assert!(s.signal("clicked").is_some());
        let mut c = Checkbox::default();
        assert!(c.set_prop(s.prop("checked").unwrap(), &Value::str("yes")).is_err(), "type-checked");
    }

    #[test]
    fn measures_indicator_gap_and_caption() {
        let s = testkit::measure(&boxed("OK"), StateBits::NONE);
        let adv = crate::shim::testing::MonoShaper::advance(13.0);
        assert_eq!(s.w, 18.0 + 6.0 + 2.0 * adv);
        assert_eq!(s.h, 18.0);
        assert_eq!(testkit::measure(&Checkbox::default(), StateBits::NONE), Size::new(18.0, 18.0), "the box alone");
    }

    #[test]
    fn records_unchecked_checked_and_indeterminate() {
        let mut c = boxed("Accept");
        let (list, res) = testkit::record(&c, SIZE, StateBits::NONE);
        assert_eq!(testkit::kinds(&list), ["quad", "glyphs"]);
        assert_eq!(res.runs.len(), 1);
        let Prim::Quad(q) = &list.items[0] else { unreachable!() };
        assert_eq!(q.rect, Rect::new(0.0, 0.0, 18.0, 18.0));
        assert_eq!(q.radii.tl, 3.0);
        assert_eq!(q.border_widths.top, 1.0);
        assert_eq!(fill(&list.items[0]), ua_token(true, "checkbox-bg").unwrap());
        let Prim::Glyphs(g) = &list.items[1] else { unreachable!() };
        assert_eq!(g.origin.x, 24.0, "the caption after the gap");

        c.props.checked = true;
        let (list, _) = testkit::record(&c, SIZE, StateBits::NONE);
        assert_eq!(testkit::kinds(&list), ["quad", "glyphs", "line", "line"], "the box, caption, then the check mark");
        assert_eq!(fill(&list.items[0]), ua_token(true, "toggle-on-bg").unwrap(), "R17 on-fill");
        let (Prim::Line(a), Prim::Line(b)) = (&list.items[2], &list.items[3]) else { unreachable!() };
        assert_eq!(a.to, b.from, "one polyline");
        assert_eq!((a.cap, a.width), (LineCap::Round, 2.0));
        assert_eq!(a.color, ua_token(true, "fg").unwrap());
        assert!((a.from.x - 18.0 * 0.22).abs() < 1e-4 && (b.to.x - 18.0 * 0.78).abs() < 1e-4);

        c.props.indeterminate = true;
        let (list, _) = testkit::record(&c, SIZE, StateBits::NONE);
        assert_eq!(testkit::kinds(&list), ["quad", "glyphs", "line"], "the dash");
        let Prim::Line(d) = &list.items[2] else { unreachable!() };
        assert_eq!((d.from.y, d.to.y), (9.0, 9.0));
        assert_eq!(fill(&list.items[0]), ua_token(true, "toggle-on-bg").unwrap());
    }

    #[test]
    fn records_hover_focus_and_disabled() {
        let mut c = boxed("Accept");
        let (list, _) = testkit::record(&c, SIZE, StateBits::HOVERED);
        assert_eq!(fill(&list.items[0]), ua_token(true, "button-bg-hover").unwrap());
        let (list, _) = testkit::record(&c, SIZE, StateBits::PRESSED);
        assert_eq!(fill(&list.items[0]), ua_token(true, "button-bg-active").unwrap());

        let (list, _) = testkit::record(&c, SIZE, StateBits::FOCUSED | StateBits::FOCUS_VISIBLE);
        assert_eq!(testkit::kinds(&list), ["quad", "glyphs", "quad"], "the ring last");
        let Prim::Quad(ring) = list.items.last().unwrap() else { unreachable!() };
        assert_eq!(ring.rect, Rect::new(-3.0, -3.0, 21.0, 21.0), "around the indicator, 2px at +1 (R8)");
        assert_eq!(ring.border_color, ua_token(true, "focus").unwrap());
        assert_eq!(testkit::kinds(&testkit::record(&c, SIZE, StateBits::FOCUSED).0), ["quad", "glyphs"], "no ring for pointer focus");

        c.props.disabled = true;
        c.props.checked = true;
        let (list, _) = testkit::record(&c, SIZE, StateBits::NONE);
        let (Prim::Glyphs(g), Prim::Line(m)) = (&list.items[1], &list.items[2]) else { unreachable!() };
        assert_eq!(g.color, ua_token(true, "disabled-fg").unwrap());
        assert_eq!(m.color, ua_token(true, "disabled-fg").unwrap());
    }

    #[test]
    fn click_toggles_and_emits() {
        let mut d = Driver::new(boxed("A"), SIZE);
        d.click();
        assert_eq!(d.signals, [("clicked", vec![]), ("changed", vec![Value::Bool(true)])]);
        assert_eq!(d.changed, ["checked"]);
        assert_eq!(d.get("checked"), Some(Value::Bool(true)));
        assert!(d.state.contains(StateBits::CHECKED));
        d.signals.clear();
        d.click();
        assert_eq!(d.take(), ["clicked", "changed"]);
        assert_eq!(d.get("checked"), Some(Value::Bool(false)));
        assert!(!d.state.contains(StateBits::CHECKED));
    }

    #[test]
    fn space_toggles_when_focused() {
        let mut d = Driver::new(boxed("A"), SIZE);
        d.key(Key::Space);
        assert!(d.take().is_empty(), "not focused");
        d.focus(true);
        assert!(d.state.contains(StateBits::FOCUS_VISIBLE));
        d.key(Key::Space);
        assert_eq!(d.take(), ["clicked", "changed"]);
        assert_eq!(d.get("checked"), Some(Value::Bool(true)));
        assert!(d.kind.claims_key(Key::Space, Default::default()));
    }

    #[test]
    fn accesskit_click_toggles() {
        let mut d = Driver::new(boxed("A"), SIZE);
        d.a11y_action(accesskit::Action::Click);
        assert_eq!(d.take(), ["clicked", "changed"]);
        assert_eq!(d.get("checked"), Some(Value::Bool(true)));
    }

    #[test]
    fn disabled_does_not_toggle() {
        let mut d = Driver::new(boxed("A"), SIZE);
        d.set("disabled", Value::Bool(true));
        d.click();
        d.a11y_action(accesskit::Action::Click);
        assert!(d.take().is_empty());
        assert_eq!(d.get("checked"), Some(Value::Bool(false)));
        assert!(d.state.contains(StateBits::DISABLED) && d.state.contains(StateBits::HOVERED), "R9: still hoverable");
    }

    #[test]
    fn indeterminate_click_checks_and_tristate_cycles() {
        let mut d = Driver::new(boxed("A"), SIZE);
        d.set("indeterminate", Value::Bool(true));
        d.click();
        assert_eq!((d.get("checked"), d.get("indeterminate")), (Some(Value::Bool(true)), Some(Value::Bool(false))), "Qt, GTK");
        assert_eq!(d.changed, ["checked", "indeterminate"]);

        let mut d = Driver::new(boxed("A"), SIZE);
        d.set("tristate", Value::Bool(true));
        let mut seen = Vec::new();
        for _ in 0..3 {
            d.click();
            seen.push((d.get("checked").unwrap().as_bool().unwrap(), d.get("indeterminate").unwrap().as_bool().unwrap()));
        }
        assert_eq!(seen, [(true, false), (false, true), (false, false)], "unchecked → checked → indeterminate → unchecked");
        assert!(!d.state.contains(StateBits::INDETERMINATE));
    }

    #[test]
    fn a11y_node() {
        let mut c = boxed("Accept");
        let n = testkit::a11y(&c, StateBits::NONE, SIZE);
        assert_eq!(n.role(), accesskit::Role::CheckBox);
        assert_eq!(n.label(), Some("Accept"));
        assert_eq!(n.toggled(), Some(accesskit::Toggled::False));
        assert!(n.supports_action(accesskit::Action::Click) && n.supports_action(accesskit::Action::Focus));
        c.props.checked = true;
        assert_eq!(testkit::a11y(&c, StateBits::NONE, SIZE).toggled(), Some(accesskit::Toggled::True));
        c.props.indeterminate = true;
        assert_eq!(testkit::a11y(&c, StateBits::NONE, SIZE).toggled(), Some(accesskit::Toggled::Mixed));
        c.props.disabled = true;
        assert!(testkit::a11y(&c, StateBits::NONE, SIZE).is_disabled());
    }

    #[test]
    fn a_prevented_changed_keeps_the_value() {
        let mut d = Driver::new(Checkbox::default(), skelter_base::geom::Size::new(80.0, 20.0));
        d.prevent = vec!["changed"];
        d.click();
        assert!(d.names().contains(&"changed"));
        assert!(!d.kind.props.checked && !d.state.contains(StateBits::CHECKED), "the toggle did not happen");
        assert!(!d.changed.contains(&"checked"));
    }
}
