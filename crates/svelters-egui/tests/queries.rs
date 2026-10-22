//! SG3 (plans/research/ARCH-SCENE.md §6.3, §7.4, §14; R13, R14, R20): queries and hit-testing on a
//! running embed.
//!
//! - Two roots in two viewports (the root viewport and a deferred one, through the dock adapter
//!   as in `tests/viewports.rs`): queries by anchor, id path, kind, ext, state and change tick
//!   (R14); hit-tests in each viewport; R20 after `set_viewport`; a hidden root is not hit.
//! - Rust-built content in two overlapping slots: the topmost node comes first, `HIT_TRANSPARENT`
//!   (on the node or an ancestor) lets the point through, a higher egui layer order wins over draw
//!   order.
//!
//! The bundle is `fixtures/embed-egui/dist/embed.js` (`pnpm build`).

use egui::{ViewportBuilder, ViewportId};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use svelters_egui::embed::{Embed, EmbedConfig, EntryId, Props, RootId, SlotId, SlotOptions};
use svelters_egui::harness::dock::{PaneEvent, SvelteDock};
use svelters_egui::harness::{EmbedHarness, Viewports};
use svelters_egui::scene::{ChangeFilter, ChangeMask, ExtKey, NodeFlags, NodeId, ProviderId, RootChange, RootIx, SceneHandle, StateBits, Target, keys};
use svelters_host::HostConfig;

static MARK: ExtKey<&'static str> = ExtKey::new("sg3.mark");

fn bundle() -> PathBuf {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/embed-egui/dist/embed.js");
    assert!(p.exists(), "{} is missing: run `pnpm build` (needs `cargo build -p svelters-compiler` first)", p.display());
    p
}

fn cfg() -> EmbedConfig {
    EmbedConfig { host: HostConfig { echo_console: false, echo_errors: false, quit_on_last_window: false, ..HostConfig::default() }, ..EmbedConfig::default() }
}

fn vp(n: u32) -> ViewportId {
    ViewportId::from_hash_of(("window", n))
}

type Panes = Arc<Mutex<HashMap<ViewportId, Vec<String>>>>;

fn draw_panes(ui: &mut egui::Ui, panes: &Panes) {
    let v = ui.ctx().viewport_id();
    let list = panes.lock().unwrap().get(&v).cloned().unwrap_or_default();
    if !list.is_empty() {
        ui.columns(list.len(), |cols| {
            for (i, p) in list.iter().enumerate() {
                SvelteDock::pane_ui(&mut cols[i], p);
            }
        });
    }
    SvelteDock::viewport_done(ui);
}

/// A host app with a dock: the root viewport and deferred windows 2 and 3.
struct Rig {
    vps: Viewports,
    dock: Rc<RefCell<SvelteDock>>,
    events: Rc<RefCell<Vec<PaneEvent>>>,
    panes: Panes,
    entry: EntryId,
}

impl Rig {
    fn new() -> Rig {
        let mut vps = Viewports::new(egui::vec2(400.0, 300.0));
        let embed = Embed::new(&vps.ctx, cfg()).unwrap();
        let entry = embed.load(bundle()).unwrap();
        let dock = Rc::new(RefCell::new(SvelteDock::new(embed.clone())));
        let events: Rc<RefCell<Vec<PaneEvent>>> = Rc::default();
        let panes: Panes = Arc::default();
        let (d, ev, l) = (dock.clone(), events.clone(), panes.clone());
        vps.set_ui(move |ui| {
            for e in ev.borrow_mut().drain(..) {
                d.borrow_mut().on_event(&e).unwrap();
            }
            embed.logic(ui.ctx());
            draw_panes(ui, &l);
            for n in [2, 3] {
                let l = l.clone();
                ui.ctx().show_viewport_deferred(vp(n), ViewportBuilder::default(), move |ui, _| draw_panes(ui, &l));
            }
        });
        Rig { vps, dock, events, panes, entry }
    }

    fn embed(&self) -> Embed {
        self.dock.borrow().embed.clone()
    }

    fn scene(&self) -> SceneHandle {
        self.embed().backend().scene()
    }

    fn open(&mut self, pane: &str, viewport: ViewportId, props: Props) {
        self.dock.borrow_mut().open(pane, self.entry, "Panel", props);
        self.panes.lock().unwrap().entry(viewport).or_default().push(pane.to_string());
        self.events.borrow_mut().push(PaneEvent::Shown { pane: pane.to_string(), viewport });
        self.vps.request(ViewportId::ROOT);
    }

    fn root(&self, pane: &str) -> RootId {
        self.dock.borrow().root(pane).unwrap()
    }

    fn slot(&self, pane: &str) -> SlotId {
        self.embed().slot(pane).unwrap()
    }

    /// The pane's scene root.
    fn rix(&self, pane: &str) -> RootIx {
        let h = self.slot(pane).handle();
        self.scene().read(|s| s.get_raw(h).unwrap().root())
    }

    /// The centre of `anchor` in `pane` (last frame) and its viewport.
    fn at(&self, pane: &str, anchor: &str) -> (ViewportId, egui::Pos2) {
        let (v, r) = self.embed().anchor_rect(self.root(pane), anchor).unwrap_or_else(|| panic!("no anchor {anchor} in {pane}"));
        (v, r.center())
    }

    fn settle(&mut self) {
        self.vps.run(200);
        assert!(self.vps.due().is_empty());
    }
}

/// The nodes `hit_test` reports, topmost first.
fn hits(scene: &SceneHandle, v: ViewportId, pos: egui::Pos2) -> Vec<NodeId> {
    scene.read(|s| s.hit_test(v, pos)).into_iter().map(|t| match t {
        Target::Node(n) => n,
        Target::Virtual(v) => panic!("no provider parts here: {v:?}"),
    })
    .collect()
}

/// §14 SG3 acceptance: query by anchor, kind, ext and state across two roots and two viewports;
/// hit-test in each viewport; R14 `changed_since`; R20 after `set_viewport`.
#[test]
fn queries_and_hit_tests_across_two_roots_and_two_viewports() {
    let mut r = Rig::new();
    let items = Props::json(serde_json::json!({ "title": "A", "items": ["x", "y"] }));
    r.open("a", ViewportId::ROOT, items);
    r.open("b", vp(2), Props::json(serde_json::json!({ "title": "B", "items": ["x", "y", "z"] })));
    r.settle();
    let (ra, rb) = (r.rix("a"), r.rix("b"));
    assert_ne!(ra, rb);
    let scene = r.scene();
    let app = ProviderId::named("sg3.test");

    // anchors and id paths, per root
    let (head_a, head_b, x_a, x_b) = scene.read(|s| {
        let head_a = s.by_anchor(ra, "heading").unwrap();
        let head_b = s.by_anchor(rb, "heading").unwrap();
        assert_eq!(head_a.text(), "A: 0");
        assert_eq!(head_b.text(), "B: 0");
        assert_eq!(s.by_path("b/heading").map(|n| n.id()), Some(head_b.id()), "root key + anchor");
        assert_eq!(s.by_path("a/item-y").map(|n| n.text()), Some("A item y"));
        assert!(s.by_path("a/item-z").is_none() && s.by_path("nope/heading").is_none());
        assert_eq!(s.root_by_key("b"), Some(rb));
        assert_eq!(s.root_viewport(rb), Some(vp(2)), "set_viewport reached the scene's root entry");
        (head_a.id(), head_b.id(), s.by_anchor(ra, "item-x").unwrap().id(), s.by_anchor(rb, "item-x").unwrap().id())
    });
    assert_eq!(r.embed().anchor_id(r.root("b"), "item-z"), scene.read(|s| s.by_anchor(rb, "item-z").map(|n| n.egui_id())), "Embed::anchor_id uses the anchor map");

    // kind, scope
    scene.read(|s| {
        // heading, clicks, visible + the items
        assert_eq!(s.query().tag("label").within(ra).count(), 3 + 2);
        assert_eq!(s.query().tag("label").within(rb).count(), 3 + 3);
        assert_eq!(s.query().tag("button").count(), 2);
        assert_eq!(s.query().tag("label").within(rb).filter(|n| n.anchor().is_some_and(|a| a.starts_with("item-"))).count(), 3);
    });

    // ext and state from Rust, across both roots; R14: what changed since a tick
    let t = scene.read(|s| s.tick());
    scene
        .write(app, |m| {
            m.node(x_a).unwrap().ext_mut(&MARK).set("x of a");
            m.node(x_b).unwrap().ext_mut(&MARK).set("x of b");
            m.node(head_b).unwrap().set_state(StateBits::SELECTED, true);
        })
        .unwrap();
    scene.read(|s| {
        let mut both = s.query().has(&MARK).ids();
        both.sort();
        let mut want = vec![x_a, x_b];
        want.sort();
        assert_eq!(both, want);
        assert_eq!(s.query().has(&MARK).within(rb).ids(), vec![x_b]);
        assert_eq!(s.query().ext(&MARK, |v| v.ends_with('a')).ids(), vec![x_a]);
        assert_eq!(s.query().state(StateBits::SELECTED).tag("label").ids(), vec![head_b]);
        assert_eq!(s.query().state(StateBits::SELECTED).within(ra).count(), 0);
        let mut changed = s.query().changed_since(t).ids();
        changed.sort();
        let mut want = vec![x_a, x_b, head_b];
        want.sort();
        assert_eq!(changed, want, "R14");
        assert_eq!(s.query().changed_since(t).within(ra).has(&MARK).ids(), vec![x_a]);
    });
    // a JS change is a change too
    let t2 = scene.read(|s| s.tick());
    r.embed().set_root_props(r.root("a"), Props::new().with("n", 5)).unwrap();
    r.settle();
    scene.read(|s| {
        assert_eq!(s.get(head_a).unwrap().text(), "A: 5");
        assert_eq!(s.query().changed_since(t2).tag("label").within(ra).ids(), vec![head_a]);
        assert_eq!(s.query().changed_since(t2).within(rb).count(), 0);
    });

    // hit tests, per viewport: the topmost node under the point
    let (va, pa) = r.at("a", "heading");
    let (vb, pb) = r.at("b", "item-x");
    assert_eq!((va, vb), (ViewportId::ROOT, vp(2)));
    assert_eq!(hits(&scene, ViewportId::ROOT, pa), vec![head_a]);
    assert_eq!(hits(&scene, vp(2), pb), vec![x_b]);
    assert!(!hits(&scene, vp(2), pa).contains(&head_a), "viewports have their own lists");
    assert!(hits(&scene, vp(3), pb).is_empty(), "nothing drawn in window 3");
    assert_eq!(scene.read(|s| s.hit_node(vp(2), pb).map(|n| n.text().to_string())), Some("B item x".into()));

    // R20: right after set_viewport (window 2 has not drawn again), b's entries in window 2's list
    // are skipped
    let obs = scene.observe(ChangeFilter::only(ChangeMask::ROOT).scope(rb));
    r.embed().set_viewport(r.slot("b"), vp(3));
    assert!(hits(&scene, vp(2), pb).is_empty(), "R20");
    assert!(obs.take().records.iter().any(|c| matches!(c, svelters_egui::scene::Change::Root { what: RootChange::Viewport(v), .. } if *v == vp(3))), "journaled");
    // the dock lays it out in window 3; it is hit there after window 3 drew it
    {
        let mut p = r.panes.lock().unwrap();
        p.get_mut(&vp(2)).unwrap().retain(|x| x != "b");
        p.entry(vp(3)).or_default().push("b".into());
    }
    r.vps.request(vp(2));
    r.vps.request(vp(3));
    r.settle();
    let (v3, p3) = r.at("b", "item-x");
    assert_eq!(v3, vp(3));
    assert_eq!(hits(&scene, vp(3), p3), vec![x_b]);

    // a hidden root is not hit, and its nodes are not visible to queries
    r.embed().set_visible(r.slot("a"), false);
    assert!(hits(&scene, ViewportId::ROOT, pa).is_empty());
    scene.read(|s| {
        assert!(!s.root_visible(ra));
        assert_eq!(s.query().tag("label").within(ra).visible().count(), 0);
        assert_eq!(s.query().tag("label").within(rb).visible().count(), 6);
    });
    r.embed().set_visible(r.slot("a"), true);
    assert_eq!(hits(&scene, ViewportId::ROOT, pa), vec![head_a], "shown again: last pass's entries count");
}

/// Two slots drawn over each other, with Rust-built buttons: the topmost comes first;
/// `HIT_TRANSPARENT` on the node or an ancestor lets the point through; a slot drawn first in a
/// foreground `Area` is on top of one drawn later in the background.
#[test]
fn hit_test_returns_the_topmost_and_honours_hit_transparent() {
    let mut h = EmbedHarness::new(egui::vec2(300.0, 200.0), cfg()).unwrap();
    let e = h.embed.clone();
    let under = e.create_slot("under", SlotOptions::fill());
    let over = e.create_slot("over", SlotOptions::fill());
    let scene = e.backend().scene();
    let app = ProviderId::named("sg3.test");
    let button = |slot: SlotId, text: &str| {
        let text = text.to_string();
        scene
            .write(app, move |m| {
                let b = m.create_tag("button").unwrap();
                m.node(b).unwrap().set(&keys::button::TEXT, text).unwrap();
                m.insert(NodeId::from_raw(slot.handle()).unwrap(), b, None).unwrap();
                b
            })
            .unwrap()
    };
    let (b_under, b_over) = (button(under, "under"), button(over, "over"));
    let foreground = Rc::new(Cell::new(false));
    let fg = foreground.clone();
    h.set_ui(move |e, ui| {
        let r = ui.max_rect();
        if fg.get() {
            egui::Area::new(egui::Id::unique("sg3-fg")).order(egui::Order::Foreground).fixed_pos(r.min).show(ui.ctx(), |ui| {
                ui.set_min_size(r.size());
                e.show_slot(ui, under);
            });
        } else {
            ui.scope_builder(egui::UiBuilder::new().max_rect(r), |ui| e.show_slot(ui, under));
        }
        ui.scope_builder(egui::UiBuilder::new().max_rect(r), |ui| e.show_slot(ui, over));
    });
    h.run();
    let pos = scene.read(|s| s.get(b_over).unwrap().drawn().unwrap().left_top() + egui::vec2(4.0, 4.0));
    assert!(scene.read(|s| s.get(b_under).unwrap().drawn().unwrap().contains(pos)), "the buttons overlap");
    let top = ViewportId::ROOT;
    assert_eq!(hits(&scene, top, pos), vec![b_over, b_under], "drawn later, on top");
    assert_eq!(scene.read(|s| s.hit_node(top, pos).map(|n| n.id())), Some(b_over));
    assert!(hits(&scene, top, egui::pos2(-5.0, -5.0)).is_empty());

    // HIT_TRANSPARENT on the node, then on an ancestor (the slot's box)
    let flag = |n: NodeId, on: bool| scene.write(app, |m| m.node(n).unwrap().set_flags(NodeFlags::HIT_TRANSPARENT, on)).unwrap();
    flag(b_over, true);
    assert_eq!(hits(&scene, top, pos), vec![b_under]);
    flag(b_over, false);
    let over_box = NodeId::from_raw(over.handle()).unwrap();
    flag(over_box, true);
    assert_eq!(hits(&scene, top, pos), vec![b_under], "pointer-events: none is inherited");
    flag(over_box, false);
    assert_eq!(hits(&scene, top, pos), vec![b_over, b_under]);

    // a higher layer order beats draw order
    foreground.set(true);
    h.run();
    // (the area is placed without the panel's margin: take a point inside both buttons)
    let pos = scene.read(|s| s.get(b_under).unwrap().drawn().unwrap().intersect(s.get(b_over).unwrap().drawn().unwrap()).center());
    assert_eq!(hits(&scene, top, pos), vec![b_under, b_over], "the foreground area is on top");
    scene.read(|s| assert!(s.stats().hit_entries >= 2));
}
