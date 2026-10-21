//! End to end on macOS: the counter-mac fixture bundle (Svelte, built by `pnpm -r build`) runs in
//! the QuickJS host on the AppKit backend; clicks go through the button's real target-action.
//! G0 (ABI v2.7 `start`, FU1): `Host::start_with`/`run_with` mount in the started hook, and the desktop
//! order (`load_bundle` without `attach_host`) has the host attached before mount.
//! An empty suite on other platforms.

fn main() {
    #[cfg(target_os = "macos")]
    svelters_apple::testing::run_cases(
        true,
        &[
            ("counter_e2e", mac::counter_e2e),
            ("appkit_types_e2e", mac::appkit_types_e2e),
            ("start_with_mounts_in_the_hook_while_the_test_pumps", mac::start_with_mounts_in_the_hook),
            ("run_with_mounts_in_the_hook_of_the_loop", mac::run_with_mounts_in_the_hook),
            ("desktop_order_mount_has_the_host_attached", mac::desktop_order_preattach),
        ],
    );
    #[cfg(not(target_os = "macos"))]
    svelters_apple::testing::run_cases(false, &[]);
}

#[cfg(target_os = "macos")]
mod mac {
    use objc2::msg_send;
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2_foundation::NSString;
    use std::cell::Cell;
    use std::path::PathBuf;
    use std::rc::Rc;
    use svelters_abi::BackendRef;
    use svelters_appkit::AppKitBackend;
    use svelters_appkit::testing::pump;
    use svelters_host::{Host, HostConfig};

    fn text_of(o: &AnyObject) -> String {
        let s: Retained<NSString> = unsafe { msg_send![o, stringValue] };
        s.to_string()
    }

    fn class_name(o: &AnyObject) -> String {
        o.class().name().to_string_lossy().into_owned()
    }

    pub fn counter_e2e() {
        let bundle = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/counter-mac/dist/app.js");
        assert!(bundle.is_file(), "{} is missing: run `pnpm install && pnpm -r build` first", bundle.display());
        let backend = AppKitBackend::new().unwrap();
        let vt = backend.vtable();
        let bref = unsafe { BackendRef::from_raw(vt.as_raw()) }.unwrap();
        let host = Host::new(bref, HostConfig::default()).unwrap();
        backend.attach_host(host.host_callbacks());
        host.load_bundle(&bundle).unwrap();
        pump(100);
        let errors = host.take_errors();
        assert!(errors.is_empty(), "JS errors: {errors:?}");

        let wins = backend.toplevels();
        assert_eq!(wins.len(), 1, "one window");
        let win = backend.object(wins[0]).unwrap();
        let visible: bool = unsafe { msg_send![&*win, isVisible] };
        assert!(visible, "the renderer showed the window (make-key-and-order-front)");
        let title: Retained<NSString> = unsafe { msg_send![&*win, title] };
        assert_eq!(title.to_string(), "Counter");

        let stack: Retained<AnyObject> = unsafe { msg_send![&*win, contentView] };
        assert_eq!(class_name(&stack), "NSStackView");
        let items: Retained<AnyObject> = unsafe { msg_send![&*stack, arrangedSubviews] };
        let n: usize = unsafe { msg_send![&*items, count] };
        assert_eq!(n, 2, "text field and button");
        let label: Retained<AnyObject> = unsafe { msg_send![&*items, objectAtIndex: 0usize] };
        let button: Retained<AnyObject> = unsafe { msg_send![&*items, objectAtIndex: 1usize] };
        assert_eq!(class_name(&label), "NSTextField");
        assert_eq!(class_name(&button), "NSButton");
        assert_eq!(text_of(&label), "Count: 0");

        for _ in 0..3 {
            let _: () = unsafe { msg_send![&*button, performClick: Option::<&AnyObject>::None] };
            pump(30);
        }
        assert_eq!(text_of(&label), "Count: 3", "three clicks through target-action");
        let errors = host.take_errors();
        assert!(errors.is_empty(), "JS errors: {errors:?}");
        let console = host.take_console();
        eprintln!("console: {:?}", console.iter().map(|l| l.text.clone()).collect::<Vec<_>>());
    }

    /// N4: fixtures/appkit-types on the real host. Non-GUI types from `svelters:appkit/*` (a string-made
    /// NSURL value, a constructed NSUndoManager, NSColor/NSFont factories into element props, an
    /// NSItemProvider completion handler as a Promise that completes on a background queue).
    pub fn appkit_types_e2e() {
        let bundle = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/appkit-types/dist/app.js");
        assert!(bundle.is_file(), "{} is missing: run `pnpm install && pnpm -r build` first", bundle.display());
        let backend = AppKitBackend::new().unwrap();
        let vt = backend.vtable();
        let bref = unsafe { BackendRef::from_raw(vt.as_raw()) }.unwrap();
        let host = Host::new(bref, HostConfig::default()).unwrap();
        backend.attach_host(host.host_callbacks());
        host.load_bundle(&bundle).unwrap();
        let fields = || -> Vec<Retained<AnyObject>> {
            let wins = backend.toplevels();
            let win = backend.object(*wins.last().expect("a window")).unwrap();
            let stack: Retained<AnyObject> = unsafe { msg_send![&*win, contentView] };
            let items: Retained<AnyObject> = unsafe { msg_send![&*stack, arrangedSubviews] };
            let n: usize = unsafe { msg_send![&*items, count] };
            (0..n).map(|i| unsafe { msg_send![&*items, objectAtIndex: i] }).collect()
        };
        let mut tries = 0;
        while tries < 100 {
            pump(30);
            let f = fields();
            if f.len() == 3 && text_of(&f[2]) != "loading" {
                break;
            }
            tries += 1;
        }
        let errors = host.take_errors();
        assert!(errors.is_empty(), "JS errors: {errors:?}");
        let f = fields();
        assert_eq!(f.len(), 3, "three text fields");
        assert_eq!(text_of(&f[0]), "example.com", "NSURL(string).host()");
        assert_eq!(text_of(&f[1]), "undo levels 5", "construct-time KVC prop read back");
        assert_eq!(text_of(&f[2]), "from a provider", "the completion block resolved the Promise with the NSData");
        let color: Retained<AnyObject> = unsafe { msg_send![&*f[0], textColor] };
        let red: f64 = unsafe { msg_send![&*color, redComponent] };
        assert!((red - 0.8).abs() < 0.01, "text-color from NSColor.colorWithSRGBRedGreenBlueAlpha: red {red}");
        let font: Retained<AnyObject> = unsafe { msg_send![&*f[0], font] };
        let size: f64 = unsafe { msg_send![&*font, pointSize] };
        assert_eq!(size, 15.0, "font from NSFont.boldSystemFontOfSize");
        eprintln!("console: {:?}", host.take_console().iter().map(|l| l.text.clone()).collect::<Vec<_>>());
    }
    fn counter_bundle() -> PathBuf {
        let bundle = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/counter-mac/dist/app.js");
        assert!(bundle.is_file(), "{} is missing: run `pnpm install && pnpm -r build` first", bundle.display());
        bundle
    }

    /// The counter's (label, button) in the last window `backend` created.
    fn counter_views(backend: &AppKitBackend) -> (Retained<AnyObject>, Retained<AnyObject>) {
        let wins = backend.toplevels();
        let win = backend.object(*wins.last().expect("a window")).unwrap();
        let stack: Retained<AnyObject> = unsafe { msg_send![&*win, contentView] };
        let items: Retained<AnyObject> = unsafe { msg_send![&*stack, arrangedSubviews] };
        let n: usize = unsafe { msg_send![&*items, count] };
        assert_eq!(n, 2, "text field and button");
        unsafe { (msg_send![&*items, objectAtIndex: 0usize], msg_send![&*items, objectAtIndex: 1usize]) }
    }

    fn click(button: &AnyObject) {
        let _: () = unsafe { msg_send![button, performClick: Option::<&AnyObject>::None] };
        pump(30);
    }

    /// `start_with` attaches without running `[NSApp run]` and mounts in the hook while the test
    /// pumps the run loop; after `stop` no wake reaches the host.
    pub fn start_with_mounts_in_the_hook() {
        let bundle = counter_bundle();
        let backend = AppKitBackend::new().unwrap();
        let vt = backend.vtable();
        let bref = unsafe { BackendRef::from_raw(vt.as_raw()) }.unwrap();
        assert!(bref.has_start());
        let host = Host::new(bref, HostConfig::default()).unwrap();
        let fired = Rc::new(Cell::new(0));
        let f = fired.clone();
        host.start_with(move |h| {
            f.set(f.get() + 1);
            h.load_bundle(&bundle).unwrap();
        })
        .unwrap();
        assert_eq!(fired.get(), 1, "the hook ran when start returned");
        assert!(host.is_started());
        let before = host.checkpoint_count();
        pump(100);
        assert!(host.checkpoint_count() > before, "start queued a checkpoint for the first loop turn");
        let (label, button) = counter_views(&backend);
        assert_eq!(text_of(&label), "Count: 0");
        click(&button);
        assert_eq!(text_of(&label), "Count: 1", "target-action reached the started host");
        let errors = host.take_errors();
        assert!(errors.is_empty(), "JS errors: {errors:?}");
        host.stop();
        assert!(!host.is_started());
        // Detached: neither a wake nor a click reaches the host.
        let before = host.checkpoint_count();
        bref.wake();
        pump(50);
        assert_eq!(host.checkpoint_count(), before, "no checkpoint after stop");
        click(&button);
        assert_eq!(text_of(&label), "Count: 1", "no event after stop");
    }

    /// `run_with` mounts in the hook, which fires from inside `[NSApp run]`; the hook quits the loop.
    pub fn run_with_mounts_in_the_hook() {
        let bundle = counter_bundle();
        let backend = AppKitBackend::new().unwrap();
        let vt = backend.vtable();
        let bref = unsafe { BackendRef::from_raw(vt.as_raw()) }.unwrap();
        let host = Host::new(bref, HostConfig::default()).unwrap();
        let fired = Rc::new(Cell::new(0));
        let f = fired.clone();
        host.run_with(move |h| {
            f.set(f.get() + 1);
            assert!(h.is_started());
            h.load_bundle(&bundle).unwrap();
            bref.quit();
        })
        .unwrap();
        assert_eq!(fired.get(), 1);
        assert!(!host.is_started());
        let (label, _) = counter_views(&backend);
        assert_eq!(text_of(&label), "Count: 0");
        let errors = host.take_errors();
        assert!(errors.is_empty(), "JS errors: {errors:?}");
    }

    /// The desktop order without `attach_host`: `load_bundle` attaches through `start` before mount,
    /// so wakes and target-action reach the host before any `run` (the N3/G0 attach-before-mount rule).
    pub fn desktop_order_preattach() {
        let bundle = counter_bundle();
        let backend = AppKitBackend::new().unwrap();
        let vt = backend.vtable();
        let bref = unsafe { BackendRef::from_raw(vt.as_raw()) }.unwrap();
        let host = Host::new(bref, HostConfig::default()).unwrap();
        host.load_bundle(&bundle).unwrap();
        assert!(!host.is_started(), "the pre-attach does not count as started");
        let before = host.checkpoint_count();
        bref.wake();
        pump(50);
        assert!(host.checkpoint_count() > before, "a wake before run reaches the host");
        let (label, button) = counter_views(&backend);
        click(&button);
        assert_eq!(text_of(&label), "Count: 1", "target-action reached the pre-attached host");
        let errors = host.take_errors();
        assert!(errors.is_empty(), "JS errors: {errors:?}");
        drop(host);
        // Drop detached the host: a later wake calls nobody (would crash on a freed vtable).
        bref.wake();
        pump(30);
    }
}
