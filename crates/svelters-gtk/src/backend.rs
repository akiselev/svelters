//! `svl_backend` over GTK4 (see plans/architecture/backend-abi.md).
//!
//! Lifetime rules (docs/notes/gtk-v1-archaeology.md): the handle table holds one strong reference
//! per live handle, released only by `destroy`, so a removed-but-not-destroyed widget can be
//! re-inserted. Signal handlers capture a `Weak<Inner>` and are disconnected before the table
//! drops its reference, so a handler can never run against a freed entry. Natively destroyed
//! toplevels are reported through `host.on_destroyed`.

use crate::convert::{self, StrBuf};
use crate::generated::{slots, tables::*};
use crate::lookup;
use gtk::glib;
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::rc::Rc;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::Arc;
use svelters_abi::*;

struct Entry {
    /// Weak: toplevels are kept alive by GTK itself, so natively closing one really destroys it.
    obj: glib::WeakRef<glib::Object>,
    /// Strong reference for everything but toplevels; released only by `destroy`.
    hold: Option<glib::Object>,
    ptr: usize,
    kind: u32,
    signals: HashMap<u32, glib::SignalHandlerId>,
    /// (parent handle, slot id) while inserted through a slot.
    slot: Option<(u64, u32)>,
}

#[derive(Default)]
struct State {
    next: u64,
    entries: HashMap<u64, Entry>,
    by_obj: HashMap<usize, u64>,
}

pub(crate) struct Inner {
    app: gtk::Application,
    host: Arc<AtomicPtr<svl_host>>,
    state: RefCell<State>,
    error: RefCell<CString>,
    out: RefCell<StrBuf>,
    /// The pending `schedule_wake` source, if any. Cleared by the source's own callback.
    wake_timer: RefCell<Option<glib::SourceId>>,
    /// `request_frame` was called and `host.frame` has not been delivered yet.
    frame_pending: Cell<bool>,
    /// The frame clock the `update` handler is connected to.
    frame_clock: RefCell<Option<(gtk::gdk::FrameClock, glib::SignalHandlerId)>>,
    /// Fallback for a frame the clock does not deliver (no mapped toplevel, or a hidden one).
    frame_timer: RefCell<Option<glib::SourceId>>,
    /// Frames delivered by the frame clock (the rest came from the fallback timer).
    frames_by_clock: Cell<u32>,
}

/// Slot value recorded for an event controller attached to a widget (it is not in `SLOTS`).
const CONTROLLER_SLOT: u32 = u32::MAX;
/// Frame delay used when there is no mapped toplevel to tick on.
const FRAME_FALLBACK_MS: u64 = 16;
/// A mapped toplevel whose clock stalls (hidden, minimised) still gets a frame after this long.
const FRAME_WATCHDOG_MS: u64 = 250;

type Res<T> = Result<T, String>;

impl Inner {
    fn obj_of(&self, h: u64) -> Res<(glib::Object, u32)> {
        let st = self.state.borrow();
        let e = st.entries.get(&h).ok_or_else(|| format!("stale or foreign handle {h}"))?;
        Ok((e.obj.upgrade().ok_or_else(|| format!("handle {h} was destroyed"))?, e.kind))
    }

    fn widget_of(&self, h: u64) -> Res<(gtk::Widget, u32)> {
        let (o, k) = self.obj_of(h)?;
        let w = o.downcast::<gtk::Widget>().map_err(|_| format!("handle {h} is not a widget"))?;
        Ok((w, k))
    }

    fn handle_of(&self, o: &glib::Object) -> u64 {
        self.state.borrow().by_obj.get(&(o.as_ptr() as usize)).copied().unwrap_or(0)
    }

    fn set_error(&self, msg: String) {
        *self.error.borrow_mut() = CString::new(msg.replace('\0', " ")).unwrap_or_default();
    }

    fn create(self: &Rc<Self>, kind: u32) -> Res<u64> {
        let k = KINDS.get((kind as usize).wrapping_sub(1)).ok_or("unknown kind")?;
        if k.is_abstract || k.is_interface {
            return Err(format!("{} is not instantiable", k.gtype));
        }
        crate::generated::types::ensure_registered(k.gtype);
        let t = glib::Type::from_name(k.gtype).ok_or_else(|| format!("GType {} is not registered", k.gtype))?;
        if !t.is_a(glib::Type::OBJECT) {
            return Err(format!("{} is not instantiable", k.gtype));
        }
        let obj = glib::Object::with_type(t);
        if let Some(ps) = obj.find_property("application") {
            if ps.flags().contains(glib::ParamFlags::WRITABLE) && ps.value_type() == gtk::Application::static_type() {
                obj.set_property_from_value("application", &self.app.to_value());
            }
        }
        let h = {
            let mut st = self.state.borrow_mut();
            st.next += 1;
            let h = st.next;
            st.by_obj.insert(obj.as_ptr() as usize, h);
            let hold = if obj.is::<gtk::Window>() { None } else { Some(obj.clone()) };
            st.entries.insert(h, Entry { obj: obj.downgrade(), hold, ptr: obj.as_ptr() as usize, kind, signals: HashMap::new(), slot: None });
            h
        };
        if let Some(win) = obj.downcast_ref::<gtk::Window>() {
            let weak = Rc::downgrade(self);
            win.connect_destroy(move |win| {
                if let Some(inner) = weak.upgrade() {
                    inner.native_destroyed(h, win.upcast_ref());
                }
            });
        }
        Ok(h)
    }

    /// A toplevel was destroyed by GTK (close, `gtk_window_destroy`). Its descendants are
    /// invalidated with it: GTK leaves them alive but orphaned, and the handle table would
    /// otherwise keep reporting them live. The window is reported first, then its descendants in
    /// pre-order. Descendants come from the slot records (GTK has already unparented the children
    /// by the time `destroy` is emitted), plus anything still parented below the window.
    fn native_destroyed(&self, h: u64, win: &gtk::Widget) {
        let gone: Vec<(u64, Entry)> = {
            let mut st = self.state.borrow_mut();
            let mut order: Vec<u64> = vec![h];
            let mut i = 0;
            while i < order.len() {
                let parent = order[i];
                i += 1;
                let mut kids: Vec<u64> = st.entries.iter().filter(|(k, e)| e.slot.is_some_and(|(p, _)| p == parent) && !order.contains(k)).map(|(k, _)| *k).collect();
                kids.sort_unstable();
                // pre-order: a child's own descendants follow it directly
                for (n, k) in kids.into_iter().enumerate() {
                    order.insert(i + n, k);
                }
            }
            let mut orphans: Vec<u64> = st
                .entries
                .iter()
                .filter(|(k, e)| {
                    !order.contains(k) && e.obj.upgrade().and_then(|o| o.downcast::<gtk::Widget>().ok()).is_some_and(|w| w.is_ancestor(win))
                })
                .map(|(k, _)| *k)
                .collect();
            orphans.sort_unstable();
            order.extend(orphans);
            let v: Vec<(u64, Entry)> = order.into_iter().filter_map(|k| st.entries.remove(&k).map(|e| (k, e))).collect();
            for (_, e) in &v {
                st.by_obj.remove(&e.ptr);
            }
            v
        };
        let host = self.host.load(Ordering::Acquire);
        for (k, e) in gone {
            // The window itself is being disposed: its handlers go with it.
            if k != h {
                if let Some(obj) = e.obj.upgrade() {
                    for (_, id) in e.signals {
                        obj.disconnect(id);
                    }
                }
            }
            drop(e.hold);
            if !host.is_null() {
                let reason = if k == h { SVL_GONE_CLOSED } else { SVL_GONE_PARENT };
                unsafe { ((*host).on_destroyed)((*host).self_, k, reason) };
            }
        }
    }

    /// Idempotent: the handle is stale as soon as this returns, and a second call (or a call on a
    /// handle that already died) does nothing. Nothing is finalized synchronously, because the
    /// caller may be inside a signal emission: a toplevel is hidden now and destroyed from an
    /// idle, and the table's strong reference is dropped from an idle.
    fn destroy(&self, h: u64) -> Res<()> {
        let entry = {
            let mut st = self.state.borrow_mut();
            let Some(e) = st.entries.remove(&h) else { return Ok(()) };
            st.by_obj.remove(&e.ptr);
            e
        };
        if let Some(obj) = entry.obj.upgrade() {
            for (_, id) in entry.signals {
                obj.disconnect(id);
            }
            if let Some(win) = obj.downcast_ref::<gtk::Window>() {
                win.set_visible(false);
                let win = win.clone();
                glib::idle_add_local_once(move || win.destroy());
            }
        }
        if let Some(hold) = entry.hold {
            glib::idle_add_local_once(move || drop(hold));
        }
        Ok(())
    }

    /// Event controllers are not widgets: they attach to a widget with `add_controller`.
    fn insert_controller(&self, parent: u64, child: u64, ctrl: &gtk::EventController) -> Res<()> {
        let (pw, _) = self.widget_of(parent)?;
        if ctrl.widget().is_some() {
            return Err("controller is already attached".into());
        }
        pw.add_controller(ctrl.clone());
        if let Some(e) = self.state.borrow_mut().entries.get_mut(&child) {
            e.slot = Some((parent, CONTROLLER_SLOT));
        }
        Ok(())
    }

    fn insert(&self, parent: u64, child: u64, before: u64, slot: u32) -> Res<()> {
        if let Ok((co, _)) = self.obj_of(child) {
            if let Some(ctrl) = co.downcast_ref::<gtk::EventController>() {
                return self.insert_controller(parent, child, ctrl);
            }
        }
        let (pw, pk) = self.widget_of(parent)?;
        let (cw, _) = self.widget_of(child)?;
        let slot = if slot == 0 { lookup::default_slot(pk) } else { slot };
        if slot == 0 {
            return Err("parent has no default slot".into());
        }
        let si = SLOTS.get(slot as usize - 1).ok_or("unknown slot")?;
        if !lookup::in_chain(pk, si.kind) {
            return Err(format!("slot {} does not belong to {}", si.name, KINDS[pk as usize - 1].gtype));
        }
        let bw = if before != 0 { Some(self.widget_of(before)?.0) } else { None };
        if let Some(b) = &bw {
            if !si.ordered {
                return Err(format!("slot {} does not support insert-before", si.name));
            }
            if b.parent().as_ref() != Some(&pw) {
                return Err("`before` is not a child of the parent".into());
            }
        }
        if cw.parent().is_some() {
            return Err("child already has a parent".into());
        }
        if !si.many {
            // A replaced child is released by GTK (unparented, not destroyed): forget its slot.
            if let Some(old) = slots::slot_get(slot, &pw)? {
                let oh = self.handle_of(old.upcast_ref());
                if let Some(e) = self.state.borrow_mut().entries.get_mut(&oh) {
                    e.slot = None;
                }
            }
        }
        slots::slot_insert(slot, &pw, &cw, bw.as_ref())?;
        if let Some(e) = self.state.borrow_mut().entries.get_mut(&child) {
            e.slot = Some((parent, slot));
        }
        Ok(())
    }

    fn remove(&self, parent: u64, child: u64) -> Res<()> {
        let (pw, _) = self.widget_of(parent)?;
        if let Ok((co, _)) = self.obj_of(child) {
            if let Some(ctrl) = co.downcast_ref::<gtk::EventController>() {
                let attached = self.state.borrow().entries.get(&child).and_then(|e| e.slot) == Some((parent, CONTROLLER_SLOT));
                if !attached {
                    return Err(format!("handle {child} is not inserted in {parent}"));
                }
                pw.remove_controller(ctrl);
                if let Some(e) = self.state.borrow_mut().entries.get_mut(&child) {
                    e.slot = None;
                }
                return Ok(());
            }
        }
        let (cw, _) = self.widget_of(child)?;
        let slot = {
            let st = self.state.borrow();
            match st.entries.get(&child).and_then(|e| e.slot) {
                Some((p, s)) if p == parent => s,
                _ => return Err(format!("handle {child} is not inserted in {parent}")),
            }
        };
        slots::slot_remove(slot, &pw, &cw)?;
        if let Some(e) = self.state.borrow_mut().entries.get_mut(&child) {
            e.slot = None;
        }
        Ok(())
    }

    fn set_prop(&self, h: u64, prop: u32, v: &svl_value) -> Res<()> {
        let (obj, kind) = self.obj_of(h)?;
        let p = PROPS.get((prop as usize).wrapping_sub(1)).ok_or("unknown property")?;
        if !lookup::in_chain(kind, p.kind) {
            return Err(format!("property {} does not belong to {}", p.name, KINDS[kind as usize - 1].gtype));
        }
        if p.flags & F_WRITE == 0 {
            return Err(format!("property {} is read-only", p.name));
        }
        if p.flags & F_VIRTUAL != 0 {
            let text = match v.tag {
                SVL_NULL => None,
                _ => Some(unsafe { convert::as_str(v) }.ok_or("expected string")?),
            };
            return slots::virtual_set(prop, &obj, text);
        }
        let pspec = obj.find_property(p.name).ok_or_else(|| format!("{} has no property {}", obj.type_().name(), p.name))?;
        let value = unsafe { convert::to_value(p.ty, &pspec, v, &|h| self.obj_of(h).ok().map(|x| x.0)) }
            .map_err(|e| format!("{}: {e}", p.name))?;
        // Echo suppression: handlers for the signals this property would raise are blocked while JS writes it.
        let st = self.state.borrow();
        let blocked: Vec<&glib::SignalHandlerId> = st
            .entries
            .get(&h)
            .map(|e| {
                e.signals
                    .iter()
                    .filter(|(sid, _)| p.echo.contains(&SIGNALS[**sid as usize - 1].name))
                    .map(|(_, id)| id)
                    .collect()
            })
            .unwrap_or_default();
        for id in &blocked {
            obj.block_signal(id);
        }
        obj.set_property_from_value(p.name, &value);
        for id in &blocked {
            obj.unblock_signal(id);
        }
        Ok(())
    }

    fn get_prop(&self, h: u64, prop: u32, out: &mut svl_value) -> Res<()> {
        let (obj, kind) = self.obj_of(h)?;
        let p = PROPS.get((prop as usize).wrapping_sub(1)).ok_or("unknown property")?;
        if !lookup::in_chain(kind, p.kind) {
            return Err(format!("property {} does not belong to {}", p.name, KINDS[kind as usize - 1].gtype));
        }
        if p.flags & F_READ == 0 {
            return Err(format!("property {} is write-only", p.name));
        }
        let mut buf = self.out.borrow_mut();
        *buf = StrBuf::default();
        if p.flags & F_VIRTUAL != 0 {
            *out = match slots::virtual_get(prop, &obj)? {
                Some(s) => buf.str_value(&s),
                None => convert::null_value(),
            };
            return Ok(());
        }
        if obj.find_property(p.name).is_none() {
            return Err(format!("{} has no property {}", obj.type_().name(), p.name));
        }
        let value = obj.property_value(p.name);
        *out = convert::from_value(&value, &|o| self.handle_of(o), &mut buf);
        Ok(())
    }

    fn call(&self, h: u64, method: u32, n: usize, out: &mut svl_value) -> Res<()> {
        let (obj, kind) = self.obj_of(h)?;
        let m = METHODS.get((method as usize).wrapping_sub(1)).ok_or("unknown method")?;
        if !lookup::in_chain(kind, m.kind) {
            return Err(format!("method {} does not belong to {}", m.name, KINDS[kind as usize - 1].gtype));
        }
        if n != 0 {
            return Err(format!("method {} takes no arguments", m.name));
        }
        slots::call_method(method, &obj)?;
        *out = convert::null_value();
        Ok(())
    }

    fn listen(self: &Rc<Self>, h: u64, signal: u32, on: bool) -> Res<()> {
        let (obj, kind) = self.obj_of(h)?;
        let sig = SIGNALS.get((signal as usize).wrapping_sub(1)).ok_or("unknown signal")?;
        if !lookup::in_chain(kind, sig.kind) {
            return Err(format!("signal {} does not belong to {}", sig.name, KINDS[kind as usize - 1].gtype));
        }
        let connected = self.state.borrow().entries[&h].signals.contains_key(&signal);
        if on && !connected {
            let weak = Rc::downgrade(self);
            let closure = glib::RustClosure::new_local(move |vals| weak.upgrade().and_then(|i| i.deliver(h, signal, vals)));
            let id = obj.connect_closure(sig.name, false, closure);
            self.state.borrow_mut().entries.get_mut(&h).unwrap().signals.insert(signal, id);
        } else if !on && connected {
            let id = self.state.borrow_mut().entries.get_mut(&h).unwrap().signals.remove(&signal).unwrap();
            obj.disconnect(id);
        }
        Ok(())
    }

    /// Native signal -> the host. Returns the value the signal's accumulator expects.
    ///
    /// A signal with a handler result (`handler_result != none` or `sync_required` in the generated
    /// table) goes through `host.call_sync`: the JS listeners run now and decide the result
    /// (`true` = stop, e.g. veto the close or mark the key handled). Other signals are posted and
    /// delivered at the next checkpoint. When the host answers busy (JS is running; it queued the
    /// event itself) or fails, the signal gets its deferred default: do not stop, except
    /// `close-request`, which vetoes so that the late handler can still decide (ABI v2 behaviour).
    fn deliver(&self, h: u64, signal: u32, vals: &[glib::Value]) -> Option<glib::Value> {
        let sig = &SIGNALS[signal as usize - 1];
        let sync = sig.handler_result != 0 || sig.sync_required;
        let host = self.host.load(Ordering::Acquire);
        let mut sync_result: Option<svl_value> = None;
        let mut deferred = false;
        if !host.is_null() {
            let mut buf = StrBuf::default();
            let mut args: Vec<svl_value> = vals.iter().skip(1).map(|v| convert::from_value(v, &|o| self.handle_of(o), &mut buf)).collect();
            // GtkEditable::changed has no arguments; the JS contract (and the Qt backend) delivers `[text]`.
            if sig.name == "changed" && args.is_empty() {
                if let Some(e) = vals.first().and_then(|v| v.get::<glib::Object>().ok()).and_then(|o| o.downcast::<gtk::Editable>().ok()) {
                    args.push(buf.str_value(&e.text()));
                }
            }
            if sync {
                let mut out = convert::null_value();
                let rc = unsafe { ((*host).call_sync)((*host).self_, h, signal, args.as_ptr(), args.len(), &mut out) };
                match rc {
                    SVL_SYNC_OK => sync_result = Some(out),
                    SVL_SYNC_BUSY => deferred = true,
                    _ => {}
                }
            } else {
                unsafe { ((*host).post_event)((*host).self_, h, signal, args.as_ptr(), args.len()) };
            }
        }
        if !sync {
            return None;
        }
        match sig.result {
            "bool" => {
                let stop = match &sync_result {
                    Some(v) => match v.tag {
                        SVL_BOOL => unsafe { v.u.b },
                        SVL_I64 => unsafe { v.u.i != 0 },
                        _ => false,
                    },
                    None => deferred && sig.name == "close-request",
                };
                Some(stop.to_value())
            }
            "i32" => match &sync_result {
                Some(v) if v.tag == SVL_I64 => Some((unsafe { v.u.i } as i32).to_value()),
                _ => None,
            },
            "string" => match &sync_result {
                Some(v) if v.tag == SVL_STR => unsafe { convert::as_str(v) }.map(|s| s.to_value()),
                _ => None,
            },
            _ => None,
        }
    }

    // ---- frame clock ----

    /// The frame clock of the lowest-handle mapped toplevel, if there is one.
    fn tick_clock(&self) -> Option<gtk::gdk::FrameClock> {
        let wins: Vec<(u64, gtk::Window)> = {
            let st = self.state.borrow();
            st.entries.iter().filter_map(|(h, e)| e.obj.upgrade().and_then(|o| o.downcast::<gtk::Window>().ok()).map(|w| (*h, w))).collect()
        };
        wins.into_iter().filter(|(_, w)| w.is_mapped()).min_by_key(|(h, _)| *h).and_then(|(_, w)| w.frame_clock())
    }

    fn request_frame(self: &Rc<Self>) {
        if self.frame_pending.replace(true) {
            return;
        }
        let weak = Rc::downgrade(self);
        let mut delay = FRAME_FALLBACK_MS;
        if let Some(clock) = self.tick_clock() {
            let same = self.frame_clock.borrow().as_ref().is_some_and(|(c, _)| c == &clock);
            if !same {
                if let Some((old, id)) = self.frame_clock.borrow_mut().take() {
                    old.disconnect(id);
                }
                let w = weak.clone();
                let id = clock.connect_update(move |_| {
                    if let Some(i) = w.upgrade() {
                        i.frame_due(true);
                    }
                });
                *self.frame_clock.borrow_mut() = Some((clock.clone(), id));
            }
            clock.request_phase(gtk::gdk::FrameClockPhase::UPDATE);
            delay = FRAME_WATCHDOG_MS;
        }
        let id = glib::timeout_add_local_once(std::time::Duration::from_millis(delay), move || {
            if let Some(i) = weak.upgrade() {
                // The source is finished once this runs: forget its id before anything removes it.
                i.frame_timer.borrow_mut().take();
                i.frame_due(false);
            }
        });
        *self.frame_timer.borrow_mut() = Some(id);
    }

    /// The clock ticked (or the fallback timer fired): hand the frame to the host.
    fn frame_due(&self, from_clock: bool) {
        if !self.frame_pending.replace(false) {
            return;
        }
        if from_clock {
            self.frames_by_clock.set(self.frames_by_clock.get() + 1);
        }
        if let Some(id) = self.frame_timer.borrow_mut().take() {
            id.remove();
        }
        let host = self.host.load(Ordering::Acquire);
        if !host.is_null() {
            unsafe { ((*host).frame)((*host).self_) };
        }
    }
}

/// Everything `wake` may touch from another thread lives in `host`; the rest is main-thread only.
struct Ctl {
    host: Arc<AtomicPtr<svl_host>>,
    local: Rc<Inner>,
}

thread_local! {
    static APP: RefCell<Option<gtk::Application>> = const { RefCell::new(None) };
}

/// One registered `GtkApplication` per UI thread: GTK exports it on the session bus, which
/// allows a single instance per process.
fn shared_app() -> Result<gtk::Application, String> {
    APP.with(|slot| {
        if let Some(app) = slot.borrow().as_ref() {
            return Ok(app.clone());
        }
        let app = gtk::Application::builder().flags(gtk::gio::ApplicationFlags::NON_UNIQUE).build();
        app.register(None::<&gtk::gio::Cancellable>).map_err(|e| e.to_string())?;
        *slot.borrow_mut() = Some(app.clone());
        Ok(app)
    })
}

pub struct GtkBackend {
    vtable: Box<svl_backend>,
    ctl: Box<Ctl>,
}

impl GtkBackend {
    /// Initialise GTK on the calling thread (which must become the UI thread) and build the backend.
    pub fn new() -> Result<GtkBackend, String> {
        gtk::init().map_err(|e| e.to_string())?;
        let app = shared_app()?;
        let host = Arc::new(AtomicPtr::new(std::ptr::null_mut()));
        let inner = Rc::new(Inner {
            app,
            host: host.clone(),
            state: RefCell::new(State::default()),
            error: RefCell::new(CString::default()),
            out: RefCell::new(StrBuf::default()),
            wake_timer: RefCell::new(None),
            frame_pending: Cell::new(false),
            frame_clock: RefCell::new(None),
            frame_timer: RefCell::new(None),
            frames_by_clock: Cell::new(0),
        });
        let mut ctl = Box::new(Ctl { host, local: inner });
        let vtable = Box::new(svl_backend {
            abi_version: SVL_ABI_VERSION,
            self_: &mut *ctl as *mut Ctl as *mut c_void,
            resolve: b_resolve,
            create: b_create,
            insert: b_insert,
            remove: b_remove,
            destroy: b_destroy,
            set_prop: b_set_prop,
            get_prop: b_get_prop,
            call: b_call,
            listen: b_listen,
            handle_info: b_handle_info,
            run: b_run,
            wake: b_wake,
            schedule_wake: b_schedule_wake,
            quit: b_quit,
            last_error: b_last_error,
            request_frame: b_request_frame,
        });
        Ok(GtkBackend { vtable, ctl })
    }

    /// The C ABI vtable; valid while `self` lives.
    pub fn as_raw(&self) -> *const svl_backend {
        &*self.vtable
    }

    /// Provide the host callbacks without entering `run` (tests, embedding).
    ///
    /// # Safety
    /// `host` must outlive every later backend call.
    pub unsafe fn attach_host(&self, host: *const svl_host) {
        self.ctl.host.store(host as *mut svl_host, Ordering::Release);
    }

    /// Frames delivered by the GDK frame clock so far (the others came from the fallback timer).
    pub fn frames_by_clock(&self) -> u32 {
        self.ctl.local.frames_by_clock.get()
    }

    /// Diagnostic access to the object behind a handle.
    pub fn object(&self, h: u64) -> Option<glib::Object> {
        self.ctl.local.obj_of(h).ok().map(|x| x.0)
    }
}

unsafe fn ctl<'a>(p: *mut c_void) -> &'a Ctl {
    unsafe { &*(p as *const Ctl) }
}

fn run_int(c: &Ctl, f: impl FnOnce(&Rc<Inner>) -> Res<()>) -> c_int {
    let _t = crate::timing::scope();
    match catch_unwind(AssertUnwindSafe(|| f(&c.local))) {
        Ok(Ok(())) => 0,
        Ok(Err(e)) => {
            c.local.set_error(e);
            -1
        }
        Err(_) => {
            c.local.set_error("internal panic in svelters-gtk".into());
            -1
        }
    }
}

unsafe extern "C" fn b_resolve(p: *mut c_void, what: u32, kind: svl_id, name: *const c_char, len: usize) -> svl_id {
    let bytes = unsafe { std::slice::from_raw_parts(name as *const u8, len) };
    let Ok(name) = std::str::from_utf8(bytes) else { return 0 };
    let _ = p;
    match what {
        SVL_KIND => lookup::kind_by_name(name),
        SVL_PROP => lookup::find_prop(kind, name),
        SVL_SIGNAL => lookup::find_signal(kind, name),
        SVL_METHOD => lookup::find_method(kind, name),
        SVL_SLOT => lookup::find_slot(kind, name),
        _ => 0,
    }
}

unsafe extern "C" fn b_create(p: *mut c_void, kind: svl_id) -> svl_handle {
    let c = unsafe { ctl(p) };
    let mut out = 0;
    run_int(c, |i| {
        out = i.create(kind)?;
        Ok(())
    });
    out
}

unsafe extern "C" fn b_insert(p: *mut c_void, parent: svl_handle, child: svl_handle, before: svl_handle, slot: svl_id) -> c_int {
    run_int(unsafe { ctl(p) }, |i| i.insert(parent, child, before, slot))
}

unsafe extern "C" fn b_remove(p: *mut c_void, parent: svl_handle, child: svl_handle) -> c_int {
    run_int(unsafe { ctl(p) }, |i| i.remove(parent, child))
}

unsafe extern "C" fn b_destroy(p: *mut c_void, h: svl_handle) {
    run_int(unsafe { ctl(p) }, |i| i.destroy(h));
}

unsafe extern "C" fn b_set_prop(p: *mut c_void, h: svl_handle, prop: svl_id, v: *const svl_value) -> c_int {
    run_int(unsafe { ctl(p) }, |i| i.set_prop(h, prop, unsafe { &*v }))
}

unsafe extern "C" fn b_get_prop(p: *mut c_void, h: svl_handle, prop: svl_id, out: *mut svl_value) -> c_int {
    run_int(unsafe { ctl(p) }, |i| i.get_prop(h, prop, unsafe { &mut *out }))
}

unsafe extern "C" fn b_call(p: *mut c_void, h: svl_handle, method: svl_id, _args: *const svl_value, n: usize, out: *mut svl_value) -> c_int {
    run_int(unsafe { ctl(p) }, |i| i.call(h, method, n, unsafe { &mut *out }))
}

unsafe extern "C" fn b_listen(p: *mut c_void, h: svl_handle, signal: svl_id, on: bool) -> c_int {
    run_int(unsafe { ctl(p) }, |i| i.listen(h, signal, on))
}

unsafe extern "C" fn b_handle_info(p: *mut c_void, h: svl_handle, out: *mut svl_handle_info) -> c_int {
    run_int(unsafe { ctl(p) }, |i| {
        let st = i.state.borrow();
        let e = st.entries.get(&h).ok_or_else(|| format!("stale or foreign handle {h}"))?;
        let lifetime = if e.hold.is_none() { SVL_LIFE_TOPLEVEL } else { SVL_LIFE_OWNED };
        unsafe { *out = svl_handle_info { kind: e.kind, lifetime, flags: 0 } };
        Ok(())
    })
}

/// Arms one glib timeout that runs a host checkpoint, replacing any pending one; `ms < 0` cancels.
unsafe extern "C" fn b_schedule_wake(p: *mut c_void, ms: i64) {
    let c = unsafe { ctl(p) };
    let inner = &c.local;
    if let Some(id) = inner.wake_timer.borrow_mut().take() {
        id.remove();
    }
    if ms < 0 {
        return;
    }
    let weak = Rc::downgrade(inner);
    let id = glib::timeout_add_local_once(std::time::Duration::from_millis(ms as u64), move || {
        let Some(inner) = weak.upgrade() else { return };
        // The source is finished once this callback runs: forget its id before anything can try
        // to remove it.
        inner.wake_timer.borrow_mut().take();
        let h = inner.host.load(Ordering::Acquire);
        if !h.is_null() {
            unsafe { ((*h).checkpoint)((*h).self_) };
        }
    });
    *inner.wake_timer.borrow_mut() = Some(id);
}

/// ABI v2.1: one `host.frame` on the next frame of a mapped toplevel's frame clock.
unsafe extern "C" fn b_request_frame(p: *mut c_void) {
    let c = unsafe { ctl(p) };
    c.local.request_frame();
}

unsafe extern "C" fn b_run(p: *mut c_void, host: *const svl_host) -> c_int {
    let c = unsafe { ctl(p) };
    c.host.store(host as *mut svl_host, Ordering::Release);
    let hostp = c.host.clone();
    // The host decides when the app quits (`quit`), so closing the last window must not.
    let _hold = c.local.app.hold();
    c.local.app.connect_activate(move |_| {
        let h = hostp.load(Ordering::Acquire);
        if !h.is_null() {
            unsafe { ((*h).checkpoint)((*h).self_) };
        }
    });
    let rc = c.local.app.run_with_args::<&str>(&[]).get() as c_int;
    // The host vtable is only guaranteed while `run` is on the stack: timers (a pending frame, a
    // wakeup) that fire later must not call into it.
    c.host.store(std::ptr::null_mut(), Ordering::Release);
    rc
}

/// Thread-safe: schedules a host checkpoint on the default main context.
unsafe extern "C" fn b_wake(p: *mut c_void) {
    let host = unsafe { &(*(p as *const Ctl)).host }.load(Ordering::Acquire) as usize;
    if host == 0 {
        return;
    }
    glib::idle_add_once(move || {
        let h = host as *const svl_host;
        unsafe { ((*h).checkpoint)((*h).self_) };
    });
}

unsafe extern "C" fn b_quit(p: *mut c_void) {
    unsafe { ctl(p) }.local.app.quit();
}

unsafe extern "C" fn b_last_error(p: *mut c_void) -> *const c_char {
    let c = unsafe { ctl(p) };
    let e = c.local.error.borrow();
    // The CString is only replaced by the next failing call, which the ABI allows to invalidate it.
    let s: &CStr = e.as_c_str();
    s.as_ptr()
}
