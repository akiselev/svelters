//! X3 end to end on Qt: the real host runs the `fixtures/child-attrs` Svelte bundle. Cells are
//! placed by `parent:column`/`parent:row` attributes; a click reorders the keyed list and changes a
//! span, and the QGridLayout follows reactively (each move is remove + insert, and the attributes
//! are applied again after the insert). Needs a display (xvfb-run), the main thread (no libtest
//! harness) and the built fixture (`pnpm -r build`); skipped without the bundle.

use counter_qt::hooks;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use svelters_abi::{BackendRef, NameKind};
use svelters_host::{Host, HostConfig};

const WIN: u64 = 1;
const GRID: u64 = 2;
const BUTTON: u64 = 6;

/// (column, row, column span, row span), the GTK test's order.
type Cell = (i32, i32, i32, i32);

fn cells() -> Vec<Cell> {
    (3..=6)
        .map(|h| {
            let (r, c, rs, cs) = hooks::grid_cell(GRID, h).unwrap_or_else(|| panic!("handle {h} is not in the grid"));
            (c, r, cs, rs)
        })
        .collect()
}

fn main() {
    counter_qt::run_as_test("a_grid_places_children_by_attributes_and_follows_reactive_changes", body);
}

fn body() {
    let bundle = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/child-attrs/dist/app.js");
    if !bundle.exists() {
        eprintln!("skipped: {} is not built", bundle.display());
        return;
    }
    let raw = unsafe { BackendRef::from_raw(counter_qt::backend()) }.unwrap();
    let cfg = HostConfig { echo_console: false, echo_errors: false, ..HostConfig::default() };
    let host = Host::new(raw, cfg).unwrap();
    host.load_bundle(bundle).unwrap();

    let seen: Rc<RefCell<Vec<Vec<Cell>>>> = Rc::default();
    let s = seen.clone();
    hooks::run_later(300, Box::new(move || {
        s.borrow_mut().push(cells());
        let button_kind = raw.resolve(NameKind::Kind, 0, "button");
        let click = raw.resolve(NameKind::Method, button_kind, "click");
        raw.call(BUTTON, click, &[]).unwrap();
    }));
    let s = seen.clone();
    hooks::run_later(700, Box::new(move || {
        s.borrow_mut().push(cells());
        hooks::close(WIN).unwrap();
    }));
    hooks::run_later(8000, Box::new(move || raw.quit())); // safety net: the host quits after the last window

    host.run().unwrap();
    let errs: Vec<String> = host.take_errors().into_iter().map(|e| e.message).collect();
    assert!(errs.is_empty(), "{errs:?}");
    let seen = seen.borrow();
    assert_eq!(seen.len(), 2, "both timers ran");
    // Labels a, b, c and the button: (column, row, column span, row span).
    assert_eq!(seen[0], vec![(0, 0, 1, 1), (1, 0, 1, 1), (0, 1, 1, 1), (0, 5, 1, 1)], "initial placement");
    // Rotated order b, c, a with b spanning two columns: b (0,0)+2, c (1,0), a (0,1); the button did not move.
    assert_eq!(seen[1], vec![(0, 1, 1, 1), (0, 0, 2, 1), (1, 0, 1, 1), (0, 5, 1, 1)], "after the click");
    println!("host_childattr ok");
}
