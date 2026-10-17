//! X3 end to end on GTK: the real host runs the `fixtures/child-attrs` Svelte bundle. Cells are
//! placed by `parent:column`/`parent:row` attributes; a click reorders the keyed list and changes a
//! span, and the grid follows reactively. Needs a display (xvfb-run) and the built fixture
//! (`pnpm -r build`); skipped without the bundle.

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

const WIN: u64 = 1;
const GRID: u64 = 2;
const BUTTON: u64 = 6;

type Cell = (i32, i32, i32, i32);

fn cell(b: &GtkBackend, h: u64) -> Cell {
    let grid = b.object(GRID).unwrap().downcast::<gtk::Grid>().unwrap();
    let w = b.object(h).unwrap().downcast::<gtk::Widget>().unwrap();
    grid.query_child(&w)
}

#[test]
fn a_grid_places_children_by_attributes_and_follows_reactive_changes() {
    let bundle = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/child-attrs/dist/app.js");
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

        let seen: Rc<RefCell<Vec<Vec<Cell>>>> = Rc::default();
        let (b, s) = (backend.clone(), seen.clone());
        glib::timeout_add_local_once(Duration::from_millis(300), move || {
            s.borrow_mut().push((3..=6).map(|h| cell(&b, h)).collect());
            b.object(BUTTON).unwrap().downcast::<gtk::Button>().unwrap().emit_clicked();
        });
        let (b, s) = (backend.clone(), seen.clone());
        glib::timeout_add_local_once(Duration::from_millis(700), move || {
            s.borrow_mut().push((3..=6).map(|h| cell(&b, h)).collect());
            b.object(WIN).unwrap().downcast::<gtk::Window>().unwrap().close();
        });
        let raw2 = raw;
        glib::timeout_add_local_once(Duration::from_millis(8000), move || raw2.quit());

        host.run().unwrap();
        let errs: Vec<String> = host.take_errors().into_iter().map(|e| e.message).collect();
        assert!(errs.is_empty(), "{errs:?}");
        let seen = seen.borrow();
        // (column, row, width, height) of labels a, b, c and the button.
        assert_eq!(seen[0], vec![(0, 0, 1, 1), (1, 0, 1, 1), (0, 1, 1, 1), (0, 5, 1, 1)], "initial placement");
        // Rotated order b, c, a with b spanning two columns: b (0,0)+2, c (1,0), a (0,1); the
        // button did not move.
        assert_eq!(seen[1], vec![(0, 1, 1, 1), (0, 0, 2, 1), (1, 0, 1, 1), (0, 5, 1, 1)], "after the click");
    });
}
