//! Hand-written AppKit backend tests (macOS; an empty suite elsewhere): handle lifetime, borrowed
//! handles, the dynamic bridge, the run loop (wake, schedule_wake, request_frame, run/quit) and
//! the window-close cascade. The per-fact tests are generated (tests/generated.rs).

fn main() {
    #[cfg(target_os = "macos")]
    svelters_apple::testing::run_cases(true, mac::CASES);
    #[cfg(not(target_os = "macos"))]
    svelters_apple::testing::run_cases(false, &[]);
}

#[cfg(target_os = "macos")]
mod mac {
    use svelters_abi::Value as V;
    use svelters_abi::*;
    use svelters_appkit::testing::Harness;

    pub static CASES: &[(&str, fn())] = &[
        ("handles_are_never_reused", handles_are_never_reused),
        ("unknown_names_resolve_to_zero", unknown_names_resolve_to_zero),
        ("stale_handle_is_an_error", stale_handle_is_an_error),
        ("borrowed_handle_from_getter", borrowed_handle_from_getter),
        ("dynamic_prop_and_method", dynamic_prop_and_method),
        ("string_props_take_numbers", string_props_take_numbers),
        ("read_only_prop_is_refused", read_only_prop_is_refused),
        ("enum_names_round_trip", enum_names_round_trip),
        ("destroy_window_reports_nothing", destroy_window_reports_nothing),
        ("close_cascades_to_children", close_cascades_to_children),
        ("busy_host_vetoes_close", busy_host_vetoes_close),
        ("schedule_wake_runs_a_checkpoint", schedule_wake_runs_a_checkpoint),
        ("wake_from_another_thread", wake_from_another_thread),
        ("request_frame_delivers_one_frame", request_frame_delivers_one_frame),
        ("run_returns_after_quit", run_returns_after_quit),
        ("no_leaked_handles", no_leaked_handles),
    ];

    fn handles_are_never_reused() {
        let h = Harness::shared();
        let a = h.create("view");
        h.destroy(a);
        h.destroy(a); // idempotent
        let b = h.create("view");
        assert!(b > a);
        assert!(!h.alive(a));
        h.destroy(b);
    }

    fn unknown_names_resolve_to_zero() {
        let h = Harness::shared();
        let b = h.backend_trait();
        assert_eq!(b.resolve(NameKind::Kind, 0, "no-such-tag"), 0);
        assert_eq!(b.resolve(NameKind::Kind, 0, "control"), 0, "NSControl is not an element");
        let k = b.resolve(NameKind::Kind, 0, "button");
        assert_eq!(b.resolve(NameKind::Prop, k, "no-such-prop"), 0);
        assert_eq!(b.resolve(NameKind::Signal, k, "clicked"), 0, "AppKit names only: the signal is `action`");
        assert!(b.resolve(NameKind::Signal, k, "action") != 0);
    }

    fn stale_handle_is_an_error() {
        let h = Harness::shared();
        let a = h.create("button");
        h.destroy(a);
        assert!(h.set(a, "title", V::Str("x".into())).is_err());
        assert!(h.listen(a, "action", true).is_err());
    }

    fn borrowed_handle_from_getter() {
        let h = Harness::shared();
        let w = h.create("window");
        let V::Handle(cv) = h.get(w, "content-view").unwrap() else { panic!("content-view is a handle") };
        assert_eq!(h.lifetime(cv), SVL_LIFE_BORROWED);
        assert_eq!(h.get(w, "content-view").unwrap(), V::Handle(cv), "one handle per native object");
        let s = h.create("stack-view");
        assert!(h.insert(cv, s, None, None).is_err(), "borrowed handles are not parents");
        h.set(cv, "hidden", V::Bool(true)).unwrap();
        assert_eq!(h.get(cv, "hidden").unwrap(), V::Bool(true));
        h.destroy(cv); // forgets only
        assert!(h.alive(w));
        h.destroy(s);
        h.destroy(w);
        assert!(h.take_gone().is_empty());
    }

    fn dynamic_prop_and_method() {
        let h = Harness::shared();
        let b = h.create("button");
        // `identifier` comes from the NSUserInterfaceItemIdentification protocol, not the tables.
        h.set(b, "identifier", V::Str("ok-button".into())).unwrap();
        assert_eq!(h.get(b, "identifier").unwrap(), V::Str("ok-button".into()));
        h.set(b, "title", V::Str("A much longer title".into())).unwrap();
        // `sizeToFit` is not annotated: found in the runtime and called through NSInvocation.
        h.call(b, "size-to-fit", vec![]).unwrap();
        let V::Record { fields, .. } = h.get(b, "frame").unwrap() else { panic!("frame is a Rect") };
        assert!(matches!(fields[2], V::F64(w) if w > 50.0), "sized to the title: {fields:?}");
        assert!(h.call(b, "init", vec![]).is_err(), "owning families are refused");
        h.destroy(b);
    }

    fn string_props_take_numbers() {
        let h = Harness::shared();
        let t = h.create("text-field");
        h.set(t, "string-value", V::I64(42)).unwrap();
        assert_eq!(h.get(t, "string-value").unwrap(), V::Str("42".into()));
        h.set(t, "string-value", V::Null).unwrap();
        assert_eq!(h.get(t, "string-value").unwrap(), V::Str("".into()));
        h.destroy(t);
    }

    fn read_only_prop_is_refused() {
        let h = Harness::shared();
        let v = h.create("view");
        let e = h.set(v, "subviews", V::List(vec![])).unwrap_err();
        assert!(e.contains("read-only"), "{e}");
        h.destroy(v);
    }

    fn enum_names_round_trip() {
        let h = Harness::shared();
        let s = h.create("stack-view");
        h.set(s, "orientation", V::I64(1)).unwrap();
        assert_eq!(h.get(s, "orientation").unwrap(), V::Str("vertical".into()));
        let e = h.set(s, "orientation", V::Str("diagonal".into())).unwrap_err();
        assert!(e.contains("horizontal"), "{e}");
        h.destroy(s);
    }

    fn destroy_window_reports_nothing() {
        let h = Harness::shared();
        h.take_gone();
        let w = h.create("window");
        h.call(w, "make-key-and-order-front", vec![]).unwrap();
        h.destroy(w);
        h.pump(30);
        assert!(h.take_gone().is_empty(), "the app's own destroy is silent");
    }

    fn close_cascades_to_children() {
        let h = Harness::shared();
        h.take_gone();
        let w = h.create("window");
        let s = h.create("stack-view");
        let b = h.create("button");
        h.insert(s, b, None, None).unwrap();
        h.insert(w, s, None, None).unwrap();
        h.call(w, "make-key-and-order-front", vec![]).unwrap();
        h.close_window(w);
        h.pump(30);
        let gone = h.take_gone();
        assert_eq!(gone, vec![(w, SVL_GONE_CLOSED), (s, SVL_GONE_PARENT), (b, SVL_GONE_PARENT)]);
        assert!(!h.alive(w) && !h.alive(s) && !h.alive(b));
    }

    fn busy_host_vetoes_close() {
        // The recording host never answers busy; this checks the listened path end to end with a
        // `false` result (allow), the complement of the generated veto test.
        let h = Harness::shared();
        h.take_gone();
        let w = h.create("window");
        h.call(w, "make-key-and-order-front", vec![]).unwrap();
        h.listen(w, "window-should-close", true).unwrap();
        h.set_sync_reply(V::Bool(false));
        h.perform_close(w);
        h.pump(30);
        assert!(!h.alive(w), "false = do not prevent");
        assert_eq!(h.take_gone(), vec![(w, SVL_GONE_CLOSED)]);
        h.set_sync_reply(V::Null);
    }

    fn schedule_wake_runs_a_checkpoint() {
        let h = Harness::shared();
        let before = h.checkpoints();
        h.backend_trait().schedule_wake(20);
        h.pump(120);
        assert_eq!(h.checkpoints(), before + 1, "exactly one checkpoint");
        h.backend_trait().schedule_wake(20);
        h.backend_trait().schedule_wake(-1);
        h.pump(80);
        assert_eq!(h.checkpoints(), before + 1, "cancelled");
    }

    fn wake_from_another_thread() {
        let h = Harness::shared();
        let before = h.checkpoints();
        let w = h.backend_trait().waker();
        std::thread::spawn(move || {
            w();
            w(); // coalesces
        })
        .join()
        .unwrap();
        h.pump(50);
        assert_eq!(h.checkpoints(), before + 1);
    }

    fn request_frame_delivers_one_frame() {
        let h = Harness::shared();
        let w = h.create("window");
        h.call(w, "make-key-and-order-front", vec![]).unwrap();
        let before = h.frames();
        h.backend_trait().request_frame();
        h.backend_trait().request_frame(); // idempotent while pending
        h.pump(150);
        assert_eq!(h.frames(), before + 1);
        h.backend_trait().request_frame();
        h.pump(150);
        assert_eq!(h.frames(), before + 2);
        h.destroy(w);
    }

    fn run_returns_after_quit() {
        let h = Harness::shared();
        let t = std::time::Instant::now();
        h.run_until_quit(30).unwrap();
        assert!(t.elapsed() < std::time::Duration::from_secs(5));
    }

    fn no_leaked_handles() {
        let h = Harness::shared();
        h.pump(20);
        assert_eq!(h.backend.live_handles(), 0, "every test destroyed what it created");
    }
}
