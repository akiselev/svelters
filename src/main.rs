use std::fs::read_to_string;

use gio::prelude::ApplicationExt;
use gtk4::Application;
use hotwatch::{Event, Hotwatch};
use rquickjs::{Ctx, Value};

use svelters::Svelters;
use svelters_gtk::*;

#[tracing::instrument]
fn main() {
    svelters::logging::init();
    tracing::info!("Starting GTK...");
    let name = "org.svelters.gtk4.example";
    let sv = Svelters::new()
        .build()
        .expect("Could not start Svelters runtime");
    let app =
        sv.with_ctx(|ctx| initialize_gtk(ctx, name).expect("Could not initialize GTK Application"));

    // Connect to "activate" signal of `app`
    start(app, move |app| {
        let sv = sv.clone();
        tracing::info!("App::activate...");
        sv.with_ctx(|ctx| {
            let rt = ctx.runtime();
            rt.spawn_executor(GtkExecutor);
        });
        if let Err(err) = sv.with_js(|ctx| initialize(ctx)) {
            tracing::error!("ERROR: {}", err)
            // std::process::exit(1);
        };
        app.connect_shutdown(move |app| {
            app.hold();
            Svelters::handle().shutdown().unwrap();
        });
        sv.with_ctx(|ctx| {
            let rt = ctx.runtime();
            while rt.execute_pending_job().unwrap() {}
            rt.run_gc();
        });
        let app = app.clone();
        run(app, sv);
    });

    tracing::info!("Finished...");
}

fn run(app: Application, sv: Svelters) {
    let path = "./dist/index.js";

    let mut hotwatch = Hotwatch::new().expect("hotwatch failed to initialize!");
    let handle = Svelters::handle();
    hotwatch
        .watch(path, move |event: Event| {
            if let Err(error) = handle.updated(event) {
                tracing::error!("Failed sending file update event to main thread: {}", error);
            }
        })
        .expect("failed to watch file!");

    spawn_local(async move {
        if let Err(err) = sv.run().await {
            tracing::error!("Svelters error: {}", err);
        };
        app.release();
        tracing::info!("Shutting down...");
    });
}

fn initialize<'js>(ctx: Ctx<'js>) -> rquickjs::Result<()> {
    let path = "./dist/index.js";
    let module = ctx
        .compile(path, read_to_string(path).unwrap())
        .expect("Module compile Failed");
    let start_fn: rquickjs::Function = module
        .get("default")
        .expect("Javascript entrypoint must export a default start function");
    if let Err(err) = start_fn.call::<(bool,), Value>((false,)) {
        tracing::error!("ERROR: {}", err);
        // std::process::exit(1);
    }
    let global = ctx.globals();
    global.set("module", module.clone()).unwrap();
    global.set("start", start_fn.clone()).unwrap();
    Ok(())
}
