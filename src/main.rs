use std::{ffi::CString, time::Duration};

use anyhow::Result;

use quickjs_runtime;
use rquickjs::{
    qjs, Async, BuiltinLoader, BuiltinResolver, Context, Ctx, FileResolver, Func, Function, Loaded,
    Module, NativeLoader, Persistent, Runtime, Script, ScriptLoader, Value,
};
use svelters::{
    compile,
    runtime::{loader::Loader, resolver::Resolver},
};
use tokio::{
    task::{self, JoinHandle},
    time::sleep,
};

#[tokio::main]
async fn main() -> Result<()> {
    let resolver = Resolver::new();
    let loader = Loader::new();

    let local = task::LocalSet::new();
    let rt = Runtime::new()?;
    rt.set_loader(resolver, loader);
    let ctx = Context::full(&rt)?;

    // Run the local task set.
    local.spawn_local(async move { rt.spawn_executor(rquickjs::Tokio) });

    let res: Result<()> = ctx.with(|ctx| {
        let global = ctx.globals();
        // let module = Module::new(ctx, "root", res)?;
        // let module = module.eval()?;
        Ok(())
    });
    Ok(res?)
}
