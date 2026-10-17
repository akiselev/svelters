//! Differential test of `acorn::regexp` against real acorn 8.19.0 verdicts recorded in
//! `tests/regexp_corpus/corpus.json` (see `tests/regexp_corpus/README.md`).

use serde_json::Value;
use svelters_compiler::acorn::regexp::{validate_flags, validate_pattern};

/// acorn's order (readRegexp): flags first, then the pattern.
fn run(pattern: &str, flags: &str, ecma_version: u32) -> Result<(), String> {
    validate_flags(flags, ecma_version)?;
    validate_pattern(pattern, flags, ecma_version).map_err(|(message, offset)| {
        assert_eq!(offset, 0, "acorn raises regexp errors at the pattern start");
        message
    })
}

fn expected(pattern: &str, result: &Value) -> Result<(), String> {
    match result {
        Value::Number(n) if n.as_u64() == Some(0) => Ok(()),
        Value::String(s) => match s.strip_prefix(':') {
            Some(rest) => Err(format!("Invalid regular expression: /{pattern}/: {rest}")),
            None => Err(s.clone()),
        },
        other => panic!("bad corpus result {other}"),
    }
}

#[test]
fn regexp_corpus_matches_acorn() {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/regexp_corpus/corpus.json"
    ))
    .unwrap();
    let corpus: Value = serde_json::from_str(&text).unwrap();
    let versions: Vec<u32> = corpus["versions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u32)
        .collect();
    let cases = corpus["cases"].as_array().unwrap();
    let (mut total, mut failures) = (0usize, Vec::new());
    for case in cases {
        let pattern = case[0].as_str().unwrap();
        let flags = case[1].as_str().unwrap();
        for (i, &version) in versions.iter().enumerate() {
            let result = if case[2].is_array() {
                &case[2][i]
            } else {
                &case[2]
            };
            let want = expected(pattern, result);
            let got = run(pattern, flags, version);
            total += 1;
            if got != want {
                failures.push(format!(
                    "/{pattern}/{flags} ecmaVersion {version}: want {want:?}, got {got:?} ({})",
                    case[3]
                ));
            }
        }
    }
    // acorn also accepts year-style versions; spot-check that they normalize.
    assert_eq!(run("(?<a>)|(?<a>)", "", 2025), Ok(()));
    assert_eq!(run("a", "v", 2024), Ok(()));
    eprintln!(
        "regexp corpus: {} / {total} verdicts agree ({} cases)",
        total - failures.len(),
        cases.len()
    );
    assert!(
        failures.is_empty(),
        "{} disagreements:\n{}",
        failures.len(),
        failures
            .iter()
            .take(50)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
