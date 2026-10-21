//! The eframe runner opens a real window (glow, software GL under Xvfb) and goes idle: after the
//! counter settles, no frame is drawn and the process uses ~no CPU. Needs a display: nextest runs
//! every Linux test under tools/dev/headless; a plain `cargo test` without DISPLAY skips it.

use std::process::Command;

#[test]
fn eframe_window_draws_then_idles_at_zero_cpu() {
    if std::env::var_os("DISPLAY").is_none() {
        eprintln!("SKIP: no DISPLAY (run under tools/dev/headless)");
        return;
    }
    let bundle = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/counter-egui/dist/app.js");
    assert!(bundle.exists(), "run `pnpm build` first");
    let out = Command::new(env!("CARGO_BIN_EXE_svelters-egui-run"))
        .args(["--idle-report", "2000"])
        .arg(&bundle)
        .output()
        .expect("runner");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "runner failed: {:?}\n{stdout}\n{}", out.status, String::from_utf8_lossy(&out.stderr));
    let line = stdout.lines().find(|l| l.starts_with('{')).expect("report line");
    let r: serde_json::Value = serde_json::from_str(line).unwrap();
    eprintln!("idle report: {r}");
    assert!(r["frames_before"].as_u64().unwrap() > 0, "the window drew its first frames");
    // Locally this is 0 frames / 0.0 ms. CI's Xvfb once delivered a late window event (6 frames,
    // 30 ms): allow such a burst, but not a repaint loop (that would be 100+ frames in 2 s).
    assert!(r["frames"].as_u64().unwrap() <= 10, "a repaint loop while idle: {r}");
    assert!(r["cpu_ms"].as_f64().unwrap() < 100.0, "idle CPU over 2 s: {r}");
}
