#![feature(generic_associated_types)]
// Allows impl Trait in associated type
#![feature(min_type_alias_impl_trait)]
//
#![feature(const_impl_trait)]
// Features for evaluating expressions in const generics
// #![feature(const_evaluatable_checked)]
// Allows const str from utf8 bytes
// #![feature(const_str_from_utf8_unchecked)]
#![feature(const_generics)]
#![feature(const_fn_trait_bound)]
#![feature(const_type_name)]
#![feature(specialization)]
#![feature(impl_trait_in_bindings)]
#![allow(incomplete_features)]
#![recursion_limit = "1024"]
#![deny(clippy::all)]
use std::{ffi::CString, os::raw::c_char, time::Duration};

use anyhow::Result;
use rquickjs::{
    bind, BuiltinLoader, BuiltinResolver, Context, Ctx, FileResolver, Func, Function, IntoJs,
    Module, NativeLoader, Persistent, Result as JsResult, Runtime as JsRuntime, ScriptLoader,
};

use svelters_compiler::{compile, SvelteCompiler};
use svelters_core::prelude::*;

pub mod logging;

pub struct Runtime<FW: Framework> {
    framework: FW,
    rt: JsRuntime,
    ctx: Context,
}

impl<FW: Framework> Runtime<FW> {
    pub fn new(framework: FW) -> Result<Self> {
        let rt = JsRuntime::new()?;
        let ctx = Context::full(&rt)?;
        ctx.with(|ctx| {
            svelters_core::console::init(ctx)?;
            svelters_core::timing::init(ctx)?;
            Result::<()>::Ok(())
        })?;
        Ok(Self { rt, ctx, framework })
    }

    pub async fn start<P: AsRef<str>>(self, path: P) -> Result<()> {
        let path = path.as_ref().to_string();
        let local = tokio::task::LocalSet::new();
        let handle = local.spawn_local(self.main_task(path));
        local.await;
        Ok(handle.await??)
    }

    #[tracing::instrument(skip(self))]
    async fn main_task(self, path: String) -> Result<()> {
        let compiler = SvelteCompiler::new();
        let src = compile(&compiler, "./app/index.ts").await?;

        self.rt.spawn_executor(rquickjs::Tokio);
        let (resolver, loader) = FW::get_loader();
        self.rt
            .set_loader((resolver, compiler.clone()), (loader, compiler.clone()));

        let res: Result<_> = self.ctx.with(|ctx| {
            let app = FW::init_app(self.ctx.clone())?;
            let app = tokio::task::spawn_local(app);
            ctx.eval_global_module::<(), _>("./app/index.ts", Some(src.into_bytes()))
                .unwrap();
            // Ok(())
            Ok(app)
        });
        while let Ok(true) = self.ctx.runtime().execute_pending_job() {}
        Ok(res?.await??)
    }
}
