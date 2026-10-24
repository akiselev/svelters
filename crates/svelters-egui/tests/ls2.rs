//! LS2 (plans/milestones/LS2-rust-row-models.md): Rust row models for the own `table` (SV41), its
//! columns, Rust-side sort and filter and tree rows (SV44/SV52), and the T2 benches (SV47): bind
//! per row, a page rebind and a fling at 20k and 100k rows, for `virtual-list` (a `RowSource`) and
//! `table` (a `RowModel`). The benches print their numbers and assert generous bounds (tighter in
//! release; CI's `egui-benches` job runs them in release: `-E 'binary(ls2) & test(/bench/)'`).

use egui::Role;
use egui_kittest::kittest::{NodeT, Queryable};
use std::borrow::Cow;
use std::cell::Cell as StdCell;
use std::rc::Rc;
use std::time::Instant;
use svelters_abi::{Backend, Value};
use svelters_egui::embed::{EmbedConfig, SlotId, SlotOptions};
use svelters_egui::harness::EmbedHarness;
use svelters_egui::kinds;
use svelters_egui::scene::{NodeId, ProviderId, RowPool, RowSource, SceneHandle, SceneMut, keys};
use svelters_host::HostConfig;
use svelters_host::rows::{Cell, CellKind, Column, RowFilter, RowModel, RowView, VecModel};

const APP: &str = "ls2.test";

// ---- the nets list (cadatomic's ECAD shape) ----

/// `n` nets: name, class, pad count. Counts the cells it is asked for.
struct Nets {
    cols: Vec<Column>,
    names: Vec<String>,
    pads: Vec<i64>,
    version: u64,
    reads: Rc<StdCell<u64>>,
}

impl Nets {
    fn new(n: usize) -> Nets {
        Nets {
            cols: vec![Column::new("name", "Net").width(160.0), Column::new("class", "Class").width(100.0), Column::new("pads", "Pads").width(80.0).kind(CellKind::Number)],
            names: (0..n).map(|i| format!("N{i}")).collect(),
            pads: (0..n).map(|i| (i % 7) as i64 + 1).collect(),
            version: 0,
            reads: Rc::default(),
        }
    }
}

impl RowModel for Nets {
    fn len(&self) -> usize {
        self.names.len()
    }
    fn key(&self, row: usize) -> u64 {
        row as u64
    }
    fn columns(&self) -> &[Column] {
        &self.cols
    }
    fn cell(&self, row: usize, col: usize) -> Cell<'_> {
        self.reads.set(self.reads.get() + 1);
        match col {
            0 => Cell::text(&self.names[row]),
            1 => Cell::text(if row % 3 == 0 { "power" } else { "signal" }),
            _ => Cell::Int(self.pads[row]),
        }
    }
    fn version(&self) -> u64 {
        self.version
    }
    fn anchor(&self, key: u64) -> Option<String> {
        Some(format!("row[net:{}]", self.names[key as usize]))
    }
}

// ---- the rig: one slot, Rust-built nodes ----

struct Rig {
    h: EmbedHarness,
    scene: SceneHandle,
    slot: NodeId,
}

impl Rig {
    fn new(w: f32, h: f32) -> Rig {
        let cfg = EmbedConfig { host: HostConfig { echo_console: false, echo_errors: false, quit_on_last_window: false, ..HostConfig::default() }, ..EmbedConfig::default() };
        let mut eh = EmbedHarness::new(egui::vec2(w, h), cfg).unwrap();
        let e = eh.embed.clone();
        let slot: SlotId = e.create_slot("ls2", SlotOptions::fill());
        eh.set_ui(move |e, ui| {
            e.show_slot(ui, slot);
        });
        let scene = e.backend().scene();
        let slot = NodeId::from_raw(slot.handle()).unwrap();
        Rig { h: eh, scene, slot }
    }

    fn add(&self, parent: NodeId, tag: &str, props: &[(&str, Value)]) -> NodeId {
        let props: Vec<(String, Value)> = props.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
        let tag = tag.to_string();
        self.scene
            .write(ProviderId::named(APP), move |m| {
                let n = m.create_tag(&tag).unwrap();
                for (k, v) in &props {
                    m.node(n).unwrap().set_by_name(k, v).unwrap_or_else(|e| panic!("{tag}.{k}: {e:?}"));
                }
                m.insert(parent, n, None).unwrap();
                n
            })
            .unwrap()
    }

    fn write<R>(&self, f: impl FnOnce(&mut SceneMut<'_>) -> R) -> R {
        self.scene.write(ProviderId::named(APP), f).unwrap()
    }

    /// A table showing `model`.
    fn table(&mut self, model: impl RowModel, view: RowView, props: &[(&str, Value)]) -> NodeId {
        let mut p = vec![("flex-grow", Value::F64(1.0)), ("accessible-name", Value::Str("Nets".into()))];
        p.extend(props.iter().cloned());
        let t = self.add(self.slot, "table", &p);
        self.write(|m| m.set_table_model(t, model, view)).unwrap();
        self.h.run();
        t
    }

    fn set(&self, n: NodeId, name: &str, v: Value) {
        let name = name.to_string();
        self.write(move |m| m.node(n).unwrap().set_by_name(&name, &v).unwrap());
    }

    fn get(&self, n: NodeId, name: &str) -> Value {
        self.scene.read(|s| s.get(n).unwrap().value_of(name).unwrap())
    }

    fn call(&self, t: NodeId, name: &str, args: &[Value]) -> Value {
        let b = self.h.embed.backend();
        let (m, _) = kinds::find_method(kinds::tag_index("table").unwrap(), name).unwrap();
        b.call(t.to_raw(), m, args).unwrap()
    }

    fn cell(&self, text: &str) -> Option<egui::Rect> {
        self.h.harness.query_by_role_and_label(Role::Cell, text).map(|n| n.rect())
    }

    fn has(&self, text: &str) -> bool {
        self.cell(text).is_some()
    }

    fn click_at(&mut self, at: egui::Pos2) {
        let (m, b) = (egui::Modifiers::NONE, egui::PointerButton::Primary);
        for e in [egui::Event::PointerMoved(at), egui::Event::PointerButton { pos: at, button: b, pressed: true, modifiers: m }, egui::Event::PointerButton { pos: at, button: b, pressed: false, modifiers: m }] {
            self.h.harness.event(e);
            self.h.harness.step();
        }
        self.h.run();
    }

    fn key(&mut self, key: egui::Key) {
        for pressed in [true, false] {
            self.h.harness.event(egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers: egui::Modifiers::NONE });
        }
        self.h.run();
    }

    fn wheel_over(&mut self, n: NodeId, dy: f32) {
        let c = self.scene.read(|s| s.get(n).unwrap().drawn().unwrap()).center();
        self.h.harness.event(egui::Event::PointerMoved(c));
        self.h.harness.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, dy),
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Unknown,
            modifiers: egui::Modifiers::NONE,
        });
    }

    /// The keys of the rows the table shows, in order.
    fn shown(&self, t: NodeId) -> Vec<u64> {
        self.scene.read(|s| s.table_model::<VecModel, _>(t, |m, v| v.keys(m))).or_else(|| self.scene.read(|s| s.table_model::<Nets, _>(t, |m, v| v.keys(m)))).unwrap()
    }
}

fn ids(v: &[u64]) -> Value {
    Value::List(v.iter().map(|&k| Value::I64(k as i64)).collect())
}

// ---- SV41: the table reads a Rust model ----

#[test]
fn a_rust_model_feeds_the_table_with_columns_from_the_model() {
    let mut r = Rig::new(600.0, 300.0);
    let nets = Nets::new(20_000);
    let reads = nets.reads.clone();
    let t = r.table(nets, RowView::new(), &[]);
    // the model's columns became table-columns (name = key; Pads right-aligned)
    for h in ["Net", "Class", "Pads"] {
        assert!(r.h.harness.query_by_role_and_label(Role::ColumnHeader, h).is_some(), "header {h}");
    }
    let cols: Vec<NodeId> = r.scene.read(|s| s.get(t).unwrap().children().map(|c| c.id()).collect());
    assert_eq!(cols.len(), 3);
    assert_eq!((r.get(cols[2], "name"), r.get(cols[2], "align")), (Value::Str("pads".into()), Value::Str("end".into())));
    assert!(r.has("N0") && r.has("N12") && !r.has("N500"));
    // the cells read are the shown ones (no window, no `range`)
    let before = reads.get();
    r.h.harness.step();
    let per_frame = reads.get() - before;
    assert!(per_frame > 0 && per_frame <= 3 * 20, "one frame reads the visible cells only: {per_frame}");
    r.call(t, "scroll-to-row", &[Value::I64(15_000)]);
    r.h.run();
    assert!(r.has("N15000"), "scrolled to row 15000");
    assert_eq!(r.scene.read(|s| s.table_model::<Nets, _>(t, |m, v| (m.len(), v.len()))), Some((20_000, 20_000)));
    // the model changes: a new version redraws
    r.write(|m| m.with_table_model::<Nets, _>(t, |n, _| {
        n.names[15_000] = "GND".into();
        n.version += 1;
    }));
    r.h.run();
    assert!(r.has("GND") && !r.has("N15000"));
}

// ---- SV44/SV52: sort and filter in Rust, selection by key ----

#[test]
fn sorting_and_filtering_run_in_rust_and_keep_the_selection_by_key() {
    let mut r = Rig::new(600.0, 300.0);
    let t = r.table(Nets::new(2_000), RowView::new(), &[("sortable", Value::Bool(true)), ("selection", Value::Str("multi".into()))]);
    // select N5 by a click
    let c = r.cell("N5").unwrap().center();
    r.click_at(c);
    assert_eq!(r.get(t, "selected"), ids(&[5]));
    // sort on Pads (a header click): ascending pads, ties in model order
    r.h.harness.get_by_role_and_label(Role::ColumnHeader, "Pads").click();
    r.h.run();
    assert_eq!((r.get(t, "sort-column"), r.get(t, "sort-order")), (Value::I64(2), Value::Str("ascending".into())));
    assert_eq!(r.shown(t)[..3], [0, 7, 14], "pads 1 first");
    // descending by name, naturally: N1999 first
    r.set(t, "sort-column", Value::I64(0));
    r.set(t, "sort-order", Value::Str("descending".into()));
    r.h.run();
    assert_eq!(r.shown(t)[..2], [1999, 1998]);
    assert!(r.has("N1999"));
    assert_eq!(r.get(t, "selected"), ids(&[5]), "the selection is the same row after sorting");
    // a filter: only power nets with a 5 in their name
    r.write(|m| m.with_table_model::<Nets, _>(t, |_, v| v.set_filter(Some(RowFilter::custom(|m, row| m.cell(row, 1).display() == "power" && m.cell(row, 0).display().contains('5'))))));
    r.h.run();
    let shown = r.shown(t);
    assert!(!shown.is_empty() && shown.iter().all(|&k| k % 3 == 0), "{shown:?}");
    assert!(!shown.contains(&5));
    assert_eq!(r.get(t, "selected"), ids(&[5]), "a filtered-out row stays selected (by key)");
    r.write(|m| m.with_table_model::<Nets, _>(t, |_, v| v.set_filter(None)));
    r.h.run();
    assert_eq!(r.shown(t).len(), 2_000);
}

// ---- SV44/SV52: tree rows ----

/// a (b, c (d)), e
fn tree() -> VecModel {
    let mut m = VecModel::new(vec![Column::new("name", "Name").width(200.0), Column::new("kind", "Kind")]);
    for (k, (d, name, kind)) in [(0, "a", "group"), (1, "b", "item"), (1, "c", "group"), (2, "d", "item"), (0, "e", "item")].into_iter().enumerate() {
        m.rows.push((k as u64, d, vec![Cell::Text(Cow::Borrowed(name)), Cell::text(kind)]));
    }
    m
}

#[test]
fn tree_rows_expand_by_their_expander_and_the_keyboard() {
    let mut r = Rig::new(500.0, 300.0);
    let t = r.table(tree(), RowView::new(), &[]);
    assert_eq!(r.shown(t), [0, 4], "collapsed");
    // a11y: rows carry their level and expanded state
    let rows: Vec<(Option<usize>, Option<bool>)> = r.h.harness.query_all_by_role(Role::Row).map(|n| (n.accesskit_node().level(), n.accesskit_node().data().is_expanded())).collect();
    assert_eq!(rows, [(Some(1), Some(false)), (Some(1), None)]);
    // a click on a's chevron (4 px cell padding, 16 px expander at depth 0)
    let a = r.cell("a").unwrap();
    let chevron = egui::pos2(a.left() + 4.0 + 8.0, a.center().y);
    r.click_at(chevron);
    assert_eq!(r.shown(t), [0, 1, 2, 4]);
    assert_eq!(r.get(t, "selected"), ids(&[]), "the expander does not select");
    // c is indented one level: its text starts right of a's
    assert!(r.cell("c").is_some());
    // the keyboard: select c, Right expands it, Left collapses, Left again goes to its parent
    let c = r.cell("c").unwrap().center();
    r.click_at(egui::pos2(c.x + 60.0, c.y));
    assert_eq!(r.get(t, "selected"), ids(&[2]));
    r.key(egui::Key::ArrowRight);
    assert_eq!(r.shown(t), [0, 1, 2, 3, 4]);
    assert!(r.has("d"));
    r.key(egui::Key::ArrowLeft);
    assert_eq!(r.shown(t), [0, 1, 2, 4]);
    r.key(egui::Key::ArrowLeft);
    assert_eq!(r.get(t, "selected"), ids(&[0]), "to the parent");
    // sorting orders siblings under their parent
    r.set(t, "sort-column", Value::I64(0));
    r.set(t, "sort-order", Value::Str("descending".into()));
    r.h.run();
    assert_eq!(r.shown(t), [4, 0, 2, 1]);
}

// ---- SV47: T2 benches ----

/// 20k or 100k parameters as realised rows (the property-grid shape): a name and a value label.
struct Params {
    n: usize,
    version: u64,
    binds: Rc<StdCell<u64>>,
}

impl RowSource for Params {
    fn len(&self) -> usize {
        self.n
    }
    fn key(&self, row: usize) -> u64 {
        row as u64
    }
    fn uniform_height(&self) -> Option<f32> {
        Some(24.0)
    }
    fn build(&mut self, s: &mut SceneMut<'_>, _template: u32) -> NodeId {
        let row = s.create_tag("box").unwrap();
        s.node(row).unwrap().set(&keys::container::FLEX_DIRECTION, "row").unwrap();
        for _ in 0..2 {
            let l = s.create_tag("label").unwrap();
            s.insert(row, l, None).unwrap();
        }
        row
    }
    fn bind(&mut self, s: &mut SceneMut<'_>, node: NodeId, row: usize) {
        self.binds.set(self.binds.get() + 1);
        let kids: Vec<NodeId> = s.read().get(node).unwrap().children().map(|n| n.id()).collect();
        s.node(kids[0]).unwrap().set(&keys::label::TEXT, format!("p{row}")).unwrap();
        s.node(kids[1]).unwrap().set(&keys::label::TEXT, format!("{} mm v{}", row * 10, self.version)).unwrap();
    }
    fn version(&self) -> u64 {
        self.version
    }
}

/// Median and max of frame times (ms).
fn stats(v: &mut [f64]) -> (f64, f64) {
    v.sort_by(f64::total_cmp);
    (v[v.len() / 2], v[v.len() - 1])
}

fn frame_ms(r: &mut Rig) -> f64 {
    let t0 = Instant::now();
    r.h.harness.step();
    t0.elapsed().as_secs_f64() * 1e3
}

/// The bounds: generous, tighter in release (CI's numbers are reported, not gated).
const FRAME_MAX_MS: f64 = if cfg!(debug_assertions) { 400.0 } else { 40.0 };
const BIND_ROW_MAX_MS: f64 = if cfg!(debug_assertions) { 20.0 } else { 1.0 };
const SORT_MAX_MS: f64 = if cfg!(debug_assertions) { 20_000.0 } else { 1_000.0 };

fn report(line: String) {
    eprintln!("{line}");
    if let Ok(p) = std::env::var("LS2_BENCH_OUT") {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(p).unwrap();
        writeln!(f, "{line}").unwrap();
    }
}

fn bench_virtual_list(n: usize) {
    let mut r = Rig::new(480.0, 600.0);
    let list = r.add(r.slot, "virtual-list", &[("flex-grow", Value::F64(1.0)), ("row-height", Value::I64(24))]);
    let binds = Rc::new(StdCell::new(0));
    let src = Params { n, version: 0, binds: binds.clone() };
    r.write(|m| m.set_row_source(list, src, RowPool::new())).unwrap();
    r.h.run();
    let shown = binds.get();
    assert!(shown > 10, "{shown} rows bound");
    // a page rebind: the model changes, every realised row binds again in one frame
    let mut rebind = Vec::new();
    for _ in 0..10 {
        let b0 = binds.get();
        r.write(|m| m.with_row_source::<Params, _>(list, |p| p.version += 1));
        rebind.push((frame_ms(&mut r), binds.get() - b0));
    }
    let rows = rebind.iter().map(|x| x.1).max().unwrap();
    let mut ms: Vec<f64> = rebind.iter().map(|x| x.0).collect();
    let (rebind_ms, _) = stats(&mut ms);
    let per_row = rebind_ms / rows.max(1) as f64;
    // a fling: 60 frames of 600 points each
    let mut fling = Vec::new();
    for _ in 0..60 {
        r.wheel_over(list, -600.0);
        fling.push(frame_ms(&mut r));
    }
    let (fling_med, fling_max) = stats(&mut fling);
    let off = r.scene.read(|s| s.virtual_list(list).unwrap().offset());
    assert!(off > 10_000.0, "the fling scrolled: {off}");
    report(format!(
        "SV47 virtual-list {n} rows: page rebind {rebind_ms:.2} ms ({rows} rows, {per_row:.3} ms/row); fling frame median {fling_med:.2} ms, max {fling_max:.2} ms"
    ));
    assert!(per_row < BIND_ROW_MAX_MS, "bind per row {per_row:.3} ms");
    assert!(rebind_ms < FRAME_MAX_MS && fling_max < FRAME_MAX_MS, "frames: rebind {rebind_ms:.2}, fling max {fling_max:.2}");
}

fn bench_table(n: usize) {
    let mut r = Rig::new(600.0, 600.0);
    let nets = Nets::new(n);
    let reads = nets.reads.clone();
    let t0 = Instant::now();
    let t = r.table(nets, RowView::new(), &[("sortable", Value::Bool(true))]);
    let first_ms = t0.elapsed().as_secs_f64() * 1e3;
    // a page rebind: a jump far away shows only new rows
    let mut jumps = Vec::new();
    let mut cells = 0;
    for k in 1..=10u64 {
        r.call(t, "scroll-to-row", &[Value::I64((k * n as u64 / 11) as i64)]);
        let c0 = reads.get();
        jumps.push(frame_ms(&mut r));
        cells = cells.max(reads.get() - c0);
    }
    let (jump_ms, jump_max) = stats(&mut jumps);
    let per_row = jump_ms / (cells as f64 / 3.0).max(1.0);
    // a fling
    let mut fling = Vec::new();
    for _ in 0..60 {
        r.wheel_over(t, -600.0);
        fling.push(frame_ms(&mut r));
    }
    let (fling_med, fling_max) = stats(&mut fling);
    // sorting all rows in Rust (a header click), and a text filter
    let t1 = Instant::now();
    r.set(t, "sort-column", Value::I64(0));
    r.set(t, "sort-order", Value::Str("descending".into()));
    r.h.harness.step();
    let sort_ms = t1.elapsed().as_secs_f64() * 1e3;
    assert_eq!(r.shown(t)[0], n as u64 - 1);
    let t2 = Instant::now();
    r.write(|m| m.with_table_model::<Nets, _>(t, |_, v| v.set_filter(Some(RowFilter::text("99")))));
    r.h.harness.step();
    let filter_ms = t2.elapsed().as_secs_f64() * 1e3;
    report(format!(
        "SV47 table {n} rows: first frame {first_ms:.1} ms; page jump median {jump_ms:.2} ms, max {jump_max:.2} ms ({cells} cells, {per_row:.3} ms/row); fling frame median {fling_med:.2} ms, max {fling_max:.2} ms; sort by name {sort_ms:.1} ms; filter {filter_ms:.1} ms"
    ));
    assert!(per_row < BIND_ROW_MAX_MS, "per row {per_row:.3} ms");
    assert!(jump_max < FRAME_MAX_MS && fling_max < FRAME_MAX_MS, "frames: jump max {jump_max:.2}, fling max {fling_max:.2}");
    assert!(sort_ms < SORT_MAX_MS && filter_ms < SORT_MAX_MS, "sort {sort_ms:.1} ms, filter {filter_ms:.1} ms");
}

#[test]
fn bench_virtual_list_20k() {
    bench_virtual_list(20_000);
}

#[test]
fn bench_virtual_list_100k() {
    bench_virtual_list(100_000);
}

#[test]
fn bench_table_20k() {
    bench_table(20_000);
}

#[test]
fn bench_table_100k() {
    bench_table(100_000);
}
