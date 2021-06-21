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
    let rt = svelters::Runtime::new(svelters_qt::Qt::new());
    rt.start("./app/index.ts").await?;
    Ok(())
}
