use std::fs::read_to_string;

use gio::prelude::ApplicationExt;
use gtk4::Application;
use hotwatch::{Event, Hotwatch};
use rquickjs::{Ctx, Value};

use svelters::Svelters;
use svelters_zig::{initialize_gtk, spawn_local, start, GtkExecutor};

enum SvEvent {
    Updated(Event),
    Shutdown,
}

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
        if let Err(err) = sv.with_js(|ctx| initialize(ctx)) {
            tracing::error!("ERROR: {}", err)
            // std::process::exit(1);
        };
        let (tx, rx) = flume::bounded::<SvEvent>(10);
        let tx2 = tx.clone();
        app.connect_shutdown(move |app| {
            app.hold();
            tx2.send(SvEvent::Shutdown).unwrap();
        });
        sv.with_ctx(|ctx| {
            let rt = ctx.runtime();
            while rt.execute_pending_job().unwrap() {}
        });
        let app = app.clone();
        spawn_local(async move { run(app, sv, tx, rx).await });
    });

    tracing::info!("Finished...");
}

async fn run(
    app: Application,
    sv: Svelters,
    tx: flume::Sender<SvEvent>,
    rx: flume::Receiver<SvEvent>,
) {
    let path = "./dist/index.js";

    let mut hotwatch = Hotwatch::new().expect("hotwatch failed to initialize!");
    hotwatch
        .watch(path, move |event: Event| {
            if let Err(error) = tx.send(SvEvent::Updated(event)) {
                tracing::error!("Failed sending file update event to main thread: {}", error);
            }
        })
        .expect("failed to watch file!");

    while let Ok(event) = rx.recv_async().await {
        match event {
            SvEvent::Updated(event) => {
                if let Event::Write(update_path) = event {
                    tracing::info!("File {:?} updated", update_path);
                    if let Err(error) = reload(&sv) {
                        tracing::error!("Reload error: {}", error);
                    };
                }
            }
            SvEvent::Shutdown => {
                sv.with_js(|ctx| {
                    ctx.eval::<(), _>("document.cleanup()").unwrap();
                });
                break;
            }
        }
    }
    app.release();
    tracing::info!("Shutting down...");
}

fn reload<'js>(sv: &Svelters) -> rquickjs::Result<()> {
    let path = "./dist/index.js";
    sv.with_js(|ctx| {
        let module = ctx
            .compile(
                path,
                read_to_string(path).expect("Module read for reload failed"),
            )
            .expect("Module compile Failed");
        let start_fn: rquickjs::Function = module
            .get("default")
            .expect("Javascript entrypoint must export a default start function");
        if let Err(err) = start_fn.call::<(bool,), Value>((true,)) {
            tracing::error!("ERROR: {}", err);
            // std::process::exit(1);
        }
        let global = ctx.globals();
        global.set("module", module.clone()).unwrap();
        global.set("start", start_fn.clone()).unwrap();
        rquickjs::Result::Ok(())
    })?;
    sv.with_ctx(|ctx| {
        ctx.runtime().run_gc();
    });
    Ok(())
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
