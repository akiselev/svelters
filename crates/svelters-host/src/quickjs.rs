//! rquickjs (QuickJS-NG) implementation of [`JsEngine`]: a synchronous runtime on the calling
//! thread, a bundle-relative module loader, the `__svelters_host` object, and a JS prelude for
//! `console`, timers and `queueMicrotask`.

use crate::convert;
use crate::core::{Core, Name, OpError, OpKind};
use crate::engine::{ErrorKind, JobStatus, JsEngine, JsError};
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

pub(crate) struct QjsEngine {
    // `ctx` before `rt`: the context is released first.
    ctx: Context,
    rt: Runtime,
    base: Instant,
    deadline_ns: Arc<AtomicU64>,
    budget: Option<Duration>,
    async_errors: Rc<RefCell<Vec<JsError>>>,
    core: Rc<Core>,
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
        ctx.with(|ctx| {
            install(&ctx, weak, base)
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
        let r = self.ctx.with(|ctx| f(&ctx).map_err(|e| js_err(&ctx, e)));
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

    fn dispatch(&mut self, handle: u64, signal: &str, args: &[AV]) -> Result<(), JsError> {
        let core = self.core.clone();
        self.call_guarded(|ctx| {
            let f: Value = ctx.globals().get("__svelters_dispatch")?;
            let Some(f) = f.as_function() else {
                return Err(Exception::throw_message(
                    ctx,
                    "globalThis.__svelters_dispatch is not registered",
                ));
            };
            let conv = QjsConv::new(ctx.clone(), core);
            let list = convert::decode(&conv, &AV::List(args.to_vec()), &*conv.core)
                .map_err(|m| Exception::throw_message(ctx, &m))?;
            f.call::<_, Value>((handle as f64, signal, list))
                .map(|_| ())
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

/// Throws an op failure as the JS error class that matches it.
fn throw<'js, T>(ctx: &Ctx<'js>, e: OpError) -> Res<T> {
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

fn install<'js>(ctx: &Ctx<'js>, core: Weak<Core>, base: Instant) -> Res<()> {
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
        native.set(
            "now",
            Function::new(ctx.clone(), move || base.elapsed().as_secs_f64() * 1000.0)?,
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
        host.set(
            "create",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>, kind: Value<'js>| -> Res<f64> {
                    let c = core_of(&ctx, &w)?;
                    let conv = QjsConv::new(ctx.clone(), c.clone());
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
            host.set(
                js,
                Function::new(
                    ctx.clone(),
                    move |ctx: Ctx<'js>, a: Value<'js>, b: Opt<Value<'js>>| -> Res<u32> {
                        let c = core_of(&ctx, &w)?;
                        let conv = QjsConv::new(ctx.clone(), c.clone());
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
                            let conv = QjsConv::new(ctx.clone(), c.clone());
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
        host.set(
            "setProp",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>, h: f64, name: Value<'js>, value: Value<'js>| -> Res<()> {
                    let c = core_of(&ctx, &w)?;
                    let conv = QjsConv::new(ctx.clone(), c.clone());
                    let h = handle_of(&ctx, h)?;
                    let r = msg(&ctx, conv.with_name(&name, |n| c.op_set_prop(&conv, h, n, &value)))?;
                    op_conv(&ctx, &conv, r)
                },
            )?,
        )?;
        let w = core.clone();
        host.set(
            "getProp",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>, h: f64, name: Value<'js>| -> Res<Value<'js>> {
                    let c = core_of(&ctx, &w)?;
                    let conv = QjsConv::new(ctx.clone(), c.clone());
                    let h = handle_of(&ctx, h)?;
                    let r = msg(&ctx, conv.with_name(&name, |n| c.op_get_prop(&conv, h, n)))?;
                    op(&ctx, r)
                },
            )?,
        )?;
        let w = core.clone();
        host.set(
            "call",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>, h: f64, method: Value<'js>, args: Value<'js>| -> Res<Value<'js>> {
                    let c = core_of(&ctx, &w)?;
                    if !args.is_array() {
                        return Err(Exception::throw_type(&ctx, "call: args must be an array"));
                    }
                    let conv = QjsConv::new(ctx.clone(), c.clone());
                    let h = handle_of(&ctx, h)?;
                    let r = msg(&ctx, conv.with_name(&method, |n| c.op_call(&conv, h, n, &args)))?;
                    op_conv(&ctx, &conv, r)
                },
            )?,
        )?;
        let w = core;
        host.set(
            "listen",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>, h: f64, signal: Value<'js>, on: bool| -> Res<()> {
                    let c = core_of(&ctx, &w)?;
                    let conv = QjsConv::new(ctx.clone(), c.clone());
                    let h = handle_of(&ctx, h)?;
                    let r = msg(&ctx, conv.with_name(&signal, |n| c.op_listen(h, n, on)))?;
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
