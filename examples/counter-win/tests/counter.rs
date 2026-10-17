//! The counter on WinUI, end to end: spawns `counter-win --selftest` (WinUI wants the process's
//! own STA main thread) and checks its report. Needs the Windows App Runtime and the built bundle
//! (docs/notes/windows-setup.md). Off Windows the target lists no tests.

const NAME: &str = "counter_win_selftest";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--list") {
        if cfg!(windows) && !args.iter().any(|a| a == "--ignored") {
            println!("{NAME}: test");
        }
        return;
    }
    let filters: Vec<&String> = args.iter().filter(|a| !a.starts_with('-')).collect();
    if !filters.is_empty() && !filters.iter().any(|f| NAME.contains(f.as_str())) {
        return;
    }
    #[cfg(windows)]
    selftest();
}

#[cfg(windows)]
fn selftest() {
    let dir = std::env::temp_dir().join(format!("counter-win-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let report = dir.join("report.json");
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_counter-win"))
        .args(["--selftest", "--timeout", "30", "--report"])
        .arg(&report)
        .output()
        .expect("spawn counter-win");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    println!("{stdout}");
    eprintln!("{stderr}");
    let text = std::fs::read_to_string(&report).unwrap_or_else(|_| panic!("no report (exit {:?})\n{stderr}", out.status.code()));
    let rep: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(rep["ok"], serde_json::json!(true), "selftest failed: {}", rep["failures"]);
    assert!(out.status.success(), "exit {:?}", out.status.code());
    println!("test {NAME} ... ok");
}
