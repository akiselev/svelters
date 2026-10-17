//! Minimal libtest command line for `harness = false` test binaries whose cases must run on the
//! process's first thread (AppKit, like Qt), so `cargo test` and `cargo nextest` can list and run
//! them. Same contract as `counter_qt::run_cases`.
//!
//! - `--list [--format terse] [--ignored]` prints `<name>: test` per case;
//! - a run executes the cases selected by the filters (all when none), `--exact` matches whole names;
//! - each case is wrapped in `catch_unwind` and reported `test <name> ... ok|FAILED`; the process
//!   exits 1 if any failed.

use std::panic::{AssertUnwindSafe, catch_unwind};

/// Runs the selected cases in order on the calling thread. `enabled == false` (not on macOS) lists
/// nothing and runs nothing, so the binary is an empty test suite there.
pub fn run_cases(enabled: bool, cases: &[(&str, fn())]) {
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
    let selected: Vec<&(&str, fn())> = cases
        .iter()
        .filter(|(n, _)| filters.is_empty() || filters.iter().any(|f| matches(n, f)))
        .filter(|(n, _)| !skips.iter().any(|s| n.contains(s)))
        .collect();
    println!("running {} tests", selected.len());
    let mut failed = Vec::new();
    for (name, body) in selected {
        let ok = catch_unwind(AssertUnwindSafe(body)).is_ok();
        println!("test {name} ... {}", if ok { "ok" } else { "FAILED" });
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
