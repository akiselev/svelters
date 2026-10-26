//! `metadata/semantics/skelter.json` matches the kinds (`SVELTERS_UPDATE=1 cargo test -p
//! svelters-skelter --test spec` rewrites it), and its ids are skelter-core's hashes.

use std::path::PathBuf;
use svelters_skelter::spec::{SPEC_PATH, builtin_text};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn update() -> bool {
    std::env::var_os("SVELTERS_UPDATE").is_some()
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
    panic!("{SPEC_PATH} is stale; run SVELTERS_UPDATE=1 cargo test -p svelters-skelter --test spec");
}

#[test]
fn ids_are_unique_and_every_member_has_one() {
    let kinds = svelters_skelter::builtin_kinds();
    let spec: serde_json::Value = serde_json::from_str(&builtin_text()).unwrap();
    let mut seen = std::collections::HashSet::new();
    for (name, k) in spec["kinds"].as_object().unwrap() {
        assert!(k["id"].as_u64().is_some(), "{name} has no id");
        assert!(seen.insert(k["id"].as_u64().unwrap()), "{name}: id used twice");
        for section in ["props", "signals", "methods"] {
            for (m, j) in k[section].as_object().unwrap() {
                let id = j["id"].as_u64().unwrap_or_else(|| panic!("{name}.{m} has no id"));
                assert!(seen.insert(id), "{name}.{m}: id {id} used twice");
                assert_eq!(j["fingerprint"].as_str(), Some(format!("{id:08x}").as_str()), "{name}.{m}");
            }
        }
    }
    assert!(kinds.iter().count() > 20);
    assert_eq!(spec["ids"]["spec_hash"].as_str().map(str::len), Some(16));
}

/// A18: the host splits a signal id's top bits off as listen flags, so no wire id may reach them.
#[test]
fn no_wire_id_reaches_the_flag_bits() {
    let spec: serde_json::Value = serde_json::from_str(&builtin_text()).unwrap();
    let max = svelters_abi::SVL_SIGNAL_ID_MASK as u64;
    let mut n = 0;
    for (name, k) in spec["kinds"].as_object().unwrap() {
        let mut ids = vec![(name.clone(), k["id"].as_u64().unwrap())];
        for section in ["props", "signals", "methods"] {
            for (m, j) in k[section].as_object().unwrap() {
                ids.push((format!("{name}.{m}"), j["id"].as_u64().unwrap()));
            }
        }
        for (s, id) in k["slot_ids"].as_object().into_iter().flatten() {
            ids.push((format!("{name} slot {s}"), id.as_u64().unwrap()));
        }
        for (what, id) in ids {
            assert!(id <= max, "{what}: id {id:#x} sets flag bits");
            n += 1;
        }
    }
    assert!(n > 200);
}

#[test]
fn ids_are_skelter_core_hashes() {
    use skelter_core::{KindId, PropId, SignalId, common};
    let spec: serde_json::Value = serde_json::from_str(&builtin_text()).unwrap();
    let button = &spec["kinds"]["Button"];
    // core's fingerprints in the host's id space (A18)
    let m = |id: u32| Some((id & svelters_abi::SVL_SIGNAL_ID_MASK) as u64);
    assert_eq!(button["id"].as_u64(), m(KindId::of("button").0));
    // a kind's own native member: its core id
    assert_eq!(button["props"]["text"]["id"].as_u64(), m(PropId::of("button", "text", "string").0));
    assert_eq!(button["signals"]["clicked"]["id"].as_u64(), m(SignalId::of("button", "clicked", "void").0));
    // `Widget` members hash under core's common pseudo-kind; same-named ones are core's
    let widget = &spec["kinds"]["Widget"];
    assert_eq!(widget["props"]["autofocus"]["id"].as_u64(), m(common::AUTOFOCUS.0));
    assert_eq!(widget["signals"]["click"]["id"].as_u64(), m(common::CLICK.0));
    // Q2: egui's names, core's native names
    assert_eq!(widget["props"]["enabled"]["native"], "disabled");
    assert_eq!(widget["props"]["accessible-name"]["native"], "aria-label");
    assert_eq!(button["widget"], "skelter-widgets");
    assert_eq!(spec["kinds"]["Slider"]["widget"], "skelter-widgets");
    assert_eq!(spec["kinds"]["TextEdit"]["widget"], "placeholder");
}

#[test]
fn spec_carries_skelter_fields() {
    let spec: serde_json::Value = serde_json::from_str(&builtin_text()).unwrap();
    assert_eq!(spec["backend"], "skelter");
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
