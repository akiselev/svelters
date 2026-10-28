//! `collapsing-header` (ARCH-WIDGETS §6.1, WIDGET-PARITY §2.16; ported from svelters-egui
//! `widgets/collapsing.rs`): a header row (the `header` paint part: the `twistie` chevron, DESIGN
//! R14, and the `text` caption as the `label` part) above its children, which are laid out only
//! while `open` (closed, they get `display: none` and are not shown: not painted, hit or in the
//! accessibility tree, [`Kind::child_layout`]). The body is indented to the caption, past the
//! twistie ([`Kind::chrome_padding`]). It takes the box props ([`box_`](crate::controls::box_)).
//!
//! Children in `slot="end"` (header actions) are laid out out of the body's flow, right-aligned in
//! the header row, open or closed: as tall as the row, or centred in it when their height is set.
//! The header is their containing block (`position: relative` unless CSS sets `position`).
//!
//! Units: Hoverable, Focusable, Pressable, Expandable. A click on the header row (or Space/Enter on
//! the focused header, AccessKit `Click`) toggles `open`; Right and Left (AccessKit
//! `Expand`/`Collapse`) open and close it. Each user change emits `changed(open)` (svelters-egui's
//! name and argument) and announces `open`; a host write is never announced. States: `:expanded`,
//! `:hover`, `:active`, `:focus-visible`. AccessKit `Role::DisclosureTriangle` named by `text`,
//! with `expanded`.
//!
//! Not yet: svelters-egui's `accordion` groups and the header `icon` (the surface keeps both props).

use crate::controls::box_::{ALIGN, DIRECTION, Explicit, JUSTIFY, Layout, WRAP};
use crate::controls::common::{self, Units};
use crate::interact::{Expand, ExpandParams, Expandable, FocusParams, Focusable, Hoverable, Intents, Mods, PressParams, Pressable, Sense, StateBits, Unit};
use crate::kind_props;
use crate::paint::{self, Align, PaintCx, Ua, UaRule, UaValue::*};
use crate::shim::core::{A11yCx, ChildCx, Event, EventCx, Handled, Kind, KindSpec, MeasureCx, NameFrom, PrepareCx, PropError, PropId, RecordCx, SignalSpec, StyleCx, Styles, Value, ValueType};
use crate::shim::text::TextShaper;
use skelter_base::accesskit;
use skelter_base::color::Color;
use skelter_base::display::{Line, LineCap, Prim};
use skelter_base::geom::{Point, Rect, Size};
use skelter_base::key::Key;
use taffy::{Display, LengthPercentageAuto, Position, Style};

/// The host: the body's gap below the header.
pub const UA: Ua = Ua(&[UaRule::base(&[("gap", Px(2.0))])]);

/// `::part(header)`: the row (VS Code pane header metrics, flat).
pub const HEADER: Ua = Ua(&[
    UaRule::base(&[
        ("height", Token("header-h")),
        ("gap", Px(4.0)),
        ("padding-right", Px(4.0)),
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

/// `::part(label)`: the caption.
pub const LABEL: Ua = Ua(&[]);

kind_props! {
    /// The collapsing header's props.
    pub struct CollapsingProps;
    /// The header's caption (also its accessible name).
    text "text": String = Str("") ; Str, Layout;
    /// Shows the children (`bind:open`); a click on the header (or Enter/Space on it) toggles it.
    open "open": bool = Bool(false) ; Bool, Layout, bindable;
    disabled "disabled": bool = Bool(false) ; Bool, Paint;
    /// Main axis of the body.
    flex_direction "flex-direction": String = Enum("column") ; Enum(DIRECTION), Layout;
    /// Whether children wrap onto more lines.
    flex_wrap "flex-wrap": String = Enum("nowrap") ; Enum(WRAP), Layout;
    /// Space between children on both axes, in points (row-gap and column-gap override it).
    gap "gap": f64 = F64(0.0) ; F64, Layout;
    /// Space between rows in points; null to use `gap`.
    row_gap "row-gap": Option<f64> = None ; F64, Layout;
    /// Space between columns in points; null to use `gap`.
    column_gap "column-gap": Option<f64> = None ; F64, Layout;
    /// Inner spacing on all four sides, in points (added to the header row and the indent).
    padding "padding": f64 = F64(0.0) ; F64, Layout;
    /// Cross-axis alignment of the children.
    align_items "align-items": String = Enum("stretch") ; Enum(ALIGN), Layout;
    /// Main-axis distribution of the children.
    justify_content "justify-content": String = Enum("start") ; Enum(JUSTIFY), Layout;
}

/// The signals, in spec order (svelters-egui's).
pub const SIGNALS: &[SignalSpec] = &[SignalSpec::new("changed", &[("open", ValueType::Bool)], "The user opened or closed it; carries the new state.")];

pub static SPEC: KindSpec = KindSpec {
    tag: "collapsing-header",
    props: CollapsingProps::SPECS,
    signals: SIGNALS,
    slots: &["end"],
    children: true,
    role: accesskit::Role::DisclosureTriangle,
    name_from: NameFrom::Prop("text"),
    text_prop: None,
    focusable: true,
};

/// The header row's metrics.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Head {
    /// The row's height.
    h: f32,
    /// The body's indent: the twistie and the header gap (the caption's x).
    indent: f32,
    /// The host's gap between the row and the body.
    gap: f32,
    /// The row's natural width: the twistie and the whole caption, with the header's padding.
    w: f32,
}

/// A header that shows or hides its children.
#[derive(Debug, Default)]
pub struct CollapsingHeader {
    pub props: CollapsingProps,
    explicit: Explicit,
    /// The header row's height in node-local points (from [`Kind::prepare`]): a click below it
    /// (the body's empty space) does not toggle.
    head: f32,
    /// The header row's height as the last restyle measured it ([`Kind::chrome_padding`], which
    /// runs before the children are placed): what [`Kind::child_layout`] fits the end slot to.
    row: std::cell::Cell<f32>,
    units: Units,
    hover: Hoverable,
    focus: Focusable,
    press: Pressable,
    expand: Expandable,
}

impl CollapsingHeader {
    fn expand_params(&self) -> ExpandParams {
        ExpandParams { expanded: self.props.open, on_click: false }
    }

    /// The header row's metrics under `host` (and its parts' computed styles in `style`).
    fn head(&self, style: &Styles, text: &mut dyn TextShaper) -> Head {
        let host = common::host_view(style, UA);
        let head = host.part(style.part_css("header"), HEADER);
        let tw = host.part(style.part_css("twistie"), TWISTIE);
        let label = host.part(style.part_css("label"), LABEL);
        let t = if self.props.text.is_empty() { Size::ZERO } else { paint::measure_text(text, &label, &self.props.text, None) };
        let twistie = tw.px("width").unwrap_or(16.0);
        let gap = head.px("gap").unwrap_or(4.0);
        let [pl, pr, pt, pb] = head.sides("padding");
        Head { h: head.px("height").unwrap_or(0.0).max(t.h + pt + pb), indent: pl + twistie + gap, gap: host.px("gap").unwrap_or(0.0), w: pl + twistie + gap + t.w + pr }
    }

    /// The node's state with the prop mirrors (`:expanded`, `:disabled`).
    fn mirror(&self, state: &mut StateBits) {
        state.set(StateBits::EXPANDED, self.props.open);
        state.set(StateBits::DISABLED, self.props.disabled);
    }
}

impl Kind for CollapsingHeader {
    fn parts() -> &'static [&'static str] {
        &["header", "twistie", "label"]
    }
    fn places_children() -> bool {
        true
    }
    fn prop_state(&self) -> StateBits {
        let mut s = StateBits::NONE;
        self.mirror(&mut s);
        s
    }
    fn prop_states() -> StateBits {
        StateBits::EXPANDED
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
        l.display(&mut s, Display::Flex);
        l.direction(&mut s, P::flex_direction.id(), &p.flex_direction);
        l.wrap(&mut s, P::flex_wrap.id(), &p.flex_wrap);
        l.gaps(&mut s, (P::gap.id(), p.gap), Some((P::row_gap.id(), p.row_gap)), Some((P::column_gap.id(), p.column_gap)));
        l.padding(&mut s, P::padding.id(), p.padding);
        l.align_items(&mut s, P::align_items.id(), &p.align_items);
        l.justify_content(&mut s, P::justify_content.id(), &p.justify_content);
        // the containing block of the end slot's absolute boxes (boxes are `static` by default, so
        // they would escape to an ancestor)
        if !cx.computed.is_some_and(|c| c.get("position").is_some()) {
            s.position = Position::Relative;
        }
        s
    }

    /// Open: the header row and the gap on top, the indent on the left; closed: the row alone
    /// (the body's children are `display: none`).
    fn chrome_padding(&self, cx: &mut MeasureCx) -> Option<[f32; 4]> {
        let h = self.head(&cx.style, cx.text);
        self.row.set(h.h);
        Some([if self.props.open { h.indent } else { 0.0 }, 0.0, if self.props.open { h.h + h.gap } else { h.h }, 0.0])
    }

    /// The header row: the twistie and the whole caption (egui's header sizes the widget).
    fn chrome_min_size(&self, cx: &mut MeasureCx) -> Option<[f32; 2]> {
        let h = self.head(&cx.style, cx.text);
        Some([h.w.ceil(), h.h])
    }

    fn child_layout(&self, cx: &ChildCx, style: &mut Style) -> bool {
        if cx.slot_of(cx.index) == "end" {
            // a header action: out of the body's flow, at the right end of the header row (inside
            // the header's UA right padding), filling the row's height or centred in it
            style.position = Position::Absolute;
            let row = self.row.get();
            let raw = style.size.height.into_raw();
            let top = if raw.tag() == taffy::style::CompactLength::LENGTH_TAG {
                ((row - raw.value()) / 2.0).max(0.0)
            } else {
                if row > 0.0 && style.size.height.is_auto() {
                    style.size.height = taffy::Dimension::length(row);
                }
                0.0
            };
            style.inset = taffy::Rect { left: LengthPercentageAuto::auto(), right: LengthPercentageAuto::length(4.0), top: LengthPercentageAuto::length(top), bottom: LengthPercentageAuto::auto() };
            return true;
        }
        if !self.props.open {
            style.display = Display::None;
        }
        self.props.open
    }

    fn prepare(&mut self, cx: &mut PrepareCx) {
        self.head = self.head(&cx.style, cx.text).h;
    }

    fn record(&self, cx: &mut RecordCx) {
        let host = common::host_view(&cx.style, UA);
        let head_v = host.part(cx.style.part_css("header"), HEADER);
        let tw = host.part(cx.style.part_css("twistie"), TWISTIE);
        let label = host.part(cx.style.part_css("label"), LABEL);
        let h = self.head(&cx.style, cx.text);
        let row = Rect::from_origin_size(Point::ZERO, Size::new(cx.size.w, h.h));
        let mut pc = PaintCx::of(cx);
        let g = paint::paint_box(&mut pc, &head_v, row);
        // the chevron: `>` closed, `v` open
        let twidth = tw.px("width").unwrap_or(16.0);
        let color = tw.color("color").unwrap_or(Color::TRANSPARENT);
        let (c, s) = (Point::new(g.content.min.x + twidth / 2.0, (row.min.y + row.max.y) / 2.0), 3.0);
        let (a, b, d) = if self.props.open {
            (Point::new(c.x - s, c.y - s / 2.0), Point::new(c.x, c.y + s / 2.0), Point::new(c.x + s, c.y - s / 2.0))
        } else {
            (Point::new(c.x - s / 2.0, c.y - s), Point::new(c.x + s / 2.0, c.y), Point::new(c.x - s / 2.0, c.y + s))
        };
        for (from, to) in [(a, b), (b, d)] {
            let ink = Rect::new(from.x.min(to.x), from.y.min(to.y), from.x.max(to.x), from.y.max(to.y)).inflate(1.5);
            pc.push(Prim::Line(Line { from, to, width: 1.5, color, cap: LineCap::Round, dash: None, wavy: false }), ink);
        }
        if !self.props.text.is_empty() {
            let x = g.content.min.x + twidth + head_v.px("gap").unwrap_or(4.0);
            let w = (g.content.max.x - x).max(0.0);
            let text = paint::elide(pc.text, &label, &self.props.text, paint::Elide::End, w);
            let layout = paint::text_layout(pc.text, &label, &text, None);
            let tr = Rect { min: Point::new(x, row.min.y), max: Point::new(g.content.max.x.max(x), row.max.y) };
            paint::paint_text(&mut pc, &label, &layout, tr, Align::Start);
        }
        paint::paint_outline(&mut pc, &head_v, row);
    }

    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        self.mirror(cx.state);
        cx.enabled = !self.props.disabled;
        let (fp, pp, ep) = (FocusParams::default(), PressParams::default(), self.expand_params());
        let sense = Sense::HOVER.union(self.focus.sense(&fp)).union(self.press.sense(&pp)).union(self.expand.sense(&ep));
        let mut expanded = Intents::default();
        let (hover, focus, press, expand) = (&mut self.hover, &mut self.focus, &mut self.press, &mut self.expand);
        let pass = self.units.run(cx, ev, sense, |icx| {
            hover.input(&(), icx, &mut Intents::default());
            focus.input(&fp, icx, &mut Intents::default());
            press.input(&pp, icx, &mut Intents::default());
            expand.input(&ep, icx, &mut expanded);
        });
        let mut want = expanded.iter().map(|Expand::Set(v)| *v).next_back();
        // a click on the header row (or Space/Enter, AccessKit `Click`: no pointer) toggles; one on
        // the body's empty space does not
        if pass.resp.clicked && cx.enabled && pass.resp.interact_pointer_pos.is_none_or(|p| p.y <= self.head) {
            want = Some(!self.props.open);
        }
        let mut changed = false;
        if let Some(v) = want.filter(|v| *v != self.props.open) {
            self.props.open = v;
            cx.prop_changed(P::open.id());
            common::emit(cx, &SPEC, "changed", vec![Value::Bool(v)]);
            changed = true;
        }
        self.mirror(cx.state);
        if (ev.is_pointer() && pass.resp.down_on) || changed { Handled::Yes } else { Handled::No }
    }

    fn a11y(&self, cx: &mut A11yCx) {
        cx.node.set_role(accesskit::Role::DisclosureTriangle);
        if !self.props.text.is_empty() {
            cx.node.set_label(self.props.text.clone());
        }
        if self.props.disabled {
            cx.node.set_disabled();
        }
        self.focus.a11y(&FocusParams::default(), cx.node);
        self.press.a11y(&PressParams::default(), cx.node);
        self.expand.a11y(&self.expand_params(), cx.node);
    }

    fn claims_key(&self, key: Key, m: skelter_base::event::Modifiers) -> bool {
        let m = Mods::of(m, false);
        self.press.claims(&PressParams::default(), key, m) || self.expand.claims(&self.expand_params(), key, m) || self.focus.claims(&FocusParams::default(), key, m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::testkit::Driver;
    use crate::shim::testing::MonoShaper;

    fn set(k: &mut CollapsingHeader, name: &str, v: Value) {
        k.set_prop(SPEC.prop(name).unwrap(), &v).unwrap();
    }

    fn styles() -> Styles<'static> {
        Styles { computed: None, parts: &[], children: &[], state: StateBits::NONE, dark: true, theme: None, inherited: None }
    }

    fn header(text: &str) -> CollapsingHeader {
        let mut k = CollapsingHeader::default();
        set(&mut k, "text", Value::str(text));
        let mut sh = MonoShaper;
        k.prepare(&mut PrepareCx { size: Size::new(200.0, 100.0), style: styles(), text: &mut sh, emitted: Vec::new(), layouts: None });
        k
    }

    fn chrome(k: &CollapsingHeader) -> [f32; 4] {
        let mut sh = MonoShaper;
        k.chrome_padding(&mut MeasureCx { style: styles(), text: &mut sh, scale: 1.0 }).unwrap()
    }

    #[test]
    fn spec_is_eguis_surface() {
        let s = CollapsingHeader::spec();
        assert_eq!(s.tag, "collapsing-header");
        assert_eq!(s.role, accesskit::Role::DisclosureTriangle);
        assert_eq!(s.slots, ["end"]);
        assert!(s.prop_spec(s.prop("open").unwrap()).unwrap().bindable);
        assert_eq!(s.signals[0].name, "changed");
        assert_eq!(s.signals[0].args, &[("open", ValueType::Bool)]);
    }

    /// svelters-egui's `collapsing_header_toggles_open_and_lays_out_its_body`, the kind's part: a
    /// click on the header opens it (`changed(true)`, `open` announced), again closes it; a click
    /// on the body's empty space does not toggle.
    #[test]
    fn a_header_click_toggles_open() {
        let mut d = Driver::new(header("Placement"), Size::new(200.0, 100.0));
        d.hover(true);
        d.down(40.0, 8.0);
        d.advance(50);
        d.up(40.0, 8.0);
        assert_eq!(d.signals, [("changed", vec![Value::Bool(true)])]);
        assert_eq!(d.changed, ["open"]);
        assert_eq!(d.get("open"), Some(Value::Bool(true)));
        assert!(d.state.contains(StateBits::EXPANDED), "`:expanded`");
        d.advance(500);
        d.down(40.0, 8.0);
        d.advance(50);
        d.up(40.0, 8.0);
        assert_eq!(d.get("open"), Some(Value::Bool(false)));
        assert!(!d.state.contains(StateBits::EXPANDED));
        d.take();
        d.set("open", Value::Bool(true));
        d.advance(500);
        d.down(40.0, 80.0);
        d.advance(50);
        d.up(40.0, 80.0);
        assert!(d.take().is_empty(), "a click below the header row");
        assert_eq!(d.get("open"), Some(Value::Bool(true)));
    }

    /// svelters-egui's `ow1a2_collapsing_header_expands_by_keyboard_and_mirrors_expanded`: Space and
    /// Enter toggle the focused header, Left closes it and Right opens it.
    #[test]
    fn keys_toggle_open_and_close() {
        let mut d = Driver::new(header("Section"), Size::new(200.0, 100.0));
        d.focus(true);
        d.key(Key::Space);
        assert_eq!(d.get("open"), Some(Value::Bool(true)));
        d.key(Key::Enter);
        assert_eq!(d.get("open"), Some(Value::Bool(false)));
        d.key(Key::ArrowRight);
        assert_eq!(d.get("open"), Some(Value::Bool(true)));
        d.key(Key::ArrowLeft);
        assert_eq!(d.get("open"), Some(Value::Bool(false)));
        let args: Vec<_> = d.signals.iter().map(|(_, a)| a.clone()).collect();
        assert_eq!(args, [vec![Value::Bool(true)], vec![Value::Bool(false)], vec![Value::Bool(true)], vec![Value::Bool(false)]]);
        assert!(d.kind.claims_key(Key::ArrowLeft, Default::default()) && d.kind.claims_key(Key::Space, Default::default()));
        d.a11y_action(accesskit::Action::Expand);
        assert_eq!(d.get("open"), Some(Value::Bool(true)));
    }

    #[test]
    fn a11y_names_by_text_and_mirrors_expanded() {
        let mut k = header("Placement");
        let n = crate::controls::testkit::a11y(&k, StateBits::NONE, Size::new(200.0, 22.0));
        assert_eq!(n.role(), accesskit::Role::DisclosureTriangle);
        assert_eq!(n.label(), Some("Placement"));
        assert_eq!(n.is_expanded(), Some(false));
        set(&mut k, "open", Value::Bool(true));
        assert_eq!(crate::controls::testkit::a11y(&k, StateBits::NONE, Size::new(200.0, 22.0)).is_expanded(), Some(true));
    }

    #[test]
    fn closed_hides_the_body_and_open_indents_it_below_the_header() {
        let mut k = header("Placement");
        let row = chrome(&k)[2];
        assert!(row >= 22.0, "the header row (header-h): {row}");
        let mut s = Style::default();
        assert!(!k.child_layout(&ChildCx { index: 0, anchors: &["", ""], slots: &["", "end"], children: &[] }, &mut s));
        assert_eq!(s.display, Display::None, "closed: not laid out");
        let mut e = Style::default();
        assert!(k.child_layout(&ChildCx { index: 1, anchors: &["", ""], slots: &["", "end"], children: &[] }, &mut e), "header actions show closed");
        assert_eq!(e.position, Position::Absolute);
        // the end slot fills the header row (as measured by the last chrome_padding), or is centred
        // in it when its height is set
        assert_eq!(e.size.height, taffy::Dimension::length(row));
        assert_eq!(e.inset.top, LengthPercentageAuto::length(0.0));
        let mut fixed = Style { size: taffy::Size { width: taffy::Dimension::auto(), height: taffy::Dimension::length(row - 6.0) }, ..Style::default() };
        k.child_layout(&ChildCx { index: 1, anchors: &["", ""], slots: &["", "end"], children: &[] }, &mut fixed);
        assert_eq!(fixed.inset.top, LengthPercentageAuto::length(3.0));
        // the header is the end slot's containing block
        assert_eq!(k.layout_style(&StyleCx { computed: None, state: StateBits::NONE, dark: true, base: Some(Style { position: Position::Static, ..Style::default() }) }).position, Position::Relative);
        set(&mut k, "open", Value::Bool(true));
        let mut s = Style::default();
        assert!(k.child_layout(&ChildCx { index: 0, anchors: &["", ""], slots: &["", "end"], children: &[] }, &mut s));
        assert_ne!(s.display, Display::None);
        let [l, _, t, _] = chrome(&k);
        assert_eq!(l, 16.0 + 4.0, "indented past the twistie and the gap");
        assert_eq!(t, row + 2.0, "below the header row and the host's gap");
    }

    /// A9c: a content-sized header (in a row, as the Properties panel and TourPlayer lay it out)
    /// keeps its header row: the chevron and the whole caption, not "> …".
    #[test]
    fn a_content_sized_header_keeps_its_chevron_and_caption() {
        use crate::testdoc::{child, doc, frame};
        let (mut d, root) = doc(Size::new(300.0, 200.0), |r| {
            crate::host::register::<CollapsingHeader>(r).unwrap();
        });
        crate::host::install(&mut d, skelter_text::TextSystem::new(skelter_text::FontStore::new(skelter_text::FontConfig::testing())), crate::host::os_policy(), crate::theme::Theme::dark_modern());
        d.set_style(root, skelter_core::StyleInput { inline: Some("flex-direction: row; align-items: flex-start".into()), ..Default::default() });
        let h = child(&mut d, root, skelter_core::KindId::of("collapsing-header"), "");
        let text = d.registry().get(skelter_core::KindId::of("collapsing-header")).unwrap().spec.prop_named("text").unwrap();
        d.set_prop(h, text, skelter_core::Value::from("Placement")).unwrap();
        let c = frame(&mut d).unwrap();
        let r = d.layout_rect(h).unwrap();
        assert!(r.width() > 16.0 + 4.0 + 40.0 && r.height() >= 16.0, "the header row's size: {r:?}");
        let (_, list) = c.lists.iter().find(|(k, _)| *k == h).expect("it records");
        let lines = list.items.iter().filter(|p| matches!(p, Prim::Line(_))).count();
        assert_eq!(lines, 2, "the chevron");
        let glyphs = list.items.iter().find_map(|p| if let Prim::Glyphs(g) = p { c.runs.iter().find(|(id, _)| *id == g.run).map(|(_, run)| run.glyphs.len()) } else { None });
        assert_eq!(glyphs, Some("Placement".len()), "the whole caption, not elided");
    }

    #[test]
    fn records_header_chevron_and_caption() {
        let k = header("Placement");
        let (list, _) = crate::controls::testkit::record(&k, Size::new(200.0, 22.0), StateBits::HOVERED);
        assert_eq!(crate::controls::testkit::kinds(&list), ["quad", "line", "line", "glyphs"]);
    }
}

