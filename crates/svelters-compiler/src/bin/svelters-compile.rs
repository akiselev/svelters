//! `svelters-compile --backend gtk|qt [--spec path.json] [--filename name] [--json] <file.svelte | ->`
//!
//! Prints the generated JS (or, with `--json`, `{js, map, diagnostics, backend, timings}`).
//! Diagnostics go to stderr; exit status 1 when there are errors.

use std::io::Read;
use svelters_compiler::compile::{CompileOptions, compile};
use svelters_compiler::semantics::SpecBackend;

fn main() {
    let mut backend = "gtk".to_string();
    let mut spec: Option<String> = None;
    let mut filename: Option<String> = None;
    let mut json = false;
    let mut input: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--backend" => backend = args.next().expect("--backend needs a value"),
            "--spec" => spec = args.next(),
            "--filename" => filename = args.next(),
            "--json" => json = true,
            "-h" | "--help" => {
                eprintln!("usage: svelters-compile --backend gtk|qt [--spec spec.json] [--filename name] [--json] <file.svelte | ->");
                return;
            }
            _ => input = Some(a),
        }
    }
    let input = input.unwrap_or_else(|| "-".into());
    let src = if input == "-" {
        let mut s = String::new();
        std::io::stdin().read_to_string(&mut s).expect("read stdin");
        s
    } else {
        std::fs::read_to_string(&input).unwrap_or_else(|e| panic!("{input}: {e}"))
    };
    let sem = match &spec {
        Some(p) => SpecBackend::from_path(p),
        None => SpecBackend::builtin(&backend),
    }
    .unwrap_or_else(|e| {
        eprintln!("svelters-compile: {e}");
        std::process::exit(2)
    });
    let filename = filename.unwrap_or_else(|| if input == "-" { "Component.svelte".into() } else { input.clone() });
    let res = compile(&src, &CompileOptions { filename: filename.clone(), backend: &sem });
    for d in &res.diagnostics {
        eprintln!("{}", d.render(&filename));
    }
    if json {
        println!("{}", serde_json::to_string(&res).expect("result serializes"));
    } else if let Some(js) = &res.js {
        print!("{js}");
    }
    if res.has_errors() {
        std::process::exit(1);
    }
}
