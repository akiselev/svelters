//! Shared pieces of the hand-written kinds (`virtual-list`, menus) and of typelib calls: the kind-id
//! range of hand-written kinds, call errors (`GError` as an `ERROR` value) and the async operations
//! of folded `_async`/`_finish` pairs.
//!
//! The NT0-era `gio-file` service that lived here (hand-written callbacks, async and `GError` on
//! `GFile`) was retired by N1: the same members now come from the GIR through the generic invoker
//! (`gi.rs`, `closures.rs`) and `Gio.File` in `svelters:gtk/gio`.

use super::*;
use gtk::gio;

/// Hand-written kinds live in 0x4000..0x5000 (`virtual-list` 0x4002, menus 0x44xx).
pub fn is_extra(kind: u32) -> bool {
    (0x4000..0x5000).contains(&kind)
}

/// A failure of a call: a plain message, or a `GError` that crosses as an `ERROR` value.
pub(super) enum CallErr {
    Plain(String),
    Gerror(glib::Error),
}

impl From<String> for CallErr {
    fn from(s: String) -> CallErr {
        CallErr::Plain(s)
    }
}

impl From<&str> for CallErr {
    fn from(s: &str) -> CallErr {
        CallErr::Plain(s.to_string())
    }
}

impl From<glib::Error> for CallErr {
    fn from(e: glib::Error) -> CallErr {
        CallErr::Gerror(e)
    }
}

pub(super) fn args_of<'a>(args: *const svl_value, n: usize) -> &'a [svl_value] {
    if args.is_null() || n == 0 { &[] } else { unsafe { std::slice::from_raw_parts(args, n) } }
}

/// A folded async call in flight: who started it and how to cancel it.
pub(super) struct Pending {
    pub(super) owner: u64,
    pub(super) cancellable: gio::Cancellable,
}

impl Inner {
    pub(super) fn extra_kind(&self, h: u64) -> Option<u32> {
        self.state.borrow().entries.get(&h).map(|e| e.kind).filter(|k| is_extra(*k) && !super::menus::is_menu_kind(*k))
    }

    /// A `GError` as the ABI carries it: domain, the member name of the domain's error enum (any
    /// typelib-registered domain: `G_IO_ERROR` -> `GIOErrorEnum`, `GTK_DIALOG_ERROR`, ...) or the bare
    /// code, and the message.
    pub(super) fn gerror_value(&self, e: &glib::Error) -> Value {
        let domain = e.domain().as_str().to_string();
        let enum_t = self.gi().ok().and_then(|g| g.error_enum(e.domain()));
        let raw = unsafe { (*glib::translate::ToGlibPtr::<*const glib::ffi::GError>::to_glib_none(e).0).code } as i64;
        let code = match enum_t {
            Some(t) if glib::EnumClass::with_type(t).is_some_and(|c| c.value(raw as i32).is_some()) => {
                Value::Enum { ty: self.types.borrow_mut().id(t), value: raw }
            }
            _ => Value::I64(raw),
        };
        Value::Error(Box::new(NativeError { domain, code, message: e.message().to_string() }))
    }

    pub(super) fn write_error(&self, e: &glib::Error, out: &mut svl_value) {
        let Value::Error(n) = self.gerror_value(e) else { return };
        let mut buf = self.out.borrow_mut();
        *buf = StrBuf::default();
        let dom = buf.str_value(&n.domain);
        let code = match &n.code {
            Value::Enum { ty, value } => svl_value::typed(SVL_ENUM, *ty, svl_value_u { i: *value }),
            Value::I64(i) => svl_value::new(SVL_I64, svl_value_u { i: *i }),
            _ => svl_value::new(SVL_I64, svl_value_u { i: 0 }),
        };
        let msg = buf.str_value(&n.message);
        *out = buf.list_value(SVL_ERROR, 0, vec![dom, code, msg]);
    }

    /// `backend->cancel(callback)`: cancel the async operation completing `callback`.
    pub(super) fn cancel_callback(&self, callback: u64) -> bool {
        let c = self.pending.borrow().get(&callback).map(|p| p.cancellable.clone());
        match c {
            Some(c) => {
                c.cancel();
                true
            }
            None => false,
        }
    }

    /// The owner of running async operations is destroyed: stop the native work. (The host has already
    /// settled or dropped their completions.)
    pub(super) fn cancel_owned(&self, h: u64) {
        let gone: Vec<(u64, gio::Cancellable)> = {
            let mut p = self.pending.borrow_mut();
            let ids: Vec<u64> = p.iter().filter(|(_, x)| x.owner == h).map(|(id, _)| *id).collect();
            ids.into_iter().filter_map(|id| p.remove(&id).map(|x| (id, x.cancellable))).collect()
        };
        for (_, c) in gone {
            c.cancel();
        }
    }
}
