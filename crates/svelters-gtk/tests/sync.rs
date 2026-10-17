//! X2 on GTK: sync signal handlers (`call_sync`) and the frame clock, at the backend level with a
//! scripted host answer. The full JS path is in `host_sync.rs`. Needs a display (xvfb-run).

use gtk::prelude::*;
use svelters_abi::{SVL_GONE_CLOSED, SVL_GONE_PARENT};
use svelters_gtk::testing::{on_gtk, pump, Harness, SyncReply, V};

fn key_pressed(h: &Harness, ctrl: u64, keyval: u32) -> bool {
    h.obj(ctrl).emit_by_name::<bool>("key-pressed", &[&keyval, &38u32, &gtk::gdk::ModifierType::empty()])
}

#[test]
fn close_request_decides_per_event_through_call_sync() {
    on_gtk(|| {
        let h = Harness::new();
        let win = h.create("window");
        let label = h.create("label");
        h.insert(win, label, None, None).unwrap();
        h.call(win, "present").unwrap();
        pump(150);
        h.listen(win, "close-request", true).unwrap();

        // The handler vetoes: the window stays and nothing is posted asynchronously.
        h.set_sync_reply(SyncReply::Value(V::Bool(true)));
        h.widget(win).downcast::<gtk::Window>().unwrap().close();
        pump(100);
        assert_eq!(h.take_sync_calls(), vec![(win, "close-request".to_string(), vec![])]);
        assert!(h.take_events().is_empty());
        assert!(h.take_gone().is_empty());
        assert!(h.handle_info(win).is_ok(), "vetoed: the window is still there");

        // No decision (null): the toolkit default, the window closes natively and the cascade runs.
        h.set_sync_reply(SyncReply::Value(V::Null));
        h.widget(win).downcast::<gtk::Window>().unwrap().close();
        pump(100);
        assert_eq!(h.take_gone(), vec![(win, SVL_GONE_CLOSED), (label, SVL_GONE_PARENT)]);
        assert!(h.handle_info(win).is_err());
    });
}

#[test]
fn close_request_falls_back_to_the_veto_when_the_host_is_busy_and_closes_on_failure() {
    on_gtk(|| {
        let h = Harness::new();
        let win = h.create("window");
        h.call(win, "present").unwrap();
        pump(100);
        h.listen(win, "close-request", true).unwrap();

        // Busy: the host queued the event (the harness records it as one); the window is vetoed,
        // as in ABI v2, so the late handler can still decide.
        h.set_sync_reply(SyncReply::Busy);
        h.widget(win).downcast::<gtk::Window>().unwrap().close();
        pump(100);
        assert_eq!(h.take_events(), vec![(win, "close-request".to_string())]);
        assert!(h.handle_info(win).is_ok());

        // A failing call (no handler ran): the default close.
        h.set_sync_reply(SyncReply::Failed);
        h.widget(win).downcast::<gtk::Window>().unwrap().close();
        pump(100);
        assert_eq!(h.take_gone(), vec![(win, SVL_GONE_CLOSED)]);
    });
}

#[test]
fn key_controller_result_stops_default_handling() {
    on_gtk(|| {
        let h = Harness::new();
        let win = h.create("window");
        let entry = h.create("entry");
        let ctrl = h.create("EventControllerKey");
        h.insert(win, entry, None, None).unwrap();
        h.insert(entry, ctrl, None, None).unwrap();
        h.listen(ctrl, "key-pressed", true).unwrap();
        h.call(win, "present").unwrap();
        pump(100);
        assert!(h.obj(ctrl).downcast_ref::<gtk::EventController>().unwrap().widget().is_some(), "attached to the entry");

        // handled -> true: GTK stops propagation and the entry's own handling does not run
        h.set_sync_reply(SyncReply::Value(V::Bool(true)));
        assert!(key_pressed(&h, ctrl, 97));
        let calls = h.take_sync_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!((calls[0].0, calls[0].1.as_str()), (ctrl, "key-pressed"));
        assert_eq!(calls[0].2[..2], [V::I64(97), V::I64(38)]);
        // no decision, explicit false, a failed call and a busy host all let GTK carry on
        h.set_sync_reply(SyncReply::Value(V::Null));
        assert!(!key_pressed(&h, ctrl, 98));
        h.set_sync_reply(SyncReply::Value(V::Bool(false)));
        assert!(!key_pressed(&h, ctrl, 98));
        h.set_sync_reply(SyncReply::Failed);
        assert!(!key_pressed(&h, ctrl, 98));
        h.set_sync_reply(SyncReply::Busy);
        assert!(!key_pressed(&h, ctrl, 99));
        assert_eq!(h.take_events().len(), 1, "the busy call was queued as an ordinary event");

        // the controller detaches like a child
        h.remove(entry, ctrl).unwrap();
        assert!(h.obj(ctrl).downcast_ref::<gtk::EventController>().unwrap().widget().is_none());
        assert!(h.remove(entry, ctrl).is_err());
    });
}

#[test]
fn request_frame_ticks_on_the_gdk_frame_clock_once_per_request() {
    on_gtk(|| {
        let h = Harness::new();
        let win = h.create("window");
        h.call(win, "present").unwrap();
        pump(300);

        h.request_frame();
        h.request_frame(); // idempotent while pending
        assert_eq!(h.take_frames(), 0, "never synchronous");
        pump(150);
        assert_eq!(h.take_frames(), 1);
        assert_eq!(h.backend.frames_by_clock(), 1, "the mapped window's frame clock delivered it");

        pump(100);
        assert_eq!(h.take_frames(), 0, "one frame per request");
        h.request_frame();
        pump(150);
        assert_eq!(h.take_frames(), 1);
        assert_eq!(h.backend.frames_by_clock(), 2);
    });
}

#[test]
fn request_frame_without_a_mapped_window_uses_the_fallback_timer() {
    on_gtk(|| {
        let h = Harness::new();
        h.request_frame();
        pump(100);
        assert_eq!(h.take_frames(), 1);
        assert_eq!(h.backend.frames_by_clock(), 0);
    });
}
