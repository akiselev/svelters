//! The window's top layer in a running app (the headless platform and compositor, the production
//! dispatch path): a date picker's popup and a select's list open in the window's overlay stack,
//! draw in a layer above the content and take presses there; a press outside dismisses them; an
//! in-app drag shows the ghost in the drag layer, which follows the pointer and goes at the drop.

use skelter::CoreDocument;
use skelter_base::event::{InputEvent, Modifiers, PointerEvent, PointerPhase, PointerType};
use skelter_base::geom::{Point, Rect, Size, Vec2};
use skelter_base::ids::NodeId;
use skelter_base::layer::LayerKind;
use skelter_core::{Document, KindId, KindRegistry, StyleInput, Value, common};
use skelter_test::Harness;
use std::sync::Arc;

fn text() -> skelter_text::TextSystem {
    skelter_text::TextSystem::new(skelter_text::FontStore::new(skelter_text::FontConfig::testing()))
}

fn harness() -> Harness<CoreDocument> {
    let mut reg = KindRegistry::new();
    skelter_widgets::host::register_tier0(&mut reg).unwrap();
    skelter_widgets::views::register_all(&mut reg).unwrap();
    skelter_widgets::pickers::kinds::register(&mut reg).unwrap();
    let doc = Document::new(Arc::new(reg), Box::new(text()));
    let doc = CoreDocument::with_widgets(doc, KindId::of("box"), text(), skelter_widgets::host::os_policy(), skelter_widgets::theme::Theme::light_modern());
    Harness::<CoreDocument>::builder().with_size(Size::new(500.0, 500.0)).with_accessibility_check(false).build(doc)
}

/// A node of `kind` under the root, placed with `css`.
fn add(h: &mut Harness<CoreDocument>, kind: &str, css: &str) -> NodeId {
    let w = h.window();
    let d = &mut h.state_mut().doc;
    let root = d.window_root(w).unwrap();
    let n = d.create(KindId::of(kind));
    d.set_style(n, StyleInput { inline: Some(css.into()), ..Default::default() });
    d.insert(root, n, None, None).unwrap();
    n
}

fn set(h: &mut Harness<CoreDocument>, n: NodeId, kind: &str, prop: &str, v: Value) {
    let d = &mut h.state_mut().doc;
    let p = d.registry().get(KindId::of(kind)).and_then(|e| e.spec.prop_named(prop)).unwrap();
    d.set_prop(n, p, v).unwrap();
}

fn get(h: &Harness<CoreDocument>, n: NodeId, kind: &str, prop: &str) -> Option<Value> {
    let d = &h.state().doc;
    let p = d.registry().get(KindId::of(kind)).and_then(|e| e.spec.prop_named(prop)).unwrap();
    d.get_prop(n, p)
}

/// The tree changed outside input: a key the document ignores brings the frame.
fn settle(h: &mut Harness<CoreDocument>) {
    h.key_press(skelter_base::key::Key::F24);
    h.run();
}

/// The open overlay surface's rect (window points) and its node.
fn open_surface(h: &Harness<CoreDocument>) -> Option<(NodeId, Rect)> {
    let st = h.state().doc.overlay_stack(h.window())?;
    st.entries().last().map(|e| (e.spec.node, e.spec.rect))
}

/// The compositor's overlay layers drawing `node`'s overlay part, and whether each paints after
/// every content layer (z above all non-overlay siblings).
fn drawn_overlay(h: &Harness<CoreDocument>, node: NodeId) -> Vec<bool> {
    let key = NodeId(node.0 | 1 << 62);
    h.compositor()
        .with_tree(h.window(), |t| {
            let top_content = t.layers.values().filter(|l| l.desc.kind != LayerKind::Overlay && l.desc.parent.is_some()).map(|l| l.desc.z).max().unwrap_or(0);
            t.layers
                .values()
                .filter(|l| l.desc.kind == LayerKind::Overlay && l.items.iter().any(|i| i.node == key) && t.lists.contains_key(&key))
                .map(|l| l.desc.z > top_content)
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn a_date_pickers_popup_draws_in_the_overlay_layer_takes_clicks_and_light_dismisses() {
    let mut h = harness();
    let dp = add(&mut h, "date-picker", "position: absolute; left: 20px; top: 20px; width: 160px; height: 28px");
    // content under where the popup opens
    let under = add(&mut h, "box", "position: absolute; left: 0px; top: 60px; width: 500px; height: 440px");
    let e = {
        let d = &h.state().doc;
        let spec = d.registry().get(KindId::of("date-picker")).unwrap().spec;
        let p = (0..spec.props.len()).find(|&i| spec.props[i].name == "commit-mode").unwrap();
        spec.props[p].enum_values.iter().position(|v| *v == "immediate").unwrap() as u32
    };
    set(&mut h, dp, "date-picker", "commit-mode", Value::Enum(e as _));
    settle(&mut h);
    assert!(open_surface(&h).is_none());

    h.click_at(Point::new(60.0, 34.0));
    h.run();
    let (node, rect) = open_surface(&h).expect("the popup opened in the overlay stack");
    assert_eq!(node, dp);
    assert!(rect.min.y >= 48.0, "below the button: {rect:?}");
    assert_eq!(drawn_overlay(&h, dp), vec![true], "one overlay layer above the content draws it");
    let at = Point::new((rect.min.x + rect.max.x) / 2.0, (rect.min.y + rect.max.y) / 2.0);
    assert_eq!(h.state().doc.hit_test(h.window(), at), Some(dp), "the popup, not the content under it");
    assert!(h.state().doc.world_rect(under).unwrap().contains(at));

    // a click on a day in the popup reaches the picker: immediate mode picks it and closes
    h.click_at(at);
    h.run();
    let v = get(&h, dp, "date-picker", "value");
    assert!(matches!(&v, Some(Value::Str(s)) if !s.is_empty()), "a day was picked: {v:?}");

    // reopen; a press outside dismisses and the layer goes
    if open_surface(&h).is_none() {
        h.click_at(Point::new(60.0, 34.0));
        h.run();
    }
    assert!(open_surface(&h).is_some());
    h.click_at(Point::new(450.0, 450.0));
    h.run();
    assert!(open_surface(&h).is_none(), "light dismissed");
    assert!(drawn_overlay(&h, dp).is_empty(), "its layer is gone");
}

#[test]
fn a_selects_list_opens_in_the_overlay_layer_and_picks_there() {
    let mut h = harness();
    let sel = add(&mut h, "select", "position: absolute; left: 20px; top: 20px; width: 120px; height: 26px");
    let opts = Value::List(["one", "two", "three"].iter().map(|s| Value::from(*s)).collect::<Vec<_>>().into());
    set(&mut h, sel, "select", "options", opts);
    settle(&mut h);
    h.click_at(Point::new(60.0, 33.0));
    h.run();
    let (node, rect) = open_surface(&h).expect("the list opened");
    assert_eq!(node, sel);
    assert_eq!(drawn_overlay(&h, sel), vec![true]);
    // the last row, outside the select's own box
    let at = Point::new((rect.min.x + rect.max.x) / 2.0, rect.max.y - 8.0);
    assert!(at.y > 46.0);
    h.click_at(at);
    h.run();
    assert_eq!(get(&h, sel, "select", "selected"), Some(Value::I64(2)), "the press in the list picked its row");
    assert!(open_surface(&h).is_none(), "picking closed it");
    assert!(drawn_overlay(&h, sel).is_empty());
}

fn held_move(h: &Harness<CoreDocument>, x: f32, y: f32) {
    h.event(InputEvent::Pointer(PointerEvent {
        phase: PointerPhase::Move,
        pointer: 0,
        kind: PointerType::Mouse,
        button: None,
        buttons: 1,
        pos: Point::new(x, y),
        modifiers: Modifiers::default(),
        pressure: 0.5,
        tilt: Vec2::ZERO,
        history: Default::default(),
    }));
}

/// The drag layer's offset from the window origin in the compositor's scene.
fn drag_layer_at(h: &Harness<CoreDocument>) -> Option<(f32, f32)> {
    let (layer, tf) = h.state().doc.drag_layer(h.window())?;
    h.compositor()
        .with_tree(h.window(), |t| {
            let l = t.layers.get(&layer)?;
            assert_eq!(l.desc.kind, LayerKind::Overlay);
            assert_eq!(l.desc.z, i32::MAX, "the drag layer is topmost");
            let w = t.props.world_transform(tf, &mut Default::default());
            Some((w.e, w.f))
        })
        .flatten()
}

#[test]
fn a_drag_shows_the_ghost_in_the_drag_layer_which_follows_the_pointer() {
    let mut h = harness();
    let src = add(&mut h, "box", "position: absolute; left: 20px; top: 20px; width: 60px; height: 30px");
    h.state_mut().doc.set_prop(src, common::DRAG_TYPE, Value::from("cad/part")).unwrap();
    h.state_mut().doc.set_prop(src, common::DRAG_DATA, Value::from("R1")).unwrap();
    settle(&mut h);
    h.hover_at(Point::new(30.0, 30.0));
    h.drag_at(Point::new(30.0, 30.0));
    held_move(&h, 60.0, 40.0);
    h.run();
    let a = drag_layer_at(&h).expect("the ghost is in the drag layer");
    let applied = h.compositor().applied_count();
    held_move(&h, 160.0, 140.0);
    h.run();
    let b = drag_layer_at(&h).expect("still dragging");
    assert_eq!((b.0 - a.0, b.1 - a.1), (100.0, 100.0), "it follows the pointer");
    let re_recorded = h.compositor().applied()[applied..].iter().any(|c| c.lists.iter().any(|(n, _)| n.0 >> 61 == 3));
    assert!(!re_recorded, "moved by its transform, not re-recorded");
    h.drop_at(Point::new(160.0, 140.0));
    h.run();
    assert_eq!(h.state().doc.drag_layer(h.window()), None);
    let gone = h.compositor().with_tree(h.window(), |t| !t.layers.values().any(|l| l.desc.z == i32::MAX)).unwrap();
    assert!(gone, "the drag layer is gone after the drop");
}
