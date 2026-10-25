//! `frame` (ARCH-WIDGETS §6.1, WIDGET-PARITY §2.16, Qt's QGroupBox; ported from svelters-egui
//! `widgets/frame.rs`): a container that paints a box per `frame-style` and the `caption` paint
//! part, and lays out its children inside the border, the UA padding and the caption row
//! ([`Kind::chrome_padding`]: a CSS `padding` adds to the UA's). It takes the box props
//! ([`box_`](crate::controls::box_)).
//!
//! `checkable` (QGroupBox): an indicator before the caption (the checkbox's `indicator` and `mark`
//! parts); unchecking it makes the content inert ([`Kind::children_inert`]: nothing in it takes
//! input). AccessKit `Role::Group` named by the label. States: `:checked` (when checkable),
//! `:hover`, `:focus-visible` (the ring around the indicator). Only a click on the caption row (or
//! Space/Enter on the focused frame, AccessKit `Click`) toggles the check. DESIGN R16: a section is
//! a 1px `border`, never a shadow; the floating styles (`window`, `menu`, `popup`) cast `elev-*`
//! shadows (R7).

use crate::controls::box_::{ALIGN, DIRECTION, Explicit, JUSTIFY, Layout, WRAP};
use crate::controls::common::{self, Units};
use crate::interact::{FocusParams, Focusable, Hoverable, Intents, Mods, PressParams, Pressable, Sense, StateBits, Toggle, ToggleParams, Toggleable, Unit};
use crate::kind_props;
use crate::paint::{self, Align, PaintCx, StyleView, Ua, UaRule, UaValue::*};
use crate::shim::core::{A11yCx, Event, EventCx, Handled, Kind, KindSpec, MeasureCx, NameFrom, PrepareCx, PropError, PropId, RecordCx, SignalSpec, StyleCx, Styles, Value, ValueType};
use crate::shim::text::TextShaper;
use presto_base::accesskit;
use presto_base::color::Color;
use presto_base::display::{Line, LineCap, Prim};
use presto_base::geom::{Point, Rect, Size, Vec2};
use presto_base::key::Key;
use svelters_style::ComputedStyle;
use taffy::{LengthPercentage, Style};

pub const FRAME_STYLE: &[&str] = &["group", "canvas", "window", "menu", "popup", "panel", "none"];

const FOCUS: UaRule = UaRule::when(StateBits::FOCUS_VISIBLE, &[("outline-style", Keyword("solid"))]);
const RING: [(&str, paint::UaValue); 3] = [("outline-color", Token("focus")), ("outline-width", Px(1.0)), ("outline-offset", Px(1.0))];

/// `frame-style="group"` (the default): a 1px `border` section, radius 4 (R16).
pub const GROUP: Ua = Ua(&[
    UaRule::base(&[
        ("border-color", Token("border")),
        ("border-width", Token("stroke")),
        ("border-radius", Token("radius")),
        ("padding-left", Px(6.0)),
        ("padding-right", Px(6.0)),
        ("padding-top", Px(6.0)),
        ("padding-bottom", Px(6.0)),
        ("gap", Px(6.0)),
        RING[0],
        RING[1],
        RING[2],
    ]),
    FOCUS,
]);
/// `canvas`: the field look (sunken content, R4's fill without the inset).
pub const CANVAS: Ua = Ua(&[UaRule::base(&[
    ("background-color", Token("control-bg")),
    ("border-color", Token("control-border")),
    ("border-width", Token("stroke")),
    ("border-radius", Token("radius-xs")),
    ("gap", Px(6.0)),
])]);
/// `window`, `menu`, `popup`: floating surfaces (R7).
pub const FLOATING: Ua = Ua(&[UaRule::base(&[
    ("background-color", Token("menu-bg")),
    ("border-color", Token("menu-border-color")),
    ("border-width", Token("stroke")),
    ("border-radius", Token("menu-radius")),
    ("box-shadow", Token("elev-2")),
    ("padding-left", Px(6.0)),
    ("padding-right", Px(6.0)),
    ("padding-top", Px(6.0)),
    ("padding-bottom", Px(6.0)),
    ("gap", Px(6.0)),
])]);
/// `panel`: the panel fill, flat.
pub const PANEL: Ua = Ua(&[UaRule::base(&[
    ("background-color", Token("panel-bg")),
    ("padding-left", Px(8.0)),
    ("padding-right", Px(8.0)),
    ("padding-top", Px(2.0)),
    ("padding-bottom", Px(2.0)),
    ("gap", Px(6.0)),
])]);
/// `none`: no chrome.
pub const NONE: Ua = Ua(&[UaRule::base(&[("gap", Px(6.0)), RING[0], RING[1], RING[2]]), FOCUS]);

/// `::part(caption)`: the title, strong (QGroupBox::title).
pub const CAPTION: Ua = Ua(&[UaRule::base(&[("font-weight", Number(600.0)), ("color", Token("fg-strong"))])]);

/// `::part(indicator)`: the checkbox's indicator (the same table as `checkbox`'s).
pub const INDICATOR: Ua = Ua(&[
    UaRule::base(&[
        ("width", Px(18.0)),
        ("height", Px(18.0)),
        ("background-color", Token("checkbox-bg")),
        ("border-color", Token("checkbox-border-color")),
        ("border-width", Token("stroke")),
        ("border-radius", Px(3.0)),
    ]),
    UaRule::when(StateBits::HOVERED, &[("background-color", Token("button-bg-hover"))]),
    UaRule::when(StateBits::CHECKED, &[("background-color", Token("toggle-on-bg")), ("border-color", Token("toggle-on-border"))]),
    UaRule::when(StateBits::INDETERMINATE, &[("background-color", Token("toggle-on-bg")), ("border-color", Token("toggle-on-border"))]),
    UaRule::when(StateBits::PRESSED, &[("background-color", Token("button-bg-active"))]),
]);

/// `::part(mark)`: the check glyph's colour (the same table as `checkbox`'s).
pub const MARK: Ua = Ua(&[UaRule::base(&[("color", Token("fg"))]), UaRule::when(StateBits::DISABLED, &[("color", Token("disabled-fg"))])]);

kind_props! {
    /// The frame's props.
    pub struct FrameProps;
    /// The caption above the content.
    label "label": String = Str("") ; Str, Layout;
    /// The chrome: `group`, `canvas`, `window`, `menu`, `popup`, `panel`, `none`.
    frame_style "frame-style": String = Enum("group") ; Enum(FRAME_STYLE), Layout;
    /// WIDGET-PARITY §2.16 (QGroupBox `checkable`): a check before the caption; unchecked makes
    /// the content inert.
    checkable "checkable": bool = Bool(false) ; Bool, Layout;
    /// Checked (`bind:checked`), when `checkable`.
    checked "checked": bool = Bool(true) ; Bool, Paint, bindable;
    disabled "disabled": bool = Bool(false) ; Bool, Paint;
    /// Main axis of the content.
    flex_direction "flex-direction": String = Enum("column") ; Enum(DIRECTION), Layout;
    /// Whether children wrap onto more lines.
    flex_wrap "flex-wrap": String = Enum("nowrap") ; Enum(WRAP), Layout;
    /// Space between children on both axes, in points (row-gap and column-gap override it).
    gap "gap": f64 = F64(0.0) ; F64, Layout;
    /// Space between rows in points; null to use `gap`.
    row_gap "row-gap": Option<f64> = None ; F64, Layout;
    /// Space between columns in points; null to use `gap`.
    column_gap "column-gap": Option<f64> = None ; F64, Layout;
    /// Inner spacing on all four sides, in points (added to the chrome's).
    padding "padding": f64 = F64(0.0) ; F64, Layout;
    /// Cross-axis alignment of the children.
    align_items "align-items": String = Enum("stretch") ; Enum(ALIGN), Layout;
    /// Main-axis distribution of the children.
    justify_content "justify-content": String = Enum("start") ; Enum(JUSTIFY), Layout;
}

/// The signals, in spec order.
pub const SIGNALS: &[SignalSpec] = &[SignalSpec::new("changed", &[("checked", ValueType::Bool)], "The user toggled the check; carries the new state.")];

pub static SPEC: KindSpec = KindSpec { tag: "frame", props: FrameProps::SPECS, signals: SIGNALS, slots: &[], children: true, role: accesskit::Role::Group, name_from: NameFrom::Prop("label"), text_prop: None, focusable: false };

/// `padding` grown by `add` (left, right, top, bottom, in points) where it is a length: what
/// presto-core does with [`Kind::chrome_padding`] (svelters-egui `LayoutCx::padded_layout`).
pub fn padded(s: &mut Style, [l, r, t, b]: [f32; 4]) {
    let grow = |p: LengthPercentage, a: f32| {
        let raw = p.into_raw();
        if raw.tag() == taffy::style::CompactLength::LENGTH_TAG { LengthPercentage::length(raw.value() + a) } else { p }
    };
    let p = s.padding;
    s.padding = taffy::Rect { left: grow(p.left, l), right: grow(p.right, r), top: grow(p.top, t), bottom: grow(p.bottom, b) };
}

/// The check mark inside `r`, in `color`: two round-capped strokes.
fn paint_mark(pc: &mut PaintCx, r: Rect, color: Color) {
    let w = pc.snap_width((r.width() / 9.0).max(1.0));
    let inner = r.inflate(-r.width() * 0.22);
    let (iw, ih) = (inner.width(), inner.height());
    let pts = [
        Point::new(inner.min.x, (inner.min.y + inner.max.y) / 2.0 + ih * 0.02),
        Point::new(inner.min.x + iw * 0.38, inner.max.y - ih * 0.12),
        Point::new(inner.max.x, inner.min.y + ih * 0.12),
    ];
    for k in 0..2 {
        let line = Line { from: pts[k], to: pts[k + 1], width: w, color, cap: LineCap::Round, dash: None, wavy: false };
        pc.push(Prim::Line(line), inner.inflate(w));
    }
}

/// A box around its content, with an optional caption (and check).
#[derive(Debug, Default)]
pub struct Frame {
    pub props: FrameProps,
    explicit: Explicit,
    /// The bottom of the caption row in node-local points (from [`Kind::prepare`]): a press below
    /// it does not toggle the check.
    head: f32,
    units: Units,
    hover: Hoverable,
    focus: Focusable,
    press: Pressable,
    toggle: Toggleable,
}

impl Frame {
    fn toggle_params(&self) -> ToggleParams {
        ToggleParams { checked: self.props.checked, ..ToggleParams::default() }
    }
    fn focus_params(&self) -> FocusParams {
        FocusParams { tab_stop: self.props.checkable, ..FocusParams::default() }
    }

    /// The UA table of its `frame-style`.
    pub fn ua(&self) -> Ua {
        match self.props.frame_style.as_str() {
            "canvas" => CANVAS,
            "window" | "menu" | "popup" => FLOATING,
            "panel" => PANEL,
            "none" => NONE,
            _ => GROUP,
        }
    }

    /// `::part(indicator)`'s view: its computed style over [`INDICATOR`] when a rule reaches it,
    /// else the UA table alone in the host's state.
    fn indicator_view<'a>(host: &'a StyleView<'a>, css: Option<&'a ComputedStyle>) -> StyleView<'a> {
        match css {
            Some(c) => host.part(Some(c), INDICATOR),
            None => StyleView::new(None, INDICATOR, host.state(), host.is_dark()),
        }
    }

    /// The indicator's size (its `width`/`height`, else 16).
    fn indicator_size(ind: &StyleView) -> Vec2 {
        Vec2::new(ind.px("width").unwrap_or(16.0), ind.px("height").unwrap_or(16.0))
    }

    /// The caption row's size (indicator, gap, caption), zero without a label or check.
    fn caption_size(&self, text: &mut dyn TextShaper, host: &StyleView, cap: &StyleView, ind_css: Option<&ComputedStyle>) -> Vec2 {
        if self.props.label.is_empty() && !self.props.checkable {
            return Vec2::ZERO;
        }
        let ind = Self::indicator_view(host, ind_css);
        let indicator = self.props.checkable.then(|| Self::indicator_size(&ind));
        let label = (!self.props.label.is_empty()).then(|| paint::measure_text(text, cap, &self.props.label, None));
        let gap = if indicator.is_some() && label.is_some() { host.px("gap").unwrap_or(6.0) } else { 0.0 };
        let (i, t) = (indicator.unwrap_or(Vec2::ZERO), label.unwrap_or(Size::ZERO));
        Vec2::new(i.x + gap + t.w, i.y.max(t.h))
    }

    /// What the content is inset by: (left, right, top, bottom) = UA border + UA padding, plus the
    /// caption row and a gap on top.
    fn chrome(&self, style: &Styles, text: &mut dyn TextShaper) -> [f32; 4] {
        let ua = StyleView::ua_only(self.ua(), StateBits::NONE, true);
        let [bl, br, bt, bb] = ua.sides("border-width");
        let [pl, pr, pt, pb] = ua.sides("padding");
        let host = common::host_view(style, self.ua());
        let cap = host.part(style.part_css("caption"), CAPTION);
        let c = self.caption_size(text, &host, &cap, style.part_css("indicator"));
        let top = if c.y > 0.0 { c.y + host.px("gap").unwrap_or(6.0) } else { 0.0 };
        [bl + pl, br + pr, bt + pt + top, bb + pb]
    }

    /// The node's state with the prop mirrors (`:checked` when checkable, `:disabled`).
    fn mirror(&self, state: &mut StateBits) {
        state.set(StateBits::CHECKED, self.props.checkable && self.props.checked);
        state.set(StateBits::DISABLED, self.props.disabled);
    }
}

impl Kind for Frame {
    fn prop_state(&self) -> crate::interact::StateBits {
        { let mut s = crate::interact::StateBits::NONE; self.mirror(&mut s); s }
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
        self.explicit.mark(prop);
        Ok(())
    }
    fn get_prop(&self, prop: PropId) -> Option<Value> {
        self.props.get(prop)
    }

    fn layout_style(&self, cx: &StyleCx) -> Style {
        let mut s = cx.default_layout();
        let l = Layout { cx, explicit: self.explicit };
        let p = &self.props;
        l.display(&mut s, taffy::Display::Flex);
        l.direction(&mut s, P::flex_direction.id(), &p.flex_direction);
        l.wrap(&mut s, P::flex_wrap.id(), &p.flex_wrap);
        l.gaps(&mut s, (P::gap.id(), p.gap), Some((P::row_gap.id(), p.row_gap)), Some((P::column_gap.id(), p.column_gap)));
        l.padding(&mut s, P::padding.id(), p.padding);
        l.align_items(&mut s, P::align_items.id(), &p.align_items);
        l.justify_content(&mut s, P::justify_content.id(), &p.justify_content);
        s
    }

    fn chrome_padding(&self, cx: &mut MeasureCx) -> Option<[f32; 4]> {
        Some(self.chrome(&cx.style, cx.text))
    }

    fn children_inert(&self) -> bool {
        self.props.checkable && !self.props.checked
    }

    fn prepare(&mut self, cx: &mut PrepareCx) {
        let host = common::host_view(&cx.style, self.ua());
        let cap = host.part(cx.style.part_css("caption"), CAPTION);
        let rect = Rect::from_origin_size(Point::ZERO, cx.size);
        let row = self.caption_size(cx.text, &host, &cap, cx.style.part_css("indicator"));
        self.head = paint::box_geom(&host, rect).content.min.y + row.y;
    }

    fn record(&self, cx: &mut RecordCx) {
        let ua = self.ua();
        let host = common::host_view(&cx.style, ua);
        let cap = host.part(cx.style.part_css("caption"), CAPTION);
        let ind_css = cx.style.part_css("indicator");
        let mark_css = cx.style.part_css("mark");
        let rect = cx.rect();
        let mut pc = PaintCx::of(cx);
        let geom = paint::paint_box(&mut pc, &host, rect);
        let csz = self.caption_size(pc.text, &host, &cap, ind_css);
        if csz.y <= 0.0 {
            return;
        }
        let row = Rect::from_origin_size(geom.content.min, Size::new(geom.content.width(), csz.y));
        let mut text_left = row.min.x;
        if self.props.checkable {
            let ind = Self::indicator_view(&host, ind_css);
            let isz = Self::indicator_size(&ind);
            let mid = (row.min.y + row.max.y) / 2.0;
            let ir = Rect::from_origin_size(Point::new(row.min.x, mid - isz.y / 2.0), Size::new(isz.x, isz.y));
            paint::paint_box(&mut pc, &ind, ir);
            if self.props.checked {
                let mark = match mark_css {
                    Some(c) => host.part(Some(c), MARK),
                    None => StyleView::new(None, MARK, host.state(), host.is_dark()),
                };
                paint_mark(&mut pc, ir, mark.color("color").unwrap_or(Color::BLACK));
            }
            // the focus ring goes around the indicator
            paint::paint_outline(&mut pc, &host, ir);
            text_left = ir.max.x + host.px("gap").unwrap_or(6.0);
        }
        if !self.props.label.is_empty() {
            let w = (row.max.x - text_left).max(0.0);
            let text = paint::elide(pc.text, &cap, &self.props.label, paint::Elide::End, w);
            let layout = paint::text_layout(pc.text, &cap, &text, None);
            let tr = Rect { min: Point::new(text_left, row.min.y), max: row.max };
            paint::paint_text(&mut pc, &cap, &layout, tr, Align::of(&cap, Align::Start));
        }
    }

    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        self.mirror(cx.state);
        cx.enabled = !self.props.disabled;
        let (fp, pp, tp) = (self.focus_params(), PressParams::default(), self.toggle_params());
        let sense = if self.props.checkable { Sense::HOVER.union(self.focus.sense(&fp)).union(self.press.sense(&pp)).union(self.toggle.sense(&tp)) } else { Sense::HOVER };
        let mut toggled = Intents::default();
        let (hover, focus, press, toggle) = (&mut self.hover, &mut self.focus, &mut self.press, &mut self.toggle);
        let pass = self.units.run(cx, ev, sense, |icx| {
            hover.input(&(), icx, &mut Intents::default());
            focus.input(&fp, icx, &mut Intents::default());
            press.input(&pp, icx, &mut Intents::default());
            toggle.input(&tp, icx, &mut toggled);
        });
        // only a click on the caption row (or Space on the focused frame) toggles the check
        let on_head = pass.resp.interact_pointer_pos.is_none_or(|p| p.y <= self.head);
        let mut changed = false;
        if self.props.checkable && on_head {
            for Toggle::Toggled { checked, .. } in toggled.iter().copied() {
                common::emit(cx, &SPEC, "changed", vec![Value::Bool(checked)]);
                self.props.checked = checked;
                cx.prop_changed(P::checked.id());
                changed = true;
            }
        }
        self.mirror(cx.state);
        if self.props.checkable && (pass.resp.down_on || changed) { Handled::Yes } else { Handled::No }
    }

    fn a11y(&self, cx: &mut A11yCx) {
        cx.node.set_role(accesskit::Role::Group);
        if !self.props.label.is_empty() {
            cx.node.set_label(self.props.label.clone());
        }
        if self.props.disabled {
            cx.node.set_disabled();
        }
        if self.props.checkable {
            self.focus.a11y(&self.focus_params(), cx.node);
            self.press.a11y(&PressParams::default(), cx.node);
            self.toggle.a11y(&self.toggle_params(), cx.node);
        }
    }

    fn claims_key(&self, key: Key, m: presto_base::event::Modifiers) -> bool {
        let m = Mods::of(m, false);
        self.props.checkable && (self.press.claims(&PressParams::default(), key, m) || self.toggle.claims(&self.toggle_params(), key, m) || self.focus.claims(&self.focus_params(), key, m))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::testkit::{self, Driver};
    use crate::paint::ua_theme;
    use crate::shim::testing::MonoShaper;
    use presto_base::change::ChangeClass;

    fn set(k: &mut Frame, name: &str, v: Value) {
        let id = SPEC.prop(name).unwrap();
        k.set_prop(id, &v).unwrap();
    }

    fn styles(state: StateBits) -> Styles<'static> {
        Styles { computed: None, parts: &[], children: &[], state, dark: true }
    }

    fn chrome(k: &Frame) -> [f32; 4] {
        let mut text = MonoShaper;
        k.chrome_padding(&mut MeasureCx { style: styles(StateBits::NONE), text: &mut text, scale: 1.0 }).unwrap()
    }

    fn prepare(k: &mut Frame, size: Size) {
        let mut text = MonoShaper;
        k.prepare(&mut PrepareCx { size, style: styles(StateBits::NONE), text: &mut text });
    }

    /// The caption line's height under the monospace shaper (13px body text: 16px lines).
    fn caption_h() -> f32 {
        let fs = ua_theme(true).px("font-size").unwrap();
        (fs * 1.25).round()
    }

    #[test]
    fn spec() {
        let s = Frame::spec();
        assert_eq!(s.tag, "frame");
        assert_eq!(s.role, accesskit::Role::Group);
        assert!(s.children);
        let checked = s.prop_spec(s.prop("checked").unwrap()).unwrap();
        assert!(checked.bindable && checked.change == ChangeClass::Paint);
        assert_eq!(checked.default, crate::shim::core::Default::Bool(true));
        assert_eq!(s.prop_spec(s.prop("label").unwrap()).unwrap().change, ChangeClass::Layout);
        assert_eq!(s.prop_spec(s.prop("frame-style").unwrap()).unwrap().default, crate::shim::core::Default::Enum("group"));
        assert!(s.prop("flex-direction").is_some() && s.prop("padding").is_some(), "the box props");
        assert_eq!(s.signals[0].args, &[("checked", ValueType::Bool)]);
        let mut k = Frame::default();
        assert!(k.set_prop(s.prop("frame-style").unwrap(), &Value::str("fancy")).is_err());
    }

    #[test]
    fn chrome_is_ua_border_padding_and_caption_row() {
        let stroke = ua_theme(true).px("stroke").unwrap();
        let mut k = Frame::default();
        // group: border + 6px padding on every side, no caption row
        assert_eq!(chrome(&k), [stroke + 6.0, stroke + 6.0, stroke + 6.0, stroke + 6.0]);
        // a label adds its line and the 6px gap on top
        set(&mut k, "label", Value::str("Placement"));
        assert_eq!(chrome(&k)[2], stroke + 6.0 + caption_h() + 6.0);
        // checkable: the 18px indicator is taller than the line
        set(&mut k, "checkable", Value::Bool(true));
        assert_eq!(chrome(&k)[2], stroke + 6.0 + 18.0f32.max(caption_h()) + 6.0);
        // panel: 8/2 padding, no border
        set(&mut k, "frame-style", Value::str("panel"));
        set(&mut k, "label", Value::str(""));
        set(&mut k, "checkable", Value::Bool(false));
        assert_eq!(chrome(&k), [8.0, 8.0, 2.0, 2.0]);
        set(&mut k, "frame-style", Value::str("none"));
        assert_eq!(chrome(&k), [0.0; 4]);
    }

    #[test]
    fn layout_style_takes_the_box_props_and_the_chrome_adds_to_padding() {
        let mut k = Frame::default();
        set(&mut k, "padding", Value::F64(4.0));
        set(&mut k, "flex-direction", Value::str("row"));
        set(&mut k, "gap", Value::F64(3.0));
        let mut s = testkit::layout_style(&k, None, StateBits::NONE);
        assert_eq!(s.flex_direction, taffy::FlexDirection::Row);
        assert_eq!(s.gap.width, LengthPercentage::length(3.0));
        assert_eq!(s.padding, taffy::Rect::length(4.0));
        let add = chrome(&k);
        padded(&mut s, add);
        assert_eq!(s.padding.left, LengthPercentage::length(4.0 + add[0]));
        assert_eq!(s.padding.top, LengthPercentage::length(4.0 + add[2]));
        // a percentage padding is left as it is
        s.padding.right = LengthPercentage::percent(0.1);
        padded(&mut s, [1.0; 4]);
        assert_eq!(s.padding.right, LengthPercentage::percent(0.1));
    }

    #[test]
    fn records_group_box_and_caption() {
        let mut k = Frame::default();
        let size = Size::new(200.0, 100.0);
        // group: the 1px border section, no shadow (R16)
        let (list, _) = testkit::record(&k, size, StateBits::NONE);
        assert_eq!(testkit::kinds(&list), ["quad"]);
        let Prim::Quad(q) = &list.items[0] else { unreachable!() };
        assert_eq!(q.radii.tl, ua_theme(true).px("radius").unwrap());
        assert_eq!(q.border_widths.top, ua_theme(true).px("stroke").unwrap());
        assert_eq!(q.border_color, ua_theme(true).color("border").unwrap());
        set(&mut k, "label", Value::str("Placement"));
        let (list, res) = testkit::record(&k, size, StateBits::NONE);
        assert_eq!(testkit::kinds(&list), ["quad", "glyphs"]);
        assert_eq!(res.runs.len(), 1);
        let Prim::Glyphs(g) = &list.items[1] else { unreachable!() };
        assert_eq!(g.color, ua_theme(true).color("fg-strong").unwrap(), "the caption is strong");
        // the caption starts at the content box: border + 6px padding
        assert_eq!(g.origin.x, ua_theme(true).px("stroke").unwrap() + 6.0);
        // floating styles cast their shadow first (R7)
        set(&mut k, "frame-style", Value::str("popup"));
        assert_eq!(testkit::kinds(&testkit::record(&k, size, StateBits::NONE).0), ["shadow", "quad", "glyphs"]);
        // none: only the caption
        set(&mut k, "frame-style", Value::str("none"));
        assert_eq!(testkit::kinds(&testkit::record(&k, size, StateBits::NONE).0), ["glyphs"]);
    }

    #[test]
    fn records_the_check() {
        let mut k = Frame::default();
        set(&mut k, "label", Value::str("Enabled"));
        set(&mut k, "checkable", Value::Bool(true));
        let size = Size::new(200.0, 100.0);
        // checked: border, indicator box, the two strokes of the mark, caption
        let (list, _) = testkit::record(&k, size, StateBits::CHECKED);
        assert_eq!(testkit::kinds(&list), ["quad", "quad", "line", "line", "glyphs"]);
        let Prim::Quad(ind) = &list.items[1] else { unreachable!() };
        let inset = ua_theme(true).px("stroke").unwrap() + 6.0;
        assert_eq!(ind.rect.min.x, inset);
        assert_eq!(ind.rect.width(), 18.0);
        assert_eq!(ind.fill, presto_base::color::Fill::Solid(ua_theme(true).color("toggle-on-bg").unwrap()));
        let Prim::Glyphs(g) = &list.items[4] else { unreachable!() };
        assert_eq!(g.origin.x, inset + 18.0 + 6.0, "the caption follows the indicator and the gap");
        // unchecked: no mark; keyboard focus: the ring around the indicator, after it
        set(&mut k, "checked", Value::Bool(false));
        let (list, _) = testkit::record(&k, size, StateBits::FOCUSED | StateBits::FOCUS_VISIBLE);
        assert_eq!(testkit::kinds(&list), ["quad", "quad", "quad", "glyphs"]);
        let Prim::Quad(ring) = &list.items[2] else { unreachable!() };
        assert_eq!(ring.rect.min.x, inset - 2.0, "outside the indicator by offset 1 and width 1");
        assert_eq!(ring.border_color, ua_theme(true).color("focus").unwrap());
    }

    #[test]
    fn caption_click_toggles_the_check() {
        let mut k = Frame::default();
        set(&mut k, "label", Value::str("Enabled"));
        set(&mut k, "checkable", Value::Bool(true));
        let size = Size::new(200.0, 100.0);
        prepare(&mut k, size);
        let mut d = Driver::new(k, size);
        d.hover(true);
        d.down(20.0, 10.0);
        d.advance(50);
        d.up(20.0, 10.0);
        assert_eq!(d.take(), ["changed"]);
        assert_eq!(d.get("checked"), Some(Value::Bool(false)));
        assert_eq!(d.changed, ["checked"]);
        assert!(!d.state.contains(StateBits::CHECKED));
        assert!(d.kind.children_inert(), "unchecked: the content is inert");
        d.advance(500);
        d.down(20.0, 10.0);
        d.advance(50);
        d.up(20.0, 10.0);
        assert_eq!(d.get("checked"), Some(Value::Bool(true)));
        assert!(d.state.contains(StateBits::CHECKED));
        assert!(!d.kind.children_inert());
    }

    #[test]
    fn changed_carries_the_new_state() {
        let mut k = Frame::default();
        set(&mut k, "checkable", Value::Bool(true));
        prepare(&mut k, Size::new(100.0, 60.0));
        let mut d = Driver::new(k, Size::new(100.0, 60.0));
        d.hover(true);
        d.down(10.0, 10.0);
        d.advance(50);
        d.up(10.0, 10.0);
        assert_eq!(d.signals, [("changed", vec![Value::Bool(false)])]);
    }

    #[test]
    fn a_click_below_the_caption_does_not_toggle() {
        let mut k = Frame::default();
        set(&mut k, "label", Value::str("Enabled"));
        set(&mut k, "checkable", Value::Bool(true));
        let size = Size::new(200.0, 100.0);
        prepare(&mut k, size);
        let mut d = Driver::new(k, size);
        d.click();
        assert!(d.take().is_empty(), "the centre (y 50) is below the caption row");
        assert_eq!(d.get("checked"), Some(Value::Bool(true)));
    }

    #[test]
    fn keyboard_and_accesskit_toggle() {
        let mut k = Frame::default();
        set(&mut k, "checkable", Value::Bool(true));
        let mut d = Driver::new(k, Size::new(100.0, 60.0));
        d.focus(true);
        assert!(d.state.contains(StateBits::FOCUS_VISIBLE));
        d.key(Key::Space);
        assert_eq!(d.take(), ["changed"]);
        assert_eq!(d.get("checked"), Some(Value::Bool(false)));
        d.a11y_action(accesskit::Action::Click);
        assert_eq!(d.take(), ["changed"]);
        assert_eq!(d.get("checked"), Some(Value::Bool(true)));
        assert!(d.kind.claims_key(Key::Space, Default::default()));
    }

    #[test]
    fn not_checkable_takes_no_input_but_hovers() {
        let mut d = Driver::new(Frame::default(), Size::new(100.0, 60.0));
        d.hover(true);
        assert!(d.state.contains(StateBits::HOVERED));
        d.down(10.0, 5.0);
        d.advance(50);
        d.up(10.0, 5.0);
        d.focus(true);
        d.key(Key::Space);
        assert!(d.take().is_empty());
        assert!(!d.state.contains(StateBits::CHECKED), "`:checked` only when checkable");
        assert!(!d.kind.children_inert());
        assert!(!d.kind.claims_key(Key::Space, Default::default()));
    }

    #[test]
    fn disabled_does_not_toggle() {
        let mut k = Frame::default();
        set(&mut k, "checkable", Value::Bool(true));
        set(&mut k, "disabled", Value::Bool(true));
        prepare(&mut k, Size::new(100.0, 60.0));
        let mut d = Driver::new(k, Size::new(100.0, 60.0));
        d.hover(true);
        d.down(10.0, 10.0);
        d.advance(50);
        d.up(10.0, 10.0);
        assert!(d.take().is_empty());
        assert!(d.state.contains(StateBits::DISABLED));
        assert_eq!(d.get("checked"), Some(Value::Bool(true)));
    }

    #[test]
    fn a11y_node() {
        let mut k = Frame::default();
        set(&mut k, "label", Value::str("Placement"));
        let n = testkit::a11y(&k, StateBits::NONE, Size::new(100.0, 60.0));
        assert_eq!(n.role(), accesskit::Role::Group);
        assert_eq!(n.label(), Some("Placement"));
        assert!(n.toggled().is_none() && !n.supports_action(accesskit::Action::Click));
        set(&mut k, "checkable", Value::Bool(true));
        let n = testkit::a11y(&k, StateBits::NONE, Size::new(100.0, 60.0));
        assert_eq!(n.toggled(), Some(accesskit::Toggled::True));
        assert!(n.supports_action(accesskit::Action::Click) && n.supports_action(accesskit::Action::Focus));
    }
}
