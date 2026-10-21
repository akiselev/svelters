//! rquickjs (QuickJS-NG) implementation of [`JsEngine`]: a synchronous runtime on the calling
//! thread, a bundle-relative module loader, the `__svelters_host` object, and a JS prelude for
//! `console`, timers and `queueMicrotask`.

use crate::convert;
use crate::core::{Core, Name, OpError, OpKind};
use crate::engine::{ChangedProp, ErrorKind, JobStatus, JsEngine, JsError};
use rquickjs::function::Opt;
use rquickjs::loader::{ImportAttributes, Loader, Resolver};
use rquickjs::module::Declared;
use rquickjs::prelude::Coerced;
use rquickjs::{
    CatchResultExt, Context, Ctx, Exception, FromJs, Function, Module, Object, Runtime, Value,
};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use svelters_abi::{NameKind, Value as AV};

mod conv;
use conv::QjsConv;

const PRELUDE: &str = include_str!("prelude.js");

type Res<T> = rquickjs::Result<T>;

thread_local! {
    /// The context of the JS that is running right now, so a native callback that arrives while a
    /// host op is on the stack can re-enter it. `Context::with` is not re-entrant; the raw context is.
    static CURRENT_CTX: std::cell::Cell<Option<std::ptr::NonNull<rquickjs::qjs::JSContext>>> =
        const { std::cell::Cell::new(None) };
}

/// Marks `ctx` as the running JS context until dropped (restoring the previous mark).
struct RunningCtx(Option<std::ptr::NonNull<rquickjs::qjs::JSContext>>);

impl RunningCtx {
    fn enter(ctx: &Ctx<'_>) -> RunningCtx {
        RunningCtx::enter_raw(ctx.as_raw())
    }

    fn enter_raw(raw: std::ptr::NonNull<rquickjs::qjs::JSContext>) -> RunningCtx {
        RunningCtx(CURRENT_CTX.with(|c| c.replace(Some(raw))))
    }
}

impl Drop for RunningCtx {
    fn drop(&mut self) {
        CURRENT_CTX.with(|c| c.set(self.0));
    }
}

/// The JS functions behind `CALLBACK` values, and how to call them.
pub(crate) struct QjsCallbacks {
    ctx: Context,
    core: Weak<Core>,
    pub(crate) funcs: RefCell<std::collections::HashMap<u64, rquickjs::Persistent<Function<'static>>>>,
}

impl crate::core::CallbackEngine for QjsCallbacks {
    fn invoke(&self, id: u64, sig: u32, args: &[AV]) -> Result<AV, JsError> {
        let f = self
            .funcs
            .borrow()
            .get(&id)
            .cloned()
            .ok_or_else(|| JsError::new(ErrorKind::Host, "callback function is gone"))?;
        let core_rc = self
            .core
            .upgrade()
            .ok_or_else(|| JsError::new(ErrorKind::Host, "host has shut down"))?;
        let run = |ctx: &Ctx<'_>| -> Res<AV> {
            let func = f.clone().restore(ctx)?;
            let conv = QjsConv::new(ctx.clone(), core_rc.clone(), None);
            let mut call_args = rquickjs::function::Args::new(ctx.clone(), args.len());
            for a in args {
                let v = convert::decode(&conv, a, &*core_rc).map_err(|m| Exception::throw_message(ctx, &m))?;
                call_args.push_arg(v)?;
            }
            let ret: Value = func.call_arg(call_args)?;
            // The result is converted per the signature's return slot; a void signature ignores it.
            let slot = match core_rc.callback_ret_slot(sig) {
                Some(s) if s.tag != svelters_abi::SVL_NULL => s,
                _ => return Ok(AV::Null),
            };
            let reg = crate::core::CallbackRegistry(&core_rc);
            let cx = convert::Cx { types: &*core_rc, utf16: false, callbacks: Some(&reg), owner: 0 };
            let mut ar = convert::Arena::new();
            let raw = convert::encode(&conv, &ret, &slot, &cx, &mut ar)
                .map_err(|e| Exception::throw_type(ctx, &e.render("callback result")))?;
            Ok(unsafe { AV::from_raw(&raw) })
        };
        match CURRENT_CTX.with(|c| c.get()) {
            // Nested: a host op is on the stack. Re-enter the running context.
            Some(raw) => {
                // SAFETY: the pointer is the context of the JS currently running on this thread.
                let ctx = unsafe { Ctx::from_raw(raw) };
                run(&ctx).map_err(|e| js_err(&ctx, e))
            }
            None => self.ctx.with(|ctx| {
                let _running = RunningCtx::enter(&ctx);
                run(&ctx).map_err(|e| js_err(&ctx, e))
            }),
        }
    }

    fn drop_function(&self, id: u64) {
        // Dropping a persistent function releases a reference; it runs no JS.
        self.funcs.borrow_mut().remove(&id);
    }
}

pub(crate) struct QjsEngine {
    // `ctx` before `rt`: the context is released first.
    ctx: Context,
    rt: Runtime,
    base: Instant,
    deadline_ns: Arc<AtomicU64>,
    budget: Option<Duration>,
    async_errors: Rc<RefCell<Vec<JsError>>>,
    core: Rc<Core>,
    callbacks: Rc<QjsCallbacks>,
}

impl QjsEngine {
    pub(crate) fn new(core: &Rc<Core>) -> Result<QjsEngine, JsError> {
        let host_err =
            |e: rquickjs::Error| JsError::new(ErrorKind::Host, format!("quickjs init: {e}"));
        let rt = Runtime::new().map_err(host_err)?;
        rt.set_memory_limit(core.cfg.memory_limit);
        rt.set_max_stack_size(core.cfg.max_stack_size);

        let base = Instant::now();
        let deadline_ns = Arc::new(AtomicU64::new(0));
        {
            let dl = deadline_ns.clone();
            let flag: Arc<AtomicBool> = core.interrupt.clone();
            rt.set_interrupt_handler(Some(Box::new(move || {
                if flag.swap(false, Ordering::Relaxed) {
                    return true;
                }
                let d = dl.load(Ordering::Relaxed);
                d != 0 && base.elapsed().as_nanos() as u64 >= d
            })));
        }

        let async_errors: Rc<RefCell<Vec<JsError>>> = Rc::default();
        {
            let sink = async_errors.clone();
            rt.set_host_promise_rejection_tracker(Some(Box::new(
                move |ctx, _promise, reason, handled| {
                    if !handled {
                        let mut e = describe(&ctx, reason);
                        if e.kind == ErrorKind::Exception {
                            e.kind = ErrorKind::UnhandledRejection;
                        }
                        sink.borrow_mut().push(e);
                    }
                },
            )));
        }

        let ctx = Context::full(&rt).map_err(host_err)?;
        let weak = Rc::downgrade(core);
        let callbacks = Rc::new(QjsCallbacks {
            ctx: ctx.clone(),
            core: Rc::downgrade(core),
            funcs: RefCell::default(),
        });
        core.set_callback_engine(callbacks.clone());
        let cbs = callbacks.clone();
        ctx.with(|ctx| {
            install(&ctx, weak, cbs, base)
                .catch(&ctx)
                .map_err(|e| JsError::new(ErrorKind::Host, format!("install host bindings: {e}")))
        })?;

        Ok(QjsEngine {
            ctx,
            rt,
            base,
            deadline_ns,
            budget: core.cfg.turn_budget,
            async_errors,
            core: core.clone(),
            callbacks,
        })
    }

    fn arm(&self) {
        let d = self
            .budget
            .map_or(0, |b| (self.base.elapsed() + b).as_nanos() as u64);
        self.deadline_ns.store(d, Ordering::Relaxed);
    }

    fn disarm(&self) {
        self.deadline_ns.store(0, Ordering::Relaxed);
    }

    fn call_guarded<R>(&self, f: impl for<'js> FnOnce(&Ctx<'js>) -> Res<R>) -> Result<R, JsError> {
        self.arm();
        let r = self.ctx.with(|ctx| {
            let _running = RunningCtx::enter(&ctx);
            f(&ctx).map_err(|e| js_err(&ctx, e))
        });
        self.disarm();
        r.map_err(|mut e| {
            // QuickJS throws `null` when it cannot even allocate the out-of-memory error, so the
            // message alone is unreliable; a failure at the memory limit is an OOM.
            if e.kind == ErrorKind::Exception && self.near_memory_limit() {
                e.kind = ErrorKind::OutOfMemory;
            }
            e
        })
    }

    fn near_memory_limit(&self) -> bool {
        let u = self.rt.memory_usage();
        u.malloc_limit > 0 && u.malloc_size + (u.malloc_limit / 8).max(256 * 1024) >= u.malloc_limit
    }
}

impl JsEngine for QjsEngine {
    fn load_module(&mut self, entry: &Path) -> Result<(), JsError> {
        let load_err = |m: String| JsError::new(ErrorKind::Load, m);
        let entry = entry
            .canonicalize()
            .map_err(|e| load_err(format!("{}: {e}", entry.display())))?;
        let root = entry
            .parent()
            .ok_or_else(|| load_err("entry has no parent directory".into()))?
            .to_path_buf();
        let name = entry
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| load_err("entry file name is not UTF-8".into()))?
            .to_string();
        let source = std::fs::read_to_string(&entry)
            .map_err(|e| load_err(format!("{}: {e}", entry.display())))?;

        self.rt.set_loader(BundleResolver, BundleLoader { root });
        let res = self.call_guarded(|ctx| {
            let (_m, promise) = Module::declare(ctx.clone(), name.as_str(), source)?.eval()?;
            // A module that failed synchronously has a rejected promise; one that is waiting on
            // top-level await settles later and is reported as an unhandled rejection if it fails.
            match promise.result::<Value>() {
                Some(Err(e)) => Err(e),
                _ => Ok(()),
            }
        });
        res.map_err(|mut e| {
            // The rejection tracker saw the same failure; report it once, as the load error.
            self.async_errors.borrow_mut().clear();
            if e.kind == ErrorKind::Exception {
                e.kind = ErrorKind::Load;
            }
            e
        })
    }

    fn dispatch(
        &mut self,
        handle: u64,
        signal: &str,
        args: &[AV],
        deferred: bool,
        flags: u32,
    ) -> Result<(), JsError> {
        let core = self.core.clone();
        self.call_guarded(|ctx| {
            let f: Value = ctx.globals().get("__svelters_dispatch")?;
            let Some(f) = f.as_function() else {
                return Err(Exception::throw_message(
                    ctx,
                    "globalThis.__svelters_dispatch is not registered",
                ));
            };
            let conv = QjsConv::new(ctx.clone(), core, None);
            let list = convert::decode(&conv, &AV::List(args.to_vec()), &*conv.core)
                .map_err(|m| Exception::throw_message(ctx, &m))?;
            f.call::<_, Value>((handle as f64, signal, list, deferred, flags))
                .map(|_| ())
        })
    }

    fn dispatch_sync(&mut self, handle: u64, signal: &str, args: &[AV], flags: u32) -> Result<AV, JsError> {
        let core = self.core.clone();
        self.call_guarded(|ctx| {
            let f: Value = ctx.globals().get("__svelters_dispatch_sync")?;
            let Some(f) = f.as_function() else {
                return Err(Exception::throw_message(
                    ctx,
                    "globalThis.__svelters_dispatch_sync is not registered",
                ));
            };
            let conv = QjsConv::new(ctx.clone(), core, None);
            let list = convert::decode(&conv, &AV::List(args.to_vec()), &*conv.core)
                .map_err(|m| Exception::throw_message(ctx, &m))?;
            let res: Value = f.call((handle as f64, signal, list, flags))?;
            // A sync result is a primitive (no decision, a flag, a number, a string) or, for signals with
            // out-parameters, a plain object `{ret, name: primitive}` / an array of primitives.
            sync_result_value(ctx, res, true)
        })
    }

    fn run_frame(&mut self) -> Result<(), JsError> {
        self.call_guarded(|ctx| {
            let internal: Object = ctx.globals().get("__svelters_internal")?;
            let frame: Function = internal.get("frame")?;
            frame.call::<_, ()>(())
        })
    }

    fn dispatch_gone(&mut self, handle: u64, reason: &str) -> Result<(), JsError> {
        self.call_guarded(|ctx| {
            let f: Value = ctx.globals().get("__svelters_gone")?;
            let Some(f) = f.as_function() else {
                return Ok(());
            };
            f.call::<_, Value>((handle as f64, reason)).map(|_| ())
        })
    }

    fn dispatch_changed(&mut self, batch: &[ChangedProp]) -> Result<(), JsError> {
        let core = self.core.clone();
        self.call_guarded(|ctx| {
            let f: Value = ctx.globals().get("__svelters_changed")?;
            let Some(f) = f.as_function() else {
                return Ok(());
            };
            let conv = QjsConv::new(ctx.clone(), core, None);
            let arr = rquickjs::Array::new(ctx.clone())?;
            let mut i = 0;
            for c in batch {
                let v = match &c.value {
                    Some(v) => convert::decode(&conv, v, &*conv.core).map_err(|m| Exception::throw_message(ctx, &m))?,
                    None => Value::new_undefined(ctx.clone()),
                };
                arr.set(i, c.handle as f64)?;
                arr.set(i + 1, c.prop.as_str())?;
                arr.set(i + 2, v)?;
                i += 3;
            }
            f.call::<_, Value>((arr,)).map(|_| ())
        })
    }

    fn fire_timer(&mut self, id: u32) -> Result<(), JsError> {
        self.call_guarded(|ctx| {
            let internal: Object = ctx.globals().get("__svelters_internal")?;
            let fire: Function = internal.get("fire")?;
            fire.call::<_, ()>((id,))
        })
    }

    fn run_jobs(&mut self, max: usize) -> JobStatus {
        let mut st = JobStatus::default();
        self.arm();
        // Jobs run host ops too: mark the context as running for nested native callbacks.
        let _running = RunningCtx::enter_raw(self.ctx.with(|c| c.as_raw()));
        loop {
            if st.ran >= max {
                st.pending = self.rt.is_job_pending();
                break;
            }
            match self.rt.execute_pending_job() {
                Ok(true) => st.ran += 1,
                Ok(false) => break,
                Err(job_ctx) => {
                    st.ran += 1;
                    let e = job_ctx.0.with(|c| describe(&c, c.catch()));
                    self.async_errors.borrow_mut().push(e);
                }
            }
        }
        self.disarm();
        st
    }

    fn collect_garbage(&mut self) {
        self.rt.run_gc();
    }

    fn take_errors(&mut self) -> Vec<JsError> {
        std::mem::take(&mut *self.async_errors.borrow_mut())
    }
}

// ---- errors ----

fn classify(message: &str) -> ErrorKind {
    if message == "interrupted" {
        ErrorKind::Interrupted
    } else if message.contains("out of memory") {
        ErrorKind::OutOfMemory
    } else {
        ErrorKind::Exception
    }
}

fn describe<'js>(ctx: &Ctx<'js>, v: Value<'js>) -> JsError {
    let ex = v
        .as_object()
        .and_then(|o| Exception::from_object(o.clone()));
    let (message, stack) = match ex {
        Some(ex) => (ex.message().unwrap_or_default(), ex.stack()),
        None => (
            Coerced::<String>::from_js(ctx, v)
                .map(|c| c.0)
                .unwrap_or_else(|_| "<unprintable thrown value>".into()),
            None,
        ),
    };
    JsError {
        kind: classify(&message),
        message,
        stack,
    }
}

fn js_err<'js>(ctx: &Ctx<'js>, e: rquickjs::Error) -> JsError {
    match e {
        rquickjs::Error::Exception => describe(ctx, ctx.catch()),
        rquickjs::Error::Allocation => JsError::new(ErrorKind::OutOfMemory, "out of memory"),
        other => JsError::new(ErrorKind::Host, other.to_string()),
    }
}

// ---- host bindings ----

fn core_of<'js>(ctx: &Ctx<'js>, w: &Weak<Core>) -> Res<Rc<Core>> {
    w.upgrade()
        .ok_or_else(|| Exception::throw_message(ctx, "host has shut down"))
}

fn handle_of<'js>(ctx: &Ctx<'js>, n: f64) -> Res<u64> {
    if n.is_finite() && n >= 1.0 && n.fract() == 0.0 && n < 9_007_199_254_740_992.0 {
        Ok(n as u64)
    } else {
        Err(Exception::throw_type(ctx, "invalid handle"))
    }
}

/// Throws an op failure as the JS error class that matches it. A native error (GError, a mapped C++
/// exception) becomes a `NativeError` with `domain`, `code` and `message`.
fn throw<'js, T>(ctx: &Ctx<'js>, e: OpError) -> Res<T> {
    if let Some(n) = &e.native {
        let code: Res<Value<'js>> = match &n.code {
            AV::Str(s) => rquickjs::String::from_str(ctx.clone(), s).map(|s| s.into_value()),
            AV::I64(i) => Ok(Value::new_float(ctx.clone(), *i as f64)),
            _ => Ok(Value::new_null(ctx.clone())),
        };
        let made = (|| -> Res<Value<'js>> {
            let internal: Object = ctx.globals().get("__svelters_internal")?;
            let make: Function = internal.get("makeNativeError")?;
            make.call((n.domain.as_str(), code?, n.message.as_str()))
        })();
        if let Ok(err) = made {
            return Err(ctx.throw(err));
        }
    }
    Err(match e.kind {
        OpKind::Type => Exception::throw_type(ctx, &e.msg),
        OpKind::Range => Exception::throw_range(ctx, &e.msg),
        OpKind::Error => Exception::throw_message(ctx, &e.msg),
    })
}

fn op<'js, T>(ctx: &Ctx<'js>, r: Result<T, OpError>) -> Res<T> {
    match r {
        Ok(v) => Ok(v),
        Err(e) => throw(ctx, e),
    }
}

/// Like [`op`], but rethrows an exception that user JS threw while the value was converted.
fn op_conv<'js, T>(ctx: &Ctx<'js>, conv: &QjsConv<'js>, r: Result<T, OpError>) -> Res<T> {
    match r {
        Ok(v) => Ok(v),
        Err(e) => match conv.take_pending() {
            Some(ex) => Err(ctx.throw(ex)),
            None => throw(ctx, e),
        },
    }
}

fn msg<'js, T>(ctx: &Ctx<'js>, r: Result<T, String>) -> Res<T> {
    r.map_err(|m| Exception::throw_message(ctx, &m))
}

fn install<'js>(ctx: &Ctx<'js>, core: Weak<Core>, cbs: Rc<QjsCallbacks>, base: Instant) -> Res<()> {
    let globals = ctx.globals();

    // Natives for the prelude. Not reachable from user code after the prelude runs.
    let native = Object::new(ctx.clone())?;
    {
        let w = core.clone();
        native.set(
            "log",
            Function::new(ctx.clone(), move |level: String, text: String| {
                if let Some(c) = w.upgrade() {
                    c.console(&level, text);
                }
            })?,
        )?;
        let w = core.clone();
        native.set(
            "schedule",
            Function::new(ctx.clone(), move |id: u32, delay_ms: f64, repeat: bool| {
                if let Some(c) = w.upgrade() {
                    c.timer_set(id, Duration::from_millis(delay_ms.max(0.0) as u64), repeat);
                }
            })?,
        )?;
        let w = core.clone();
        native.set(
            "cancel",
            Function::new(ctx.clone(), move |id: u32| {
                if let Some(c) = w.upgrade() {
                    c.timer_clear(id);
                }
            })?,
        )?;
        let w = core.clone();
        native.set(
            "frame_request",
            Function::new(ctx.clone(), move || -> bool {
                w.upgrade().is_some_and(|c| c.frame_request())
            })?,
        )?;
        native.set(
            "now",
            Function::new(ctx.clone(), move || base.elapsed().as_secs_f64() * 1000.0)?,
        )?;
        // An AbortSignal fired: ask the backend to cancel the operation behind a completion callback.
        let w = core.clone();
        native.set(
            "cancelAsync",
            Function::new(ctx.clone(), move |id: f64| {
                if let Some(c) = w.upgrade() {
                    c.op_cancel_async(id as u64);
                }
            })?,
        )?;
        // Boxed values: explicit release is immediate; the GC path only queues the handle.
        let w = core.clone();
        native.set(
            "releaseBoxed",
            Function::new(ctx.clone(), move |h: f64| {
                if let Some(c) = w.upgrade() {
                    c.release_boxed(h as u64);
                }
            })?,
        )?;
        let w = core.clone();
        native.set(
            "queueRelease",
            Function::new(ctx.clone(), move |h: f64| {
                if let Some(c) = w.upgrade() {
                    c.queue_release(h as u64);
                }
            })?,
        )?;
    }

    // __svelters_host: the JS host ops from backend-abi.md. Member arguments are names or the
    // ids the `*Id` resolvers return; ids skip the name lookup and string handling entirely.
    let host = Object::new(ctx.clone())?;
    {
        let w = core.clone();
        let cb = cbs.clone();
        host.set(
            "create",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>, kind: Value<'js>| -> Res<f64> {
                    let c = core_of(&ctx, &w)?;
                    let conv = QjsConv::new(ctx.clone(), c.clone(), Some(cb.clone()));
                    if let Some(id) = kind.as_int() {
                        return Ok(op(&ctx, c.op_create_id(id as u32))? as f64);
                    }
                    let r = msg(&ctx, conv.with_name(&kind, |n| match n {
                        Name::Str(s) => c.op_create(s),
                        Name::Id(i) => c.op_create_id(i),
                    }))?;
                    Ok(op(&ctx, r)? as f64)
                },
            )?,
        )?;
        for (js, what) in [
            ("kindId", None),
            ("propId", Some(NameKind::Prop)),
            ("signalId", Some(NameKind::Signal)),
            ("methodId", Some(NameKind::Method)),
            ("slotId", Some(NameKind::Slot)),
        ] {
            let w = core.clone();
            let cb = cbs.clone();
            host.set(
                js,
                Function::new(
                    ctx.clone(),
                    move |ctx: Ctx<'js>, a: Value<'js>, b: Opt<Value<'js>>| -> Res<u32> {
                        let c = core_of(&ctx, &w)?;
                        let conv = QjsConv::new(ctx.clone(), c.clone(), Some(cb.clone()));
                        match what {
                            None => {
                                let r = msg(&ctx, conv.with_name(&a, |n| match n {
                                    Name::Str(s) => c.op_kind_id(s),
                                    Name::Id(i) => Ok(i),
                                }))?;
                                msg(&ctx, r)
                            }
                            Some(what) => {
                                let kind = a
                                    .as_int()
                                    .ok_or_else(|| Exception::throw_type(&ctx, "expected a kind id"))?
                                    as u32;
                                let name = b
                                    .0
                                    .ok_or_else(|| Exception::throw_type(&ctx, "expected a member name"))?;
                                let r = msg(&ctx, conv.with_name(&name, |n| match n {
                                    Name::Str(s) => c.op_member_id(what, kind, s),
                                    Name::Id(i) => Ok(i),
                                }))?;
                                msg(&ctx, r)
                            }
                        }
                    },
                )?,
            )?;
        }
        let w = core.clone();
        host.set(
            "propType",
            Function::new(ctx.clone(), move |ctx: Ctx<'js>, kind: u32, prop: u32| -> Res<Value<'js>> {
                let c = core_of(&ctx, &w)?;
                match c.op_prop_type(kind, prop) {
                    Some(t) => Ok(rquickjs::String::from_str(ctx.clone(), t)?.into_value()),
                    None => Ok(Value::new_null(ctx.clone())),
                }
            })?,
        )?;
        let w = core.clone();
        let cb = cbs.clone();
        host.set(
            "insert",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>,
                      parent: f64,
                      child: f64,
                      before: Opt<Option<f64>>,
                      slot: Opt<Option<Value<'js>>>|
                      -> Res<()> {
                    let c = core_of(&ctx, &w)?;
                    let before = match before.0.flatten() {
                        Some(b) => Some(handle_of(&ctx, b)?),
                        None => None,
                    };
                    let (parent, child) = (handle_of(&ctx, parent)?, handle_of(&ctx, child)?);
                    match slot.0.flatten() {
                        None => op(&ctx, c.op_insert(parent, child, before, None)),
                        Some(sv) => {
                            let conv = QjsConv::new(ctx.clone(), c.clone(), Some(cb.clone()));
                            let r = msg(&ctx, conv.with_name(&sv, |n| c.op_insert(parent, child, before, Some(n))))?;
                            op(&ctx, r)
                        }
                    }
                },
            )?,
        )?;
        let w = core.clone();
        host.set(
            "remove",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>, parent: f64, child: f64| -> Res<()> {
                    let c = core_of(&ctx, &w)?;
                    op(
                        &ctx,
                        c.op_remove(handle_of(&ctx, parent)?, handle_of(&ctx, child)?),
                    )
                },
            )?,
        )?;
        let w = core.clone();
        host.set(
            "destroy",
            Function::new(ctx.clone(), move |ctx: Ctx<'js>, h: f64| -> Res<()> {
                let c = core_of(&ctx, &w)?;
                op(&ctx, c.op_destroy(handle_of(&ctx, h)?))
            })?,
        )?;
        let w = core.clone();
        let cb = cbs.clone();
        host.set(
            "setProp",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>, h: f64, name: Value<'js>, value: Value<'js>| -> Res<()> {
                    let c = core_of(&ctx, &w)?;
                    let conv = QjsConv::new(ctx.clone(), c.clone(), Some(cb.clone()));
                    let h = handle_of(&ctx, h)?;
                    let r = msg(&ctx, conv.with_name(&name, |n| c.op_set_prop(&conv, h, n, &value)))?;
                    op_conv(&ctx, &conv, r)
                },
            )?,
        )?;
        let w = core.clone();
        let cb = cbs.clone();
        host.set(
            "getProp",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>, h: f64, name: Value<'js>| -> Res<Value<'js>> {
                    let c = core_of(&ctx, &w)?;
                    let conv = QjsConv::new(ctx.clone(), c.clone(), Some(cb.clone()));
                    let h = handle_of(&ctx, h)?;
                    let r = msg(&ctx, conv.with_name(&name, |n| c.op_get_prop(&conv, h, n)))?;
                    op(&ctx, r)
                },
            )?,
        )?;
        let w = core.clone();
        let cb = cbs.clone();
        host.set(
            "call",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>, h: f64, method: Value<'js>, args: Value<'js>| -> Res<Value<'js>> {
                    let c = core_of(&ctx, &w)?;
                    if !args.is_array() {
                        return Err(Exception::throw_type(&ctx, "call: args must be an array"));
                    }
                    let conv = QjsConv::new(ctx.clone(), c.clone(), Some(cb.clone()));
                    let h = handle_of(&ctx, h)?;
                    let r = msg(&ctx, conv.with_name(&method, |n| c.op_call(&conv, h, n, &args)))?;
                    op_conv(&ctx, &conv, r)
                },
            )?,
        )?;
        let w = core.clone();
        let cb = cbs.clone();
        host.set(
            "construct",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>, kind: Value<'js>, props: Opt<Value<'js>>| -> Res<Value<'js>> {
                    let c = core_of(&ctx, &w)?;
                    let conv = QjsConv::new(ctx.clone(), c.clone(), Some(cb.clone()));
                    let props = props.0.unwrap_or_else(|| Value::new_undefined(ctx.clone()));
                    let r = msg(&ctx, conv.with_name(&kind, |n| c.op_construct(&conv, n, &props)))?;
                    op_conv(&ctx, &conv, r)
                },
            )?,
        )?;
        let w = core.clone();
        let cb = cbs.clone();
        host.set(
            "callStatic",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>, kind: Value<'js>, method: Value<'js>, args: Value<'js>| -> Res<Value<'js>> {
                    let c = core_of(&ctx, &w)?;
                    if !args.is_array() {
                        return Err(Exception::throw_type(&ctx, "callStatic: args must be an array"));
                    }
                    let conv = QjsConv::new(ctx.clone(), c.clone(), Some(cb.clone()));
                    let r = msg(
                        &ctx,
                        conv.with_name(&kind, |nk| conv.with_name(&method, |nm| c.op_call_static(&conv, nk, nm, &args)))
                            .and_then(|x| x),
                    )?;
                    op_conv(&ctx, &conv, r)
                },
            )?,
        )?;
        let w = core.clone();
        host.set(
            "typeChain",
            Function::new(ctx.clone(), move |ctx: Ctx<'js>, h: f64| -> Res<Vec<String>> {
                let c = core_of(&ctx, &w)?;
                op(&ctx, c.op_type_chain(handle_of(&ctx, h)?))
            })?,
        )?;
        let w = core.clone();
        let cb = cbs.clone();
        host.set(
            "listen",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>, h: f64, signal: Value<'js>, on: bool, flags: Opt<u32>| -> Res<()> {
                    let c = core_of(&ctx, &w)?;
                    let conv = QjsConv::new(ctx.clone(), c.clone(), Some(cb.clone()));
                    let h = handle_of(&ctx, h)?;
                    let flags = flags.0.unwrap_or(0);
                    let r = msg(&ctx, conv.with_name(&signal, |n| c.op_listen(h, n, on, flags)))?;
                    op(&ctx, r)
                },
            )?,
        )?;
        let w = core.clone();
        let cb = cbs.clone();
        host.set(
            "observe",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>, h: f64, prop: Value<'js>, on: bool| -> Res<bool> {
                    let c = core_of(&ctx, &w)?;
                    let conv = QjsConv::new(ctx.clone(), c.clone(), Some(cb.clone()));
                    let h = handle_of(&ctx, h)?;
                    let r = msg(&ctx, conv.with_name(&prop, |n| c.op_observe(h, n, on)))?;
                    op(&ctx, r)
                },
            )?,
        )?;
        let w = core.clone();
        host.set(
            "style",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>, opcode: u32, h: f64, text: Value<'js>| -> Res<i32> {
                    let c = core_of(&ctx, &w)?;
                    let h = if h == 0.0 { 0 } else { handle_of(&ctx, h)? };
                    let text = if text.is_string() {
                        Some(text.get::<String>()?)
                    } else if text.is_null() || text.is_undefined() {
                        None
                    } else {
                        return Err(Exception::throw_type(&ctx, "style: the payload must be a string or null"));
                    };
                    op(&ctx, c.op_style(opcode, h, text.as_deref()))
                },
            )?,
        )?;
        let w = core.clone();
        let cb = cbs.clone();
        host.set(
            "setChildAttr",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>, parent: f64, child: f64, name: Value<'js>, value: Value<'js>| -> Res<()> {
                    let c = core_of(&ctx, &w)?;
                    let conv = QjsConv::new(ctx.clone(), c.clone(), Some(cb.clone()));
                    let parent = handle_of(&ctx, parent)?;
                    let child = handle_of(&ctx, child)?;
                    let r = msg(&ctx, conv.with_name(&name, |n| c.op_set_child_attr(&conv, parent, child, n, &value)))?;
                    op_conv(&ctx, &conv, r)
                },
            )?,
        )?;
        let w = core.clone();
        let cb = cbs.clone();
        host.set(
            "getChildAttr",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>, parent: f64, child: f64, name: Value<'js>| -> Res<Value<'js>> {
                    let c = core_of(&ctx, &w)?;
                    let conv = QjsConv::new(ctx.clone(), c.clone(), Some(cb.clone()));
                    let parent = handle_of(&ctx, parent)?;
                    let child = handle_of(&ctx, child)?;
                    let r = msg(&ctx, conv.with_name(&name, |n| c.op_get_child_attr(&conv, parent, child, n)))?;
                    op(&ctx, r)
                },
            )?,
        )?;
    }
    globals.set("__svelters_host", host)?;

    let prelude: Function = ctx.eval(PRELUDE)?;
    prelude.call::<_, ()>((native, globals))
}

// ---- module loading ----

/// Resolves relative specifiers against the importing module's name. Names are slash-separated
/// paths relative to the bundle directory; a specifier that would climb above it is rejected.
struct BundleResolver;

impl Resolver for BundleResolver {
    fn resolve<'js>(
        &mut self,
        _ctx: &Ctx<'js>,
        base: &str,
        name: &str,
        _attrs: Option<ImportAttributes<'js>>,
    ) -> Res<String> {
        // Host built-in modules: the app's Rust exports and the object core they import.
        if name == "svelters:core" || name.starts_with("svelters:app/") {
            return Ok(name.to_string());
        }
        if !(name.starts_with("./") || name.starts_with("../")) {
            return Err(rquickjs::Error::new_resolving_message(
                base,
                name,
                "only relative imports inside the bundle directory are supported",
            ));
        }
        let mut parts: Vec<&str> = base
            .rsplit_once('/')
            .map_or(Vec::new(), |(dir, _)| dir.split('/').collect());
        for seg in name.split('/') {
            match seg {
                "" | "." => {}
                ".." => {
                    if parts.pop().is_none() {
                        return Err(rquickjs::Error::new_resolving_message(
                            base,
                            name,
                            "import escapes the bundle directory",
                        ));
                    }
                }
                s => parts.push(s),
            }
        }
        Ok(parts.join("/"))
    }
}

struct BundleLoader {
    root: PathBuf,
}

impl Loader for BundleLoader {
    fn load<'js>(
        &mut self,
        ctx: &Ctx<'js>,
        name: &str,
        _attrs: Option<ImportAttributes<'js>>,
    ) -> Res<Module<'js, Declared>> {
        if name.starts_with("svelters:") {
            let source = crate::exports::builtin_module(name)
                .ok_or_else(|| rquickjs::Error::new_loading_message(name, "no such module: this binary exports no such svelters:app module"))?;
            return Module::declare(ctx.clone(), name, source);
        }
        let path = self
            .root
            .join(name)
            .canonicalize()
            .map_err(|e| rquickjs::Error::new_loading_message(name, e.to_string()))?;
        // Symlinks must not lead out of the bundle directory.
        if !path.starts_with(&self.root) {
            return Err(rquickjs::Error::new_loading_message(
                name,
                "resolves outside the bundle directory",
            ));
        }
        let source = std::fs::read_to_string(&path)
            .map_err(|e| rquickjs::Error::new_loading_message(name, e.to_string()))?;
        Module::declare(ctx.clone(), name, source)
    }
}

/// Reads a sync handler result. `nested` allows one level of object/array whose members must be primitives.
fn sync_result_value<'js>(ctx: &rquickjs::Ctx<'js>, res: Value<'js>, nested: bool) -> rquickjs::Result<AV> {
    Ok(match res.type_of() {
        rquickjs::Type::Null | rquickjs::Type::Undefined | rquickjs::Type::Uninitialized => AV::Null,
        rquickjs::Type::Bool => AV::Bool(res.as_bool().unwrap_or(false)),
        rquickjs::Type::Int => AV::I64(res.as_int().unwrap_or(0) as i64),
        rquickjs::Type::Float => {
            let x = res.as_float().unwrap_or(0.0);
            if x.is_finite() && x.fract() == 0.0 && x.abs() < 9_007_199_254_740_992.0 {
                AV::I64(x as i64)
            } else {
                AV::F64(x)
            }
        }
        rquickjs::Type::String => AV::Str(res.as_string().map(|s| s.to_string()).transpose()?.unwrap_or_default()),
        rquickjs::Type::Array if nested => {
            let arr = res.as_array().cloned().unwrap();
            let mut out = Vec::new();
            for i in 0..arr.len() {
                out.push(sync_result_value(ctx, arr.get(i)?, false)?);
            }
            AV::List(out)
        }
        rquickjs::Type::Object if nested => {
            let obj = res.as_object().cloned().unwrap();
            let mut out = Vec::new();
            for kv in obj.props::<String, Value>() {
                let (k, v) = kv?;
                out.push((k, sync_result_value(ctx, v, false)?));
            }
            AV::Map(out)
        }
        other => {
            return Err(Exception::throw_type(
                ctx,
                &format!("unsupported value type '{}' in a sync handler result", other.as_str()),
            ));
        }
    })
}
