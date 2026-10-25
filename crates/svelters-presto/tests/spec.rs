//! `metadata/semantics/presto.json` and the wire id table `crates/svelters-presto/ids.txt` match the
//! kind registry. `SVELTERS_UPDATE=1 cargo test -p svelters-presto --test spec` appends missing ids
//! and rewrites the spec.

use std::path::PathBuf;
use svelters_presto::ids::{Ids, TABLE, TABLE_PATH};
use svelters_presto::spec::{SPEC_PATH, builtin_text};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn update() -> bool {
    std::env::var_os("SVELTERS_UPDATE").is_some()
}

#[test]
fn id_table_is_complete() {
    let reg = svelters_presto::builtin_registry();
    let ids = Ids::parse(TABLE).unwrap();
    let missing = ids.missing(&reg);
    if missing.is_empty() {
        return;
    }
    if update() {
        std::fs::write(root().join(TABLE_PATH), ids.appended(TABLE, &missing)).unwrap();
        return;
    }
    panic!("{TABLE_PATH} lacks {} ids (first: {:?}); run with SVELTERS_UPDATE=1", missing.len(), missing[0]);
}

#[test]
fn spec_is_current() {
    let path = root().join(SPEC_PATH);
    let want = builtin_text();
    let have = std::fs::read_to_string(&path).unwrap_or_default();
    if have == want {
        return;
    }
    if update() {
        std::fs::write(&path, want).unwrap();
        return;
    }
    panic!("{SPEC_PATH} is stale; run SVELTERS_UPDATE=1 cargo test -p svelters-presto --test spec");
}

#[test]
fn ids_are_unique_and_every_member_has_one() {
    let reg = svelters_presto::builtin_registry();
    let spec: serde_json::Value = serde_json::from_str(&builtin_text()).unwrap();
    let mut seen = std::collections::HashSet::new();
    for (name, k) in spec["kinds"].as_object().unwrap() {
        assert!(k["id"].as_u64().is_some(), "{name} has no id");
        assert!(seen.insert(k["id"].as_u64().unwrap()), "{name}: id used twice");
        for section in ["props", "signals", "methods"] {
            for (m, j) in k[section].as_object().unwrap() {
                let id = j["id"].as_u64().unwrap_or_else(|| panic!("{name}.{m} has no id"));
                assert!(seen.insert(id), "{name}.{m}: id {id} used twice");
                assert_eq!(j["fingerprint"].as_str().map(str::len), Some(16), "{name}.{m}");
            }
        }
    }
    assert!(reg.kinds().count() > 20);
    assert_eq!(spec["ids"]["spec_hash"].as_str().map(str::len), Some(16));
}

#[test]
fn a_new_row_moves_no_id() {
    let ids = Ids::parse(TABLE).unwrap();
    let extra = [svelters_presto::ids::Key::member(svelters_presto::ids::What::Prop, "Button", "zz-new")];
    let text = ids.appended(TABLE, &extra);
    let grown = Ids::parse(&text).unwrap();
    assert_eq!(grown.len(), ids.len() + 1);
    for (id, key) in (1..=ids.len() as u32).filter_map(|i| ids.key(i).map(|k| (i, k))) {
        assert_eq!(grown.get(key), Some(id));
    }
}

#[test]
fn spec_carries_presto_fields() {
    let spec: serde_json::Value = serde_json::from_str(&builtin_text()).unwrap();
    assert_eq!(spec["backend"], "presto");
    assert_eq!(spec["style"]["engine"], "backend");
    let caps: Vec<&str> = spec["capabilities"].as_array().unwrap().iter().map(|c| c.as_str().unwrap()).collect();
    assert_eq!(caps, ["ids", "templates", "native_transitions", "native_motion", "css_precompiled"]);
    let button = &spec["kinds"]["Button"];
    assert_eq!(button["props"]["text"]["change"], "layout");
    assert_eq!(button["a11y"]["role"], "Button");
    let widget = &spec["kinds"]["Widget"];
    assert_eq!(widget["signals"]["wheel"]["passive"], true);
    assert_eq!(widget["signals"]["click"]["cancelable"], true);
    assert_eq!(spec["style"]["properties"]["opacity"]["change"], "property");
    assert_eq!(spec["style"]["properties"]["transform"]["change"], "property");
    assert_eq!(spec["kinds"]["Slider"]["bindings"]["value"]["event"], "changed");
    assert_eq!(spec["tags"]["button"], "Button");
}
