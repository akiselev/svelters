//! The counter on WinUI, end to end: spawns `counter-win --selftest` (WinUI wants the process's
//! own STA main thread) and checks its report. Needs the Windows App Runtime and the built bundle
//! (docs/notes/windows-setup.md). Off Windows the target lists no tests.
//!
//! `winui_types_selftest` does the same for `fixtures/winui-types` (N4: non-GUI WinRT types from Svelte).
//! The `start_*` tests run `counter-win --start-test <mode>` (G0, ABI v2.7 `start`, FU1).

/// (test name, `counter-win` arguments).
const TESTS: &[(&str, &[&str])] = &[
    ("counter_win_selftest", &["--fixture", "counter-win", "--selftest", "--timeout", "30"]),
    // WG3w: the UI thread starts its loop 3 s late (a slow first frame), so every startup timer is due at once;
    // the read-back must still follow the driving (the CI flake "Count: 0").
    ("counter_win_selftest_slow_start", &["--fixture", "counter-win", "--selftest", "--timeout", "30", "--stall-ms", "3000"]),
    ("winui_types_selftest", &["--fixture", "winui-types", "--selftest", "--timeout", "30"]),
    ("start_with_mounts_in_the_hook_while_pumping", &["--start-test", "start-with"]),
    ("run_with_mounts_in_the_hook_of_the_loop", &["--start-test", "run-with"]),
    ("desktop_order_mount_has_the_host_attached", &["--start-test", "preattach"]),
];

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--list") {
        if cfg!(windows) && !args.iter().any(|a| a == "--ignored") {
            for (name, _) in TESTS {
                println!("{name}: test");
            }
        }
        return;
    }
    let filters: Vec<&String> = args.iter().filter(|a| !a.starts_with('-')).collect();
    for (name, argv) in TESTS {
        if !filters.is_empty() && !filters.iter().any(|f| name.contains(f.as_str())) {
            continue;
        }
        #[cfg(windows)]
        selftest(name, argv);
        #[cfg(not(windows))]
        let _ = argv;
    }
}

#[cfg(windows)]
fn selftest(name: &str, argv: &[&str]) {
    let dir = std::env::temp_dir().join(format!("counter-win-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let report = dir.join("report.json");
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_counter-win"))
        .args(argv)
        .arg("--report")
        .arg(&report)
        .output()
        .expect("spawn counter-win");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    println!("{stdout}");
    eprintln!("{stderr}");
    let text = std::fs::read_to_string(&report).unwrap_or_else(|_| panic!("{name}: no report (exit {:?})\n{stderr}", out.status.code()));
    let rep: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(rep["ok"], serde_json::json!(true), "{name}: selftest failed: {}", rep["failures"]);
    assert!(out.status.success(), "{name}: exit {:?}", out.status.code());
    println!("test {name} ... ok");
}
