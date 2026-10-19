//! ABI v2.6: the nullable backend member `observe`, the nullable host member `prop_changed`, and the
//! `SVL_M_OBSERVABLE` flag, through the safe wrappers.

use std::cell::RefCell;
use std::sync::Arc;
use svelters_abi::*;

/// Observes prop 1 of handle 7; prop 2 has no change notification; everything else is unknown.
#[derive(Default)]
struct Watcher {
    calls: RefCell<Vec<(svl_handle, svl_id, bool)>>,
}

impl Backend for Watcher {
    fn resolve(&self, _: NameKind, _: svl_id, _: &str) -> svl_id {
        0
    }
    fn create(&self, _: svl_id) -> Result<svl_handle, String> {
        Ok(7)
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
    fn run(&self, host: &HostRef<'_>) -> Result<(), String> {
        // A native change on an observed property.
        host.prop_changed(7, 1);
        Ok(())
    }
    fn waker(&self) -> Waker {
        Arc::new(|| {})
    }
    fn quit(&self) {}
    fn schedule_wake(&self, _: i64) {}
    fn supports_observe(&self) -> bool {
        true
    }
    fn observe(&self, h: svl_handle, prop: svl_id, on: bool) -> Result<(), ObserveError> {
        self.calls.borrow_mut().push((h, prop, on));
        match (h, prop) {
            (7, 1) => Ok(()),
            (7, 2) => Err(ObserveError::Unsupported("prop 2 has no NOTIFY signal".into())),
            _ => Err(format!("stale handle {h} or unknown prop {prop}").into()),
        }
    }
    fn describe(&self, what: NameKind, _: svl_id, member: svl_id) -> Option<MemberDesc> {
        (what == NameKind::Prop).then(|| MemberDesc {
            name: format!("p{member}"),
            params: vec![],
            ret: SlotDesc::new(SVL_I64),
            flags: if member == 1 { SVL_M_OBSERVABLE } else { 0 },
        })
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
fn observe_reaches_the_backend_with_distinct_codes() {
    let vt = BackendVtable::new(Watcher::default());
    assert!(unsafe { &*vt.as_raw() }.observe.is_some());
    let b = vt.backend_ref();
    assert!(b.has_observe());
    b.observe(7, 1, true).unwrap();
    b.observe(7, 1, false).unwrap();
    let e = b.observe(7, 2, true).unwrap_err();
    assert_eq!((e.code, e.message.as_str()), (SVL_OBSERVE_UNSUPPORTED, "prop 2 has no NOTIFY signal"));
    let e = b.observe(9, 1, true).unwrap_err();
    assert_eq!(e.code, -1);
    assert!(e.message.contains("stale handle 9"), "{}", e.message);
    assert_eq!(b.describe(NameKind::Prop, 1, 1).unwrap().flags, SVL_M_OBSERVABLE);
    assert_eq!(b.describe(NameKind::Prop, 1, 2).unwrap().flags, 0);
}

#[test]
fn a_backend_without_observe_has_a_null_member_and_answers_unsupported() {
    let vt = BackendVtable::new(Plain);
    assert!(unsafe { &*vt.as_raw() }.observe.is_none());
    let b = vt.backend_ref();
    assert!(!b.has_observe());
    let e = b.observe(1, 1, true).unwrap_err();
    assert_eq!(e.code, SVL_OBSERVE_UNSUPPORTED);
    assert!(e.message.contains("does not implement observe"));
}

struct Host(std::rc::Rc<RefCell<Vec<(svl_handle, svl_id)>>>);

impl HostCallbacks for Host {
    fn post_event(&self, _: svl_handle, _: svl_id, _: &[Value]) {}
    fn checkpoint(&self) {}
    fn call_sync(&self, _: svl_handle, _: svl_id, _: &[Value]) -> Result<Value, SyncError> {
        Ok(Value::Null)
    }
    fn on_destroyed(&self, _: svl_handle, _: u32) {}
    fn frame(&self) {}
    fn prop_changed(&self, h: svl_handle, prop: svl_id) {
        self.0.borrow_mut().push((h, prop));
    }
}

struct Quiet;

impl HostCallbacks for Quiet {
    fn post_event(&self, _: svl_handle, _: svl_id, _: &[Value]) {}
    fn checkpoint(&self) {}
    fn call_sync(&self, _: svl_handle, _: svl_id, _: &[Value]) -> Result<Value, SyncError> {
        Ok(Value::Null)
    }
    fn on_destroyed(&self, _: svl_handle, _: u32) {}
    fn frame(&self) {}
}

#[test]
fn prop_changed_reaches_the_host_and_a_null_member_is_a_no_op() {
    let seen = std::rc::Rc::new(RefCell::new(Vec::new()));
    let hvt = HostVtable::new(Host(seen.clone()));
    let bvt = BackendVtable::new(Watcher::default());
    bvt.backend_ref().run(&hvt).unwrap();
    assert_eq!(*seen.borrow(), vec![(7, 1)]);

    // The default HostCallbacks::prop_changed ignores the call.
    let quiet = HostVtable::new(Quiet);
    quiet.host_ref().prop_changed(7, 1);
    // A hand-built host that leaves the member NULL: HostRef skips it.
    let raw = svl_host { prop_changed: None, ..unsafe { std::ptr::read(quiet.as_raw()) } };
    unsafe { HostRef::from_raw(&raw) }.prop_changed(7, 1);
}

#[test]
fn the_v2_6_constants_have_their_documented_values() {
    assert_eq!((SVL_M_OBSERVABLE, SVL_OBSERVE_UNSUPPORTED), (64, -2));
}
