//! SG3 (ARCH-SCENE §7.4, R13): a provider kind with `Caps::HIT_TEST` answers `SceneRef::hit_test`
//! for the points inside it: `Hit::Part(key)` becomes `Target::Virtual(VNodeId { host, key })`,
//! `Hit::Miss` lets the point through to the node below. The kind is defined here, the way
//! cadatomic defines its 3D viewport or model tree, and built from Rust into embed slots.

use svelters_egui::embed::{EmbedConfig, SlotId, SlotOptions};
use svelters_egui::harness::EmbedHarness;
use svelters_egui::prelude::*;
use svelters_egui::scene::{NodeId, ProviderId, Target, VNodeId, keys};
use svelters_host::HostConfig;

/// Rows of 20 points on its left half; its right half is empty (the point goes through).
#[svelters::widget(namespace = "sg3test", tag = "row-strip", caps = HIT_TEST)]
pub struct RowStrip {
    /// Shown in the first row.
    #[prop]
    label: String,
}

impl Widget for RowStrip {
    fn measure(&mut self, _cx: &mut MeasureCx<'_>, _avail: AvailableSpace) -> egui::Vec2 {
        egui::vec2(200.0, 100.0)
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        let r = cx.rect();
        ui.painter().rect_filled(egui::Rect::from_min_size(r.min, egui::vec2(r.width() / 2.0, r.height())), 0.0, egui::Color32::DARK_GRAY);
        ui.allocate_rect(r, egui::Sense::hover())
    }

    fn hit_test(&self, cx: &HitCx, local: egui::Pos2) -> Hit {
        if local.x > cx.rect.width() / 2.0 { Hit::Miss } else { Hit::Part((local.y / 20.0) as u64) }
    }
}

#[test]
fn a_hit_test_provider_reports_parts_and_misses() {
    let cfg = EmbedConfig { host: HostConfig { echo_console: false, echo_errors: false, quit_on_last_window: false, ..HostConfig::default() }, ..EmbedConfig::default() };
    let mut h = EmbedHarness::new(egui::vec2(300.0, 200.0), cfg).unwrap();
    let e = h.embed.clone();
    let under = e.create_slot("under", SlotOptions::fill());
    let over = e.create_slot("over", SlotOptions::fill());
    let scene = e.backend().scene();
    let app = ProviderId::named("sg3.test");
    let add = |slot: SlotId, tag: &'static str| {
        scene
            .write(app, move |m| {
                let n = m.create_tag(tag).unwrap();
                m.insert(NodeId::from_raw(slot.handle()).unwrap(), n, None).unwrap();
                n
            })
            .unwrap()
    };
    let label = add(under, "label");
    scene.write(app, |m| m.node(label).unwrap().set(&keys::label::TEXT, "a label under the strip, long enough to reach its right half".to_string()).unwrap()).unwrap();
    let strip = add(over, "row-strip");
    h.set_ui(move |e, ui| {
        let r = ui.max_rect();
        ui.scope_builder(egui::UiBuilder::new().max_rect(r), |ui| e.show_slot(ui, under));
        ui.scope_builder(egui::UiBuilder::new().max_rect(r), |ui| e.show_slot(ui, over));
    });
    h.run();
    let (rect, lrect) = scene.read(|s| (s.get(strip).unwrap().drawn().unwrap(), s.get(label).unwrap().drawn().unwrap()));
    let top = egui::ViewportId::ROOT;
    // the left half: rows, as parts of the strip
    let p = rect.min + egui::vec2(10.0, 45.0);
    assert_eq!(scene.read(|s| s.hit_test(top, p)), vec![Target::Virtual(VNodeId { host: strip, key: 2 })]);
    assert_eq!(scene.read(|s| s.hit_node(top, p).map(|n| n.id())), Some(strip), "the host of the part");
    // the left half over the label: the strip's row is on top, the label under it
    let p = egui::pos2(rect.min.x + 10.0, lrect.center().y);
    assert_eq!(scene.read(|s| s.hit_test(top, p)), vec![Target::Virtual(VNodeId { host: strip, key: 0 }), Target::Node(label)]);
    // the right half over the label: a miss on the strip, the label is hit
    let p = egui::pos2(rect.min.x + rect.width() * 0.75, lrect.center().y);
    assert!(lrect.contains(p), "the label reaches the strip's right half: {lrect:?} {rect:?}");
    assert_eq!(scene.read(|s| s.hit_test(top, p)), vec![Target::Node(label)]);
}
