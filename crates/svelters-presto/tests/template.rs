//! Native templates: `instantiate` builds a fragment's static part in one call with contiguous
//! handles (`base + k`), and leaves the tree exactly as today's op sequence does; `destroy_range`
//! frees it; a failure leaves nothing behind.

mod common;

use common::Rig;
use svelters_abi::*;
use svelters_presto::{Template, TemplateNode};

fn node(kind: svl_id, parent: Option<u32>) -> TemplateNode {
    TemplateNode { kind, parent, slot: 0, props: Vec::new(), classes: None, listens: Vec::new() }
}

/// The counter's column: box > (label, button "Increment" listening to `clicked`).
fn column(r: &Rig) -> Template {
    let br = r.br();
    let (bx, label, button) = (br.resolve(NameKind::Kind, 0, "box"), br.resolve(NameKind::Kind, 0, "label"), br.resolve(NameKind::Kind, 0, "button"));
    let text = br.resolve(NameKind::Prop, button, "text");
    let clicked = br.resolve(NameKind::Signal, button, "clicked");
    let mut b = node(button, Some(0));
    b.props.push((text, Value::Str("Increment".into())));
    b.classes = Some("svelte-1".into());
    b.listens.push((clicked, 0));
    Template { nodes: vec![node(bx, None), node(label, Some(0)), b] }
}

#[test]
fn instantiate_builds_the_same_tree_as_the_ops() {
    let r = Rig::new();
    let t = column(&r);
    let base = r.backend.instantiate(&t).unwrap();
    let (bx, label, button) = (base, base + 1, base + 2);
    assert_eq!(r.kind(label), r.br().resolve(NameKind::Kind, 0, "label"));
    assert_eq!(r.get(button, "text"), Value::Str("Increment".into()));
    let n = r.backend.node(bx).unwrap();
    let kids: Vec<svl_handle> = r.backend.with_doc(|d| d.children(n).iter().map(|&k| r.backend.handle(k).unwrap()).collect());
    assert_eq!(kids, [label, button]);
    let bn = r.backend.node(button).unwrap();
    let reg = r.backend.registry().clone();
    let listening = r.backend.with_doc(|d| {
        let (s, _) = reg.signal(d.kind(bn).unwrap(), "clicked").unwrap();
        d.listening(bn, s).is_some()
    });
    assert!(listening);
    assert_eq!(r.backend.with_doc(|d| d.style(bn).unwrap().classes.clone()), ["svelte-1"]);
    // the root is the caller's to insert
    assert_eq!(r.backend.with_doc(|d| d.parent(n)), None);
    // a second instance follows the first
    let again = r.backend.instantiate(&t).unwrap();
    assert_eq!(again, base + 3);
}

#[test]
fn destroy_range_frees_the_instance() {
    let r = Rig::new();
    let base = r.backend.instantiate(&column(&r)).unwrap();
    r.backend.destroy_range(base, 3);
    for k in 0..3 {
        assert!(r.br().handle_info(base + k).is_err());
    }
}

#[test]
fn a_failed_instantiation_leaves_nothing() {
    let r = Rig::new();
    let mut t = column(&r);
    let button = r.br().resolve(NameKind::Kind, 0, "button");
    // a child under the button, which takes none
    t.nodes.push(node(button, Some(2)));
    let next = r.create("box");
    assert!(r.backend.instantiate(&t).is_err());
    for k in 1..=4 {
        assert!(r.br().handle_info(next + k).is_err(), "handle {} survived", next + k);
    }
}

#[test]
fn templates_validate_their_shape() {
    let r = Rig::new();
    let bx = r.br().resolve(NameKind::Kind, 0, "box");
    assert!(Template::default().validate().is_err());
    assert!(Template { nodes: vec![node(bx, Some(0))] }.validate().is_err(), "a node is not its own parent");
    let mut reg = svelters_presto::template::Templates::default();
    reg.register(7, column(&r)).unwrap();
    reg.register(7, column(&r)).unwrap();
    reg.release(7);
    assert!(reg.get(7).is_some());
    reg.release(7);
    assert!(reg.is_empty());
}
