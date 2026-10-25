//! `position: absolute` boxes: taffy places them in their containing block (the nearest positioned
//! ancestor, else the window root), and every space derived from layout (world rects, hit tests, the
//! transform nodes the lists paint in, clips) must follow.

use super::*;
use skelter_base::props::PropOp;

fn world(d: &Document, n: NodeId) -> (f32, f32) {
    let r = d.world_rect(n).expect("laid out");
    (r.min.x, r.min.y)
}

#[test]
fn nested_absolute_boxes_accumulate() {
    let (mut d, root) = doc();
    let berth = child(&mut d, root, BOX, "position: absolute; left: 26px; top: 14px; width: 100px; height: 60px");
    let dock = child(&mut d, berth, BOX, "position: absolute; left: 0px; top: 32px; width: 50px; height: 20px");
    let inner = child(&mut d, dock, BOX, "position: absolute; left: 5px; top: 3px; width: 10px; height: 10px");
    frame(&mut d);
    assert_eq!(world(&d, berth), (26.0, 14.0));
    assert_eq!(world(&d, dock), (26.0, 46.0), "inside its absolute parent");
    assert_eq!(world(&d, inner), (31.0, 49.0));
    assert_eq!(d.hit_test(W, Point::new(33.0, 51.0)), Some(inner));
    assert_eq!(d.hit_test(W, Point::new(28.0, 48.0)), Some(dock));
}

#[test]
fn an_absolute_box_under_a_static_box_is_placed_in_the_window() {
    let (mut d, root) = doc();
    child(&mut d, root, BOX, "height: 40px");
    let holder = child(&mut d, root, BOX, "height: 40px");
    let abs = child(&mut d, holder, BOX, "position: absolute; left: 30px; top: 10px; width: 20px; height: 20px");
    frame(&mut d);
    assert_eq!(world(&d, holder), (0.0, 40.0));
    assert_eq!(world(&d, abs), (30.0, 10.0), "its containing block is the window root, not the static box");
    assert_eq!(d.hit_test(W, Point::new(35.0, 15.0)), Some(abs));
}

#[test]
fn an_absolute_box_under_a_relative_box_is_placed_in_it() {
    let (mut d, root) = doc();
    child(&mut d, root, BOX, "height: 40px");
    let holder = child(&mut d, root, BOX, "position: relative; height: 40px");
    let abs = child(&mut d, holder, BOX, "position: absolute; left: 30px; top: 10px; width: 20px; height: 20px");
    frame(&mut d);
    assert_eq!(world(&d, abs), (30.0, 50.0));
}

#[test]
fn an_absolute_box_paints_in_its_containing_blocks_space() {
    let (mut d, root) = doc();
    let holder = child(&mut d, root, BOX, "margin-top: 40px; height: 40px");
    let abs = child(&mut d, holder, BOX, "position: absolute; left: 30px; top: 10px; width: 20px; height: 20px; background-color: #ff0000");
    let c = frame(&mut d).unwrap();
    let transform = |n: NodeId| {
        let id = c.layers.iter().find_map(|l| match l {
            skelter_base::layer::LayerDelta::Items { items, .. } => items.iter().find(|it| it.node == n).map(|it| it.transform),
            _ => None,
        })?;
        c.props.ops.iter().find_map(|op| match op {
            PropOp::Transform(t, node) if *t == id => Some(*node),
            _ => None,
        })
    };
    let t = transform(abs).expect("its list is placed");
    assert_eq!((t.local.e, t.local.f), (30.0, 10.0));
    let root_space = c.props.ops.iter().find_map(|op| match op {
        PropOp::Transform(id, n) if n.parent.is_none() => Some(*id),
        _ => None,
    });
    // the window root's own node is a child of the root space; the box hangs under the root's node
    let holder_t = c.props.ops.iter().find_map(|op| match op {
        PropOp::Transform(_, n) if n.local.f == 40.0 => Some(*n),
        _ => None,
    });
    assert!(holder_t.is_some());
    assert_ne!(t.parent, root_space, "not the window's own space");
    let _ = holder;
}

#[test]
fn a_clip_between_an_absolute_box_and_its_containing_block_does_not_clip_it() {
    let (mut d, root) = doc();
    let clipper = child(&mut d, root, BOX, "position: relative; width: 100px; height: 60px");
    let hidden = child(&mut d, clipper, BOX, "overflow-x: hidden; overflow-y: hidden; width: 10px; height: 10px");
    let abs = child(&mut d, hidden, BOX, "position: absolute; left: 40px; top: 20px; width: 20px; height: 20px");
    frame(&mut d);
    // CSS: overflow clips an absolute descendant only when it is the containing block or above it
    assert_eq!(d.hit_test(W, Point::new(45.0, 25.0)), Some(abs));
    let _ = clipper;
}

#[test]
fn a_clipping_containing_block_clips_its_absolute_children() {
    let (mut d, root) = doc();
    let cb = child(&mut d, root, BOX, "position: relative; overflow-x: hidden; overflow-y: hidden; width: 50px; height: 50px");
    let abs = child(&mut d, cb, BOX, "position: absolute; left: 40px; top: 0px; width: 30px; height: 20px");
    frame(&mut d);
    assert_eq!(d.hit_test(W, Point::new(45.0, 5.0)), Some(abs));
    assert_ne!(d.hit_test(W, Point::new(60.0, 5.0)), Some(abs), "clipped by its containing block");
}
