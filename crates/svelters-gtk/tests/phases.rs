//! X3 on GTK: listener phases (`listen_ex`). `after` is `connect_after`; `capture` puts an event
//! controller into the capture phase. Needs a display (xvfb-run).

use gtk::prelude::*;
use svelters_abi::{SVL_LISTEN_AFTER, SVL_LISTEN_CAPTURE, Value};
use svelters_gtk::testing::{on_gtk, pump, Harness, SyncReply};

fn key_pressed(h: &Harness, ctrl: u64) -> bool {
    h.obj(ctrl).emit_by_name::<bool>("key-pressed", &[&97u32, &38u32, &gtk::gdk::ModifierType::empty()])
}

fn phase(h: &Harness, ctrl: u64) -> gtk::PropagationPhase {
    h.obj(ctrl).downcast_ref::<gtk::EventController>().unwrap().propagation_phase()
}

#[test]
fn after_listeners_run_after_plain_ones_whatever_the_connection_order() {
    on_gtk(|| {
        let h = Harness::new();
        let win = h.create("window");
        let entry = h.create("entry");
        let ctrl = h.create("EventControllerKey");
        h.insert(win, entry, None, None).unwrap();
        h.insert(entry, ctrl, None, None).unwrap();
        // The after listener is connected first and must still run last.
        h.listen_ex(ctrl, "key-pressed", true, SVL_LISTEN_AFTER).unwrap();
        h.listen(ctrl, "key-pressed", true).unwrap();
        h.set_sync_reply(SyncReply::Busy);
        key_pressed(&h, ctrl);
        let got: Vec<(String, u32)> = h.take_events_flags().into_iter().map(|(_, n, f)| (n, f)).collect();
        assert_eq!(got, vec![("key-pressed".to_string(), 0), ("key-pressed".to_string(), SVL_LISTEN_AFTER)]);

        // Each flags value reports itself in the sync call, and decides on its own.
        h.set_sync_reply(SyncReply::Value(Value::Bool(true)));
        assert!(key_pressed(&h, ctrl));
        let calls = h.take_sync_flags();
        assert_eq!(calls, vec![(ctrl, "key-pressed".to_string(), 0)], "the plain handler stopped the emission before the after handler");

        // Off: only the matching connection goes away.
        h.listen_ex(ctrl, "key-pressed", false, SVL_LISTEN_AFTER).unwrap();
        h.set_sync_reply(SyncReply::Busy);
        key_pressed(&h, ctrl);
        assert_eq!(h.take_events_flags().len(), 1);
        h.listen(ctrl, "key-pressed", false).unwrap();
        key_pressed(&h, ctrl);
        assert!(h.take_events_flags().is_empty());
    });
}

#[test]
fn capture_puts_the_controller_in_the_capture_phase_while_it_is_listened() {
    on_gtk(|| {
        let h = Harness::new();
        let win = h.create("window");
        let entry = h.create("entry");
        let ctrl = h.create("EventControllerKey");
        h.insert(win, entry, None, None).unwrap();
        h.insert(entry, ctrl, None, None).unwrap();
        assert_eq!(phase(&h, ctrl), gtk::PropagationPhase::Bubble);

        h.listen_ex(ctrl, "key-pressed", true, SVL_LISTEN_CAPTURE).unwrap();
        assert_eq!(phase(&h, ctrl), gtk::PropagationPhase::Capture);
        h.listen_ex(ctrl, "key-released", true, SVL_LISTEN_CAPTURE).unwrap();
        h.listen_ex(ctrl, "key-pressed", false, SVL_LISTEN_CAPTURE).unwrap();
        assert_eq!(phase(&h, ctrl), gtk::PropagationPhase::Capture, "another capture listener remains");
        h.listen_ex(ctrl, "key-released", false, SVL_LISTEN_CAPTURE).unwrap();
        assert_eq!(phase(&h, ctrl), gtk::PropagationPhase::Bubble, "the previous phase is restored");

        // The phase the app chose itself is what comes back.
        h.set_str(ctrl, "propagation-phase", "target").unwrap();
        h.listen_ex(ctrl, "key-pressed", true, SVL_LISTEN_CAPTURE).unwrap();
        h.set_sync_reply(SyncReply::Busy);
        key_pressed(&h, ctrl);
        assert_eq!(h.take_events_flags(), vec![(ctrl, "key-pressed".to_string(), SVL_LISTEN_CAPTURE)]);
        h.listen_ex(ctrl, "key-pressed", false, SVL_LISTEN_CAPTURE).unwrap();
        assert_eq!(phase(&h, ctrl), gtk::PropagationPhase::Target);

        // A capture listener and an after listener are two more connections of their own.
        h.listen_ex(ctrl, "key-pressed", true, SVL_LISTEN_CAPTURE | SVL_LISTEN_AFTER).unwrap();
        key_pressed(&h, ctrl);
        assert_eq!(h.take_events_flags(), vec![(ctrl, "key-pressed".to_string(), SVL_LISTEN_CAPTURE | SVL_LISTEN_AFTER)]);
        h.listen_ex(ctrl, "key-pressed", false, SVL_LISTEN_CAPTURE | SVL_LISTEN_AFTER).unwrap();
        pump(10);
    });
}

#[test]
fn capture_is_refused_where_it_has_no_native_meaning() {
    on_gtk(|| {
        let h = Harness::new();
        let button = h.create("button");
        let err = h.listen_ex(button, "clicked", true, SVL_LISTEN_CAPTURE).unwrap_err();
        assert!(err.contains("event controller"), "{err}");
        // After works on any signal.
        h.listen_ex(button, "clicked", true, SVL_LISTEN_AFTER).unwrap();
        assert!(h.listen_ex(button, "clicked", true, 8).unwrap_err().contains("flags"));
        h.listen_ex(button, "clicked", false, SVL_LISTEN_AFTER).unwrap();
    });
}
