use std::{future, time::Duration};

use svelters::svelters_eval_file;
use svelters_compiler::SvelteCompiler;
use svelters_core::prelude::Svelters;
use svelters_zig::{initialize_svgtk, run_app, spawn_local};

#[tracing::instrument]
fn main() {
    svelters::logging::init();
    tracing::info!("Starting GTK...");
    run_app("org.svelters.gtk4.example", |app| {
        let compiler = SvelteCompiler::new();
        let sv = Svelters::new()
            .with_initializer(initialize_svgtk)
            .build_with(compiler.clone(), compiler.clone())
            .expect("Could not start Svelters runtime");
        // Testing to make sure we've managed memory in svelters-zig correctly
        sv.gc();

        if let Err(err) = svelters_eval_file(&sv, &compiler, "./app/index.js") {
            tracing::error!("Eval error: {}", err);
        }
        spawn_local(async move {
            let _ = sv;
            future::pending().await
        })
    });
    tracing::info!("Finished...");
}
