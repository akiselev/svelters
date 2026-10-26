//! `CoreIslands`: a skelter-core document with an `egui-island` node, an island mounted through
//! `IslandHost`, driven by the harness. Queries reach the egui button inside the island's grafted tree, a
//! click goes through skelter-core's dispatch (hit testing, the kind's input forwarding) into egui, and an
//! AccessKit action on the island's tree reaches the island. The island's AccessKit subtree reaches the
//! platform adapter through the commit (`Commit::a11y_subtrees`), and skelter-core's in-app drag over the
//! island node becomes egui's drag and drop (`IslandHost::event_tap` on the `CoreDocument`).

use skelter::backends::CoreDocument;
use skelter_base::event::{InputEvent, Modifiers, MouseButton, PointerEvent, PointerPhase, PointerType};
use skelter_base::geom::{Point, Rect, Scale, Size, Vec2};
use skelter_base::ids::{NodeId, TextureId};
use skelter_core::testkinds::{BoxKind, FixedText};
use skelter_core::{Document, KindId, KindRegistry, StyleInput, Value, common};
use skelter_egui_island::{EGUI_ISLAND, EguiIsland, IslandDesc, IslandDrag, IslandHost, island_tree_id};
use skelter_test::{CoreIslands, Harness, PlatformCall, Role};
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

const BOX: KindId = KindId::of("box");

type Ui = Box<dyn FnMut(NodeId, &mut egui::Ui)>;

fn rig() -> (Harness<CoreIslands<Ui>>, Rc<Cell<u32>>) {
    let clicks = Rc::new(Cell::new(0));
    let seen = clicks.clone();
    let ui: Ui = Box::new(move |_, ui| {
        if ui.button("Pane button").clicked() {
            seen.set(seen.get() + 1);
        }
    });
    (rig_with(ui).0, clicks)
}

/// A 400 × 300 window whose root's first child is a 300 × 200 `egui-island` node with an island mounted.
fn rig_with(ui: Ui) -> (Harness<CoreIslands<Ui>>, NodeId) {
    let mut reg = KindRegistry::new();
    reg.register::<BoxKind>().unwrap();
    IslandHost::register(&mut reg).unwrap();
    let mut doc = Document::new(Arc::new(reg), Box::new(FixedText));
    doc.set_text_shaper(Box::new(FixedText));
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
    (h, node)
}

fn pointer(phase: PointerPhase, pos: Point, held: bool) -> InputEvent {
    InputEvent::Pointer(PointerEvent {
        phase,
        pointer: 0,
        kind: PointerType::Mouse,
        button: matches!(phase, PointerPhase::Down | PointerPhase::Up).then_some(MouseButton::Primary),
        buttons: u32::from(held),
        pos,
        modifiers: Modifiers::default(),
        pressure: 0.5,
        tilt: Vec2::ZERO,
        history: Default::default(),
    })
}

#[test]
fn queries_reach_widgets_in_a_mounted_island() {
    let (h, _) = rig();
    let b = h.get_by_role_and_label(Role::Button, "Pane button");
    assert_ne!(b.tree(), skelter_base::accesskit::TreeId::ROOT);
    assert!(b.bounds().is_some_and(|r| r.max.x <= 300.0 && r.max.y <= 200.0));
}

#[test]
fn island_widgets_are_reported_where_they_are_drawn_when_the_island_is_offset() {
    // As in an app: the island node is laid out off the window's origin before the island is
    // mounted, so its first AccessKit tree (with its root) is built there.
    let mut reg = KindRegistry::new();
    reg.register::<BoxKind>().unwrap();
    IslandHost::register(&mut reg).unwrap();
    let mut doc = Document::new(Arc::new(reg), Box::new(FixedText));
    doc.set_text_shaper(Box::new(FixedText));
    let ui: Ui = Box::new(|_, ui| {
        let _ = ui.button("Pane button");
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
            inline: Some("width: 300px; height: 200px; margin-left: 60px; margin-top: 40px".into()),
            ..Default::default()
        },
    );
    doc.insert(root, node, None, None).unwrap();
    h.run();
    let island = EguiIsland::new(IslandDesc {
        texture: TextureId(9),
        tree: island_tree_id(0),
        rect: Rect::ZERO,
        scale: Scale(1.0),
        renderer: Default::default(),
    });
    IslandHost::mount(h.state_mut().doc(), node, island).unwrap_or_else(|_| panic!("mounts"));
    h.run();
    let at = h.state_mut().doc().world_rect(node).expect("the island node is laid out");
    assert_eq!((at.min.x, at.min.y), (60.0, 40.0));
    let b = h.get_by_role_and_label(Role::Button, "Pane button").bounds().expect("bounds");
    // The subtree follows its node once: the button sits at the island's top left, not twice its offset.
    assert!(
        b.min.x >= at.min.x && b.min.y >= at.min.y && b.max.x <= at.max.x && b.max.y <= at.max.y,
        "the button {b:?} is inside its island {at:?}"
    );
    assert!(b.min.x < at.min.x + 30.0 && b.min.y < at.min.y + 30.0, "near the island's top left: {b:?}");
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

#[test]
fn the_island_subtree_reaches_the_platform_adapter_under_its_node() {
    let (h, node) = rig_with(Box::new(|_, ui| {
        let _ = ui.button("Pane button");
    }));
    let tree = island_tree_id(node.0);
    assert_eq!(h.get_by_label("Pane button").tree(), tree);
    let updates: Vec<_> = h
        .calls()
        .iter()
        .filter_map(|c| match c {
            PlatformCall::UpdateA11y(w, u) if *w == h.window() => Some(u),
            _ => None,
        })
        .collect();
    // The host tree's graft node, then the island's own update, both through `update_a11y`.
    let graft = updates
        .iter()
        .position(|u| u.tree_id == skelter_base::accesskit::TreeId::ROOT && u.nodes.iter().any(|(_, n)| n.tree_id() == Some(tree)))
        .expect("the host tree carries the graft node");
    let sub = updates
        .iter()
        .position(|u| u.tree_id == tree && u.nodes.iter().any(|(_, n)| n.role() == Role::Button))
        .expect("the island's subtree reached the platform");
    assert!(graft <= sub, "the graft node is sent first");
}

#[test]
fn a_skelter_drag_over_the_island_reaches_egui_drag_and_drop() {
    let zone = Rc::new(Cell::new(egui::Rect::NOTHING));
    let got: Rc<Cell<Option<String>>> = Rc::new(Cell::new(None));
    let (z, g) = (zone.clone(), got.clone());
    let (mut h, node) = rig_with(Box::new(move |_, ui| {
        let (inner, payload) = ui.dnd_drop_zone::<IslandDrag, _>(egui::Frame::default(), |ui| {
            ui.set_min_size(egui::vec2(120.0, 80.0));
            ui.label("drop here");
        });
        z.set(inner.response.rect);
        if let Some(p) = payload {
            p.accept();
            g.set(Some(format!("{}={}", p.drag.ty, p.drag.data)));
        }
    }));
    // A skelter drag source below the island; the island node takes `cad/*`.
    let window = h.window();
    let doc = h.state_mut().doc();
    let root = doc.window_root(window).unwrap();
    let src = doc.create(BOX);
    doc.set_style(
        src,
        StyleInput {
            inline: Some("width: 60px; height: 40px".into()),
            ..Default::default()
        },
    );
    doc.insert(root, src, None, None).unwrap();
    doc.set_prop(src, common::DRAG_TYPE, Value::from("cad/part")).unwrap();
    doc.set_prop(src, common::DRAG_DATA, Value::from("42")).unwrap();
    doc.set_prop(node, common::DROP_TYPES, Value::from("cad/*")).unwrap();
    h.run();

    let doc = h.state_mut().doc();
    let s = doc.world_rect(src).unwrap();
    let origin = doc.world_rect(node).unwrap().min;
    let from = Point::new(s.min.x + 10.0, s.min.y + 10.0);
    let to = Point::new(origin.x + zone.get().center().x, origin.y + zone.get().center().y);
    h.event(pointer(PointerPhase::Move, from, false));
    h.event(pointer(PointerPhase::Down, from, true));
    h.event(pointer(PointerPhase::Move, Point::new(from.x + 20.0, from.y), true));
    h.event(pointer(PointerPhase::Move, to, true));
    h.run();
    let ctx = IslandHost::island_mut(h.state_mut().doc(), node).unwrap().context().clone();
    assert!(egui::DragAndDrop::has_any_payload(&ctx), "core's drag-enter reached the island");

    h.event(pointer(PointerPhase::Up, to, false));
    h.run();
    assert_eq!(got.take().as_deref(), Some("cad/part=42"), "the drop reached the egui drop zone");
    assert!(!egui::DragAndDrop::has_any_payload(&ctx), "egui's drag ended with the drop");
}
