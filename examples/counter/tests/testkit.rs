//! TK1: svelters' own test cases (`tests/cases/*.svelte`) on the real GTK and Qt toolkits.
//!
//! Each case is first run on the log backend for the toolkit (`svelters_testkit::log`), which
//! resolves its `click`/`type` steps to handles and gives the plan: a `script.json` plus the
//! expectations of each checkpoint. The `counter` binary performs that script natively (button
//! clicks, typing), dumps the widget tree at every checkpoint step (`checkpoint_dumps` in its
//! report) and takes a screenshot (`target/p5/testkit-<toolkit>-<case>.png`); the expectations are
//! checked on those dumps, and the results go through the case ratchet (`tests/cases/baseline.json`).
//!
//! Needs a display and the case bundles: `pnpm --filter ./tests/cases build`, then
//! `tools/dev/headless cargo nextest run -p counter testkit`. `TK_CASE=<substring>` filters cases
//! (the ratchet is then not checked).

use std::path::{Path, PathBuf};
use std::process::Command;
use svelters_testkit::{Outcome, case, log, ratchet, repo_cases, results_dir, view};

fn out_dir() -> PathBuf {
    let d = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/p5");
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn run_real(toolkit: &str, c: &case::Case) -> Outcome {
    let dist = repo_cases().join("dist");
    let bundle = c.bundle(&dist, toolkit);
    let (resolved, plan) = log::run(c, &bundle, toolkit);
    if !resolved.passed() {
        let mut o = Outcome::fail("the case does not pass on the log backend, which resolves its steps:");
        o.failures.extend(resolved.failures);
        return o;
    }
    let name = format!("testkit-{toolkit}-{}", c.name);
    let dir = out_dir();
    let script = dir.join(format!("{name}.script.json"));
    std::fs::write(&script, plan.script_json()).unwrap();
    let report = dir.join(format!("{name}.json"));
    let _ = std::fs::remove_file(&report);
    let out = Command::new(env!("CARGO_BIN_EXE_counter"))
        // see tests/integration.rs: cairo avoids GDK's Vulkan path under Xvfb
        .env("GSK_RENDERER", std::env::var("GSK_RENDERER").unwrap_or_else(|_| "cairo".into()))
        .args(["--backend", toolkit, "--timeout", "30", "--fixture"])
        .arg(bundle.parent().unwrap().parent().unwrap())
        .arg("--script")
        .arg(&script)
        .arg("--screenshot")
        .arg(dir.join(format!("{name}.png")))
        .arg("--report")
        .arg(&report)
        .output()
        .expect("spawn counter");
    let stderr = String::from_utf8_lossy(&out.stderr);
    let Some(r) = std::fs::read_to_string(&report).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()) else {
        return Outcome::fail(format!("no report (exit {:?}); stderr:\n{stderr}", out.status));
    };
    let mut o = Outcome::default();
    for key in ["failures", "errors"] {
        for f in r[key].as_array().into_iter().flatten() {
            o.failures.push(format!("{key}: {}", f.as_str().unwrap_or_default()));
        }
    }
    let dumps: Vec<Vec<view::ViewNode>> = r["checkpoint_dumps"].as_array().into_iter().flatten().map(view::from_dump).collect();
    o.failures.extend(plan.verify(&dumps));
    o.checks = plan.checks.iter().map(Vec::len).sum();
    o
}

fn run_all(toolkit: &str) {
    let filter = std::env::var("TK_CASE").ok();
    let results: Vec<ratchet::CaseResult> = case::load_dir(&repo_cases())
        .unwrap()
        .iter()
        .filter(|c| c.targets(toolkit) && filter.as_ref().is_none_or(|f| c.name.contains(f.as_str())))
        .map(|c| ratchet::CaseResult::new(c, toolkit, run_real(toolkit, c)))
        .collect();
    let cases = if filter.is_some() { results_dir() } else { repo_cases() };
    match ratchet::report(&cases, &results_dir(), toolkit, &results) {
        Ok(summary) => eprintln!("{summary}"),
        Err(e) => panic!("{e}"),
    }
}

#[test]
fn testkit_gtk() {
    run_all("gtk");
}

#[test]
fn testkit_qt() {
    run_all("qt");
}
