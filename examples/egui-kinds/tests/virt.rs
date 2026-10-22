//! SG5 (ARCH-SCENE §8.2, §10.3): a painted-row provider kind, `<virtual-tree>`, the shape of
//! cadatomic's model tree: no node per row, rows painted from a Rust model inside
//! `ScrollArea::show_viewport` with a uniform `RowHeights`, exposed through `VirtualChildren` to
//! queries (`include_virtual`), anchors (`target_by_anchor`, off-screen rows too), hit-tests
//! (`Hit::Part(key)` → `Target::Virtual`) and a11y (set size and position per drawn row).
//!
//! Gates: 10k rows scroll with 0 JS entries (no checkpoint, event or call while scrolling) and
//! ≤ 1 ms of scene time per frame (release; debug builds report the number and check a loose
//! bound); 1M rows with the uniform height model.

use std::borrow::Cow;
use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};
use svelters_egui::embed::{EmbedConfig, SlotOptions};
use svelters_egui::harness::EmbedHarness;
use svelters_egui::harness::egui_kittest::kittest::{NodeT, Queryable};
use svelters_egui::prelude::*;
use svelters_egui::scene::{NodeId, ProviderId, RowHeights, Target, VNodeId, VirtualChildren, keys, virt};
use svelters_host::HostConfig;

/// A tree of `rows` painted rows, 20 points each; row `r` has key `r * 7` and is named
/// `Feature <key>`.
#[svelters::widget(namespace = "sg5test", tag = "virtual-tree", caps = VIRTUAL | HIT_TEST)]
pub struct VirtualTree {
    /// How many rows the model has.
    #[prop]
    rows: f64,
    #[state]
    heights: RowHeights,
    /// The scroll offset drawn last (from the scroll area's viewport).
    #[state]
    scroll: f32,
    /// The inner rect's top left relative to the node's rect.
    #[state]
    inner: egui::Vec2,
    #[state]
    width: f32,
}

const ROW: f32 = 20.0;

impl VirtualTree {
    fn sync(&mut self) {
        let n = self.rows.max(0.0) as usize;
        if self.heights.len() != n || self.heights.estimate() != ROW {
            self.heights = RowHeights::uniform(n, ROW);
        }
    }
}

impl Widget for VirtualTree {
    fn measure(&mut self, _cx: &mut MeasureCx<'_>, avail: AvailableSpace) -> egui::Vec2 {
        egui::vec2(avail.width_or(240.0), 100.0)
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        self.sync();
        let total = self.heights.total() as f32;
        let id = cx.id();
        let node_min = cx.rect().min;
        let len = self.heights.len();
        let out = egui::ScrollArea::vertical().id_salt(id).auto_shrink(false).show_viewport(ui, |ui, vp| {
            ui.set_height(total);
            self.scroll = vp.min.y;
            let top = ui.max_rect().min;
            self.inner = egui::vec2(0.0, top.y + vp.min.y - node_min.y);
            self.width = ui.max_rect().width();
            for row in self.heights.range(vp.min.y as f64, vp.max.y as f64) {
                let key = row as u64 * 7;
                let r = egui::Rect::from_min_size(egui::pos2(top.x, top.y + self.heights.offset(row) as f32), egui::vec2(ui.max_rect().width(), ROW));
                let rid = id.with(("row", key));
                let resp = ui.interact(r, rid, egui::Sense::click());
                if resp.hovered() {
                    ui.painter().rect_filled(r, 0.0, ui.visuals().widgets.hovered.weak_bg_fill);
                }
                ui.painter().text(r.left_center() + egui::vec2(4.0 + (row % 4) as f32 * 12.0, 0.0), egui::Align2::LEFT_CENTER, format!("Feature {key}"), egui::FontId::proportional(13.0), ui.visuals().text_color());
                virt::a11y_row(ui.ctx(), rid, row, len, |n| {
                    n.set_role(egui::accesskit::Role::TreeItem);
                    n.set_label(format!("Feature {key}"));
                    n.set_level((row % 4) + 1);
                });
            }
        });
        ui.interact(out.inner_rect, id, egui::Sense::hover())
    }

    fn hit_test(&self, _cx: &HitCx, local: egui::Pos2) -> Hit {
        let y = local.y as f64 - self.inner.y as f64 + self.scroll as f64;
        if self.heights.is_empty() || y < 0.0 || y >= self.heights.total() {
            return Hit::Node;
        }
        Hit::Part(self.heights.row_at(y) as u64 * 7)
    }

    fn virtual_children(&self) -> Option<&dyn VirtualChildren> {
        Some(self)
    }
}

impl VirtualChildren for VirtualTree {
    fn len(&self) -> usize {
        self.heights.len()
    }
    fn key(&self, row: usize) -> u64 {
        row as u64 * 7
    }
    fn row_of(&self, key: u64) -> Option<usize> {
        (key % 7 == 0 && ((key / 7) as usize) < self.heights.len()).then_some((key / 7) as usize)
    }
    fn rect(&self, row: usize) -> Option<egui::Rect> {
        // node-local: the scroll area's content top is `inner.y - scroll` below the node's top
        (self.width > 0.0 && row < self.heights.len()).then(|| self.heights.rect(row, self.width, self.scroll as f64).translate(egui::vec2(0.0, self.inner.y)))
    }
    fn anchor(&self, key: u64) -> Option<Cow<'_, str>> {
        self.row_of(key).map(|_| Cow::Owned(format!("row[feature:{key}]")))
    }
    fn find_anchor(&self, anchor: &str) -> Option<u64> {
        let k: u64 = anchor.strip_prefix("row[feature:")?.strip_suffix(']')?.parse().ok()?;
        self.row_of(k).map(|_| k)
    }
}

struct Rig {
    h: EmbedHarness,
    tree: NodeId,
    scene_time: Rc<Cell<Duration>>,
}

fn rig(rows: usize) -> Rig {
    let cfg = EmbedConfig { host: HostConfig { echo_console: false, echo_errors: false, quit_on_last_window: false, ..HostConfig::default() }, ..EmbedConfig::default() };
    let mut h = EmbedHarness::new(egui::vec2(320.0, 400.0), cfg).unwrap();
    let e = h.embed.clone();
    let slot = e.create_slot("panel", SlotOptions::fill());
    let scene = e.backend().scene();
    let tree = scene
        .write(ProviderId::named("sg5.test"), move |m| {
            let n = m.create_tag("virtual-tree").unwrap();
            m.insert(NodeId::from_raw(slot.handle()).unwrap(), n, None).unwrap();
            let mut nm = m.node(n).unwrap();
            nm.set_by_name("rows", &svelters_abi::Value::F64(rows as f64)).unwrap();
            nm.set(&keys::widget::FLEX_GROW, 1.0).unwrap();
            nm.set(&keys::widget::ANCHOR, "tree".to_string()).unwrap();
            n
        })
        .unwrap();
    let scene_time = Rc::new(Cell::new(Duration::ZERO));
    let t = scene_time.clone();
    h.set_ui(move |e, ui| {
        let t0 = Instant::now();
        e.show_slot(ui, slot);
        t.set(t0.elapsed());
    });
    h.run();
    Rig { h, tree, scene_time }
}

impl Rig {
    fn rect(&self) -> egui::Rect {
        self.h.embed.backend().scene().read(|s| s.get(self.tree).unwrap().drawn().unwrap())
    }
    fn wheel(&mut self, dy: f32) {
        let c = self.rect().center();
        self.h.harness.event(egui::Event::PointerMoved(c));
        self.h.harness.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, dy),
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Unknown,
            modifiers: egui::Modifiers::NONE,
        });
    }
}

#[test]
fn a_10k_row_painted_tree_scrolls_with_no_js_and_a_small_scene_time() {
    let mut r = rig(10_000);
    let e = r.h.embed.clone();
    let scene = e.backend().scene();
    let nodes = scene.read(|s| s.len());
    let before = e.backend().stats();
    let mut times = Vec::new();
    for _ in 0..120 {
        r.wheel(-150.0);
        r.h.step();
        times.push(r.scene_time.get());
    }
    let after = e.backend().stats();
    assert_eq!((after.checkpoints, after.events, after.changes), (before.checkpoints, before.events, before.changes), "no JS entry while scrolling");
    assert_eq!(scene.read(|s| s.len()), nodes, "no node per row");
    let scroll = scene.read(|s| s.get(r.tree).unwrap().widget(|t: &VirtualTree| t.scroll)).unwrap();
    assert!(scroll > 2000.0, "it scrolled: {scroll}");
    times.sort();
    let median = times[times.len() / 2];
    eprintln!("SG5 painted tree, 10k rows: scene time per frame median {median:?}, max {:?}", times.last().unwrap());
    let budget = if cfg!(debug_assertions) { Duration::from_millis(30) } else { Duration::from_millis(1) };
    assert!(median <= budget, "scene time {median:?} > {budget:?}");
    // a11y: the drawn rows are tree items with set size and position over all 10k
    let first = (scroll / ROW).ceil() as u64 + 1;
    let node = r.h.harness.get_by_label(&format!("Feature {}", first * 7));
    let ak = node.accesskit_node();
    assert_eq!(ak.size_of_set(), Some(10_000));
    assert_eq!(ak.position_in_set(), Some(first as usize + 1));
    // hit-test: the row under the pointer is a virtual target of the tree
    let c = r.rect().center();
    let hit = scene.read(|s| s.hit_test(egui::ViewportId::ROOT, c));
    let Some(Target::Virtual(v)) = hit.first().copied() else { panic!("{hit:?}") };
    assert_eq!(v.host, r.tree);
    assert_eq!(v.key % 7, 0);
    let (_, rr) = scene.read(|s| s.target_rect(Target::Virtual(v))).unwrap();
    assert!(rr.contains(c), "the hit row's rect {rr:?} contains the point {c:?}");
}

#[test]
fn a_1m_row_tree_resolves_anchors_and_queries_off_screen() {
    let mut r = rig(1_000_000);
    let e = r.h.embed.clone();
    let scene = e.backend().scene();
    let root = scene.read(|s| s.get(r.tree).unwrap().root());
    // an off-screen row near the end
    let t = scene.read(|s| s.target_by_anchor(root, "tree/row[feature:6999993]"));
    assert_eq!(t, Some(Target::Virtual(VNodeId { host: r.tree, key: 6_999_993 })));
    assert_eq!(scene.read(|s| s.target_anchor(t.unwrap())).as_deref(), Some("tree/row[feature:6999993]"));
    assert!(scene.read(|s| s.target_by_anchor(root, "tree/row[feature:6999994]")).is_none(), "not a key");
    let (_, rect) = scene.read(|s| s.target_rect(t.unwrap())).unwrap();
    assert!(rect.min.y > 1.9e7, "far below: {rect:?}");
    // the query sees every row (lazily: count only)
    assert_eq!(scene.read(|s| s.query().tag("virtual-tree").include_virtual().targets().count()), 1_000_001);
    // scrolling stays cheap at 1M rows
    for _ in 0..20 {
        r.wheel(-2000.0);
        r.h.step();
    }
    let scroll = scene.read(|s| s.get(r.tree).unwrap().widget(|t: &VirtualTree| t.scroll)).unwrap();
    assert!(scroll > 1000.0);
}
