//! The callback table: JS functions the backend may call back (ABI `CALLBACK` values).
//!
//! A callback is an id (`generation << 48 | index`) tied to an owner handle and a lifetime scope.
//! The function itself lives in the engine ([`CallbackEngine`]); this table only decides whether an
//! id is still callable. Rules (docs/notes/js-native-serdes.md section 2.2, 5.4):
//!
//! - `call` scope: released when the backend call that received it returns;
//! - `async` scope: released after its first invocation;
//! - `notified` scope: released when the backend calls `release_callback` (GDestroyNotify), or when
//!   the owner handle dies, whichever comes first;
//! - `forever` is refused at registration.
//!
//! A call on a released id is a clean error, never a crash. The generation in the id makes a reused
//! slot a different id. JS function objects are dropped at the next checkpoint (`process_releases`),
//! never inside `on_destroyed` or a finalizer. An invocation is synchronous and re-entrant like
//! `call_sync`: it takes no job drain and no checkpoint, and tree operations are refused while it
//! runs (`Core::tree_ops_allowed`).

use super::*;
use crate::engine::JsError;
use svelters_abi::SlotDesc;

/// What the engine provides for callbacks.
pub(crate) trait CallbackEngine {
    /// Runs the stored function for `id` with `args` and returns its result converted per the
    /// callback signature `sig` (null for a void signature). Must work both from idle and from inside
    /// a host op (nested).
    fn invoke(&self, id: u64, sig: svl_id, args: &[Value]) -> Result<Value, JsError>;
    /// Drops the stored function. Runs no JS.
    fn drop_function(&self, id: u64);
}

struct Slot {
    gen_: u16,
    live: bool,
    owner: svl_handle,
    scope: u16,
    sig: svl_id,
}

#[derive(Default)]
pub(crate) struct CbTable {
    slots: Vec<Slot>,
    free: Vec<u32>,
    by_owner: HashMap<svl_handle, Vec<u64>>,
    /// Released ids whose JS function is dropped at the next checkpoint.
    to_free: Vec<u64>,
}

fn pack(gen_: u16, index: u32) -> u64 {
    ((gen_ as u64) << 48) | (index as u64 + 1)
}

fn unpack(id: u64) -> (u16, usize) {
    ((id >> 48) as u16, ((id & 0xFFFF_FFFF_FFFF) as usize).wrapping_sub(1))
}

impl CbTable {
    pub(crate) fn is_live(&self, id: u64) -> bool {
        self.live(id).is_some()
    }

    fn live(&self, id: u64) -> Option<&Slot> {
        let (g, i) = unpack(id);
        self.slots.get(i).filter(|s| s.live && s.gen_ == g)
    }
}

impl Core {
    pub(crate) fn set_callback_engine(&self, e: Rc<dyn CallbackEngine>) {
        *self.cb_engine.borrow_mut() = Some(e);
    }

    /// Reserves an id for a function passed to the backend as argument of `owner`'s call.
    pub(crate) fn cb_register(&self, owner: svl_handle, sig: svl_id, scope: u16) -> Result<u64, String> {
        match scope {
            SVL_CB_CALL | SVL_CB_ASYNC | SVL_CB_NOTIFIED => {}
            SVL_CB_FOREVER => {
                return Err("callbacks with scope 'forever' are refused: they could never be released".into());
            }
            _ => return Err("the callback type declares no lifetime scope".into()),
        }
        let mut st = self.st.borrow_mut();
        let t = &mut st.callbacks;
        let index = match t.free.pop() {
            Some(i) => i,
            None => {
                t.slots.push(Slot { gen_: 0, live: false, owner: 0, scope: 0, sig: 0 });
                (t.slots.len() - 1) as u32
            }
        };
        let s = &mut t.slots[index as usize];
        s.live = true;
        s.owner = owner;
        s.scope = scope;
        s.sig = sig;
        let id = pack(s.gen_, index);
        t.by_owner.entry(owner).or_default().push(id);
        Ok(id)
    }

    /// Marks a callback dead (idempotent). Returns whether it was live.
    pub(crate) fn cb_release(&self, id: u64) -> bool {
        let mut st = self.st.borrow_mut();
        let t = &mut st.callbacks;
        let (g, i) = unpack(id);
        let Some(s) = t.slots.get_mut(i).filter(|s| s.live && s.gen_ == g) else {
            return false;
        };
        s.live = false;
        s.gen_ = s.gen_.wrapping_add(1);
        let owner = s.owner;
        t.free.push(i as u32);
        if let Some(v) = t.by_owner.get_mut(&owner) {
            v.retain(|x| *x != id);
            if v.is_empty() {
                t.by_owner.remove(&owner);
            }
        }
        t.to_free.push(id);
        true
    }

    /// The owner died: every callback tied to it is dead.
    pub(crate) fn cb_release_owner(&self, owner: svl_handle) {
        let ids = self.st.borrow().callbacks.by_owner.get(&owner).cloned().unwrap_or_default();
        for id in ids {
            self.cb_release(id);
        }
    }

    /// Drops the JS functions of released callbacks. Called at checkpoints, outside any JS entry.
    pub(crate) fn cb_process_frees(&self) {
        let ids = std::mem::take(&mut self.st.borrow_mut().callbacks.to_free);
        if ids.is_empty() {
            return;
        }
        let eng = self.cb_engine.borrow().clone();
        if let Some(eng) = eng {
            for id in ids {
                eng.drop_function(id);
            }
        }
    }

    pub(crate) fn live_callbacks(&self) -> usize {
        self.st.borrow().callbacks.slots.iter().filter(|s| s.live).count()
    }

    /// False while a native callback runs JS: creating, inserting, removing and destroying nodes
    /// would change the tree under the native operation that is waiting for the callback.
    pub(crate) fn tree_ops_allowed(&self) -> Result<(), OpError> {
        if self.cb_depth.get() > 0 {
            return Err(OpError {
                kind: OpKind::Error,
                msg: "tree operations (create, insert, remove, destroy) are not allowed inside a native callback".into(),
                native: None,
            });
        }
        Ok(())
    }

    /// `host->invoke_callback`: runs the JS function behind `id` synchronously.
    pub(crate) fn invoke_callback(&self, id: u64, args: &[Value]) -> Result<Value, String> {
        let (owner, scope, sig) = {
            let st = self.st.borrow();
            let s = st
                .callbacks
                .live(id)
                .ok_or_else(|| "callback was released or its owner is gone".to_string())?;
            (s.owner, s.scope, s.sig)
        };
        if owner != 0 && !self.st.borrow().handles.contains_key(&owner) {
            self.cb_release(id);
            return Err("callback owner is gone".into());
        }
        let eng = self.cb_engine.borrow().clone().ok_or("no JS engine for callbacks")?;
        // From idle, hold the JS guard so a nested checkpoint is deferred; from inside a host op the
        // guard is already held up the stack.
        let guard = if self.busy.get() == 0 { self.enter() } else { None };
        let nested = guard.is_none();
        self.cb_depth.set(self.cb_depth.get() + 1);
        let res = eng.invoke(id, sig, args);
        self.cb_depth.set(self.cb_depth.get() - 1);
        if scope == SVL_CB_ASYNC {
            self.cb_release(id);
        }
        drop(guard);
        if !nested {
            // Promise reactions queued by the callback run at a checkpoint the backend loop schedules.
            self.backend.wake();
        }
        match res {
            Ok(v) => Ok(v),
            Err(e) => {
                let msg = e.message.clone();
                self.report(e);
                Err(format!("callback threw: {msg}"))
            }
        }
    }

    /// The return slot of the callback signature `sig`.
    pub(crate) fn callback_ret_slot(&self, sig: svl_id) -> Option<SlotDesc> {
        use crate::convert::TypeSource;
        self.type_info(sig).map(|t| t.desc.ret.clone())
    }

    /// `host->release_callback`: the backend dropped its reference.
    pub(crate) fn release_callback_from_native(&self, id: u64) {
        self.cb_release(id);
    }
}

pub(crate) struct CallbackRegistry<'a>(pub &'a Core);

impl crate::convert::CallbackSink for CallbackRegistry<'_> {
    fn register(&self, owner: svl_handle, sig: svl_id, scope: u16) -> Result<u64, String> {
        self.0.cb_register(owner, sig, scope)
    }
}
