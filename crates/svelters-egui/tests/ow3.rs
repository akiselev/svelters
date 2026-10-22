//! OW3 (plans/milestones/OW3-popups-menus.md): svelters' own menus, popup, select and tooltips.
//! The behaviour both implementations share is in `menus.rs` and `widgets.rs` (run on both); this
//! file covers what only the own ones do: the keyboard model, placement, dismissal, EV1 filters,
//! AccessKit roles and the look (PaintTrace).

mod common;

use common::Rig;
use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT, Queryable};
use svelters_abi::*;
use svelters_egui::events::{EventKind, FilterScope, Flow, SceneEvent};
use svelters_egui::native::{self, Native};
use svelters_egui::paint::{TraceKind, trace};
use svelters_egui::scene::SceneRef;
use svelters_egui::widgets::KindSet;

fn s(x: &str) -> Value {
    Value::Str(x.into())
}

const OW3: [&str; 6] = ["menu-bar", "menu-button", "context-menu", "popup", "combo-box", "tooltip"];

fn rig() -> Rig {
    let rig = Rig::new();
    native::set(&rig.harness.ctx, Native::Gtk);
    rig.backend.set_own_widgets(KindSet::tags(OW3));
    rig
}

fn events(rig: &Rig, h: svl_handle, name: &str) -> Vec<Vec<Value>> {
    let sig = rig.signal_id(h, name);
    rig.log.borrow().events.iter().filter(|e| e.0 == h && e.1 == sig).map(|e| e.2.clone()).collect()
}

fn has(rig: &Rig, label: &str) -> bool {
    rig.harness.query_by_label(label).is_some()
}

fn key(rig: &mut Rig, k: egui::Key) {
    rig.harness.key_press(k);
    rig.run();
}

fn click_at(rig: &mut Rig, pos: egui::Pos2, button: egui::PointerButton) {
    rig.harness.hover_at(pos);
    for pressed in [true, false] {
        rig.harness.event(egui::Event::PointerButton { pos, button, pressed, modifiers: egui::Modifiers::NONE });
    }
    rig.run();
}

fn click_label(rig: &mut Rig, label: &str) {
    let r = rig.harness.get_by_label(label).rect();
    click_at(rig, r.center(), egui::PointerButton::Primary);
}

/// Whether the row labelled `label` is the menu's current row (`:current`).
fn current(rig: &Rig, h: svl_handle) -> bool {
    rig.backend.with_scene(|sc| sc.n(h).state.contains(svelters_egui::provider::StateBits::CURRENT))
}

/// bar > File (Open, Recent > (a.txt, b.txt), separator, separator, Bold (check), Quit), Edit (Copy)
struct Bar {
    open: svl_handle,
    a: svl_handle,
    b: svl_handle,
    bold: svl_handle,
    quit: svl_handle,
    recent: svl_handle,
    copy: svl_handle,
}

fn bar(rig: &mut Rig) -> Bar {
    let (_w, col) = rig.window();
    let bar = rig.leaf(col, "menu-bar", &[]);
    let file = rig.leaf(bar, "menu", &[("label", s("File"))]);
    let open = rig.leaf(file, "action", &[("label", s("Open")), ("shortcut", s("Ctrl+O"))]);
    let recent = rig.leaf(file, "menu", &[("label", s("Recent"))]);
    let a = rig.leaf(recent, "action", &[("label", s("a.txt"))]);
    let b = rig.leaf(recent, "action", &[("label", s("b.txt"))]);
    rig.leaf(file, "menu-separator", &[]);
    rig.leaf(file, "menu-separator", &[]);
    let bold = rig.leaf(file, "action", &[("label", s("Bold")), ("checkable", Value::Bool(true))]);
    let quit = rig.leaf(file, "action", &[("label", s("Quit"))]);
    let edit = rig.leaf(bar, "menu", &[("label", s("Edit"))]);
    let copy = rig.leaf(edit, "action", &[("label", s("Copy"))]);
    rig.leaf(col, "text-edit", &[("accessible-name", s("field"))]);
    for h in [open, a, b, bold, quit, copy] {
        rig.listen(h, "activate");
    }
    rig.run();
    Bar { open, a, b, bold, quit, recent, copy }
}

#[test]
fn ow3_menu_bar_keyboard() {
    let mut rig = rig();
    let m = bar(&mut rig);
    // F10: the bar takes the keyboard, its first title current; Down opens it at its first row
    key(&mut rig, egui::Key::F10);
    assert!(!has(&rig, "Open"));
    key(&mut rig, egui::Key::ArrowDown);
    assert!(has(&rig, "Open") && current(&rig, m.open));
    // roles: menu bar, menu, menu items; the accelerator is AccessKit's keyboard shortcut
    rig.harness.get_by_role(Role::MenuBar);
    rig.harness.get_by_role(Role::Menu);
    let open = rig.harness.get_by_role_and_label(Role::MenuItem, "Open");
    assert!(open.accesskit_node().data().keyboard_shortcut().is_some_and(|k| k.contains("O")));
    rig.harness.get_by_role_and_label(Role::MenuItemCheckBox, "Bold");
    // Down to the submenu row, Right opens it at its first row, Left closes it
    key(&mut rig, egui::Key::ArrowDown);
    key(&mut rig, egui::Key::ArrowRight);
    assert!(has(&rig, "a.txt") && current(&rig, m.a));
    key(&mut rig, egui::Key::ArrowDown);
    assert!(current(&rig, m.b));
    key(&mut rig, egui::Key::ArrowLeft);
    assert!(!has(&rig, "a.txt") && has(&rig, "Open"));
    // the doubled separator collapsed: Down from Recent reaches Bold, End reaches Quit, Down wraps
    key(&mut rig, egui::Key::ArrowDown);
    assert!(current(&rig, m.bold));
    key(&mut rig, egui::Key::End);
    assert!(current(&rig, m.quit));
    key(&mut rig, egui::Key::ArrowDown);
    assert!(current(&rig, m.open));
    // type-ahead: "b" moves to Bold; Space toggles and closes
    rig.harness.event(egui::Event::Text("b".into()));
    rig.run();
    assert!(current(&rig, m.bold));
    rig.harness.key_press(egui::Key::Enter);
    rig.run();
    assert_eq!(events(&rig, m.bold, "activate"), vec![vec![Value::Bool(true)]]);
    assert!(!has(&rig, "Open"), "a pick closes the menus");
    // Right across the bar: File → Edit
    key(&mut rig, egui::Key::F10);
    key(&mut rig, egui::Key::ArrowDown);
    key(&mut rig, egui::Key::ArrowRight);
    assert!(has(&rig, "Copy") && !has(&rig, "Open") && current(&rig, m.copy));
    // Escape closes the menu, focus goes back to where it was
    key(&mut rig, egui::Key::Escape);
    assert!(!has(&rig, "Copy"));
    assert!(events(&rig, m.copy, "activate").is_empty() && events(&rig, m.quit, "activate").is_empty());
}

#[test]
fn ow3_hover_switches_titles_and_opens_submenus_after_the_delay() {
    let mut rig = rig();
    let m = bar(&mut rig);
    click_label(&mut rig, "File");
    assert!(has(&rig, "Open"));
    // hovering another title while one is open switches to it
    let edit = rig.harness.get_by_label("Edit").rect().center();
    rig.harness.hover_at(edit);
    rig.run();
    assert!(has(&rig, "Copy") && !has(&rig, "Open"));
    click_label(&mut rig, "Edit");
    assert!(!has(&rig, "Copy"), "a click on the open title closes it");
    // the submenu waits for the delay
    native::override_policy(&rig.harness.ctx, |p| p.submenu_delay_ms = 60_000);
    click_label(&mut rig, "File");
    let r = rig.harness.get_by_label("Recent").rect().center();
    rig.harness.hover_at(r);
    rig.harness.run();
    assert!(!has(&rig, "a.txt"), "not before the delay");
    native::override_policy(&rig.harness.ctx, |p| p.submenu_delay_ms = 0);
    rig.harness.run_steps(2);
    rig.harness.run();
    assert!(has(&rig, "a.txt"));
    // a click outside every level closes them all
    click_at(&mut rig, egui::pos2(400.0, 330.0), egui::PointerButton::Primary);
    assert!(!has(&rig, "Open") && !has(&rig, "a.txt"));
    assert!(events(&rig, m.a, "activate").is_empty());
}

#[test]
fn ow3_filters_prevent_and_stop_activate() {
    let mut rig = rig();
    let m = bar(&mut rig);
    let bold = m.bold;
    let _f = rig.backend.scene().register_filter(FilterScope::All, move |ev: &SceneEvent, _s: SceneRef<'_>| {
        if ev.kind == EventKind::Signal("activate") && ev.target_node().to_raw() == bold { Flow::Prevent } else { Flow::Pass }
    });
    click_label(&mut rig, "File");
    click_label(&mut rig, "Bold");
    assert_eq!(rig.get(m.bold, "checked"), Value::Bool(false), "prevented: no toggle");
    assert_eq!(events(&rig, m.bold, "activate"), vec![vec![Value::Bool(false)]], "JS still sees it");
    // the accelerator goes through the same path
    rig.harness.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::O);
    rig.run();
    assert_eq!(events(&rig, m.open, "activate").len(), 1);
}

#[test]
fn ow3_menu_look_and_placement() {
    let mut rig = rig();
    let m = bar(&mut rig);
    trace::enable(&rig.harness.ctx, true);
    click_label(&mut rig, "File");
    let r = rig.harness.get_by_label("Quit").rect().center();
    rig.harness.hover_at(r);
    rig.run();
    let t = trace::last(&rig.harness.ctx);
    // the hovered row is current: `menu-sel` (VS Code Dark Modern #0078D4) under `menu-sel-fg`
    let row = t.find(m.quit, None, TraceKind::Box).expect("the row's box");
    assert_eq!(row.fill, egui::Color32::from_rgb(0x00, 0x78, 0xD4), "{row:?}");
    // the menu sits under its title, rows 26 tall (VS Code's 2em)
    let file = rig.harness.get_by_label("File").rect();
    let open = rig.harness.get_by_label("Open").rect();
    assert!(open.min.y >= file.max.y && (open.height() - 26.0).abs() < 0.5, "{file:?} {open:?}");
    // the check column: Bold is unchecked, the label starts after it
    let bold = rig.harness.get_by_label("Bold").rect();
    assert!(t.find(m.bold, Some("check"), TraceKind::Path).is_none());
    assert!(t.find(m.bold, Some("label"), TraceKind::Text).is_some_and(|e| e.rect.min.x > bold.min.x + 20.0));
    // a submenu row shows its chevron (R14)

    assert!(t.find(m.recent, Some("chevron"), TraceKind::Path).is_some());
}

#[test]
fn ow3_context_menu_keys_and_flip() {
    let mut rig = rig();
    let (_w, col) = rig.window();
    // a button at the bottom edge: its menu flips above it
    rig.leaf(col, "box", &[("height", Value::F64(300.0))]);
    let btn = rig.leaf(col, "button", &[("text", s("Target")), ("align-self", s("start"))]);
    let cm = rig.leaf(btn, "context-menu", &[]);
    let cut = rig.leaf(cm, "action", &[("label", s("Cut"))]);
    rig.leaf(cm, "action", &[("label", s("Paste"))]);
    rig.listen(cut, "activate");
    rig.run();
    // Tab to the button, Shift+F10 opens its menu (FM1 QA8) at its first row
    key(&mut rig, egui::Key::Tab);
    assert!(rig.harness.get_by_label("Target").is_focused());
    rig.harness.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::F10);
    rig.run();
    assert!(has(&rig, "Cut") && current(&rig, cut));
    let item = rig.harness.get_by_label("Paste").rect();
    assert!(item.max.y <= rig.drawn(btn).max.y + 1.0, "flipped above: {item:?} {:?}", rig.drawn(btn));
    key(&mut rig, egui::Key::Escape);
    assert!(!has(&rig, "Cut"));
    assert!(rig.harness.get_by_label("Target").is_focused(), "focus back on the owner");
    // the Menu key too; Enter picks
    key(&mut rig, egui::Key::Menu);
    key(&mut rig, egui::Key::Enter);
    assert_eq!(events(&rig, cut, "activate").len(), 1);
}

#[test]
fn ow3_popup_flips_and_dismisses() {
    let mut rig = rig();
    let (_w, col) = rig.window();
    rig.leaf(col, "box", &[("height", Value::F64(300.0))]);
    let btn = rig.leaf(col, "button", &[("text", s("Pick")), ("align-self", s("start"))]);
    let pop = rig.create("popup");
    rig.set(pop, "padding", Value::F64(6.0));
    rig.insert(btn, pop);
    rig.leaf(pop, "label", &[("text", s("Inside"))]);
    rig.leaf(pop, "label", &[("text", s("Second line"))]);
    rig.listen(pop, "closed");
    rig.set(pop, "open", Value::Bool(true));
    rig.run();
    let inside = rig.harness.get_by_label("Second line").rect();
    assert!(inside.max.y <= rig.drawn(btn).min.y, "no room below: above its owner {inside:?}");
    rig.harness.get_by_role(Role::Dialog);
    // a click on the popup itself keeps it; outside closes it
    click_label(&mut rig, "Inside");
    assert!(has(&rig, "Inside"));
    click_at(&mut rig, egui::pos2(400.0, 20.0), egui::PointerButton::Primary);
    assert!(!has(&rig, "Inside"));
    assert_eq!(events(&rig, pop, "closed").len(), 1);
}

fn select(rig: &mut Rig) -> svl_handle {
    let (_w, col) = rig.window();
    let opts = Value::List(vec![s("Alpha"), s("Beta"), s("Gamma"), s("Delta")]);
    let cb = rig.leaf(col, "combo-box", &[("options", opts), ("accessible-name", s("Plane")), ("align-self", s("start"))]);
    for e in ["changed", "activate", "highlight", "commit"] {
        rig.listen(cb, e);
    }
    rig.observe(cb, "open", true);
    rig.run();
    cb
}

#[test]
fn ow3_select_keyboard() {
    let mut rig = rig();
    let cb = select(&mut rig);
    let combo = rig.harness.get_by_role_and_label(Role::ComboBox, "Plane");
    assert_eq!(combo.accesskit_node().value().as_deref(), Some("Alpha"));
    key(&mut rig, egui::Key::Tab);
    // closed: Down picks the next option at once, type-ahead picks by prefix
    key(&mut rig, egui::Key::ArrowDown);
    assert_eq!(rig.get(cb, "selected"), Value::I64(1));
    rig.harness.event(egui::Event::Text("g".into()));
    rig.run();
    assert_eq!(rig.get(cb, "selected"), Value::I64(2));
    assert_eq!(events(&rig, cb, "changed"), vec![vec![Value::I64(1)], vec![Value::I64(2)]]);
    // Enter opens the list (bind:open), at the selected row; Down moves (highlight), Enter picks
    rig.harness.key_press(egui::Key::Enter);
    rig.harness.run_steps(2);
    rig.run();
    assert_eq!(rig.get(cb, "open"), Value::Bool(true));
    rig.harness.get_by_role(Role::ListBox);
    assert_eq!(rig.harness.query_all_by_role(Role::ListBoxOption).count(), 4);
    key(&mut rig, egui::Key::ArrowDown);
    assert_eq!(events(&rig, cb, "highlight"), vec![vec![Value::I64(3)]]);
    key(&mut rig, egui::Key::Enter);
    assert_eq!(rig.get(cb, "open"), Value::Bool(false));
    assert_eq!(rig.get(cb, "selected"), Value::I64(3));
    assert_eq!(events(&rig, cb, "activate").last(), Some(&vec![Value::I64(3)]));
    assert_eq!(events(&rig, cb, "commit").last(), Some(&vec![Value::I64(3)]));
    assert!(rig.harness.get_by_role_and_label(Role::ComboBox, "Plane").is_focused(), "focus back on the select");
    // Escape closes without a pick
    rig.harness.key_press(egui::Key::Space);
    rig.harness.run_steps(2);
    rig.run();
    key(&mut rig, egui::Key::Home);
    key(&mut rig, egui::Key::Escape);
    assert_eq!(rig.get(cb, "open"), Value::Bool(false));
    assert_eq!(rig.get(cb, "selected"), Value::I64(3));
}

#[test]
fn ow3_select_pointer_and_width() {
    let mut rig = rig();
    let cb = select(&mut rig);
    trace::enable(&rig.harness.ctx, true);
    let r = rig.drawn(cb);
    click_at(&mut rig, r.center(), egui::PointerButton::Primary);
    assert_eq!(rig.get(cb, "open"), Value::Bool(true));
    let gamma = rig.harness.get_by_label("Gamma").rect();
    assert!(gamma.width() >= r.width() - 8.0 && gamma.min.y >= r.max.y, "below, as wide as the select: {gamma:?} {r:?}");
    // the chevron is a part (VIS1 gap 8)
    let t = trace::last(&rig.harness.ctx);
    assert!(t.find(cb, Some("chevron"), TraceKind::Path).is_some());
    click_at(&mut rig, gamma.center(), egui::PointerButton::Primary);
    assert_eq!(rig.get(cb, "selected"), Value::I64(2));
    assert_eq!(rig.get(cb, "open"), Value::Bool(false));
    // the host opens it; a press outside closes it
    rig.set(cb, "open", Value::Bool(true));
    rig.run();
    assert!(has(&rig, "Delta"));
    click_at(&mut rig, egui::pos2(400.0, 330.0), egui::PointerButton::Primary);
    assert_eq!(rig.get(cb, "open"), Value::Bool(false));
    assert_eq!(rig.get(cb, "selected"), Value::I64(2));
}

#[test]
fn ow3_tooltips_delay_disabled_and_description() {
    let mut rig = rig();
    native::override_policy(&rig.harness.ctx, |p| p.tooltip_delay_ms = 60_000);
    let (_w, col) = rig.window();
    let a = rig.leaf(col, "button", &[("text", s("Save")), ("tooltip", s("Saves the file")), ("align-self", s("start"))]);
    let b = rig.leaf(col, "button", &[("text", s("Undo")), ("tooltip", s("Nothing to undo")), ("enabled", Value::Bool(false)), ("align-self", s("start"))]);
    rig.run();
    // the host is described by its tooltip, shown or not
    let save = rig.harness.get_by_role_and_label(Role::Button, "Save");
    assert_eq!(save.accesskit_node().description().as_deref(), Some("Saves the file"));
    let tip = |rig: &Rig, t: &str| rig.harness.query_all_by_label(t).filter(|n| n.accesskit_node().role() == Role::Tooltip).count();
    rig.harness.hover_at(rig.drawn(a).center());
    rig.harness.run();
    assert_eq!(tip(&rig, "Saves the file"), 0, "not before the delay");
    native::override_policy(&rig.harness.ctx, |p| p.tooltip_delay_ms = 0);
    rig.harness.run_steps(2);
    rig.harness.run();
    assert_eq!(tip(&rig, "Saves the file"), 1);
    // a press hides it
    let pa = rig.drawn(a).center();
    click_at(&mut rig, pa, egui::PointerButton::Primary);
    assert_eq!(tip(&rig, "Saves the file"), 0);
    // follow-on: the disabled button's reason shows at once (R9)
    rig.harness.hover_at(rig.drawn(b).center());
    rig.harness.run_steps(2);
    rig.harness.run();
    assert_eq!(tip(&rig, "Nothing to undo"), 1, "a disabled control keeps its tooltip");
    rig.harness.hover_at(egui::pos2(400.0, 330.0));
    rig.harness.run();
    assert_eq!(tip(&rig, "Nothing to undo"), 0);
}

#[test]
fn ow3_menu_button_keyboard_and_chevron() {
    let mut rig = rig();
    let (_w, col) = rig.window();
    let mb = rig.leaf(col, "menu-button", &[("label", s("More")), ("align-self", s("start"))]);
    let copy = rig.leaf(mb, "action", &[("label", s("Copy"))]);
    let paste = rig.leaf(mb, "action", &[("label", s("Paste")), ("enabled", Value::Bool(false))]);
    let cut = rig.leaf(mb, "action", &[("label", s("Cut"))]);
    for h in [copy, paste, cut] {
        rig.listen(h, "activate");
    }
    rig.observe(mb, "active", true);
    trace::enable(&rig.harness.ctx, true);
    rig.run();
    // R14: a chevron part
    assert!(trace::last(&rig.harness.ctx).find(mb, Some("chevron"), TraceKind::Path).is_some());
    key(&mut rig, egui::Key::Tab);
    key(&mut rig, egui::Key::Enter);
    assert_eq!(rig.get(mb, "active"), Value::Bool(true));
    assert!(current(&rig, copy), "opened from the keyboard at its first row");
    // R9: the disabled row is reachable, not pickable
    key(&mut rig, egui::Key::ArrowDown);
    assert!(current(&rig, paste));
    key(&mut rig, egui::Key::Enter);
    assert!(events(&rig, paste, "activate").is_empty() && has(&rig, "Paste"));
    key(&mut rig, egui::Key::ArrowDown);
    key(&mut rig, egui::Key::Enter);
    assert_eq!(events(&rig, cut, "activate").len(), 1);
    assert_eq!(rig.get(mb, "active"), Value::Bool(false));
    assert!(rig.harness.get_by_role_and_label(Role::Button, "More").is_focused(), "focus back on the button");
}
