//! The AppKit backend (macOS only): an implementation of `svelters_abi::Backend`.
//!
//! Design (docs/notes/apple-gui-backend.md, docs/notes/mac1-appkit.md):
//! - Kinds, props, signals, methods and slots come from the generated tables; props and methods
//!   that are not in the tables are found at run time in the ObjC runtime (dev mode is dynamic).
//! - Props go through KVC, methods through `NSInvocation` (svelters-apple's bridge).
//! - The handle table holds one strong reference per owned/toplevel handle and a weak one per
//!   borrowed handle; per-handle helper objects (action target, window delegate) live in the entry
//!   because AppKit holds them weakly.
//! - The loop is `[NSApp run]` with the wake source and timers of `svelters_apple::objc::runloop`
//!   in common modes; frames come from `CADisplayLink` (macOS 14+) with a timer watchdog behind it
//!   (a link does not tick off screen or on a headless display), or a 16 ms timer.
//! - Natives are released from the run loop's top level (a graveyard drained after each
//!   checkpoint), never inside a native callout.

use crate::lookup::{self, DYN_BASE};
use crate::tables::*;
use objc2::encode::{Encode, Encoding};
use objc2::rc::{Retained, Weak};
use objc2::runtime::{AnyClass, AnyObject, MessageReceiver, NSObject, Sel};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, class, define_class, msg_send, sel};
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy, NSWindow, NSWindowDelegate};
use objc2_foundation::{NSNotification, NSObjectProtocol, NSString};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use svelters_abi::*;
use svelters_apple::enc::{self, StructTy};
use svelters_apple::handles::HandleTable;
use svelters_apple::objc::bridge::{self, Objects};
use svelters_apple::objc::runloop::{LoopEvents, RunLoop};
use svelters_apple::objc::target::{self, SveltersTarget, TargetEvent};

#[path = "observe.rs"]
mod observe;
#[path = "types.rs"]
mod types;
#[path = "style.rs"]
mod style;

use crate::types_table::TYPE_KIND_BASE;

/// `NSRect` with its ObjC encoding (avoids the objc2-core-foundation dependency for one struct).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

unsafe impl Encode for Rect {
    const ENCODING: Encoding = Encoding::Struct(
        "CGRect",
        &[Encoding::Struct("CGPoint", &[Encoding::Double, Encoding::Double]), Encoding::Struct("CGSize", &[Encoding::Double, Encoding::Double])],
    );
}

const NS_WINDOW_BELOW: isize = -1;
const NS_BACKING_STORE_BUFFERED: usize = 2;
const NS_EVENT_TYPE_APPLICATION_DEFINED: usize = 15;

enum Obj {
    Strong(Retained<AnyObject>),
    Weak(Weak<AnyObject>),
}

struct Native {
    obj: Obj,
    /// Receives target-action and notifications for this handle (controls hold targets weakly).
    target: Option<Retained<SveltersTarget>>,
    /// The backend's delegate of a window or a tab view (AppKit holds delegates weakly).
    delegate: Option<Retained<NSObject>>,
    /// Table signal ids JS listens to.
    listening: Vec<u32>,
    /// ABI v2.6 observations and the notification / target-action users they share with `listen`.
    obs: observe::Observing,
}

impl Native {
    fn strong(obj: Retained<AnyObject>) -> Native {
        Native { obj: Obj::Strong(obj), target: None, delegate: None, listening: Vec::new(), obs: Default::default() }
    }
    fn load(&self) -> Option<Retained<AnyObject>> {
        match &self.obj {
            Obj::Strong(o) => Some(o.clone()),
            Obj::Weak(w) => w.load(),
        }
    }
}

/// A property resolved at run time from the ObjC runtime (not in the tables).
struct DynProp {
    key: String,
    ty: enc::Ty,
    writable: bool,
}

/// What `set_prop`/`get_prop` need to know about a prop, from either source.
struct PropSpec {
    key: String,
    ty: enc::Ty,
    members: &'static [(&'static str, i64)],
    flags: bool,
    writable: bool,
    setter: &'static str,
    /// Written through methods, not KVC (`item-titles`, `index-of-selected-item`, `labels`).
    write: Option<&'static PropWrite>,
    /// Table type `Str`: numbers are stringified, null is "".
    string: bool,
}

pub(crate) struct Inner {
    mtm: MainThreadMarker,
    table: RefCell<HandleTable<Native>>,
    by_ptr: RefCell<HashMap<usize, u64>>,
    host: Cell<*const svl_host>,
    rl: RunLoop,
    dyn_props: RefCell<Vec<(u32, String, DynProp)>>,
    dyn_methods: RefCell<Vec<(u32, String, Sel)>>,
    graveyard: RefCell<Vec<Box<dyn std::any::Any>>>,
    frame_pending: Cell<bool>,
    /// Frames delivered by (display-link tick, timer), for tests.
    frame_sources: Cell<(u32, u32)>,
    /// (window handle, CADisplayLink)
    display_link: RefCell<Option<(u64, Retained<AnyObject>)>>,
    frame_target: Retained<SveltersTarget>,
    /// The handle whose getter/call is running: owner of borrowed handles minted meanwhile.
    cur_owner: Cell<u64>,
    /// A non-GUI type op is converting values (N4): NSAttributedString results stay handles.
    in_types: Cell<bool>,
    running: Cell<bool>,
    quit_requested: Cell<bool>,
    /// The one KVO observer of the backend (`observeValueForKeyPath:ofObject:change:context:`).
    observer: Retained<observe::SveltersObserver>,
    /// The (handle, prop) `set_prop` is writing (backend-side echo suppression of proportional props).
    setting: Cell<Option<(u64, u32)>>,
    /// Non-GUI type kinds (ObjC classes) registered by name (N4, `types.rs`).
    type_kinds: RefCell<types::TypeKinds>,
    /// Async operations in flight, by completion callback id.
    pending: RefCell<HashMap<u64, types::Pending>>,
    /// ST2: per-handle styles of the decl dialect (`style.rs`).
    styles: RefCell<HashMap<u64, style::ElemStyle>>,
    /// Observes NSWindowDidUpdateNotification while some element has a :focus/:disabled/:checked block.
    style_window_observer: RefCell<Option<Retained<style::StyleTracker>>>,
}

thread_local! {
    static CURRENT: RefCell<std::rc::Weak<Inner>> = RefCell::new(std::rc::Weak::new());
}

fn with_current<R>(f: impl FnOnce(&Inner) -> R) -> Option<R> {
    let inner = CURRENT.with(|c| c.borrow().upgrade());
    inner.map(|i| f(&i))
}

pub struct DelegateIvars {
    token: u64,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements; WindowDelegate does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "SveltersWindowDelegate"]
    #[ivars = DelegateIvars]
    pub(crate) struct WindowDelegate;

    unsafe impl NSObjectProtocol for WindowDelegate {}

    unsafe impl NSWindowDelegate for WindowDelegate {
        #[unsafe(method(windowShouldClose:))]
        fn window_should_close(&self, _sender: &NSWindow) -> bool {
            let token = self.ivars().token;
            with_current(|i| i.should_close(token)).unwrap_or(true)
        }

        #[unsafe(method(windowWillClose:))]
        fn window_will_close(&self, _note: &NSNotification) {
            let token = self.ivars().token;
            with_current(|i| i.will_close(token));
        }
    }
);

impl WindowDelegate {
    fn new(mtm: MainThreadMarker, token: u64) -> Retained<WindowDelegate> {
        let this = Self::alloc(mtm).set_ivars(DelegateIvars { token });
        unsafe { msg_send![super(this), init] }
    }
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements; TabViewDelegate does not implement Drop.
    // The NSTabViewDelegate methods are all optional and AppKit asks respondsToSelector:, so the
    // class does not declare the protocol; every method below is harmless when JS does not listen
    // (should-select answers YES).
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "SveltersTabViewDelegate"]
    #[ivars = DelegateIvars]
    pub(crate) struct TabViewDelegate;

    impl TabViewDelegate {
        #[unsafe(method(tabView:shouldSelectTabViewItem:))]
        fn should_select(&self, _tab_view: &AnyObject, _item: Option<&AnyObject>) -> bool {
            let token = self.ivars().token;
            with_current(|i| i.delegate_veto(token, "tabView:shouldSelectTabViewItem:", true)).unwrap_or(true)
        }

        #[unsafe(method(tabView:willSelectTabViewItem:))]
        fn will_select(&self, _tab_view: &AnyObject, _item: Option<&AnyObject>) {
            let token = self.ivars().token;
            with_current(|i| i.delegate_post(token, "tabView:willSelectTabViewItem:"));
        }

        #[unsafe(method(tabView:didSelectTabViewItem:))]
        fn did_select(&self, _tab_view: &AnyObject, _item: Option<&AnyObject>) {
            let token = self.ivars().token;
            with_current(|i| i.delegate_post(token, "tabView:didSelectTabViewItem:"));
        }

        #[unsafe(method(tabViewDidChangeNumberOfTabViewItems:))]
        fn did_change_number(&self, _tab_view: &AnyObject) {
            let token = self.ivars().token;
            with_current(|i| i.delegate_post(token, "tabViewDidChangeNumberOfTabViewItems:"));
        }
    }
);

impl TabViewDelegate {
    fn new(mtm: MainThreadMarker, token: u64) -> Retained<TabViewDelegate> {
        let this = Self::alloc(mtm).set_ivars(DelegateIvars { token });
        unsafe { msg_send![super(this), init] }
    }
}

struct Events(std::rc::Weak<Inner>);

impl LoopEvents for Events {
    fn wake(&self) {
        if let Some(i) = self.0.upgrade() {
            i.drain();
            // KVO from other threads and deaths of observed borrowed objects, hopped here (REACTIVITY.md 5.6).
            i.observe_hops();
            // Completion blocks of async type members, queued from any thread (N4).
            i.type_hops();
            i.checkpoint();
            i.drain();
        }
    }
    fn timer(&self) {
        self.wake();
    }
    fn frame(&self) {
        if let Some(i) = self.0.upgrade() {
            i.deliver_frame("timer");
            i.drain();
        }
    }
}

#[link(name = "Foundation", kind = "framework")]
unsafe extern "C" {
    static NSRunLoopCommonModes: &'static NSString;
}

/// How long a requested frame waits for a display-link tick before the timer delivers it
/// (about four frames at 60 Hz, two at the slowest common refresh rates).
const FRAME_WATCHDOG_MS: u64 = 64;

/// `SVELTERS_APPKIT_TRACE_FRAMES=1` prints the frame path to stderr (CI diagnostics).
fn trace_frames(msg: impl FnOnce() -> String) {
    thread_local! {
        static ON: bool = std::env::var_os("SVELTERS_APPKIT_TRACE_FRAMES").is_some_and(|v| v != "0");
    }
    if ON.with(|o| *o) {
        eprintln!("[appkit frames] {}", msg());
    }
}

/// The AppKit backend. Cheap to clone (shared state); create one per process, on the main thread.
#[derive(Clone)]
pub struct AppKitBackend {
    pub(crate) inner: Rc<Inner>,
}

impl AppKitBackend {
    /// Initialises `NSApplication` (regular activation policy) and the run-loop sources.
    /// Must be called on the process's main thread.
    pub fn new() -> Result<AppKitBackend, String> {
        let mtm = MainThreadMarker::new().ok_or("the AppKit backend must be created on the main thread")?;
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
        install_menu(mtm, &app);
        let inner = Rc::new(Inner {
            mtm,
            table: RefCell::new(HandleTable::new()),
            by_ptr: RefCell::new(HashMap::new()),
            host: Cell::new(std::ptr::null()),
            rl: RunLoop::new(),
            dyn_props: RefCell::new(Vec::new()),
            dyn_methods: RefCell::new(Vec::new()),
            graveyard: RefCell::new(Vec::new()),
            frame_pending: Cell::new(false),
            frame_sources: Cell::new((0, 0)),
            display_link: RefCell::new(None),
            frame_target: SveltersTarget::new(mtm, 0),
            cur_owner: Cell::new(0),
            in_types: Cell::new(false),
            running: Cell::new(false),
            quit_requested: Cell::new(false),
            observer: observe::SveltersObserver::new(),
            setting: Cell::new(None),
            type_kinds: RefCell::new(Default::default()),
            pending: RefCell::new(HashMap::new()),
            styles: RefCell::new(HashMap::new()),
            style_window_observer: RefCell::new(None),
        });
        observe::set_waker(inner.rl.waker());
        let weak = Rc::downgrade(&inner);
        CURRENT.with(|c| *c.borrow_mut() = weak.clone());
        inner.rl.set_events(Some(Rc::new(Events(weak.clone()))));
        let w2 = weak.clone();
        target::set_receiver(Some(Rc::new(move |ev: TargetEvent<'_>| {
            if let Some(i) = w2.upgrade() {
                i.on_target(ev);
            }
        })));
        Ok(AppKitBackend { inner })
    }

    /// Builds the C vtable for this backend (leaked: backends live for the process).
    pub fn vtable(&self) -> &'static BackendVtable {
        Box::leak(Box::new(BackendVtable::new(self.clone())))
    }

    /// Lets a test drive events without `run`: post_event/call_sync/on_destroyed go to `host`.
    pub fn attach_host(&self, host: *const svl_host) {
        self.inner.host.set(host);
    }

    /// The native object of a live handle (tests and tools; never handed to JS).
    pub fn object(&self, h: u64) -> Option<Retained<AnyObject>> {
        self.inner.object(h)
    }

    /// Releases natives whose handles were destroyed (normally done by the run loop).
    pub fn drain(&self) {
        self.inner.drain();
    }

    /// Async type operations in flight (N4 tests).
    pub fn pending_async(&self) -> usize {
        self.inner.pending_async()
    }

    /// Delivers completions other threads queued, as the run loop's wake does (tests).
    pub fn run_type_hops(&self) {
        self.inner.type_hops();
    }

    /// Live handles (leak checks in tests).
    pub fn live_handles(&self) -> usize {
        self.inner.table.borrow().len()
    }

    /// Frames delivered so far by (display-link tick, timer) (tests: the link must really tick).
    pub fn frame_sources(&self) -> (u32, u32) {
        self.inner.frame_sources.get()
    }

    /// Every live handle (test cleanup).
    pub fn handles(&self) -> Vec<u64> {
        self.inner.table.borrow().iter().map(|(h, _)| h).collect()
    }

    /// The handles of live windows created by `create`.
    pub fn toplevels(&self) -> Vec<u64> {
        self.inner.table.borrow().iter().filter(|(_, e)| e.life == SVL_LIFE_TOPLEVEL).map(|(h, _)| h).collect()
    }
}

/// The default main menu: the application menu (Hide, Hide Others, Show All, Quit; Quit stops
/// `[NSApp run]` so the host's `run` returns), Edit and Window. Key equivalents only reach a text
/// field through menu items, so without Edit, Cmd+C/V/X/A/Z did nothing in any text field. The
/// items have no target: the action goes to the first responder (the focused field's editor).
fn install_menu(mtm: MainThreadMarker, app: &NSApplication) {
    let _ = mtm;
    const CMD: usize = 1 << 20;
    const OPT: usize = 1 << 19;
    const SHIFT: usize = 1 << 17;
    unsafe fn item(title: &str, action: Option<Sel>, key: &str, mods: usize) -> Retained<AnyObject> {
        let title = NSString::from_str(title);
        let key = NSString::from_str(key);
        let alloc: objc2::rc::Allocated<AnyObject> = unsafe { msg_send![class!(NSMenuItem), alloc] };
        let it: Retained<AnyObject> = unsafe { msg_send![alloc, initWithTitle: &*title, action: action, keyEquivalent: &*key] };
        if !key.is_empty() {
            let _: () = unsafe { msg_send![&*it, setKeyEquivalentModifierMask: mods] };
        }
        it
    }
    unsafe fn submenu(bar: &AnyObject, title: &str, items: Vec<Option<Retained<AnyObject>>>) -> Retained<AnyObject> {
        let top = unsafe { item(title, None, "", 0) };
        let t = NSString::from_str(title);
        let alloc: objc2::rc::Allocated<AnyObject> = unsafe { msg_send![class!(NSMenu), alloc] };
        let menu: Retained<AnyObject> = unsafe { msg_send![alloc, initWithTitle: &*t] };
        for it in items {
            let it = it.unwrap_or_else(|| unsafe { msg_send![class!(NSMenuItem), separatorItem] });
            let _: () = unsafe { msg_send![&*menu, addItem: &*it] };
        }
        let _: () = unsafe { msg_send![&*top, setSubmenu: &*menu] };
        let _: () = unsafe { msg_send![bar, addItem: &*top] };
        menu
    }
    unsafe {
        let bar: Retained<AnyObject> = msg_send![class!(NSMenu), new];
        submenu(&bar, "", vec![
            Some(item("Hide", Some(sel!(hide:)), "h", CMD)),
            Some(item("Hide Others", Some(sel!(hideOtherApplications:)), "h", CMD | OPT)),
            Some(item("Show All", Some(sel!(unhideAllApplications:)), "", 0)),
            None,
            Some(item("Quit", Some(sel!(stop:)), "q", CMD)),
        ]);
        submenu(&bar, "Edit", vec![
            Some(item("Undo", Some(sel!(undo:)), "z", CMD)),
            Some(item("Redo", Some(sel!(redo:)), "z", CMD | SHIFT)),
            None,
            Some(item("Cut", Some(sel!(cut:)), "x", CMD)),
            Some(item("Copy", Some(sel!(copy:)), "c", CMD)),
            Some(item("Paste", Some(sel!(paste:)), "v", CMD)),
            Some(item("Delete", Some(sel!(delete:)), "", 0)),
            Some(item("Select All", Some(sel!(selectAll:)), "a", CMD)),
        ]);
        let window = submenu(&bar, "Window", vec![
            Some(item("Minimize", Some(sel!(performMiniaturize:)), "m", CMD)),
            Some(item("Zoom", Some(sel!(performZoom:)), "", 0)),
            Some(item("Close", Some(sel!(performClose:)), "w", CMD)),
        ]);
        let _: () = msg_send![app, setMainMenu: &*bar];
        let _: () = msg_send![app, setWindowsMenu: &*window];
    }
}

define_class!(
    // SAFETY: NSView has no subclassing requirements beyond the main thread; DocumentView does not implement Drop.
    #[unsafe(super(objc2_app_kit::NSView, objc2_app_kit::NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "SveltersDocumentView"]
    struct DocumentView;

    impl DocumentView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }
    }
);

/// A document view that sizes itself with Auto Layout: a stack or grid view (a plain view keeps the
/// frame it is given; a text view manages its own frame in a scroll view).
fn sizes_itself(c: &AnyObject) -> bool {
    ["NSStackView", "NSGridView"].iter().any(|n| bridge::class_named(n).is_some_and(|k| bridge::is_kind(c, k)))
}

/// `document-view` of a scroll view for a view laid out by Auto Layout (a stack view): wrapped in a
/// flipped view (content starts at the top) pinned to the clip view's top, leading and trailing
/// edges, the child pinned to the wrapper's edges. The scroll view then scrolls the child's height.
unsafe fn pin_document(mtm: MainThreadMarker, scroll: &AnyObject, c: &AnyObject) {
    unsafe {
        let wrap: Retained<DocumentView> = msg_send![DocumentView::alloc(mtm), init];
        let w: &AnyObject = &wrap;
        let _: () = msg_send![w, setTranslatesAutoresizingMaskIntoConstraints: false];
        let _: () = msg_send![c, setTranslatesAutoresizingMaskIntoConstraints: false];
        let _: () = msg_send![w, addSubview: c];
        let _: () = msg_send![scroll, setDocumentView: w];
        let clip: Retained<AnyObject> = msg_send![scroll, contentView];
        let pin = |a: &AnyObject, b: &AnyObject, anchor: Sel| {
            let x: *mut AnyObject = a.send_message(anchor, ());
            let y: *mut AnyObject = b.send_message(anchor, ());
            let k: Retained<AnyObject> = msg_send![x, constraintEqualToAnchor: y];
            let _: () = msg_send![&*k, setActive: true];
        };
        for a in [sel!(leadingAnchor), sel!(trailingAnchor), sel!(topAnchor), sel!(bottomAnchor)] {
            pin(c, w, a);
        }
        for a in [sel!(leadingAnchor), sel!(trailingAnchor), sel!(topAnchor)] {
            pin(w, &clip, a);
        }
    }
}

impl Inner {
    fn host(&self) -> Option<&svl_host> {
        let h = self.host.get();
        if h.is_null() { None } else { Some(unsafe { &*h }) }
    }

    fn checkpoint(&self) {
        if let Some(h) = self.host() {
            unsafe { (h.checkpoint)(h.self_) };
        }
    }

    fn post(&self, h: u64, signal: u32, args: &[Value]) {
        if let Some(host) = self.host() {
            let raw = RawValues::new(args);
            unsafe { (host.post_event)(host.self_, h, signal, raw.as_ptr(), raw.len()) };
        }
    }

    fn drain(&self) {
        let dead = std::mem::take(&mut *self.graveyard.borrow_mut());
        drop(dead);
    }

    fn bury(&self, x: impl std::any::Any) {
        self.graveyard.borrow_mut().push(Box::new(x));
    }

    fn entry_kind(&self, h: u64) -> Result<(u32, u32), String> {
        self.table.borrow().get(h).map(|e| (e.kind, e.life)).ok_or_else(|| format!("stale or foreign handle {h}"))
    }

    fn object(&self, h: u64) -> Option<Retained<AnyObject>> {
        self.table.borrow().get(h).and_then(|e| e.load())
    }

    /// The live object of `h`. A borrowed handle whose object is gone is reported dead (owner).
    fn live(&self, h: u64) -> Result<Retained<AnyObject>, String> {
        let r = self.table.borrow().get(h).map(|e| e.load());
        match r {
            Some(Some(o)) => Ok(o),
            Some(None) => {
                self.forget(h);
                if let Some(host) = self.host() {
                    unsafe { (host.on_destroyed)(host.self_, h, SVL_GONE_OWNER) };
                }
                Err(format!("handle {h} was destroyed by its owner"))
            }
            None => Err(format!("stale or foreign handle {h}")),
        }
    }

    fn forget(&self, h: u64) {
        if let Some(e) = self.table.borrow_mut().take(h) {
            self.by_ptr.borrow_mut().retain(|_, v| *v != h);
            self.bury(e);
        }
    }

    fn class_of_kind(&self, kind: u32) -> Option<&'static AnyClass> {
        if kind >= TYPE_KIND_BASE {
            return self.type_kind(kind).map(|t| t.1);
        }
        lookup::kind(kind).and_then(|k| bridge::class_named(k.class))
    }

    // ---- props ----

    fn prop_spec(&self, kind: u32, prop: u32) -> Result<PropSpec, String> {
        if prop >= DYN_BASE {
            let d = self.dyn_props.borrow();
            let (k, _, p) = d.get((prop - DYN_BASE) as usize).ok_or("unknown prop id")?;
            if !lookup::chain(kind).contains(k) && *k != kind && !self.type_chain(kind).contains(k) {
                return Err("prop does not apply to this kind".into());
            }
            return Ok(PropSpec { key: p.key.clone(), ty: p.ty.clone(), members: &[], flags: false, writable: p.writable, setter: "", write: None, string: false });
        }
        let info = lookup::prop_info(prop).ok_or("unknown prop id")?;
        if !lookup::chain(kind).contains(&(info.kind as u32)) {
            return Err(format!("prop {} does not apply to this kind", info.name));
        }
        let (members, flags) = lookup::members(info.ty);
        Ok(PropSpec {
            key: info.key.to_string(),
            ty: native_ty(info.ty),
            members,
            flags,
            writable: info.writable,
            setter: info.setter,
            write: lookup::prop_write(prop),
            string: info.ty == Ty::Str,
        })
    }

    /// `set_prop` as a host op: marks (h, prop) as our own write while it runs (REACTIVITY.md 5.4).
    fn set_prop(&self, h: u64, prop: u32, v: &Value) -> Result<(), String> {
        let prev = self.setting.replace(Some((h, prop)));
        let r = self.set_prop_raw(h, prop, v);
        self.setting.set(prev);
        r
    }

    /// Validates everything about a write before any value is converted or any native is touched:
    /// the prop must apply to the kind, be writable, and not be child storage owned by a slot.
    fn set_prop_raw(&self, h: u64, prop: u32, v: &Value) -> Result<(), String> {
        let (kind, _) = self.entry_kind(h)?;
        let spec = self.prop_spec(kind, prop)?;
        if spec.setter.is_empty() {
            if let Some(slot) = lookup::slot_owning_key(kind, &spec.key) {
                return Err(format!(
                    "{} is read-only: children are managed through the {} slot (insert/remove)",
                    names_of(&spec.key),
                    slot.name
                ));
            }
            if !spec.writable {
                return Err(format!("{} is read-only", names_of(&spec.key)));
            }
        }
        let obj = self.live(h)?;
        if let Some(w) = spec.write {
            return self.write_through(&obj, w, &spec, v);
        }
        if !spec.setter.is_empty() {
            let arg = match (&spec.ty, v) {
                (enc::Ty::I(_) | enc::Ty::U(_), Value::Str(_)) => match enc::scalar_in(v, &spec.ty, spec.members, spec.flags)? {
                    enc::Scalar::I64(i) => Value::I64(i),
                    enc::Scalar::U64(u) => Value::U64(u),
                    _ => v.clone(),
                },
                _ => v.clone(),
            };
            bridge::invoke(&obj, bridge::sel(spec.setter), &[arg], self)?;
            return Ok(());
        }
        let v = if spec.string { stringify(v) } else { v.clone() };
        let boxed = bridge::box_value(&v, &spec.ty, spec.members, spec.flags, self)?;
        bridge::kvc_set(&obj, &spec.key, boxed.as_deref())
    }

    /// A prop written through methods (`PROP_WRITES`): `clear` then `selector` with the value, or an indexed
    /// setter (`count` with the list's length, then `selector` with (item, index) per item).
    fn write_through(&self, obj: &AnyObject, w: &PropWrite, spec: &PropSpec, v: &Value) -> Result<(), String> {
        if !w.count.is_empty() {
            let items: Vec<Value> = match v {
                Value::List(items) => items.clone(),
                Value::Null => Vec::new(),
                _ => return Err(format!("{} takes a list", names_of(w.selector))),
            };
            bridge::invoke(obj, bridge::sel(w.count), &[Value::I64(items.len() as i64)], self)?;
            for (i, it) in items.into_iter().enumerate() {
                let it = if matches!(it, Value::Str(_) | Value::Str16(_) | Value::Handle(_)) { it } else { stringify(&it) };
                bridge::invoke(obj, bridge::sel(w.selector), &[it, Value::I64(i as i64)], self)?;
            }
            return Ok(());
        }
        // A list prop takes null as "no items"; a string list stringifies numbers.
        let v = match (v, &spec.ty) {
            (Value::Null, enc::Ty::Object(None)) => Value::List(Vec::new()),
            (Value::List(items), _) => Value::List(items.iter().map(|x| if matches!(x, Value::Handle(_)) { x.clone() } else { stringify(x) }).collect()),
            _ => v.clone(),
        };
        if !w.clear.is_empty() {
            bridge::send0(obj, w.clear)?;
        }
        bridge::invoke(obj, bridge::sel(w.selector), &[v], self).map(|_| ())
    }

    fn get_prop(&self, h: u64, prop: u32) -> Result<Value, String> {
        let (kind, _) = self.entry_kind(h)?;
        let spec = self.prop_spec(kind, prop)?;
        if !spec.setter.is_empty() {
            return Err(format!("{} is write-only (set with {})", names_of(&spec.key), spec.setter));
        }
        let obj = self.live(h)?;
        let raw = bridge::kvc_get(&obj, &spec.key)?;
        let prev = self.cur_owner.replace(h);
        let types = self.in_types.replace(kind >= TYPE_KIND_BASE);
        let v = bridge::unbox_value(raw.as_deref(), &spec.ty, spec.members, spec.flags, self);
        self.in_types.set(types);
        self.cur_owner.set(prev);
        Ok(v)
    }

    // ---- tree ----

    fn create(&self, kind: u32) -> Result<u64, String> {
        let info = lookup::kind(kind).filter(|k| !k.tag.is_empty()).ok_or_else(|| format!("unknown kind id {kind}"))?;
        let cls = bridge::class_named(info.class).ok_or_else(|| format!("class {} is not available on this macOS", info.class))?;
        let obj: Retained<AnyObject> = match info.ctor {
            Ctor::New => bridge::catch(info.class, || unsafe { msg_send![cls, new] })?,
            Ctor::Window { width, height, style_mask } => {
                let rect = Rect { x: 0.0, y: 0.0, w: width, h: height };
                let w: Option<Retained<AnyObject>> = bridge::catch(info.class, || unsafe {
                    let a: objc2::rc::Allocated<AnyObject> = msg_send![cls, alloc];
                    msg_send![a, initWithContentRect: rect, styleMask: style_mask as usize, backing: NS_BACKING_STORE_BUFFERED, defer: false]
                })?;
                let w = w.ok_or("NSWindow init failed")?;
                unsafe {
                    let _: () = msg_send![&*w, setReleasedWhenClosed: false];
                    let _: () = msg_send![&*w, center];
                }
                w
            }
        };
        let life = if info.toplevel { SVL_LIFE_TOPLEVEL } else { SVL_LIFE_OWNED };
        let ptr = Retained::as_ptr(&obj) as usize;
        let h = self.table.borrow_mut().mint(Native::strong(obj.clone()), kind, life);
        self.by_ptr.borrow_mut().insert(ptr, h);
        self.install_delegate(h, kind, &obj, info.toplevel);
        Ok(h)
    }

    /// The backend's own delegate, installed at create (AppKit holds delegates weakly, so the entry keeps it):
    /// windows (close cascade, vetoable close) and tab views (selection events, vetoable selection).
    fn install_delegate(&self, h: u64, kind: u32, obj: &AnyObject, toplevel: bool) {
        let d: Retained<NSObject> = if toplevel {
            Retained::into_super(WindowDelegate::new(self.mtm, h))
        } else if lookup::chain(kind).contains(&lookup::kind_by_class("NSTabView")) {
            Retained::into_super(TabViewDelegate::new(self.mtm, h))
        } else {
            return;
        };
        let _: () = unsafe { msg_send![obj, setDelegate: &*d] };
        self.table.borrow_mut().get_mut(h).unwrap().delegate = Some(d);
    }

    fn slot_for(&self, parent_kind: u32, slot: u32) -> Result<(u32, &'static SlotInfo), String> {
        let id = if slot == 0 { lookup::default_slot(parent_kind) } else { slot };
        let info = lookup::slot_info(id).ok_or("this element takes no children")?;
        if !lookup::chain(parent_kind).contains(&(info.kind as u32)) {
            return Err(format!("slot {} does not belong to this element", info.name));
        }
        Ok((id, info))
    }

    fn insert(&self, parent: u64, child: u64, before: Option<u64>, slot: u32) -> Result<(), String> {
        let (pk, plife) = self.entry_kind(parent)?;
        let (_, clife) = self.entry_kind(child)?;
        if plife == SVL_LIFE_BORROWED || clife == SVL_LIFE_BORROWED {
            return Err("borrowed handles cannot be inserted or be parents".into());
        }
        if clife == SVL_LIFE_TOPLEVEL {
            return Err("windows inside windows (transient toplevels) are not supported by the AppKit backend yet".into());
        }
        let (sid, info) = self.slot_for(pk, slot)?;
        let p = self.live(parent)?;
        let c = self.live(child)?;
        if let Some(b) = before {
            let same = self.table.borrow().get(b).and_then(|e| e.parent) == Some((parent, sid));
            if !same {
                return Err(format!("before handle {b} is not a child of {parent} in slot {}", info.name));
            }
        }
        // One-child slots replace: the old child is released by the parent, not destroyed.
        let replaced = if info.many { None } else { self.table.borrow().children_in_slot(parent, sid).first().copied() };
        // The old child is unlinked only so `link` can validate the new one; any failure puts it
        // back, so a refused insert never leaves the table disagreeing with the native tree.
        if let Some(old) = replaced {
            self.table.borrow_mut().unlink(parent, old)?;
        }
        let restore = |this: &Self| {
            if let Some(old) = replaced {
                let _ = this.table.borrow_mut().link(parent, old, sid, None);
            }
        };
        let linked = self.table.borrow_mut().link(parent, child, sid, before);
        if let Err(e) = linked {
            restore(self);
            return Err(e);
        }
        let r = self.native_insert(&p, &c, before.and_then(|b| self.object(b)).as_deref(), info);
        if r.is_err() {
            let _ = self.table.borrow_mut().unlink(parent, child);
            restore(self);
        }
        r
    }

    fn native_insert(&self, p: &AnyObject, c: &AnyObject, before: Option<&AnyObject>, info: &SlotInfo) -> Result<(), String> {
        if !info.many {
            if info.set == "setDocumentView:" && sizes_itself(c) {
                return bridge::catch("document-view", || unsafe { pin_document(self.mtm, p, c) });
            }
            return bridge::send_obj(p, info.set, Some(c));
        }
        match before {
            Some(b) if !info.insert_at.is_empty() => {
                let list = bridge::kvc_get(p, info.list)?;
                let idx: usize = match list {
                    Some(l) => unsafe { msg_send![&*l, indexOfObject: b] },
                    None => usize::MAX,
                };
                if idx == usize::MAX {
                    return Err("before is not in the native child list".into());
                }
                let s = bridge::sel(info.insert_at);
                bridge::catch(info.insert_at, || unsafe {
                    let _: () = p.send_message(s, (Some(c), idx as isize));
                })
            }
            Some(b) if !info.insert_relative.is_empty() => {
                let s = bridge::sel(info.insert_relative);
                bridge::catch(info.insert_relative, || unsafe {
                    let _: () = p.send_message(s, (Some(c), NS_WINDOW_BELOW, Some(b)));
                })
            }
            _ if !info.append.is_empty() => bridge::send_obj(p, info.append, Some(c)),
            _ => Err(format!("slot {} cannot append", info.name)),
        }
    }

    fn native_remove(&self, p: &AnyObject, c: &AnyObject, info: &SlotInfo) -> Result<(), String> {
        if !info.many {
            let mut cur = if info.get.is_empty() { None } else { bridge::kvc_get(p, info.get)? };
            if info.set == "setDocumentView:" && cur.as_deref().is_some_and(|d| bridge::is_kind(d, <DocumentView as objc2::ClassType>::class())) {
                // the flipped wrapper of pin_document: unwrap the child
                let sup: Option<Retained<AnyObject>> = unsafe { msg_send![c, superview] };
                if sup.as_deref().zip(cur.as_deref()).is_some_and(|(a, b)| std::ptr::eq(a, b)) {
                    unsafe {
                        let _: () = msg_send![c, removeFromSuperview];
                    }
                    cur = Some(unsafe { Retained::retain(c as *const AnyObject as *mut AnyObject) }.unwrap());
                }
            }
            if cur.as_deref().is_some_and(|x| std::ptr::eq(x, c)) || info.get.is_empty() {
                match info.clear {
                    Clear::Nil => bridge::send_obj(p, info.set, None)?,
                    Clear::EmptyView => {
                        let v: Retained<AnyObject> = unsafe { msg_send![class!(NSView), new] };
                        bridge::send_obj(p, info.set, Some(&v))?;
                    }
                }
            }
            return Ok(());
        }
        for s in info.remove {
            bridge::send_obj(p, s, Some(c))?;
        }
        // Children that are not views (tab-view-item, menu-item) have no superview: `remove` was all.
        if info.detach.is_empty() || !bridge::responds(c, sel!(superview)) {
            return Ok(());
        }
        let sup: Option<Retained<AnyObject>> = unsafe { msg_send![c, superview] };
        if sup.as_deref().is_some_and(|s| std::ptr::eq(s, p)) {
            for s in info.detach {
                bridge::send0(c, s)?;
            }
        }
        Ok(())
    }

    fn remove(&self, parent: u64, child: u64) -> Result<(), String> {
        let (pk, _) = self.entry_kind(parent)?;
        let sid = self.table.borrow_mut().unlink(parent, child)?;
        let info = lookup::slot_info(sid).ok_or("unknown slot")?;
        let _ = pk;
        let (Some(p), Some(c)) = (self.object(parent), self.object(child)) else { return Ok(()) };
        self.native_remove(&p, &c, info)
    }

    fn cleanup(&self, h: u64, e: &Native) {
        let Some(obj) = e.load() else { return };
        // Observers go first, while the object is certainly alive: KVO must be removed before it deallocates.
        self.remove_observations(&obj, e);
        if let Some(t) = &e.target {
            unsafe {
                if bridge::responds(&obj, sel!(setTarget:)) {
                    let cur: Option<Retained<AnyObject>> = msg_send![&*obj, target];
                    if cur.as_deref().is_some_and(|c| std::ptr::eq(c, t.as_any())) {
                        let _: () = msg_send![&*obj, setTarget: Option::<&AnyObject>::None];
                        let _: () = msg_send![&*obj, setAction: Option::<Sel>::None];
                    }
                }
                let center: Retained<AnyObject> = msg_send![class!(NSNotificationCenter), defaultCenter];
                let _: () = msg_send![&*center, removeObserver: t.as_any()];
            }
        }
        if e.delegate.is_some() {
            let _: () = unsafe { msg_send![&*obj, setDelegate: Option::<&AnyObject>::None] };
        }
        if let Some((w, link)) = self.display_link.borrow().as_ref() {
            if *w == h {
                let _: () = unsafe { msg_send![&**link, invalidate] };
            }
        }
        let mut dl = self.display_link.borrow_mut();
        if dl.as_ref().is_some_and(|(w, _)| *w == h) {
            if let Some(x) = dl.take() {
                self.bury(x.1);
            }
        }
    }

    fn destroy(&self, h: u64) {
        self.drop_pending_of(h);
        self.style_forget(h);
        let parent = self.table.borrow().get(h).and_then(|e| e.parent);
        let Some(e) = self.table.borrow_mut().take(h) else { return };
        self.by_ptr.borrow_mut().remove(&match &e.obj.obj {
            Obj::Strong(o) => Retained::as_ptr(o) as usize,
            Obj::Weak(_) => 0,
        });
        self.by_ptr.borrow_mut().retain(|_, v| *v != h);
        self.cleanup(h, &e);
        if e.life != SVL_LIFE_BORROWED {
            if let (Some((p, sid)), Some(c)) = (parent, e.load()) {
                if let (Some(po), Some(info)) = (self.object(p), lookup::slot_info(sid)) {
                    let _ = self.native_remove(&po, &c, info);
                }
            }
            if e.life == SVL_LIFE_TOPLEVEL {
                if let Some(w) = e.load() {
                    let _ = bridge::catch("close", || unsafe {
                        let _: () = msg_send![&*w, orderOut: Option::<&AnyObject>::None];
                        let _: () = msg_send![&*w, close];
                    });
                }
            }
        }
        self.bury(e);
    }

    // ---- methods ----

    fn call(&self, h: u64, method: u32, args: &[Value]) -> Result<Value, String> {
        let (kind, _) = self.entry_kind(h)?;
        if kind >= TYPE_KIND_BASE {
            return self.type_call(h, kind, method, args);
        }
        let s = if method >= DYN_BASE {
            let d = self.dyn_methods.borrow();
            let (k, _, s) = d.get((method - DYN_BASE) as usize).ok_or("unknown method id")?;
            if !lookup::chain(kind).contains(k) {
                return Err("method does not apply to this kind".into());
            }
            *s
        } else {
            let info = lookup::method_info(method).ok_or("unknown method id")?;
            if !lookup::chain(kind).contains(&(info.kind as u32)) {
                return Err(format!("method {} does not apply to this kind", info.name));
            }
            bridge::sel(info.selector)
        };
        let obj = self.live(h)?;
        let prev = self.cur_owner.replace(h);
        let r = bridge::invoke(&obj, s, args, self);
        self.cur_owner.set(prev);
        r
    }

    // ---- signals ----

    fn listen(&self, h: u64, signal: u32, on: bool) -> Result<(), String> {
        let (kind, life) = self.entry_kind(h)?;
        let info = lookup::signal_info(signal).ok_or("unknown signal id")?;
        if !lookup::chain(kind).contains(&(info.kind as u32)) {
            return Err(format!("signal {} does not apply to this kind", info.name));
        }
        let obj = self.live(h)?;
        let already = self.table.borrow().get(h).is_some_and(|e| e.listening.contains(&signal));
        if on == already {
            return Ok(());
        }
        match info.via {
            Via::TargetAction | Via::Notification => {
                let t = self.target_of(h);
                if info.via == Via::TargetAction {
                    if !bridge::responds(&obj, sel!(setTarget:)) {
                        return Err(format!("{} has no target/action", bridge::class_name_of(&obj)));
                    }
                    let _ = t;
                    self.action_use(h, &obj, on);
                } else {
                    self.note_use(h, Some(&obj), info.native, on);
                }
            }
            Via::Delegate => {
                if life == SVL_LIFE_BORROWED {
                    return Err("delegate signals of borrowed objects are not supported".into());
                }
            }
        }
        let mut t = self.table.borrow_mut();
        let e = t.get_mut(h).ok_or("stale handle")?;
        if on {
            e.listening.push(signal);
        } else {
            e.listening.retain(|&s| s != signal);
        }
        Ok(())
    }

    fn target_of(&self, h: u64) -> Retained<SveltersTarget> {
        let mut t = self.table.borrow_mut();
        let e = t.get_mut(h).expect("live handle");
        e.target.get_or_insert_with(|| SveltersTarget::new(self.mtm, h)).clone()
    }

    fn listened(&self, h: u64, pred: impl Fn(&SignalInfo) -> bool) -> Option<u32> {
        let t = self.table.borrow();
        let e = t.get(h)?;
        e.listening.iter().copied().find(|&s| lookup::signal_info(s).is_some_and(&pred))
    }

    fn signal_args(&self, h: u64, info: &SignalInfo) -> Vec<Value> {
        let (kind, _) = match self.entry_kind(h) {
            Ok(k) => k,
            Err(_) => return Vec::new(),
        };
        info.args.iter().map(|a| self.get_prop(h, lookup::prop(kind, a)).unwrap_or(Value::Null)).collect()
    }

    fn on_target(&self, ev: TargetEvent<'_>) {
        match ev {
            TargetEvent::Action { token, .. } => {
                if let Some(sig) = self.listened(token, |s| s.via == Via::TargetAction) {
                    let info = lookup::signal_info(sig).unwrap();
                    let args = self.signal_args(token, info);
                    self.post(token, sig, &args);
                }
                self.observed_fired(token, |o| o.action);
                self.style_refresh_states(token);
            }
            TargetEvent::Notification { token, name, .. } => {
                if let Some(sig) = self.listened(token, |s| s.via == Via::Notification && s.native == name) {
                    let info = lookup::signal_info(sig).unwrap();
                    let args = self.signal_args(token, info);
                    self.post(token, sig, &args);
                }
                self.observed_fired(token, |o| o.notifications.contains(&name.as_str()));
            }
            TargetEvent::Tick { .. } => self.deliver_frame("link tick"),
        }
    }

    /// `windowShouldClose:`. A listened `window-should-close` runs JS now (call_sync); a busy host vetoes.
    fn should_close(&self, h: u64) -> bool {
        self.delegate_veto(h, "windowShouldClose:", false)
    }

    /// A vetoable delegate method (`result: veto_bool`): a listened signal runs JS now (call_sync) and a `true`
    /// result (`e.preventDefault()`) makes the delegate answer NO. No listener, no host or no decision allows;
    /// a busy host (JS running, e.g. a select call from JS) answers `busy_allows`.
    fn delegate_veto(&self, h: u64, selector: &str, busy_allows: bool) -> bool {
        let Some(sig) = self.listened(h, |s| s.via == Via::Delegate && s.native == selector) else { return true };
        let Some(host) = self.host() else { return true };
        let info = lookup::signal_info(sig).unwrap();
        let args = self.signal_args(h, info);
        let raw = RawValues::new(&args);
        let mut out = svl_value::null();
        let rc = unsafe { (host.call_sync)(host.self_, h, sig, raw.as_ptr(), raw.len(), &mut out) };
        match rc {
            SVL_SYNC_OK => {
                let stop = match out.tag {
                    SVL_BOOL => unsafe { out.u.b },
                    SVL_I64 => unsafe { out.u.i != 0 },
                    _ => false,
                };
                !stop
            }
            SVL_SYNC_BUSY => busy_allows,
            _ => true,
        }
    }

    /// A notifying delegate method: posts the listened signal with its `args` (props of the emitter).
    fn delegate_post(&self, h: u64, selector: &str) {
        if let Some(sig) = self.listened(h, |s| s.via == Via::Delegate && s.native == selector) {
            let info = lookup::signal_info(sig).unwrap();
            let args = self.signal_args(h, info);
            self.post(h, sig, &args);
        }
    }

    /// `windowWillClose:`: post the event if listened, then report the window and everything
    /// inserted below it dead (the window first, `closed`; the rest `parent`).
    fn will_close(&self, h: u64) {
        if !self.table.borrow().contains(h) {
            return; // destroyed by the app: `destroy` already forgot it
        }
        if let Some(sig) = self.listened(h, |s| s.via == Via::Delegate && s.native == "windowWillClose:") {
            self.post(h, sig, &[]);
        }
        let sub = self.table.borrow().subtree(h);
        let mut reports = Vec::new();
        for (i, x) in sub.into_iter().enumerate() {
            let e = self.table.borrow_mut().take(x);
            if let Some(e) = e {
                self.by_ptr.borrow_mut().retain(|_, v| *v != x);
                self.cleanup(x, &e);
                self.bury(e);
                reports.push((x, if i == 0 { SVL_GONE_CLOSED } else { SVL_GONE_PARENT }));
            }
        }
        if let Some(host) = self.host() {
            for (x, r) in reports {
                unsafe { (host.on_destroyed)(host.self_, x, r) };
            }
        }
        // Let the host notice (and maybe quit) from the loop's top level.
        self.rl.waker().wake();
    }

    // ---- frames ----

    /// One frame per request. With a `CADisplayLink` the frame comes from the next vsync tick, but
    /// a link only ticks while its view is on a screen that is compositing (not hidden, minimised,
    /// fully occluded, or on a headless VM display), so the fallback timer is armed as a watchdog
    /// every time: whichever fires first delivers, the other finds nothing pending.
    fn request_frame(&self) {
        if self.frame_pending.replace(true) {
            return;
        }
        if self.display_link.borrow().is_none() {
            self.make_display_link();
        }
        match self.display_link.borrow().as_ref() {
            Some((_, link)) => {
                let _: () = unsafe { msg_send![&**link, setPaused: false] };
                trace_frames(|| "request: link unpaused, watchdog armed".into());
                self.rl.frame_fallback(FRAME_WATCHDOG_MS);
            }
            None => {
                trace_frames(|| "request: no link, 16 ms timer".into());
                self.rl.frame_fallback(16);
            }
        }
    }

    /// `-[NSView displayLinkWithTarget:selector:]` (macOS 14+) on the content view of a live window.
    fn make_display_link(&self) {
        let windows: Vec<(u64, Retained<AnyObject>)> = self
            .table
            .borrow()
            .iter()
            .filter(|(_, e)| e.life == SVL_LIFE_TOPLEVEL)
            .filter_map(|(h, e)| e.load().map(|o| (h, o)))
            .collect();
        for (h, w) in windows {
            let view: Option<Retained<AnyObject>> = unsafe { msg_send![&*w, contentView] };
            let Some(view) = view else { continue };
            if !bridge::responds(&view, sel!(displayLinkWithTarget:selector:)) {
                trace_frames(|| "no -displayLinkWithTarget:selector: (macOS < 14)".into());
                return; // macOS < 14: the 16 ms timer
            }
            let link: Option<Retained<AnyObject>> = unsafe { msg_send![&*view, displayLinkWithTarget: self.frame_target.as_any(), selector: sel!(tick:)] };
            trace_frames(|| {
                let (visible, occl, screen): (bool, usize, Option<Retained<AnyObject>>) =
                    unsafe { (msg_send![&*w, isVisible], msg_send![&*w, occlusionState], msg_send![&*w, screen]) };
                format!(
                    "link for window {h}: {} (visible={visible} occlusion={occl:#x} screen={})",
                    link.as_deref().map_or("nil".into(), bridge::class_name_of),
                    screen.is_some()
                )
            });
            if let Some(link) = link {
                unsafe {
                    let rl: Retained<AnyObject> = msg_send![class!(NSRunLoop), mainRunLoop];
                    // The real constant: CADisplayLink does not treat an equal string as the
                    // common-modes pseudo-mode (an NSString "kCFRunLoopCommonModes" never ticks).
                    let _: () = msg_send![&*link, addToRunLoop: &*rl, forMode: NSRunLoopCommonModes];
                    let _: () = msg_send![&*link, setPaused: true];
                }
                *self.display_link.borrow_mut() = Some((h, link));
                return;
            }
        }
    }

    /// Delivers the pending frame (from a link tick or the timer) and pauses the link, so it does
    /// not keep ticking after a timer-delivered frame.
    fn deliver_frame(&self, from: &str) {
        let pending = self.frame_pending.replace(false);
        trace_frames(|| format!("{from}: pending={pending}"));
        if !pending {
            return;
        }
        if let Some((_, link)) = self.display_link.borrow().as_ref() {
            let _: () = unsafe { msg_send![&**link, setPaused: true] };
        }
        let (link, timer) = self.frame_sources.get();
        self.frame_sources.set(if from == "link tick" { (link + 1, timer) } else { (link, timer + 1) });
        if let Some(h) = self.host() {
            unsafe { (h.frame)(h.self_) };
        }
    }

    // ---- loop ----

    fn run(&self, host: *const svl_host) -> Result<(), String> {
        self.host.set(host);
        let app = NSApplication::sharedApplication(self.mtm);
        // `activate` (macOS 14+) is only a request the system ignores unless the active app yields, so
        // an app started from a terminal stayed behind it (keys like Cmd+Q went to the terminal). The
        // deprecated `activateIgnoringOtherApps:` still brings it forward (checked on macOS 15.7).
        unsafe {
            let _: () = msg_send![&*app, activateIgnoringOtherApps: true];
        }
        // A checkpoint as soon as the loop runs (jobs the bundle left pending).
        self.rl.waker().wake();
        if !self.quit_requested.get() {
            self.running.set(true);
            app.run();
            self.running.set(false);
        }
        self.quit_requested.set(false);
        self.drain();
        Ok(())
    }

    /// ABI v2.7 `start`: attaches the host without running the loop (`AppKitBackend::new` already
    /// set up `NSApplication`, its menu and the run-loop sources); the caller pumps the run loop or
    /// calls `run` later. Like `run`, it queues a checkpoint for the first loop turn. NULL detaches:
    /// wakes, timers and frames read the host when they fire, so none reaches a detached host.
    fn start(&self, host: *const svl_host) {
        self.host.set(host);
        if !host.is_null() {
            self.rl.waker().wake();
        }
    }

    fn quit(&self) {
        self.quit_requested.set(true);
        if !self.running.get() {
            return;
        }
        let app = NSApplication::sharedApplication(self.mtm);
        app.stop(None);
        // `stop:` takes effect after the next event: post one.
        unsafe {
            let ev: Option<Retained<AnyObject>> = msg_send![
                class!(NSEvent),
                otherEventWithType: NS_EVENT_TYPE_APPLICATION_DEFINED,
                location: Point { x: 0.0, y: 0.0 },
                modifierFlags: 0usize,
                timestamp: 0.0f64,
                windowNumber: 0isize,
                context: Option::<&AnyObject>::None,
                subtype: 0i16,
                data1: 0isize,
                data2: 0isize
            ];
            if let Some(ev) = ev {
                let _: () = msg_send![&*app, postEvent: &*ev, atStart: true];
            }
        }
    }
}

/// `NSPoint` with its ObjC encoding.
#[repr(C)]
#[derive(Clone, Copy)]
struct Point {
    x: f64,
    y: f64,
}

unsafe impl Encode for Point {
    const ENCODING: Encoding = Encoding::Struct("CGPoint", &[Encoding::Double, Encoding::Double]);
}

impl Objects for Inner {
    fn object(&self, h: u64) -> Option<Retained<AnyObject>> {
        Inner::object(self, h)
    }

    fn attributed_as_handle(&self) -> bool {
        self.in_types.get()
    }

    fn handle_of(&self, obj: &AnyObject) -> u64 {
        let ptr = obj as *const AnyObject as usize;
        if let Some(h) = self.by_ptr.borrow().get(&ptr) {
            return *h;
        }
        let found = self.table.borrow().iter().find_map(|(h, e)| match &e.obj.obj {
            Obj::Weak(w) => w.load().filter(|o| Retained::as_ptr(o) as usize == ptr).map(|_| h),
            Obj::Strong(_) => None,
        });
        if let Some(h) = found {
            return h;
        }
        // NON-GUI types (decision Q1): a non-widget object is a SHARED handle with a strong reference.
        if let Some(h) = self.shared_handle(obj) {
            return h;
        }
        // A native-made object: mint a borrowed handle of the nearest kind along its class chain.
        let kind = bridge::class_chain(obj.class())
            .into_iter()
            .map(|c| lookup::kind_by_class(&c.name().to_string_lossy()))
            .find(|&k| k != 0)
            .unwrap_or(0);
        let native = Native { obj: Obj::Weak(Weak::new(obj)), target: None, delegate: None, listening: Vec::new(), obs: Default::default() };
        let mut t = self.table.borrow_mut();
        let h = t.mint(native, kind, SVL_LIFE_BORROWED);
        let owner = self.cur_owner.get();
        if owner != 0 && t.contains(owner) {
            t.get_mut(h).unwrap().owner = Some(owner);
        }
        h
    }
}

/// The bridge type of a table type.
fn native_ty(t: Ty) -> enc::Ty {
    match t {
        Ty::Bool => enc::Ty::Bool,
        Ty::I32 => enc::Ty::I(4),
        Ty::I64 => enc::Ty::I(8),
        Ty::U32 => enc::Ty::U(4),
        Ty::U64 => enc::Ty::U(8),
        Ty::F32 => enc::Ty::F32,
        Ty::F64 => enc::Ty::F64,
        Ty::Str => enc::Ty::Object(Some("NSString".into())),
        Ty::Object(c) => enc::Ty::Object((!c.is_empty()).then(|| c.to_string())),
        Ty::Rect => enc::Ty::Struct(StructTy::Rect),
        Ty::Size => enc::Ty::Struct(StructTy::Size),
        Ty::Point => enc::Ty::Struct(StructTy::Point),
        Ty::Insets => enc::Ty::Struct(StructTy::Insets),
        Ty::Range => enc::Ty::Struct(StructTy::Range),
        Ty::Enum(_) => enc::Ty::I(8),
        Ty::Flags(_) => enc::Ty::U(8),
        Ty::List => enc::Ty::Object(None),
    }
}

/// String props take any primitive: `string-value={count}` shows the number; null is "".
fn stringify(v: &Value) -> Value {
    match v {
        Value::Null => Value::Str(String::new()),
        Value::Bool(b) => Value::Str(b.to_string()),
        Value::I64(i) => Value::Str(i.to_string()),
        Value::U64(u) => Value::Str(u.to_string()),
        Value::F64(f) if f.fract() == 0.0 && f.abs() < 1e15 => Value::Str(format!("{}", *f as i64)),
        Value::F64(f) => Value::Str(f.to_string()),
        Value::Str16(w) => Value::Str(String::from_utf16_lossy(w)),
        other => other.clone(),
    }
}

fn names_of(key: &str) -> String {
    svelters_apple::names::kebab(key)
}

impl Backend for AppKitBackend {
    fn resolve(&self, what: NameKind, kind: svl_id, name: &str) -> svl_id {
        let i = &self.inner;
        match what {
            NameKind::Kind => match lookup::kind_by_tag(name) {
                0 => i.type_kind_by_name(name),
                k => k,
            },
            NameKind::Prop => {
                let id = lookup::prop(kind, name);
                if id != 0 {
                    return id;
                }
                i.dyn_prop(kind, name)
            }
            NameKind::Signal => lookup::signal(kind, name),
            NameKind::Method if kind >= TYPE_KIND_BASE => i.type_method(kind, name),
            NameKind::Method => {
                let id = lookup::method(kind, name);
                if id != 0 {
                    return id;
                }
                i.dyn_method(kind, name)
            }
            NameKind::Slot => lookup::slot(kind, name),
            _ => 0,
        }
    }

    fn create(&self, kind: svl_id) -> StrResult<svl_handle> {
        objc2::rc::autoreleasepool(|_| self.inner.create(kind))
    }

    fn insert(&self, parent: svl_handle, child: svl_handle, before: Option<svl_handle>, slot: svl_id) -> StrResult<()> {
        objc2::rc::autoreleasepool(|_| {
            self.inner.insert(parent, child, before, slot)?;
            self.inner.style_after_insert(child);
            Ok(())
        })
    }

    fn remove(&self, parent: svl_handle, child: svl_handle) -> StrResult<()> {
        objc2::rc::autoreleasepool(|_| self.inner.remove(parent, child))
    }

    fn destroy(&self, h: svl_handle) {
        objc2::rc::autoreleasepool(|_| self.inner.destroy(h))
    }

    fn set_prop(&self, h: svl_handle, prop: svl_id, v: &Value) -> StrResult<()> {
        objc2::rc::autoreleasepool(|_| {
            let r = self.inner.set_prop(h, prop, v);
            self.inner.style_after_set(h);
            r
        })
    }

    fn style(&self, op: u32, h: svl_handle, text: Option<&str>) -> StrResult<i32> {
        objc2::rc::autoreleasepool(|_| self.inner.style_op(op, h, text))
    }

    fn get_prop(&self, h: svl_handle, prop: svl_id) -> StrResult<Value> {
        objc2::rc::autoreleasepool(|_| self.inner.get_prop(h, prop))
    }

    fn call(&self, h: svl_handle, method: svl_id, args: &[Value]) -> StrResult<Value> {
        objc2::rc::autoreleasepool(|_| self.inner.call(h, method, args))
    }

    fn listen(&self, h: svl_handle, signal: svl_id, on: bool) -> StrResult<()> {
        self.inner.listen(h, signal, on)
    }

    fn handle_info(&self, h: svl_handle) -> StrResult<svl_handle_info> {
        let (kind, life) = self.inner.entry_kind(h)?;
        Ok(svl_handle_info { kind, lifetime: life, flags: 0 })
    }

    fn run(&self, host: &HostRef<'_>) -> StrResult<()> {
        self.inner.run(host.as_raw())
    }

    fn supports_start(&self) -> bool {
        true
    }

    fn start(&self, host: Option<&HostRef<'_>>) -> StrResult<()> {
        self.inner.start(host.map_or(std::ptr::null(), |h| h.as_raw()));
        Ok(())
    }

    fn waker(&self) -> Waker {
        let w = self.inner.rl.waker();
        Arc::new(move || w.wake())
    }

    fn quit(&self) {
        self.inner.quit();
    }

    fn schedule_wake(&self, ms: i64) {
        self.inner.rl.schedule_wake(ms);
    }

    fn request_frame(&self) {
        self.inner.request_frame();
    }

    fn describe(&self, what: NameKind, kind: svl_id, member: svl_id) -> Option<MemberDesc> {
        // ABI v2.5: a kind's native (class) name and its superclass's kind (the host's typeChain).
        if what == NameKind::Kind {
            let (name, parent) = if member >= TYPE_KIND_BASE {
                let (n, _, p) = self.inner.type_kind(member)?;
                (n, p)
            } else {
                let k = lookup::kind(member)?;
                (k.class.to_string(), k.parent as u32)
            };
            return Some(MemberDesc { name, params: Vec::new(), ret: SlotDesc::new(SVL_NULL).typed(parent), flags: 0 });
        }
        if what == NameKind::Method && kind >= TYPE_KIND_BASE {
            return self.inner.type_method_desc(kind, member);
        }
        // Scalar props get descriptors so the renderer coerces static attributes ("8" -> 8,
        // "" -> true). Everything else converts dynamically.
        if what != NameKind::Prop || member >= DYN_BASE {
            return None;
        }
        let p = lookup::prop_info(member)?;
        let slot = match p.ty {
            Ty::Bool => SlotDesc::new(SVL_BOOL),
            Ty::I32 | Ty::I64 | Ty::U32 | Ty::U64 => SlotDesc::new(SVL_I64),
            Ty::F32 | Ty::F64 => SlotDesc::new(SVL_F64),
            Ty::Str => SlotDesc::new(SVL_STR),
            // Well-known records: the host turns `{x, y, width, height}` objects into RECORDs.
            Ty::Rect => SlotDesc::new(SVL_RECORD).typed(SVL_T_RECT),
            Ty::Size => SlotDesc::new(SVL_RECORD).typed(SVL_T_SIZE),
            Ty::Point => SlotDesc::new(SVL_RECORD).typed(SVL_T_POINT),
            Ty::Insets => SlotDesc::new(SVL_RECORD).typed(SVL_T_INSETS),
            _ => return None,
        };
        Some(MemberDesc {
            name: p.name.to_string(),
            params: if p.writable { vec![slot.clone()] } else { Vec::new() },
            ret: if p.setter.is_empty() { slot } else { SlotDesc::new(SVL_NULL) },
            // ABI v2.6: the fact along the element's chain is exact or partial with a mechanism we install.
            flags: if p.setter.is_empty() && lookup::observable(if kind == 0 { p.kind as u32 } else { kind }, p.name) { SVL_M_OBSERVABLE } else { 0 },
        })
    }

    fn supports_observe(&self) -> bool {
        true
    }

    fn observe(&self, h: svl_handle, prop: svl_id, on: bool) -> Result<(), ObserveError> {
        objc2::rc::autoreleasepool(|_| self.inner.observe(h, prop, on))
    }

    fn supports_construct(&self) -> bool {
        true
    }

    fn construct(&self, kind: svl_id, props: &[svl_id], values: &[Value]) -> StrResult<Value> {
        if kind < TYPE_KIND_BASE {
            return Err("construct makes non-GUI objects; elements are made with create".into());
        }
        objc2::rc::autoreleasepool(|_| self.inner.type_construct(kind, props, values))
    }

    fn call_static(&self, kind: svl_id, method: svl_id, args: &[Value]) -> StrResult<Value> {
        if kind < TYPE_KIND_BASE {
            return Err("this kind has no static methods".into());
        }
        objc2::rc::autoreleasepool(|_| self.inner.type_call_static(kind, method, args))
    }

    fn cancel(&self, callback: u64) -> bool {
        self.inner.type_cancel(callback)
    }

    fn describe_type(&self, ty: svl_id) -> Option<TypeDesc> {
        if ty == types::T_ASYNC_DONE {
            return Some(types::async_done_desc());
        }
        let (name, fields): (&str, &[&str]) = match ty {
            SVL_T_RECT => ("Rect", &["x", "y", "width", "height"]),
            SVL_T_SIZE => ("Size", &["width", "height"]),
            SVL_T_POINT => ("Point", &["x", "y"]),
            SVL_T_INSETS => ("Insets", &["top", "right", "bottom", "left"]),
            _ => return None,
        };
        Some(TypeDesc {
            name: name.to_string(),
            kind: SVL_RECORD,
            scope: 0,
            members: Vec::new(),
            fields: fields.iter().map(|f| SlotDesc::new(SVL_F64).named(f)).collect(),
            ret: SlotDesc::new(SVL_NULL),
        })
    }
}

impl Inner {
    fn dyn_prop(&self, kind: u32, name: &str) -> u32 {
        if let Some(i) = self.dyn_props.borrow().iter().position(|(k, n, _)| *k == kind && n == name) {
            return DYN_BASE + i as u32;
        }
        let Some(cls) = self.class_of_kind(kind) else { return 0 };
        let js = if kind >= TYPE_KIND_BASE { svelters_apple::names::kebab(name) } else { name.to_string() };
        let Some((key, attrs)) = bridge::find_property(cls, &js).or_else(|| bridge::find_accessor(cls, &js)) else { return 0 };
        let mut ty = attrs.ty;
        if ty == enc::Ty::I(1) && kind >= TYPE_KIND_BASE && crate::types_table::returns_bool(&key) {
            ty = enc::Ty::Bool; // BOOL is `c` on x86-64
        }
        let mut d = self.dyn_props.borrow_mut();
        d.push((kind, name.to_string(), DynProp { key, ty, writable: !attrs.readonly }));
        DYN_BASE + d.len() as u32 - 1
    }

    fn dyn_method(&self, kind: u32, name: &str) -> u32 {
        if let Some(i) = self.dyn_methods.borrow().iter().position(|(k, n, _)| *k == kind && n == name) {
            return DYN_BASE + i as u32;
        }
        let Some(cls) = self.class_of_kind(kind) else { return 0 };
        let Some(s) = bridge::find_selector(cls, name) else { return 0 };
        let mut d = self.dyn_methods.borrow_mut();
        d.push((kind, name.to_string(), s));
        DYN_BASE + d.len() as u32 - 1
    }
}
