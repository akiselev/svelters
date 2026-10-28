//! The accessibility projection: a valid full tree first, incremental updates after, roles, labels,
//! states and focus from the document.

use super::*;
use crate::testkinds::{BOX_TINT, TEXT as TEXT_PROP};
use crate::{FocusReason, Value, common};
use accesskit::{NodeId as AxId, Role, Toggled};
use skelter_base::color::Color;
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

static OPTS: crate::KindSpec = crate::KindSpec {
    props: &[
        crate::PropSpec::new("count", crate::ValueType::I64, skelter_base::change::ChangeClass::Paint).default(crate::PropDefault::I64(0)),
        crate::PropSpec::new("picked", crate::ValueType::I64, skelter_base::change::ChangeClass::Paint).default(crate::PropDefault::I64(-1)).native(),
        crate::PropSpec::new("nest", crate::ValueType::Bool, skelter_base::change::ChangeClass::Paint).default(crate::PropDefault::Bool(false)),
    ],
    role: Role::ListBox,
    container: true,
    ..crate::KindSpec::new("opts")
};
const OPTS_COUNT: crate::PropId = crate::PropId::of("opts", "count", "int");
const OPTS_PICKED: crate::PropId = crate::PropId::of("opts", "picked", "int");
const OPTS_NEST: crate::PropId = crate::PropId::of("opts", "nest", "bool");

/// A list drawn without nodes: `count` virtual options with author ids `opt-<i>`; a click on one
/// writes its index to `picked`. With `nest`, each option has one cell under it.
#[derive(Default)]
struct Opts;

impl crate::Kind for Opts {
    fn spec() -> &'static crate::KindSpec {
        &OPTS
    }
    fn record(&self, cx: &mut crate::RecordCx) {
        cx.draw_box();
    }
    fn event(&mut self, cx: &mut crate::EventCx, ev: &crate::Event) -> crate::Handled {
        match ev {
            crate::Event::A11yChild(r, i) if r.action == accesskit::Action::Click => {
                let _ = cx.set_native(OPTS_PICKED, Value::I64(*i as i64));
                crate::Handled::Yes
            }
            _ => crate::Handled::No,
        }
    }
    fn a11y(&self, cx: &mut crate::A11yCx) {
        cx.default_a11y();
        let n = cx.node.prop(OPTS_COUNT).and_then(Value::as_i64).unwrap_or(0);
        if cx.node.prop_bool(OPTS_NEST) {
            for i in 0..n {
                let row = cx.add_virtual_child(accesskit::Node::new(Role::Row)).unwrap();
                let mut c = accesskit::Node::new(Role::Cell);
                c.add_action(accesskit::Action::Click);
                assert_eq!(cx.add_virtual_child_of(row, c), Some(2 * i as u32 + 1));
            }
            assert_eq!(cx.add_virtual_child_of(999, accesskit::Node::new(Role::Cell)), None, "no such parent");
            return;
        }
        for i in 0..n {
            let mut o = accesskit::Node::new(Role::ListBoxOption);
            o.set_author_id(format!("opt-{i}"));
            o.add_action(accesskit::Action::Click);
            assert_eq!(cx.add_virtual_child(o), Some(i as u32));
        }
    }
}

#[test]
fn virtual_children_go_out_with_their_owner_and_take_actions() {
    let mut r = crate::KindRegistry::new();
    r.register::<testkinds::BoxKind>().unwrap();
    r.register::<Opts>().unwrap();
    let mut d = Document::new(std::sync::Arc::new(r), Box::new(FixedText));
    let root = d.create_window(W, BOX, Size::new(200.0, 100.0), Scale(1.0));
    let l = child(&mut d, root, KindId::of("opts"), "");
    let b = child(&mut d, l, BOX, "");
    d.set_prop(l, OPTS_COUNT, Value::I64(3)).unwrap();
    let u = frame(&mut d).unwrap().a11y.unwrap();
    let v = |i| crate::a11y::virtual_id(l, i);
    let owner = &u.nodes.iter().find(|(id, _)| *id == ax(l)).unwrap().1;
    assert_eq!(owner.children(), &[ax(b), v(0), v(1), v(2)], "after the real children");
    for i in 0..3 {
        let n = &u.nodes.iter().find(|(id, _)| *id == v(i)).expect("sent with the owner").1;
        assert_eq!(n.author_id(), Some(format!("opt-{i}").as_str()));
        assert_ne!(v(i).0 & crate::a11y::VIRTUAL_BIT, 0);
        assert_eq!(crate::a11y::virtual_child(v(i)), Some((l, i)));
    }
    assert_eq!(crate::a11y::virtual_child(ax(l)), None, "a node's id is not virtual");
    // an action on child 2 reaches the kind with its index
    let req = accesskit::ActionRequest { action: accesskit::Action::Click, target_tree: accesskit::TreeId::ROOT, target_node: v(2), data: None };
    let out = d.a11y_action(W, &req);
    assert!(out.handled);
    assert_eq!(out.target, Some(l));
    assert_eq!(d.get_prop(l, OPTS_PICKED), Some(Value::I64(2)));
    let _ = frame(&mut d);
    // fewer children: the owner is re-sent without the dropped ones, which are not sent
    d.set_prop(l, OPTS_COUNT, Value::I64(1)).unwrap();
    let u = frame(&mut d).unwrap().a11y.unwrap();
    let owner = &u.nodes.iter().find(|(id, _)| *id == ax(l)).expect("re-sent").1;
    assert_eq!(owner.children(), &[ax(b), v(0)]);
    let ids: HashSet<AxId> = u.nodes.iter().map(|(id, _)| *id).collect();
    assert!(ids.contains(&v(0)) && !ids.contains(&v(1)) && !ids.contains(&v(2)));
    // an update that does not re-send the owner names none of them
    d.set_prop(b, BOX_TINT, Value::Color(Color::WHITE)).unwrap();
    if let Some(u) = frame(&mut d).and_then(|c| c.a11y) {
        assert!(u.nodes.iter().all(|(id, _)| crate::a11y::virtual_child(*id).is_none()), "virtual children only with their owner");
    }
}

/// A virtual child under another (a table row's cell) is that one's child, not the owner's, and
/// takes actions by its own index.
#[test]
fn nested_virtual_children_hang_under_their_parent() {
    let mut r = crate::KindRegistry::new();
    r.register::<testkinds::BoxKind>().unwrap();
    r.register::<Opts>().unwrap();
    let mut d = Document::new(std::sync::Arc::new(r), Box::new(FixedText));
    let root = d.create_window(W, BOX, Size::new(200.0, 100.0), Scale(1.0));
    let l = child(&mut d, root, KindId::of("opts"), "");
    d.set_prop(l, OPTS_NEST, Value::Bool(true)).unwrap();
    d.set_prop(l, OPTS_COUNT, Value::I64(2)).unwrap();
    let u = frame(&mut d).unwrap().a11y.unwrap();
    let v = |i| crate::a11y::virtual_id(l, i);
    let node = |id| &u.nodes.iter().find(|(x, _)| *x == id).expect("sent").1;
    assert_eq!(node(ax(l)).children(), &[v(0), v(2)], "only the rows under the owner");
    assert_eq!(node(v(0)).children(), &[v(1)]);
    assert_eq!(node(v(2)).children(), &[v(3)]);
    assert_eq!(node(v(3)).role(), Role::Cell);
    let req = accesskit::ActionRequest { action: accesskit::Action::Click, target_tree: accesskit::TreeId::ROOT, target_node: v(3), data: None };
    assert!(d.a11y_action(W, &req).handled);
    assert_eq!(d.get_prop(l, OPTS_PICKED), Some(Value::I64(3)));
}

#[test]
fn aria_live_and_busy_apply_after_the_kinds_own_node() {
    let (mut d, root) = doc();
    let status = child(&mut d, root, TEXT, "");
    frame(&mut d);
    d.set_prop(status, common::ARIA_LIVE, Value::Enum(1)).unwrap();
    d.set_prop(status, common::ARIA_BUSY, Value::Bool(true)).unwrap();
    let u = frame(&mut d).unwrap().a11y.expect("the props re-project the node");
    let n = &u.nodes.iter().find(|(k, _)| *k == ax(status)).unwrap().1;
    assert_eq!(n.live(), Some(accesskit::Live::Polite));
    assert!(n.is_busy());
    d.set_prop(status, common::ARIA_LIVE, Value::Enum(2)).unwrap();
    d.set_prop(status, common::ARIA_BUSY, Value::Bool(false)).unwrap();
    let u = frame(&mut d).unwrap().a11y.unwrap();
    let n = &u.nodes.iter().find(|(k, _)| *k == ax(status)).unwrap().1;
    assert_eq!(n.live(), Some(accesskit::Live::Assertive));
    assert!(!n.is_busy());
}

#[test]
fn role_level_expanded_selected_and_description_make_a_row_a_tree_item() {
    let (mut d, root) = doc();
    let row = child(&mut d, root, BOX, "width: 50px; height: 20px");
    frame(&mut d);
    d.set_prop(row, common::ROLE, Value::from("treeitem")).unwrap();
    d.set_prop(row, common::ARIA_LEVEL, Value::I64(2)).unwrap();
    d.set_prop(row, common::ARIA_EXPANDED, Value::Enum(1)).unwrap();
    d.set_prop(row, common::ARIA_SELECTED, Value::Enum(2)).unwrap();
    d.set_prop(row, common::ARIA_DESCRIPTION, Value::from("3 children")).unwrap();
    let u = frame(&mut d).unwrap().a11y.expect("the props re-project the node");
    let n = &u.nodes.iter().find(|(k, _)| *k == ax(row)).unwrap().1;
    assert_eq!(n.role(), Role::TreeItem);
    assert_eq!(n.level(), Some(2));
    assert_eq!(n.is_expanded(), Some(true));
    assert_eq!(n.is_selected(), Some(false));
    assert_eq!(n.description(), Some("3 children"));
    // an unknown role keeps the kind's; unset leaves the states off
    d.set_prop(row, common::ROLE, Value::from("bogus")).unwrap();
    d.set_prop(row, common::ARIA_EXPANDED, Value::Enum(0)).unwrap();
    let u = frame(&mut d).unwrap().a11y.unwrap();
    let n = &u.nodes.iter().find(|(k, _)| *k == ax(row)).unwrap().1;
    assert_eq!(n.role(), Role::GenericContainer);
    assert_eq!(n.is_expanded(), None);
}

#[test]
fn menu_roles_and_the_aria_props_of_qa11y_f4() {
    let (mut d, root) = doc();
    let bar = child(&mut d, root, BOX, "width: 100px; height: 20px");
    let file = child(&mut d, bar, BOX, "width: 40px; height: 20px");
    let menu = child(&mut d, root, BOX, "width: 60px; height: 40px");
    let item = child(&mut d, menu, BOX, "width: 60px; height: 20px");
    let hidden = child(&mut d, menu, BOX, "display: none");
    d.set_prop(item, common::ANCHOR, Value::from("wrap")).unwrap();
    d.set_prop(menu, common::ANCHOR, Value::from("file-menu")).unwrap();
    d.set_prop(hidden, common::ANCHOR, Value::from("gone")).unwrap();
    d.set_prop(bar, common::ROLE, Value::from("menubar")).unwrap();
    d.set_prop(file, common::ROLE, Value::from("menuitem")).unwrap();
    d.set_prop(file, common::ARIA_HASPOPUP, Value::Enum(1)).unwrap();
    d.set_prop(file, common::ARIA_CONTROLS, Value::from("file-menu nowhere")).unwrap();
    d.set_prop(menu, common::ROLE, Value::from("menu")).unwrap();
    d.set_prop(menu, common::ARIA_ACTIVEDESCENDANT, Value::from("wrap")).unwrap();
    d.set_prop(item, common::ROLE, Value::from("menuitemcheckbox")).unwrap();
    d.set_prop(item, common::ARIA_CHECKED, Value::Enum(1)).unwrap();
    d.set_prop(root, common::ARIA_MODAL, Value::Bool(true)).unwrap();
    let u = frame(&mut d).unwrap().a11y.unwrap();
    let n = |id: NodeId| u.nodes.iter().find(|(k, _)| *k == ax(id)).unwrap().1.clone();
    assert_eq!(n(bar).role(), Role::MenuBar);
    assert_eq!(n(file).has_popup(), Some(accesskit::HasPopup::Menu));
    assert_eq!(n(file).controls(), &[ax(menu)], "an anchor that names no node is left out");
    assert_eq!(n(menu).active_descendant(), Some(ax(item)));
    assert_eq!(n(item).role(), Role::MenuItemCheckBox);
    assert_eq!(n(item).toggled(), Some(Toggled::True));
    assert!(n(root).is_modal());
    for (name, role) in [("menuitemradio", Role::MenuItemRadio), ("alertdialog", Role::AlertDialog), ("searchbox", Role::SearchInput), ("spinbutton", Role::SpinButton), ("tooltip", Role::Tooltip), ("none", Role::GenericContainer), ("presentation", Role::GenericContainer)] {
        assert_eq!(common::role_named(name), Some(role), "{name}");
    }
    // a reference to a node outside the tree is dropped before the update leaves
    d.set_prop(menu, common::ARIA_ACTIVEDESCENDANT, Value::from("gone")).unwrap();
    d.set_prop(file, common::ARIA_CONTROLS, Value::from("gone")).unwrap();
    let u = frame(&mut d).unwrap().a11y.unwrap();
    let n = |id: NodeId| u.nodes.iter().find(|(k, _)| *k == ax(id)).unwrap().1.clone();
    // `gone` is sent hidden (display: none keeps it in the tree), so it may stay; a stale id never does
    let present = |a: AxId| u.nodes.iter().any(|(k, _)| *k == a) || a == ax(hidden);
    assert!(n(menu).active_descendant().is_none_or(present));
    assert!(n(file).controls().iter().all(|c| present(*c)));
    // unset: the kind's own
    d.set_prop(item, common::ARIA_CHECKED, Value::Enum(0)).unwrap();
    let u = frame(&mut d).unwrap().a11y.unwrap();
    assert_eq!(u.nodes.iter().find(|(k, _)| *k == ax(item)).unwrap().1.toggled(), None);
}

/// SV-OBJ4: what a `described` node exposes lives in a document service.
struct Described(&'static str);

static DESCRIBED: crate::KindSpec = crate::KindSpec { role: Role::Canvas, ..crate::KindSpec::new("described") };

#[derive(Default)]
struct DescribedKind;

impl crate::Kind for DescribedKind {
    fn spec() -> &'static crate::KindSpec {
        &DESCRIBED
    }
    fn record(&self, cx: &mut crate::RecordCx) {
        cx.draw_box();
    }
    fn a11y(&self, cx: &mut crate::A11yCx) {
        cx.default_a11y();
        if let Some(d) = cx.service::<Described>() {
            cx.out.set_description(d.0);
        }
    }
}

#[test]
fn sv_obj4_a_kind_reads_a_service_and_the_host_resends_its_node() {
    let mut r = crate::KindRegistry::new();
    r.register::<testkinds::BoxKind>().unwrap();
    r.register::<DescribedKind>().unwrap();
    let mut d = Document::new(std::sync::Arc::new(r), Box::new(FixedText));
    let root = d.create_window(W, BOX, Size::new(200.0, 100.0), Scale(1.0));
    let c = child(&mut d, root, KindId::of("described"), "width: 50px; height: 50px");
    let other = child(&mut d, root, BOX, "width: 50px; height: 50px");
    d.set_service(Described("two parts"));
    let u = frame(&mut d).unwrap().a11y.unwrap();
    let desc = |u: &accesskit::TreeUpdate, n: NodeId| u.nodes.iter().find(|(id, _)| *id == ax(n)).map(|(_, x)| x.description().map(str::to_owned));
    assert_eq!(desc(&u, c), Some(Some("two parts".into())), "read through A11yCx::service");
    // the service changes outside the node's props: nothing goes out until the host asks
    d.set_service(Described("three parts"));
    assert!(frame(&mut d).and_then(|c| c.a11y).is_none());
    d.request_a11y(c);
    let u = frame(&mut d).unwrap().a11y.expect("the node goes out again");
    assert_eq!(desc(&u, c), Some(Some("three parts".into())));
    assert_eq!(u.nodes.len(), 1, "and no other node");
    assert!(desc(&u, other).is_none());
    // a removed node: nothing
    d.destroy(c);
    let _ = frame(&mut d);
    d.request_a11y(c);
    assert!(frame(&mut d).and_then(|c| c.a11y).is_none(), "request_a11y on a removed node is a no-op");
}
