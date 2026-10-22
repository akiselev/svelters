//! SG1 acceptance tests that need the scene's internals (ARCH-SCENE §11, §14):
//!
//! - `random_op_interleavings`: runtime-like and Rust op interleavings against a reference model
//!   (a plain tree), with `validate` after every op, owner rules (S3) checked, and a layout oracle:
//!   incremental layout (boundaries, per-root queues) places every node exactly where a from-scratch
//!   layout does.
//! - `label_change_in_11k_root_lays_out_one_boundary`: one label change in an 11k-node root lays
//!   out one boundary and measures one leaf.
//! - phases (R17) and the defer queue (R16).

use super::*;
use crate::layout::{self, Metrics};
use std::collections::HashMap;

fn fonts_ctx() -> egui::Context {
    let ctx = egui::Context::default();
    let mut out = ctx.run_ui(Default::default(), |_| {});
    out.textures_delta.clear();
    ctx
}

fn kind(tag: &str) -> usize {
    kinds::tag_index(tag).unwrap()
}

fn prop(s: &Scene, id: NodeId, name: &str) -> u32 {
    kinds::find_prop(s.core[s.ix(id)].kind as usize, name).unwrap().0
}

fn set(s: &mut Scene, id: NodeId, name: &str, v: svelters_abi::Value, o: Origin) -> Result<(), PropError> {
    let p = prop(s, id, name);
    s.set_value(id, p, &v, o)
}

/// xorshift64*: deterministic, no dependency.
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

#[derive(Clone, Debug)]
struct M {
    parent: Option<NodeId>,
    children: Vec<NodeId>,
    js: bool,
    container: bool,
    window: bool,
}

#[derive(Default)]
struct Model {
    n: HashMap<NodeId, M>,
}

impl Model {
    fn is_ancestor(&self, a: NodeId, mut x: NodeId) -> bool {
        loop {
            if x == a {
                return true;
            }
            match self.n[&x].parent {
                Some(p) => x = p,
                None => return false,
            }
        }
    }
    fn unlink(&mut self, c: NodeId) {
        if let Some(p) = self.n[&c].parent {
            self.n.get_mut(&p).unwrap().children.retain(|&x| x != c);
            self.n.get_mut(&c).unwrap().parent = None;
        }
    }
    fn subtree(&self, top: NodeId) -> Vec<NodeId> {
        let mut out = vec![top];
        let mut i = 0;
        while i < out.len() {
            out.extend(self.n[&out[i]].children.iter().copied());
            i += 1;
        }
        out
    }
}

/// Rects of every node under a root, then the same after dropping every cache and measurement and
/// laying the roots out from scratch: they must agree.
fn layout_oracle(s: &mut Scene, m: &Metrics, size: egui::Vec2, ctx: &str) {
    let roots: Vec<RootIx> = s.roots.iter().map(|(r, _)| r).collect();
    for &r in &roots {
        layout::remeasure(s, m, r);
        layout::run_root(s, m, r, size);
    }
    s.validate().unwrap_or_else(|e| panic!("{ctx}: after layout: {e}"));
    let attached: Vec<NodeId> = s.ids().filter(|&id| !s.core[s.ix(id)].root.is_detached()).collect();
    let incremental: Vec<egui::Rect> = attached.iter().map(|&id| s.core[s.ix(id)].rect).collect();
    for id in s.ids().collect::<Vec<_>>() {
        let i = s.ix(id);
        s.layout[i].cache.clear();
        s.layout[i].intrinsic = None;
    }
    for &r in &roots {
        s.mark_full(r);
        layout::run_root(s, m, r, size);
    }
    for (k, &id) in attached.iter().enumerate() {
        let fresh = s.core[s.ix(id)].rect;
        if fresh != incremental[k] {
            let top = s.roots.get(s.core[s.ix(id)].root).unwrap().top;
            panic!("{ctx}: {id} <{}> incremental {:?} != from scratch {fresh:?}\n{}", s.tag_at(s.ix(id)), incremental[k], s.dump(top, DumpOpts { rects: true, dirty: true }));
        }
    }
}

#[test]
fn random_op_interleavings() {
    let ctx = fonts_ctx();
    let m = Metrics::new(&ctx);
    let rust = Origin::Rust(ProviderId::named("sg1-test"));
    let size = egui::vec2(300.0, 200.0);
    let (mut boundaries, mut layouts) = (0, 0);
    // SG1_SEEDS=n runs more seeds (local stress runs)
    let seeds: u64 = std::env::var("SG1_SEEDS").ok().and_then(|v| v.parse().ok()).unwrap_or(24);
    for seed in 1..=seeds {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let mut s = Scene::default();
        let mut md = Model::default();
        let w = s.create(kind("window"), Owner::Js).unwrap();
        md.n.insert(w, M { parent: None, children: vec![], js: true, container: true, window: true });
        for step in 0..400 {
            let ctx = format!("seed {seed} step {step}");
            let live: Vec<NodeId> = {
                let mut v: Vec<NodeId> = md.n.keys().copied().collect();
                v.sort();
                v
            };
            let origin = if rng.below(3) == 0 { rust } else { Origin::Js };
            let js = matches!(origin, Origin::Js);
            match rng.below(20) {
                // create
                0..=4 => {
                    let tags: &[&str] = if js { &["box", "box", "label", "button", "window"] } else { &["box", "label"] };
                    let tag = rng.pick(tags).unwrap();
                    if tag == "window" && md.n.values().filter(|x| x.window).count() >= 2 {
                        continue;
                    }
                    let owner = if js { Owner::Js } else { Owner::Rust(ProviderId::named("sg1-test")) };
                    let id = s.create(kind(tag), owner).unwrap();
                    if tag == "box" && rng.below(2) == 0 {
                        use svelters_abi::Value::F64;
                        set(&mut s, id, "width", F64(60.0 + rng.below(200) as f64), origin).unwrap();
                        set(&mut s, id, "height", F64(30.0 + rng.below(100) as f64), origin).unwrap();
                        set(&mut s, id, "flex-shrink", F64(0.0), origin).unwrap();
                    }
                    md.n.insert(id, M { parent: None, children: vec![], js, container: tag == "box" || tag == "window", window: tag == "window" });
                }
                // insert
                5..=10 => {
                    // half the time under an attached container, so roots and boundaries see work
                    let attached: Vec<NodeId> = live.iter().copied().filter(|&x| md.n[&x].container && !s.core[s.ix(x)].root.is_detached()).collect();
                    let p = if rng.below(2) == 0 { rng.pick(&attached) } else { rng.pick(&live) };
                    let (Some(p), Some(c)) = (p, rng.pick(&live)) else { continue };
                    let before = if rng.below(2) == 0 { rng.pick(&md.n[&p].children.clone()) } else { None };
                    let (pm, cm) = (md.n[&p].clone(), md.n[&c].clone());
                    let owner_ok = js || (!pm.js && !cm.js && cm.parent.is_none_or(|op| !md.n[&op].js));
                    let want = if !pm.container || cm.window || p == c || !owner_ok || md.is_ancestor(c, p) {
                        Err(())
                    } else if before == Some(c) {
                        Ok(false)
                    } else {
                        Ok(true)
                    };
                    let got = s.insert(p, c, before, origin);
                    assert_eq!(got.as_ref().map_err(|_| ()).copied(), want, "{ctx}: insert {c} into {p} before {before:?} by {origin:?}: {got:?}");
                    if want == Ok(true) {
                        md.unlink(c);
                        let kids = &mut md.n.get_mut(&p).unwrap().children;
                        let at = before.and_then(|b| kids.iter().position(|&x| x == b)).unwrap_or(kids.len());
                        kids.insert(at, c);
                        md.n.get_mut(&c).unwrap().parent = Some(p);
                    }
                }
                // remove
                11..=12 => {
                    let Some(c) = rng.pick(&live) else { continue };
                    let Some(p) = md.n[&c].parent else { continue };
                    let want_ok = js || (!md.n[&p].js && !md.n[&c].js);
                    let got = s.remove(p, c, origin);
                    assert_eq!(got.is_ok(), want_ok, "{ctx}: remove {c} from {p} by {origin:?}: {got:?}");
                    if want_ok {
                        md.unlink(c);
                    }
                }
                // destroy: a runtime destroys one node (children orphaned), Rust a whole subtree
                13..=14 => {
                    let Some(x) = rng.pick(&live) else { continue };
                    if js {
                        s.destroy(x, origin).unwrap();
                        md.unlink(x);
                        for c in md.n[&x].children.clone() {
                            md.n.get_mut(&c).unwrap().parent = None;
                        }
                        md.n.remove(&x);
                    } else {
                        let sub = md.subtree(x);
                        let want_ok = sub.iter().all(|y| !md.n[y].js) && md.n[&x].parent.is_none_or(|p| !md.n[&p].js);
                        let got = SceneMut { s: &mut s, origin }.destroy(x);
                        assert_eq!(got.is_ok(), want_ok, "{ctx}: rust destroy {x}: {got:?}");
                        if want_ok {
                            md.unlink(x);
                            for y in sub {
                                md.n.remove(&y);
                            }
                        }
                    }
                }
                // text: a different length each time
                15..=16 => {
                    let leaves: Vec<NodeId> = live.iter().copied().filter(|x| !md.n[x].container).collect();
                    let Some(x) = rng.pick(&leaves) else { continue };
                    let t = "w".repeat(rng.below(30));
                    set(&mut s, x, "text", svelters_abi::Value::Str(t), origin).unwrap();
                }
                // layout props: definite sizes make boundaries
                17..=18 => {
                    let boxes: Vec<NodeId> = live.iter().copied().filter(|x| md.n[x].container && !md.n[x].window).collect();
                    let Some(x) = rng.pick(&boxes) else { continue };
                    use svelters_abi::Value::{F64, Null, Str};
                    let (name, v) = match rng.below(6) {
                        0 => ("width", if rng.below(3) == 0 { Null } else { F64(40.0 + rng.below(100) as f64) }),
                        1 => ("height", if rng.below(3) == 0 { Null } else { F64(20.0 + rng.below(60) as f64) }),
                        2 => ("flex-shrink", F64(rng.below(2) as f64)),
                        3 => ("flex-direction", Str(["row", "column"][rng.below(2)].into())),
                        4 => ("gap", F64(rng.below(9) as f64)),
                        _ => ("padding", F64(rng.below(5) as f64)),
                    };
                    set(&mut s, x, name, v, origin).unwrap();
                }
                // visibility
                _ => {
                    let nodes: Vec<NodeId> = live.iter().copied().filter(|x| !md.n[x].window).collect();
                    let Some(x) = rng.pick(&nodes) else { continue };
                    set(&mut s, x, "visible", svelters_abi::Value::Bool(rng.below(4) != 0), origin).unwrap();
                }
            }
            s.validate().unwrap_or_else(|e| panic!("{ctx}: {e}"));
            // the scene matches the model
            assert_eq!(s.len(), md.n.len(), "{ctx}: live count");
            for (&id, mn) in &md.n {
                let i = s.idx(id).unwrap_or_else(|| panic!("{ctx}: {id} not live"));
                assert_eq!(s.core[i].parent, mn.parent, "{ctx}: parent of {id}");
                assert_eq!(s.children[i], mn.children, "{ctx}: children of {id}");
            }
            if step % 16 == 15 {
                layout_oracle(&mut s, &m, size, &ctx);
            }
        }
        layout_oracle(&mut s, &m, size, &format!("seed {seed} end"));
        boundaries += s.counters.boundaries_laid_out;
        layouts += s.counters.layouts;
    }
    // the interleavings did exercise boundary relayouts, not only whole-root ones
    assert!(boundaries > 20 && layouts > 20, "{boundaries} boundary relayouts, {layouts} root layouts");
    eprintln!("{seeds} seeds: {boundaries} boundary relayouts, {layouts} root layouts");
}

/// ARCH-SCENE §14 SG1: one label change in an 11k-node root lays out ≤ 1 boundary and measures
/// 1 leaf (and no whole-root layout).
#[test]
fn label_change_in_11k_root_lays_out_one_boundary() {
    use svelters_abi::Value::{F64, Str};
    let ctx = fonts_ctx();
    let m = Metrics::new(&ctx);
    let mut s = Scene::default();
    let js = Origin::Js;
    let w = s.create(kind("window"), Owner::Js).unwrap();
    let col = s.create(kind("box"), Owner::Js).unwrap();
    s.insert(w, col, None, js).unwrap();
    let mut labels = Vec::new();
    for r in 0..1000 {
        let row = s.create(kind("box"), Owner::Js).unwrap();
        set(&mut s, row, "flex-direction", Str("row".into()), js).unwrap();
        set(&mut s, row, "width", F64(600.0), js).unwrap();
        set(&mut s, row, "height", F64(20.0), js).unwrap();
        set(&mut s, row, "flex-shrink", F64(0.0), js).unwrap();
        s.insert(col, row, None, js).unwrap();
        for k in 0..10 {
            let l = s.create(kind("label"), Owner::Js).unwrap();
            set(&mut s, l, "text", Str(format!("{r}.{k}")), js).unwrap();
            // no wrapping: a wrapping label's min-content query shapes its text again (debug-slow)
            set(&mut s, l, "wrap-mode", Str("extend".into()), js).unwrap();
            s.insert(row, l, None, js).unwrap();
            labels.push(l);
        }
    }
    assert_eq!(s.len(), 11_002);
    let r = s.core[s.ix(w)].root;
    let size = egui::vec2(800.0, 600.0);
    let run = layout::run_root(&mut s, &m, r, size).unwrap();
    assert!(run.full);
    assert_eq!(layout::run_root(&mut s, &m, r, size), None, "nothing queued: no layout");
    let before = s.counters;
    let target = labels[5_432];
    set(&mut s, target, "text", Str("a much longer label".into()), js).unwrap();
    layout::remeasure(&mut s, &m, r);
    let run = layout::run_root(&mut s, &m, r, size).unwrap();
    let after = s.counters;
    assert!(!run.full, "{run:?}");
    assert_eq!(after.layouts - before.layouts, 0, "no whole-root layout");
    assert!(after.boundaries_laid_out - before.boundaries_laid_out <= 1, "{run:?}");
    assert_eq!(after.boundaries_laid_out - before.boundaries_laid_out, 1);
    assert_eq!(after.leaves_measured - before.leaves_measured, 1, "one leaf measured");
    assert!(after.nodes_placed - before.nodes_placed <= 11, "only the row is placed: {}", after.nodes_placed - before.nodes_placed);
    s.validate().unwrap();
}

/// R17: `write` is refused in the `Ui` phase and `defer` runs at the next logic; R16: the defer
/// queue is visible as pending work.
#[test]
fn phases_refuse_writes_in_ui_and_defer_runs_next_logic() {
    let h = SceneHandle::default();
    let app = ProviderId::APP;
    let b = h.write(app, |s| s.create_tag("box").unwrap()).unwrap();
    h.set_phase(Phase::Ui);
    assert_eq!(h.write(app, |_| ()), Err(SceneBusy));
    h.defer(app, move |s| {
        let l = s.create_tag("label").unwrap();
        s.insert(b, l, None).unwrap();
    });
    assert_eq!(h.deferred_len(), 1);
    h.set_phase(Phase::Logic);
    assert_eq!(h.run_deferred(), 1);
    assert_eq!(h.deferred_len(), 0);
    h.read(|s| {
        assert_eq!(s.get(b).unwrap().children().count(), 1);
        assert_eq!(s.stats().writes_busy, 1);
        assert_eq!(s.stats().deferred_run, 1);
        s.validate().unwrap();
    });
    // a write inside a write is refused, not a panic
    h.write(app, |_| assert_eq!(h.write(app, |_| ()), Err(SceneBusy))).unwrap();
}

/// Ticks: one per batch, nodes remember theirs.
#[test]
fn ticks_and_typed_reads() {
    let h = SceneHandle::default();
    let app = ProviderId::APP;
    let (b, l) = h
        .write(app, |s| {
            let b = s.create_tag("box").unwrap();
            let l = s.create_tag("label").unwrap();
            s.insert(b, l, None).unwrap();
            (b, l)
        })
        .unwrap();
    let t = h.read(|s| s.tick());
    h.write(app, |s| s.node(l).unwrap().set(&keys::label::TEXT, "hi".to_string()).unwrap()).unwrap();
    h.read(|s| {
        let n = s.get(l).unwrap();
        assert!(n.changed_since(t));
        assert!(!s.get(b).unwrap().changed_since(t));
        assert_eq!(n.get(&keys::label::TEXT).map(String::as_str), Some("hi"));
        assert_eq!(n.get(&keys::widget::VISIBLE), Some(&true));
        assert_eq!(n.get(&keys::button::TEXT), None, "not a button");
        assert_eq!(n.owner(), Owner::Rust(app));
        assert_eq!(n.parent().map(|p| p.id()), Some(b));
    });
}

#[test]
fn every_builtin_key_names_a_prop_of_its_kind() {
    let check = |tag: &str, names: &[&str]| {
        for n in names {
            assert!(kinds::find_prop(kind(tag), n).is_some(), "<{tag}> {n}");
        }
    };
    use keys::*;
    check("label", &[label::TEXT.name, label::WRAP_MODE.name, label::SELECTABLE.name, widget::ACCESSIBLE_NAME.name, widget::MARGIN.name]);
    check("button", &[button::TEXT.name, button::SELECTED.name, button::FRAME.name, button::SMALL.name]);
    check("checkbox", &[checkbox::TEXT.name, checkbox::CHECKED.name]);
    check("text-edit", &[text_edit::TEXT.name, text_edit::HINT_TEXT.name, text_edit::MULTILINE.name, text_edit::PASSWORD.name, text_edit::CHAR_LIMIT.name]);
    check("slider", &[slider::VALUE.name, slider::MIN.name, slider::MAX.name, slider::STEP.name, slider::TEXT.name, slider::SHOW_VALUE.name]);
    check("window", &[window::TITLE.name, window::DEFAULT_WIDTH.name, window::DEFAULT_HEIGHT.name]);
    check("box", &[container::FLEX_DIRECTION.name, container::FLEX_WRAP.name, container::GAP.name, container::ROW_GAP.name, container::COLUMN_GAP.name, container::PADDING.name, container::ALIGN_ITEMS.name, container::JUSTIFY_CONTENT.name, widget::WIDTH.name]);
}
