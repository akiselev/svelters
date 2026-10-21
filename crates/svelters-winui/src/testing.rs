//! Test harness for the generated verification tests (`tests/generated.rs`) and the example's
//! tests. Windows only. All tests share one backend on the main thread (WinUI is initialized once
//! per thread), driven through the safe [`Backend`] trait and a recording host.
use crate::WinUiBackend;
use std::cell::RefCell;
use std::rc::Rc;
use svelters_abi::*;
pub use svelters_abi::ObserveError;

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
    /// ABI v2.6 `prop_changed` calls, in order.
    changed: Vec<(u64, u32)>,
    /// `invoke_callback` calls (async completions, N4), in order.
    invoked: Vec<(u64, Vec<Value>)>,
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
    fn prop_changed(&self, h: svl_handle, prop: svl_id) {
        self.0.borrow_mut().changed.push((h, prop));
    }
    fn invoke_callback(&self, callback: u64, args: &[Value]) -> StrResult<Value> {
        self.0.borrow_mut().invoked.push((callback, args.to_vec()));
        Ok(Value::Null)
    }
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

    /// The prop id `name` resolves to on `h`.
    pub fn prop_id(&self, h: u64, name: &str) -> u32 {
        self.id(NameKind::Prop, h, name)
    }

    /// ABI v2.6 `observe` through the backend trait.
    pub fn observe(&self, h: u64, prop: &str, on: bool) -> Result<(), ObserveError> {
        self.s.backend.observe(h, self.id(NameKind::Prop, h, prop), on)
    }

    /// A write that is not a host op (no echo marker): what native code changing the prop does.
    pub fn set_natively(&self, h: u64, prop: &str, v: Value) -> Result<(), String> {
        self.s.backend.set_prop_natively(h, self.id(NameKind::Prop, h, prop), &v)
    }

    /// `prop_changed` calls for (h, prop) since the last call (all calls are taken).
    pub fn take_changed(&self, h: u64, prop: u32) -> usize {
        let all = std::mem::take(&mut self.s.record.borrow_mut().changed);
        all.into_iter().filter(|c| *c == (h, prop)).count()
    }

    /// Creates a window, puts `child` in it and activates it (layout, Loaded, theme need a live tree).
    pub fn mount(&mut self, child: u64) -> u64 {
        let win = self.create("window");
        self.insert(win, child, None, None).unwrap_or_else(|e| panic!("mount: {e}"));
        self.call(win, "activate", &[]).unwrap_or_else(|e| panic!("activate: {e}"));
        self.pump(300);
        win
    }

    /// The `on_destroyed` reason reported for `h`, if any.
    pub fn gone(&self, h: u64) -> Option<u32> {
        self.s.record.borrow().gone.iter().find(|(g, _)| *g == h).map(|g| g.1)
    }
}

// ---- non-GUI types (N4) ----------------------------------------------------------------------------------------

static NEXT_CALLBACK: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1 << 40);

impl Harness {
    /// The kind id of a native type name (`Windows.Foundation.Uri`); panics when unbound.
    pub fn kind(&self, name: &str) -> u32 {
        let k = self.s.backend.resolve(NameKind::Kind, 0, name);
        assert!(k != 0, "unknown type {name}");
        k
    }

    fn keep(&mut self, v: &Value) {
        if let Value::Handle(h) = v
            && !self.created.contains(h)
        {
            self.created.push(*h);
        }
    }

    /// `construct(kind, props)`: the default constructor, then each prop (ABI v2.5).
    pub fn construct(&mut self, name: &str, props: &[(&str, Value)]) -> Result<u64, String> {
        let k = self.kind(name);
        let ids: Vec<u32> = props.iter().map(|(p, _)| self.s.backend.resolve(NameKind::Prop, k, p)).collect();
        if let Some(i) = ids.iter().position(|id| *id == 0) {
            return Err(format!("{name} has no prop {:?}", props[i].0));
        }
        let vals: Vec<Value> = props.iter().map(|(_, v)| v.clone()).collect();
        let v = self.s.backend.construct(k, &ids, &vals)?;
        self.keep(&v);
        match v {
            Value::Handle(h) => Ok(h),
            other => Err(format!("construct {name}: {other:?}")),
        }
    }

    /// `call_static(kind, method, args)`; handles in the result are destroyed with the harness.
    pub fn call_static(&mut self, name: &str, method: &str, args: &[Value]) -> Result<Value, String> {
        let k = self.kind(name);
        let m = self.s.backend.resolve(NameKind::Method, k, method);
        if m == 0 {
            return Err(format!("{name} has no static {method:?}"));
        }
        let v = self.s.backend.call_static(k, m, args)?;
        self.keep(&v);
        Ok(v)
    }

    /// A constructor with arguments (`call_static(kind, "new", args)`).
    pub fn new_object(&mut self, name: &str, args: &[Value]) -> Result<u64, String> {
        match self.call_static(name, "new", args)? {
            Value::Handle(h) => Ok(h),
            other => Err(format!("new {name}: {other:?}")),
        }
    }

    /// `call` that keeps returned handles for cleanup.
    pub fn call_keep(&mut self, h: u64, method: &str, args: &[Value]) -> Result<Value, String> {
        let v = self.call(h, method, args)?;
        self.keep(&v);
        Ok(v)
    }

    /// The descriptor of a member of a kind (`what`: method, prop or signal), if it resolves.
    pub fn describe_member(&self, kind: u32, what: NameKind, name: &str) -> Option<MemberDesc> {
        let id = self.s.backend.resolve(what, kind, name);
        if id == 0 {
            return None;
        }
        self.s.backend.describe(what, kind, id)
    }

    /// `describe(SVL_KIND, 0, kind)`: (qualified name, parent kind id).
    pub fn describe_kind(&self, kind: u32) -> Option<(String, u32)> {
        self.s.backend.describe(NameKind::Kind, 0, kind).map(|d| (d.name, d.ret.ty))
    }

    pub fn info(&self, h: u64) -> Result<svl_handle_info, String> {
        self.s.backend.handle_info(h)
    }

    /// Live handles of the backend (elements and type objects): the lifetime tests' baseline.
    pub fn live_handles(&self) -> usize {
        self.s.backend.inner().handles.borrow().len()
    }

    pub fn destroy(&mut self, h: u64) {
        self.created.retain(|c| *c != h);
        self.s.backend.destroy(h);
    }

    fn start(&mut self, recv: Result<u64, &str>, method: &str, args: &[Value]) -> Result<u64, String> {
        let (k, id) = match recv {
            Ok(h) => {
                let k = self.kind_of(h);
                (k, self.s.backend.resolve(NameKind::Method, k, method))
            }
            Err(name) => {
                let k = self.kind(name);
                (k, self.s.backend.resolve(NameKind::Method, k, method))
            }
        };
        if id == 0 {
            return Err(format!("unknown method {method}"));
        }
        let desc = self.s.backend.describe(NameKind::Method, k, id).ok_or("no descriptor")?;
        if desc.flags & SVL_M_ASYNC == 0 {
            return Err(format!("{method} is not async"));
        }
        let done = desc.params.last().filter(|p| p.tag == SVL_CALLBACK).ok_or("no completion slot")?.ty;
        let cb = NEXT_CALLBACK.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut vals = args.to_vec();
        vals.push(Value::Callback { id: cb, ty: done });
        let r = match recv {
            Ok(h) => self.s.backend.call(h, id, &vals)?,
            Err(_) => self.s.backend.call_static(k, id, &vals)?,
        };
        if let Value::Error(e) = r {
            return Err(format!("{method}: {e:?}"));
        }
        Ok(cb)
    }

    /// Starts an async method (`SVL_M_ASYNC`) of a handle; the completion is recorded under the returned id.
    pub fn call_async(&mut self, h: u64, method: &str, args: &[Value]) -> Result<u64, String> {
        self.start(Ok(h), method, args)
    }

    /// Starts an async static.
    pub fn call_static_async(&mut self, name: &str, method: &str, args: &[Value]) -> Result<u64, String> {
        self.start(Err(name), method, args)
    }

    /// Pumps until the completion `cb` arrives (or `ms` pass): its value (a handle in it is kept for cleanup).
    pub fn wait(&mut self, cb: u64, ms: u64) -> Option<Value> {
        let end = std::time::Instant::now() + std::time::Duration::from_millis(ms);
        loop {
            let found = {
                let mut r = self.s.record.borrow_mut();
                let i = r.invoked.iter().position(|(id, _)| *id == cb);
                i.map(|i| r.invoked.remove(i).1.into_iter().next().unwrap_or(Value::Null))
            };
            if let Some(v) = found {
                self.keep(&v);
                return Some(v);
            }
            if std::time::Instant::now() > end {
                return None;
            }
            self.pump(10);
        }
    }

    /// Starts and awaits an async member: `Ok(value)`, or `Err` for a rejection or a timeout.
    pub fn await_call(&mut self, h: u64, method: &str, args: &[Value]) -> Result<Value, String> {
        let cb = self.call_async(h, method, args)?;
        match self.wait(cb, 10_000) {
            Some(Value::Error(e)) => Err(format!("{method} rejected: {e:?}")),
            Some(v) => Ok(v),
            None => Err(format!("{method}: no completion in 10 s")),
        }
    }

    pub fn await_static(&mut self, name: &str, method: &str, args: &[Value]) -> Result<Value, String> {
        let cb = self.call_static_async(name, method, args)?;
        match self.wait(cb, 10_000) {
            Some(Value::Error(e)) => Err(format!("{name}.{method} rejected: {e:?}")),
            Some(v) => Ok(v),
            None => Err(format!("{name}.{method}: no completion in 10 s")),
        }
    }

    pub fn cancel(&self, cb: u64) -> bool {
        self.s.backend.cancel(cb)
    }

    /// Events of `h` since the harness was made, by event name: their argument lists.
    pub fn event_args(&self, h: u64, event: &str) -> Vec<Vec<Value>> {
        let id = self.id(NameKind::Signal, h, event);
        self.s.record.borrow().events.iter().filter(|(eh, ev, _)| *eh == h && ev & SVL_SIGNAL_ID_MASK == id).map(|e| e.2.clone()).collect()
    }

    /// `prop_changed` calls for (h, prop name) since the last take.
    pub fn take_changed_named(&self, h: u64, prop: &str) -> usize {
        let id = self.id(NameKind::Prop, h, prop);
        self.take_changed(h, id)
    }

    /// A fresh temporary directory (removed at process exit is not attempted: CI runners are discarded).
    pub fn temp_dir(tag: &str) -> String {
        let n = NEXT_CALLBACK.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!("svelters-n4-{tag}-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&p).expect("temp dir");
        p.display().to_string()
    }
}

impl Harness {
    /// Section 4.1 rule 3: an object whose DispatcherQueue belongs to another thread is refused at minting. Makes a
    /// `Microsoft.UI.Composition.Compositor` on a second thread with its own dispatcher queue, then asks the
    /// backend for its handle here. Returns the minting result.
    pub fn mint_foreign_compositor(&mut self) -> Result<u64, String> {
        use crate::bindings::Microsoft::UI::Dispatching::DispatcherQueueController;
        use crate::bindings::Windows::Win32::{RO_INIT_SINGLETHREADED, RoInitialize};
        use windows_core::Interface;
        struct Raw(usize);
        unsafe impl Send for Raw {}
        let (tx, rx) = std::sync::mpsc::channel::<Result<Raw, String>>();
        let (stop_tx, stop_rx) = std::sync::mpsc::channel::<()>();
        let worker = std::thread::spawn(move || {
            let r = (|| -> Result<Raw, String> {
                unsafe { RoInitialize(RO_INIT_SINGLETHREADED) }.ok().map_err(crate::conv::err)?;
                let controller = DispatcherQueueController::CreateOnCurrentThread().map_err(crate::conv::err)?;
                let f = crate::types::rt::factory("Microsoft.UI.Composition.Compositor", &crate::types::rt::iid(activation_iid()))
                    .map_err(|e| crate::types::rt::err_text(&e))?;
                let mut out: *mut std::ffi::c_void = std::ptr::null_mut();
                unsafe { crate::types::rt::vcall(&f, 6, &[crate::types::invoke::Word::ptr(&mut out)]) }.map_err(|e| crate::types::rt::err_text(&e))?;
                std::mem::forget(controller);
                Ok(Raw(out as usize))
            })();
            let _ = tx.send(r);
            let _ = stop_rx.recv();
        });
        let raw = rx.recv().map_err(|e| e.to_string())??;
        // SAFETY: an owned IInspectable of the Compositor (agile), handed over by the worker.
        let obj = unsafe { windows_core::IUnknown::from_raw(raw.0 as *mut std::ffi::c_void) };
        let r = crate::types::ops::mint(self.s.backend.inner(), obj.clone(), 0);
        if let Ok(h) = r {
            self.created.push(h);
        }
        drop(obj);
        let _ = stop_tx.send(());
        let _ = worker.join();
        r
    }
}

/// Index of IActivationFactory in the IID table.
fn activation_iid() -> u16 {
    let want = 0x00000035_0000_0000_c000_000000000046u128;
    crate::types::tables::IIDS.iter().position(|g| *g == want).expect("IActivationFactory in IIDS") as u16
}

impl Drop for Harness {
    fn drop(&mut self) {
        for h in self.created.drain(..).rev() {
            self.s.backend.destroy(h);
        }
        self.s.backend.pump(10);
    }
}

// ---- Table-driven checks of the generated tests (inferred facts, tools/metadata/winui/gen_backend.py) ----

/// Writes each value of `cases` to `prop` of a new `<tag>` and reads back the expected value.
pub fn roundtrip(tag: &str, prop: &str, cases: &[(Value, Value)]) {
    let mut h = Harness::new();
    let w = h.create(tag);
    for (v, want) in cases {
        h.set(w, prop, v.clone()).unwrap_or_else(|e| panic!("<{tag}> {prop} = {v:?}: {e}"));
        let got = h.get(w, prop).unwrap_or_else(|e| panic!("<{tag}> read {prop}: {e}"));
        assert_eq!(got, *want, "<{tag}> {prop} after writing {v:?}");
    }
}

/// A read-only prop of a new `<tag>` reads without an error.
pub fn readable(tag: &str, prop: &str) {
    let mut h = Harness::new();
    let w = h.create(tag);
    h.get(w, prop).unwrap_or_else(|e| panic!("<{tag}> read {prop}: {e}"));
}

/// `notify::<prop>`: while listened, writing `values` in order (at least one of them changes the prop)
/// raises the event; after unlistening, writes raise nothing.
pub fn notify(tag: &str, prop: &str, event: &str, values: &[Value]) {
    let mut h = Harness::new();
    let w = h.create(tag);
    h.listen(w, event, true).unwrap_or_else(|e| panic!("<{tag}> listen {event}: {e}"));
    for v in values {
        h.set(w, prop, v.clone()).unwrap_or_else(|e| panic!("<{tag}> {prop} = {v:?}: {e}"));
        h.pump(20);
    }
    let n = h.events(w, event);
    assert!(n >= 1, "<{tag}> {event}: no callback after writing {values:?}");
    h.listen(w, event, false).unwrap_or_else(|e| panic!("<{tag}> unlisten {event}: {e}"));
    for v in values.iter().rev() {
        h.set(w, prop, v.clone()).unwrap();
    }
    h.pump(20);
    assert_eq!(h.events(w, event), n, "<{tag}> {event}: raised after unlistening");
}

/// An event without a test trigger: listening and unlistening on a new `<tag>` succeed (the delegate and
/// its add/remove accessors work). Strength `connect`: native causation is not checked.
pub fn connect(tag: &str, event: &str) {
    let mut h = Harness::new();
    let w = h.create(tag);
    h.listen(w, event, true).unwrap_or_else(|e| panic!("<{tag}> listen {event}: {e}"));
    h.pump(10);
    h.listen(w, event, false).unwrap_or_else(|e| panic!("<{tag}> unlisten {event}: {e}"));
}

/// A method without arguments on a new `<tag>` returns without an error.
pub fn call0(tag: &str, method: &str) {
    let mut h = Harness::new();
    let w = h.create(tag);
    h.call(w, method, &[]).unwrap_or_else(|e| panic!("<{tag}> {method}(): {e}"));
}

// ---- Observe facts (milestone R4; plans/research/REACTIVITY.md 4.5) ----

/// How an observe row checks its fact (`tools/metadata/winui/gen_backend.py` picks it from the fact).
pub enum ObsCheck {
    /// Write these values natively in order (a write, not a host op): at least one is announced. Then the
    /// identical-write, echo, coalescing and teardown checks of [`observe`].
    Change(Vec<Value>),
    /// A native change that is not a write of the prop (the fact's `verify.trigger`): announced.
    Cross(Trig),
    /// The mechanism installs and uninstalls; nothing triggers it (`verified: connect`).
    Connect,
    /// `observe` answers SVL_OBSERVE_UNSUPPORTED (class none or set_only: WinUI has no mechanism).
    Unsupported,
    /// `observe` answers -1 (class constant).
    Refused,
}

/// Native triggers of `cross` facts (`verify.trigger` in metadata/rules/winui.yaml `observe.verify`).
#[derive(Clone, Copy, Debug)]
pub enum Trig {
    /// Mount the element in an activated window (`Loaded`).
    Mount,
    /// Mount, then change `width` (layout: `SizeChanged`).
    Resize,
    /// Mount, then flip `requested-theme` (`ActualThemeChanged`).
    Theme,
    /// Text, then `select(1, 3)` on a mounted TextBox (`SelectionChanged`).
    Select,
    /// Text and selection enabled, then `select-all` on a mounted TextBlock (`SelectionChanged`).
    SelectAll,
    /// `activate` the window itself (`VisibilityChanged`).
    Activate,
}

/// What an identical native write is expected to do (`fires` of the fact; `Any`: not decided yet, logged).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fires {
    OnChange,
    OnSet,
    Any,
}

fn fire(h: &mut Harness, w: u64, t: Trig, n: usize) {
    match t {
        Trig::Mount => {
            h.mount(w);
        }
        Trig::Resize => {
            let width = if n % 2 == 0 { 220.0 } else { 160.0 };
            h.set_natively(w, "width", Value::F64(width)).unwrap_or_else(|e| panic!("width: {e}"));
            h.pump(200);
        }
        Trig::Theme => {
            let cur = h.get(w, "actual-theme").unwrap_or(Value::Null);
            let next = if cur == Value::Str("dark".into()) { "Light" } else { "Dark" };
            h.set_natively(w, "requested-theme", Value::Str(next.into())).unwrap_or_else(|e| panic!("requested-theme: {e}"));
            h.pump(200);
        }
        Trig::Select => {
            let (a, b) = if n % 2 == 0 { (1, 3) } else { (2, 2) };
            h.call(w, "select", &[Value::I64(a), Value::I64(b)]).unwrap_or_else(|e| panic!("select: {e}"));
            h.pump(50);
        }
        Trig::SelectAll => {
            h.call(w, "select-all", &[]).unwrap_or_else(|e| panic!("select-all: {e}"));
            h.pump(50);
        }
        Trig::Activate => {
            h.call(w, "activate", &[]).unwrap_or_else(|e| panic!("activate: {e}"));
            h.pump(300);
        }
    }
}

/// Prepares the element a trigger acts on (mounted, with text) before observation starts.
fn prepare(h: &mut Harness, w: u64, t: Trig) {
    match t {
        Trig::Mount | Trig::Activate => {}
        Trig::Resize | Trig::Theme => {
            h.mount(w);
        }
        Trig::Select => {
            h.set(w, "text", Value::Str("hello world".into())).unwrap_or_else(|e| panic!("text: {e}"));
            h.mount(w);
        }
        Trig::SelectAll => {
            h.set(w, "text", Value::Str("hello world".into())).unwrap_or_else(|e| panic!("text: {e}"));
            h.set(w, "is-text-selection-enabled", Value::Bool(true)).unwrap_or_else(|e| panic!("is-text-selection-enabled: {e}"));
            h.mount(w);
        }
    }
}

/// One observe fact of a new `<tag>` through the real ABI v2.6 path (REACTIVITY.md 4.5, backend level):
/// 1. native change: announced at least once (`prop_changed(h, prop)`), synchronously on the UI thread;
/// 2. identical write: announced per `fires` (logged when `Any`);
/// 3. echo: a write through `set_prop` (the host op) is announced unless the prop is proportional (the host's
///    compare drops the echo of cheap props; the backend suppresses proportional ones);
/// 4. coalescing input: 10 native changes produce at least one announcement (more than one per change is logged; the
///    host coalesces them into one delivery per checkpoint);
/// 5. teardown: after `observe(off)` nothing is announced; nested `on`s need as many `off`s; destroying the handle
///    while observed ends the observation (no crash, nothing announced).
pub fn observe(tag: &str, prop: &str, check: ObsCheck, fires: Fires) {
    let mut h = Harness::new();
    let w = h.create(tag);
    let pid = h.prop_id(w, prop);
    match check {
        ObsCheck::Unsupported => {
            match h.observe(w, prop, true) {
                Err(ObserveError::Unsupported(_)) => {}
                Err(ObserveError::Failed(m)) => panic!("<{tag}> {prop}: observe failed ({m}), want SVL_OBSERVE_UNSUPPORTED"),
                Ok(()) => panic!("<{tag}> {prop}: observe accepted a prop the fact says WinUI cannot announce"),
            }
            return;
        }
        ObsCheck::Refused => {
            match h.observe(w, prop, true) {
                Err(ObserveError::Failed(_)) => {}
                other => panic!("<{tag}> {prop}: observe answered {other:?}, want -1 (constant)"),
            }
            return;
        }
        _ => {}
    }
    if let ObsCheck::Cross(t) = check {
        prepare(&mut h, w, t);
    }
    h.observe(w, prop, true).unwrap_or_else(|e| panic!("<{tag}> {prop}: observe: {e:?}"));
    assert!(h.backend().is_observed(w, pid));
    h.take_changed(w, pid);
    // The writes cycle through the test values and the initial read-back, so consecutive writes differ even for a
    // single test value (DP callbacks fire on effective changes only).
    let seq: Vec<Value> = match &check {
        ObsCheck::Change(values) => {
            let mut s = values.clone();
            s.push(h.get(w, prop).unwrap_or_else(|e| panic!("<{tag}> read {prop}: {e}")));
            s
        }
        _ => Vec::new(),
    };
    let change = |h: &mut Harness, n: usize| match &check {
        ObsCheck::Change(_) => {
            let v = &seq[n % seq.len()];
            h.set_natively(w, prop, v.clone()).unwrap_or_else(|e| panic!("<{tag}> {prop} = {v:?}: {e}"));
        }
        ObsCheck::Cross(t) => fire(h, w, *t, n),
        _ => {}
    };
    if matches!(check, ObsCheck::Connect) {
        h.observe(w, prop, false).unwrap_or_else(|e| panic!("<{tag}> {prop}: observe(off): {e:?}"));
        assert!(!h.backend().is_observed(w, pid));
        return;
    }
    // 1. a native change (every value once: at least one differs from the default)
    let values = if let ObsCheck::Change(v) = &check { v.len() } else { 1 };
    for n in 0..values {
        change(&mut h, n);
    }
    h.pump(20);
    let n1 = h.take_changed(w, pid);
    assert!(n1 >= 1, "<{tag}> {prop}: a native change was not announced");
    // 2. an identical write
    if let ObsCheck::Change(_) = &check {
        // The read-back written again (its wire form is accepted by the setter).
        let cur = h.get(w, prop).unwrap_or_else(|e| panic!("<{tag}> read {prop}: {e}"));
        if h.set_natively(w, prop, cur).is_ok() {
            h.pump(20);
            let n2 = h.take_changed(w, pid);
            println!("observe <{tag}> {prop}: identical write announced {n2} time(s)");
            match fires {
                Fires::OnChange => assert_eq!(n2, 0, "<{tag}> {prop}: an identical write was announced (fires: on_change)"),
                Fires::OnSet => assert!(n2 >= 1, "<{tag}> {prop}: an identical write was not announced (fires: on_set)"),
                Fires::Any => {}
            }
        }
        // 3. echo: our own writes (the host op) of every value of the cycle: at least one is a change
        {
            for v in &seq {
                h.set(w, prop, v.clone()).unwrap();
            }
            h.pump(20);
            let n3 = h.take_changed(w, pid);
            let proportional = crate::table::observe(pid).is_some_and(|o| o.proportional);
            if proportional {
                assert_eq!(n3, 0, "<{tag}> {prop}: our own write of a proportional prop was announced");
            } else {
                assert!(n3 >= 1, "<{tag}> {prop}: our own write was not announced (the host compare drops it, not the backend)");
            }
        }
    }
    // 4. coalescing input: 10 native changes (a one-shot trigger, mounting, activating, select-all, cannot repeat)
    let repeatable = match &check {
        // Selecting all again selects the same text: no change.
        ObsCheck::Cross(t) => !matches!(t, Trig::Mount | Trig::Activate | Trig::SelectAll),
        _ => true,
    };
    if repeatable {
        for n in 0..10 {
            change(&mut h, n + 1);
        }
        h.pump(20);
        let n4 = h.take_changed(w, pid);
        assert!(n4 >= 1, "<{tag}> {prop}: 10 native changes were not announced");
        if n4 > 10 {
            println!("observe <{tag}> {prop}: 10 native changes announced {n4} times");
        }
    }
    // 5. teardown: nesting, off, destroy while observed
    h.observe(w, prop, true).unwrap();
    h.observe(w, prop, false).unwrap();
    assert!(h.backend().is_observed(w, pid), "<{tag}> {prop}: a nested off ended the observation");
    h.observe(w, prop, false).unwrap();
    assert!(!h.backend().is_observed(w, pid));
    h.take_changed(w, pid);
    if repeatable {
        for n in 0..2 {
            change(&mut h, n);
        }
        h.pump(20);
        assert_eq!(h.take_changed(w, pid), 0, "<{tag}> {prop}: announced after observe(off)");
    }
    h.observe(w, prop, false).unwrap(); // off for an unobserved prop is 0
    h.observe(w, prop, true).unwrap();
    // Keep the element alive past the handle, so a callback that outlived `destroy` would still fire.
    let obj = h.backend().object(w).unwrap();
    h.backend().destroy(w);
    h.created.retain(|x| *x != w);
    h.pump(20);
    if let ObsCheck::Change(values) = &check {
        let set = crate::dispatch::PROP_SET[pid as usize - 1].unwrap();
        for v in values {
            let _ = set(&obj, v);
        }
    }
    h.pump(20);
    assert_eq!(h.take_changed(w, pid), 0, "<{tag}> {prop}: announced after the handle was destroyed");
    drop(obj);
}

/// Runs `tests` on this thread; see [`crate::harness::main`].
pub fn run_tests(tests: &[(&str, fn())]) -> i32 {
    crate::harness::main(tests)
}
