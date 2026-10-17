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
    let mut failed = Vec::new();
    let mut ran = 0;
    for (name, f) in tests {
        if !selected(name) {
            continue;
        }
        ran += 1;
        match std::panic::catch_unwind(f) {
            Ok(()) => println!("test {name} ... ok"),
            Err(e) => {
                let msg = e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default();
                println!("test {name} ... FAILED\n    {msg}");
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
