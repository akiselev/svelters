use rquickjs::{Context, Result};

use tracing_subscriber::prelude::*;

pub mod console;
pub mod logging;

pub fn initialize(ctx: &Context) -> Result<()> {
    // let layer = logging::TracingLayer::new();
    // let subscriber = tracing_subscriber::Registry::default()
    //     .with(tracing_subscriber::fmt::layer())
    //     .with(layer);
    // tracing_log::LogTracer::init().expect("Failed to set logger");
    // tracing::subscriber::set_global_default(subscriber).expect("Failed to set subscriber");

    Ok(())
}
