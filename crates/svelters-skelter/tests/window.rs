//! `<window>`: its children are a flex column inside `padding`, `gap` apart (POL2-LAYOUT 5).

mod common;

use common::Rig;
use skelter_base::geom::{Rect, Scale, Size};
use skelter_base::ids::WindowId;
use skelter_base::time::Time;
use svelters_abi::*;

fn laid_out(padding: Option<f64>, css: Option<&str>) -> (Rect, Rect) {
    let r = Rig::new();
    let w = r.create("window");
    if let Some(p) = padding {
        r.set(w, "padding", Value::F64(p));
    }
    r.set(w, "gap", Value::F64(6.0));
    let (a, b) = (r.create("box"), r.create("box"));
    for k in [a, b] {
        r.set(k, "height", Value::F64(20.0));
        r.insert(w, k);
    }
    if let Some(css) = css {
        let n = r.backend.node(w).unwrap();
        r.backend.document().borrow_mut().set_style(n, skelter_core::StyleInput { inline: Some(css.into()), ..Default::default() });
    }
    r.backend.attach_presented(w, WindowId(9), Size::new(200.0, 100.0), Scale(1.0)).unwrap();
    let doc = r.backend.document();
    doc.borrow_mut().frame(WindowId(9), Time(0));
    let d = doc.borrow();
    (d.layout_rect(r.backend.node(a).unwrap()).unwrap(), d.layout_rect(r.backend.node(b).unwrap()).unwrap())
}

#[test]
fn the_window_lays_its_children_out_inside_padding_and_gap_apart() {
    let (a, b) = laid_out(Some(12.0), None);
    assert_eq!((a.min.x, a.min.y, a.max.x), (12.0, 12.0, 188.0), "{a:?}");
    assert_eq!(b.min.y, a.max.y + 6.0, "{b:?}");
    // an unwritten padding is none (not the surface's 8: existing windows keep their layout)
    let (a, _) = laid_out(None, None);
    assert_eq!((a.min.x, a.min.y), (0.0, 0.0), "{a:?}");
    let (a, _) = laid_out(Some(0.0), None);
    assert_eq!((a.min.x, a.min.y), (0.0, 0.0), "{a:?}");
    // CSS wins, as on `box`
    let (a, b) = laid_out(Some(12.0), Some("padding: 2px; gap: 1px"));
    assert_eq!((a.min.x, a.min.y), (2.0, 2.0), "{a:?}");
    assert_eq!(b.min.y, a.max.y + 1.0, "{b:?}");
}
