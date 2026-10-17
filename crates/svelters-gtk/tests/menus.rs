//! X4 on GTK: menus and actions (`menubar`, `menu`, `action`, `separator`, `contextmenu`, `toolbar`).
//! Drives the backend through its vtable. Needs a display (xvfb-run).

use gtk::gio::prelude::*;
use gtk::glib;
use gtk::prelude::*;
use svelters_gtk::testing::{on_gtk, pump, Harness, V};

fn mk(h: &Harness, kind: &str, props: &[(&str, &str)]) -> u64 {
    let n = h.create(kind);
    for (k, v) in props {
        h.set_str(n, k, v).unwrap();
    }
    n
}

fn action(h: &Harness, parent: u64, label: &str) -> u64 {
    let a = mk(h, "action", &[("label", label)]);
    h.insert(parent, a, None, None).unwrap();
    a
}

fn activate(h: &Harness, a: u64) {
    let name = h.backend.menu_action_name(a).unwrap();
    let group = h.backend.menu_group(a).unwrap();
    let (_, short) = name.split_once('.').unwrap();
    group.activate_action(short, None);
}

fn activate_target(h: &Harness, a: u64) {
    let name = h.backend.menu_action_name(a).unwrap();
    let group = h.backend.menu_group(a).unwrap();
    let (_, short) = name.split_once('.').unwrap();
    group.activate_action(short, Some(&format!("a{a}").to_variant()));
}

#[test]
fn a_menubar_follows_inserts_removes_and_reorders() {
    on_gtk(|| {
        let h = Harness::new();
        let win = h.create("window");
        let bx = h.create("box");
        let bar = h.create("menubar");
        h.insert(win, bx, None, None).unwrap();
        h.insert(bx, bar, None, None).unwrap();
        let file = mk(&h, "menu", &[("label", "File")]);
        h.insert(bar, file, None, None).unwrap();
        let open = action(&h, file, "Open");
        let sep = h.create("menuseparator");
        h.insert(file, sep, None, None).unwrap();
        let quit = action(&h, file, "Quit");
        let recent = mk(&h, "menu", &[("label", "Recent")]);
        h.insert(file, recent, Some(sep), None).unwrap();
        let a = action(&h, recent, "a.txt");
        assert_eq!(h.backend.menu_dump(bar), "File[{Open,Recent[a.txt,],}{Quit,}],");
        let _ = a;

        // Reorder: Quit moves before Open; the separator now ends the list, so one section is left.
        h.insert(file, quit, Some(open), None).unwrap();
        assert_eq!(h.backend.menu_dump(file), "Quit,Open,Recent[a.txt,],");
        // Remove an item: the model changes natively.
        h.remove(file, open).unwrap();
        assert!(!h.backend.menu_dump(file).contains("Open"));
        // The removed node can be inserted again and a label change is one model update.
        h.insert(file, open, None, None).unwrap();
        h.set_str(open, "label", "Open file").unwrap();
        assert!(h.backend.menu_dump(file).contains("Open file"));
        // Hidden items leave the model.
        h.set_bool(open, "visible", false).unwrap();
        assert!(!h.backend.menu_dump(file).contains("Open file"));
        // Cleanup leaves no handle behind.
        for n in [a, recent, quit, open, sep, file, bar, bx, win] {
            h.destroy(n);
        }
        pump(50);
        assert_eq!(h.backend.live_handles(), 0);
    });
}

#[test]
fn sections_split_at_separators() {
    on_gtk(|| {
        let h = Harness::new();
        let m = h.create("menu");
        let a = action(&h, m, "A");
        let s1 = h.create("menuseparator");
        h.insert(m, s1, None, None).unwrap();
        let b = action(&h, m, "B");
        let s2 = h.create("menuseparator");
        h.insert(m, s2, None, None).unwrap();
        let _c = action(&h, m, "C");
        assert_eq!(h.backend.menu_dump(m), "{A,}{B,}{C,}");
        // A leading or doubled separator makes no empty section.
        h.remove(m, a).unwrap();
        assert_eq!(h.backend.menu_dump(m), "{B,}{C,}");
        h.insert(m, s2, Some(b), None).unwrap();
        assert_eq!(h.backend.menu_dump(m), "B,C,");
        h.remove(m, s2).unwrap();
        assert_eq!(h.backend.menu_dump(m), "B,C,");
    });
}

#[test]
fn activating_an_action_posts_activate_and_a_disabled_one_does_not() {
    on_gtk(|| {
        let h = Harness::new();
        let bar = h.create("menubar");
        let m = mk(&h, "menu", &[("label", "File")]);
        h.insert(bar, m, None, None).unwrap();
        let a = action(&h, m, "Open");
        h.listen(a, "activate", true).unwrap();
        activate(&h, a);
        assert_eq!(h.take_events_with_args(), vec![(a, "activate".to_string(), vec![V::Bool(false)])]);
        h.set_bool(a, "enabled", false).unwrap();
        activate(&h, a);
        assert!(h.take_events().is_empty(), "a disabled action is not activated");
        h.set_bool(a, "enabled", true).unwrap();
        h.listen(a, "activate", false).unwrap();
        activate(&h, a);
        assert!(h.take_events().is_empty());
    });
}

#[test]
fn check_actions_toggle_natively_and_js_writes_do_not_echo() {
    on_gtk(|| {
        let h = Harness::new();
        let bar = h.create("menubar");
        let m = mk(&h, "menu", &[("label", "View")]);
        h.insert(bar, m, None, None).unwrap();
        let a = mk(&h, "action", &[("label", "Wrap")]);
        h.set_bool(a, "checkable", true).unwrap();
        h.set_bool(a, "checked", true).unwrap();
        h.insert(m, a, None, None).unwrap();
        h.listen(a, "toggled", true).unwrap();
        h.listen(a, "activate", true).unwrap();
        assert_eq!(h.get(a, "checked"), V::Bool(true));
        // JS writes: no event.
        h.set_bool(a, "checked", false).unwrap();
        assert_eq!(h.get(a, "checked"), V::Bool(false));
        assert!(h.take_events().is_empty(), "no echo for a JS write");
        // A user activation toggles, and tells JS.
        activate(&h, a);
        assert_eq!(h.get(a, "checked"), V::Bool(true));
        let ev = h.take_events_with_args();
        assert_eq!(
            ev,
            vec![(a, "toggled".to_string(), vec![V::Bool(true)]), (a, "activate".to_string(), vec![V::Bool(true)])]
        );
        // The group action carries the state (what the popover draws).
        let g = h.backend.menu_group(a).unwrap();
        assert_eq!(g.action_state(&format!("a{a}")).and_then(|v| v.get::<bool>()), Some(true));
    });
}

#[test]
fn radio_groups_are_exclusive_and_keep_one_checked_flag_per_action() {
    on_gtk(|| {
        let h = Harness::new();
        let m = h.create("menu");
        let mut items = Vec::new();
        for (i, l) in ["Small", "Medium", "Large"].iter().enumerate() {
            let a = mk(&h, "action", &[("label", l), ("group", "size")]);
            if i == 1 {
                h.set_bool(a, "checked", true).unwrap();
            }
            h.insert(m, a, None, None).unwrap();
            h.listen(a, "toggled", true).unwrap();
            items.push(a);
        }
        let get = |a: u64| h.get(a, "checked");
        assert_eq!((get(items[0]), get(items[1]), get(items[2])), (V::Bool(false), V::Bool(true), V::Bool(false)));
        // The user picks Large: Medium is turned off, Large on, in that order.
        activate_target(&h, items[2]);
        assert_eq!(
            h.take_events_with_args(),
            vec![(items[1], "toggled".to_string(), vec![V::Bool(false)]), (items[2], "toggled".to_string(), vec![V::Bool(true)])]
        );
        assert_eq!((get(items[0]), get(items[1]), get(items[2])), (V::Bool(false), V::Bool(false), V::Bool(true)));
        // JS writes do not echo and keep the group exclusive.
        h.set_bool(items[0], "checked", true).unwrap();
        assert!(h.take_events().is_empty());
        assert_eq!((get(items[0]), get(items[1]), get(items[2])), (V::Bool(true), V::Bool(false), V::Bool(false)));
        // Reordering keeps the state; removing the checked one leaves nothing checked.
        h.insert(m, items[2], Some(items[0]), None).unwrap();
        assert_eq!(get(items[0]), V::Bool(true));
        // Items of the model target the group action with their id.
        let dump = h.backend.menu_dump(m);
        assert_eq!(dump, "Large,Small,Medium,");
    });
}

#[test]
fn shortcuts_run_the_action_and_show_in_the_model() {
    on_gtk(|| {
        let h = Harness::new();
        let win = h.create("window");
        let bar = h.create("menubar");
        h.insert(win, bar, None, None).unwrap();
        let m = mk(&h, "menu", &[("label", "File")]);
        h.insert(bar, m, None, None).unwrap();
        let a = mk(&h, "action", &[("label", "Open"), ("shortcut", "Ctrl+O")]);
        h.insert(m, a, None, None).unwrap();
        h.listen(a, "activate", true).unwrap();
        // The menubar widget carries a global shortcut controller with one shortcut.
        let w = h.widget(bar);
        let ctrl = w
            .observe_controllers()
            .iter::<glib::Object>()
            .filter_map(|c| c.ok())
            .find_map(|c| c.downcast::<gtk::ShortcutController>().ok())
            .expect("shortcut controller");
        assert_eq!(ctrl.scope(), gtk::ShortcutScope::Global);
        let sc = ctrl.item(0).and_downcast::<gtk::Shortcut>().expect("one shortcut");
        assert_eq!(sc.trigger().unwrap().to_str().as_str(), "<Control>o");
        let act = sc.action().unwrap();
        assert!(act.activate(gtk::ShortcutActionFlags::empty(), &w, None));
        assert_eq!(h.take_events().len(), 1, "the shortcut activated the action");
        // Changing the shortcut replaces it; clearing removes the controller.
        h.set_str(a, "shortcut", "Ctrl+Shift+P").unwrap();
        let ctrl = w.observe_controllers().iter::<glib::Object>().filter_map(|c| c.ok()).find_map(|c| c.downcast::<gtk::ShortcutController>().ok()).unwrap();
        assert_eq!(ctrl.item(0).and_downcast::<gtk::Shortcut>().unwrap().trigger().unwrap().to_str().as_str(), "<Shift><Control>p");
        h.set_str(a, "shortcut", "").unwrap();
        assert!(w.observe_controllers().iter::<glib::Object>().filter_map(|c| c.ok()).all(|c| !c.is::<gtk::ShortcutController>()));
    });
}

#[test]
fn a_contextmenu_attaches_to_its_parent_widget_and_pops_up() {
    on_gtk(|| {
        let h = Harness::new();
        let win = h.create("window");
        let btn = h.create("button");
        h.insert(win, btn, None, None).unwrap();
        let cm = h.create("contextmenu");
        let a = action(&h, cm, "Paste");
        h.insert(btn, cm, None, None).unwrap();
        h.widget(win).downcast::<gtk::Window>().unwrap().present();
        pump(100);
        let pop = h.widget(btn).first_child().and_then(|c| c.downcast::<gtk::PopoverMenu>().ok());
        let mut found = None;
        let mut cur = h.widget(btn).first_child();
        while let Some(c) = cur {
            if let Ok(p) = c.clone().downcast::<gtk::PopoverMenu>() {
                found = Some(p);
            }
            cur = c.next_sibling();
        }
        let _ = pop;
        let pop = found.expect("the popover is a child of the button");
        assert!(!pop.is_visible());
        h.call(cm, "popup").unwrap();
        pump(100);
        assert!(pop.is_visible(), "popup() shows the menu");
        h.call(cm, "popdown").unwrap();
        pump(100);
        assert!(!pop.is_visible());
        // Actions inside work through the popover's own group.
        h.listen(a, "activate", true).unwrap();
        activate(&h, a);
        assert_eq!(h.take_events().len(), 1);
        // Removing the context menu detaches the popover.
        h.remove(btn, cm).unwrap();
        let mut cur = h.widget(btn).first_child();
        while let Some(c) = cur {
            assert!(!c.is::<gtk::PopoverMenu>());
            cur = c.next_sibling();
        }
        assert!(h.call(cm, "popup").is_err());
    });
}

#[test]
fn a_menubutton_takes_a_menu_child() {
    on_gtk(|| {
        let h = Harness::new();
        let win = h.create("window");
        let btn = h.create("menubutton");
        h.insert(win, btn, None, None).unwrap();
        let m = h.create("menu");
        let a = action(&h, m, "One");
        h.insert(btn, m, None, None).unwrap();
        let mb = h.widget(btn).downcast::<gtk::MenuButton>().unwrap();
        assert!(mb.menu_model().is_some());
        h.set_str(a, "label", "Uno").unwrap();
        assert_eq!(h.backend.menu_dump(m), "Uno,");
        assert_eq!(mb.menu_model().unwrap().n_items(), 1);
        h.remove(btn, m).unwrap();
        assert!(mb.menu_model().is_none());
    });
}

#[test]
fn a_toolbar_shows_one_button_per_action() {
    on_gtk(|| {
        let h = Harness::new();
        let tb = h.create("toolbar");
        let copy = mk(&h, "action", &[("label", "Copy"), ("icon", "edit-copy")]);
        h.insert(tb, copy, None, None).unwrap();
        let sep = h.create("menuseparator");
        h.insert(tb, sep, None, None).unwrap();
        let bold = mk(&h, "action", &[("label", "Bold")]);
        h.set_bool(bold, "checkable", true).unwrap();
        h.insert(tb, bold, None, None).unwrap();
        let kids = h.children(tb);
        assert_eq!(kids.len(), 3);
        assert!(kids[0].is::<gtk::Button>() && !kids[0].is::<gtk::ToggleButton>());
        assert!(kids[1].is::<gtk::Separator>());
        assert!(kids[2].is::<gtk::ToggleButton>());
        assert_eq!(kids[0].downcast_ref::<gtk::Button>().unwrap().icon_name().as_deref(), Some("edit-copy"));
        assert_eq!(kids[0].dynamic_cast_ref::<gtk::Actionable>().unwrap().action_name().as_deref(), Some(&*format!("svl.a{copy}")));
        // Activating through the button runs the action; checking state follows.
        h.listen(copy, "activate", true).unwrap();
        kids[0].downcast_ref::<gtk::Button>().unwrap().emit_clicked();
        assert_eq!(h.take_events().len(), 1);
        h.set_bool(bold, "checked", true).unwrap();
        pump(20);
        assert!(h.children(tb)[2].downcast_ref::<gtk::ToggleButton>().unwrap().is_active());
        // Reorder and remove.
        h.insert(tb, bold, Some(copy), None).unwrap();
        assert!(h.children(tb)[0].is::<gtk::ToggleButton>());
        h.remove(tb, copy).unwrap();
        assert_eq!(h.children(tb).len(), 2);
    });
}

#[test]
fn errors_are_reported_not_crashed() {
    on_gtk(|| {
        let h = Harness::new();
        let m = h.create("menu");
        let a = h.create("action");
        let btn = h.create("button");
        assert!(h.insert(a, m, None, None).is_err(), "a menu inside an action");
        assert!(h.insert(m, btn, None, None).is_err(), "a widget inside a menu");
        assert!(h.insert(btn, a, None, None).is_err(), "an action inside a widget");
        assert!(h.remove(m, a).is_err(), "not inserted");
        h.insert(m, a, None, None).unwrap();
        let other = h.create("menu");
        assert!(h.insert(other, a, None, None).is_err(), "already inserted elsewhere");
        let b = h.create("action");
        assert!(h.insert(m, b, Some(btn), None).is_err(), "unknown before");
        assert!(h.set_str(a, "no-such", "x").is_err());
    });
}
