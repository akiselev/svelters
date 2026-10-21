//! ABI v2.6 `observe`: watching a property natively through `notify::<name>`.
//!
//! While (handle, prop) is observed, every `notify` of the property calls `host.prop_changed(h,
//! prop)`, with no value: the host coalesces, reads, compares and batches (REACTIVITY.md §5.2). Any
//! GObject handle can be observed, widgets (generated property ids) and non-widget objects
//! (reflected ids) alike.
//!
//! Facts (milestone R1): the observe facts of `metadata/observe/gtk.json` are in the generated
//! `OBSERVE` table, keyed by (declaring GType, property). `describe` reports `SVL_M_OBSERVABLE` only
//! for an `exact` or `partial` fact (a `constant`, `set_only` or `none` fact is not observable; a
//! property without a fact keeps the mechanical rule: readable and not construct-only), and
//! `SVL_M_PROPORTIONAL` for `value_cost: proportional`. `observe` itself still installs `notify` on
//! any readable, non-construct-only property: the facts' verification rows rely on it.
//!
//! Forwarded properties (`via`, rule O9: GtkRange `value` lives on its adjustment) are watched on the
//! object they are forwarded to, and re-targeted: the element's own `notify::<via>` (`adjustment`)
//! moves the handler to the new object and reports the property changed (its value comes from the
//! new object now).
//!
//! Echo (REACTIVITY.md §5.4): the host drops the echo of its own writes by comparing with the value
//! it read back, except for proportional properties, which it never reads. For those the backend
//! drops the `notify` of the (handle, prop) it is writing in `set_prop` ([`EchoGuard`]); other
//! properties notified by the same write, and other handles observing the same object, still report.
//!
//! Ownership: calls nest, so each (handle, prop) has one connection and a count. The connection is
//! stored in the handle's entry and disconnected by the matching last `observe(off)` and by
//! `release_entry` (destroy, native death of a parent window). Handlers capture only ids, the host
//! pointer cell and (for `via`) a weak reference to the current target: never the element, never a
//! JS value, so an observation keeps nothing alive.
//!
//! Relation to `listen('notify::<prop>')`: that is an ordinary signal listener (reflected kinds resolve
//! the detail in `Reflect::find_signal`, generated kinds through the tables) and posts an event with
//! the `GParamSpec` argument. `observe` is separate on purpose: it works the same for widgets,
//! non-widgets and forwarded properties, reports through `prop_changed` instead of `post_event`, and
//! needs a `Send` handler. Both use the property's canonical name as the detail.
//!
//! Threads: `notify` is emitted on the thread that changed the object. On the backend's thread the
//! host is called synchronously; from any other thread the call is moved to the default main
//! context with an idle (plans/research/REACTIVITY.md §5.6).

use super::*;
use std::sync::Mutex;

/// The current target of a forwarded observation: the object and the handler connected on it. The
/// weak references are made, upgraded and dropped on the main thread only (handlers hop there first).
type ViaSlot = Arc<Mutex<Option<(glib::SendWeakRef<glib::Object>, glib::SignalHandlerId)>>>;

enum Target {
    /// `notify::<prop>` on the element itself.
    Own(glib::SignalHandlerId),
    /// `notify::<target>` on the forwarded-to object (`cur`), plus `notify::<via>` on the element
    /// (`retarget`), which moves the first one when the element's `via` property changes.
    Via { retarget: glib::SignalHandlerId, cur: ViaSlot },
}

pub(super) struct Observer {
    target: Target,
    count: u32,
}

/// Disconnects every observation of an entry (destroy, native death).
pub(super) fn disconnect_all(e: &mut Entry, obj: &glib::Object) {
    for (_, o) in e.observers.drain() {
        disconnect(o, obj);
    }
}

fn disconnect(o: Observer, obj: &glib::Object) {
    match o.target {
        Target::Own(id) => obj.disconnect(id),
        Target::Via { retarget, cur } => {
            obj.disconnect(retarget);
            drop_via(&cur);
        }
    }
}

/// Disconnects the handler on the current forwarded-to object, if it is still alive.
fn drop_via(cur: &ViaSlot) {
    let old = cur.lock().unwrap_or_else(|e| e.into_inner()).take();
    // A forwarded-to object that is already gone took its handlers with it.
    if let Some((w, id)) = old {
        if let Some(t) = w.upgrade() {
            t.disconnect(id);
        }
    }
}

// ---- facts ------------------------------------------------------------------------------------

/// `SVL_PROP` descriptor flags from the observe fact of `prop` declared by `owner` (a GType name).
/// Without a fact, `mechanical` (the property can be watched through `notify`) decides.
pub(super) fn member_flags(owner: &str, prop: &str, mechanical: bool) -> u32 {
    let f = observe_fact(owner, prop);
    let observable = match f & O_CLASS {
        0 => mechanical,
        O_EXACT | O_PARTIAL => mechanical,
        _ => false,
    };
    let mut flags = if observable { SVL_M_OBSERVABLE } else { 0 };
    if f & O_PROPORTIONAL != 0 {
        flags |= SVL_M_PROPORTIONAL;
    }
    flags
}

// ---- echo suppression for proportional properties (REACTIVITY.md §5.4) ------------------------

thread_local! {
    /// The (handle, prop) the backend is writing in `set_prop`, while that prop is proportional.
    static WRITING: Cell<Option<(u64, u32)>> = const { Cell::new(None) };
}

/// Held around the backend's own `set_property` of a proportional property: the `notify` of that
/// (handle, prop) is not reported to the host.
pub(super) struct EchoGuard(Option<Option<(u64, u32)>>);

impl EchoGuard {
    pub(super) fn for_write(h: u64, prop: u32, owner: &str, name: &str) -> EchoGuard {
        if observe_fact(owner, name) & O_PROPORTIONAL == 0 {
            return EchoGuard(None);
        }
        EchoGuard(Some(WRITING.with(|w| w.replace(Some((h, prop))))))
    }
}

impl Drop for EchoGuard {
    fn drop(&mut self) {
        if let Some(prev) = self.0 {
            WRITING.with(|w| w.set(prev));
        }
    }
}

/// Whether `notify` of (h, prop) is the echo of our own write of a proportional property.
fn is_own_echo(h: u64, prop: u32) -> bool {
    WRITING.with(|w| w.get() == Some((h, prop)))
}

// ---- reporting --------------------------------------------------------------------------------

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

/// What a handler needs to report (h, prop): ids and the host cell only, so it is `Send`.
#[derive(Clone)]
struct Reporter {
    host: Arc<AtomicPtr<svl_host>>,
    main: std::thread::ThreadId,
    h: u64,
    prop: u32,
}

impl Reporter {
    /// Runs `f` on the main thread: now when already there, else from an idle.
    fn on_main(&self, f: impl FnOnce(&Reporter) + Send + 'static) {
        if std::thread::current().id() == self.main {
            f(self);
        } else {
            let r = self.clone();
            glib::idle_add_once(move || f(&r));
        }
    }

    fn report(&self) {
        self.on_main(|r| {
            if !is_own_echo(r.h, r.prop) {
                notify_host(&r.host, r.h, r.prop);
            }
        });
    }

    /// `notify::<name>` on `target`, reporting (h, prop).
    fn connect(&self, target: &glib::Object, name: &str) -> glib::SignalHandlerId {
        let r = self.clone();
        // `connect_notify` (not `_local`): a notify from another thread must not panic in the guard,
        // so the closure is Send and carries only ids and the host cell.
        target.connect_notify(Some(name), move |_, _| r.report())
    }
}

type ObsRes<T> = Result<T, (c_int, String)>;

fn failed(msg: impl Into<String>) -> (c_int, String) {
    (-1, msg.into())
}

fn unsupported(msg: impl Into<String>) -> (c_int, String) {
    (SVL_OBSERVE_UNSUPPORTED, msg.into())
}

/// Where an observation connects.
enum Watch {
    /// The element's own property (canonical name).
    Own(glib::Object, String),
    /// A forwarded property: the element, its `via` property, the forwarded-to object and the
    /// target property's canonical name.
    Via { elem: glib::Object, via: &'static str, target: glib::Object, name: String },
}

impl Inner {
    /// The object(s) and the GObject property name `notify` reports `prop` of `h` under.
    fn observed_target(&self, h: u64, prop: u32) -> ObsRes<Watch> {
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
            (obj.clone(), ps.name().to_string(), None)
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
                (via_object(&obj, p.via, p.name).map_err(failed)?, p.target.to_string(), Some(p.via))
            } else {
                (obj.clone(), p.name.to_string(), None)
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
        let name = ps.name().to_string();
        Ok(match via {
            Some(via) => Watch::Via { elem: obj, via, target, name },
            None => Watch::Own(target, name),
        })
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
            match obj {
                Some(obj) => disconnect(o, &obj),
                // The element is gone: its own handlers went with it, a forwarded target may not have.
                None => {
                    if let Target::Via { cur, .. } = &o.target {
                        drop_via(cur);
                    }
                }
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
        let r = Reporter { host: self.host.clone(), main: self.main_thread, h, prop };
        let target = match self.observed_target(h, prop)? {
            Watch::Own(obj, name) => Target::Own(r.connect(&obj, &name)),
            Watch::Via { elem, via, target, name } => {
                let id = r.connect(&target, &name);
                let cur: ViaSlot = Arc::new(Mutex::new(Some((target.downgrade().into(), id))));
                let retarget = {
                    let (r, cur) = (r.clone(), cur.clone());
                    let ew: Arc<glib::SendWeakRef<glib::Object>> = Arc::new(elem.downgrade().into());
                    let name: Arc<str> = name.into();
                    elem.connect_notify(Some(via), move |_, _| {
                        // The element's `via` object changed: watch the new one, and report the
                        // property (its value now comes from the new object).
                        let (cur, ew, name) = (cur.clone(), ew.clone(), name.clone());
                        r.on_main(move |r| {
                            drop_via(&cur);
                            let Some(elem) = ew.upgrade() else { return };
                            let new = elem.property_value(via).get::<Option<glib::Object>>().ok().flatten();
                            if let Some(t) = new {
                                let id = r.connect(&t, &name);
                                *cur.lock().unwrap_or_else(|e| e.into_inner()) = Some((t.downgrade().into(), id));
                            }
                            notify_host(&r.host, r.h, r.prop);
                        });
                    })
                };
                Target::Via { retarget, cur }
            }
        };
        let observer = Observer { target, count: 1 };
        self.state.borrow_mut().entries.get_mut(&h).unwrap().observers.insert(prop, observer);
        Ok(())
    }
}
