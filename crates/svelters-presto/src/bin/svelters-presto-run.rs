//! Runs a svelters app in presto windows:
//!
//! ```text
//! svelters-presto-run [--bundle] <dist/app.js>          # a bundle built for presto
//! svelters-presto-run --entry <app dir | file> [--dev]  # from source, compiled live for presto
//! ```
//!
//! The app mounts its `<window>`s itself; each one it presents opens a platform window (winit) drawn
//! by presto's compositor (wgpu). Text uses presto-text over the system fonts.

use std::sync::Arc;
use svelters_presto::app::SveltersApp;
use svelters_presto::embed::{Embed, EmbedConfig};
use svelters_presto::PrestoBackend;

const USAGE: &str = "usage: svelters-presto-run ([--bundle] <dist/app.js> | --entry <app dir|file> [--dev])";

enum Load {
    Bundle(std::path::PathBuf),
    Entry(std::path::PathBuf),
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (mut load, mut dev) = (None, false);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--bundle" => load = it.next().map(|p| Load::Bundle(p.into())),
            "--entry" => load = it.next().map(|p| Load::Entry(p.into())),
            "--dev" => dev = true,
            "-h" | "--help" => {
                eprintln!("{USAGE}");
                std::process::exit(0);
            }
            _ => load = Some(Load::Bundle(a.into())),
        }
    }
    let Some(load) = load else {
        eprintln!("{USAGE}");
        std::process::exit(2);
    };
    let fail = |e: String| -> ! {
        eprintln!("{e}");
        std::process::exit(1)
    };

    // text: one font store for measuring (the tree), shaping (the kinds) and rasterising (the GPU)
    let fonts = presto_text::FontStore::new(presto_text::FontConfig::default());
    let backend = PrestoBackend::with_text(svelters_presto::builtin_kinds(), Box::new(presto_text::TextSystem::new(fonts.clone())));
    svelters_presto::app::install_widget_services(&backend, presto_text::TextSystem::new(fonts.clone()));

    let host = svelters_host::HostConfig::default();
    let embed = Embed::with_backend(backend, EmbedConfig::app(host)).unwrap_or_else(|e| fail(e.to_string()));
    let loaded = match load {
        Load::Bundle(bundle) => embed.load_app(&bundle),
        Load::Entry(entry) => {
            let mut cfg = svelters_host::modules::SourceConfig::for_path(&entry).unwrap_or_else(|e| fail(e));
            cfg.backend = "presto".into();
            cfg.dev = dev;
            let provider = svelters_host::modules::SourceProvider::new(cfg).unwrap_or_else(|e| fail(e));
            embed.load_app_from(std::rc::Rc::new(provider))
        }
    };
    if let Err(e) = loaded {
        fail(e.to_string());
    }

    let glyphs: Arc<dyn presto_base::text::GlyphRasterizer> = Arc::new(presto_text::FontRasterizer::new(fonts));
    let spawn = presto::compositor(|clock| presto_compositor::CompositorConfig::new(clock, presto_base::time::Span::from_micros(16_667)), glyphs);
    if let Err(e) = presto_platform::run(Box::new(SveltersApp::new(embed, spawn))) {
        fail(format!("svelters-presto-run: {e}"));
    }
}
