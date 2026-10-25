//! Islands mounted on presto-core nodes: input through `Document::dispatch`, the AccessKit graft on the
//! node, detach and remount keeping egui state, and presto-core's in-app drag dropping on an egui drop zone.

use presto_base::event::{InputEnvelope, InputEvent, Modifiers, MouseButton, PointerEvent, PointerPhase, PointerType};
use presto_base::geom::{Point, Rect, Scale, Size, Vec2};
use presto_base::ids::{InputSeq, NodeId, TextureId, WindowId};
use presto_base::time::Time;
use presto_core::testkinds::{BoxKind, FixedText};
use presto_core::{Document, JsEvent, KindId, KindRegistry, ListenFlags, StyleInput, Value, common};
use presto_egui_island::host::{MOUNTED, PASS};
use presto_egui_island::{EGUI_ISLAND, EguiIsland, IslandDesc, IslandDrag, IslandHost, island_tree_id};
use std::cell::Cell;
use std::sync::Arc;

const W: WindowId = WindowId(1);
const BOX: KindId = KindId::of("box");

struct Rig {
    doc: Document,
    root: NodeId,
    node: NodeId,
    now: u64,
    seq: u64,
}

impl Rig {
    /// A 400 × 300 window: a 300 × 200 island node first, then a 60 × 40 box.
    fn new() -> Rig {
        let mut reg = KindRegistry::new();
        reg.register::<BoxKind>().unwrap();
        IslandHost::register(&mut reg).unwrap();
        let mut doc = Document::new(Arc::new(reg), Box::new(FixedText));
        doc.set_text_shaper(Box::new(FixedText));
        let root = doc.create_window(W, BOX, Size::new(400.0, 300.0), Scale(1.0));
        let node = child(&mut doc, root, EGUI_ISLAND, "width: 300px; height: 200px");
        let mut rig = Rig { doc, root, node, now: 1_000_000_000, seq: 0 };
        rig.frame();
        rig
    }

    fn frame(&mut self) -> Option<presto_base::commit::Commit> {
        self.now += 16_000_000;
        self.doc.frame(W, Time(self.now))
    }

    /// The runner's order: island passes, then the document's frame.
    fn step(&mut self, ui: &mut impl FnMut(NodeId, &mut egui::Ui)) -> Option<presto_base::commit::Commit> {
        self.now += 16_000_000;
        IslandHost::run(&mut self.doc, Time(self.now), |n, u| ui(n, u));
        self.frame()
    }

    fn pointer(&mut self, phase: PointerPhase, pos: Point, held: bool) -> Vec<JsEvent> {
        self.seq += 1;
        self.now += 16_000_000;
        let ev = InputEnvelope {
            seq: InputSeq(self.seq),
            window: W,
            os_time: Time(self.now),
            recv_time: Time(self.now),
            event: InputEvent::Pointer(PointerEvent {
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
            }),
        };
        self.doc.dispatch(&ev).events
    }
}

fn child(doc: &mut Document, parent: NodeId, kind: KindId, css: &str) -> NodeId {
    let n = doc.create(kind);
    doc.set_style(n, StyleInput { inline: Some(css.into()), ..Default::default() });
    doc.insert(parent, n, None, None).unwrap();
    n
}

fn island() -> EguiIsland {
    EguiIsland::new(IslandDesc {
        texture: TextureId(9),
        tree: island_tree_id(0),
        rect: Rect::ZERO,
        scale: Scale(1.0),
        renderer: Default::default(),
    })
}

fn center(origin: Point, r: egui::Rect) -> Point {
    Point::new(origin.x + r.center().x, origin.y + r.center().y)
}

#[test]
fn a_mounted_island_records_its_texture_and_takes_clicks_through_the_document() {
    let mut rig = Rig::new();
    let clicks = Cell::new(0);
    let rect = Cell::new(egui::Rect::NOTHING);
    let mut ui = |_: NodeId, ui: &mut egui::Ui| {
        let b = ui.button("Press");
        rect.set(b.rect);
        if b.clicked() {
            clicks.set(clicks.get() + 1);
        }
    };
    assert!(IslandHost::mount(&mut rig.doc, rig.node, island()).is_ok());
    let rejected = IslandHost::mount(&mut rig.doc, rig.node, island()).unwrap_err();
    assert_eq!(rejected.reason, presto_egui_island::MountError::Occupied(rig.node));
    assert!(IslandHost::mount(&mut rig.doc, rig.root, island()).is_err(), "only egui-island nodes");

    let commit = rig.step(&mut ui).expect("the pass re-records the node");
    let list = commit.lists.iter().find(|(n, _)| *n == rig.node).map(|(_, l)| l.clone()).expect("the island's list");
    match &list.items[..] {
        [presto_base::display::Prim::Surface(s)] => {
            assert_eq!(s.texture, TextureId(9));
            assert_eq!(s.rect, Rect::new(0.0, 0.0, 300.0, 200.0));
        }
        other => panic!("expected the island's surface, got {other:?}"),
    }
    assert_eq!(rig.doc.get_prop(rig.node, PASS), Some(Value::I64(1)));

    let origin = rig.doc.world_rect(rig.node).unwrap().min;
    let at = center(origin, rect.get());
    rig.pointer(PointerPhase::Move, at, false);
    rig.step(&mut ui);
    rig.pointer(PointerPhase::Down, at, true);
    rig.step(&mut ui);
    rig.pointer(PointerPhase::Up, at, false);
    rig.step(&mut ui);
    assert_eq!(clicks.get(), 1);
    assert_eq!(rig.doc.focused(W), Some(rig.node), "a press focuses the island node");
}

#[test]
fn the_island_node_is_the_graft_point_of_its_subtree() {
    let mut rig = Rig::new();
    let ui = |_: NodeId, ui: &mut egui::Ui| {
        let _ = ui.button("Press");
    };
    IslandHost::mount(&mut rig.doc, rig.node, island()).unwrap();
    let tree = island_tree_id(rig.node.0);
    assert_eq!(IslandHost::island_mut(&mut rig.doc, rig.node).unwrap().a11y().tree_id(), tree);
    assert_eq!(rig.doc.get_prop(rig.node, MOUNTED), Some(Value::Bool(true)));
    rig.doc.a11y_full_tree(W);
    let now = Time(rig.now + 16_000_000);
    let outputs = IslandHost::run(&mut rig.doc, now, |n, u| ui(n, u));
    let sub = outputs.into_iter().find_map(|(n, o)| (n == rig.node).then_some(o.a11y).flatten()).expect("the subtree update");
    assert_eq!(sub.tree_id, tree);
    let commit = rig.frame().expect("a commit");
    let host = commit.a11y.expect("the full host tree");
    let graft = host.nodes.iter().find(|(_, n)| n.tree_id() == Some(tree)).expect("a graft node for the island");
    assert_eq!(graft.1.role(), accesskit::Role::Pane);

    // Detached, the node stops grafting and the island keeps its state for a remount.
    let island = IslandHost::detach(&mut rig.doc, rig.node).expect("mounted");
    assert_eq!(rig.doc.get_prop(rig.node, MOUNTED), Some(Value::Bool(false)));
    let other = child(&mut rig.doc, rig.root, EGUI_ISLAND, "width: 100px; height: 100px");
    rig.frame();
    IslandHost::mount(&mut rig.doc, other, island).unwrap();
    assert_eq!(IslandHost::island_mut(&mut rig.doc, other).unwrap().a11y().tree_id(), island_tree_id(other.0));
    assert_eq!(IslandHost::mounted(&mut rig.doc), vec![other]);
}

#[test]
fn a_presto_drag_drops_on_an_egui_drop_zone_in_a_mounted_island() {
    let mut rig = Rig::new();
    let src = child(&mut rig.doc, rig.root, BOX, "width: 60px; height: 40px");
    rig.doc.set_prop(src, common::DRAG_TYPE, Value::from("cad/part")).unwrap();
    rig.doc.set_prop(src, common::DRAG_DATA, Value::from("42")).unwrap();
    rig.doc.set_prop(rig.node, common::DROP_TYPES, Value::from("cad/*")).unwrap();
    rig.doc.listen(src, common::DRAG_END, ListenFlags::EMPTY);
    IslandHost::mount(&mut rig.doc, rig.node, island()).unwrap();

    let zone = Cell::new(egui::Rect::NOTHING);
    let got: Cell<Option<String>> = Cell::new(None);
    let mut ui = |_: NodeId, ui: &mut egui::Ui| {
        let (inner, payload) = ui.dnd_drop_zone::<IslandDrag, _>(egui::Frame::default(), |ui| {
            ui.set_min_size(egui::vec2(120.0, 80.0));
            ui.label("drop here");
        });
        zone.set(inner.response.rect);
        if let Some(p) = payload {
            p.accept();
            got.set(Some(format!("{}={}", p.drag.ty, p.drag.data)));
        }
    };
    rig.step(&mut ui);
    rig.step(&mut ui);

    let s = rig.doc.world_rect(src).unwrap();
    let from = Point::new(s.min.x + 10.0, s.min.y + 10.0);
    let to = center(rig.doc.world_rect(rig.node).unwrap().min, zone.get());
    let mut events = rig.pointer(PointerPhase::Down, from, true);
    events.extend(rig.pointer(PointerPhase::Move, Point::new(from.x + 20.0, from.y), true));
    events.extend(rig.pointer(PointerPhase::Move, to, true));
    IslandHost::bridge_events(&mut rig.doc, &events);
    IslandHost::drag_moved(&mut rig.doc, to);
    rig.step(&mut ui);
    let events = rig.pointer(PointerPhase::Up, to, false);
    assert!(events.iter().any(|e| e.target == rig.node && e.signal == common::DROP), "core dropped on the island node");
    IslandHost::bridge_events(&mut rig.doc, &events);
    rig.step(&mut ui);
    assert_eq!(got.take().as_deref(), Some("cad/part=42"));
}
