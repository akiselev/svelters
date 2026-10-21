//! The per-host table of exported objects: handles in the app range, identity by allocation.

use super::{APP_HANDLE_BIT, AnyObj, ExportObject, host_error};
use std::any::TypeId;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use svelters_abi::{NativeError, svl_handle, svl_id};

pub(crate) struct Entry {
    pub kind: svl_id,
    pub obj: AnyObj,
    key: (TypeId, usize),
    pub poisoned: bool,
}

#[derive(Default)]
pub(crate) struct AppObjects {
    next: Cell<u64>,
    map: RefCell<HashMap<svl_handle, Entry>>,
    by_ptr: RefCell<HashMap<(TypeId, usize), svl_handle>>,
}

impl AppObjects {
    /// The handle of `obj`: the existing one while the same allocation is live, else a new one.
    pub fn adopt(&self, kind: svl_id, ty: TypeId, obj: AnyObj) -> svl_handle {
        let key = (ty, obj.addr());
        if let Some(h) = self.by_ptr.borrow().get(&key) {
            return *h;
        }
        let n = self.next.get() + 1;
        self.next.set(n);
        let h = APP_HANDLE_BIT | n;
        self.map.borrow_mut().insert(h, Entry { kind, obj, key, poisoned: false });
        self.by_ptr.borrow_mut().insert(key, h);
        h
    }

    /// The kind of a live app handle.
    pub fn kind(&self, h: svl_handle) -> Option<svl_id> {
        self.map.borrow().get(&h).map(|e| e.kind)
    }

    pub fn is_poisoned(&self, h: svl_handle) -> bool {
        self.map.borrow().get(&h).is_some_and(|e| e.poisoned)
    }

    pub fn poison(&self, h: svl_handle) {
        if let Some(e) = self.map.borrow_mut().get_mut(&h) {
            e.poisoned = true;
        }
    }

    /// The object behind `h` as `T`'s holder (a clone of the `Rc`/`Arc`: no borrow is held).
    pub fn holder<T: ExportObject>(&self, h: svl_handle) -> Result<T::Holder, NativeError> {
        let obj = match self.map.borrow().get(&h) {
            Some(e) => e.obj.clone(),
            None => return Err(host_error("disposed", format!("{} #{} was disposed", T::NAME, h & !APP_HANDLE_BIT))),
        };
        T::__holder(&obj).ok_or_else(|| host_error("type", format!("expected {}, got another object type", T::NAME)))
    }

    /// Drops our reference. Unknown handles are ignored (released twice, or from an old runtime).
    pub fn release(&self, h: svl_handle) -> bool {
        let e = self.map.borrow_mut().remove(&h);
        match e {
            Some(e) => {
                self.by_ptr.borrow_mut().remove(&e.key);
                // The object's `Drop` runs here, with no table borrow held.
                drop(e);
                true
            }
            None => false,
        }
    }

    pub fn handles(&self) -> Vec<svl_handle> {
        let mut v: Vec<svl_handle> = self.map.borrow().keys().copied().collect();
        v.sort_unstable();
        v
    }

    pub fn len(&self) -> usize {
        self.map.borrow().len()
    }
}
