//! Runs a svelters app in skelter windows:
//!
//! ```text
//! svelters-skelter-run [--bundle] <dist/app.js>          # a bundle built for skelter
//! svelters-skelter-run --entry <app dir | file> [--dev]  # from source, compiled live for skelter
//! ```
//!
//! The app mounts its `<window>`s itself; each one it presents opens a platform window (winit) drawn
//! by skelter's compositor (wgpu). Text uses skelter-text over the system fonts.

use std::sync::Arc;
use svelters_skelter::app::SveltersApp;
use svelters_skelter::embed::{Embed, EmbedConfig};
use svelters_skelter::SkelterBackend;

const USAGE: &str = "usage: svelters-skelter-run ([--bundle] <dist/app.js> | --entry <app dir|file> [--dev])";

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
    let fonts = skelter_text::FontStore::new(skelter_text::FontConfig::default());
    let backend = SkelterBackend::with_text(svelters_skelter::builtin_kinds(), Box::new(skelter_text::TextSystem::new(fonts.clone())));
    svelters_skelter::app::install_widget_services(&backend, skelter_text::TextSystem::new(fonts.clone()));

    let host = svelters_host::HostConfig::default();
    let embed = Embed::with_backend(backend, EmbedConfig::app(host)).unwrap_or_else(|e| fail(e.to_string()));
    let loaded = match load {
        Load::Bundle(bundle) => embed.load_app(&bundle),
        Load::Entry(entry) => {
            let mut cfg = svelters_host::modules::SourceConfig::for_path(&entry).unwrap_or_else(|e| fail(e));
            cfg.backend = "skelter".into();
            cfg.dev = dev;
            let provider = svelters_host::modules::SourceProvider::new(cfg).unwrap_or_else(|e| fail(e));
            embed.load_app_from(std::rc::Rc::new(provider))
        }
    };
    if let Err(e) = loaded {
        fail(e.to_string());
    }

    let glyphs: Arc<dyn skelter_base::text::GlyphRasterizer> = Arc::new(skelter_text::FontRasterizer::new(fonts));
    let spawn = skelter::compositor(|clock| skelter_compositor::CompositorConfig::new(clock, skelter_base::time::Span::from_micros(16_667)), glyphs);
    if let Err(e) = skelter_platform::run(Box::new(SveltersApp::new(embed, spawn))) {
        fail(format!("svelters-skelter-run: {e}"));
    }
}
