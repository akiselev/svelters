//! WG2a (plans/milestones/WG2a-menus-popups.md): the menu family and the overlays under kittest,
//! through the real vtable: each kind rendered, driven by its role and label, its events and
//! widget-side prop changes checked.

mod common;

use common::Rig;
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use svelters_abi::*;

fn s(x: &str) -> Value {
    Value::Str(x.into())
}

fn events(rig: &Rig, h: svl_handle, name: &str) -> Vec<Vec<Value>> {
    let sig = rig.signal_id(h, name);
    rig.log
        .borrow()
        .events
        .iter()
        .filter(|(eh, es, _)| *eh == h && *es == sig)
        .map(|(_, _, a)| a.clone())
        .collect()
}

fn changed(rig: &Rig, h: svl_handle, prop: &str) -> usize {
    let p = rig.prop_id(h, prop);
    rig.log
        .borrow()
        .changed
        .iter()
        .filter(|c| **c == (h, p))
        .count()
}

fn has(rig: &Rig, label: &str) -> bool {
    rig.harness.query_by_label(label).is_some()
}

/// A pointer click of `button` at `pos`.
fn click_at(rig: &mut Rig, pos: egui::Pos2, button: egui::PointerButton) {
    rig.harness.hover_at(pos);
    for pressed in [true, false] {
        rig.harness.event(egui::Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    rig.harness.run();
}

/// A pointer click on the accessibility node labelled `label` (by its bounds).
fn click_label(rig: &mut Rig, label: &str) {
    let r = rig.harness.get_by_label(label).rect();
    click_at(rig, r.center(), egui::PointerButton::Primary);
}

/// window > box > menu-bar > File (Open Ctrl+O, Autosave (check), separator, Recent > a.txt), Size
/// (Small, Large: radio group).
struct Menus {
    bar: svl_handle,
    open: svl_handle,
    autosave: svl_handle,
    recent: svl_handle,
    small: svl_handle,
    large: svl_handle,
}

fn menus(rig: &mut Rig) -> Menus {
    let (_w, b) = rig.window();
    let bar = rig.leaf(b, "menu-bar", &[]);
    let file = rig.leaf(bar, "menu", &[("label", s("File"))]);
    let open = rig.leaf(
        file,
        "action",
        &[("label", s("Open")), ("shortcut", s("Ctrl+O"))],
    );
    let autosave = rig.leaf(
        file,
        "action",
        &[("label", s("Autosave")), ("checkable", Value::Bool(true))],
    );
    rig.leaf(file, "menu-separator", &[]);
    let rm = rig.leaf(file, "menu", &[("label", s("Recent"))]);
    let recent = rig.leaf(rm, "action", &[("label", s("a.txt"))]);
    let size = rig.leaf(bar, "menu", &[("label", s("Size"))]);
    let small = rig.leaf(
        size,
        "action",
        &[
            ("label", s("Small")),
            ("group", s("size")),
            ("checked", Value::Bool(true)),
        ],
    );
    let large = rig.leaf(
        size,
        "action",
        &[("label", s("Large")), ("group", s("size"))],
    );
    rig.leaf(b, "label", &[("text", s("Below"))]);
    for (h, sigs) in [
        (open, &["activate"][..]),
        (autosave, &["activate", "toggled"]),
        (recent, &["activate"]),
        (small, &["toggled"]),
        (large, &["toggled", "activate"]),
    ] {
        for sig in sigs {
            rig.listen(h, sig);
        }
    }
    rig.observe(autosave, "checked", true);
    rig.observe(small, "checked", true);
    rig.observe(large, "checked", true);
    rig.run();
    Menus {
        bar,
        open,
        autosave,
        recent,
        small,
        large,
    }
}

#[test]
fn menu_bar_menus_actions_and_check_items() {
    let mut rig = Rig::new();
    let m = menus(&mut rig);
    // the bar is one row high, across the window, above the label
    let bar = rig.drawn(m.bar);
    assert!(bar.width() > 300.0 && bar.height() >= 18.0, "{bar:?}");
    assert!(
        rig.harness
            .get_by_role_and_label(Role::Button, "File")
            .rect()
            .max
            .y
            <= bar.max.y + 1.0
    );
    assert!(!has(&rig, "Open"), "closed menus draw nothing");

    // a click opens File; its items are found by role and label
    click_label(&mut rig, "File");
    rig.harness.get_by_role_and_label(Role::Button, "Open");
    rig.harness
        .get_by_role_and_label(Role::CheckBox, "Autosave");
    assert!(has(&rig, "Ctrl+O"), "the shortcut is shown");
    click_label(&mut rig, "Open");
    assert_eq!(
        events(&rig, m.open, "activate"),
        vec![vec![Value::Bool(false)]]
    );
    assert!(!has(&rig, "Open"), "picking an item closes the menu");

    // a check item toggles: bind:checked (exact observation) and toggled + activate
    click_label(&mut rig, "File");
    click_label(&mut rig, "Autosave");
    assert_eq!(rig.get(m.autosave, "checked"), Value::Bool(true));
    assert_eq!(
        events(&rig, m.autosave, "toggled"),
        vec![vec![Value::Bool(true)]]
    );
    assert_eq!(
        events(&rig, m.autosave, "activate"),
        vec![vec![Value::Bool(true)]]
    );
    assert_eq!(changed(&rig, m.autosave, "checked"), 1);

    // a submenu opens on hover
    click_label(&mut rig, "File");
    let r = rig.harness.get_by_label("Recent").rect();
    rig.harness.hover_at(r.center());
    rig.harness.run();
    click_label(&mut rig, "a.txt");
    assert_eq!(events(&rig, m.recent, "activate").len(), 1);
}

#[test]
fn radio_group_and_shortcuts() {
    let mut rig = Rig::new();
    let m = menus(&mut rig);
    click_label(&mut rig, "Size");
    rig.harness
        .get_by_role_and_label(Role::RadioButton, "Small");
    click_label(&mut rig, "Large");
    assert_eq!(rig.get(m.large, "checked"), Value::Bool(true));
    assert_eq!(
        rig.get(m.small, "checked"),
        Value::Bool(false),
        "a pick turns the rest of the group off"
    );
    assert_eq!(
        events(&rig, m.small, "toggled"),
        vec![vec![Value::Bool(false)]]
    );
    assert_eq!(
        events(&rig, m.large, "toggled"),
        vec![vec![Value::Bool(true)]]
    );
    assert_eq!(
        events(&rig, m.large, "activate"),
        vec![vec![Value::Bool(true)]]
    );
    assert_eq!(
        (
            changed(&rig, m.small, "checked"),
            changed(&rig, m.large, "checked")
        ),
        (1, 1)
    );

    // the accelerator works with the menu closed
    rig.harness
        .key_press_modifiers(egui::Modifiers::CTRL, egui::Key::O);
    rig.harness.run();
    assert_eq!(events(&rig, m.open, "activate").len(), 1);
    // a disabled action does not fire
    rig.set(m.open, "enabled", Value::Bool(false));
    rig.run();
    rig.harness
        .key_press_modifiers(egui::Modifiers::CTRL, egui::Key::O);
    rig.harness.run();
    assert_eq!(events(&rig, m.open, "activate").len(), 1);

    // shortcuts are checked on write
    let bad = rig.prop_id(m.open, "shortcut");
    assert!(rig.br().set_prop(m.open, bad, &s("Ctrl+Nope")).is_err());
    assert!(rig.br().set_prop(m.open, bad, &s("Hyper+O")).is_err());
    rig.set(m.open, "shortcut", s("Primary+Shift+F5"));
}

#[test]
fn menu_button_opens_its_menu_and_reports_active() {
    let mut rig = Rig::new();
    let (_w, b) = rig.window();
    let mb = rig.leaf(
        b,
        "menu-button",
        &[("label", s("More")), ("align-self", s("start"))],
    );
    // a `menu` child is the button's menu: its items are shown inline
    let menu = rig.leaf(mb, "menu", &[]);
    let copy = rig.leaf(menu, "action", &[("label", s("Copy"))]);
    let paste = rig.leaf(mb, "action", &[("label", s("Paste"))]);
    rig.listen(mb, "changed");
    rig.listen(copy, "activate");
    rig.listen(paste, "activate");
    rig.observe(mb, "active", true);
    rig.run();
    let btn = rig
        .harness
        .get_by_role_and_label(Role::Button, "More")
        .rect();
    assert!(
        btn.width() > 20.0 && btn.width() < 200.0,
        "measured like a button: {btn:?}"
    );

    click_label(&mut rig, "More");
    assert!(has(&rig, "Copy") && has(&rig, "Paste"));
    assert_eq!(rig.get(mb, "active"), Value::Bool(true));
    assert_eq!(events(&rig, mb, "changed"), vec![vec![Value::Bool(true)]]);
    click_label(&mut rig, "Copy");
    assert_eq!(events(&rig, copy, "activate").len(), 1);
    assert_eq!(rig.get(mb, "active"), Value::Bool(false));
    assert_eq!(changed(&rig, mb, "active"), 2);

    // the host opens and closes it
    rig.set(mb, "active", Value::Bool(true));
    rig.run();
    assert!(has(&rig, "Paste"));
    rig.call(mb, "popdown");
    rig.run();
    assert!(!has(&rig, "Paste"));
    assert_eq!(rig.get(mb, "active"), Value::Bool(false));
    rig.call(mb, "popup");
    rig.run();
    assert!(has(&rig, "Paste"));
}

#[test]
fn context_menu_on_a_leaf_and_on_a_box() {
    let mut rig = Rig::new();
    let (_w, b) = rig.window();
    let inner = rig.leaf(
        b,
        "box",
        &[("padding", Value::F64(20.0)), ("height", Value::F64(120.0))],
    );
    let btn = rig.leaf(inner, "button", &[("text", s("Target"))]);
    // attached to a leaf (the compiler's `attaches_to`): any element takes one
    let cm = rig.leaf(btn, "context-menu", &[]);
    let paste = rig.leaf(cm, "action", &[("label", s("Paste"))]);
    // and to a box, placed among its children (no place in its flow)
    let bm = rig.leaf(inner, "context-menu", &[]);
    rig.leaf(bm, "action", &[("label", s("Box item"))]);
    let after = rig.leaf(inner, "label", &[("text", s("After"))]);
    rig.listen(paste, "activate");
    rig.listen(btn, "clicked");
    rig.run();
    // the context menus take no space: the label follows the button directly (the box has no gap)
    assert!(
        (rig.drawn(after).min.y - rig.drawn(btn).max.y).abs() < 1.0,
        "{:?} {:?}",
        rig.drawn(btn),
        rig.drawn(after)
    );

    // a secondary click on the button opens its menu (the innermost owner), at the pointer
    let pos = rig.drawn(btn).center();
    click_at(&mut rig, pos, egui::PointerButton::Secondary);
    rig.harness.get_by_role_and_label(Role::Button, "Paste");
    assert!(!has(&rig, "Box item"));
    click_label(&mut rig, "Paste");
    assert_eq!(events(&rig, paste, "activate").len(), 1);
    assert!(!has(&rig, "Paste"));
    assert!(
        events(&rig, btn, "clicked").is_empty(),
        "the menu pick is not a click of the owner"
    );

    // on the box's own area: the box's menu
    let r = rig.drawn(btn);
    click_at(
        &mut rig,
        egui::pos2(r.max.x + 40.0, r.center().y),
        egui::PointerButton::Secondary,
    );
    assert!(has(&rig, "Box item") && !has(&rig, "Paste"));
    rig.harness.key_press(egui::Key::Escape);
    rig.harness.run();
    assert!(!has(&rig, "Box item"));

    // popup() opens it below its owner
    rig.call(cm, "popup");
    rig.run();
    let item = rig.harness.get_by_label("Paste").rect();
    assert!(
        item.min.y >= rig.drawn(btn).max.y - 1.0,
        "{item:?} below {:?}",
        rig.drawn(btn)
    );
}

#[test]
fn popup_attached_to_a_button() {
    let mut rig = Rig::new();
    let (_w, b) = rig.window();
    let btn = rig.leaf(
        b,
        "button",
        &[("text", s("Pick")), ("align-self", s("start"))],
    );
    let pop = rig.create("popup");
    rig.set(pop, "padding", Value::F64(6.0));
    rig.set(pop, "gap", Value::F64(4.0));
    rig.insert(btn, pop);
    rig.leaf(pop, "label", &[("text", s("Inside"))]);
    let te = rig.leaf(pop, "text-edit", &[("hint-text", s("Filter"))]);
    let below = rig.leaf(b, "label", &[("text", s("Below"))]);
    rig.listen(pop, "changed");
    rig.listen(pop, "closed");
    rig.listen(te, "changed");
    rig.observe(pop, "open", true);
    rig.run();
    assert!(!has(&rig, "Inside"));
    let gap_before = rig.drawn(below).min.y - rig.drawn(btn).max.y;

    rig.set(pop, "open", Value::Bool(true));
    rig.run();
    let inside = rig.harness.get_by_label("Inside").rect();
    assert!(
        inside.min.y >= rig.drawn(btn).max.y,
        "below its owner: {inside:?}"
    );
    // the content is laid out by taffy: a measured text field under the label
    let field = rig.drawn(te);
    assert!(
        field.width() > 100.0 && field.min.y > inside.max.y,
        "{field:?}"
    );
    assert_eq!(
        rig.drawn(below).min.y - rig.drawn(btn).max.y,
        gap_before,
        "no place in the flow"
    );
    // its inputs work
    rig.type_into(te, "ab");
    assert_eq!(rig.get(te, "text"), s("ab"));
    assert_eq!(
        events(&rig, pop, "changed").len(),
        0,
        "typing inside keeps it open"
    );

    // Escape closes it: bind:open sees false, `closed` fires
    rig.harness.key_press(egui::Key::Escape);
    rig.harness.run();
    assert!(!has(&rig, "Inside"));
    assert_eq!(rig.get(pop, "open"), Value::Bool(false));
    assert_eq!(events(&rig, pop, "changed"), vec![vec![Value::Bool(false)]]);
    assert_eq!(events(&rig, pop, "closed").len(), 1);
    assert_eq!(changed(&rig, pop, "open"), 1);

    // popup() opens it (announced); a click outside closes it with autohide
    rig.call(pop, "popup");
    rig.run();
    assert!(has(&rig, "Inside"));
    assert_eq!(rig.get(pop, "open"), Value::Bool(true));
    click_at(
        &mut rig,
        egui::pos2(400.0, 300.0),
        egui::PointerButton::Primary,
    );
    assert!(!has(&rig, "Inside"));
    // without autohide it stays
    rig.set(pop, "autohide", Value::Bool(false));
    rig.set(pop, "open", Value::Bool(true));
    rig.run();
    click_at(
        &mut rig,
        egui::pos2(400.0, 300.0),
        egui::PointerButton::Primary,
    );
    assert!(has(&rig, "Inside"));
}

#[test]
fn modal_dialog() {
    let mut rig = Rig::new();
    let (_w, b) = rig.window();
    let back = rig.leaf(b, "button", &[("text", s("Behind"))]);
    let modal = rig.create("modal");
    rig.set(modal, "title", s("Confirm"));
    rig.set(modal, "gap", Value::F64(8.0));
    rig.set(modal, "open", Value::Bool(false));
    rig.insert(b, modal);
    rig.leaf(modal, "label", &[("text", s("Delete it?"))]);
    let ok = rig.leaf(modal, "button", &[("text", s("OK"))]);
    rig.listen(ok, "clicked");
    rig.listen(back, "clicked");
    rig.listen(modal, "closed");
    rig.listen(modal, "changed");
    rig.run();
    assert!(!has(&rig, "Delete it?"));

    rig.set(modal, "open", Value::Bool(true));
    rig.run();
    assert!(has(&rig, "Confirm") && has(&rig, "Delete it?"));
    rig.click(ok);
    assert_eq!(events(&rig, ok, "clicked").len(), 1);
    // the backdrop blocks what is behind it
    rig.click(back);
    assert!(events(&rig, back, "clicked").is_empty());
    // dismissable: false keeps it on Escape
    rig.set(modal, "dismissable", Value::Bool(false));
    rig.run();
    rig.harness.key_press(egui::Key::Escape);
    rig.harness.run();
    assert!(has(&rig, "Delete it?"));
    rig.set(modal, "dismissable", Value::Bool(true));
    rig.run();
    rig.harness.key_press(egui::Key::Escape);
    rig.harness.run();
    assert!(!has(&rig, "Delete it?"));
    assert_eq!(rig.get(modal, "open"), Value::Bool(false));
    assert_eq!(events(&rig, modal, "closed").len(), 1);
    assert_eq!(
        events(&rig, modal, "changed"),
        vec![vec![Value::Bool(false)]]
    );
}

#[test]
fn inner_window_with_content_and_close_button() {
    let mut rig = Rig::new();
    let (_w, b) = rig.window();
    let win = rig.create("inner-window");
    rig.set(win, "title", s("Inspector"));
    rig.set(win, "resizable", Value::Bool(false));
    rig.insert(b, win);
    let inner = rig.leaf(
        win,
        "box",
        &[("flex-direction", s("row")), ("gap", Value::F64(6.0))],
    );
    rig.leaf(inner, "label", &[("text", s("Name"))]);
    let field = rig.leaf(inner, "text-edit", &[("hint-text", s("Value"))]);
    rig.listen(win, "closed");
    rig.observe(win, "open", true);
    rig.run();
    // open by default: the title bar and the content, laid out as a row
    assert!(has(&rig, "Inspector"));
    let name = rig.harness.get_by_label("Name").rect();
    let f = rig.drawn(field);
    assert!(
        f.min.x > name.max.x && (f.center().y - name.center().y).abs() < 4.0,
        "{name:?} {f:?}"
    );

    // a resizable window lays its content out at its size
    rig.set(win, "resizable", Value::Bool(true));
    rig.set(field, "flex-grow", Value::F64(1.0));
    rig.set(win, "default-width", Value::F64(300.0));
    rig.run();
    assert!(rig.drawn(field).width() > 150.0, "{:?}", rig.drawn(field));

    // the close button closes it
    let close = rig.harness.get_by_label("Close window").rect();
    click_at(&mut rig, close.center(), egui::PointerButton::Primary);
    assert!(!has(&rig, "Inspector"));
    assert_eq!(rig.get(win, "open"), Value::Bool(false));
    assert_eq!(events(&rig, win, "closed").len(), 1);
    assert_eq!(changed(&rig, win, "open"), 1);
}

#[test]
fn kinds_resolve_and_entries_stay_out_of_boxes() {
    let rig = Rig::new();
    for tag in [
        "menu-bar",
        "menu",
        "action",
        "menu-separator",
        "context-menu",
        "menu-button",
        "popup",
        "modal",
        "inner-window",
    ] {
        assert_ne!(rig.br().resolve(NameKind::Kind, 0, tag), 0, "{tag}");
    }
    // a slotless leaf takes only the attaching kinds
    let label = rig.create("label");
    let cm = rig.create("context-menu");
    let pop = rig.create("popup");
    let act = rig.create("action");
    rig.br().insert(label, cm, None, 0).unwrap();
    rig.br().insert(label, pop, None, 0).unwrap();
    assert!(rig.br().insert(label, act, None, 0).is_err());
    let spec = svelters_egui::spec::semantics();
    assert_eq!(spec["kinds"]["ContextMenu"]["attaches_to"], "Widget");
    assert_eq!(
        spec["kinds"]["MenuBar"]["children"]["slots"]["items"]["accepts"],
        serde_json::json!(["Menu", "Action", "MenuSeparator"])
    );
    assert_eq!(
        spec["kinds"]["Action"]["bindings"]["checked"]["event"],
        "toggled"
    );
    assert_eq!(
        spec["kinds"]["Popup"]["bindings"]["open"]["event"],
        "changed"
    );
}
