//! A2: `use:ns.name={params}` on skelter. The ops are the ones `nativeBehavior` (native-runtime, as on
//! egui) makes for `use:run={{ command: 'file.save' }}`: create `<ns>.<name>`, set each param, insert
//! into the element, set what changes, destroy. They reach the app's Rust behaviour as attach, update
//! and detach; the event tap sees a click on the element with no JS listener.

mod common;

use common::Rig;
use skelter_base::event::{InputEnvelope, InputEvent, Modifiers, MouseButton, PointerEvent, PointerPhase, PointerType};
use skelter_base::geom::{Point, Scale, Size, Vec2};
use skelter_base::ids::{InputSeq, WindowId};
use skelter_base::time::Time;
use skelter_core::{ListenFlags, common as core_common};
use std::cell::RefCell;
use std::rc::Rc;
use svelters_abi::*;
use svelters_skelter::behavior::{Behavior, BehaviorCx, Props};

type Calls = Rc<RefCell<Vec<String>>>;

/// A command behaviour: logs its hooks, and listens natively to `click` on its element so the tap
/// sees the clicks.
struct Run(Calls);

impl Behavior for Run {
    fn attach(&mut self, cx: &mut BehaviorCx<'_>, props: &Props) {
        cx.doc.listen(cx.node, core_common::CLICK, ListenFlags::PASSIVE);
        self.0.borrow_mut().push(format!("attach {:?}", props.get("command")));
    }
    fn update(&mut self, _cx: &mut BehaviorCx<'_>, props: &Props) {
        self.0.borrow_mut().push(format!("update {:?}", props.get("command")));
    }
    fn detach(&mut self, cx: &mut BehaviorCx<'_>) {
        cx.doc.unlisten(cx.node, core_common::CLICK, false);
        self.0.borrow_mut().push("detach".into());
    }
}

fn rig() -> (Rig, Calls) {
    let r = Rig::new();
    let calls: Calls = Rc::default();
    let c = calls.clone();
    r.backend.register_behavior("cmd", "run", move || Box::new(Run(c.clone())) as Box<dyn Behavior>);
    (r, calls)
}

/// `nativeBehavior('cmd.run')(element, params)`: create, set params, insert.
fn use_run(r: &Rig, element: svl_handle, command: &str) -> svl_handle {
    let k = r.br().resolve(NameKind::Kind, 0, "cmd.run");
    assert_ne!(k, 0, "the behaviour kind resolves");
    let b = r.br().create(k).unwrap();
    r.set(b, "command", Value::Str(command.into()));
    r.br().insert(element, b, None, 0).unwrap();
    b
}

#[test]
fn use_directive_attaches_updates_and_detaches_the_rust_behavior() {
    let (r, calls) = rig();
    let parent = r.create("box");
    let el = r.create("button");
    r.insert(parent, el);
    let b = use_run(&r, el, "file.save");
    assert_eq!(r.br().handle_info(b).unwrap().kind, r.br().resolve(NameKind::Kind, 0, "cmd.run"));
    // the params change: one update with the whole record
    r.set(b, "command", Value::Str("file.open".into()));
    assert_eq!(r.get(b, "command"), Value::Str("file.open".into()));
    // the behaviour is no child of the element
    let n = r.backend.node(el).unwrap();
    assert!(r.backend.with_doc(|d| d.children(n).is_empty()));
    // the element's node removed (freed): the behaviour detaches with it; the action's destroy after is a no-op
    r.br().destroy(el);
    r.br().destroy(b);
    assert_eq!(*calls.borrow(), ["attach Some(Str(\"file.save\"))", "update Some(Str(\"file.open\"))", "detach"]);
    // the action destroyed on a live element detaches it too
    let el2 = r.create("button");
    let b2 = use_run(&r, el2, "edit.undo");
    r.br().destroy(b2);
    assert_eq!(calls.borrow().last().unwrap(), "detach");
    assert!(r.br().handle_info(b2).is_err());
}

#[test]
fn behavior_modules_export_one_action_per_behavior() {
    let (r, _) = rig();
    let mods = r.backend.behavior_modules();
    assert_eq!(mods.len(), 1);
    assert_eq!(mods[0].0, "cmd");
    assert!(mods[0].1.contains("export const run = nativeBehavior('cmd.run', []);"), "{}", mods[0].1);
}

fn pointer(phase: PointerPhase, seq: u64) -> InputEnvelope {
    let p = PointerEvent {
        phase,
        pointer: 0,
        kind: PointerType::Mouse,
        button: Some(MouseButton::Primary),
        buttons: if phase == PointerPhase::Down { 1 } else { 0 },
        pos: Point::new(10.0, 10.0),
        modifiers: Modifiers::default(),
        pressure: 0.5,
        tilt: Vec2::ZERO,
        history: Default::default(),
    };
    InputEnvelope { seq: InputSeq(seq), window: WindowId(1), os_time: Time(seq * 1000), recv_time: Time(seq * 1000), event: InputEvent::Pointer(p) }
}

#[test]
fn the_event_tap_sees_a_click_with_no_js() {
    let (r, _) = rig();
    let root = r.create("box");
    r.backend.attach_presented(root, WindowId(1), Size::new(200.0, 100.0), Scale(1.0)).unwrap();
    use_run(&r, root, "file.save");
    let seen: Rc<RefCell<Vec<(skelter_core::SignalId, skelter_base::ids::NodeId)>>> = Rc::default();
    let s = seen.clone();
    r.backend.add_event_tap(move |_doc, _ev, events| s.borrow_mut().extend(events.iter().map(|e| (e.signal, e.target))));
    r.backend.with_doc(|d| d.frame(WindowId(1), Time(0)));
    r.backend.dispatch(&pointer(PointerPhase::Down, 1));
    r.backend.dispatch(&pointer(PointerPhase::Up, 2));
    let n = r.backend.node(root).unwrap();
    assert!(seen.borrow().contains(&(core_common::CLICK, n)), "{:?}", seen.borrow());
    // (the host still gets the click on the root's handle; the runtime finds no handler for it)
}
