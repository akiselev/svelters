//! EM2 (plans/research/ARCH-EMBED.md §5.2, §5.3, §7.1, §10): slots in several viewports. Deferred
//! viewports run headless in `harness::Viewports` (per-viewport pass counts); panes are driven
//! through the dock-adapter helper `harness::dock::SvelteDock`, as cadatomic's berth adapter does.
//! The bundle is `fixtures/embed-egui/dist/embed.js` (`pnpm build`).

use egui::{ViewportBuilder, ViewportId};
use egui_kittest::kittest::Queryable;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use svelters_egui::embed::{
    Embed, EmbedConfig, EmbedEvent, EntryId, MountSpec, Props, RootId, SlotOptions, SlotWarning,
};
use svelters_egui::harness::dock::{CloseReason, PaneEvent, SvelteDock};
use svelters_egui::harness::{EmbedHarness, Viewports};
use svelters_host::HostConfig;

fn bundle() -> PathBuf {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/embed-egui/dist/embed.js");
    assert!(
        p.exists(),
        "{} is missing: run `pnpm build` (needs `cargo build -p svelters-compiler` first)",
        p.display()
    );
    p
}

fn cfg(prepass_plugin: bool) -> EmbedConfig {
    EmbedConfig {
        host: HostConfig {
            echo_console: false,
            echo_errors: false,
            quit_on_last_window: false,
            ..HostConfig::default()
        },
        prepass_plugin,
        ..EmbedConfig::default()
    }
}

fn vp(n: u32) -> ViewportId {
    ViewportId::from_hash_of(("window", n))
}

/// Which panes each viewport shows (the dock's layout), shared with the deferred closures, and
/// what each viewport saw focused after drawing its panes.
#[derive(Default)]
struct Layout {
    panes: HashMap<ViewportId, Vec<String>>,
    focused: HashMap<ViewportId, Option<egui::Id>>,
}

type Shared = Arc<Mutex<Layout>>;

/// Draws the panes of `ui`'s viewport side by side (berth's `pane_ui` per pane).
fn draw_panes(ui: &mut egui::Ui, layout: &Shared) {
    let v = ui.ctx().viewport_id();
    let panes = layout
        .lock()
        .unwrap()
        .panes
        .get(&v)
        .cloned()
        .unwrap_or_default();
    if !panes.is_empty() {
        ui.columns(panes.len(), |cols| {
            for (i, p) in panes.iter().enumerate() {
                SvelteDock::pane_ui(&mut cols[i], p);
            }
        });
    }
    SvelteDock::viewport_done(ui);
    let f = ui.memory(|m| m.focused());
    layout.lock().unwrap().focused.insert(v, f);
}

/// A host app: `App::logic` = dock events, then `embed.logic`; `App::ui` = the root viewport's
/// panes, and windows 2 and 3 as deferred viewports whose closures capture only the layout.
struct Rig {
    vps: Viewports,
    dock: Rc<RefCell<SvelteDock>>,
    events: Rc<RefCell<Vec<PaneEvent>>>,
    layout: Shared,
    entry: EntryId,
}

use std::cell::RefCell;
use std::rc::Rc;

fn rig(prepass_plugin: bool) -> Rig {
    let mut vps = Viewports::new(egui::vec2(400.0, 300.0));
    let embed = Embed::new(&vps.ctx, cfg(prepass_plugin)).unwrap();
    let entry = embed.load(bundle()).unwrap();
    let dock = Rc::new(RefCell::new(SvelteDock::new(embed.clone())));
    let events: Rc<RefCell<Vec<PaneEvent>>> = Rc::default();
    let layout: Shared = Arc::default();
    let (d, ev, l) = (dock.clone(), events.clone(), layout.clone());
    vps.set_ui(move |ui| {
        // App::logic
        for e in ev.borrow_mut().drain(..) {
            d.borrow_mut().on_event(&e).unwrap();
        }
        embed.logic(ui.ctx());
        // App::ui
        draw_panes(ui, &l);
        for n in [2, 3] {
            let l = l.clone();
            ui.ctx().show_viewport_deferred(
                vp(n),
                ViewportBuilder::default().with_title(format!("window {n}")),
                move |ui, _| draw_panes(ui, &l),
            );
        }
    });
    Rig {
        vps,
        dock,
        events,
        layout,
        entry,
    }
}

impl Rig {
    fn embed(&self) -> Embed {
        self.dock.borrow().embed.clone()
    }

    /// The dock opens `pane` in `viewport` (berth: the pane is laid out there, `Shown` follows).
    fn open(&mut self, pane: &str, export: &str, viewport: ViewportId, props: Props) {
        self.dock.borrow_mut().open(pane, self.entry, export, props);
        self.layout
            .lock()
            .unwrap()
            .panes
            .entry(viewport)
            .or_default()
            .push(pane.to_string());
        self.events.borrow_mut().push(PaneEvent::Shown {
            pane: pane.to_string(),
            viewport,
        });
        self.vps.request(ViewportId::ROOT);
    }

    /// berth moves `pane` to another window: its layout changes, the pane event follows.
    fn move_pane(&mut self, pane: &str, from: ViewportId, to: ViewportId) {
        {
            let mut l = self.layout.lock().unwrap();
            l.panes.get_mut(&from).unwrap().retain(|p| p != pane);
            l.panes.entry(to).or_default().push(pane.to_string());
        }
        self.events.borrow_mut().push(PaneEvent::MovedToViewport {
            pane: pane.to_string(),
            from,
            to,
        });
        // berth repaints the windows whose layout changed
        self.vps.request(ViewportId::ROOT);
        self.vps.request(from);
        self.vps.request(to);
    }

    fn root(&self, pane: &str) -> RootId {
        self.dock.borrow().root(pane).unwrap()
    }

    fn title(&self, pane: &str) -> String {
        self.embed().root_info(self.root(pane)).unwrap().title
    }

    /// The screen centre of `anchor` in `pane`'s root (last frame's rect) and its viewport.
    fn at(&self, pane: &str, anchor: &str) -> (ViewportId, egui::Pos2) {
        let (v, r) = self
            .embed()
            .anchor_rect(self.root(pane), anchor)
            .unwrap_or_else(|| panic!("no anchor {anchor} in {pane}"));
        (v, r.center())
    }

    fn settle(&mut self) {
        self.vps.run(200);
        assert!(self.vps.due().is_empty());
    }
}

/// Every viewport is idle: nothing due, no delayed repaint, the embed settled; and more steps run
/// no pass anywhere.
fn assert_idle(r: &mut Rig) {
    assert!(r.vps.due().is_empty(), "due: {:?}", r.vps.due());
    assert!(r.vps.delayed().is_empty(), "delayed: {:?}", r.vps.delayed());
    assert!(
        r.embed().settled().is_settled(),
        "{:?}",
        r.embed().settled()
    );
    r.vps.reset_passes();
    for _ in 0..20 {
        assert!(!r.vps.step(), "a pass ran while idle");
    }
    for v in r.vps.ids() {
        assert_eq!(r.vps.passes(v), 0, "{v:?}");
    }
}

fn three_panes(prepass_plugin: bool) -> Rig {
    let mut r = rig(prepass_plugin);
    r.open(
        "a",
        "Pane",
        ViewportId::ROOT,
        Props::new().with("name", "a"),
    );
    r.open("b", "Pane", vp(2), Props::new().with("name", "b"));
    r.open("c", "Pane", vp(3), Props::new().with("name", "c"));
    r.settle();
    assert_eq!(
        r.vps.ids().len(),
        3,
        "the root viewport and two deferred ones"
    );
    for p in ["a", "b", "c"] {
        assert_eq!(r.title(p), format!("{p}: 0"));
    }
    r
}

#[test]
fn dock_w2_a_change_in_window_2_repaints_only_window_2() {
    let mut r = three_panes(true);
    assert_idle(&mut r);
    let (v, pos) = r.at("b", "inc");
    assert_eq!(v, vp(2));
    r.vps.reset_passes();
    r.vps.click(vp(2), pos);
    r.settle();
    assert_eq!(
        r.title("b"),
        "b: 1",
        "the click in window 2 ran its handler"
    );
    assert_eq!(
        (r.title("a"), r.title("c")),
        ("a: 0".to_string(), "c: 0".to_string())
    );
    assert!(r.vps.passes(vp(2)) >= 3, "window 2 ran its input passes");
    assert_eq!(
        r.vps.passes(ViewportId::ROOT),
        0,
        "no root-viewport frame (the prepass plugin ran logic in window 2)"
    );
    assert_eq!(r.vps.passes(vp(3)), 0, "window 3 untouched");
    assert_idle(&mut r);

    // a change made from the root viewport (props) repaints window 3 only, besides the root's own
    // logic frame
    let c = r.root("c");
    r.embed()
        .set_root_props(c, Props::new().with("name", "c2"))
        .unwrap();
    r.vps.reset_passes();
    r.settle();
    assert_eq!(r.title("c"), "c2: 0");
    assert_eq!(r.vps.passes(vp(2)), 0, "window 2 untouched");
    assert!(r.vps.passes(vp(3)) >= 1, "window 3 drew the change");
    assert_idle(&mut r);
}

#[test]
fn dock_w2_without_the_prepass_plugin_input_costs_one_root_frame() {
    // cadatomic's seam 4: the host owns the per-frame order, logic runs in the root viewport only
    let mut r = three_panes(false);
    assert_idle(&mut r);
    let (_, pos) = r.at("b", "inc");
    r.vps.reset_passes();
    r.vps.click(vp(2), pos);
    r.settle();
    assert_eq!(r.title("b"), "b: 1");
    let root = r.vps.passes(ViewportId::ROOT);
    assert!(
        (1..=2).contains(&root),
        "one root frame (egui repaints twice per request): {root}"
    );
    assert_eq!(r.vps.passes(vp(3)), 0, "window 3 untouched");
    assert_idle(&mut r);
}

#[test]
fn dock_w4_a_root_moved_between_viewports_keeps_caret_focus_and_instances() {
    let mut r = rig(true);
    r.open("e", "Editor", ViewportId::ROOT, Props::new());
    r.settle();
    let e = r.embed();
    let root = r.root("e");
    let mounts = e.stats().mounts;
    let inits = |evs: &[EmbedEvent]| {
        evs.iter()
            .filter(|x| matches!(x, EmbedEvent::Emitted { name, .. } if name == "init"))
            .count()
    };
    assert_eq!(inits(&e.take_events()), 1);
    let edit = e.anchor_id(root, "edit").unwrap();

    // focus the field in the root viewport and type
    let (v, pos) = r.at("e", "edit");
    assert_eq!(v, ViewportId::ROOT);
    r.vps.click(v, pos);
    r.vps.type_text(v, "abc");
    r.settle();
    assert_eq!(r.title("e"), "abc");
    assert_eq!(
        r.layout.lock().unwrap().focused[&ViewportId::ROOT],
        Some(edit)
    );
    let caret = egui::TextEdit::load_state(&r.vps.ctx, edit)
        .and_then(|s| s.cursor.char_range())
        .unwrap();
    assert_eq!(caret.primary.index, egui::text::CharIndex(3));

    // berth moves the pane to window 2 (a deferred viewport)
    r.move_pane("e", ViewportId::ROOT, vp(2));
    r.settle();
    assert_eq!(e.root_info(root).unwrap().viewport, vp(2));
    assert_eq!(e.anchor_id(root, "edit"), Some(edit), "same egui id");
    assert_eq!(
        r.layout.lock().unwrap().focused[&vp(2)],
        Some(edit),
        "focus restored in window 2"
    );
    let caret2 = egui::TextEdit::load_state(&r.vps.ctx, edit)
        .and_then(|s| s.cursor.char_range())
        .unwrap();
    assert_eq!(caret2, caret, "caret kept");
    assert_eq!(e.stats().mounts, mounts, "no remount");
    assert_eq!(inits(&e.take_events()), 0, "no new component instance");
    // root().viewport reached JS (the layout event)
    assert!(e.backend().with_scene(|s| s.handles().iter().any(|&h| matches!(&s.n(h).w, svelters_egui::scene::W::Label { text, .. } if text.starts_with("in ") && text.contains("focused true") && !text.contains("in root")))));

    // typing in window 2 goes on where the caret was
    r.vps.type_text(vp(2), "d");
    r.settle();
    assert_eq!(r.title("e"), "abcd");
    assert_idle(&mut r);

    // and back: to window 3
    r.move_pane("e", vp(2), vp(3));
    r.settle();
    r.vps.type_text(vp(3), "e");
    r.settle();
    assert_eq!(r.title("e"), "abcde");
    assert_eq!(e.stats().mounts, mounts);
    assert!(e.take_warnings().is_empty(), "{:?}", e.take_warnings());
}

#[test]
fn dock_close_flow_and_lazy_mount() {
    let mut r = rig(true);
    // opened but never shown: not mounted
    r.dock
        .borrow_mut()
        .open("late", r.entry, "Pane", Props::new().with("name", "late"));
    r.settle();
    assert!(
        r.dock.borrow().root("late").is_none(),
        "lazy: mounted at the first Shown"
    );
    r.open(
        "p",
        "Pane",
        ViewportId::ROOT,
        Props::new().with("name", "p"),
    );
    r.settle();
    let root = r.root("p");
    // the tab's close button: ask the root, then (allowed) berth closes it
    assert_eq!(r.dock.borrow().request_close("p"), Some(root));
    r.vps.request(ViewportId::ROOT);
    r.settle();
    let allow = r
        .embed()
        .take_events()
        .iter()
        .any(|e| matches!(e, EmbedEvent::CloseDecided { root: x, allow: true } if *x == root));
    assert!(allow);
    r.layout
        .lock()
        .unwrap()
        .panes
        .get_mut(&ViewportId::ROOT)
        .unwrap()
        .retain(|p| p != "p");
    r.events.borrow_mut().push(PaneEvent::Closed {
        pane: "p".into(),
        reason: CloseReason::Api,
    });
    r.vps.request(ViewportId::ROOT);
    r.settle();
    assert!(r.embed().slot("p").is_none() && r.embed().root_info(root).is_none());
    assert_idle(&mut r);
}

#[test]
fn a_hidden_roots_animation_frames_do_not_run() {
    let mut eh = EmbedHarness::new(egui::vec2(400.0, 300.0), cfg(true)).unwrap();
    let e = eh.embed.clone();
    let entry = e.load(bundle()).unwrap();
    let slot = e.create_slot("anim", SlotOptions::fill());
    eh.set_ui(move |e, ui| {
        e.show_slot(ui, slot);
    });
    let root = e
        .mount(MountSpec::new(entry, "Animator", slot, "anim"))
        .unwrap();
    eh.run();
    // a loop started from an event handler (requestAnimationFrame in a handler: the root's queue)
    eh.harness.get_by_label("Animate").click();
    for _ in 0..5 {
        eh.step();
    }
    let ticks = |e: &Embed| {
        e.root_info(root)
            .unwrap()
            .title
            .trim_start_matches("ticks ")
            .parse::<u32>()
            .unwrap()
    };
    let running = ticks(&e);
    assert!(running >= 2, "the loop runs while shown: {running}");
    e.set_visible(slot, false);
    eh.step(); // the next logic sends visible = false
    let hidden = ticks(&e);
    for _ in 0..10 {
        eh.step();
    }
    assert_eq!(
        ticks(&e),
        hidden,
        "a hidden root's frame callbacks do not run"
    );
    assert!(
        eh.run_until_settled(5).is_ok(),
        "hidden: settled, no frame pending"
    );
    e.set_visible(slot, true);
    for _ in 0..5 {
        eh.step();
    }
    assert!(ticks(&e) > hidden, "shown again: the loop resumes");
    e.unmount(root);
    eh.run();
}

#[test]
fn move_root_moves_a_root_into_another_slot_without_a_remount() {
    let mut eh = EmbedHarness::new(egui::vec2(800.0, 400.0), cfg(true)).unwrap();
    let e = eh.embed.clone();
    let entry = e.load(bundle()).unwrap();
    let left = e.create_slot("left", SlotOptions::fill());
    let right = e.create_slot("right", SlotOptions::fill());
    eh.set_ui(move |e, ui| {
        ui.columns(2, |cols| {
            e.show_slot(&mut cols[0], left);
            e.show_slot(&mut cols[1], right);
        });
    });
    let root = e
        .mount(
            MountSpec::new(entry, "Panel", left, "panel")
                .props(Props::new().with("title", "P").with("items", vec![1])),
        )
        .unwrap();
    eh.run();
    let id = e.anchor_id(root, "heading").unwrap();
    let mounts = e.stats().mounts;
    assert!(eh.harness.get_by_label("P: 0").rect().max.x <= 400.0);
    eh.harness.get_by_label("Inc P").click();
    eh.run();
    assert!(eh.harness.query_by_label("P clicks: 1").is_some());

    assert!(
        e.move_root(root, left).is_ok(),
        "into its own slot: nothing to do"
    );
    e.move_root(root, right).unwrap();
    eh.run();
    assert_eq!(e.root_info(root).unwrap().slot, right);
    assert!(
        eh.harness.get_by_label("P: 0").rect().min.x >= 400.0,
        "drawn in the right slot"
    );
    assert!(
        eh.harness.query_by_label("P clicks: 1").is_some(),
        "local state kept"
    );
    assert_eq!(e.anchor_id(root, "heading"), Some(id), "same egui ids");
    assert_eq!(e.stats().mounts, mounts, "no remount");
    assert_eq!(e.stats().moves, 1);
    // new nodes go to the new slot (the runtime's target moved too)
    e.set_root_props(root, Props::new().with("items", vec![1, 2]))
        .unwrap();
    eh.run();
    assert!(eh.harness.get_by_label("P item 2").rect().min.x >= 400.0);
    // the old slot is empty and takes another root
    let other = e
        .mount(MountSpec::new(entry, "Panel", left, "other").props(Props::new().with("title", "O")))
        .unwrap();
    eh.run();
    assert!(eh.harness.get_by_label("O: 0").rect().max.x <= 400.0);
    assert!(e.move_root(other, right).is_err(), "an occupied slot");
    e.unmount(root);
    e.unmount(other);
    eh.run();
}

#[test]
fn debug_checks_flag_hidden_draws_undrawn_slots_and_wrong_viewports() {
    let mut eh = EmbedHarness::new(egui::vec2(400.0, 300.0), cfg(true)).unwrap();
    let e = eh.embed.clone();
    let entry = e.load(bundle()).unwrap();
    let shown = e.create_slot("shown", SlotOptions::fill());
    let forgotten = e.create_slot("forgotten", SlotOptions::fill());
    e.mount(MountSpec::new(entry, "Pane", shown, "shown"))
        .unwrap();
    e.mount(MountSpec::new(entry, "Pane", forgotten, "forgotten"))
        .unwrap();
    e.set_visible(shown, false);
    eh.set_ui(move |e, ui| {
        e.show_slot(ui, shown); // drawn although hidden
    });
    for _ in 0..61 {
        eh.step();
    }
    let w = e.take_warnings();
    assert!(
        w.iter()
            .any(|x| matches!(x, SlotWarning::DrawnWhileHidden { slot, .. } if *slot == shown)),
        "{w:?}"
    );
    assert!(
        w.iter().any(
            |x| matches!(x, SlotWarning::NotDrawn { slot, passes: 60, .. } if *slot == forgotten)
        ),
        "{w:?}"
    );
    assert_eq!(w.len(), 2, "each once: {w:?}");
    e.set_visible(shown, true);
    e.set_viewport(shown, vp(9)); // but drawn in the root viewport
    eh.step();
    let w = e.take_warnings();
    assert!(w.iter().any(|x| matches!(x, SlotWarning::WrongViewport { slot, set, drawn, .. } if *slot == shown && *set == vp(9) && *drawn == ViewportId::ROOT)), "{w:?}");
}
