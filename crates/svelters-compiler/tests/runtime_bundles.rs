//! The hand-written components in the generated-code shape
//! (`experiments/native-compiler/handwritten/<name>/`, built on `@svelters/native-runtime`) run
//! on the Rust log backend and must reproduce the op log the Node TestHost recorded for them
//! (`expected.oplog` in each directory). Build the bundles first:
//! `pnpm --filter @svelters-experiments/handwritten build`. Unbuilt bundles are skipped.

use std::path::PathBuf;
use svelters_host::testing::{FixtureOutcome, run_fixture};

const NAMES: &[&str] = &[
    "counter",
    "if-toggle",
    "keyed-list",
    "keyed-stress",
    "native-close",
    "timers",
];

#[test]
fn handwritten_bundles_match_on_the_log_backend() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../experiments/native-compiler/handwritten");
    let mut failures = Vec::new();
    let mut ran = 0;
    for name in NAMES {
        match run_fixture(&root.join(name)) {
            FixtureOutcome::Skipped(why) => {
                eprintln!(
                    "skipping {name}: {why} (run `pnpm --filter @svelters-experiments/handwritten build`)"
                )
            }
            FixtureOutcome::Pass => ran += 1,
            FixtureOutcome::Fail(why) => failures.push(format!("{name}: {why}")),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
    eprintln!("{ran}/{} hand-written bundles matched", NAMES.len());
}
