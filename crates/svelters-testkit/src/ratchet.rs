//! Results per backend and the ratchet (the conformance runner's `baseline.json` idea).
//!
//! Rules, per backend:
//! - a case that targets the backend passes, or is declared `@xfail <backend> "<reason>"`;
//! - an `@xfail` case that passes is an error (remove the `@xfail`);
//! - `<cases>/baseline.json` lists the (backend, case) pairs that passed when it was last updated;
//!   each of them must still pass (so a regression cannot hide behind a new `@xfail` or a dropped
//!   toolkit). New passes are reported; `SVELTERS_UPDATE=1` records them (and refuses to drop a
//!   listed pair unless `FORCE=1`). Without a `baseline.json` (a new cases directory, cadatomic's)
//!   only the first two rules apply.
//!
//! Each run writes `<out>/<backend>.json` (the per-case results) for CI artifacts.

use crate::case::Case;
use crate::run::Outcome;
use serde_json::{Value as Json, json};
use std::collections::BTreeSet;
use std::path::Path;

pub struct CaseResult {
    pub case: String,
    pub xfail: Option<String>,
    pub outcome: Outcome,
}

impl CaseResult {
    pub fn new(case: &Case, backend: &str, outcome: Outcome) -> CaseResult {
        CaseResult { case: case.name.clone(), xfail: case.xfail(backend).map(str::to_string), outcome }
    }

    fn status(&self) -> &'static str {
        match (self.outcome.passed(), &self.xfail) {
            (true, None) => "pass",
            (true, Some(_)) => "xpass",
            (false, Some(_)) => "xfail",
            (false, None) => "fail",
        }
    }
}

/// Checks `results` of `backend` against the rules above; `Ok` holds the summary, `Err` every
/// violation (and the summary).
pub fn report(cases_dir: &Path, out_dir: &Path, backend: &str, results: &[CaseResult]) -> Result<String, String> {
    let mut lines = Vec::new();
    let mut errors = Vec::new();
    for r in results {
        let st = r.status();
        let mut line = format!("{st:5} {backend}/{}", r.case);
        if let Some(why) = &r.xfail {
            line.push_str(&format!(" (xfail: {why})"));
        }
        for f in &r.outcome.failures {
            line.push_str(&format!("\n        {f}"));
        }
        for n in &r.outcome.notes {
            line.push_str(&format!("\n        note: {n}"));
        }
        match st {
            "fail" => errors.push(format!("{backend}/{} failed", r.case)),
            "xpass" => errors.push(format!("{backend}/{} passes: remove its @xfail {backend}", r.case)),
            _ => {}
        }
        lines.push(line);
    }
    let passed: BTreeSet<String> = results.iter().filter(|r| r.outcome.passed()).map(|r| r.case.clone()).collect();

    let _ = std::fs::create_dir_all(out_dir);
    let doc = json!({
        "backend": backend,
        "cases": results.iter().map(|r| json!({
            "case": r.case, "status": r.status(), "xfail": r.xfail, "checks": r.outcome.checks,
            "failures": r.outcome.failures, "notes": r.outcome.notes,
        })).collect::<Vec<_>>(),
    });
    let _ = std::fs::write(out_dir.join(format!("{backend}.json")), serde_json::to_string_pretty(&doc).unwrap());

    let path = cases_dir.join("baseline.json");
    if let Ok(text) = std::fs::read_to_string(&path) {
        let mut base: Json = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        let listed: BTreeSet<String> = base["pass"][backend].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()).unwrap_or_default();
        let lost: Vec<&String> = listed.difference(&passed).collect();
        let new: Vec<&String> = passed.difference(&listed).collect();
        if std::env::var_os("SVELTERS_UPDATE").is_some() {
            if !lost.is_empty() && std::env::var_os("FORCE").is_none() {
                errors.push(format!("baseline: refusing to drop {lost:?} from {backend} (FORCE=1 to drop)"));
            } else if !new.is_empty() || !lost.is_empty() {
                base["pass"][backend] = json!(passed.iter().collect::<Vec<_>>());
                let mut text = serde_json::to_string_pretty(&base).unwrap();
                text.push('\n');
                std::fs::write(&path, text).map_err(|e| e.to_string())?;
                lines.push(format!("baseline: {backend} updated ({} new, {} dropped)", new.len(), lost.len()));
            }
        } else {
            for c in &lost {
                errors.push(format!("{backend}/{c} is in baseline.json but did not pass"));
            }
            if !new.is_empty() {
                lines.push(format!("baseline: new passes on {backend}: {new:?} (record them with SVELTERS_UPDATE=1)"));
            }
        }
    }
    let summary = format!(
        "{backend}: {} pass, {} xfail, {} fail\n{}",
        results.iter().filter(|r| r.status() == "pass").count(),
        results.iter().filter(|r| r.status() == "xfail").count(),
        results.iter().filter(|r| matches!(r.status(), "fail" | "xpass")).count(),
        lines.join("\n")
    );
    if errors.is_empty() { Ok(summary) } else { Err(format!("{summary}\n\nerrors:\n  {}", errors.join("\n  "))) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(case: &str, xfail: Option<&str>, pass: bool) -> CaseResult {
        let outcome = if pass { Outcome::default() } else { Outcome::fail("line 3: no node shows \"x\"") };
        CaseResult { case: case.into(), xfail: xfail.map(str::to_string), outcome }
    }

    #[test]
    fn rules() {
        let dir = std::env::temp_dir().join(format!("svelters-testkit-ratchet-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let out = dir.join("out");
        // no baseline: passes and declared failures are fine, others are not
        assert!(report(&dir, &out, "egui", &[result("a", None, true), result("b", Some("why"), false)]).is_ok());
        assert!(report(&dir, &out, "egui", &[result("a", None, false)]).unwrap_err().contains("egui/a failed"));
        assert!(report(&dir, &out, "egui", &[result("a", Some("why"), true)]).unwrap_err().contains("remove its @xfail"));
        // a baseline entry cannot hide behind a new @xfail
        std::fs::write(dir.join("baseline.json"), r#"{"pass": {"egui": ["a"]}}"#).unwrap();
        let e = report(&dir, &out, "egui", &[result("a", Some("now broken"), false)]).unwrap_err();
        assert!(e.contains("egui/a is in baseline.json but did not pass"), "{e}");
        assert!(report(&dir, &out, "egui", &[result("a", None, true), result("c", None, true)]).unwrap().contains("new passes"));
        assert!(out.join("egui.json").is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
