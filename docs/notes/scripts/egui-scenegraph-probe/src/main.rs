//! egui scene-graph probe for docs/notes/egui-scene-graph.md.
//!
//! cargo run --release -- bench     frame cost of walking N retained nodes (native / culled / taffy, ± AccessKit)
//! cargo run --release -- layout    taffy two-pass layout timings (cold, clean, one leaf dirty, resize)
//! cargo run --release -- checks    id stability (reorder, hot reload), deferred events, in-frame mutation, kittest
//! cargo run --release              all of the above

mod scene;
mod tree;

use egui::{Context, Event, Modifiers, PointerButton, Pos2, RawInput, Rect, vec2};
use scene::{IdMode, LayoutMode, Scene};
use std::time::Instant;
use tree::{Arena, Handle, Kind};

const W: f32 = 1280.0;
const H: f32 = 800.0;

fn input(frame: u64, events: Vec<Event>) -> RawInput {
    RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(W, H))),
        time: Some(frame as f64 / 60.0),
        events,
        ..Default::default()
    }
}

struct FrameStat {
    run_us: f64,
    tess_us: f64,
    passes: usize,
}

/// One frame: `run_ui` (all passes) + tessellation, as an integration would do minus the GPU.
fn frame(ctx: &Context, scene: &mut Scene, n: u64, events: Vec<Event>) -> FrameStat {
    let t0 = Instant::now();
    let mut out = ctx.run_ui(input(n, events), |ui| scene.ui(ui));
    let t1 = Instant::now();
    let shapes = std::mem::take(&mut out.shapes);
    let _prims = ctx.tessellate(shapes, out.pixels_per_point);
    let t2 = Instant::now();
    out.textures_delta.clear();
    FrameStat {
        run_us: (t1 - t0).as_secs_f64() * 1e6,
        tess_us: (t2 - t1).as_secs_f64() * 1e6,
        passes: out.platform_output.num_completed_passes,
    }
}

/// `rows` rows of 10 leaves each (+1 row node), under one root column: 11 * rows + 1 nodes.
/// Identities depend only on (row key, column), never on handles.
fn build(rows: usize) -> (Arena, Handle) {
    let mut a = Arena::default();
    let root = a.create(Kind::Column, 1);
    for r in 0..rows {
        let row = a.create(Kind::Row, hash(&("row", r)));
        a.insert(root, row, None).unwrap();
        let leaves: [(Kind, String); 10] = [
            (Kind::Label, format!("row {r}")),
            (Kind::Label, "lorem ipsum dolor".into()),
            (Kind::Button, "Edit".into()),
            (Kind::Checkbox, "done".into()),
            (Kind::Label, format!("{} items", r * 7 % 113)),
            (Kind::TextEdit, format!("value {r}")),
            (Kind::Label, "status: ok".into()),
            (Kind::Button, "Delete".into()),
            (Kind::Label, format!("#{r:05}")),
            (Kind::Label, "tail".into()),
        ];
        for (c, (k, t)) in leaves.into_iter().enumerate() {
            let h = a.create(k, hash(&("cell", r, c)));
            a.n_mut(h).text = t;
            a.n_mut(h).listen_clicked = k == Kind::Button;
            a.n_mut(h).observe_text = k == Kind::TextEdit;
            a.insert(row, h, None).unwrap();
        }
    }
    (a, root)
}

fn hash<T: std::hash::Hash>(t: &T) -> u64 {
    use std::hash::Hasher;
    // deterministic FNV-1a, so identities are the same across runs (hot reload)
    let mut f = FixedHasher(0xcbf29ce484222325);
    t.hash(&mut f);
    f.finish()
}

struct FixedHasher(u64);
impl std::hash::Hasher for FixedHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 ^= *b as u64;
            self.0 = self.0.wrapping_mul(0x100000001b3);
        }
    }
}

fn median(v: &mut [f64]) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

fn bench() {
    println!("## bench: per-frame cost, window {W}x{H}, ScrollArea at top (release build)");
    println!(
        "| nodes | mode | accesskit | run_ui median ms | tessellate median ms | visited nodes | passes frame 1 / steady |"
    );
    println!("|---:|---|---|---:|---:|---:|---|");
    for rows in [100usize, 1000, 5000] {
        let configs: &[(&str, LayoutMode, bool, bool, bool)] = &[
            // name, layout, cull, scoped_leaves, accesskit
            ("native, full walk", LayoutMode::Native, false, false, false),
            ("native, full walk", LayoutMode::Native, false, false, true),
            ("native, culled", LayoutMode::Native, true, false, false),
            ("native, culled, scoped leaves", LayoutMode::Native, true, true, false),
            ("native, culled", LayoutMode::Native, true, false, true),
            ("taffy, culled", LayoutMode::Taffy, true, false, false),
            ("taffy, culled", LayoutMode::Taffy, true, false, true),
        ];
        for &(name, layout, cull, scoped, ak) in configs {
            let (a, root) = build(rows);
            let nodes = a.live;
            let mut s = Scene::new(a, root);
            s.layout = layout;
            s.cull = cull;
            s.scoped_leaves = scoped;
            if layout == LayoutMode::Taffy {
                s.build_taffy();
            }
            let ctx = Context::default();
            if ak {
                ctx.enable_accesskit();
            }
            let first = frame(&ctx, &mut s, 0, vec![]);
            let warm = 3;
            let m = match rows {
                100 => 60,
                1000 => 30,
                _ => 10,
            };
            let mut run = Vec::new();
            let mut tess = Vec::new();
            let mut steady_passes = 0;
            for i in 0..warm + m {
                let st = frame(&ctx, &mut s, 1 + i as u64, vec![]);
                if i >= warm {
                    run.push(st.run_us);
                    tess.push(st.tess_us);
                    steady_passes = steady_passes.max(st.passes);
                }
            }
            println!(
                "| {nodes} | {name} | {} | {:.3} | {:.3} | {} | {} / {} |",
                if ak { "on" } else { "off" },
                median(&mut run) / 1000.0,
                median(&mut tess) / 1000.0,
                s.visited,
                first.passes,
                steady_passes
            );
        }
    }
}

fn layout_timings() {
    println!("\n## layout: taffy two-pass timings (leaves measured with egui fonts)");
    println!("| nodes | mirror build ms | cold layout ms (taffy part, measure calls) | clean check us | one leaf dirty ms (taffy part, measure calls) | resize ms (taffy part, measure calls) |");
    println!("|---:|---:|---:|---:|---:|---:|");
    for rows in [100usize, 1000, 5000] {
        let (a, root) = build(rows);
        let nodes = a.live;
        let mut s = Scene::new(a, root);
        s.layout = LayoutMode::Taffy;
        let t = Instant::now();
        s.build_taffy();
        let build_ms = t.elapsed().as_secs_f64() * 1e3;
        let ctx = Context::default();
        // a first pass so fonts exist
        let mut o = ctx.run_ui(input(0, vec![]), |_ui| {});
        o.textures_delta.clear();
        let mut res = (0.0, 0.0, 0.0, 0.0);
        let mut det = Vec::new();
        let mid_label = s.arena.n(s.arena.n(root).children[rows / 2]).children[0];
        let mut o = ctx.run_ui(input(1, vec![]), |ui| {
            let ctx = ui.ctx().clone();
            let t = Instant::now();
            assert!(s.layout_taffy(&ctx, W));
            res.0 = t.elapsed().as_secs_f64() * 1e3;
            det.push((s.compute_us / 1e3, s.measure_calls.replace(0)));
            let t = Instant::now();
            assert!(!s.layout_taffy(&ctx, W));
            res.1 = t.elapsed().as_secs_f64() * 1e6;
            s.arena.n_mut(mid_label).text = "a much longer label that changes the row".into();
            s.mark_dirty(mid_label);
            let t = Instant::now();
            assert!(s.layout_taffy(&ctx, W));
            res.2 = t.elapsed().as_secs_f64() * 1e3;
            det.push((s.compute_us / 1e3, s.measure_calls.replace(0)));
            let t = Instant::now();
            assert!(s.layout_taffy(&ctx, W - 200.0));
            res.3 = t.elapsed().as_secs_f64() * 1e3;
            det.push((s.compute_us / 1e3, s.measure_calls.replace(0)));
        });
        o.textures_delta.clear();
        println!(
            "| {nodes} | {build_ms:.2} | {:.2} ({:.2}, {}) | {:.1} | {:.3} ({:.3}, {}) | {:.2} ({:.2}, {}) |",
            res.0, det[0].0, det[0].1, res.1, res.2, det[1].0, det[1].1, res.3, det[2].0, det[2].1
        );
    }
}

// ------------------------------------------------------------------------ checks

fn click_events(p: Pos2) -> (Vec<Event>, Vec<Event>) {
    let press = vec![
        Event::PointerMoved(p),
        Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE },
    ];
    let release = vec![Event::PointerButton {
        pos: p,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    }];
    (press, release)
}

/// 20 text edits keyed 0..20; click key 7, type, reverse the keyed order (remove + insert, as the
/// keyed diff does), type again; then "hot reload" (new handles, same identities) and type again.
fn id_stability(mode: IdMode) -> (bool, bool) {
    let build_edits = |a: &mut Arena| {
        let root = a.create(Kind::Column, 1);
        let mut keys = Vec::new();
        for k in 0..20u64 {
            let h = a.create(Kind::TextEdit, hash(&("edit", k)));
            a.n_mut(h).text = format!("k{k}");
            a.insert(root, h, None).unwrap();
            keys.push(h);
        }
        (root, keys)
    };
    let mut a = Arena::default();
    let (root, keys) = build_edits(&mut a);
    let mut s = Scene::new(a, root);
    s.id_mode = mode;
    s.scroll = false;
    let ctx = Context::default();
    let mut n = 0;
    let mut run = |s: &mut Scene, ev: Vec<Event>| {
        n += 1;
        frame(&ctx, s, n, ev);
    };
    run(&mut s, vec![]);
    let (press, release) = click_events(s.center_of(keys[7]));
    run(&mut s, press);
    run(&mut s, release);
    run(&mut s, vec![Event::Text("A".into())]);
    // keyed reorder: reverse
    for &k in &keys {
        s.arena.remove(root, k).unwrap();
    }
    for &k in keys.iter().rev() {
        s.arena.insert(root, k, None).unwrap();
    }
    run(&mut s, vec![]);
    run(&mut s, vec![Event::Text("B".into())]);
    let reorder_ok = s.arena.n(keys[7]).text == "k7AB";
    // hot reload: destroy every node and rebuild in the same arena (fresh handles, same
    // identities/text), as the in-process reload does
    let texts: Vec<String> = keys.iter().map(|&k| s.arena.n(k).text.clone()).collect();
    for &k in &keys {
        s.arena.destroy(k);
    }
    s.arena.destroy(root);
    let (root2, keys2) = build_edits(&mut s.arena);
    assert!(keys2.iter().all(|k| !keys.contains(k)), "handles must be fresh");
    for (i, &k) in keys2.iter().enumerate() {
        s.arena.n_mut(k).text = texts[i].clone();
    }
    for &k in &keys2 {
        s.arena.remove(root2, k).unwrap();
    }
    for &k in keys2.iter().rev() {
        s.arena.insert(root2, k, None).unwrap();
    }
    s.root = root2;
    run(&mut s, vec![]);
    run(&mut s, vec![Event::Text("C".into())]);
    let reload_ok = s.arena.n(keys2[7]).text == "k7ABC";
    (reorder_ok, reload_ok)
}

fn counter_scene(layout: LayoutMode) -> (Scene, Handle, Handle) {
    let mut a = Arena::default();
    let root = a.create(Kind::Column, 1);
    let label = a.create(Kind::Label, 2);
    a.n_mut(label).text = "count: 0".into();
    let button = a.create(Kind::Button, 3);
    a.n_mut(button).text = "Increment".into();
    a.n_mut(button).listen_clicked = true;
    let edit = a.create(Kind::TextEdit, 4);
    a.n_mut(edit).observe_text = true;
    a.insert(root, label, None).unwrap();
    a.insert(root, button, None).unwrap();
    a.insert(root, edit, None).unwrap();
    let mut s = Scene::new(a, root);
    s.layout = layout;
    if layout == LayoutMode::Taffy {
        s.build_taffy();
    }
    (s, label, edit)
}

/// The "JS side": runs after `run_ui` returned, never during the walk.
fn dispatch(s: &mut Scene, label: Handle, count: &mut u32) -> (usize, usize) {
    let events = std::mem::take(&mut s.events);
    let changed = std::mem::take(&mut s.changed);
    for &(_h, sig) in &events {
        if sig == "clicked" {
            *count += 1;
            s.arena.n_mut(label).text = format!("count: {count}");
            s.mark_dirty(label);
        }
    }
    (events.len(), changed.len())
}

fn events_check(layout: LayoutMode) -> String {
    let (mut s, label, edit) = counter_scene(layout);
    let button = s.arena.n(s.root).children[1];
    let ctx = Context::default();
    let mut count = 0;
    frame(&ctx, &mut s, 0, vec![]);
    let (press, release) = click_events(s.center_of(button));
    frame(&ctx, &mut s, 1, press);
    let after_press = dispatch(&mut s, label, &mut count);
    frame(&ctx, &mut s, 2, release);
    let after_release = dispatch(&mut s, label, &mut count);
    frame(&ctx, &mut s, 3, vec![]);
    // in-frame mutation of a bound value: click the text edit and type
    let (p2, r2) = click_events(s.center_of(edit));
    frame(&ctx, &mut s, 4, p2);
    frame(&ctx, &mut s, 5, r2);
    dispatch(&mut s, label, &mut count);
    frame(&ctx, &mut s, 6, vec![Event::Text("hi".into())]);
    let typed = dispatch(&mut s, label, &mut count);
    format!(
        "{layout:?}: events after press={} release={} (label now {:?}); typing 'hi' -> prop changes={} text={:?}",
        after_press.0,
        after_release.0,
        s.arena.n(label).text,
        typed.1,
        s.arena.n(edit).text
    )
}

fn kittest_check() -> String {
    use egui_kittest::Harness;
    use egui_kittest::kittest::{NodeT, Queryable};
    let (s, label, _edit) = counter_scene(LayoutMode::Native);
    let mut harness = Harness::new_ui_state(|ui, s: &mut Scene| s.ui(ui), s);
    harness.get_by_label("Increment").click();
    harness.run();
    let mut count = 0;
    let (ev, _) = dispatch(harness.state_mut(), label, &mut count);
    harness.run();
    let found = harness.query_by_label("count: 1").is_some();
    let edit_id = harness.state().egui_id(harness.state().arena.n(harness.state().root).children[2]).unwrap();
    let by_id = harness.get_by_label("Increment").accesskit_node().id();
    format!(
        "kittest: clicked via AccessKit query, {ev} event(s) dispatched after the frame, label 'count: 1' found={found}; \
         AccessKit node ids are egui Ids (text edit id {:#x} = accesskit {:?}; button node {:?})",
        edit_id.value(),
        edit_id.accesskit_id(),
        by_id
    )
}

fn checks() {
    println!("\n## checks");
    for mode in [IdMode::Identity, IdMode::Handle, IdMode::Positional] {
        let (r, h) = id_stability(mode);
        println!("- id {mode:?}: focus+typing survives keyed reorder: {r}; survives hot reload (new handles): {h}");
    }
    println!("- {}", events_check(LayoutMode::Native));
    println!("- {}", events_check(LayoutMode::Taffy));
    println!("- {}", kittest_check());
}

fn main() {
    let arg = std::env::args().nth(1).unwrap_or_default();
    match arg.as_str() {
        "bench" => bench(),
        "layout" => layout_timings(),
        "checks" => checks(),
        _ => {
            checks();
            layout_timings();
            bench();
        }
    }
}

/// Diagnostic: with native culling, rows below the first must still be walked (regression test
/// for the `Align::Center` row that grew to the full window height).
#[cfg(test)]
mod debug {
    use super::*;
    #[test]
    fn culled_rows() {
        let (a, root) = build(100);
        let mut s = Scene::new(a, root);
        s.cull = true;
        let ctx = Context::default();
        for i in 0..3 {
            frame(&ctx, &mut s, i, vec![]);
            let rows = &s.arena.n(root).children;
            eprintln!("frame {i}: visited {} row0 {:?} row1 {:?} size1 {:?}", s.visited,
                s.arena.n(rows[0]).screen_rect, s.arena.n(rows[1]).screen_rect, s.arena.n(rows[1]).last_size);
        }
    }
}

/// Diagnostic: taffy re-queries no leaf when only containers are marked dirty.
#[cfg(test)]
mod debug_cache {
    use super::*;
    #[test]
    fn cache_hits() {
        let (a, root) = build(100);
        let mut s = Scene::new(a, root);
        s.layout = LayoutMode::Taffy;
        s.build_taffy();
        let ctx = Context::default();
        let mut o = ctx.run_ui(input(0, vec![]), |_ui| {});
        o.textures_delta.clear();
        let mut o = ctx.run_ui(input(1, vec![]), |ui| {
            let ctx = ui.ctx().clone();
            s.layout_taffy(&ctx, W);
            eprintln!("cold: {}", s.measure_calls.replace(0));
            let rt = s.arena.n(root).tnode.unwrap();
            s.taffy.mark_dirty(rt).unwrap();
            s.layout_taffy(&ctx, W);
            eprintln!("root only dirty: {}", s.measure_calls.replace(0));
            let row = s.arena.n(root).children[50];
            let rtn = s.arena.n(row).tnode.unwrap();
            s.taffy.mark_dirty(rtn).unwrap();
            s.layout_taffy(&ctx, W);
            eprintln!("one row dirty (no change): {}", s.measure_calls.replace(0));
        });
        o.textures_delta.clear();
    }
}
