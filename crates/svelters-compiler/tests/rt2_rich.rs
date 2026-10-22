//! RT2: inline rich text on egui (WIDGET-PARITY Q4; plans/milestones/RT2-rich-text.md). A
//! `label`'s text mixed with run elements (`span`, `b`, `i`, `code`, `a`) lowers to run nodes, its
//! bare text to anonymous `span`s (spec `inline_text`); `{expressions}` stay reactive. Golden
//! cases: `tests/rt2/<case>.txt` holds the input, the diagnostics and the module.
//! `UPDATE=1 cargo test -p svelters-compiler --test rt2_rich` rewrites them (review the diff!).

use std::fs;
use std::path::Path;
use svelters_compiler::compile::{CompileOptions, compile};
use svelters_compiler::semantics::SpecBackend;

const CASES: &[(&str, &str)] = &[
    (
        "mixed",
        r#"<script>let name = $state("Ada"); function go() {}</script>
<window><box><label>Hello <b>{name}</b>, see <a href="https://svelters.dev" onclicked={go}>the docs</a>.</label></box></window>"#,
    ),
    (
        "nested-scoped",
        r#"<window><box><label><b>bold <i>both</i></b> and <code class="k">x = 1</code></label></box></window>
<style>.k { color: red; } label span { font-weight: 600; }</style>"#,
    ),
    ("text-only", r#"<script>let n = $state(1);</script><window><box><label>plain {n}</label></box></window>"#),
    (
        "blocks",
        r#"<script>let on = $state(true); let xs = $state(["a", "b"]);</script>
<window><box><label>{#if on}<b>on</b>{:else}off{/if}: {#each xs as x}<code>{x}</code> {/each}</label></box></window>"#,
    ),
    ("not-a-run", r#"<window><box><label>Go <button>x</button></label></box></window>"#),
    ("button-stays-text", r#"<window><box><button>Go <b>x</b></button></box></window>"#),
];

fn render(src: &str) -> String {
    let b = SpecBackend::builtin("egui").unwrap();
    let res = compile(src, &CompileOptions::new("App.svelte", &b));
    let mut out = format!("## input\n{src}\n\n## diagnostics\n");
    for d in &res.diagnostics {
        out.push_str(&format!("{:?}[{}] @{}:{}: {}\n", d.severity, d.code, d.line, d.column, d.message));
    }
    if let Some(js) = &res.js {
        out.push_str("\n## js\n");
        out.push_str(js);
    }
    out
}

#[test]
fn rt2_golden() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/rt2");
    let mut bad = Vec::new();
    for (name, src) in CASES {
        let got = render(src);
        let path = dir.join(format!("{name}.txt"));
        if std::env::var("UPDATE").as_deref() == Ok("1") {
            fs::write(&path, &got).unwrap();
            continue;
        }
        if fs::read_to_string(&path).ok().as_deref() != Some(got.as_str()) {
            bad.push(format!("{name}:\n{got}"));
        }
    }
    assert!(bad.is_empty(), "stale expectations (UPDATE=1 rewrites them):\n{}", bad.join("\n"));
}
