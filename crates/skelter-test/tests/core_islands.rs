//! `CoreIslands`: a skelter-core document with an `egui-island` node, an island mounted through
//! `IslandHost`, driven by the harness. Queries reach the egui button inside the island's grafted tree, a
//! click goes through skelter-core's dispatch (hit testing, the kind's input forwarding) into egui, and an
//! AccessKit action on the island's tree reaches the island.

use skelter::backends::CoreDocument;
use skelter_base::geom::{Rect, Scale, Size};
use skelter_base::ids::{NodeId, TextureId};
use skelter_core::testkinds::{BoxKind, FixedText};
use skelter_core::{Document, KindId, KindRegistry, StyleInput};
use skelter_egui_island::{EGUI_ISLAND, EguiIsland, IslandDesc, IslandHost, island_tree_id};
use skelter_test::{CoreIslands, Harness, Role};
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

const BOX: KindId = KindId::of("box");

type Ui = Box<dyn FnMut(NodeId, &mut egui::Ui)>;

fn rig() -> (Harness<CoreIslands<Ui>>, Rc<Cell<u32>>) {
    let mut reg = KindRegistry::new();
    reg.register::<BoxKind>().unwrap();
    IslandHost::register(&mut reg).unwrap();
    let mut doc = Document::new(Arc::new(reg), Box::new(FixedText));
    doc.set_text_shaper(Box::new(FixedText));
    let clicks = Rc::new(Cell::new(0));
    let seen = clicks.clone();
    let ui: Ui = Box::new(move |_, ui| {
        if ui.button("Pane button").clicked() {
            seen.set(seen.get() + 1);
        }
    });
    let mut h = skelter_test::HarnessBuilder::new()
        .with_size(Size::new(400.0, 300.0))
        .with_accessibility_check(false)
        .build_core_islands(CoreDocument::new(doc, BOX), ui);
    let window = h.window();
    let doc = h.state_mut().doc();
    let root = doc.window_root(window).expect("the harness window's root");
    let node = doc.create(EGUI_ISLAND);
    doc.set_style(
        node,
        StyleInput {
            inline: Some("width: 300px; height: 200px".into()),
            ..Default::default()
        },
    );
    doc.insert(root, node, None, None).unwrap();
    let island = EguiIsland::new(IslandDesc {
        texture: TextureId(9),
        tree: island_tree_id(0),
        rect: Rect::ZERO,
        scale: Scale(1.0),
        renderer: Default::default(),
    });
    IslandHost::mount(doc, node, island).unwrap_or_else(|_| panic!("mounts"));
    h.run();
    (h, clicks)
}

#[test]
fn queries_reach_widgets_in_a_mounted_island() {
    let (h, _) = rig();
    let b = h.get_by_role_and_label(Role::Button, "Pane button");
    assert_ne!(b.tree(), skelter_base::accesskit::TreeId::ROOT);
    assert!(b.bounds().is_some_and(|r| r.max.x <= 300.0 && r.max.y <= 200.0));
}

#[test]
fn a_click_goes_through_the_document_into_egui() {
    let (mut h, clicks) = rig();
    h.get_by_label("Pane button").click();
    h.run();
    assert_eq!(clicks.get(), 1);
}

#[test]
fn accesskit_actions_on_the_island_tree_reach_it() {
    let (mut h, clicks) = rig();
    h.get_by_label("Pane button").click_accesskit();
    h.run();
    assert_eq!(clicks.get(), 1);
}
