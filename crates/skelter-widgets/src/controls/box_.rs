//! `box` (EGUI.md §5, ported from svelters-egui `kinds/containers.rs` and `layout.rs` `style_of`): a
//! flexbox container laid out by taffy. It paints its own CSS box (background, border, shadows,
//! outline) and mirrors `:hover` (a Hoverable on every box, so `box:hover` rules apply).
//!
//! The box props (`flex-direction`, `flex-wrap`, `gap`, `row-gap`, `column-gap`, `padding`,
//! `align-items`, `justify-content`) are the CSS properties of the same names as props
//! (svelters-style's setters). Precedence is CSS.md §2.8's: a prop the runtime set wins over every
//! rule, then the computed style, then the prop's default (the native default: a box is a column).
//! The other containers (`frame`, `sides`, `grid`, `stack`) use the same rule through [`Layout`].

use crate::controls::common::Units;
use crate::interact::{Hoverable, Intents, Sense, Unit};
use crate::kind_props;
use crate::paint::{self, PaintCx, Ua};
use crate::shim::core::{A11yCx, Event, EventCx, Handled, Kind, KindSpec, NameFrom, PropError, PropId, RecordCx, StyleCx, Value};
use skelter_base::accesskit;
use taffy::{AlignContent, AlignItems, Display, FlexDirection, FlexWrap, LengthPercentage, Style};

pub const ALIGN: &[&str] = &["start", "end", "center", "stretch", "baseline"];
pub const JUSTIFY: &[&str] = &["start", "end", "center", "space-between", "space-around", "space-evenly"];
pub const DIRECTION: &[&str] = &["column", "row", "column-reverse", "row-reverse"];
pub const WRAP: &[&str] = &["nowrap", "wrap", "wrap-reverse"];

// ---- the shared layout rule ----

/// The props the runtime set explicitly, by spec index: they win over every CSS rule.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Explicit(u64);

impl Explicit {
    pub fn mark(&mut self, id: PropId) {
        if id.0 < 64 {
            self.0 |= 1 << id.0;
        }
    }
    pub fn has(&self, id: PropId) -> bool {
        id.0 < 64 && self.0 & (1 << id.0) != 0
    }
}

/// Applies a container's layout props over the CSS-derived style: each through [`Layout::applies`].
pub struct Layout<'a, 'c> {
    pub cx: &'a StyleCx<'c>,
    pub explicit: Explicit,
}

impl Layout<'_, '_> {
    /// Whether any of the CSS properties `css` has a computed value on the node.
    pub fn css_sets(&self, css: &[&str]) -> bool {
        self.cx.computed.is_some_and(|c| css.iter().any(|n| c.get(n).is_some()))
    }

    /// Whether a prop's value applies: the runtime set one of `props`, or no CSS rule sets any of
    /// `css` (the prop's default is then the native default).
    pub fn applies(&self, props: &[PropId], css: &[&str]) -> bool {
        props.iter().any(|p| self.explicit.has(*p)) || !self.css_sets(css)
    }

    /// `display`: `d` unless CSS sets `display` or the node is `display: none`.
    pub fn display(&self, s: &mut Style, d: Display) {
        if s.display != Display::None && !self.css_sets(&["display"]) {
            s.display = d;
        }
    }

    pub fn direction(&self, s: &mut Style, prop: PropId, v: &str) {
        if self.applies(&[prop], &["flex-direction"]) {
            s.flex_direction = direction(v);
        }
    }

    pub fn wrap(&self, s: &mut Style, prop: PropId, v: &str) {
        if self.applies(&[prop], &["flex-wrap"]) {
            s.flex_wrap = match v {
                "wrap" => FlexWrap::Wrap,
                "wrap-reverse" => FlexWrap::WrapReverse,
                _ => FlexWrap::NoWrap,
            };
        }
    }

    /// `gap` with its per-axis overrides (`row-gap`, `column-gap`; `None`: `gap`). CSS `gap` is
    /// `row-gap column-gap`; taffy's `gap.width` is the column gap.
    pub fn gaps(&self, s: &mut Style, (gap_id, gap): (PropId, f64), row: Option<(PropId, Option<f64>)>, column: Option<(PropId, Option<f64>)>) {
        let row_ids: Vec<PropId> = std::iter::once(gap_id).chain(row.map(|r| r.0)).collect();
        let col_ids: Vec<PropId> = std::iter::once(gap_id).chain(column.map(|c| c.0)).collect();
        if self.applies(&col_ids, &["gap", "column-gap"]) {
            s.gap.width = LengthPercentage::length(column.and_then(|c| c.1).unwrap_or(gap) as f32);
        }
        if self.applies(&row_ids, &["gap", "row-gap"]) {
            s.gap.height = LengthPercentage::length(row.and_then(|r| r.1).unwrap_or(gap) as f32);
        }
    }

    /// `padding` on all four sides.
    pub fn padding(&self, s: &mut Style, prop: PropId, v: f64) {
        if self.applies(&[prop], &["padding", "padding-left", "padding-right", "padding-top", "padding-bottom"]) {
            s.padding = taffy::Rect::length(v as f32);
        }
    }

    pub fn align_items(&self, s: &mut Style, prop: PropId, v: &str) {
        if self.applies(&[prop], &["align-items"]) {
            s.align_items = align_items(v);
        }
    }

    pub fn justify_content(&self, s: &mut Style, prop: PropId, v: &str) {
        if self.applies(&[prop], &["justify-content"]) {
            s.justify_content = justify_content(v);
        }
    }
}

pub fn direction(v: &str) -> FlexDirection {
    match v {
        "row" => FlexDirection::Row,
        "row-reverse" => FlexDirection::RowReverse,
        "column-reverse" => FlexDirection::ColumnReverse,
        _ => FlexDirection::Column,
    }
}

/// An [`ALIGN`] name as taffy's (`stretch` for anything else, as the source).
pub fn align_items(v: &str) -> AlignItems {
    match v {
        "start" => AlignItems::START,
        "end" => AlignItems::END,
        "center" => AlignItems::CENTER,
        "baseline" => AlignItems::BASELINE,
        _ => AlignItems::STRETCH,
    }
}

/// A [`JUSTIFY`] name as taffy's (`start` for anything else).
pub fn justify_content(v: &str) -> AlignContent {
    match v {
        "end" => AlignContent::END,
        "center" => AlignContent::CENTER,
        "space-between" => AlignContent::SPACE_BETWEEN,
        "space-around" => AlignContent::SPACE_AROUND,
        "space-evenly" => AlignContent::SPACE_EVENLY,
        _ => AlignContent::START,
    }
}

// ---- the kind ----

kind_props! {
    /// The box's props.
    pub struct BoxProps;
    /// Main axis of the box.
    flex_direction "flex-direction": String = Enum("column") ; Enum(DIRECTION), Layout;
    /// Whether children wrap onto more lines.
    flex_wrap "flex-wrap": String = Enum("nowrap") ; Enum(WRAP), Layout;
    /// Space between children on both axes, in points (row-gap and column-gap override it).
    gap "gap": f64 = F64(0.0) ; F64, Layout;
    /// Space between rows in points; null to use `gap`.
    row_gap "row-gap": Option<f64> = None ; F64, Layout;
    /// Space between columns in points; null to use `gap`.
    column_gap "column-gap": Option<f64> = None ; F64, Layout;
    /// Inner spacing on all four sides, in points.
    padding "padding": f64 = F64(0.0) ; F64, Layout;
    /// Cross-axis alignment of the children.
    align_items "align-items": String = Enum("stretch") ; Enum(ALIGN), Layout;
    /// Main-axis distribution of the children.
    justify_content "justify-content": String = Enum("start") ; Enum(JUSTIFY), Layout;
}

pub static SPEC: KindSpec = KindSpec { tag: "box", props: BoxProps::SPECS, signals: &[], slots: &[], children: true, role: accesskit::Role::GenericContainer, name_from: NameFrom::None, text_prop: None, focusable: false };

impl BoxProps {
    /// Applies the box props over `s` (the flex container part of `style_of`).
    pub fn apply(&self, l: &Layout, s: &mut Style) {
        l.display(s, Display::Flex);
        l.direction(s, P::flex_direction.id(), &self.flex_direction);
        l.wrap(s, P::flex_wrap.id(), &self.flex_wrap);
        l.gaps(s, (P::gap.id(), self.gap), Some((P::row_gap.id(), self.row_gap)), Some((P::column_gap.id(), self.column_gap)));
        l.padding(s, P::padding.id(), self.padding);
        l.align_items(s, P::align_items.id(), &self.align_items);
        l.justify_content(s, P::justify_content.id(), &self.justify_content);
    }
}

/// A flexbox container (the `box` kind).
#[derive(Debug, Default)]
pub struct FlexBox {
    pub props: BoxProps,
    explicit: Explicit,
    units: Units,
    hover: Hoverable,
}

impl Kind for FlexBox {
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
        self.props.apply(&Layout { cx, explicit: self.explicit }, &mut s);
        s
    }

    fn record(&self, cx: &mut RecordCx) {
        let host = crate::controls::common::host_view(&cx.style, Ua::NONE);
        let rect = cx.rect();
        let mut pc = PaintCx::of(cx);
        paint::paint_box(&mut pc, &host, rect);
        paint::paint_outline(&mut pc, &host, rect);
    }

    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        let hover = &mut self.hover;
        self.units.run(cx, ev, Sense::HOVER, |icx| hover.input(&(), icx, &mut Intents::default()));
        Handled::No
    }

    fn a11y(&self, cx: &mut A11yCx) {
        cx.node.set_role(accesskit::Role::GenericContainer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::testkit::{self, Driver};
    use crate::interact::StateBits;
    use skelter_base::change::ChangeClass;
    use skelter_base::display::Prim;
    use skelter_base::geom::Size;

    fn set(b: &mut FlexBox, name: &str, v: Value) {
        let id = SPEC.prop(name).unwrap();
        b.set_prop(id, &v).unwrap();
    }

    #[test]
    fn spec() {
        let s = FlexBox::spec();
        assert_eq!(s.tag, "box");
        assert!(s.children && s.signals.is_empty());
        assert_eq!(s.role, accesskit::Role::GenericContainer);
        for p in s.props {
            assert_eq!(p.change, ChangeClass::Layout, "{}", p.name);
        }
        assert_eq!(s.prop_spec(s.prop("flex-direction").unwrap()).unwrap().default, crate::shim::core::Default::Enum("column"));
        let mut b = FlexBox::default();
        assert!(b.set_prop(s.prop("flex-direction").unwrap(), &Value::str("diagonal")).is_err(), "not a direction");
        assert!(b.set_prop(s.prop("gap").unwrap(), &Value::str("8")).is_err(), "type-checked");
        // a nullable gap: unset with an empty list
        set(&mut b, "row-gap", Value::F64(3.0));
        assert_eq!(b.props.row_gap, Some(3.0));
        set(&mut b, "row-gap", Value::List(Vec::new()));
        assert_eq!(b.props.row_gap, None);
    }

    #[test]
    fn layout_style_defaults_to_a_flex_column() {
        let b = FlexBox::default();
        let s = testkit::layout_style(&b, None, StateBits::NONE);
        assert_eq!(s.display, Display::Flex);
        assert_eq!(s.flex_direction, FlexDirection::Column);
        assert_eq!(s.flex_wrap, FlexWrap::NoWrap);
        assert_eq!(s.align_items, AlignItems::STRETCH);
        assert_eq!(s.justify_content, AlignContent::START);
        assert_eq!(s.gap.width, LengthPercentage::length(0.0));
        assert_eq!(s.padding.left, LengthPercentage::length(0.0));
    }

    #[test]
    fn layout_style_from_props() {
        let mut b = FlexBox::default();
        set(&mut b, "flex-direction", Value::str("row-reverse"));
        set(&mut b, "flex-wrap", Value::Enum("wrap"));
        set(&mut b, "gap", Value::F64(8.0));
        set(&mut b, "column-gap", Value::F64(2.0));
        set(&mut b, "padding", Value::I64(5));
        set(&mut b, "align-items", Value::str("center"));
        set(&mut b, "justify-content", Value::str("space-between"));
        let s = testkit::layout_style(&b, None, StateBits::NONE);
        assert_eq!(s.flex_direction, FlexDirection::RowReverse);
        assert_eq!(s.flex_wrap, FlexWrap::Wrap);
        // taffy's width is the column gap, height the row gap (`row-gap` unset: `gap`)
        assert_eq!(s.gap.width, LengthPercentage::length(2.0));
        assert_eq!(s.gap.height, LengthPercentage::length(8.0));
        assert_eq!(s.padding, taffy::Rect::length(5.0));
        assert_eq!(s.align_items, AlignItems::CENTER);
        assert_eq!(s.justify_content, AlignContent::SPACE_BETWEEN);
    }

    #[test]
    fn css_beats_defaults_and_explicit_props_beat_css() {
        let css = testkit::css("flex-direction: row; gap: 4px");
        let mut b = FlexBox::default();
        let s = testkit::layout_style(&b, Some(&css), StateBits::NONE);
        // the CSS rule decides over the prop default `column` (the base skelter-core maps keeps it)
        assert_ne!(s.flex_direction, FlexDirection::Column);
        set(&mut b, "flex-direction", Value::str("column"));
        set(&mut b, "gap", Value::F64(10.0));
        let s = testkit::layout_style(&b, Some(&css), StateBits::NONE);
        assert_eq!(s.flex_direction, FlexDirection::Column, "a prop the runtime set wins");
        assert_eq!(s.gap.height, LengthPercentage::length(10.0));
    }

    #[test]
    fn records_its_css_box() {
        let b = FlexBox::default();
        let size = Size::new(100.0, 40.0);
        assert!(testkit::record(&b, size, StateBits::NONE).0.items.is_empty(), "no CSS: nothing to paint");
        let css = testkit::css("background-color: #336699; border-width: 2px; border-color: #ff0000; border-radius: 6px; outline-style: solid; outline-width: 1px; outline-color: #00ff00");
        let (list, _) = testkit::record_css(&b, size, StateBits::NONE, &css);
        assert_eq!(testkit::kinds(&list), ["quad", "quad"]);
        let Prim::Quad(q) = &list.items[0] else { unreachable!() };
        let v = paint::StyleView::new(Some(&css), Ua::NONE, StateBits::NONE, true);
        assert_eq!(q.rect, skelter_base::geom::Rect::new(0.0, 0.0, 100.0, 40.0));
        assert_eq!(q.radii.tl, 6.0);
        assert_eq!(q.border_widths.left, 2.0);
        assert_eq!(q.border_color, v.border_sides()[0].1);
        assert!(q.border_color.a > 0.0);
        assert_eq!(q.fill, skelter_base::color::Fill::Solid(v.color("background-color").unwrap()));
        // the outline: outside the box by its width, a border only
        let Prim::Quad(o) = &list.items[1] else { unreachable!() };
        assert_eq!(o.rect, skelter_base::geom::Rect::new(-1.0, -1.0, 101.0, 41.0));
        assert_eq!(o.border_widths.top, 1.0);
        assert_eq!(o.border_color, v.color("outline-color").unwrap());
    }

    #[test]
    fn mirrors_hover() {
        let mut d = Driver::new(FlexBox::default(), Size::new(50.0, 50.0));
        d.hover(true);
        assert!(d.state.contains(StateBits::HOVERED));
        assert_eq!(d.handled.last(), Some(&Handled::No), "a box has no native default");
        d.hover(false);
        assert!(!d.state.contains(StateBits::HOVERED));
        d.click();
        assert!(d.take().is_empty());
    }

    #[test]
    fn a11y_node() {
        let n = testkit::a11y(&FlexBox::default(), StateBits::NONE, Size::new(10.0, 10.0));
        assert_eq!(n.role(), accesskit::Role::GenericContainer);
    }
}
