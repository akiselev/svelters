//! kittest's query semantics on a fixed tree: a form whose text field is labelled by a label node, two
//! "OK" buttons (one inside the form), a hidden node. Covers the label rule (label nodes match by their
//! text and drop out when they label another match), predicates, scoped and non-recursive queries,
//! `children`/`parent`, `accesskit_node()`, and the panics of `get_*`/`query_*`.

use skelter::seams::{Dispatched, DocumentApi};
use skelter_base::accesskit::{self, ActionRequest, Node as AkNode, Role, TreeInfo, TreeUpdate};
use skelter_base::anim::AnimationEvent;
use skelter_base::commit::Commit;
use skelter_base::event::{InputEnvelope, WindowEvent};
use skelter_base::geom::{Scale, Size, Vec2};
use skelter_base::ids::{PropNodeId, Revision, WindowId};
use skelter_base::time::Time;
use skelter_test::{By, Harness, by};

const ROOT: u64 = 1;
const FORM: u64 = 2;
const NAME_LABEL: u64 = 3;
const NAME_FIELD: u64 = 4;
const FORM_OK: u64 = 5;
const OK: u64 = 6;
const HIDDEN: u64 = 7;

struct Fixed {
    sent: bool,
}

fn rect(x: f64, y: f64) -> accesskit::Rect {
    accesskit::Rect {
        x0: x,
        y0: y,
        x1: x + 80.0,
        y1: y + 20.0,
    }
}

impl Fixed {
    fn tree() -> TreeUpdate {
        let id = accesskit::NodeId;
        let mut root = AkNode::new(Role::Window);
        root.set_children(vec![id(FORM), id(OK), id(HIDDEN)]);
        let mut form = AkNode::new(Role::Group);
        form.set_label("Form");
        form.set_author_id("form");
        form.set_bounds(rect(0.0, 0.0));
        form.set_children(vec![id(NAME_LABEL), id(NAME_FIELD), id(FORM_OK)]);
        let mut label = AkNode::new(Role::Label);
        label.set_value("Name:");
        label.set_bounds(rect(0.0, 0.0));
        let mut field = AkNode::new(Role::TextInput);
        field.set_labelled_by(vec![id(NAME_LABEL)]);
        field.set_value("Ann");
        field.set_bounds(rect(0.0, 30.0));
        let mut form_ok = AkNode::new(Role::Button);
        form_ok.set_label("OK");
        form_ok.set_author_id("form-ok");
        form_ok.set_bounds(rect(0.0, 60.0));
        let mut ok = AkNode::new(Role::Button);
        ok.set_label("OK");
        ok.set_description("outside");
        ok.set_bounds(rect(100.0, 60.0));
        let mut hidden = AkNode::new(Role::Button);
        hidden.set_label("Secret");
        hidden.set_hidden();
        hidden.set_bounds(rect(200.0, 0.0));
        TreeUpdate {
            nodes: vec![
                (id(ROOT), root),
                (id(FORM), form),
                (id(NAME_LABEL), label),
                (id(NAME_FIELD), field),
                (id(FORM_OK), form_ok),
                (id(OK), ok),
                (id(HIDDEN), hidden),
            ],
            tree: Some(TreeInfo::new(id(ROOT))),
            tree_id: accesskit::TreeId::ROOT,
            focus: id(NAME_FIELD),
        }
    }
}

impl DocumentApi for Fixed {
    fn dispatch(&mut self, _ev: &InputEnvelope) -> Dispatched {
        Dispatched::default()
    }
    fn frame(&mut self, window: WindowId, _target: Time) -> Option<Commit> {
        if std::mem::replace(&mut self.sent, true) {
            return None;
        }
        Some(Commit {
            window,
            revision: Revision(1),
            a11y: Some(Self::tree()),
            ..Commit::default()
        })
    }
    fn needs_frame(&self, _window: WindowId) -> bool {
        !self.sent
    }
    fn resize(&mut self, _window: WindowId, _size: Size, _scale: Scale) {}
    fn close_window(&mut self, _window: WindowId) {}
    fn scroll_moved(&mut self, _window: WindowId, _node: PropNodeId, _offset: Vec2) {}
    fn animation_event(&mut self, _window: WindowId, _ev: AnimationEvent) {}
    fn accessibility_action(&mut self, _window: WindowId, _req: &ActionRequest) -> bool {
        false
    }
    fn accessibility_activated(&mut self, _window: WindowId) {}
    fn window_event(&mut self, _window: WindowId, _ev: &WindowEvent) {}
    fn device_reset(&mut self) {}
}

fn harness() -> Harness<Fixed> {
    Harness::new(Fixed { sent: false })
}

#[test]
fn a_label_query_finds_the_labelled_field_not_its_label() {
    let h = harness();
    let field = h.get_by_label("Name:");
    assert_eq!(field.role(), Role::TextInput);
    assert_eq!(field.label().as_deref(), Some("Name:"));
    assert_eq!(
        h.query_all(by().label("Name:").include_labels()).len(),
        2,
        "include_labels keeps the label node"
    );
    assert_eq!(h.get_by_label_contains("Name").role(), Role::TextInput);
}

#[test]
fn scoped_queries_search_below_the_node() {
    let h = harness();
    assert_eq!(h.query_all_by_label("OK").len(), 2);
    let form = h.get_by_author_id("form");
    let ok = form.get_by_label("OK");
    assert_eq!(ok.author_id(), Some("form-ok"));
    assert_eq!(form.get_by_role_and_label(Role::Button, "OK").id(), ok.id());
    assert_eq!(form.children().len(), 3);
    assert_eq!(form.children_recursive().len(), 3);
    assert_eq!(h.root().children().len(), 3, "form, OK, hidden");
    assert_eq!(h.root().children_recursive().len(), 6);
    assert_eq!(ok.parent().map(|p| p.id()), Some(form.id()));
    assert!(
        form.query_by_label("Form").is_none(),
        "the scope node itself never matches"
    );
}

#[test]
fn predicates_see_the_accesskit_node() {
    let h = harness();
    let outside = h.get_by(|n| n.description().as_deref() == Some("outside"));
    assert_eq!(outside.label().as_deref(), Some("OK"));
    assert_eq!(h.query_all_by(|n| n.role() == Role::Button).len(), 3);
    assert_eq!(
        h.get_by_value("Ann").accesskit_node().role(),
        Role::TextInput
    );
    assert!(h.get_by_value("Ann").is_focused());
    assert_eq!(h.get_all_by_role(Role::Button).count(), 3);
}

#[test]
fn hidden_nodes_match_unless_excluded() {
    let h = harness();
    assert!(h.query_by_label("Secret").is_some());
    assert!(h.query(By::new().label("Secret").exclude_hidden()).is_none());
}

#[test]
fn non_recursive_queries_stop_at_children() {
    let h = harness();
    assert_eq!(
        h.query_all(By::new().role(Role::Button).recursive(false)).len(),
        2,
        "OK and the hidden button; the form's OK is a grandchild"
    );
}

#[test]
#[should_panic(expected = "no node matches")]
fn get_all_panics_on_none() {
    let h = harness();
    let _ = h.get_all_by_label("Nope");
}

#[test]
#[should_panic(expected = "expected at most one")]
fn query_panics_on_several() {
    let h = harness();
    let _ = h.query_by_label("OK");
}

#[test]
fn unnamed_inputs_fail_the_accessibility_check() {
    let h = harness();
    assert!(h.unnamed_widgets().is_empty(), "the field is named by its label");
}
