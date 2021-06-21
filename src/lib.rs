#![recursion_limit = "1024"]
#![deny(clippy::all)]
use std::{ffi::CString, os::raw::c_char, time::Duration};

use anyhow::Result;
use rquickjs::{
    bind, BuiltinLoader, BuiltinResolver, Context, Ctx, FileResolver, Func, Function, IntoJs,
    Module, NativeLoader, Persistent, Result as JsResult, Runtime as JsRuntime, ScriptLoader,
};
use tokio::{task::LocalSet, time::sleep};

use svelters_compiler::{compile, SvelteCompiler};
use svelters_core::Framework;

pub mod logging;
pub mod runtime {
    pub mod console;
}

pub struct Runtime<T: Framework> {
    framework: T,
}

impl<T: Framework> Runtime<T> {
    pub fn new(framework: T) -> Self {
        Self { framework }
    }

    pub async fn start<P: AsRef<str>>(self, path: P) -> Result<()> {
        let path = path.as_ref().to_string();
        let local = LocalSet::new();
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

        let res: Result<()> = ctx.with(|ctx| {
            svelters_core::console::init(ctx)?;
            let global = ctx.globals();
            global.set("this", global.clone())?;
            global.init_def::<SetTimeout>()?;
            ctx.eval_global_module::<(), _>("./app/index.ts", Some(src.into_bytes()))
                .unwrap();
            Ok(())
        });
        ctx.runtime().execute_pending_job()?;
        Ok(res?)
    }
}

#[bind(object)]
#[quickjs(rename = "setTimeout")]
pub fn set_timeout<'js>(ctx: Ctx<'js>, func: Function<'js>, ms: u64) -> JsResult<()> {
    if ms > 0 {
        let func = Persistent::save(ctx, func);
        let context = Context::from_ctx(ctx)?;
        ctx.spawn(async move {
            sleep(Duration::from_millis(ms)).await;
            context
                .with(move |ctx| {
                    let func = func.restore(ctx).unwrap();
                    func.call::<_, ()>(())
                })
                .unwrap();
        });
    } else {
        func.call(())?;
    }
    Ok(())
}
