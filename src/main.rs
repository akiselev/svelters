use std::{sync::Arc, time::Duration};

use anyhow::Result;

use rquickjs::{
    BuiltinLoader, BuiltinResolver, Context, Ctx, FileResolver, Func, Function, IntoJs, Module,
    NativeLoader, Persistent, Runtime, ScriptLoader,
};
use svelters::compile;
use tokio::{
    task::{self, JoinHandle},
    time::sleep,
};

#[tokio::main]
async fn main() -> Result<()> {
    let res = compile()?;
    std::fs::write("./dist/bundle.js", &res)?;

    let resolver = (
        BuiltinResolver::default()
            .with_module("svelte/internal")
            .with_module("./internal/index.mjs")
            .with_module("svelte"),
        FileResolver::default()
            .with_path("./")
            .with_path("./node_modules/svelte/"),
    );
    let loader = (
        BuiltinLoader::default()
            .with_module(
                "svelte/internal",
                std::fs::read_to_string("./node_modules/svelte/internal/index.mjs")?,
            )
            .with_module(
                "./internal/index.mjs",
                std::fs::read_to_string("./node_modules/svelte/internal/index.mjs")?,
            )
            .with_module(
                "svelte",
                std::fs::read_to_string("./node_modules/svelte/index.mjs")?,
            ),
        ScriptLoader::default(),
        NativeLoader::default(),
    );

    let local = task::LocalSet::new();
    let rt = Runtime::new()?;
    rt.set_loader(resolver, loader);
    let ctx = Context::full(&rt)?;

    // Run the local task set.
    local.spawn_local(async move { rt.spawn_executor(rquickjs::Tokio) });

    let res: Result<()> = ctx.with(|ctx| {
        let global = ctx.globals();
        global.set("setTimeout", Func::from())?;
        let module = Module::new(ctx, "root", res)?;
        let module = module.eval()?;
        Ok(())
    });
    Ok(res?)
}

fn set_timeout<'a, 'b>(ctx: &Ctx<'a>) -> impl IntoJs<'a> {
    return move |ctx: Ctx<'a>, ms: u64, func: Function<'b>| {
        let func: Arc<Persistent<Function>> = Arc::new(Persistent::save(ctx, func));

        ctx.spawn(async move {
            if ms > 0 {
                sleep(Duration::from_millis(ms)).await;
            }
            let func = func.restore(ctx).unwrap();
            func.call(()).unwrap()
        });
    };
}
