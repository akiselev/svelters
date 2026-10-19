//! Async native methods (`SVL_M_ASYNC`): the JS caller gets a Promise, the backend gets a one-shot
//! `CALLBACK` it completes exactly once.
//!
//! The descriptor lists the host-supplied completion callback as the last parameter (scope `async`).
//! The JS arguments are the other parameters, plus an optional trailing options object
//! `{ signal: AbortSignal }`. The completion callback is a small JS function created here, so the
//! backend finishes an operation with the ordinary `host->invoke_callback(id, [result])`, or with
//! `[ERROR]` for a failure, which rejects the Promise with a `NativeError`. Promise reactions run
//! at the next checkpoint like every other job: completing does not run user code.
//!
//! An abort signal maps to `backend->cancel(id)` (a `GCancellable`); the backend then completes the
//! operation with its cancellation error. A signal that is already aborted cancels right after the
//! start. A backend that fails to *start* the operation rejects the Promise with an Error.

use super::*;
use crate::convert::{self, Arena, Cx, JsPromises, JsReader, JsWriter};
use svelters_abi::*;

impl Core {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn op_call_async<E>(
        &self,
        e: &E,
        h: svl_handle,
        m: svl_id,
        k: svl_id,
        desc: &MemberDesc,
        mname: &str,
        args: &<E as JsReader>::V,
        is_static: bool,
    ) -> Result<<E as JsReader>::V, OpError>
    where
        E: JsReader + JsWriter<V = <E as JsReader>::V> + JsPromises,
    {
        let bad = |msg: String| OpError { kind: OpKind::Type, msg, native: None };
        let Some(cb_slot) = desc.params.last().filter(|p| p.tag == SVL_CALLBACK) else {
            return Err(format!("<{}>.{mname}: async method without a completion callback in its descriptor", self.kind_name(k)).into());
        };
        let visible = &desc.params[..desc.params.len() - 1];
        let mut given = JsReader::len(e, args)?;
        // An optional trailing `{ signal }` after the declared parameters.
        let mut signal = None;
        if given == visible.len() + 1 {
            let opts = JsReader::index(e, args, given - 1)?;
            if matches!(JsReader::kind(e, &opts), convert::JsKind::Object) {
                if JsReader::has_field(e, &opts, "signal")? {
                    let s = JsReader::field(e, &opts, "signal")?;
                    if !matches!(JsReader::kind(e, &s), convert::JsKind::Undefined | convert::JsKind::Null) {
                        signal = Some(s);
                    }
                }
                given -= 1;
            }
        }
        let required = visible.iter().filter(|p| !p.has(SVL_D_OPTIONAL)).count();
        if given < required || given > visible.len() {
            return Err(bad(format!(
                "<{}>.{mname}: expected {} argument(s), got {given}",
                self.kind_name(k),
                if required == visible.len() { required.to_string() } else { format!("{required} to {}", visible.len()) }
            )));
        }

        let mut ar = Arena::new();
        let registry = CallbackRegistry(self);
        let cx = Cx { types: self, utf16: self.utf16(), callbacks: Some(&registry), owner: h };
        let mut raw: Vec<svl_value> = Vec::with_capacity(visible.len() + 1);
        for (i, slot) in visible.iter().enumerate() {
            if i >= given {
                raw.push(svl_value::null());
                continue;
            }
            let v = JsReader::index(e, args, i)?;
            let prefix = format!("<{}>.{mname}({})", self.kind_name(k), slot.name.as_deref().unwrap_or("arg"));
            match convert::encode(e, &v, slot, &cx, &mut ar) {
                Ok(x) => raw.push(x),
                Err(c) => {
                    self.finish_callbacks(&mut ar, false);
                    return Err(self.conv_err(&prefix, c));
                }
            }
        }
        let aborted_already = match &signal {
            Some(s) => JsReader::has_field(e, s, "aborted")? && {
                let a = JsReader::field(e, s, "aborted")?;
                matches!(JsReader::kind(e, &a), convert::JsKind::Bool) && JsReader::boolean(e, &a)
            },
            None => false,
        };

        // The completion callback: a JS function that settles the Promise.
        let ti = convert::TypeSource::type_info(self, cb_slot.ty)
            .ok_or_else(|| OpError::from(format!("no descriptor for callback type {}", cb_slot.ty)))?;
        let id = self.cb_register_completion(h, cb_slot.ty, ti.desc.scope)?;
        let promise = match e.new_completion(id) {
            Ok(p) => p,
            Err(msg) => {
                self.cb_release(id);
                return Err(msg.into());
            }
        };
        raw.push(svl_value::typed(SVL_CALLBACK, cb_slot.ty, svl_value_u { u: id }));
        if !is_static && self.kind_of(h).is_err() {
            self.cb_release(id);
            self.finish_callbacks(&mut ar, false);
            return Err(format!("stale or foreign handle {h}").into());
        }
        // Static async functions (`callStatic` of an `SVL_M_STATIC | SVL_M_ASYNC` member) have no owner.
        let started = if is_static { self.backend.call_static_raw(k, m, &raw) } else { self.backend.call_raw(h, m, &raw) };
        match started {
            Ok(_) => {
                self.finish_callbacks(&mut ar, true);
                if let Some(s) = &signal {
                    if aborted_already {
                        self.backend.cancel(id);
                    } else if let Err(msg) = e.watch_abort(s, id) {
                        return Err(msg.into());
                    }
                }
            }
            Err(err) => {
                self.finish_callbacks(&mut ar, false);
                // Rejects the Promise (and releases the one-shot callback) instead of throwing.
                let oe = self.backend_err(err);
                let native = oe.native.unwrap_or(NativeError {
                    domain: "svelters".into(),
                    code: Value::Null,
                    message: oe.msg,
                });
                let _ = self.invoke_callback(id, &[Value::Error(Box::new(native))]);
                self.cb_release(id);
            }
        }
        Ok(promise)
    }

    /// `backend->cancel(id)`: the JS side aborted the operation behind a completion callback.
    pub(crate) fn op_cancel_async(&self, id: u64) {
        if self.st.borrow().callbacks.is_live(id) {
            self.backend.cancel(id);
        }
    }
}
