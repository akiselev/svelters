//! CX1: the diagnostic code registry (`src/codes.rs`) covers every code the sources report, and
//! the generated documentation page is up to date (`UPDATE=1` rewrites it).

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use svelters_compiler::codes;

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            sources(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

fn is_code(s: &str) -> bool {
    s.contains('_') && s.starts_with(|c: char| c.is_ascii_lowercase()) && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// The string literals of `src` that are written where a diagnostic code goes: the first argument
/// of `error(`/`warning(`/`note(`/`native_error(`/`native_warning(`/`Unresolved::new(`, the second
/// of the Svelte port's `e(node, ...)`/`w(node, ...)`, `code: "..."`, and `const X: &str = "..."`.
fn codes_in(src: &str) -> Vec<String> {
    const BEFORE: &[&str] = &["error(", "warning(", "note(", "native_error(", "native_warning(", "Unresolved::new(", "code:", "str =", "node,", "span,"];
    let mut out = Vec::new();
    for (start, end) in string_literals(src) {
        let lit = &src[start..end];
        if is_code(lit) && BEFORE.iter().any(|k| src[..start - 1].trim_end().ends_with(k)) {
            out.push(lit.to_string());
        }
    }
    out
}

/// The contents of the plain string literals of Rust source `src` (byte ranges inside the quotes),
/// skipping comments, char literals and raw strings.
fn string_literals(src: &str) -> Vec<(usize, usize)> {
    let b = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'/' if b.get(i + 1) == Some(&b'/') => i = src[i..].find('\n').map_or(b.len(), |n| i + n),
            b'/' if b.get(i + 1) == Some(&b'*') => i = src[i + 2..].find("*/").map_or(b.len(), |n| i + n + 4),
            b'r' if (b.get(i + 1) == Some(&b'"') || b.get(i + 1) == Some(&b'#')) && (i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_')) => {
                let hashes = b[i + 1..].iter().take_while(|&&c| c == b'#').count();
                if b.get(i + 1 + hashes) != Some(&b'"') {
                    i += 1;
                    continue;
                }
                let close = format!("\"{}", "#".repeat(hashes));
                let from = i + 2 + hashes;
                i = src[from..].find(&close).map_or(b.len(), |n| from + n + close.len());
            }
            b'\'' => {
                // a char literal ('x', '\n', '"'), else a lifetime
                if b.get(i + 1) == Some(&b'\\') {
                    i = src[i + 2..].find('\'').map_or(b.len(), |n| i + n + 3);
                } else {
                    let ch = src[i + 1..].chars().next().map_or(1, char::len_utf8);
                    i += if b.get(i + 1 + ch) == Some(&b'\'') { ch + 2 } else { 1 };
                }
            }
            b'"' => {
                let start = i + 1;
                let mut j = start;
                while j < b.len() && b[j] != b'"' {
                    j += if b[j] == b'\\' { 2 } else { 1 };
                }
                out.push((start, j.min(b.len())));
                i = j + 1;
            }
            _ => i += 1,
        }
    }
    out
}

#[test]
fn every_reported_code_is_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    sources(&root, &mut files);
    let mut found = BTreeSet::new();
    for f in files.iter().filter(|f| !f.ends_with("codes.rs")) {
        let text = fs::read_to_string(f).unwrap();
        // unit tests may use made-up codes
        let text = text.split("#[cfg(test)]\nmod tests {").next().unwrap();
        found.extend(codes_in(text));
    }
    assert!(found.len() > 140, "the scan found only {} codes: is it broken? {found:?}", found.len());
    let missing: Vec<_> = found.iter().filter(|c| codes::lookup(c).is_none()).collect();
    assert!(
        missing.is_empty(),
        "codes reported in crates/svelters-compiler/src but not registered in src/codes.rs (add a row with a stable id, then UPDATE=1 for the docs page): {missing:?}"
    );
    let unused: Vec<_> = codes::all().iter().filter(|c| !found.contains(c.code)).map(|c| c.code).collect();
    assert!(unused.is_empty(), "registered codes the scan does not find in the sources (mark them as no longer reported, never delete an id): {unused:?}");
}

#[test]
fn docs_page_is_generated_from_the_registry() {
    let page = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/site/src/content/docs/guides/diagnostics.md");
    let want = codes::docs_markdown();
    if std::env::var_os("UPDATE").is_some() {
        fs::write(&page, &want).unwrap();
        return;
    }
    let have = fs::read_to_string(&page).unwrap_or_default();
    assert!(have == want, "{} is out of date: run `UPDATE=1 cargo nextest run -p svelters-compiler --test cx1_codes`", page.display());
    for c in codes::all() {
        assert!(want.contains(&format!("`{}`", c.id)), "{} missing from the page", c.id);
    }
}

#[test]
fn explain_names_the_id_and_the_docs() {
    let e = codes::explain("bind_partially_observed").unwrap();
    assert!(e.starts_with("svelters::bind::partially_observed (svelters)\n"), "{e}");
    assert!(e.contains("severity: warning"), "{e}");
    let e = codes::explain("svelte::expected_token").unwrap();
    assert!(e.contains("docs: https://svelte.dev/e/expected_token"), "{e}");
    assert!(codes::explain("no_such_code").is_none());
}
