#![recursion_limit = "1024"]
#![deny(clippy::all)]
use std::{ffi::CString, os::raw::c_char, time::Duration};

use anyhow::Result;
use rquickjs::{
    bind, BuiltinLoader, BuiltinResolver, Context, Ctx, FileResolver, Func, Function, IntoJs,
    Module, NativeLoader, Persistent, Result as JsResult, Runtime as JsRuntime, ScriptLoader,
};

use svelters_compiler::{compile, SvelteCompiler};
use svelters_core::{Application, Framework};

pub mod logging;

pub struct Runtime<T: Framework> {
    framework: T,
}

impl<T: Framework> Runtime<T> {
    pub fn new(framework: T) -> Self {
        Self { framework }
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

        let rt = JsRuntime::new()?;
        rt.spawn_executor(rquickjs::Tokio);
        rt.set_loader(compiler.clone(), compiler.clone());
        let ctx = Context::full(&rt)?;

        let res: Result<_> = ctx.with(|ctx| {
            svelters_core::console::init(ctx)?;
            svelters_core::timing::init(ctx)?;
            let app = self.framework.init_app(ctx)?;
            let app = tokio::task::spawn_local(app.start());
            ctx.eval_global_module::<(), _>("./app/index.ts", Some(src.into_bytes()))
                .unwrap();
            Ok(app)
        });
        ctx.runtime().execute_pending_job()?;
        Ok(res?.await??)
    }
}
