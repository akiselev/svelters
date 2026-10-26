//! `sides` (ported from svelters-egui `kinds/panes.rs` and `elements/panes.rs`, egui's `Sides`): a
//! flex row with its first child at the start and its second at the end (`justify-content:
//! space-between`; wrap several items in a `box`). With more children the free space is shared
//! between them. It paints its own CSS box.
//!
//! The row and the space-between are the kind's own and always apply; `gap`, `padding` and
//! `align-items` follow the box props' precedence ([`Layout`]).

use crate::controls::box_::{ALIGN, Explicit, Layout};
use crate::kind_props;
use crate::paint::{self, PaintCx, Ua};
use crate::shim::core::{A11yCx, Kind, KindSpec, NameFrom, PropError, PropId, RecordCx, StyleCx, Value};
use skelter_base::accesskit;
use taffy::{AlignContent, Display, FlexDirection, Style};

kind_props! {
    /// The sides' props.
    pub struct SidesProps;
    /// Minimum space between the two sides, in points.
    gap "gap": f64 = F64(0.0) ; F64, Layout;
    /// Inner spacing on all four sides, in points.
    padding "padding": f64 = F64(0.0) ; F64, Layout;
    /// Cross-axis alignment of the two sides (`center` is egui's look).
    align_items "align-items": String = Enum("stretch") ; Enum(ALIGN), Layout;
}

pub static SPEC: KindSpec = KindSpec { tag: "sides", props: SidesProps::SPECS, signals: &[], slots: &[], children: true, role: accesskit::Role::GenericContainer, name_from: NameFrom::None, text_prop: None, focusable: false };

/// A row with its children pushed to the two ends.
#[derive(Debug, Default)]
pub struct Sides {
    pub props: SidesProps,
    explicit: Explicit,
}

impl Kind for Sides {
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
        l.display(&mut s, Display::Flex);
        l.gaps(&mut s, (P::gap.id(), self.props.gap), None, None);
        l.padding(&mut s, P::padding.id(), self.props.padding);
        l.align_items(&mut s, P::align_items.id(), &self.props.align_items);
        s.flex_direction = FlexDirection::Row;
        s.justify_content = AlignContent::SPACE_BETWEEN;
        s
    }

    fn record(&self, cx: &mut RecordCx) {
        let host = crate::controls::common::host_view(&cx.style, Ua::NONE);
        let rect = cx.rect();
        let mut pc = PaintCx::of(cx);
        paint::paint_box(&mut pc, &host, rect);
        paint::paint_outline(&mut pc, &host, rect);
    }

    fn a11y(&self, cx: &mut A11yCx) {
        cx.node.set_role(accesskit::Role::GenericContainer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::testkit;
    use crate::interact::StateBits;
    use skelter_base::change::ChangeClass;
    use skelter_base::geom::Size;
    use taffy::{AlignItems, LengthPercentage};

    fn set(k: &mut Sides, name: &str, v: Value) {
        let id = SPEC.prop(name).unwrap();
        k.set_prop(id, &v).unwrap();
    }

    #[test]
    fn spec() {
        let s = Sides::spec();
        assert_eq!(s.tag, "sides");
        assert!(s.children && s.slots.is_empty() && s.signals.is_empty());
        assert_eq!(s.props.iter().map(|p| p.name).collect::<Vec<_>>(), ["gap", "padding", "align-items"]);
        assert!(s.props.iter().all(|p| p.change == ChangeClass::Layout));
        let mut k = Sides::default();
        assert!(k.set_prop(s.prop("align-items").unwrap(), &Value::str("middle")).is_err());
    }

    #[test]
    fn layout_style_is_a_space_between_row() {
        let mut k = Sides::default();
        let s = testkit::layout_style(&k, None, StateBits::NONE);
        assert_eq!(s.display, Display::Flex);
        assert_eq!(s.flex_direction, FlexDirection::Row);
        assert_eq!(s.justify_content, AlignContent::SPACE_BETWEEN);
        assert_eq!(s.align_items, AlignItems::STRETCH);
        set(&mut k, "gap", Value::F64(12.0));
        set(&mut k, "padding", Value::F64(4.0));
        set(&mut k, "align-items", Value::str("center"));
        let s = testkit::layout_style(&k, None, StateBits::NONE);
        assert_eq!(s.gap.width, LengthPercentage::length(12.0));
        assert_eq!(s.gap.height, LengthPercentage::length(12.0));
        assert_eq!(s.padding, taffy::Rect::length(4.0));
        assert_eq!(s.align_items, AlignItems::CENTER);
        // CSS cannot turn it into a column: the row is the kind's
        let css = testkit::css("flex-direction: column; justify-content: start");
        let s = testkit::layout_style(&k, Some(&css), StateBits::NONE);
        assert_eq!(s.flex_direction, FlexDirection::Row);
        assert_eq!(s.justify_content, AlignContent::SPACE_BETWEEN);
    }

    #[test]
    fn records_its_css_box() {
        let k = Sides::default();
        let size = Size::new(200.0, 24.0);
        assert!(testkit::record(&k, size, StateBits::NONE).0.items.is_empty());
        let css = testkit::css("background-color: #202020");
        assert_eq!(testkit::kinds(&testkit::record_css(&k, size, StateBits::NONE, &css).0), ["quad"]);
    }
}
