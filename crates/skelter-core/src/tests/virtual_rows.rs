//! Driving `Kind::virtual_children`: rows realised for the viewport before layout, placed at their
//! row rects, recycled through a `RowPool` while scrolling, the scroll extent from the model.

use super::*;
use crate::cx::{RealizeCx, RecordCx};
use crate::kind::{Kind, KindSpec, VirtualChildren};
use crate::virt::RowPool;
use crate::{KindRegistry, NodeFlags};
use skelter_base::geom::Rect;
use std::ops::Range;
use std::sync::Arc;

static VLIST: KindSpec = KindSpec { container: true, ..KindSpec::new("vlist") };
const VL: KindId = KindId::of("vlist");
const ROW_H: f32 = 10.0;

#[derive(Default)]
struct VList {
    pool: Option<RowPool>,
}

impl VirtualChildren for VList {
    fn row_count(&self) -> usize {
        1000
    }
    fn visible(&self, v: Rect) -> Range<usize> {
        (v.min.y / ROW_H).floor() as usize..(v.max.y / ROW_H).ceil() as usize
    }
    fn row_rect(&self, i: usize) -> Rect {
        Rect::new(0.0, i as f32 * ROW_H, 100.0, (i + 1) as f32 * ROW_H)
    }
    fn content_extent(&self) -> Vec2 {
        Vec2::new(100.0, 1000.0 * ROW_H)
    }
    fn realize(&mut self, cx: &mut RealizeCx, rows: Range<usize>) -> Vec<(usize, NodeId)> {
        let pool = self.pool.get_or_insert_with(RowPool::new);
        let node = cx.node();
        pool.release_except(cx.doc(), |k| rows.contains(&(k as usize)));
        let mut out = Vec::new();
        for r in rows {
            let n = pool.row(cx.doc(), r as u64, 0, |d| {
                let n = d.create(BOX);
                d.insert(node, n, None, None).unwrap();
                n
            });
            out.push((r, n));
        }
        pool.release_unused(cx.doc());
        out
    }
}

impl Kind for VList {
    fn spec() -> &'static KindSpec {
        &VLIST
    }
    fn record(&self, cx: &mut RecordCx) {
        cx.draw_box();
    }
    fn virtual_children(&mut self) -> Option<&mut dyn VirtualChildren> {
        Some(self)
    }
}

fn vdoc() -> (Document, NodeId) {
    let mut r = KindRegistry::new();
    r.register::<testkinds::BoxKind>().unwrap();
    r.register::<VList>().unwrap();
    let mut d = Document::new(Arc::new(r), Box::new(FixedText));
    let root = d.create_window(W, BOX, Size::new(200.0, 100.0), Scale(1.0));
    let list = child(&mut d, root, VL, "height: 50px; overflow-y: scroll");
    (d, list)
}

fn rows(d: &Document, list: NodeId) -> Vec<usize> {
    d.virtual_rows(list).iter().map(|&(r, _)| r).collect()
}

#[test]
fn the_viewport_rows_are_realised_and_placed() {
    let (mut d, list) = vdoc();
    assert!(d.flags(list).unwrap().contains(NodeFlags::VIRTUAL));
    frame(&mut d);
    assert_eq!(rows(&d, list), (0..5).collect::<Vec<_>>());
    let (_, third) = d.virtual_rows(list)[3];
    assert!(d.flags(third).unwrap().contains(NodeFlags::ROW));
    let r = d.layout_rect(third).unwrap();
    assert_eq!((r.min.y, r.height(), r.width()), (30.0, 10.0, 100.0));
    assert_eq!(d.children(list).len(), 5);
}

#[test]
fn scrolling_recycles_rows_and_the_extent_is_the_models() {
    let (mut d, list) = vdoc();
    frame(&mut d);
    let before: Vec<NodeId> = d.children(list).to_vec();
    d.set_scroll_offset(list, Vec2::new(0.0, 100.0));
    assert_eq!(d.scroll_offset(list), Vec2::new(0.0, 100.0));
    frame(&mut d);
    assert_eq!(rows(&d, list), (10..15).collect::<Vec<_>>());
    let mut after: Vec<NodeId> = d.virtual_rows(list).iter().map(|&(_, n)| n).collect();
    after.sort();
    let mut b = before.clone();
    b.sort();
    assert_eq!(after, b, "the same five nodes, rebound");
    assert_eq!(d.layout_rect(d.virtual_rows(list)[0].1).unwrap().min.y, 100.0);
    // the model's extent bounds the scroll range: 10000 - 50
    d.set_scroll_offset(list, Vec2::new(0.0, 1.0e6));
    assert_eq!(d.scroll_offset(list).y, 9950.0);
}

#[test]
fn a_smaller_range_parks_the_rows_it_no_longer_needs() {
    let (mut d, list) = vdoc();
    frame(&mut d);
    d.set_style(list, StyleInput { inline: Some("height: 20px; overflow-y: scroll".into()), ..Default::default() });
    frame(&mut d);
    assert_eq!(rows(&d, list), vec![0, 1]);
    assert_eq!(d.children(list).len(), 5, "pooled, not destroyed");
    let parked = d.children(list).iter().filter(|c| !d.virtual_rows(list).iter().any(|&(_, n)| n == **c)).count();
    assert_eq!(parked, 3);
    for &c in d.children(list) {
        let shown = d.virtual_rows(list).iter().any(|&(_, n)| n == c);
        assert_eq!(d.layout_rect(c).unwrap().height() > 0.0, shown);
    }
}
