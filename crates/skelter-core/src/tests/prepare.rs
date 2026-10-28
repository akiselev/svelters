//! POL2-REGISTER R1 and R3: a prepare that asks for another pass (`PrepareCx::request_paint`) is
//! prepared again next frame, and a prepare reads where its children were laid out
//! (`NodeView::layout_rect`).

use super::*;
use crate::cx::{PrepareCx, RecordCx};
use crate::kind::{Kind, KindSpec};
use crate::KindRegistry;
use skelter_base::geom::Rect;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

static PREPARES: AtomicUsize = AtomicUsize::new(0);
static CHILD_BOX: Mutex<Option<Rect>> = Mutex::new(None);
static PROBE_SPEC: KindSpec = KindSpec { container: true, ..KindSpec::new("probe") };

/// Asks for one more pass on its first prepare; notes its first child's layout box.
#[derive(Default)]
struct Probe {
    asked: bool,
}

impl Kind for Probe {
    fn spec() -> &'static KindSpec {
        &PROBE_SPEC
    }
    fn prepare(&mut self, cx: &mut PrepareCx) {
        PREPARES.fetch_add(1, Ordering::SeqCst);
        *CHILD_BOX.lock().unwrap() = cx.node.child(0).and_then(|c| c.layout_rect());
        if !std::mem::replace(&mut self.asked, true) {
            cx.request_paint();
        }
    }
    fn record(&self, cx: &mut RecordCx) {
        cx.draw_box();
    }
}

#[test]
fn a_prepares_request_brings_a_second_prepare_and_children_have_their_boxes() {
    let mut r = KindRegistry::new();
    r.register::<testkinds::BoxKind>().unwrap();
    r.register::<Probe>().unwrap();
    let mut d = Document::new(Arc::new(r), Box::new(FixedText));
    let root = d.create_window(W, BOX, Size::new(200.0, 100.0), Scale(1.0));
    let probe = child(&mut d, root, KindId::of("probe"), "padding-left: 12px; padding-top: 8px; width: 100px; height: 60px");
    child(&mut d, probe, BOX, "width: 30px; height: 20px");
    frame(&mut d);
    assert_eq!(PREPARES.load(Ordering::SeqCst), 1);
    assert_eq!(*CHILD_BOX.lock().unwrap(), Some(Rect::new(12.0, 8.0, 42.0, 28.0)), "the child's box in its parent, at prepare time");
    assert!(d.needs_frame(W), "the request makes the next frame due");
    frame(&mut d);
    assert_eq!(PREPARES.load(Ordering::SeqCst), 2, "prepared again");
    assert!(!d.needs_frame(W), "settles once nothing asks");
}
