//! Handles whose GType the generated tables do not name (FU1): a member id from the tables used on
//! such a handle fails with the normal error (`rc -1`, message naming the runtime GType) instead of
//! panicking while formatting it. Covers set_prop, get_prop, call, listen and observe on a plain
//! GObject subclass (`GMenu`, reflected kind) and on a type the probe skipped (`gtk_media_file_new`'s
//! `GtkNoMediaFile` or media-backend subclass). Needs a display (xvfb-run).

use gtk::prelude::*;
use std::ffi::CStr;
use svelters_abi::{svl_backend, svl_value};
use svelters_gtk::testing::{on_gtk, Harness, V};

fn vt(h: &Harness) -> &svl_backend {
    unsafe { &*h.backend.as_raw() }
}

fn err(h: &Harness) -> String {
    let vt = vt(h);
    unsafe { CStr::from_ptr((vt.last_error)(vt.self_)).to_string_lossy().into_owned() }
}

/// Every table-member path on `o` (whose runtime GType is `gtype`) returns -1 with a message
/// that names `gtype`.
fn check_paths(h: &Harness, o: u64, gtype: &str) {
    assert!(h.handle_info(o).is_ok(), "{gtype} handle");
    let label_prop = h.resolve_prop("label", "label");
    let clicked = h.resolve_signal("button", "clicked");
    let bar = h.create("progress-bar");
    let pulse = h.resolve_method(bar, "pulse");
    assert!(label_prop != 0 && clicked != 0 && pulse != 0);
    let vt = vt(h);

    let v = svl_value::null();
    let rc = unsafe { (vt.set_prop)(vt.self_, o, label_prop, &v) };
    assert_eq!(rc, -1, "set_prop");
    let m = err(h);
    assert!(m.contains("does not belong to") && m.contains(gtype), "set_prop: {m}");

    let mut out = svl_value::null();
    let rc = unsafe { (vt.get_prop)(vt.self_, o, label_prop, &mut out) };
    assert_eq!(rc, -1, "get_prop");
    let m = err(h);
    assert!(m.contains("does not belong to") && m.contains(gtype), "get_prop: {m}");

    let mut out = svl_value::null();
    let rc = unsafe { (vt.call)(vt.self_, o, pulse, std::ptr::null(), 0, &mut out) };
    assert_eq!(rc, -1, "call");
    let m = err(h);
    assert!(m.contains("does not belong to") && m.contains(gtype), "call: {m}");

    let rc = unsafe { (vt.listen)(vt.self_, o, clicked, true) };
    assert_eq!(rc, -1, "listen");
    let m = err(h);
    assert!(m.contains("does not belong to") && m.contains(gtype), "listen: {m}");

    let (rc, m) = h.observe_id(o, label_prop, true).unwrap_err();
    assert_eq!(rc, -1, "observe");
    assert!(m.contains("does not belong to") && m.contains(gtype), "observe: {m}");

    // The handle still works for its own (reflected) members afterwards.
    assert!(h.handle_info(o).is_ok());
}

#[test]
fn plain_gobject_subclass_outside_the_tables_errors() {
    on_gtk(|| {
        let h = Harness::new();
        let menu = h.create("Gio.Menu");
        check_paths(&h, menu, "GMenu");
    });
}

#[test]
fn probe_skipped_type_outside_the_tables_errors() {
    on_gtk(|| {
        let h = Harness::new();
        let f = match h.call_static("Gtk.MediaFile", "new", &[]) {
            Ok(V::Handle(f)) => f,
            other => panic!("Gtk.MediaFile.new: {other:?}"),
        };
        // GtkNoMediaFile without a media backend, else the backend's own private subclass
        // (GtkGstMediaFile...): neither is in the tables.
        let gtype = h.obj(f).type_().name().to_string();
        assert!(svelters_gtk::generated::tables::KINDS.iter().all(|k| k.gtype != gtype), "{gtype} is in the tables");
        check_paths(&h, f, &gtype);
    });
}
