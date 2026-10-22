//! rquickjs (QuickJS-NG) implementation of [`JsEngine`]: a synchronous runtime on the calling
//! thread, a module loader over a [`ModuleProvider`], the `__svelters_host` object, and a JS prelude for
//! `console`, timers and `queueMicrotask`.

use crate::convert;
use crate::core::{Core, Name, OpError, OpKind};
use crate::engine::{ChangedProp, ErrorKind, HotOutcome, JobStatus, JsEngine, JsError, ModuleProvider};
use std::collections::{HashMap, HashSet};
use svelters_modules::ModuleCode;
use rquickjs::function::Opt;
use rquickjs::loader::{ImportAttributes, Loader, Resolver};
use rquickjs::module::Declared;
use rquickjs::prelude::Coerced;
use rquickjs::{
    CatchResultExt, Context, Ctx, Exception, FromJs, Function, Module, Object, Runtime, Value,
};
use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use svelters_abi::{NameKind, Value as AV};

mod buffers;
mod conv;
mod platform;
use conv::QjsConv;

/// Runtimes leaked on this thread because Rust still held one of their transferred buffers (E2).
pub(crate) fn leaked_runtimes() -> u64 {
    buffers::LEAKED_RUNTIMES.with(std::cell::Cell::get)
}

/// How long dropping a runtime waits for workers to drop its transferred buffers before it leaks the
/// runtime instead (freeing it would free memory a worker is using).
const TRANSFER_GRACE: Duration = Duration::from_secs(1);

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
    /// E2: this runtime's transfer tokens.
    pub(crate) home: Arc<buffers::QjsHome>,
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
    async_errors: Rc<RefCell<Vec<JsError>>>,
    core: Rc<Core>,
    callbacks: Rc<QjsCallbacks>,
    /// EM1: module namespaces of the entries loaded with `load_entry`.
    entries: Vec<rquickjs::Persistent<Object<'static>>>,
    /// H1: the module versions the resolver hands out (`<id>?v=<n>`), the provider of the last
    /// load and the swaps applied since.
    versions: Versions,
    provider: Option<Rc<dyn ModuleProvider>>,
    swaps: u32,
}

/// The latest `?v=<n>` of each module that was swapped in place.
type Versions = Rc<RefCell<HashMap<String, u32>>>;

/// A plain object from `(key, value)` pairs (EM1 props and info patches).
fn plain_object<'js>(ctx: &Ctx<'js>, core: &Rc<Core>, pairs: &[(String, AV)]) -> Res<Object<'js>> {
    let conv = QjsConv::new(ctx.clone(), core.clone(), None);
    let o = Object::new(ctx.clone())?;
    for (k, v) in pairs {
        let v = convert::decode(&conv, v, &**core).map_err(|m| Exception::throw_message(ctx, &m))?;
        o.set(k.as_str(), v)?;
    }
    Ok(o)
}

fn embed_api<'js>(ctx: &Ctx<'js>) -> Res<Object<'js>> {
    let api: Value = ctx.globals().get("__svelters_embed")?;
    api.into_object().ok_or_else(|| {
        Exception::throw_message(ctx, "globalThis.__svelters_embed is not installed: build the bundle with svelteConfig({ embed })")
    })
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
            home: buffers::QjsHome::new(core.ops.queue.clone()),
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
            async_errors,
            core: core.clone(),
            callbacks,
            entries: Vec::new(),
            versions: Versions::default(),
            provider: None,
            swaps: 0,
        })
    }

    fn arm(&self) {
        let d = self
            .core
            .budget
            .get()
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

    /// Installs the provider as the module loader and evaluates its entry; returns the entry's
    /// namespace. A module that failed synchronously has a rejected promise; one waiting on
    /// top-level await settles later and is reported as an unhandled rejection if it fails.
    fn eval_entry(&mut self, provider: Rc<dyn ModuleProvider>) -> Result<rquickjs::Persistent<Object<'static>>, JsError> {
        provider.begin();
        let entry = provider.entry();
        let code = provider.load(&entry).map_err(|m| JsError::new(ErrorKind::Load, m))?;
        let core = Rc::downgrade(&self.core);
        let hot = provider.hmr();
        self.versions.borrow_mut().clear();
        self.swaps = 0;
        self.provider = Some(provider.clone());
        self.rt.set_loader(
            ProviderResolver { provider: provider.clone(), versions: self.versions.clone() },
            ProviderLoader { provider, core: core.clone(), hot },
        );
        let res = self.call_guarded(|ctx| {
            if hot {
                ensure_hot(ctx)?;
            }
            let (m, promise) = declare(ctx, &entry, code, &core, hot)?.eval()?;
            if let Some(Err(e)) = promise.result::<Value>() {
                return Err(e);
            }
            let ns = m.namespace()?;
            Ok(rquickjs::Persistent::save(ctx, ns))
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

    fn near_memory_limit(&self) -> bool {
        let u = self.rt.memory_usage();
        u.malloc_limit > 0 && u.malloc_size + (u.malloc_limit / 8).max(256 * 1024) >= u.malloc_limit
    }
}

impl JsEngine for QjsEngine {
    fn hot_update(&mut self, provider: Rc<dyn ModuleProvider>, changed: &[String]) -> Result<HotOutcome, JsError> {
        if !provider.hmr() {
            return Ok(HotOutcome::Reload("the app is not compiled for HMR".into()));
        }
        let cfg = &self.core.cfg;
        if cfg.hmr_max_swaps > 0 && self.swaps >= cfg.hmr_max_swaps {
            return Ok(HotOutcome::Reload(format!("swap budget: {} in-place updates since the last full load", self.swaps)));
        }
        let usage = self.rt.memory_usage();
        if cfg.memory_limit > 0 && usage.malloc_size as f64 > cfg.memory_limit as f64 * cfg.hmr_memory_fraction as f64 {
            return Ok(HotOutcome::Reload(format!("memory budget: heap at {} KiB", usage.malloc_size / 1024)));
        }
        // Boundaries: walk up from each changed module through the importers to the first
        // self-accepting module. `redo` are the modules to re-evaluate, `boundaries` the accepting
        // ones among them (their accept callbacks run).
        let accepts = |id: &str| -> Result<bool, JsError> {
            self.call_guarded(|ctx| {
                let hot: Object = ctx.globals().get("__svelters_hot")?;
                let f: Function = hot.get("accepts")?;
                f.call((id,))
            })
        };
        // H1b: a boundary that accepts only some exports (`acceptExports`) while another export changed is
        // rejected; the change then propagates to that module's importers (as one that does not accept).
        let saved = self.versions.borrow().clone();
        let core = Rc::downgrade(&self.core);
        let versions = self.versions.clone();
        let mut redo: Vec<String> = Vec::new();
        let res = (|| -> Result<Option<String>, JsError> {
            let mut rejected: HashSet<String> = HashSet::new();
            loop {
                redo.clear();
                let mut boundaries: Vec<String> = Vec::new();
                let mut seen: HashSet<String> = HashSet::new();
                let mut queue: Vec<String> = changed.to_vec();
                while let Some(m) = queue.pop() {
                    if !seen.insert(m.clone()) {
                        continue;
                    }
                    redo.push(m.clone());
                    if !rejected.contains(&m) && accepts(&m)? {
                        boundaries.push(m);
                        continue;
                    }
                    let importers = provider.importers(&m);
                    if importers.is_empty() {
                        let why = if rejected.contains(&m) { "changes an export that nothing accepts" } else { "is not accepted by it or by any importer" };
                        return Ok(Some(format!("{m} {why}")));
                    }
                    queue.extend(importers);
                }
                // New versions: the resolver now hands out `<id>?v=<n>` for every re-evaluated module, so
                // the boundaries' imports reach the new code. A failure restores the previous versions.
                {
                    let mut v = versions.borrow_mut();
                    for id in &redo {
                        *v.entry(id.clone()).or_insert(0) += 1;
                    }
                }
                let mut loaded = Vec::new();
                for id in &boundaries {
                    let name = versioned(id, &versions);
                    let code = provider.load(id).map_err(|m| JsError::new(ErrorKind::Load, m))?;
                    let ns = self.call_guarded(|ctx| {
                        let (m, promise) = declare(ctx, &name, code, &core, true)?.eval()?;
                        if let Some(Err(e)) = promise.result::<Value>() {
                            return Err(e);
                        }
                        Ok(rquickjs::Persistent::save(ctx, m.namespace()?))
                    })?;
                    loaded.push((id.clone(), ns));
                }
                let mut failing = Vec::new();
                for (id, ns) in &loaded {
                    let reason: Option<String> = self.call_guarded(|ctx| {
                        let hot: Object = ctx.globals().get("__svelters_hot")?;
                        let f: Function = hot.get("check")?;
                        f.call((id.as_str(), ns.clone().restore(ctx)?))
                    })?;
                    if reason.is_some() {
                        failing.push(id.clone());
                    }
                }
                if !failing.is_empty() {
                    rejected.extend(failing);
                    continue;
                }
                for (id, ns) in loaded {
                    let reason: Option<String> = self.call_guarded(|ctx| {
                        let hot: Object = ctx.globals().get("__svelters_hot")?;
                        let f: Function = hot.get("update")?;
                        f.call((id.as_str(), ns.restore(ctx)?))
                    })?;
                    if reason.is_some() {
                        return Ok(reason);
                    }
                }
                return Ok(None);
            }
        })();
        match res {
            Ok(None) => {
                self.swaps += 1;
                self.async_errors.borrow_mut().clear();
                Ok(HotOutcome::Applied { modules: redo })
            }
            Ok(Some(reason)) => Ok(HotOutcome::Reload(reason)),
            Err(mut e) => {
                *self.versions.borrow_mut() = saved;
                self.async_errors.borrow_mut().clear();
                if e.kind == ErrorKind::Exception {
                    e.kind = ErrorKind::Load;
                }
                Err(e)
            }
        }
    }

    fn load_module(&mut self, provider: Rc<dyn ModuleProvider>) -> Result<(), JsError> {
        self.eval_entry(provider).map(|_| ())
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

    fn settle_ops(&mut self, batch: &[crate::engine::OpSettle]) -> Result<(), JsError> {
        let core = self.core.clone();
        // The callback store gives the converter this runtime's buffer counters (E2 results).
        let cbs = self.callbacks.clone();
        self.call_guarded(|ctx| {
            let conv = QjsConv::new(ctx.clone(), core.clone(), Some(cbs));
            let arr = rquickjs::Array::new(ctx.clone())?;
            let mut i = 0;
            for s in batch {
                let (kind, v) = conv.op_value(s);
                arr.set(i, kind)?;
                arr.set(i + 1, s.id as f64)?;
                arr.set(i + 2, v)?;
                i += 3;
            }
            let internal: Object = ctx.globals().get("__svelters_internal")?;
            let f: Function = internal.get("settleOps")?;
            f.call::<_, ()>((arr,))
        })
    }

    fn take_errors(&mut self) -> Vec<JsError> {
        std::mem::take(&mut *self.async_errors.borrow_mut())
    }

    fn release_buffers(&mut self, ids: &[u64]) {
        let home = self.callbacks.home.clone();
        let r = self.ctx.with(|ctx| buffers::release(&ctx, &home, ids).map_err(|e| js_err(&ctx, e)));
        if let Err(e) = r {
            self.core.report(e);
        }
    }

    fn buffer_stats(&self) -> crate::engine::BufferStats {
        self.callbacks.home.snapshot()
    }

    fn jobs_pending(&self) -> bool {
        self.rt.is_job_pending()
    }

    fn load_entry(&mut self, provider: Rc<dyn ModuleProvider>) -> Result<u32, JsError> {
        let ns = self.eval_entry(provider)?;
        self.entries.push(ns);
        Ok(self.entries.len() as u32 - 1)
    }

    fn mount_root(&mut self, entry: u32, export: &str, root: u32, target: u64, props: &[(String, AV)]) -> Result<(), JsError> {
        let ns = self
            .entries
            .get(entry as usize)
            .cloned()
            .ok_or_else(|| JsError::new(ErrorKind::Host, format!("no entry #{entry}")))?;
        let core = self.core.clone();
        self.call_guarded(|ctx| {
            let ns = ns.restore(ctx)?;
            let component: Value = ns.get(export)?;
            if !component.is_function() {
                return Err(Exception::throw_type(ctx, &format!("the bundle exports no component named '{export}'")));
            }
            let props = plain_object(ctx, &core, props)?;
            let mount: Function = embed_api(ctx)?.get("mount")?;
            mount.call::<_, Value>((component, root, target as f64, props)).map(|_| ())
        })
    }

    fn root_call(&mut self, method: &str, root: u32, patch: Option<&[(String, AV)]>) -> Result<AV, JsError> {
        let core = self.core.clone();
        self.call_guarded(|ctx| {
            let f: Function = embed_api(ctx)?.get(method)?;
            let res: Value = match patch {
                Some(p) => f.call((root, plain_object(ctx, &core, p)?))?,
                None => f.call((root,))?,
            };
            sync_result_value(ctx, res, false)
        })
    }
}

impl Drop for QjsEngine {
    // ZU3: host teardown: waits up to TRANSFER_GRACE for workers to hand back buffers, after the last frame.
    #[allow(clippy::disallowed_methods)]
    fn drop(&mut self) {
        // E2: a worker may still use a transferred buffer of this runtime (its call was cancelled, but
        // it has not looked yet). Wait a little; past that, leak the runtime rather than free memory
        // under the worker (`docs/notes/egui-rust-exports.md` §8, shutdown order).
        let home = self.callbacks.home.clone();
        if home.live.load(Ordering::Acquire) == 0 {
            return;
        }
        let end = Instant::now() + TRANSFER_GRACE;
        loop {
            // A worker that stops returns its buffer as a result nobody waits for: dropped here.
            self.core.ops_reap();
            if home.live.load(Ordering::Acquire) == 0 || Instant::now() >= end {
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        let live = home.live.load(Ordering::Acquire);
        if live > 0 {
            buffers::LEAKED_RUNTIMES.with(|n| n.set(n.get() + 1));
            eprintln!(
                "[svelters] {live} transferred buffer(s) of a JS runtime that is going away are still held by Rust                  (a worker that ignores cancellation, or a stored Buffer<T>): the runtime is leaked instead of freed"
            );
            // A context reference that is never dropped keeps the runtime (and the memory) alive.
            std::mem::forget(self.ctx.clone());
        }
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
        handle: None,
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
        // Scripted mode reads the virtual clock: a transition there would otherwise wait for wall
        // time while its frames fire on the virtual clock, ending only by chance before
        // `max_virtual_timer_fires` (TK1 saw both outcomes).
        let w = core.clone();
        native.set(
            "now",
            Function::new(ctx.clone(), move || {
                w.upgrade().and_then(|c| c.virtual_ms()).unwrap_or_else(|| base.elapsed().as_secs_f64() * 1000.0)
            })?,
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
        // E1: the signal of an offloaded export call aborted (JS rejected its Promise already).
        let w = core.clone();
        native.set(
            "cancelOp",
            Function::new(ctx.clone(), move |id: f64| {
                if let Some(c) = w.upgrade() {
                    c.op_cancel_from_js(id as u64);
                }
            })?,
        )?;
        // E3: a stream iterator handed `n` items out: room for the producer.
        let w = core.clone();
        native.set(
            "streamAck",
            Function::new(ctx.clone(), move |id: f64, n: f64| {
                if let Some(c) = w.upgrade() {
                    c.op_stream_ack(id as u64, n.max(0.0) as usize);
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
        // EM1: what `@svelters/native-runtime/embed` reports about a mounted root.
        let w = core.clone();
        host.set(
            "rootFault",
            Function::new(ctx.clone(), move |ctx: Ctx<'js>, root: u32, message: String, stack: Opt<Option<String>>| -> Res<()> {
                let c = core_of(&ctx, &w)?;
                let mut error = JsError::new(classify(&message), message);
                error.stack = stack.0.flatten();
                let error = c.map_error(error);
                c.root_message(crate::engine::RootMessage::Fault { root, error });
                Ok(())
            })?,
        )?;
        let w = core.clone();
        host.set(
            "rootInfo",
            Function::new(ctx.clone(), move |ctx: Ctx<'js>, root: u32, title: Opt<Option<String>>, dirty: Opt<Option<bool>>| -> Res<()> {
                let c = core_of(&ctx, &w)?;
                c.root_message(crate::engine::RootMessage::Info { root, title: title.0.flatten(), dirty: dirty.0.flatten() });
                Ok(())
            })?,
        )?;
        let w = core.clone();
        host.set(
            "rootEmit",
            Function::new(ctx.clone(), move |ctx: Ctx<'js>, root: u32, name: String, value: String| -> Res<()> {
                let c = core_of(&ctx, &w)?;
                c.root_message(crate::engine::RootMessage::Emit { root, name, value });
                Ok(())
            })?,
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
        // E3: `persist(key)` → [handle, native class] or null; `persist(key, h)` keeps the app object `h`
        // across hot reloads; `persist(key, null)` forgets it.
        let w = core.clone();
        host.set(
            "persist",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>, key: String, value: rquickjs::function::Opt<Value<'js>>| -> Res<Value<'js>> {
                    let c = core_of(&ctx, &w)?;
                    match value.0 {
                        None => match c.op_persist_get(&key) {
                            Some((h, native)) => {
                                let a = rquickjs::Array::new(ctx.clone())?;
                                a.set(0, h as f64)?;
                                a.set(1, native)?;
                                Ok(a.into_value())
                            }
                            None => Ok(Value::new_null(ctx.clone())),
                        },
                        Some(v) => {
                            let h = if v.is_null() || v.is_undefined() {
                                None
                            } else if let Some(n) = v.as_number() {
                                Some(handle_of(&ctx, n)?)
                            } else {
                                let o = v.as_object().ok_or_else(|| Exception::throw_type(&ctx, "persist(key, obj): obj must be an object of a svelters:app module"))?;
                                let n: f64 = o.get("svlHandle").map_err(|_| Exception::throw_type(&ctx, "persist(key, obj): obj has no handle"))?;
                                Some(handle_of(&ctx, n)?)
                            };
                            op(&ctx, c.op_persist_set(&key, h))?;
                            Ok(Value::new_bool(ctx.clone(), true))
                        }
                    }
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
        // CX3: native links between a store field and a native prop (`core/links.rs`)
        let w = core.clone();
        host.set(
            "link",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>, store: f64, field: String, node: f64, prop: String| -> Res<u32> {
                    let c = core_of(&ctx, &w)?;
                    let store = handle_of(&ctx, store)?;
                    let node = handle_of(&ctx, node)?;
                    op(&ctx, c.op_link(store, &field, node, &prop))
                },
            )?,
        )?;
        let w = core.clone();
        host.set(
            "unlink",
            Function::new(ctx.clone(), move |ctx: Ctx<'js>, id: u32| -> Res<()> {
                let c = core_of(&ctx, &w)?;
                op(&ctx, c.op_unlink(id))
            })?,
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
        host.set(
            "bounds",
            Function::new(ctx.clone(), move |ctx: Ctx<'js>, h: f64| -> Res<Value<'js>> {
                let c = core_of(&ctx, &w)?;
                let h = handle_of(&ctx, h)?;
                match op(&ctx, c.op_bounds(h))? {
                    Some(r) => {
                        let a = rquickjs::Array::new(ctx.clone())?;
                        for (i, v) in r.iter().enumerate() {
                            a.set(i, *v)?;
                        }
                        Ok(a.into_value())
                    }
                    None => Ok(Value::new_null(ctx.clone())),
                }
            })?,
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
        // DX1: settings, UI state, paths and `logger()` records (`svelters:settings|paths|log`).
        let w = core.clone();
        host.set(
            "dx",
            Function::new(ctx.clone(), move |ctx: Ctx<'js>, op: String, a: String, b: String| -> Res<String> {
                let c = core_of(&ctx, &w)?;
                c.dx_op(&op, &a, &b).map_err(|(code, m)| Exception::throw_message(&ctx, &format!("{code}: {m}")))
            })?,
        )?;
        // E2: `alloc(Float32Array, n)` (`@svelters/types/core`): a typed array over a Rust allocation,
        // which a `Buffer<T>` parameter takes back without a copy.
        let cb = cbs.clone();
        host.set(
            "alloc",
            Function::new(ctx.clone(), move |ctx: Ctx<'js>, name: String, n: f64| -> Res<Value<'js>> {
                let elem = svelters_abi::Elem::from_js_name(&name).ok_or_else(|| {
                    Exception::throw_type(&ctx, &format!("alloc: {name} is not a typed array a Rust export takes (Uint8Array, Int8Array, Uint16Array, Int16Array, Uint32Array, Int32Array, BigUint64Array, BigInt64Array, Float32Array, Float64Array)"))
                })?;
                if !(n >= 0.0 && n.fract() == 0.0 && n < 9_007_199_254_740_992.0) {
                    return Err(Exception::throw_range(&ctx, &format!("alloc: invalid length {n}")));
                }
                buffers::alloc(&ctx, &cb.home, elem, n as usize).map_err(|m| match m.strip_prefix("RangeError: ") {
                    Some(r) => Exception::throw_range(&ctx, r),
                    None => Exception::throw_message(&ctx, &m),
                })
            })?,
        )?;
    }
    platform::install(ctx, &host, &core)?;
    globals.set("__svelters_host", host)?;

    let prelude: Function = ctx.eval(PRELUDE)?;
    prelude.call::<_, ()>((native, globals))
}

// ---- module loading ----

/// The QuickJS side of a [`ModuleProvider`]: resolution.
struct ProviderResolver {
    provider: Rc<dyn ModuleProvider>,
    versions: Versions,
}

/// A module's id without the `?v=<n>` of an in-place update.
fn unversioned(name: &str) -> &str {
    name.split_once("?v=").map_or(name, |(id, _)| id)
}

/// `id` as the engine knows it now: `id?v=<n>` once it was swapped in place.
fn versioned(id: &str, versions: &Versions) -> String {
    match versions.borrow().get(id) {
        Some(n) => format!("{id}?v={n}"),
        None => id.to_string(),
    }
}

impl Resolver for ProviderResolver {
    fn resolve<'js>(&mut self, _ctx: &Ctx<'js>, base: &str, name: &str, _attrs: Option<ImportAttributes<'js>>) -> Res<String> {
        let id = self.provider.resolve(unversioned(base), name).map_err(|m| rquickjs::Error::new_resolving_message(base, name, m))?;
        Ok(versioned(&id, &self.versions))
    }
}

/// The QuickJS side of a [`ModuleProvider`]: loading. Source maps go to the core's map store.
struct ProviderLoader {
    provider: Rc<dyn ModuleProvider>,
    core: Weak<Core>,
    /// Modules get `import.meta.hot` (the provider compiles for HMR).
    hot: bool,
}

impl Loader for ProviderLoader {
    fn load<'js>(&mut self, ctx: &Ctx<'js>, name: &str, _attrs: Option<ImportAttributes<'js>>) -> Res<Module<'js, Declared>> {
        let code = self.provider.load(unversioned(name)).map_err(|m| rquickjs::Error::new_loading_message(name, m))?;
        declare(ctx, name, code, &self.core, self.hot)
    }
}

/// The engine tag of [`ModuleCode::Cached`] bytecode this engine reads.
const BYTECODE_TAG: &str = "quickjs-ng";

/// The JS side of `import.meta.hot` (hot.js).
const HOT_JS: &str = include_str!("hot.js");

fn ensure_hot(ctx: &Ctx<'_>) -> Res<()> {
    if ctx.globals().get::<_, Value>("__svelters_hot")?.is_undefined() {
        ctx.eval::<(), _>(HOT_JS)?;
    }
    Ok(())
}

fn declare<'js>(ctx: &Ctx<'js>, name: &str, code: ModuleCode, core: &Weak<Core>, hot: bool) -> Res<Module<'js, Declared>> {
    let m = declare_code(ctx, name, code, core)?;
    if hot {
        // `import.meta.hot` of module `<id>[?v=n]` is the context of `<id>`
        let hot_obj: Object = ctx.globals().get("__svelters_hot")?;
        let make: Function = hot_obj.get("ctx")?;
        m.meta()?.set("hot", make.call::<_, Value>((unversioned(name),))?)?;
    }
    Ok(m)
}

fn declare_code<'js>(ctx: &Ctx<'js>, name: &str, code: ModuleCode, core: &Weak<Core>) -> Res<Module<'js, Declared>> {
    match code {
        ModuleCode::Source { js, map } => {
            let core = core.upgrade();
            if let (Some(map), Some(core)) = (map, &core) {
                core.maps.borrow_mut().insert(name, &map);
            }
            // Q2: through the bytecode cache, except HMR re-imports (`?v=n`, one-off names)
            match core.and_then(|c| c.bytecode.borrow().clone()).filter(|_| !name.contains('?')) {
                Some(cache) => declare_cached(ctx, name, js, &cache),
                None => Module::declare(ctx.clone(), name, js),
            }
        }
        // Safety: only bytecode tagged for this engine is read; its writer is the same QuickJS build.
        ModuleCode::Cached { engine, bytes } if engine == BYTECODE_TAG => unsafe { Module::load(ctx.clone(), &bytes) },
        ModuleCode::Cached { engine, .. } => Err(rquickjs::Error::new_loading_message(name, format!("bytecode for engine {engine:?}, not {BYTECODE_TAG:?}"))),
    }
}

/// Declares a source module from its cached bytecode, or compiles it and caches the bytecode.
fn declare_cached<'js>(ctx: &Ctx<'js>, name: &str, js: String, cache: &crate::bytecode::BytecodeCache) -> Res<Module<'js, Declared>> {
    if let Some(bytes) = cache.get(name, &js) {
        // Safety: the entry passed its header and checksum checks for this engine stamp, name and
        // source; it was written by `Module::write` of this QuickJS build (bytecode.rs, "Trust").
        match unsafe { Module::load(ctx.clone(), &bytes) } {
            Ok(m) => return Ok(m),
            Err(e) => {
                tracing::debug!(target: "svelters::host", "bytecode cache entry for {name} rejected ({e}); recompiling");
                let _ = ctx.catch();
                cache.remove(name, &js);
            }
        }
    }
    let m = Module::declare(ctx.clone(), name, js.as_str())?;
    match m.write(crate::bytecode::write_options()) {
        Ok(bytes) => {
            if let Err(e) = cache.put(name, &js, &bytes) {
                tracing::debug!(target: "svelters::host", "bytecode cache entry for {name} not written: {e}");
            }
        }
        Err(e) => {
            tracing::debug!(target: "svelters::host", "bytecode of {name} not written: {e}");
            let _ = ctx.catch();
        }
    }
    Ok(m)
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
