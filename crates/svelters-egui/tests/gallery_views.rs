//! WG4a gallery pages (`fixtures/gallery-egui/src/pages/{TableView,TabView,SplitView,Toasts}.svelte`)
//! mounted in an embed slot under kittest: the Svelte side of the Tier 1 views (`bind:`, events,
//! methods called from JS). The bundle is `fixtures/gallery-egui/dist/embed.js` (`pnpm build`).

use egui::Role;
use egui_kittest::kittest::{NodeT, Queryable};
use std::path::PathBuf;
use svelters_egui::embed::{EmbedConfig, EmbedEvent, MountSpec, Props, SlotOptions};
use svelters_egui::harness::EmbedHarness;
use svelters_host::HostConfig;

fn bundle() -> PathBuf {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/gallery-egui/dist/embed.js");
    assert!(p.exists(), "{} is missing: run `pnpm build` (needs `cargo build -p svelters-compiler` first)", p.display());
    p
}

fn cfg() -> EmbedConfig {
    EmbedConfig { host: HostConfig { echo_console: false, echo_errors: true, quit_on_last_window: false, ..HostConfig::default() }, ..EmbedConfig::default() }
}

/// Page `export` mounted in a slot filling a `w` x `h` harness.
fn page(export: &'static str, w: f32, h: f32, props: Props) -> EmbedHarness {
    let mut eh = EmbedHarness::new(egui::vec2(w, h), cfg()).unwrap();
    let e = eh.embed.clone();
    let entry = e.load(bundle()).unwrap();
    let slot = e.create_slot(export, SlotOptions::fill());
    eh.set_ui(move |e, ui| {
        e.show_slot(ui, slot);
    });
    e.mount(MountSpec::new(entry, export, slot, export).props(props)).unwrap();
    eh.run();
    let ev = e.take_events();
    assert!(!ev.iter().any(|x| matches!(x, EmbedEvent::Faulted { .. })), "{ev:?}");
    eh
}

fn has(eh: &EmbedHarness, label: &str) -> bool {
    eh.harness.query_by_label(label).is_some()
}

#[test]
fn table_page_feeds_rows_through_range_and_binds_selection() {
    // 2000 parts: the sort runs in a debug QuickJS inside the embed's logic budget
    let mut eh = page("TableView", 500.0, 400.0, Props::new().with("count", 2000));
    // range -> JS slice -> set-rows; the first frame asks, the next shows the data
    eh.run();
    assert!(has(&eh, "R0") && has(&eh, "2k"), "rows came from JS");
    assert!(!has(&eh, "R500"), "only the window");
    assert!(eh.harness.query_by_role_and_label(Role::ColumnHeader, "Value").is_some());
    eh.harness.get_by_label("R3").click();
    eh.run();
    assert!(has(&eh, "Selected row: 3"), "bind:selected");
    // header-clicked -> JS sorts -> the window is sent again
    eh.harness.get_by_role_and_label(Role::ColumnHeader, "Footprint").click();
    eh.run();
    let ev = eh.embed.take_events();
    assert!(!ev.iter().any(|x| matches!(x, EmbedEvent::Faulted { .. })), "{ev:?}");
    let dir = eh.harness.get_by_role_and_label(Role::ColumnHeader, "Footprint").accesskit_node().data().sort_direction();
    assert_eq!(dir, Some(egui::accesskit::SortDirection::Ascending), "the sort follows the prop");
    assert!(has(&eh, "R1") && !has(&eh, "R0"), "sorted by footprint: the odd rows (0402) come first");
}

#[test]
fn tab_page_binds_selected_and_closes_a_tab() {
    let mut eh = page("TabView", 400.0, 300.0, Props::new());
    assert!(has(&eh, "General page") && has(&eh, "Selected tab: 0"));
    eh.harness.get_by_role_and_label(Role::Tab, "Advanced").click();
    eh.run();
    assert!(has(&eh, "Advanced page") && has(&eh, "Selected tab: 1"));
    eh.harness.get_by_label("Close Notes").click();
    eh.run();
    assert!(has(&eh, "Tabs: 2") && eh.harness.query_by_role_and_label(Role::Tab, "Notes").is_none());
}

#[test]
fn split_page_binds_position_both_ways() {
    let mut eh = page("SplitView", 500.0, 300.0, Props::new());
    assert!(has(&eh, "Position: 120"));
    let tree = eh.harness.get_by_label("Tree pane").rect();
    let editor = eh.harness.get_by_label("Editor pane").rect();
    assert!(editor.min.x > tree.min.x + 120.0, "{tree:?} {editor:?}");
    // keys on the divider: bind:divider-position follows
    eh.harness.get_by_role_and_label(Role::Splitter, "Main split").focus();
    eh.harness.step();
    eh.harness.key_press(egui::Key::ArrowRight);
    eh.run();
    assert!(has(&eh, "Position: 130"));
    // and a JS write moves it
    eh.harness.get_by_label("Reset split").click();
    eh.run();
    assert!(has(&eh, "Position: 200"));
    let editor2 = eh.harness.get_by_label("Editor pane").rect();
    assert!((editor2.min.x - editor.min.x - 80.0).abs() < 1.5, "{editor:?} -> {editor2:?}");
    assert!(eh.harness.query_by_role_and_label(Role::Splitter, "Editor split").is_some(), "the nested split");
}

#[test]
fn toast_page_shows_toasts_from_js_and_reports_closes() {
    let mut eh = page("Toasts", 400.0, 300.0, Props::new());
    eh.harness.get_by_label("Notify").click();
    eh.run();
    assert!(eh.harness.query_by_role_and_label(Role::Alert, "Success: Hello 1").is_some());
    eh.harness.get_by_label("Notify").click();
    eh.run();
    assert!(has(&eh, "Hello 2"));
    eh.harness.get_by_label("Hello 2").click();
    eh.run();
    assert!(has(&eh, "Clicked: 2"));
    eh.harness.get_all_by_label("Dismiss").next().unwrap().click();
    eh.run();
    assert!(has(&eh, "Closed: 1 dismissed") && !has(&eh, "Hello 1"));
    eh.harness.get_by_label("Clear all").click();
    eh.run();
    assert!(has(&eh, "Closed: 2 method") && !has(&eh, "Hello 2"));
}

/// WG5a: `ItemViews.svelte`: keyed selection through `bind:selected`/`bind:current`, a column
/// `editor` snippet realised by the runtime (`editor-bind`/`editor-value`), `oncommit` rejecting
/// with a reason and accepting into the page's data, the built-in editor after Tab.
#[test]
fn item_views_page_edits_with_a_snippet_editor_and_validates_commits() {
    let mut eh = page("ItemViews", 600.0, 400.0, Props::new().with("count", 200));
    eh.run();
    assert!(has(&eh, "R3"), "rows came from JS");
    eh.harness.get_by_label("R3").click();
    eh.run();
    assert!(has(&eh, "Selected: 3") && has(&eh, "Current: 3"), "bind:selected and bind:current carry part ids");
    eh.harness.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::ArrowDown);
    eh.run();
    assert!(has(&eh, "Selected: 3, 4"), "Shift+Down extends");
    // Right to Qty, F2: the snippet editor, with the cell's value
    eh.harness.key_press(egui::Key::ArrowRight);
    eh.run();
    eh.harness.key_press(egui::Key::F2);
    eh.run();
    let ed = eh.harness.get_by_role_and_label(Role::TextInput, "Quantity");
    assert_eq!(ed.accesskit_node().value().as_deref(), Some("4"), "R4's qty");
    // a rejected value: the reason is shown, the cell keeps its value
    eh.harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    eh.harness.event(egui::Event::Text("250".into()));
    eh.run();
    eh.harness.key_press(egui::Key::Enter);
    eh.run();
    assert!(has(&eh, "Qty must be a whole number from 0 to 99"), "the reason is shown");
    assert!(eh.harness.query_by_role_and_label(Role::TextInput, "Quantity").is_none(), "reverted and closed");
    // accepted: the page's data changes and the window is sent again
    eh.harness.key_press(egui::Key::F2);
    eh.run();
    eh.harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    eh.harness.event(egui::Event::Text("42".into()));
    eh.run();
    eh.harness.key_press(egui::Key::Tab);
    eh.run();
    assert!(has(&eh, "Committed: R4: 42") && has(&eh, "42"), "committed through oncommit");
    // Tab went on to Note: the built-in text field
    assert!(eh.harness.query_by_role_and_label(Role::TextInput, "Note").is_some(), "Tab: the next editable cell");
    let ev = eh.embed.take_events();
    assert!(!ev.iter().any(|x| matches!(x, EmbedEvent::Faulted { .. })), "{ev:?}");
}
