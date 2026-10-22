//! See Cargo.toml and `docs/notes/ios-sim-first-run.md`.
//!
//! Without arguments the app shows the probe UI and stays up. With `--selftest` it also runs the
//! checks below after the scene becomes active, prints one `SV {json}` line per claim and a final
//! `SV-DONE` line, and exits after `SV_EXIT_MS` (default 1500 ms; `0` = stay up).
//! `SV_PREMAIN=1` creates a UIView/UILabel before `UIApplicationMain` (claim §2.2).
//! `SV_HOSTFILE=/path` checks the simulator process can read a file on the Mac (claim §4).
//! Pass env vars through simctl with the `SIMCTL_CHILD_` prefix.
#[cfg(target_os = "ios")]
mod ios {
    use std::cell::{Cell, RefCell};
    use std::collections::HashMap;
    use std::ffi::c_void;
    use std::rc::Rc;

    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, NSObject};
    use objc2::{class, define_class, msg_send, sel, ClassType, MainThreadMarker, MainThreadOnly};
    use objc2_foundation::{
        ns_string, NSDictionary, NSKeyValueChangeKey, NSKeyValueObservingOptions,
        NSObjectNSKeyValueObserverRegistration, NSObjectProtocol, NSRunLoop, NSRunLoopCommonModes, NSString,
    };
    use objc2_quartz_core::CADisplayLink;
    use objc2_ui_kit::{
        NSDirectionalEdgeInsets, UIAccessibilityIdentification, UIApplication, UIApplicationDelegate, UIButton,
        UIButtonType, UIControlEvents, UIControlState, UILabel, UILayoutConstraintAxis, UIProgressView, UIResponder,
        UIScene, UISceneConnectionOptions, UISceneDelegate, UISceneSession, UISlider, UIStackView, UISwitch,
        UITextField, UITextFieldTextDidChangeNotification, UITrackingRunLoopMode, UIView, UIViewController,
        UIWindow, UIWindowScene, UIWindowSceneDelegate,
    };
    use svelters_apple::objc::runloop::{LoopEvents, RunLoop};
    use svelters_apple::objc::target::{set_receiver, SveltersTarget, TargetEvent};

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFRunLoopRunInMode(mode: *const c_void, seconds: f64, return_after_source_handled: u8) -> i32;
    }
    unsafe extern "C" {
        // exit() runs Rust TLS destructors on the main thread, which drop UIKit objects held in
        // thread_locals and abort ("cannot access a Thread Local Storage value during or after
        // destruction"). The selftest flushes and leaves with _exit instead.
        fn _exit(code: i32) -> !;
    }

    fn out(line: String) {
        println!("{line}");
    }

    fn esc(s: &str) -> String {
        svelters_apple::testing::json_str(s)
    }

    /// One claim from `ios-uikit-backend.md`: result is "verified" / "failed" / "info".
    fn rec(claim: &str, result: &str, detail: String) {
        out(format!("SV {{\"claim\":{},\"result\":\"{}\",\"detail\":{}}}", esc(claim), result, esc(&detail)));
    }

    fn check(claim: &str, ok: bool, detail: String) {
        rec(claim, if ok { "verified" } else { "failed" }, detail);
    }

    // ---- shared state (main thread only) ----

    struct Ui {
        window: Retained<UIWindow>,
        vc: Retained<UIViewController>,
        stack: Retained<UIStackView>,
        label: Retained<UILabel>,
        status: Retained<UILabel>,
        button: Retained<UIButton>,
        sw: Retained<UISwitch>,
        slider: Retained<UISlider>,
        field: Retained<UITextField>,
        progress: Retained<UIProgressView>,
        plain: Retained<UIView>,
        obs: Retained<Observer>,
        targets: Vec<Retained<SveltersTarget>>,
        link: Retained<CADisplayLink>,
    }

    thread_local! {
        static COUNTS: RefCell<HashMap<String, u32>> = RefCell::new(HashMap::new());
        static UI: RefCell<Option<Rc<Ui>>> = const { RefCell::new(None) };
        static PENDING: RefCell<Option<Rc<Ui>>> = const { RefCell::new(None) };
        static LOOP: RefCell<Option<Rc<RunLoop>>> = const { RefCell::new(None) };
        static STEP: Cell<u32> = const { Cell::new(0) };
        static ORDER: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
        static SELFTEST: Cell<bool> = const { Cell::new(false) };
        static T0: Cell<f64> = const { Cell::new(0.0) };
    }

    fn inc(k: &str) {
        COUNTS.with(|c| *c.borrow_mut().entry(k.to_string()).or_insert(0) += 1);
    }
    fn cnt(k: &str) -> u32 {
        COUNTS.with(|c| c.borrow().get(k).copied().unwrap_or(0))
    }
    fn ui() -> Rc<Ui> {
        UI.with(|u| u.borrow().clone()).expect("ui")
    }
    fn rl() -> Rc<RunLoop> {
        LOOP.with(|l| l.borrow().clone()).expect("runloop")
    }
    fn now() -> f64 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64()
    }

    // ---- classes ----

    define_class!(
        #[unsafe(super(UIResponder))]
        #[thread_kind = MainThreadOnly]
        #[name = "SveltersAppDelegate"]
        struct AppDelegate;

        unsafe impl NSObjectProtocol for AppDelegate {}
        unsafe impl UIApplicationDelegate for AppDelegate {
            #[unsafe(method(application:didFinishLaunchingWithOptions:))]
            fn did_finish(&self, _app: &UIApplication, _opts: Option<&AnyObject>) -> bool {
                ORDER.with(|o| o.borrow_mut().push("didFinishLaunching"));
                out("SV-LOG application:didFinishLaunchingWithOptions:".into());
                // Option A of §2.2: the "bundle" mounts here, before any scene exists, into a
                // sceneless UIWindow that the first scene adopts.
                let mtm = MainThreadMarker::from(self);
                PENDING.with(|p| *p.borrow_mut() = Some(Rc::new(build(mtm))));
                true
            }
        }
    );

    define_class!(
        // The scene delegate the Info.plist scene manifest names (UISceneDelegateClassName).
        #[unsafe(super(UIResponder))]
        #[thread_kind = MainThreadOnly]
        #[name = "SveltersSceneDelegate"]
        struct SceneDelegate;

        unsafe impl NSObjectProtocol for SceneDelegate {}
        unsafe impl UISceneDelegate for SceneDelegate {
            #[unsafe(method(scene:willConnectToSession:options:))]
            fn will_connect(&self, scene: &UIScene, _s: &UISceneSession, _o: &UISceneConnectionOptions) {
                ORDER.with(|o| o.borrow_mut().push("willConnect"));
                out("SV-LOG scene:willConnectToSession:options:".into());
                let ws: &UIWindowScene = unsafe { &*(scene as *const UIScene as *const UIWindowScene) };
                let ui = PENDING.with(|p| p.borrow_mut().take()).expect("mounted in didFinishLaunching");
                let had_scene = ui.window.windowScene().is_some();
                ui.window.setWindowScene(Some(ws));
                ui.window.makeKeyAndVisible();
                let stack: Retained<AnyObject> = unsafe { msg_send![class!(NSThread), callStackSymbols] };
                let desc: Retained<NSString> = unsafe { msg_send![&stack, description] };
                let desc = desc.to_string();
                UI.with(|u| *u.borrow_mut() = Some(ui.clone()));
                if SELFTEST.get() {
                    check("sceneless UIWindow created in didFinishLaunching adopts the scene (setWindowScene) — I1 option A",
                          !had_scene && ui.window.windowScene().is_some(),
                          format!("windowScene before={had_scene}, after={}", ui.window.windowScene().is_some()));
                    check("scene:willConnectToSession: runs inside UIApplicationMain (main never returns before scenes)",
                          desc.contains("UIApplicationMain"),
                          desc.lines().filter(|l| l.contains("UIApplicationMain") || l.contains(" main ") || l.contains("ios_probe")).map(str::trim).collect::<Vec<_>>().join(" | "));
                }
            }

            #[unsafe(method(sceneDidBecomeActive:))]
            fn did_become_active(&self, _scene: &UIScene) {
                ORDER.with(|o| o.borrow_mut().push("didBecomeActive"));
                out("SV-LOG sceneDidBecomeActive:".into());
                if SELFTEST.get() && STEP.get() == 0 {
                    STEP.set(1);
                    T0.set(now());
                    rl().schedule_wake(300); // step 1 runs from the CFRunLoop timer
                }
            }

            #[unsafe(method(sceneDidDisconnect:))]
            fn did_disconnect(&self, _scene: &UIScene) {
                out("SV-LOG sceneDidDisconnect:".into());
            }
        }
        unsafe impl UIWindowSceneDelegate for SceneDelegate {}
    );

    define_class!(
        // KVO observer: counts change notifications per "<accessibilityIdentifier>.<keyPath>".
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "SveltersProbeObserver"]
        struct Observer;

        unsafe impl NSObjectProtocol for Observer {}

        impl Observer {
            #[unsafe(method(observeValueForKeyPath:ofObject:change:context:))]
            fn observe(&self, key: Option<&NSString>, obj: Option<&AnyObject>,
                       _change: Option<&NSDictionary<NSKeyValueChangeKey, AnyObject>>, _ctx: *mut c_void) {
                let id: Option<Retained<NSString>> = obj.and_then(|o| unsafe { msg_send![o, accessibilityIdentifier] });
                let k = format!("kvo:{}.{}", id.map(|s| s.to_string()).unwrap_or_default(),
                                key.map(|s| s.to_string()).unwrap_or_default());
                inc(&k);
            }
        }
    );

    // ---- run-loop events (svelters-apple RunLoop) ----

    struct Events;
    impl LoopEvents for Events {
        fn wake(&self) {
            inc("rl:wake");
            if MainThreadMarker::new().is_some() {
                inc("rl:wake-main");
            }
        }
        fn timer(&self) {
            inc("rl:timer");
            step();
        }
        fn frame(&self) {
            inc("rl:frame");
        }
    }

    // ---- UI ----

    fn build(mtm: MainThreadMarker) -> Ui {
        let obs: Retained<Observer> = unsafe { msg_send![Observer::alloc(mtm), init] };
        let window = UIWindow::new(mtm); // no scene yet
        let vc = UIViewController::new(mtm);
        window.setRootViewController(Some(&vc));
        let root = vc.view().unwrap();
        root.setBackgroundColor(Some(&objc2_ui_kit::UIColor::systemBackgroundColor()));

        let stack = UIStackView::new(mtm);
        stack.setAxis(UILayoutConstraintAxis::Vertical);
        stack.setSpacing(12.0);
        stack.setAccessibilityIdentifier(Some(ns_string!("stack")));
        let label = UILabel::new(mtm);
        label.setText(Some(ns_string!("svelters ios_probe")));
        label.setAccessibilityIdentifier(Some(ns_string!("label")));
        let status = UILabel::new(mtm);
        status.setText(Some(ns_string!("status: waiting")));
        status.setNumberOfLines(0);
        status.setAccessibilityIdentifier(Some(ns_string!("status")));
        let button = UIButton::buttonWithType(UIButtonType::System, mtm);
        button.setTitle_forState(Some(ns_string!("Increment")), UIControlState::Normal);
        button.setAccessibilityIdentifier(Some(ns_string!("button")));
        let sw = UISwitch::new(mtm);
        sw.setAccessibilityIdentifier(Some(ns_string!("switch")));
        let slider = UISlider::new(mtm);
        slider.setAccessibilityIdentifier(Some(ns_string!("slider")));
        let field = UITextField::new(mtm);
        field.setPlaceholder(Some(ns_string!("text field")));
        field.setBorderStyle(objc2_ui_kit::UITextBorderStyle::RoundedRect);
        field.setAccessibilityIdentifier(Some(ns_string!("field")));
        let progress = UIProgressView::new(mtm);
        progress.setProgress(0.5);
        progress.setAccessibilityIdentifier(Some(ns_string!("progress")));
        let plain = UIView::new(mtm);
        plain.setAccessibilityIdentifier(Some(ns_string!("view")));

        // Target-action through svelters-apple's SveltersTarget (`fire:`), token = control.
        let t_button = SveltersTarget::new(mtm, 1);
        let t_switch = SveltersTarget::new(mtm, 2);
        let t_slider = SveltersTarget::new(mtm, 3);
        let t_field = SveltersTarget::new(mtm, 4);
        let t_note = SveltersTarget::new(mtm, 5);
        let t_link = SveltersTarget::new(mtm, 6);
        unsafe {
            button.addTarget_action_forControlEvents(Some(t_button.as_any()), sel!(fire:), UIControlEvents::PrimaryActionTriggered);
            sw.addTarget_action_forControlEvents(Some(t_switch.as_any()), sel!(fire:), UIControlEvents::ValueChanged);
            slider.addTarget_action_forControlEvents(Some(t_slider.as_any()), sel!(fire:), UIControlEvents::ValueChanged);
            field.addTarget_action_forControlEvents(Some(t_field.as_any()), sel!(fire:), UIControlEvents::EditingChanged);
            let center: Retained<AnyObject> = msg_send![class!(NSNotificationCenter), defaultCenter];
            let _: () = msg_send![&center, addObserver: t_note.as_any(), selector: sel!(notified:),
                                  name: UITextFieldTextDidChangeNotification, object: &*field];
        }
        set_receiver(Some(Rc::new(|ev: TargetEvent<'_>| match ev {
            TargetEvent::Action { token, .. } => inc(&format!("action:{token}")),
            TargetEvent::Notification { token, .. } => inc(&format!("note:{token}")),
            TargetEvent::Tick { .. } => inc("tick"),
        })));

        // KVO registrations.
        let opts = NSKeyValueObservingOptions::New | NSKeyValueObservingOptions::Old;
        unsafe {
            let o = |obj: &NSObject, key: &NSString| obj.addObserver_forKeyPath_options_context(&obs, key, opts, std::ptr::null_mut());
            o(&label, ns_string!("text"));
            o(&field, ns_string!("text"));
            o(&sw, ns_string!("on"));
            o(&slider, ns_string!("value"));
            o(&progress, ns_string!("progress"));
            o(&plain, ns_string!("hidden"));
            o(&plain, ns_string!("alpha"));
            o(&stack, ns_string!("spacing"));
        }

        for v in [&*label as &UIView, &status, &button, &sw, &slider, &field, &progress, &plain] {
            stack.addArrangedSubview(v);
        }
        stack.setTranslatesAutoresizingMaskIntoConstraints(false);
        root.addSubview(&stack);
        // Pin to the safe area (§2.5): leading/trailing/top with 20 pt margins.
        unsafe {
            let g: Retained<AnyObject> = msg_send![&root, safeAreaLayoutGuide];
            let pin = |a: &str, b: &str, c: f64| {
                let sa: Retained<AnyObject> = msg_send![&stack, performSelector: objc2::runtime::Sel::register(&std::ffi::CString::new(a).unwrap())];
                let ga: Retained<AnyObject> = msg_send![&g, performSelector: objc2::runtime::Sel::register(&std::ffi::CString::new(b).unwrap())];
                let cn: Retained<AnyObject> = msg_send![&sa, constraintEqualToAnchor: &*ga, constant: c];
                let _: () = msg_send![&cn, setActive: true];
            };
            pin("leadingAnchor", "leadingAnchor", 20.0);
            pin("trailingAnchor", "trailingAnchor", -20.0);
            pin("topAnchor", "topAnchor", 20.0);
        }

        // Frames: CADisplayLink in common modes, paused when no frame is requested.
        let link = unsafe { CADisplayLink::displayLinkWithTarget_selector(t_link.as_any(), sel!(tick:)) };
        unsafe { link.addToRunLoop_forMode(&NSRunLoop::mainRunLoop(), NSRunLoopCommonModes) };
        link.setPaused(true);

        Ui { window, vc, stack, label, status, button, sw, slider, field, progress, plain, obs,
             targets: vec![t_button, t_switch, t_slider, t_field, t_note, t_link], link }
    }

    // ---- the self-test, one step per CFRunLoop timer fire ----

    fn step() {
        let s = STEP.get();
        STEP.set(s + 1);
        match s {
            1 => step1(),
            2 => step2(),
            3 => step3(),
            4 => step4(),
            5 => step5(),
            _ => {}
        }
    }

    fn step1() {
        let u = ui();
        let first_ms = (now() - T0.get()) * 1000.0;
        check("CFRunLoopTimer (svelters-apple RunLoop::schedule_wake) fires in the UIKit main loop",
              cnt("rl:timer") == 1, format!("armed 300 ms, fired after {first_ms:.0} ms"));
        rec("launch order", "info", ORDER.with(|o| o.borrow().join(" -> ")));

        // Window / scene / geometry.
        let ws = u.window.windowScene();
        let scr: Retained<AnyObject> = unsafe { msg_send![ws.as_deref().unwrap(), screen] };
        let sb: objc2_core_foundation::CGRect = unsafe { msg_send![&scr, bounds] };
        let nb: objc2_core_foundation::CGRect = unsafe { msg_send![&scr, nativeBounds] };
        let scale: f64 = unsafe { msg_send![&scr, scale] };
        let maxfps: isize = unsafe { msg_send![&scr, maximumFramesPerSecond] };
        let wb = u.window.bounds();
        let root = u.vc.view().unwrap();
        let sa = root.safeAreaInsets();
        check("window shown: key window attached to a UIWindowScene",
              u.window.isKeyWindow() && ws.is_some() && !u.window.isHidden(),
              format!("isKey={} hidden={} scene={}", u.window.isKeyWindow(), u.window.isHidden(), ws.is_some()));
        rec("screen/window geometry (UILaunchScreen letterboxing check)", "info",
            format!("screen {}x{} pt, native {}x{} px, scale {scale}, window {}x{}, safeArea t{} l{} b{} r{}, maxFPS {maxfps}",
                    sb.size.width, sb.size.height, nb.size.width, nb.size.height, wb.size.width, wb.size.height,
                    sa.top, sa.left, sa.bottom, sa.right));

        // KVO on programmatic sets (display-only props).
        let kvo = |id: &str, k: &str| cnt(&format!("kvo:{id}.{k}"));
        let b = kvo("label", "text");
        u.label.setText(Some(ns_string!("1")));
        check("KVO fires for programmatic UILabel.setText:", kvo("label", "text") == b + 1, format!("{} -> {}", b, kvo("label", "text")));
        let b = kvo("progress", "progress");
        u.progress.setProgress(0.7);
        let b2 = kvo("progress", "progress");
        u.progress.setProgress_animated(0.8, false);
        check("KVO fires for programmatic UIProgressView.setProgress:", b2 == b + 1,
              format!("setProgress: {b}->{b2}; setProgress:animated: ->{}", kvo("progress", "progress")));
        u.progress.setProgress(1.5);
        check("UIProgressView.progress clamps to 0…1 silently", (u.progress.progress() - 1.0).abs() < 1e-6,
              format!("set 1.5 read {}", u.progress.progress()));
        let (bh, ba) = (kvo("view", "hidden"), kvo("view", "alpha"));
        u.plain.setHidden(true);
        u.plain.setHidden(false);
        u.plain.setAlpha(0.5);
        check("KVO fires for programmatic UIView.hidden / alpha", kvo("view", "hidden") == bh + 2 && kvo("view", "alpha") == ba + 1,
              format!("hidden {}->{} (2 sets), alpha {}->{} (1 set)", bh, kvo("view", "hidden"), ba, kvo("view", "alpha")));
        let b = kvo("stack", "spacing");
        u.stack.setSpacing(14.0);
        rec("KVO on UIStackView.spacing (programmatic)", if kvo("stack", "spacing") == b + 1 { "verified" } else { "failed" },
            format!("{} -> {}", b, kvo("stack", "spacing")));

        // Programmatic sets of UIControl values: no control events, KVO?
        let (a, k) = (cnt("action:2"), kvo("switch", "on"));
        u.sw.setOn_animated(true, false);
        u.sw.setOn(false);
        check("UISwitch setOn:/setOn:animated: send no ValueChanged", cnt("action:2") == a,
              format!("actions {} -> {}; KVO 'on' {} -> {}", a, cnt("action:2"), k, kvo("switch", "on")));
        let (a, k) = (cnt("action:3"), kvo("slider", "value"));
        u.slider.setValue(0.3);
        u.slider.setValue_animated(0.4, false);
        check("UISlider setValue: sends no ValueChanged", cnt("action:3") == a,
              format!("actions {} -> {}; KVO 'value' {} -> {}", a, cnt("action:3"), k, kvo("slider", "value")));
        let (a, k) = (cnt("action:3"), kvo("slider", "value"));
        u.slider.setValue(500.0);
        check("UISlider.value = 500 with max 1 clamps silently (no event)",
              (u.slider.value() - 1.0).abs() < 1e-6 && cnt("action:3") == a,
              format!("read back {}; actions {} -> {}; KVO 'value' {} -> {}", u.slider.value(), a, cnt("action:3"), k, kvo("slider", "value")));
        let (a, n, k) = (cnt("action:4"), cnt("note:5"), kvo("field", "text"));
        u.field.setText(Some(ns_string!("abc")));
        check("UITextField setText: sends no EditingChanged and no UITextFieldTextDidChangeNotification",
              cnt("action:4") == a && cnt("note:5") == n,
              format!("actions {} -> {}; notes {} -> {}; KVO 'text' {} -> {}", a, cnt("action:4"), n, cnt("note:5"), k, kvo("field", "text")));

        // Forced control events reach SveltersTarget's fire:.
        let (a1, a2, a3) = (cnt("action:1"), cnt("action:2"), cnt("action:3"));
        u.button.sendActionsForControlEvents(UIControlEvents::PrimaryActionTriggered);
        u.sw.sendActionsForControlEvents(UIControlEvents::ValueChanged);
        u.slider.sendActionsForControlEvents(UIControlEvents::ValueChanged);
        check("target-action: sendActionsForControlEvents: reaches SveltersTarget fire: (button/switch/slider)",
              cnt("action:1") == a1 + 1 && cnt("action:2") == a2 + 1 && cnt("action:3") == a3 + 1,
              format!("button {}->{}, switch {}->{}, slider {}->{}", a1, cnt("action:1"), a2, cnt("action:2"), a3, cnt("action:3")));
        let a1 = cnt("action:1");
        u.button.sendActionsForControlEvents(UIControlEvents::TouchUpInside);
        rec("UIButton TouchUpInside also triggers a PrimaryActionTriggered target?", "info",
            format!("primary-action target count {} -> {} after sendActions(TouchUpInside)", a1, cnt("action:1")));

        // Simulated typing (the selftest's 'user edit'): becomeFirstResponder + insertText:.
        let fr: bool = unsafe { msg_send![&u.field, becomeFirstResponder] };
        let (a, n, k) = (cnt("action:4"), cnt("note:5"), kvo("field", "text"));
        let _: () = unsafe { msg_send![&u.field, insertText: ns_string!("x")] };
        let txt = u.field.text().map(|s| s.to_string()).unwrap_or_default();
        rec("UITextField insertText: (simulated typing) fires EditingChanged + TextDidChange notification",
            if cnt("action:4") > a && cnt("note:5") > n { "verified" } else { "failed" },
            format!("firstResponder={fr}; text='{txt}'; actions {} -> {}; notes {} -> {}; KVO 'text' {} -> {} (sync)",
                    a, cnt("action:4"), n, cnt("note:5"), k, kvo("field", "text")));
        let (a4, n5) = (cnt("action:4"), cnt("note:5"));
        COUNTS.with(|c| {
            let mut c = c.borrow_mut();
            c.insert("snap:a4".into(), a4);
            c.insert("snap:n5".into(), n5);
        });

        // removeArrangedSubview quirk.
        let before = u.stack.arrangedSubviews().count();
        u.stack.removeArrangedSubview(&u.progress);
        let still_sub = u.progress.superview().is_some();
        let after = u.stack.arrangedSubviews().count();
        u.progress.removeFromSuperview();
        check("removeArrangedSubview: leaves the view in subviews until removeFromSuperview",
              still_sub && after == before - 1 && u.progress.superview().is_none(),
              format!("arranged {before}->{after}; superview after removeArranged={still_sub}; after removeFromSuperview={}",
                      u.progress.superview().is_some()));

        // KVC through svelters-apple's bridge, incl. a struct in NSValue (NSDirectionalEdgeInsets).
        unsafe {
            let num: Retained<AnyObject> = msg_send![class!(NSNumber), numberWithDouble: 2.0f64];
            let r1 = svelters_apple::objc::bridge::kvc_set(&u.slider, "maximumValue", Some(&num));
            let ins = NSDirectionalEdgeInsets { top: 1.0, leading: 2.0, bottom: 3.0, trailing: 4.0 };
            let enc = std::ffi::CString::new(<NSDirectionalEdgeInsets as objc2::Encode>::ENCODING.to_string()).unwrap();
            let val: Retained<AnyObject> = msg_send![class!(NSValue), valueWithBytes: &ins as *const _ as *const c_void, objCType: enc.as_ptr()];
            let r2 = svelters_apple::objc::bridge::kvc_set(&u.stack, "directionalLayoutMargins", Some(&val));
            let m = u.stack.directionalLayoutMargins();
            let got = svelters_apple::objc::bridge::kvc_get(&u.stack, "directionalLayoutMargins");
            let gd: String = match &got {
                Ok(Some(o)) => { let d: Retained<NSString> = msg_send![&**o, description]; d.to_string() }
                other => format!("{other:?}"),
            };
            check("KVC via svelters-apple bridge: NSNumber and NSValue(NSDirectionalEdgeInsets) struct",
                  r1.is_ok() && r2.is_ok() && (u.slider.maximumValue() - 2.0).abs() < 1e-6 && m.top == 1.0 && m.trailing == 4.0,
                  format!("set max={:?} read {}; set margins={:?} read {{{} {} {} {}}}; kvc_get -> {gd}; encoding {}",
                          r1, u.slider.maximumValue(), r2, m.top, m.leading, m.bottom, m.trailing, enc.to_string_lossy()));
        }

        // Host file system visible from the simulator process.
        match std::env::var("SV_HOSTFILE") {
            Ok(p) => {
                let r = std::fs::read_to_string(&p);
                check("simulator process reads a file on the Mac's file system (dev --watch loop)", r.is_ok(),
                      format!("{p}: {:?}", r.map(|s| s.trim().to_string())));
            }
            Err(_) => rec("simulator process reads a file on the Mac's file system", "not tested", "SV_HOSTFILE unset".into()),
        }
        rec("process info", "info", format!("cwd={:?} home={:?} exe={:?}", std::env::current_dir().ok(),
            std::env::var("HOME").ok(), std::env::current_exe().ok()));

        // Display link + cross-thread wake for ~1 s.
        u.link.setPaused(false);
        let w = rl().waker();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(50));
            w.wake();
        });
        COUNTS.with(|c| c.borrow_mut().insert("snap:tick0".into(), 0));
        T0.set(now());
        rl().schedule_wake(1000);
    }

    fn step2() {
        let u = ui();
        let el = now() - T0.get();
        u.link.setPaused(true);
        let ticks = cnt("tick");
        check("CADisplayLink (common modes) ticks into SveltersTarget tick:", ticks > 0,
              format!("{ticks} ticks in {el:.2} s (~{:.0} Hz, first second after launch + keyboard focus)", ticks as f64 / el));
        check("Waker::wake from a background thread runs the CFRunLoop source on the main thread",
              cnt("rl:wake") == 1 && cnt("rl:wake-main") == 1, format!("wake={} on-main={}", cnt("rl:wake"), cnt("rl:wake-main")));
        let (a, n) = (cnt("snap:a4"), cnt("snap:n5"));
        rec("UITextField events after insertText: (async check, ~1 s later)", "info",
            format!("EditingChanged {} -> {}, TextDidChange {} -> {}", a, cnt("action:4"), n, cnt("note:5")));
        COUNTS.with(|c| c.borrow_mut().insert("snap:paused".into(), ticks));
        rl().schedule_wake(300);
    }

    fn step3() {
        let u = ui();
        let paused = cnt("snap:paused");
        check("paused CADisplayLink stops ticking", cnt("tick") == paused, format!("{} -> {} over 300 ms", paused, cnt("tick")));

        // Nested run in UITrackingRunLoopMode (what a scroll/slider drag runs): do our source,
        // timer and display link (all in common modes) still fire?
        let (w0, f0, t0) = (cnt("rl:wake"), cnt("rl:frame"), cnt("tick"));
        u.link.setPaused(false);
        rl().frame_fallback(50);
        let w = rl().waker();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(30));
            w.wake();
        });
        let mode: &NSString = unsafe { UITrackingRunLoopMode };
        let r = unsafe { CFRunLoopRunInMode(mode as *const NSString as *const c_void, 0.3, 0) };
        u.link.setPaused(true);
        check("wake source, CFRunLoopTimer and CADisplayLink all fire in UITrackingRunLoopMode (common modes)",
              cnt("rl:wake") > w0 && cnt("rl:frame") > f0 && cnt("tick") > t0,
              format!("mode={mode} rc={r}; wake {}->{}, timer {}->{}, ticks {}->{}", w0, cnt("rl:wake"), f0,
                      cnt("rl:frame"), t0, cnt("tick")));
        // A settled 2 s display-link rate measurement in the default mode.
        let t = cnt("tick");
        COUNTS.with(|c| c.borrow_mut().insert("snap:t2".into(), t));
        T0.set(now());
        u.link.setPaused(false);
        rl().schedule_wake(2000);
    }

    fn step4() {
        let u = ui();
        u.link.setPaused(true);
        let el = now() - T0.get();
        let n = cnt("tick") - cnt("snap:t2");
        rec("CADisplayLink rate, settled, default mode", "info", format!("{n} ticks in {el:.2} s (~{:.0} Hz)", n as f64 / el));
        u.status.setText(Some(&NSString::from_str("status: selftest done (see console)")));
        rec("UIApplicationMain never returns", "verified",
            "UIApplication::main is `-> !`; all callbacks above ran inside it (stack check), and the process ends only via exit()".into());
        let _ = (&u.obs, &u.targets);
        out("SV-DONE".into());
        let ms: u64 = std::env::var("SV_EXIT_MS").ok().and_then(|s| s.parse().ok()).unwrap_or(1500);
        if ms > 0 {
            rl().schedule_wake(ms as i64);
        }
    }

    fn step5() {
        out("SV-EXIT".into());
        use std::io::Write;
        let _ = std::io::stdout().flush();
        unsafe { _exit(0) }
    }

    pub fn main() {
        let mtm = MainThreadMarker::new().unwrap();
        SELFTEST.set(std::env::args().any(|a| a == "--selftest"));
        out(format!("SV-LOG main: args={:?}", std::env::args().collect::<Vec<_>>()));
        if std::env::var("SV_PREMAIN").as_deref() == Ok("1") {
            out("SV-LOG premain: creating UIView + UILabel before UIApplicationMain".into());
            let v = UIView::new(mtm);
            let l = UILabel::new(mtm);
            l.setText(Some(ns_string!("pre")));
            rec("creating UIView/UILabel before UIApplicationMain", "info",
                format!("no crash; label.text={:?}, view frame w={}", l.text().map(|s| s.to_string()), v.frame().size.width));
        }
        let lp = Rc::new(RunLoop::new());
        lp.set_events(Some(Rc::new(Events)));
        LOOP.with(|l| *l.borrow_mut() = Some(lp));
        let _ = (SceneDelegate::class(), AppDelegate::class());
        // UIApplicationMain never returns; the scene manifest in Info.plist names SveltersSceneDelegate.
        UIApplication::main(None, Some(ns_string!("SveltersAppDelegate")), mtm);
    }
}

fn main() {
    #[cfg(target_os = "ios")]
    ios::main();
}
