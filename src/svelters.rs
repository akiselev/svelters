use std::cell::Cell;
use std::{cell::UnsafeCell, fs::read_to_string};

use rquickjs::{ExecutorSpawner, IntoAtom};

use crate::SvEvent::{self, *};
use crate::{js::*, svbuilder::Initializers, SvHandle, SveltersBuilder};

type Result<T> = JsResult<T>;

type Sender = flume::Sender<crate::SvEvent>;
type Receiver = flume::Receiver<crate::SvEvent>;

thread_local! {
    static TX: UnsafeCell<Option<Sender>> = UnsafeCell::new(None);
}

#[derive(Clone)]
pub struct Svelters {
    runtime: rquickjs::Runtime,
    context: rquickjs::Context,
    rx: Receiver,
    gc_runs: Cell<usize>,
}

impl Svelters {
    #[tracing::instrument(skip(self))]
    pub async fn run(self) -> Result<()> {
        while let Ok(event) = self.rx.recv_async().await {
            self.gc_runs.set(0);
            let mut shutdown = false;
            if !self.handle_event(event) {
                shutdown = true;
            }

            if !self.check_new_events() {
                shutdown = true;
            }

            if shutdown {
                break;
            }
        }
        tracing::info!("Svelters::run finished");
        Ok(())
    }

    fn check_new_events(&self) -> bool {
        while let Ok(event) = self.rx.try_recv() {
            if !self.handle_event(event) {
                return false;
            }
        }
        true
    }

    fn handle_event(&self, event: SvEvent) -> bool {
        match event {
            Gc => {
                self.gc();
                self.gc_runs.set(self.gc_runs.get() + 1);
                tracing::info!("Ran tracing run number {}", self.gc_runs.get());
            }
            Shutdown => {
                self.on_shutdown().unwrap();
                return false;
            }
            Updated(event) => {
                self.on_updated(event).unwrap();
            }
            ScheduleCallback(func) => {
                self.on_schedule_func(func).unwrap();
            }
        }
        // Tell the caller to continue
        true
    }

    fn on_schedule_func(
        &self,
        func: Box<dyn FnOnce(&Svelters) -> rquickjs::Result<()>>,
    ) -> Result<()> {
        func(self)
    }

    fn on_updated(&self, event: hotwatch::Event) -> Result<()> {
        if let hotwatch::Event::Write(update_path) = event {
            tracing::info!("File {:?} updated", update_path);
            if let Err(error) = self.reload() {
                tracing::error!("Reload error: {}", error);
            };
        }
        Ok(())
    }

    fn on_shutdown(&self) -> Result<()> {
        self.with_js(|ctx| ctx.eval::<(), _>("document.cleanup()"))?;
        Ok(())
    }

    fn reload(&self) -> rquickjs::Result<()> {
        let path = "./dist/index.js";
        self.with_js(|ctx| {
            let module = ctx
                .compile(
                    path,
                    read_to_string(path).expect("Module read for reload failed"),
                )
                .expect("Module compile Failed");
            let start_fn: rquickjs::Function = module
                .get("default")
                .expect("Javascript entrypoint must export a default start function");
            if let Err(err) = start_fn.call::<(bool,), Value>((true,)) {
                tracing::error!("ERROR: {}", err);
                // std::process::exit(1);
            }
            let global = ctx.globals();
            global.set("module", module.clone()).unwrap();
            global.set("start", start_fn.clone()).unwrap();
            rquickjs::Result::Ok(())
        })?;
        self.with_ctx(|ctx| {
            ctx.runtime().run_gc();
        });
        Ok(())
    }

    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> SveltersBuilder {
        SveltersBuilder::new()
    }

    pub fn handle() -> SvHandle {
        let tx = TX.with(|tx| unsafe {
            let ptr: &mut Option<_> = &mut *tx.get();
            if let Some(tx) = ptr {
                tx.clone()
            } else {
                panic!("Svelters has not been initialized");
            }
        });
        SvHandle::new(tx)
    }

    pub fn gc(&self) {
        self.runtime.run_gc();
    }
    pub fn with_ctx<F: FnOnce(&Context) -> R, R>(&self, func: F) -> R {
        func(&self.context)
    }

    pub fn with_js<F: for<'js> FnOnce(Ctx<'js>) -> R, R>(&self, func: F) -> R {
        self.context.with(func)
    }

    pub fn set_global<K, V>(&self, key: K, value: V) -> Result<()>
    where
        K: for<'js> IntoAtom<'js>,
        V: for<'js> IntoJs<'js>,
    {
        self.context.with(move |ctx| ctx.globals().set(key, value))
    }

    pub fn execute_pending_job(&self) -> Result<bool> {
        self.runtime.execute_pending_job()
    }

    pub fn execute_pending_jobs(&self) -> Result<()> {
        while self.runtime.execute_pending_job()? {}
        Ok(())
    }

    pub fn spawn_executor<A: ExecutorSpawner>(&self, executor: A) -> A::JoinHandle {
        self.runtime.spawn_executor(executor)
    }

    pub(crate) fn build(
        runtime: Runtime,
        context: Context,
        initializers: Initializers,
    ) -> Result<Self> {
        let rx = TX.with(|tx| unsafe {
            let ptr: &mut Option<_> = &mut *tx.get();
            if ptr.is_some() {
                panic!("Svelters has already been initialized");
            }
            let (tx, rx) = flume::unbounded();
            *ptr = Some(tx);
            rx
        });
        let svelters = Self {
            context,
            runtime,
            rx,
            gc_runs: Cell::new(0),
        };

        for init in initializers.into_iter() {
            init(&svelters)?;
        }
        Ok(svelters)
    }
}

impl Drop for Svelters {
    fn drop(&mut self) {
        if let Err(err) = cleanup_context(&self.context) {
            tracing::error!("Error while cleaning up context: {:?}", err);
        }
    }
}

#[tracing::instrument(skip(ctx))]
fn cleanup_context(ctx: &rquickjs::Context) -> Result<()> {
    ctx.with(move |ctx| {
        ctx.clear_registery();
        tracing::info!("Cleaning up Svelters...");
        ctx.eval::<(), _>("document?.cleanup?.()")?;
        ctx.globals().keys::<String>().for_each(|key| {
            ctx.globals()
                .set(key.unwrap(), Value::new_undefined(ctx))
                .ok();
        });
        ctx.globals().set("document", Value::new_undefined(ctx))?;
        ctx.globals().set("window", Value::new_undefined(ctx))?;
        ctx.globals().set("setTimeout", Value::new_undefined(ctx))?;
        ctx.globals().set("console", Value::new_undefined(ctx))?;
        tracing::info!("Done cleaning up Svelters...");
        rquickjs::Result::Ok(())
    })?;
    ctx.runtime().run_gc();
    ctx.runtime().run_gc();
    ctx.runtime().run_gc();
    ctx.runtime().run_gc();
    Ok(())
}
