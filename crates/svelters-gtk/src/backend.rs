//! `svl_backend` over GTK4 (see plans/architecture/backend-abi.md).
//!
//! Lifetime rules (docs/notes/gtk-v1-archaeology.md): the handle table holds one strong reference
//! per live handle, released only by `destroy`, so a removed-but-not-destroyed widget can be
//! re-inserted. Signal handlers capture a `Weak<Inner>` and are disconnected before the table
//! drops its reference, so a handler can never run against a freed entry. Natively destroyed
//! toplevels are reported through `host.on_destroyed`.
//!
//! Borrowed handles (plans/milestones/X1-borrowed-gi.md): an object a getter or call returns that
//! we did not create gets a BORROWED handle on first sight and keeps it for as long as the object
//! lives. The identity is stored on the object (GObject qdata), the entry holds only a weak
//! reference plus a `g_object_weak_ref` notify that reports the object's death as `SVL_GONE_OWNER`.

use crate::convert::{self, Env, StrBuf};
use crate::types::{self, TypeTable};
use crate::generated::{slots, tables::*};
use crate::gi::{Gi, GiErr, Ret};
use crate::lookup;
use gtk::glib;
use gtk::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::rc::Rc;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::Arc;
use svelters_abi::*;

mod extras;
use extras::CallErr;

struct Entry {
    /// Weak: toplevels are kept alive by GTK itself, so natively closing one really destroys it.
    obj: glib::WeakRef<glib::Object>,
    /// Strong reference for everything but toplevels; released only by `destroy`.
    hold: Option<glib::Object>,
    /// `SVL_LIFE_*`.
    class: u32,
    /// For borrowed handles: the handle whose getter or call first returned the object.
    owner: u64,
    /// The `g_object_weak_ref` data of a borrowed handle, null once consumed or released.
    note: *mut WeakNote,
    kind: u32,
    signals: HashMap<u32, glib::SignalHandlerId>,
    /// (parent handle, slot id) while inserted through a slot.
    slot: Option<(u64, u32)>,
}

#[derive(Default)]
struct State {
    next: u64,
    entries: HashMap<u64, Entry>,
    /// Opaque boxed values handed to JS as `BOXED` handles.
    boxed: HashMap<u64, glib::Value>,
}

/// Data of the weak-ref notify that reports the native death of a borrowed object.
struct WeakNote {
    inner: std::rc::Weak<Inner>,
    h: u64,
}

unsafe extern "C" fn weak_notify(data: glib::ffi::gpointer, _obj: *mut glib::gobject_ffi::GObject) {
    let note = unsafe { Box::from_raw(data as *mut WeakNote) };
    if let Some(inner) = note.inner.upgrade() {
        inner.borrowed_died(note.h);
    }
}

static BACKEND_IDS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn handle_quark() -> glib::Quark {
    glib::Quark::from_str("svelters-handle")
}

pub(crate) struct Inner {
    /// Distinguishes this backend's qdata from another backend's in the same process.
    id: u64,
    gi: RefCell<Option<Rc<Gi>>>,
    /// Weak self reference, for the native-death notifies minted by `intern`.
    me: std::rc::Weak<Inner>,
    /// The handle whose getter or call is converting results (owner of borrowed handles it mints).
    cur_owner: std::cell::Cell<u64>,
    app: gtk::Application,
    host: Arc<AtomicPtr<svl_host>>,
    state: RefCell<State>,
    error: RefCell<CString>,
    out: RefCell<StrBuf>,
    types: RefCell<TypeTable>,
    descs: DescStore,
    /// The pending `schedule_wake` source, if any. Cleared by the source's own callback.
    wake_timer: RefCell<Option<glib::SourceId>>,
    /// State of the `gio-file` services (see `extras.rs`).
    files: RefCell<HashMap<u64, extras::FileState>>,
}

type Res<T> = Result<T, String>;

impl Env for Inner {
    fn object(&self, h: u64) -> Option<glib::Object> {
        self.obj_of(h).ok().map(|x| x.0)
    }

    fn boxed(&self, h: u64) -> Option<glib::Value> {
        self.state.borrow().boxed.get(&h).cloned()
    }

    fn handle_of(&self, o: &glib::Object) -> u64 {
        self.tag_of(o)
    }

    fn mint_boxed(&self, v: glib::Value) -> u64 {
        let mut st = self.state.borrow_mut();
        st.next += 1;
        let h = st.next;
        st.boxed.insert(h, v);
        h
    }

    fn type_id(&self, t: glib::Type) -> svl_id {
        self.types.borrow_mut().id(t)
    }

    fn borrow_object(&self, o: &glib::Object) -> u64 {
        if !lookup::is_handle_type(o.type_()) {
            return 0;
        }
        self.intern(o.clone(), self.cur_owner.get(), false)
    }
}

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

    /// The live handle of `o`, or 0. Read from the object itself, so a recycled address can never
    /// alias another handle.
    fn tag_of(&self, o: &glib::Object) -> u64 {
        let Some(p) = (unsafe { o.qdata::<(u64, u64)>(handle_quark()) }) else { return 0 };
        let (b, h) = unsafe { *p.as_ref() };
        if b == self.id && self.state.borrow().entries.contains_key(&h) { h } else { 0 }
    }

    fn tag_object(&self, o: &glib::Object, h: u64) {
        unsafe { o.set_qdata(handle_quark(), (self.id, h)) };
    }

    fn untag_object(&self, o: &glib::Object, h: u64) {
        if let Some(p) = unsafe { o.qdata::<(u64, u64)>(handle_quark()) } {
            if unsafe { *p.as_ref() } == (self.id, h) {
                unsafe {
                    let _ = o.steal_qdata::<(u64, u64)>(handle_quark());
                }
            }
        }
    }

    /// The handle for an object a getter or call returned: its existing one (an object we created
    /// keeps its OWNED/TOPLEVEL handle) or a new BORROWED one. `adopt` is true when the caller
    /// passes a reference it owns (`transfer full`); such an object nobody else holds becomes an
    /// OWNED handle that keeps it alive. Otherwise the handle holds the object weakly.
    fn intern(&self, o: glib::Object, owner: u64, adopt: bool) -> u64 {
        let h = self.tag_of(&o);
        if h != 0 {
            return h;
        }
        let kind = lookup::nearest_kind(o.type_());
        let owned = adopt && !o.is::<gtk::Window>();
        let (class, hold, note) = if owned {
            (SVL_LIFE_OWNED, Some(o.clone()), std::ptr::null_mut())
        } else {
            let note = Box::into_raw(Box::new(WeakNote { inner: self.me.clone(), h: 0 }));
            (SVL_LIFE_BORROWED, None, note)
        };
        let h = {
            let mut st = self.state.borrow_mut();
            st.next += 1;
            let h = st.next;
            if !note.is_null() {
                unsafe { (*note).h = h };
            }
            st.entries.insert(h, Entry { obj: o.downgrade(), hold, class, owner, note, kind, signals: HashMap::new(), slot: None });
            h
        };
        self.tag_object(&o, h);
        if !note.is_null() {
            unsafe { glib::gobject_ffi::g_object_weak_ref(o.as_ptr() as *mut _, Some(weak_notify), note as glib::ffi::gpointer) };
        }
        h
    }

    /// A borrowed object was disposed natively (its owner released it).
    fn borrowed_died(&self, h: u64) {
        let removed = self.state.borrow_mut().entries.remove(&h);
        if removed.is_none() {
            return;
        }
        let host = self.host.load(Ordering::Acquire);
        if !host.is_null() {
            unsafe { ((*host).on_destroyed)((*host).self_, h, SVL_GONE_OWNER) };
        }
    }

    /// Detach everything an entry attached to its object: signal handlers, identity, weak notify.
    fn release_entry(&self, h: u64, e: &mut Entry, obj: Option<&glib::Object>) {
        if let Some(obj) = obj {
            for (_, id) in e.signals.drain() {
                obj.disconnect(id);
            }
            self.untag_object(obj, h);
            if !e.note.is_null() {
                unsafe {
                    glib::gobject_ffi::g_object_weak_unref(obj.as_ptr() as *mut _, Some(weak_notify), e.note as glib::ffi::gpointer);
                    drop(Box::from_raw(e.note));
                }
                e.note = std::ptr::null_mut();
            }
        }
    }

    /// The dynamic-method table, loaded on first use.
    fn gi(&self) -> Res<Rc<Gi>> {
        if let Some(g) = self.gi.borrow().as_ref() {
            return Ok(g.clone());
        }
        let g = Rc::new(Gi::new()?);
        *self.gi.borrow_mut() = Some(g.clone());
        Ok(g)
    }

    /// `resolve(SVL_METHOD)` fallback: ids past the generated `METHODS` are typelib methods.
    fn resolve_dynamic(&self, kind: u32, name: &str) -> u32 {
        let Some(k) = KINDS.get((kind as usize).wrapping_sub(1)) else { return 0 };
        let gi = match self.gi() {
            Ok(g) => g,
            Err(e) => {
                self.set_error(e);
                return 0;
            }
        };
        match gi.resolve(kind, k.gtype, name) {
            Some(i) => METHODS.len() as u32 + 1 + i,
            None => 0,
        }
    }

    fn set_error(&self, msg: String) {
        *self.error.borrow_mut() = CString::new(msg.replace('\0', " ")).unwrap_or_default();
    }

    fn create(self: &Rc<Self>, kind: u32) -> Res<u64> {
        if extras::is_extra(kind) {
            return self.create_extra(kind);
        }
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
            let top = obj.is::<gtk::Window>();
            let hold = if top { None } else { Some(obj.clone()) };
            let class = if top { SVL_LIFE_TOPLEVEL } else { SVL_LIFE_OWNED };
            st.entries.insert(h, Entry { obj: obj.downgrade(), hold, class, owner: 0, note: std::ptr::null_mut(), kind, signals: HashMap::new(), slot: None });
            h
        };
        self.tag_object(&obj, h);
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
                let mut kids: Vec<u64> = st
                    .entries
                    .iter()
                    .filter(|(k, e)| (e.slot.is_some_and(|(p, _)| p == parent) || (e.class == SVL_LIFE_BORROWED && e.owner == parent)) && !order.contains(k))
                    .map(|(k, _)| *k)
                    .collect();
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
            v
        };
        let host = self.host.load(Ordering::Acquire);
        for (k, mut e) in gone {
            // The window itself is being disposed: its handlers go with it.
            if k != h {
                let obj = e.obj.upgrade();
                self.release_entry(k, &mut e, obj.as_ref());
            } else {
                self.untag_object(win.upcast_ref(), k);
            }
            drop(e.hold.take());
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
        // A boxed handle: drop the held value (idempotent).
        if self.state.borrow_mut().boxed.remove(&h).is_some() {
            return Ok(());
        }
        if self.extra_kind(h).is_some() {
            self.destroy_extra(h);
        }
        let mut entry = {
            let mut st = self.state.borrow_mut();
            let Some(e) = st.entries.remove(&h) else { return Ok(()) };
            e
        };
        let obj = entry.obj.upgrade();
        self.release_entry(h, &mut entry, obj.as_ref());
        if let Some(obj) = obj {
            // A borrowed handle is only forgotten: the object belongs to its native owner.
            if entry.class == SVL_LIFE_BORROWED {
                return Ok(());
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

    fn not_borrowed(&self, h: u64, what: &str) -> Res<()> {
        match self.state.borrow().entries.get(&h) {
            Some(e) if e.class == SVL_LIFE_BORROWED => Err(format!("handle {h} is a borrowed native object and cannot be {what} through the tree")),
            _ => Ok(()),
        }
    }

    fn insert(&self, parent: u64, child: u64, before: u64, slot: u32) -> Res<()> {
        let (pw, pk) = self.widget_of(parent)?;
        let (cw, _) = self.widget_of(child)?;
        self.not_borrowed(child, "inserted")?;
        self.not_borrowed(parent, "inserted into")?;
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
                let oh = self.tag_of(old.upcast_ref());
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
        let (cw, _) = self.widget_of(child)?;
        self.not_borrowed(child, "removed")?;
        self.not_borrowed(parent, "removed from")?;
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
        if self.extra_kind(h).is_some() {
            return self.extra_set_prop(h, prop, v);
        }
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
            return slots::virtual_set(prop, &obj, text.as_deref());
        }
        let pspec = obj.find_property(p.name).ok_or_else(|| format!("{} has no property {}", obj.type_().name(), p.name))?;
        let value = unsafe { convert::to_value(pspec.value_type(), v, self) }.map_err(|e| format!("{}: {e}", p.name))?;
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
        if self.extra_kind(h).is_some() {
            return self.extra_get_prop(h, prop, out);
        }
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
        // Objects we did not create get a borrowed handle, minted by `Env::borrow_object`.
        self.cur_owner.set(h);
        *out = convert::from_value(&value, self, &mut buf);
        Ok(())
    }

    fn call(&self, h: u64, method: u32, args: &[svl_value], out: &mut svl_value) -> Result<(), CallErr> {
        let (obj, kind) = self.obj_of(h)?;
        let Some(m) = METHODS.get((method as usize).wrapping_sub(1)) else {
            return self.call_dynamic(h, &obj, method, args, out);
        };
        if !lookup::in_chain(kind, m.kind) {
            return Err(format!("method {} does not belong to {}", m.name, KINDS[kind as usize - 1].gtype).into());
        }
        if !args.is_empty() {
            return Err(format!("method {} takes no arguments", m.name).into());
        }
        slots::call_method(method, &obj)?;
        *out = convert::null_value();
        Ok(())
    }

    /// A typelib method (ids past the generated `METHODS`), invoked through GIRepository.
    fn call_dynamic(&self, h: u64, obj: &glib::Object, method: u32, args: &[svl_value], out: &mut svl_value) -> Result<(), CallErr> {
        let idx = method - METHODS.len() as u32 - 1;
        let gi = self.gi()?;
        let dm = gi.method(idx).ok_or("unknown method")?;
        self.cur_owner.set(h);
        let ret = dm.call(obj, args, self).map_err(|e| match e {
            GiErr::Plain(s) => CallErr::Plain(s),
            GiErr::Gerror(e) => CallErr::Gerror(e),
        })?;
        let mut buf = self.out.borrow_mut();
        *buf = StrBuf::default();
        *out = match ret {
            Ret::Null => convert::null_value(),
            Ret::Bool(b) => svl_value::new(SVL_BOOL, svl_value_u { b }),
            Ret::Int(i) => svl_value::new(SVL_I64, svl_value_u { i }),
            Ret::Float(f) => svl_value::new(SVL_F64, svl_value_u { f }),
            Ret::Str(s) => buf.str_value(&s),
            Ret::Strv(v) => {
                let items = v.iter().map(|s| buf.str_value(s)).collect();
                buf.list_value(SVL_LIST, 0, items)
            }
            Ret::Value(v) => convert::from_value(&v, self, &mut buf),
            Ret::Object(o, owned) if lookup::is_handle_type(o.type_()) => match self.intern(o, h, owned) {
                0 => convert::null_value(),
                x => svl_value::new(SVL_HANDLE, svl_value_u { h: x }),
            },
            // Other objects (a layout, an icon) are opaque values: BOXED handles.
            Ret::Object(o, _) => convert::from_value(&o.to_value(), self, &mut buf),
        };
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

    /// Native signal -> `host.post_event`. Returns the value the signal's accumulator expects.
    fn deliver(&self, h: u64, signal: u32, vals: &[glib::Value]) -> Option<glib::Value> {
        let sig = &SIGNALS[signal as usize - 1];
        let host = self.host.load(Ordering::Acquire);
        if !host.is_null() {
            let mut buf = StrBuf::default();
            let mut args: Vec<svl_value> = vals.iter().skip(1).map(|v| convert::from_value(v, self, &mut buf)).collect();
            // GtkEditable::changed has no arguments; the JS contract (and the Qt backend) delivers `[text]`.
            if sig.name == "changed" && args.is_empty() {
                if let Some(e) = vals.first().and_then(|v| v.get::<glib::Object>().ok()).and_then(|o| o.downcast::<gtk::Editable>().ok()) {
                    args.push(buf.str_value(&e.text()));
                }
            }
            unsafe { ((*host).post_event)((*host).self_, h, signal, args.as_ptr(), args.len()) };
        }
        // onRequestClose: a JS listener on close-request owns the decision, so veto the native
        // close; the app destroys the window when its state says so.
        if sig.name == "close-request" {
            return Some(true.to_value());
        }
        match sig.handler_result {
            1 => Some(false.to_value()),
            _ => None,
        }
    }
}

impl Inner {
    /// The descriptor of a property of the handle kind `kind` (`SVL_PROP` only; other member
    /// kinds have no value descriptors yet).
    fn describe_member(&self, what: u32, kind: u32, member: u32) -> Option<MemberDesc> {
        if extras::is_extra(kind) {
            return extras::describe_member(what, member);
        }
        if what == SVL_METHOD && member as usize > METHODS.len() {
            let gi = self.gi.borrow().clone()?;
            return gi.method(member - METHODS.len() as u32 - 1)?.describe(&mut self.types.borrow_mut());
        }
        if what != SVL_PROP {
            return None;
        }
        let p = PROPS.get((member as usize).checked_sub(1)?)?;
        let k = KINDS.get((kind as usize).checked_sub(1)?)?;
        if !lookup::in_chain(kind, p.kind) {
            return None;
        }
        let mut table = self.types.borrow_mut();
        let readable = p.flags & F_READ != 0;
        let writable = p.flags & F_WRITE != 0;
        if p.flags & F_VIRTUAL != 0 {
            let s = SlotDesc::new(SVL_STR).flags(SVL_D_NULLABLE).named("value");
            return Some(MemberDesc {
                name: p.name.to_string(),
                params: if writable { vec![s.clone()] } else { vec![] },
                ret: if readable { s } else { SlotDesc::new(SVL_NULL) },
                flags: 0,
            });
        }
        // The class (not the interface) of the handle kind knows the GParamSpec, inherited
        // and interface-provided properties included.
        crate::generated::types::ensure_registered(k.gtype);
        let t = glib::Type::from_name(k.gtype)?;
        let ps = if convert::is_iface(t) {
            None
        } else {
            glib::object::ObjectClass::from_type(t).and_then(|c| c.find_property(p.name))
        };
        let vt = match &ps {
            Some(ps) => ps.value_type(),
            None => glib::Type::from_name(p.gtype).unwrap_or(glib::Type::INVALID),
        };
        let slot = types::slot_for(&mut table, vt, ps.as_ref()).named(p.js_name);
        Some(MemberDesc {
            name: p.name.to_string(),
            params: if writable { vec![slot.clone()] } else { vec![] },
            ret: if readable { slot } else { SlotDesc::new(SVL_NULL) },
            flags: 0,
        })
    }

    fn describe_type(&self, id: u32) -> Option<TypeDesc> {
        if let Some(t) = extras::describe_type(id) {
            return Some(t);
        }
        let mut table = self.types.borrow_mut();
        let t = table.lookup(id)?;
        Some(types::type_desc(&mut table, t))
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
        let inner = Rc::new_cyclic(|me| Inner {
            id: BACKEND_IDS.fetch_add(1, Ordering::Relaxed),
            gi: RefCell::new(None),
            me: me.clone(),
            cur_owner: std::cell::Cell::new(0),
            app,
            host: host.clone(),
            state: RefCell::new(State::default()),
            error: RefCell::new(CString::default()),
            out: RefCell::new(StrBuf::default()),
            types: RefCell::new(TypeTable::default()),
            descs: DescStore::default(),
            wake_timer: RefCell::new(None),
            files: RefCell::new(HashMap::new()),
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
            caps: 0,
            describe: Some(b_describe),
            describe_type: Some(b_describe_type),
            cancel: Some(b_cancel),
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
    match what {
        SVL_KIND => match extras::kind_by_name(name) {
            0 => lookup::kind_by_name(name),
            k => k,
        },
        _ if extras::is_extra(kind) => extras::find_member(what, kind, name),
        SVL_PROP => lookup::find_prop(kind, name),
        SVL_SIGNAL => lookup::find_signal(kind, name),
        SVL_METHOD => match lookup::find_method(kind, name) {
            0 => unsafe { ctl(p) }.local.resolve_dynamic(kind, name),
            id => id,
        },
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

unsafe extern "C" fn b_call(p: *mut c_void, h: svl_handle, method: svl_id, args: *const svl_value, n: usize, out: *mut svl_value) -> c_int {
    let c = unsafe { ctl(p) };
    let _t = crate::timing::scope();
    let out = unsafe { &mut *out };
    let r = catch_unwind(AssertUnwindSafe(|| {
        if c.local.extra_kind(h).is_some() {
            c.local.extra_call(h, method, args, n, out)
        } else {
            c.local.call(h, method, extras::args_of(args, n), out)
        }
    }));
    match r {
        Ok(Ok(())) => 0,
        Ok(Err(CallErr::Plain(e))) => {
            c.local.set_error(e);
            -1
        }
        // A GError crosses as an ERROR value in `out` (and its message as `last_error`).
        Ok(Err(CallErr::Gerror(e))) => {
            c.local.set_error(e.message().to_string());
            c.local.write_error(&e, out);
            -1
        }
        Err(_) => {
            c.local.set_error("internal panic in svelters-gtk".into());
            -1
        }
    }
}

unsafe extern "C" fn b_cancel(p: *mut c_void, callback: u64) -> c_int {
    unsafe { ctl(p) }.local.cancel_callback(callback) as c_int
}

unsafe extern "C" fn b_listen(p: *mut c_void, h: svl_handle, signal: svl_id, on: bool) -> c_int {
    run_int(unsafe { ctl(p) }, |i| i.listen(h, signal, on))
}

unsafe extern "C" fn b_handle_info(p: *mut c_void, h: svl_handle, out: *mut svl_handle_info) -> c_int {
    run_int(unsafe { ctl(p) }, |i| {
        let st = i.state.borrow();
        let e = st.entries.get(&h).ok_or_else(|| format!("stale or foreign handle {h}"))?;
        unsafe { *out = svl_handle_info { kind: e.kind, lifetime: e.class, flags: 0 } };
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
    c.local.app.run_with_args::<&str>(&[]).get() as c_int
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

unsafe extern "C" fn b_describe(p: *mut c_void, what: u32, kind: svl_id, member: svl_id) -> *const svl_member_desc {
    let c = unsafe { ctl(p) };
    let i = &c.local;
    i.descs.member((what, kind, member), || catch_unwind(AssertUnwindSafe(|| i.describe_member(what, kind, member))).ok().flatten())
}

unsafe extern "C" fn b_describe_type(p: *mut c_void, ty: svl_id) -> *const svl_type_desc {
    let c = unsafe { ctl(p) };
    let i = &c.local;
    i.descs.type_desc(ty, || catch_unwind(AssertUnwindSafe(|| i.describe_type(ty))).ok().flatten())
}
