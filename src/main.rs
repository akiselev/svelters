use std::{sync::Arc, time::Duration};

use anyhow::Result;

use rquickjs::{
    bind, BuiltinLoader, BuiltinResolver, Context, Ctx, FileResolver, Func, IntoJs, Module,
    NativeLoader, Persistent, Result as JsResult, Runtime, ScriptLoader,
};
use rquickjs::{Function, OnceFn};
use tokio::task::LocalSet;
use tokio::{
    task::{self, JoinHandle},
    time::sleep,
};
use tracing::subscriber::set_global_default;
use tracing_log::LogTracer;
use tracing_subscriber::{fmt, layer::SubscriberExt, EnvFilter, Registry};

use svelters_compiler::{compile, SvelteCompiler};

#[tokio::main]
async fn main() -> Result<()> {
    svelters::logging::init();
    tracing::info!("Starting...");

    // Run the local task set.
    let local = LocalSet::new();
    let _ = local.spawn_local(main_task());
    local.await;
    Ok(())
}

#[tracing::instrument()]
async fn main_task() -> Result<()> {
    let compiler = SvelteCompiler::new();
    let src = compile(&compiler, "./src/ts/index.ts").await?;

    let rt = Runtime::new()?;
    rt.spawn_executor(rquickjs::Tokio);
    rt.set_loader(compiler.clone(), compiler.clone());
    let ctx = Context::full(&rt)?;

    let res: Result<()> = ctx.with(|ctx| {
        svelters::runtime::console::init(ctx)?;
        let global = ctx.globals();
        global.set("this", global.clone())?;
        global.init_def::<SetTimeout>()?;
        ctx.eval_global_module::<(), _>("./src/ts/index.ts", Some(src.into_bytes()))
            .unwrap();
        Ok(())
    });
    ctx.runtime().execute_pending_job()?;
    tokio::time::sleep(Duration::from_millis(10000)).await;
    Ok(res?)
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
