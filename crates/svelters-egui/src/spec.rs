//! The egui semantic spec (`svelters-semantics/v0`, `metadata/semantics/egui.json`), generated from
//! the kind table. `tests/spec.rs` fails when the checked-in file is stale
//! (`SVELTERS_UPDATE=1 cargo test -p svelters-egui --test spec` rewrites it).

use crate::kinds::{self, Def, KINDS, KindDef, Native, PropDef, Ty};
use serde_json::{Map, Value as J, json};

/// Where the checked-in spec lives, relative to the workspace root.
pub const SPEC_PATH: &str = "metadata/semantics/egui.json";

fn ty_name(ty: Ty) -> &'static str {
    match ty {
        Ty::Bool => "bool",
        Ty::F64 => "f64",
        Ty::I64 => "i64",
        Ty::Str => "string",
        Ty::Enum(_) => "enum",
    }
}

fn default_json(d: Def) -> J {
    match d {
        Def::Null => J::Null,
        Def::Bool(b) => json!(b),
        Def::F64(f) => json!(f),
        Def::I64(i) => json!(i),
        Def::Str(s) => json!(s),
    }
}

fn prop_json(k: &KindDef, p: &PropDef) -> J {
    let observe = match p.native {
        Native::Widget => json!({
            "class": "exact",
            "fact": format!("egui:{}:{}", k.id, p.name),
            "mechanism": "widget",
            "verified": "delivery",
        }),
        Native::Never => json!({
            "class": "set_only",
            "fact": format!("egui:{}:{}", k.id, p.name),
            "mechanism": "scene",
            "verified": "weak",
        }),
    };
    let mut m = Map::new();
    m.insert("construct_only".into(), json!(false));
    m.insert("default".into(), default_json(p.default));
    m.insert("doc".into(), json!(p.doc));
    m.insert("js".into(), json!(p.name));
    m.insert("native".into(), json!(p.name));
    m.insert("nullable".into(), json!(p.nullable));
    m.insert("observe".into(), observe);
    m.insert("readable".into(), json!(true));
    m.insert("type".into(), json!(ty_name(p.ty)));
    if let Ty::Enum(values) = p.ty {
        m.insert("values".into(), json!(values));
    }
    m.insert("writable".into(), json!(true));
    J::Object(m)
}

fn kind_json(k: &KindDef) -> J {
    let chain: Vec<&str> = kinds::chain(kinds::kind_index(k.id).unwrap()).into_iter().map(|i| KINDS[i].id).collect();
    let props: Map<String, J> = k.props.iter().map(|p| (p.name.to_string(), prop_json(k, p))).collect();
    let signals: Map<String, J> = k
        .signals
        .iter()
        .map(|s| {
            let args: Vec<J> = s.args.iter().map(|(n, t)| json!({"name": n, "type": ty_name(*t)})).collect();
            (
                s.name.to_string(),
                json!({
                    "args": args,
                    "disconnect_on_drop": true,
                    "doc": s.doc,
                    "handler_result": if s.sync { "bool_stop" } else { "none" },
                    "js_event": s.name,
                    "name": s.name,
                    "phases": [],
                    "sync": s.sync,
                    "sync_required": false,
                }),
            )
        })
        .collect();
    let methods: Map<String, J> = k
        .methods
        .iter()
        .map(|m| {
            (
                m.name.to_string(),
                json!({"dispatch": "generated", "doc": m.doc, "native": m.name, "params": [], "returns": {"nullable": false, "type": "void"}}),
            )
        })
        .collect();
    // Two-way bindings for what the widget changes: the `changed` signal carries the new value;
    // the scene never echoes a host write.
    let bindings: Map<String, J> = k
        .props
        .iter()
        .filter(|p| p.native == Native::Widget)
        .map(|p| {
            (
                p.name.to_string(),
                json!({
                    "compare_before_set": true,
                    "direction": "two_way",
                    "echo": "none",
                    "event": "changed",
                    "lifetime": {"on_destroy": "idempotent_disconnect", "on_native_death": "end_binding", "on_stale_update": "warn_noop"},
                    "natively_normalizes": p.key == kinds::P::Value,
                    "property": p.name,
                    "provenance": format!("svelters-egui: the {} widget writes {} into the node during the pass; `changed` carries it (arg 0), set_prop never announces", k.native, p.name),
                    "read_back": p.key == kinds::P::Value,
                    "sync_signals_on_set": [],
                    "value": {"arg": 0},
                }),
            )
        })
        .collect();
    let children = k.slot.map(|slot| {
        json!({
            "default_slot": slot,
            "policy": "ordered",
            "slots": {slot: {"accepts": ["Widget"], "attrs": {}, "many": true, "order": "before_sibling", "route_by_kind": false}},
        })
    });
    json!({
        "abstract": k.tag.is_none(),
        "bindings": bindings,
        "chain": chain,
        "children": children,
        "doc": k.doc,
        "interface": false,
        "lifetime": if k.toplevel { "toplevel" } else { "owned" },
        "methods": methods,
        "native": k.native,
        "present": k.present,
        "props": props,
        "provenance": {"kind": "crates/svelters-egui/src/kinds.rs"},
        "signals": signals,
        "text_content": k.text_content,
    })
}

/// The style table (D3): every layout prop is a setter, so `<style>` rules can use the CSS names
/// on egui. Visual properties (the backend cascade) are G2.
fn style_json() -> J {
    let mut props: Map<String, J> = Map::new();
    for (ki, k) in KINDS.iter().enumerate() {
        for p in k.props.iter().filter(|p| p.layout) {
            // the tags whose chain holds this prop
            let mut tags: Vec<&str> = KINDS
                .iter()
                .enumerate()
                .filter(|(i, kk)| kk.tag.is_some() && kinds::chain(*i).contains(&ki))
                .map(|(_, kk)| kk.tag.unwrap())
                .collect();
            let entry = props.entry(p.name.to_string()).or_insert_with(|| {
                let value = match p.ty {
                    Ty::Bool => "bool",
                    Ty::Enum(_) => "enum",
                    Ty::F64 if matches!(p.key, kinds::P::FlexGrow | kinds::P::FlexShrink) => "number",
                    _ => "length",
                };
                let mut m = Map::new();
                m.insert("kinds".into(), json!([]));
                m.insert("native".into(), json!(p.name));
                m.insert("prop".into(), json!(p.name));
                m.insert("tags".into(), json!([]));
                m.insert("type".into(), json!(value));
                if let Ty::Enum(values) = p.ty {
                    m.insert("values".into(), json!(values));
                }
                m.insert("via".into(), json!("setter"));
                J::Object(m)
            });
            let m = entry.as_object_mut().unwrap();
            let kinds_list = m.get_mut("kinds").unwrap().as_array_mut().unwrap();
            kinds_list.push(json!(k.id));
            let t = m.get_mut("tags").unwrap().as_array_mut().unwrap();
            for tag in tags.drain(..) {
                if !t.iter().any(|x| x == tag) {
                    t.push(json!(tag));
                }
            }
            let mut sorted: Vec<String> = t.iter().map(|x| x.as_str().unwrap().to_string()).collect();
            sorted.sort();
            *t = sorted.into_iter().map(J::from).collect();
        }
    }
    json!({
        "dialect": "none",
        "properties": props,
        "setter_selectors": ["type", "class", "id"],
        "units": {"px": {"provenance": "egui points: 1px in a style sheet is one point", "scale": 1.0}},
    })
}

/// The whole spec.
pub fn semantics() -> J {
    let kinds: Map<String, J> = KINDS.iter().map(|k| (k.id.to_string(), kind_json(k))).collect();
    let tags: Map<String, J> = KINDS.iter().filter_map(|k| k.tag.map(|t| (t.to_string(), json!(k.id)))).collect();
    json!({
        "backend": "egui",
        "event_aliases": {},
        "framework": "egui 0.37 (svelters-egui; taffy 0.15 layout)",
        "kinds": kinds,
        "method_lookup": "annotated",
        "schema": "svelters-semantics/v0",
        "source": {"kinds": "crates/svelters-egui/src/kinds.rs", "generator": "crates/svelters-egui/src/spec.rs"},
        "style": style_json(),
        "tags": tags,
    })
}

/// The spec as written to [`SPEC_PATH`].
pub fn semantics_text() -> String {
    let mut s = serde_json::to_string_pretty(&semantics()).expect("spec json");
    s.push('\n');
    s
}
