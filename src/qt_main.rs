use std::{cell::RefCell, fs::read_to_string, rc::Rc};

use gio::prelude::ApplicationExtManual;
use glib::Cast;
use gtk4::{prelude::ApplicationExt, Application};
use hotwatch::{Event, Hotwatch};
use rquickjs::{Ctx, Value};

use svelters_qt::*;

static PATH: &str = "./dist/index.js";

#[tracing::instrument]
fn main() {
    // svelters::logging::init();
    // {
    //     tracing::info!("Starting our Application...");

    //     let name = "org.svelters.gtk4.example";
    //     let app = SveltersApplication::new(name);

    //     // If we put it in connect_activate, it will get dropped so we alloc here
    //     let hotwatch = Rc::new(RefCell::new(
    //         Hotwatch::new().expect("hotwatch failed to initialize!"),
    //     ));

    //     app.connect_activate(move |app| {
    //         tracing::info!("App::activate...");
    //         let app = app.downcast_ref::<SveltersApplication>().unwrap();

    //         app.with(|ctx| initialize(ctx, PATH, false)).unwrap();
    //         let handle = app.handle();
    //         hotwatch
    //             .borrow_mut()
    //             .watch(PATH, move |event: Event| {
    //                 println!("{} updated", PATH);
    //                 if let Event::Write(_) = event {
    //                     if let Err(error) = handle.with(|ctx| initialize(ctx, PATH, true)) {
    //                         tracing::error!(
    //                             "Failed sending file update event to main thread: {}",
    //                             error
    //                         );
    //                     }
    //                 }
    //             })
    //             .expect("failed to watch file!");
    //     });

    //     app.run();

    //     tracing::info!("Cleaning up app...");
    //     app.cleanup();
    // }

    tracing::info!("Finished...");
}

fn initialize(ctx: Ctx, path: &str, reload: bool) -> rquickjs::Result<()> {
    // let module = ctx
    //     .compile(path, read_to_string(path).unwrap())
    //     .expect("Module compile Failed");
    // let start_fn: rquickjs::Function = module
    //     .get("default")
    //     .expect("Javascript entrypoint must export a default start function");
    // if let Err(err) = start_fn.call::<(bool,), Value>((reload,)) {
    //     tracing::error!("ERROR: {}", err);
    //     // std::process::exit(1);
    // }
    // let global = ctx.globals();
    // global.set("module", module.clone()).unwrap();
    // global.set("start", start_fn.clone()).unwrap();
    Ok(())
}
