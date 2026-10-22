//! Builds, inspects and precompiles `.svb` release bundles (Q4b, ASSETS.md §4):
//!
//! ```text
//! svelters-bundle build <app dir | file> -o <app.svb> [--no-sources] [--backend <name>] [--precompile]
//! svelters-bundle precompile <app.svb> -o <out.svb>
//! svelters-bundle ls <app.svb>
//! svelters-bundle cat <app.svb> <module id | file path> [--map]
//! ```
//!
//! `build` compiles the app for production (`svelters_modules::svb::build`; the backend defaults to
//! egui). `precompile` adds bytecode for this binary's engine (`svelters_host::bytecode::precompile`),
//! which `svelters-egui-run --app` of the same build reads instead of compiling the JS; another
//! engine ignores it and compiles. `ls` prints the index summary, the modules and the files; `cat`
//! prints a module's JS (or its source map) or a file's bytes.

// A command-line tool, not the UI thread: the ZU3 lint does not apply.
#![allow(clippy::disallowed_methods)]

use std::io::Write;
use std::path::PathBuf;
use svelters_host::modules::svb::{self, BuildOptions, Svb};

const USAGE: &str = "usage:
  svelters-bundle build <app dir|file> -o <app.svb> [--no-sources] [--backend <name>] [--precompile]
  svelters-bundle precompile <app.svb> -o <out.svb>
  svelters-bundle ls <app.svb>
  svelters-bundle cat <app.svb> <module id|file path> [--map]";

fn fail(e: impl std::fmt::Display) -> ! {
    eprintln!("svelters-bundle: {e}");
    std::process::exit(1)
}

fn usage() -> ! {
    eprintln!("{USAGE}");
    std::process::exit(2)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (mut pos, mut out, mut sources, mut backend, mut precompile, mut map) = (Vec::new(), None, true, "egui".to_string(), false, false);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-o" | "--out" => out = Some(PathBuf::from(it.next().unwrap_or_else(|| usage()))),
            "--no-sources" => sources = false,
            "--backend" => backend = it.next().unwrap_or_else(|| usage()).clone(),
            "--precompile" => precompile = true,
            "--map" => map = true,
            "-h" | "--help" => {
                println!("{USAGE}");
                return;
            }
            _ if a.starts_with('-') => usage(),
            _ => pos.push(a.as_str()),
        }
    }
    let write = |bytes: &[u8]| {
        let out = out.as_ref().unwrap_or_else(|| usage());
        std::fs::write(out, bytes).unwrap_or_else(|e| fail(format!("{}: {e}", out.display())));
        let svb = Svb::from_bytes(bytes.to_vec()).unwrap_or_else(|e| fail(e));
        eprintln!("{}: {} B, {} modules, build {}", out.display(), bytes.len(), svb.index().modules.len(), svb.index().build);
    };
    let open = |p: &str| Svb::open(std::path::Path::new(p)).unwrap_or_else(|e| fail(format!("{p}: {e}")));
    match pos.as_slice() {
        ["build", app] => {
            let mut cfg = svelters_host::modules::SourceConfig::for_path(std::path::Path::new(app)).unwrap_or_else(|e| fail(e));
            cfg.backend = backend;
            let bytes = svb::build(cfg, &BuildOptions { sources, ..BuildOptions::default() }).unwrap_or_else(|e| fail(e));
            let bytes = if precompile {
                let svb = Svb::from_bytes(bytes).unwrap_or_else(|e| fail(e));
                svelters_host::bytecode::precompile(&svb).unwrap_or_else(|e| fail(e))
            } else {
                bytes
            };
            write(&bytes);
        }
        ["precompile", file] => write(&svelters_host::bytecode::precompile(&open(file)).unwrap_or_else(|e| fail(e))),
        ["ls", file] => {
            let svb = open(file);
            let ix = svb.index();
            println!("kind {:?}, root {}, version {}, build {}", ix.kind, ix.root, ix.version.as_deref().unwrap_or("-"), ix.build);
            println!("compiler {}, runtime {}", ix.stamp.compiler, ix.stamp.runtime.as_deref().unwrap_or("-"));
            println!("bytecode for: {}", ix.stamp.engine_build.as_deref().unwrap_or("none"));
            println!("entry {}", ix.entry);
            println!("\nmodules ({}):", ix.modules.len());
            for (id, m) in &ix.modules {
                let len = |b: Option<usize>| b.map_or("-".to_string(), |b| svb.blob(b).len().to_string());
                println!("  {id}  js {} map {} bytecode {}", svb.blob(m.js).len(), len(m.map), len(m.bytecode));
            }
            println!("\nfiles ({}):", ix.files.len());
            for (path, f) in &ix.files {
                println!("  {path}  {:?} {} B{}", f.role, svb.blob(f.blob).len(), f.mime.as_deref().map(|m| format!(" {m}")).unwrap_or_default());
            }
            for (name, s) in &ix.sections {
                println!("section {name} v{} {} B{}", s.version, svb.blob(s.blob).len(), if s.required { " required" } else { "" });
            }
        }
        ["cat", file, what] => {
            let svb = open(file);
            let bytes = match svb.index().modules.get(*what) {
                Some(m) if map => m.map.map(|b| svb.blob(b)).unwrap_or_else(|| fail(format!("{what}: no source map"))),
                Some(m) => svb.blob(m.js),
                None => svb.file(what).unwrap_or_else(|| fail(format!("{what}: no such module or file"))),
            };
            std::io::stdout().write_all(bytes).unwrap_or_else(|e| fail(e));
        }
        _ => usage(),
    }
}
