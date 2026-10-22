//! `metadata/semantics/egui.json` and `packages/native-runtime/types/egui.d.ts` are generated from
//! the kind table (`src/kinds/`): this is their `--check`. `SVELTERS_UPDATE=1 cargo test -p
//! svelters-egui --test spec` rewrites them.

use svelters_egui::kinds::{self, KINDS};
use svelters_egui::spec;

fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn checked_in_spec_is_current() {
    let path = root().join(spec::SPEC_PATH);
    let want = spec::semantics_text();
    if std::env::var("SVELTERS_UPDATE").as_deref() == Ok("1") {
        std::fs::write(&path, &want).unwrap();
        return;
    }
    let have = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        have == want,
        "{} is stale: run SVELTERS_UPDATE=1 cargo test -p svelters-egui --test spec",
        spec::SPEC_PATH
    );
}

#[test]
fn checked_in_typings_are_current() {
    let path = root().join(spec::TYPINGS_PATH);
    let want = spec::builtin_typings();
    if std::env::var("SVELTERS_UPDATE").as_deref() == Ok("1") {
        std::fs::write(&path, &want).unwrap();
        return;
    }
    let have = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        have == want,
        "{} is stale: run SVELTERS_UPDATE=1 cargo test -p svelters-egui --test spec",
        spec::TYPINGS_PATH
    );
}

/// DOC1: every built-in kind has exactly one docs row (`kinds/docs.rs`) and every row names a kind,
/// so a new kind can't reach `egui.json` (and the widget reference) without its category, states,
/// accessibility and paint.
#[test]
fn every_kind_has_one_docs_row() {
    use svelters_egui::kinds::docs::KIND_DOCS;
    for k in KINDS {
        let n = KIND_DOCS.iter().filter(|d| d.id == k.id).count();
        assert_eq!(n, 1, "kind {} has {n} rows in kinds/docs.rs (want 1)", k.id);
    }
    for d in KIND_DOCS {
        assert!(KINDS.iter().any(|k| k.id == d.id), "kinds/docs.rs has a row for unknown kind {}", d.id);
    }
}

#[test]
fn kind_table_is_consistent() {
    for (i, k) in KINDS.iter().enumerate() {
        if let Some(b) = k.base {
            assert!(kinds::kind_index(b).is_some(), "{}: unknown base {b}", k.id);
        }
        // a name resolves to the most derived definition and back to the same def
        for p in k.props {
            let (id, def) = kinds::find_prop(i, p.name).unwrap();
            assert!(std::ptr::eq(def, p), "{}.{} is shadowed", k.id, p.name);
            assert!(std::ptr::eq(kinds::prop_def(i, id).unwrap(), p));
        }
        for s in k.signals {
            let (id, def) = kinds::find_signal(i, s.name).unwrap();
            assert!(std::ptr::eq(kinds::signal_def(i, id).unwrap(), def));
        }
        if let Some(t) = k.text_content {
            assert!(kinds::find_prop(i, t).is_some(), "{}: text_content {t} is not a prop", k.id);
        }
    }
    // ids of one kind are not accepted on an unrelated one
    let label = kinds::tag_index("label").unwrap();
    let slider = kinds::tag_index("slider").unwrap();
    let (value, _) = kinds::find_prop(slider, "value").unwrap();
    assert!(kinds::prop_def(label, value).is_none());
}
