//! A libtest-compatible runner for `harness = false` test targets (WinUI objects need one STA
//! thread, so the tests run sequentially on the main thread). It understands what cargo test and
//! nextest pass: `--list [--ignored]`, name filters, `--exact`, and ignores the other flags.
//! Platform-neutral so the same binary lists zero tests on Linux and nextest stays happy.

pub fn main(tests: &[(&str, fn())]) -> i32 {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--list") {
        if !args.iter().any(|a| a == "--ignored") {
            for (name, _) in tests {
                println!("{name}: test");
            }
        }
        return 0;
    }
    let exact = args.iter().any(|a| a == "--exact");
    let mut filters = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--test-threads" | "--format" | "--color" | "--skip" | "--logfile" => {
                it.next();
            }
            f if !f.starts_with('-') => filters.push(f.to_string()),
            _ => {}
        }
    }
    let selected = |name: &str| filters.is_empty() || filters.iter().any(|f| if exact { name == f } else { name.contains(f.as_str()) });
    let mut results = Results::open();
    let mut failed = Vec::new();
    let mut ran = 0;
    for (name, f) in tests {
        if !selected(name) {
            continue;
        }
        ran += 1;
        results.line(name, "start", "");
        match std::panic::catch_unwind(f) {
            Ok(()) => {
                println!("test {name} ... ok");
                results.line(name, "pass", "");
            }
            Err(e) => {
                let msg = e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default();
                println!("test {name} ... FAILED\n    {msg}");
                results.line(name, "fail", &msg);
                failed.push(*name);
            }
        }
    }
    println!("\ntest result: {}. {} passed; {} failed", if failed.is_empty() { "ok" } else { "FAILED" }, ran - failed.len(), failed.len());
    for f in &failed {
        println!("    {f}");
    }
    if failed.is_empty() { 0 } else { 1 }
}

/// With `SVELTERS_TEST_RESULTS=<file>`, one JSON line per test start and per result, flushed as it
/// happens, so a test that takes the process down leaves its `start` line without a result. The
/// ledger imports the file from the CI artifact (`tools/metadata/ledger.py import --toolkit winui`).
struct Results(Option<std::fs::File>);

impl Results {
    fn open() -> Results {
        let file = std::env::var_os("SVELTERS_TEST_RESULTS").and_then(|p| {
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&p)
                .map_err(|e| eprintln!("harness: cannot open {}: {e}", std::path::Path::new(&p).display()))
                .ok()
        });
        Results(file)
    }

    fn line(&mut self, test: &str, result: &str, message: &str) {
        use std::io::Write;
        if let Some(f) = &mut self.0 {
            let _ = writeln!(f, "{{\"test\":{},\"result\":{},\"message\":{}}}", json_str(test), json_str(result), json_str(message));
            let _ = f.flush();
        }
    }
}

fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn json_strings_are_escaped() {
        assert_eq!(super::json_str("a\"b\\c\nd\u{1}"), "\"a\\\"b\\\\c\\nd\\u0001\"");
    }
}
