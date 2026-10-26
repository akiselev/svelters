//! Commit deltas: the first frame sends everything, an idle frame nothing, a paint change one list,
//! a layout move one transform, a removal its list, property nodes and layer.

use super::*;
use crate::testkinds::BOX_TINT;
use crate::{KindId, Value};
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
fn an_icon_colour_transition_runs_on_the_compositor() {
    use skelter_base::anim::{AnimProperty, AnimValue, AnimationCmd, Generator};
    let (mut d, root) = doc();
    let css = "width: 16px; height: 16px; transition: color 200ms";
    let a = child(&mut d, root, KindId::of("icon"), &format!("{css}; color: #ff0000"));
    let b = child(&mut d, root, BOX, &format!("{css}; color: #ff0000"));
    frame(&mut d);
    // hover: the colour changes on both
    d.set_style(a, crate::StyleInput { inline: Some(format!("{css}; color: #0000ff").into()), ..Default::default() });
    d.set_style(b, crate::StyleInput { inline: Some(format!("{css}; color: #0000ff").into()), ..Default::default() });
    let c = frame(&mut d).unwrap();
    let starts: Vec<_> = c.animations.iter().filter_map(|x| if let AnimationCmd::Start(s) = x { Some(s) } else { None }).collect();
    assert_eq!(starts.len(), 1, "only the icon's colour goes to the compositor: {starts:?}");
    let s = starts[0];
    assert_eq!((s.node, s.property), (a, AnimProperty::Palette(0)));
    let effect = s.target.expect("it drives the icon's effect node");
    assert!(c.props.ops.iter().any(|op| matches!(op, PropOp::Effect(id, _) if *id == effect)), "the effect node is made in the same commit");
    let Generator::Keyframes { keyframes, .. } = &s.generator else { panic!() };
    assert_eq!(keyframes[0].value, AnimValue::Color(Color::from_rgba8(255, 0, 0, 255)));
    assert_eq!(keyframes[1].value, AnimValue::Color(Color::from_rgba8(0, 0, 255, 255)));
    // the icon's list is recorded once, at the end colour
    let list = c.lists.iter().find(|(n, _)| *n == a).expect("the icon re-records once").1.clone();
    let skelter_base::display::Prim::Icon(i) = &list.items[0] else { panic!() };
    assert_eq!(i.color, Some(Color::from_rgba8(0, 0, 255, 255)));
    // during the transition: the box's colour is the UI thread's per frame, the icon's is not
    let mut icon_commits = 0;
    for t in 1..20u64 {
        if let Some(c) = d.frame(W, Time(t * 10_000_000)) {
            icon_commits += c.lists.iter().filter(|(n, _)| *n == a).count();
        }
    }
    assert_eq!(icon_commits, 0, "no UI commit for the icon during its transition");
    // the box's UI transition ends with its last frame, the icon's with the compositor's event
    frame_at(&mut d, 300);
    assert!(d.busy().contains(crate::Busy::ANIMATING));
    d.animation_event(skelter_base::anim::AnimationEvent::Finished { id: s.id, at: Time(1) });
    assert!(!d.busy().contains(crate::Busy::ANIMATING));
}

fn frame_at(d: &mut Document, ms: u64) {
    d.frame(W, Time(ms * 1_000_000));
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

mod external_layer {
    use super::*;
    use crate::KindRegistry;
    use crate::cx::{MeasureCx, RecordCx};
    use crate::kind::{Kind, KindSpec};
    use skelter_base::geom::Rect;
    use skelter_base::ids::{LayerId, TextureId};
    use skelter_base::layer::LayerDesc;
    use std::collections::HashMap;
    use std::sync::Arc;

    const VIEWPORT: KindId = KindId::of("viewport");
    static VIEWPORT_SPEC: KindSpec = KindSpec { container: true, ..KindSpec::new("viewport") };
    const TEX: TextureId = TextureId(7);

    /// A 3D viewport: its box, then an offscreen texture over it.
    #[derive(Default)]
    struct Viewport;

    impl Kind for Viewport {
        fn spec() -> &'static KindSpec {
            &VIEWPORT_SPEC
        }
        fn measure(&self, _cx: &mut MeasureCx, _k: taffy::Size<Option<f32>>, _a: taffy::Size<taffy::AvailableSpace>) -> taffy::Size<f32> {
            taffy::Size { width: 0.0, height: 0.0 }
        }
        fn record(&self, cx: &mut RecordCx) {
            cx.draw_box();
            cx.external_layer(LayerKind::Texture(TEX), Rect::new(0.0, 0.0, 80.0, 40.0));
        }
    }

    fn vdoc() -> (Document, NodeId) {
        let mut r = KindRegistry::new();
        r.register::<crate::testkinds::BoxKind>().unwrap();
        r.register::<Viewport>().unwrap();
        let mut d = Document::new(Arc::new(r), Box::new(crate::testkinds::FixedText));
        let root = d.create_window(W, BOX, Size::new(200.0, 100.0), Scale(1.0));
        (d, root)
    }

    /// The layers and items the commits sent so far, applied in order.
    #[derive(Default)]
    struct Applied {
        layers: HashMap<LayerId, LayerDesc>,
        items: HashMap<LayerId, Vec<NodeId>>,
    }

    impl Applied {
        fn apply(&mut self, c: &skelter_base::commit::Commit) {
            for l in &c.layers {
                match l {
                    LayerDelta::Add(desc) | LayerDelta::Update(desc) => {
                        self.layers.insert(desc.id, *desc);
                    }
                    LayerDelta::Remove(id) => {
                        self.layers.remove(id);
                        self.items.remove(id);
                    }
                    LayerDelta::Items { layer, items } => {
                        self.items.insert(*layer, items.iter().map(|i| i.node).collect());
                    }
                    LayerDelta::Damage { .. } => {}
                }
            }
        }
        /// Paint order as the compositor flattens it: a layer's items, then its children by z.
        fn order(&self) -> Vec<String> {
            let root = self.layers.values().find(|l| l.parent.is_none()).expect("root").id;
            let mut out = Vec::new();
            self.walk(root, &mut out);
            out
        }
        fn walk(&self, id: LayerId, out: &mut Vec<String>) {
            let desc = self.layers[&id];
            if let LayerKind::Texture(t) = desc.kind {
                out.push(format!("texture {}", t.0));
            }
            for n in self.items.get(&id).into_iter().flatten() {
                out.push(format!("node {}", n.0));
            }
            let mut kids: Vec<_> = self.layers.values().filter(|l| l.parent == Some(id)).collect();
            kids.sort_by_key(|l| l.z);
            for k in kids {
                self.walk(k.id, out);
            }
        }
    }

    #[test]
    fn an_external_layer_sits_between_what_paints_before_and_after_it() {
        let (mut d, root) = vdoc();
        let red = "background-color: #ff0000; width: 80px; height: 20px";
        let below = child(&mut d, root, BOX, red);
        let view = child(&mut d, root, VIEWPORT, "background-color: #00ff00; width: 80px; height: 40px");
        let overlay = child(&mut d, view, BOX, red);
        let above = child(&mut d, root, BOX, red);
        let mut a = Applied::default();
        a.apply(&frame(&mut d).unwrap());
        let n = |id: NodeId| format!("node {}", id.0);
        assert_eq!(a.order(), [n(below), n(view), "texture 7".into(), n(overlay), n(above)]);
        let tex = a.layers.values().find(|l| l.kind == LayerKind::Texture(TEX)).copied().unwrap();
        assert_eq!(tex.bounds, Rect::new(0.0, 0.0, 80.0, 40.0));
        assert_eq!(a.layers.len(), 3, "root, the texture and the continuation: {:?}", a.layers);
        // an idle frame changes nothing; removing the viewport removes both layers
        assert!(frame(&mut d).is_none());
        d.destroy(view);
        let c = frame(&mut d).unwrap();
        assert_eq!(c.layers.iter().filter(|l| matches!(l, LayerDelta::Remove(_))).count(), 2, "{:?}", c.layers);
        a.apply(&c);
        assert_eq!(a.layers.len(), 1);
        assert_eq!(a.order(), [n(below), n(above)]);
    }

    /// A26: a will-change layer gets the same continuation: later siblings paint above it.
    #[test]
    fn a_will_change_layer_keeps_later_siblings_above_it() {
        let (mut d, root) = vdoc();
        let red = "background-color: #ff0000; width: 80px; height: 20px";
        let below = child(&mut d, root, BOX, red);
        let lifted = child(&mut d, root, BOX, "will-change: transform; background-color: #00ff00; width: 80px; height: 40px");
        let inside = child(&mut d, lifted, BOX, red);
        let above = child(&mut d, root, BOX, red);
        let mut a = Applied::default();
        a.apply(&frame(&mut d).unwrap());
        let n = |id: NodeId| format!("node {}", id.0);
        assert_eq!(a.order(), [n(below), n(lifted), n(inside), n(above)]);
        assert_eq!(a.layers.len(), 3, "root, the node's layer and the continuation: {:?}", a.layers);
        // dropping will-change drops both layers; everything is back in the root layer
        d.set_style(lifted, crate::StyleInput { inline: Some("background-color: #00ff00; width: 80px; height: 40px".into()), ..Default::default() });
        while let Some(c) = frame(&mut d) {
            a.apply(&c);
        }
        assert_eq!(a.layers.len(), 1, "{:?}", a.layers);
        assert_eq!(a.order(), [n(below), n(lifted), n(inside), n(above)]);
    }

    /// A28: a window overlay made of nodes: an absolute child of the root with the highest z-index
    /// paints above every content layer (an external layer, a will-change layer, content added
    /// later), wherever it sits in tree order, and keeps its anchor for queries and AccessKit.
    #[test]
    fn a_top_z_index_root_child_is_a_window_overlay_that_keeps_its_anchor() {
        let (mut d, root) = vdoc();
        let red = "background-color: #ff0000; width: 80px; height: 20px";
        let grip = child(&mut d, root, BOX, "position: absolute; z-index: 1000; left: 10px; top: 10px; width: 6px; height: 6px; background-color: #0000ff");
        d.set_prop(grip, crate::common::ANCHOR, Value::from("designer.grip")).unwrap();
        let below = child(&mut d, root, BOX, red);
        let view = child(&mut d, root, VIEWPORT, "background-color: #00ff00; width: 80px; height: 40px");
        let lifted = child(&mut d, root, BOX, "will-change: transform; background-color: #00ff00; width: 80px; height: 20px");
        let mut a = Applied::default();
        let c = frame(&mut d).unwrap();
        a.apply(&c);
        let n = |id: NodeId| format!("node {}", id.0);
        assert_eq!(a.order(), [n(below), n(view), "texture 7".into(), n(lifted), n(grip)]);
        assert_eq!(d.find_anchor(W, "designer.grip"), Some(grip));
        let ax = c.a11y.expect("the first frame's tree");
        let node = ax.nodes.iter().find(|(id, _)| id.0 == grip.0).map(|(_, n)| n).expect("the grip's AccessKit node");
        assert_eq!(node.author_id(), Some("designer.grip"));
        assert_eq!(d.hit_test(W, Point::new(12.0, 12.0)), Some(grip), "it takes input above the content it covers");
        // content added later still paints below it
        let late = child(&mut d, root, BOX, red);
        while let Some(c) = frame(&mut d) {
            a.apply(&c);
        }
        assert_eq!(a.order().last(), Some(&n(grip)), "{:?} (late: {})", a.order(), n(late));
    }
}
