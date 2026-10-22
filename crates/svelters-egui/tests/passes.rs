//! SG4 (ARCH-SCENE §7.2–§7.5, §14 SG4; R16, R21, R23): state providers, scene passes, overlay
//! roots and `position: absolute`, from Rust only (no bundle). Provider kinds (shadow children,
//! custom layout, prepare, panics) are tested in `examples/egui-kinds/tests/passes.rs`.

use egui::{Pos2, ViewportId};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use svelters_egui::embed::{Busy, EmbedConfig, SlotId, SlotOptions};
use svelters_egui::harness::EmbedHarness;
use svelters_egui::harness::dock::SvelteDock;
use svelters_egui::prelude::*;
use svelters_egui::scene::{Change, ChangeFilter, ChangeMask, Damage, NodeId, ProviderId, RootIx, SceneHandle, keys};
use svelters_host::HostConfig;

fn cfg() -> EmbedConfig {
    EmbedConfig { host: HostConfig { echo_console: false, echo_errors: false, quit_on_last_window: false, ..HostConfig::default() }, ..EmbedConfig::default() }
}

fn slot_node(s: SlotId) -> NodeId {
    NodeId::from_raw(s.handle()).unwrap()
}

/// `n` labels in a box under `slot`, built from Rust.
fn labels(scene: &SceneHandle, slot: SlotId, n: usize) -> Vec<NodeId> {
    scene
        .write(ProviderId::named("sg4.test"), |m| {
            let col = m.create_tag("box").unwrap();
            m.insert(slot_node(slot), col, None).unwrap();
            (0..n)
                .map(|k| {
                    let l = m.create_tag("label").unwrap();
                    m.node(l).unwrap().set(&keys::label::TEXT, format!("row {k}")).unwrap();
                    m.insert(col, l, None).unwrap();
                    l
                })
                .collect()
        })
        .unwrap()
}

/// Selects (or clears) every node in `nodes` when kicked; can be told to want more rounds or to panic.
#[derive(Default)]
struct Selector {
    nodes: Vec<NodeId>,
    on: bool,
    rounds: u32,
    panic: bool,
    updates: u32,
}

impl StateProvider for Selector {
    fn name(&self) -> &'static str {
        "sg4.select"
    }
    fn update(&mut self, scene: &mut SceneMut<'_>, _cx: &ProviderCx<'_>) {
        self.updates += 1;
        assert!(!self.panic, "sg4: a provider that panics");
        self.rounds = self.rounds.saturating_sub(1);
        let on = self.on;
        for &n in &self.nodes {
            scene.node(n).unwrap().set_state(StateBits::SELECTED, on);
        }
    }
    fn wants(&self) -> bool {
        self.rounds > 0
    }
}

/// ARCH-SCENE §14 SG4: "a `StateProvider` toggles 5k bits in one batch with one repaint"; R23's
/// handle and R16's `Busy::PROVIDER`.
#[test]
fn a_state_provider_toggles_5k_bits_in_one_batch_with_one_repaint() {
    let mut h = EmbedHarness::new(egui::vec2(400.0, 300.0), cfg()).unwrap();
    let e = h.embed.clone();
    let slot = e.create_slot("rows", SlotOptions::fill());
    let scene = e.backend().scene();
    let rows = labels(&scene, slot, 5000);
    h.set_ui(move |e, ui| {
        e.show_slot(ui, slot);
    });
    h.run();
    assert!(e.settled().is_settled(), "{:?}", e.settled());
    let sel = e.register_state_provider(Selector { nodes: rows.clone(), ..Default::default() });
    assert!(e.settled().is_settled(), "a registered provider that does not want is idle");
    let obs = scene.observe(ChangeFilter::only(ChangeMask::STATE));

    // R23: with_mut marks the provider wanting and asks for a frame; R16: busy until it ran
    sel.with_mut(|p| p.on = true);
    assert!(sel.is_kicked());
    assert!(e.settled().busy.contains(Busy::PROVIDER), "{:?}", e.settled());
    let logics0 = e.stats().logics;
    let updates0 = scene.read(|s| s.stats().provider_updates);
    h.run();
    let logics = e.stats().logics - logics0;
    assert_eq!(scene.read(|s| s.stats().provider_updates) - updates0, 1, "one update");
    assert_eq!(sel.with(|p| p.updates), 1);
    assert!(logics >= 1 && logics <= 2, "one repaint (egui may run its pass twice): {logics} logics");
    assert!(e.settled().is_settled(), "{:?}", e.settled());
    assert_eq!(scene.read(|s| s.with_state(StateBits::SELECTED).count()), 5000);

    // one batch: one Batch marker, labelled with the provider's name, then 5000 state records
    let set = obs.take();
    let batches: Vec<&'static str> = set.records.iter().filter_map(|c| if let Change::Batch { label, .. } = c { Some(*label) } else { None }).collect();
    assert_eq!(batches, vec!["sg4.select"]);
    assert_eq!(set.records.iter().filter(|c| matches!(c, Change::State { .. })).count(), 5000);
    let ticks: std::collections::BTreeSet<_> = scene.read(|s| rows.iter().map(|&r| s.get(r).unwrap().changed()).collect());
    assert_eq!(ticks.len(), 1, "one tick for all 5000");

    // nothing more runs while it does not want
    h.run();
    assert_eq!(sel.with(|p| p.updates), 1);

    // wants() keeps it running, one update per logic, until it stops wanting (R16)
    sel.with_mut(|p| {
        p.on = false;
        p.rounds = 3;
    });
    h.run();
    assert_eq!(sel.with(|p| p.updates), 4, "kicked once, then two more rounds while it wanted");
    assert!(e.settled().is_settled());
    assert_eq!(scene.read(|s| s.with_state(StateBits::SELECTED).count()), 0);

    // a panic in update disables that provider only; the scene's batch is closed again
    let other = e.register_state_provider(Selector { nodes: rows[..10].to_vec(), ..Default::default() });
    sel.with_mut(|p| p.panic = true);
    other.with_mut(|p| p.on = true);
    h.run();
    assert_eq!(other.with(|p| p.updates), 1, "the other provider still ran");
    assert_eq!(scene.read(|s| s.with_state(StateBits::SELECTED).count()), 10);
    sel.with_mut(|p| p.panic = false);
    h.run();
    assert_eq!(sel.with(|p| p.updates), 5, "a faulted provider is not called again");
    assert!(e.settled().is_settled(), "a faulted provider does not keep the embed busy: {:?}", e.settled());
    let t = scene.read(|s| s.tick());
    scene.write(ProviderId::named("sg4.test"), |m| m.node(rows[0]).unwrap().set_state(StateBits::SELECTED, false)).unwrap();
    let t1 = scene.read(|s| s.tick());
    scene.write(ProviderId::named("sg4.test"), |m| m.node(rows[0]).unwrap().set_state(StateBits::SELECTED, true)).unwrap();
    assert!(t1 > t && scene.read(|s| s.tick()) > t1, "writes after the panic get their own ticks");
    other.unregister();
    sel.unregister();
    assert_eq!(scene.pass_counts().0, 0);
}

fn click(h: &mut EmbedHarness, pos: Pos2) {
    h.harness.hover_at(pos);
    h.run();
    for pressed in [true, false] {
        h.harness.event(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::default() });
    }
    h.run();
}

/// ARCH-SCENE §14 SG4: "an overlay root over a canvas passes pointer input through"; §10.1's
/// overlay labels as absolutely positioned boxes (taffy hoisting).
#[test]
fn an_overlay_root_over_a_canvas_passes_pointer_input_through() {
    let mut h = EmbedHarness::new(egui::vec2(400.0, 300.0), cfg()).unwrap();
    let e = h.embed.clone();
    let overlay = e.create_overlay("dims", egui::Order::Foreground, SlotOptions::fill());
    let scene = e.backend().scene();
    let app = ProviderId::named("sg4.test");
    // two labels placed by left/top (out of flow), and a checkbox that takes input
    let (a, b, check) = scene
        .write(app, |m| {
            let mk = |m: &mut SceneMut<'_>, tag: &str, text: &str, left: f64, top: f64| {
                let n = m.create_tag(tag).unwrap();
                let mut nm = m.node(n).unwrap();
                nm.set_by_name("text", &svelters_abi::Value::Str(text.into())).unwrap();
                nm.set(&keys::widget::POSITION, "absolute").unwrap();
                nm.set(&keys::widget::LEFT, Some(left)).unwrap();
                nm.set(&keys::widget::TOP, Some(top)).unwrap();
                m.insert(slot_node(overlay), n, None).unwrap();
                n
            };
            (mk(m, "label", "12.5 mm", 40.0, 30.0), mk(m, "label", "90°", 200.0, 120.0), mk(m, "checkbox", "lock", 40.0, 200.0))
        })
        .unwrap();
    let clicks = Rc::new(Cell::new(0));
    let overlay_rect = Rc::new(Cell::new(egui::Rect::NOTHING));
    let (c, or) = (clicks.clone(), overlay_rect.clone());
    h.set_ui(move |e, ui| {
        // the canvas: a click-sensing area over the whole panel (a 3D viewport's stand-in)
        let r = ui.max_rect();
        if ui.interact(r, egui::Id::unique("sg4-canvas"), egui::Sense::click()).clicked() {
            c.set(c.get() + 1);
        }
        let rect = r.shrink(10.0);
        or.set(rect);
        e.show_overlay(ui.ctx(), overlay, rect);
    });
    h.run();
    h.run();
    let rect = overlay_rect.get();
    let (ra, rb, rc) = scene.read(|s| (s.get(a).unwrap().drawn().unwrap(), s.get(b).unwrap().drawn().unwrap(), s.get(check).unwrap().drawn().unwrap()));
    // hoisted: each box is at its left/top in the overlay root (its containing block)
    assert_eq!(ra.min, rect.min + egui::vec2(40.0, 30.0), "{ra:?} in {rect:?}");
    assert_eq!(rb.min, rect.min + egui::vec2(200.0, 120.0), "{rb:?} in {rect:?}");
    assert_eq!(scene.read(|s| s.get(a).unwrap().rect().min), egui::pos2(40.0, 30.0));

    // a click on an overlay label reaches the canvas under it
    click(&mut h, ra.center());
    assert_eq!(clicks.get(), 1, "the label is click-through");
    click(&mut h, egui::pos2(rect.max.x - 5.0, rect.max.y - 5.0));
    assert_eq!(clicks.get(), 2, "the overlay's empty area is click-through");
    // the hit list still reports the label on top of the canvas (typed picking, MCP "what is here")
    assert_eq!(scene.read(|s| s.hit_node(ViewportId::ROOT, ra.center()).map(|n| n.id())), Some(a));

    // a node that takes input captures it while the pointer is over it
    let checked = || scene.read(|s| s.get(check).unwrap().value_of("checked").unwrap());
    assert_eq!(checked(), svelters_abi::Value::Bool(false));
    click(&mut h, rc.center());
    assert_eq!(checked(), svelters_abi::Value::Bool(true), "the checkbox got the click");
    assert_eq!(clicks.get(), 2, "the canvas did not");

    // moving a label (one batch of left/top writes, a camera change) moves it, and only it
    scene
        .write(app, |m| {
            let mut n = m.node(a).unwrap();
            n.set(&keys::widget::LEFT, Some(100.0)).unwrap();
            n.set(&keys::widget::TOP, Some(60.0)).unwrap();
        })
        .unwrap();
    h.run();
    let (ra2, rb2) = scene.read(|s| (s.get(a).unwrap().drawn().unwrap(), s.get(b).unwrap().drawn().unwrap()));
    assert_eq!(ra2.min, rect.min + egui::vec2(100.0, 60.0));
    assert_eq!(rb2, rb);
}

/// `position: absolute` inside a positioned box is placed in that box; a text change inside an
/// absolute box whose containing block is above its layout boundary still places it right.
#[test]
fn absolute_boxes_are_placed_in_their_containing_block() {
    let mut h = EmbedHarness::new(egui::vec2(400.0, 300.0), cfg()).unwrap();
    let e = h.embed.clone();
    let slot = e.create_slot("abs", SlotOptions::fill());
    let scene = e.backend().scene();
    let app = ProviderId::named("sg4.test");
    let (inner, abs, flow, fixed_box, deep) = scene
        .write(app, |m| {
            let top = slot_node(slot);
            let flow = m.create_tag("label").unwrap();
            m.node(flow).unwrap().set(&keys::label::TEXT, "in flow".to_string()).unwrap();
            m.insert(top, flow, None).unwrap();
            // a positioned box with a definite size (a layout boundary) at (50, 40)
            let inner = m.create_tag("box").unwrap();
            {
                let mut n = m.node(inner).unwrap();
                n.set(&keys::widget::POSITION, "relative").unwrap();
                n.set(&keys::widget::WIDTH, Some(200.0)).unwrap();
                n.set(&keys::widget::HEIGHT, Some(100.0)).unwrap();
                n.set(&keys::widget::LEFT, Some(50.0)).unwrap();
                n.set(&keys::widget::TOP, Some(40.0)).unwrap();
            }
            m.insert(top, inner, None).unwrap();
            let abs = m.create_tag("label").unwrap();
            {
                let mut n = m.node(abs).unwrap();
                n.set(&keys::label::TEXT, "in inner".to_string()).unwrap();
                n.set(&keys::widget::POSITION, "absolute").unwrap();
                n.set(&keys::widget::RIGHT, Some(0.0)).unwrap();
                n.set(&keys::widget::BOTTOM, Some(0.0)).unwrap();
            }
            m.insert(inner, abs, None).unwrap();
            // a static box with a definite size (a boundary) holding an absolute label whose
            // containing block is the root's top
            let fixed_box = m.create_tag("box").unwrap();
            {
                let mut n = m.node(fixed_box).unwrap();
                n.set(&keys::widget::WIDTH, Some(100.0)).unwrap();
                n.set(&keys::widget::HEIGHT, Some(50.0)).unwrap();
                n.set(&keys::widget::FLEX_SHRINK, 0.0).unwrap();
            }
            m.insert(top, fixed_box, None).unwrap();
            let deep = m.create_tag("label").unwrap();
            {
                let mut n = m.node(deep).unwrap();
                n.set(&keys::label::TEXT, "x".to_string()).unwrap();
                n.set(&keys::widget::POSITION, "absolute").unwrap();
                n.set(&keys::widget::LEFT, Some(300.0)).unwrap();
                n.set(&keys::widget::TOP, Some(10.0)).unwrap();
            }
            m.insert(fixed_box, deep, None).unwrap();
            (inner, abs, flow, fixed_box, deep)
        })
        .unwrap();
    h.set_ui(move |e, ui| {
        e.show_slot(ui, slot);
    });
    h.run();
    let rect = |n: NodeId| scene.read(|s| s.get(n).unwrap().rect());
    let ri = rect(inner);
    // relative: offset from its in-flow place (under the label) by left/top
    assert_eq!(ri.min, egui::pos2(50.0, rect(flow).max.y + 40.0), "{ri:?}");
    let ra = rect(abs);
    assert_eq!(ra.max, ri.max, "absolute right/bottom 0 in its positioned parent: {ra:?} {ri:?}");
    assert_eq!(rect(deep).min, egui::pos2(300.0, 10.0), "contained by the root, not its static parent");
    assert!(scene.read(|s| s.get(fixed_box).unwrap().is_layout_boundary()));
    // a text change inside the boundary with an escaping absolute box
    scene.write(app, |m| m.node(deep).unwrap().set(&keys::label::TEXT, "a much longer text".to_string()).unwrap()).unwrap();
    h.run();
    assert_eq!(rect(deep).min, egui::pos2(300.0, 10.0));
    assert!(rect(deep).width() > 60.0, "re-measured: {:?}", rect(deep));
    // and one inside the positioned boundary: its own relayout places it
    scene.write(app, |m| m.node(abs).unwrap().set(&keys::label::TEXT, "longer text in inner".to_string()).unwrap()).unwrap();
    h.run();
    assert_eq!(rect(abs).max, rect(inner).max);
    scene.read(|s| s.validate()).unwrap();
}

/// Records what a pass saw.
#[derive(Default)]
struct Log {
    overlays: Vec<(ViewportId, Option<RootIx>, egui::Rect)>,
    layouts: Vec<Damage>,
}

struct Marker {
    scope: PassScope,
    log: Rc<RefCell<Log>>,
}

impl ScenePass for Marker {
    fn scope(&self) -> PassScope {
        self.scope
    }
    fn overlay(&mut self, ui: &mut egui::Ui, scene: SceneRef<'_>, cx: &mut PassCx<'_>) {
        assert!(!scene.is_empty());
        ui.painter().rect_stroke(ui.max_rect(), 0.0, egui::Stroke::new(1.0, egui::Color32::RED), egui::StrokeKind::Inside);
        assert_eq!(ui.layer_id().order, egui::Order::Foreground);
        self.log.borrow_mut().overlays.push((cx.viewport(), cx.root(), ui.max_rect()));
    }
    fn after_layout(&mut self, _scene: SceneRef<'_>, damage: &Damage) {
        self.log.borrow_mut().layouts.push(damage.clone());
    }
}

/// R21: `PassScope::Viewport`/`AllViewports` passes run from `show_viewport_passes` (the dock's
/// `viewport_done`, through the thread-local entry point); `Root` passes after the root's walk;
/// `after_layout` sees each layout of a root in scope.
#[test]
fn scene_passes_run_per_root_and_per_viewport() {
    let mut h = EmbedHarness::new(egui::vec2(400.0, 300.0), cfg()).unwrap();
    let e = h.embed.clone();
    let slot = e.create_slot("p", SlotOptions::fill());
    let scene = e.backend().scene();
    let rows = labels(&scene, slot, 3);
    let r = scene.read(|s| s.get(rows[0]).unwrap().root());
    let (root_log, vp_log) = (Rc::new(RefCell::new(Log::default())), Rc::new(RefCell::new(Log::default())));
    let rp = e.register_pass(Marker { scope: PassScope::Root(r), log: root_log.clone() });
    let vpp = e.register_pass(Marker { scope: PassScope::AllViewports, log: vp_log.clone() });
    let ran = Rc::new(Cell::new(0));
    let rn = ran.clone();
    h.set_ui(move |e, ui| {
        e.show_slot(ui, slot);
        rn.set(rn.get() + SvelteDock::viewport_done(ui));
    });
    h.run();
    let passes = ran.get();
    assert!(passes >= 1, "the viewport pass ran");
    {
        let l = root_log.borrow();
        assert!(!l.overlays.is_empty());
        let (vp, root, rect) = l.overlays[0];
        assert_eq!((vp, root), (ViewportId::ROOT, Some(r)));
        assert!(rect.is_positive());
        assert!(!l.layouts.is_empty(), "the root's first layout");
        assert!(!l.layouts[0].relayout.is_empty());
        let v = vp_log.borrow();
        assert_eq!(v.overlays.len(), passes, "one overlay per viewport pass");
        assert!(v.overlays.iter().all(|&(vp, root, _)| vp == ViewportId::ROOT && root.is_none()));
        assert_eq!(v.layouts.len(), l.layouts.len(), "AllViewports covers the root's layouts too");
    }
    // a relayout of one label is seen as damage
    scene.write(ProviderId::named("sg4.test"), |m| m.node(rows[1]).unwrap().set(&keys::label::TEXT, "a longer row text".to_string()).unwrap()).unwrap();
    let n = root_log.borrow().layouts.len();
    h.run();
    assert!(root_log.borrow().layouts.len() > n, "the relayout is seen");
    // the free-function entry point (deferred viewports) reaches the current embed
    vpp.unregister();
    rp.unregister();
    let before = root_log.borrow().overlays.len();
    h.run_until_settled(5).unwrap();
    scene.write(ProviderId::named("sg4.test"), |m| m.node(rows[1]).unwrap().set(&keys::label::TEXT, "again".to_string()).unwrap()).unwrap();
    h.run();
    assert_eq!(root_log.borrow().overlays.len(), before, "unregistered passes do not run");
    assert_eq!(scene.pass_counts(), (0, 0));
}
