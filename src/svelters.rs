use rquickjs::{ExecutorSpawner, IntoAtom};
use std::future;

use crate::{js::*, svbuilder::Initializers, SveltersBuilder};

type Result<T> = JsResult<T>;

#[derive(Clone)]
pub struct Svelters {
    runtime: rquickjs::Runtime,
    context: rquickjs::Context,
}

impl Svelters {
    pub fn new() -> SveltersBuilder {
        SveltersBuilder::new()
    }

    pub async fn run(&self) -> Result<()> {
        future::pending().await
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
        let svelters = Self { context, runtime };

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
        ctx.eval::<(), _>("document.cleanup()")?;
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
    Ok(())
}
