//! GAL1 item 1: story.toml parsing and its errors, discovery, coverage and the matrix order.

use skelter_gallery::catalog::declared_props;
use skelter_gallery::{Axes, Matrix, State, Theme, discover};
use skelter_widgets::theme::Density;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

#[test]
fn discovery_finds_every_story_in_family_order() {
    let cat = discover(&[fixture("gallery")]).unwrap();
    let ids: Vec<&str> = cat.stories().iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["button", "stack"], "controls before layout");
    let b = cat.get("button").unwrap();
    assert_eq!(b.title, "Button");
    assert_eq!(b.size, [160, 48]);
    assert_eq!(b.states, State::DEFAULT_SET, "no `states`: the default set");
    assert_eq!(b.dir, Path::new("stories/controls/button"));
    assert_eq!(b.variants[0].file, Path::new("stories/controls/button/primary.svelte"));
    assert!(b.path(&b.variants[0].file).is_file());
    assert!(b.variants[0].takes_disabled(), "{:?}", b.variants[0].props);
    assert!(!b.variants[1].takes_disabled());
    assert_eq!(cat.get("stack").unwrap().states, [State::Normal]);
}

#[test]
fn errors_name_the_file_and_each_field() {
    let errs = discover(&[fixture("bad-toml")]).unwrap_err();
    let file = fixture("bad-toml").canonicalize().unwrap().join("stories/controls/wrong/story.toml");
    assert!(errs.iter().all(|e| e.file == file), "{errs:?}");
    let fields: Vec<&str> = errs.iter().filter_map(|e| e.field.as_deref()).collect();
    for f in ["colour", "id", "family", "kinds", "size", "states[1]", "variant[0].file", "variant[0].args.disabled"] {
        assert!(fields.contains(&f), "no error for {f}: {errs:#?}");
    }
    let shown = errs.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("\n");
    assert!(shown.contains("story.toml: states[1]: \"glowing\" is not a state"), "{shown}");
}

#[test]
fn duplicate_ids_across_roots_are_refused() {
    let errs = discover(&[fixture("gallery"), fixture("gallery")]).unwrap_err();
    assert!(errs.iter().any(|e| e.field.as_deref() == Some("id") && e.message.contains("also defined")), "{errs:?}");
}

#[test]
fn coverage_reads_the_registry() {
    let cat = discover(&[fixture("gallery")]).unwrap();
    let mut reg = skelter_core::KindRegistry::new();
    svelters_skelter::kinds::register_builtin(&mut reg).unwrap();
    let cov = cat.coverage(&reg);
    for k in ["button", "box", "label", "select"] {
        assert!(cov.covered.iter().any(|c| c == k), "{k} covered: {cov:?}");
    }
    assert!(cov.missing.iter().any(|c| c == "slider"), "{cov:?}");
    assert!(cov.unknown.is_empty(), "{cov:?}");
}

#[test]
fn the_matrix_expands_in_the_stable_order() {
    let cat = discover(&[fixture("gallery")]).unwrap();
    let b = cat.get("button").unwrap();
    let m = Matrix::new(b, &Axes::default()).unwrap();
    // primary: 2 sizes × 2 tones; plain: 1; × 5 states × 4 themes
    assert_eq!(m.len(), (4 + 1) * 5 * 4);
    let first: Vec<String> = m.cells[..6].iter().map(|c| c.label()).collect();
    assert_eq!(
        first,
        [
            "primary size=small,tone=plain normal dark-modern",
            "primary size=small,tone=plain normal light-modern",
            "primary size=small,tone=plain normal hc-dark",
            "primary size=small,tone=plain normal hc-light",
            "primary size=small,tone=plain hover dark-modern",
            "primary size=small,tone=plain hover light-modern",
        ]
    );
    // args: the first key (size) slowest
    let combos: Vec<String> = b.variants[0].arg_combinations().iter().map(|a| format!("{a:?}")).collect();
    assert_eq!(combos.len(), 4);
    assert!(combos[1].contains("\"small\"") && combos[1].contains("\"accent\""), "{combos:?}");
    assert_eq!(m.cells.last().unwrap().label(), "plain disabled hc-light");

    let axes = Axes {
        variants: Some(vec!["primary".into()]),
        args: [("size".to_string(), serde_json::json!("large"))].into(),
        states: Some(vec![State::Hover, State::Normal]),
        themes: vec![Theme::LightModern],
        densities: vec![Density::Compact, Density::Comfortable],
        zooms: vec![1.0, 2.0],
    };
    let m = Matrix::new(b, &axes).unwrap();
    assert_eq!(m.len(), 2 * 2 * 2 * 2, "tone × states × densities × zooms");
    assert_eq!(m.cells[0].label(), "primary size=large,tone=plain normal light-modern compact", "states in the story's order");
    assert_eq!(m.cells[1].label(), "primary size=large,tone=plain normal light-modern compact x2");

    assert!(Matrix::new(b, &Axes { states: Some(vec![State::Checked]), ..Axes::default() }).unwrap_err().contains("does not show state checked"));
    assert!(Matrix::new(b, &Axes { variants: Some(vec!["nope".into()]), ..Axes::default() }).unwrap_err().contains("no variant nope"));
}

#[test]
fn props_are_read_from_the_destructuring() {
    assert_eq!(declared_props("let { a, b = 1, c: d, e = { x: 1, y: [2, 3] }, ...rest } = $props();"), ["a", "b", "c", "e"]);
    assert_eq!(declared_props("let { disabled }: Props = $props();"), ["disabled"]);
    assert!(declared_props("let p = $props();").is_empty());
    assert!(declared_props("<button />").is_empty());
}
