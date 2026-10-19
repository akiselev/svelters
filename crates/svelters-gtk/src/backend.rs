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
use crate::gi::{self, Called, Gi, GiErr, Ret};
use crate::lookup;
use crate::reflect::{self, Reflect};
use crate::util::AttrV;
use gtk::glib::{self, Type};
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::rc::Rc;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::Arc;
use svelters_abi::*;

pub mod canvas;
mod extras;
mod menus;
pub(crate) use menus::signal_name as menus_signal_name;
mod style;
mod vlist;
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
    /// Connections by wire id: the signal id with the listener flags in bits 28..30.
    signals: HashMap<u32, glib::SignalHandlerId>,
    /// (parent handle, slot id) while inserted through a slot.
    slot: Option<(u64, u32)>,
    /// The propagation phase an event controller had before its first capture listener.
    capture_prev: Option<gtk::PropagationPhase>,
}

#[derive(Default)]
struct State {
    next: u64,
    entries: HashMap<u64, Entry>,
    /// Opaque boxed values handed to JS as `BOXED` handles.
    boxed: HashMap<u64, glib::Value>,
    /// The object a `via` signal (GtkTextView `changed`) is connected on, by (handle, wire id).
    via_targets: HashMap<(u64, u32), glib::Object>,
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
    /// Reflected kinds, properties and signals (see `reflect.rs`).
    reflect: RefCell<Reflect>,
    descs: DescStore,
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
    /// State of the `gio-file` services (see `extras.rs`).
    files: RefCell<HashMap<u64, extras::FileState>>,
    /// `virtual-list` widgets (see `vlist.rs`).
    vlists: RefCell<HashMap<u64, Rc<vlist::VList>>>,
    /// Menu and action nodes (see `menus.rs`).
    menus: RefCell<HashMap<u64, menus::Node>>,
    /// Styling state (see `style.rs`).
    style: RefCell<style::StyleState>,
    style_flush_queued: Cell<bool>,
}

/// Slot value recorded for an event controller attached to a widget (it is not in `SLOTS`).
const CONTROLLER_SLOT: u32 = u32::MAX;
/// Slot value recorded for a toplevel inserted under a parent: it is transient for the parent's
/// window (`gtk_window_set_transient_for`), see "Toplevels in the tree" in backend-abi.md.
const TRANSIENT_SLOT: u32 = u32::MAX - 1;
/// Frame delay used when there is no mapped toplevel to tick on.
const FRAME_FALLBACK_MS: u64 = 16;
/// A mapped toplevel whose clock stalls (hidden, minimised) still gets a frame after this long.
const FRAME_WATCHDOG_MS: u64 = 250;

type Res<T> = Result<T, String>;

/// The receiver of a dynamic call.
enum Recv {
    Obj(glib::Object),
    Val(glib::Value),
}

fn gi_err(e: GiErr) -> CallErr {
    match e {
        GiErr::Plain(s) => CallErr::Plain(s),
        GiErr::Gerror(e) => CallErr::Gerror(e),
    }
}

fn attr_from_svl(v: &svl_value) -> Res<AttrV> {
    Ok(match v.tag {
        SVL_NULL => AttrV::Null,
        SVL_BOOL => AttrV::Bool(unsafe { v.u.b }),
        SVL_I64 => AttrV::Int(unsafe { v.u.i }),
        SVL_F64 => AttrV::Float(unsafe { v.u.f }),
        SVL_STR => AttrV::Str(unsafe { convert::as_str(v) }.ok_or("bad string")?.into_owned()),
        other => return Err(format!("unsupported value tag {other} for a child attribute")),
    })
}

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

    fn type_by_name(&self, name: &str) -> Option<Type> {
        match self.gi() {
            Ok(g) => g.type_by_name(name),
            Err(_) => Type::from_name(name).filter(|t| *t != Type::INVALID),
        }
    }

    fn type_name(&self, t: Type) -> String {
        match self.gi() {
            Ok(g) => g.qualified_name(t),
            Err(_) => t.name().to_string(),
        }
    }

    fn borrow_object(&self, o: &glib::Object) -> u64 {
        // Every GObject crosses as a handle: widgets BORROWED, everything else SHARED.
        self.intern(o.clone(), self.cur_owner.get(), false)
    }
}

impl Inner {
    fn obj_of(&self, h: u64) -> Res<(glib::Object, u32)> {
        let st = self.state.borrow();
        let e = st.entries.get(&h).ok_or_else(|| format!("stale or foreign handle {h}"))?;
        Ok((e.obj.upgrade().ok_or_else(|| format!("handle {h} was destroyed"))?, self.table_kind(e.kind)))
    }

    /// The kind the generated tables use for `kind`: a `virtual-list` is a `ScrolledWindow` there.
    fn table_kind(&self, kind: u32) -> u32 {
        if kind == vlist::KIND_VLIST {
            lookup::kind_by_name("ScrolledWindow")
        } else if let Some(n) = menus::table_name(kind) {
            lookup::kind_by_name(n)
        } else {
            kind
        }
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
        let widget = o.is::<gtk::Widget>();
        let kind = if widget { lookup::nearest_kind(o.type_()) } else { self.reflect.borrow_mut().kind_for_type(o.type_()) };
        // Non-widget objects are SHARED: the table holds a strong reference until `destroy`, so
        // identity by qdata is safe and the host may release the handle when the wrapper is collected.
        let shared = !widget;
        let owned = adopt && widget && !o.is::<gtk::Window>();
        let (class, hold, note) = if shared {
            (SVL_LIFE_SHARED, Some(o.clone()), std::ptr::null_mut())
        } else if owned {
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
            let owner = if shared { 0 } else { owner };
            st.entries.insert(h, Entry { obj: o.downgrade(), hold, class, owner, note, kind, signals: HashMap::new(), slot: None, capture_prev: None });
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
            for (key, id) in e.signals.drain() {
                match self.state.borrow_mut().via_targets.remove(&(h, key)) {
                    Some(t) => t.disconnect(id),
                    None => obj.disconnect(id),
                }
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
        let Some(t) = self.reflect.borrow().type_of_kind(kind) else { return 0 };
        let gi = match self.gi() {
            Ok(g) => g,
            Err(e) => {
                self.set_error(e);
                return 0;
            }
        };
        match gi.resolve(kind, t, name) {
            Some(i) => METHODS.len() as u32 + 1 + i,
            None => 0,
        }
    }

    /// `resolve(SVL_KIND)` fallback: a qualified native name (`Gio.ListStore`) or a GType name
    /// becomes a reflected kind.
    fn resolve_kind_native(&self, name: &str) -> u32 {
        let Ok(gi) = self.gi() else { return 0 };
        let Some(t) = gi.type_by_name(name) else { return 0 };
        if !(t.is_a(Type::OBJECT) || t.is_a(Type::BOXED) || t.is_a(Type::INTERFACE)) {
            return 0;
        }
        self.reflect.borrow_mut().kind_for_type(t)
    }

    /// `describe(SVL_KIND, 0, kind)`: the qualified native name and the nearest bound ancestor kind.
    fn describe_kind(&self, kind: u32) -> Option<MemberDesc> {
        let t = self.reflect.borrow().type_of_kind(kind)?;
        let name = match self.gi() {
            Ok(g) => g.qualified_name(t),
            Err(_) => t.name().to_string(),
        };
        let parent = if t.is_a(Type::OBJECT) || t.is_a(Type::INTERFACE) {
            t.parent().map_or(0, |p| self.reflect.borrow_mut().kind_for_type(p))
        } else {
            0
        };
        Some(MemberDesc { name, params: vec![], ret: SlotDesc::new(SVL_NULL).typed(parent), flags: 0 })
    }

    fn set_error(&self, msg: String) {
        *self.error.borrow_mut() = CString::new(msg.replace('\0', " ")).unwrap_or_default();
    }

    fn create(self: &Rc<Self>, kind: u32) -> Res<u64> {
        if kind == vlist::KIND_VLIST {
            return self.create_vlist();
        }
        if menus::is_menu_kind(kind) {
            return self.create_menu_node(kind);
        }
        if extras::is_extra(kind) {
            return self.create_extra(kind);
        }
        if kind >= reflect::KIND_BASE {
            return self.construct(kind, &[], &[]);
        }
        self.create_with(kind, Vec::new())
    }

    /// ABI v2.5 `construct`: an instance with construct-time properties (`g_object_new_with_properties`,
    /// so construct-only properties work). Widgets get OWNED/TOPLEVEL handles like `create`; every other
    /// object gets a SHARED one.
    fn construct(self: &Rc<Self>, kind: u32, props: &[u32], values: &[svl_value]) -> Res<u64> {
        if props.len() != values.len() {
            return Err("construct: props and values differ in length".into());
        }
        if kind == vlist::KIND_VLIST || menus::is_menu_kind(kind) || extras::is_extra(kind) {
            if !props.is_empty() {
                return Err("construct: this kind takes its properties through setProp".into());
            }
            return self.create(kind);
        }
        let t = self.reflect.borrow().type_of_kind(kind).ok_or("unknown kind")?;
        if !t.is_a(Type::OBJECT) || t.is_a(Type::INTERFACE) || unsafe { glib::gobject_ffi::g_type_test_flags(glib::translate::IntoGlib::into_glib(t), glib::gobject_ffi::G_TYPE_FLAG_ABSTRACT as u32) != 0 } {
            return Err(format!("{} is not instantiable", t.name()));
        }
        let mut vals: Vec<(String, glib::Value)> = Vec::new();
        for (p, v) in props.iter().zip(values) {
            let ps = self.prop_pspec(kind, t, *p)?;
            if !ps.flags().contains(glib::ParamFlags::WRITABLE) {
                return Err(format!("property {} is read-only", ps.name()));
            }
            let value = unsafe { convert::to_value(ps.value_type(), v, &**self) }.map_err(|e| format!("{}: {e}", ps.name()))?;
            vals.push((ps.name().to_string(), value));
        }
        if t.is_a(gtk::Widget::static_type()) {
            return self.create_with(kind, vals);
        }
        let mut pairs: Vec<(&str, glib::Value)> = vals.iter().map(|(n, v)| (n.as_str(), v.clone())).collect();
        let obj = glib::Object::with_mut_values(t, &mut pairs);
        Ok(self.intern(obj, 0, true))
    }

    /// The `GParamSpec` behind property id `prop` of `kind` (a generated or a reflected id).
    fn prop_pspec(&self, kind: u32, t: Type, prop: u32) -> Res<glib::ParamSpec> {
        if prop >= reflect::DYN_BASE {
            let r = self.reflect.borrow();
            let ps = r.prop(prop).ok_or("unknown property")?;
            if !t.is_a(ps.owner_type()) {
                return Err(format!("property {} does not belong to {}", ps.name(), t.name()));
            }
            return Ok(ps.clone());
        }
        let p = PROPS.get((prop as usize).wrapping_sub(1)).ok_or("unknown property")?;
        if !lookup::in_chain(kind, p.kind) {
            return Err(format!("property {} does not belong to {}", p.name, t.name()));
        }
        glib::object::ObjectClass::from_type(t).and_then(|c| c.find_property(p.name)).ok_or_else(|| format!("{} has no property {}", t.name(), p.name))
    }

    fn create_with(self: &Rc<Self>, kind: u32, props: Vec<(String, glib::Value)>) -> Res<u64> {
        let k = KINDS.get((kind as usize).wrapping_sub(1)).ok_or("unknown kind")?;
        if k.is_abstract || k.is_interface {
            return Err(format!("{} is not instantiable", k.gtype));
        }
        crate::compat::require(k.since, &format!("<{}> ({})", k.tag, k.gtype))?;
        crate::generated::types::ensure_registered(k.gtype);
        let t = glib::Type::from_name(k.gtype).ok_or_else(|| format!("GType {} is not registered", k.gtype))?;
        if !t.is_a(glib::Type::OBJECT) {
            return Err(format!("{} is not instantiable", k.gtype));
        }
        let mut pairs: Vec<(&str, glib::Value)> = props.iter().map(|(n, v)| (n.as_str(), v.clone())).collect();
        let obj = glib::Object::with_mut_values(t, &mut pairs);
        if let Some(da) = obj.downcast_ref::<gtk::DrawingArea>() {
            canvas::install(da);
        }
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
            st.entries.insert(h, Entry { obj: obj.downgrade(), hold, class, owner: 0, note: std::ptr::null_mut(), kind, signals: HashMap::new(), slot: None, capture_prev: None });
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
                // GTK leaves a transient window alive when its parent goes; the tree says it belongs to the parent.
                if e.slot.is_some_and(|(_, s)| s == TRANSIENT_SLOT) {
                    if let Some(w) = obj.as_ref().and_then(|o| o.downcast_ref::<gtk::Window>()) {
                        w.set_visible(false);
                        let w = w.clone();
                        glib::idle_add_local_once(move || w.destroy());
                    }
                }
            } else {
                self.untag_object(win.upcast_ref(), k);
            }
            drop(e.hold.take());
            self.menus_forget(k);
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
    fn destroy(self: &Rc<Self>, h: u64) -> Res<()> {
        self.style_forget(h);
        // A boxed handle: drop the held value (idempotent).
        if self.state.borrow_mut().boxed.remove(&h).is_some() {
            return Ok(());
        }
        if self.menu_kind(h).is_some() {
            self.destroy_menu_node(h);
        } else if self.extra_kind(h) == Some(vlist::KIND_VLIST) {
            self.destroy_vlist(h);
        } else if self.extra_kind(h).is_some() {
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

    /// A toplevel child makes the window of `parent` its transient parent (dialogs, secondary windows).
    fn insert_transient(&self, parent: u64, child: u64, pw: &gtk::Widget, win: &gtk::Window) -> Res<()> {
        let owner = match pw.downcast_ref::<gtk::Window>() {
            Some(w) => w.clone(),
            None => pw.root().and_then(|r| r.downcast::<gtk::Window>().ok()).ok_or("insert: the parent is not inside a window")?,
        };
        if owner == *win {
            return Err("insert: a window cannot be transient for itself".into());
        }
        if self.state.borrow().entries.get(&child).and_then(|e| e.slot).is_some() {
            return Err("insert: the toplevel is already inserted".into());
        }
        win.set_transient_for(Some(&owner));
        if let Some(e) = self.state.borrow_mut().entries.get_mut(&child) {
            e.slot = Some((parent, TRANSIENT_SLOT));
        }
        Ok(())
    }

    fn not_borrowed(&self, h: u64, what: &str) -> Res<()> {
        match self.state.borrow().entries.get(&h) {
            Some(e) if e.class == SVL_LIFE_BORROWED => Err(format!("handle {h} is a borrowed native object and cannot be {what} through the tree")),
            _ => Ok(()),
        }
    }

    fn insert(self: &Rc<Self>, parent: u64, child: u64, before: u64, slot: u32) -> Res<()> {
        if self.menu_kind(child).is_some_and(|k| !matches!(k, menus::KIND_MENUBAR | menus::KIND_TOOLBAR)) {
            return self.menus_insert(parent, child, before);
        }
        if let Ok((co, _)) = self.obj_of(child) {
            if let Some(ctrl) = co.downcast_ref::<gtk::EventController>() {
                return self.insert_controller(parent, child, ctrl);
            }
        }
        let (pw, pk) = self.widget_of(parent)?;
        let (cw, _) = self.widget_of(child)?;
        self.not_borrowed(child, "inserted")?;
        self.not_borrowed(parent, "inserted into")?;
        if let Some(win) = cw.downcast_ref::<gtk::Window>() {
            return self.insert_transient(parent, child, &pw, win);
        }
        let slot = if slot == 0 { lookup::default_slot(pk) } else { slot };
        if slot == 0 {
            return Err("parent has no default slot".into());
        }
        let si = SLOTS.get(slot as usize - 1).ok_or("unknown slot")?;
        if !lookup::in_chain(pk, si.kind) {
            return Err(format!("slot {} does not belong to {}", si.name, KINDS[pk as usize - 1].gtype));
        }
        let bw = if before != 0 && !si.unordered { Some(self.widget_of(before)?.0) } else { None };
        if let Some(b) = &bw {
            if !si.ordered {
                return Err(format!("slot {} does not support insert-before", si.name));
            }
            // Wrapping containers (GtkListBox rows, GtkFlowBox children) put a wrapper in between.
            if !b.is_ancestor(&pw) {
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

    fn remove(self: &Rc<Self>, parent: u64, child: u64) -> Res<()> {
        if self.menu_kind(child).is_some_and(|k| !matches!(k, menus::KIND_MENUBAR | menus::KIND_TOOLBAR)) {
            return self.menus_remove(parent, child);
        }
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
        self.not_borrowed(child, "removed")?;
        self.not_borrowed(parent, "removed from")?;
        if let Some(win) = cw.downcast_ref::<gtk::Window>() {
            if self.state.borrow().entries.get(&child).and_then(|e| e.slot) != Some((parent, TRANSIENT_SLOT)) {
                return Err(format!("handle {child} is not inserted in {parent}"));
            }
            win.set_transient_for(None::<&gtk::Window>);
            if let Some(e) = self.state.borrow_mut().entries.get_mut(&child) {
                e.slot = None;
            }
            return Ok(());
        }
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

    /// The `ATTRS` entry for attribute `attr` of the child `child` inserted in `parent`, with the widgets.
    fn child_attr(&self, parent: u64, child: u64, attr: u32) -> Res<(u32, gtk::Widget, gtk::Widget)> {
        let (pw, pk) = self.widget_of(parent)?;
        let (cw, _) = self.widget_of(child)?;
        let slot = match self.state.borrow().entries.get(&child).and_then(|e| e.slot) {
            Some((p, s)) if p == parent && s != CONTROLLER_SLOT && s != TRANSIENT_SLOT => s,
            _ => return Err(format!("handle {child} is not inserted in {parent}: child attributes need an inserted child")),
        };
        let a = ATTRS.get((attr as usize).wrapping_sub(1)).ok_or("unknown child attribute")?;
        let id = lookup::attr_for_slot(pk, attr, slot).ok_or_else(|| {
            format!("child attribute {} does not apply to slot {} of {}", a.name, SLOTS[slot as usize - 1].name, KINDS[pk as usize - 1].gtype)
        })?;
        Ok((id, pw, cw))
    }

    /// ABI v2.2: sets a per-child attribute; null resets it to its default.
    fn set_child_attr(&self, parent: u64, child: u64, attr: u32, v: &svl_value) -> Res<()> {
        let (id, pw, cw) = self.child_attr(parent, child, attr)?;
        let info = &ATTRS[id as usize - 1];
        if !info.prop.is_empty() {
            let obj = slots::attr_object(id, &pw, &cw)?.ok_or("attribute has no backing object")?;
            let pspec = obj.find_property(info.prop).ok_or_else(|| format!("{} has no property {}", obj.type_().name(), info.prop))?;
            let value = if v.tag == SVL_NULL {
                pspec.default_value().clone()
            } else {
                unsafe { convert::to_value(pspec.value_type(), v, self) }.map_err(|e| format!("{}: {e}", info.name))?
            };
            obj.set_property_from_value(info.prop, &value);
            return Ok(());
        }
        let val = if v.tag == SVL_NULL {
            match info.default {
                AttrDef::Null => AttrV::Null,
                AttrDef::Bool(b) => AttrV::Bool(b),
                AttrDef::Int(i) => AttrV::Int(i),
                AttrDef::Float(f) => AttrV::Float(f),
                AttrDef::Str(t) => AttrV::Str(t.to_string()),
                AttrDef::Pspec => return Err(format!("attribute {} has no default to reset to", info.name)),
            }
        } else {
            attr_from_svl(v).map_err(|e| format!("{}: {e}", info.name))?
        };
        slots::attr_set_native(id, &pw, &cw, &val)
    }

    fn get_child_attr(&self, parent: u64, child: u64, attr: u32, out: &mut svl_value) -> Res<()> {
        let (id, pw, cw) = self.child_attr(parent, child, attr)?;
        let info = &ATTRS[id as usize - 1];
        let mut buf = self.out.borrow_mut();
        *buf = StrBuf::default();
        if !info.prop.is_empty() {
            let obj = slots::attr_object(id, &pw, &cw)?.ok_or("attribute has no backing object")?;
            *out = convert::from_value(&obj.property_value(info.prop), self, &mut buf);
            return Ok(());
        }
        *out = match slots::attr_get_native(id, &pw, &cw)? {
            AttrV::Null => convert::null_value(),
            AttrV::Bool(b) => svl_value::new(SVL_BOOL, svl_value_u { b }),
            AttrV::Int(i) => svl_value::new(SVL_I64, svl_value_u { i }),
            AttrV::Float(f) => svl_value::new(SVL_F64, svl_value_u { f }),
            AttrV::Str(t) => buf.str_value(&t),
        };
        Ok(())
    }

    fn set_prop(self: &Rc<Self>, h: u64, prop: u32, v: &svl_value) -> Res<()> {
        if let Some(k) = self.menu_kind(h) {
            if menus::is_own_member(prop) {
                return self.menus_set_prop(h, prop, v);
            } else if menus::table_name(k).is_none() {
                return Err("unknown property".into());
            }
        }
        match self.extra_kind(h) {
            Some(vlist::KIND_VLIST) if vlist::is_own_member(prop) => return self.vlist_set_prop(h, prop, v),
            Some(vlist::KIND_VLIST) | None => {}
            Some(_) => return self.extra_set_prop(h, prop, v),
        }
        if self.state.borrow().boxed.contains_key(&h) {
            return Err("a VALUE handle is immutable: it has no properties to set".into());
        }
        let (obj, kind) = self.obj_of(h)?;
        if prop >= reflect::DYN_BASE {
            let ps = self.reflect.borrow().prop(prop).cloned().ok_or("unknown property")?;
            if !obj.type_().is_a(ps.owner_type()) {
                return Err(format!("property {} does not belong to {}", ps.name(), obj.type_().name()));
            }
            if !ps.flags().contains(glib::ParamFlags::WRITABLE) {
                return Err(format!("property {} is read-only", ps.name()));
            }
            if ps.flags().contains(glib::ParamFlags::CONSTRUCT_ONLY) {
                return Err(format!("property {} can only be set when the object is constructed", ps.name()));
            }
            let value = unsafe { convert::to_value(ps.value_type(), v, &**self) }.map_err(|e| format!("{}: {e}", ps.name()))?;
            obj.set_property_from_value(ps.name(), &value);
            return Ok(());
        }
        let p = PROPS.get((prop as usize).wrapping_sub(1)).ok_or("unknown property")?;
        if !lookup::in_chain(kind, p.kind) {
            return Err(format!("property {} does not belong to {}", p.name, KINDS[kind as usize - 1].gtype));
        }
        if p.flags & F_WRITE == 0 {
            return Err(format!("property {} is read-only", p.name));
        }
        crate::compat::require(p.since, &format!("property {}:{}", KINDS[p.kind as usize - 1].gtype, p.name))?;
        if p.flags & F_VIRTUAL != 0 {
            let text = match v.tag {
                SVL_NULL => None,
                _ => Some(unsafe { convert::as_str(v) }.ok_or("expected string")?),
            };
            return slots::virtual_set(prop, &obj, text.as_deref());
        }
        // A forwarded property (GtkRange `value` is the adjustment's) is written on the object behind `via`.
        let elem = obj;
        let (obj, pname) = if p.flags & F_VIA != 0 { (via_object(&elem, p.via, p.name)?, p.target) } else { (elem.clone(), p.name) };
        let pspec = obj.find_property(pname).ok_or_else(|| format!("{} has no property {}", obj.type_().name(), pname))?;
        let value = unsafe { convert::to_value(pspec.value_type(), v, &**self) }.map_err(|e| format!("{}: {e}", p.name))?;
        // Echo suppression: handlers for the signals this property would raise are blocked while JS writes it.
        let st = self.state.borrow();
        // A `via` signal is connected on the forwarded-to object, not on the element.
        let blocked: Vec<(glib::Object, &glib::SignalHandlerId)> = st
            .entries
            .get(&h)
            .map(|e| {
                e.signals
                    .iter()
                    // Reflected signals (ids >= reflect::DYN_BASE, on non-widget objects) are not in the table.
                    .filter(|(sid, _)| SIGNALS.get(((**sid & SVL_SIGNAL_ID_MASK) as usize).wrapping_sub(1)).is_some_and(|s| p.echo.contains(&s.name)))
                    .map(|(sid, id)| (st.via_targets.get(&(h, *sid)).cloned().unwrap_or_else(|| elem.clone()), id))
                    .collect()
            })
            .unwrap_or_default();
        for (o, id) in &blocked {
            o.block_signal(id);
        }
        obj.set_property_from_value(pname, &value);
        for (o, id) in &blocked {
            o.unblock_signal(id);
        }
        Ok(())
    }

    fn get_prop(&self, h: u64, prop: u32, out: &mut svl_value) -> Res<()> {
        if let Some(k) = self.menu_kind(h) {
            if menus::is_own_member(prop) {
                return self.menus_get_prop(h, prop, out);
            } else if menus::table_name(k).is_none() {
                return Err("unknown property".into());
            }
        }
        match self.extra_kind(h) {
            Some(vlist::KIND_VLIST) if vlist::is_own_member(prop) => return self.vlist_get_prop(h, prop, out),
            Some(vlist::KIND_VLIST) | None => {}
            Some(_) => return self.extra_get_prop(h, prop, out),
        }
        if self.state.borrow().boxed.contains_key(&h) {
            return Err("a VALUE handle has no properties".into());
        }
        let (obj, kind) = self.obj_of(h)?;
        if prop >= reflect::DYN_BASE {
            let ps = self.reflect.borrow().prop(prop).cloned().ok_or("unknown property")?;
            if !obj.type_().is_a(ps.owner_type()) {
                return Err(format!("property {} does not belong to {}", ps.name(), obj.type_().name()));
            }
            if !ps.flags().contains(glib::ParamFlags::READABLE) {
                return Err(format!("property {} is write-only", ps.name()));
            }
            let value = obj.property_value(ps.name());
            let mut buf = self.out.borrow_mut();
            *buf = StrBuf::default();
            self.cur_owner.set(h);
            *out = convert::from_value(&value, self, &mut buf);
            return Ok(());
        }
        let p = PROPS.get((prop as usize).wrapping_sub(1)).ok_or("unknown property")?;
        if !lookup::in_chain(kind, p.kind) {
            return Err(format!("property {} does not belong to {}", p.name, KINDS[kind as usize - 1].gtype));
        }
        if p.flags & F_READ == 0 {
            return Err(format!("property {} is write-only", p.name));
        }
        crate::compat::require(p.since, &format!("property {}:{}", KINDS[p.kind as usize - 1].gtype, p.name))?;
        let mut buf = self.out.borrow_mut();
        *buf = StrBuf::default();
        if p.flags & F_VIRTUAL != 0 {
            *out = match slots::virtual_get(prop, &obj)? {
                Some(s) => buf.str_value(&s),
                None => convert::null_value(),
            };
            return Ok(());
        }
        let (obj, pname) = if p.flags & F_VIA != 0 { (via_object(&obj, p.via, p.name)?, p.target) } else { (obj, p.name) };
        if obj.find_property(pname).is_none() {
            return Err(format!("{} has no property {}", obj.type_().name(), pname));
        }
        let value = obj.property_value(pname);
        // Objects we did not create get a borrowed handle, minted by `Env::borrow_object`.
        self.cur_owner.set(h);
        *out = convert::from_value(&value, self, &mut buf);
        Ok(())
    }

    fn call(&self, h: u64, method: u32, args: &[svl_value], out: &mut svl_value) -> Result<(), CallErr> {
        // A VALUE handle: methods come from the typelib only.
        let value = self.state.borrow().boxed.get(&h).cloned();
        if let Some(v) = value {
            if (method as usize).wrapping_sub(1) < METHODS.len() {
                return Err("unknown method".to_string().into());
            }
            return self.call_dynamic(h, Recv::Val(v), method, args, out);
        }
        let (obj, kind) = self.obj_of(h)?;
        let Some(m) = METHODS.get((method as usize).wrapping_sub(1)) else {
            return self.call_dynamic(h, Recv::Obj(obj), method, args, out);
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

    /// A typelib method (ids past the generated `METHODS`), invoked through GIRepository. The
    /// function is looked up again on the receiver's own type when the id came from a sibling kind.
    fn call_dynamic(&self, h: u64, recv: Recv, method: u32, args: &[svl_value], out: &mut svl_value) -> Result<(), CallErr> {
        let idx = method - METHODS.len() as u32 - 1;
        let gi = self.gi()?;
        let mut dm = gi.method(idx).ok_or("unknown method")?;
        let rt = match &recv {
            Recv::Obj(o) => o.type_(),
            Recv::Val(v) => v.type_(),
        };
        if !rt.is_a(dm.declaring()) {
            dm = gi.resolve_on(rt, dm.name()).ok_or_else(|| format!("{} has no method {}", rt.name(), dm.name()))?;
        }
        self.cur_owner.set(h);
        let called = match &recv {
            Recv::Obj(o) => dm.call(o, args, self, &gi),
            Recv::Val(v) => dm.call_boxed(v, args, self, &gi),
        }
        .map_err(gi_err)?;
        self.write_called(&gi, &dm, called, h, out);
        Ok(())
    }

    /// ABI v2.5 `call_static`: a static function or constructor of `kind`.
    fn call_static(&self, kind: u32, method: u32, args: &[svl_value], out: &mut svl_value) -> Result<(), CallErr> {
        if (method as usize).wrapping_sub(1) < METHODS.len() {
            return Err("unknown static method".to_string().into());
        }
        let _ = kind;
        let idx = method - METHODS.len() as u32 - 1;
        let gi = self.gi()?;
        let dm = gi.method(idx).ok_or("unknown method")?;
        self.cur_owner.set(0);
        let called = dm.call_static(args, self, &gi).map_err(gi_err)?;
        self.write_called(&gi, &dm, called, 0, out);
        Ok(())
    }

    /// Writes a call result: the plain value, or `[ret, out...]` when the function has out parameters.
    fn write_called(&self, gi: &Gi, dm: &gi::DynMethod, called: Called, owner: u64, out: &mut svl_value) {
        let mut buf = self.out.borrow_mut();
        *buf = StrBuf::default();
        if dm.has_outs() {
            let mut items = vec![self.ret_value(gi, called.ret, &mut buf, owner)];
            for o in called.outs {
                items.push(self.ret_value(gi, o, &mut buf, owner));
            }
            *out = buf.list_value(SVL_LIST, 0, items);
        } else {
            *out = self.ret_value(gi, called.ret, &mut buf, owner);
        }
    }

    fn ret_value(&self, gi: &Gi, ret: Ret, buf: &mut StrBuf, owner: u64) -> svl_value {
        match ret {
            Ret::Null => convert::null_value(),
            Ret::Bool(b) => svl_value::new(SVL_BOOL, svl_value_u { b }),
            Ret::Int(i) => svl_value::new(SVL_I64, svl_value_u { i }),
            Ret::Float(f) => svl_value::new(SVL_F64, svl_value_u { f }),
            Ret::Str(s) => buf.str_value(&s),
            Ret::Strv(v) => {
                let items = v.iter().map(|s| buf.str_value(s)).collect();
                buf.list_value(SVL_LIST, 0, items)
            }
            Ret::Value(v) => convert::from_value(&v, self, buf),
            Ret::Object(o, owned) => match self.intern(o, owner, owned) {
                0 => convert::null_value(),
                x => svl_value::new(SVL_HANDLE, svl_value_u { h: x }),
            },
            Ret::List(items) => {
                let items = items.into_iter().map(|r| self.ret_value(gi, r, buf, owner)).collect();
                buf.list_value(SVL_LIST, 0, items)
            }
            Ret::Bytes(b) => buf.bytes_value(b),
            Ret::Gtype(t) => buf.str_value(&gi.qualified_name(t)),
        }
    }

    fn listen(self: &Rc<Self>, h: u64, signal: u32, on: bool) -> Res<()> {
        self.listen_ex(h, signal, on, 0)
    }

    /// `listen` with `SVL_LISTEN_*` flags (ABI v2.2). Each (signal, flags) is its own connection and
    /// reports itself with `flags` in the high bits of the signal id.
    ///
    /// - `AFTER`: `connect_after`, so the handler runs after the class handler.
    /// - `CAPTURE`: GTK signals have no capture phase; event controllers do. Listening in the capture
    ///   phase puts the controller itself into `PropagationPhase::Capture` for as long as any capture
    ///   listener exists (the previous phase is restored after the last). Any other object is refused.
    fn listen_ex(self: &Rc<Self>, h: u64, signal: u32, on: bool, flags: u32) -> Res<()> {
        if let Some(k) = self.menu_kind(h) {
            if menus::is_own_member(signal) {
                return self.menus_listen(h, signal, on, flags);
            } else if menus::table_name(k).is_none() {
                return Err("unknown signal".into());
            }
        }
        if vlist::is_own_member(signal) && self.extra_kind(h) == Some(vlist::KIND_VLIST) {
            self.vlist_listen(h, signal, on)?;
            return Ok(());
        }
        let (obj, kind) = self.obj_of(h)?;
        if signal >= reflect::DYN_BASE {
            return self.listen_dyn(h, &obj, signal, on, flags);
        }
        let sig = SIGNALS.get((signal as usize).wrapping_sub(1)).ok_or("unknown signal")?;
        if !lookup::in_chain(kind, sig.kind) {
            return Err(format!("signal {} does not belong to {}", sig.name, KINDS[kind as usize - 1].gtype));
        }
        if flags & !(SVL_LISTEN_AFTER | SVL_LISTEN_CAPTURE) != 0 {
            return Err(format!("unknown listener flags {flags:#x}"));
        }
        let key = signal | (flags << SVL_SIGNAL_FLAGS_SHIFT);
        let connected = self.state.borrow().entries[&h].signals.contains_key(&key);
        let is_capture = |k: &u32| (k >> SVL_SIGNAL_FLAGS_SHIFT) & SVL_LISTEN_CAPTURE != 0;
        if on && !connected {
            if flags & SVL_LISTEN_CAPTURE != 0 {
                let ctrl = obj.downcast_ref::<gtk::EventController>().ok_or_else(|| {
                    format!(
                        "capture listeners need an event controller, and {} is not one: only controllers have a propagation phase",
                        obj.type_().name()
                    )
                })?;
                let first = !self.state.borrow().entries[&h].signals.keys().any(is_capture);
                if first {
                    self.state.borrow_mut().entries.get_mut(&h).unwrap().capture_prev = Some(ctrl.propagation_phase());
                }
                ctrl.set_propagation_phase(gtk::PropagationPhase::Capture);
            }
            let weak = Rc::downgrade(self);
            let closure = glib::RustClosure::new_local(move |vals| weak.upgrade().and_then(|i| i.deliver(h, signal, flags, vals)));
            let target = if sig.via.is_empty() { obj.clone() } else { via_object(&obj, sig.via, sig.name)? };
            let id = target.connect_closure(sig.name, flags & SVL_LISTEN_AFTER != 0, closure);
            let mut st = self.state.borrow_mut();
            if !sig.via.is_empty() {
                st.via_targets.insert((h, key), target);
            }
            st.entries.get_mut(&h).unwrap().signals.insert(key, id);
        } else if !on && connected {
            let id = self.state.borrow_mut().entries.get_mut(&h).unwrap().signals.remove(&key).unwrap();
            match self.state.borrow_mut().via_targets.remove(&(h, key)) {
                Some(t) => t.disconnect(id),
                None => obj.disconnect(id),
            }
            if flags & SVL_LISTEN_CAPTURE != 0 && !self.state.borrow().entries[&h].signals.keys().any(is_capture) {
                let prev = self.state.borrow_mut().entries.get_mut(&h).unwrap().capture_prev.take();
                if let (Some(prev), Some(ctrl)) = (prev, obj.downcast_ref::<gtk::EventController>()) {
                    ctrl.set_propagation_phase(prev);
                }
            }
        }
        Ok(())
    }

    /// `listen` for a reflected signal (a non-widget object's signal found by name): events are
    /// posted with their arguments converted by GType, and the handler result is not used.
    fn listen_dyn(self: &Rc<Self>, h: u64, obj: &glib::Object, signal: u32, on: bool, flags: u32) -> Res<()> {
        if flags & !SVL_LISTEN_AFTER != 0 {
            return Err(format!("unknown listener flags {flags:#x} for a reflected signal"));
        }
        let (_, name) = self.reflect.borrow().signal(signal).cloned().ok_or("unknown signal")?;
        let key = signal | (flags << SVL_SIGNAL_FLAGS_SHIFT);
        let connected = self.state.borrow().entries[&h].signals.contains_key(&key);
        if on && !connected {
            let weak = Rc::downgrade(self);
            let closure = glib::RustClosure::new_local(move |vals| {
                if let Some(i) = weak.upgrade() {
                    i.deliver_dyn(h, key, vals);
                }
                None
            });
            let id = obj.connect_closure(&name, flags & SVL_LISTEN_AFTER != 0, closure);
            self.state.borrow_mut().entries.get_mut(&h).unwrap().signals.insert(key, id);
        } else if !on && connected {
            let id = self.state.borrow_mut().entries.get_mut(&h).unwrap().signals.remove(&key).unwrap();
            obj.disconnect(id);
        }
        Ok(())
    }

    fn deliver_dyn(&self, h: u64, wire: u32, vals: &[glib::Value]) {
        let host = self.host.load(Ordering::Acquire);
        if host.is_null() {
            return;
        }
        let mut buf = StrBuf::default();
        self.cur_owner.set(h);
        let args: Vec<svl_value> = vals.iter().skip(1).map(|v| convert::from_value(v, self, &mut buf)).collect();
        unsafe { ((*host).post_event)((*host).self_, h, wire, args.as_ptr(), args.len()) };
    }

    /// Native signal -> the host. Returns the value the signal's accumulator expects.
    ///
    /// A signal with a handler result (`handler_result != none` or `sync_required` in the generated
    /// table) goes through `host.call_sync`: the JS listeners run now and decide the result
    /// (`true` = stop, e.g. veto the close or mark the key handled). Other signals are posted and
    /// delivered at the next checkpoint. When the host answers busy (JS is running; it queued the
    /// event itself) or fails, the signal gets its deferred default: do not stop, except
    /// `close-request`, which vetoes so that the late handler can still decide (ABI v2 behaviour).
    fn deliver(&self, h: u64, signal: u32, flags: u32, vals: &[glib::Value]) -> Option<glib::Value> {
        let sig = &SIGNALS[signal as usize - 1];
        let wire = signal | (flags << SVL_SIGNAL_FLAGS_SHIFT);
        let sync = sig.handler_result != 0 || sig.sync_required;
        let host = self.host.load(Ordering::Acquire);
        let mut sync_result: Option<svl_value> = None;
        let mut deferred = false;
        if !host.is_null() {
            let mut buf = StrBuf::default();
            // Out-parameters (`gpointer` slots the handler result fills) are not arguments for JS.
            let mut args: Vec<svl_value> = vals
                .iter()
                .enumerate()
                .skip(1)
                .filter(|(i, _)| !sig.outs.iter().any(|o| o.0 as usize + 1 == *i))
                .map(|(_, v)| convert::from_value(v, self, &mut buf))
                .collect();
            // GtkEditable::changed has no arguments; the JS contract (and the Qt backend) delivers `[text]`.
            if sig.name == "changed" && args.is_empty() {
                let emitter = vals.first().and_then(|v| v.get::<glib::Object>().ok());
                if let Some(e) = emitter.clone().and_then(|o| o.downcast::<gtk::Editable>().ok()) {
                    args.push(buf.str_value(&e.text()));
                } else if let Some(b) = emitter.and_then(|o| o.downcast::<gtk::TextBuffer>().ok()) {
                    // GtkTextView `changed` is its buffer's; like an entry it carries the new text.
                    args.push(buf.str_value(&b.text(&b.start_iter(), &b.end_iter(), false)));
                }
            }
            if sync {
                let mut out = convert::null_value();
                let rc = unsafe { ((*host).call_sync)((*host).self_, h, wire, args.as_ptr(), args.len(), &mut out) };
                match rc {
                    SVL_SYNC_OK => sync_result = Some(out),
                    SVL_SYNC_BUSY => deferred = true,
                    _ => {}
                }
            } else {
                unsafe { ((*host).post_event)((*host).self_, h, wire, args.as_ptr(), args.len()) };
            }
        }
        if !sync {
            return None;
        }
        if !sig.outs.is_empty() {
            return Some(typed_result(sig, vals, sync_result.as_ref()).unwrap_or_else(|| if sig.result == "bool" { false.to_value() } else { 0i32.to_value() }));
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

impl Inner {
    /// The descriptor of a property of the handle kind `kind` (`SVL_PROP` only; other member
    /// kinds have no value descriptors yet).
    fn describe_member(&self, what: u32, kind: u32, member: u32) -> Option<MemberDesc> {
        if what == SVL_KIND {
            return self.describe_kind(member);
        }
        if what == SVL_PROP && member >= reflect::DYN_BASE {
            let r = self.reflect.borrow();
            let ps = r.prop(member)?;
            let t = r.type_of_kind(kind)?;
            if !t.is_a(ps.owner_type()) {
                return None;
            }
            let mut table = self.types.borrow_mut();
            let flags = ps.flags();
            let readable = flags.contains(glib::ParamFlags::READABLE);
            let writable = flags.contains(glib::ParamFlags::WRITABLE) && !flags.contains(glib::ParamFlags::CONSTRUCT_ONLY);
            let construct_only = flags.contains(glib::ParamFlags::WRITABLE) && flags.contains(glib::ParamFlags::CONSTRUCT_ONLY);
            let slot = types::slot_for(&mut table, ps.value_type(), Some(ps));
            let mut ret = if readable { slot.clone() } else { SlotDesc::new(SVL_NULL) };
            if !writable {
                ret.flags |= SVL_D_READONLY;
            }
            // A construct-only property is writable once: through `construct`. The descriptor keeps
            // its parameter so the host can encode it there; `setProp` refuses it with a clear error.
            let params = if writable || construct_only { vec![slot] } else { vec![] };
            return Some(MemberDesc { name: ps.name().to_string(), params, ret, flags: 0 });
        }
        let kind = if kind == vlist::KIND_VLIST && !vlist::is_own_member(member) { self.table_kind(kind) } else { kind };
        if menus::is_menu_kind(kind) {
            if menus::is_own_member(member) {
                return menus::describe_member(what, member);
            }
            if menus::table_name(kind).is_none() {
                return None;
            }
            return self.describe_member(what, self.table_kind(kind), member);
        }
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
        // A property newer than the running GTK is unavailable (no descriptor), with the reason.
        if let Err(e) = crate::compat::require(p.since, &format!("property {}:{}", KINDS[p.kind as usize - 1].gtype, p.name)) {
            self.set_error(e);
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
        let mut d = types::type_desc(&mut table, t);
        if d.kind == SVL_BOXED {
            if let Some(g) = self.gi.borrow().as_ref() {
                d.name = g.qualified_name(t);
            }
        }
        Some(d)
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
/// The typed handler result of a signal with out-parameters (ABI v2.4). `e.result` is `{ret?, <out>: v}`
/// (S2's out-param object shape) or a bare number, shorthand for `{ret: 1, <first out>: n}`; `null` or
/// anything else is no decision (the toolkit default, no out-parameter is written). `ret` is the signal's
/// own return value (bool or int); when absent it is 1 (handled) as soon as an out-parameter was written.
fn typed_result(sig: &SignalInfo, vals: &[glib::Value], reply: Option<&svl_value>) -> Option<glib::Value> {
    use glib::translate::ToGlibPtr;
    let reply = unsafe { Value::from_raw(reply?) };
    let (ret, named): (Option<i64>, Vec<(String, Value)>) = match reply {
        Value::Map(m) => {
            let mut ret = None;
            let mut rest = Vec::new();
            for (k, v) in m {
                match (k.as_str(), &v) {
                    ("ret", Value::I64(i)) => ret = Some(*i),
                    ("ret", Value::Bool(b)) => ret = Some(*b as i64),
                    _ => rest.push((k, v)),
                }
            }
            (ret, rest)
        }
        Value::I64(_) | Value::F64(_) => (None, vec![(sig.outs[0].1.to_string(), reply)]),
        _ => return None,
    };
    let mut wrote = false;
    for (idx, name, ty) in sig.outs {
        let Some((_, v)) = named.iter().find(|(k, _)| k == name) else { continue };
        let num = match v {
            Value::I64(i) => *i as f64,
            Value::F64(f) => *f,
            Value::Bool(b) => *b as i64 as f64,
            _ => continue,
        };
        let p = unsafe { glib::gobject_ffi::g_value_get_pointer(vals.get(*idx as usize + 1)?.to_glib_none().0) };
        if p.is_null() {
            continue;
        }
        unsafe {
            match *ty {
                "f64" => *(p as *mut f64) = num,
                "i32" => *(p as *mut i32) = num as i32,
                "u32" => *(p as *mut u32) = num as u32,
                "i64" => *(p as *mut i64) = num as i64,
                "bool" => *(p as *mut glib::ffi::gboolean) = (num != 0.0) as glib::ffi::gboolean,
                _ => continue,
            }
        }
        wrote = true;
    }
    let ret = ret.unwrap_or(wrote as i64);
    Some(match sig.result {
        "bool" => (ret != 0).to_value(),
        _ => (ret as i32).to_value(),
    })
}

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
            reflect: RefCell::new(Reflect::default()),
            descs: DescStore::default(),
            wake_timer: RefCell::new(None),
            frame_pending: Cell::new(false),
            frame_clock: RefCell::new(None),
            frame_timer: RefCell::new(None),
            frames_by_clock: Cell::new(0),
            files: RefCell::new(HashMap::new()),
            vlists: RefCell::new(HashMap::new()),
            menus: RefCell::new(HashMap::new()),
            style: RefCell::default(),
            style_flush_queued: Cell::new(false),
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
            request_frame: b_request_frame,
            set_child_attr: Some(b_set_child_attr),
            get_child_attr: Some(b_get_child_attr),
            listen_ex: Some(b_listen_ex),
            style: Some(b_style),
            construct: Some(b_construct),
            call_static: Some(b_call_static),
        });
        Ok(GtkBackend { vtable, ctl })
    }

    /// CSS parse errors reported by the styling providers since the last call.
    pub fn take_style_errors(&self) -> Vec<String> {
        self.ctl.local.take_style_errors()
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

    /// Handles currently in the table (owned, toplevel and borrowed): the leak check of the tests.
    pub fn live_handles(&self) -> usize {
        self.ctl.local.state.borrow().entries.len()
    }

    /// Diagnostic access to the object behind a handle.
    pub fn object(&self, h: u64) -> Option<glib::Object> {
        self.ctl.local.obj_of(h).ok().map(|x| x.0)
    }

    /// Test hook: the name of a signal id, generated (`clicked`) or reflected (`notify::text`).
    pub fn signal_name(&self, id: u32) -> String {
        let id = id & SVL_SIGNAL_ID_MASK;
        if let Some(s) = SIGNALS.get((id as usize).wrapping_sub(1)) {
            return s.name.to_string();
        }
        self.ctl.local.reflect.borrow().signal(id).map(|(_, n)| n.clone()).unwrap_or_else(|| format!("#{id}"))
    }

    /// Test hook: the items of a menu container as text (`a,b,{c,d},Sub[e],`).
    pub fn menu_dump(&self, h: u64) -> String {
        self.ctl.local.menu_dump(h)
    }

    /// Test hook: the first menu node of a kind (`action`, `menu`, `menubar`, ...) with this label.
    pub fn menu_find(&self, kind: &str, label: &str) -> Option<u64> {
        self.ctl.local.menu_find(kind, label)
    }

    /// Test hook: the root action group of a menu node.
    pub fn menu_group(&self, h: u64) -> Option<gtk::gio::SimpleActionGroup> {
        self.ctl.local.menu_group(h)
    }

    /// Test hook: the full action name (`svl.a5`) behind an action node.
    pub fn menu_action_name(&self, h: u64) -> Option<String> {
        self.ctl.local.menu_action_name(h)
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

fn resolve_generic(p: *mut c_void, what: u32, kind: svl_id, name: &str) -> svl_id {
    match what {
        SVL_PROP => find_prop(p, kind, name),
        SVL_SIGNAL => find_signal(p, kind, name),
        SVL_METHOD => match lookup::find_method(kind, name) {
            0 => unsafe { ctl(p) }.local.resolve_dynamic(kind, name),
            id => id,
        },
        SVL_SLOT => lookup::find_slot(kind, name),
        _ => 0,
    }
}

/// A generated property id, else a reflected one (non-widget types only).
fn find_prop(p: *mut c_void, kind: svl_id, name: &str) -> svl_id {
    match lookup::find_prop(kind, name) {
        0 => unsafe { ctl(p) }.local.reflect.borrow_mut().find_prop(kind, name),
        id => id,
    }
}

fn find_signal(p: *mut c_void, kind: svl_id, name: &str) -> svl_id {
    match lookup::find_signal(kind, name) {
        0 => unsafe { ctl(p) }.local.reflect.borrow_mut().find_signal(kind, name),
        id => id,
    }
}

unsafe extern "C" fn b_resolve(p: *mut c_void, what: u32, kind: svl_id, name: *const c_char, len: usize) -> svl_id {
    let bytes = unsafe { std::slice::from_raw_parts(name as *const u8, len) };
    let Ok(name) = std::str::from_utf8(bytes) else { return 0 };
    match what {
        SVL_KIND if name == "virtual-list" => vlist::KIND_VLIST,
        SVL_KIND => match (menus::kind_by_name(name), extras::kind_by_name(name)) {
            (0, 0) => match lookup::kind_by_name(name) {
                0 => unsafe { ctl(p) }.local.resolve_kind_native(name),
                k => k,
            },
            (0, k) | (k, _) => k,
        },
        _ if menus::is_menu_kind(kind) => match menus::find_member(what, kind, name) {
            0 if menus::table_name(kind).is_some() => {
                let tk = unsafe { ctl(p) }.local.table_kind(kind);
                match what {
                    SVL_ATTR => lookup::find_attr(tk, name),
                    _ => resolve_generic(p, what, tk, name),
                }
            }
            id => id,
        },
        _ if kind == vlist::KIND_VLIST => match vlist::find_member(what, name) {
            0 => resolve_generic(p, what, unsafe { ctl(p) }.local.table_kind(kind), name),
            id => id,
        },
        _ if extras::is_extra(kind) => extras::find_member(what, kind, name),
        SVL_PROP => find_prop(p, kind, name),
        SVL_SIGNAL => find_signal(p, kind, name),
        SVL_METHOD => match lookup::find_method(kind, name) {
            0 => unsafe { ctl(p) }.local.resolve_dynamic(kind, name),
            id => id,
        },
        SVL_SLOT => lookup::find_slot(kind, name),
        SVL_ATTR => lookup::find_attr(kind, name),
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
        if let Some(k) = c.local.menu_kind(h).filter(|k| menus::is_own_member(method) || menus::table_name(*k).is_none()) {
            *out = convert::null_value();
            let _ = k;
            c.local.menus_call(h, method, extras::args_of(args, n)).map_err(CallErr::Plain)
        } else if c.local.extra_kind(h) == Some(vlist::KIND_VLIST) && vlist::is_own_member(method) {
            c.local.vlist_call(h, method, extras::args_of(args, n), out)
        } else if c.local.extra_kind(h).is_some_and(|k| k != vlist::KIND_VLIST) {
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

unsafe extern "C" fn b_construct(p: *mut c_void, kind: svl_id, props: *const svl_id, values: *const svl_value, n: usize, out: *mut svl_value) -> c_int {
    let (props, values): (&[svl_id], &[svl_value]) =
        if n == 0 { (&[], &[]) } else { unsafe { (std::slice::from_raw_parts(props, n), std::slice::from_raw_parts(values, n)) } };
    run_int(unsafe { ctl(p) }, |i| {
        let h = i.construct(kind, props, values)?;
        unsafe { *out = svl_value::new(SVL_HANDLE, svl_value_u { h }) };
        Ok(())
    })
}

unsafe extern "C" fn b_call_static(p: *mut c_void, kind: svl_id, method: svl_id, args: *const svl_value, n: usize, out: *mut svl_value) -> c_int {
    let c = unsafe { ctl(p) };
    let _t = crate::timing::scope();
    let out = unsafe { &mut *out };
    let r = catch_unwind(AssertUnwindSafe(|| c.local.call_static(kind, method, extras::args_of(args, n), out)));
    match r {
        Ok(Ok(())) => 0,
        Ok(Err(CallErr::Plain(e))) => {
            c.local.set_error(e);
            -1
        }
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

unsafe extern "C" fn b_listen_ex(p: *mut c_void, h: svl_handle, signal: svl_id, on: bool, flags: u32) -> c_int {
    run_int(unsafe { ctl(p) }, |i| i.listen_ex(h, signal, on, flags))
}

unsafe extern "C" fn b_style(p: *mut c_void, op: u32, h: svl_handle, v: *const svl_value) -> c_int {
    let c = unsafe { ctl(p) };
    let text: Option<String> = if v.is_null() || unsafe { (*v).tag } != SVL_STR {
        None
    } else {
        unsafe { convert::as_str(&*v) }.map(|s| s.into_owned())
    };
    match catch_unwind(AssertUnwindSafe(|| c.local.style_op(op, h, text.as_deref()))) {
        Ok(Ok(n)) => n.max(0),
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

unsafe extern "C" fn b_set_child_attr(p: *mut c_void, parent: svl_handle, child: svl_handle, attr: svl_id, v: *const svl_value) -> c_int {
    if v.is_null() {
        return run_int(unsafe { ctl(p) }, |_| Err("set_child_attr: null value".into()));
    }
    run_int(unsafe { ctl(p) }, |i| i.set_child_attr(parent, child, attr, unsafe { &*v }))
}

unsafe extern "C" fn b_get_child_attr(p: *mut c_void, parent: svl_handle, child: svl_handle, attr: svl_id, out: *mut svl_value) -> c_int {
    run_int(unsafe { ctl(p) }, |i| i.get_child_attr(parent, child, attr, unsafe { &mut *out }))
}

unsafe extern "C" fn b_handle_info(p: *mut c_void, h: svl_handle, out: *mut svl_handle_info) -> c_int {
    run_int(unsafe { ctl(p) }, |i| {
        let st = i.state.borrow();
        // A boxed value is a VALUE handle: refcounted and immutable, released by `destroy`.
        if let Some(v) = st.boxed.get(&h) {
            let kind = i.reflect.borrow_mut().kind_for_type(v.type_());
            unsafe { *out = svl_handle_info { kind, lifetime: SVL_LIFE_VALUE, flags: 0 } };
            return Ok(());
        }
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

/// The object property `via` of `obj` returns (a text view's buffer, a range's adjustment), for a
/// property or signal `what` that is forwarded to it.
fn via_object(obj: &glib::Object, via: &str, what: &str) -> Res<glib::Object> {
    if obj.find_property(via).is_none() {
        return Err(format!("{} has no property {via}, which {what} is forwarded to", obj.type_().name()));
    }
    obj.property_value(via)
        .get::<Option<glib::Object>>()
        .map_err(|e| format!("{via}: {e}"))?
        .ok_or_else(|| format!("{} has no {via} to forward {what} to", obj.type_().name()))
}
