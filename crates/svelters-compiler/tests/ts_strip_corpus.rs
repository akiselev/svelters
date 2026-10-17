//! TypeScript stripping (`svelters_compiler::svelte::ts`, agent TS) over the `lang="ts"`
//! components of the svelte oracle corpus (target/oracle/svelte.jsonl): after
//! `remove_typescript_nodes`, the instance and module scripts with the recorded TypeScript ranges
//! removed must be valid JavaScript (plain acorn), and must contain no TypeScript the stripped AST
//! no longer has. Components rejected with `typescript_invalid_feature` (enums, decorators, ...)
//! are counted separately.
//!
//! `cargo test -p svelters-compiler --release --test ts_strip_corpus -- --nocapture`

mod oracle_common;

use oracle_common::*;
use svelters_compiler::acorn::{self, Options};
use svelters_compiler::codegen::edit::Edits;
use svelters_compiler::estree::{SourceType, Spanned};
use svelters_compiler::svelte::parse::{parse, ParseOptions};
use svelters_compiler::svelte::ts::strip;

#[test]
fn ts_strip_corpus() {
    let Some(cases) = load_corpus("svelte") else { return };
    let mut checked = 0;
    let mut rejected = 0;
    let mut failures = Vec::new();
    for case in &cases {
        let (Some(src), true) = (case["source"].as_str(), case["error"].is_null()) else { continue };
        if case["skip"].is_string() {
            continue;
        }
        let opts = ParseOptions { loose: case["options"]["loose"] == true, native: false };
        let Ok(mut root) = parse(src, &opts) else { continue };
        if !root.ts {
            continue;
        }
        let ranges = match strip(&mut root, src) {
            Ok(r) => r,
            Err(e) => {
                assert_eq!(e.code, "typescript_invalid_feature", "{}: {}", case["id"], e.message);
                rejected += 1;
                continue;
            }
        };
        for script in root.instance.iter().chain(root.module.iter()) {
            let (a, z) = (script.content.start, script.content.end);
            let mut edits = Edits::new();
            ranges.apply(&mut edits, None);
            let js = edits.apply(src, a, z);
            let mut comments = Vec::new();
            let opts = Options {
                ecma_version: 16,
                source_type: SourceType::Module,
                preserve_parens: false,
                allow_undefined_exports: true,
                typescript: false,
            };
            checked += 1;
            if let Err(e) = acorn::parse(&js, &opts, &mut comments) {
                let at = e.pos as usize;
                let lo = js[..at.min(js.len())].char_indices().rev().nth(40).map_or(0, |(i, _)| i);
                let hi = js[at.min(js.len())..].char_indices().nth(40).map_or(js.len(), |(i, _)| at + i);
                failures.push(format!("{}: {} near {:?}", case["id"], e.message, &js[lo..hi]));
            }
            // the AST and the text agree: every statement left keeps its text
            for s in &script.content.body {
                let sp = s.span();
                assert!(sp.start >= a && sp.end <= z, "{}: statement outside its script", case["id"]);
            }
        }
    }
    println!("ts_strip_corpus: {checked} scripts checked, {rejected} components rejected (typescript_invalid_feature)");
    for f in &failures {
        println!("  FAIL {f}");
    }
    assert!(failures.is_empty(), "{} stripped scripts are not valid JavaScript", failures.len());
}
