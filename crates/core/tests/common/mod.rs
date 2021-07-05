use rquickjs::{Context, Ctx, Evaluated, Runtime};
use svelters_core::prelude::Framework;

pub(in crate) mod defs;
pub(in crate) mod framework;
pub(in crate) mod modules;
pub(in crate) mod types;

pub use defs::*;
pub use framework::*;
pub use modules::*;
pub use types::*;

pub use rquickjs::{
    BuiltinResolver, Error, Loader, Module as QJsModule, ModuleLoader, Resolver, Result, Value,
};
pub use svelters_core::prelude::*;

pub struct Runner {
    src: &'static str,
}

impl Runner {
    pub const fn new(src: &'static str) -> Self {
        Self { src }
    }

    pub fn run<F: for<'js> FnOnce(Ctx<'js>, QJsModule<'js, Evaluated>) -> anyhow::Result<R>, R>(
        &self,
        name: &'static str,
        func: F,
    ) -> anyhow::Result<R> {
        let rt = Runtime::new()?;
        let ctx = Context::full(&rt)?;
        let (resolver, loader) = types::Fw::get_loader();
        rt.set_loader(resolver, loader);
        let res: anyhow::Result<R> = ctx.with(move |ctx| {
            svelters_core::console::init(ctx)?;
            svelters_core::timing::init(ctx)?;
            let module = QJsModule::new(ctx, name, self.src)?;
            let evaled = module.eval()?;
            func(ctx, evaled)
        });

        while let true = ctx.runtime().execute_pending_job()? {}

        res
    }
}
