//! Each window's top layer: overlay parts in the window's overlay stack ([`RecordCx::overlay`]),
//! drawn above the content and hit before it, light dismiss and Escape; the app's overlay list
//! ([`Document::set_window_overlay`]); the drag layer showing a drag's [`DragPreview`] at the pointer.

use super::*;
use crate::cx::{EventCx, RecordCx};
use crate::kind::{DragPreview, Event, Handled, Kind, KindSpec};
use crate::overlay::{DismissReason, OverlayClass, OverlayPart};
use crate::{KindRegistry, Value, common};
use skelter_base::color::{Color, Fill};
use skelter_base::commit::{Commit, FastCommit};
use skelter_base::display::{BorderStyle, DisplayList, Prim, Quad};
use skelter_base::geom::{Affine, Corners, Insets, Rect};
use skelter_base::layer::{LayerDelta, LayerKind};
use skelter_base::props::PropOp;
use std::sync::Arc;

const POPPER: KindId = KindId::of("popper");
static POPPER_SPEC: KindSpec = KindSpec::new("popper");

/// Where the popper's popup sits, in its local points: below its 40×30 box.
const POPUP: Rect = Rect { min: Point { x: 0.0, y: 40.0 }, max: Point { x: 80.0, y: 90.0 } };

fn quad(r: Rect) -> Prim {
    Prim::Quad(Quad { rect: r, radii: Corners::default(), fill: Fill::Solid(Color::BLACK), border_widths: Insets::ZERO, border_color: Color::BLACK, border_style: BorderStyle::Solid })
}

/// A drop button: a click on its box opens its popup (an overlay part); it logs the presses it
/// gets (local points) and why the stack closed the popup.
#[derive(Default)]
struct Popper {
    open: bool,
    presses: Vec<Point>,
    closed: Vec<DismissReason>,
}

impl Kind for Popper {
    fn spec() -> &'static KindSpec {
        &POPPER_SPEC
    }
    fn record(&self, cx: &mut RecordCx) {
        cx.draw_box();
        if self.open {
            cx.overlay(OverlayPart::new(OverlayClass::Popup, POPUP), |cx| cx.push(quad(POPUP), POPUP));
        }
    }
    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        match ev {
            Event::PointerDown(p) => {
                self.presses.push(p.local);
                Handled::Yes
            }
            Event::Click(p) if p.local.y < 30.0 => {
                self.open = !self.open;
                cx.request_paint();
                Handled::Yes
            }
            _ => Handled::No,
        }
    }
    fn overlay_closed(&mut self, cx: &mut EventCx, reason: DismissReason) -> Handled {
        self.closed.push(reason);
        self.open = false;
        cx.request_paint();
        Handled::Yes
    }
    fn as_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
}

fn odoc() -> (Document, NodeId) {
    let mut r = KindRegistry::new();
    r.register::<testkinds::BoxKind>().unwrap();
    r.register::<Popper>().unwrap();
    let mut d = Document::new(Arc::new(r), Box::new(FixedText));
    d.set_text_shaper(Box::new(FixedText));
    let root = d.create_window(W, BOX, Size::new(200.0, 100.0), Scale(1.0));
    (d, root)
}

fn popper(d: &Document, n: NodeId) -> (bool, Vec<Point>, Vec<DismissReason>) {
    d.with_kind(n, |k| {
        let p = k.as_any().and_then(|a| a.downcast_ref::<Popper>()).expect("a popper");
        (p.open, p.presses.clone(), p.closed.clone())
    })
    .expect("live")
}

fn overlay_adds(c: &Commit) -> Vec<skelter_base::layer::LayerDesc> {
    c.layers.iter().filter_map(|l| if let LayerDelta::Add(d) = l && d.kind == LayerKind::Overlay { Some(*d) } else { None }).collect()
}

#[test]
fn a_popup_paints_above_the_content_takes_presses_outside_its_box_and_light_dismisses() {
    let (mut d, root) = odoc();
    let pop = child(&mut d, root, POPPER, "position: absolute; left: 10px; top: 0px; width: 40px; height: 30px");
    // content the popup covers
    let under = child(&mut d, root, BOX, "position: absolute; left: 0px; top: 30px; width: 200px; height: 70px; background-color: red");
    frame(&mut d);
    click(&mut d, 20.0, 10.0);
    assert!(popper(&d, pop).0, "the click opened it");
    let c = frame(&mut d).expect("the popup's commit");
    let adds = overlay_adds(&c);
    assert_eq!(adds.len(), 1, "one overlay layer: {:?}", c.layers);
    let layer = adds[0];
    assert!(layer.z >= crate::toplayer::OVERLAY_Z, "above every content layer");
    assert_eq!(layer.bounds, POPUP.translate(Vec2::new(10.0, 0.0)), "window points");
    let key = crate::record::overlay_key(pop);
    assert!(c.lists.iter().any(|(n, l)| *n == key && l.items.len() == 1), "the part is its own list");
    assert!(c.layers.iter().any(|l| matches!(l, LayerDelta::Items { layer: id, items } if *id == layer.id && items[0].node == key)));
    let st = d.overlay_stack(W).expect("the stack");
    assert_eq!(st.entries().len(), 1);
    assert_eq!(st.entries()[0].spec.node, pop);

    // a press in the popup, over `under` and outside the popper's box, reaches the popper
    assert_eq!(d.hit_test(W, Point::new(30.0, 60.0)), Some(pop));
    assert_eq!(d.hit_test(W, Point::new(150.0, 60.0)), Some(under), "beside it, the content");
    click(&mut d, 30.0, 60.0);
    let (open, presses, closed) = popper(&d, pop);
    assert!(open && closed.is_empty(), "a press inside keeps it open");
    assert_eq!(presses.last(), Some(&Point::new(20.0, 60.0)), "in the popper's own points");

    // a press outside closes it: the kind hears why, and its layer goes
    click(&mut d, 150.0, 60.0);
    let (open, _, closed) = popper(&d, pop);
    assert_eq!((open, closed.as_slice()), (false, &[DismissReason::OutsidePress][..]));
    let c = frame(&mut d).expect("the close's commit");
    assert!(c.layers.contains(&LayerDelta::Remove(layer.id)), "{:?}", c.layers);
    assert!(c.removed.contains(&key));
    assert!(d.overlay_stack(W).unwrap().is_empty());
    assert_eq!(d.hit_test(W, Point::new(30.0, 60.0)), Some(under));

    // Escape that nothing focused takes closes the topmost surface
    click(&mut d, 20.0, 10.0);
    frame(&mut d);
    assert!(popper(&d, pop).0);
    let out = d.dispatch(&key_ev(Key::Escape));
    assert!(out.handled);
    assert_eq!(popper(&d, pop).2.last(), Some(&DismissReason::Escape));
    frame(&mut d);
    assert!(d.overlay_stack(W).unwrap().is_empty());
}

fn key_ev(k: Key) -> InputEnvelope {
    key(k, Modifiers::default())
}

#[test]
fn the_apps_overlay_list_draws_above_the_content_and_takes_no_input() {
    let (mut d, root) = odoc();
    let under = child(&mut d, root, BOX, "position: absolute; left: 0px; top: 0px; width: 200px; height: 100px");
    frame(&mut d);
    let r = Rect::new(5.0, 5.0, 50.0, 50.0);
    let mut list = DisplayList::new();
    list.push(quad(r), r);
    d.set_window_overlay(W, Some((list, Vec::new())));
    assert!(d.needs_frame(W));
    let c = frame(&mut d).expect("the overlay's commit");
    let adds = overlay_adds(&c);
    assert_eq!(adds.len(), 1, "{:?}", c.layers);
    assert_eq!(adds[0].z, crate::toplayer::APP_OVERLAY_Z);
    assert!(adds[0].z > 0 && adds[0].z < crate::toplayer::OVERLAY_Z, "above the content, below the stack's surfaces");
    assert_eq!(Some(adds[0].id), d.window_overlay_layer(W));
    let key = crate::record::app_overlay_key(W);
    assert!(c.lists.iter().any(|(n, l)| *n == key && l.bounds == r));
    assert!(c.layers.iter().any(|l| matches!(l, LayerDelta::Items { layer, items } if *layer == adds[0].id && items[0].node == key)));
    assert_eq!(d.hit_test(W, Point::new(10.0, 10.0)), Some(under), "presses go through it");
    d.set_window_overlay(W, None);
    let c = frame(&mut d).expect("the removal");
    assert!(c.layers.contains(&LayerDelta::Remove(adds[0].id)) && c.removed.contains(&key));
    assert_eq!(d.window_overlay_layer(W), None);
}

#[test]
fn a_drag_shows_its_preview_in_the_drag_layer_which_follows_the_pointer_until_the_drop() {
    let (mut d, root) = odoc();
    let src = child(&mut d, root, BOX, "position: absolute; left: 0px; top: 0px; width: 40px; height: 40px");
    d.set_prop(src, common::DRAG_TYPE, Value::from("cad/part")).unwrap();
    d.set_drag_preview_provider(Box::new(|_, _, _| {
        let mut list = DisplayList::new();
        list.push(quad(Rect::new(0.0, 0.0, 20.0, 10.0)), Rect::new(0.0, 0.0, 20.0, 10.0));
        Some(DragPreview::List { list, runs: Vec::new(), offset: Vec2::new(-5.0, -5.0) })
    }));
    frame(&mut d);
    let moved = |d: &mut Document, x: f32, y: f32| {
        let mut mv = pointer(PointerPhase::Move, x, y, 10);
        if let InputEvent::Pointer(p) = &mut mv.event {
            p.buttons = 1;
        }
        d.dispatch(&mv);
    };
    d.dispatch(&pointer(PointerPhase::Down, 10.0, 10.0, 0));
    moved(&mut d, 40.0, 10.0);
    assert!(d.drag_preview().is_some(), "dragging");
    let c = frame(&mut d).expect("the drag's commit");
    let adds = overlay_adds(&c);
    assert_eq!(adds.len(), 1, "{:?}", c.layers);
    assert_eq!(adds[0].z, crate::toplayer::DRAG_Z, "topmost");
    let (layer, tf) = d.drag_layer(W).expect("the drag layer");
    assert_eq!((adds[0].id, adds[0].transform), (layer, tf));
    let key = crate::record::drag_key(W);
    assert!(c.lists.iter().any(|(n, _)| *n == key), "the preview's list");
    let local = |c: &Commit| c.props.ops.iter().find_map(|o| if let PropOp::Transform(id, n) = o && *id == tf { Some(n.local) } else { None });
    assert_eq!(local(&c), Some(Affine::translate(35.0, 5.0)), "at the pointer, by the preview's offset");
    d.take_fast_commits();

    // a move goes to the compositor at once as the layer's transform, not a re-record
    moved(&mut d, 60.0, 30.0);
    let fast = d.take_fast_commits();
    assert!(
        fast.iter().any(|f| matches!(f, FastCommit::Transform { node, local, .. } if *node == tf && *local == Affine::translate(55.0, 25.0))),
        "{fast:?}"
    );
    if let Some(c) = frame(&mut d) {
        assert!(!c.lists.iter().any(|(n, _)| *n == key), "not re-recorded");
        assert_eq!(local(&c), Some(Affine::translate(55.0, 25.0)), "the commit agrees");
    }

    // the drop removes it
    d.dispatch(&pointer(PointerPhase::Up, 60.0, 30.0, 20));
    let c = frame(&mut d).expect("the drop's commit");
    assert!(c.layers.contains(&LayerDelta::Remove(layer)), "{:?}", c.layers);
    assert!(c.removed.contains(&key) && c.props.ops.contains(&PropOp::Remove(tf)));
    assert_eq!(d.drag_layer(W), None);
}

/// SV61: a provider that logs what the document reports and shows a fixed tip while a host is hovered.
struct LogTips(std::rc::Rc<std::cell::RefCell<Vec<(crate::tips::TitleInput, Time)>>>);

impl crate::tips::TitleTips for LogTips {
    fn step(&mut self, doc: &mut Document, window: skelter_base::ids::WindowId, input: &crate::tips::TitleInput, now: Time) -> Option<Time> {
        self.0.borrow_mut().push((input.clone(), now));
        let r = Rect::new(20.0, 60.0, 60.0, 80.0);
        let tip = input.host.as_ref().map(|_| {
            let mut list = DisplayList::new();
            list.push(quad(r), r);
            (list, Vec::new())
        });
        doc.set_window_tip(window, tip);
        input.host.as_ref().map(|_| Time(now.0 + 500))
    }
}

#[test]
fn the_title_host_reaches_the_tip_provider_and_its_layer_tops_the_overlays() {
    let (mut d, root) = odoc();
    let host = child(&mut d, root, BOX, "position: absolute; left: 0px; top: 0px; width: 50px; height: 40px; background-color: red");
    let inner = child(&mut d, host, BOX, "width: 10px; height: 10px; background-color: blue");
    d.set_prop(host, common::TITLE, Value::from("Save")).unwrap();
    let log = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    d.set_title_tips(Some(Box::new(LogTips(log.clone()))));
    frame(&mut d);
    // over the inner box: the innermost node with a title is the host
    d.dispatch(&pointer(PointerPhase::Move, 5.0, 5.0, 1_000));
    assert_eq!(log.borrow().last().unwrap().0.host, Some((host, Arc::from("Save"))));
    assert_eq!(d.title_host(W).map(|h| h.0), Some(host));
    assert_eq!(d.next_timer(), Some(Time(1_500)), "the provider's wake-up is the document's");
    let c = frame(&mut d).expect("the tip's commit");
    let adds = overlay_adds(&c);
    assert_eq!(adds.len(), 1, "{:?}", c.layers);
    assert_eq!(adds[0].z, crate::toplayer::TIP_Z);
    assert!(adds[0].z > crate::toplayer::OVERLAY_Z && adds[0].z < crate::toplayer::DRAG_Z);
    assert_eq!(d.window_tip_layer(W), Some(adds[0].id));
    // the wake-up steps it without input
    d.run_timers(Time(1_500));
    assert_eq!(log.borrow().last().unwrap().1, Time(1_500));
    // a press and Escape are reported
    d.dispatch(&pointer(PointerPhase::Down, 5.0, 5.0, 2_000));
    assert!(log.borrow().last().unwrap().0.pressed);
    d.dispatch(&key(Key::Escape, Modifiers::default()));
    assert!(log.borrow().last().unwrap().0.escape);
    d.dispatch(&pointer(PointerPhase::Up, 5.0, 5.0, 2_100));
    // off the host: no host, the layer goes
    d.dispatch(&pointer(PointerPhase::Move, 150.0, 90.0, 3_000));
    assert_eq!(log.borrow().last().unwrap().0.host, None);
    let c = frame(&mut d).expect("the removal");
    assert!(c.layers.contains(&LayerDelta::Remove(adds[0].id)));
    let _ = inner;
}

/// R2: a hover at event time (a coalesced move) steps the tip at the move's time, not at the time
/// of the input before it, so the tip's delay starts when the pointer came to rest.
#[test]
fn a_timed_hover_starts_the_tip_at_its_own_time() {
    let (mut d, root) = odoc();
    let host = child(&mut d, root, BOX, "position: absolute; left: 0px; top: 0px; width: 50px; height: 40px; background-color: red");
    d.set_prop(host, common::TITLE, Value::from("Save")).unwrap();
    let log = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    d.set_title_tips(Some(Box::new(LogTips(log.clone()))));
    frame(&mut d);
    // the last input, long ago and off the host
    d.dispatch(&pointer(PointerPhase::Move, 150.0, 90.0, 1_000));
    assert!(d.hover_at(W, Point::new(5.0, 5.0), Time(9_000)));
    assert_eq!(log.borrow().last().unwrap(), &(crate::tips::TitleInput { host: Some((host, Arc::from("Save"))), ..Default::default() }, Time(9_000)));
    assert_eq!(d.next_timer(), Some(Time(9_500)), "the delay runs from the hover");
    // an older time never moves the clock back
    assert!(d.hover_at(W, Point::new(150.0, 90.0), Time(5_000)));
    assert_eq!(log.borrow().last().unwrap().1, Time(9_000));
}
