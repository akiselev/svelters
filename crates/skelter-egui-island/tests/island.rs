//! An island driven through skelter input, without a GPU: clicks, keys, pointer capture, repaint
//! scheduling, the AccessKit graft (applied to an accesskit_consumer tree, as the platform adapters apply
//! it) and the drag-and-drop bridge.

use skelter_base::event::{
    InputEnvelope, InputEvent, KeyEvent, KeyLocation, KeyState, Modifiers, MouseButton, PointerEvent,
    PointerPhase, PointerType, ScrollDelta, WheelEvent,
};
use skelter_base::geom::{Point, Rect, Scale, Vec2};
use skelter_base::ids::{InputSeq, TextureId, WindowId};
use skelter_base::key::Key;
use skelter_base::time::{Span, Time};
use skelter_egui_island::{EguiIsland, IslandDesc, IslandDrag, IslandOutput, DragData, island_tree_id};
use std::cell::Cell;

/// The island sits at (100, 50) in its window, 300 × 200 points.
const ORIGIN: Point = Point::new(100.0, 50.0);

struct Rig {
    island: EguiIsland,
    now: Time,
    seq: u64,
}

impl Rig {
    fn new(scale: f32) -> Rig {
        let island = EguiIsland::new(IslandDesc {
            texture: TextureId(7),
            tree: island_tree_id(7),
            rect: Rect::new(ORIGIN.x, ORIGIN.y, ORIGIN.x + 300.0, ORIGIN.y + 200.0),
            scale: Scale(scale),
            renderer: Default::default(),
        });
        Rig { island, now: Time(1_000_000_000), seq: 0 }
    }

    fn tick(&mut self) -> Time {
        self.now = self.now + Span::from_millis(16);
        self.now
    }

    fn send(&mut self, event: InputEvent) -> bool {
        self.seq += 1;
        let now = self.tick();
        self.island.handle_input(&InputEnvelope {
            seq: InputSeq(self.seq),
            window: WindowId(1),
            os_time: now,
            recv_time: now,
            event,
        })
    }

    fn pass(&mut self, ui: impl FnMut(&mut egui::Ui)) -> IslandOutput {
        let now = self.tick();
        self.island.run(now, ui)
    }

    fn pointer(&mut self, phase: PointerPhase, pos: Point, button: Option<MouseButton>) -> bool {
        self.send(InputEvent::Pointer(PointerEvent {
            phase,
            pointer: 0,
            kind: PointerType::Mouse,
            button,
            buttons: if phase == PointerPhase::Down { 1 } else { 0 },
            pos,
            modifiers: Modifiers::default(),
            pressure: 0.0,
            tilt: Vec2::ZERO,
            history: Default::default(),
        }))
    }

    /// Move, press and release at `pos` (window points), with a pass after each step.
    fn click(&mut self, pos: Point, ui: &mut impl FnMut(&mut egui::Ui)) {
        assert!(self.pointer(PointerPhase::Move, pos, None), "the island takes a move over it");
        self.pass(&mut *ui);
        assert!(self.pointer(PointerPhase::Down, pos, Some(MouseButton::Primary)));
        self.pass(&mut *ui);
        assert!(self.pointer(PointerPhase::Up, pos, Some(MouseButton::Primary)));
        self.pass(&mut *ui);
    }

    fn key(&mut self, key: Key, text: Option<&str>, modifiers: Modifiers) {
        for state in [KeyState::Pressed, KeyState::Released] {
            self.send(InputEvent::Key(KeyEvent {
                key: Some(key),
                physical: 0,
                text: text.map(str::to_owned),
                state,
                repeat: false,
                location: KeyLocation::Standard,
                modifiers,
            }));
        }
    }
}

/// An egui rect (island points) to its centre in window points.
fn window_center(r: egui::Rect) -> Point {
    Point::new(ORIGIN.x + r.center().x, ORIGIN.y + r.center().y)
}

#[test]
fn button_and_label_are_clickable_through_skelter_input() {
    let mut rig = Rig::new(1.0);
    let button_clicks = Cell::new(0);
    let label_clicks = Cell::new(0);
    let button_rect = Cell::new(egui::Rect::NOTHING);
    let label_rect = Cell::new(egui::Rect::NOTHING);
    let mut ui = |ui: &mut egui::Ui| {
        let b = ui.button("Press");
        button_rect.set(b.rect);
        if b.clicked() {
            button_clicks.set(button_clicks.get() + 1);
        }
        let l = ui.add(egui::Label::new("Hello").sense(egui::Sense::click()));
        label_rect.set(l.rect);
        if l.clicked() {
            label_clicks.set(label_clicks.get() + 1);
        }
    };
    assert!(rig.island.needs_pass(rig.now), "a new island needs its first pass");
    rig.pass(&mut ui);
    assert!(button_rect.get().is_positive() && label_rect.get().is_positive());

    rig.click(window_center(button_rect.get()), &mut ui);
    assert_eq!((button_clicks.get(), label_clicks.get()), (1, 0));

    rig.click(window_center(label_rect.get()), &mut ui);
    assert_eq!((button_clicks.get(), label_clicks.get()), (1, 1));
}

#[test]
fn clicks_respect_the_window_scale() {
    // At scale 2 the island's egui still works in points: input positions are window points.
    let mut rig = Rig::new(2.0);
    let clicks = Cell::new(0);
    let rect = Cell::new(egui::Rect::NOTHING);
    let mut ui = |ui: &mut egui::Ui| {
        let b = ui.button("Press");
        rect.set(b.rect);
        if b.clicked() {
            clicks.set(clicks.get() + 1);
        }
    };
    rig.pass(&mut ui);
    rig.click(window_center(rect.get()), &mut ui);
    assert_eq!(clicks.get(), 1);
    assert_eq!(rig.island.target_size(), skelter_base::geom::DeviceSize { w: 600, h: 400 });
}

#[test]
fn input_outside_the_island_is_not_taken() {
    let mut rig = Rig::new(1.0);
    let clicks = Cell::new(0);
    let mut ui = |ui: &mut egui::Ui| {
        if ui.button("Press").clicked() {
            clicks.set(clicks.get() + 1);
        }
    };
    rig.pass(&mut ui);
    let outside = Point::new(20.0, 20.0);
    assert!(!rig.pointer(PointerPhase::Move, outside, None));
    assert!(!rig.pointer(PointerPhase::Down, outside, Some(MouseButton::Primary)));
    assert!(!rig.pointer(PointerPhase::Up, outside, Some(MouseButton::Primary)));
    assert!(!rig.send(InputEvent::Wheel(WheelEvent {
        pos: outside,
        delta: ScrollDelta::Lines(Vec2::new(0.0, 1.0)),
        phase: None,
        momentum: None,
        modifiers: Modifiers::default(),
    })));
    // Keys need keyboard focus.
    rig.key(Key::A, Some("a"), Modifiers::default());
    rig.pass(&mut ui);
    assert_eq!(clicks.get(), 0);
}

#[test]
fn a_press_inside_keeps_the_pointer_until_release() {
    let mut rig = Rig::new(1.0);
    let rect = Cell::new(egui::Rect::NOTHING);
    let clicks = Cell::new(0);
    let mut ui = |ui: &mut egui::Ui| {
        let b = ui.button("Press");
        rect.set(b.rect);
        if b.clicked() {
            clicks.set(clicks.get() + 1);
        }
    };
    rig.pass(&mut ui);
    let inside = window_center(rect.get());
    rig.pointer(PointerPhase::Move, inside, None);
    rig.pointer(PointerPhase::Down, inside, Some(MouseButton::Primary));
    rig.pass(&mut ui);
    let outside = Point::new(10.0, 10.0);
    assert!(rig.pointer(PointerPhase::Move, outside, None), "captured: moves outside still reach egui");
    rig.pass(&mut ui);
    assert!(rig.pointer(PointerPhase::Up, outside, Some(MouseButton::Primary)), "captured release");
    rig.pass(&mut ui);
    assert_eq!(clicks.get(), 0, "a press dragged off the button and released outside is no click");
    assert!(!rig.pointer(PointerPhase::Move, outside, None), "capture ended with the release");
}

#[test]
fn a_focused_island_takes_keys_and_text() {
    let mut rig = Rig::new(1.0);
    let mut text = String::new();
    let rect = Cell::new(egui::Rect::NOTHING);
    let mut ui = |ui: &mut egui::Ui| {
        rect.set(ui.text_edit_singleline(&mut text).rect);
    };
    rig.pass(&mut ui);
    rig.island.set_focused(true);
    rig.click(window_center(rect.get()), &mut ui);
    rig.key(Key::H, Some("h"), Modifiers::default());
    rig.key(Key::I, Some("i"), Modifiers::default());
    rig.pass(&mut ui);
    // A command chord (Ctrl, and Cmd on macOS) types nothing; Super elsewhere is no command key.
    rig.key(Key::S, Some("s"), Modifiers::CTRL);
    if cfg!(target_os = "macos") {
        rig.key(Key::S, Some("s"), Modifiers::SUPER);
    }
    rig.pass(&mut ui);
    drop(ui);
    assert_eq!(text, "hi");
}

#[test]
fn an_idle_island_sets_no_deadline() {
    let mut rig = Rig::new(1.0);
    let mut ui = |ui: &mut egui::Ui| {
        ui.label("idle");
    };
    let mut out = rig.pass(&mut ui);
    // egui repaints a few times while it settles its first layout.
    for _ in 0..8 {
        if out.repaint_at.is_none() {
            break;
        }
        out = rig.pass(&mut ui);
    }
    assert_eq!(out.repaint_at, None);
    assert_eq!(rig.island.next_wakeup(), None);
    assert!(!rig.island.needs_pass(rig.now + Span::from_millis(1000)));

    // Input wakes it at the input's time.
    rig.pointer(PointerPhase::Move, Point::new(150.0, 100.0), None);
    assert!(rig.island.needs_pass(rig.now));
}

#[test]
fn the_display_list_is_the_island_texture_over_its_box() {
    let rig = Rig::new(1.0);
    let list = rig.island.display_list();
    assert_eq!(list.items.len(), 1);
    match &list.items[0] {
        skelter_base::display::Prim::Surface(s) => {
            assert_eq!(s.texture, TextureId(7));
            assert_eq!(s.rect, Rect::new(0.0, 0.0, 300.0, 200.0));
            assert_eq!(s.opacity, 1.0);
        }
        other => panic!("expected a Surface, got {other:?}"),
    }
    assert_eq!(rig.island.layer_kind(), skelter_base::layer::LayerKind::Texture(TextureId(7)));
}

mod a11y {
    use super::*;
    use accesskit::{Action, ActionRequest, Affine, Node, NodeId, Role, TreeId, TreeInfo, TreeUpdate};
    use accesskit_consumer::{NodeRef, Tree, TreeChangeHandler};

    struct Noop;
    impl TreeChangeHandler for Noop {
        fn node_added(&mut self, _: &NodeRef) {}
        fn node_updated(&mut self, _: &NodeRef, _: &NodeRef) {}
        fn focus_moved(&mut self, _: Option<&NodeRef>, _: Option<&NodeRef>) {}
        fn node_removed(&mut self, _: &NodeRef) {}
    }

    /// skelter's side: a window root (scaled to device pixels, as egui's own root is) with one graft node
    /// for the island.
    fn host_tree(island: &EguiIsland, scale: f64) -> Tree {
        let (root, graft) = (NodeId(1), NodeId(2));
        let mut r = Node::new(Role::Window);
        r.set_transform(Affine::scale(scale));
        r.set_children(vec![graft]);
        let mut g = Node::new(Role::GenericContainer);
        g.set_bounds(accesskit::Rect { x0: 100.0, y0: 50.0, x1: 400.0, y1: 250.0 });
        island.a11y().graft_node(&mut g);
        Tree::new(
            TreeUpdate {
                nodes: vec![(root, r), (graft, g)],
                tree: Some(TreeInfo::new(root)),
                tree_id: TreeId::ROOT,
                focus: root,
            },
            true,
        )
    }

    fn find<'a>(node: NodeRef<'a>, role: Role, label: &str) -> Option<NodeRef<'a>> {
        if node.role() == role && node.label().as_deref() == Some(label) {
            return Some(node);
        }
        node.children().find_map(|c| find(c, role, label))
    }

    #[test]
    fn the_graft_is_a_valid_nested_tree() {
        let mut rig = Rig::new(2.0);
        rig.island.enable_accesskit();
        let clicks = Cell::new(0);
        let rect = Cell::new(egui::Rect::NOTHING);
        let mut ui = |ui: &mut egui::Ui| {
            let b = ui.button("Press");
            rect.set(b.rect);
            if b.clicked() {
                clicks.set(clicks.get() + 1);
            }
        };
        let out = rig.pass(&mut ui);
        let update = out.a11y.expect("accesskit on: the pass grafts egui's update");
        let tree_id = rig.island.a11y().tree_id();
        assert_eq!(update.tree_id, tree_id);
        assert_ne!(tree_id, TreeId::ROOT);
        assert!(update.tree.is_some(), "the first subtree update carries its tree info");
        assert!(update.nodes.iter().any(|(id, _)| *id == update.focus), "focus is a node of the update");

        let mut tree = host_tree(&rig.island, 2.0);
        tree.update_and_process_changes(update, &mut Noop);
        let state = tree.state();
        let sub_root = state.subtree_root(tree_id).expect("the subtree is attached at the graft node");
        let sub_root = state.node_by_id(sub_root).unwrap();
        let button = find(sub_root, Role::Button, "Press").expect("the button is in the grafted subtree");
        let found = find(state.root(), Role::Button, "Press").expect("and reachable from the host root");
        assert_eq!(found.id(), button.id());

        // Bounds compose through the graft: island offset (window points), then the host root's scale.
        let bb = button.bounding_box().expect("the button has bounds");
        let r = rect.get();
        let expect = |v: f32, o: f32| f64::from((v + o) * 2.0);
        for (got, want) in [
            (bb.x0, expect(r.min.x, ORIGIN.x)),
            (bb.y0, expect(r.min.y, ORIGIN.y)),
            (bb.x1, expect(r.max.x, ORIGIN.x)),
            (bb.y1, expect(r.max.y, ORIGIN.y)),
        ] {
            assert!((got - want).abs() < 0.5, "bounds {bb:?} vs button {r:?}");
        }
        let (button_id, button_tree) = state.locate_node(button.id()).unwrap();
        assert_eq!(button_tree, tree_id);

        // egui resends its whole tree each pass; the repeat applies too.
        rig.island.request_pass();
        let again = rig.pass(&mut ui).a11y.expect("a second update");
        tree.update_and_process_changes(again, &mut Noop);

        // An action for the island's tree reaches egui as an action on its root tree.
        let click = |target_tree| ActionRequest { action: Action::Click, target_tree, target_node: button_id, data: None };
        assert!(!rig.island.handle_a11y_action(click(TreeId::ROOT)), "the host's own tree is not the island's");
        assert!(rig.island.handle_a11y_action(click(tree_id)));
        rig.pass(&mut ui);
        assert_eq!(clicks.get(), 1);
    }
}

mod dnd {
    use super::*;

    #[test]
    fn a_skelter_drag_drops_on_an_egui_drop_zone() {
        let mut rig = Rig::new(1.0);
        let rect = Cell::new(egui::Rect::NOTHING);
        let got: Cell<Option<String>> = Cell::new(None);
        let mut ui = |ui: &mut egui::Ui| {
            let (inner, payload) = ui.dnd_drop_zone::<IslandDrag, _>(egui::Frame::default(), |ui| {
                ui.set_min_size(egui::vec2(120.0, 80.0));
                ui.label("drop here");
            });
            rect.set(inner.response.rect);
            if let Some(p) = payload {
                p.accept();
                got.set((&*p.drag.ty == "text/plain").then(|| p.drag.data.to_string()));
            }
        };
        rig.pass(&mut ui);
        let at = window_center(rect.get());
        rig.island.drag_enter(&DragData::new("text/plain", "hello"), at);
        let out = rig.pass(&mut ui);
        assert_eq!(out.dnd.dropped, None, "hovering is no drop");
        rig.island.drag_drop(at);
        let out = rig.pass(&mut ui);
        assert_eq!(out.dnd.dropped, Some(true));
        assert_eq!(got.take().as_deref(), Some("hello"));
    }

    #[test]
    fn a_skelter_drag_leaving_clears_the_egui_payload() {
        let mut rig = Rig::new(1.0);
        let mut ui = |ui: &mut egui::Ui| {
            ui.label("nothing to drop on");
        };
        rig.pass(&mut ui);
        rig.island.drag_enter(&DragData::new("text/plain", "x"), Point::new(150.0, 100.0));
        rig.pass(&mut ui);
        assert!(egui::DragAndDrop::has_any_payload(rig.island.context()));
        rig.island.drag_leave();
        let out = rig.pass(&mut ui);
        assert!(!egui::DragAndDrop::has_any_payload(rig.island.context()));
        assert_eq!(out.dnd, Default::default());
    }

    #[test]
    fn an_egui_drag_leaving_the_island_becomes_a_skelter_drag() {
        let mut rig = Rig::new(1.0);
        let rect = Cell::new(egui::Rect::NOTHING);
        let mut ui = |ui: &mut egui::Ui| {
            let r = ui.dnd_drag_source(egui::Id::unique("part"), IslandDrag::new(DragData::new("cad/part", "42")), |ui| {
                ui.label("part 42")
            });
            if r.inner.rect.is_positive() && !egui::DragAndDrop::has_any_payload(ui.ctx()) {
                rect.set(r.inner.rect);
            }
        };
        rig.pass(&mut ui);
        let start = window_center(rect.get());
        rig.pointer(PointerPhase::Move, start, None);
        rig.pass(&mut ui);
        rig.pointer(PointerPhase::Down, start, Some(MouseButton::Primary));
        rig.pass(&mut ui);
        // Past egui's drag threshold, still inside: an egui drag, nothing for skelter yet.
        rig.pointer(PointerPhase::Move, start + Vec2::new(30.0, 10.0), None);
        let out = rig.pass(&mut ui);
        assert_eq!(out.dnd.drag_out, None);
        assert!(egui::DragAndDrop::has_any_payload(rig.island.context()), "egui started its drag");

        // Out of the island with the button held: skelter takes the drag over.
        let mut out = IslandOutput::default();
        for x in [60.0, 20.0] {
            rig.pointer(PointerPhase::Move, Point::new(x, 20.0), None);
            let o = rig.pass(&mut ui);
            if o.dnd.drag_out.is_some() {
                assert!(out.dnd.drag_out.is_none(), "exported once per drag");
                out = o;
            }
        }
        assert_eq!(out.dnd.drag_out, Some(DragData::new("cad/part", "42")));
        rig.island.drag_out_finished(true);
        rig.pointer(PointerPhase::Up, Point::new(20.0, 20.0), Some(MouseButton::Primary));
        rig.pass(&mut ui);
        assert!(!egui::DragAndDrop::has_any_payload(rig.island.context()));
    }
}
