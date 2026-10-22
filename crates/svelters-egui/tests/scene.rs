//! The scene's public API from Rust (ARCH-SCENE §6, SG1), next to a runtime driving the same
//! scene through host ops (the recording host of `common`, headless kittest).

mod common;

use common::Rig;
use svelters_abi::*;
use svelters_egui::scene::{ChangeFilter, ChangeMask, DumpOpts, ExtKey, NodeId, Owner, ProviderId, SceneError, SceneHandle, StateBits, keys};

fn id(h: svl_handle) -> NodeId {
    NodeId::from_raw(h).unwrap()
}

/// R18: a Rust write of an observed (bound) prop is announced once with `prop_changed`, at the
/// next logic, and never fires the kind's events; an unobserved one is not announced.
#[test]
fn rust_write_of_a_bound_prop_announces_and_fires_no_event() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let cb = rig.leaf(col, "checkbox", &[("text", Value::Str("Snap".into()))]);
    let slider = rig.leaf(col, "slider", &[("accessible-name", Value::Str("Gain".into()))]);
    rig.observe(cb, "checked", true);
    rig.listen(cb, "changed");
    rig.listen(slider, "changed");
    rig.run();
    let checkpoints = rig.log.borrow().checkpoints;
    rig.log.borrow_mut().changed.clear();

    let app = ProviderId::named("cad.test");
    rig.backend
        .scene()
        .write(app, |s| {
            s.node(id(cb)).unwrap().set(&keys::checkbox::CHECKED, true).unwrap();
            s.node(id(slider)).unwrap().set(&keys::slider::VALUE, 42.0).unwrap();
            // the same value again: nothing to announce
            s.node(id(cb)).unwrap().set(&keys::checkbox::CHECKED, true).unwrap();
        })
        .unwrap();
    rig.run();
    assert_eq!(rig.get(cb, "checked"), Value::Bool(true));
    assert_eq!(rig.get(slider, "value"), Value::F64(42.0));
    let l = rig.log.borrow();
    let pid = rig.prop_id(cb, "checked");
    assert_eq!(l.changed, vec![(cb, pid)], "announced once, the unobserved slider not at all");
    assert!(l.events.is_empty(), "no `changed` event for a Rust write: {:?}", l.events);
    assert!(l.checkpoints > checkpoints, "the announcement runs a checkpoint");
}

/// Owners (S3): Rust builds and edits its own subtrees, mounts them under a runtime's node only
/// through a Rust-owned parent, and cannot change a runtime's children.
#[test]
fn rust_edits_its_own_structure_only() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let js_label = rig.leaf(col, "label", &[("text", Value::Str("js".into()))]);
    let h = rig.backend.scene();
    let app = ProviderId::APP;
    let (panel, row) = h
        .write(app, |s| {
            let panel = s.create_tag("box").unwrap();
            let row = s.create_tag("label").unwrap();
            s.insert(panel, row, None).unwrap();
            assert!(matches!(s.insert(id(col), panel, None), Err(SceneError::NotOwner { owner: Owner::Js, .. })), "no Rust edits of a runtime's children");
            assert!(matches!(s.remove(id(js_label)), Err(SceneError::NotOwner { .. })));
            assert!(matches!(s.destroy(id(js_label)), Err(SceneError::NotOwner { .. })));
            (panel, row)
        })
        .unwrap();
    // a runtime may mount Rust's panel (a host op), then Rust keeps editing inside it
    rig.br().insert(col, panel.to_raw(), None, 0).unwrap();
    h.write(app, |s| {
        let extra = s.create_tag("button").unwrap();
        s.node(extra).unwrap().set(&keys::button::TEXT, "Rust".to_string()).unwrap();
        s.insert(panel, extra, Some(row)).unwrap();
    })
    .unwrap();
    rig.run();
    h.read(|s| {
        s.validate().unwrap();
        let p = s.get(panel).unwrap();
        assert_eq!(p.owner(), Owner::Rust(app));
        assert_eq!(p.parent().map(|n| n.id()), Some(id(col)));
        assert_eq!(p.children().map(|c| c.tag()).collect::<Vec<_>>(), ["button", "label"]);
        assert_eq!(p.root(), s.get(id(col)).unwrap().root());
        assert!(p.children().next().unwrap().drawn().is_some(), "drawn in the window");
        let dump = s.dump(id(col), DumpOpts::default());
        assert!(dump.contains("<button>") && dump.contains("[rust:app]") && dump.contains("\"Rust\""), "{dump}");
    });
    // mounted under a runtime's node, it is the runtime's child: Rust cannot take it out
    h.write(app, |s| assert!(matches!(s.destroy(panel), Err(SceneError::NotOwner { owner: Owner::Js, .. })))).unwrap();
    // once the runtime removed it, Rust destroys its subtree; the runtime's nodes stay
    rig.br().remove(col, panel.to_raw()).unwrap();
    h.write(app, |s| s.destroy(panel).unwrap()).unwrap();
    rig.run();
    h.read(|s| {
        assert!(s.get(panel).is_none() && s.get(row).is_none());
        assert_eq!(s.get(id(col)).unwrap().children().count(), 1);
        s.validate().unwrap();
    });
}

/// `defer` (callable anywhere on the UI thread) runs at the start of the next frame's logic, so
/// the runtime sees the write in that frame's checkpoint.
#[test]
fn deferred_writes_run_at_the_next_logic() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let l = rig.leaf(col, "label", &[("text", Value::Str("before".into()))]);
    rig.run();
    let h = rig.backend.scene();
    h.defer(ProviderId::APP, move |s| s.node(id(l)).unwrap().set(&keys::label::TEXT, "after".to_string()).unwrap());
    assert_eq!(h.deferred_len(), 1, "R16: pending work");
    assert_eq!(rig.get(l, "text"), Value::Str("before".into()));
    rig.run();
    assert_eq!(h.deferred_len(), 0);
    assert_eq!(rig.get(l, "text"), Value::Str("after".into()));
}

/// A label change inside a fixed-size row re-lays out that row only, and a resize re-lays out the
/// whole root (counters through `SceneRef::stats`).
#[test]
fn a_label_change_relays_out_its_boundary_only() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let mut labels = Vec::new();
    for r in 0..20 {
        let row = rig.create("box");
        for (k, v) in [("flex-direction", Value::Str("row".into())), ("width", Value::F64(400.0)), ("height", Value::F64(18.0)), ("flex-shrink", Value::F64(0.0))] {
            rig.set(row, k, v);
        }
        rig.insert(col, row);
        for k in 0..5 {
            labels.push(rig.leaf(row, "label", &[("text", Value::Str(format!("{r}.{k}")))]));
        }
    }
    rig.run();
    let h = rig.backend.scene();
    let before = h.read(|s| s.stats());
    rig.set(labels[37], "text", Value::Str("a longer label than before".into()));
    rig.run();
    let after = h.read(|s| s.stats());
    assert_eq!(after.layouts, before.layouts, "no whole-root layout");
    assert_eq!(after.boundaries_laid_out - before.boundaries_laid_out, 1);
    assert_eq!(after.leaves_measured - before.leaves_measured, 1);
    h.read(|s| {
        s.validate().unwrap();
        let n = s.get(id(labels[37])).unwrap();
        assert!(n.parent().unwrap().is_layout_boundary());
        assert!(n.rect().width() > 100.0, "laid out at its new width: {:?}", n.rect());
        assert_eq!(n.get(&keys::label::TEXT).unwrap(), "a longer label than before");
    });
    // bounds (a synchronous layout) agrees with the frame's
    let b = rig.br().bounds(labels[37]).unwrap().unwrap();
    let r = h.read(|s| s.get(id(labels[37])).unwrap().rect());
    assert_eq!([b[0] as f32, b[1] as f32, b[2] as f32, b[3] as f32], [r.min.x, r.min.y, r.width(), r.height()]);
}

// ---- SG2: allocation and `with_ext` budgets ----

/// Counts this thread's allocations (and reallocations), so a test can assert a code path makes
/// none.
struct Counting;

thread_local! {
    static ALLOCS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

fn count() {
    let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
}

fn allocs() -> u64 {
    ALLOCS.with(|c| c.get())
}

// SAFETY: forwards every call to the system allocator unchanged.
unsafe impl std::alloc::GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: std::alloc::Layout) -> *mut u8 {
        count();
        unsafe { std::alloc::System.alloc(l) }
    }
    unsafe fn alloc_zeroed(&self, l: std::alloc::Layout) -> *mut u8 {
        count();
        unsafe { std::alloc::System.alloc_zeroed(l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: std::alloc::Layout, n: usize) -> *mut u8 {
        count();
        unsafe { std::alloc::System.realloc(p, l, n) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: std::alloc::Layout) {
        unsafe { std::alloc::System.dealloc(p, l) }
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

static MARK: ExtKey<u32> = ExtKey::new("sg2.test.mark");
static HEAT: ExtKey<f32> = ExtKey::dense("sg2.test.heat");

/// ARCH-SCENE §14 SG2: no allocation per mutation when nobody observes (a counting allocator).
/// Covers prop writes (number, bool, a common prop), state bits, extension entries (set, change,
/// remove and re-add in sparse and dense columns), a reorder among siblings and a batch. The first
/// rounds warm the vectors up; the next ones must allocate nothing. Also with an observer that
/// wants none of it, and after the last observer is dropped.
#[test]
fn mutations_allocate_nothing_without_observers() {
    let h = SceneHandle::default();
    let app = ProviderId::APP;
    let (top, kids) = h
        .write(app, |s| {
            let top = s.create_tag("box").unwrap();
            let mut kids = Vec::new();
            for tag in ["slider", "checkbox", "label", "box"] {
                let k = s.create_tag(tag).unwrap();
                s.insert(top, k, None).unwrap();
                kids.push(k);
            }
            (top, kids)
        })
        .unwrap();
    let round = |k: u32| {
        h.write(app, |s| {
            s.node(kids[0]).unwrap().set(&keys::slider::VALUE, k as f64).unwrap();
            s.node(kids[1]).unwrap().set(&keys::checkbox::CHECKED, k % 2 == 0).unwrap();
            s.node(kids[2]).unwrap().set(&keys::widget::ENABLED, k % 2 == 0).unwrap();
            s.node(kids[3]).unwrap().set_state(StateBits::SELECTED, k % 2 == 0);
            for &x in &kids {
                let mut n = s.node(x).unwrap();
                n.ext_mut(&MARK).set(k);
                *n.ext_mut(&MARK).get_mut().unwrap() += 1;
                n.ext_mut(&HEAT).set(k as f32);
            }
            s.node(kids[1]).unwrap().ext_mut(&MARK).remove();
            s.node(kids[1]).unwrap().ext_mut(&HEAT).remove();
            // move the last child to the front, and back
            s.insert(top, kids[3], Some(kids[0])).unwrap();
            s.insert(top, kids[3], None).unwrap();
            s.batch("b", |s| s.node(kids[2]).unwrap().set_state(StateBits::HOVERED, k % 2 == 1));
        })
        .unwrap();
    };
    for k in 0..3 {
        round(k); // warm-up
    }
    let a = allocs();
    for k in 3..53 {
        round(k);
    }
    assert_eq!(allocs() - a, 0, "50 rounds of mutations with no observer allocated");

    // an observer that wants none of these kinds of change costs nothing either
    let roots = h.observe(ChangeFilter::only(ChangeMask::ROOT));
    let a = allocs();
    for k in 53..63 {
        round(k);
    }
    assert_eq!(allocs() - a, 0, "with an uninterested observer");
    // an interested one does record (and so allocates its queue)
    let all = h.observe(ChangeFilter::all());
    round(63);
    assert!(all.pending() > 10, "{}", all.pending());
    drop(all);
    drop(roots);
    round(64); // prunes the dropped observers
    let a = allocs();
    for k in 65..75 {
        round(k);
    }
    assert_eq!(allocs() - a, 0, "after the observers are dropped");
    h.read(|s| s.validate().unwrap());
}

/// ARCH-SCENE §14 SG2: `with_ext` over 5k entries among 100k nodes takes ≤ 50 µs (the best of 20
/// runs in a release build: `cargo nextest run --release -p svelters-egui --test scene with_ext`).
/// CI runs debug builds: there the budget is 4 ms, and the walk must beat a scan of every node 5×.
#[test]
fn with_ext_over_5k_of_100k_nodes() {
    let h = SceneHandle::default();
    let app = ProviderId::APP;
    h.write(app, |s| {
        for k in 0..100_000u32 {
            let n = s.create_tag("box").unwrap();
            if k % 20 == 0 {
                s.node(n).unwrap().ext_mut(&MARK).set(k);
            }
        }
    })
    .unwrap();
    let (t, sum) = h.read(|s| {
        let mut best = std::time::Duration::MAX;
        let mut sum = 0u64;
        for _ in 0..20 {
            let t = std::time::Instant::now();
            sum = std::hint::black_box(s.with_ext(&MARK).map(|(n, v)| n.id().to_raw() ^ *v as u64).fold(0u64, |a, x| a.wrapping_add(x)));
            best = best.min(t.elapsed());
        }
        (best, sum)
    });
    assert_eq!(h.read(|s| s.with_ext(&MARK).count()), 5_000);
    assert_eq!(h.read(|s| s.len()), 100_000);
    assert!(sum != 0);
    // O(entries), not O(nodes): far cheaper than looking at every node
    let scan = h.read(|s| {
        let t = std::time::Instant::now();
        std::hint::black_box(s.nodes().filter(|n| n.ext(&MARK).is_some()).count());
        t.elapsed()
    });
    let budget = std::time::Duration::from_micros(if cfg!(debug_assertions) { 4_000 } else { 50 });
    eprintln!("with_ext: 5000 entries of 100000 nodes in {t:?} (budget {budget:?}); a scan of every node: {scan:?}");
    assert!(t <= budget, "with_ext over 5k entries took {t:?} (budget {budget:?})");
    assert!(t * 5 < scan, "with_ext {t:?} vs a full scan {scan:?}");
}
