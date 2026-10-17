//! X6 end to end on Qt: the real host runs the `fixtures/styles` Svelte bundle. Component styles,
//! `class:`, `style:`, tokens and the colour scheme all come from JS; the results are read back
//! natively (rendered pixels, palette, class property). Needs a display (xvfb-run), the main thread
//! (no libtest harness) and the built fixture (`pnpm -r build`); skipped without the bundle.
//!
//! Handles in the fixture: window #1, card box #2, label #3, toggle button #4, togglebutton #5,
//! theme button #6, accent button #7.

use counter_qt::hooks;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use svelters_abi::{BackendRef, NameKind};
use svelters_host::{Host, HostConfig};

const WIN: u64 = 1;
const CARD: u64 = 2;
const LABEL: u64 = 3;
const TOGGLE: u64 = 4;
const THEME: u64 = 6;
const ACCENT: u64 = 7;

fn px(h: u64, x: i32, y: i32) -> String {
    format!("{:06x}", hooks::pixel(h, x, y).unwrap_or_else(|| panic!("no pixel {x},{y} on #{h}")))
}

fn main() {
    counter_qt::run_as_test("styles_from_svelte_reach_qt_widgets_and_follow_tokens_and_scheme", body);
}

fn body() {
    let bundle = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/styles/dist/app.js");
    if !bundle.exists() {
        eprintln!("skipped: {} is not built", bundle.display());
        return;
    }
    let raw = unsafe { BackendRef::from_raw(counter_qt::backend()) }.unwrap();
    let cfg = HostConfig { echo_console: false, echo_errors: false, ..HostConfig::default() };
    let host = Host::new(raw, cfg).unwrap();
    host.load_bundle(bundle).unwrap();

    let click = move |h: u64| {
        let k = raw.resolve(NameKind::Kind, 0, "button");
        let m = raw.resolve(NameKind::Method, k, "click");
        raw.call(h, m, &[]).unwrap();
    };
    let log: Rc<RefCell<Vec<(&'static str, String)>>> = Rc::default();
    let rec = |l: &Rc<RefCell<Vec<(&'static str, String)>>>, k: &'static str, v: String| l.borrow_mut().push((k, v));

    let l = log.clone();
    hooks::run_later(400, Box::new(move || {
        rec(&l, "win bg (token --sv-bg)", px(WIN, 2, 2));
        rec(&l, "card bg (token --sv-surface)", px(CARD, 8, 8));
        rec(&l, "button bg (token --sv-accent)", px(TOGGLE, 20, 14));
        rec(&l, "label class", hooks::native_prop(LABEL, "class", false));
        click(TOGGLE);
    }));
    let l = log.clone();
    hooks::run_later(800, Box::new(move || {
        rec(&l, "label class after toggle", hooks::native_prop(LABEL, "class", false));
        rec(&l, "label fg (style:color)", format!("{:06x}", hooks::palette_color(LABEL, false).unwrap()));
        rec(&l, "button class after toggle", hooks::native_prop(TOGGLE, "class", false));
        rec(&l, "button border (class:active)", px(TOGGLE, 30, 0));
        click(THEME);
    }));
    let l = log.clone();
    hooks::run_later(1300, Box::new(move || {
        rec(&l, "win bg dark", px(WIN, 2, 2));
        rec(&l, "card bg dark", px(CARD, 8, 8));
        rec(&l, "window palette dark", format!("{:06x}", hooks::palette_color(WIN, true).unwrap()));
        click(ACCENT);
    }));
    let l = log.clone();
    hooks::run_later(1800, Box::new(move || {
        rec(&l, "button bg after setTokens", px(TOGGLE, 20, 14));
        hooks::close(WIN).unwrap();
    }));
    hooks::run_later(9000, Box::new(move || raw.quit())); // safety net

    host.run().unwrap();
    let errs: Vec<String> = host.take_errors().into_iter().map(|e| e.message).collect();
    assert!(errs.is_empty(), "{errs:?}");
    let seen = log.borrow();
    let get = |k: &str| seen.iter().find(|(n, _)| *n == k).unwrap_or_else(|| panic!("no record {k}: {seen:?}")).1.clone();
    assert_eq!(get("win bg (token --sv-bg)"), "f6f5f4");
    assert_eq!(get("card bg (token --sv-surface)"), "ffffff");
    assert_eq!(get("button bg (token --sv-accent)"), "3584e4");
    assert!(get("label class").contains("title") && !get("label class").contains("big"), "{}", get("label class"));
    assert!(get("label class after toggle").contains("big"));
    assert_eq!(get("label fg (style:color)"), "c01c28");
    assert!(get("button class after toggle").contains("active"));
    assert_eq!(get("button border (class:active)"), "c01c28", "2px --sv-danger border from button.primary.active");
    assert_eq!(get("win bg dark"), "242424");
    assert_eq!(get("card bg dark"), "303030");
    assert_eq!(get("window palette dark"), "242424");
    assert_eq!(get("button bg after setTokens"), "26a269");
    println!("host_style ok");
}
