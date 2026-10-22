//! svelters' own TK1 cases (`tests/cases/*.svelte`) on the in-process backends: the log backend
//! standing in for GTK and Qt, and egui under kittest. Needs the bundles: `pnpm --filter
//! ./tests/cases build` (after `cargo build -p svelters-compiler --bin svelters-compile`). The real
//! GTK and Qt runs are in `examples/counter/tests/testkit.rs`.
//!
//! `TK_CASE=<substring>` runs the matching cases only (the ratchet is then not checked).

use svelters_testkit::{case, ratchet, repo_cases, results_dir};

fn cases_for(toolkit: &str) -> Vec<case::Case> {
    let filter = std::env::var("TK_CASE").ok();
    case::load_dir(&repo_cases())
        .unwrap()
        .into_iter()
        .filter(|c| c.targets(toolkit) && filter.as_ref().is_none_or(|f| c.name.contains(f.as_str())))
        .collect()
}

fn finish(backend: &str, results: Vec<ratchet::CaseResult>) {
    let filtered = std::env::var_os("TK_CASE").is_some();
    let dir = if filtered { results_dir().join("filtered") } else { results_dir() };
    // a filtered run checks its cases but not the ratchet
    let cases = if filtered { results_dir() } else { repo_cases() };
    match ratchet::report(&cases, &dir, backend, &results) {
        Ok(summary) => eprintln!("{summary}"),
        Err(e) => panic!("{e}"),
    }
}

fn log_backend(toolkit: &str) {
    let dist = repo_cases().join("dist");
    let backend = svelters_testkit::log::backend_name(toolkit);
    let results = cases_for(toolkit)
        .iter()
        .map(|c| {
            let (out, _plan) = svelters_testkit::log::run(c, &c.bundle(&dist, toolkit), toolkit);
            ratchet::CaseResult::new(c, &backend, out)
        })
        .collect();
    finish(&backend, results);
}

#[test]
fn cases_on_log_gtk() {
    log_backend("gtk");
}

#[test]
fn cases_on_log_qt() {
    log_backend("qt");
}

#[test]
fn cases_on_egui() {
    let dir = repo_cases();
    let dist = dir.join("dist");
    let mut recorded = Vec::new();
    let results = cases_for("egui")
        .iter()
        .map(|c| {
            let (out, rec) = svelters_testkit::egui::run(c, &c.bundle(&dist, "egui"), Some(&dir.join("snapshots")));
            recorded.extend(rec);
            ratchet::CaseResult::new(c, svelters_testkit::egui::BACKEND, out)
        })
        .collect();
    for p in recorded {
        eprintln!("recorded a new snapshot: {} (commit it)", p.display());
    }
    finish(svelters_testkit::egui::BACKEND, results);
}
