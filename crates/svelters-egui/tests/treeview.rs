//! WG5b: `<TreeView>` (@svelters/lists) on egui's `tree-view` (`src/kinds/treeview.rs`), the
//! gallery's `Tree` page (`fixtures/gallery-egui/src/pages/Tree.svelte`, through `TreeWindow.svelte`)
//! compiled live under kittest: lazy children on expand (a Promise), the keyboard (arrows,
//! Right/Left, Shift, type-ahead, Enter, Ctrl+C), multi-selection by key across a filter (SV46),
//! a11y roles and levels, a row dragged onto another, and a 100k-node subtree that binds only
//! the rows in view.

use egui::accesskit::Role;
use egui::{Key, Modifiers};
use egui_kittest::kittest::{NodeT, Queryable};
use std::sync::Arc;
use svelters_egui::harness::TestApp;
use svelters_egui::scene::NodeId;
use svelters_host::HostConfig;
use svelters_host::platform::{ClipboardKind, MemoryClipboard, Platform, RecordingShell};

fn start() -> (TestApp, Arc<MemoryClipboard>, NodeId) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/gallery-egui/src/TreeWindow.svelte");
    let clip = Arc::new(MemoryClipboard::new());
    let cfg = HostConfig { echo_console: false, echo_errors: false, platform: Platform::new(clip.clone(), Arc::new(RecordingShell::new())), ..HostConfig::default() };
    let mut app = TestApp::from_source_with(path, 1.0 / 60.0, cfg).unwrap();
    app.run();
    assert!(app.errors().is_empty(), "{:?}", app.errors());
    let tree = NodeId::from_raw(app.backend.handles_of("tree-view")[0]).unwrap();
    (app, clip, tree)
}

fn has(app: &TestApp, label: &str) -> bool {
    app.harness.query_by_label(label).is_some()
}

/// The screen rect of the shown row whose first label is `text`, across the whole tree.
fn row(app: &TestApp, tree: NodeId, text: &str) -> egui::Rect {
    app.backend.scene().read(|s| {
        let list = s.get(tree).unwrap().drawn().unwrap();
        for (n, _) in s.list_rows(tree) {
            let node = s.get(n).unwrap();
            if node.descendants().find(|x| x.tag() == "label").is_some_and(|l| l.text() == text) {
                let r = node.drawn().unwrap();
                return egui::Rect::from_x_y_ranges(list.x_range(), r.y_range());
            }
        }
        panic!("no row {text}");
    })
}

fn click_row(app: &mut TestApp, tree: NodeId, text: &str, mods: Modifiers) {
    let r = row(app, tree, text);
    let p = egui::pos2(r.max.x - 24.0, r.center().y);
    // egui reads modifiers from the raw input, not from the pointer event
    app.harness.event_modifiers(egui::Event::PointerMoved(p), mods);
    app.harness.event_modifiers(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: true, modifiers: mods }, mods);
    app.harness.event_modifiers(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: false, modifiers: mods }, mods);
    app.run();
}

fn key(app: &mut TestApp, k: Key, mods: Modifiers) {
    app.harness.key_press_modifiers(mods, k);
    app.run();
}

fn tree_item(app: &TestApp, label: &str) -> (Option<usize>, Option<bool>, bool) {
    let n = app.harness.get_by_role_and_label(Role::TreeItem, label);
    let a = n.accesskit_node();
    (a.level(), a.data().is_expanded(), a.is_selected().unwrap_or(false))
}

#[test]
fn lazy_children_keyboard_and_selection_across_a_filter() {
    let (mut app, clip, tree) = start();
    assert!(has(&app, "Assembly") && has(&app, "Library") && has(&app, "Big (100000)"));
    assert!(!has(&app, "Frame"), "children load only on expand");
    assert!(has(&app, "loads: 0"));
    assert_eq!(app.harness.get_by_role(Role::Tree).accesskit_node().role(), Role::Tree);
    assert_eq!(tree_item(&app, "Assembly"), (Some(1), Some(false), false));

    // a click selects and focuses; Right expands: the Promise children arrive
    click_row(&mut app, tree, "Assembly", Modifiers::NONE);
    assert!(has(&app, "selected: asm"));
    key(&mut app, Key::ArrowRight, Modifiers::NONE);
    assert!(has(&app, "Frame") && has(&app, "Board"), "lazy children shown");
    assert!(has(&app, "loads: 1") && has(&app, "expanded: asm"));
    assert_eq!(tree_item(&app, "Assembly").1, Some(true));
    // Right again: to the first child; Right on it expands (inline children), then enters
    key(&mut app, Key::ArrowRight, Modifiers::NONE);
    assert!(has(&app, "selected: asm/frame"));
    key(&mut app, Key::ArrowRight, Modifiers::NONE);
    assert!(has(&app, "Bolt"));
    key(&mut app, Key::ArrowRight, Modifiers::NONE);
    assert!(has(&app, "selected: asm/frame/bolt"));
    assert_eq!(tree_item(&app, "Bolt").0, Some(3), "level 3");
    // Left: to the parent, then collapse it
    key(&mut app, Key::ArrowLeft, Modifiers::NONE);
    assert!(has(&app, "selected: asm/frame"));
    key(&mut app, Key::ArrowLeft, Modifiers::NONE);
    assert!(!has(&app, "Bolt") && has(&app, "expanded: asm"));
    // Shift+Down extends (multi)
    key(&mut app, Key::ArrowDown, Modifiers::SHIFT);
    assert!(has(&app, "selected: asm/board,asm/frame") || has(&app, "selected: asm/frame,asm/board"));
    // Ctrl+C: the selected rows go to the host's clipboard (rowDragType and text/plain)
    key(&mut app, Key::C, Modifiers::COMMAND);
    let got = clip.contents(ClipboardKind::Clipboard);
    assert_eq!(got.get_str("text/plain"), Some("Frame\nBoard"));
    assert_eq!(got.get_str("cad/part"), Some("\"asm/frame\"\n\"asm/board\""));
    // type-ahead: "li" goes to Library
    app.harness.event(egui::Event::Text("li".into()));
    app.run();
    assert!(has(&app, "selected: lib"));
    // Enter activates
    key(&mut app, Key::Enter, Modifiers::NONE);
    assert!(has(&app, "activated: Library"));

    // a disclosure click expands Library; Ctrl+click adds Resistor
    let r = row(&app, tree, "Library");
    let p = egui::pos2(r.min.x + 8.0, r.center().y);
    app.harness.event(egui::Event::PointerMoved(p));
    app.harness.event(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    app.harness.event(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    app.run();
    assert!(has(&app, "Resistor"));
    click_row(&mut app, tree, "Resistor", Modifiers::COMMAND);
    assert!(has(&app, "selected: lib,lib/r"));
    assert!(tree_item(&app, "Resistor").2);

    // the filter hides Resistor: it stays selected, and comes back selected (SV46)
    app.harness.get_by_label("filter").click();
    app.run();
    app.harness.get_by_label("filter").type_text("cap");
    app.run();
    assert!(!has(&app, "Resistor") && has(&app, "Capacitor"));
    assert!(has(&app, "selected: lib,lib/r"));
    for _ in 0..3 {
        app.harness.key_press(Key::Backspace);
    }
    app.run();
    assert!(has(&app, "Resistor"));
    assert!(tree_item(&app, "Resistor").2, "selected again");
    assert!(app.errors().is_empty(), "{:?}", app.errors());
}

#[test]
fn a_row_drags_onto_another_and_a_100k_subtree_binds_its_window() {
    let (mut app, _, tree) = start();
    // expand Assembly and Library through their disclosure triangles
    for name in ["Assembly", "Library"] {
        let r = row(&app, tree, name);
        let p = egui::pos2(r.min.x + 8.0, r.center().y);
        app.harness.event(egui::Event::PointerMoved(p));
        app.harness.event(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
        app.harness.event(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
        app.run();
    }
    assert!(has(&app, "Board") && has(&app, "Resistor"));
    // Resistor dropped in the middle of Board: `into`
    let (from, to) = (row(&app, tree, "Resistor"), row(&app, tree, "Board"));
    let (from, to) = (egui::pos2(from.max.x - 24.0, from.center().y), egui::pos2(to.max.x - 24.0, to.center().y));
    let h = &mut app.harness;
    h.event(egui::Event::PointerMoved(from));
    h.step();
    h.event(egui::Event::PointerButton { pos: from, button: egui::PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.step();
    for k in 1..=8 {
        h.event(egui::Event::PointerMoved(from + (to - from) * (k as f32 / 8.0)));
        h.step();
    }
    h.step();
    h.step();
    h.event(egui::Event::PointerButton { pos: to, button: egui::PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    app.run();
    assert!(has(&app, "dropped: into asm/board cad/part \"lib/r\""), "dropped into Board");

    // 100k children: the list has them all, only the rows in view are bound
    let r = row(&app, tree, "Big (100000)");
    let p = egui::pos2(r.min.x + 8.0, r.center().y);
    app.harness.event(egui::Event::PointerMoved(p));
    app.harness.event(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    app.harness.event(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    app.run();
    let (len, slots, stats) = app.backend.scene().read(|s| {
        let v = s.virtual_list(tree).unwrap();
        (v.len(), v.slot_count(), v.stats())
    });
    assert!(len > 100_000, "{len} rows");
    assert!(slots <= 60, "{slots} row slots");
    assert!(stats.binds < 200, "{} binds", stats.binds);
    assert!(app.errors().is_empty(), "{:?}", app.errors());
}
