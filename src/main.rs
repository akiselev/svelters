use std::{sync::Arc, time::Duration};

use anyhow::Result;

use rquickjs::{
    bind, BuiltinLoader, BuiltinResolver, Context, Ctx, FileResolver, Func, IntoJs, Module,
    NativeLoader, Persistent, Result as JsResult, Runtime, ScriptLoader,
};
use rquickjs::{Function, OnceFn};
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
    init_logging();
    tracing::info!("Starting...");
    let compiler = SvelteCompiler::new();
    println!("{}", compile(&compiler, "./src/ts/App.svelte").await?);
    Ok(())
    // let res = compile()?;
    // std::fs::write("./dist/bundle.js", &res)?;

    // let resolver = (
    //     BuiltinResolver::default()
    //         .with_module("svelte/internal")
    //         .with_module("./internal/index.mjs")
    //         .with_module("svelte"),
    //     FileResolver::default()
    //         .with_path("./")
    //         .with_path("./node_modules/svelte/"),
    // );
    // let loader = (
    //     BuiltinLoader::default()
    //         .with_module(
    //             "svelte/internal",
    //             std::fs::read_to_string("./node_modules/svelte/internal/index.mjs")?,
    //         )
    //         .with_module(
    //             "./internal/index.mjs",
    //             std::fs::read_to_string("./node_modules/svelte/internal/index.mjs")?,
    //         )
    //         .with_module(
    //             "svelte",
    //             std::fs::read_to_string("./node_modules/svelte/index.mjs")?,
    //         ),
    //     ScriptLoader::default(),
    //     NativeLoader::default(),
    // );

    // let local = task::LocalSet::new();
    // let rt = Runtime::new()?;
    // rt.set_loader(resolver, loader);
    // let ctx = Context::full(&rt)?;

    // // Run the local task set.
    // local.spawn_local(async move { rt.spawn_executor(rquickjs::Tokio) });

    // let res: Result<()> = ctx.with(|ctx| {
    //     let global = ctx.globals();
    //     global.init_def::<SetTimeout>()?;
    //     let module = Module::new(ctx, "root", res)?;
    //     let module = module.eval()?;
    //     Ok(())
    // });
    // Ok(res?)
}

#[bind(object)]
pub fn set_timeout<'js>(ctx: Ctx<'js>, ms: u64, func: Function<'js>) -> JsResult<()> {
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

fn init_logging() {
    // qmetaobject::log::init_qt_to_rust();

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let fmt_layer = fmt::layer().with_target(false);
    let subscriber = Registry::default().with(env_filter).with(fmt_layer);
    LogTracer::init().expect("Failed to set logger");
    set_global_default(subscriber).expect("Failed to set subscriber");
}
