//! CS5g: `box-shadow` painted by the egui cascade, and CSS-only sizes as relayout boundaries,
//! headless under kittest (the recording host of `common`, no JS).

mod common;

use common::Rig;
use svelters_abi::*;
use svelters_egui::scene::NodeId;
use svelters_style::ComputedValue;

fn sheet(rig: &Rig, id: &str, css: &str) {
    rig.br().style(SVL_STYLE_SHEET, 0, Some(&format!("{id}{SVL_STYLE_SEP}{css}"))).unwrap();
}

fn classes(rig: &Rig, h: svl_handle, c: &str) {
    rig.br().style(SVL_STYLE_CLASSES, h, Some(c)).unwrap();
}

/// The blurred or plain rects of the last frame painted in `color`.
fn rects_in(rig: &Rig, color: egui::Color32) -> Vec<egui::epaint::RectShape> {
    let mut out = Vec::new();
    for c in &rig.harness.output().shapes {
        if let egui::Shape::Rect(r) = &c.shape
            && r.fill == color
        {
            out.push(r.clone());
        }
    }
    out
}

/// A box's outer shadow paints below its background (moved, spread, blurred: CSS blur 4 = a
/// penumbra of 8), its inset shadow above it as bands clipped to the box; a button's shadow
/// follows its hover state; `none` removes them.
#[test]
fn shadows_paint_around_boxes_and_widgets() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let card = rig.leaf(col, "box", &[]);
    rig.leaf(card, "label", &[("text", Value::Str("card".into()))]);
    let btn = rig.leaf(col, "button", &[("text", Value::Str("go".into()))]);
    classes(&rig, card, "card");
    sheet(
        &rig,
        "app",
        ".card { width: 200px; height: 80px; background-color: #ffffff; border-radius: 6px;
                 box-shadow: 3px 5px 4px 2px #ff0000, inset 0 0 0 7px #0000ff; }
         button { box-shadow: 0 2px #00ff00; }
         button:hover { box-shadow: 0 4px 0 1px #00ffff; }",
    );
    rig.run();
    let Some(ComputedValue::Shadows(list)) = rig.backend.computed_style(card).unwrap().get("box-shadow").cloned() else { panic!() };
    assert_eq!(list.len(), 2);

    // the box's own rect: its white background (containers have no `drawn` rect)
    let r = rects_in(&rig, egui::Color32::WHITE).into_iter().find(|x| x.rect.size() == egui::vec2(200.0, 80.0)).expect("the card's background").rect;
    let red = rects_in(&rig, egui::Color32::from_rgb(255, 0, 0));
    assert_eq!(red.len(), 1, "{red:?}");
    assert_eq!(red[0].rect, r.translate(egui::vec2(3.0, 5.0)).expand(2.0));
    assert_eq!(red[0].blur_width, 8.0);
    assert_eq!(red[0].corner_radius, egui::CornerRadius::same(8), "radius grows with the spread");
    // paint order: shadow, background, inset shadow
    let shapes = &rig.harness.output().shapes;
    let at = |c: egui::Color32, rect: Option<egui::Rect>| {
        shapes.iter().position(|s| matches!(&s.shape, egui::Shape::Rect(x) if x.fill == c && rect.is_none_or(|r| x.rect == r))).unwrap()
    };
    let (shadow, bg, inset) = (at(egui::Color32::from_rgb(255, 0, 0), None), at(egui::Color32::WHITE, Some(r)), at(egui::Color32::from_rgb(0, 0, 255), None));
    assert!(shadow < bg && bg < inset, "{shadow} {bg} {inset}");
    let blue: Vec<_> = shapes.iter().filter(|s| matches!(&s.shape, egui::Shape::Rect(x) if x.fill == egui::Color32::from_rgb(0, 0, 255))).collect();
    assert_eq!(blue.len(), 4, "four bands frame the inset hole");
    for b in &blue {
        assert!(r.contains_rect(b.clip_rect), "clipped to the box: {:?} in {r:?}", b.clip_rect);
    }

    // a stock widget: the outer shadow of its current state, behind it
    let g = rects_in(&rig, egui::Color32::from_rgb(0, 255, 0));
    assert_eq!(g.len(), 1);
    assert_eq!(g[0].rect, rig.drawn(btn).translate(egui::vec2(0.0, 2.0)));
    rig.harness.hover_at(rig.drawn(btn).center());
    rig.harness.step();
    rig.harness.step();
    assert!(rects_in(&rig, egui::Color32::from_rgb(0, 255, 0)).is_empty());
    let c = rects_in(&rig, egui::Color32::from_rgb(0, 255, 255));
    assert_eq!(c.len(), 1, "the hover state's shadow");
    assert_eq!(c[0].rect, rig.drawn(btn).translate(egui::vec2(0.0, 4.0)).expand(1.0));

    sheet(&rig, "app", ".card { box-shadow: none; background-color: #ffffff; }");
    rig.run();
    assert!(rects_in(&rig, egui::Color32::from_rgb(255, 0, 0)).is_empty());
    assert!(rects_in(&rig, egui::Color32::from_rgb(0, 0, 255)).is_empty());
}

/// A node scrolled off screen whose shadow reaches into view still paints (the walk culls by
/// the rect grown by the outer shadows).
#[test]
fn shadows_widen_the_culling_rect() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let spacer = rig.leaf(col, "box", &[]);
    let card = rig.leaf(col, "box", &[]);
    classes(&rig, spacer, "spacer");
    classes(&rig, card, "card");
    sheet(&rig, "app", ".spacer { height: 2000px; flex-shrink: 0; } .card { height: 40px; flex-shrink: 0; margin-top: -2400px; box-shadow: 0 0 0 400px #ff00ff; }");
    rig.run();
    let m = rects_in(&rig, egui::Color32::from_rgb(255, 0, 255));
    assert_eq!(m.len(), 1, "painted although the box itself is out of view");
    let card_bottom = m[0].rect.max.y - 400.0;
    assert!(card_bottom < 0.0 && m[0].rect.max.y > 0.0, "the box is above the screen, its shadow reaches in: {:?}", m[0].rect);
    let _ = (card, col);
}

/// A size set only by CSS makes a layout boundary: a label change inside a row sized by its
/// class re-lays out that row alone (CS5e's `is_boundary` read the props only).
#[test]
fn css_only_sizes_are_layout_boundaries() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let mut rows = Vec::new();
    let mut labels = Vec::new();
    for r in 0..10 {
        let row = rig.leaf(col, "box", &[]);
        classes(&rig, row, "row");
        rows.push(row);
        for k in 0..3 {
            labels.push(rig.leaf(row, "label", &[("text", Value::Str(format!("{r}.{k}")))]));
        }
    }
    sheet(&rig, "app", ".row { flex-direction: row; width: 300px; height: 18px; flex-shrink: 0; }");
    rig.run();
    let h = rig.backend.scene();
    let id = |x: svl_handle| NodeId::from_raw(x).unwrap();
    h.read(|s| {
        for &r in &rows {
            let n = s.get(id(r)).unwrap();
            assert!(n.is_layout_boundary(), "CSS width/height/flex-shrink make a boundary");
        }
        assert!(!s.get(id(col)).unwrap().is_layout_boundary());
    });
    let before = h.read(|s| s.stats());
    rig.set(labels[13], "text", Value::Str("a much longer label".into()));
    rig.run();
    let after = h.read(|s| s.stats());
    assert_eq!(after.layouts, before.layouts, "no whole-root layout");
    assert_eq!(after.boundaries_laid_out - before.boundaries_laid_out, 1, "the row alone");
    h.read(|s| assert!(s.get(id(labels[13])).unwrap().rect().width() > 60.0));

    // without the CSS height the row's size depends on its content again
    sheet(&rig, "app", ".row { flex-direction: row; width: 300px; flex-shrink: 0; }");
    rig.run();
    h.read(|s| assert!(!s.get(id(rows[0])).unwrap().is_layout_boundary()));
}
