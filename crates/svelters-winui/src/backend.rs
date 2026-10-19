//! The `svl_backend` implementation over the generated tables.
//!
//! Rules this follows (plans/architecture/backend-abi.md):
//! - Handles are minted monotonically and never reused. The table holds one strong COM reference
//!   per handle; `destroy` drops it (and detaches / closes the native object).
//! - No table borrow is held across a native call: WinUI raises some events synchronously inside
//!   setters (`Checked`, `ValueChanged`, `BeforeTextChanging`), and a sync handler re-enters the
//!   backend from JS.
//! - Event handlers capture a [`Sink`] (handle + event id), never a COM reference.
//! - Window close: while JS listens to `closed` the close is vetoed (`Handled`) and posted; otherwise
//!   the window closes and is reported `SVL_GONE_CLOSED`, then its inserted descendants
//!   `SVL_GONE_PARENT`.
use crate::app::Xaml;
use crate::bindings::Microsoft::UI::Dispatching::{DispatcherQueue, DispatcherQueueHandler, DispatcherQueueTimer};
use crate::bindings::Microsoft::UI::Xaml::Media::CompositionTarget;
use crate::bindings::Microsoft::UI::Xaml::{UIElement, Window};
use crate::bindings::Windows::Win32::{DispatchMessageW, GetMessageW, MSG, PM_REMOVE, PeekMessageW, PostQuitMessage, TranslateMessage};
use crate::conv;
use crate::dispatch::{ATTR_GET, ATTR_SET, CREATE, EVENT_CONNECT, METHOD_CALL, PROP_GET, PROP_SET, SLOT_OPS, SlotOps};
use crate::names::EVENTS;
use crate::sink::{Registration, Sink};
use crate::table::{self, Ty};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use svelters_abi::*;
use windows_collections::IVector;
use windows_core::{EventRevoker, IInspectable, IUnknown, Interface};

thread_local! {
    /// `WinUiBackend::after` callbacks, by id, with the timer that fires them.
    static AFTER: RefCell<HashMap<u64, (DispatcherQueueTimer, Option<EventRevoker>, Option<Box<dyn FnOnce()>>)>> = RefCell::default();
    static AFTER_NEXT: Cell<u64> = const { Cell::new(1) };
    static CURRENT: RefCell<Weak<Inner>> = RefCell::new(Weak::new());
    /// Event registrations to drop outside the handler that ended them.
    static GRAVEYARD: RefCell<Vec<Registration>> = const { RefCell::new(Vec::new()) };
}

fn current() -> Option<Rc<Inner>> {
    CURRENT.with(|c| c.borrow().upgrade())
}

struct Entry {
    obj: IInspectable,
    kind: u32,
    parent: Option<u64>,
    /// Slot id the handle was inserted into (0 when not inserted).
    slot: u32,
    children: Vec<u64>,
    listeners: HashMap<(u32, u32), Registration>,
    /// JS listeners of the close event (window `closed`).
    close_listeners: u32,
    /// Always-on registrations (the close handler of a toplevel).
    internal: Vec<Registration>,
}

struct WakeShared {
    queue: DispatcherQueue,
    pending: AtomicBool,
}

pub(crate) struct Inner {
    xaml: Xaml,
    host: Cell<*const svl_host>,
    handles: RefCell<HashMap<u64, Entry>>,
    /// IUnknown identity -> handle, for objects we created (they are alive while in `handles`).
    by_ptr: RefCell<HashMap<usize, u64>>,
    next: Cell<u64>,
    timer: DispatcherQueueTimer,
    frame_timer: DispatcherQueueTimer,
    frame_pending: Cell<bool>,
    frame_sub: RefCell<Option<EventRevoker>>,
    timer_subs: RefCell<Vec<EventRevoker>>,
    wake: Arc<WakeShared>,
    running: Cell<bool>,
    quit_early: Cell<bool>,
}

/// The WinUI backend. One per thread; create it on the thread that will call `run`. Clones share
/// the same state (keep one for driving while another lives in the `BackendVtable`).
#[derive(Clone)]
pub struct WinUiBackend {
    inner: Rc<Inner>,
}

fn identity(o: &IInspectable) -> usize {
    o.cast::<IUnknown>().map(|u| u.as_raw() as usize).unwrap_or(0)
}

/// The handle of an object this backend created, if any.
pub(crate) fn handle_of(o: &IInspectable) -> Option<u64> {
    let inner = current()?;
    let id = identity(o);
    inner.by_ptr.borrow().get(&id).copied()
}

fn host_of(inner: &Inner) -> Option<HostRef<'static>> {
    let p = inner.host.get();
    // SAFETY: set from `run` (or a test harness) and cleared before the host goes away.
    (!p.is_null()).then(|| unsafe { HostRef::from_raw(p) })
}

pub(crate) fn emit_post(sink: Sink, args: &[Value]) {
    let Some(inner) = current() else { return };
    if !inner.handles.borrow().contains_key(&sink.h) {
        return;
    }
    if let Some(host) = host_of(&inner) {
        host.post_event(sink.h, sink.ev, args);
    }
}

pub(crate) fn emit_sync(sink: Sink, args: &[Value]) -> Option<Value> {
    let inner = current()?;
    if !inner.handles.borrow().contains_key(&sink.h) {
        return None;
    }
    let host = host_of(&inner)?;
    drop(inner);
    host.call_sync(sink.h, sink.ev, args).ok()
}

/// A toplevel's close event. `true` vetoes the native close.
pub(crate) fn emit_close(sink: Sink) -> bool {
    let Some(inner) = current() else { return false };
    let listened = match inner.handles.borrow().get(&sink.h) {
        None => return false, // destroyed by the app: let it close
        Some(e) => e.close_listeners > 0,
    };
    if listened {
        if let Some(host) = host_of(&inner) {
            host.post_event(sink.h, sink.ev, &[]);
        }
        return true;
    }
    inner.native_death(sink.h, SVL_GONE_CLOSED);
    false
}

fn on_wake() {
    let Some(inner) = current() else { return };
    inner.wake.pending.store(false, Ordering::SeqCst);
    GRAVEYARD.with(|g| g.borrow_mut().clear());
    if let Some(host) = host_of(&inner) {
        drop(inner);
        host.checkpoint();
    }
}

fn on_timer() {
    let Some(inner) = current() else { return };
    let _ = inner.timer.Stop();
    if let Some(host) = host_of(&inner) {
        drop(inner);
        host.checkpoint();
    }
}

fn on_frame() {
    let Some(inner) = current() else { return };
    let _ = inner.frame_timer.Stop();
    if !inner.frame_pending.replace(false) {
        // Nobody asked for this frame: stop listening (outside this handler).
        if let Some(r) = inner.frame_sub.borrow_mut().take() {
            GRAVEYARD.with(|g| g.borrow_mut().push(r.into()));
            inner.enqueue_wake_noop();
        }
        return;
    }
    if let Some(host) = host_of(&inner) {
        drop(inner);
        host.frame();
    }
}

impl Inner {
    fn get(&self, h: u64) -> StrResult<(IInspectable, u32)> {
        match self.handles.borrow().get(&h) {
            Some(e) => Ok((e.obj.clone(), e.kind)),
            None => Err(format!("stale or foreign handle {h}")),
        }
    }

    fn enqueue_wake_noop(&self) {
        let _ = self.wake.queue.TryEnqueue(&DispatcherQueueHandler::new(|| {
            GRAVEYARD.with(|g| g.borrow_mut().clear());
            Ok(())
        }));
    }

    /// A native death (window closed): forget `h` and every handle inserted below it, then tell the
    /// host, the window first.
    fn native_death(&self, h: u64, reason: u32) {
        let mut order = Vec::new();
        let mut stack = vec![(h, reason)];
        let mut dropped = Vec::new();
        {
            let mut handles = self.handles.borrow_mut();
            if let Some(p) = handles.get(&h).and_then(|e| e.parent) {
                if let Some(pe) = handles.get_mut(&p) {
                    pe.children.retain(|c| *c != h);
                }
            }
            while let Some((cur, why)) = stack.pop() {
                let Some(e) = handles.remove(&cur) else { continue };
                self.by_ptr.borrow_mut().remove(&identity(&e.obj));
                for c in e.children.iter().rev() {
                    stack.push((*c, SVL_GONE_PARENT));
                }
                order.push((cur, why));
                dropped.push(e);
            }
        }
        // Registrations are dropped later: we may be inside the window's own Closed handler.
        GRAVEYARD.with(|g| {
            let mut g = g.borrow_mut();
            for e in dropped {
                g.extend(e.listeners.into_values());
                g.extend(e.internal);
            }
        });
        self.enqueue_wake_noop();
        if let Some(host) = host_of(self) {
            for (cur, why) in order {
                host.on_destroyed(cur, why);
            }
        }
    }

    fn default_slot(&self, kind: u32, slot: u32) -> StrResult<u32> {
        let k = table::kind(kind).ok_or("unknown kind")?;
        if slot == 0 {
            if k.default_slot == 0 {
                return Err(format!("<{}> takes no children", k.tag));
            }
            return Ok(k.default_slot);
        }
        let s = table::slot(slot).ok_or_else(|| format!("unknown slot id {slot}"))?;
        if !table::applies(kind, s.class) {
            return Err(format!("<{}> has no slot {:?}", k.tag, s.name));
        }
        Ok(slot)
    }

    fn any_toplevel(&self) -> bool {
        self.handles.borrow().values().any(|e| table::kind(e.kind).is_some_and(|k| k.toplevel))
    }
}

fn single_occupant(handles: &HashMap<u64, Entry>, parent: u64, slot: u32) -> Option<u64> {
    handles.get(&parent)?.children.iter().copied().find(|c| handles.get(c).is_some_and(|e| e.slot == slot))
}

impl WinUiBackend {
    /// Initializes WinUI on this thread (Windows App SDK runtime, dispatcher queue, Application).
    pub fn new() -> Result<WinUiBackend, String> {
        if current().is_some() {
            return Err("a WinUI backend already exists on this thread".into());
        }
        let xaml = Xaml::init()?;
        let e = |what: &'static str| move |err: windows_core::Error| format!("{what}: {}", conv::err(err));
        let timer = xaml.queue.CreateTimer().map_err(e("DispatcherQueue.CreateTimer"))?;
        timer.SetIsRepeating(false).map_err(e("DispatcherQueueTimer.IsRepeating"))?;
        let frame_timer = xaml.queue.CreateTimer().map_err(e("DispatcherQueue.CreateTimer"))?;
        frame_timer.SetIsRepeating(false).map_err(e("DispatcherQueueTimer.IsRepeating"))?;
        frame_timer.SetInterval(windows_time::TimeSpan::from_millis(16)).map_err(e("DispatcherQueueTimer.Interval"))?;
        let subs = vec![
            timer.Tick(|_, _| on_timer()).map_err(e("DispatcherQueueTimer.Tick"))?,
            frame_timer.Tick(|_, _| on_frame()).map_err(e("DispatcherQueueTimer.Tick"))?,
        ];
        let wake = Arc::new(WakeShared { queue: xaml.queue.clone(), pending: AtomicBool::new(false) });
        let inner = Rc::new(Inner {
            xaml,
            host: Cell::new(std::ptr::null()),
            handles: RefCell::default(),
            by_ptr: RefCell::default(),
            next: Cell::new(1),
            timer,
            frame_timer,
            frame_pending: Cell::new(false),
            frame_sub: RefCell::new(None),
            timer_subs: RefCell::new(subs),
            wake,
            running: Cell::new(false),
            quit_early: Cell::new(false),
        });
        CURRENT.with(|c| *c.borrow_mut() = Rc::downgrade(&inner));
        Ok(WinUiBackend { inner })
    }

    /// Point events at `host` without running the loop (test harnesses). The host must outlive
    /// every later call, or be detached with a null pointer.
    pub fn attach_host(&self, host: *const svl_host) {
        self.inner.host.set(host);
    }

    /// Dispatch pending messages for up to `ms` milliseconds (test harnesses; `run` does this forever).
    pub fn pump(&self, ms: u64) {
        let end = std::time::Instant::now() + std::time::Duration::from_millis(ms);
        let mut msg = MSG::default();
        while std::time::Instant::now() < end {
            let got = unsafe { PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE as u32) }.as_bool();
            if got {
                self.dispatch(&msg);
            } else {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
    }

    fn dispatch(&self, msg: &MSG) {
        let handled = match self.inner.xaml.pretranslate {
            Some(f) => unsafe { f(msg) }.as_bool(),
            None => false,
        };
        if !handled {
            unsafe {
                let _ = TranslateMessage(msg);
                DispatchMessageW(msg);
            }
        }
    }

    /// Run `f` on the UI thread once, `ms` milliseconds from now (drivers and tests).
    pub fn after(&self, ms: u64, f: impl FnOnce() + 'static) -> Result<(), String> {
        let timer = self.inner.xaml.queue.CreateTimer().map_err(conv::err)?;
        timer.SetIsRepeating(false).map_err(conv::err)?;
        timer.SetInterval(windows_time::TimeSpan::from_millis(ms as i64)).map_err(conv::err)?;
        let id = AFTER_NEXT.with(|n| n.replace(n.get() + 1));
        let rev = timer
            .Tick(move |_, _| {
                let entry = AFTER.with(|a| a.borrow_mut().remove(&id));
                if let Some((t, rev, f)) = entry {
                    let _ = t.Stop();
                    if let Some(r) = rev {
                        GRAVEYARD.with(|g| g.borrow_mut().push(r.into()));
                    }
                    if let Some(f) = f {
                        f();
                    }
                }
            })
            .map_err(conv::err)?;
        timer.Start().map_err(conv::err)?;
        AFTER.with(|a| a.borrow_mut().insert(id, (timer, Some(rev), Some(Box::new(f)))));
        Ok(())
    }

    /// Click a button the way UI Automation does (`ButtonAutomationPeer.Invoke`).
    pub fn invoke(&self, h: u64) -> Result<(), String> {
        use crate::bindings::Microsoft::UI::Xaml::Automation::Peers::ButtonAutomationPeer;
        use crate::bindings::Microsoft::UI::Xaml::Controls::Button;
        let (o, _) = self.inner.get(h)?;
        let b = o.cast::<Button>().map_err(|_| format!("handle {h} is not a button"))?;
        let peer = ButtonAutomationPeer::CreateInstanceWithOwner(&b).map_err(conv::err)?;
        peer.Invoke().map_err(conv::err)
    }

    /// The native object behind a handle (tests and the example's driver).
    pub fn object(&self, h: u64) -> Option<IInspectable> {
        self.inner.handles.borrow().get(&h).map(|e| e.obj.clone())
    }

    /// Handles inserted into `parent` through slot `slot` (0 = any), in insertion order.
    pub fn children(&self, parent: u64, slot: u32) -> Vec<u64> {
        let handles = self.inner.handles.borrow();
        let Some(p) = handles.get(&parent) else { return Vec::new() };
        p.children.iter().copied().filter(|c| slot == 0 || handles.get(c).is_some_and(|e| e.slot == slot)).collect()
    }

    /// Native children of a slot, mapped back to handles (verification read-back).
    pub fn native_slot_items(&self, parent: u64, slot: u32) -> Result<Vec<Option<u64>>, String> {
        let (obj, _) = self.inner.get(parent)?;
        let s = table::slot(slot).ok_or("unknown slot")?;
        let _ = s;
        match &SLOT_OPS[slot as usize - 1] {
            SlotOps::Single { get, .. } => Ok(get(&obj).into_iter().map(|c| handle_of(&c)).collect()),
            SlotOps::Many { items } => {
                let coll = items(&obj).and_then(|c| c.cast::<IVector<UIElement>>()).map_err(conv::err)?;
                let n = coll.Size().map_err(conv::err)?;
                let mut out = Vec::new();
                for i in 0..n {
                    let c: IInspectable = coll.GetAt(i).map_err(conv::err)?.into();
                    out.push(handle_of(&c));
                }
                Ok(out)
            }
        }
    }

    fn listen_impl(&self, h: u64, ev: u32, on: bool, flags: u32) -> StrResult<()> {
        let inner = &self.inner;
        let (obj, kind) = inner.get(h)?;
        let e = table::event(ev).ok_or_else(|| format!("unknown event id {ev}"))?;
        if !table::applies(kind, e.class) {
            return Err(format!("<{}> has no event {:?}", table::kind(kind).map_or("?", |k| k.tag), e.name));
        }
        if flags != 0 {
            return Err(format!(
                "{:?}: WinUI events have no capture or after phase here (routed-event phases are not mapped yet)",
                e.name
            ));
        }
        if e.close {
            let mut handles = inner.handles.borrow_mut();
            let entry = handles.get_mut(&h).ok_or("stale handle")?;
            entry.close_listeners = if on { entry.close_listeners + 1 } else { entry.close_listeners.saturating_sub(1) };
            return Ok(());
        }
        let key = (ev, flags);
        if on {
            if inner.handles.borrow().get(&h).is_some_and(|e| e.listeners.contains_key(&key)) {
                return Ok(());
            }
            let rev = EVENT_CONNECT[ev as usize - 1](&obj, Sink { h, ev: ev | (flags << SVL_SIGNAL_FLAGS_SHIFT) }).map_err(conv::err)?;
            if let Some(entry) = inner.handles.borrow_mut().get_mut(&h) {
                entry.listeners.insert(key, rev);
            }
        } else {
            let rev = inner.handles.borrow_mut().get_mut(&h).and_then(|e| e.listeners.remove(&key));
            drop(rev);
        }
        Ok(())
    }

    fn prop_desc(&self, kind: u32, member: u32) -> Option<MemberDesc> {
        let p = table::prop(member)?;
        if !table::applies(kind, p.class) {
            return None;
        }
        let slot = match p.ty {
            Ty::Bool => SlotDesc::new(SVL_BOOL),
            Ty::NullableBool => SlotDesc::new(SVL_BOOL).flags(SVL_D_NULLABLE),
            Ty::I32 => SlotDesc::new(SVL_I64).flags(SVL_D_INTEGRAL).range(i32::MIN as f64, i32::MAX as f64),
            Ty::F64 => SlotDesc::new(SVL_F64),
            Ty::Str | Ty::Enum(_) | Ty::Custom => SlotDesc::new(SVL_STR),
            Ty::ObjectString => SlotDesc::new(SVL_STR).flags(SVL_D_NULLABLE),
            // A number, a string form or a record: converted by the backend.
            Ty::Thickness => return None,
        }
        .named(p.name);
        Some(MemberDesc {
            name: p.name.to_string(),
            params: if p.write { vec![slot.clone()] } else { vec![] },
            ret: if p.read || p.ty == Ty::Custom { slot } else { SlotDesc::new(SVL_NULL) },
            flags: 0,
        })
    }
}

impl WinUiBackend {
    /// Tears WinUI down on this thread: closes the windows we own, releases every XAML object and
    /// event registration the backend holds, then closes the Application and the XAML manager and
    /// shuts the dispatcher queue down (the islands order, `Xaml::shutdown`). Call it after `run`
    /// returns and before the process exits: exiting with live XAML windows and a running XAML
    /// core fail-fasts (0xE0464645, CI run 1 of FW1). Idempotent; the backend is unusable after.
    pub fn shutdown(&self) {
        let inner = &self.inner;
        if inner.xaml.down.get() {
            return;
        }
        let entries: Vec<Entry> = inner.handles.borrow_mut().drain().map(|(_, e)| e).collect();
        inner.by_ptr.borrow_mut().clear();
        let mut wins = Vec::new();
        let mut regs = Vec::new();
        for e in entries {
            let top = table::kind(e.kind).is_some_and(|k| k.toplevel);
            regs.extend(e.listeners.into_values());
            regs.extend(e.internal);
            if top {
                wins.push(e.obj);
            }
        }
        // Unsubscribe first, so the close rule's handler does not run against a drained table.
        drop(regs);
        for w in wins {
            if let Ok(w) = w.cast::<Window>() {
                let _ = w.Close();
            }
        }
        let _ = inner.timer.Stop();
        let _ = inner.frame_timer.Stop();
        inner.frame_sub.borrow_mut().take();
        inner.timer_subs.borrow_mut().clear();
        let after: Vec<_> = AFTER.with(|a| a.borrow_mut().drain().map(|(_, v)| v).collect());
        for (t, rev, f) in after {
            let _ = t.Stop();
            drop((rev, f));
        }
        GRAVEYARD.with(|g| g.borrow_mut().clear());
        // Let the closes and releases above run through the queue before it shuts down.
        self.pump(50);
        inner.xaml.shutdown();
    }
}

impl Drop for WinUiBackend {
    fn drop(&mut self) {
        if Rc::strong_count(&self.inner) > 1 {
            return;
        }
        self.shutdown();
        CURRENT.with(|c| *c.borrow_mut() = Weak::new());
    }
}

impl Backend for WinUiBackend {
    fn resolve(&self, what: NameKind, kind: svl_id, name: &str) -> svl_id {
        match what {
            NameKind::Kind => table::kind_by_tag(name),
            NameKind::Prop => table::prop_id(kind, name),
            NameKind::Signal => table::event_id(kind, name),
            NameKind::Method => table::method_id(kind, name),
            NameKind::Slot => table::slot_id(kind, name),
            NameKind::Attr => table::attr_id(kind, name),
        }
    }

    fn create(&self, kind: svl_id) -> StrResult<svl_handle> {
        let k = table::kind(kind).ok_or_else(|| format!("unknown kind id {kind}"))?;
        let obj = CREATE[kind as usize - 1]().map_err(|e| format!("create <{}> ({}): {}", k.tag, k.native, conv::err(e)))?;
        let inner = &self.inner;
        let h = inner.next.get();
        inner.next.set(h + 1);
        let mut internal = Vec::new();
        if k.toplevel {
            for (i, ev) in EVENTS.iter().enumerate() {
                if ev.close && table::applies(kind, ev.class) {
                    internal.push(EVENT_CONNECT[i](&obj, Sink { h, ev: i as u32 + 1 }).map_err(conv::err)?);
                }
            }
        }
        inner.by_ptr.borrow_mut().insert(identity(&obj), h);
        inner.handles.borrow_mut().insert(
            h,
            Entry { obj, kind, parent: None, slot: 0, children: Vec::new(), listeners: HashMap::new(), close_listeners: 0, internal },
        );
        Ok(h)
    }

    fn insert(&self, parent: svl_handle, child: svl_handle, before: Option<svl_handle>, slot: svl_id) -> StrResult<()> {
        let inner = &self.inner;
        let (pobj, pkind) = inner.get(parent)?;
        let (cobj, ckind) = inner.get(child)?;
        if parent == child {
            return Err("cannot insert an element into itself".into());
        }
        let ctop = table::kind(ckind).is_some_and(|k| k.toplevel);
        if ctop {
            // X7: a toplevel under another one. WinUI windows have no XAML-level owner; keep the
            // edge for lifetime (the child dies with the parent window) only.
            let mut handles = inner.handles.borrow_mut();
            if handles.get(&child).is_some_and(|e| e.parent.is_some()) {
                return Err(format!("window {child} is already inserted"));
            }
            handles.get_mut(&child).unwrap().parent = Some(parent);
            handles.get_mut(&parent).unwrap().children.push(child);
            return Ok(());
        }
        let slot = inner.default_slot(pkind, slot)?;
        // A child already in this slot is moved (keyed reorder); elsewhere is an error.
        let current = inner.handles.borrow().get(&child).and_then(|e| e.parent.map(|p| (p, e.slot)));
        match current {
            Some((p, s)) if p == parent && s == slot => self.remove(parent, child)?,
            Some((p, _)) => return Err(format!("handle {child} is already inserted into {p}; remove it first")),
            None => {}
        }
        let celem = cobj.cast::<UIElement>().map_err(|_| format!("handle {child} is not a UIElement"))?;
        match &SLOT_OPS[slot as usize - 1] {
            SlotOps::Single { set, .. } => {
                if before.is_some() {
                    return Err(format!("slot {:?} holds one child: `before` does not apply", table::slot(slot).unwrap().name));
                }
                if let Some(other) = single_occupant(&inner.handles.borrow(), parent, slot) {
                    return Err(format!("slot {:?} is occupied by {other}; remove it first", table::slot(slot).unwrap().name));
                }
                set(&pobj, Some(&cobj)).map_err(conv::err)?;
            }
            SlotOps::Many { items } => {
                let coll = items(&pobj).and_then(|c| c.cast::<IVector<UIElement>>()).map_err(conv::err)?;
                match before {
                    Some(b) => {
                        let (bobj, _) = inner.get(b)?;
                        let belem = bobj.cast::<UIElement>().map_err(conv::err)?;
                        let mut idx = 0u32;
                        if !coll.IndexOf(&belem, &mut idx).map_err(conv::err)? {
                            return Err(format!("`before` {b} is not a child of {parent}"));
                        }
                        coll.InsertAt(idx, &celem).map_err(conv::err)?;
                    }
                    None => coll.Append(&celem).map_err(conv::err)?,
                }
            }
        }
        let mut handles = inner.handles.borrow_mut();
        let c = handles.get_mut(&child).unwrap();
        c.parent = Some(parent);
        c.slot = slot;
        let p = handles.get_mut(&parent).unwrap();
        let at = before.and_then(|b| p.children.iter().position(|x| *x == b)).unwrap_or(p.children.len());
        p.children.insert(at, child);
        Ok(())
    }

    fn remove(&self, parent: svl_handle, child: svl_handle) -> StrResult<()> {
        let inner = &self.inner;
        let (pobj, _) = inner.get(parent)?;
        let (cobj, ckind) = inner.get(child)?;
        let slot = match inner.handles.borrow().get(&child) {
            Some(e) if e.parent == Some(parent) => e.slot,
            _ => return Err(format!("handle {child} is not a child of {parent}")),
        };
        if !table::kind(ckind).is_some_and(|k| k.toplevel) {
            match &SLOT_OPS[slot as usize - 1] {
                SlotOps::Single { set, .. } => set(&pobj, None).map_err(conv::err)?,
                SlotOps::Many { items } => {
                    let coll = items(&pobj).and_then(|c| c.cast::<IVector<UIElement>>()).map_err(conv::err)?;
                    let elem = cobj.cast::<UIElement>().map_err(conv::err)?;
                    let mut idx = 0u32;
                    if coll.IndexOf(&elem, &mut idx).map_err(conv::err)? {
                        coll.RemoveAt(idx).map_err(conv::err)?;
                    }
                }
            }
        }
        let mut handles = inner.handles.borrow_mut();
        if let Some(c) = handles.get_mut(&child) {
            c.parent = None;
            c.slot = 0;
        }
        if let Some(p) = handles.get_mut(&parent) {
            p.children.retain(|x| *x != child);
        }
        Ok(())
    }

    fn destroy(&self, h: svl_handle) {
        let inner = &self.inner;
        let Some(parent) = inner.handles.borrow().get(&h).map(|e| e.parent) else { return };
        if let Some(p) = parent {
            let _ = self.remove(p, h);
        }
        let Some(e) = inner.handles.borrow_mut().remove(&h) else { return };
        inner.by_ptr.borrow_mut().remove(&identity(&e.obj));
        {
            let mut handles = inner.handles.borrow_mut();
            for c in &e.children {
                if let Some(ce) = handles.get_mut(c) {
                    ce.parent = None;
                    ce.slot = 0;
                }
            }
        }
        let top = table::kind(e.kind).is_some_and(|k| k.toplevel);
        let Entry { obj, listeners, internal, .. } = e;
        drop(listeners);
        if top {
            // The entry is gone, so the Closed handler neither vetoes nor reports this close.
            if let Ok(w) = obj.cast::<Window>() {
                let _ = w.Close();
            }
        }
        GRAVEYARD.with(|g| g.borrow_mut().extend(internal));
        inner.enqueue_wake_noop();
    }

    fn set_prop(&self, h: svl_handle, prop: svl_id, v: &Value) -> StrResult<()> {
        let (obj, kind) = self.inner.get(h)?;
        let p = table::prop(prop).ok_or_else(|| format!("unknown prop id {prop}"))?;
        if !table::applies(kind, p.class) {
            return Err(format!("<{}> has no prop {:?}", table::kind(kind).map_or("?", |k| k.tag), p.name));
        }
        let set = PROP_SET[prop as usize - 1].ok_or_else(|| format!("{:?} is read-only", p.name))?;
        set(&obj, v).map_err(|e| format!("{}: {e}", p.name))
    }

    fn get_prop(&self, h: svl_handle, prop: svl_id) -> StrResult<Value> {
        let (obj, kind) = self.inner.get(h)?;
        let p = table::prop(prop).ok_or_else(|| format!("unknown prop id {prop}"))?;
        if !table::applies(kind, p.class) {
            return Err(format!("<{}> has no prop {:?}", table::kind(kind).map_or("?", |k| k.tag), p.name));
        }
        let get = PROP_GET[prop as usize - 1].ok_or_else(|| format!("{:?} is write-only", p.name))?;
        get(&obj).map_err(|e| format!("{}: {e}", p.name))
    }

    fn call(&self, h: svl_handle, method: svl_id, args: &[Value]) -> StrResult<Value> {
        let (obj, kind) = self.inner.get(h)?;
        let m = table::method(method).ok_or_else(|| format!("unknown method id {method}"))?;
        if !table::applies(kind, m.class) {
            return Err(format!("<{}> has no method {:?}", table::kind(kind).map_or("?", |k| k.tag), m.name));
        }
        METHOD_CALL[method as usize - 1](&obj, args).map_err(|e| format!("{}(): {e}", m.name))
    }

    fn listen(&self, h: svl_handle, signal: svl_id, on: bool) -> StrResult<()> {
        self.listen_impl(h, signal, on, 0)
    }

    fn listen_ex(&self, h: svl_handle, signal: svl_id, on: bool, flags: u32) -> StrResult<()> {
        self.listen_impl(h, signal, on, flags)
    }

    fn handle_info(&self, h: svl_handle) -> StrResult<svl_handle_info> {
        let (_, kind) = self.inner.get(h)?;
        let top = table::kind(kind).is_some_and(|k| k.toplevel);
        Ok(svl_handle_info { kind, lifetime: if top { SVL_LIFE_TOPLEVEL } else { SVL_LIFE_OWNED }, flags: 0 })
    }

    fn run(&self, host: &HostRef<'_>) -> StrResult<()> {
        let inner = &self.inner;
        inner.host.set(host.as_raw());
        inner.running.set(true);
        if inner.wake.pending.load(Ordering::SeqCst) {
            on_wake();
        }
        let mut result = Ok(());
        if !inner.quit_early.replace(false) {
            let mut msg = MSG::default();
            loop {
                let r = unsafe { GetMessageW(&mut msg, None, 0, 0) }.0;
                if r == 0 {
                    break;
                }
                if r == -1 {
                    result = Err("GetMessageW failed".to_string());
                    break;
                }
                self.dispatch(&msg);
            }
        }
        inner.running.set(false);
        inner.host.set(std::ptr::null());
        result
    }

    fn waker(&self) -> Waker {
        let shared = self.inner.wake.clone();
        Arc::new(move || {
            if !shared.pending.swap(true, Ordering::SeqCst) {
                let _ = shared.queue.TryEnqueue(&DispatcherQueueHandler::new(|| {
                    on_wake();
                    Ok(())
                }));
            }
        })
    }

    fn quit(&self) {
        if self.inner.running.get() {
            unsafe { PostQuitMessage(0) };
        } else {
            self.inner.quit_early.set(true);
        }
    }

    fn schedule_wake(&self, ms: i64) {
        let t = &self.inner.timer;
        let _ = t.Stop();
        if ms >= 0 {
            let _ = t.SetInterval(windows_time::TimeSpan::from_millis(ms));
            let _ = t.Start();
        }
    }

    fn request_frame(&self) {
        let inner = &self.inner;
        if inner.frame_pending.replace(true) {
            return;
        }
        if !inner.any_toplevel() {
            let _ = inner.frame_timer.Start();
            return;
        }
        if inner.frame_sub.borrow().is_none() {
            match CompositionTarget::Rendering(|_, _| on_frame()) {
                Ok(r) => *inner.frame_sub.borrow_mut() = Some(r),
                Err(_) => {
                    let _ = inner.frame_timer.Start();
                }
            }
        }
    }

    fn set_child_attr(&self, parent: svl_handle, child: svl_handle, attr: svl_id, v: &Value) -> StrResult<()> {
        let inner = &self.inner;
        let (_, pkind) = inner.get(parent)?;
        let (cobj, _) = inner.get(child)?;
        let a = table::attr(attr).ok_or_else(|| format!("unknown attribute id {attr}"))?;
        if !table::applies(pkind, a.owner) {
            return Err(format!("<{}> has no child attribute {:?}", table::kind(pkind).map_or("?", |k| k.tag), a.name));
        }
        match inner.handles.borrow().get(&child) {
            Some(e) if e.parent == Some(parent) && e.slot == a.slot => {}
            _ => return Err(format!("handle {child} is not in the {:?} slot of {parent}", table::slot(a.slot).map_or("?", |s| s.name))),
        }
        let v = if matches!(v, Value::Null) { Value::I64(a.default) } else { v.clone() };
        ATTR_SET[attr as usize - 1](&cobj, &v).map_err(|e| format!("{}: {e}", a.name))
    }

    fn get_child_attr(&self, parent: svl_handle, child: svl_handle, attr: svl_id) -> StrResult<Value> {
        let inner = &self.inner;
        let (_, pkind) = inner.get(parent)?;
        let (cobj, _) = inner.get(child)?;
        let a = table::attr(attr).ok_or_else(|| format!("unknown attribute id {attr}"))?;
        if !table::applies(pkind, a.owner) {
            return Err(format!("<{}> has no child attribute {:?}", table::kind(pkind).map_or("?", |k| k.tag), a.name));
        }
        if inner.handles.borrow().get(&child).is_none_or(|e| e.parent != Some(parent)) {
            return Err(format!("handle {child} is not a child of {parent}"));
        }
        ATTR_GET[attr as usize - 1](&cobj).map_err(|e| format!("{}: {e}", a.name))
    }

    fn describe(&self, what: NameKind, kind: svl_id, member: svl_id) -> Option<MemberDesc> {
        match what {
            NameKind::Prop => self.prop_desc(kind, member),
            _ => None,
        }
    }
}
