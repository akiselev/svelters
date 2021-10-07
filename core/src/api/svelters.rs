use rquickjs::{ExecutorSpawner, IntoAtom};

use crate::{init::cleanup_svelters, inner::*};

pub struct Svelters {
    runtime: Runtime,
    quickjs: rquickjs::Runtime,
    context: rquickjs::Context,
}

impl Svelters {
    pub fn new() -> SveltersBuilder {
        SveltersBuilder::new()
    }

    pub fn with_ctx<F: FnOnce(&Context) -> R, R>(&self, func: F) -> R {
        func(&self.context)
    }

    pub fn with_js<F: for<'js> FnOnce(Ctx<'js>) -> R, R>(&self, func: F) -> R {
        self.context.with(func)
    }

    pub fn with_types<F: for<'js> FnOnce(TypeBuilder<'js>)>(&self, func: F) -> Result<()> {
        self.context
            .with(|ctx| Ok(func(TypeBuilder::from_ctx(ctx)?)))
    }

    pub fn execute_pending_job(&self) -> Result<bool> {
        self.quickjs.execute_pending_job()
    }

    pub fn execute_pending_jobs(&self) -> Result<()> {
        while self.quickjs.execute_pending_job()? {}
        Ok(())
    }

    pub fn spawn_executor<A: ExecutorSpawner>(&self, executor: A) -> A::JoinHandle {
        self.quickjs.spawn_executor(executor)
    }

    pub fn set_global<K, V>(&self, key: K, value: V) -> Result<()>
    where
        K: for<'js> IntoAtom<'js>,
        V: for<'js> IntoJs<'js>,
    {
        self.context.with(move |ctx| ctx.globals().set(key, value))
    }

    pub(crate) fn build(
        runtime: Runtime,
        quickjs: rquickjs::Runtime,
        context: rquickjs::Context,
        initializers: Initializers,
    ) -> Result<Self> {
        let svelters = Self {
            runtime,
            quickjs,
            context,
        };

        for init in initializers.into_iter() {
            init(&svelters)?;
        }
        Ok(svelters)
    }
}

impl Drop for Svelters {
    fn drop(&mut self) {
        tracing::warn!("Droping svelters runtime");
        cleanup_svelters(&self.context).expect("Failed to clean up Svelters runtime");
    }
}
