//! `metadata/semantics/egui.json` is generated from the kind table (`src/kinds.rs`): this is its
//! `--check`. `SVELTERS_UPDATE=1 cargo test -p svelters-egui --test spec` rewrites it.

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
