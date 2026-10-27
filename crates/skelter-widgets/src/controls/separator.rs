//! `separator` (ARCH-WIDGETS §6.1, ported from svelters-egui `widgets/separator.rs`): box. A
//! 1 px `border` hairline centred across the node, filling it along `orientation` (DESIGN §2.3:
//! separators stay flat). The line is the node's `border-color` and `border-width` (CSS), else the
//! UA's; its margin is the node's padding.

use crate::kind_props;
use crate::paint::{PaintCx, StyleView, Ua, UaRule, UaValue::*};
use crate::shim::core::{A11yCx, Kind, KindSpec, MeasureCx, NameFrom, PropError, PropId, RecordCx, StyleCx, Value};
use skelter_base::accesskit;
use skelter_base::color::{Color, Fill};
use skelter_base::display::{BorderStyle, Prim, Quad};
use skelter_base::geom::{Corners, Insets, Rect, Vec2};

pub const UA: Ua = Ua(&[UaRule::base(&[("border-color", Token("border")), ("border-width", Token("stroke")), ("padding", Px(3.0))])]);

kind_props! {
    /// The separator's props.
    pub struct SeparatorProps;
    /// `horizontal` (a line across, the default) or `vertical`.
    orientation "orientation": String = Enum("horizontal") ; Enum(&["horizontal", "vertical"]), Layout;
}

/// No AccessKit node of its own: a separator is decoration, as egui's was (an unnamed `Splitter`
/// would be announced as a resize handle). The spec's role is a hidden generic container.
pub static SPEC: KindSpec = KindSpec { tag: "separator", props: SeparatorProps::SPECS, signals: &[], slots: &[], children: false, role: accesskit::Role::GenericContainer, name_from: NameFrom::None, text_prop: None, focusable: false };

/// Its thickness across under `s` (the length along is the layout's).
pub fn measure(s: &StyleView, vertical: bool) -> Vec2 {
    let [l, r, t, b] = s.sides("padding");
    let w = s.sides("border-width")[2].max(1.0);
    if vertical { Vec2::new(l + w + r, 0.0) } else { Vec2::new(0.0, t + w + b) }
}

/// Paints the rule centred in `r` under `s`: a filled quad on the pixel grid.
pub fn paint(pc: &mut PaintCx, s: &StyleView, r: Rect, vertical: bool) {
    let w = pc.snap_width(s.sides("border-width")[2].max(1.0));
    let color = s.color("border-top-color").or_else(|| s.color("border-color")).unwrap_or(Color::TRANSPARENT);
    let (cx, cy) = ((r.min.x + r.max.x) / 2.0, (r.min.y + r.max.y) / 2.0);
    let line = if vertical { Rect::new(cx - w / 2.0, r.min.y, cx + w / 2.0, r.max.y) } else { Rect::new(r.min.x, cy - w / 2.0, r.max.x, cy + w / 2.0) };
    let line = pc.snap_rect(line);
    if color.a <= 0.0 || line.is_empty() {
        return;
    }
    let q = Quad { rect: line, radii: Corners::ZERO, fill: Fill::Solid(color), border_widths: Insets::ZERO, border_color: Color::TRANSPARENT, border_style: BorderStyle::Solid };
    pc.push(Prim::Quad(q), line);
}

/// A horizontal or vertical rule.
#[derive(Debug, Default)]
pub struct Separator {
    pub props: SeparatorProps,
}

impl Separator {
    fn vertical(&self) -> bool {
        self.props.orientation == "vertical"
    }
}

impl Kind for Separator {
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

    /// It stretches across its parent's cross axis unless CSS sets `align-self`: it measures no
    /// length of its own, so a vertical rule in a row that centres its items (a toolbar) would
    /// otherwise be zero tall.
    fn layout_style(&self, cx: &StyleCx) -> taffy::Style {
        let mut s = cx.default_layout();
        if s.align_self.is_none() {
            s.align_self = Some(taffy::AlignSelf::STRETCH);
        }
        s
    }

    fn measure(&self, cx: &mut MeasureCx, _known: taffy::Size<Option<f32>>, _avail: taffy::Size<taffy::AvailableSpace>) -> taffy::Size<f32> {
        let s = measure(&crate::controls::common::host_view(&cx.style, UA), self.vertical());
        taffy::Size { width: s.x, height: s.y }
    }

    fn record(&self, cx: &mut RecordCx) {
        let s = crate::controls::common::host_view(&cx.style, UA);
        let rect = cx.rect();
        paint(&mut PaintCx::of(cx), &s, rect, self.vertical());
    }

    fn a11y(&self, cx: &mut A11yCx) {
        cx.node.set_role(accesskit::Role::GenericContainer);
        cx.node.set_hidden();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::testkit::{self, Driver};
    use crate::interact::StateBits;
    use crate::shim::core::{Event, Handled};
    use skelter_base::change::ChangeClass;
    use skelter_base::geom::Size;

    fn stroke() -> f32 {
        StyleView::ua_only(UA, StateBits::NONE, true).sides("border-width")[2].max(1.0)
    }

    #[test]
    fn spec() {
        let s = Separator::spec();
        assert_eq!((s.tag, s.focusable, s.signals.len()), ("separator", false, 0));
        let o = s.prop("orientation").unwrap();
        assert_eq!(s.prop_spec(o).unwrap().change, ChangeClass::Layout);
        let mut sep = Separator::default();
        assert!(sep.set_prop(o, &Value::str("diagonal")).is_err());
        sep.set_prop(o, &Value::str("vertical")).unwrap();
        assert!(sep.vertical());
    }

    #[test]
    fn measures_its_thickness_across() {
        let mut s = Separator::default();
        assert_eq!(testkit::measure(&s, StateBits::NONE), Size::new(0.0, 6.0 + stroke()), "3px padding each side");
        s.props.orientation = "vertical".into();
        assert_eq!(testkit::measure(&s, StateBits::NONE), Size::new(6.0 + stroke(), 0.0));
    }

    #[test]
    fn stretches_across_its_row_unless_css_aligns_it() {
        let mut s = Separator::default();
        s.props.orientation = "vertical".into();
        let st = s.layout_style(&StyleCx { computed: None, state: StateBits::NONE, dark: true, base: None });
        assert_eq!(st.align_self, Some(taffy::AlignSelf::STRETCH), "a toolbar row that centres its items still gives it a length");
        let base = taffy::Style { align_self: Some(taffy::AlignSelf::CENTER), ..Default::default() };
        let st = s.layout_style(&StyleCx { computed: None, state: StateBits::NONE, dark: true, base: Some(base) });
        assert_eq!(st.align_self, Some(taffy::AlignSelf::CENTER), "CSS align-self wins");
    }

    #[test]
    fn records_a_centred_hairline() {
        let mut s = Separator::default();
        let (list, _) = testkit::record(&s, Size::new(100.0, 7.0), StateBits::NONE);
        assert_eq!(testkit::kinds(&list), ["quad"]);
        let Prim::Quad(q) = &list.items[0] else { unreachable!() };
        let w = stroke();
        let y0 = (3.5 - w / 2.0).round();
        assert_eq!(q.rect, Rect::new(0.0, y0, 100.0, (3.5 + w / 2.0).round()));
        assert_eq!(q.fill, Fill::Solid(crate::paint::ua_token(true, "border").unwrap()));
        s.props.orientation = "vertical".into();
        let (list, _) = testkit::record(&s, Size::new(7.0, 40.0), StateBits::NONE);
        let Prim::Quad(q) = &list.items[0] else { unreachable!() };
        assert_eq!((q.rect.min.y, q.rect.max.y), (0.0, 40.0));
        assert_eq!((q.rect.min.x, q.rect.max.x), ((3.5 - w / 2.0).round(), (3.5 + w / 2.0).round()));
    }

    #[test]
    fn takes_no_input_and_is_hidden_to_assistive_technology() {
        let mut d = Driver::new(Separator::default(), Size::new(100.0, 7.0));
        assert_eq!(d.send(Event::Hover(true)), Handled::No);
        assert!(d.signals.is_empty());
        let n = testkit::a11y(&Separator::default(), StateBits::NONE, Size::new(100.0, 7.0));
        assert!(n.is_hidden());
    }
}
