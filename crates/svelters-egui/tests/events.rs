//! EV1: DOM-style events on the egui scene (`svelters_egui::events`): capture, target and bubble
//! order through `call_sync`, `stopPropagation` / `preventDefault` answers, the targets (leaves,
//! box padding, the focused node), Rust event filters (SV6) and overlays (SV7). The host here
//! records every listener call and answers from a table; the Svelte side is `ev1_svelte_*`.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use svelters_abi::*;
use svelters_egui::EguiBackend;
use svelters_egui::events::{At, EventKind, EventPhase, FilterScope, Flow, Outcome, Overlay, OverlayCx, SceneEvent};
use svelters_egui::scene::{NodeId, SceneRef, Target};

const CAPTURE: u32 = SVL_LISTEN_CAPTURE;
const STOP: i64 = 1;
const PREVENT: i64 = 2;
const IMMEDIATE: i64 = 4;

fn s(v: &str) -> Value {
    Value::Str(v.into())
}

/// One listener call: handle, signal name, flags, phase argument, target argument.
#[derive(Clone, Debug, PartialEq)]
struct Call {
    h: svl_handle,
    name: String,
    flags: u32,
    phase: i64,
    target: svl_handle,
    args: Vec<Value>,
}

#[derive(Default)]
struct Log {
    calls: Vec<Call>,
    posted: Vec<(svl_handle, svl_id, Vec<Value>)>,
    /// (handle, signal name, flags) -> the answer bits.
    answers: HashMap<(svl_handle, String, u32), i64>,
    names: HashMap<svl_id, String>,
}

struct Rec(Rc<RefCell<Log>>);

impl HostCallbacks for Rec {
    fn post_event(&self, h: svl_handle, signal: svl_id, args: &[Value]) {
        self.0.borrow_mut().posted.push((h, signal, args.to_vec()));
    }
    fn checkpoint(&self) {}
    fn call_sync(&self, h: svl_handle, signal: svl_id, args: &[Value]) -> Result<Value, SyncError> {
        let mut l = self.0.borrow_mut();
        let (id, flags) = (signal & SVL_SIGNAL_ID_MASK, signal >> SVL_SIGNAL_FLAGS_SHIFT);
        let name = l.names.get(&id).cloned().unwrap_or_else(|| format!("#{id}"));
        let num = |k: usize| match args.get(k) {
            Some(Value::I64(v)) => *v,
            _ => -1,
        };
        l.calls.push(Call { h, name: name.clone(), flags, phase: num(1), target: num(0) as svl_handle, args: args.to_vec() });
        Ok(l.answers.get(&(h, name, flags)).map_or(Value::Null, |&b| Value::I64(b)))
    }
    fn on_destroyed(&self, _h: svl_handle, _reason: u32) {}
    fn frame(&self) {}
    fn prop_changed(&self, _h: svl_handle, _prop: svl_id) {}
}

struct Rig {
    backend: EguiBackend,
    vt: &'static BackendVtable,
    log: Rc<RefCell<Log>>,
    harness: Harness<'static, EguiBackend>,
}

impl Rig {
    fn new() -> Rig {
        let backend = EguiBackend::new();
        let vt: &'static BackendVtable = Box::leak(Box::new(backend.vtable()));
        let log: Rc<RefCell<Log>> = Rc::default();
        let hv: &'static HostVtable = Box::leak(Box::new(HostVtable::new(Rec(log.clone()))));
        vt.backend_ref().start(hv).expect("start");
        // 60 frames a second: kittest delivers one queued event per frame, and a double click
        // needs its two clicks within egui's 0.3 s
        let harness = Harness::builder().with_size(egui::vec2(480.0, 360.0)).with_step_dt(1.0 / 60.0).build_ui_state(|ui, b: &mut EguiBackend| b.ui(ui), backend.clone());
        // a scroll lands without the smooth-scroll animation
        harness.ctx.all_styles_mut(|st| st.scroll_animation = egui::style::ScrollAnimation::none());
        Rig { backend, vt, log, harness }
    }
    fn br(&self) -> BackendRef<'static> {
        self.vt.backend_ref()
    }
    fn kind(&self, h: svl_handle) -> svl_id {
        self.br().handle_info(h).unwrap().kind
    }
    fn create(&self, tag: &str) -> svl_handle {
        let k = self.br().resolve(NameKind::Kind, 0, tag);
        assert_ne!(k, 0, "unknown tag {tag}");
        self.br().create(k).unwrap()
    }
    fn set(&self, h: svl_handle, name: &str, v: Value) {
        let p = self.br().resolve(NameKind::Prop, self.kind(h), name);
        assert_ne!(p, 0, "no prop {name}");
        self.br().set_prop(h, p, &v).unwrap()
    }
    fn get(&self, h: svl_handle, name: &str) -> Value {
        let p = self.br().resolve(NameKind::Prop, self.kind(h), name);
        self.br().get_prop(h, p).unwrap()
    }
    fn sig(&self, h: svl_handle, name: &str) -> svl_id {
        let id = self.br().resolve(NameKind::Signal, self.kind(h), name);
        assert_ne!(id, 0, "no signal {name}");
        self.log.borrow_mut().names.insert(id, name.to_string());
        id
    }
    fn listen(&self, h: svl_handle, name: &str) {
        let id = self.sig(h, name);
        self.br().listen(h, id, true).unwrap()
    }
    fn capture(&self, h: svl_handle, name: &str) {
        let id = self.sig(h, name);
        self.br().listen_ex(h, id, true, CAPTURE).unwrap()
    }
    fn answer(&self, h: svl_handle, name: &str, flags: u32, bits: i64) {
        self.log.borrow_mut().answers.insert((h, name.to_string(), flags), bits);
    }
    fn insert(&self, parent: svl_handle, child: svl_handle) {
        self.br().insert(parent, child, None, 0).unwrap()
    }
    /// A presented window holding a column box; returns (window, box).
    fn window(&self) -> (svl_handle, svl_handle) {
        let w = self.create("window");
        let b = self.create("box");
        self.set(b, "gap", Value::F64(8.0));
        self.insert(w, b);
        let m = self.br().resolve(NameKind::Method, self.kind(w), "present");
        self.br().call(w, m, &[]).unwrap();
        (w, b)
    }
    fn node(&self, parent: svl_handle, tag: &str, props: &[(&str, Value)]) -> svl_handle {
        let h = self.create(tag);
        for (k, v) in props {
            self.set(h, k, v.clone());
        }
        self.insert(parent, h);
        h
    }
    fn run(&mut self) {
        self.harness.run();
    }
    fn drawn(&self, h: svl_handle) -> egui::Rect {
        self.backend.with_scene(|s| s.n(h).drawn)
    }
    fn press_release(&mut self, pos: egui::Pos2, button: egui::PointerButton) {
        self.harness.hover_at(pos);
        for pressed in [true, false] {
            self.harness.event(egui::Event::PointerButton { pos, button, pressed, modifiers: egui::Modifiers::NONE });
        }
        self.harness.run();
    }
    fn click_at(&mut self, pos: egui::Pos2) {
        self.press_release(pos, egui::PointerButton::Primary);
    }
    fn click(&mut self, h: svl_handle) {
        self.click_at(self.drawn(h).center());
    }
    /// The listener calls since the last `take`, as (handle, name, flags, phase).
    fn take(&self) -> Vec<(svl_handle, String, u32, i64)> {
        std::mem::take(&mut self.log.borrow_mut().calls).into_iter().map(|c| (c.h, c.name, c.flags, c.phase)).collect()
    }
    fn take_calls(&self) -> Vec<Call> {
        std::mem::take(&mut self.log.borrow_mut().calls)
    }
    fn posted(&self, h: svl_handle, name: &str) -> usize {
        let id = self.sig(h, name);
        self.log.borrow().posted.iter().filter(|e| e.0 == h && e.1 == id).count()
    }
    fn id(&self, h: svl_handle) -> NodeId {
        NodeId::from_raw(h).unwrap()
    }
}

fn c(h: svl_handle, name: &str, flags: u32, phase: i64) -> (svl_handle, String, u32, i64) {
    (h, name.to_string(), flags, phase)
}

/// outer box > inner box > button, every set of click listeners connected.
fn nested() -> (Rig, svl_handle, svl_handle, svl_handle) {
    let mut rig = Rig::new();
    let (_w, outer) = rig.window();
    let inner = rig.node(outer, "box", &[("padding", Value::F64(20.0))]);
    let button = rig.node(inner, "button", &[("text", s("Child"))]);
    rig.listen(button, "clicked");
    for h in [outer, inner, button] {
        rig.capture(h, "click");
        rig.listen(h, "click");
    }
    rig.run();
    (rig, outer, inner, button)
}

#[test]
fn ev1_click_runs_capture_target_bubble_in_dom_order() {
    let (mut rig, outer, inner, button) = nested();
    rig.click(button);
    let calls = rig.take_calls();
    let got: Vec<_> = calls.iter().map(|c| (c.h, c.name.clone(), c.flags, c.phase)).collect();
    assert_eq!(
        got,
        vec![
            c(outer, "click", CAPTURE, 1),
            c(inner, "click", CAPTURE, 1),
            c(button, "click", CAPTURE, 2),
            c(button, "click", 0, 2),
            c(inner, "click", 0, 3),
            c(outer, "click", 0, 3),
        ]
    );
    assert!(calls.iter().all(|c| c.target == button), "every listener is told the target");
    // the point and the button (0: primary) follow target and phase
    let p = rig.drawn(button).center();
    assert_eq!(&calls[0].args[2..5], &[Value::F64(p.x as f64), Value::F64(p.y as f64), Value::I64(0)]);
    assert_eq!(rig.posted(button, "clicked"), 1, "egui still saw the click");
}

#[test]
fn ev1_stop_propagation_ends_the_path_and_immediate_skips_the_rest_of_the_node() {
    let (mut rig, outer, inner, button) = nested();
    rig.answer(inner, "click", 0, STOP);
    rig.click(button);
    assert_eq!(
        rig.take(),
        vec![c(outer, "click", CAPTURE, 1), c(inner, "click", CAPTURE, 1), c(button, "click", CAPTURE, 2), c(button, "click", 0, 2), c(inner, "click", 0, 3)],
        "the outer box's bubble listener does not run"
    );
    assert_eq!(rig.posted(button, "clicked"), 1, "stopPropagation keeps the default");
    // in the capture phase, at the outer box: nothing below it runs
    rig.answer(inner, "click", 0, 0);
    rig.answer(outer, "click", CAPTURE, STOP);
    rig.click(button);
    assert_eq!(rig.take(), vec![c(outer, "click", CAPTURE, 1)]);
    // at the target: stopImmediatePropagation in its capture listener skips its plain one too
    rig.answer(outer, "click", CAPTURE, 0);
    rig.answer(button, "click", CAPTURE, STOP | IMMEDIATE);
    rig.click(button);
    assert_eq!(rig.take(), vec![c(outer, "click", CAPTURE, 1), c(inner, "click", CAPTURE, 1), c(button, "click", CAPTURE, 2)]);
}

#[test]
fn ev1_prevent_default_keeps_the_input_from_egui() {
    let (mut rig, outer, inner, button) = nested();
    rig.listen(button, "pointerdown");
    // pointerdown: egui never sees the press, so no click (the DOM click still runs)
    rig.answer(button, "pointerdown", 0, PREVENT);
    rig.click(button);
    assert_eq!(rig.posted(button, "clicked"), 0, "the press was taken out");
    assert!(rig.take().iter().any(|x| x.1 == "click"), "click is still dispatched");
    // a listener after the one that prevented sees phase + 8
    rig.answer(button, "pointerdown", 0, 0);
    rig.answer(outer, "click", CAPTURE, PREVENT);
    rig.click(button);
    assert_eq!(rig.posted(button, "clicked"), 0, "the click's default was prevented: egui saw a release without a click");
    let calls = rig.take();
    assert!(calls.contains(&c(inner, "click", CAPTURE, 1 | 8)), "{calls:?}");
    // nothing prevented: the click reaches the button again
    rig.answer(outer, "click", CAPTURE, 0);
    rig.click(button);
    assert_eq!(rig.posted(button, "clicked"), 1);
}

#[test]
fn ev1_a_click_on_a_box_targets_the_box_and_on_two_nodes_their_common_ancestor() {
    let (mut rig, outer, inner, button) = nested();
    let r = rig.backend.scene().read(|s| s.box_rect(egui::ViewportId::ROOT, rig.id(inner))).expect("the box is in the hit list");
    // the inner box's padding: the target is the box
    rig.click_at(r.min + egui::vec2(5.0, 5.0));
    let calls = rig.take_calls();
    assert!(!calls.is_empty());
    assert!(calls.iter().all(|c| c.target == inner), "{calls:?}");
    assert!(calls.iter().all(|c| c.h != button));
    // pressed on the button, released on the padding: the click goes to their common ancestor
    let (a, b) = (rig.drawn(button).center(), r.min + egui::vec2(5.0, 5.0));
    rig.harness.hover_at(a);
    rig.harness.event(egui::Event::PointerButton { pos: a, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
    rig.harness.event(egui::Event::PointerButton { pos: b, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
    rig.run();
    let calls = rig.take_calls();
    assert_eq!(calls.first().map(|c| (c.h, c.target)), Some((outer, inner)));
}

#[test]
fn ev1_pointer_wheel_contextmenu_and_dblclick_events() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let area = rig.node(col, "scroll-area", &[("height", Value::F64(100.0))]);
    let tall = rig.node(area, "box", &[("min-height", Value::F64(1000.0))]);
    let _l = rig.node(tall, "label", &[("text", s("content"))]);
    let button = rig.node(col, "button", &[("text", s("B"))]);
    for (h, n) in [(col, "wheel"), (col, "contextmenu"), (col, "dblclick"), (col, "pointerdown"), (col, "pointerup")] {
        rig.listen(h, n);
    }
    rig.run();
    // wheel: bubbles to the column; prevented, the area does not scroll
    rig.answer(col, "wheel", 0, PREVENT);
    let wheel = |rig: &mut Rig| {
        let p = rig.drawn(area).center();
        rig.harness.event(egui::Event::PointerMoved(p));
        rig.run();
        rig.harness.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -60.0),
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Unknown,
            modifiers: egui::Modifiers::NONE,
        });
        rig.harness.run_steps(40);
    };
    wheel(&mut rig);
    let calls = rig.take_calls();
    let w = calls.iter().find(|c| c.name == "wheel").expect("wheel dispatched");
    assert_eq!(w.args[5], Value::F64(60.0), "DOM direction: positive scrolls down");
    assert_eq!(rig.get(area, "scroll-y"), Value::F64(0.0), "prevented: no scroll");
    rig.answer(col, "wheel", 0, 0);
    wheel(&mut rig);
    rig.take();
    let y = rig.get(area, "scroll-y");
    assert!(matches!(y, Value::F64(y) if y > 0.0), "not prevented: it scrolls ({y:?}, area {:?}, content {:?})", rig.drawn(area), rig.backend.with_scene(|sc| sc.n(tall).rect));
    // the secondary button: pointerdown, then contextmenu (button 2), then pointerup
    rig.press_release(rig.drawn(button).center(), egui::PointerButton::Secondary);
    let names: Vec<_> = rig.take_calls().into_iter().map(|c| (c.name, c.args[4].clone())).collect();
    assert_eq!(names, vec![("pointerdown".into(), Value::I64(2)), ("contextmenu".into(), Value::I64(2)), ("pointerup".into(), Value::I64(2))]);
    // two clicks in one frame: the second is a dblclick
    let p = rig.drawn(button).center();
    rig.harness.hover_at(p);
    for pressed in [true, false, true, false] {
        rig.harness.event(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
    }
    rig.run();
    assert_eq!(rig.take().iter().filter(|x| x.1 == "dblclick").count(), 1);
}

/// `a` as egui-winit sends it: the key down, the text it types, the key up.
fn type_a(rig: &mut Rig) {
    let key = |pressed| egui::Event::Key { key: egui::Key::A, physical_key: None, pressed, repeat: false, modifiers: egui::Modifiers::NONE };
    rig.harness.event(key(true));
    rig.harness.event(egui::Event::Text("a".into()));
    rig.harness.event(key(false));
    rig.run();
}

#[test]
fn ev1_keys_go_to_the_focused_node_and_focus_events_bubble() {
    let mut rig = Rig::new();
    let (w, col) = rig.window();
    let field = rig.node(col, "text-edit", &[("accessible-name", s("field"))]);
    for n in ["keydown", "focusin", "focusout"] {
        rig.listen(col, n);
    }
    rig.capture(w, "keydown");
    rig.run();
    // no focus: keys go to the window
    rig.harness.key_press(egui::Key::F5);
    rig.run();
    assert_eq!(rig.take(), vec![c(w, "keydown", CAPTURE, 2)]);
    rig.click(field);
    assert!(rig.harness.query_by_label("field").is_some_and(|n| n.is_focused()));
    let calls = rig.take_calls();
    let fin: Vec<_> = calls.iter().filter(|c| c.name == "focusin").collect();
    assert_eq!(fin.len(), 1, "{calls:?}");
    assert_eq!((fin[0].h, fin[0].target, fin[0].phase), (col, field, 3 | 16), "bubbled from the field, not cancelable");
    // typing: keydown at the window (capture), then the column (bubble); prevented, no text
    rig.answer(col, "keydown", 0, PREVENT);
    type_a(&mut rig);
    let k = rig.take_calls();
    let got: Vec<_> = k.iter().filter(|c| c.name == "keydown").map(|c| (c.h, c.flags, c.phase, c.target, c.args[2].clone())).collect();
    assert_eq!(got, vec![(w, CAPTURE, 1, field, s("A")), (col, 0, 3, field, s("A"))]);
    assert_eq!(rig.get(field, "text"), s(""), "the key and its text never reached the field");
    rig.answer(col, "keydown", 0, 0);
    type_a(&mut rig);
    assert_eq!(rig.get(field, "text"), s("a"));
}

#[test]
fn ev1_listen_ex_takes_capture_on_dom_events_only() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let button = rig.node(col, "button", &[("text", s("B"))]);
    rig.run();
    let clicked = rig.sig(button, "clicked");
    let e = rig.br().listen_ex(button, clicked, true, CAPTURE).unwrap_err();
    assert!(e.to_string().contains("no capture phase"), "{e}");
    let click = rig.sig(button, "click");
    let e = rig.br().listen_ex(button, click, true, SVL_LISTEN_AFTER).unwrap_err();
    assert!(e.to_string().contains("no listen phase"), "{e}");
    rig.br().listen_ex(button, click, true, CAPTURE).unwrap();
    rig.br().listen_ex(button, click, false, CAPTURE).unwrap();
    rig.click(button);
    assert!(rig.take().is_empty(), "off again");
}

/// SV6: Rust filters see the event before Svelte, outermost first, and can consume it.
#[test]
fn ev1_filters_run_before_svelte_and_can_consume() {
    let (mut rig, outer, inner, button) = nested();
    let scene = rig.backend.scene();
    let seen: Rc<RefCell<Vec<(&'static str, String, NodeId, EventPhase)>>> = Rc::default();
    let after: Rc<RefCell<Vec<(String, Outcome)>>> = Rc::default();
    struct Watch(Rc<RefCell<Vec<(&'static str, String, NodeId, EventPhase)>>>, Rc<RefCell<Vec<(String, Outcome)>>>);
    impl svelters_egui::events::EventFilter for Watch {
        fn filter(&mut self, ev: &SceneEvent, _s: SceneRef<'_>) -> Flow {
            self.0.borrow_mut().push(("all", ev.name().to_string(), ev.current, ev.phase));
            Flow::Pass
        }
        fn after(&mut self, ev: &SceneEvent, o: &Outcome, _s: SceneRef<'_>) {
            self.1.borrow_mut().push((ev.name().to_string(), *o));
        }
    }
    let all = scene.register_filter(FilterScope::All, Watch(seen.clone(), after.clone()));
    let consume = Rc::new(RefCell::new(true));
    let (sn, cn) = (seen.clone(), consume.clone());
    let sub = scene.register_filter(FilterScope::Subtree(rig.id(inner)), move |ev: &SceneEvent, s: SceneRef<'_>| {
        assert!(s.get(ev.target_node()).is_some());
        sn.borrow_mut().push(("inner", ev.name().to_string(), ev.current, ev.phase));
        if *cn.borrow() && matches!(ev.kind, EventKind::PointerDown | EventKind::Click) { Flow::Consume } else { Flow::Pass }
    });
    assert_eq!(scene.filter_count(), 2);
    rig.click(button);
    assert!(rig.take().is_empty(), "no Svelte listener saw a consumed event");
    assert_eq!(rig.posted(button, "clicked"), 0, "nor egui");
    let seen_now: Vec<_> = seen.borrow_mut().drain(..).collect();
    let (o, i, b) = (rig.id(outer), rig.id(inner), rig.id(button));
    let _ = (o, b);
    assert_eq!(seen_now[0], ("all", "pointerdown".into(), rig.backend.scene().read(|s| s.get(o).unwrap().parent().unwrap().id()), EventPhase::Capture), "the scene's filters at the root");
    assert_eq!(seen_now[1], ("inner", "pointerdown".into(), i, EventPhase::Capture));
    assert!(after.borrow().iter().any(|(n, o)| n == "pointerdown" && o.consumed && o.prevented && o.listeners == 0));
    // passing: Svelte runs after the filters
    *consume.borrow_mut() = false;
    rig.click(button);
    assert_eq!(rig.take().len(), 6);
    assert_eq!(rig.posted(button, "clicked"), 1);
    assert!(after.borrow().iter().any(|(n, o)| n == "click" && !o.consumed && o.listeners == 6));
    all.unregister();
    sub.unregister();
    assert_eq!(scene.filter_count(), 0);
}

/// SV7: an overlay paints after the walk, with rects by node, target and anchor.
#[test]
fn ev1_overlay_paints_after_the_walk_with_rects_by_node_and_anchor() {
    let (mut rig, _outer, inner, button) = nested();
    rig.set(button, "anchor", s("the-button"));
    rig.run();
    #[derive(Default)]
    struct Spot {
        node: Option<NodeId>,
        boxed: Option<NodeId>,
        seen: Vec<(Option<egui::Rect>, Option<egui::Rect>, Option<egui::Rect>, Option<Target>)>,
    }
    impl Overlay for Spot {
        fn paint(&mut self, cx: &mut OverlayCx<'_>) {
            let (n, b) = (self.node.unwrap(), self.boxed.unwrap());
            let r = cx.scene().get(n).unwrap().root();
            let by_node = cx.rect(n);
            let by_anchor = cx.rect(At::Anchor(r, "the-button".into()));
            if let Some(rect) = by_node {
                cx.painter().rect_stroke(rect, 2.0, egui::Stroke::new(2.0, egui::Color32::RED), egui::StrokeKind::Outside);
            }
            let hovered = cx.hovered();
            self.seen.push((by_node, by_anchor, cx.rect(b), hovered));
        }
    }
    let scene = rig.backend.scene();
    let o = scene.register_overlay(Spot { node: Some(rig.id(button)), boxed: Some(rig.id(inner)), ..Default::default() });
    rig.harness.hover_at(rig.drawn(button).center());
    rig.run();
    let (by_node, by_anchor, boxed, hovered) = o.with(|s| *s.seen.last().expect("the overlay ran"));
    assert_eq!(by_node, Some(rig.drawn(button)));
    assert_eq!(by_anchor, by_node);
    assert!(boxed.is_some_and(|b| b.contains_rect(rig.drawn(button))), "a box's rect: {boxed:?}");
    assert_eq!(hovered, Some(Target::Node(rig.id(button))));
    let n = o.with(|s| s.seen.len());
    o.unregister();
    rig.harness.hover_at(egui::pos2(2.0, 2.0));
    rig.run();
    assert_eq!(o.with(|s| s.seen.len()), n, "unregistered");
}

/// The Svelte side (from source): `onclick` on a parent sees a child's click in bubble order,
/// `onclickcapture` / `on:click|capture` run first, `stopPropagation()` ends the path and
/// `preventDefault()` on `pointerdown` keeps the click from the button.
#[cfg(feature = "source")]
#[test]
fn ev1_svelte_capture_and_bubble_from_source() {
    use svelters_egui::harness::TestApp;
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/motion-egui/src/Propagation.svelte");
    let mut app = TestApp::from_source(path).unwrap();
    app.run();
    let click = |app: &mut TestApp, label: &str| {
        let pos = app.harness.get_by_label(label).rect().center();
        app.harness.hover_at(pos);
        for pressed in [true, false] {
            app.harness.event(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
        }
        app.run();
    };
    let expect = |app: &TestApp, log: &str, why: &str| {
        let errs = app.errors();
        assert!(errs.is_empty(), "{errs:?}");
        assert!(app.harness.query_by_label(&format!("Log: {log}")).is_some(), "expected `Log: {log}` ({why})");
    };
    click(&mut app, "Child");
    expect(&app, "C1cTbB3!", "capture outer, capture inner, target, bubble inner, bubble outer, then the button's own clicked");
    click(&mut app, "Stop");
    click(&mut app, "Clear");
    click(&mut app, "Child");
    expect(&app, "C1cTb!", "stopPropagation in the inner box");
    click(&mut app, "Prevent");
    click(&mut app, "Clear");
    click(&mut app, "Child");
    expect(&app, "C1cTb", "pointerdown prevented: the button never clicked");
}
