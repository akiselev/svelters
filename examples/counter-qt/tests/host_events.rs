//! X8 end to end on Qt: the real host runs a JS bundle whose handlers decide event-family signals
//! (`mouse-press` plain and in the capture phase, `wheel`), see a posted `resize` and a `mouse-move`
//! on a `canvas`, and the canvas paints the display list JS set. Events are native (`hooks::send_mouse`).
//! Needs a display (xvfb-run) and the main thread, so no libtest harness.

use counter_qt::hooks;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use svelters_abi::BackendRef;
use svelters_host::{Host, HostConfig};

const WIN: u64 = 1;
const AREA: u64 = 4;
const CANVAS: u64 = 5;

#[derive(Default)]
struct Seen {
    dump: String,
    green: Option<u32>,
    magenta: Option<u32>,
    empty: Option<u32>,
    late_dump: String,
}

fn main() {
    counter_qt::run_as_test("js_handlers_decide_event_families_and_the_canvas_paints", body);
}

fn body() {
    let raw = unsafe { BackendRef::from_raw(counter_qt::backend()) }.unwrap();
    let cfg = HostConfig { echo_console: false, echo_errors: false, ..HostConfig::default() };
    let host = Host::new(raw, cfg).unwrap();
    host.load_bundle(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/js/events_e2e.js")).unwrap();

    let seen = Rc::new(RefCell::new(Seen::default()));
    let s = seen.clone();
    hooks::run_later(400, Box::new(move || {
        hooks::send_mouse(AREA, 0, 7.0, 9.0, 1).unwrap(); // left: consumed by the capturing ancestor
        hooks::send_mouse(AREA, 0, 3.0, 4.0, 2).unwrap(); // right: through the capture, consumed by the area
        hooks::send_mouse(AREA, 3, 1.0, 1.0, 120).unwrap(); // wheel up
        hooks::send_mouse(CANVAS, 2, 4.0, 5.0, 0).unwrap(); // move over the canvas
        let mut st = s.borrow_mut();
        st.green = hooks::pixel(CANVAS, 5, 5);
        st.magenta = hooks::pixel(CANVAS, 35, 5);
        st.empty = hooks::pixel(CANVAS, 60, 30);
        st.dump = hooks::dump(WIN);
    }));
    let s = seen.clone();
    hooks::run_later(700, Box::new(move || {
        s.borrow_mut().late_dump = hooks::dump(WIN);
        let _ = hooks::close(WIN);
    }));
    let raw2 = raw;
    hooks::run_later(8000, Box::new(move || raw2.quit()));

    host.run().unwrap();
    let errs: Vec<String> = host.take_errors().into_iter().map(|e| e.message).collect();
    assert!(errs.is_empty(), "{errs:?}");
    let st = seen.borrow();
    let dump = &st.dump;
    assert!(dump.contains("capture {\\\"x\\\":7,\\\"y\\\":9} b1|capture {\\\"x\\\":3,\\\"y\\\":4} b2|press {\\\"x\\\":3,\\\"y\\\":4} b2"), "positions are records, the capture handler runs first and consumes the left press: {dump}");
    assert!(!dump.contains("press {\\\"x\\\":7"), "the consumed press never reached the area's own listener: {dump}");
    assert!(dump.contains("wheel {\\\"x\\\":0,\\\"y\\\":120}"), "the wheel delta is a Point record: {dump}");
    assert!(dump.contains("mouse-move [{\\\"x\\\":4,\\\"y\\\":5},0,0,0]"), "mouse-move without a pressed button (tracking is on): {dump}");
    assert!(st.late_dump.contains("async resize [{\\\"width\\\":"), "resize is posted with a Size record: {}", st.late_dump);
    assert_eq!(st.green, Some(0x00ff00), "canvas ops replayed (fillRect)");
    assert_eq!(st.magenta, Some(0xff00ff), "canvas ops replayed (second fillRect)");
    assert_ne!(st.empty, Some(0x00ff00), "outside the drawn rectangles");
    println!("host_events ok");
}
