//! See Cargo.toml. Not run anywhere: only `cargo check --target aarch64-apple-ios` from Linux.
#[cfg(target_os = "ios")]
mod ios {
    use std::ffi::c_void;
    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, NSObject, ProtocolObject};
    use objc2::{define_class, msg_send, sel, ClassType, MainThreadMarker, MainThreadOnly};
    use objc2_foundation::{
        ns_string, NSDictionary, NSKeyValueChangeKey, NSKeyValueObservingOptions,
        NSObjectNSKeyValueObserverRegistration, NSObjectProtocol, NSSet, NSString, NSRunLoop, NSRunLoopCommonModes,
    };
    use objc2_quartz_core::CADisplayLink;
    use objc2_ui_kit::{
        UIApplication, UIButton, UIButtonType, UIControlEvents, UILabel, UILayoutConstraintAxis,
        UIProgressView, UIScene, UISceneConnectionOptions, UISceneDelegate, UISceneSession, UISlider, UIStackView,
        UISwitch, UITextField, UIView, UIViewController, UIWindow, UIWindowScene, UIWindowSceneDelegate, UIResponder,
        UIAccessibilityIdentification,
    };

    pub struct Ivars;

    define_class!(
        // The scene delegate the Info.plist scene manifest names (UISceneDelegateClassName).
        #[unsafe(super(UIResponder))]
        #[thread_kind = MainThreadOnly]
        #[name = "SveltersSceneDelegate"]
        #[ivars = Ivars]
        struct SceneDelegate;

        unsafe impl NSObjectProtocol for SceneDelegate {}
        unsafe impl UISceneDelegate for SceneDelegate {
            #[unsafe(method(scene:willConnectToSession:options:))]
            fn will_connect(&self, scene: &UIScene, _s: &UISceneSession, _o: &UISceneConnectionOptions) {
                let mtm = MainThreadMarker::from(self);
                let ws: &UIWindowScene = unsafe { &*(scene as *const UIScene as *const UIWindowScene) };
                let win = UIWindow::initWithWindowScene(UIWindow::alloc(mtm), ws);
                let vc = UIViewController::new(mtm);
                win.setRootViewController(Some(&vc));
                build(mtm, &vc.view().unwrap());
                win.makeKeyAndVisible();
                std::mem::forget(win); // the real backend keeps it in the handle table
            }
        }
        unsafe impl UIWindowSceneDelegate for SceneDelegate {}
    );

    pub struct ObsIvars;
    define_class!(
        // KVO observer + target-action receiver + display-link target in one class.
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "SveltersObserver"]
        #[ivars = ObsIvars]
        struct Observer;

        unsafe impl NSObjectProtocol for Observer {}

        impl Observer {
            #[unsafe(method(observeValueForKeyPath:ofObject:change:context:))]
            fn observe(&self, key: Option<&NSString>, _obj: Option<&AnyObject>,
                       _change: Option<&NSDictionary<NSKeyValueChangeKey, AnyObject>>, _ctx: *mut c_void) {
                let _ = key; // -> host.post_event(handle, notify::<prop>)
            }
            #[unsafe(method(fire:))]
            fn fire(&self, _sender: Option<&AnyObject>) {}
            #[unsafe(method(tick:))]
            fn tick(&self, _link: &CADisplayLink) {}
        }
    );

    fn build(mtm: MainThreadMarker, root: &UIView) {
        let this = Observer::alloc(mtm).set_ivars(ObsIvars);
        let obs: Retained<Observer> = unsafe { msg_send![super(this), init] };
        let stack = UIStackView::new(mtm);
        stack.setAxis(UILayoutConstraintAxis::Vertical);
        stack.setSpacing(8.0);
        let label = UILabel::new(mtm);
        label.setText(Some(ns_string!("0")));
        let button = UIButton::buttonWithType(UIButtonType::System, mtm);
        button.setTitle_forState(Some(ns_string!("Increment")), objc2_ui_kit::UIControlState::Normal);
        unsafe { button.addTarget_action_forControlEvents(Some(&obs), sel!(fire:), UIControlEvents::PrimaryActionTriggered) };
        let sw = UISwitch::new(mtm);
        unsafe { sw.addTarget_action_forControlEvents(Some(&obs), sel!(fire:), UIControlEvents::ValueChanged) };
        let slider = UISlider::new(mtm);
        unsafe { slider.addTarget_action_forControlEvents(Some(&obs), sel!(fire:), UIControlEvents::ValueChanged) };
        let field = UITextField::new(mtm);
        unsafe { field.addTarget_action_forControlEvents(Some(&obs), sel!(fire:), UIControlEvents::EditingChanged) };
        field.setAccessibilityIdentifier(Some(ns_string!("sv-7")));
        let progress = UIProgressView::new(mtm);
        progress.setProgress(0.5);
        // KVO on a programmatic-set property (label.text): fires for setText:, never for user input.
        unsafe {
            label.addObserver_forKeyPath_options_context(&obs, ns_string!("text"),
                NSKeyValueObservingOptions::New | NSKeyValueObservingOptions::Old, std::ptr::null_mut());
        }
        for v in [&*label as &UIView, &button, &sw, &slider, &field, &progress] {
            stack.addArrangedSubview(v);
        }
        // removeArrangedSubview keeps it a subview: the backend must also removeFromSuperview.
        stack.removeArrangedSubview(&progress);
        progress.removeFromSuperview();
        root.addSubview(&stack);
        // Frames: CADisplayLink in common modes, paused when no frame is requested.
        let link = unsafe { CADisplayLink::displayLinkWithTarget_selector(&obs, sel!(tick:)) };
        unsafe { link.addToRunLoop_forMode(&NSRunLoop::mainRunLoop(), NSRunLoopCommonModes) };
        link.setPaused(true);
        let _ = (UIApplication::sharedApplication(mtm), NSSet::<NSString>::new(), ProtocolObject::<dyn NSObjectProtocol>::from_ref(&*obs));
    }

    pub fn main() {
        // UIApplicationMain never returns; the scene manifest in Info.plist names SveltersSceneDelegate.
        let mtm = MainThreadMarker::new().unwrap();
        let _ = SceneDelegate::class();
        UIApplication::main(None, None, mtm);
    }
}

fn main() {
    #[cfg(target_os = "ios")]
    ios::main();
}
