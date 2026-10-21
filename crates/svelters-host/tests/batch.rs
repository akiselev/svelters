//! ABI v2.6 host contract (plans/research/REACTIVITY.md §5.2, plans/architecture/backend-abi.md
//! "Host contract"), one test per rule, on the live (non-scripted) checkpoint path:
//!
//! - refcount per (h, prop), transitions only;
//! - an ordered, deduplicated dirty set with one wake on the first insert;
//! - a native read at the checkpoint, skipped for `SVL_M_PROPORTIONAL`;
//! - compare with `last` (f64 by bits with NaN equal, strings by content, BOXED always changed);
//! - one `__svelters_changed` batch per checkpoint, before the queued events, no drain in between;
//! - the set path: read back after our own `set_prop` (echo dropped, a normalized value delivered once);
//! - the sync flush before `call_sync` runs JS;
//! - cleanup on `on_destroyed`; native changes during JS are delivered at the next checkpoint.
//!
//! The backend is a fake toolkit object `obj` with props `value` (f64, clamped to 0..=100 by
//! `set_prop`), `text` (string, proportional), `box` (each read mints a new BOXED handle), `count`
//! (i64) and `other` (no change notification), a method `poke(prop, value, times)` that changes a
//! prop natively and announces it `times` times, and a signal `evt`.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use svelters_abi::*;
use svelters_host::{Host, HostConfig};

const PROPS: [&str; 5] = ["value", "text", "box", "count", "other"];
const P_VALUE: svl_id = 1;
const P_TEXT: svl_id = 2;
const P_BOX: svl_id = 3;
const P_COUNT: svl_id = 4;
const EVT: svl_id = 1;
const OBJ: svl_handle = 1;

#[derive(Default)]
struct State {
    values: RefCell<HashMap<(svl_handle, svl_id), Value>>,
    observed: RefCell<HashSet<(svl_handle, svl_id)>>,
    /// `observe #h prop on|off`, `read #h prop`, `release #h` (a BOXED handle destroyed)
    log: RefCell<Vec<String>>,
    next: Cell<svl_handle>,
    wakes: Arc<AtomicUsize>,
    host: Cell<*const svl_host>,
}

impl State {
    fn host(&self) -> HostRef<'_> {
        unsafe { HostRef::from_raw(self.host.get()) }
    }
    fn take_log(&self) -> Vec<String> {
        std::mem::take(&mut *self.log.borrow_mut())
    }
    fn native_set(&self, h: svl_handle, p: svl_id, v: Value, times: usize) {
        self.values.borrow_mut().insert((h, p), v);
        for _ in 0..times {
            self.host().prop_changed(h, p);
        }
    }
}

struct Fake(Rc<State>);

fn name(p: svl_id) -> &'static str {
    PROPS[p as usize - 1]
}

impl Backend for Fake {
    fn resolve(&self, what: NameKind, _: svl_id, n: &str) -> svl_id {
        match what {
            NameKind::Kind => (n == "obj") as svl_id,
            NameKind::Prop => PROPS.iter().position(|p| *p == n).map_or(0, |i| i as svl_id + 1),
            NameKind::Method => (n == "poke") as svl_id,
            NameKind::Signal => (n == "evt") as svl_id,
            _ => 0,
        }
    }
    fn describe(&self, what: NameKind, _: svl_id, member: svl_id) -> Option<MemberDesc> {
        if what != NameKind::Prop || member == 0 || member as usize > PROPS.len() {
            return None;
        }
        let tag = match member {
            P_VALUE => SVL_F64,
            P_COUNT => SVL_I64,
            P_TEXT => SVL_STR,
            _ => SVL_NULL,
        };
        let slot = SlotDesc::new(tag).named(name(member));
        let mut flags = SVL_M_OBSERVABLE;
        if member == P_TEXT {
            flags |= SVL_M_PROPORTIONAL;
        }
        Some(MemberDesc { name: name(member).into(), params: vec![slot.clone()], ret: slot, flags })
    }
    fn create(&self, _: svl_id) -> Result<svl_handle, String> {
        let h = self.0.next.get() + 1;
        self.0.next.set(h);
        Ok(h)
    }
    fn insert(&self, _: svl_handle, _: svl_handle, _: Option<svl_handle>, _: svl_id) -> Result<(), String> {
        Ok(())
    }
    fn remove(&self, _: svl_handle, _: svl_handle) -> Result<(), String> {
        Ok(())
    }
    fn destroy(&self, h: svl_handle) {
        if h >= 1000 {
            self.0.log.borrow_mut().push(format!("release #{h}"));
        }
    }
    fn set_prop(&self, h: svl_handle, p: svl_id, v: &Value) -> Result<(), String> {
        let v = match (p, v) {
            (P_VALUE, Value::F64(x)) => Value::F64(x.clamp(0.0, 100.0)),
            (_, v) => v.clone(),
        };
        self.0.values.borrow_mut().insert((h, p), v);
        // Like GTK: the toolkit announces our own write synchronously, except for a proportional
        // prop, whose echo the backend suppresses (REACTIVITY.md §5.4).
        if p != P_TEXT && self.0.observed.borrow().contains(&(h, p)) {
            self.0.host().prop_changed(h, p);
        }
        Ok(())
    }
    fn get_prop(&self, h: svl_handle, p: svl_id) -> Result<Value, String> {
        self.0.log.borrow_mut().push(format!("read #{h} {}", name(p)));
        if p == P_BOX {
            let b = self.0.next.get().max(999) + 1;
            self.0.next.set(b);
            return Ok(Value::Boxed { ty: 0, handle: b });
        }
        Ok(self.0.values.borrow().get(&(h, p)).cloned().unwrap_or(Value::Null))
    }
    fn call(&self, h: svl_handle, _: svl_id, args: &[Value]) -> Result<Value, String> {
        let (Some(Value::Str(p)), Some(v), Some(times)) = (args.first(), args.get(1), args.get(2)) else {
            return Err("poke(prop, value, times)".into());
        };
        let p = PROPS.iter().position(|x| x == p).unwrap() as svl_id + 1;
        let times = match times {
            Value::F64(t) => *t as usize,
            Value::I64(t) => *t as usize,
            _ => 1,
        };
        self.0.native_set(h, p, v.clone(), times);
        Ok(Value::Null)
    }
    fn listen(&self, _: svl_handle, _: svl_id, _: bool) -> Result<(), String> {
        Ok(())
    }
    fn handle_info(&self, _: svl_handle) -> Result<svl_handle_info, String> {
        Ok(svl_handle_info { kind: 1, lifetime: SVL_LIFE_OWNED, flags: 0 })
    }
    fn run(&self, _: &HostRef<'_>) -> Result<(), String> {
        Ok(())
    }
    fn waker(&self) -> Waker {
        let n = self.0.wakes.clone();
        Arc::new(move || {
            n.fetch_add(1, Ordering::Relaxed);
        })
    }
    fn quit(&self) {}
    fn schedule_wake(&self, _: i64) {}
    fn supports_observe(&self) -> bool {
        true
    }
    fn observe(&self, h: svl_handle, p: svl_id, on: bool) -> Result<(), ObserveError> {
        if name(p) == "other" {
            return Err(ObserveError::Unsupported("no notification".into()));
        }
        self.0.log.borrow_mut().push(format!("observe #{h} {} {}", name(p), if on { "on" } else { "off" }));
        if on {
            self.0.observed.borrow_mut().insert((h, p));
        } else {
            self.0.observed.borrow_mut().remove(&(h, p));
        }
        Ok(())
    }
}


struct Rig {
    st: Rc<State>,
    host: Host<'static>,
    _vt: Box<BackendVtable>,
}

impl Rig {
    /// Loads `tests/js/batch.js` (which creates `obj`, handle 1, and listens to `evt`) followed by
    /// `extra`, then forgets the backend log and console of the load.
    fn new(extra: &str) -> Rig {
        let st = Rc::new(State::default());
        let vt = Box::new(BackendVtable::new(Fake(st.clone())));
        let raw = unsafe { BackendRef::from_raw(vt.as_raw()) }.unwrap();
        let cfg = HostConfig { echo_console: false, echo_errors: false, ..HostConfig::default() };
        let host = Host::new(raw, cfg).unwrap();
        st.host.set(host.host_callbacks());
        let dir = std::env::temp_dir().join(format!("svelters-batch-{}-{}", std::process::id(), next_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let base = std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/js/batch.js")).unwrap();
        std::fs::write(dir.join("app.js"), format!("{base}\n{extra}\n")).unwrap();
        host.load_bundle(dir.join("app.js")).unwrap();
        let r = Rig { st, host, _vt: vt };
        r.console();
        r.st.take_log();
        r.wakes();
        r
    }
    fn console(&self) -> Vec<String> {
        let errors = self.host.take_errors();
        assert!(errors.is_empty(), "{errors:?}");
        self.host.take_console().into_iter().map(|c| c.text).collect()
    }
    fn checkpoint(&self) -> Vec<String> {
        self.host.checkpoint();
        self.console()
    }
    /// Runs `code` as the handler of an `evt` event (delivered at a checkpoint) and returns the console.
    fn run(&self, code: &str) -> Vec<String> {
        self.st.host().post_event(OBJ, EVT, &[Value::Str(code.into())]);
        self.checkpoint()
    }
    fn wakes(&self) -> usize {
        self.st.wakes.swap(0, Ordering::Relaxed)
    }
}

fn next_id() -> usize {
    static N: AtomicUsize = AtomicUsize::new(0);
    N.fetch_add(1, Ordering::Relaxed)
}

#[test]
fn refcount_reaches_the_backend_on_transitions_only() {
    let r = Rig::new("");
    let out = r.run(
        "log(h.observe(obj, 'value', true), h.observe(obj, 'value', true), h.observe(obj, 'value', false), \
         h.observe(obj, 'value', false), h.observe(obj, 'value', false), h.observe(obj, 'other', true));",
    );
    assert_eq!(out, ["true true true false false false"]);
    assert_eq!(r.st.take_log(), ["observe #1 value on", "read #1 value", "observe #1 value off"], "baseline read at 0->1 only");
    assert_eq!(r.host.observed_props(), 0);
}

#[test]
fn notifications_coalesce_per_prop_in_first_announcement_order_with_one_wake() {
    let r = Rig::new("h.observe(obj, 'value', true); h.observe(obj, 'count', true);");
    r.st.native_set(OBJ, P_VALUE, Value::F64(5.0), 1);
    r.st.native_set(OBJ, P_COUNT, Value::I64(1), 2);
    r.st.native_set(OBJ, P_VALUE, Value::F64(7.0), 3);
    assert_eq!(r.wakes(), 1, "one wake for the first insert of the turn");
    assert_eq!(r.checkpoint(), ["changed 1,\"value\",7,1,\"count\",1", "job after changed"]);
    assert_eq!(r.st.take_log(), ["read #1 value", "read #1 count"], "one read per dirty pair");
    // An unobserved pair is ignored: no wake, nothing delivered, nothing read.
    r.st.host().prop_changed(OBJ, P_TEXT);
    assert_eq!(r.wakes(), 0);
    assert!(r.checkpoint().is_empty());
    assert!(r.st.take_log().is_empty());
}

#[test]
fn the_batch_comes_before_queued_events_with_no_drain_in_between() {
    let r = Rig::new("h.observe(obj, 'value', true);");
    // The event was queued first; the batch still goes first.
    r.st.host().post_event(OBJ, EVT, &[]);
    r.st.native_set(OBJ, P_VALUE, Value::F64(3.0), 1);
    assert_eq!(r.checkpoint(), ["changed 1,\"value\",3", "event #1 evt sees 3", "job after changed", "job after evt"]);
}

#[test]
fn equal_values_are_dropped_by_compare() {
    let r = Rig::new("h.observe(obj, 'value', true); h.observe(obj, 'count', true);");
    // The baseline (read at the first observation) is null: a native null is no change.
    r.st.host().prop_changed(OBJ, P_VALUE);
    assert!(r.checkpoint().is_empty());
    r.st.native_set(OBJ, P_VALUE, Value::F64(f64::NAN), 1);
    assert_eq!(r.checkpoint(), ["changed 1,\"value\",NaN", "job after changed"]);
    r.st.native_set(OBJ, P_VALUE, Value::F64(f64::NAN), 1);
    assert!(r.checkpoint().is_empty(), "NaN equals NaN");
    r.st.native_set(OBJ, P_VALUE, Value::F64(0.0), 1);
    r.st.native_set(OBJ, P_COUNT, Value::I64(0), 1);
    assert_eq!(r.checkpoint(), ["changed 1,\"value\",0,1,\"count\",0", "job after changed"]);
    r.st.native_set(OBJ, P_VALUE, Value::F64(-0.0), 1);
    r.st.native_set(OBJ, P_COUNT, Value::I64(0), 1);
    assert_eq!(r.checkpoint(), ["changed 1,\"value\",0", "job after changed"], "f64 by bits: -0 is not 0");
    r.st.native_set(OBJ, P_VALUE, Value::F64(-0.0), 1);
    assert!(r.checkpoint().is_empty());
}

#[test]
fn strings_compare_by_content() {
    let r = Rig::new("h.observe(obj, 'count', true);");
    r.st.native_set(OBJ, P_COUNT, Value::Str("ab".into()), 1);
    assert_eq!(r.checkpoint(), ["changed 1,\"count\",\"ab\"", "job after changed"]);
    r.st.native_set(OBJ, P_COUNT, Value::Str16("ab".encode_utf16().collect()), 1);
    assert!(r.checkpoint().is_empty(), "UTF-8 and UTF-16 with the same content are equal");
    r.st.native_set(OBJ, P_COUNT, Value::List(vec![Value::I64(1), Value::Str("x".into())]), 1);
    assert_eq!(r.checkpoint(), ["changed 1,\"count\",boxed", "job after changed"]);
    r.st.native_set(OBJ, P_COUNT, Value::List(vec![Value::I64(1), Value::Str("x".into())]), 1);
    assert!(r.checkpoint().is_empty(), "lists compare deeply");
}

#[test]
fn proportional_props_are_never_read_and_always_delivered_as_an_invalidation() {
    let r = Rig::new("h.observe(obj, 'text', true);");
    assert_eq!(r.st.take_log(), Vec::<String>::new(), "no baseline read (load log is cleared; see below)");
    r.st.native_set(OBJ, P_TEXT, Value::Str("a".into()), 2);
    assert_eq!(r.checkpoint(), ["changed 1,\"text\",undefined", "job after changed"]);
    r.st.native_set(OBJ, P_TEXT, Value::Str("a".into()), 1);
    assert_eq!(r.checkpoint(), ["changed 1,\"text\",undefined", "job after changed"], "never compared");
    // Our own write: no read-back (this backend suppresses the echo of a proportional prop).
    r.run("h.setProp(obj, 'text', 'mine');");
    assert!(r.st.take_log().is_empty(), "never read natively");
    assert!(r.checkpoint().is_empty());
}

#[test]
fn the_first_observation_reads_a_baseline_unless_proportional() {
    let r = Rig::new("");
    r.run("h.observe(obj, 'text', true); h.observe(obj, 'value', true);");
    assert_eq!(r.st.take_log(), ["observe #1 text on", "observe #1 value on", "read #1 value"]);
}

#[test]
fn boxed_values_always_count_as_changed_and_unseen_reads_are_released() {
    let r = Rig::new("");
    r.run("h.observe(obj, 'box', true);");
    // The baseline read minted #1000, which nobody sees: released at once.
    assert_eq!(r.st.take_log(), ["observe #1 box on", "read #1 box", "release #1000"]);
    r.st.host().prop_changed(OBJ, P_BOX);
    assert_eq!(r.checkpoint(), ["changed 1,\"box\",boxed", "job after changed"]);
    r.st.host().prop_changed(OBJ, P_BOX);
    assert_eq!(r.checkpoint(), ["changed 1,\"box\",boxed", "job after changed"], "a new handle each read: never equal");
}

#[test]
fn our_own_write_is_read_back_its_echo_dropped_and_a_normalized_value_delivered_once() {
    let r = Rig::new("h.observe(obj, 'value', true);");
    r.run("h.setProp(obj, 'value', 50);");
    assert_eq!(r.st.take_log(), ["read #1 value"], "the read-back");
    // The backend announced our write synchronously: dropped by compare with the read-back.
    assert!(r.checkpoint().is_empty());
    // The toolkit clamps 150 to 100: JS gets 100 once, at the next checkpoint.
    assert!(r.run("h.setProp(obj, 'value', 150);").is_empty());
    assert!(r.wakes() > 0);
    assert_eq!(r.checkpoint(), ["changed 1,\"value\",100", "job after changed"]);
    assert!(r.checkpoint().is_empty());
}

#[test]
fn a_sync_handler_sees_fresh_state_and_no_job_runs_before_it() {
    let r = Rig::new("h.observe(obj, 'value', true);");
    r.st.native_set(OBJ, P_VALUE, Value::F64(9.0), 1);
    let res = r.st.host().call_sync(OBJ, EVT, &[]);
    assert!(res.is_ok());
    assert_eq!(r.console(), ["changed 1,\"value\",9", "sync #1 evt sees 9"]);
    assert_eq!(r.checkpoint(), ["job after changed", "job after sync"]);
}

#[test]
fn native_changes_while_js_runs_wait_for_the_next_checkpoint() {
    let r = Rig::new("h.observe(obj, 'value', true);");
    let out = r.run("h.call(obj, 'poke', ['value', 42, 2]); log('after poke');");
    assert_eq!(out, ["after poke"], "never delivered re-entrantly");
    assert_eq!(r.checkpoint(), ["changed 1,\"value\",42", "job after changed"]);
}

#[test]
fn death_drops_observations_and_dirty_entries() {
    let r = Rig::new("h.observe(obj, 'value', true); h.observe(obj, 'count', true);");
    r.st.native_set(OBJ, P_VALUE, Value::F64(1.0), 1);
    r.st.host().on_destroyed(OBJ, SVL_GONE_CLOSED);
    assert_eq!(r.host.observed_props(), 0);
    assert!(r.checkpoint().is_empty());
    assert!(r.st.take_log().is_empty(), "nothing read for a dead handle");

    // destroy from JS likewise.
    let r = Rig::new("h.observe(obj, 'value', true);");
    r.st.native_set(OBJ, P_VALUE, Value::F64(1.0), 1);
    r.st.host().post_event(OBJ, EVT, &[Value::Str("h.destroy(obj);".into())]);
    // The batch goes first (the object was alive at the checkpoint), then the handler destroys it.
    assert_eq!(r.checkpoint(), ["changed 1,\"value\",1", "job after changed"]);
    assert_eq!(r.host.observed_props(), 0);
    r.st.host().prop_changed(OBJ, P_VALUE);
    assert!(r.checkpoint().is_empty());
}

#[test]
fn unobserving_drops_pending_dirty_entries() {
    let r = Rig::new("h.observe(obj, 'value', true);");
    // A native change during JS marks the pair dirty; the same handler then stops observing.
    assert!(r.run("h.call(obj, 'poke', ['value', 5, 1]); h.observe(obj, 'value', false);").is_empty());
    assert!(r.checkpoint().is_empty());
    assert!(r.st.take_log().iter().all(|l| !l.starts_with("read")), "an unobserved pair is not read");
}
