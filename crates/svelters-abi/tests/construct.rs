//! ABI v2.5: `construct` and `call_static` through the vtable, and their absence (null members) on a
//! backend that does not implement them.

use std::cell::RefCell;
use std::sync::Arc;
use svelters_abi::*;

#[derive(Default)]
struct Objects {
    calls: RefCell<Vec<String>>,
}

impl Backend for Objects {
    fn resolve(&self, _: NameKind, _: svl_id, _: &str) -> svl_id {
        1
    }
    fn create(&self, _: svl_id) -> Result<svl_handle, String> {
        Ok(1)
    }
    fn insert(&self, _: svl_handle, _: svl_handle, _: Option<svl_handle>, _: svl_id) -> Result<(), String> {
        Ok(())
    }
    fn remove(&self, _: svl_handle, _: svl_handle) -> Result<(), String> {
        Ok(())
    }
    fn destroy(&self, _: svl_handle) {}
    fn set_prop(&self, _: svl_handle, _: svl_id, _: &Value) -> Result<(), String> {
        Ok(())
    }
    fn get_prop(&self, _: svl_handle, _: svl_id) -> Result<Value, String> {
        Ok(Value::Null)
    }
    fn call(&self, _: svl_handle, _: svl_id, _: &[Value]) -> Result<Value, String> {
        Ok(Value::Null)
    }
    fn listen(&self, _: svl_handle, _: svl_id, _: bool) -> Result<(), String> {
        Ok(())
    }
    fn handle_info(&self, h: svl_handle) -> Result<svl_handle_info, String> {
        Ok(svl_handle_info { kind: 1, lifetime: if h >= 100 { SVL_LIFE_SHARED } else { SVL_LIFE_OWNED }, flags: 0 })
    }
    fn run(&self, _: &HostRef<'_>) -> Result<(), String> {
        Ok(())
    }
    fn waker(&self) -> Waker {
        Arc::new(|| {})
    }
    fn quit(&self) {}
    fn schedule_wake(&self, _: i64) {}

    fn supports_construct(&self) -> bool {
        true
    }
    fn construct(&self, kind: svl_id, props: &[svl_id], values: &[Value]) -> Result<Value, String> {
        if kind == 13 {
            return Err("cannot construct kind 13".into());
        }
        self.calls.borrow_mut().push(format!("construct {kind} {props:?} {values:?}"));
        Ok(Value::Handle(100))
    }
    fn call_static(&self, kind: svl_id, method: svl_id, args: &[Value]) -> Result<Value, String> {
        self.calls.borrow_mut().push(format!("static {kind}.{method} {args:?}"));
        Ok(Value::Str("static result".into()))
    }
}

struct Plain;

impl Backend for Plain {
    fn resolve(&self, _: NameKind, _: svl_id, _: &str) -> svl_id {
        0
    }
    fn create(&self, _: svl_id) -> Result<svl_handle, String> {
        Ok(1)
    }
    fn insert(&self, _: svl_handle, _: svl_handle, _: Option<svl_handle>, _: svl_id) -> Result<(), String> {
        Ok(())
    }
    fn remove(&self, _: svl_handle, _: svl_handle) -> Result<(), String> {
        Ok(())
    }
    fn destroy(&self, _: svl_handle) {}
    fn set_prop(&self, _: svl_handle, _: svl_id, _: &Value) -> Result<(), String> {
        Ok(())
    }
    fn get_prop(&self, _: svl_handle, _: svl_id) -> Result<Value, String> {
        Ok(Value::Null)
    }
    fn call(&self, _: svl_handle, _: svl_id, _: &[Value]) -> Result<Value, String> {
        Ok(Value::Null)
    }
    fn listen(&self, _: svl_handle, _: svl_id, _: bool) -> Result<(), String> {
        Ok(())
    }
    fn handle_info(&self, _: svl_handle) -> Result<svl_handle_info, String> {
        Ok(svl_handle_info::default())
    }
    fn run(&self, _: &HostRef<'_>) -> Result<(), String> {
        Ok(())
    }
    fn waker(&self) -> Waker {
        Arc::new(|| {})
    }
    fn quit(&self) {}
    fn schedule_wake(&self, _: i64) {}
}

#[test]
fn construct_and_call_static_reach_the_backend() {
    let vt = BackendVtable::new(Objects::default());
    let raw = unsafe { &*vt.as_raw() };
    assert!(raw.construct.is_some() && raw.call_static.is_some());
    let b = vt.backend_ref();
    assert!(b.has_construct() && b.has_call_static());
    let v = b.construct(2, &[5, 6], &[Value::Str("a".into()), Value::I64(3)]).unwrap();
    assert_eq!(v, Value::Handle(100));
    assert_eq!(b.construct(2, &[], &[]).unwrap(), Value::Handle(100), "create(kind) is construct with no props");
    assert_eq!(b.construct(13, &[], &[]).unwrap_err().message, "cannot construct kind 13");
    assert!(b.construct(2, &[1], &[]).unwrap_err().message.contains("differ in length"));
    assert_eq!(b.call_static(2, 9, &[Value::Str("/tmp".into())]).unwrap(), Value::Str("static result".into()));
    assert_eq!(b.handle_info(100).unwrap().lifetime, SVL_LIFE_SHARED);
}

#[test]
fn a_backend_without_construct_has_null_members_and_a_clear_error() {
    let vt = BackendVtable::new(Plain);
    let raw = unsafe { &*vt.as_raw() };
    assert!(raw.construct.is_none() && raw.call_static.is_none());
    let b = vt.backend_ref();
    assert!(!b.has_construct() && !b.has_call_static());
    assert!(b.construct(1, &[], &[]).unwrap_err().message.contains("does not implement construct"));
    assert!(b.call_static(1, 1, &[]).unwrap_err().message.contains("does not implement call_static"));
}

#[test]
fn the_new_constants_have_their_documented_values() {
    assert_eq!((SVL_LIFE_SHARED, SVL_T_TYPE, SVL_M_STATIC, SVL_M_CTOR), (5, 11, 16, 32));
}
