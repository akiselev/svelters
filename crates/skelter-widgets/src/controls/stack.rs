//! `stack` (ported from svelters-egui `kinds/panes.rs` and `elements/panes.rs` `stack_layout`):
//! shows one of its children (GTK's `stack`, Qt's `stacked-widget`): the one whose `anchor` is
//! `visible-child-name`, else the one at `visible-index`. The others are neither drawn nor hit. It
//! paints its own CSS box.
//!
//! Layout: a one-cell taffy grid (one `1fr` column, one `1fr` row) with every child placed in the
//! cell ([`Kind::child_layout`]), so the visible child fills the stack's content box. `homogeneous`
//! keeps the hidden children in the cell, so the stack is as large as its largest child; otherwise
//! they get `display: none` and the stack sizes to the visible child. `padding` follows the box
//! props' precedence ([`Layout`]).

use crate::controls::box_::{Explicit, Layout};
use crate::kind_props;
use crate::paint::{self, PaintCx, Ua};
use crate::shim::core::{A11yCx, ChildCx, Kind, KindSpec, NameFrom, PropError, PropId, RecordCx, StyleCx, Value};
use skelter_base::accesskit;
use taffy::style_helpers::{fr, line};
use taffy::{Display, GridTemplateComponent, Style};

kind_props! {
    /// The stack's props.
    pub struct StackProps;
    /// Shows the child whose `anchor` is this name (GTK's `visible-child-name`; the child names
    /// itself with `anchor`). Empty: `visible-index` decides.
    visible_child_name "visible-child-name": String = Str("") ; Str, Layout;
    /// Shows the child at this index when `visible-child-name` is empty or names no child (Qt's
    /// `current-index`).
    visible_index "visible-index": i64 = I64(0) ; I64, Layout;
    /// Sized to the largest child (GTK's `hhomogeneous`/`vhomogeneous`); false: to the visible child.
    homogeneous "homogeneous": bool = Bool(true) ; Bool, Layout;
    /// Inner spacing on all four sides, in points.
    padding "padding": f64 = F64(0.0) ; F64, Layout;
}

pub static SPEC: KindSpec = KindSpec { tag: "stack", props: StackProps::SPECS, signals: &[], slots: &[], children: true, role: accesskit::Role::GenericContainer, name_from: NameFrom::None, text_prop: None, focusable: false };

/// Shows one child at a time.
#[derive(Debug, Default)]
pub struct Stack {
    pub props: StackProps,
    explicit: Explicit,
}

impl Stack {
    /// The index of the child it shows among children anchored `anchors`: the first anchored
    /// `visible-child-name`, else `visible-index` when it is in range.
    pub fn visible(&self, anchors: &[&str]) -> Option<usize> {
        let name = &self.props.visible_child_name;
        let by_name = (!name.is_empty()).then(|| anchors.iter().position(|a| a == name)).flatten();
        by_name.or_else(|| usize::try_from(self.props.visible_index).ok().filter(|k| *k < anchors.len()))
    }
}

impl Kind for Stack {
    fn places_children() -> bool {
        true
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
        if s.display != Display::None {
            s.display = Display::Grid;
        }
        s.grid_template_columns = vec![GridTemplateComponent::Single(fr(1.0))];
        s.grid_template_rows = vec![GridTemplateComponent::Single(fr(1.0))];
        l.padding(&mut s, P::padding.id(), self.props.padding);
        s
    }

    fn child_layout(&self, cx: &ChildCx, style: &mut Style) -> bool {
        style.grid_row = taffy::Line { start: line(1), end: line(2) };
        style.grid_column = taffy::Line { start: line(1), end: line(2) };
        let shown = self.visible(cx.anchors) == Some(cx.index);
        if !shown && !self.props.homogeneous {
            style.display = Display::None;
        }
        shown
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
    use taffy::LengthPercentage;

    fn set(k: &mut Stack, name: &str, v: Value) {
        let id = SPEC.prop(name).unwrap();
        k.set_prop(id, &v).unwrap();
    }

    /// Runs `child_layout` over children anchored `anchors`: (shown, display) per child.
    fn children(k: &Stack, anchors: &[&str]) -> Vec<(bool, Display)> {
        (0..anchors.len())
            .map(|index| {
                let mut s = Style::default();
                let shown = k.child_layout(&ChildCx { index, anchors, slots: &[], children: &[] }, &mut s);
                assert_eq!(s.grid_row, taffy::Line { start: line(1), end: line(2) });
                assert_eq!(s.grid_column, taffy::Line { start: line(1), end: line(2) });
                (shown, s.display)
            })
            .collect()
    }

    #[test]
    fn spec() {
        let s = Stack::spec();
        assert_eq!(s.tag, "stack");
        assert!(s.children && s.signals.is_empty());
        assert!(s.props.iter().all(|p| p.change == ChangeClass::Layout));
        assert_eq!(s.prop_spec(s.prop("homogeneous").unwrap()).unwrap().default, crate::shim::core::Default::Bool(true));
        let mut k = Stack::default();
        assert!(k.set_prop(s.prop("visible-index").unwrap(), &Value::str("1")).is_err());
    }

    #[test]
    fn picks_by_name_then_index() {
        let mut k = Stack::default();
        let anchors = ["general", "", "advanced"];
        assert_eq!(k.visible(&anchors), Some(0));
        set(&mut k, "visible-index", Value::I64(2));
        assert_eq!(k.visible(&anchors), Some(2));
        set(&mut k, "visible-child-name", Value::str("general"));
        assert_eq!(k.visible(&anchors), Some(0), "the name wins");
        set(&mut k, "visible-child-name", Value::str("missing"));
        assert_eq!(k.visible(&anchors), Some(2), "a name no child has: the index");
        set(&mut k, "visible-index", Value::I64(5));
        assert_eq!(k.visible(&anchors), None, "out of range: nothing shown");
        set(&mut k, "visible-index", Value::I64(-1));
        assert_eq!(k.visible(&anchors), None);
        assert_eq!(k.visible(&[]), None);
    }

    #[test]
    fn layout_style_is_one_cell() {
        let mut k = Stack::default();
        let s = testkit::layout_style(&k, None, StateBits::NONE);
        assert_eq!(s.display, Display::Grid);
        assert_eq!(s.grid_template_columns, vec![GridTemplateComponent::Single(fr(1.0))]);
        assert_eq!(s.grid_template_rows, vec![GridTemplateComponent::Single(fr(1.0))]);
        assert_eq!(s.padding, taffy::Rect::length(0.0));
        set(&mut k, "padding", Value::F64(6.0));
        let s = testkit::layout_style(&k, None, StateBits::NONE);
        assert_eq!(s.padding.top, LengthPercentage::length(6.0));
    }

    #[test]
    fn homogeneous_keeps_hidden_children_in_the_cell() {
        let mut k = Stack::default();
        set(&mut k, "visible-index", Value::I64(1));
        let anchors = ["a", "b", "c"];
        let base = <Style>::default().display;
        assert_eq!(children(&k, &anchors), [(false, base), (true, base), (false, base)]);
        set(&mut k, "homogeneous", Value::Bool(false));
        assert_eq!(children(&k, &anchors), [(false, Display::None), (true, base), (false, Display::None)]);
        set(&mut k, "visible-child-name", Value::str("c"));
        assert_eq!(children(&k, &anchors), [(false, Display::None), (false, Display::None), (true, base)]);
    }

    #[test]
    fn records_its_css_box() {
        let k = Stack::default();
        let size = Size::new(80.0, 80.0);
        assert!(testkit::record(&k, size, StateBits::NONE).0.items.is_empty());
        let css = testkit::css("background-color: #101010");
        assert_eq!(testkit::kinds(&testkit::record_css(&k, size, StateBits::NONE, &css).0), ["quad"]);
    }
}
