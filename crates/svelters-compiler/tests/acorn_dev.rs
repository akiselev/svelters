//! Dev-only differential check of the acorn port against real acorn (agent A's helper; the
//! official harness is `tests/oracle_*.rs`).
//!
//! Generate a corpus with `tests/acorn_dev/collect_acorn_dev.mjs`, then run
//! `ACORN_DEV_CORPUS=<corpus.jsonl> cargo test -p svelters-compiler --release --test acorn_dev -- --nocapture`.
//! Without `ACORN_DEV_CORPUS` the test does nothing. `ACORN_DEV_FAILURES=<file>` writes details.

use std::collections::BTreeMap;
use std::io::{BufRead, Write};

use serde_json::Value;
use svelters_compiler::acorn::{
    parse_expression_at_with, parse_with, AcornOptions, AcornSourceType, AllowReserved, ECMA_LATEST,
};

/// Fields acorn only emits for newer `ecmaVersion`s; the typed AST always has them.
const VERSION_GATED: &[&str] =
    &["generator", "async", "expression", "method", "shorthand", "computed", "optional", "await", "options", "attributes", "exported"];

#[allow(clippy::field_reassign_with_default)]
fn options_from(v: &Value) -> AcornOptions {
    let mut o = AcornOptions::default();
    o.ecma_version = match v.get("ecmaVersion") {
        Some(Value::String(s)) if s == "latest" => ECMA_LATEST,
        Some(Value::Number(n)) => n.as_u64().unwrap_or(5) as u32,
        _ => 5,
    };
    o.source_type = match v.get("sourceType").and_then(|s| s.as_str()) {
        Some("module") => AcornSourceType::Module,
        Some("commonjs") => AcornSourceType::CommonJs,
        _ => AcornSourceType::Script,
    };
    let b = |k: &str| v.get(k).and_then(|x| x.as_bool());
    o.allow_reserved = match v.get("allowReserved") {
        Some(Value::Bool(true)) => Some(AllowReserved::Yes),
        Some(Value::Bool(false)) => Some(AllowReserved::No),
        Some(Value::String(s)) if s == "never" => Some(AllowReserved::Never),
        _ => None,
    };
    o.allow_return_outside_function = b("allowReturnOutsideFunction").unwrap_or(false);
    o.allow_import_export_everywhere = b("allowImportExportEverywhere").unwrap_or(false);
    o.allow_await_outside_function = b("allowAwaitOutsideFunction");
    o.allow_super_outside_method = b("allowSuperOutsideMethod");
    o.allow_hash_bang = b("allowHashBang");
    o.check_private_fields = b("checkPrivateFields").unwrap_or(true);
    o.preserve_parens = b("preserveParens").unwrap_or(false);
    o.strict = b("strict").unwrap_or(false);
    o
}

fn diff(path: &str, exp: &Value, ours: &Value) -> Option<String> {
    match (exp, ours) {
        (Value::Object(e), Value::Object(o)) => {
            for (k, ev) in e {
                match o.get(k) {
                    None => return Some(format!("{path}.{k}: missing (expected {ev})")),
                    Some(ov) => {
                        if let Some(d) = diff(&format!("{path}.{k}"), ev, ov) {
                            return Some(d);
                        }
                    }
                }
            }
            for (k, ov) in o {
                if !e.contains_key(k) && !VERSION_GATED.contains(&k.as_str()) {
                    return Some(format!("{path}.{k}: unexpected {ov}"));
                }
            }
            None
        }
        (Value::Array(e), Value::Array(o)) => {
            if e.len() != o.len() {
                return Some(format!("{path}: length {} vs {}", e.len(), o.len()));
            }
            e.iter().zip(o).enumerate().find_map(|(i, (a, b))| diff(&format!("{path}[{i}]"), a, b))
        }
        (Value::Number(a), Value::Number(b)) => {
            if a.as_f64() == b.as_f64() {
                None
            } else {
                Some(format!("{path}: {a} vs {b}"))
            }
        }
        // A regex literal's `value` is `null` when the host engine cannot construct the RegExp
        // (engine-dependent, e.g. duplicate named groups on older Node).
        (Value::Null, Value::Object(o)) if o.is_empty() && path.ends_with(".value") => None,
        _ => {
            if exp == ours {
                None
            } else {
                Some(format!("{path}: {exp} vs {ours}"))
            }
        }
    }
}

/// Canonical JSON (sorted keys, normalized numbers); mirrors `canon` in collect_acorn_dev.mjs.
fn canon(v: &Value, out: &mut String) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => {
            let f = n.as_f64().unwrap();
            if f.fract() == 0.0 && f.abs() < 9007199254740992.0 {
                out.push_str(&format!("{}", f as i64));
            } else {
                out.push_str(&format!("{:.16e}", f));
            }
        }
        Value::String(s) => out.push_str(&serde_json::to_string(s).unwrap()),
        Value::Array(a) => {
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                canon(x, out);
            }
            out.push(']');
        }
        Value::Object(o) => {
            let mut keys: Vec<&String> = o.keys().collect();
            keys.sort();
            out.push('{');
            for (i, k) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(k).unwrap());
                out.push(':');
                canon(&o[*k], out);
            }
            out.push('}');
        }
    }
}

fn fnv(s: &str) -> u64 {
    let mut h: u32 = 0x811c9dc5;
    for u in s.encode_utf16() {
        h ^= u as u32;
        h = h.wrapping_mul(0x01000193);
    }
    h as u64
}

fn run_case(case: &Value, opts: &AcornOptions) -> Value {
    let code = case["code"].as_str().unwrap();
    let mut comments = Vec::new();
    let r = if case["kind"] == "expr" {
        let pos = case["pos"].as_u64().unwrap() as u32;
        parse_expression_at_with(code, pos, opts, &mut comments).map(|e| serde_json::to_value(&e).unwrap())
    } else {
        parse_with(code, opts, &mut comments).map(|p| serde_json::to_value(&p).unwrap())
    };
    match r {
        Ok(ast) => {
            let v = serde_json::json!({ "ast": ast, "comments": serde_json::to_value(&comments).unwrap() });
            if case["expect"].get("hash").is_some() {
                let mut s = String::new();
                canon(&v, &mut s);
                serde_json::json!({ "hash": fnv(&s) })
            } else {
                v
            }
        }
        Err(e) => serde_json::json!({ "error": e.message, "pos": e.pos }),
    }
}

fn corpus_lines(path: &str) -> impl Iterator<Item = Value> {
    let f = std::io::BufReader::new(std::fs::File::open(path).unwrap());
    f.lines().map(|l| l.unwrap()).filter(|l| !l.is_empty()).map(|l| serde_json::from_str(&l).unwrap())
}

/// Parse throughput over the module files of the corpus (run with --release).
#[test]
fn acorn_dev_perf() {
    let Ok(path) = std::env::var("ACORN_DEV_CORPUS") else { return };
    let mut sources = Vec::new();
    for case in corpus_lines(&path) {
        let id = case["id"].as_str().unwrap();
        if id.contains('/') && !id.contains('#') && !id.contains('@') {
            sources.push(case["code"].as_str().unwrap().to_string());
        }
    }
    let opts = AcornOptions { source_type: AcornSourceType::Module, ..AcornOptions::default() };
    let lines: usize = sources.iter().map(|s| s.lines().count()).sum();
    let t = std::time::Instant::now();
    for s in &sources {
        let mut c = Vec::new();
        let _ = parse_with(s, &opts, &mut c);
    }
    let el = t.elapsed();
    println!(
        "parsed {} files, {} lines in {:?} ({:.2} ms per 5k lines)",
        sources.len(),
        lines,
        el,
        el.as_secs_f64() * 1000.0 * 5000.0 / lines.max(1) as f64
    );
}

#[test]
fn acorn_dev_corpus() {
    let Ok(path) = std::env::var("ACORN_DEV_CORPUS") else {
        eprintln!("ACORN_DEV_CORPUS not set; skipping");
        return;
    };
    let failures_path = std::env::var("ACORN_DEV_FAILURES").ok();
    let handle = std::thread::Builder::new()
        .stack_size(512 << 20)
        .spawn(move || {
            let mut groups: BTreeMap<String, (usize, usize)> = BTreeMap::new();
            let mut fails = Vec::new();
            for case in corpus_lines(&path) {
                let id = case["id"].as_str().unwrap().to_string();
                let code = case["code"].as_str().unwrap();
                let opts = options_from(&case["options"]);
                let group = if !id.contains('/') {
                    "acorn-tests"
                } else if id.ends_with("#hash") {
                    "files(hashed)"
                } else if id.contains('@') {
                    "files(parseExpressionAt)"
                } else if id.ends_with("#parens") {
                    "files(preserveParens)"
                } else {
                    "files"
                };
                let ours = std::panic::catch_unwind(|| run_case(&case, &opts))
                    .unwrap_or_else(|_| serde_json::json!({ "panic": true }));
                let entry = groups.entry(group.to_string()).or_default();
                entry.1 += 1;
                match diff("$", &case["expect"], &ours) {
                    None => entry.0 += 1,
                    Some(d) => fails.push(format!(
                        "{id}\n  code: {}\n  opts: {}\n  diff: {d}\n",
                        code.chars().take(300).collect::<String>(),
                        case["options"]
                    )),
                }
            }
            for (g, (ok, total)) in &groups {
                println!("{g}: {ok}/{total} passing");
            }
            if let Some(fp) = failures_path {
                let mut f = std::fs::File::create(fp).unwrap();
                for x in &fails {
                    writeln!(f, "{x}").unwrap();
                }
            }
            fails.len()
        })
        .unwrap();
    let n = handle.join().unwrap();
    println!("{n} failures");
}
