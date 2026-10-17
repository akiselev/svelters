//! Runs every `fixtures/*/` that has `dist/app.js`, `script.json` and `expected.oplog` against the
//! log backend and compares the op log byte for byte. A fixture with a missing file is skipped
//! (build with `pnpm install && pnpm -r build`).
//!
//! Set `SVELTERS_REQUIRE_FIXTURES=1` to turn skips into failures (P5/CI).

use svelters_host::testing::{FixtureOutcome, default_fixtures_root, find_fixtures, run_fixture};

#[test]
fn p1_fixtures_match_expected_oplog() {
    let require = std::env::var_os("SVELTERS_REQUIRE_FIXTURES").is_some();
    let root = default_fixtures_root();
    let mut failures = Vec::new();
    let (mut passed, mut skipped) = (0, 0);
    for dir in find_fixtures(&root) {
        let name = dir.file_name().unwrap().to_string_lossy().into_owned();
        match run_fixture(&dir) {
            FixtureOutcome::Pass => {
                passed += 1;
                eprintln!("fixture {name}: ok");
            }
            FixtureOutcome::Skipped(why) => {
                skipped += 1;
                eprintln!("fixture {name}: skipped ({why})");
                if require {
                    failures.push(format!(
                        "{name}: skipped but SVELTERS_REQUIRE_FIXTURES is set ({why})"
                    ));
                }
            }
            FixtureOutcome::Fail(msg) => failures.push(format!("{name}: {msg}")),
        }
    }
    eprintln!("fixtures: {passed} passed, {skipped} skipped");
    assert!(failures.is_empty(), "\n{}", failures.join("\n\n"));
}
