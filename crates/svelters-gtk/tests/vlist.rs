//! M5 on GTK: the `virtuallist` kind end to end with the real host. JS (tests/js/vlist.js) plays the
//! list controller by hand; this test watches the native model and view. Needs a display
//! (xvfb-run); `GSK_RENDERER=cairo` avoids the GPU path.

use gtk::glib;
use gtk::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;
use svelters_abi::BackendRef;
use svelters_gtk::testing::on_gtk;
use svelters_gtk::GtkBackend;
use svelters_host::{Host, HostConfig};

const STATUS: u64 = 3;
const LIST: u64 = 4;

fn after(ms: u64, f: impl FnOnce() + 'static) {
    glib::timeout_add_local_once(Duration::from_millis(ms), f);
}

fn parse(label: &str) -> (String, HashMap<String, i64>) {
    let (kv, step) = label.split_once(";step=").unwrap_or((label, ""));
    let map = kv.split(';').filter_map(|p| p.split_once('=')).filter_map(|(k, v)| v.parse().ok().map(|n| (k.to_string(), n))).collect();
    (step.to_string(), map)
}

/// `(pos, removed, added)` per `items-changed`.
type Changes = Rc<RefCell<Vec<(u32, u32, u32)>>>;

fn view_of(b: &GtkBackend) -> (gtk::Widget, gtk::SelectionModel) {
    let sw = b.object(LIST).unwrap().downcast::<gtk::ScrolledWindow>().unwrap();
    let view = sw.child().unwrap();
    let sel = if let Some(v) = view.downcast_ref::<gtk::ListView>() { v.model() } else { view.downcast_ref::<gtk::GridView>().unwrap().model() };
    (view, sel.unwrap())
}

#[test]
fn virtual_list_protocol_on_gtk() {
    on_gtk(|| {
        let backend = Rc::new(GtkBackend::new().expect("gtk"));
        let raw = unsafe { BackendRef::from_raw(backend.as_raw()) }.unwrap();
        let cfg = HostConfig { echo_console: false, echo_errors: false, ..HostConfig::default() };
        let host = Host::new(raw, cfg).unwrap();
        host.load_bundle(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/js/vlist.js")).unwrap();

        let snaps: Rc<RefCell<HashMap<String, HashMap<String, i64>>>> = Rc::default();
        let changes: Changes = Rc::default();
        let sel_seen = Rc::new(RefCell::new((0u32, 0u32)));
        let handles_start = Rc::new(RefCell::new(0usize));

        // Poll the status label: every distinct `step=` is recorded once, and the native side
        // acts on the steps it cares about (so the test does not depend on timer drift).
        let b = backend.clone();
        let s = snaps.clone();
        let diag: Rc<RefCell<String>> = Rc::default();
        let dg = diag.clone();
        let c = changes.clone();
        let hs = handles_start.clone();
        let ss = sel_seen.clone();
        let quit = raw;
        let tick = glib::timeout_add_local(Duration::from_millis(30), move || {
            if let Some(l) = b.object(STATUS).and_then(|o| o.downcast::<gtk::Label>().ok()) {
                let (step, map) = parse(&l.label());
                if !step.is_empty() && !s.borrow().contains_key(&step) {
                    match step.as_str() {
                        "filled" => {
                            if let Some(w) = b.object(LIST).and_then(|o| o.downcast::<gtk::Widget>().ok()) {
                                let v = w.first_child();
                                *dg.borrow_mut() = format!("sw={}x{} view={:?} firstrow={:?}", w.width(), w.height(), v.as_ref().map(|v| (v.width(), v.height(), v.type_().name())), v.and_then(|v| v.first_child()).map(|r| (r.width(), r.height(), r.type_().name())));
                            }
                            let (_, sel) = view_of(&b);
                            let model = sel.downcast_ref::<gtk::SingleSelection>().unwrap().model().unwrap();
                            let c = c.clone();
                            model.connect_items_changed(move |_, p, r, a| c.borrow_mut().push((p, r, a)));
                        }
                        "done" => after(200, move || quit.quit()),
                        "scrolled-start" => *hs.borrow_mut() = b.live_handles(),
                        "selected-by-js" => {
                            // The user selects 7 natively; JS must hear it.
                            let (_, sel) = view_of(&b);
                            let single = sel.downcast_ref::<gtk::SingleSelection>().unwrap();
                            ss.borrow_mut().0 = single.selected();
                            single.set_selected(7);
                        }
                        _ => {}
                    }
                    s.borrow_mut().insert(step, map);
                }
            }
            glib::ControlFlow::Continue
        });
        let raw2 = raw;
        after(15000, move || raw2.quit()); // safety net
        host.run().unwrap();
        glib::source::SourceId::remove(tick);

        let errs: Vec<String> = host.take_errors().into_iter().map(|e| e.message).collect();
        assert!(errs.is_empty(), "{errs:?}");
        let snaps = snaps.borrow();
        let get = |step: &str, key: &str| snaps.get(step).unwrap_or_else(|| panic!("no snapshot `{step}`; have {:?}", snaps.keys().collect::<Vec<_>>()))[key];

        // Fill: 10k items natively, only a screenful of rows exist.
        assert_eq!(get("filled", "count"), 10000);
        let rows = get("filled", "rows");
        assert!((5..=400).contains(&rows), "a 320px window needs a handful of row slots, got {rows}: {:?} diag={} win={:?}", snaps.get("filled"), diag.borrow(), backend.object(1).and_then(|o| o.downcast::<gtk::Window>().ok()).map(|w| (w.width(), w.height())));
        assert_eq!(get("filled", "setups"), rows);
        assert!(get("filled", "bound") >= 5);
        assert_eq!(get("filled", "minId"), 1);
        // Scrolling to the end binds the last items, and does not create a row per item.
        assert_eq!(get("scrolled-end", "maxId"), 10000);
        assert!(get("scrolled-end", "rows") <= rows + 4, "rows are recycled: {} vs {rows}", get("scrolled-end", "rows"));
        assert!(get("scrolled-end", "binds") > get("filled", "binds"));
        assert_eq!(get("scrolled-start", "minId"), 1);
        // GTK may trim an oversized first pool (the first layout runs before the window has its size).
        assert!(get("scrolled-start", "rows") <= rows, "the pool does not grow when scrolling back");

        // Minimal notifications: four splices, four items-changed, nothing else.
        assert_eq!(changes.borrow()[..4], [(5, 0, 1), (2, 3, 0), (0, 1, 0), (9, 0, 1)], "{:?}", changes.borrow());
        assert_eq!(get("mutated", "count"), 10000 + 1 - 3 - 1 + 1);

        // Selection: JS writes are quiet, native changes arrive as `select`.
        assert_eq!(sel_seen.borrow().0, 3, "selected=3 from JS reached the selection model");
        assert_eq!(get("selected-by-js", "selected"), -2, "a JS write is not echoed back as an event");
        assert_eq!(get("selected-by-user", "selected"), 7);

        // Layout switch rebuilds the view: old rows are torn down, new ones set up.
        assert!(get("grid", "teardowns") >= rows, "switching layout tore the old rows down");
        assert_eq!(get("cleared", "count"), 0);
        assert_eq!(get("done", "rows"), 0);
        // Handles: everything the rows created is gone again.
        let end = backend.live_handles();
        assert!(end < *handles_start.borrow(), "row handles were released: {end} vs {}", handles_start.borrow());
        assert_eq!(end, 3, "only window, box and status label remain, got {end}");
    });
}
