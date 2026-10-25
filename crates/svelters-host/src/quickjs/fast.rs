//! W-J1 (plans/research/presto/compiler-and-js.md §5.4 option (a), §5.5): the hot host ops as raw
//! QuickJS C functions, and node wrappers whose handlers the GC traces.
//!
//! A raw C function gets the interpreter's own argument slots and an integer `magic`, so a call
//! costs no rquickjs closure dispatch, context duplication or parameter list. These ops read
//! numbers and booleans from the value's tag, borrow strings in place (`JS_ToCStringLen2`, no copy
//! for 8-bit text), resolve string prop names through atoms (a literal is already one), and write
//! scalars through [`Core::op_set_scalar`] with no conversion tree. Everything else falls back to
//! the engine-agnostic path (`QjsConv` over `JsReader`/`JsWriter`), with its exact errors.
//!
//! | op | args | what |
//! |---|---|---|
//! | `createId(kind)` | kind id | `create` |
//! | `insertId(parent, child, before?, slot?)` | handles, slot id | `insert` |
//! | `setp(h, prop, value)` | prop id or name | `setProp` |
//! | `setter(prop)` | prop id | a function `(h, value)` whose `magic` is the prop id |
//! | `listenId(h, signal, on, flags?)` | signal id | `listen` |
//!
//! Wrappers (`wrap(h)`, class `SveltersNode`): one object per live handle, with the handle and a
//! generation in its opaque slot and a list of handler functions (`wrapHandler(w, i, fn)`) that
//! its `gc_mark` reports to the GC. The host holds each wrapper strongly while its node lives and
//! lets go when the node is destroyed or dies, so a handler capturing its own node is a cycle the
//! GC collects. A Rust-held `JSValue` would be an untraced root and leak it. The finalizer frees
//! only what the wrapper owns.

use super::{QjsCallbacks, QjsConv, core_of, handle_of, msg, op, op_conv};
use crate::core::{Core, Name, Scalar};
use rquickjs::qjs;
use rquickjs::{Ctx, Exception, Function, Object, Value};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ffi::{CStr, c_char, c_int};
use std::ptr::NonNull;
use std::rc::{Rc, Weak};
use svelters_abi::{NameKind, svl_handle, svl_id};

type Res<T> = rquickjs::Result<T>;

const OP_CREATE: c_int = 0;
const OP_INSERT: c_int = 1;
const OP_SETP: c_int = 2;
const OP_LISTEN: c_int = 3;

/// What the raw functions of one context need, kept in the context's opaque slot (rquickjs uses
/// only the runtime's).
pub(super) struct FastCx {
    core: Weak<Core>,
    cbs: Rc<QjsCallbacks>,
    /// (kind, prop name atom) -> prop id. The cache holds one reference to each atom.
    atoms: RefCell<HashMap<(svl_id, qjs::JSAtom), svl_id>>,
    /// The `SveltersNode` class of this runtime.
    class: qjs::JSClassID,
}

thread_local! {
    /// `SveltersNode` objects alive on this thread (for tests: `Host::live_node_wrappers`).
    pub(crate) static LIVE_WRAPPERS: Cell<usize> = const { Cell::new(0) };
}

/// The opaque slot of a wrapper.
struct NodeSlot {
    handle: svl_handle,
    generation: u32,
    handlers: Vec<qjs::JSValue>,
}

/// The wrappers of live handles, held strongly (in [`QjsCallbacks`], which the core reaches).
#[derive(Default)]
pub(crate) struct Wrappers {
    /// handle -> (wrapper, its generation): a handle number a backend reuses gets a new wrapper.
    live: RefCell<HashMap<svl_handle, (qjs::JSValue, u32)>>,
    /// Released by `handle_gone` (maybe outside JS), freed at the next `free_gone`.
    gone: RefCell<Vec<qjs::JSValue>>,
    next_generation: Cell<u32>,
}

impl Wrappers {
    /// The node `h` is gone: its wrapper is no longer held. Runs no JS and frees nothing (a
    /// backend may report a death from anywhere).
    pub(crate) fn handle_gone(&self, h: svl_handle) {
        let v = self.live.borrow_mut().remove(&h);
        if let Some((v, _)) = v {
            self.gone.borrow_mut().push(v);
        }
    }

    /// Frees the references [`Wrappers::handle_gone`] released (the objects die at the GC if
    /// nothing else holds them).
    pub(crate) fn free_gone(&self, rt: *mut qjs::JSRuntime) {
        let gone = std::mem::take(&mut *self.gone.borrow_mut());
        for v in gone {
            unsafe { qjs::JS_FreeValueRT(rt, v) };
        }
    }

    /// Frees every reference (the context is going away).
    pub(crate) fn free_all(&self, rt: *mut qjs::JSRuntime) {
        let live: Vec<_> = self.live.borrow_mut().drain().map(|(_, (v, _))| v).collect();
        self.gone.borrow_mut().extend(live);
        self.free_gone(rt);
    }
}

unsafe extern "C" fn node_finalizer(rt: *mut qjs::JSRuntime, val: qjs::JSValue) {
    let slot = unsafe { qjs::JS_GetOpaque(val, qjs::JS_GetClassID(val)) } as *mut NodeSlot;
    if slot.is_null() {
        return;
    }
    let slot = unsafe { Box::from_raw(slot) };
    for v in slot.handlers {
        unsafe { qjs::JS_FreeValueRT(rt, v) };
    }
    LIVE_WRAPPERS.with(|n| n.set(n.get().saturating_sub(1)));
}

unsafe extern "C" fn node_mark(rt: *mut qjs::JSRuntime, val: qjs::JSValue, mark: qjs::JS_MarkFunc) {
    let slot = unsafe { qjs::JS_GetOpaque(val, qjs::JS_GetClassID(val)) } as *const NodeSlot;
    if slot.is_null() {
        return;
    }
    for &v in unsafe { &(*slot).handlers } {
        unsafe { qjs::JS_MarkValue(rt, v, mark) };
    }
}

fn fast_cx<'a>(ctx: *mut qjs::JSContext) -> Option<&'a FastCx> {
    let p = unsafe { qjs::JS_GetContextOpaque(ctx) } as *const FastCx;
    // SAFETY: set by `install` to a box that lives until `uninstall` (the engine's drop).
    unsafe { p.as_ref() }
}

/// Installs the raw ops on `host` and the context's opaque slot. Undone by [`uninstall`].
pub(super) fn install<'js>(ctx: &Ctx<'js>, host: &Object<'js>, core: Weak<Core>, cbs: Rc<QjsCallbacks>) -> Res<()> {
    let raw = ctx.as_raw().as_ptr();
    let rt = unsafe { qjs::JS_GetRuntime(raw) };
    let mut class: qjs::JSClassID = 0;
    unsafe { qjs::JS_NewClassID(rt, &mut class) };
    let def = qjs::JSClassDef {
        class_name: c"SveltersNode".as_ptr(),
        finalizer: Some(node_finalizer),
        gc_mark: Some(node_mark),
        call: None,
        exotic: std::ptr::null_mut(),
    };
    if unsafe { qjs::JS_NewClass(rt, class, &def) } < 0 {
        return Err(Exception::throw_internal(ctx, "SveltersNode: class registration failed"));
    }
    let cx = Box::new(FastCx { core, cbs, atoms: RefCell::default(), class });
    unsafe { qjs::JS_SetContextOpaque(raw, Box::into_raw(cx).cast()) };

    for (name, len, magic) in [(c"createId", 1, OP_CREATE), (c"insertId", 4, OP_INSERT), (c"setp", 3, OP_SETP), (c"listenId", 4, OP_LISTEN)] {
        host.set(name.to_str().unwrap_or_default(), raw_function(ctx, fast_op, name, len, magic)?)?;
    }
    host.set(
        "setter",
        Function::new(ctx.clone(), |ctx: Ctx<'js>, prop: u32| -> Res<Value<'js>> {
            let magic = c_int::try_from(prop).map_err(|_| Exception::throw_range(&ctx, "setter: prop id out of range"))?;
            raw_function(&ctx, fast_setter, c"setter", 2, magic)
        })?,
    )?;
    host.set("wrap", raw_function(ctx, wrap_op, c"wrap", 1, 0)?)?;
    host.set("handleOf", raw_function(ctx, wrap_op, c"handleOf", 1, 1)?)?;
    host.set("wrapHandler", raw_function(ctx, wrap_op, c"wrapHandler", 3, 2)?)?;
    Ok(())
}

/// Frees the context's [`FastCx`] (and the atoms it holds). Call before the context goes away.
pub(super) fn uninstall(ctx: &Ctx<'_>) {
    let raw = ctx.as_raw().as_ptr();
    let p = unsafe { qjs::JS_GetContextOpaque(raw) } as *mut FastCx;
    if p.is_null() {
        return;
    }
    unsafe { qjs::JS_SetContextOpaque(raw, std::ptr::null_mut()) };
    let cx = unsafe { Box::from_raw(p) };
    for (_, atom) in cx.atoms.borrow().keys() {
        unsafe { qjs::JS_FreeAtom(raw, *atom) };
    }
}

type MagicFn = unsafe extern "C" fn(*mut qjs::JSContext, qjs::JSValue, c_int, *mut qjs::JSValue, c_int) -> qjs::JSValue;

fn raw_function<'js>(ctx: &Ctx<'js>, f: MagicFn, name: &CStr, len: c_int, magic: c_int) -> Res<Value<'js>> {
    let raw = ctx.as_raw().as_ptr();
    // SAFETY: `generic_magic` is the member the `JS_CFUNC_generic_magic` prototype reads.
    let ty = qjs::JSCFunctionType { generic_magic: Some(f) };
    let v = unsafe { qjs::JS_NewCFunction2(raw, ty.generic, name.as_ptr(), len, qjs::JSCFunctionEnum_JS_CFUNC_generic_magic, magic) };
    if unsafe { qjs::JS_IsException(v) } {
        return Err(rquickjs::Error::Exception);
    }
    Ok(unsafe { Value::from_raw(ctx.clone(), v) })
}

/// A JS number argument as a handle (`handle_of`'s rules), without leaving the tag.
fn number(v: qjs::JSValue) -> Option<f64> {
    unsafe {
        let tag = qjs::JS_VALUE_GET_TAG(v);
        if tag == qjs::JS_TAG_INT {
            Some(qjs::JS_VALUE_GET_INT(v) as f64)
        } else if qjs::JS_TAG_IS_FLOAT64(tag) {
            Some(qjs::JS_VALUE_GET_FLOAT64(v))
        } else {
            None
        }
    }
}

fn is_nullish(v: qjs::JSValue) -> bool {
    unsafe { qjs::JS_IsNull(v) || qjs::JS_IsUndefined(v) }
}

/// Runs `f` with an rquickjs view of the raw context; an `Err` becomes the pending exception.
fn with_ctx(ctx: *mut qjs::JSContext, f: impl for<'js> FnOnce(&Ctx<'js>, &FastCx) -> Res<qjs::JSValue>) -> qjs::JSValue {
    let Some(nn) = NonNull::new(ctx) else { return qjs::JS_EXCEPTION };
    // SAFETY: called by QuickJS with the context it is running.
    let c = unsafe { Ctx::from_raw(nn) };
    let Some(cx) = fast_cx(ctx) else {
        let _ = Exception::throw_internal(&c, "svelters host ops are not installed in this context");
        return qjs::JS_EXCEPTION;
    };
    match f(&c, cx) {
        Ok(v) => v,
        Err(rquickjs::Error::Exception) => qjs::JS_EXCEPTION,
        Err(e) => {
            let _ = Exception::throw_internal(&c, &e.to_string());
            qjs::JS_EXCEPTION
        }
    }
}

fn handle_arg(c: &Ctx<'_>, v: qjs::JSValue) -> Res<svl_handle> {
    match number(v) {
        Some(n) => handle_of(c, n),
        None => Err(Exception::throw_type(c, "invalid handle")),
    }
}

fn id_arg(c: &Ctx<'_>, v: qjs::JSValue, what: &str) -> Res<svl_id> {
    match number(v) {
        Some(n) if n >= 1.0 && n <= u32::MAX as f64 && n.fract() == 0.0 => Ok(n as svl_id),
        _ => Err(Exception::throw_type(c, &format!("expected a {what} id"))),
    }
}

unsafe extern "C" fn fast_op(ctx: *mut qjs::JSContext, _this: qjs::JSValue, _argc: c_int, argv: *mut qjs::JSValue, magic: c_int) -> qjs::JSValue {
    // SAFETY: QuickJS pads the arguments to the declared length with `undefined`.
    let arg = |i: usize| unsafe { *argv.add(i) };
    with_ctx(ctx, |c, cx| {
        let core = core_of(c, &cx.core)?;
        match magic {
            OP_CREATE => {
                let kind = id_arg(c, arg(0), "kind")?;
                let h = op(c, core.op_create_id(kind))?;
                Ok(qjs::JS_NewFloat64(h as f64))
            }
            OP_INSERT => {
                let parent = handle_arg(c, arg(0))?;
                let child = handle_arg(c, arg(1))?;
                let before = if is_nullish(arg(2)) { None } else { Some(handle_arg(c, arg(2))?) };
                let slot = if is_nullish(arg(3)) { None } else { Some(Name::Id(id_arg(c, arg(3), "slot")?)) };
                op(c, core.op_insert(parent, child, before, slot))?;
                Ok(qjs::JS_UNDEFINED)
            }
            OP_SETP => {
                let h = handle_arg(c, arg(0))?;
                set_scalar(c, cx, &core, h, arg(1), arg(2))?;
                Ok(qjs::JS_UNDEFINED)
            }
            OP_LISTEN => {
                let h = handle_arg(c, arg(0))?;
                let signal = id_arg(c, arg(1), "signal")?;
                let on = unsafe { qjs::JS_IsBool(arg(2)) && qjs::JS_VALUE_GET_BOOL(arg(2)) };
                let flags = match number(arg(3)) {
                    Some(f) if f >= 0.0 && f <= u32::MAX as f64 => f as u32,
                    _ => 0,
                };
                op(c, core.op_listen(h, Name::Id(signal), on, flags))?;
                Ok(qjs::JS_UNDEFINED)
            }
            _ => Err(Exception::throw_internal(c, "unknown fast op")),
        }
    })
}

unsafe extern "C" fn fast_setter(ctx: *mut qjs::JSContext, _this: qjs::JSValue, _argc: c_int, argv: *mut qjs::JSValue, magic: c_int) -> qjs::JSValue {
    let arg = |i: usize| unsafe { *argv.add(i) };
    with_ctx(ctx, |c, cx| {
        let core = core_of(c, &cx.core)?;
        let h = handle_arg(c, arg(0))?;
        let prop = qjs::JS_NewFloat64(magic as u32 as f64);
        set_scalar(c, cx, &core, h, prop, arg(1))?;
        Ok(qjs::JS_UNDEFINED)
    })
}

/// The prop named by `v` (an id, or a string looked up as an atom) on a node of kind `kind`.
fn prop_name(c: &Ctx<'_>, cx: &FastCx, core: &Core, h: svl_handle, v: qjs::JSValue) -> Res<Option<svl_id>> {
    if let Some(n) = number(v) {
        return Ok((n >= 1.0 && n <= u32::MAX as f64 && n.fract() == 0.0).then_some(n as svl_id));
    }
    if !unsafe { qjs::JS_IsString(v) } {
        return Err(Exception::throw_type(c, "expected a prop name or id"));
    }
    let Ok(Some(kind)) = core.live_kind(h) else { return Ok(None) };
    let raw = c.as_raw().as_ptr();
    let atom = unsafe { qjs::JS_ValueToAtom(raw, v) };
    if atom == qjs::JS_ATOM_NULL {
        return Err(rquickjs::Error::Exception);
    }
    if let Some(&p) = cx.atoms.borrow().get(&(kind, atom)) {
        unsafe { qjs::JS_FreeAtom(raw, atom) };
        return Ok(Some(p));
    }
    let mut len: qjs::size_t = 0;
    let s = unsafe { qjs::JS_AtomToCStringLen(raw, &mut len, atom) };
    if s.is_null() {
        unsafe { qjs::JS_FreeAtom(raw, atom) };
        return Err(rquickjs::Error::Exception);
    }
    let bytes = unsafe { std::slice::from_raw_parts(s.cast::<u8>(), len as usize) };
    let resolved = std::str::from_utf8(bytes).map_err(|_| "prop name is not UTF-8".to_string()).and_then(|n| core.op_member_id(NameKind::Prop, kind, n));
    unsafe { qjs::JS_FreeCString(raw, s) };
    match resolved {
        Ok(p) => {
            cx.atoms.borrow_mut().insert((kind, atom), p);
            Ok(Some(p))
        }
        Err(m) => {
            unsafe { qjs::JS_FreeAtom(raw, atom) };
            Err(Exception::throw_message(c, &m))
        }
    }
}

/// `setp`: a scalar for a scalar slot goes straight to [`Core::op_set_scalar`]; anything else
/// (an object, a coercion, an error to word) through `op_set_prop`.
fn set_scalar<'js>(c: &Ctx<'js>, cx: &FastCx, core: &Rc<Core>, h: svl_handle, prop: qjs::JSValue, v: qjs::JSValue) -> Res<()> {
    if let Some(p) = prop_name(c, cx, core, h, prop)?
        && let Some((p, slot)) = op(c, core.fast_prop(h, Name::Id(p)))?
    {
        let tag = unsafe { qjs::JS_VALUE_GET_TAG(v) };
        let scalar = unsafe {
            if tag == qjs::JS_TAG_INT {
                Some(Scalar::Int(qjs::JS_VALUE_GET_INT(v)))
            } else if qjs::JS_TAG_IS_FLOAT64(tag) {
                Some(Scalar::Float(qjs::JS_VALUE_GET_FLOAT64(v)))
            } else if tag == qjs::JS_TAG_BOOL {
                Some(Scalar::Bool(qjs::JS_VALUE_GET_BOOL(v)))
            } else if tag == qjs::JS_TAG_NULL {
                Some(Scalar::Null)
            } else {
                None
            }
        };
        if let Some(raw) = scalar.and_then(|s| slot.encode(s)) {
            return op(c, core.op_set_scalar(h, p, &raw));
        }
        if unsafe { qjs::JS_IsString(v) } && !core.utf16_in() {
            let r = c.as_raw().as_ptr();
            let mut len: qjs::size_t = 0;
            let s = unsafe { qjs::JS_ToCStringLen2(r, &mut len, v, false) };
            if s.is_null() {
                return Err(rquickjs::Error::Exception);
            }
            let bytes = unsafe { std::slice::from_raw_parts(s.cast::<u8>(), len as usize) };
            let res = slot.encode(Scalar::Str(bytes)).map(|raw| core.op_set_scalar(h, p, &raw));
            unsafe { qjs::JS_FreeCString(r, s as *const c_char) };
            if let Some(res) = res {
                return op(c, res);
            }
        }
    }
    // the general path: the converter, with its coercions and error messages
    let name = unsafe { Value::from_raw(c.clone(), qjs::JS_DupValue(c.as_raw().as_ptr(), prop)) };
    let value = unsafe { Value::from_raw(c.clone(), qjs::JS_DupValue(c.as_raw().as_ptr(), v)) };
    let conv = QjsConv::new(c.clone(), core.clone(), Some(cx.cbs.clone()));
    let r = msg(c, conv.with_name(&name, |n| core.op_set_prop(&conv, h, n, &value)))?;
    op_conv(c, &conv, r)
}

unsafe extern "C" fn wrap_op(ctx: *mut qjs::JSContext, _this: qjs::JSValue, _argc: c_int, argv: *mut qjs::JSValue, magic: c_int) -> qjs::JSValue {
    let arg = |i: usize| unsafe { *argv.add(i) };
    with_ctx(ctx, |c, cx| {
        let raw = c.as_raw().as_ptr();
        match magic {
            // wrap(h): the wrapper of live handle h (one per handle)
            0 => {
                let core = core_of(c, &cx.core)?;
                let h = handle_arg(c, arg(0))?;
                msg(c, core.kind_of(h))?;
                let wrappers = &cx.cbs.wrappers;
                if let Some(&(w, _)) = wrappers.live.borrow().get(&h) {
                    return Ok(unsafe { qjs::JS_DupValue(raw, w) });
                }
                let obj = unsafe { qjs::JS_NewObjectClass(raw, cx.class) };
                if unsafe { qjs::JS_IsException(obj) } {
                    return Err(rquickjs::Error::Exception);
                }
                let generation = wrappers.next_generation.get().wrapping_add(1);
                wrappers.next_generation.set(generation);
                let slot = Box::new(NodeSlot { handle: h, generation, handlers: Vec::new() });
                unsafe { qjs::JS_SetOpaque(obj, Box::into_raw(slot).cast()) };
                LIVE_WRAPPERS.with(|n| n.set(n.get() + 1));
                wrappers.live.borrow_mut().insert(h, (unsafe { qjs::JS_DupValue(raw, obj) }, generation));
                Ok(obj)
            }
            // handleOf(w): its handle while the host still holds it, else null
            1 => {
                let slot = node_slot(c, cx, arg(0))?;
                let held = cx.cbs.wrappers.live.borrow().get(&slot.handle).is_some_and(|&(_, g)| g == slot.generation);
                Ok(if held { qjs::JS_NewFloat64(slot.handle as f64) } else { qjs::JS_NULL })
            }
            // wrapHandler(w, i, fn | null): sets handler i (traced by the wrapper's gc_mark)
            2 => {
                let slot = node_slot(c, cx, arg(0))?;
                let i = match number(arg(1)) {
                    Some(n) if n >= 0.0 && n < 4096.0 && n.fract() == 0.0 => n as usize,
                    _ => return Err(Exception::throw_range(c, "wrapHandler: index out of range")),
                };
                let f = arg(2);
                if !is_nullish(f) && !unsafe { qjs::JS_IsFunction(raw, f) } {
                    return Err(Exception::throw_type(c, "wrapHandler: expected a function or null"));
                }
                if slot.handlers.len() <= i {
                    slot.handlers.resize(i + 1, qjs::JS_UNDEFINED);
                }
                let old = std::mem::replace(&mut slot.handlers[i], unsafe { qjs::JS_DupValue(raw, f) });
                unsafe { qjs::JS_FreeValue(raw, old) };
                Ok(qjs::JS_UNDEFINED)
            }
            _ => Err(Exception::throw_internal(c, "unknown wrapper op")),
        }
    })
}

/// The opaque slot of wrapper `w`; a type error for anything else.
fn node_slot<'a>(c: &Ctx<'_>, cx: &FastCx, w: qjs::JSValue) -> Res<&'a mut NodeSlot> {
    let p = unsafe { qjs::JS_GetOpaque(w, cx.class) } as *mut NodeSlot;
    // SAFETY: the slot lives as long as the object, which the caller's argument keeps alive.
    unsafe { p.as_mut() }.ok_or_else(|| Exception::throw_type(c, "expected a node wrapper (host.wrap)"))
}

