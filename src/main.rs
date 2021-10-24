use std::{future, time::Duration};

use svelters::svelters_eval_file;
use svelters_compiler::SvelteCompiler;
use svelters_core::prelude::Svelters;
// use svelters_gtk4::{initialize_gtk, spawn_local, Gtk};

#[tracing::instrument]
fn main() {
    svelters::logging::init();
    tracing::info!("Starting GTK...");
    // Gtk::start("org.svelters.gtk4.example", |app| {
    //     let compiler = SvelteCompiler::new();
    //     let sv = Svelters::new()
    //         .with_initializer(initialize_gtk(app.clone()))
    //         .build_with(compiler.clone(), compiler.clone())
    //         .expect("Could not start Svelters runtime");

    //     if let Err(err) = svelters_eval_file(&sv, &compiler, "./app/index.ts") {
    //         tracing::error!("Eval error: {}", err);
    //     }
    //     spawn_local(async move {
    //         let sv = sv;
    //         future::pending().await
    //     })
    // })
}
