//! X4 end to end on GTK: the real host runs the `fixtures/menus` Svelte bundle (a menubar with plain,
//! check and radio items, a keyed submenu, a toolbar and a context menu). Actions are activated the
//! way a click does (the item's action) and by shortcut; the app state, the label that shows it and
//! the native `GMenu` are compared at each step. Needs a display (xvfb-run) and the built fixture.

use gtk::glib;
use gtk::prelude::*;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;
use svelters_abi::BackendRef;
use svelters_gtk::testing::on_gtk;
use svelters_gtk::GtkBackend;
use svelters_host::{Host, HostConfig};

fn walk(w: &gtk::Widget, out: &mut Vec<gtk::Widget>) {
    out.push(w.clone());
    let mut c = w.first_child();
    while let Some(x) = c {
        walk(&x, out);
        c = x.next_sibling();
    }
}

fn all_widgets() -> Vec<gtk::Widget> {
    let mut v = Vec::new();
    for t in gtk::Window::list_toplevels() {
        walk(&t, &mut v);
    }
    v
}

fn status() -> String {
    all_widgets().iter().filter_map(|w| w.downcast_ref::<gtk::Label>()).map(|l| l.text().to_string()).find(|t| t.contains('|')).unwrap_or_default()
}

fn button(text: &str) -> gtk::Button {
    all_widgets().into_iter().filter_map(|w| w.downcast::<gtk::Button>().ok()).find(|b| b.label().as_deref().map(str::trim) == Some(text)).expect(text)
}

fn act(b: &GtkBackend, label: &str, target: bool) {
    let a = b.menu_find("action", label).unwrap_or_else(|| panic!("no action {label}"));
    let name = b.menu_action_name(a).unwrap();
    let (_, short) = name.split_once('.').unwrap();
    let t = target.then(|| format!("a{a}").to_variant());
    b.menu_group(a).unwrap().activate_action(short, t.as_ref());
}

#[test]
fn the_menus_fixture_drives_native_menus_both_ways() {
    let bundle = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/menus/dist/app.js");
    if !bundle.exists() {
        eprintln!("skipped: {} is not built", bundle.display());
        return;
    }
    on_gtk(move || {
        let backend = Rc::new(GtkBackend::new().expect("gtk"));
        let raw = unsafe { BackendRef::from_raw(backend.as_raw()) }.unwrap();
        let cfg = HostConfig { echo_console: false, echo_errors: false, ..HostConfig::default() };
        let host = Host::new(raw, cfg).unwrap();
        host.load_bundle(bundle).unwrap();

        let seen: Rc<RefCell<Vec<String>>> = Rc::default();
        let step = |ms: u64, f: Box<dyn Fn(&GtkBackend, &mut Vec<String>)>| {
            let (b, s) = (backend.clone(), seen.clone());
            glib::timeout_add_local_once(Duration::from_millis(ms), move || f(&b, &mut s.borrow_mut()));
        };
        step(
            400,
            Box::new(|b, s| {
                let file = b.menu_find("menu", "File").unwrap();
                s.push(b.menu_dump(file));
                s.push(status());
                act(b, "Open", false);
            }),
        );
        step(
            700,
            Box::new(|b, s| {
                s.push(status());
                act(b, "Autosave", false);
                act(b, "Large", true);
            }),
        );
        step(
            1000,
            Box::new(|b, s| {
                s.push(status());
                s.push(b.menu_dump(b.menu_find("menu", "Recent").unwrap()));
                button("Shuffle").emit_clicked();
            }),
        );
        step(
            1300,
            Box::new(|b, s| {
                s.push(b.menu_dump(b.menu_find("menu", "Recent").unwrap()));
                // The toolbar button runs its action.
                let w = all_widgets();
                let copy = w.iter().filter_map(|x| x.downcast_ref::<gtk::Button>()).find(|x| x.icon_name().as_deref() == Some("edit-copy")).unwrap().clone();
                copy.emit_clicked();
                // The shortcut controller's action runs the same action as the menu item.
                let bar = b.menu_find("menubar", "").unwrap();
                let bw = b.object(bar).unwrap().downcast::<gtk::Widget>().unwrap();
                let ctrl = bw.observe_controllers().iter::<glib::Object>().filter_map(|c| c.ok()).find_map(|c| c.downcast::<gtk::ShortcutController>().ok()).unwrap();
                let quit_sc = (0..ctrl.n_items()).filter_map(|i| ctrl.item(i).and_downcast::<gtk::Shortcut>()).find(|s| s.trigger().unwrap().to_str().as_str() == "<Control>q").unwrap();
                quit_sc.action().unwrap().activate(gtk::ShortcutActionFlags::empty(), &bw, None);
                // The context menu pops up from the secondary button.
                let ctx = button("Context");
                let gesture = ctx
                    .observe_controllers()
                    .iter::<glib::Object>()
                    .filter_map(|c| c.ok())
                    .find_map(|c| c.downcast::<gtk::GestureClick>().ok())
                    .unwrap();
                gesture.emit_by_name::<()>("pressed", &[&1i32, &5.0f64, &5.0f64]);
            }),
        );
        step(
            1700,
            Box::new(|b, s| {
                s.push(status());
                let mut pops = Vec::new();
                for w in all_widgets() {
                    if let Ok(p) = w.downcast::<gtk::PopoverMenu>() {
                        if p.is_visible() {
                            pops.push(p);
                        }
                    }
                }
                s.push(format!("visible popovers: {}", pops.len()));
                let _ = b;
                for p in pops {
                    p.popdown();
                }
                gtk::Window::list_toplevels().iter().for_each(|w| {
                    if let Some(w) = w.downcast_ref::<gtk::Window>() {
                        w.close();
                    }
                });
            }),
        );
        let raw2 = raw;
        glib::timeout_add_local_once(Duration::from_millis(9000), move || raw2.quit());

        host.run().unwrap();
        let errs: Vec<String> = host.take_errors().into_iter().map(|e| e.message).collect();
        assert!(errs.is_empty(), "{errs:?}");
        let seen = seen.borrow();
        assert_eq!(seen[0], "{Open,Autosave,}{Recent[a.txt,b.txt,],Quit,}", "initial model");
        assert_eq!(seen[1], "|false|true|false");
        assert_eq!(seen[2], "open;|false|true|false", "activate");
        assert_eq!(seen[3], "open;|true|false|true", "check toggled natively and radio moved, both bound");
        assert_eq!(seen[4], "a.txt,b.txt,");
        assert_eq!(seen[5], "c.txt,b.txt,", "keyed reorder and insert reached the GMenu");
        assert_eq!(seen[6], "open;copy;quit;|true|false|true", "toolbar and shortcut");
        assert_eq!(seen[7], "visible popovers: 1", "the context menu is open");
    });
}
