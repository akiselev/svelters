use std::fs::read_to_string;

use rquickjs::Value;
use svelters::Svelters;
use svelters_zig::{initialize_gtk, spawn_local, start, GtkExecutor};

#[tracing::instrument]
fn main() {
    svelters::logging::init();
    tracing::info!("Starting GTK...");
    let name = "org.svelters.gtk4.example";
    let sv = Svelters::new()
        .build()
        .expect("Could not start Svelters runtime");
    let app = sv
        .with_ctx(|ctx| initialize_gtk(ctx, name))
        .expect("Could not initialize GTK Application");

    // Connect to "activate" signal of `app`
    start(app, move |app| {
        let sv = sv.clone();
        tracing::info!("App::activate...");
        sv.with_ctx(|ctx| {
            let rt = ctx.runtime();
            rt.spawn_executor(GtkExecutor);
        });
        sv.with_js(|ctx| {
            let path = "./dist/index.js";
            let module = ctx
                .compile(path, read_to_string(path).unwrap())
                .expect("Module compile Failed");
            let start_fn: rquickjs::Function = module
                .get("default")
                .expect("Javascript entrypoint must export a default start function");
            if let Err(err) = start_fn.call::<(), Value>(()) {
                println!("ERROR: {}", err);
                std::process::exit(1);
            }
            let global = ctx.globals();
            global.set("module", module.clone()).unwrap();
            global.set("start", start_fn.clone()).unwrap();
        });
        sv.with_ctx(|ctx| {
            let rt = ctx.runtime();
            while rt.execute_pending_job().unwrap() {}
        });
        spawn_local(async move {
            sv.run().await.unwrap();
        });
    });

    tracing::info!("Finished...");
}
