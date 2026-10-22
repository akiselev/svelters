//! EV1 from Rust (SV6, ARCH-WIDGETS §1.2): a widget's and a behaviour's own events go through
//! `cx.dispatch` and the Rust filters (registered filters, `Behavior::filter`) **before** their
//! default action; a behaviour's filter consumes a pointer press so egui never sees it; an event
//! on a widget's node part (a shadow child) reaches Svelte retargeted to the host. The kinds are
//! defined here, the way cadatomic defines its own, and built from Rust into an embed slot.

use std::cell::RefCell;
use std::rc::Rc;
use svelters_egui::embed::{EmbedConfig, SlotId, SlotOptions};
use svelters_egui::events::{EventKind, EventPhase, FilterScope, Flow, SceneEvent};
use svelters_egui::harness::EmbedHarness;
use svelters_egui::prelude::*;
use svelters_egui::scene::{NodeId, ProviderId, SceneHandle, keys};
use svelters_host::HostConfig;

/// Its press is a user-visible event: dispatched through the filters before it counts.
#[svelters::widget(namespace = "ev1test", tag = "tap-pad")]
pub struct TapPad {
    #[event]
    tapped: Event<()>,
    #[state]
    taps: u32,
    #[state]
    prevented: u32,
}

impl Widget for TapPad {
    fn measure(&mut self, _cx: &mut MeasureCx<'_>, _avail: AvailableSpace) -> egui::Vec2 {
        egui::vec2(80.0, 30.0)
    }
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        let r = ui.interact(cx.rect(), cx.id(), egui::Sense::click());
        if r.clicked() {
            // the default action (counting) only when no filter prevented it
            if cx.dispatch(&self.tapped, ()).prevented() {
                self.prevented += 1;
            } else {
                self.taps += 1;
            }
        }
        r
    }
}

/// A host with one node part: a shadow label built in `prepare`, drawn through `cx.children`.
#[svelters::widget(namespace = "ev1test", tag = "parted-host", caps = CONTAINER | PREPARE)]
pub struct Parted {
    #[prop]
    title: String,
    #[state]
    part: Option<NodeId>,
}

impl Widget for Parted {
    fn measure(&mut self, _cx: &mut MeasureCx<'_>, _avail: AvailableSpace) -> egui::Vec2 {
        egui::vec2(10.0, 10.0)
    }
    fn prepare(&mut self, cx: &mut PrepareCx<'_>) {
        if self.part.is_some() {
            return;
        }
        let host = cx.host();
        let mut m = cx.scene();
        let l = m.create_tag("label").unwrap();
        m.node(l).unwrap().set(&keys::label::TEXT, "the part".to_string()).unwrap();
        m.insert_shadow(host, l, None).unwrap();
        self.part = Some(l);
    }
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        let r = ui.interact(cx.rect(), cx.id(), egui::Sense::hover());
        cx.children(ui);
        r
    }
}

/// Vetoes (`Prevent`) the `tapped` events of its host's subtree while `active`, consumes pointer
/// presses while `swallow`; records what it saw.
#[svelters::behavior(namespace = "ev1test", name = "veto")]
pub struct Veto {
    #[prop(default = true)]
    active: bool,
    #[prop(default = false)]
    swallow: bool,
    #[state]
    seen: Vec<(String, EventPhase)>,
}

impl Behavior for Veto {
    fn after_ui(&mut self, _host: NodeRef<'_>, _resp: &egui::Response, _cx: &mut BehaviorCx<'_>) {}
    fn filter(&mut self, _host: NodeRef<'_>, ev: &SceneEvent, _cx: &mut BehaviorCx<'_>) -> Flow {
        self.seen.push((ev.name().to_string(), ev.phase));
        match ev.kind {
            EventKind::Signal("tapped") if self.active => Flow::Prevent,
            EventKind::PointerDown if self.swallow => Flow::Consume,
            _ => Flow::Pass,
        }
    }
}

/// A `use:` behaviour whose click is a user event of its own (`BehaviorCx::dispatch`).
#[svelters::behavior(namespace = "ev1test", name = "tapper")]
pub struct Tapper {
    #[event]
    tapped: Event<()>,
    #[state]
    prevented: Vec<bool>,
}

impl Behavior for Tapper {
    fn sense(&self) -> egui::Sense {
        egui::Sense::click()
    }
    fn after_ui(&mut self, _host: NodeRef<'_>, resp: &egui::Response, cx: &mut BehaviorCx<'_>) {
        if resp.clicked() {
            let o = cx.dispatch(&self.tapped, ());
            self.prevented.push(o.prevented());
        }
    }
}

fn cfg() -> EmbedConfig {
    EmbedConfig { host: HostConfig { echo_console: false, echo_errors: false, quit_on_last_window: false, ..HostConfig::default() }, ..EmbedConfig::default() }
}

fn slot_node(s: SlotId) -> NodeId {
    NodeId::from_raw(s.handle()).unwrap()
}

fn rig() -> (EmbedHarness, SceneHandle, SlotId) {
    let mut h = EmbedHarness::new(egui::vec2(400.0, 300.0), cfg()).unwrap();
    let slot = h.embed.create_slot("ev1", SlotOptions::fill());
    let scene = h.embed.backend().scene();
    h.set_ui(move |e, ui| {
        e.show_slot(ui, slot);
    });
    (h, scene, slot)
}

fn app() -> ProviderId {
    ProviderId::named("ev1.test")
}

fn click(h: &mut EmbedHarness, scene: &SceneHandle, n: NodeId) {
    let pos = scene.read(|s| s.get(n).unwrap().drawn().expect("drawn").center());
    h.harness.hover_at(pos);
    for pressed in [true, false] {
        h.harness.event(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
    }
    h.run();
}

#[test]
fn a_widget_dispatch_runs_the_filters_before_its_default_action() {
    let (mut h, scene, slot) = rig();
    let (col, veto, pad) = scene
        .write(app(), |m| {
            let col = m.create_tag("box").unwrap();
            m.insert(slot_node(slot), col, None).unwrap();
            let veto = m.create_tag("ev1test.veto").unwrap();
            m.insert(col, veto, None).unwrap(); // attached to the box
            let pad = m.create_tag("tap-pad").unwrap();
            m.insert(col, pad, None).unwrap();
            (col, veto, pad)
        })
        .unwrap();
    h.run();
    let pad_n = |f: fn(&TapPad) -> u32| scene.read(|s| s.get(pad).unwrap().widget::<TapPad, _>(f).unwrap());
    let seen = || scene.read(|s| s.get(veto).unwrap().widget::<Veto, _>(|v| v.seen.clone()).unwrap());

    // the box's behaviour vetoes the pad's `tapped` on the way down and up: no default action
    click(&mut h, &scene, pad);
    assert_eq!((pad_n(|p| p.taps), pad_n(|p| p.prevented)), (0, 1));
    let s = seen();
    assert!(s.contains(&("tapped".into(), EventPhase::Capture)) && s.contains(&("tapped".into(), EventPhase::Bubble)), "{s:?}");
    assert!(s.contains(&("pointerdown".into(), EventPhase::Capture)), "the DOM-style events pass it too: {s:?}");

    // inactive: the default runs
    scene.write(app(), |m| m.node(veto).unwrap().set_by_name("active", &svelters_abi::Value::Bool(false)).unwrap()).unwrap();
    click(&mut h, &scene, pad);
    assert_eq!((pad_n(|p| p.taps), pad_n(|p| p.prevented)), (1, 1));

    // a registered filter on the pad's subtree consumes it at the target
    let at: Rc<RefCell<Vec<(NodeId, EventPhase)>>> = Rc::default();
    let at2 = at.clone();
    let f = scene.register_filter(FilterScope::Subtree(pad), move |ev: &SceneEvent, _s: SceneRef<'_>| {
        if ev.kind == EventKind::Signal("tapped") {
            at2.borrow_mut().push((ev.current, ev.phase));
            return Flow::Consume;
        }
        Flow::Pass
    });
    click(&mut h, &scene, pad);
    assert_eq!((pad_n(|p| p.taps), pad_n(|p| p.prevented)), (1, 2));
    assert_eq!(*at.borrow(), vec![(pad, EventPhase::Target)]);
    f.unregister();

    // a behaviour's filter consumes the press: egui never sees it, the pad is not clicked at all
    scene.write(app(), |m| m.node(veto).unwrap().set_by_name("swallow", &svelters_abi::Value::Bool(true)).unwrap()).unwrap();
    let n = seen().len();
    click(&mut h, &scene, pad);
    assert_eq!((pad_n(|p| p.taps), pad_n(|p| p.prevented)), (1, 2), "no click reached the pad");
    let s = seen();
    assert_eq!(s[n..].iter().filter(|e| e.0 == "pointerdown").count(), 1, "consumed on the way down: {:?}", &s[n..]);
    assert!(!s[n..].iter().any(|e| e.0 == "tapped" || e.0 == "click"), "{:?}", &s[n..]);
    let _ = col;
}

#[test]
fn a_behaviour_dispatch_and_parts_retarget_to_their_host() {
    let (mut h, scene, slot) = rig();
    let (label, tapper, parted) = scene
        .write(app(), |m| {
            let col = m.create_tag("box").unwrap();
            m.insert(slot_node(slot), col, None).unwrap();
            let veto = m.create_tag("ev1test.veto").unwrap();
            m.insert(col, veto, None).unwrap();
            let label = m.create_tag("label").unwrap();
            m.node(label).unwrap().set(&keys::label::TEXT, "tap me".to_string()).unwrap();
            m.insert(col, label, None).unwrap();
            let tapper = m.create_tag("ev1test.tapper").unwrap();
            m.insert(label, tapper, None).unwrap();
            let parted = m.create_tag("parted-host").unwrap();
            m.insert(col, parted, None).unwrap();
            (label, tapper, parted)
        })
        .unwrap();
    h.run();
    h.run();
    // BehaviorCx::dispatch: the box's veto prevents the behaviour's own event
    click(&mut h, &scene, label);
    assert_eq!(scene.read(|s| s.get(tapper).unwrap().widget::<Tapper, _>(|t| t.prevented.clone()).unwrap()), vec![true]);

    // a click on the node part: Rust sees the part, Svelte the host
    let seen: Rc<RefCell<Vec<(NodeId, NodeId)>>> = Rc::default();
    let s2 = seen.clone();
    let _f = scene.register_filter(FilterScope::All, move |ev: &SceneEvent, _s: SceneRef<'_>| {
        if ev.kind == EventKind::Click {
            s2.borrow_mut().push((ev.target_node(), ev.light_target));
        }
        Flow::Pass
    });
    let part = scene.read(|s| s.get(parted).unwrap().widget::<Parted, _>(|p| p.part).unwrap()).expect("the part was built");
    assert!(scene.read(|s| s.get(part).unwrap().is_shadow()));
    click(&mut h, &scene, part);
    assert_eq!(*seen.borrow(), vec![(part, parted)]);
}
