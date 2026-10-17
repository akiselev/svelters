//! QuickJS implementation of the converter's engine traits ([`JsReader`], [`JsWriter`]).

use crate::convert::{Guard, JsKind, JsReader, JsWriter, StrView};
use crate::core::{Core, Name};
use rquickjs::qjs;
use rquickjs::{Array, Ctx, Function, IntoJs, Object, Type, Value};
use std::ptr::NonNull;
use std::rc::Rc;
use svelters_abi::{svl_handle, svl_id};

type R<T> = Result<T, String>;

/// Frees a borrowed C string (UTF-8 or UTF-16) when the arena that holds it is dropped.
struct CStr8 {
    ctx: NonNull<qjs::JSContext>,
    ptr: *const std::ffi::c_char,
}

impl Drop for CStr8 {
    fn drop(&mut self) {
        unsafe { qjs::JS_FreeCString(self.ctx.as_ptr(), self.ptr) }
    }
}

struct CStr16 {
    ctx: NonNull<qjs::JSContext>,
    ptr: *const u16,
}

impl Drop for CStr16 {
    fn drop(&mut self) {
        unsafe { qjs::JS_FreeCStringUTF16(self.ctx.as_ptr(), self.ptr) }
    }
}

/// Reads and builds JS values in one QuickJS context.
pub(crate) struct QjsConv<'js> {
    pub ctx: Ctx<'js>,
    pub core: Rc<Core>,
    /// Where function arguments are stored (`None` where callbacks make no sense, e.g. decoding).
    cbs: Option<Rc<super::QjsCallbacks>>,
    /// An exception thrown by user JS while a value was read (a getter, a Proxy trap). The op
    /// rethrows it as is instead of wrapping its message.
    pending: std::cell::RefCell<Option<Value<'js>>>,
}

impl<'js> QjsConv<'js> {
    pub(crate) fn new(ctx: Ctx<'js>, core: Rc<Core>, cbs: Option<Rc<super::QjsCallbacks>>) -> QjsConv<'js> {
        QjsConv {
            ctx,
            core,
            cbs,
            pending: Default::default(),
        }
    }

    /// The exception user JS threw during the last conversion, if any.
    pub(crate) fn take_pending(&self) -> Option<Value<'js>> {
        self.pending.borrow_mut().take()
    }

    fn err(&self, e: rquickjs::Error) -> String {
        match e {
            rquickjs::Error::Exception => {
                let ex = self.ctx.catch();
                *self.pending.borrow_mut() = Some(ex.clone());
                match ex.as_object().and_then(|o| rquickjs::Exception::from_object(o.clone())) {
                    Some(ex) => ex.message().unwrap_or_default(),
                    None => "JS exception".to_string(),
                }
            }
            other => other.to_string(),
        }
    }

    fn internal(&self) -> R<Object<'js>> {
        self.ctx
            .globals()
            .get::<_, Object>("__svelters_internal")
            .map_err(|e| self.err(e))
    }

    /// Runs `f` with a member name: a number is a pre-resolved id, a string is borrowed (no copy).
    pub(crate) fn with_name<T>(&self, v: &Value<'js>, f: impl FnOnce(Name<'_>) -> T) -> R<T> {
        if let Some(n) = v.as_int() {
            return u32::try_from(n)
                .map(|id| f(Name::Id(id)))
                .map_err(|_| "invalid member id".to_string());
        }
        if v.is_string() {
            let mut guards = Vec::new();
            let sv = JsReader::string(self, v, false, &mut guards)?;
            let bytes = unsafe { std::slice::from_raw_parts(sv.ptr, sv.len) };
            let s = std::str::from_utf8(bytes).map_err(|_| "name is not valid UTF-8".to_string())?;
            return Ok(f(Name::Str(s)));
        }
        Err("member must be a name or a resolved id".into())
    }
}

impl<'js> JsReader for QjsConv<'js> {
    type V = Value<'js>;

    fn kind(&self, v: &Value<'js>) -> JsKind {
        match v.type_of() {
            Type::Undefined | Type::Uninitialized => JsKind::Undefined,
            Type::Null => JsKind::Null,
            Type::Bool => JsKind::Bool,
            Type::Int | Type::Float => JsKind::Number,
            Type::String => JsKind::String,
            Type::BigInt => JsKind::BigInt,
            Type::Array => JsKind::Array,
            Type::Function | Type::Constructor => JsKind::Function,
            Type::Object | Type::Proxy => JsKind::Object,
            _ => JsKind::Other,
        }
    }

    fn boolean(&self, v: &Value<'js>) -> bool {
        v.as_bool().unwrap_or(false)
    }

    fn number(&self, v: &Value<'js>) -> f64 {
        match v.as_int() {
            Some(i) => i as f64,
            None => v.as_float().unwrap_or(f64::NAN),
        }
    }

    fn bigint(&self, v: &Value<'js>) -> Option<i64> {
        v.as_big_int().and_then(|b| b.clone().to_i64().ok())
    }

    fn string(&self, v: &Value<'js>, want_utf16: bool, guards: &mut Vec<Guard>) -> R<StrView> {
        let ctx = self.ctx.as_raw();
        let mut len: u64 = 0;
        if want_utf16 {
            let p = unsafe { qjs::JS_ToCStringLenUTF16(ctx.as_ptr(), &mut len, v.as_raw()) };
            if p.is_null() {
                return Err(self.err(rquickjs::Error::Exception));
            }
            guards.push(Box::new(CStr16 { ctx, ptr: p }));
            Ok(StrView {
                ptr: p.cast(),
                len: len as usize,
                utf16: true,
                nul_term: false,
            })
        } else {
            let p = unsafe { qjs::JS_ToCStringLen2(ctx.as_ptr(), &mut len, v.as_raw(), false) };
            if p.is_null() {
                return Err(self.err(rquickjs::Error::Exception));
            }
            guards.push(Box::new(CStr8 { ctx, ptr: p }));
            Ok(StrView {
                ptr: p.cast(),
                len: len as usize,
                utf16: false,
                nul_term: true,
            })
        }
    }

    fn field(&self, obj: &Value<'js>, key: &str) -> R<Value<'js>> {
        let o = obj.as_object().ok_or("not an object")?;
        o.get::<_, Value>(key).map_err(|e| self.err(e))
    }

    fn has_field(&self, obj: &Value<'js>, key: &str) -> R<bool> {
        let o = obj.as_object().ok_or("not an object")?;
        // `in` semantics: inherited and Proxy `has` traps count, like a plain property read.
        o.contains_key(key).map_err(|e| self.err(e))
    }

    fn keys(&self, obj: &Value<'js>) -> R<Vec<String>> {
        let o = obj.as_object().ok_or("not an object")?;
        o.keys::<String>()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| self.err(e))
    }

    fn len(&self, arr: &Value<'js>) -> R<usize> {
        Ok(arr.as_array().ok_or("not an array")?.len())
    }

    fn index(&self, arr: &Value<'js>, i: usize) -> R<Value<'js>> {
        arr.as_array()
            .ok_or("not an array")?
            .get::<Value>(i)
            .map_err(|e| self.err(e))
    }

    fn store_function(&self, v: &Value<'js>, id: u64) -> R<()> {
        let f = v.as_function().ok_or("not a function")?;
        let cbs = self.cbs.as_ref().ok_or("callbacks are not available here")?;
        cbs.funcs.borrow_mut().insert(id, rquickjs::Persistent::save(&self.ctx, f.clone()));
        Ok(())
    }

    fn date_ms(&self, v: &Value<'js>) -> Option<f64> {
        if v.type_of() != Type::Object {
            return None;
        }
        let internal = self.internal().ok()?;
        let f = internal.get::<_, Function>("dateMs").ok()?;
        f.call::<_, Option<f64>>((v.clone(),)).ok().flatten()
    }

    fn is_bytes(&self, v: &Value<'js>) -> bool {
        if v.type_of() != Type::Object {
            return false;
        }
        if unsafe { qjs::JS_IsArrayBuffer(v.as_raw()) } {
            return true;
        }
        let Ok(internal) = self.internal() else { return false };
        let Ok(f) = internal.get::<_, Function>("isView") else { return false };
        f.call::<_, bool>((v.clone(),)).unwrap_or(false)
    }

    fn bytes(&self, v: &Value<'js>, _guards: &mut Vec<Guard>) -> R<(*const u8, usize)> {
        let ctx = self.ctx.as_raw();
        unsafe {
            if qjs::JS_IsArrayBuffer(v.as_raw()) {
                let mut size = 0u64;
                let p = qjs::JS_GetArrayBuffer(ctx.as_ptr(), &mut size, v.as_raw());
                if p.is_null() && size != 0 {
                    return Err("ArrayBuffer is detached".into());
                }
                return Ok((p, size as usize));
            }
            let (mut off, mut len, mut bpe) = (0u64, 0u64, 0u64);
            let buf = qjs::JS_GetTypedArrayBuffer(ctx.as_ptr(), v.as_raw(), &mut off, &mut len, &mut bpe);
            // Wrap so the temporary reference is released.
            let buf = Value::from_raw(self.ctx.clone(), buf);
            if buf.is_exception() {
                return Err(self.err(rquickjs::Error::Exception));
            }
            let mut size = 0u64;
            let p = qjs::JS_GetArrayBuffer(ctx.as_ptr(), &mut size, buf.as_raw());
            if p.is_null() && len != 0 {
                return Err("typed array is detached".into());
            }
            // The typed array (`v`) keeps the buffer alive; the pointer stays valid with it.
            Ok((if p.is_null() { p } else { p.add(off as usize) }, len as usize))
        }
    }

    fn typed_variant(&self, v: &Value<'js>) -> R<Option<(String, Value<'js>)>> {
        let Some(o) = v.as_object() else { return Ok(None) };
        let marker: Value = o.get("__svlVariant").map_err(|e| self.err(e))?;
        let Some(a) = marker.as_array() else { return Ok(None) };
        let sig: String = a.get(0).map_err(|e| self.err(e))?;
        let value: Value = a.get(1).map_err(|e| self.err(e))?;
        Ok(Some((sig, value)))
    }

    fn node_handle(&self, v: &Value<'js>) -> R<Option<Option<f64>>> {
        let Some(o) = v.as_object() else { return Ok(None) };
        let h: Value = o.get("svlHandle").map_err(|e| self.err(e))?;
        if h.is_undefined() {
            return Ok(None);
        }
        Ok(Some(h.as_number()))
    }

    fn boxed(&self, v: &Value<'js>) -> R<Option<(svl_handle, svl_id)>> {
        let Some(o) = v.as_object() else { return Ok(None) };
        let marker: Value = o.get("__svlBoxed").map_err(|e| self.err(e))?;
        let Some(a) = marker.as_array() else { return Ok(None) };
        let h: f64 = a.get(0).map_err(|e| self.err(e))?;
        let t: f64 = a.get(1).map_err(|e| self.err(e))?;
        Ok(Some((h as u64, t as u32)))
    }
}

impl<'js> crate::convert::JsPromises for QjsConv<'js> {
    fn new_completion(&self, id: u64) -> R<Value<'js>> {
        let f: Function = self.internal()?.get("makeCompletion").map_err(|e| self.err(e))?;
        let pair: Array = f.call(()).map_err(|e| self.err(e))?;
        let promise: Value = pair.get(0).map_err(|e| self.err(e))?;
        let done: Function = pair.get(1).map_err(|e| self.err(e))?;
        let cbs = self.cbs.as_ref().ok_or("callbacks are not available here")?;
        cbs.funcs.borrow_mut().insert(id, rquickjs::Persistent::save(&self.ctx, done));
        Ok(promise)
    }

    fn watch_abort(&self, signal: &Value<'js>, id: u64) -> R<()> {
        let f: Function = self.internal()?.get("watchAbort").map_err(|e| self.err(e))?;
        f.call::<_, ()>((signal.clone(), id as f64)).map_err(|e| self.err(e))
    }
}

impl<'js> JsWriter for QjsConv<'js> {
    type V = Value<'js>;

    fn null(&self) -> R<Value<'js>> {
        Ok(Value::new_null(self.ctx.clone()))
    }

    fn boolean(&self, b: bool) -> R<Value<'js>> {
        Ok(Value::new_bool(self.ctx.clone(), b))
    }

    fn number(&self, n: f64) -> R<Value<'js>> {
        if n.fract() == 0.0 && n.abs() < 2_147_483_648.0 {
            Ok(Value::new_int(self.ctx.clone(), n as i32))
        } else {
            Ok(Value::new_float(self.ctx.clone(), n))
        }
    }

    fn string(&self, s: &str) -> R<Value<'js>> {
        rquickjs::String::from_str(self.ctx.clone(), s)
            .map(|s| s.into_value())
            .map_err(|e| self.err(e))
    }

    fn string16(&self, s: &[u16]) -> R<Value<'js>> {
        // SAFETY: the context is live, `s` is a valid slice, and the returned value is owned by us.
        let raw = unsafe { qjs::JS_NewStringUTF16(self.ctx.as_raw().as_ptr(), s.as_ptr(), s.len() as _) };
        if unsafe { qjs::JS_IsException(raw) } {
            return Err(self.err(rquickjs::Error::Exception));
        }
        Ok(unsafe { Value::from_raw(self.ctx.clone(), raw) })
    }

    fn array(&self, items: Vec<Value<'js>>) -> R<Value<'js>> {
        let arr = Array::new(self.ctx.clone()).map_err(|e| self.err(e))?;
        for (i, x) in items.into_iter().enumerate() {
            arr.set(i, x).map_err(|e| self.err(e))?;
        }
        Ok(arr.into_value())
    }

    fn object(&self, entries: Vec<(String, Value<'js>)>) -> R<Value<'js>> {
        let o = Object::new(self.ctx.clone()).map_err(|e| self.err(e))?;
        for (k, x) in entries {
            o.set(k, x).map_err(|e| self.err(e))?;
        }
        Ok(o.into_value())
    }

    fn bytes(&self, b: &[u8]) -> R<Value<'js>> {
        rquickjs::TypedArray::<u8>::new_copy(self.ctx.clone(), b)
            .map(|t| t.into_value())
            .map_err(|e| self.err(e))
    }

    fn date(&self, ms: f64) -> R<Value<'js>> {
        let f: Function = self.internal()?.get("makeDate").map_err(|e| self.err(e))?;
        f.call::<_, Value>((ms,)).map_err(|e| self.err(e))
    }

    fn handle(&self, h: svl_handle) -> R<Value<'js>> {
        let n = JsWriter::number(self, h as f64)?;
        // Results hand out renderer nodes when the renderer is installed (it keeps one node per
        // handle, so identity holds); a bare host sees the number.
        match self.ctx.globals().get::<_, Option<Function>>("__svelters_node").map_err(|e| self.err(e))? {
            Some(f) => f.call((n,)).map_err(|e| self.err(e)),
            None => Ok(n),
        }
    }

    fn boxed(&self, handle: svl_handle, ty: svl_id, type_name: &str) -> R<Value<'js>> {
        self.core.note_boxed(handle);
        let f: Function = self.internal()?.get("makeBoxed").map_err(|e| self.err(e))?;
        f.call::<_, Value>((handle as f64, ty, type_name))
            .map_err(|e| self.err(e))
    }

    fn native_error(&self, domain: &str, code: Value<'js>, message: &str) -> R<Value<'js>> {
        let f: Function = self.internal()?.get("makeNativeError").map_err(|e| self.err(e))?;
        f.call::<_, Value>((domain, code, message)).map_err(|e| self.err(e))
    }
}

/// Keeps `IntoJs` imported for the f64/u32 conversions in `call` above.
#[allow(dead_code)]
fn _assert_into_js<'js, T: IntoJs<'js>>() {}
