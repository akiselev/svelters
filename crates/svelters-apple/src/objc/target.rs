//! `SveltersTarget`: the one runtime class that receives target-action messages, notifications and
//! display-link ticks for every handle (research note §4.1). Each instance carries a `token` (the
//! handle it reports for) and forwards to the receiver installed with [`set_receiver`].
//!
//! Controls and notification centers hold their target weakly, so the backend keeps each target
//! alive in its handle entry and clears `target`/observers before releasing it.

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use std::cell::RefCell;
use std::rc::Rc;

/// What a target reports.
#[derive(Debug)]
pub enum TargetEvent<'a> {
    /// Target-action from a control (`-fire:`); `sender` is the control.
    Action { token: u64, sender: Option<&'a AnyObject> },
    /// A notification observed with `-notified:`; `name` is the notification name.
    Notification { token: u64, name: String, object: Option<&'a AnyObject> },
    /// A display-link tick (`-tick:`).
    Tick { token: u64 },
}

thread_local! {
    static RECEIVER: RefCell<Option<Rc<dyn Fn(TargetEvent<'_>)>>> = const { RefCell::new(None) };
}

/// Installs the function every target forwards to (main thread).
pub fn set_receiver(f: Option<Rc<dyn Fn(TargetEvent<'_>)>>) {
    RECEIVER.with(|r| *r.borrow_mut() = f);
}

fn deliver(ev: TargetEvent<'_>) {
    let f = RECEIVER.with(|r| r.borrow().clone());
    if let Some(f) = f {
        f(ev);
    }
}

pub struct TargetIvars {
    pub token: u64,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and SveltersTarget does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "SveltersTarget"]
    #[ivars = TargetIvars]
    pub struct SveltersTarget;

    impl SveltersTarget {
        #[unsafe(method(fire:))]
        fn fire(&self, sender: Option<&AnyObject>) {
            deliver(TargetEvent::Action { token: self.ivars().token, sender });
        }

        #[unsafe(method(notified:))]
        fn notified(&self, note: Option<&AnyObject>) {
            let Some(note) = note else { return };
            let name: Option<Retained<objc2_foundation::NSString>> = unsafe { msg_send![note, name] };
            let object: Option<Retained<AnyObject>> = unsafe { msg_send![note, object] };
            deliver(TargetEvent::Notification {
                token: self.ivars().token,
                name: name.map(|n| n.to_string()).unwrap_or_default(),
                object: object.as_deref(),
            });
        }

        #[unsafe(method(tick:))]
        fn tick(&self, _link: Option<&AnyObject>) {
            deliver(TargetEvent::Tick { token: self.ivars().token });
        }
    }
);

impl SveltersTarget {
    pub fn new(mtm: MainThreadMarker, token: u64) -> Retained<SveltersTarget> {
        let this = Self::alloc(mtm).set_ivars(TargetIvars { token });
        unsafe { msg_send![super(this), init] }
    }

    pub fn token(&self) -> u64 {
        self.ivars().token
    }

    pub fn as_any(&self) -> &AnyObject {
        self.as_ref()
    }
}
