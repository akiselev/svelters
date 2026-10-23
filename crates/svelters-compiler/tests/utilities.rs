//! U4 (plans/research/UTILITY-CSS.md §4.6, §4.7, §7): the compiler's utility-class checker and
//! `@apply`.
//!
//! - each `utility_*` code fires on a minimal component, and each fix-it, applied, clears it;
//! - `checked:` on `<box>` warns and on `<checkbox>` doesn't (per-kind states, parts, provider
//!   states and properties);
//! - static `class`, `class:` names, clsx literals and (opt-in) `<script>` strings are checked;
//! - `@apply hover:bg-list-hover` scopes like the hand-written CSS;
//! - the egui fixtures have no utility diagnostics, and the checker is off by default.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use svelters_compiler::check::{CheckOptions, CheckResult, FileState, Fix, check_with};
use svelters_compiler::compile::{CompileOptions, compile};
use svelters_compiler::diag::Severity;
use svelters_compiler::semantics::SpecBackend;
use svelters_compiler::utilities::UtilityCheck;

fn egui() -> &'static SpecBackend {
    static B: OnceLock<SpecBackend> = OnceLock::new();
    B.get_or_init(|| SpecBackend::builtin("egui").unwrap())
}

fn on() -> UtilityCheck {
    UtilityCheck::default()
}

fn run_with(src: &str, u: Option<UtilityCheck>) -> CheckResult {
    let opts = CheckOptions { utilities: u, ..Default::default() };
    check_with("App.svelte", src, egui(), &opts, &mut FileState::new())
}

fn run(src: &str) -> CheckResult {
    run_with(src, Some(on()))
}

fn utility_codes(r: &CheckResult) -> Vec<(&'static str, String)> {
    r.diagnostics.iter().filter(|d| d.code.starts_with("utility_") || d.code == "expected_token").map(|d| (d.code, d.message.clone())).collect()
}

fn codes(r: &CheckResult) -> Vec<&'static str> {
    utility_codes(r).into_iter().map(|c| c.0).collect()
}

fn count(r: &CheckResult, code: &str) -> usize {
    r.diagnostics.iter().filter(|d| d.code == code).count()
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

/// Applies each fix-it of each `utility_*` (and `expected_token`) diagnostic and checks again: its
/// code is reported fewer times, and the first fix-it adds nothing. Returns the fix-it messages.
fn assert_fixes_resolve(src: &str, u: Option<UtilityCheck>) -> Vec<String> {
    let before = run_with(src, u.clone());
    let mut out = Vec::new();
    for d in before.diagnostics.iter().filter(|d| d.code.starts_with("utility_") || d.code == "expected_token") {
        for f in &d.fixes {
            out.push(f.message.clone());
            let fixed = apply(src, f);
            let after = run_with(&fixed, u.clone());
            assert!(count(&after, d.code) < count(&before, d.code), "fix-it {:?} of {} did not remove it:\n{fixed}\n{:#?}", f.message, d.code, utility_codes(&after));
            if std::ptr::eq(f, &d.fixes[0]) {
                for a in &after.diagnostics {
                    assert!(count(&after, a.code) <= count(&before, a.code), "fix-it {:?} of {} adds {}: {}\n{fixed}", f.message, d.code, a.code, a.message);
                }
            }
        }
    }
    out
}

#[test]
fn unknown_values_and_variants_suggest() {
    let src = r#"<box class="bg-acent hovr:p-1 row my-widget" />"#;
    let r = run(src);
    assert_eq!(codes(&r), ["utility_unknown", "utility_unknown"], "{:#?}", utility_codes(&r));
    let d = &r.diagnostics[0];
    assert_eq!(d.id, "svelters::utility::unknown");
    assert_eq!(&src[d.range.start as usize..d.range.end as usize], "bg-acent");
    let fixes = assert_fixes_resolve(src, Some(on()));
    assert!(fixes.contains(&"replace with `bg-accent`".to_string()), "{fixes:?}");
    assert!(fixes.contains(&"replace `hovr:` with `hover:`".to_string()), "{fixes:?}");
}

#[test]
fn unsupported_names_offer_the_replacement() {
    let src = r#"<box class="ring-2 forced-colors:p-1 block" />"#;
    let r = run(src);
    // `block` without a prefix is as likely a semantic class
    assert_eq!(codes(&r), ["utility_unsupported", "utility_unsupported"], "{:#?}", utility_codes(&r));
    let fixes = assert_fixes_resolve(src, Some(on()));
    assert!(fixes.contains(&"replace `ring` with `outline`".to_string()), "{fixes:?}");
    assert!(fixes.contains(&"replace `forced-colors:` with `contrast-more:`".to_string()), "{fixes:?}");

    // with a prefix, only prefixed classes are utilities, and a bare unsupported one is reported
    let tw = UtilityCheck { prefix: Some("tw".into()), ..Default::default() };
    let src = r#"<box class="tw:block bg-acent tw:bg-acent" />"#;
    let r = run_with(src, Some(tw.clone()));
    assert_eq!(codes(&r), ["utility_unsupported", "utility_unknown"], "{:#?}", utility_codes(&r));
    let fixes = assert_fixes_resolve(src, Some(tw));
    assert!(fixes.contains(&"replace with `flex`".to_string()), "{fixes:?}");
}

#[test]
fn bad_values_and_colour_literals() {
    let src = r#"<box class="w-[red] bg-[#ff0000] p-[13px]" />"#;
    let r = run(src);
    assert_eq!(codes(&r), ["utility_bad_value", "utility_arbitrary_color"], "{:#?}", utility_codes(&r));
    assert_eq!(r.diagnostics.iter().find(|d| d.code == "utility_bad_value").unwrap().severity, Severity::Error);
    assert_eq!(r.diagnostics.iter().find(|d| d.code == "utility_arbitrary_color").unwrap().severity, Severity::Warning);
    let strict = UtilityCheck { strict_tokens: true, ..Default::default() };
    let r = run_with(src, Some(strict));
    assert_eq!(r.diagnostics.iter().find(|d| d.code == "utility_arbitrary_color").unwrap().severity, Severity::Error);
}

#[test]
fn aliases_are_notes_with_the_canonical_spelling() {
    let src = r#"<checkbox class="aria-checked:bg-accent [&::part(indicator)]:bg-accent" />"#;
    let r = run(src);
    assert_eq!(codes(&r), ["utility_alias", "utility_alias"], "{:#?}", utility_codes(&r));
    assert!(r.diagnostics.iter().all(|d| d.severity == Severity::Note));
    let fixes = assert_fixes_resolve(src, Some(on()));
    assert_eq!(fixes, ["replace `aria-checked:` with `checked:`", "replace with `part-indicator:`"]);
}

#[test]
fn variants_are_checked_per_kind() {
    // a state the kind never sets
    let r = run(r#"<box class="checked:bg-accent hover:bg-accent focus-visible:bg-accent" />"#);
    assert_eq!(utility_codes(&r).len(), 2, "{:#?}", utility_codes(&r));
    assert!(utility_codes(&r).iter().all(|c| c.0 == "utility_variant_never_matches"));
    assert!(utility_codes(&r)[0].1.contains("`<box>` never has the `checked` state"), "{:#?}", utility_codes(&r));
    // `focus-visible` is handled like `focus`
    assert!(codes(&run(r#"<checkbox class="checked:bg-accent focus-visible:bg-accent" />"#)).is_empty());

    // parts: a part the kind lacks, the undeclared scroll and row parts, a state after a paint part
    let src = r#"<button class="part-labl:text-accent part-thumb-y:bg-accent part-label:hover:text-accent part-icon:checked:text-accent" />"#;
    let r = run(src);
    assert_eq!(codes(&r), ["utility_variant_never_matches", "utility_variant_never_matches"], "{:#?}", utility_codes(&r));
    assert!(utility_codes(&r)[0].1.contains("no part `labl`"), "{:#?}", utility_codes(&r));
    assert!(utility_codes(&r)[1].1.contains("`checked`"), "{:#?}", utility_codes(&r));
    let fixes = assert_fixes_resolve(src, Some(on()));
    assert!(fixes.contains(&"replace with `part-label:`".to_string()), "{fixes:?}");
    assert!(codes(&run(r#"<box class="part-thumb:bg-accent" />"#)) == ["utility_variant_never_matches"]);

    // provider states
    assert_eq!(codes(&run(r#"<box class="state-[sv.overflow-top]:bg-accent" />"#)), ["utility_variant_never_matches"]);
    assert!(codes(&run(r#"<scroll-area class="state-[sv.overflow-top]:bg-accent state-[cad.sketch]:bg-accent" />"#)).is_empty());

    // a property the kind doesn't take, unless the rule styles a part
    let r = run(r#"<button class="gap-1 part-label:gap-1" /><box class="gap-1" />"#);
    assert_eq!(codes(&r), ["utility_not_applicable"], "{:#?}", utility_codes(&r));
}

#[test]
fn conflicts_in_one_static_list() {
    let src = r#"<box class="p-1 hover:p-1 p-2 px-1 hover:p-2!" />"#;
    let r = run(src);
    assert_eq!(utility_codes(&r).len(), 1, "{:#?}", utility_codes(&r));
    let d = r.diagnostics.iter().find(|d| d.code == "utility_conflict").unwrap();
    assert_eq!(&src[d.range.start as usize..d.range.end as usize], "p-1");
    assert!(d.message.contains("`p-2` wins"), "{}", d.message);
    let fixes = assert_fixes_resolve(src, Some(on()));
    assert_eq!(fixes, ["remove `p-1`"]);
    assert_eq!(apply(src, &d.fixes[0]), r#"<box class="hover:p-1 p-2 px-1 hover:p-2!" />"#);
}

#[test]
fn interpolations_inside_a_utility_are_dynamic_values() {
    let r = run(r#"<script>let n = $state(1); let x = $state('a');</script><box class="p-{n} w-[{n}px] row {x} bg-acent{x}" />"#);
    assert_eq!(codes(&r), ["utility_dynamic_value", "utility_dynamic_value"], "{:#?}", utility_codes(&r));
    let d = r.diagnostics.iter().find(|d| d.code == "utility_dynamic_value").unwrap();
    assert!(d.help.as_deref().is_some_and(|h| h.contains("style:padding")), "{:?}", d.help);
}

#[test]
fn class_directives_and_clsx_literals() {
    let src = r#"<script>let x = $state(true);</script>
<box class:bg-acent={x} class:hover:bg-accent={x} />
<box class={['bg-acent', x && 'p-1 p-2', { 'checked:p-1': x, row: x }, x ? `hovr:p-1 ${x}` : cn('w-[red]')]} />"#;
    let r = run(src);
    assert_eq!(
        codes(&r),
        ["utility_unknown", "utility_unknown", "utility_conflict", "utility_variant_never_matches", "utility_unknown", "utility_bad_value"],
        "{:#?}",
        utility_codes(&r)
    );
    assert_fixes_resolve(src, Some(on()));
}

#[test]
fn script_strings_are_scanned_on_request() {
    let src = "<script>\n  import x from './x.svelte';\n  const busy = 'bg-acent p-2';\n  const label = 'Loading files…';\n</script>\n<box />";
    assert!(codes(&run(src)).is_empty());
    let scan = UtilityCheck { scan_script: true, ..Default::default() };
    let r = run_with(src, Some(scan.clone()));
    assert_eq!(codes(&r), ["utility_unknown"], "{:#?}", utility_codes(&r));
    assert_fixes_resolve(src, Some(scan));
}

#[test]
fn style_classes_that_are_utilities_are_shadowed() {
    let r = run("<box class=\"flex card\" />\n<style>\n  .flex { gap: 4px; }\n  .card { gap: 2px; }\n</style>\n");
    assert_eq!(codes(&r), ["utility_shadowed_class"], "{:#?}", utility_codes(&r));
    // off by default: no utility diagnostics at all
    let r = run_with("<box class=\"flex bg-acent checked:p-1\" />\n<style>\n  .flex { gap: 4px; }\n</style>\n", None);
    assert!(codes(&r).is_empty(), "{:#?}", utility_codes(&r));
}

#[test]
fn class_directive_with_a_slash_gets_a_clsx_fix() {
    let src = "<script>let x = $state(true);</script>\n<box class:w-1/2={x} />";
    let r = run(src);
    assert_eq!(codes(&r), ["expected_token"], "{:#?}", utility_codes(&r));
    let fixes = assert_fixes_resolve(src, Some(on()));
    assert_eq!(fixes, ["write it as `class={{ 'w-1/2': x }}`"]);
    // no fix-it when the tag has a class attribute already
    let r = run("<script>let x = $state(true);</script>\n<box class=\"a\" class:w-1/2={x} />");
    assert!(r.diagnostics.iter().all(|d| d.fixes.is_empty()));
}

fn css(src: &str) -> String {
    let mut o = CompileOptions::new("App.svelte", egui());
    o.scoped_css = true;
    o.css_scope = Some("svelte-u4".into());
    let r = compile(src, &o);
    assert!(!r.has_errors(), "{:#?}", r.diagnostics);
    r.css.expect("scoped css").code
}

#[test]
fn apply_expands_like_hand_written_css() {
    let with_apply = "<box class=\"a\" />\n<style>\n  .a { @apply bg-surface hover:bg-list-hover dark:p-1 part-thumb:bg-accent!; gap: 2px; }\n</style>\n";
    let by_hand = "<box class=\"a\" />\n<style>\n  .a { background-color: var(--sv-surface); &:hover { background-color: var(--sv-list-hover); } @media (prefers-color-scheme: dark) { & { padding: var(--sv-space-1); } } &::part(thumb) { background-color: var(--sv-accent) !important; } gap: 2px; }\n</style>\n";
    assert_eq!(css(with_apply), css(by_hand));
    // utilities need not be on for `@apply`; diagnostics after an expansion keep their positions
    let src = "<box class=\"a\" />\n<style>\n  .a { @apply bg-surface; }\n  .b { gap: 2px; }\n</style>\n";
    let r = run_with(src, None);
    let unused: Vec<_> = r.diagnostics.iter().filter(|d| d.code == "css_unused_selector").collect();
    assert!(unused.is_empty() || unused.iter().all(|d| &src[d.range.start as usize..d.range.end as usize] == ".b"), "{:#?}", r.diagnostics);
}

#[test]
fn apply_of_an_unknown_name_is_an_error() {
    let src = "<box class=\"a\" />\n<style>\n  .a { @apply bg-acent row p-2; }\n</style>\n";
    let r = run_with(src, None);
    assert_eq!(codes(&r), ["utility_apply_unknown", "utility_apply_unknown"], "{:#?}", utility_codes(&r));
    assert!(r.diagnostics.iter().filter(|d| d.code == "utility_apply_unknown").all(|d| d.severity == Severity::Error));
    let d = r.diagnostics.iter().find(|d| d.code == "utility_apply_unknown").unwrap();
    assert_eq!(&src[d.range.start as usize..d.range.end as usize], "bg-acent");
    let fixes = assert_fixes_resolve(src, None);
    assert_eq!(fixes, ["replace with `bg-accent`"]);
}

fn svelte_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() && !p.ends_with("node_modules") {
            svelte_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "svelte") {
            out.push(p);
        }
    }
}

/// The egui fixtures (and cadatomic's panels when `SVELTERS_CADATOMIC_UI` names its `ui/` dir) have
/// no utility diagnostics: their semantic classes stay plain.
#[test]
fn fixtures_have_no_utility_diagnostics() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    for d in ["fixtures/gallery-egui", "fixtures/embed-egui", "fixtures/counter-egui", "fixtures/motion-egui", "fixtures/lists-egui", "examples/egui-kinds"] {
        svelte_files(&root.join(d), &mut files);
    }
    if let Some(ui) = std::env::var_os("SVELTERS_CADATOMIC_UI") {
        svelte_files(Path::new(&ui), &mut files);
    }
    assert!(files.len() > 20, "{files:?}");
    let scan = UtilityCheck { scan_script: true, ..Default::default() };
    for f in &files {
        let src = fs::read_to_string(f).unwrap();
        let r = run_with(&src, Some(scan.clone()));
        let found: Vec<_> = r.diagnostics.iter().filter(|d| d.code.starts_with("utility_")).map(|d| (d.range.start_pos.line, d.code, d.message.clone())).collect();
        assert!(found.is_empty(), "{}: {found:#?}", f.display());
    }
}
