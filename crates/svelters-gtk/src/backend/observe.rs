//! ABI v2.6 `observe`: watching a property natively through `notify::<name>`.
//!
//! Mechanics only (plans/research/REACTIVITY.md §12 decides the semantics): while (handle, prop) is
//! observed, every `notify` of the property calls `host.prop_changed(h, prop)`, with no value and no
//! comparison. Any GObject handle can be observed, widgets (generated property ids) and non-widget
//! objects (reflected ids) alike; a forwarded property (`via`, GtkRange `value` lives on its
//! adjustment) is watched on the object it is forwarded to, as that object was when observation
//! started.
//!
//! Ownership: calls nest, so each (handle, prop) has one connection and a count. The connection is
//! stored in the handle's entry and disconnected by the matching last `observe(off)` and by
//! `release_entry` (destroy, native death of a parent window). The handler captures only ids and the
//! host pointer cell: never the object, never a JS value, so an observation keeps nothing alive. A
//! `via` target is held weakly.
//!
//! Threads: `notify` is emitted on the thread that changed the object. On the backend's thread the
//! host is called synchronously; from any other thread the call is moved to the default main
//! context with an idle (plans/research/REACTIVITY.md §5.6).

use super::*;

pub(super) struct Observer {
    /// The object the handler is connected on when it is not the element itself (`via`).
    via: Option<glib::WeakRef<glib::Object>>,
    id: glib::SignalHandlerId,
    count: u32,
}

/// Disconnects every observation of an entry (destroy, native death).
pub(super) fn disconnect_all(e: &mut Entry, obj: &glib::Object) {
    for (_, o) in e.observers.drain() {
        disconnect(o, obj);
    }
}

fn disconnect(o: Observer, obj: &glib::Object) {
    match o.via {
        // A forwarded-to object that is already gone took its handlers with it.
        Some(w) => {
            if let Some(t) = w.upgrade() {
                t.disconnect(o.id);
            }
        }
        None => obj.disconnect(o.id),
    }
}

/// `host.prop_changed(h, prop)` if a host is attached (only while `run` is on the stack, or after
/// `attach_host`).
fn notify_host(host: &AtomicPtr<svl_host>, h: u64, prop: u32) {
    let p = host.load(Ordering::Acquire);
    if p.is_null() {
        return;
    }
    if let Some(f) = unsafe { (*p).prop_changed } {
        unsafe { f((*p).self_, h, prop) };
    }
}

type ObsRes<T> = Result<T, (c_int, String)>;

fn failed(msg: impl Into<String>) -> (c_int, String) {
    (-1, msg.into())
}

fn unsupported(msg: impl Into<String>) -> (c_int, String) {
    (SVL_OBSERVE_UNSUPPORTED, msg.into())
}

impl Inner {
    /// The object and the GObject property name `notify` reports `prop` of `h` under.
    fn observed_target(&self, h: u64, prop: u32) -> ObsRes<(glib::Object, String, bool)> {
        if let Some(k) = self.menu_kind(h) {
            if menus::is_own_member(prop) || menus::table_name(k).is_none() {
                return Err(unsupported("menu and action properties cannot be observed"));
            }
        }
        match self.extra_kind(h) {
            Some(vlist::KIND_VLIST) if vlist::is_own_member(prop) => {
                return Err(unsupported("virtual-list properties cannot be observed"));
            }
            Some(vlist::KIND_VLIST) | None => {}
            Some(_) => return Err(unsupported("properties of this service kind cannot be observed")),
        }
        let (obj, kind) = self.obj_of(h).map_err(failed)?;
        let (target, name, via) = if prop >= reflect::DYN_BASE {
            let ps = self.reflect.borrow().prop(prop).cloned().ok_or_else(|| failed("unknown property"))?;
            if !obj.type_().is_a(ps.owner_type()) {
                return Err(failed(format!("property {} does not belong to {}", ps.name(), obj.type_().name())));
            }
            (obj, ps.name().to_string(), false)
        } else {
            let p = PROPS.get((prop as usize).wrapping_sub(1)).ok_or_else(|| failed("unknown property"))?;
            if !lookup::in_chain(kind, p.kind) {
                return Err(failed(format!("property {} does not belong to {}", p.name, KINDS[kind as usize - 1].gtype)));
            }
            crate::compat::require(p.since, &format!("property {}:{}", KINDS[p.kind as usize - 1].gtype, p.name)).map_err(failed)?;
            if p.flags & F_VIRTUAL != 0 {
                return Err(unsupported(format!("{} is computed by svelters, not a GObject property: it has no notify", p.name)));
            }
            if p.flags & F_VIA != 0 {
                (via_object(&obj, p.via, p.name).map_err(failed)?, p.target.to_string(), true)
            } else {
                (obj, p.name.to_string(), false)
            }
        };
        let ps = target
            .find_property(&name)
            .ok_or_else(|| failed(format!("{} has no property {name}", target.type_().name())))?;
        if !ps.flags().contains(glib::ParamFlags::READABLE) {
            return Err(failed(format!("property {name} is write-only")));
        }
        if ps.flags().contains(glib::ParamFlags::CONSTRUCT_ONLY) {
            return Err(failed(format!("property {name} is construct-only: it never changes")));
        }
        // `notify` details use the canonical name.
        Ok((target, ps.name().to_string(), via))
    }

    /// ABI v2.6 `observe`. `Err((rc, message))` with rc `SVL_OBSERVE_UNSUPPORTED` or -1.
    pub(super) fn observe(&self, h: u64, prop: u32, on: bool) -> ObsRes<()> {
        let existing = self.state.borrow().entries.get(&h).map(|e| e.observers.contains_key(&prop));
        if !on {
            // `off` for a dead handle or an unobserved property: nothing to do.
            let Some(true) = existing else { return Ok(()) };
            let mut st = self.state.borrow_mut();
            let e = st.entries.get_mut(&h).unwrap();
            let o = e.observers.get_mut(&prop).unwrap();
            o.count -= 1;
            if o.count > 0 {
                return Ok(());
            }
            let o = e.observers.remove(&prop).unwrap();
            let obj = e.obj.upgrade();
            drop(st);
            if let Some(obj) = obj {
                disconnect(o, &obj);
            }
            return Ok(());
        }
        match existing {
            None if self.state.borrow().boxed.contains_key(&h) => return Err(failed("a VALUE handle has no observable properties")),
            None => return Err(failed(format!("stale or foreign handle {h}"))),
            Some(true) => {
                self.state.borrow_mut().entries.get_mut(&h).unwrap().observers.get_mut(&prop).unwrap().count += 1;
                return Ok(());
            }
            Some(false) => {}
        }
        let (target, name, via) = self.observed_target(h, prop)?;
        let host = self.host.clone();
        let main = self.main_thread;
        // `connect_notify` (not `_local`): a notify from another thread must not panic in the guard,
        // so the closure is Send and carries only ids and the host cell.
        let id = target.connect_notify(Some(&name), move |_, _| {
            if std::thread::current().id() == main {
                notify_host(&host, h, prop);
            } else {
                let host = host.clone();
                glib::idle_add_once(move || notify_host(&host, h, prop));
            }
        });
        let observer = Observer { via: via.then(|| target.downgrade()), id, count: 1 };
        self.state.borrow_mut().entries.get_mut(&h).unwrap().observers.insert(prop, observer);
        Ok(())
    }
}
