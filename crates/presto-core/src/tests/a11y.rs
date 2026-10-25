//! The accessibility projection: a valid full tree first, incremental updates after, roles, labels,
//! states and focus from the document.

use super::*;
use crate::testkinds::{BOX_TINT, TEXT as TEXT_PROP};
use crate::{FocusReason, Value, common};
use accesskit::{NodeId as AxId, Role, Toggled};
use presto_base::color::Color;
use std::collections::HashSet;

fn ax(n: NodeId) -> AxId {
    AxId(n.0)
}

#[test]
fn the_first_update_is_a_valid_full_tree() {
    let (mut d, root) = doc();
    let a = child(&mut d, root, BOX, "width: 50px; height: 20px");
    let t = child(&mut d, a, TEXT, "");
    d.set_prop(t, TEXT_PROP, Value::from("Hello")).unwrap();
    let g = child(&mut d, root, TOGGLE, "");
    d.set_prop(g, common::ARIA_LABEL, Value::from("Wrap")).unwrap();
    let u = frame(&mut d).unwrap().a11y.unwrap();
    assert_eq!(u.tree.as_ref().map(|t| t.root), Some(ax(root)));
    assert_eq!(u.focus, ax(root), "no focus: the root");
    let ids: HashSet<AxId> = u.nodes.iter().map(|(id, _)| *id).collect();
    assert_eq!(ids.len(), u.nodes.len(), "no duplicates");
    assert_eq!(ids, [root, a, t, g].into_iter().map(ax).collect());
    for (_, n) in &u.nodes {
        for c in n.children() {
            assert!(ids.contains(c), "child {c:?} is in the tree");
        }
    }
    let node = |id: NodeId| &u.nodes.iter().find(|(k, _)| *k == ax(id)).unwrap().1;
    assert_eq!(node(t).role(), Role::Label);
    assert_eq!(node(t).label(), Some("Hello"));
    assert_eq!(node(g).role(), Role::CheckBox);
    assert_eq!(node(g).label(), Some("Wrap"));
    assert_eq!(node(g).toggled(), Some(Toggled::False));
    let b = node(a).bounds().unwrap();
    assert_eq!((b.x1 - b.x0, b.y1 - b.y0), (50.0, 20.0));
}

#[test]
fn later_updates_send_only_changed_nodes_and_the_focus() {
    let (mut d, root) = doc();
    // it paints already, so the tint changes its list, not the layer's items
    let a = child(&mut d, root, BOX, "width: 50px; height: 20px; background-color: #000000");
    let g = child(&mut d, root, TOGGLE, "");
    frame(&mut d);
    d.set_prop(a, BOX_TINT, Value::Color(Color::WHITE)).unwrap();
    let u = frame(&mut d).unwrap().a11y.unwrap();
    assert!(u.tree.is_none());
    assert_eq!(u.nodes.iter().map(|(id, _)| *id).collect::<Vec<_>>(), [ax(a)]);
    d.focus(g, FocusReason::Keyboard);
    let u = frame(&mut d).unwrap().a11y.unwrap();
    assert_eq!(u.focus, ax(g));
    assert!(u.nodes.iter().any(|(id, _)| *id == ax(g)));
}

#[test]
fn removing_nodes_resends_a_tree_without_them() {
    let (mut d, root) = doc();
    let a = child(&mut d, root, BOX, "");
    let b = child(&mut d, a, BOX, "");
    frame(&mut d);
    d.destroy(a);
    let u = frame(&mut d).unwrap().a11y.unwrap();
    let ids: Vec<AxId> = u.nodes.iter().map(|(id, _)| *id).collect();
    assert!(!ids.contains(&ax(a)) && !ids.contains(&ax(b)));
    let r = &u.nodes.iter().find(|(id, _)| *id == ax(root)).unwrap().1;
    assert!(r.children().is_empty());
}

#[test]
fn a_move_resends_only_the_node_that_moved() {
    let (mut d, root) = doc();
    let a = child(&mut d, root, BOX, "width: 50px; height: 20px; margin-left: 0px");
    let b = child(&mut d, a, BOX, "width: 10px; height: 10px");
    frame(&mut d);
    d.set_style(a, crate::StyleInput { inline: Some("width: 50px; height: 20px; margin-left: 30px".into()), ..Default::default() });
    let u = frame(&mut d).unwrap().a11y.expect("the moved node is re-sent");
    assert!(u.tree.is_none(), "an incremental update");
    let ids: Vec<AxId> = u.nodes.iter().map(|(id, _)| *id).collect();
    assert!(ids.contains(&ax(a)) && !ids.contains(&ax(b)), "its child keeps its transform to it");
    let n = &u.nodes.iter().find(|(id, _)| *id == ax(a)).unwrap().1;
    assert_eq!(n.transform().map(|t| t.as_coeffs()[4]), Some(30.0));
    let bounds = n.bounds().unwrap();
    assert_eq!((bounds.x0, bounds.x1), (0.0, 50.0), "bounds are local");
}

#[test]
fn an_inserted_subtree_joins_incrementally() {
    let (mut d, root) = doc();
    child(&mut d, root, BOX, "");
    frame(&mut d);
    let a = d.create(BOX);
    let b = d.create(TEXT);
    d.insert(a, b, None, None).unwrap();
    d.insert(root, a, None, None).unwrap();
    let u = frame(&mut d).unwrap().a11y.unwrap();
    assert!(u.tree.is_none());
    let ids: HashSet<AxId> = u.nodes.iter().map(|(id, _)| *id).collect();
    assert!(ids.contains(&ax(root)) && ids.contains(&ax(a)) && ids.contains(&ax(b)), "the parent and the new subtree");
}

static NAMED: crate::KindSpec = crate::KindSpec { container: true, role: Role::Button, name_from: &[crate::NameSource::Children], ..crate::KindSpec::new("named") };

#[derive(Default)]
struct Named;

impl crate::Kind for Named {
    fn spec() -> &'static crate::KindSpec {
        &NAMED
    }
    fn record(&self, cx: &mut crate::RecordCx) {
        cx.draw_box();
    }
}

#[test]
fn a_name_from_children_follows_their_text() {
    let mut r = crate::KindRegistry::new();
    r.register::<testkinds::BoxKind>().unwrap();
    r.register::<testkinds::TextKind>().unwrap();
    r.register::<Named>().unwrap();
    let mut d = Document::new(std::sync::Arc::new(r), Box::new(FixedText));
    let root = d.create_window(W, BOX, Size::new(200.0, 100.0), Scale(1.0));
    let b = child(&mut d, root, KindId::of("named"), "");
    let t1 = child(&mut d, b, TEXT, "");
    let t2 = child(&mut d, b, TEXT, "");
    d.set_prop(t1, TEXT_PROP, Value::from("Save")).unwrap();
    d.set_prop(t2, TEXT_PROP, Value::from("all")).unwrap();
    let u = frame(&mut d).unwrap().a11y.unwrap();
    assert_eq!(u.nodes.iter().find(|(id, _)| *id == ax(b)).unwrap().1.label(), Some("Save all"));
    d.set_prop(t2, TEXT_PROP, Value::from("as")).unwrap();
    let u = frame(&mut d).unwrap().a11y.unwrap();
    assert_eq!(u.nodes.iter().find(|(id, _)| *id == ax(b)).expect("re-sent").1.label(), Some("Save as"));
}
