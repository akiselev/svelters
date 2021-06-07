use tracing::subscriber::set_global_default;
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_log::LogTracer;
use tracing_subscriber::{fmt, layer::SubscriberExt, EnvFilter, Registry};

pub fn init() {
    qmetaobject::log::init_qt_to_rust();

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    // let formatting_filter = BunyanFormattingLayer::new("svelte-qt".to_string(), std::io::stdout);
    let fmt_layer = fmt::layer().with_target(false);
    let subscriber = Registry::default()
        .with(env_filter)
        .with(JsonStorageLayer)
        .with(fmt_layer);
    // .with(formatting_filter);
    LogTracer::init().expect("Failed to set logger");
    set_global_default(subscriber).expect("Failed to set subscriber");
}
