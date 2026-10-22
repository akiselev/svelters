//! SG2 tests that need the scene's internals (ARCH-SCENE §14 SG2):
//!
//! - `journal_replay_rebuilds_the_dump`: random runtime and Rust op interleavings (structure, text,
//!   state bits, sparse and dense extension keys, batches, subtree frees); an observer applies the
//!   records it takes to a mirror tree, which must print the scene's dump (with state and ext).
//! - extension columns (sparse and dense), ticks, `with_ext`; R19's `Ext { Removed }` on remove and
//!   on destroy; filters (scope, kinds, props, states, origins, ignore); batches, overflow, dropped
//!   observers; layout records and relayout damage.
//!
//! The counting-allocator test and the `with_ext` timing are in `tests/scene.rs`.

use super::*;
use crate::layout::{self, Metrics};
use std::collections::{BTreeSet, HashMap};
use svelters_abi::Value;

static NOTE: ExtKey<u32> = ExtKey::new("sg2.note");
static PICK: ExtKey<u8> = ExtKey::dense("sg2.pick");
static OTHER: ExtKey<String> = ExtKey::new("sg2.other");

fn kind(tag: &str) -> usize {
    kinds::tag_index(tag).unwrap()
}

fn set(s: &mut Scene, id: NodeId, name: &str, v: Value, o: Origin) {
    let p = kinds::find_prop(s.core[s.ix(id)].kind as usize, name)
        .unwrap()
        .0;
    s.set_value(id, p, &v, o).unwrap();
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn pick<T: Copy>(&mut self, v: &[T]) -> Option<T> {
        (!v.is_empty()).then(|| v[self.below(v.len())])
    }
}

// ---- the mirror an observer rebuilds ----

struct MNode {
    tag: &'static str,
    owner: Owner,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
    text: String,
    state: StateBits,
    ext: BTreeSet<ExtId>,
}

#[derive(Default)]
struct Mirror {
    n: HashMap<NodeId, MNode>,
}

impl Mirror {
    fn apply(&mut self, s: &Scene, c: &Change) {
        match *c {
            Change::Created { node, kind, owner } => {
                let tag = kinds::kind(kinds::kind_of_id(kind.0).unwrap()).tag.unwrap();
                let prev = self.n.insert(
                    node,
                    MNode {
                        tag,
                        owner,
                        parent: None,
                        children: vec![],
                        text: String::new(),
                        state: StateBits::NONE,
                        ext: BTreeSet::new(),
                    },
                );
                assert!(prev.is_none(), "{node} created twice");
            }
            Change::Inserted {
                parent,
                node,
                before,
            } => {
                assert_eq!(
                    self.n[&node].parent, None,
                    "{node} inserted while in {parent:?}"
                );
                let kids = &mut self.n.get_mut(&parent).unwrap().children;
                let at = before
                    .map(|b| {
                        kids.iter()
                            .position(|&x| x == b)
                            .expect("before is a child")
                    })
                    .unwrap_or(kids.len());
                kids.insert(at, node);
                self.n.get_mut(&node).unwrap().parent = Some(parent);
            }
            Change::Removed { parent, node } => {
                assert_eq!(self.n[&node].parent, Some(parent));
                self.n
                    .get_mut(&parent)
                    .unwrap()
                    .children
                    .retain(|&x| x != node);
                self.n.get_mut(&node).unwrap().parent = None;
            }
            Change::Destroyed { node, .. } => {
                let m = self.n.remove(&node).expect("destroyed node known");
                if let Some(p) = m.parent
                    && let Some(pm) = self.n.get_mut(&p)
                {
                    pm.children.retain(|&x| x != node);
                }
                for k in m.children {
                    if let Some(km) = self.n.get_mut(&k) {
                        km.parent = None;
                    }
                }
                assert!(
                    m.ext.is_empty(),
                    "{node} destroyed with ext entries not journaled as removed: {:?}",
                    m.ext
                );
            }
            Change::Prop { node, .. } => {
                // the new value is read from the scene (records name what changed)
                if let Some(n) = SceneRef::new(s).get(node) {
                    self.n.get_mut(&node).unwrap().text = n.text().to_string();
                }
            }
            Change::State { node, after, .. } => self.n.get_mut(&node).unwrap().state = after,
            Change::Ext { node, ext, what } => {
                let e = &mut self.n.get_mut(&node).unwrap().ext;
                match what {
                    ExtChange::Added => {
                        assert!(e.insert(ext), "{node}: {} added twice", ext.name())
                    }
                    ExtChange::Changed => assert!(
                        e.contains(&ext),
                        "{node}: {} changed, not present",
                        ext.name()
                    ),
                    ExtChange::Removed => assert!(
                        e.remove(&ext),
                        "{node}: {} removed, not present",
                        ext.name()
                    ),
                }
            }
            _ => {}
        }
    }

    /// The same text as `Scene::dump` with `state` and `ext`.
    fn dump(&self, top: NodeId) -> String {
        let mut out = String::new();
        let mut stack = vec![(top, 0usize)];
        while let Some((id, depth)) = stack.pop() {
            let m = &self.n[&id];
            out.push_str(&"  ".repeat(depth));
            out.push_str(&format!("<{}> {id}", m.tag));
            match m.owner {
                Owner::Js => {}
                Owner::Rust(p) => out.push_str(&format!(" [rust:{}]", p.name())),
                Owner::Host => out.push_str(" [host]"),
            }
            if !m.text.is_empty() {
                out.push_str(&format!(" {:?}", m.text));
            }
            if m.state != StateBits::NONE {
                out.push_str(&format!(" state({})", m.state.names().join("|")));
            }
            if !m.ext.is_empty() {
                out.push_str(&format!(
                    " ext({})",
                    m.ext.iter().map(|k| k.name()).collect::<Vec<_>>().join(",")
                ));
            }
            out.push('\n');
            for &k in m.children.iter().rev() {
                stack.push((k, depth + 1));
            }
        }
        out
    }

    fn dump_all(&self) -> String {
        let mut tops: Vec<NodeId> = self
            .n
            .iter()
            .filter(|(_, m)| m.parent.is_none())
            .map(|(&id, _)| id)
            .collect();
        tops.sort();
        tops.into_iter().map(|t| self.dump(t)).collect()
    }
}

fn scene_dump_all(s: &Scene) -> String {
    let opts = DumpOpts {
        state: true,
        ext: true,
        ..Default::default()
    };
    let mut tops: Vec<NodeId> = s
        .ids()
        .filter(|&id| s.core[s.ix(id)].parent.is_none())
        .collect();
    tops.sort();
    tops.into_iter().map(|t| s.dump(t, opts)).collect()
}

/// ARCH-SCENE §14 SG2: an observer sees a journal it can replay (it rebuilds the same dump).
#[test]
fn journal_replay_rebuilds_the_dump() {
    let rust_p = ProviderId::named("sg2-test");
    let rust = Origin::Rust(rust_p);
    let seeds: u64 = std::env::var("SG2_SEEDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(16);
    let mut takes = 0;
    let mut records = 0;
    for seed in 1..=seeds {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let mut s = Scene::default();
        // nodes made before the observer: the mirror starts from a take of an empty journal plus a
        // copy, so start empty instead
        let obs = s.journal.observe(ChangeFilter::all());
        // a narrower observer and a short-lived one alongside (they must not disturb the first)
        let narrow = s
            .journal
            .observe(ChangeFilter::only(ChangeMask::EXT).ext(&NOTE));
        let mut short = Some(s.journal.observe(ChangeFilter::only(ChangeMask::PROPS)));
        let mut mirror = Mirror::default();
        let mut note_live: BTreeSet<NodeId> = BTreeSet::new();
        for step in 0..400 {
            let ctx = format!("seed {seed} step {step}");
            let mut live: Vec<NodeId> = s.ids().collect();
            live.sort();
            let origin = if rng.below(3) == 0 { rust } else { Origin::Js };
            let js = matches!(origin, Origin::Js);
            match rng.below(24) {
                0..=4 => {
                    let tags: &[&str] = if js {
                        &["box", "box", "label", "button", "window"]
                    } else {
                        &["box", "label"]
                    };
                    let tag = rng.pick(tags).unwrap();
                    if tag == "window" && s.roots.len() >= 2 {
                        continue;
                    }
                    let owner = if js { Owner::Js } else { Owner::Rust(rust_p) };
                    let id = s.create(kind(tag), owner).unwrap();
                    if tag == "label" {
                        set(&mut s, id, "text", Value::Str(format!("l{step}")), origin);
                    }
                }
                5..=10 => {
                    let (Some(p), Some(c)) = (rng.pick(&live), rng.pick(&live)) else {
                        continue;
                    };
                    let before = if rng.below(2) == 0 {
                        rng.pick(&s.children[s.ix(p)].clone())
                    } else {
                        None
                    };
                    let _ = s.insert(p, c, before, origin);
                }
                11..=12 => {
                    let Some(c) = rng.pick(&live) else { continue };
                    if let Some(p) = s.core[s.ix(c)].parent {
                        let _ = s.remove(p, c, origin);
                    }
                }
                13 => {
                    let Some(x) = rng.pick(&live) else { continue };
                    if js {
                        s.destroy(x, origin).unwrap();
                    } else {
                        let _ = SceneMut { s: &mut s, origin }.destroy(x);
                    }
                }
                14 => {
                    // a native close: the whole subtree, from the host
                    let Some(x) = rng.pick(&live) else { continue };
                    if rng.below(3) == 0 {
                        s.free_subtree(x);
                    }
                }
                15..=16 => {
                    let leaves: Vec<NodeId> = live
                        .iter()
                        .copied()
                        .filter(|&x| matches!(s.w(s.ix(x)), W::Label { .. } | W::Button { .. }))
                        .collect();
                    let Some(x) = rng.pick(&leaves) else { continue };
                    set(
                        &mut s,
                        x,
                        "text",
                        Value::Str("w".repeat(rng.below(12))),
                        origin,
                    );
                }
                17..=18 => {
                    let Some(x) = rng.pick(&live) else { continue };
                    let bit =
                        [StateBits::HOVERED, StateBits::SELECTED, StateBits::INVALID][rng.below(3)];
                    SceneMut { s: &mut s, origin }
                        .node(x)
                        .unwrap()
                        .set_state(bit, rng.below(2) == 0);
                }
                19..=21 => {
                    let Some(x) = rng.pick(&live) else { continue };
                    let mut m = SceneMut { s: &mut s, origin };
                    let mut n = m.node(x).unwrap();
                    match rng.below(7) {
                        0 => {
                            n.ext_mut(&NOTE).set(step);
                        }
                        1 => {
                            n.ext_mut(&NOTE).remove();
                        }
                        2 => {
                            if let Some(v) = n.ext_mut(&NOTE).get_mut() {
                                *v += 1;
                            }
                        }
                        3 => {
                            *n.ext_mut(&PICK).or_insert_with(|| 1) += 1;
                        }
                        4 => {
                            n.ext_mut(&PICK).remove();
                        }
                        5 => {
                            n.ext_mut(&OTHER).set(format!("{step}"));
                        }
                        _ => {
                            n.ext_mut(&OTHER).remove();
                        }
                    }
                }
                22 => {
                    // a batch of writes: one tick, one Batch marker
                    let mut m = SceneMut { s: &mut s, origin };
                    m.batch("sg2-batch", |m| {
                        for &x in live.iter().take(3) {
                            m.node(x).unwrap().set_state(StateBits::CHECKED, true);
                            m.node(x).unwrap().ext_mut(&NOTE).set(7);
                        }
                    });
                }
                _ => {
                    if rng.below(2) == 0 {
                        short = None;
                    }
                }
            }
            s.validate().unwrap_or_else(|e| panic!("{ctx}: {e}"));
            if step % 7 == 6 || step == 399 {
                let set = obs.take();
                assert!(!set.overflowed, "{ctx}");
                records += set.records.len();
                takes += 1;
                for c in &set.records {
                    mirror.apply(&s, c);
                }
                assert_eq!(
                    mirror.dump_all(),
                    scene_dump_all(&s),
                    "{ctx}: the replayed journal rebuilds the dump"
                );
                // the narrow observer saw NOTE records only, enough to track NOTE's marked set (R19)
                for c in narrow.take().records {
                    match c {
                        Change::Ext { node, ext, what } => {
                            assert_eq!(ext, NOTE.id());
                            match what {
                                ExtChange::Added => assert!(note_live.insert(node)),
                                ExtChange::Removed => assert!(note_live.remove(&node)),
                                ExtChange::Changed => assert!(note_live.contains(&node)),
                            }
                        }
                        Change::Batch { .. } => {}
                        other => panic!("{ctx}: the NOTE observer got {other:?}"),
                    }
                }
                let marked: BTreeSet<NodeId> = SceneRef::new(&s)
                    .with_ext(&NOTE)
                    .map(|(n, _)| n.id())
                    .collect();
                assert_eq!(
                    note_live, marked,
                    "{ctx}: NOTE's marked set from the journal"
                );
            }
        }
        drop(short);
    }
    assert!(
        records > 1000 && takes > 100,
        "{records} records in {takes} takes"
    );
}

/// Sparse and dense columns: access, swap-remove, ticks, iteration, freeing with the node.
#[test]
fn ext_columns_sparse_and_dense() {
    let h = SceneHandle::default();
    let app = ProviderId::APP;
    let ids: Vec<NodeId> = h
        .write(app, |s| {
            (0..10).map(|_| s.create_tag("box").unwrap()).collect()
        })
        .unwrap();
    let t0 = h.read(|s| s.tick());
    h.write(app, |s| {
        for (k, &id) in ids.iter().enumerate() {
            let mut n = s.node(id).unwrap();
            if k % 2 == 0 {
                assert_eq!(n.ext_mut(&NOTE).set(k as u32), None);
            }
            n.ext_mut(&PICK).set(k as u8);
        }
    })
    .unwrap();
    let t1 = h.read(|s| s.tick());
    h.write(app, |s| {
        // swap-remove from the middle of the packed column
        assert_eq!(s.node(ids[2]).unwrap().ext_mut(&NOTE).remove(), Some(2));
        assert_eq!(s.node(ids[2]).unwrap().ext_mut(&NOTE).remove(), None);
        assert_eq!(s.node(ids[4]).unwrap().ext_mut(&NOTE).set(40), Some(4));
        *s.node(ids[6]).unwrap().ext_mut(&NOTE).get_mut().unwrap() += 1;
        assert_eq!(s.node(ids[1]).unwrap().ext_mut(&NOTE).get_mut(), None);
        s.destroy(ids[8]).unwrap();
    })
    .unwrap();
    h.read(|s| {
        s.validate().unwrap();
        let mut got: Vec<(NodeId, u32)> = s.with_ext(&NOTE).map(|(n, v)| (n.id(), *v)).collect();
        got.sort();
        assert_eq!(got, vec![(ids[0], 0), (ids[4], 40), (ids[6], 7)]);
        assert_eq!(s.ext_len(&NOTE), 3);
        assert_eq!(s.ext_len(&PICK), 9);
        assert_eq!(s.ext_len(&OTHER), 0);
        assert_eq!(s.with_ext(&OTHER).count(), 0);
        let picks: Vec<u8> = s.with_ext(&PICK).map(|(_, v)| *v).collect();
        assert_eq!(picks, vec![0, 1, 2, 3, 4, 5, 6, 7, 9], "dense: slot order");
        let n0 = s.get(ids[0]).unwrap();
        assert_eq!(n0.ext(&NOTE), Some(&0));
        let mut names: Vec<&str> = n0.ext_block().names().collect();
        names.sort();
        assert_eq!(names, ["sg2.note", "sg2.pick"]);
        assert_eq!(s.get(ids[1]).unwrap().ext_block().len(), 1);
        // ticks: added in the first write, changed in the second for 4 and 6 only
        let tk = |i: usize| s.get(ids[i]).unwrap().ext_ticks(&NOTE).unwrap();
        assert!(tk(0).is_added_since(t0) && !tk(0).is_changed_since(t1));
        assert!(!tk(4).is_added_since(t1) && tk(4).is_changed_since(t1));
        assert!(tk(6).is_changed_since(t1));
        let changed: Vec<NodeId> = s
            .with_ext_ticks(&NOTE)
            .filter(|(_, _, t)| t.is_changed_since(t1))
            .map(|(n, _, _)| n.id())
            .collect();
        assert_eq!(changed.len(), 2);
        // node ticks follow ext writes
        assert!(s.get(ids[2]).unwrap().changed_since(t1));
        assert!(!s.get(ids[3]).unwrap().changed_since(t1));
        assert_eq!(
            s.changed_since(t1).count(),
            3,
            "2 (removed), 4 (set), 6 (get_mut); 1's get_mut found nothing"
        );
    });
}

/// R19: removing an entry, and freeing a node that has entries, journal `Ext { Removed }` (before
/// `Destroyed`), so an observer of one key can track its marked set.
#[test]
fn ext_removal_is_journaled_on_remove_and_on_destroy() {
    let h = SceneHandle::default();
    let app = ProviderId::APP;
    let obs = h.observe(ChangeFilter::ext_of(&NOTE));
    let all = h.observe(ChangeFilter::all());
    let (a, b, c) = h
        .write(app, |s| {
            let a = s.create_tag("box").unwrap();
            let b = s.create_tag("label").unwrap();
            let c = s.create_tag("label").unwrap();
            s.insert(a, b, None).unwrap();
            s.insert(a, c, None).unwrap();
            for id in [a, b, c] {
                s.node(id).unwrap().ext_mut(&NOTE).set(1);
            }
            s.node(b).unwrap().ext_mut(&PICK).set(1);
            (a, b, c)
        })
        .unwrap();
    let added: Vec<Change> = obs
        .take()
        .records
        .into_iter()
        .filter(|c| !matches!(c, Change::Batch { .. }))
        .collect();
    assert_eq!(added.len(), 3);
    assert!(added.iter().all(|c| matches!(
        c,
        Change::Ext {
            what: ExtChange::Added,
            ..
        }
    )));
    all.take();
    h.write(app, |s| {
        s.node(c).unwrap().ext_mut(&NOTE).remove();
        s.destroy(a).unwrap();
    })
    .unwrap();
    let removed: Vec<NodeId> = obs
        .take()
        .records
        .into_iter()
        .filter_map(|c| match c {
            Change::Ext {
                node,
                what: ExtChange::Removed,
                ..
            } => Some(node),
            Change::Batch { .. } => None,
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(
        removed,
        vec![c, a, b],
        "the explicit remove, then the subtree's entries in pre-order"
    );
    // in the full journal, each node's ext removals come before its Destroyed
    let recs = all.take().records;
    let pos = |pred: &dyn Fn(&Change) -> bool| recs.iter().position(pred).unwrap();
    let b_pick = pos(
        &|c| matches!(c, Change::Ext { node, ext, what: ExtChange::Removed } if *node == b && *ext == PICK.id()),
    );
    let b_gone = pos(&|c| matches!(c, Change::Destroyed { node, .. } if *node == b));
    assert!(b_pick < b_gone);
    assert!(matches!(recs[0], Change::Batch { label: "write", .. }));
    h.read(|s| assert_eq!(s.ext_len(&NOTE), 0));
}

/// Filters: scope (root and subtree), kinds, props, state bits, origins, ignore.
#[test]
fn observer_filters() {
    let h = SceneHandle::default();
    let app = ProviderId::APP;
    let other = ProviderId::named("sg2-other");
    let (w, col, l1, l2, b1, loose) = h
        .write(app, |s| {
            let w = s.create_tag("window").unwrap();
            let col = s.create_tag("box").unwrap();
            let l1 = s.create_tag("label").unwrap();
            let l2 = s.create_tag("label").unwrap();
            let b1 = s.create_tag("button").unwrap();
            let loose = s.create_tag("label").unwrap();
            s.insert(w, col, None).unwrap();
            for x in [l1, l2, b1] {
                s.insert(col, x, None).unwrap();
            }
            (w, col, l1, l2, b1, loose)
        })
        .unwrap();
    let root = h.read(|s| s.get(w).unwrap().root());
    let text = h.read(|s| s.get(l1).unwrap().prop_id("text").unwrap());
    let in_root = h.observe(ChangeFilter::only(ChangeMask::PROPS).scope(root));
    let in_col = h.observe(ChangeFilter::all().scope(col));
    let labels = h.observe(
        ChangeFilter::only(ChangeMask::PROPS | ChangeMask::STATE)
            .kind(KindId(kinds::kind_id(kind("label")))),
    );
    let texts = h.observe(ChangeFilter::only(ChangeMask::PROPS).prop(text));
    let selected = h.observe(ChangeFilter::only(ChangeMask::STATE).states(StateBits::SELECTED));
    let not_app = h.observe(ChangeFilter::all().ignore(app));
    let only_js = h.observe(ChangeFilter::all().origins(OriginMask::JS));
    h.write(app, |s| {
        s.node(l1)
            .unwrap()
            .set(&keys::label::TEXT, "a".into())
            .unwrap();
        s.node(loose)
            .unwrap()
            .set(&keys::label::TEXT, "b".into())
            .unwrap();
        s.node(b1)
            .unwrap()
            .set(&keys::button::TEXT, "c".into())
            .unwrap();
        s.node(l2)
            .unwrap()
            .set(&keys::widget::VISIBLE, false)
            .unwrap();
        s.node(l2).unwrap().set_state(StateBits::HOVERED, true);
        s.node(b1).unwrap().set_state(StateBits::SELECTED, true);
    })
    .unwrap();
    h.write(other, |s| {
        s.node(w)
            .unwrap()
            .set(&keys::window::TITLE, "t".into())
            .unwrap();
    })
    .unwrap();
    let nodes = |o: &Observer| -> Vec<(NodeId, ChangeMask)> { o.take().by_node().collect() };
    assert_eq!(
        nodes(&in_root),
        vec![
            (l1, ChangeMask::PROPS),
            (b1, ChangeMask::PROPS),
            (l2, ChangeMask::PROPS),
            (w, ChangeMask::PROPS)
        ],
        "the loose label is not in the root"
    );
    assert_eq!(
        nodes(&in_col),
        vec![
            (l1, ChangeMask::PROPS),
            (b1, ChangeMask::PROPS | ChangeMask::STATE),
            (l2, ChangeMask::PROPS | ChangeMask::STATE)
        ],
        "col's subtree, not the window itself"
    );
    assert_eq!(
        nodes(&labels),
        vec![
            (l1, ChangeMask::PROPS),
            (loose, ChangeMask::PROPS),
            (l2, ChangeMask::PROPS | ChangeMask::STATE)
        ]
    );
    assert_eq!(
        nodes(&texts),
        vec![(l1, ChangeMask::PROPS), (loose, ChangeMask::PROPS)],
        "label text only (a button's text is another prop id)"
    );
    assert_eq!(
        nodes(&selected),
        vec![(b1, ChangeMask::STATE)],
        "hovered is not watched"
    );
    assert_eq!(
        nodes(&not_app),
        vec![(w, ChangeMask::PROPS)],
        "the app's own writes are ignored"
    );
    assert!(only_js.take().records.is_empty());
    // a state change records before and after
    h.write(app, |s| {
        s.node(b1).unwrap().set_state(StateBits::SELECTED, false)
    })
    .unwrap();
    let recs = selected.take().records;
    assert!(recs.iter().any(|c| matches!(c, Change::State { node, before, after } if *node == b1 && before.contains(StateBits::SELECTED) && !after.contains(StateBits::SELECTED))));
    // a provider state bit registered with RA1's registry filters the same way
    if let Some(bit) = crate::provider::state_registry().first() {
        let o = h.observe(ChangeFilter::only(ChangeMask::STATE).states(*bit));
        h.write(app, |s| {
            s.node(l1).unwrap().set_state(StateBits::FOCUSED, true);
            s.node(l1).unwrap().set_state(*bit, true);
        })
        .unwrap();
        assert_eq!(nodes(&o), vec![(l1, ChangeMask::STATE)]);
    }
}

/// Batches carry one tick and a marker; an overflowing queue reports it and stops; a dropped
/// observer leaves the journal empty again (no records, no union interest).
#[test]
fn batches_overflow_and_dropped_observers() {
    let h = SceneHandle::default();
    let app = ProviderId::APP;
    let l = h.write(app, |s| s.create_tag("label").unwrap()).unwrap();
    let obs = h.observe(ChangeFilter::only(ChangeMask::PROPS));
    h.write(app, |s| {
        s.node(l)
            .unwrap()
            .set(&keys::label::TEXT, "a".into())
            .unwrap();
        s.batch("inner", |s| {
            s.node(l)
                .unwrap()
                .set(&keys::label::TEXT, "b".into())
                .unwrap()
        });
    })
    .unwrap();
    h.defer(app, move |s| {
        s.node(l)
            .unwrap()
            .set(&keys::label::TEXT, "c".into())
            .unwrap()
    });
    h.set_phase(Phase::Logic);
    h.run_deferred();
    let recs = obs.take().records;
    let t = match recs[0] {
        Change::Batch {
            label: "write",
            tick,
        } => tick,
        ref other => panic!("{other:?}"),
    };
    assert!(
        matches!(recs[1], Change::Prop { .. }) && matches!(recs[2], Change::Prop { .. }),
        "a nested batch joins the write: {recs:?}"
    );
    assert!(
        matches!(recs[3], Change::Batch { label: "defer", tick } if tick > t),
        "{recs:?}"
    );
    assert_eq!(recs.len(), 5);

    // overflow
    let small = h.observe(ChangeFilter::only(ChangeMask::PROPS).capacity(4));
    for k in 0..10 {
        h.write(app, |s| {
            s.node(l)
                .unwrap()
                .set(&keys::label::TEXT, format!("{k}"))
                .unwrap()
        })
        .unwrap();
    }
    let set = small.take();
    assert!(set.overflowed && set.records.is_empty());
    h.write(app, |s| {
        s.node(l)
            .unwrap()
            .set(&keys::label::TEXT, "z".into())
            .unwrap()
    })
    .unwrap();
    let set = small.take();
    assert!(
        !set.overflowed && set.records.len() == 2,
        "recording resumes after a take: {set:?}"
    );

    // dropped observers: the next record prunes them and clears the union interest
    drop(small);
    drop(obs);
    h.write(app, |s| {
        s.node(l)
            .unwrap()
            .set(&keys::label::TEXT, "y".into())
            .unwrap()
    })
    .unwrap();
    h.read(|s| assert_eq!(s.stats().observers, 0));
    assert!(!h.borrow().journal.wants(ChangeMask::ALL));
    // and observe works inside a write
    let inner = h.write(app, |s| {
        let o = s.observe(ChangeFilter::all());
        s.node(l).unwrap().set_state(StateBits::BUSY, true);
        o
    });
    assert_eq!(h.take_changes(&inner.unwrap()).records.len(), 2);
}

/// `Layout` records and `Damage::relayout`: a whole-root layout reports the root's top, a text
/// change the boundary it re-lays out; repaint damage follows drawn rects.
#[test]
fn layout_records_and_damage() {
    let ctx = egui::Context::default();
    let mut out = ctx.run_ui(Default::default(), |_| {});
    out.textures_delta.clear();
    let m = Metrics::new(&ctx);
    let mut s = Scene::default();
    let js = Origin::Js;
    let w = s.create(kind("window"), Owner::Js).unwrap();
    let row = s.create(kind("box"), Owner::Js).unwrap();
    for (k, v) in [("width", 300.0), ("height", 40.0), ("flex-shrink", 0.0)] {
        set(&mut s, row, k, Value::F64(v), js);
    }
    set(&mut s, row, "flex-direction", Value::Str("row".into()), js);
    s.insert(w, row, None, js).unwrap();
    let l = s.create(kind("label"), Owner::Js).unwrap();
    set(&mut s, l, "text", Value::Str("hi".into()), js);
    set(&mut s, l, "wrap-mode", Value::Str("extend".into()), js);
    s.insert(row, l, None, js).unwrap();
    let r = s.core[s.ix(w)].root;
    let obs = s
        .journal
        .observe(ChangeFilter::only(ChangeMask::LAYOUT).scope(r));
    let size = egui::vec2(400.0, 300.0);
    layout::run_root(&mut s, &m, r, size).unwrap();
    let set1 = obs.take();
    let laid: Vec<NodeId> = set1
        .records
        .iter()
        .filter_map(|c| matches!(c, Change::Layout { .. }).then(|| c.node().unwrap()))
        .collect();
    assert_eq!(laid.len(), 3, "{:?}", set1.records);
    assert_eq!(set1.damage.relayout, vec![w], "a whole-root layout");
    // a longer text: the row (a boundary) is laid out alone; only the label's rect changes
    let li = s.ix(l);
    s.core[li].drawn = egui::Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(20.0, 14.0));
    set(
        &mut s,
        l,
        "text",
        Value::Str("a much longer text".into()),
        js,
    );
    layout::remeasure(&mut s, &m, r);
    layout::run_root(&mut s, &m, r, size).unwrap();
    let set2 = obs.take();
    assert_eq!(set2.damage.relayout, vec![row]);
    assert!(
        matches!(set2.records[..], [Change::Layout { node, before, after }] if node == l && after.width() > before.width()),
        "{:?}",
        set2.records
    );
    assert_eq!(
        set2.damage.repaint,
        vec![(egui::ViewportId::ROOT, s.core[s.ix(l)].drawn)]
    );
    assert_eq!(set2.damage.a11y, vec![l]);
}
