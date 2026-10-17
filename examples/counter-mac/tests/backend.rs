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
        frame_path_probe,
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
        no_leaked_handles,
    ];

    mod probe {
        use std::ffi::c_void;
        use std::sync::atomic::{AtomicU32, Ordering};
        pub static CV_TICKS: AtomicU32 = AtomicU32::new(0);
        type Cb = unsafe extern "C" fn(*mut c_void, *const c_void, *const c_void, u64, *mut u64, *mut c_void) -> i32;
        #[link(name = "CoreVideo", kind = "framework")]
        unsafe extern "C" {
            fn CVDisplayLinkCreateWithActiveCGDisplays(out: *mut *mut c_void) -> i32;
            fn CVDisplayLinkSetOutputCallback(l: *mut c_void, cb: Cb, ctx: *mut c_void) -> i32;
            fn CVDisplayLinkStart(l: *mut c_void) -> i32;
            fn CVDisplayLinkStop(l: *mut c_void) -> i32;
        }
        #[link(name = "CoreGraphics", kind = "framework")]
        unsafe extern "C" {
            fn CGMainDisplayID() -> u32;
            fn CGDisplayCopyDisplayMode(d: u32) -> *mut c_void;
            fn CGDisplayModeGetRefreshRate(m: *mut c_void) -> f64;
            fn CGDisplayPixelsWide(d: u32) -> usize;
        }
        unsafe extern "C" fn cb(_: *mut c_void, _: *const c_void, _: *const c_void, _: u64, _: *mut u64, _: *mut c_void) -> i32 {
            CV_TICKS.fetch_add(1, Ordering::Relaxed);
            0
        }
        /// (main display id, width, refresh rate, CVDisplayLink ticks in `ms`)
        pub fn cv(ms: u64) -> (u32, usize, f64, i32, u32) {
            unsafe {
                let d = CGMainDisplayID();
                let m = CGDisplayCopyDisplayMode(d);
                let hz = if m.is_null() { -1.0 } else { CGDisplayModeGetRefreshRate(m) };
                let mut l = std::ptr::null_mut();
                let rc = CVDisplayLinkCreateWithActiveCGDisplays(&mut l);
                if rc != 0 || l.is_null() {
                    return (d, CGDisplayPixelsWide(d), hz, rc, 0);
                }
                CVDisplayLinkSetOutputCallback(l, cb, std::ptr::null_mut());
                CVDisplayLinkStart(l);
                std::thread::sleep(std::time::Duration::from_millis(ms));
                CVDisplayLinkStop(l);
                (d, CGDisplayPixelsWide(d), hz, rc, CV_TICKS.load(Ordering::Relaxed))
            }
        }
    }

    pub struct TickIvars {
        pub n: std::cell::Cell<u32>,
    }

    objc2::define_class!(
        #[unsafe(super(objc2::runtime::NSObject))]
        #[thread_kind = objc2::MainThreadOnly]
        #[name = "FA1TickCounter"]
        #[ivars = TickIvars]
        pub struct TickCounter;

        impl TickCounter {
            #[unsafe(method(tick:))]
            fn tick(&self, _l: Option<&objc2::runtime::AnyObject>) {
                self.ivars().n.set(self.ivars().n.get() + 1);
            }
        }
    );

    /// Diagnostics for CI (only with SVELTERS_APPKIT_TRACE_FRAMES): which display-link setups tick
    /// on this machine, and how late run-loop timers are.
    fn frame_path_probe() {
        use objc2::rc::Retained;
        use objc2::runtime::{AnyObject, Sel};
        use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, class, msg_send, sel};
        if std::env::var_os("SVELTERS_APPKIT_TRACE_FRAMES").is_none() {
            return;
        }
        let h = Harness::shared();
        let mtm = MainThreadMarker::new().unwrap();
        eprintln!("[probe] CVDisplayLink (display, width, Hz, rc, ticks/300ms): {:?}", probe::cv(300));
        // Timers before any window exists: request_frame takes the 16 ms timer path.
        for _ in 0..3 {
            let before = h.frames();
            let t = std::time::Instant::now();
            h.backend_trait().request_frame();
            while h.frames() == before && t.elapsed().as_millis() < 1000 {
                h.pump(2);
            }
            eprintln!("[probe] no window: 16 ms frame timer fired after {:?}", t.elapsed());
            let c = h.checkpoints();
            let t = std::time::Instant::now();
            h.backend_trait().schedule_wake(16);
            while h.checkpoints() == c && t.elapsed().as_millis() < 1000 {
                h.pump(2);
            }
            eprintln!("[probe] no window: 16 ms schedule_wake fired after {:?}", t.elapsed());
        }
        let w = h.create("window");
        h.call(w, "make-key-and-order-front", vec![]).unwrap();
        h.pump(100);
        let win = h.backend.object(w).unwrap();
        let view: Retained<AnyObject> = unsafe { msg_send![&*win, contentView] };
        let screen: Option<Retained<AnyObject>> = unsafe { msg_send![&*win, screen] };
        let counter = || {
            let c = TickCounter::alloc(mtm).set_ivars(TickIvars { n: std::cell::Cell::new(0) });
            let c: Retained<TickCounter> = unsafe { msg_send![super(c), init] };
            c
        };
        let add = |link: &AnyObject, mode: &str| unsafe {
            let rl: Retained<AnyObject> = msg_send![class!(NSRunLoop), mainRunLoop];
            let m = objc2_foundation::NSString::from_str(mode);
            let _: () = msg_send![link, addToRunLoop: &*rl, forMode: &*m];
        };
        let run = |label: &str, src: &AnyObject, mode: &str, pump: &dyn Fn()| {
            let c = counter();
            let tick: Sel = sel!(tick:);
            let link: Option<Retained<AnyObject>> = unsafe { msg_send![src, displayLinkWithTarget: &*c, selector: tick] };
            let Some(link) = link else {
                eprintln!("[probe] {label}: no link");
                return;
            };
            add(&link, mode);
            let paused: bool = unsafe { msg_send![&*link, isPaused] };
            pump();
            let _: () = unsafe { msg_send![&*link, invalidate] };
            eprintln!("[probe] {label} ({mode}, paused at start={paused}): {} ticks", c.ivars().n.get());
        };
        let next_event = || h.pump(300);
        let cf_run = || {
            unsafe extern "C" {
                fn CFRunLoopRunInMode(mode: *const std::ffi::c_void, secs: f64, ret: u8) -> i32;
                static kCFRunLoopDefaultMode: *const std::ffi::c_void;
            }
            unsafe { CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.3, 0) };
        };
        run("view link, nextEvent", &view, "kCFRunLoopCommonModes", &next_event);
        run("view link, nextEvent", &view, "kCFRunLoopDefaultMode", &next_event);
        run("view link, CFRunLoopRunInMode", &view, "kCFRunLoopDefaultMode", &cf_run);
        run("window link", &win, "kCFRunLoopDefaultMode", &next_event);
        if let Some(s) = &screen {
            run("screen link", s, "kCFRunLoopDefaultMode", &next_event);
        }
        let _: () = unsafe { msg_send![&*view, setWantsLayer: true] };
        h.pump(50);
        run("layer-backed view link", &view, "kCFRunLoopDefaultMode", &next_event);
        unsafe {
            let app: Retained<AnyObject> = msg_send![class!(NSApplication), sharedApplication];
            let _: () = msg_send![&*app, finishLaunching];
            let _: () = msg_send![&*app, activateIgnoringOtherApps: true];
        }
        h.pump(100);
        run("after finishLaunching, view link", &view, "kCFRunLoopDefaultMode", &next_event);
        run("after finishLaunching, window link", &win, "kCFRunLoopDefaultMode", &next_event);
        h.destroy(w);
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
        // Off screen the display link does not tick: the watchdog still delivers exactly one.
        h.call(w, "order-out", vec![V::Null]).unwrap();
        h.backend_trait().request_frame();
        h.pump(150);
        assert_eq!(h.frames(), before + 3);
        h.pump(100);
        assert_eq!(h.frames(), before + 3, "nothing after the frame without a new request");
        h.destroy(w);
        h.backend_trait().request_frame();
        h.pump(150);
        assert_eq!(h.frames(), before + 4, "no window at all: the 16 ms timer");
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
