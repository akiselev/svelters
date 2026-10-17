//! X4 end to end on Qt: the real host runs the `fixtures/menus-qt` Svelte bundle (a menu bar with plain,
//! check and radio items, a keyed submenu, a tool bar and a context menu). Actions are triggered the
//! way a click does, a shortcut goes through Qt's shortcut map, and the context menu opens from a
//! real context-menu event; the app state, the label showing it and the native QActions are compared
//! at each step. Needs a display (xvfb-run), the main thread (no libtest harness) and the built
//! fixture (`pnpm -r build`); skipped without the bundle.

use counter_qt::hooks;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use svelters_abi::BackendRef;
use svelters_host::{Host, HostConfig};

const WIN: u64 = 1;

fn status() -> String {
    hooks::label_starting("S:").trim_start_matches("S:").to_string()
}

fn main() {
    counter_qt::run_as_test("the_menus_fixture_drives_native_menus_both_ways", body);
}

fn body() {
    let bundle = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/menus-qt/dist/app.js");
    if !bundle.exists() {
        eprintln!("skipped: {} is not built", bundle.display());
        return;
    }
    let raw = unsafe { BackendRef::from_raw(counter_qt::backend()) }.unwrap();
    let cfg = HostConfig { echo_console: false, echo_errors: false, ..HostConfig::default() };
    let host = Host::new(raw, cfg).unwrap();
    host.load_bundle(bundle).unwrap();

    let seen: Rc<RefCell<Vec<String>>> = Rc::default();
    let s = seen.clone();
    hooks::run_later(400, Box::new(move || {
        let mut s = s.borrow_mut();
        s.push(hooks::menu_dump(hooks::find_item("File")));
        s.push(status());
        assert!(hooks::trigger_item(hooks::find_item("Open")));
    }));
    let s = seen.clone();
    hooks::run_later(700, Box::new(move || {
        s.borrow_mut().push(status());
        assert!(hooks::trigger_item(hooks::find_item("Autosave")));
        assert!(hooks::trigger_item(hooks::find_item("Large")));
    }));
    let s = seen.clone();
    hooks::run_later(1000, Box::new(move || {
        let mut s = s.borrow_mut();
        s.push(status());
        s.push(hooks::menu_dump(hooks::find_item("Recent")));
        assert!(hooks::click_button("Shuffle"));
    }));
    let s = seen.clone();
    hooks::run_later(1300, Box::new(move || {
        let mut s = s.borrow_mut();
        s.push(hooks::menu_dump(hooks::find_item("Recent")));
        // The toolbar action, then the shortcut through the shortcut map, then a right click.
        assert!(hooks::trigger_item(hooks::find_item("Copy")));
        s.push(format!("shortcut ran: {}", hooks::shortcut(WIN, "Ctrl+Q")));
        assert!(hooks::context_request(hooks::find_button("Context")));
    }));
    let s = seen.clone();
    hooks::run_later(1700, Box::new(move || {
        let mut s = s.borrow_mut();
        s.push(status());
        s.push(format!("open menus: {}", hooks::open_menus()));
        s.push(hooks::menu_dump(hooks::find_item("Size")));
        hooks::close_menus();
        hooks::close(WIN).unwrap();
    }));
    hooks::run_later(8000, Box::new(move || raw.quit())); // safety net: the host quits after the last window

    host.run().unwrap();
    let errs: Vec<String> = host.take_errors().into_iter().map(|e| e.message).collect();
    assert!(errs.is_empty(), "{errs:?}");
    let seen = seen.borrow();
    assert_eq!(seen[0], "Open,Autosave,-,Recent[a.txt,b.txt,],Quit,", "initial menu");
    assert_eq!(seen[1], "|false|true|false");
    assert_eq!(seen[2], "open;|false|true|false", "activate");
    assert_eq!(seen[3], "open;|true|false|true", "check toggled and radio moved, both bound");
    assert_eq!(seen[4], "a.txt,b.txt,");
    assert_eq!(seen[5], "c.txt,b.txt,", "keyed reorder and insert reached the QMenu");
    assert_eq!(seen[6], "shortcut ran: true", "Ctrl+Q went through the shortcut map");
    assert_eq!(seen[7], "open;copy;quit;|true|false|true", "toolbar action and shortcut");
    assert_eq!(seen[8], "open menus: 1", "the context menu is open");
    assert_eq!(seen[9], "Small,Large*,", "radio items in the menu");
    println!("host_menus ok");
}
