#![feature(generic_associated_types)]
// Allows impl Trait in associated type
#![feature(type_alias_impl_trait)]
#![feature(const_impl_trait)]
// Features for evaluating expressions in const generics
// #![feature(const_evaluatable_checked)]
// Allows const str from utf8 bytes
// #![feature(const_str_from_utf8_unchecked)]
#![feature(const_fn_trait_bound)]
#![feature(const_type_name)]
#![feature(specialization)]
#![allow(incomplete_features)]
#![recursion_limit = "1024"]
#![deny(clippy::all)]
use std::{ffi::CString, os::raw::c_char, path::Path, time::Duration};

use rquickjs::{
    bind, BuiltinLoader, BuiltinResolver, Context, Ctx, ExecutorSpawner, FileResolver, FromJs,
    Func, Function, IntoAtom, IntoJs, ModuleLoader, NativeLoader, Object, Persistent,
    Result as JsResult, Runtime as JsRuntime, ScriptLoader, Value,
};

use svelters_compiler::{compile_sync, SvelteCompiler};
use svelters_core::prelude::*;

pub mod logging;

pub struct Svelters {
    rt: JsRuntime,
    ctx: Context,
    compiler: SvelteCompiler,
}

impl Svelters {
    pub fn new<MOD: Module>(module: MOD) -> Result<Self> {
        let rt = JsRuntime::new()?;
        let ctx = Context::full(&rt)?;
        let compiler = SvelteCompiler::new();
        let resolver = BuiltinResolver::default().with_module(MOD::MODULE_NAME);
        let loader = ModuleLoader::default().with_module(MOD::MODULE_NAME, module.loader());
        rt.set_loader((resolver, compiler.clone()), (loader, compiler.clone()));
        ctx.with(|ctx| {
            initialize_svelters(ctx)?;
            Result::<()>::Ok(())
        })?;
        Ok(Self { rt, ctx, compiler })
    }

    #[tracing::instrument(skip(self))]
    pub fn eval<P>(&self, path: P) -> Result<()>
    where
        P: std::fmt::Debug + AsRef<Path>,
    {
        let modpath = path.as_ref().to_owned();
        let src = compile_sync(&self.compiler, path)?;

        let res: Result<()> = self.ctx.with(|ctx| {
            let res = ctx.eval_global_module::<Value, _>(modpath, Some(src.into_bytes()));
            match res {
                Ok(ok) => {
                    tracing::info!("Success! Result: {:?}", ok);
                    Ok(())
                }
                Err(err) => {
                    tracing::error!("Failure! Result: {}", err);
                    Err(err)
                }
            }
        });
        res
    }

    pub fn execute_pending_job(&self) -> Result<bool> {
        self.ctx.runtime().execute_pending_job()
    }

    pub fn execute_pending_jobs(&self) -> Result<()> {
        while self.ctx.runtime().execute_pending_job()? {}
        Ok(())
    }

    pub fn spawn_executor<A: ExecutorSpawner>(&self, executor: A) -> A::JoinHandle {
        self.ctx.runtime().spawn_executor(executor)
    }

    pub fn set_global<K, V>(&self, key: K, value: V) -> Result<()>
    where
        K: for<'js> IntoAtom<'js>,
        V: for<'js> IntoJs<'js>,
    {
        self.ctx.with(move |ctx| ctx.globals().set(key, value))
    }

    pub fn with_js<FUNC, R>(&self, func: FUNC) -> R
    where
        FUNC: for<'js> FnOnce(Ctx<'js>) -> R,
        R: for<'js> FromJs<'js>,
    {
        self.ctx.with(move |ctx| func(ctx))
    }
}

impl Drop for Svelters {
    fn drop(&mut self) {
        while let Ok(true) = self.ctx.runtime().execute_pending_job() {}
        self.ctx.runtime().run_gc();
        self.ctx.with(|ctx| {
            if let Err(err) = cleanup_svelters(ctx) {
                tracing::error!("Error while cleaning up svelters: {}", err);
            }
        });
    }
}
