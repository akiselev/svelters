//! The presto semantic spec (`svelters-semantics/v0`), generated from the kind registry as egui's
//! `spec.rs` generates `egui.json` (compiler-and-js §2: the binary is the source of truth):
//! [`semantics`] writes `metadata/semantics/presto.json`, which the compiler embeds.
//!
//! What it adds over egui's:
//! - **stable ids** per kind, prop, signal, method and slot: presto-core's FNV-1a hashes of (declaring
//!   kind, member, type) (`kinds.rs`; coordinator Q1), each with its `fingerprint` (the same hash in
//!   hex), and the spec's `ids.spec_hash` over all of them;
//! - `native` per prop: presto-core's name for it (`disabled` for `enabled`, coordinator Q2), and
//!   `widget` per kind: whether presto-widgets draws it or a placeholder kind holds its members;
//! - `change` per prop and per engine CSS property (architecture §4.3);
//! - `passive` and `cancelable` per signal (§11.5);
//! - `native_state` per kind and `a11y` (role, name sources, whether a name is required);
//! - `capabilities`: the op shapes presto accepts (`ids templates native_transitions native_motion
//!   css_precompiled`).
//!
//! `tests/spec.rs` fails when `presto.json` is stale (`SVELTERS_UPDATE=1 cargo test -p
//! svelters-presto --test spec` rewrites it).

use crate::kinds::{self, COMMON_ALIASES, Kinds};
use crate::surface::{DefaultValue, KindSpec, PropSpec, SignalSpec, ValueType};
use serde_json::{Map, Value as J, json};

/// Where the checked-in spec lives, relative to the workspace root.
pub const SPEC_PATH: &str = "metadata/semantics/presto.json";

/// The op shapes this backend accepts beyond the generic path (compiler-and-js §2).
pub const CAPABILITIES: &[&str] = &["ids", "templates", "native_transitions", "native_motion", "css_precompiled"];

fn default_json(d: DefaultValue, ty: ValueType) -> J {
    match d {
        DefaultValue::Null => J::Null,
        DefaultValue::Bool(b) => json!(b),
        DefaultValue::I64(i) => json!(i),
        DefaultValue::F64(f) => json!(f),
        DefaultValue::Str(s) => json!(s),
        DefaultValue::Enum(i) => match ty {
            ValueType::Enum(names) => json!(names.get(i as usize)),
            _ => J::Null,
        },
    }
}

/// The spec's `type` for a value type; colours travel as strings (`#rrggbbaa`).
fn spec_type(ty: ValueType) -> &'static str {
    match ty {
        ValueType::Color => "string",
        t => t.name(),
    }
}

fn id_fields(m: &mut Map<String, J>, id: u32) {
    m.insert("id".into(), json!(id));
    m.insert("fingerprint".into(), json!(format!("{id:08x}")));
}

/// FNV-1a, 64 bits, over the spec's ids in order.
fn spec_hash(ids: &[u32]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for id in ids {
        for b in id.to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

/// presto-core's name of a surface prop declared on `k`.
fn native_name(k: &KindSpec, p: &PropSpec) -> &'static str {
    match COMMON_ALIASES.iter().find(|a| a.0 == p.name) {
        Some(a) if k.tag.is_none() => a.1,
        _ => p.name,
    }
}

fn prop_json(k: &KindSpec, p: &PropSpec) -> J {
    let fact = format!("presto:{}:{}", k.name, p.name);
    let observe = if p.native {
        // the kind announces each change it makes (`Dispatched::changed`)
        json!({"class": "exact", "fact": fact, "mechanism": "kind", "verified": "delivery"})
    } else {
        // only JS writes it: the tree holds what was written
        json!({"class": "set_only", "fact": fact, "mechanism": "tree", "verified": "weak"})
    };
    let mut m = Map::new();
    m.insert("change".into(), json!(p.change.name()));
    m.insert("construct_only".into(), json!(false));
    m.insert("default".into(), default_json(p.default, p.ty));
    m.insert("doc".into(), json!(p.doc));
    id_fields(&mut m, kinds::prop_wire(k, p));
    m.insert("js".into(), json!(p.name));
    m.insert("native".into(), json!(native_name(k, p)));
    m.insert("nullable".into(), json!(p.nullable));
    m.insert("observe".into(), observe);
    m.insert("readable".into(), json!(true));
    m.insert("type".into(), json!(spec_type(p.ty)));
    if let ValueType::Enum(values) = p.ty {
        m.insert("values".into(), json!(values));
    }
    if let Some(of) = p.ty.of() {
        m.insert("of".into(), json!(of));
    }
    if p.ty == ValueType::Color {
        m.insert("format".into(), json!("color"));
    }
    m.insert("writable".into(), json!(true));
    J::Object(m)
}

fn signal_json(k: &KindSpec, s: &SignalSpec) -> J {
    let args: Vec<J> = s.args.iter().map(|(n, t)| json!({"name": n, "type": spec_type(*t)})).collect();
    let mut m = Map::new();
    m.insert("args".into(), J::Array(args));
    m.insert("cancelable".into(), json!(s.cancelable));
    m.insert("disconnect_on_drop".into(), json!(true));
    m.insert("doc".into(), json!(s.doc));
    m.insert("handler_result".into(), json!(if s.sync { "bool_stop" } else { "none" }));
    id_fields(&mut m, kinds::signal_wire(k, s));
    m.insert("js_event".into(), json!(s.name));
    m.insert("name".into(), json!(s.name));
    m.insert("passive".into(), json!(s.passive));
    // EV1: the DOM-style events propagate along the tree and take capture listeners
    m.insert("phases".into(), if s.bubbles { json!(["capture"]) } else { json!([]) });
    m.insert("sync".into(), json!(s.sync));
    m.insert("sync_required".into(), json!(false));
    J::Object(m)
}

/// Two-way bindings for the native-owned props: the kind's `changed` signal carries the new value
/// (else its first signal with an argument named like the prop), as on egui.
fn bindings_json(k: &'static KindSpec) -> J {
    let all_signals: Vec<&SignalSpec> = k.chain().flat_map(|c| c.signals.iter()).collect();
    let bind_signal = |p: &PropSpec| all_signals.iter().find(|s| s.name == "changed").or_else(|| all_signals.iter().find(|s| s.args.iter().any(|(n, _)| *n == p.name))).copied();
    let commit = all_signals.iter().find(|s| s.name == "commit").copied();
    let mut out = Map::new();
    for p in k.chain().flat_map(|c| c.props.iter()).filter(|p| p.native) {
        let sig = bind_signal(p);
        let event = sig.map_or("changed", |s| s.name);
        let arg = sig.map_or(&[][..], |s| s.args).iter().position(|(n, _)| *n == p.name).unwrap_or(0);
        // the kind itself changes a written value (a slider clamps to its range and step)
        let normalizes = p.name == "value" && k.chain().any(|c| c.props.iter().any(|q| q.name == "max"));
        let mut b = json!({
            "compare_before_set": true,
            "direction": "two_way",
            "echo": "none",
            "event": event,
            "lifetime": {"on_destroy": "idempotent_disconnect", "on_native_death": "end_binding", "on_stale_update": "warn_noop"},
            "natively_normalizes": normalizes,
            "property": p.name,
            "provenance": format!("svelters-presto: <{}> writes {} into the tree as native-owned state (architecture §4.5) and emits `{event}` with it (arg {arg}); a JS write is never echoed", k.tag.unwrap_or(k.name), p.name),
            "read_back": normalizes,
            "sync_signals_on_set": [],
            "value": {"arg": arg},
        });
        // CX3: `bind:<prop>|commit` when the kind's `commit` event has the change event's shape
        if let (Some(c), Some(s)) = (commit, sig)
            && c.args.len() == s.args.len()
            && c.args.iter().zip(s.args).all(|(a, b)| a.1 == b.1)
        {
            b["commit"] = json!({"event": "commit", "arg": arg});
        }
        out.insert(p.name.to_string(), b);
    }
    J::Object(out)
}

fn children_json(k: &'static KindSpec) -> J {
    let Some(owner) = k.chain().find(|c| !c.slots.is_empty()) else { return J::Null };
    let slots: Map<String, J> = owner
        .slots
        .iter()
        .map(|s| (s.name.to_string(), json!({"accepts": s.accepts, "attrs": {}, "many": s.many, "order": "before_sibling", "route_by_kind": false})))
        .collect();
    json!({"default_slot": owner.slots[0].name, "policy": "ordered", "slots": slots})
}

fn kind_json(km: &kinds::KindMap) -> J {
    let k = km.surface;
    let chain: Vec<&str> = k.chain().map(|c| c.name).collect();
    let props: Map<String, J> = k.props.iter().map(|p| (p.name.to_string(), prop_json(k, p))).collect();
    let signals: Map<String, J> = k.signals.iter().map(|s| (s.name.to_string(), signal_json(k, s))).collect();
    let methods: Map<String, J> = k
        .methods
        .iter()
        .map(|m| {
            let params: Vec<J> = m.params.iter().map(|(n, t)| json!({"name": n, "type": spec_type(*t), "nullable": false, "optional": false})).collect();
            let returns = match m.ret {
                None => json!({"nullable": false, "type": "void"}),
                Some(t) => json!({"nullable": false, "type": spec_type(t)}),
            };
            let mut j = json!({"dispatch": "generated", "doc": m.doc, "native": m.name, "params": params, "returns": returns});
            let o = j.as_object_mut().expect("method object");
            id_fields(o, kinds::method_wire(k, m));
            (m.name.to_string(), j)
        })
        .collect();
    let slot_ids: Map<String, J> = k.slots.iter().map(|s| (s.name.to_string(), json!(kinds::slot_wire(k, s)))).collect();
    let widget = match (km.core, km.hosted) {
        (None, _) => J::Null,
        (Some(_), true) => json!("presto-widgets"),
        (Some(_), false) => json!("placeholder"),
    };
    json!({
        "a11y": {"name_from": k.a11y.name_from, "name_required": k.a11y.name_required, "role": if k.a11y.role.is_empty() { J::Null } else { json!(k.a11y.role) }},
        "abstract": k.tag.is_none(),
        "bindings": bindings_json(k),
        "chain": chain,
        "children": children_json(k),
        "doc": k.doc,
        "fingerprint": format!("{:08x}", km.wire),
        "id": km.wire,
        "interface": false,
        "lifetime": if k.toplevel { "toplevel" } else { "owned" },
        "methods": methods,
        "native": format!("presto {}", k.name),
        "native_state": k.native_state,
        "present": if k.toplevel { json!("present") } else { J::Null },
        "props": props,
        "provenance": {"kind": "crates/svelters-presto/src/surface/tier0.rs"},
        "signals": signals,
        "slot_ids": slot_ids,
        "text_content": k.text_content,
        "widget": widget,
    })
}

fn range_json(name: &str) -> Option<J> {
    let &(_, min, max) = crate::css::PROP_RANGES.iter().find(|r| r.0 == name)?;
    let mut m = Map::new();
    if min.is_finite() {
        m.insert("min".into(), json!(min));
    }
    if max.is_finite() {
        m.insert("max".into(), json!(max));
    }
    Some(J::Object(m))
}

/// The style table: the kind props that read as CSS are setters (so `<style>` rules can use the CSS
/// names, as on egui), and the engine's own properties are `via: css` with their change class.
fn style_json(all: &[&'static KindSpec]) -> J {
    let mut props: Map<String, J> = Map::new();
    for &k in all {
        for p in k.props.iter().filter(|p| STYLE_SETTERS.contains(&p.name) && crate::css::ENGINE_PROPS.iter().all(|e| e.0 != p.name)) {
            // only props that read as CSS: lengths, numbers, keywords
            let value = match p.ty {
                ValueType::Bool => "bool",
                ValueType::Enum(_) => "enum",
                ValueType::Str => "string",
                ValueType::F64 if matches!(p.name, "flex-grow" | "flex-shrink") => "number",
                ValueType::F64 => "length",
                ValueType::I64 => "int",
                _ => continue,
            };
            let tags: Vec<&str> = all.iter().filter(|kk| kk.tag.is_some() && kk.chain().any(|c| std::ptr::eq(c, k))).map(|kk| kk.tag.unwrap()).collect();
            let e = props.entry(p.name.to_string()).or_insert_with(|| {
                let mut m = Map::new();
                m.insert("change".into(), json!(p.change.name()));
                m.insert("kinds".into(), json!([]));
                m.insert("native".into(), json!(p.name));
                m.insert("prop".into(), json!(p.name));
                m.insert("tags".into(), json!([]));
                m.insert("type".into(), json!(value));
                if let ValueType::Enum(values) = p.ty {
                    m.insert("values".into(), json!(values));
                }
                m.insert("via".into(), json!("setter"));
                if let Some(r) = range_json(p.name) {
                    m.insert("range".into(), r);
                }
                J::Object(m)
            });
            let m = e.as_object_mut().expect("style entry");
            m.get_mut("kinds").and_then(J::as_array_mut).expect("kinds").push(json!(k.name));
            let t = m.get_mut("tags").and_then(J::as_array_mut).expect("tags");
            let mut all: Vec<String> = t.iter().filter_map(|x| x.as_str().map(str::to_string)).chain(tags.iter().map(|s| s.to_string())).collect();
            all.sort();
            all.dedup();
            *t = all.into_iter().map(J::from).collect();
        }
    }
    for &(name, ty, values, change) in crate::css::ENGINE_PROPS {
        let mut m = Map::new();
        m.insert("change".into(), json!(change.name()));
        m.insert("decl_type".into(), json!(ty));
        if !values.is_empty() {
            m.insert("values".into(), json!(values));
        }
        if let Some(r) = range_json(name) {
            m.insert("range".into(), r);
        }
        m.insert("via".into(), json!("css"));
        props.insert(name.to_string(), J::Object(m));
    }
    json!({
        "dialect": "none",
        // the backend runs the Rust CSS engine; `<style>` reaches it as CSS (D9)
        "engine": "backend",
        "media_features": svelters_style::MEDIA_FEATURES.iter().map(|(n, kind, values)| json!({"name": n, "type": kind, "values": values})).collect::<Vec<_>>(),
        "properties": props,
        "pseudo_classes": svelters_style::STATE_PSEUDO_CLASSES.iter().map(|(n, _, neg)| json!({"name": n, "negated": neg})).collect::<Vec<_>>(),
        "setter_selectors": ["type", "class", "id"],
        "units": {"px": {"provenance": "presto points: 1px in a style sheet is one device-independent point", "scale": 1.0}},
    })
}

/// The layout props that are also CSS properties (the egui style table's setters).
const STYLE_SETTERS: &[&str] = &[
    "align-items",
    "align-self",
    "bottom",
    "column-gap",
    "dialog-role",
    "flex-basis",
    "flex-direction",
    "flex-grow",
    "flex-shrink",
    "flex-wrap",
    "gap",
    "height",
    "justify-content",
    "justify-items",
    "left",
    "max-height",
    "max-width",
    "min-height",
    "min-width",
    "position",
    "right",
    "row-gap",
    "status-section",
    "top",
    "visible",
    "width",
];

/// Every id the spec publishes, in kind order.
fn all_ids(k: &KindSpec) -> Vec<u32> {
    let mut v = vec![kinds::kind_wire(k)];
    v.extend(k.props.iter().map(|p| kinds::prop_wire(k, p)));
    v.extend(k.signals.iter().map(|s| kinds::signal_wire(k, s)));
    v.extend(k.methods.iter().map(|m| kinds::method_wire(k, m)));
    v.extend(k.slots.iter().map(|s| kinds::slot_wire(k, s)));
    v
}

/// The whole spec for `all`.
pub fn semantics(all: &Kinds) -> J {
    let mut kinds = Map::new();
    let mut tags = Map::new();
    let mut ids = Vec::new();
    for km in all.iter() {
        let k = km.surface;
        kinds.insert(k.name.to_string(), kind_json(km));
        ids.extend(all_ids(k));
        if let Some(t) = k.tag {
            tags.insert(t.to_string(), json!(k.name));
        }
    }
    let surface: Vec<&'static KindSpec> = all.surface().collect();
    json!({
        "backend": "presto",
        "bind_sources": [{"doc": "A writable field of a Rust store (#[svelters::store]) bound to a native prop: linked in the host, both ways, with no JS per change.", "export": "*", "lowering": "link", "module": "svelters:app/*"}],
        "capabilities": CAPABILITIES,
        "event_aliases": {},
        "framework": "presto (svelters-presto; retained tree, taffy layout, wgpu compositor)",
        "ids": {"scheme": "presto-core FNV-1a-32 of (declaring kind, member, type)", "spec_hash": format!("{:016x}", spec_hash(&ids))},
        "inline_text": {"kind": "span", "prop": "text"},
        "kinds": kinds,
        "method_lookup": "annotated",
        "schema": "svelters-semantics/v0",
        "source": {"generator": "crates/svelters-presto/src/spec.rs", "kinds": "crates/svelters-presto/src/surface/tier0.rs", "widgets": "crates/presto-widgets"},
        "style": style_json(&surface),
        "tags": tags,
    })
}

/// The checked-in text for the built-in registry: pretty JSON with a trailing newline.
pub fn builtin_text() -> String {
    let mut s = serde_json::to_string_pretty(&semantics(&crate::builtin_kinds())).expect("spec serializes");
    s.push('\n');
    s
}
