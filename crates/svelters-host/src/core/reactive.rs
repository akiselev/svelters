//! Reactive exports on the host (E3, `docs/notes/egui-rust-exports.md` §6.4, §6.6, §8): stores join R1's
//! dirty set, `Emitter` events join the event queue, and `persist` keeps objects across hot reloads.
//!
//! - **Stores.** `host.observe(h, field, true)` on a store object subscribes this host to the store (one
//!   [`StoreSub`] per handle: a dirty mask per 64 fields) and refcounts (h, field) in the same `observed`
//!   table as native props, with the field's current value as the baseline. Writers on any thread set
//!   bits and wake the host once ([`Hub`]). At the checkpoint, [`collect`] folds the dirty fields that are
//!   observed into the dirty set; `take_changes` reads them (the getter: a clone under the read lock),
//!   compares them with `last` and delivers the changed ones in the one `__svelters_changed` batch.
//!   JS writes go through `setProp` → the generated setter (`on_write`) → R1's read-back (`after_set`), so
//!   a normalized value is delivered once and the echo of the write is dropped.
//! - **Events.** `host.listen(h, event, true)` on an object with an `Emitter` subscribes this host to it.
//!   [`collect`] converts the queued payloads (on the JS thread) into ordinary signal events, delivered
//!   after the change batch, in order.
//! - **Reload.** The old runtime's observations and listeners are set aside while the new bundle loads
//!   ([`Core::reactive_stash`]): dropped if it loads, restored if it fails. Store subscriptions and
//!   persisted objects stay.

use super::*;
use crate::exports::reactive::{EmitSub, Hub, HubEvent, StoreSub};
use crate::exports::{self, AnyObj, CallCx, schema};
use std::any::TypeId;
use std::panic::{AssertUnwindSafe, catch_unwind};

/// Above this many events for one object in one checkpoint, debug builds warn (once per object).
const EVENT_FLOOD: u32 = 1000;

/// Counters of the reactive path (see [`crate::Host::reactive_stats`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReactiveStats {
    /// Checkpoints that collected store or event work.
    pub collects: u64,
    /// Dirty observed store fields folded into the dirty set.
    pub store_marks: u64,
    /// Store fields delivered in a `__svelters_changed` batch.
    pub store_delivered: u64,
    /// Dirty store fields the compare dropped (unchanged values, the echo of our own write).
    pub store_unchanged: u64,
    /// Change batches that carried at least one store field.
    pub store_batches: u64,
    /// `Emitter` events queued for JS.
    pub events: u64,
    /// Events dropped: their object was released or no longer listened.
    pub events_dropped: u64,
}

pub(crate) struct Persisted {
    kind: svl_id,
    ty: TypeId,
    obj: AnyObj,
}

pub(crate) struct Reactive {
    pub(crate) hub: Arc<Hub>,
    stores: RefCell<HashMap<svl_handle, Arc<StoreSub>>>,
    emits: RefCell<HashMap<svl_handle, Arc<EmitSub>>>,
    persist: RefCell<HashMap<String, Persisted>>,
    stats: Cell<ReactiveStats>,
    warned: RefCell<HashSet<svl_handle>>,
}

impl Reactive {
    pub(crate) fn new(hub: Arc<Hub>) -> Reactive {
        Reactive {
            hub,
            stores: RefCell::default(),
            emits: RefCell::default(),
            persist: RefCell::default(),
            stats: Cell::new(ReactiveStats::default()),
            warned: RefCell::default(),
        }
    }

    fn stat(&self, f: impl FnOnce(&mut ReactiveStats)) {
        let mut s = self.stats.get();
        f(&mut s);
        self.stats.set(s);
    }
}

/// The old runtime's observations and listeners on app objects, while a reload loads the new one.
pub(crate) struct ReactiveStash {
    observed: Vec<((svl_handle, svl_id), Observed)>,
    listened: Vec<(svl_handle, u64)>,
}

fn disposed(h: svl_handle) -> OpError {
    let msg = format!("app object #a{} was disposed", h & !exports::APP_HANDLE_BIT);
    OpError { kind: OpKind::Error, msg: msg.clone(), native: Some(exports::host_error("disposed", msg)) }
}

impl Core {
    pub(crate) fn reactive_stats(&self) -> ReactiveStats {
        self.reactive.stats.get()
    }

    /// `host.observe(h, prop, on)` on an app object: a store field is observable (`exact`); any other
    /// app prop answers `false` (a plain read).
    pub(crate) fn app_observe(&self, h: svl_handle, prop: Name<'_>, on: bool) -> Result<bool, OpError> {
        let kind = match self.apps.kind(h) {
            Some(k) => k,
            None if !on => return Ok(false),
            None => return Err(disposed(h)),
        };
        let Some(store) = schema().class(kind).and_then(|c| c.store.as_ref()) else { return Ok(false) };
        let p = match prop {
            Name::Str(s) => schema().resolve(NameKind::Prop, kind, s),
            Name::Id(i) => i,
        };
        let Some((_, field)) = store.field_of_prop(p) else { return Ok(false) };
        let count = self.st.borrow().observed.get(&(h, p)).map_or(0, |o| o.count);
        if on {
            if count > 0 {
                self.st.borrow_mut().observed.get_mut(&(h, p)).unwrap().count += 1;
                return Ok(true);
            }
            self.store_sub(h, kind)?;
            let name = match prop {
                Name::Str(s) => s.to_string(),
                Name::Id(_) => field.name.to_string(),
            };
            let last = if field.proportional { None } else { self.read_last(h, p) };
            let o = Observed { count: 1, name, proportional: field.proportional, last, force: false };
            self.st.borrow_mut().observed.insert((h, p), o);
            Ok(true)
        } else {
            match count {
                0 => Ok(false),
                1 => {
                    self.st.borrow_mut().observed.remove(&(h, p));
                    Ok(false)
                }
                _ => {
                    self.st.borrow_mut().observed.get_mut(&(h, p)).unwrap().count -= 1;
                    Ok(true)
                }
            }
        }
    }

    /// This host's subscription to the store behind `h` (made on first use).
    fn store_sub(&self, h: svl_handle, kind: svl_id) -> Result<Arc<StoreSub>, OpError> {
        if let Some(s) = self.reactive.stores.borrow().get(&h) {
            return Ok(s.clone());
        }
        let store = schema().class(kind).and_then(|c| c.store.as_ref()).ok_or_else(|| OpError::from(format!("app object #a{} is not a store", h & !exports::APP_HANDLE_BIT)))?;
        let (_, _, obj) = self.apps.object(h).ok_or_else(|| disposed(h))?;
        let sub = StoreSub::new(h, store.fields.len(), self.reactive.hub.clone());
        if !(store.subscribe)(&obj, &sub) {
            return Err(OpError::from(format!("app object #a{} is not the store its class says", h & !exports::APP_HANDLE_BIT)));
        }
        self.reactive.stores.borrow_mut().insert(h, sub.clone());
        Ok(sub)
    }

    /// `host.listen(h, event, on)` on an app object with an `Emitter`.
    pub(crate) fn app_listen(&self, h: svl_handle, signal: Name<'_>, on: bool) -> Result<(), OpError> {
        let kind = match self.apps.kind(h) {
            Some(k) => k,
            None if !on => return Ok(()),
            None => return Err(disposed(h)),
        };
        let Some(class) = schema().class(kind) else {
            return Err(OpError { kind: OpKind::Type, msg: format!("{} has no events", schema().kind_name(kind).unwrap_or_default()), native: None });
        };
        let index = match signal {
            Name::Str(s) => class.events.iter().position(|e| e.name == s),
            Name::Id(i) => (i as usize).checked_sub(1).filter(|i| *i < class.events.len()),
        };
        let Some(index) = index else {
            if !on {
                return Ok(());
            }
            let name = match signal {
                Name::Str(s) => s.to_string(),
                Name::Id(i) => format!("#{i}"),
            };
            let have: Vec<&str> = class.events.iter().map(|e| e.name).collect();
            let msg = if have.is_empty() {
                format!("{} has no events (listen '{name}'): give it an `Emitter<E>` field", class.native)
            } else {
                format!("{} has no event '{name}' (events: {})", class.native, have.join(", "))
            };
            return Err(OpError { kind: OpKind::Type, msg, native: None });
        };
        if !on {
            if let Some(sub) = self.reactive.emits.borrow().get(&h) {
                sub.listen(index, false);
            }
            return Ok(());
        }
        let sub = match self.reactive.emits.borrow().get(&h).cloned() {
            Some(s) => s,
            None => {
                let get = class.emitter.ok_or_else(|| OpError::from(format!("{} has no emitter", class.native)))?;
                let (_, _, obj) = self.apps.object(h).ok_or_else(|| disposed(h))?;
                let em = get(&obj).ok_or_else(|| OpError::from(format!("{} is in use (re-entrant listen)", class.native)))?;
                let sub = EmitSub::new(h, self.reactive.hub.clone());
                em.subscribe(&sub);
                self.reactive.emits.borrow_mut().insert(h, sub.clone());
                sub
            }
        };
        sub.listen(index, true);
        self.st.borrow_mut().signal_names.insert((kind, index as svl_id + 1), class.events[index].name.to_string());
        Ok(())
    }

    /// The current value of an app prop (a getter call), for the compare of `take_changes` and the
    /// read-back of the set path.
    pub(crate) fn app_prop_value(&self, h: svl_handle, p: svl_id) -> Option<Value> {
        let kind = self.apps.kind(h)?;
        let m = schema().member(NameKind::Prop, kind, p)?;
        let get = m.thunk?;
        let module = schema().module_of(kind).map(|x| x.name.as_str()).unwrap_or("");
        let cx = CallCx { objects: &self.apps, schema: schema(), this: Some(h), module };
        catch_unwind(AssertUnwindSafe(|| get(&cx, Vec::new()))).ok()?.ok()
    }

    /// Counts a store entry of `take_changes`: delivered, or dropped by the compare.
    pub(crate) fn note_store_change(&self, delivered: bool) {
        self.reactive.stat(|s| if delivered { s.store_delivered += 1 } else { s.store_unchanged += 1 });
    }

    pub(crate) fn note_store_batch(&self) {
        self.reactive.stat(|s| s.store_batches += 1);
    }

    /// An app handle was released: its observations, dirty entries and subscriptions go with it.
    pub(crate) fn app_forget(&self, h: svl_handle) {
        {
            let mut st = self.st.borrow_mut();
            st.observed.retain(|k, _| k.0 != h);
            st.dirty.retain(|k| k.0 != h);
            st.dirty_set.retain(|k| k.0 != h);
        }
        let store = self.reactive.stores.borrow_mut().remove(&h);
        drop(store);
        let emit = self.reactive.emits.borrow_mut().remove(&h);
        if let Some(e) = emit {
            e.set_listened_mask(0);
        }
        self.reactive.warned.borrow_mut().remove(&h);
    }

    /// A reload starts: the old runtime's observations of and listeners on app objects are set aside.
    pub(crate) fn reactive_stash(&self) -> ReactiveStash {
        let observed = {
            let mut st = self.st.borrow_mut();
            let keys: Vec<(svl_handle, svl_id)> = st.observed.keys().filter(|k| exports::is_app_handle(k.0)).copied().collect();
            st.dirty.retain(|k| !exports::is_app_handle(k.0));
            st.dirty_set.retain(|k| !exports::is_app_handle(k.0));
            keys.into_iter().filter_map(|k| st.observed.remove(&k).map(|o| (k, o))).collect()
        };
        let listened = self
            .reactive
            .emits
            .borrow()
            .iter()
            .map(|(h, s)| {
                let m = s.listened_mask();
                s.set_listened_mask(0);
                (*h, m)
            })
            .collect();
        ReactiveStash { observed, listened }
    }

    /// The reload failed: the old runtime's observations and listeners come back (merged with what the
    /// failed load registered on handles that stay live).
    pub(crate) fn reactive_restore(&self, s: ReactiveStash) {
        {
            let mut st = self.st.borrow_mut();
            for (k, o) in s.observed {
                if self.apps.kind(k.0).is_none() {
                    continue;
                }
                match st.observed.get_mut(&k) {
                    Some(cur) => cur.count += o.count,
                    None => {
                        st.observed.insert(k, o);
                    }
                }
            }
        }
        let emits = self.reactive.emits.borrow();
        for (h, m) in s.listened {
            if let Some(sub) = emits.get(&h) {
                sub.set_listened_mask(sub.listened_mask() | m);
            }
        }
    }

    /// `persist(key)`: the persisted object's handle in this runtime and its native class name.
    pub(crate) fn op_persist_get(&self, key: &str) -> Option<(svl_handle, String)> {
        let (kind, ty, obj) = {
            let p = self.reactive.persist.borrow();
            let e = p.get(key)?;
            (e.kind, e.ty, e.obj.clone())
        };
        let h = self.apps.adopt(kind, ty, obj);
        Some((h, schema().kind_name(kind).unwrap_or_default()))
    }

    /// `persist(key, obj)` keeps the app object `h` across hot reloads under `key`; `h = None` forgets it.
    pub(crate) fn op_persist_set(&self, key: &str, h: Option<svl_handle>) -> Result<(), OpError> {
        let Some(h) = h else {
            let gone = self.reactive.persist.borrow_mut().remove(key);
            drop(gone);
            return Ok(());
        };
        if !exports::is_app_handle(h) {
            return Err(OpError { kind: OpKind::Type, msg: format!("persist('{key}', obj): only objects of svelters:app modules (Rust exports) can be persisted"), native: None });
        }
        let (kind, ty, obj) = self.apps.object(h).ok_or_else(|| disposed(h))?;
        let old = self.reactive.persist.borrow_mut().insert(key.to_string(), Persisted { kind, ty, obj });
        drop(old);
        Ok(())
    }

    /// Persisted keys (tests, diagnostics).
    pub(crate) fn persisted_keys(&self) -> Vec<String> {
        let mut v: Vec<String> = self.reactive.persist.borrow().keys().cloned().collect();
        v.sort();
        v
    }

    /// The host is going away: writers reach nobody; subscriptions and persisted objects are dropped
    /// here, on the UI thread.
    pub(crate) fn reactive_shutdown(&self) {
        self.reactive.hub.detach();
        let stores = std::mem::take(&mut *self.reactive.stores.borrow_mut());
        drop(stores);
        let emits = std::mem::take(&mut *self.reactive.emits.borrow_mut());
        for e in emits.values() {
            e.set_listened_mask(0);
        }
        drop(emits);
        let persist = std::mem::take(&mut *self.reactive.persist.borrow_mut());
        drop(persist);
    }
}

/// Checkpoint step 1b (E3), after the offload drain and before the change batch: dirty store fields join
/// the dirty set, queued `Emitter` events join the event queue. Runs no JS.
pub(crate) fn collect(core: &Core) {
    let r = &core.reactive;
    if !r.hub.pending() {
        return;
    }
    let (stores, events) = r.hub.take();
    let mut marks = 0u64;
    for h in stores {
        let Some(sub) = r.stores.borrow().get(&h).cloned() else { continue };
        let fields = sub.take();
        if fields.is_empty() {
            continue;
        }
        let Some(store) = core.apps.kind(h).and_then(|k| schema().class(k)).and_then(|c| c.store.as_ref()) else { continue };
        let mut st = core.st.borrow_mut();
        for i in fields {
            let Some(f) = store.fields.get(i) else { continue };
            let key = (h, f.prop);
            if st.observed.contains_key(&key) {
                mark_dirty(&mut st, key);
                marks += 1;
            }
        }
    }
    let (mut queued, mut dropped) = (0u64, 0u64);
    let mut per: HashMap<svl_handle, u32> = HashMap::new();
    for e in events {
        let (handle, index, payload) = match e {
            HubEvent::Event { handle, index, payload } => (handle, index, payload),
            HubEvent::Latest { handle, index, sub } => match sub.take_latest(index) {
                Some(p) => (handle, index, p),
                None => continue,
            },
        };
        let listened = r.emits.borrow().get(&handle).is_some_and(|s| s.listens(index));
        let kind = core.apps.kind(handle);
        let (true, Some(kind)) = (listened, kind) else {
            drop(payload);
            dropped += 1;
            continue;
        };
        let module = schema().module_of(kind).map(|x| x.name.as_str()).unwrap_or("");
        let cx = CallCx { objects: &core.apps, schema: schema(), this: Some(handle), module };
        let args = match catch_unwind(AssertUnwindSafe(|| payload(&cx))) {
            Ok(Ok(v)) => v.into_iter().collect::<Vec<Value>>(),
            Ok(Err(n)) => {
                core.report(JsError::new(ErrorKind::Host, format!("event {} of {}: {}", index, schema().kind_name(kind).unwrap_or_default(), n.message)));
                dropped += 1;
                continue;
            }
            Err(_) => {
                core.report(JsError::new(ErrorKind::Host, format!("event {} of {}: a panic while converting it", index, schema().kind_name(kind).unwrap_or_default())));
                dropped += 1;
                continue;
            }
        };
        core.pin_args(&args, true);
        core.st.borrow_mut().events.push_back(Event::Signal { handle, signal: index as svl_id + 1, args, deferred: false, flags: 0 });
        queued += 1;
        let n = per.entry(handle).or_insert(0);
        *n += 1;
        if cfg!(debug_assertions) && *n == EVENT_FLOOD + 1 && r.warned.borrow_mut().insert(handle) {
            core.console(
                "warn",
                format!(
                    "svelters: {} #a{} emitted more than {EVENT_FLOOD} events in one checkpoint; use #[event(latest)] or a store for high-rate values",
                    schema().kind_name(kind).unwrap_or_default(),
                    handle & !exports::APP_HANDLE_BIT
                ),
            );
        }
    }
    r.stat(|s| {
        s.collects += 1;
        s.store_marks += marks;
        s.events += queued;
        s.events_dropped += dropped;
    });
}
