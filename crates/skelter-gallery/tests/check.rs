//! GAL1 item 5: `check` and `list` through the CLI's entry point (`skelter_gallery::cli::run`).

use skelter_gallery::check::ProblemKind;
use skelter_gallery::{Extensions, Renderer};
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

fn gpu() -> bool {
    let on = std::env::var("SKELTER_GPU_TESTS").as_deref() == Ok("1");
    if !on {
        eprintln!("skipped: set SKELTER_GPU_TESTS=1 to run GPU tests");
    }
    on
}

#[test]
fn check_names_each_broken_variant() {
    if !gpu() {
        return;
    }
    let mut r = Renderer::new(&Extensions::new().story_root(fixture("bad"))).unwrap();
    let rep = r.check();
    assert_eq!((rep.stories, rep.variants), (2, 2));
    let found: Vec<(ProblemKind, &str)> = rep.problems.iter().map(|p| (p.kind, p.variant.as_deref().unwrap())).collect();
    assert!(found.contains(&(ProblemKind::Load, "syntax")), "{:#?}", rep.problems);
    assert!(found.contains(&(ProblemKind::NoSubject, "bare")), "{:#?}", rep.problems);
    let load = rep.problems.iter().find(|p| p.kind == ProblemKind::Load).unwrap();
    assert!(load.file.as_ref().unwrap().ends_with("syntax.svelte"), "{load:?}");

    let mut good = Renderer::new(&Extensions::new().story_root(fixture("gallery"))).unwrap();
    let rep = good.check();
    assert!(rep.ok(), "{:#?}", rep.problems);
    assert_eq!(rep.variants, 3);
}

#[test]
fn the_cli_exit_codes() {
    let run = |args: &[&str]| skelter_gallery::cli::run(Extensions::new(), &args.iter().map(|s| s.to_string()).collect::<Vec<_>>());
    let root = fixture("gallery").display().to_string();
    assert_eq!(run(&["--root", &root, "list", "--json"]), 0);
    assert_eq!(run(&["--root", &root, "frobnicate"]), 2, "unknown command");
    assert_eq!(run(&["--root", &root, "shot", "nope"]), 2, "unknown story");
    assert_eq!(run(&["--root", &root, "shot", "button", "--theme", "solarized"]), 2, "a theme outside the four");
    assert_eq!(run(&["--root", &root, "shot", "button", "--state", "glowing"]), 2);
    assert_eq!(run(&["--root", &root, "sheet"]), 2, "sheet needs a story, --family or --all");
    let bad = fixture("bad-toml").display().to_string();
    assert_eq!(run(&["--root", &bad, "check"]), 1, "a story.toml error fails check");
}
