//! SG4 (ARCH-SCENE §7.3, §4.4, §14 SG4): provider kinds with shadow children, their own layout
//! (`Caps::CUSTOM_LAYOUT`), `prepare` (`Caps::PREPARE`), read access to the scene while drawn
//! (payload take-out), and a provider panic that faults one root only.

use std::path::PathBuf;
use svelters_egui::embed::{EmbedConfig, EmbedEvent, MountSpec, Props, RootState, SlotId, SlotOptions};
use svelters_egui::harness::EmbedHarness;
use svelters_egui::prelude::*;
use svelters_egui::scene::{NodeId, ProviderId, SceneError, keys};
use svelters_egui::taffy::{LayoutInput, LayoutOutput, RunMode, Size};
use egui_kittest::kittest::Queryable;
use svelters_host::HostConfig;

/// Builds `count` shadow labels in `prepare` and lays them out on a diagonal itself: row `k` at
/// (30k, 20k). Draws them through `cx.children`.
#[svelters::widget(namespace = "sg4test", tag = "stair-view", caps = CUSTOM_LAYOUT | PREPARE)]
pub struct StairView {
    /// How many rows.
    #[prop]
    count: f64,
    #[state]
    rows: Vec<NodeId>,
    #[state]
    prepared: u32,
    #[state]
    drawn: usize,
    #[state]
    saw_scene: usize,
    #[state]
    self_visible: bool,
}

impl Widget for StairView {
    fn measure(&mut self, _cx: &mut MeasureCx<'_>, _avail: AvailableSpace) -> egui::Vec2 {
        egui::Vec2::ZERO
    }

    fn prepare(&mut self, cx: &mut PrepareCx<'_>) {
        self.prepared += 1;
        let want = self.count.max(0.0) as usize;
        let host = cx.host();
        let mut m = cx.scene();
        while self.rows.len() < want {
            let k = self.rows.len();
            let l = m.create_tag("label").unwrap();
            m.node(l).unwrap().set(&keys::label::TEXT, format!("step {k}")).unwrap();
            m.insert_shadow(host, l, None).unwrap();
            self.rows.push(l);
        }
        while self.rows.len() > want {
            let l = self.rows.pop().unwrap();
            m.destroy(l).unwrap();
        }
    }

    fn layout(&mut self, cx: &mut LayoutCx<'_>, inputs: LayoutInput) -> LayoutOutput {
        let kids = cx.children().to_vec();
        let mut extent = egui::Vec2::ZERO;
        let mut placed = Vec::new();
        for (k, &c) in kids.iter().enumerate() {
            let size = cx.measure(c, egui::vec2(1000.0, 1000.0));
            let at = egui::pos2(30.0 * k as f32, 20.0 * k as f32);
            extent = extent.max(at.to_vec2() + size);
            placed.push((c, at, size));
        }
        if inputs.run_mode == RunMode::PerformLayout {
            for (c, at, size) in placed {
                cx.place(c, at, size);
            }
        }
        LayoutOutput::from_outer_size(Size { width: inputs.known_dimensions.width.unwrap_or(extent.x), height: inputs.known_dimensions.height.unwrap_or(extent.y) })
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        // payload take-out: the whole scene is readable while `self` is borrowed mutably
        self.saw_scene = cx.scene().len();
        self.self_visible = cx.node().widget::<StairView, _>(|_| ()).is_some();
        let resp = ui.interact(cx.rect(), cx.id(), egui::Sense::hover());
        cx.children(ui);
        self.drawn = cx.child_ids().len();
        resp
    }
}

/// A container provider whose shadow children get flexbox from its style (no custom layout).
#[svelters::widget(namespace = "sg4test", tag = "shadow-column", caps = CONTAINER)]
pub struct ShadowColumn {
    #[prop]
    title: String,
}

impl Widget for ShadowColumn {
    fn measure(&mut self, _cx: &mut MeasureCx<'_>, _avail: AvailableSpace) -> egui::Vec2 {
        egui::vec2(10.0, 10.0)
    }
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        let resp = ui.interact(cx.rect(), cx.id(), egui::Sense::hover());
        cx.children(ui);
        resp
    }
}

/// Panics in `ui` once armed.
#[svelters::widget(namespace = "sg4test", tag = "boom-box")]
pub struct BoomBox {
    #[prop]
    armed: bool,
}

impl Widget for BoomBox {
    fn measure(&mut self, _cx: &mut MeasureCx<'_>, _avail: AvailableSpace) -> egui::Vec2 {
        egui::vec2(40.0, 20.0)
    }
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        assert!(!self.armed, "sg4: boom-box armed");
        ui.interact(cx.rect(), cx.id(), egui::Sense::hover())
    }
}

fn cfg() -> EmbedConfig {
    EmbedConfig { host: HostConfig { echo_console: false, echo_errors: false, quit_on_last_window: false, ..HostConfig::default() }, ..EmbedConfig::default() }
}

fn slot_node(s: SlotId) -> NodeId {
    NodeId::from_raw(s.handle()).unwrap()
}

/// ARCH-SCENE §14 SG4: "a provider container lays out shadow children" (custom layout, prepare,
/// flexbox for a container provider), drawn by the provider through `cx.children`.
#[test]
fn a_provider_container_lays_out_shadow_children() {
    let mut h = EmbedHarness::new(egui::vec2(400.0, 300.0), cfg()).unwrap();
    let e = h.embed.clone();
    let slot = e.create_slot("stairs", SlotOptions::fill());
    let scene = e.backend().scene();
    let app = ProviderId::named("sg4.test");
    let (stair, column) = scene
        .write(app, |m| {
            let s = m.create_tag("stair-view").unwrap();
            m.node(s).unwrap().set_by_name("count", &svelters_abi::Value::F64(3.0)).unwrap();
            m.insert(slot_node(slot), s, None).unwrap();
            let c = m.create_tag("shadow-column").unwrap();
            m.insert(slot_node(slot), c, None).unwrap();
            (s, c)
        })
        .unwrap();
    h.set_ui(move |e, ui| {
        e.show_slot(ui, slot);
    });
    h.run();
    let st = |f: fn(&StairView) -> usize| scene.read(|s| s.get(stair).unwrap().widget::<StairView, _>(f).unwrap());
    assert!(st(|w| w.prepared as usize) >= 1);
    assert_eq!(st(|w| w.rows.len()), 3);
    assert_eq!(st(|w| w.drawn), 3, "drawn through cx.children");
    assert!(st(|w| w.saw_scene) > 3, "the scene was readable from ui");
    assert_eq!(st(|w| w.self_visible as usize), 0, "its own payload is out while it draws");
    let check = |n: usize| {
        scene.read(|s| {
            let host = s.get(stair).unwrap();
            let kids: Vec<_> = host.children().collect();
            assert_eq!(kids.len(), n);
            for (k, c) in kids.iter().enumerate() {
                assert!(c.is_shadow(), "a shadow child");
                assert_eq!(c.parent().unwrap().id(), stair);
                let at = c.rect().min - host.rect().min;
                assert_eq!(at, egui::vec2(30.0 * k as f32, 20.0 * k as f32), "row {k} placed by the provider's layout");
                assert!(c.rect().width() > 10.0, "measured: {:?}", c.rect());
                let d = c.drawn().expect("drawn");
                assert!(s.hit_test(egui::ViewportId::ROOT, d.center()).iter().any(|t| t.node() == c.id()), "in the hit list");
            }
            assert_eq!(host.rect().height(), 20.0 * (n as f32 - 1.0) + kids.last().unwrap().rect().height(), "its size is its extent");
        })
    };
    check(3);
    assert!(scene.read(|s| s.stats().custom_layouts) > 0);

    // prepare rebinds: more rows, then fewer (the extra ones destroyed)
    let set_count = |n: f64| scene.write(app, |m| m.node(stair).unwrap().set_by_name("count", &svelters_abi::Value::F64(n)).unwrap()).unwrap();
    set_count(5.0);
    h.run();
    h.run();
    check(5);
    let gone: Vec<NodeId> = scene.read(|s| s.get(stair).unwrap().children().skip(2).map(|c| c.id()).collect());
    set_count(2.0);
    h.run();
    h.run();
    check(2);
    assert!(scene.read(|s| gone.iter().all(|&g| s.get(g).is_none())));

    // shadow children of a container provider get flexbox (a column by default)
    let (a, b) = scene
        .write(app, |m| {
            let mk = |m: &mut SceneMut<'_>, t: &str| {
                let l = m.create_tag("label").unwrap();
                m.node(l).unwrap().set(&keys::label::TEXT, t.to_string()).unwrap();
                m.insert_shadow(column, l, None).unwrap();
                l
            };
            (mk(m, "first"), mk(m, "second"))
        })
        .unwrap();
    h.run();
    scene.read(|s| {
        let (ra, rb, rc) = (s.get(a).unwrap().rect(), s.get(b).unwrap().rect(), s.get(column).unwrap().rect());
        assert_eq!(ra.min, rc.min);
        assert_eq!(rb.min.y, ra.max.y, "stacked: {ra:?} {rb:?}");
        assert!(rc.height() >= ra.height() + rb.height(), "the container wraps them: {rc:?}");
        assert!(s.get(b).unwrap().drawn().is_some());
    });

    // shadow rules: a built-in box hosts no shadow children; remove takes one out; destroying the
    // host frees its shadow subtrees
    let r = scene.write(app, |m| {
        let x = m.create_tag("label").unwrap();
        m.insert_shadow(slot_node(slot), x, None)
    });
    assert!(matches!(r, Ok(Err(SceneError::Invalid(_)))), "{r:?}");
    scene.write(app, |m| m.remove(b).unwrap()).unwrap();
    assert!(scene.read(|s| !s.get(b).unwrap().is_shadow() && s.get(b).unwrap().parent().is_none()));
    let rows: Vec<NodeId> = scene.read(|s| s.get(stair).unwrap().children().map(|c| c.id()).collect());
    scene.write(app, |m| m.destroy(stair).unwrap()).unwrap();
    assert!(scene.read(|s| rows.iter().all(|&g| s.get(g).is_none())), "shadow rows freed with their host");
    h.run();
    scene.read(|s| s.validate()).unwrap();
}

fn bundle() -> PathBuf {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/embed-egui/dist/embed.js");
    assert!(p.exists(), "{} is missing: run `pnpm build` (needs `cargo build -p svelters-compiler` first)", p.display());
    p
}

/// ARCH-SCENE §14 SG4: "a provider panic faults one root" (ARCH-EMBED §6.2): the widget is dropped
/// and replaced, its root shows the fault UI, the other root keeps working.
#[test]
fn a_provider_panic_faults_one_root() {
    let mut h = EmbedHarness::new(egui::vec2(600.0, 300.0), cfg()).unwrap();
    let e = h.embed.clone();
    let entry = e.load(bundle()).unwrap();
    let (sa, sb) = (e.create_slot("a", SlotOptions::fill()), e.create_slot("b", SlotOptions::fill()));
    let ra = e.mount(MountSpec::new(entry, "Pane", sa, "pane.a").props(Props::new().with("name", "alpha"))).unwrap();
    let rb = e.mount(MountSpec::new(entry, "Pane", sb, "pane.b").props(Props::new().with("name", "beta"))).unwrap();
    h.set_ui(move |e, ui| {
        ui.columns(2, |c| {
            e.show_slot(&mut c[0], sa);
            e.show_slot(&mut c[1], sb);
        });
    });
    h.run();
    let scene = e.backend().scene();
    let app = ProviderId::named("sg4.test");
    // a Rust-built provider node next to root a's content
    let boom = scene
        .write(app, |m| {
            let b = m.create_tag("boom-box").unwrap();
            m.insert(slot_node(sa), b, None).unwrap();
            b
        })
        .unwrap();
    h.run();
    assert!(matches!(e.root_info(ra).unwrap().state, RootState::Live), "{:?}", e.root_info(ra).unwrap().state);
    e.take_events();
    scene.write(app, |m| m.node(boom).unwrap().set_by_name("armed", &svelters_abi::Value::Bool(true)).unwrap()).unwrap();
    h.run();
    h.run();
    assert!(matches!(e.root_info(ra).unwrap().state, RootState::Faulted(_)), "{:?}", e.root_info(ra).unwrap().state);
    assert!(matches!(e.root_info(rb).unwrap().state, RootState::Live), "the other root is unaffected");
    assert!(e.take_events().iter().any(|ev| matches!(ev, EmbedEvent::Faulted { root, .. } if *root == ra)));
    assert!(h.harness.query_by_label_contains("beta count").is_some(), "root b still draws");
    scene.read(|s| {
        assert_eq!(s.stats().provider_panics, 1);
        // the panicking widget was dropped; a fresh one (props at their defaults) took its place
        assert_eq!(s.get(boom).unwrap().widget::<BoomBox, _>(|w| w.armed), Some(false));
        s.validate().unwrap();
    });
}
