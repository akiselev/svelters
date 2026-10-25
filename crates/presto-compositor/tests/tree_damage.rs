//! The committed tree (folding, flattening, OS-layer surfaces, scroll transforms) and damage (frame
//! diffs, device rects, buffer-age history).

use presto_base::color::{Color, Fill};
use presto_base::commit::Commit;
use presto_base::display::{DisplayList, Prim, Quad};
use presto_base::geom::{Affine, Corners, DeviceRect, DeviceSize, Insets, Point, Rect, Scale, Size, Vec2};
use presto_base::ids::{LayerId, NodeId, PropNodeId, Revision, SurfaceId, WindowId};
use presto_base::layer::{ItemRef, LayerDelta, LayerDesc, LayerKind};
use presto_base::props::{ClipNode, ClipShape, OverscrollBehavior, PropOp, ScrollNode, TransformNode};
use presto_compositor::damage::{self, DamageHistory, Snapshot};
use presto_compositor::tree::{SurfaceKey, WindowTree};
use std::sync::Arc;

fn quad(r: Rect) -> Arc<DisplayList> {
    let mut l = DisplayList::new();
    l.push(
        Prim::Quad(Quad {
            rect: r,
            radii: Corners::ZERO,
            fill: Fill::Solid(Color::BLACK),
            border_widths: Insets::ZERO,
            border_color: Color::TRANSPARENT,
            border_style: Default::default(),
        }),
        r,
    );
    Arc::new(l)
}

fn desc(id: u32, parent: Option<u32>, kind: LayerKind, transform: u32, z: i32, bounds: Rect) -> LayerDesc {
    LayerDesc {
        id: LayerId(id),
        parent: parent.map(LayerId),
        kind,
        transform: PropNodeId(transform),
        clip: None,
        effect: None,
        z,
        bounds,
        opaque: false,
    }
}

fn item(node: u64, transform: u32) -> ItemRef {
    ItemRef { node: NodeId(node), transform: PropNodeId(transform), clip: None, effect: None }
}

fn t(id: u32, parent: Option<u32>, local: Affine) -> PropOp {
    PropOp::Transform(PropNodeId(id), TransformNode { parent: parent.map(PropNodeId), local, scroll: None })
}

fn commit(revision: u64) -> Commit {
    Commit { window: WindowId(1), revision: Revision(revision), ..Default::default() }
}

#[test]
fn layers_paint_items_then_children_by_z() {
    let mut tree = WindowTree::default();
    let full = Rect::new(0.0, 0.0, 100.0, 100.0);
    let mut c = commit(1);
    c.props.ops = vec![t(1, None, Affine::IDENTITY)];
    c.layers = vec![
        LayerDelta::Add(desc(1, None, LayerKind::Root, 1, 0, full)),
        LayerDelta::Add(desc(3, Some(1), LayerKind::Content, 1, 5, full)),
        LayerDelta::Add(desc(2, Some(1), LayerKind::Overlay, 1, 10, full)),
        LayerDelta::Add(desc(4, Some(1), LayerKind::Content, 1, 5, full)),
        LayerDelta::Items { layer: LayerId(1), items: vec![item(1, 1), item(2, 1)] },
        LayerDelta::Items { layer: LayerId(2), items: vec![item(3, 1)] },
        LayerDelta::Items { layer: LayerId(3), items: vec![item(4, 1)] },
        LayerDelta::Items { layer: LayerId(4), items: vec![item(5, 1)] },
    ];
    c.lists = (1..=5).map(|n| (NodeId(n), quad(Rect::new(0.0, 0.0, 10.0, 10.0)))).collect();
    tree.apply_commit(c);
    let flat = tree.flatten();
    let order: Vec<u64> = flat[&SurfaceKey::Main].iter().map(|i| i.node.0).collect();
    // Root's items, then z 5 in insertion order (layer 3, then 4), then the overlay at z 10.
    assert_eq!(order, vec![1, 2, 4, 5, 3]);
}

#[test]
fn later_commits_fold_over_earlier_ones() {
    let mut tree = WindowTree::default();
    let full = Rect::new(0.0, 0.0, 100.0, 100.0);
    let mut c1 = commit(1);
    c1.props.ops = vec![t(1, None, Affine::translate(10.0, 0.0))];
    c1.layers = vec![
        LayerDelta::Add(desc(1, None, LayerKind::Root, 1, 0, full)),
        LayerDelta::Items { layer: LayerId(1), items: vec![item(1, 1), item(2, 1)] },
    ];
    c1.lists = vec![(NodeId(1), quad(Rect::new(0.0, 0.0, 10.0, 10.0))), (NodeId(2), quad(Rect::new(0.0, 0.0, 5.0, 5.0)))];
    let mut c2 = commit(2);
    c2.props.ops = vec![t(1, None, Affine::translate(20.0, 0.0))];
    c2.removed = vec![NodeId(2)];
    c2.layers = vec![LayerDelta::Items { layer: LayerId(1), items: vec![item(1, 1)] }];
    tree.apply_commit(c1);
    tree.apply_commit(c2);
    assert_eq!(tree.revision, Revision(2));
    assert!(!tree.lists.contains_key(&NodeId(2)));
    let flat = tree.flatten();
    let items = &flat[&SurfaceKey::Main];
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].transform, Affine::translate(20.0, 0.0));
    assert_eq!(items[0].bounds, Rect::new(20.0, 0.0, 30.0, 10.0));
}

#[test]
fn scroll_offset_translates_content_and_clips_apply() {
    let mut tree = WindowTree::default();
    let mut c = commit(1);
    c.props.ops = vec![
        t(1, None, Affine::IDENTITY),
        PropOp::Scroll(
            PropNodeId(3),
            ScrollNode {
                space: PropNodeId(1),
                offset: Vec2::new(0.0, 40.0),
                content: Size::new(100.0, 1000.0),
                viewport: Rect::new(0.0, 0.0, 100.0, 100.0),
                scroll_x: false,
                scroll_y: true,
                overscroll: OverscrollBehavior::None,
                passive: true,
            },
        ),
        PropOp::Transform(
            PropNodeId(2),
            TransformNode { parent: Some(PropNodeId(1)), local: Affine::translate(0.0, 10.0), scroll: Some(PropNodeId(3)) },
        ),
        PropOp::Clip(
            PropNodeId(5),
            ClipNode { parent: None, space: PropNodeId(1), shape: ClipShape::Rect(Rect::new(0.0, 0.0, 100.0, 50.0)) },
        ),
    ];
    c.layers = vec![
        LayerDelta::Add(desc(1, None, LayerKind::Root, 1, 0, Rect::new(0.0, 0.0, 100.0, 100.0))),
        LayerDelta::Items {
            layer: LayerId(1),
            items: vec![ItemRef { node: NodeId(1), transform: PropNodeId(2), clip: Some(PropNodeId(5)), effect: None }],
        },
    ];
    c.lists = vec![(NodeId(1), quad(Rect::new(0.0, 0.0, 100.0, 100.0)))];
    tree.apply_commit(c);
    let flat = tree.flatten();
    let it = &flat[&SurfaceKey::Main][0];
    assert_eq!(it.transform.apply(Point::new(0.0, 0.0)), Point::new(0.0, -30.0));
    assert_eq!(it.bounds, Rect::new(0.0, 0.0, 100.0, 50.0), "clipped to the clip rect");
    assert_eq!(tree.set_scroll_offset(PropNodeId(3), Vec2::new(0.0, 5000.0)), Some(Vec2::new(0.0, 900.0)), "clamped");
}

#[test]
fn os_layer_subtrees_draw_to_their_own_surface_in_layer_space() {
    let mut tree = WindowTree::default();
    let mut c = commit(1);
    c.props.ops = vec![t(1, None, Affine::IDENTITY), t(2, Some(1), Affine::translate(50.0, 60.0))];
    c.layers = vec![
        LayerDelta::Add(desc(1, None, LayerKind::Root, 1, 0, Rect::new(0.0, 0.0, 400.0, 300.0))),
        LayerDelta::Add(desc(2, Some(1), LayerKind::OsLayer(SurfaceId(7)), 2, 0, Rect::new(0.0, 0.0, 200.0, 100.0))),
        LayerDelta::Items { layer: LayerId(1), items: vec![item(1, 1)] },
        LayerDelta::Items { layer: LayerId(2), items: vec![item(2, 2)] },
    ];
    c.lists = vec![(NodeId(1), quad(Rect::new(0.0, 0.0, 10.0, 10.0))), (NodeId(2), quad(Rect::new(5.0, 5.0, 15.0, 15.0)))];
    tree.apply_commit(c);
    let flat = tree.flatten();
    assert_eq!(flat[&SurfaceKey::Main].len(), 1);
    let os = &flat[&SurfaceKey::Os(SurfaceId(7))];
    assert_eq!(os.len(), 1);
    assert_eq!(os[0].bounds, Rect::new(5.0, 5.0, 15.0, 15.0), "in the OS layer's own space");
}

#[test]
fn texture_layers_draw_their_texture() {
    let mut tree = WindowTree::default();
    let mut c = commit(1);
    c.props.ops = vec![t(1, None, Affine::IDENTITY)];
    c.layers = vec![
        LayerDelta::Add(desc(1, None, LayerKind::Root, 1, 0, Rect::new(0.0, 0.0, 400.0, 300.0))),
        LayerDelta::Add(desc(
            2,
            Some(1),
            LayerKind::Texture(presto_base::ids::TextureId(3)),
            1,
            0,
            Rect::new(10.0, 10.0, 110.0, 60.0),
        )),
    ];
    tree.apply_commit(c);
    let a = tree.flatten();
    let b = tree.flatten();
    let it = &a[&SurfaceKey::Main][0];
    assert!(matches!(it.list.items[0], Prim::Surface(s) if s.texture.0 == 3));
    assert!(Arc::ptr_eq(&it.list, &b[&SurfaceKey::Main][0].list), "the synthetic list is cached");
}

#[test]
fn diff_damages_only_what_changed() {
    let mut tree = WindowTree::default();
    let mut c = commit(1);
    c.props.ops = vec![t(1, None, Affine::IDENTITY), t(2, Some(1), Affine::IDENTITY)];
    c.layers = vec![
        LayerDelta::Add(desc(1, None, LayerKind::Root, 1, 0, Rect::new(0.0, 0.0, 400.0, 300.0))),
        LayerDelta::Items { layer: LayerId(1), items: vec![item(1, 1), item(2, 2)] },
    ];
    c.lists = vec![(NodeId(1), quad(Rect::new(0.0, 0.0, 10.0, 10.0))), (NodeId(2), quad(Rect::new(100.0, 100.0, 120.0, 120.0)))];
    tree.apply_commit(c);
    let mut snap = Snapshot::default();
    let mut out = Vec::new();
    snap.diff(&tree.flatten()[&SurfaceKey::Main], &mut out);
    assert_eq!(out.len(), 2, "first frame: everything is new");

    out.clear();
    snap.diff(&tree.flatten()[&SurfaceKey::Main], &mut out);
    assert!(out.is_empty(), "nothing changed");

    let mut c2 = commit(2);
    c2.props.ops = vec![t(2, Some(1), Affine::translate(5.0, 0.0))];
    tree.apply_commit(c2);
    out.clear();
    snap.diff(&tree.flatten()[&SurfaceKey::Main], &mut out);
    assert_eq!(out, vec![Rect::new(100.0, 100.0, 120.0, 120.0), Rect::new(105.0, 100.0, 125.0, 120.0)]);

    // An equal list in a new Arc is not a change.
    let mut c3 = commit(3);
    c3.lists = vec![(NodeId(1), quad(Rect::new(0.0, 0.0, 10.0, 10.0)))];
    tree.apply_commit(c3);
    out.clear();
    snap.diff(&tree.flatten()[&SurfaceKey::Main], &mut out);
    assert!(out.is_empty());

    let mut c4 = commit(4);
    c4.removed = vec![NodeId(1)];
    tree.apply_commit(c4);
    out.clear();
    snap.diff(&tree.flatten()[&SurfaceKey::Main], &mut out);
    assert_eq!(out, vec![Rect::new(0.0, 0.0, 10.0, 10.0)]);
}

#[test]
fn device_rects_merge_and_cap() {
    let size = DeviceSize { w: 1000, h: 1000 };
    let r = damage::to_device(&[Rect::new(10.0, 10.0, 20.0, 20.0), Rect::new(15.0, 15.0, 30.0, 30.0)], Scale(2.0), size);
    assert_eq!(r, vec![DeviceRect { x: 19, y: 19, w: 42, h: 42 }]);
    let many: Vec<Rect> = (0..20).map(|i| Rect::new(i as f32 * 40.0, 0.0, i as f32 * 40.0 + 10.0, 10.0)).collect();
    let r = damage::to_device(&many, Scale(1.0), size);
    assert_eq!(r.len(), 1, "more than {} rects become one", damage::MAX_RECTS);
    assert!(damage::to_device(&[Rect::new(2000.0, 0.0, 2100.0, 10.0)], Scale(1.0), size).is_empty(), "off-surface");
}

#[test]
fn history_copies_what_older_images_missed() {
    let a = DeviceRect { x: 0, y: 0, w: 10, h: 10 };
    let b = DeviceRect { x: 100, y: 100, w: 10, h: 10 };
    let c = DeviceRect { x: 200, y: 200, w: 10, h: 10 };
    let mut h = DamageHistory::new(3);
    for _ in 0..3 {
        assert_eq!(h.copy_region(&[a]), None, "fresh images copy everything");
        h.presented(vec![a]);
    }
    h.presented(vec![b]);
    h.presented(vec![c]);
    let mut region = h.copy_region(&[a]).unwrap();
    region.sort_by_key(|r| r.x);
    assert_eq!(region, vec![a, b, c], "this frame plus the two the oldest image missed");
    h.reset();
    assert_eq!(h.copy_region(&[a]), None);
}

#[test]
fn pending_full_damage_covers_the_surface() {
    let size = DeviceSize { w: 64, h: 32 };
    let mut p = damage::Pending { rects: vec![], full: true };
    let d = p.take(size).unwrap();
    assert!(damage::covers(&d, size));
    assert!(p.take(size).is_none());
}

#[test]
fn reordered_items_are_damaged() {
    let mut tree = WindowTree::default();
    let mut c = commit(1);
    c.props.ops = vec![t(1, None, Affine::IDENTITY)];
    c.layers = vec![
        LayerDelta::Add(desc(1, None, LayerKind::Root, 1, 0, Rect::new(0.0, 0.0, 400.0, 300.0))),
        LayerDelta::Items { layer: LayerId(1), items: vec![item(1, 1), item(2, 1), item(3, 1)] },
    ];
    c.lists = vec![
        (NodeId(1), quad(Rect::new(0.0, 0.0, 10.0, 10.0))),
        (NodeId(2), quad(Rect::new(20.0, 0.0, 30.0, 10.0))),
        (NodeId(3), quad(Rect::new(40.0, 0.0, 50.0, 10.0))),
    ];
    tree.apply_commit(c);
    let mut snap = Snapshot::default();
    let mut out = Vec::new();
    snap.diff(&tree.flatten()[&SurfaceKey::Main], &mut out);
    // Node 1 moves above node 3.
    let mut c2 = commit(2);
    c2.layers = vec![LayerDelta::Items { layer: LayerId(1), items: vec![item(2, 1), item(3, 1), item(1, 1)] }];
    tree.apply_commit(c2);
    out.clear();
    snap.diff(&tree.flatten()[&SurfaceKey::Main], &mut out);
    assert!(out.contains(&Rect::new(0.0, 0.0, 10.0, 10.0)), "{out:?}");
    assert!(!out.contains(&Rect::new(20.0, 0.0, 30.0, 10.0)), "unmoved relative to the others: {out:?}");
}
