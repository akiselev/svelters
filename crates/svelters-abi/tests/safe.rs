use std::cell::{Cell, RefCell};
use std::sync::Arc;
use svelters_abi::*;

#[test]
fn value_roundtrips_through_raw() {
    let v = Value::List(vec![
        Value::Null,
        Value::Bool(true),
        Value::I64(-7),
        Value::F64(2.5),
        Value::Str("héllo".into()),
        Value::Str(String::new()),
        Value::Handle(9),
        Value::List(vec![Value::Str("nested".into())]),
    ]);
    let back = v.with_raw(|raw| unsafe { Value::from_raw(raw) });
    assert_eq!(v, back);
}

#[derive(Default)]
struct Recorder {
    calls: RefCell<Vec<String>>,
    quit: Cell<bool>,
}

impl Backend for Recorder {
    fn resolve(&self, what: NameKind, kind: svl_id, name: &str) -> svl_id {
        if name == "unknown" {
            0
        } else {
            (what as u32) * 100 + kind + name.len() as u32
        }
    }
    fn create(&self, kind: svl_id) -> Result<svl_handle, String> {
        if kind == 13 {
            return Err("cannot create kind 13".into());
        }
        self.calls.borrow_mut().push(format!("create {kind}"));
        Ok(42)
    }
    fn insert(
        &self,
        p: svl_handle,
        c: svl_handle,
        b: Option<svl_handle>,
        s: svl_id,
    ) -> Result<(), String> {
        self.calls
            .borrow_mut()
            .push(format!("insert {p} {c} {b:?} {s}"));
        Ok(())
    }
    fn remove(&self, _: svl_handle, _: svl_handle) -> Result<(), String> {
        panic!("backend bug")
    }
    fn destroy(&self, h: svl_handle) {
        self.calls.borrow_mut().push(format!("destroy {h}"));
    }
    fn set_prop(&self, _: svl_handle, _: svl_id, v: &Value) -> Result<(), String> {
        self.calls.borrow_mut().push(format!("set {v:?}"));
        Ok(())
    }
    fn get_prop(&self, _: svl_handle, _: svl_id) -> Result<Value, String> {
        Ok(Value::List(vec![Value::Str("out".into()), Value::I64(1)]))
    }
    fn call(&self, _: svl_handle, _: svl_id, args: &[Value]) -> Result<Value, String> {
        Ok(Value::I64(args.len() as i64))
    }
    fn listen(&self, _: svl_handle, _: svl_id, _: bool) -> Result<(), String> {
        Err("no signals".into())
    }
    fn run(&self, host: &HostRef<'_>) -> Result<(), String> {
        host.post_event(42, 5, &[Value::Str("x".into())]);
        host.checkpoint();
        Ok(())
    }
    fn waker(&self) -> Waker {
        Arc::new(|| {})
    }
    fn quit(&self) {
        self.quit.set(true);
    }
}

#[test]
fn trait_backend_through_vtable() {
    let vt = BackendVtable::new(Recorder::default());
    let b = vt.backend_ref();
    assert_eq!(b.resolve(NameKind::Prop, 2, "abc"), 105);
    assert_eq!(b.resolve(NameKind::Kind, 0, "unknown"), 0);
    assert_eq!(b.create(1).unwrap(), 42);
    let e = b.create(13).unwrap_err();
    assert_eq!(e.message, "cannot create kind 13");
    b.insert(1, 2, None, 0).unwrap();
    b.insert(1, 2, Some(3), 4).unwrap();
    b.set_prop(1, 1, &Value::Str("s".into())).unwrap();
    assert_eq!(
        b.get_prop(1, 1).unwrap(),
        Value::List(vec![Value::Str("out".into()), Value::I64(1)])
    );
    assert_eq!(
        b.call(1, 1, &[Value::Null, Value::Null]).unwrap(),
        Value::I64(2)
    );
    assert_eq!(b.listen(1, 1, true).unwrap_err().message, "no signals");
    // A panic in the backend becomes an error, not an unwind across the C boundary.
    assert_eq!(b.remove(1, 2).unwrap_err().message, "panic in backend");
    b.wake_handle().wake();
    b.quit();
    b.destroy(1);
}

struct Host {
    events: RefCell<Vec<(svl_handle, svl_id, Vec<Value>)>>,
    checkpoints: Cell<u32>,
}

impl HostCallbacks for Rc2 {
    fn post_event(&self, h: svl_handle, s: svl_id, a: &[Value]) {
        self.0.events.borrow_mut().push((h, s, a.to_vec()));
    }
    fn checkpoint(&self) {
        self.0.checkpoints.set(self.0.checkpoints.get() + 1);
    }
    fn call_sync(&self, _: svl_handle, _: svl_id, _: &[Value]) -> Result<Value, String> {
        Ok(Value::Bool(true))
    }
    fn on_destroyed(&self, _: svl_handle) {}
}

struct Rc2(std::rc::Rc<Host>);

#[test]
fn backend_run_drives_host_callbacks() {
    let host = std::rc::Rc::new(Host {
        events: RefCell::default(),
        checkpoints: Cell::new(0),
    });
    let hvt = HostVtable::new(Rc2(host.clone()));
    let bvt = BackendVtable::new(Recorder::default());
    bvt.backend_ref().run(&hvt).unwrap();
    assert_eq!(host.checkpoints.get(), 1);
    assert_eq!(
        host.events.borrow()[0],
        (42, 5, vec![Value::Str("x".into())])
    );
    assert_eq!(
        hvt.host_ref().call_sync(1, 1, &[]).unwrap(),
        Value::Bool(true)
    );
}
