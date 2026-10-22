//! `svelters-compile --backend gtk|qt|appkit|winui|egui[,...] [--spec path.json] [--spec-fragment kinds.json]... [--target quickjs-host|node-vite] [--strict-native] [--filename name] [--dev] [--module] [--css-scope class] [--json] <file | ->`
//!
//! Compiles a component (or, with `--module` or a `.svelte.js` input, a module with runes).
//! `--backend gtk,qt,appkit,winui` is the portability mode: the output is for the first platform, the
//! others add `observability_differs` warnings. `--strict-native` makes the observability warnings errors.
//! Prints the generated JS (or, with `--json`, `{js, map, diagnostics, backend, timings}`).
//! Diagnostics go to stderr; exit status 1 when there are errors.
//! `--kinds` prints the runtime spec tables of the (first) backend instead (`@svelters/native-runtime/kinds`,
//! the module spreads and `<svelte:element>` resolve attributes with at run time).
//! `--spec-fragment` (repeatable, RA1) merges a fragment (`kinds` and `tags`, e.g. an egui app's
//! dumped `kinds.json`) into the first backend's spec; `--spec` still replaces the spec.
//!
//! CX1, for tools: `--check` reports diagnostics without generating code (`svelters_compiler::check`);
//! with `--json` it prints `{path, kind, diagnostics, reuse, timings}`, each diagnostic with its
//! namespaced `id`, byte and line/column `range` and `fixes` (text edits). `--explain <code or id>`
//! prints a code's registry row; `--list-codes` prints `id code severity` per registered code.

use std::io::Read;
use svelters_compiler::check::{CheckOptions, FileState, check_with};
use svelters_compiler::compile::{CompileOptions, ModuleOptions, compile, compile_module};
use svelters_compiler::semantics::SpecBackend;
use svelters_compiler::target::builtin_target;

const USAGE: &str = "usage: svelters-compile --backend gtk|qt|appkit|winui|egui[,...] [--spec spec.json] [--spec-fragment kinds.json]... [--target quickjs-host|node-vite] [--strict-native] [--filename name] [--dev] [--module] [--css-scope class] [--check] [--json] <file.svelte | file.svelte.js | ->\n       svelters-compile --kinds --backend gtk|qt|appkit|winui|egui [--spec spec.json] [--spec-fragment kinds.json]...\n       svelters-compile --explain <code | id>\n       svelters-compile --list-codes";

fn main() {
    let mut backend = "gtk".to_string();
    let mut spec: Option<String> = None;
    let mut fragments: Vec<String> = Vec::new();
    let mut filename: Option<String> = None;
    let mut json = false;
    let mut dev = false;
    let mut module = false;
    let mut css_scope: Option<String> = None;
    let mut strict_native = false;
    let mut target = "quickjs-host".to_string();
    let mut input: Option<String> = None;
    let mut kinds = false;
    let mut check = false;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--backend" => backend = args.next().expect("--backend needs a value"),
            "--spec" => spec = args.next(),
            "--spec-fragment" => fragments.push(args.next().expect("--spec-fragment needs a value")),
            "--filename" => filename = args.next(),
            "--json" => json = true,
            "--dev" => dev = true,
            "--module" => module = true,
            "--css-scope" => css_scope = args.next(),
            "--strict-native" => strict_native = true,
            "--kinds" => kinds = true,
            "--check" => check = true,
            "--explain" => {
                let c = args.next().expect("--explain needs a code or id");
                match svelters_compiler::codes::explain(&c) {
                    Some(text) => print!("{text}"),
                    None => {
                        eprintln!("svelters-compile: no diagnostic code or id {c:?} (see --list-codes)");
                        std::process::exit(2)
                    }
                }
                return;
            }
            "--list-codes" => {
                for c in svelters_compiler::codes::all() {
                    println!("{} {} {}", c.id, c.code, serde_json::to_value(c.severity).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default());
                }
                return;
            }
            "--target" => target = args.next().expect("--target needs a value"),
            "-h" | "--help" => {
                eprintln!("{USAGE}");
                return;
            }
            _ => input = Some(a),
        }
    }
    if kinds {
        let first = backend.split(',').map(str::trim).find(|b| !b.is_empty()).unwrap_or("gtk");
        let sem = match &spec {
            Some(p) => SpecBackend::from_path(p),
            None => SpecBackend::builtin(first),
        }
        .and_then(|s| fragments.iter().try_fold(s, |s, f| s.merge_fragment_path(f)));
        match sem {
            Ok(sem) => print!("{}", svelters_compiler::semantics::kinds_module(sem.spec())),
            Err(e) => {
                eprintln!("svelters-compile: {e}");
                std::process::exit(2)
            }
        }
        return;
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
    if check {
        let fail = |e: String| -> ! {
            eprintln!("svelters-compile: {e}");
            std::process::exit(2)
        };
        let mut ids = backend.split(',').map(str::trim).filter(|b| !b.is_empty());
        let first = ids.next().unwrap_or("gtk");
        let sem = match &spec {
            Some(p) => SpecBackend::from_path(p),
            None => SpecBackend::builtin(first),
        }
        .and_then(|s| fragments.iter().try_fold(s, |s, f| s.merge_fragment_path(f)))
        .unwrap_or_else(|e| fail(e));
        let others: Vec<SpecBackend> = ids.map(|b| SpecBackend::builtin(b).unwrap_or_else(|e| fail(e))).collect();
        let opts = CheckOptions {
            strict_native,
            dev,
            portability: others.iter().map(|o| o as &dyn svelters_compiler::semantics::PlatformSemantics).collect(),
        };
        // the kind comes from the path: `--module` checks a module whatever its name
        let path = if module && !filename.ends_with(".svelte.js") { format!("{filename}.svelte.js") } else { filename.clone() };
        let res = check_with(&path, &src, &sem, &opts, &mut FileState::new());
        for d in &res.diagnostics {
            let sev = match d.severity {
                svelters_compiler::diag::Severity::Error => "error",
                svelters_compiler::diag::Severity::Warning => "warning",
                svelters_compiler::diag::Severity::Note => "note",
            };
            let be = d.backend.as_deref().map(|b| format!(" ({b})")).unwrap_or_default();
            eprintln!("{filename}:{}:{}: {sev}[{}]{be}: {}", d.range.start_pos.line, d.range.start_pos.column, d.id, d.message);
            if let Some(h) = &d.help {
                eprintln!("  help: {h}");
            }
            for f in &d.fixes {
                eprintln!("  fix: {}", f.message);
            }
        }
        if json {
            println!("{}", serde_json::to_string(&res).expect("result serializes"));
        }
        if res.has_errors() {
            std::process::exit(1);
        }
        return;
    }
    let res = if is_module {
        compile_module(&src, &ModuleOptions { filename: filename.clone(), dev })
    } else {
        let fail = |e: String| -> ! {
            eprintln!("svelters-compile: {e}");
            std::process::exit(2)
        };
        let mut ids = backend.split(',').map(str::trim).filter(|b| !b.is_empty());
        let first = ids.next().unwrap_or("gtk");
        let sem = match &spec {
            Some(p) => SpecBackend::from_path(p),
            None => SpecBackend::builtin(first),
        }
        .and_then(|s| fragments.iter().try_fold(s, |s, f| s.merge_fragment_path(f)))
        .unwrap_or_else(|e| fail(e));
        let others: Vec<SpecBackend> = ids.map(|b| SpecBackend::builtin(b).unwrap_or_else(|e| fail(e))).collect();
        let target = builtin_target(&target).unwrap_or_else(|| fail(format!("unknown --target {target:?} (known: quickjs-host, node-vite)")));
        let mut opts = CompileOptions::new(filename.clone(), &sem);
        opts.dev = dev;
        opts.css_scope = css_scope;
        opts.strict_native = strict_native;
        opts.target = target;
        opts.portability = others.iter().map(|o| o as &dyn svelters_compiler::semantics::PlatformSemantics).collect();
        compile(&src, &opts)
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
