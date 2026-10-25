//! Shared test plumbing: the presto backend behind its real vtable, attached to a recording host
//! (no JS). Native output (a kind's signal, a native-owned prop change) is produced through the tree's
//! `emit`/`native_set`, the calls presto-core's `EventCx` makes, and delivered with `pump`.
#![allow(dead_code)]

use std::cell::RefCell;
use std::rc::Rc;
use svelters_abi::*;
use svelters_presto::PrestoBackend;
use svelters_presto::shim::Value as PValue;

#[derive(Default)]
pub struct Log {
    pub events: Vec<(svl_handle, svl_id, Vec<Value>)>,
    pub changed: Vec<(svl_handle, svl_id)>,
    pub gone: Vec<(svl_handle, u32)>,
    pub checkpoints: usize,
    pub frames: usize,
    /// What `call_sync` answers.
    pub sync_answer: Option<Value>,
    pub syncs: Vec<(svl_handle, svl_id, Vec<Value>)>,
}

pub struct Rec(pub Rc<RefCell<Log>>);

impl HostCallbacks for Rec {
    fn post_event(&self, h: svl_handle, signal: svl_id, args: &[Value]) {
        self.0.borrow_mut().events.push((h, signal, args.to_vec()));
    }
    fn checkpoint(&self) {
        self.0.borrow_mut().checkpoints += 1;
    }
    fn call_sync(&self, h: svl_handle, signal: svl_id, args: &[Value]) -> Result<Value, SyncError> {
        let mut l = self.0.borrow_mut();
        l.syncs.push((h, signal, args.to_vec()));
        l.sync_answer.clone().ok_or(SyncError::Failed("no answer".into()))
    }
    fn on_destroyed(&self, h: svl_handle, reason: u32) {
        self.0.borrow_mut().gone.push((h, reason));
    }
    fn frame(&self) {
        self.0.borrow_mut().frames += 1;
    }
    fn prop_changed(&self, h: svl_handle, prop: svl_id) {
        self.0.borrow_mut().changed.push((h, prop));
    }
}

pub struct Rig {
    pub backend: PrestoBackend,
    pub vt: &'static BackendVtable,
    pub hv: &'static HostVtable,
    pub log: Rc<RefCell<Log>>,
}

impl Rig {
    pub fn new() -> Rig {
        let backend = PrestoBackend::builtin();
        let vt: &'static BackendVtable = Box::leak(Box::new(BackendVtable::new(backend.clone())));
        let log: Rc<RefCell<Log>> = Rc::default();
        let hv: &'static HostVtable = Box::leak(Box::new(HostVtable::new(Rec(log.clone()))));
        vt.backend_ref().start(hv).expect("start");
        Rig { backend, vt, hv, log }
    }

    pub fn br(&self) -> BackendRef<'static> {
        self.vt.backend_ref()
    }

    pub fn kind(&self, h: svl_handle) -> svl_id {
        self.br().handle_info(h).unwrap().kind
    }

    pub fn create(&self, tag: &str) -> svl_handle {
        let k = self.br().resolve(NameKind::Kind, 0, tag);
        assert_ne!(k, 0, "unknown tag {tag}");
        self.br().create(k).unwrap()
    }

    pub fn prop_id(&self, h: svl_handle, name: &str) -> svl_id {
        let id = self.br().resolve(NameKind::Prop, self.kind(h), name);
        assert_ne!(id, 0, "no prop {name}");
        id
    }

    pub fn signal_id(&self, h: svl_handle, name: &str) -> svl_id {
        let id = self.br().resolve(NameKind::Signal, self.kind(h), name);
        assert_ne!(id, 0, "no signal {name}");
        id
    }

    pub fn set(&self, h: svl_handle, name: &str, v: Value) {
        self.br().set_prop(h, self.prop_id(h, name), &v).unwrap()
    }

    pub fn get(&self, h: svl_handle, name: &str) -> Value {
        self.br().get_prop(h, self.prop_id(h, name)).unwrap()
    }

    pub fn listen(&self, h: svl_handle, name: &str) {
        self.br().listen(h, self.signal_id(h, name), true).unwrap()
    }

    pub fn observe(&self, h: svl_handle, name: &str, on: bool) {
        self.br().observe(h, self.prop_id(h, name), on).unwrap()
    }

    pub fn insert(&self, parent: svl_handle, child: svl_handle) {
        self.br().insert(parent, child, None, 0).unwrap()
    }

    pub fn call(&self, h: svl_handle, method: &str) {
        let m = self.br().resolve(NameKind::Method, self.kind(h), method);
        assert_ne!(m, 0, "no method {method}");
        self.br().call(h, m, &[]).unwrap();
    }

    /// A kind's native default emitted `signal` on `h` (as presto-core's `EventCx::emit` does).
    pub fn emit(&self, h: svl_handle, signal: &str, args: Vec<PValue>) {
        let n = self.backend.node(h).unwrap();
        let reg = self.backend.registry().clone();
        self.backend.with_doc(|d| {
            let k = d.kind(n).unwrap();
            let (s, _) = reg.signal(k, signal).unwrap_or_else(|| panic!("no signal {signal}"));
            d.emit(n, s, args);
        });
    }

    /// A kind changed native-owned `prop` on `h` itself (as presto-core's `EventCx::set` does).
    pub fn native_set(&self, h: svl_handle, prop: &str, v: PValue) {
        let n = self.backend.node(h).unwrap();
        let reg = self.backend.registry().clone();
        self.backend.with_doc(|d| {
            let k = d.kind(n).unwrap();
            let (p, _) = reg.prop(k, prop).unwrap_or_else(|| panic!("no prop {prop}"));
            d.native_set(n, p, v).unwrap();
        });
    }

    /// Takes what the host recorded so far.
    pub fn take(&self) -> Log {
        let mut l = self.log.borrow_mut();
        let answer = l.sync_answer.clone();
        let out = std::mem::take(&mut *l);
        l.sync_answer = answer;
        out
    }
}
