//! A kind that folds its children (a label's inline runs): it reads them through its contexts, is
//! a layout leaf, its children are not laid out, recorded or hit, and their changes remeasure it.

use super::*;
use crate::cx::{MeasureCx, RecordCx};
use crate::kind::{Kind, KindSpec};
use crate::{KindRegistry, NodeFlags, Value};
use std::sync::Arc;

static PARA: KindSpec = KindSpec { container: true, folds_children: true, ..KindSpec::new("para") };
const PARA_ID: KindId = KindId::of("para");

/// Ten points of height per character of its children's text.
#[derive(Default)]
struct Para;

impl Kind for Para {
    fn spec() -> &'static KindSpec {
        &PARA
    }
    fn measure(&self, cx: &mut MeasureCx, _known: taffy::Size<Option<f32>>, _avail: taffy::Size<taffy::AvailableSpace>) -> taffy::Size<f32> {
        let chars: usize = (0..cx.children().len()).filter_map(|k| cx.child(k)).map(|c| c.text().chars().count()).sum();
        taffy::Size { width: 10.0, height: 10.0 * chars as f32 }
    }
    fn record(&self, cx: &mut RecordCx) {
        cx.draw_box();
    }
}

#[test]
fn folded_children_are_the_parents_content() {
    let mut r = KindRegistry::new();
    r.register::<testkinds::BoxKind>().unwrap();
    r.register::<testkinds::TextKind>().unwrap();
    r.register::<Para>().unwrap();
    let mut d = Document::new(Arc::new(r), Box::new(FixedText));
    let root = d.create_window(W, BOX, Size::new(200.0, 100.0), Scale(1.0));
    let p = child(&mut d, root, PARA_ID, "background-color: #ff0000");
    assert!(d.flags(p).unwrap().contains(NodeFlags::FOLDS));
    let a = child(&mut d, p, TEXT, "");
    let b = child(&mut d, p, TEXT, "");
    d.set_prop(a, testkinds::TEXT, Value::from("ab")).unwrap();
    d.set_prop(b, testkinds::TEXT, Value::from("cde")).unwrap();
    let c = frame(&mut d).unwrap();
    assert_eq!(d.layout_rect(p).unwrap().height(), 50.0, "measured as a leaf over its children");
    assert_eq!(d.layout_rect(a).unwrap().height(), 0.0, "children are not laid out");
    assert!(!c.lists.iter().any(|(n, _)| *n == a || *n == b), "nor recorded");
    assert_eq!(d.hit_test(W, Point::new(5.0, 5.0)), Some(p), "nor hit");
    // a child's text change remeasures the parent
    d.set_prop(b, testkinds::TEXT, Value::from("cdefgh")).unwrap();
    frame(&mut d);
    assert_eq!(d.layout_rect(p).unwrap().height(), 80.0);
    // so does a structure change under it
    d.destroy(a);
    frame(&mut d);
    assert_eq!(d.layout_rect(p).unwrap().height(), 60.0);
}
