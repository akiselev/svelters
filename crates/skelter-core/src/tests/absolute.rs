//! `position: absolute` boxes: taffy places them in their containing block (the nearest positioned
//! ancestor, else the window root), and every space derived from layout (world rects, hit tests, the
//! transform nodes the lists paint in, clips) must follow.

use super::*;
use skelter_base::props::PropOp;
use crate::Value;

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

/// The compositor's view, accumulated over commits: transform nodes and each node's list placement.
#[derive(Default)]
struct Painted {
    transforms: std::collections::HashMap<skelter_base::ids::PropNodeId, skelter_base::props::TransformNode>,
    placed: std::collections::HashMap<NodeId, skelter_base::ids::PropNodeId>,
    lists: std::collections::HashSet<NodeId>,
}

impl Painted {
    fn apply(&mut self, c: &skelter_base::commit::Commit) {
        for op in &c.props.ops {
            match op {
                PropOp::Transform(id, t) => {
                    self.transforms.insert(*id, *t);
                }
                PropOp::Remove(id) => {
                    self.transforms.remove(id);
                }
                _ => {}
            }
        }
        for l in &c.layers {
            if let skelter_base::layer::LayerDelta::Items { items, .. } = l {
                for it in items {
                    self.placed.insert(it.node, it.transform);
                }
            }
        }
        // the compositor's order: removals, then the re-recorded lists
        for n in &c.removed {
            self.lists.remove(n);
        }
        for (n, _) in &c.lists {
            self.lists.insert(*n);
        }
    }
    /// Where `n`'s list is drawn (its space's origin in window points).
    fn origin(&self, n: NodeId) -> Option<(f32, f32)> {
        let mut id = Some(*self.placed.get(&n)?);
        let (mut x, mut y) = (0.0, 0.0);
        while let Some(i) = id {
            let t = self.transforms.get(&i)?;
            x += t.local.e;
            y += t.local.f;
            id = t.parent;
        }
        Some((x, y))
    }
}

/// A pane of rows (a button and two labels each) under `parent`.
fn pane_rows(d: &mut Document, parent: NodeId) -> (NodeId, Vec<[NodeId; 4]>) {
    let pane = child(d, parent, BOX, "flex-direction: column; width: 200px; height: 100px");
    let mut rows = Vec::new();
    for _ in 0..2 {
        let row = child(d, pane, BOX, "flex-direction: row; height: 20px; background-color: #222222");
        let button = child(d, row, TOGGLE, "width: 40px; height: 20px; background-color: #444444");
        let a = child(d, row, TEXT, "width: 40px; height: 20px");
        let b = child(d, row, TEXT, "width: 40px; height: 20px");
        d.set_prop(a, crate::testkinds::TEXT, Value::from("Name")).unwrap();
        d.set_prop(b, crate::testkinds::TEXT, Value::from("Value")).unwrap();
        rows.push([row, button, a, b]);
    }
    (pane, rows)
}

fn settle(d: &mut Document, seen: &mut Painted) {
    for _ in 0..8 {
        match frame(d) {
            Some(c) => seen.apply(&c),
            None => break,
        }
    }
}

fn assert_drawn_in_place(d: &Document, seen: &Painted, rows: &[[NodeId; 4]], case: &str) {
    for row in rows {
        for &n in row {
            let r = d.world_rect(n).unwrap();
            assert!(seen.lists.contains(&n), "{case}: {n:?} is drawn");
            assert_eq!(seen.origin(n), Some((r.min.x, r.min.y)), "{case}: {n:?} is drawn where it is laid out");
        }
    }
}

#[test]
fn a_subtree_under_an_absolute_ancestor_that_moves_later_draws_at_the_new_place() {
    const PLACED: &str = "position: absolute; left: 326px; top: 178px; width: 200px; height: 100px";
    // 1. the slot is placed after the pane's nodes were created and drawn
    let (mut d, root) = doc();
    let slot = child(&mut d, root, BOX, "position: absolute; left: 0px; top: 0px; width: 200px; height: 100px");
    let (_, rows) = pane_rows(&mut d, slot);
    let mut seen = Painted::default();
    settle(&mut d, &mut seen);
    d.set_style(slot, StyleInput { inline: Some(PLACED.into()), ..Default::default() });
    settle(&mut d, &mut seen);
    assert_drawn_in_place(&d, &seen, &rows, "slot moved");
    // 2. the pane, drawn elsewhere first, moves into a newly placed slot
    let (mut d, root) = doc();
    let (pane, rows) = pane_rows(&mut d, root);
    let mut seen = Painted::default();
    settle(&mut d, &mut seen);
    let dock = child(&mut d, root, BOX, "position: absolute; left: 0px; top: 0px; width: 600px; height: 400px");
    let slot = child(&mut d, dock, BOX, PLACED);
    d.insert(slot, pane, None, None).unwrap();
    settle(&mut d, &mut seen);
    assert_drawn_in_place(&d, &seen, &rows, "pane moved into the slot");
    // 3. the pane is built detached and inserted into a slot placed in the same frame
    let (mut d, root) = doc();
    let mut seen = Painted::default();
    settle(&mut d, &mut seen);
    let holder = d.create(BOX);
    let (_, rows) = pane_rows(&mut d, holder);
    let slot = child(&mut d, root, BOX, "position: absolute; left: 0px; top: 0px; width: 200px; height: 100px");
    d.insert(slot, holder, None, None).unwrap();
    settle(&mut d, &mut seen);
    d.set_style(slot, StyleInput { inline: Some(PLACED.into()), ..Default::default() });
    settle(&mut d, &mut seen);
    assert_drawn_in_place(&d, &seen, &rows, "detached pane inserted, then the slot placed");
    // 4. berth's reconcile: a placed group re-inserts its tab strip and pane in place (same parent)
    let (mut d, root) = doc();
    let group = child(&mut d, root, BOX, "position: absolute; left: 326px; top: 146px; width: 300px; height: 200px; overflow: hidden; background-color: #111111");
    let tabs = child(&mut d, group, BOX, "height: 24px; background-color: #333333");
    let (pane, rows) = pane_rows(&mut d, group);
    let mut seen = Painted::default();
    settle(&mut d, &mut seen);
    d.insert(group, tabs, Some(pane), None).unwrap();
    d.insert(group, pane, None, None).unwrap();
    settle(&mut d, &mut seen);
    assert_drawn_in_place(&d, &seen, &rows, "group re-inserted its children");
    let r = d.world_rect(group).unwrap();
    assert_eq!(seen.origin(group), Some((r.min.x, r.min.y)), "the group's own box");
    // 5. a z-indexed absolute node is removed and another inserted in the same frame
    let (mut d, root) = doc();
    let old = child(&mut d, root, BOX, "position: absolute; z-index: 5; left: 40px; top: 30px; width: 50px; height: 20px; background-color: #ff0000");
    let mut seen = Painted::default();
    settle(&mut d, &mut seen);
    d.remove(root, old).unwrap();
    let new = child(&mut d, root, BOX, "position: absolute; z-index: 5; left: 120px; top: 60px; width: 50px; height: 20px; background-color: #00ff00");
    settle(&mut d, &mut seen);
    assert!(!seen.lists.contains(&old), "the removed node leaves no paint");
    assert!(seen.lists.contains(&new), "the inserted node paints");
    assert_eq!(seen.origin(new), Some((120.0, 60.0)), "where it is laid out");
}

/// Each painting node of `nodes` is drawn where it is laid out.
fn assert_each_in_place(d: &Document, seen: &Painted, nodes: &[NodeId], case: &str) {
    for &n in nodes {
        let r = d.world_rect(n).unwrap();
        assert!(seen.lists.contains(&n), "{case}: {n:?} is drawn");
        assert_eq!(seen.origin(n), Some((r.min.x, r.min.y)), "{case}: {n:?} is drawn where it is laid out");
    }
}

/// A row of three 30×10 boxes under `parent`, before `before`.
fn three_boxes(d: &mut Document, parent: NodeId, before: Option<NodeId>) -> Vec<NodeId> {
    let row = d.create(BOX);
    d.set_style(row, StyleInput { inline: Some("flex-direction: row; height: 10px; gap: 4px; background-color: #222222".into()), ..Default::default() });
    d.insert(parent, row, before, None).unwrap();
    let mut out = vec![row];
    for _ in 0..3 {
        out.push(child(d, row, BOX, "width: 30px; height: 10px; background-color: #ff0000"));
    }
    out
}

/// A29: in-flow nodes created after the window's first frame get their final transform, not their
/// parent's origin (the compositor's transform locals, not only `world_rect`).
#[test]
fn in_flow_nodes_created_after_the_first_frame_draw_where_they_are_laid_out() {
    const COLUMN: &str = "flex-direction: column; gap: 3px; padding: 5px; width: 200px; height: 100px";
    // 1. a new in-flow row inserted into the root column, between two rows
    let (mut d, root) = doc();
    d.set_style(root, StyleInput { inline: Some(COLUMN.into()), ..Default::default() });
    let first = three_boxes(&mut d, root, None);
    let last = three_boxes(&mut d, root, None);
    let mut seen = Painted::default();
    settle(&mut d, &mut seen);
    let mid = three_boxes(&mut d, root, Some(last[0]));
    settle(&mut d, &mut seen);
    assert_each_in_place(&d, &seen, &[first, mid, last].concat(), "row inserted into the root column");
    // 2. a flex row's children destroyed and created again
    let (mut d, root) = doc();
    d.set_style(root, StyleInput { inline: Some(COLUMN.into()), ..Default::default() });
    let _top = three_boxes(&mut d, root, None);
    let row = three_boxes(&mut d, root, None);
    let mut seen = Painted::default();
    settle(&mut d, &mut seen);
    for &c in &row[1..] {
        d.destroy(c);
    }
    let mut again = vec![row[0]];
    for w in [20, 40, 30] {
        again.push(child(&mut d, row[0], BOX, &format!("width: {w}px; height: 10px; background-color: #00ff00")));
    }
    settle(&mut d, &mut seen);
    assert_each_in_place(&d, &seen, &again, "row children rebuilt");
    // 3. a leaf inserted into a nested slot
    let (mut d, root) = doc();
    d.set_style(root, StyleInput { inline: Some(COLUMN.into()), ..Default::default() });
    let _top = three_boxes(&mut d, root, None);
    let outer = child(&mut d, root, BOX, "flex-direction: row; padding: 6px; height: 40px; background-color: #333333");
    let _side = child(&mut d, outer, BOX, "width: 50px; height: 20px; background-color: #444444");
    let slot = child(&mut d, outer, BOX, "flex-direction: column; padding: 2px; width: 80px; height: 30px; background-color: #555555");
    let mut seen = Painted::default();
    settle(&mut d, &mut seen);
    let leaf = child(&mut d, slot, BOX, "width: 10px; height: 10px; margin-left: 7px; background-color: #ff00ff");
    settle(&mut d, &mut seen);
    assert_each_in_place(&d, &seen, &[outer, slot, leaf], "leaf inserted into a nested slot");
    // 4. a subtree shown after display: none
    let (mut d, root) = doc();
    d.set_style(root, StyleInput { inline: Some(COLUMN.into()), ..Default::default() });
    let _top = three_boxes(&mut d, root, None);
    let hidden = three_boxes(&mut d, root, None);
    d.set_style(hidden[0], StyleInput { inline: Some("display: none; flex-direction: row; height: 10px; gap: 4px; background-color: #222222".into()), ..Default::default() });
    let below = three_boxes(&mut d, root, None);
    let mut seen = Painted::default();
    settle(&mut d, &mut seen);
    d.set_style(hidden[0], StyleInput { inline: Some("flex-direction: row; height: 10px; gap: 4px; background-color: #222222".into()), ..Default::default() });
    settle(&mut d, &mut seen);
    assert_each_in_place(&d, &seen, &[hidden, below].concat(), "subtree shown after display: none");
    // 5. a block swapped as Svelte does it: the old row and an anchor removed, a new row inserted in
    // the same frame (the freed ids are reused by the new nodes)
    let (mut d, root) = doc();
    d.set_style(root, StyleInput { inline: Some(COLUMN.into()), ..Default::default() });
    let _top = three_boxes(&mut d, root, None);
    let old = three_boxes(&mut d, root, None);
    let anchor = child(&mut d, root, BOX, "height: 0px");
    let below = three_boxes(&mut d, root, None);
    let mut seen = Painted::default();
    settle(&mut d, &mut seen);
    d.destroy(old[0]);
    d.destroy(anchor);
    let new = three_boxes(&mut d, root, Some(below[0]));
    settle(&mut d, &mut seen);
    assert_each_in_place(&d, &seen, &[new, below].concat(), "block swapped in one frame");
    // 6. a dock that fills the window (percent and flex sizes) through resizes, with a pane built
    // after the first frame
    let (mut d, root) = doc();
    d.set_style(root, StyleInput { inline: Some("flex-direction: column".into()), ..Default::default() });
    let _bar = three_boxes(&mut d, root, None);
    let dock = child(&mut d, root, BOX, "flex-direction: row; flex-grow: 1; background-color: #111111");
    let side = child(&mut d, dock, BOX, "width: 30%; background-color: #333333");
    let group = child(&mut d, dock, BOX, "position: relative; flex-grow: 1; padding: 4px; background-color: #222222");
    let mut seen = Painted::default();
    settle(&mut d, &mut seen);
    let pane = three_boxes(&mut d, group, None);
    let floating = child(&mut d, group, BOX, "position: absolute; right: 5px; bottom: 5px; width: 20px; height: 20px; background-color: #00ffff");
    let nodes = [&[dock, side, group, floating][..], &pane].concat();
    settle(&mut d, &mut seen);
    assert_each_in_place(&d, &seen, &nodes, "dock before a resize");
    for (w, h) in [(400.0, 300.0), (150.0, 80.0), (640.0, 480.0)] {
        d.resize_window(W, skelter_base::geom::Size::new(w, h), skelter_base::geom::Scale(1.0));
        settle(&mut d, &mut seen);
        assert_each_in_place(&d, &seen, &nodes, &format!("dock after a resize to {w}x{h}"));
    }
}
