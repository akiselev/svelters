//! Minimal libtest command line for `harness = false` test binaries whose cases must run on the
//! process's first thread (AppKit, like Qt), so `cargo test` and `cargo nextest` can list and run
//! them. Same contract as `counter_qt::run_cases`.
//!
//! - `--list [--format terse] [--ignored]` prints `<name>: test` per case;
//! - a run executes the cases selected by the filters (all when none), `--exact` matches whole names;
//! - each case is wrapped in `catch_unwind` and reported `test <name> ... ok|FAILED`; the process
//!   exits 1 if any failed.
//! - with `SVELTERS_TEST_RESULTS=<file>` set, every case run also appends one JSON line
//!   `{"suite": <binary>, "test": <name>, "result": "pass"|"fail", "message": <panic message>}` to
//!   the file: the results `tools/metadata/ledger.py import` reads (CI uploads it as an artifact).
//!
//! Table-driven cases (`run_table_cases`) are one function over a row table: each row is its own
//! named case (listed, filtered and reported like any other), so a table of facts still gets one
//! result per fact.

use std::io::Write;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Mutex;

/// Runs the selected cases in order on the calling thread. `enabled == false` (not on macOS) lists
/// nothing and runs nothing, so the binary is an empty test suite there.
pub fn run_cases(enabled: bool, cases: &[(&str, fn())]) {
    run_table_cases(enabled, cases, &[]);
}

/// [`run_cases`] plus row cases: `(name, runner, row index)`; the runner is called with the index.
pub fn run_table_cases(enabled: bool, cases: &[(&str, fn())], rows: &[(&str, fn(usize), usize)]) {
    let all: Vec<(&str, Case)> = cases
        .iter()
        .map(|(n, f)| (*n, Case::Plain(*f)))
        .chain(rows.iter().map(|(n, f, i)| (*n, Case::Row(*f, *i))))
        .collect();
    run(enabled, &all);
}

#[derive(Clone, Copy)]
enum Case {
    Plain(fn()),
    Row(fn(usize), usize),
}

static LAST_PANIC: Mutex<Option<String>> = Mutex::new(None);

/// Keeps the default panic output and remembers the message of the last panic (for the results file).
fn install_panic_hook() {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let msg = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "panic without a message".to_string()
        };
        let at = info.location().map(|l| format!(" ({}:{})", l.file(), l.line())).unwrap_or_default();
        if let Ok(mut g) = LAST_PANIC.lock() {
            *g = Some(format!("{msg}{at}"));
        }
        prev(info);
    }));
}

/// A JSON string literal (the crate has no serde).
pub fn json_str(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// One results line (see the module docs).
pub fn result_line(suite: &str, test: &str, ok: bool, message: Option<&str>) -> String {
    let mut o = format!(
        "{{\"suite\": {}, \"test\": {}, \"result\": \"{}\"",
        json_str(suite),
        json_str(test),
        if ok { "pass" } else { "fail" }
    );
    if let Some(m) = message {
        let m: String = m.chars().take(400).collect();
        o.push_str(&format!(", \"message\": {}", json_str(&m)));
    }
    o.push('}');
    o
}

fn suite_name() -> String {
    let exe = std::env::args().next().unwrap_or_default();
    let stem = std::path::Path::new(&exe).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    // cargo names test binaries `<name>-<hash>`
    match stem.rsplit_once('-') {
        Some((n, h)) if h.len() == 16 && h.chars().all(|c| c.is_ascii_hexdigit()) => n.to_string(),
        _ => stem,
    }
}

fn run(enabled: bool, cases: &[(&str, Case)]) {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--list") {
        if enabled && !args.iter().any(|a| a == "--ignored") {
            for (name, _) in cases {
                println!("{name}: test");
            }
        }
        return;
    }
    if !enabled {
        println!("running 0 tests (macOS only)");
        return;
    }
    let mut filters = Vec::new();
    let mut skips = Vec::new();
    let mut exact = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--exact" => exact = true,
            "--skip" => {
                if let Some(s) = it.next() {
                    skips.push(s.as_str());
                }
            }
            "--test-threads" | "--format" | "--color" | "--logfile" => {
                it.next();
            }
            f if !f.starts_with('-') => filters.push(f),
            _ => {}
        }
    }
    let matches = |name: &str, f: &str| if exact { name == f } else { name.contains(f) };
    let selected: Vec<&(&str, Case)> = cases
        .iter()
        .filter(|(n, _)| filters.is_empty() || filters.iter().any(|f| matches(n, f)))
        .filter(|(n, _)| !skips.iter().any(|s| n.contains(s)))
        .collect();
    let mut results = std::env::var_os("SVELTERS_TEST_RESULTS").and_then(|p| {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&p)
            .map_err(|e| eprintln!("SVELTERS_TEST_RESULTS: cannot open {}: {e}", p.to_string_lossy()))
            .ok()
    });
    let suite = suite_name();
    install_panic_hook();
    println!("running {} tests", selected.len());
    let mut failed = Vec::new();
    for (name, case) in selected {
        if let Ok(mut g) = LAST_PANIC.lock() {
            *g = None;
        }
        let ok = match *case {
            Case::Plain(body) => catch_unwind(AssertUnwindSafe(body)).is_ok(),
            Case::Row(body, i) => catch_unwind(AssertUnwindSafe(|| body(i))).is_ok(),
        };
        println!("test {name} ... {}", if ok { "ok" } else { "FAILED" });
        if let Some(f) = results.as_mut() {
            let msg = if ok { None } else { LAST_PANIC.lock().ok().and_then(|g| g.clone()) };
            let _ = writeln!(f, "{}", result_line(&suite, name, ok, msg.as_deref()));
            let _ = f.flush();
        }
        if !ok {
            failed.push(*name);
        }
    }
    if !failed.is_empty() {
        println!("\nfailures:");
        for f in &failed {
            println!("    {f}");
        }
        println!("\ntest result: FAILED. {} failed", failed.len());
        std::process::exit(1);
    }
    println!("\ntest result: ok.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_lines_are_json() {
        assert_eq!(result_line("generated", "prop_a", true, None), r#"{"suite": "generated", "test": "prop_a", "result": "pass"}"#);
        assert_eq!(
            result_line("g", "t", false, Some("left: \"a\"\nright")),
            r#"{"suite": "g", "test": "t", "result": "fail", "message": "left: \"a\"\nright"}"#
        );
        assert_eq!(json_str("\u{1}"), "\"\\u0001\"");
    }
}
