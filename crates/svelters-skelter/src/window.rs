//! The `<window>` node's kind: the window's content area. Its spec is the synthesized one (the
//! surface's props, signals and methods, [`crate::kinds`]); this value lays the children out as the
//! surface says: a flex column inside `padding`, `gap` apart, as written (POL2-LAYOUT 5; an unwritten `padding` is 0, not the
//! surface's 8, so existing windows keep their layout). A CSS `padding` or
//! `gap` on the window still wins, as on `box`. It draws nothing: the theme's background is the
//! platform window's clear colour.

use skelter_core as core;

/// The node value of a `<window>`.
#[derive(Default)]
pub struct Window;

static SPEC: core::KindSpec = core::KindSpec::new("window");

impl core::Kind for Window {
    fn spec() -> &'static core::KindSpec {
        &SPEC
    }

    fn layout_style(&self, cx: &core::StyleCx) -> taffy::Style {
        let mut s = cx.default_layout();
        let spec = cx.kind().spec;
        // only a written prop: the surface's default `padding` (8, egui's) would move every existing
        // window's content (an app's root, the gallery's harness window)
        let num = |name: &str| spec.prop_named(name).and_then(|p| cx.prop_written(p)).and_then(skelter_core::Value::as_f64).map(|v| v.max(0.0) as f32);
        let css = |name: &str| cx.css().is_some_and(|c| c.get(name).is_some());
        if !["padding", "padding-top", "padding-right", "padding-bottom", "padding-left"].iter().any(|n| css(n))
            && let Some(p) = num("padding")
        {
            let l = taffy::LengthPercentage::length(p);
            s.padding = taffy::Rect { left: l, right: l, top: l, bottom: l };
        }
        if !["gap", "row-gap", "column-gap"].iter().any(|n| css(n))
            && let Some(g) = num("gap")
        {
            let l = taffy::LengthPercentage::length(g);
            s.gap = taffy::Size { width: l, height: l };
        }
        s
    }

    fn record(&self, _cx: &mut core::RecordCx) {}
}

fn window() -> Box<dyn core::Kind> {
    Box::new(Window)
}

/// The node value a synthesized kind gets instead of [`crate::kinds::Placeholder`] (`None`: the
/// placeholder): its spec stays the surface's, only its layout and look are written here.
pub fn factory(tag: &str) -> Option<fn() -> Box<dyn core::Kind>> {
    match tag {
        "window" => Some(window),
        _ => None,
    }
}
