//! CS5f: typography and padding from the egui cascade. `font-size`, `font-weight` and
//! `font-family` are used in the measure and paint of text leaves, padding applies to leaves
//! (around the widget, or inside a button's frame), and margin/padding shorthands take 1–4 values.
//! These run headless under kittest with the recording host of `common` and no JS.

mod common;

use common::Rig;
use svelters_abi::*;
use svelters_style::{ComputedValue, Length};

fn s(v: &str) -> Value {
    Value::Str(v.into())
}

fn sheet(rig: &Rig, id: &str, css: &str) {
    rig.br().style(SVL_STYLE_SHEET, 0, Some(&format!("{id}{SVL_STYLE_SEP}{css}"))).unwrap();
}

fn classes(rig: &Rig, h: svl_handle, c: &str) {
    rig.br().style(SVL_STYLE_CLASSES, h, Some(c)).unwrap();
}

fn inline(rig: &Rig, h: svl_handle, decls: &str) {
    rig.br().style(SVL_STYLE_INLINE, h, Some(decls)).unwrap();
}

fn rect(rig: &Rig, h: svl_handle) -> egui::Rect {
    rig.backend.with_scene(|s| s.n(h).rect)
}

fn close(a: f32, b: f32, what: &str) {
    assert!((a - b).abs() < 0.75, "{what}: {a} vs {b}");
}

/// A column box whose children keep their intrinsic widths (`align-items: start`).
fn column(rig: &Rig) -> svl_handle {
    let (_w, col) = rig.window();
    rig.leaf(col, "box", &[("align-items", s("start")), ("gap", Value::F64(4.0))])
}

/// A `font-size` change remeasures and repaints: the label's laid-out rect and the
/// rect its widget painted both grow. The size is inherited from a box. `font-weight` picks the
/// family registered for the nearest weight, and an explicit `font-weight` prop wins over the rule.
/// In `font-family`, an unbound name is skipped and `monospace` is drawn.
#[test]
fn fonts_remeasure_and_repaint() {
    use egui::{FontDefinitions, FontFamily};
    let mut rig = Rig::new();
    // a "cs5f-bold" family drawn with egui's monospace font, registered for 700 of Proportional
    let mut defs = FontDefinitions::default();
    let mono = defs.families[&FontFamily::Monospace].clone();
    defs.families.insert(FontFamily::Name("cs5f-bold".into()), mono);
    rig.harness.ctx.set_fonts(defs);
    svelters_egui::fonts::register_weight(FontFamily::Proportional, 700, FontFamily::Name("cs5f-bold".into()));

    let col = column(&rig);
    let text = "Wide text here";
    let regular = rig.leaf(col, "label", &[("text", s(text))]);
    let sized = rig.leaf(col, "label", &[("text", s(text))]);
    let inner = rig.leaf(col, "box", &[]);
    let inherits = rig.leaf(inner, "label", &[("text", s(text))]);
    let bold = rig.leaf(col, "label", &[("text", s(text))]);
    let pinned = rig.leaf(col, "label", &[("text", s(text)), ("font-weight", Value::I64(400))]);
    let mono = rig.leaf(col, "label", &[("text", s(text))]);
    let button = rig.leaf(col, "button", &[("text", s(text))]);
    sheet(
        &rig,
        "app",
        ".big { font-size: 28px; }
         .bold { font-weight: bold; }
         .mono { font-family: \"Not Installed\", monospace; }",
    );
    rig.run();
    let base = rect(&rig, regular);
    let base_drawn = rig.drawn(regular);
    let base_button = rect(&rig, button);
    assert_eq!(rect(&rig, sized).size(), base.size());

    // font-size: measure (rect) and paint (drawn) both follow
    for h in [sized, inner, button] {
        classes(&rig, h, "big");
    }
    rig.run();
    let r = rect(&rig, sized);
    assert!(r.height() > base.height() * 1.6, "remeasured at 28px: {r:?} vs {base:?}");
    assert!(rig.drawn(sized).width() > base_drawn.width() * 1.6, "painted at 28px: {:?}", rig.drawn(sized));
    close(rig.drawn(sized).height(), r.height(), "painted height = measured height");
    assert_eq!(rect(&rig, inherits).size(), r.size(), "inherited from the box");
    assert!(rect(&rig, button).height() > base_button.height() + 10.0, "button text at 28px: {:?}", rect(&rig, button));
    assert_eq!(
        rig.backend.computed_style(inherits).unwrap().get("font-size").cloned(),
        Some(ComputedValue::Length(Length::px(28.0)))
    );

    // back: remeasured down again
    classes(&rig, sized, "");
    rig.run();
    assert_eq!(rect(&rig, sized).size(), base.size());

    // font-weight: the registered family (monospace glyphs are wider), unless the prop is explicit
    classes(&rig, bold, "bold");
    classes(&rig, pinned, "bold");
    rig.run();
    assert!(rig.drawn(bold).width() > base_drawn.width() + 5.0, "700 → cs5f-bold: {:?} vs {base_drawn:?}", rig.drawn(bold));
    assert_eq!(rect(&rig, bold).width(), rig.drawn(bold).width().ceil(), "measured with the painted font");
    assert_eq!(rig.drawn(pinned).width(), base_drawn.width(), "an explicit font-weight prop wins");

    // font-family: the first bound family of the list
    classes(&rig, mono, "mono");
    rig.run();
    assert!(rig.drawn(mono).width() > base_drawn.width() + 5.0, "monospace: {:?}", rig.drawn(mono));
    assert_eq!(svelters_egui::fonts::take_warnings(), vec![], "an unbound family is skipped quietly");
}

/// Leaf padding is part of the measured size. A label is padded around its widget (it paints in
/// the content box). A button's padding goes inside its frame. Margin and padding shorthands take
/// 2 and 4 values.
#[test]
fn leaf_padding_and_shorthands() {
    let mut rig = Rig::new();
    let col = column(&rig);
    let text = "Padded";
    let plain = rig.leaf(col, "label", &[("text", s(text))]);
    let padded = rig.leaf(col, "label", &[("text", s(text))]);
    let b0 = rig.leaf(col, "button", &[("text", s(text))]);
    let b4 = rig.leaf(col, "button", &[("text", s(text))]);
    let m4 = rig.leaf(col, "label", &[("text", s(text))]);
    sheet(
        &rig,
        "app",
        ".two { padding: 4px 8px; }
         .four { padding: 2px 10px 6px 20px; }
         .m { margin: 1px 2px 3px 6px; }",
    );
    classes(&rig, padded, "two");
    classes(&rig, b4, "four");
    classes(&rig, m4, "m");
    rig.run();

    assert_eq!(
        rig.backend.computed_style(padded).unwrap().get("padding").cloned(),
        Some(ComputedValue::Insets([Length::px(4.0), Length::px(8.0), Length::px(4.0), Length::px(8.0)]))
    );
    // 2 values: 4px top/bottom, 8px left/right, in measure and paint
    let (p, q) = (rect(&rig, plain), rect(&rig, padded));
    close(q.width(), p.width() + 16.0, "padded width");
    close(q.height(), p.height() + 8.0, "padded height");
    let d = rig.drawn(padded);
    // layout rects are root-relative, drawn rects on screen: the unpadded label gives the offset
    let o = rig.drawn(plain).min - p.min;
    close(d.min.x - q.min.x - o.x, 8.0, "painted inside the left padding");
    close(d.min.y - q.min.y - o.y, 4.0, "painted inside the top padding");

    // 4 values on a button: inside the frame, in place of egui's button padding
    let bp = rig.harness.ctx.global_style().spacing.button_padding;
    let (r0, r4) = (rect(&rig, b0), rect(&rig, b4));
    close(r4.width() - r0.width(), (20.0 + 10.0) - 2.0 * bp.x, "button width with left 20 + right 10");
    close(r4.height(), p.height() + 2.0 + 6.0, "button height: its text + top 2 + bottom 6");
    close(rig.drawn(b4).width(), r4.width(), "the frame fills the padded rect");
    close(rig.drawn(b4).height(), r4.height(), "the frame fills the padded rect");

    // 4-value margin: left 6, top 1 (after the 4px gap)
    let m = rect(&rig, m4);
    close(m.min.x - rect(&rig, col).min.x, 6.0, "margin-left");
    close(m.min.y - r4.max.y, 4.0 + 1.0, "gap + margin-top");
}

/// Inline `style="padding: 4px 8px"` reaches the engine (the runtime sends the raw text, the
/// compiler no longer lowers margin/padding to props). A `padding` attribute is an explicit
/// prop and still wins over rules.
#[test]
fn inline_padding_and_explicit_attribute() {
    let mut rig = Rig::new();
    let col = column(&rig);
    let a = rig.leaf(col, "box", &[]);
    let la = rig.leaf(a, "label", &[("text", s("a"))]);
    let b = rig.leaf(col, "box", &[]);
    let lb = rig.leaf(b, "label", &[("text", s("b"))]);
    sheet(&rig, "app", "box { padding: 20px; }");
    inline(&rig, a, "padding: 4px 8px");
    rig.set(b, "padding", Value::F64(8.0));
    rig.run();
    assert_eq!(
        rig.backend.computed_style(a).unwrap().get("padding").cloned(),
        Some(ComputedValue::Insets([Length::px(4.0), Length::px(8.0), Length::px(4.0), Length::px(8.0)])),
        "inline beats the rule"
    );
    let (ra, rla) = (rect(&rig, a), rect(&rig, la));
    close(rla.min.x - ra.min.x, 8.0, "inline padding-left");
    close(rla.min.y - ra.min.y, 4.0, "inline padding-top");
    let (rb, rlb) = (rect(&rig, b), rect(&rig, lb));
    close(rlb.min.x - rb.min.x, 8.0, "the explicit attribute beats `box { padding: 20px }`");
    close(rlb.min.y - rb.min.y, 8.0, "the explicit attribute, top");
}
