//! The top layer: a node whose layout style is `position: fixed` is laid out against the window,
//! placed where its kind says ([`Kind::top_layer`]), drawn with its subtree in a layer of the window's
//! overlay stack (no ancestor's clip, above later siblings), hit before the content, closed by Escape
//! and outside presses in the stack's order, removed with its owner, and kept under its owner in the
//! accessibility tree.

use super::*;
use crate::cx::{EventCx, RecordCx, StyleCx, TopLayerCx};
use crate::kind::{Kind, KindSpec, PropSpec};
use crate::overlay::{Dismiss, DismissReason, OverlayClass, TopLayer};
use crate::{KindRegistry, PropId, Value, ValueType};
use accesskit::NodeId as AxId;
use skelter_base::change::ChangeClass;
use skelter_base::geom::Rect;
use skelter_base::layer::{LayerDelta, LayerKind};
use std::cell::RefCell;
use std::sync::Arc;

const FLOATER: KindId = KindId::of("floater");
const OPEN: PropId = PropId::of("floater", "open", "bool");

static FLOATER_SPEC: KindSpec = KindSpec {
    props: &[PropSpec::new("open", ValueType::Bool, ChangeClass::Structure).native()],
    container: true,
    ..KindSpec::new("floater")
};

thread_local! {
    /// Why the stack closed each floater, in order.
    static CLOSED: RefCell<Vec<(NodeId, DismissReason)>> = const { RefCell::new(Vec::new()) };
    /// The floaters ask to close (their anchor went away).
    static ASK_CLOSE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// A floating surface: while `open` it is in the top layer, below its owner's box, light-dismissed.
#[derive(Default)]
struct Floater;

impl Kind for Floater {
    fn spec() -> &'static KindSpec {
        &FLOATER_SPEC
    }
    fn layout_style(&self, cx: &StyleCx) -> taffy::Style {
        let mut s = cx.default_layout();
        if cx.prop_bool(OPEN) {
            s.position = taffy::Position::Fixed;
        }
        s
    }
    fn top_layer(&self, cx: &TopLayerCx) -> TopLayer {
        let owner = cx.owner_rect().unwrap_or(Rect::ZERO);
        let tl = TopLayer::new(OverlayClass::Popup)
            .at(Point::new(owner.min.x, owner.max.y))
            .with_dismiss(Dismiss::LIGHT)
            .with_inside(vec![owner]);
        if ASK_CLOSE.with(|a| a.get()) { tl.closing(DismissReason::AnchorHidden) } else { tl }
    }
    fn record(&self, cx: &mut RecordCx) {
        cx.draw_box();
    }
    fn overlay_closed(&mut self, cx: &mut EventCx, reason: DismissReason) -> crate::Handled {
        CLOSED.with(|c| c.borrow_mut().push((cx.node(), reason)));
        let _ = cx.set_native(OPEN, Value::Bool(false));
        crate::Handled::Yes
    }
}

fn tdoc() -> (Document, NodeId) {
    CLOSED.with(|c| c.borrow_mut().clear());
    ASK_CLOSE.with(|a| a.set(false));
    let mut r = KindRegistry::new();
    r.register::<testkinds::BoxKind>().unwrap();
    r.register::<testkinds::ToggleKind>().unwrap();
    r.register::<Floater>().unwrap();
    let mut d = Document::new(Arc::new(r), Box::new(FixedText));
    d.set_text_shaper(Box::new(FixedText));
    let root = d.create_window(W, BOX, Size::new(200.0, 100.0), Scale(1.0));
    (d, root)
}

fn closed() -> Vec<(NodeId, DismissReason)> {
    CLOSED.with(|c| c.borrow().clone())
}

fn open(d: &mut Document, n: NodeId, on: bool) {
    d.set_prop(n, OPEN, Value::Bool(on)).unwrap();
}

fn is_open(d: &Document, n: NodeId) -> bool {
    d.get_prop(n, OPEN) == Some(Value::Bool(true))
}

#[test]
fn a_floater_in_a_clipped_box_draws_whole_above_a_later_sibling_and_takes_the_press() {
    let (mut d, root) = tdoc();
    // a 100×40 clipping box at the top left, holding the owner (40×20) and its floater (80×50)
    let clipper = child(
        &mut d,
        root,
        BOX,
        "position: absolute; left: 0px; top: 0px; width: 100px; height: 40px; overflow: hidden",
    );
    let owner = child(&mut d, clipper, BOX, "width: 40px; height: 20px");
    let float = child(
        &mut d,
        owner,
        FLOATER,
        "width: 80px; height: 50px; padding-top: 25px; background-color: white",
    );
    let button = child(&mut d, float, TOGGLE, "background-color: blue");
    // a later sibling covering the whole window
    let cover = child(
        &mut d,
        root,
        BOX,
        "position: absolute; left: 0px; top: 0px; width: 200px; height: 100px; background-color: red",
    );
    frame(&mut d);
    open(&mut d, float, true);
    let c = frame(&mut d).expect("the open's commit");

    // laid out against the window, placed below its owner, out of the clipping box's flow
    assert_eq!(d.world_rect(float), Some(Rect::new(0.0, 20.0, 80.0, 70.0)));
    assert_eq!(
        d.world_rect(button),
        Some(Rect::new(0.0, 45.0, 80.0, 61.0)),
        "stretched across the column"
    );
    assert_eq!(
        d.world_rect(owner),
        Some(Rect::new(0.0, 0.0, 40.0, 20.0)),
        "the owner keeps its box"
    );

    // its subtree is the items of one overlay layer above every content layer, under no clip
    let adds: Vec<_> = c
        .layers
        .iter()
        .filter_map(|l| {
            if let LayerDelta::Add(d) = l
                && d.kind == LayerKind::Overlay
            {
                Some(*d)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(adds.len(), 1, "{:?}", c.layers);
    let layer = adds[0];
    assert!(
        layer.z >= crate::toplayer::OVERLAY_Z,
        "above the content and the cover"
    );
    assert_eq!(layer.clip, None);
    let add_at = c
        .layers
        .iter()
        .position(|l| matches!(l, LayerDelta::Add(x) if x.id == layer.id))
        .unwrap();
    let items_at = c
        .layers
        .iter()
        .position(|l| matches!(l, LayerDelta::Items { layer: id, .. } if *id == layer.id))
        .expect("its items");
    assert!(add_at < items_at, "the layer is added before its items");
    let LayerDelta::Items { items, .. } = &c.layers[items_at] else {
        unreachable!()
    };
    assert_eq!(
        items.iter().map(|i| i.node).collect::<Vec<_>>(),
        vec![float, button]
    );
    assert!(
        items.iter().all(|i| i.clip.is_none()),
        "the clipping box does not clip it: {items:?}"
    );
    let st = d.overlay_stack(W).expect("the stack");
    assert_eq!((st.entries().len(), st.entries()[0].spec.node), (1, float));

    // a press outside the clipping box, over the cover, reaches the button inside the floater
    assert_eq!(d.hit_test(W, Point::new(5.0, 50.0)), Some(button));
    assert_eq!(
        d.hit_test(W, Point::new(60.0, 30.0)),
        Some(float),
        "its padding, inside the clipping box"
    );
    assert_eq!(
        d.hit_test(W, Point::new(150.0, 50.0)),
        Some(cover),
        "beside it, the content"
    );
    click(&mut d, 5.0, 50.0);
    assert_eq!(
        d.get_prop(button, testkinds::TOGGLE_CHECKED),
        Some(Value::Bool(true)),
        "the click toggled it"
    );
    assert!(is_open(&d, float), "a press inside keeps it open");

    // closed, it goes back into its owner's flow and its layer goes
    open(&mut d, float, false);
    let c = frame(&mut d).expect("the close's commit");
    assert!(
        c.layers.contains(&LayerDelta::Remove(layer.id)),
        "{:?}",
        c.layers
    );
    assert!(d.overlay_stack(W).unwrap().is_empty());
    assert_eq!(
        d.world_rect(float).map(|r| r.min),
        Some(Point::new(0.0, 0.0)),
        "in its owner's flow again"
    );
    assert_eq!(d.hit_test(W, Point::new(5.0, 50.0)), Some(cover));
}

#[test]
fn escape_closes_one_level_and_an_outside_press_closes_the_chain_children_first() {
    let (mut d, root) = tdoc();
    let owner = child(
        &mut d,
        root,
        BOX,
        "position: absolute; left: 10px; top: 10px; width: 40px; height: 20px",
    );
    let outer = child(&mut d, owner, FLOATER, "width: 60px; height: 40px");
    let inner = child(&mut d, outer, FLOATER, "width: 30px; height: 20px");
    frame(&mut d);
    open(&mut d, outer, true);
    open(&mut d, inner, true);
    frame(&mut d);
    // the inner one opens from the outer one, above it
    let st = d.overlay_stack(W).unwrap();
    assert_eq!(
        st.entries().iter().map(|e| e.spec.node).collect::<Vec<_>>(),
        vec![outer, inner]
    );
    assert_eq!(st.entries()[1].spec.parent, Some(st.entries()[0].id));
    assert_eq!(
        d.world_rect(outer),
        Some(Rect::new(10.0, 30.0, 70.0, 70.0)),
        "below its owner"
    );
    assert_eq!(
        d.world_rect(inner),
        Some(Rect::new(10.0, 70.0, 40.0, 90.0)),
        "below the outer one"
    );

    // Escape: the inner one, then the outer one
    d.dispatch(&key(Key::Escape, Modifiers::default()));
    assert_eq!(closed(), vec![(inner, DismissReason::Escape)]);
    assert!(is_open(&d, outer) && !is_open(&d, inner));
    frame(&mut d);
    assert_eq!(d.overlay_stack(W).unwrap().entries().len(), 1);
    d.dispatch(&key(Key::Escape, Modifiers::default()));
    assert_eq!(closed().last(), Some(&(outer, DismissReason::Escape)));
    frame(&mut d);
    assert!(d.overlay_stack(W).unwrap().is_empty());

    // both open again: a press inside the inner one keeps both, a press outside closes the inner one
    // first, then the outer one
    open(&mut d, outer, true);
    open(&mut d, inner, true);
    frame(&mut d);
    CLOSED.with(|c| c.borrow_mut().clear());
    click(&mut d, 20.0, 80.0);
    assert!(closed().is_empty() && is_open(&d, outer) && is_open(&d, inner));
    // a press on the owner (the outer one's anchor) keeps the outer one; the inner one is outside
    click(&mut d, 20.0, 15.0);
    assert_eq!(closed(), vec![(inner, DismissReason::OutsidePress)]);
    open(&mut d, inner, true);
    frame(&mut d);
    CLOSED.with(|c| c.borrow_mut().clear());
    click(&mut d, 190.0, 95.0);
    assert_eq!(
        closed(),
        vec![
            (inner, DismissReason::OutsidePress),
            (outer, DismissReason::OutsidePress)
        ]
    );
    frame(&mut d);
    assert!(d.overlay_stack(W).unwrap().is_empty());
}

#[test]
fn it_goes_with_its_owner_and_stays_under_it_in_the_accessibility_tree() {
    let (mut d, root) = tdoc();
    let clipper = child(
        &mut d,
        root,
        BOX,
        "position: absolute; left: 0px; top: 0px; width: 100px; height: 30px; overflow: hidden",
    );
    let owner = child(&mut d, clipper, BOX, "width: 40px; height: 20px");
    let float = child(&mut d, owner, FLOATER, "width: 80px; height: 50px");
    let button = child(&mut d, float, TOGGLE, "");
    open(&mut d, float, true);
    let u = frame(&mut d).unwrap().a11y.unwrap();
    let node = |id: NodeId| {
        &u.nodes
            .iter()
            .find(|(k, _)| *k == AxId(id.0))
            .expect("in the tree")
            .1
    };
    assert_eq!(
        node(owner).children(),
        &[AxId(float.0)],
        "the owner is its parent"
    );
    assert_eq!(node(float).children(), &[AxId(button.0)]);
    // its bounds are its window box relative to the owner's
    let b = node(float).bounds().unwrap();
    assert_eq!((b.x0, b.y0, b.x1, b.y1), (0.0, 0.0, 80.0, 50.0));
    let t = node(float).transform().map(|t| t.as_coeffs());
    assert_eq!(t, Some([1.0, 0.0, 0.0, 1.0, 0.0, 20.0]), "below the owner");

    // removing the owner takes it, its entry and its layer
    let layer = d.overlay_stack(W).unwrap().entries()[0].spec.layer;
    d.remove(clipper, owner).unwrap();
    let c = frame(&mut d).expect("the removal");
    assert!(d.overlay_stack(W).unwrap().is_empty());
    assert!(
        c.layers.contains(&LayerDelta::Remove(layer)),
        "{:?}",
        c.layers
    );
    assert_eq!(d.hit_test(W, Point::new(10.0, 40.0)), Some(root));
}

#[test]
fn a_top_layer_node_that_asks_to_close_is_closed_after_the_frame() {
    let (mut d, root) = tdoc();
    let owner = child(&mut d, root, BOX, "width: 40px; height: 20px");
    let float = child(&mut d, owner, FLOATER, "width: 80px; height: 50px");
    open(&mut d, float, true);
    frame(&mut d);
    assert!(is_open(&d, float));
    assert!(closed().is_empty());
    // its anchor goes away: the next rebuild's `top_layer` asks to close
    ASK_CLOSE.with(|a| a.set(true));
    child(&mut d, root, BOX, "width: 10px; height: 10px");
    frame(&mut d);
    assert_eq!(closed(), vec![(float, DismissReason::AnchorHidden)], "the kind heard why");
    assert!(!is_open(&d, float), "it wrote `open` back");
    ASK_CLOSE.with(|a| a.set(false));
    frame(&mut d);
    assert_eq!(d.world_rect(float).map(|r| r.min), Some(Point::ZERO), "back in its owner's flow (at its origin), not below it");
    assert_eq!(closed().len(), 1, "closed once");
}
