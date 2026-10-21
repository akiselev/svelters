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

    /// Each case runs under `Harness::case_guard`: a case that panics has its handles destroyed,
    /// so it cannot also fail `no_leaked_handles`.
    macro_rules! guarded {
        ($($name:ident),* $(,)?) => {
            pub static CASES: &[(&str, fn())] = &[$((stringify!($name), {
                fn guarded() {
                    let _g = Harness::shared().case_guard();
                    $name()
                }
                guarded
            })),*];
        };
    }

    guarded![
        handles_are_never_reused,
        unknown_names_resolve_to_zero,
        stale_handle_is_an_error,
        borrowed_handle_from_getter,
        dynamic_prop_and_method,
        string_props_take_numbers,
        read_only_prop_is_refused,
        refused_replace_keeps_the_old_child,
        enum_names_round_trip,
        destroy_window_reports_nothing,
        close_cascades_to_children,
        busy_host_vetoes_close,
        schedule_wake_runs_a_checkpoint,
        wake_from_another_thread,
        request_frame_delivers_one_frame,
        run_returns_after_quit,
        observe_typing_into_text_field,
        observe_kvo_from_another_thread_hops,
        observe_borrowed_object_death,
        observe_codes_and_observable_flag,
        no_leaked_handles,
    ];

    // ---- ABI v2.6 observe (milestone R4) ----

    /// REACTIVITY.md 10 R4 acceptance: typing into an NSTextField (key events through -[NSWindow sendEvent:], the
    /// field editor, NSControlTextDidChangeNotification) is observed on `string-value`, which then reads the text.
    fn observe_typing_into_text_field() {
        let h = Harness::shared();
        let f = h.create("text-field");
        let w = h.mount(f);
        let pid = h.prop_id(f, "string-value");
        h.observe(f, "string-value", true).unwrap();
        assert!(h.focus(f), "the field became first responder");
        h.take_changed(f, pid);
        h.type_keys(f, "abc", 0);
        h.pump(10);
        let n = h.take_changed(f, pid);
        println!("typing 3 keys announced string-value {n} time(s)");
        assert!(n >= 3, "every keystroke is announced (notification): {n}");
        assert_eq!(h.get(f, "string-value").unwrap(), V::Str("abc".into()), "the observed prop reads the typed text");
        h.observe(f, "string-value", false).unwrap();
        h.type_keys(f, "d", 0);
        h.pump(10);
        assert_eq!(h.take_changed(f, pid), 0, "not announced after observe(off)");
        h.destroy(w);
        h.destroy(f);
    }

    /// REACTIVITY.md 5.6: KVO notifies on the thread that made the change. From another thread the change is queued
    /// (deduplicated per (h, prop)) and delivered on the main thread by the next wake, never on the changing thread.
    fn observe_kvo_from_another_thread_hops() {
        let h = Harness::shared();
        let v = h.create("view");
        let pid = h.prop_id(v, "tool-tip");
        h.observe(v, "tool-tip", true).unwrap();
        h.take_changed(v, pid);
        let obj = h.retain(v);
        let addr = objc2::rc::Retained::as_ptr(&obj) as usize;
        std::thread::spawn(move || {
            objc2::rc::autoreleasepool(|_| {
                let o = addr as *const objc2::runtime::AnyObject;
                for i in 0..3 {
                    let s = objc2_foundation::NSString::from_str(&format!("tip {i}"));
                    let key = objc2_foundation::NSString::from_str("toolTip");
                    let _: () = unsafe { objc2::msg_send![&*o, setValue: &*s, forKey: &*key] };
                }
            })
        })
        .join()
        .unwrap();
        assert_eq!(h.take_changed(v, pid), 0, "not delivered on the changing thread");
        h.backend.run_observe_hops();
        assert_eq!(h.take_changed(v, pid), 1, "three changes on another thread: one delivery on the main thread");
        h.observe(v, "tool-tip", false).unwrap();
        drop(obj);
        h.destroy(v);
    }

    /// REACTIVITY.md 5.5, decision 13.10: a BORROWED object observed through KVO (NSView `hidden` is autonotifying)
    /// is not retained; when it deallocates, the identity sentinel reports it and the handle dies with
    /// SVL_GONE_OWNER (the runtime removed the KVO registration: macOS 10.13+ relaxed unregistration).
    fn observe_borrowed_object_death() {
        let h = Harness::shared();
        let w = h.create("window");
        let cv = objc2::rc::autoreleasepool(|_| {
            let V::Handle(cv) = h.get(w, "content-view").unwrap() else { panic!("content-view is a handle") };
            cv
        });
        assert_eq!(h.lifetime(cv), SVL_LIFE_BORROWED);
        h.observe(cv, "hidden", true).unwrap();
        h.take_gone();
        // Replace the content view: the window releases the old one, nothing else holds it.
        let nv = h.create("view");
        objc2::rc::autoreleasepool(|_| h.insert(w, nv, None, Some("content-view")).unwrap());
        h.pump(20);
        h.backend.run_observe_hops();
        let mut gone = h.take_gone();
        println!("after replacing the content view: object alive {}, gone {gone:?}", h.backend.object(cv).is_some());
        if gone.is_empty() {
            // The window still holds the old content view somewhere: closing and releasing the window frees it.
            objc2::rc::autoreleasepool(|_| h.destroy(w));
            h.pump(50);
            h.backend.run_observe_hops();
            gone = h.take_gone();
            println!("after destroying the window: object alive {}, gone {gone:?}", h.backend.object(cv).is_some());
        }
        assert!(gone.contains(&(cv, SVL_GONE_OWNER)), "the sentinel reported the borrowed object's death: {gone:?}");
        assert!(!h.alive(cv));
        h.destroy(nv);
        h.destroy(w);
    }

    /// Return codes (REACTIVITY.md 5.1) and SVL_M_OBSERVABLE in `describe` from the facts.
    fn observe_codes_and_observable_flag() {
        let h = Harness::shared();
        let f = h.create("text-field");
        let b = h.backend_trait();
        let kind = b.handle_info(f).unwrap().kind;
        let flags = |name: &str| b.describe(NameKind::Prop, kind, h.prop_id(f, name)).map_or(0, |d| d.flags);
        assert_ne!(flags("string-value") & SVL_M_OBSERVABLE, 0, "exact (KVO + notification)");
        assert_eq!(flags("font") & SVL_M_OBSERVABLE, 0, "set_only");
        match h.observe(f, "font", true) {
            Err(ObserveError::Unsupported(_)) => {}
            other => panic!("set_only answers SVL_OBSERVE_UNSUPPORTED, got {other:?}"),
        }
        // A prop found at run time has no fact.
        match h.observe(f, "identifier", true) {
            Err(ObserveError::Unsupported(_)) => {}
            other => panic!("a dynamic prop answers SVL_OBSERVE_UNSUPPORTED, got {other:?}"),
        }
        let pid = h.prop_id(f, "string-value");
        assert_eq!(b.observe(f, pid, false), Ok(()), "off for an unobserved prop is 0");
        h.destroy(f);
        assert!(matches!(b.observe(f, pid, true), Err(ObserveError::Failed(_))), "a dead handle is -1");
        assert_eq!(b.observe(f, pid, false), Ok(()), "off for a dead handle is 0");
    }

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
        let c = h.create("button");
        h.insert(v, c, None, None).unwrap();
        // NSView declares `subviews` readwrite, but it is the subviews slot's storage: refused
        // before the value is looked at, whatever its type.
        for val in [V::List(vec![]), V::Null, V::Handle(c)] {
            let e = h.set(v, "subviews", val).unwrap_err();
            assert!(e.contains("read-only") && e.contains("slot"), "{e}");
        }
        assert_eq!(h.native_list(v, "subviews"), vec![h.obj(c)], "children untouched");
        let s = h.create("stack-view");
        let e = h.set(s, "arranged-subviews", V::Str("x".into())).unwrap_err();
        assert!(e.contains("read-only"), "{e}");
        let w = h.create("window");
        let e = h.set(w, "content-view", V::Handle(c)).unwrap_err();
        assert!(e.contains("read-only") && e.contains("content-view"), "{e}");
        // A plain read-only prop: the check comes before conversion too.
        let e = h.set(v, "constraints", V::Str("not a list".into())).unwrap_err();
        assert!(e.contains("read-only"), "{e}");
        h.destroy(w);
        h.destroy(s);
        h.destroy(c);
        h.destroy(v);
    }

    fn refused_replace_keeps_the_old_child() {
        let h = Harness::shared();
        let w = h.create("window");
        let a = h.create("stack-view");
        let other = h.create("stack-view");
        let b = h.create("button");
        h.insert(w, a, None, None).unwrap();
        h.insert(other, b, None, None).unwrap();
        // `b` already has a parent: the replace is refused and `a` stays the content view.
        assert!(h.insert(w, b, None, None).is_err());
        assert_eq!(h.native_get(w, "contentView"), Some(h.obj(a)));
        h.remove(w, a).expect("a is still linked as the window's child");
        for x in [b, other, a, w] {
            h.destroy(x);
        }
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

    /// Pumps until `frames` reaches `want` or 1 s passes (CI runners fire CF timers tens of ms
    /// late), then a little longer to catch a second, unwanted frame.
    fn pump_frames(h: &Harness, want: u32) -> u32 {
        let t = std::time::Instant::now();
        while h.frames() < want && t.elapsed() < std::time::Duration::from_secs(1) {
            h.pump(5);
        }
        h.pump(120);
        h.frames()
    }

    fn request_frame_delivers_one_frame() {
        let h = Harness::shared();
        let w = h.create("window");
        h.call(w, "make-key-and-order-front", vec![]).unwrap();
        let before = h.frames();
        let (link0, _) = h.backend.frame_sources();
        h.backend_trait().request_frame();
        h.backend_trait().request_frame(); // idempotent while pending
        assert_eq!(pump_frames(h, before + 1), before + 1);
        h.backend_trait().request_frame();
        assert_eq!(pump_frames(h, before + 2), before + 2);
        // A visible window's display link really ticks (it was added to the run loop with a
        // look-alike mode string once, and never fired).
        assert_eq!(h.backend.frame_sources().0, link0 + 2, "both frames came from CADisplayLink");
        // Off screen the link does not tick: the watchdog timer still delivers exactly one.
        h.call(w, "order-out", vec![V::Null]).unwrap();
        h.backend_trait().request_frame();
        assert_eq!(pump_frames(h, before + 3), before + 3);
        h.destroy(w);
        // No window at all: the plain timer.
        h.backend_trait().request_frame();
        assert_eq!(pump_frames(h, before + 4), before + 4);
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
