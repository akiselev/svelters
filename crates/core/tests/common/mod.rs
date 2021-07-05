use cucumber_rust::{async_trait, criteria::feature, futures::FutureExt, Context, Cucumber, World};
pub(crate) use rquickjs::{
    BuiltinResolver, Error, Evaluated, Loader, Module as QJsModule, ModuleLoader, RegisteryKey,
    Resolver, Result, Runtime, Value,
};

use svelters_core::prelude::Framework;

pub(in crate) mod defs;
pub(in crate) mod framework;
pub(in crate) mod modules;
pub(in crate) mod types;

pub use defs::*;
pub use framework::*;
pub use modules::*;
pub use types::*;

pub use svelters_core::prelude::*;

pub struct CoreWorld {
    pub rt: rquickjs::Runtime,
    pub ctx: rquickjs::Context,
    /// Last loaded module
    pub module: RegisteryKey,
    /// Last Object created, which is target of tests
    pub subject: RegisteryKey,
    /// Result of last operation
    pub result: RegisteryKey,
}

#[async_trait(?Send)]
impl World for CoreWorld {
    type Error = rquickjs::Error;

    async fn new() -> Result<Self> {
        let rt = rquickjs::Runtime::new()?;
        let ctx = rquickjs::Context::full(&rt)?;
        let (resolver, loader) = Fw::get_loader();
        rt.set_loader(resolver, loader);
        let key = ctx
            .with(move |ctx| {
                svelters_core::console::init(ctx)?;
                svelters_core::timing::init(ctx)?;
                let key = ctx.register(rquickjs::Undefined.into_value(ctx));
                Result::Ok(key)
            })
            .unwrap();
        Ok(CoreWorld {
            rt,
            ctx,
            module: key.clone(),
            subject: key.clone(),
            result: key.clone(),
        })
    }
}
