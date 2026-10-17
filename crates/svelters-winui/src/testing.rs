//! Test harness for the generated verification tests (`tests/generated.rs`) and the example's
//! tests. Windows only. All tests share one backend on the main thread (WinUI is initialized once
//! per thread), driven through the safe [`Backend`] trait and a recording host.
use crate::WinUiBackend;
use std::cell::RefCell;
use std::rc::Rc;
use svelters_abi::*;

/// What the recording host answers to `call_sync`.
#[derive(Clone, Debug)]
pub enum Reply {
    /// No decision (the toolkit default).
    None,
    Bool(bool),
}

#[derive(Default)]
struct Record {
    events: Vec<(u64, u32, Vec<Value>)>,
    gone: Vec<(u64, u32)>,
    reply: Option<Value>,
}

struct Recorder(Rc<RefCell<Record>>);

impl HostCallbacks for Recorder {
    fn post_event(&self, h: svl_handle, signal: svl_id, args: &[Value]) {
        self.0.borrow_mut().events.push((h, signal, args.to_vec()));
    }
    fn checkpoint(&self) {}
    fn call_sync(&self, h: svl_handle, signal: svl_id, args: &[Value]) -> Result<Value, SyncError> {
        let mut r = self.0.borrow_mut();
        r.events.push((h, signal, args.to_vec()));
        Ok(r.reply.clone().unwrap_or(Value::Null))
    }
    fn on_destroyed(&self, h: svl_handle, reason: u32) {
        self.0.borrow_mut().gone.push((h, reason));
    }
    fn frame(&self) {}
}

struct Shared {
    backend: &'static WinUiBackend,
    record: Rc<RefCell<Record>>,
    _host: &'static HostVtable,
}

thread_local! {
    /// The one backend of the UI thread, or why it could not start. Initialized once: WinUI can't
    /// be set up twice on a thread, so later tests report the first error instead of retrying.
    static SHARED: RefCell<Option<Result<Rc<Shared>, String>>> = const { RefCell::new(None) };
}

fn shared() -> Rc<Shared> {
    let r = SHARED.with(|s| {
        s.borrow_mut()
            .get_or_insert_with(|| {
                let backend = WinUiBackend::new()?;
                let backend: &'static WinUiBackend = Box::leak(Box::new(backend));
                let record = Rc::new(RefCell::new(Record::default()));
                let host: &'static HostVtable = Box::leak(Box::new(HostVtable::new(Recorder(record.clone()))));
                backend.attach_host(host.as_raw());
                Ok(Rc::new(Shared { backend, record, _host: host }))
            })
            .clone()
    });
    r.unwrap_or_else(|e| panic!("WinUI init failed: {e}"))
}

/// One test's view of the shared backend. Handles it created are destroyed when it drops.
pub struct Harness {
    s: Rc<Shared>,
    created: Vec<u64>,
}

impl Default for Harness {
    fn default() -> Self {
        Self::new()
    }
}

impl Harness {
    pub fn new() -> Harness {
        let s = shared();
        *s.record.borrow_mut() = Record::default();
        Harness { s, created: Vec::new() }
    }

    pub fn backend(&self) -> &'static WinUiBackend {
        self.s.backend
    }

    fn kind_of(&self, h: u64) -> u32 {
        self.s.backend.handle_info(h).map(|i| i.kind).unwrap_or(0)
    }

    pub fn create(&mut self, tag: &str) -> u64 {
        let b = self.s.backend;
        let kind = b.resolve(NameKind::Kind, 0, tag);
        assert!(kind != 0, "unknown tag <{tag}>");
        let h = b.create(kind).unwrap_or_else(|e| panic!("create <{tag}>: {e}"));
        self.created.push(h);
        h
    }

    fn id(&self, what: NameKind, h: u64, name: &str) -> u32 {
        let id = self.s.backend.resolve(what, self.kind_of(h), name);
        assert!(id != 0, "{what:?} {name:?} does not resolve on handle {h}");
        id
    }

    pub fn set(&self, h: u64, prop: &str, v: Value) -> Result<(), String> {
        self.s.backend.set_prop(h, self.id(NameKind::Prop, h, prop), &v)
    }

    pub fn get(&self, h: u64, prop: &str) -> Result<Value, String> {
        self.s.backend.get_prop(h, self.id(NameKind::Prop, h, prop))
    }

    pub fn call(&self, h: u64, method: &str, args: &[Value]) -> Result<Value, String> {
        self.s.backend.call(h, self.id(NameKind::Method, h, method), args)
    }

    pub fn insert(&self, p: u64, c: u64, before: Option<u64>, slot: Option<&str>) -> Result<(), String> {
        let slot = slot.map_or(0, |s| self.id(NameKind::Slot, p, s));
        self.s.backend.insert(p, c, before, slot)
    }

    pub fn remove(&self, p: u64, c: u64) -> Result<(), String> {
        self.s.backend.remove(p, c)
    }

    pub fn set_attr(&self, p: u64, c: u64, name: &str, v: Value) -> Result<(), String> {
        self.s.backend.set_child_attr(p, c, self.id(NameKind::Attr, p, name), &v)
    }

    pub fn get_attr(&self, p: u64, c: u64, name: &str) -> Result<Value, String> {
        self.s.backend.get_child_attr(p, c, self.id(NameKind::Attr, p, name))
    }

    pub fn listen(&self, h: u64, event: &str, on: bool) -> Result<(), String> {
        self.s.backend.listen(h, self.id(NameKind::Signal, h, event), on)
    }

    /// The handles natively present in a slot, read back through the slot's getter.
    pub fn slot_items(&self, p: u64, slot: &str) -> Vec<u64> {
        let id = self.id(NameKind::Slot, p, slot);
        self.s
            .backend
            .native_slot_items(p, id)
            .unwrap_or_else(|e| panic!("read slot {slot}: {e}"))
            .into_iter()
            .map(|h| h.expect("a native child that is not one of our handles"))
            .collect()
    }

    /// Answer the next `call_sync`s with `r`.
    pub fn reply(&self, r: Reply) {
        self.s.record.borrow_mut().reply = match r {
            Reply::None => None,
            Reply::Bool(b) => Some(Value::Bool(b)),
        };
    }

    pub fn pump(&self, ms: u64) {
        self.s.backend.pump(ms);
    }

    /// Click a button the way UI Automation does (`ButtonAutomationPeer.Invoke`).
    pub fn invoke(&self, h: u64) -> Result<(), String> {
        self.s.backend.invoke(h)
    }

    /// How many times `event` reached the host for `h` (posted or sync).
    pub fn events(&self, h: u64, event: &str) -> usize {
        let id = self.id(NameKind::Signal, h, event);
        self.s.record.borrow().events.iter().filter(|(eh, ev, _)| *eh == h && ev & SVL_SIGNAL_ID_MASK == id).count()
    }

    pub fn last_args(&self, h: u64, event: &str) -> Vec<Value> {
        let id = self.id(NameKind::Signal, h, event);
        let r = self.s.record.borrow();
        r.events.iter().rev().find(|(eh, ev, _)| *eh == h && ev & SVL_SIGNAL_ID_MASK == id).map(|e| e.2.clone()).unwrap_or_default()
    }

    /// The `on_destroyed` reason reported for `h`, if any.
    pub fn gone(&self, h: u64) -> Option<u32> {
        self.s.record.borrow().gone.iter().find(|(g, _)| *g == h).map(|g| g.1)
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        for h in self.created.drain(..).rev() {
            self.s.backend.destroy(h);
        }
        self.s.backend.pump(10);
    }
}

/// Runs `tests` on this thread; see [`crate::harness::main`].
pub fn run_tests(tests: &[(&str, fn())]) -> i32 {
    crate::harness::main(tests)
}
