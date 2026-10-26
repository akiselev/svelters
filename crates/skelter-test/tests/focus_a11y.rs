//! Focus as AccessKit's own consumers see it (`plans/cloud/qc-a11y/PLAN-focus-and-text.md` §5, QA11Y
//! Q1's focus half): a real skelter-core document on the harness, its updates folded into an
//! `accesskit_consumer::Tree` and, on Linux, fed to AT-SPI's translation layer (`skelter_test::consumer`).
//! Tab order, a dialog scope taking focus and giving it back, bounds under UI zoom, and focus on an egui
//! island's graft node before and after its subtree went out (AccessKit's consumer panics on a focused
//! graft without its subtree).

use skelter::backends::CoreDocument;
use skelter_base::geom::{Rect, Scale, Size};
use skelter_base::ids::{NodeId, TextureId};
use skelter_base::event::Modifiers;
use skelter_base::key::Key;
use skelter_core::testkinds::{BoxKind, FixedText, ToggleKind};
use skelter_core::{Document, KindId, KindRegistry, StyleInput, Value, common};
use skelter_egui_island::{EGUI_ISLAND, EguiIsland, IslandDesc, IslandHost, island_tree_id};
use skelter_test::Harness;
use std::sync::Arc;

const BOX: KindId = KindId::of("box");
const TOGGLE: KindId = KindId::of("toggle");

fn style(d: &mut Document, n: NodeId, css: &str) {
    d.set_style(n, StyleInput { inline: Some(css.into()), ..Default::default() });
}

fn add(d: &mut Document, parent: NodeId, kind: KindId, anchor: &str, css: &str) -> NodeId {
    let n = d.create(kind);
    style(d, n, css);
    d.set_prop(n, common::ANCHOR, Value::from(anchor)).unwrap();
    d.set_prop(n, common::ARIA_LABEL, Value::from(anchor)).unwrap();
    d.insert(parent, n, None, None).unwrap();
    n
}

/// A 400 × 300 window: toggles `a` and `b`, and a hidden `dialog` scope holding `d0` and `d1`
/// (`autofocus`).
fn shell() -> (Harness<CoreDocument>, NodeId) {
    let mut reg = KindRegistry::new();
    reg.register::<BoxKind>().unwrap();
    reg.register::<ToggleKind>().unwrap();
    let doc = Document::new(Arc::new(reg), Box::new(FixedText));
    let mut h = Harness::<CoreDocument>::builder()
        .with_size(Size::new(400.0, 300.0))
        .with_consumer(true)
        .build(CoreDocument::new(doc, BOX));
    let w = h.window();
    let d = &mut h.state_mut().doc;
    let root = d.window_root(w).unwrap();
    let t = "width: 40px; height: 20px";
    add(d, root, TOGGLE, "a", &format!("{t}; margin-left: 10px; margin-top: 8px"));
    add(d, root, TOGGLE, "b", t);
    let dialog = add(d, root, BOX, "dialog", "display: none; width: 200px; height: 100px");
    d.set_prop(dialog, common::FOCUS_SCOPE, Value::Enum(2)).unwrap();
    add(d, dialog, TOGGLE, "d0", t);
    let d1 = add(d, dialog, TOGGLE, "d1", t);
    d.set_prop(d1, common::AUTOFOCUS, Value::Bool(true)).unwrap();
    h.key_press(Key::F24);
    h.run();
    (h, dialog)
}

fn focus(h: &Harness<CoreDocument>) -> Option<String> {
    h.consumer().focus_author_id().map(str::to_owned)
}

#[test]
fn tab_order_wraps_and_a_dialog_gives_focus_back() {
    let (mut h, dialog) = shell();
    assert_eq!(focus(&h), None, "nothing focused: the root");
    let mut seen = Vec::new();
    for _ in 0..3 {
        h.key_press(Key::Tab);
        h.run();
        seen.push(focus(&h));
    }
    assert_eq!(seen, [Some("a".into()), Some("b".into()), Some("a".into())], "the hidden dialog is no stop; Tab wraps");
    h.key_press_modifiers(Modifiers::SHIFT, Key::Tab);
    h.run();
    assert_eq!(focus(&h).as_deref(), Some("b"), "Shift+Tab goes back");

    // the dialog appears: its autofocus node takes focus; Tab wraps inside it
    style(&mut h.state_mut().doc, dialog, "width: 200px; height: 100px");
    h.key_press(Key::F24);
    h.run();
    assert_eq!(focus(&h).as_deref(), Some("d1"));
    h.key_press(Key::Tab);
    h.run();
    assert_eq!(focus(&h).as_deref(), Some("d0"), "wraps inside the dialog");
    // hidden again: back to the opener
    style(&mut h.state_mut().doc, dialog, "display: none; width: 200px; height: 100px");
    h.key_press(Key::F24);
    h.run();
    assert_eq!(focus(&h).as_deref(), Some("b"), "focus back on the opener");

    #[cfg(target_os = "linux")]
    {
        let c = h.consumer();
        let id = |a: &str| c.by_author_id(a).unwrap().id();
        let at = c.atspi().unwrap().focused_events();
        let tail = &at[at.len() - 3..];
        assert_eq!(tail, [id("d1"), id("d0"), id("b")], "AT-SPI announced the field, then the opener");
    }
}

#[test]
fn programmatic_focus_under_a_backdrop_reaches_the_consumer() {
    let (mut h, dialog) = shell();
    let w = h.window();
    // the palette's order: focus() on a field, then show its backdrop, in one transaction
    let d = &mut h.state_mut().doc;
    let d0 = d.find_anchor(w, "d0").unwrap();
    d.focus(d0, skelter_core::FocusReason::Programmatic);
    style(d, dialog, "width: 200px; height: 100px");
    h.key_press(Key::F24);
    h.run();
    assert_eq!(focus(&h).as_deref(), Some("d0"), "the app's focus() wins over the dialog's autofocus");
}

#[test]
fn bounds_follow_the_ui_zoom() {
    let (mut h, _) = shell();
    h.set_zoom(1.25);
    h.run();
    let w = h.window();
    let a = h.state().doc.find_anchor(w, "a").unwrap();
    let world = h.state().doc.world_rect(a).unwrap();
    let px = |v: f32| (v * 1.25).round() as i32;
    // the harness's own snapshot stays in document points
    let ours = h.a11y().walk().into_iter().find(|n| n.author_id() == Some("a")).unwrap().bounds.unwrap();
    assert_eq!(ours, world);
    // AccessKit's consumer sees physical pixels: world rect × zoom
    let c = h.consumer();
    let b = c.by_author_id("a").unwrap().bounding_box().unwrap();
    assert_eq!((b.x0.round() as i32, b.y0.round() as i32, b.x1.round() as i32, b.y1.round() as i32), (px(world.min.x), px(world.min.y), px(world.max.x), px(world.max.y)));
    #[cfg(target_os = "linux")]
    {
        use skelter_test::consumer::CoordType;
        let e = c.atspi().unwrap().node(c.by_author_id("a").unwrap().id()).extents(CoordType::Window).unwrap();
        // AT-SPI truncates to whole pixels
        let t = |v: f32| (v * 1.25) as i32;
        assert_eq!((e.x, e.y, e.width, e.height), (t(world.min.x), t(world.min.y), t(world.width()), t(world.height())), "AT-SPI window extents");
    }
}

type Ui = Box<dyn FnMut(NodeId, &mut egui::Ui)>;

#[test]
fn focus_on_an_island_waits_for_its_subtree_and_never_panics_the_consumer() {
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
        .with_consumer(true)
        .build_core_islands(CoreDocument::new(doc, BOX), ui);
    let window = h.window();
    let doc = h.state_mut().doc();
    let root = doc.window_root(window).unwrap();
    let node = doc.create(EGUI_ISLAND);
    style(doc, node, "width: 300px; height: 200px");
    doc.insert(root, node, None, None).unwrap();
    let island = EguiIsland::new(IslandDesc {
        texture: TextureId(9),
        tree: island_tree_id(0),
        rect: Rect::ZERO,
        scale: Scale(1.0),
        renderer: Default::default(),
    });
    IslandHost::mount(doc, node, island).unwrap_or_else(|_| panic!("mounts"));
    // focused before its subtree ever went out: the host names the island's parent first
    doc.focus(node, skelter_core::FocusReason::Programmatic);
    assert_eq!(doc.focused(window), Some(node));
    h.run();
    let c = h.consumer();
    let (_, tree) = c.focus_located().expect("a focus");
    assert_eq!(tree, island_tree_id(node.0), "focus followed the graft into the island's tree");
    assert!(
        c.focus_moves().iter().all(|m| m.to.is_some()),
        "the consumer never lost the focus: {:?}",
        c.focus_moves()
    );
}
