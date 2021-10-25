use rquickjs::{ExecutorSpawner, IntoAtom};

use crate::inner::*;

pub struct Svelters {
    quickjs: rquickjs::Runtime,
    viewmodel: ViewModelRuntime,
}

impl Svelters {
    pub fn new() -> SveltersBuilder {
        SveltersBuilder::new()
    }

    pub fn gc(&self) {
        self.quickjs.run_gc();
    }

    pub fn with_ctx<F: FnOnce(&Context) -> R, R>(&self, func: F) -> R {
        self.viewmodel.with_ctx(func)
    }

    pub fn with_js<F: for<'js> FnOnce(Ctx<'js>) -> R, R>(&self, func: F) -> R {
        self.viewmodel.with_js(func)
    }

    pub fn with_types<F: for<'js> FnOnce(TypeBuilder<'js>)>(&self, func: F) -> Result<()> {
        self.viewmodel.with_types(func)
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
        self.viewmodel.set_global(key, value)
    }

    pub(crate) fn build(
        viewmodel: ViewModelRuntime,
        quickjs: rquickjs::Runtime,
        initializers: Initializers,
    ) -> Result<Self> {
        let svelters = Self { viewmodel, quickjs };

        for init in initializers.into_iter() {
            init(&svelters)?;
        }
        Ok(svelters)
    }
}
