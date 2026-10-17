//! `svelters-compile --backend gtk|qt|appkit|winui [--spec path.json] [--filename name] [--dev] [--module] [--css-scope class] [--json] <file | ->`
//!
//! Compiles a component (or, with `--module` or a `.svelte.js` input, a module with runes).
//! Prints the generated JS (or, with `--json`, `{js, map, diagnostics, backend, timings}`).
//! Diagnostics go to stderr; exit status 1 when there are errors.

use std::io::Read;
use svelters_compiler::compile::{CompileOptions, ModuleOptions, compile, compile_module};
use svelters_compiler::semantics::SpecBackend;

const USAGE: &str = "usage: svelters-compile --backend gtk|qt|appkit|winui [--spec spec.json] [--filename name] [--dev] [--module] [--css-scope class] [--json] <file.svelte | file.svelte.js | ->";

fn main() {
    let mut backend = "gtk".to_string();
    let mut spec: Option<String> = None;
    let mut filename: Option<String> = None;
    let mut json = false;
    let mut dev = false;
    let mut module = false;
    let mut css_scope: Option<String> = None;
    let mut input: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--backend" => backend = args.next().expect("--backend needs a value"),
            "--spec" => spec = args.next(),
            "--filename" => filename = args.next(),
            "--json" => json = true,
            "--dev" => dev = true,
            "--module" => module = true,
            "--css-scope" => css_scope = args.next(),
            "-h" | "--help" => {
                eprintln!("{USAGE}");
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
    let filename = filename.unwrap_or_else(|| if input == "-" { if module { "module.svelte.js".into() } else { "Component.svelte".into() } } else { input.clone() });
    let is_module = module || filename.ends_with(".svelte.js");
    let res = if is_module {
        compile_module(&src, &ModuleOptions { filename: filename.clone(), dev })
    } else {
        let sem = match &spec {
            Some(p) => SpecBackend::from_path(p),
            None => SpecBackend::builtin(&backend),
        }
        .unwrap_or_else(|e| {
            eprintln!("svelters-compile: {e}");
            std::process::exit(2)
        });
        compile(&src, &CompileOptions { filename: filename.clone(), backend: &sem, dev, css_scope })
    };
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
