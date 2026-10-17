//! End to end on macOS: the counter-mac fixture bundle (Svelte, built by `pnpm -r build`) runs in
//! the QuickJS host on the AppKit backend; clicks go through the button's real target-action.
//! An empty suite on other platforms.

fn main() {
    #[cfg(target_os = "macos")]
    svelters_apple::testing::run_cases(true, &[("counter_e2e", mac::counter_e2e)]);
    #[cfg(not(target_os = "macos"))]
    svelters_apple::testing::run_cases(false, &[]);
}

#[cfg(target_os = "macos")]
mod mac {
    use objc2::msg_send;
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2_foundation::NSString;
    use std::path::PathBuf;
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
}
