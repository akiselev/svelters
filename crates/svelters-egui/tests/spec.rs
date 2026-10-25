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

    // U0: every own widget's `#[part]`s are its kind's `parts`, in order, node or paint as declared;
    // a kind without an own widget has none
    let spec = spec::semantics();
    let reg = kinds::KindRegistry::global();
    let mut with_parts = 0;
    for (i, k) in KINDS.iter().enumerate() {
        let have: Vec<(String, bool)> = spec["kinds"][k.id]["parts"]
            .as_array()
            .unwrap_or_else(|| panic!("{}: no parts list", k.id))
            .iter()
            .map(|p| (p["name"].as_str().unwrap().to_string(), p["node"].as_bool().unwrap()))
            .collect();
        let want: Vec<(String, bool)> = reg.replacement(i).map_or(&[][..], |r| r.entry.parts).iter().map(|p| (p.name.to_string(), p.node)).collect();
        assert_eq!(have, want, "{}: parts", k.id);
        with_parts += usize::from(!want.is_empty());
    }
    assert!(with_parts >= 5, "the own widgets with parts (button, checkbox, radio, ...) are listed");
}

/// U0: every `sv` provider state is listed under `style.provider_states` and set by at least one
/// built-in kind (`kinds::docs::provider_states`), and each kind's list is what the docs table says.
#[test]
fn provider_states_are_listed_and_claimed() {
    use svelters_egui::kinds::docs::provider_states;
    use svelters_egui::provider::state_registry;
    let spec = spec::semantics();
    let listed: Vec<&str> = spec["style"]["provider_states"].as_array().unwrap().iter().map(|s| s["name"].as_str().unwrap()).collect();
    let sv: Vec<&str> = state_registry().iter().filter(|d| d.namespace == "sv").map(|d| d.full).collect();
    assert_eq!(listed, sv);
    assert!(sv.contains(&"sv.overflow-top"));
    for full in &sv {
        assert!(KINDS.iter().any(|k| provider_states(k.id).iter().any(|s| s.full == *full)), "{full} is set by no built-in kind (kinds/docs.rs provider_states)");
    }
    for k in KINDS {
        let have: Vec<&str> = spec["kinds"][k.id]["provider_states"].as_array().unwrap().iter().map(|s| s.as_str().unwrap()).collect();
        let want: Vec<&str> = provider_states(k.id).iter().map(|s| s.full).collect();
        assert_eq!(have, want, "{}", k.id);
    }
}

/// U0: the media features, the property ranges and the theme token manifest in the spec.
#[test]
fn style_metadata_for_utilities() {
    use svelters_egui::theme::{COMPONENT_COLORS, COMPONENT_METRICS, ROLES};
    let spec = spec::semantics();
    let media: Vec<&str> = spec["style"]["media_features"].as_array().unwrap().iter().map(|f| f["name"].as_str().unwrap()).collect();
    assert_eq!(media, svelters_style::MEDIA_FEATURES.iter().map(|f| f.0).collect::<Vec<_>>());

    let props = spec["style"]["properties"].as_object().unwrap();
    assert_eq!(props["opacity"]["range"], serde_json::json!({"min": 0.0, "max": 1.0}));
    assert_eq!(props["width"]["range"], serde_json::json!({"min": 0.0}));
    assert!(props["margin-top"].get("range").is_none(), "margins may be negative");
    for (name, p) in props {
        if let Some(r) = p.get("range") {
            let ty = p.get("type").or(p.get("decl_type")).and_then(|t| t.as_str()).unwrap();
            assert!(matches!(ty, "length" | "number" | "insets"), "{name}: a range on a {ty}");
            assert!(r.get("min").is_some() || r.get("max").is_some(), "{name}: empty range");
        }
    }

    let tokens = spec["theme"]["tokens"].as_object().unwrap();
    for (role, key) in ROLES {
        let t = &tokens[*role];
        assert_eq!(t["layer"], "role", "{role}");
        assert_eq!(t["type"], "color", "{role}");
        assert_eq!(t["vscode"].as_str(), *key, "{role}");
    }
    for (c, from) in COMPONENT_COLORS {
        assert_eq!((tokens[*c]["layer"].as_str(), tokens[*c]["type"].as_str()), (Some("component"), Some("color")), "{c}");
        assert_eq!(tokens[*c]["from"].as_str(), *from, "{c}");
    }
    for (c, from) in COMPONENT_METRICS {
        assert_eq!((tokens[*c]["type"].as_str(), tokens[*c]["from"].as_str()), (Some("length"), Some(*from)), "{c}");
    }
    let ty = |n: &str| tokens.get(n).and_then(|t| t["type"].as_str()).unwrap_or_else(|| panic!("no token {n}"));
    for n in ["space-0", "space-7", "font-size", "font-size-sm", "radius", "row-h", "breakpoint-sm", "container-md"] {
        assert_eq!(ty(n), "length", "{n}");
    }
    assert_eq!(ty("elev-3"), "shadow");
    assert_eq!(ty("motion-fast"), "duration");
    assert_eq!(ty("ease-out"), "easing");
    assert_eq!(ty("font-mono"), "font-family");
    assert_eq!(tokens["space-7"]["value"], "32px");
    assert_eq!(tokens["row-h"]["layer"], "density");
    assert_eq!(tokens["row-h"]["density"]["compact"], "20px");
    assert_eq!(tokens["elev-0"]["layer"], "set");
    assert!(tokens["list-hover"]["values"]["dark"].is_string(), "colours differ per set");
    assert!(!tokens.keys().any(|k| k.starts_with("--") || k.starts_with("vscode")), "only --sv-* tokens, by bare name");
}

/// W-C1: the stable id table is current: every built-in kind and member has its row
/// (`SVELTERS_UPDATE=1` appends the rows a new kind or member needs; rows are never renumbered or
/// dropped).
#[test]
fn checked_in_ids_are_current() {
    let path = root().join(kinds::ids::IDS_PATH);
    let want = spec::ids_text();
    if std::env::var("SVELTERS_UPDATE").as_deref() == Ok("1") {
        std::fs::write(&path, &want).unwrap();
        return;
    }
    let have = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(have == want, "{} lacks rows: run SVELTERS_UPDATE=1 cargo test -p svelters-egui --test spec", kinds::ids::IDS_PATH);
}

/// W-C1: the registry's ids are the table's: a kind's id, and a member's `kind id << 16 | number`
/// with the declaring kind's id; each id resolves back to the same definition.
#[test]
fn registry_ids_are_the_tables() {
    use kinds::ids::Cat;
    let table = kinds::ids::checked_in();
    let reg = kinds::KindRegistry::global();
    for (i, k) in KINDS.iter().enumerate() {
        let Some(row) = table.kinds.get(k.id) else { continue };
        assert_eq!(reg.kind_id(i), row.id, "{}: kind id", k.id);
        assert_eq!(reg.kind_of_id(row.id), Some(i), "{}: kind id resolves back", k.id);
        for p in k.props {
            let (id, def) = reg.find_prop(i, p.name).unwrap();
            assert!(std::ptr::eq(def, p));
            let n = reg.stable_ids().member(k.id, Cat::Prop, p.name).unwrap();
            assert_eq!(id, row.id << 16 | u32::from(n), "{}.{}: prop id", k.id, p.name);
        }
        for s in k.signals {
            let (id, _) = reg.find_signal(i, s.name).unwrap();
            let n = reg.stable_ids().member(k.id, Cat::Signal, s.name).unwrap();
            assert_eq!(id, row.id << 16 | u32::from(n), "{}.{}: signal id", k.id, s.name);
        }
    }
}

/// W-C1: the spec carries the ids and the spec hash; every built-in kind, prop and signal has an id
/// and a fingerprint, and the hash is [`spec::spec_hash`].
#[test]
fn spec_carries_ids() {
    let spec = spec::semantics();
    assert_eq!(spec["spec_hash"].as_str(), Some(spec::spec_hash().as_str()));
    assert_eq!(spec::spec_hash().len(), 16);
    for k in KINDS {
        let kj = &spec["kinds"][k.id];
        assert!(kj["id"].as_u64().is_some_and(|id| id > 0), "{}: id", k.id);
        assert!(kj["fingerprint"].is_string(), "{}: fingerprint", k.id);
        for (name, p) in kj["props"].as_object().unwrap() {
            assert!(p["id"].as_u64().is_some_and(|id| id > 0), "{}.{name}: id", k.id);
            assert!(p["fingerprint"].is_string(), "{}.{name}: fingerprint", k.id);
        }
        for (name, s) in kj["signals"].as_object().unwrap() {
            assert!(s["id"].as_u64().is_some_and(|id| id > 0), "{}.{name}: id", k.id);
            assert!(s["cancelable"].is_boolean() && s["passive"].is_boolean(), "{}.{name}: passivity facts", k.id);
        }
    }
    // W-C5: wheel is cancelable and passive-relevant; pointermove is not cancelable
    let w = &spec["kinds"]["Widget"]["signals"];
    assert_eq!(w["wheel"]["cancelable"], true);
    assert_eq!(w["wheel"]["passive"], true);
    assert_eq!(w["pointermove"]["cancelable"], false);
    // W-C9: a button needs a name from its text, accessible name or tooltip; it can show an icon
    let a = &spec["kinds"]["Button"]["a11y"];
    assert_eq!(a["requires_name"], true);
    assert_eq!(a["icon_props"], serde_json::json!(["icon"]));
    assert!(a["name_props"].as_array().unwrap().iter().any(|p| p == "text"));
}
