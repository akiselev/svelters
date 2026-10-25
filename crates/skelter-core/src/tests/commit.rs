//! Commit deltas: the first frame sends everything, an idle frame nothing, a paint change one list,
//! a layout move one transform, a removal its list, property nodes and layer.

use super::*;
use crate::testkinds::BOX_TINT;
use crate::Value;
use skelter_base::color::Color;
use skelter_base::layer::{LayerDelta, LayerKind};
use skelter_base::props::PropOp;

fn red_box(d: &mut Document, root: NodeId, css: &str) -> NodeId {
    child(d, root, BOX, &format!("background-color: #ff0000; {css}"))
}

#[test]
fn the_first_frame_sends_the_window_and_an_idle_frame_nothing() {
    let (mut d, root) = doc();
    let a = red_box(&mut d, root, "width: 50px; height: 20px");
    let c = frame(&mut d).expect("first frame");
    assert_eq!(c.window, W);
    assert_eq!(c.revision, d.revision());
    let roots: Vec<_> = c.layers.iter().filter(|l| matches!(l, LayerDelta::Add(desc) if desc.kind == LayerKind::Root)).collect();
    assert_eq!(roots.len(), 1, "{:?}", c.layers);
    let items: Vec<_> = c.layers.iter().filter_map(|l| if let LayerDelta::Items { items, .. } = l { Some(items) } else { None }).collect();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].iter().map(|i| i.node).collect::<Vec<_>>(), [a], "only nodes that paint are items");
    assert_eq!(c.lists.len(), 1);
    assert_eq!(c.lists[0].0, a);
    assert!(c.props.ops.iter().filter(|o| matches!(o, PropOp::Transform(..))).count() >= 3, "window, root and child transforms");
    assert!(c.a11y.is_some());
    assert!(frame(&mut d).is_none(), "nothing changed");
    assert!(d.busy().is_empty() || d.busy() == crate::Busy::COMMIT_IN_FLIGHT, "{:?}", d.busy());
}

#[test]
fn a_paint_change_sends_only_its_list() {
    let (mut d, root) = doc();
    let a = red_box(&mut d, root, "width: 50px; height: 20px");
    let _b = red_box(&mut d, root, "width: 50px; height: 20px");
    frame(&mut d);
    d.set_prop(a, BOX_TINT, Value::Color(Color::WHITE)).unwrap();
    let c = frame(&mut d).unwrap();
    assert_eq!(c.lists.iter().map(|l| l.0).collect::<Vec<_>>(), [a]);
    assert!(c.props.ops.is_empty(), "{:?}", c.props.ops);
    assert!(c.layers.is_empty(), "{:?}", c.layers);
    let a11y = c.a11y.expect("the node's a11y data is re-sent");
    assert!(a11y.tree.is_none(), "an incremental update");
}

#[test]
fn a_layout_move_is_a_transform_update_not_a_re_record() {
    let (mut d, root) = doc();
    let a = red_box(&mut d, root, "width: 50px; height: 20px");
    frame(&mut d);
    d.set_style(a, crate::StyleInput { inline: Some("background-color: #ff0000; width: 50px; height: 20px; margin-left: 10px".into()), ..Default::default() });
    let c = frame(&mut d).unwrap();
    assert!(c.lists.is_empty(), "moving does not re-record");
    assert!(c.props.ops.iter().any(|o| matches!(o, PropOp::Transform(_, t) if t.local.e == 10.0)), "{:?}", c.props.ops);
    assert_eq!(d.layout_rect(a).unwrap().min.x, 10.0);
}

#[test]
fn a_property_prop_touches_no_list() {
    let (mut d, root) = doc();
    let a = red_box(&mut d, root, "width: 50px; height: 20px");
    frame(&mut d);
    d.set_prop(a, crate::testkinds::BOX_RAISE, Value::I64(2)).unwrap();
    let (c, stats) = d.frame_with_stats(W, Time(0));
    assert_eq!(stats.recorded, 0);
    assert!(!stats.layout.full);
    let c = c.unwrap();
    assert!(c.lists.is_empty());
    assert!(!c.props.ops.is_empty());
}

#[test]
fn removing_a_node_removes_its_list_and_property_nodes() {
    let (mut d, root) = doc();
    let a = red_box(&mut d, root, "width: 50px; height: 20px; opacity: 0.5");
    frame(&mut d);
    d.destroy(a);
    let c = frame(&mut d).unwrap();
    assert_eq!(c.removed, [a]);
    let removed = c.props.ops.iter().filter(|o| matches!(o, PropOp::Remove(_))).count();
    assert_eq!(removed, 2, "its transform and effect nodes: {:?}", c.props.ops);
    let items: Vec<_> = c.layers.iter().filter_map(|l| if let LayerDelta::Items { items, .. } = l { Some(items) } else { None }).collect();
    assert_eq!(items.len(), 1);
    assert!(items[0].is_empty());
}

#[test]
fn translucent_clipping_and_scrolling_nodes_get_their_property_nodes() {
    let (mut d, root) = doc();
    let s = child(&mut d, root, BOX, "width: 100px; height: 50px; overflow-y: scroll");
    let _inner = child(&mut d, s, BOX, "height: 200px; flex-shrink: 0; background-color: #00ff00");
    let o = red_box(&mut d, root, "width: 10px; height: 10px; opacity: 0.5");
    let c = frame(&mut d).unwrap();
    assert!(c.props.ops.iter().any(|op| matches!(op, PropOp::Scroll(_, n) if n.scroll_y && !n.scroll_x && n.content.h == 200.0 && n.viewport.height() == 50.0)), "{:?}", c.props.ops);
    assert!(c.props.ops.iter().any(|op| matches!(op, PropOp::Clip(..))));
    assert!(c.props.ops.iter().any(|op| matches!(op, PropOp::Effect(_, e) if e.opacity == 0.5)));
    assert!(c.layers.iter().any(|l| matches!(l, LayerDelta::Add(desc) if desc.kind == LayerKind::Scroll { tiles: false })), "a scroll container is a layer");
    let _ = o;
}

#[test]
fn wheel_scrolling_is_native_state_and_a_property_change() {
    let (mut d, root) = doc();
    let s = child(&mut d, root, BOX, "width: 100px; height: 50px; overflow-y: scroll");
    let _inner = child(&mut d, s, BOX, "height: 200px; flex-shrink: 0; background-color: #00ff00");
    frame(&mut d);
    let wheel = skelter_base::event::WheelEvent {
        pos: Point::new(10.0, 10.0),
        delta: skelter_base::event::ScrollDelta::Pixels(Vec2::new(0.0, -30.0)),
        phase: None,
        momentum: None,
        modifiers: Default::default(),
    };
    let out = d.dispatch(&skelter_base::event::InputEnvelope { seq: skelter_base::ids::InputSeq(99), window: W, os_time: Time(0), recv_time: Time(0), event: skelter_base::event::InputEvent::Wheel(wheel) });
    assert!(out.handled && out.changed);
    assert_eq!(d.scroll_offset(s), Vec2::new(0.0, 30.0));
    let (c, stats) = d.frame_with_stats(W, Time(0));
    assert_eq!(stats.recorded, 0, "scrolling re-records nothing");
    let c = c.unwrap();
    assert!(c.props.ops.iter().any(|op| matches!(op, PropOp::Scroll(_, n) if n.offset.y == 30.0)));
    assert_eq!(c.input_seq, skelter_base::ids::InputSeq(99));
    // the offset clamps to the content
    d.set_scroll_offset(s, Vec2::new(0.0, 1000.0));
    assert_eq!(d.scroll_offset(s), Vec2::new(0.0, 150.0));
}

#[test]
fn a_transition_starts_a_compositor_animation() {
    let (mut d, root) = doc();
    let a = red_box(&mut d, root, "width: 10px; height: 10px; transition: opacity 200ms");
    frame(&mut d);
    d.set_style(a, crate::StyleInput { inline: Some("background-color: #ff0000; width: 10px; height: 10px; transition: opacity 200ms; opacity: 0.25".into()), ..Default::default() });
    let c = frame(&mut d).unwrap();
    let start = c.animations.iter().find_map(|a| if let skelter_base::anim::AnimationCmd::Start(s) = a { Some(s) } else { None }).expect("a transition");
    assert_eq!(start.node, a);
    assert_eq!(start.property, skelter_base::anim::AnimProperty::Opacity);
    assert!(start.target.is_some(), "it drives the node's effect node");
    assert!(d.busy().contains(crate::Busy::ANIMATING));
    d.animation_event(skelter_base::anim::AnimationEvent::Finished { id: start.id, at: Time(1) });
    assert!(!d.busy().contains(crate::Busy::ANIMATING));
}

#[test]
fn commits_in_flight_count_as_busy_until_applied() {
    let (mut d, root) = doc();
    red_box(&mut d, root, "width: 10px; height: 10px");
    let c = frame(&mut d).unwrap();
    assert!(d.busy().contains(crate::Busy::COMMIT_IN_FLIGHT));
    d.commit_applied(W, c.revision);
    assert!(d.busy().is_empty(), "{:?}", d.busy());
}

#[test]
fn a_nonpassive_listener_inside_a_scroller_makes_it_wait_for_js() {
    use crate::{ListenFlags, NodeFlags, common};
    let (mut d, root) = doc();
    let s = child(&mut d, root, BOX, "width: 100px; height: 50px; overflow-y: scroll");
    let mid = child(&mut d, s, BOX, "height: 200px; flex-shrink: 0");
    let leaf = child(&mut d, mid, BOX, "height: 10px");
    let passive = |c: &skelter_base::commit::Commit| c.props.ops.iter().find_map(|op| if let PropOp::Scroll(_, n) = op { Some(n.passive) } else { None });
    assert_eq!(passive(&frame(&mut d).unwrap()), Some(true));
    d.listen(leaf, common::WHEEL, ListenFlags::EMPTY);
    assert!(d.flags(mid).unwrap().contains(NodeFlags::DESC_NONPASSIVE));
    assert!(d.flags(s).unwrap().contains(NodeFlags::DESC_NONPASSIVE));
    assert_eq!(passive(&frame(&mut d).unwrap()), Some(false));
    // a passive listener does not count; removing the other clears the bits up the chain
    d.listen(leaf, common::WHEEL, ListenFlags::PASSIVE);
    assert!(!d.flags(s).unwrap().contains(NodeFlags::DESC_NONPASSIVE));
    assert_eq!(passive(&frame(&mut d).unwrap()), Some(true));
    // moving a non-passive subtree in and out updates its new and old ancestors
    d.listen(leaf, common::WHEEL, ListenFlags::EMPTY);
    d.remove(mid, leaf).unwrap();
    assert!(!d.flags(s).unwrap().contains(NodeFlags::DESC_NONPASSIVE));
    d.insert(mid, leaf, None, None).unwrap();
    assert!(d.flags(s).unwrap().contains(NodeFlags::DESC_NONPASSIVE));
}
