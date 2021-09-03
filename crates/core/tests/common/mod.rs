use cucumber_rust::{async_trait, World};
use rquickjs::Ctx;
pub(crate) use rquickjs::{
    BuiltinResolver, Evaluated, Loader, Module as QJsModule, ModuleLoader, RegisteryKey, Resolver,
    Result, Value,
};

use svelters_core::prelude::Framework;

pub(in crate) mod cast;
pub(in crate) mod classes;
pub(in crate) mod framework;
pub(in crate) mod modules;
pub(in crate) mod types;

pub use cast::*;
pub use classes::*;
pub use framework::*;
pub use modules::*;
pub use types::*;

pub use svelters_core::prelude::*;

pub struct CoreWorld {
    pub rt: rquickjs::Runtime,
    pub ctx: rquickjs::Context,
    undefined: RegisteryKey,
    /// Last loaded module
    module: RegisteryKey,
    /// Last Object created, which is target of tests
    subject: RegisteryKey,
    /// Result of last operation
    result: RegisteryKey,
}

#[async_trait(?Send)]
impl World for CoreWorld {
    type Error = rquickjs::Error;

    async fn new() -> Result<Self> {
        let rt = rquickjs::Runtime::new()?;
        let ctx = rquickjs::Context::full(&rt)?;
        let (resolver, loader) = FW::get_loader();
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
            undefined: key.clone(),
            module: key.clone(),
            subject: key.clone(),
            result: key.clone(),
        })
    }
}

impl CoreWorld {
    pub fn set_module(&mut self, key: RegisteryKey) -> Result<()> {
        if self.module != self.undefined {
            self.ctx.with(|ctx| {
                let _ = ctx.deregister(self.module);
            });
        }
        self.module = key;
        Ok(())
    }

    pub fn set_result(&mut self, key: RegisteryKey) -> Result<()> {
        if self.result != self.undefined {
            self.ctx.with(|ctx| {
                let _ = ctx.deregister(self.result);
            });
        }
        self.result = key;
        Ok(())
    }

    pub fn set_subject(&mut self, key: RegisteryKey) -> Result<()> {
        if self.subject != self.undefined {
            self.ctx.with(|ctx| {
                let _ = ctx.deregister(self.subject);
            });
        }
        self.subject = key;
        Ok(())
    }

    pub fn get_module<'a, 'js>(
        &'a self,
        ctx: Ctx<'js>,
    ) -> anyhow::Result<QJsModule<'js, Evaluated>> {
        ctx.get_register(self.module)
            .ok_or_else(|| anyhow::anyhow!("no module found"))?
            .into_module()
            .ok_or_else(|| anyhow::anyhow!("not a module"))
    }

    pub fn get_subject<'a, 'js>(&'a self, ctx: Ctx<'js>) -> anyhow::Result<rquickjs::Object<'js>> {
        ctx.get_register(self.subject)
            .ok_or_else(|| anyhow::anyhow!("no subject found"))?
            .into_object()
            .ok_or_else(|| anyhow::anyhow!("not a subject"))
    }

    pub fn get_result<'a, 'js>(&'a self, ctx: Ctx<'js>) -> anyhow::Result<Value<'js>> {
        ctx.get_register(self.result)
            .ok_or_else(|| anyhow::anyhow!("no result found"))
    }
}

impl Drop for CoreWorld {
    fn drop(&mut self) {
        self.ctx.with(|ctx| {
            ctx.deregister(self.undefined);
            ctx.deregister(self.module);
            ctx.deregister(self.subject);
            ctx.deregister(self.result);
            cleanup_svelters(ctx).unwrap()
        })
    }
}
