//! SG6 (ARCH-SCENE §8.5, §14): scene memory and paint cost of large screens, in the shape of
//! cadatomic's PropertyGrid (SC1): a scroll area of collapsing groups, each with rows of
//! `label` + editor (`drag-value`, `text-edit`, `checkbox`, `combo-box`).
//!
//! - `scene_memory`: 100k nodes stay within the budget of 640 B a node (`SceneStats::bytes`).
//! - `frame_time_10k`: the steady and scrolling frame of a 10k-node grid; the times are printed
//!   (`--nocapture`, recorded in `plans/milestones/SG6-scene-memory-culling.md`), the asserts are
//!   on the walk's node counts, which do not depend on the machine.

mod common;

use common::Rig;
use std::time::Instant;
use svelters_abi::*;

const ROWS_PER_GROUP: usize = 20;

/// A PropertyGrid of about `nodes` nodes in a scroll area filling the window; returns the
/// scroll area and the first row's label.
fn grid(rig: &Rig, nodes: usize) -> (svl_handle, svl_handle) {
    let (_w, col) = rig.window();
    let sa = rig.leaf(col, "scroll-area", &[("flex-grow", Value::F64(1.0))]);
    let body = rig.leaf(sa, "box", &[("gap", Value::F64(2.0))]);
    let per_group = 1 + ROWS_PER_GROUP * 3;
    let mut first = None;
    for g in 0..nodes.div_ceil(per_group) {
        let grp = rig.leaf(body, "collapsing-header", &[("text", Value::Str(format!("Group {g}"))), ("open", Value::Bool(true))]);
        for r in 0..ROWS_PER_GROUP {
            let row = rig.leaf(grp, "box", &[("flex-direction", Value::Str("row".into())), ("gap", Value::F64(4.0))]);
            let l = rig.leaf(row, "label", &[("text", Value::Str(format!("Property {r}"))), ("width", Value::F64(120.0))]);
            first.get_or_insert(l);
            let name = ("accessible-name", Value::Str(format!("Property {r}")));
            match r % 4 {
                0 => rig.leaf(row, "drag-value", &[("value", Value::F64(r as f64)), name]),
                1 => rig.leaf(row, "text-edit", &[("text", Value::Str("value".into())), name]),
                2 => rig.leaf(row, "checkbox", &[("text", Value::Str("on".into()))]),
                _ => rig.leaf(row, "combo-box", &[("options", Value::List(vec![Value::Str("a".into()), Value::Str("b".into())])), ("selected", Value::F64(0.0)), name]),
            };
        }
    }
    (sa, first.expect("a row"))
}

#[test]
fn scene_memory() {
    let mut rig = Rig::new();
    grid(&rig, 100_000);
    rig.run();
    let st = rig.backend.scene().read(|s| s.stats());
    let per_node = st.bytes as f64 / st.live as f64;
    let by: Vec<String> = st.bytes_by.iter().map(|(k, b)| format!("{k} {:.0}", *b as f64 / st.live as f64)).collect();
    println!("scene_memory: {} nodes, {:.1} MB, {per_node:.0} B/node ({})", st.live, st.bytes as f64 / 1e6, by.join(", "));
    assert!(st.live >= 100_000);
    assert!(per_node <= 640.0, "{per_node:.0} B/node over the 640 B budget (ARCH-SCENE §8.5)");
}

#[test]
fn frame_time_10k() {
    let mut rig = Rig::new();
    let (sa, label) = grid(&rig, 10_000);
    let t = Instant::now();
    rig.run();
    let first = t.elapsed().as_secs_f64() * 1e3;
    let live = rig.backend.scene().read(|s| s.stats().live);
    let time = |rig: &mut Rig, frames: usize, scroll: bool| {
        let mut y = 0.0;
        let t = Instant::now();
        for _ in 0..frames {
            if scroll {
                y += 40.0;
                rig.set(sa, "scroll-y", Value::F64(y));
            }
            rig.harness.step();
        }
        t.elapsed().as_secs_f64() * 1e3 / frames as f64
    };
    time(&mut rig, 5, false);
    let steady = time(&mut rig, 200, false);
    let scroll = time(&mut rig, 200, true);
    // a label's text changes: the window relays out (no boundary below it)
    let leaves0 = rig.backend.scene().read(|s| s.stats().leaf_layouts);
    let t = Instant::now();
    for k in 0..20 {
        rig.set(label, "text", Value::Str(if k % 2 == 0 { "A longer property name".into() } else { "Property 0".into() }));
        rig.harness.step();
    }
    let relayout = t.elapsed().as_secs_f64() * 1e3 / 20.0;
    let st = rig.backend.scene().read(|s| s.stats());
    let leaves = (st.leaf_layouts - leaves0) / 20;
    println!(
        "frame_time_10k: {live} nodes, first run {first:.1} ms, steady {steady:.3} ms/frame, scrolling {scroll:.3} ms/frame, relayout {relayout:.2} ms/frame ({leaves} leaf layouts), {st:?}"
    );
}
