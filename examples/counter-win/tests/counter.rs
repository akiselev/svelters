//! The counter on WinUI, end to end: spawns `counter-win --selftest` (WinUI wants the process's
//! own STA main thread) and checks its report. Needs the Windows App Runtime and the built bundle
//! (docs/notes/windows-setup.md). Off Windows the target lists no tests.
//!
//! `winui_types_selftest` does the same for `fixtures/winui-types` (N4: non-GUI WinRT types from Svelte).

const TESTS: &[(&str, &str)] = &[("counter_win_selftest", "counter-win"), ("winui_types_selftest", "winui-types")];

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
    for (name, fixture) in TESTS {
        if !filters.is_empty() && !filters.iter().any(|f| name.contains(f.as_str())) {
            continue;
        }
        #[cfg(windows)]
        selftest(name, fixture);
        #[cfg(not(windows))]
        let _ = fixture;
    }
}

#[cfg(windows)]
fn selftest(name: &str, fixture: &str) {
    let dir = std::env::temp_dir().join(format!("counter-win-{}-{fixture}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let report = dir.join("report.json");
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_counter-win"))
        .args(["--fixture", fixture, "--selftest", "--timeout", "30", "--report"])
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
