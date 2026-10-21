//! ABI v2.6 `observe` on AppKit (milestone R4; plans/research/REACTIVITY.md 3.3, 5.4-5.6, decision 13.10).
//!
//! Mechanisms, from the prop's observe fact (`OBSERVE` in the generated tables, `metadata/observe/appkit.json`):
//! - `notify`: KVO on the prop's KVC key. One observer object for the whole backend ([`SveltersObserver`],
//!   `observeValueForKeyPath:ofObject:change:context:`); `context` is a per-registration [`Ctx`] record holding
//!   the (handle, prop) ids, owned by the handle's entry and freed only after the registration is removed (or the
//!   observed object is gone).
//! - `notification`: the handle's [`SveltersTarget`] on the default center, object = the observed object
//!   (`NSControlTextDidChangeNotification`, `NSWindowDidResizeNotification`, ...).
//! - `target_action`: the control's target/action pair, set to the handle's target.
//!
//! Notifications and the target-action pair are shared with `listen` (a signal on the same notification or the
//! `action` signal), so both count their users per handle and only the first user installs, the last removes.
//!
//! Teardown per lifetime class (REACTIVITY.md 5.5):
//! - OWNED / TOPLEVEL: the entry holds a strong reference, and `destroy` (or the window's close) removes every KVO
//!   registration before the entry, and with it our reference, is released.
//! - SHARED: not minted by this backend (our strong reference would keep the object alive while observed).
//! - BORROWED: the entry holds a weak reference. For an autonotifying class that keeps NSObject's KVO state accessors
//!   (`+automaticallyNotifiesObserversForKey:` YES, `-observationInfo` not overridden), the relaxed unregistration of
//!   macOS 10.13+ cleans up the registration when the object deallocates (minimum macOS: 11), and an identity
//!   sentinel (an associated object whose `dealloc` runs during the object's) reports the death, which drops the
//!   observation and the handle (`on_destroyed(h, SVL_GONE_OWNER)`). For any other class the object is retained
//!   while it is KVO-observed, so `observe(off)` / `destroy` always remove the registration from a live object.
//!
//! Threads (REACTIVITY.md 5.6): KVO notifies on the thread that made the change. On the main thread the host is
//! called synchronously; from any other thread the (handle, prop) pair is queued (deduplicated) and the run loop
//! woken, and the next wake delivers it on the main thread (`timing: deferred`). Notifications and actions of
//! AppKit controls are main-thread.

use super::*;
use objc2::{AllocAnyThread, ClassType};
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::Mutex;

/// The (handle, prop) a KVO registration reports; its address is the registration's `context`.
pub(super) struct Ctx {
    h: u64,
    prop: u32,
}

/// One observed (handle, prop).
pub(super) struct Obs {
    count: u32,
    info: &'static ObserveInfo,
    /// The KVO registration's context (`None`: the prop has no KVO mechanism, or `addObserver` failed).
    kvo: Option<Box<Ctx>>,
    /// Whether the target-action pair is one of our users.
    action: bool,
}

/// Per-handle observe state (in the handle's entry).
#[derive(Default)]
pub(super) struct Observing {
    props: HashMap<u32, Obs>,
    /// Notification name -> users (`listen` of a notification signal and observations).
    notes: HashMap<&'static str, u32>,
    /// Users of the target-action pair (`listen('action')` and observations).
    action: u32,
    /// A BORROWED object of a manual-notifying class, retained while KVO-observed.
    keep: Option<Retained<AnyObject>>,
}

// ---- the observer and the sentinel ----

define_class!(
    // SAFETY: NSObject has no subclassing requirements; SveltersObserver has no ivars and no Drop.
    #[unsafe(super(NSObject))]
    #[name = "SveltersObserver"]
    pub(crate) struct SveltersObserver;

    unsafe impl NSObjectProtocol for SveltersObserver {}

    impl SveltersObserver {
        #[unsafe(method(observeValueForKeyPath:ofObject:change:context:))]
        fn observe_value(&self, _path: Option<&NSString>, _object: Option<&AnyObject>, _change: Option<&AnyObject>, ctx: *mut c_void) {
            if ctx.is_null() {
                return;
            }
            // SAFETY: every registration of this observer passes a live `Ctx` (freed only after its removal or the
            // observed object's death); the ids are copied out.
            let (h, prop) = unsafe {
                let c = &*(ctx as *const Ctx);
                (c.h, c.prop)
            };
            kvo_fired(h, prop);
        }
    }
);

impl SveltersObserver {
    pub(super) fn new() -> Retained<SveltersObserver> {
        let this = Self::alloc().set_ivars(());
        unsafe { msg_send![super(this), init] }
    }
}

/// Reports the death of a BORROWED observed object: its handles are queued for the main thread.
pub struct SentinelIvars {
    handles: Mutex<Vec<u64>>,
}

impl Drop for SentinelIvars {
    fn drop(&mut self) {
        let hs = std::mem::take(&mut *self.handles.lock().unwrap_or_else(|e| e.into_inner()));
        if hs.is_empty() {
            return;
        }
        DEAD.lock().unwrap_or_else(|e| e.into_inner()).extend(hs);
        wake();
    }
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements; the ivars' Drop only queues ids.
    #[unsafe(super(NSObject))]
    #[name = "SveltersIdentitySentinel"]
    #[ivars = SentinelIvars]
    pub(crate) struct Sentinel;

    unsafe impl NSObjectProtocol for Sentinel {}
);

#[link(name = "objc")]
unsafe extern "C" {
    fn objc_setAssociatedObject(object: *const AnyObject, key: *const c_void, value: *const AnyObject, policy: usize);
    fn objc_getAssociatedObject(object: *const AnyObject, key: *const c_void) -> *const AnyObject;
}

/// `OBJC_ASSOCIATION_RETAIN_NONATOMIC`.
const ASSOC_RETAIN: usize = 1;
static SENTINEL_KEY: u8 = 0;

/// Attaches (or extends) the identity sentinel of `obj` so its death reports handle `h`.
fn attach_sentinel(obj: &AnyObject, h: u64) {
    let key = &SENTINEL_KEY as *const u8 as *const c_void;
    unsafe {
        let cur = objc_getAssociatedObject(obj, key);
        if !cur.is_null() && (*cur).class() == Sentinel::class() {
            let s = &*(cur as *const Sentinel);
            let mut hs = s.ivars().handles.lock().unwrap_or_else(|e| e.into_inner());
            if !hs.contains(&h) {
                hs.push(h);
            }
            return;
        }
        let s = Sentinel::alloc().set_ivars(SentinelIvars { handles: Mutex::new(vec![h]) });
        let s: Retained<Sentinel> = msg_send![super(s), init];
        objc_setAssociatedObject(obj, key, Retained::as_ptr(&s) as *const AnyObject, ASSOC_RETAIN);
    }
}

/// Whether KVO of `key` on `obj` is automatic with NSObject's own KVO state (the condition of the 10.13 relaxed
/// unregistration): `+automaticallyNotifiesObserversForKey:` answers YES and `-observationInfo` is NSObject's.
fn autonotifying(obj: &AnyObject, key: &str) -> bool {
    let cls = obj.class();
    let k = NSString::from_str(key);
    let auto: bool = unsafe { msg_send![cls, automaticallyNotifiesObserversForKey: &*k] };
    let imp = |c: &AnyClass| c.instance_method(sel!(observationInfo)).map(|m| m.implementation() as usize);
    auto && imp(cls) == imp(NSObject::class())
}

// ---- cross-thread hops ----

static PENDING: Mutex<Vec<(u64, u32)>> = Mutex::new(Vec::new());
static DEAD: Mutex<Vec<u64>> = Mutex::new(Vec::new());
static WAKER: Mutex<Option<svelters_apple::objc::runloop::Waker>> = Mutex::new(None);

pub(super) fn set_waker(w: svelters_apple::objc::runloop::Waker) {
    *WAKER.lock().unwrap_or_else(|e| e.into_inner()) = Some(w);
}

fn wake() {
    if let Some(w) = WAKER.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        w.wake();
    }
}

fn kvo_fired(h: u64, prop: u32) {
    if MainThreadMarker::new().is_some() {
        with_current(|i| i.changed(h, prop));
        return;
    }
    let mut p = PENDING.lock().unwrap_or_else(|e| e.into_inner());
    if !p.contains(&(h, prop)) {
        p.push((h, prop));
    }
    drop(p);
    wake();
}

type ObsResult = Result<(), ObserveError>;

fn failed(m: impl Into<String>) -> ObserveError {
    ObserveError::Failed(m.into())
}

fn unsupported(m: impl Into<String>) -> ObserveError {
    ObserveError::Unsupported(m.into())
}

impl Inner {
    /// `host.prop_changed(h, prop)` while (h, prop) is observed, unless it is our own write of a proportional prop.
    pub(super) fn changed(&self, h: u64, prop: u32) {
        let proportional = match self.table.borrow().get(h).and_then(|e| e.obs.props.get(&prop)) {
            Some(o) => o.info.proportional,
            None => return,
        };
        if proportional && self.setting.get() == Some((h, prop)) {
            return;
        }
        if let Some(host) = self.host() {
            if let Some(f) = host.prop_changed {
                unsafe { f(host.self_, h, prop) };
            }
        }
    }

    /// A notification or the action of `h` arrived: every observed prop of `h` whose fact lists it changed.
    pub(super) fn observed_fired(&self, h: u64, pred: impl Fn(&ObserveInfo) -> bool) {
        let props: Vec<u32> = match self.table.borrow().get(h) {
            Some(e) => e.obs.props.iter().filter(|(_, o)| pred(o.info)).map(|(p, _)| *p).collect(),
            None => return,
        };
        for p in props {
            self.changed(h, p);
        }
    }

    /// On the main thread, from the run loop: KVO changes queued by other threads, then deaths of observed
    /// BORROWED objects (their KVO registrations were cleaned up by the runtime).
    pub(super) fn observe_hops(&self) {
        let pending = std::mem::take(&mut *PENDING.lock().unwrap_or_else(|e| e.into_inner()));
        for (h, p) in pending {
            self.changed(h, p);
        }
        let dead = std::mem::take(&mut *DEAD.lock().unwrap_or_else(|e| e.into_inner()));
        for h in dead {
            let gone = self.table.borrow().get(h).is_some_and(|e| e.life == SVL_LIFE_BORROWED && e.load().is_none());
            if !gone {
                continue;
            }
            self.forget(h);
            if let Some(host) = self.host() {
                unsafe { (host.on_destroyed)(host.self_, h, SVL_GONE_OWNER) };
            }
        }
    }

    /// Counts a user of notification `name` on `h` (`on`) or drops one; the first adds the handle's target as an
    /// observer of `name` from the object, the last removes it.
    pub(super) fn note_use(&self, h: u64, obj: Option<&AnyObject>, name: &'static str, on: bool) {
        let t = self.target_of(h);
        let edge = {
            let mut tb = self.table.borrow_mut();
            let Some(e) = tb.get_mut(h) else { return };
            let n = e.obs.notes.entry(name).or_insert(0);
            if on {
                *n += 1;
                *n == 1
            } else if *n == 0 {
                false
            } else {
                *n -= 1;
                *n == 0
            }
        };
        let (true, Some(obj)) = (edge, obj) else { return };
        let ns = NSString::from_str(name);
        unsafe {
            let center: Retained<AnyObject> = msg_send![class!(NSNotificationCenter), defaultCenter];
            if on {
                let _: () = msg_send![&*center, addObserver: t.as_any(), selector: sel!(notified:), name: &*ns, object: obj];
            } else {
                let _: () = msg_send![&*center, removeObserver: t.as_any(), name: &*ns, object: obj];
            }
        }
    }

    /// Counts a user of the target-action pair of `h`; the first sets it to the handle's target, the last clears it.
    pub(super) fn action_use(&self, h: u64, obj: &AnyObject, on: bool) {
        let t = self.target_of(h);
        let edge = {
            let mut tb = self.table.borrow_mut();
            let Some(e) = tb.get_mut(h) else { return };
            if on {
                e.obs.action += 1;
                e.obs.action == 1
            } else if e.obs.action == 0 {
                false
            } else {
                e.obs.action -= 1;
                e.obs.action == 0
            }
        };
        if !edge {
            return;
        }
        unsafe {
            if on {
                let _: () = msg_send![obj, setTarget: t.as_any()];
                let _: () = msg_send![obj, setAction: Some(sel!(fire:))];
            } else {
                let _: () = msg_send![obj, setTarget: Option::<&AnyObject>::None];
                let _: () = msg_send![obj, setAction: Option::<Sel>::None];
            }
        }
    }

    fn kvo_remove(&self, obj: &AnyObject, key: &str, ctx: &Ctx) {
        let k = NSString::from_str(key);
        let observer: &AnyObject = AsRef::<AnyObject>::as_ref(&*self.observer);
        let ptr = ctx as *const Ctx as *mut c_void;
        let _ = bridge::catch("removeObserver:forKeyPath:context:", || unsafe {
            let _: () = msg_send![obj, removeObserver: observer, forKeyPath: &*k, context: ptr];
        });
    }

    /// Removes every KVO registration of an entry from its (live) object: `destroy`, window close. The contexts and
    /// a retained BORROWED object are released with the entry (graveyard), after the removal.
    pub(super) fn remove_observations(&self, obj: &AnyObject, e: &Native) {
        for o in e.obs.props.values() {
            if let Some(ctx) = &o.kvo {
                self.kvo_remove(obj, o.info.kvo, ctx);
            }
        }
    }

    /// ABI v2.6 `observe`.
    pub(super) fn observe(&self, h: u64, prop: u32, on: bool) -> ObsResult {
        if !on {
            return self.unobserve(h, prop);
        }
        let (kind, life) = self.entry_kind(h).map_err(failed)?;
        if prop >= DYN_BASE {
            return Err(unsupported("a property found at run time has no observe fact"));
        }
        let p = lookup::prop_info(prop).ok_or_else(|| failed("unknown prop id"))?;
        if !lookup::chain(kind).contains(&(p.kind as u32)) {
            return Err(failed(format!("prop {} does not apply to this kind", p.name)));
        }
        if !p.setter.is_empty() {
            return Err(failed(format!("{} is write-only", p.name)));
        }
        let info = lookup::observe_info(kind, p.name).ok_or_else(|| unsupported(format!("{} has no observe fact", p.name)))?;
        match info.class {
            ObsClass::Constant => return Err(failed(format!("{} is constant: it never changes", p.name))),
            ObsClass::SetOnly => return Err(unsupported(format!("{} is set_only: only our setters change it", p.name))),
            ObsClass::None => return Err(unsupported(format!("{}: AppKit does not announce its changes (none)", p.name))),
            ObsClass::Exact | ObsClass::Partial => {}
        }
        if let Some(o) = self.table.borrow_mut().get_mut(h).and_then(|e| e.obs.props.get_mut(&prop)) {
            o.count += 1;
            return Ok(());
        }
        let obj = self.live(h).map_err(failed)?;
        let mut kvo = None;
        if !info.kvo.is_empty() {
            if life == SVL_LIFE_BORROWED {
                if autonotifying(&obj, info.kvo) {
                    attach_sentinel(&obj, h);
                } else if let Some(e) = self.table.borrow_mut().get_mut(h) {
                    e.obs.keep.get_or_insert_with(|| obj.clone());
                }
            }
            let ctx = Box::new(Ctx { h, prop });
            let k = NSString::from_str(info.kvo);
            let observer: &AnyObject = AsRef::<AnyObject>::as_ref(&*self.observer);
            let ptr = &*ctx as *const Ctx as *mut c_void;
            bridge::catch("addObserver:forKeyPath:options:context:", || unsafe {
                let _: () = msg_send![&*obj, addObserver: observer, forKeyPath: &*k, options: 0usize, context: ptr];
            })
            .map_err(failed)?;
            kvo = Some(ctx);
        }
        for n in info.notifications {
            self.note_use(h, Some(&obj), n, true);
        }
        let action = info.action && bridge::responds(&obj, sel!(setTarget:));
        if action {
            self.action_use(h, &obj, true);
        }
        if kvo.is_none() && info.notifications.is_empty() && !action {
            return Err(unsupported(format!("{}: no mechanism of its fact applies to {}", p.name, bridge::class_name_of(&obj))));
        }
        if let Some(e) = self.table.borrow_mut().get_mut(h) {
            e.obs.props.insert(prop, Obs { count: 1, info, kvo, action });
        }
        Ok(())
    }

    fn unobserve(&self, h: u64, prop: u32) -> ObsResult {
        let o = {
            let mut tb = self.table.borrow_mut();
            let Some(e) = tb.get_mut(h) else { return Ok(()) };
            let Some(o) = e.obs.props.get_mut(&prop) else { return Ok(()) };
            o.count -= 1;
            if o.count > 0 {
                return Ok(());
            }
            e.obs.props.remove(&prop).unwrap()
        };
        let obj = self.object(h);
        if let (Some(ctx), Some(obj)) = (&o.kvo, &obj) {
            self.kvo_remove(obj, o.info.kvo, ctx);
        }
        for n in o.info.notifications {
            self.note_use(h, obj.as_deref(), n, false);
        }
        if o.action {
            match &obj {
                Some(obj) => self.action_use(h, obj, false),
                None => {
                    if let Some(e) = self.table.borrow_mut().get_mut(h) {
                        e.obs.action = e.obs.action.saturating_sub(1);
                    }
                }
            }
        }
        // The last KVO observation of a retained BORROWED object releases it (after the removal above).
        let keep = {
            let mut tb = self.table.borrow_mut();
            tb.get_mut(h).and_then(|e| if e.obs.props.values().any(|x| x.kvo.is_some()) { None } else { e.obs.keep.take() })
        };
        self.bury(o);
        if let Some(k) = keep {
            self.bury(k);
        }
        Ok(())
    }
}

impl AppKitBackend {
    /// A write that is not a host op (no echo marker): what native code changing the prop does (tests).
    pub fn set_prop_natively(&self, h: u64, prop: u32, v: &Value) -> Result<(), String> {
        objc2::rc::autoreleasepool(|_| self.inner.set_prop_raw(h, prop, v))
    }

    /// Whether (h, prop) is observed natively right now (tests).
    pub fn is_observed(&self, h: u64, prop: u32) -> bool {
        self.inner.table.borrow().get(h).is_some_and(|e| e.obs.props.contains_key(&prop))
    }

    /// Whether a BORROWED handle's object is retained because it is KVO-observed and its class is not autonotifying
    /// with NSObject's KVO state (tests).
    pub fn retained_while_observed(&self, h: u64) -> bool {
        self.inner.table.borrow().get(h).is_some_and(|e| e.obs.keep.is_some())
    }

    /// Whether `key` of the object of `h` is autonotifying with NSObject's KVO state (tests, diagnostics).
    pub fn kvo_autonotifying(&self, h: u64, key: &str) -> Option<bool> {
        self.inner.object(h).map(|o| autonotifying(&o, key))
    }

    /// Delivers what other threads queued (KVO hops, sentinel deaths) as the run loop's wake does (tests).
    pub fn run_observe_hops(&self) {
        self.inner.observe_hops();
    }
}
