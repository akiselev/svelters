//! Tree edits (ported from svelters-egui's SG1 scene tests where they apply).

use super::*;
use crate::TreeError;

#[test]
fn insert_orders_and_moves_children() {
    let (mut d, root) = doc();
    let a = d.create(BOX);
    let b = d.create(BOX);
    let c = d.create(BOX);
    d.insert(root, a, None, None).unwrap();
    d.insert(root, c, None, None).unwrap();
    d.insert(root, b, Some(c), None).unwrap();
    assert_eq!(d.children(root), &[a, b, c]);
    // moving a node removes it from its old parent
    d.insert(a, c, None, None).unwrap();
    assert_eq!(d.children(root), &[a, b]);
    assert_eq!(d.children(a), &[c]);
    assert_eq!(d.parent(c), Some(a));
    assert_eq!(d.window_of(c), Some(W));
    // before = itself keeps the position
    d.insert(root, a, Some(a), None).unwrap();
    assert_eq!(d.children(root), &[a, b]);
}

#[test]
fn invalid_edits_are_rejected() {
    let (mut d, root) = doc();
    let a = child(&mut d, root, BOX, "");
    let b = child(&mut d, a, BOX, "");
    assert_eq!(d.insert(b, a, None, None), Err(TreeError::Cycle { parent: b, child: a }));
    assert_eq!(d.insert(a, a, None, None), Err(TreeError::Cycle { parent: a, child: a }));
    assert_eq!(d.insert(a, root, None, None), Err(TreeError::IsRoot(root)));
    let t = child(&mut d, root, TEXT, "");
    let x = d.create(BOX);
    assert_eq!(d.insert(t, x, None, None), Err(TreeError::NotAContainer(t)));
    assert_eq!(d.insert(root, x, Some(b), None), Err(TreeError::NotAChild { parent: root, child: b }));
    assert_eq!(d.remove(root, b), Err(TreeError::NotAChild { parent: root, child: b }));
    let slot = crate::SlotId::of("box", "header", "slot");
    assert_eq!(d.insert(root, x, None, Some(slot)), Err(TreeError::UnknownSlot { parent: root, slot }));
}

#[test]
fn destroy_frees_the_subtree_and_ids_go_stale() {
    let (mut d, root) = doc();
    let a = child(&mut d, root, BOX, "");
    let b = child(&mut d, a, BOX, "");
    let live = d.len();
    d.destroy(a);
    assert!(!d.contains(a) && !d.contains(b));
    assert_eq!(d.len(), live - 2);
    assert!(d.children(root).is_empty());
    assert_eq!(d.insert(root, a, None, None), Err(TreeError::Stale(a)));
    // a recycled slot gets a new generation: the old id stays stale
    let c = d.create(BOX);
    assert!(!d.contains(a) || c != a);
    assert_ne!(c, a);
    d.destroy(a); // stale: ignored
}

#[test]
fn remove_detaches_and_keeps_the_node() {
    let (mut d, root) = doc();
    let a = child(&mut d, root, BOX, "");
    let b = child(&mut d, a, TEXT, "");
    d.remove(root, a).unwrap();
    assert!(d.contains(a) && d.contains(b));
    assert_eq!(d.window_of(a), None);
    assert_eq!(d.window_of(b), None);
    d.insert(root, a, None, None).unwrap();
    assert_eq!(d.window_of(b), Some(W));
}

#[test]
fn the_journal_records_edits_in_order() {
    let (mut d, root) = doc();
    d.drain_changes();
    let a = child(&mut d, root, BOX, "");
    d.destroy(a);
    let kinds: Vec<&str> = d
        .drain_changes()
        .iter()
        .map(|c| match c {
            crate::journal::Change::Created { .. } => "created",
            crate::journal::Change::Inserted { .. } => "inserted",
            crate::journal::Change::Removed { .. } => "removed",
            crate::journal::Change::Destroyed { .. } => "destroyed",
            crate::journal::Change::Prop { .. } => "prop",
            crate::journal::Change::Style { .. } => "style",
            crate::journal::Change::State { .. } => "state",
        })
        .collect();
    assert_eq!(kinds, ["created", "inserted", "destroyed"]);
}

#[test]
fn flag_bits_are_distinct() {
    use crate::NodeFlags as F;
    let all = [F::ROOT, F::LAYOUT_BOUNDARY, F::IN_RELAYOUT, F::IN_REMEASURE, F::NONPASSIVE, F::DESC_NONPASSIVE, F::FOLDS, F::VIRTUAL, F::ROW, F::WANT_SCROLL, F::SCROLL_BELOW, F::SCROLL_CENTER, F::INK_OVERFLOW];
    for (k, a) in all.iter().enumerate() {
        for b in &all[k + 1..] {
            assert!(!a.intersects(*b), "{a:?} and {b:?} share a bit");
        }
    }
}
