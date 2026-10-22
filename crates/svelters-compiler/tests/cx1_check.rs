//! CX1: `check` (diagnostics only, incremental state per file, fix-its, stable ids).
//!
//! - The diagnostics-only pipeline reports exactly what `compile` reports, over the compiler's test
//!   inputs and the repo's fixtures.
//! - Every fix-it, applied, removes its diagnostic and adds no other.
//! - `FileState` answers an unchanged input from the previous result and carries the last
//!   complete diagnostics over a syntax error.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use svelters_compiler::check::{CheckDiagnostic, CheckResult, FileKind, FileState, Fix, Reuse, check};
use svelters_compiler::compile::{CompileOptions, check_component, compile};
use svelters_compiler::diag::Severity;
use svelters_compiler::semantics::SpecBackend;

fn backend(name: &str) -> &'static SpecBackend {
    static B: OnceLock<Vec<(String, SpecBackend)>> = OnceLock::new();
    let all = B.get_or_init(|| ["gtk", "qt", "appkit", "winui", "egui"].iter().map(|b| (b.to_string(), SpecBackend::builtin(b).unwrap())).collect());
    &all.iter().find(|(n, _)| n == name).unwrap().1
}

fn gtk() -> &'static SpecBackend {
    backend("gtk")
}

fn apply(src: &str, fix: &Fix) -> String {
    let mut out = String::new();
    let mut cur = 0usize;
    for e in &fix.edits {
        out.push_str(&src[cur..e.range.start as usize]);
        out.push_str(&e.text);
        cur = e.range.end as usize;
    }
    out.push_str(&src[cur..]);
    out
}

fn count(r: &CheckResult, code: &str) -> usize {
    r.diagnostics.iter().filter(|d| d.code == code).count()
}

fn find<'r>(r: &'r CheckResult, code: &str) -> &'r CheckDiagnostic {
    r.diagnostics.iter().find(|d| d.code == code).unwrap_or_else(|| panic!("no {code} in {:#?}", r.diagnostics))
}

/// Applies each fix-it of each diagnostic of `src` and checks again: the diagnostic's code is
/// reported fewer times, and no code more often than before. Returns the number of fix-its.
fn assert_fixes_resolve(path: &str, src: &str, b: &SpecBackend) -> usize {
    let before = check(path, src, b, &mut FileState::new());
    let mut n = 0;
    for d in &before.diagnostics {
        for f in &d.fixes {
            n += 1;
            let fixed = apply(src, f);
            let after = check(path, &fixed, b, &mut FileState::new());
            assert!(count(&after, d.code) < count(&before, d.code), "fix-it {:?} of {} did not remove it:\n{fixed}\n{:#?}", f.message, d.id, after.diagnostics);
            // the first fix-it is the likely one: it adds nothing (later "did you mean"
            // alternatives may name an element that does not fit there)
            for a in after.diagnostics.iter().filter(|_| std::ptr::eq(f, &d.fixes[0])) {
                assert!(
                    count(&after, a.code) <= count(&before, a.code),
                    "fix-it {:?} of {} adds {}: {}\n{fixed}",
                    f.message,
                    d.id,
                    a.id,
                    a.message
                );
            }
        }
    }
    n
}

#[test]
fn did_you_mean_fixes_rename_the_name() {
    let src = "<window>\n  <box orientation=\"vertical\">\n    <buton label=\"x\"></buton>\n    <label labl=\"x\" />\n    <button onclickd={() => {}} label=\"ok\" />\n  </box>\n</window>\n";
    let r = check("App.svelte", src, gtk(), &mut FileState::new());
    let el = find(&r, "unknown_element");
    assert_eq!(el.id, "svelters::element::unknown");
    let f = el.fixes.iter().find(|f| f.message == "replace with `button`").unwrap_or_else(|| panic!("{:#?}", el.fixes));
    assert_eq!(f.edits.len(), 2, "start and end tag: {f:#?}");
    assert!(apply(src, f).contains("<button label=\"x\"></button>"));
    let p = find(&r, "unknown_property");
    assert!(p.fixes.iter().any(|f| apply(src, f).contains("<label label=\"x\" />")), "{p:#?}");
    let e = find(&r, "unknown_event");
    assert!(e.fixes.iter().any(|f| apply(src, f).contains("onclicked={() => {}}")), "{e:#?}");
    assert!(assert_fixes_resolve("App.svelte", src, gtk()) >= 3);
}

#[test]
fn bind_and_modifier_fixes_resolve() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/r2");
    let src = fs::read_to_string(dir.join("diagnostics-gtk.svelte")).unwrap();
    let r = check("diagnostics-gtk.svelte", &src, gtk(), &mut FileState::new());
    // set one way: `bind:css-name={s}` (constant), `bind:title={s}` (set-only), `|nonreactive` redundant
    let one_way: Vec<String> = r.diagnostics.iter().flat_map(|d| d.fixes.iter().map(|f| f.message.clone())).filter(|m| m.ends_with("one way")).collect();
    assert_eq!(one_way, ["set `css-name` one way", "set `title` one way", "set `title` one way"]);
    let m = r.diagnostics.iter().filter(|d| d.code == "bind_modifier_invalid").map(|d| d.fixes.len()).collect::<Vec<_>>();
    assert_eq!(m, [1, 1, 1, 1], "{:#?}", r.diagnostics);
    let fixed = apply(&src, &find(&r, "bind_set_only").fixes[0]);
    assert!(fixed.contains("<header-bar title={s} />"), "{fixed}");
    assert!(assert_fixes_resolve("diagnostics-gtk.svelte", &src, gtk()) >= 7);
}

#[test]
fn event_modifier_fixes_are_exact_or_absent() {
    let src = "<script>\n  function h() {}\n</script>\n<box>\n  <button on:clicked|preventDefault={h} />\n  <button on:clicked|stopPropagation={() => h()} />\n  <button on:clicked|preventDefault|once={h} />\n  <button on:clicked|stopPropagation />\n</box>\n";
    let r = check("App.svelte", src, gtk(), &mut FileState::new());
    let fixes: Vec<String> = r.diagnostics.iter().filter(|d| d.code == "unsupported_feature").map(|d| d.fixes.first().map(|f| apply(src, f)).unwrap_or_default()).collect();
    assert_eq!(fixes.len(), 4);
    assert!(fixes[0].contains("<button onclicked={(event) => { event.preventDefault(); h(event); }} />"), "{}", fixes[0]);
    assert!(fixes[1].contains("<button onclicked={(event) => { event.stopPropagation(); (() => h())(event); }} />"), "{}", fixes[1]);
    // `|once` is left to the author, and a bare `on:` needs the prop destructured: no fix-it
    assert_eq!(fixes[2], "");
    assert_eq!(fixes[3], "");
    assert_eq!(assert_fixes_resolve("App.svelte", src, gtk()), 2);
}

#[test]
fn style_and_syntax_fixes_resolve() {
    let src = "<label class=\"a\" label=\"x\" />\n<style>\n  .a { margin-start: 4 !important; }\n</style>\n";
    let r = check("App.svelte", src, gtk(), &mut FileState::new());
    let d = find(&r, "style_unsupported_important");
    assert_eq!(apply(src, &d.fixes[0]), src.replace(" !important", ""));
    assert_eq!(assert_fixes_resolve("App.svelte", src, gtk()), 1);

    let src = "<script>let a = $state(1), b = $state(2);</script>\n{#if a}<label label=\"a\" />{:elseif b}<label label=\"b\" />{/if}\n";
    let r = check("App.svelte", src, gtk(), &mut FileState::new());
    let d = find(&r, "block_invalid_elseif");
    assert_eq!(d.id, "svelte::block_invalid_elseif");
    assert!(apply(src, &d.fixes[0]).contains("{:else if b}"));
}

#[test]
fn positions_are_bytes_and_utf16() {
    // `é` is 2 bytes and 1 UTF-16 unit, `𝒳` 4 bytes and 2 units
    let src = "<label label=\"é𝒳\" />\n<buton />\n";
    let r = check("App.svelte", src, gtk(), &mut FileState::new());
    let d = find(&r, "unknown_element");
    assert_eq!(d.range.start as usize, src.find("buton").unwrap());
    assert_eq!((d.range.start_pos.line, d.range.start_pos.column, d.range.start_pos.utf16_column), (2, 2, 2));
    assert_eq!((d.range.end_pos.line, d.range.end_pos.column), (2, 7));
    let src = "<box>é𝒳<buton /></box>\n";
    let r = check("App.svelte", src, gtk(), &mut FileState::new());
    let d = find(&r, "unknown_element");
    assert_eq!((d.range.start_pos.column, d.range.start_pos.utf16_column), (13, 10));
}

#[test]
fn unchanged_input_is_answered_from_the_state() {
    let src = "<label labl=\"x\" />\n";
    let mut st = FileState::new();
    let a = check("App.svelte", src, gtk(), &mut st);
    assert_eq!(a.reuse, Reuse::None);
    let b = check("App.svelte", src, gtk(), &mut st);
    assert_eq!(b.reuse, Reuse::Unchanged);
    assert_eq!(a.diagnostics, b.diagnostics);
    // another backend, path or text runs again
    assert_eq!(check("App.svelte", src, backend("qt"), &mut st).reuse, Reuse::None);
    assert_eq!(check("App.svelte", src, gtk(), &mut st).reuse, Reuse::None);
    assert_eq!(check("Other.svelte", src, gtk(), &mut st).reuse, Reuse::None);
    assert_eq!(check("Other.svelte", "<label />\n", gtk(), &mut st).reuse, Reuse::None);
    let s = st.stats();
    assert_eq!((s.checks, s.runs, s.unchanged), (6, 5, 1));
}

#[test]
fn a_syntax_error_carries_the_last_complete_diagnostics_over() {
    let src = "<window>\n  <box>\n    <label labl=\"x\" />\n  </box>\n  <buton />\n</window>\n";
    let mut st = FileState::new();
    let first = check("App.svelte", src, gtk(), &mut st);
    assert_eq!(first.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>(), ["unknown_property", "unknown_element"]);
    // typing an unfinished expression before the label: a syntax error, the old diagnostics shift
    let typing = src.replacen("<box>\n", "<box>\n    {1 + }\n", 1);
    let r = check("App.svelte", &typing, gtk(), &mut st);
    assert_eq!(r.reuse, Reuse::CarriedOver);
    let fresh: Vec<_> = r.diagnostics.iter().filter(|d| !d.stale).collect();
    assert_eq!(fresh.len(), 1, "{:#?}", r.diagnostics);
    assert_eq!((fresh[0].id, fresh[0].severity), ("svelte::js_parse_error", Severity::Error));
    let stale: Vec<_> = r.diagnostics.iter().filter(|d| d.stale).collect();
    assert_eq!(stale.iter().map(|d| d.code).collect::<Vec<_>>(), ["unknown_property", "unknown_element"]);
    assert_eq!(stale[0].range.start as usize, typing.find("labl").unwrap());
    assert_eq!(stale[0].range.start_pos.line, 4);
    assert_eq!(stale[1].range.start as usize, typing.find("buton").unwrap());
    assert!(stale.iter().all(|d| d.fixes.is_empty()));
    // an edit over a diagnostic drops it
    let over = typing.replacen("labl", "lab<", 1);
    let r = check("App.svelte", &over, gtk(), &mut st);
    assert!(r.diagnostics.iter().filter(|d| d.stale).all(|d| d.code != "unknown_property"), "{:#?}", r.diagnostics);
    // fixed: fresh diagnostics again
    let r = check("App.svelte", &src.replace("labl", "label"), gtk(), &mut st);
    assert_eq!(r.reuse, Reuse::None);
    assert_eq!(r.diagnostics.iter().map(|d| (d.code, d.stale)).collect::<Vec<_>>(), [("unknown_element", false)]);
    assert_eq!(st.stats().carried_over, 2);
}

#[test]
fn modules_and_other_files() {
    let r = check("store.svelte.js", "export const x = $state(0);\nx = 1;\n", gtk(), &mut FileState::new());
    assert_eq!(r.kind, FileKind::Module);
    assert!(r.diagnostics.iter().all(|d| d.id.starts_with("svelte::")), "{:#?}", r.diagnostics);
    let r = check("util.js", "this is not checked", gtk(), &mut FileState::new());
    assert_eq!(r.kind, FileKind::Other);
    assert!(r.diagnostics.is_empty());
}

#[test]
fn json_has_ids_ranges_and_fixes() {
    let r = check("App.svelte", "<label labl=\"x\" />\n", gtk(), &mut FileState::new());
    let v = serde_json::to_value(&r).unwrap();
    let d = &v["diagnostics"][0];
    assert_eq!(d["id"], "svelters::property::unknown");
    assert_eq!(d["code"], "unknown_property");
    assert_eq!(d["severity"], "error");
    assert_eq!(d["range"]["start"], 7);
    assert_eq!(d["range"]["start_pos"]["line"], 1);
    assert_eq!(d["fixes"][0]["edits"][0]["text"], "label");
    assert!(d.get("stale").is_none());
    assert_eq!(v["reuse"], "none");
}

/// The compiler's test inputs and the repo's fixtures, with the backend each is written for.
fn corpus() -> Vec<(PathBuf, &'static str)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut out = Vec::new();
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = fs::read_dir(dir) else { return };
        for e in rd {
            let p = e.unwrap().path();
            if p.is_dir() {
                if !p.ends_with("node_modules") {
                    walk(&p, out);
                }
            } else if p.to_str().is_some_and(|s| s.ends_with(".svelte") || s.ends_with(".svelte.js")) {
                out.push(p);
            }
        }
    }
    let mut files = Vec::new();
    for d in ["tests/fw1", "tests/r2", "tests/runes", "../../fixtures", "../../examples"] {
        walk(&root.join(d), &mut files);
    }
    files.sort();
    for f in files {
        let s = f.to_string_lossy().to_string();
        let backend_file = f.parent().unwrap().join("backend");
        let b = if let Ok(b) = fs::read_to_string(&backend_file) {
            match b.trim() {
                "qt" => "qt",
                "appkit" => "appkit",
                "winui" => "winui",
                "egui" => "egui",
                _ => "gtk",
            }
        } else if s.contains("-qt") || s.contains("qt-") {
            "qt"
        } else if s.contains("appkit") || s.contains("-mac") {
            "appkit"
        } else if s.contains("winui") || s.contains("-win/") {
            "winui"
        } else if s.contains("egui") {
            "egui"
        } else {
            "gtk"
        };
        out.push((f, b));
    }
    out
}

#[test]
fn check_reports_what_compile_reports() {
    let corpus = corpus();
    assert!(corpus.len() > 60, "{}", corpus.len());
    let mut with_diags = 0;
    for (path, b) in &corpus {
        let src = fs::read_to_string(path).unwrap();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if name.ends_with(".svelte.js") {
            continue;
        }
        let opts = CompileOptions::new(name.clone(), backend(b));
        let full = compile(&src, &opts);
        let only = check_component(&src, &opts);
        assert!(only.js.is_none() && only.map.is_none());
        let key = |r: &svelters_compiler::compile::CompileResult| {
            r.diagnostics.iter().map(|d| (d.code, d.start, d.end, d.severity, d.message.clone(), d.help.clone(), d.fixes.clone())).collect::<Vec<_>>()
        };
        assert_eq!(key(&full), key(&only), "{}", path.display());
        if !full.diagnostics.is_empty() {
            with_diags += 1;
        }
        let r = check(&name, &src, backend(b), &mut FileState::new());
        assert_eq!(r.diagnostics.len(), full.diagnostics.len(), "{}", path.display());
        assert!(r.diagnostics.iter().all(|d| !d.id.starts_with("svelters::unregistered")));
    }
    assert!(with_diags > 10, "{with_diags}");
}

#[test]
fn fixes_in_the_corpus_resolve() {
    let mut n = 0;
    for (path, b) in corpus() {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if name.ends_with(".svelte") {
            n += assert_fixes_resolve(&name, &fs::read_to_string(&path).unwrap(), backend(b));
        }
    }
    assert!(n >= 8, "{n}");
}
