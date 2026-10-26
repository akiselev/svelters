//! The runner's entries: `needs_frame`, event-time hover, AccessKit actions, window events, device
//! reset, a full accessibility tree on demand.

use super::*;
use crate::{ListenFlags, StateMask, common};
use skelter_base::event::{Theme, WindowEvent};

#[test]
fn a_hover_over_unstyled_nodes_needs_no_frame() {
    let (mut d, root) = doc();
    child(&mut d, root, BOX, "width: 20px; height: 20px");
    assert!(d.needs_frame(W), "a new window");
    frame(&mut d);
    assert!(!d.needs_frame(W));
    d.dispatch(&pointer(PointerPhase::Move, 5.0, 5.0, 10));
    assert!(!d.needs_frame(W), "no rule tests :hover");
    d.add_sheet("box:hover { background-color: #ff0000 }");
    frame(&mut d);
    d.dispatch(&pointer(PointerPhase::Move, 150.0, 50.0, 20));
    assert!(d.needs_frame(W), "now the hover restyles");
}

#[test]
fn hover_updates_at_event_time() {
    let (mut d, root) = doc();
    let a = child(&mut d, root, BOX, "width: 20px; height: 20px");
    d.listen(a, common::POINTERENTER, ListenFlags::EMPTY);
    frame(&mut d);
    assert!(d.hover(W, Point::new(5.0, 5.0)));
    assert!(d.state(a).contains(StateMask::HOVERED));
    assert!(d.take_events().iter().any(|e| e.signal == common::POINTERENTER && e.target == a));
    assert!(!d.hover(W, Point::new(6.0, 5.0)), "same path");
}

#[test]
fn accessibility_actions_reach_the_node_with_its_id() {
    let (mut d, root) = doc();
    let t = child(&mut d, root, TOGGLE, "");
    frame(&mut d);
    let req = accesskit::ActionRequest { action: accesskit::Action::Focus, target_tree: accesskit::TreeId::ROOT, target_node: accesskit::NodeId(t.0), data: None };
    assert!(d.accessibility_action(W, &req));
    assert_eq!(d.focused(W), Some(t));
}

#[test]
fn window_events_reach_the_tree() {
    let (mut d, root) = doc();
    d.add_sheet("@media (prefers-color-scheme: dark) { box { width: 77px } }");
    let a = child(&mut d, root, BOX, "");
    frame(&mut d);
    d.window_event(W, &WindowEvent::ThemeChanged(Theme::Dark));
    frame(&mut d);
    assert_eq!(d.layout_rect(a).unwrap().width(), 77.0);
    d.window_event(W, &WindowEvent::Focused(false));
    assert!(d.state(root).contains(StateMask::BACKDROP));
    d.window_event(W, &WindowEvent::Resized { size: skelter_base::geom::DeviceSize { w: 400, h: 200 }, scale: Scale(2.0) });
    frame(&mut d);
    assert_eq!(d.layout_rect(root).unwrap().width(), 200.0, "points = pixels / scale");
}

#[test]
fn a_device_reset_resends_everything() {
    let (mut d, root) = doc();
    child(&mut d, root, BOX, "width: 20px; height: 20px; background-color: #00ff00");
    frame(&mut d);
    assert!(frame(&mut d).is_none());
    d.device_reset();
    let c = frame(&mut d).expect("a full commit");
    assert!(!c.lists.is_empty() && !c.props.ops.is_empty());
    assert!(c.layers.iter().any(|l| matches!(l, skelter_base::layer::LayerDelta::Add(_))));
}

#[test]
fn a_full_tree_on_demand() {
    let (mut d, root) = doc();
    child(&mut d, root, BOX, "");
    frame(&mut d);
    d.a11y_full_tree(W);
    let c = frame(&mut d).expect("the tree");
    assert!(c.a11y.unwrap().tree.is_some());
}
