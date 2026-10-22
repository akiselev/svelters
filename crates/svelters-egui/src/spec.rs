//! The egui semantic spec (`svelters-semantics/v0`) and typings, generated from the kind registry
//! (EGUI.md §6: the binary is the source of truth):
//!
//! - [`semantics`]: the built-in kinds, `metadata/semantics/egui.json` (embedded by the compiler);
//! - [`fragment`]: the provider kinds of this binary (`#[svelters::widget]`, RA1), a fragment the
//!   compiler merges into the built-in spec (`svelters-compile --spec-fragment`, Vite `specFragments`);
//! - [`builtin_typings`]: `packages/native-runtime/types/egui.d.ts`, the built-in elements for
//!   svelte-check (`svelteHTML.IntrinsicElements`, with `on<event>`, `on:<event>` and `bind:` keys);
//! - [`kinds_typings`]: `kinds.d.ts`, the `IntrinsicElements` entries of the provider kinds;
//! - [`behavior_modules`] and [`behaviors_typings`]: the `svelters:behaviors/<namespace>` modules of
//!   the behaviours (`#[svelters::behavior]`, EV2b) and their typings (`behaviors.d.ts`);
//! - [`dump`]: writes the exports (`exports.json`, `app.d.ts`, stubs), `kinds.json`, `kinds.d.ts`,
//!   `behaviors.d.ts` and `behaviors/<namespace>.js`.
//!
//! Each element kind also carries the docs fields `category`, `states`, `a11y`, `paint`,
//! `css_hidden` and `css_note` ([`kinds::docs`], DOC1), which `tools/docs/egui/gen_reference.py` renders into the
//! widget reference; the compiler ignores them.
//!
//! `tests/spec.rs` fails when `egui.json` or `egui.d.ts` is stale
//! (`SVELTERS_UPDATE=1 cargo test -p svelters-egui --test spec` rewrites them). Writer lane: RUSTAPI.

use crate::kinds::{self, Def, KINDS, KindDef, KindRegistry, Native, PropDef, ProviderKind, Ty};
use serde_json::{Map, Value as J, json};
use std::fmt::Write as _;
use std::path::Path;
use svelters_abi::*;


/// Where the built-in element typings live, relative to the workspace root.
pub const TYPINGS_PATH: &str = "packages/native-runtime/types/egui.d.ts";

/// Where the checked-in spec lives, relative to the workspace root.
pub const SPEC_PATH: &str = "metadata/semantics/egui.json";

fn ty_name(ty: Ty) -> &'static str {
    match ty {
        Ty::Bool => "bool",
        Ty::F64 => "f64",
        Ty::I64 => "i64",
        Ty::Str => "string",
        Ty::Enum(_) => "enum",
        Ty::StrList | Ty::IdList => "list",
        Ty::Snippet => "other",
        Ty::Provider => "other",
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
    if p.ty == Ty::StrList {
        m.insert("of".into(), json!("string"));
    }
    if p.ty == Ty::IdList {
        m.insert("of".into(), json!("i64"));
    }
    m.insert("writable".into(), json!(true));
    J::Object(m)
}

fn kind_json(k: &KindDef) -> J {
    let index = kinds::kind_index(k.id).unwrap();
    let chain: Vec<&str> = kinds::chain(index).into_iter().map(|i| kinds::kind(i).id).collect();
    let provider = KindRegistry::global().provider(index);
    let props: Map<String, J> = k
        .props
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let mut j = prop_json(k, p);
            if let Some(pk) = provider {
                provider_prop(&mut j, pk, i);
            }
            (p.name.to_string(), j)
        })
        .collect();
    let signals: Map<String, J> = k
        .signals
        .iter()
        .map(|s| {
            let args: Vec<J> = s
                .args
                .iter()
                .map(|(n, t)| {
                    let ty = match (provider, t) {
                        (Some(pk), Ty::Provider) => {
                            let i = pk.entry.events.iter().position(|e| e.name == s.name).unwrap_or(0);
                            pk.event_type(i).map_or("other", |t| slot_spec(&t.slot).0)
                        }
                        _ => ty_name(*t),
                    };
                    json!({"name": n, "type": ty})
                })
                .collect();
            (
                s.name.to_string(),
                json!({
                    "args": args,
                    "disconnect_on_drop": true,
                    "doc": s.doc,
                    "handler_result": if s.sync { "bool_stop" } else { "none" },
                    "js_event": s.name,
                    "name": s.name,
                    // EV1: the DOM-style events propagate along the scene and take capture listeners
                    "phases": if kinds::is_dom_event(s.name) { json!(["capture"]) } else { json!([]) },
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
    // the scene never echoes a host write. Provider kinds declare none: their bound props are
    // `exact`/`delivery` (written only through `WidgetCx::set`), so `bind:` lowers onto observation.
    // The value is the `changed` argument named like the prop (WG1: `scroll-area`'s carries both
    // offsets), else argument 0.
    // WG2a: a kind without `changed` binds through its first signal with an argument named like the
    // prop (`action`'s `toggled`, as on GTK and Qt).
    let bind_signal = |p: &PropDef| {
        k.signals.iter().find(|s| s.name == "changed").or_else(|| k.signals.iter().find(|s| s.args.iter().any(|(n, _)| *n == p.name)))
    };
    let bindings: Map<String, J> = k
        .props
        .iter()
        .filter(|p| p.native == Native::Widget && provider.is_none())
        .map(|p| {
            let sig = bind_signal(p);
            let event = sig.map_or("changed", |s| s.name);
            let arg = sig.map_or(&[][..], |s| s.args).iter().position(|(n, _)| *n == p.name).unwrap_or(0);
            // the widget itself changes a written value (a slider's range, a scroll offset's extent)
            let normalizes = matches!(p.key, kinds::P::Value | kinds::P::ScrollX | kinds::P::ScrollY)
                // WG2b: a range slider keeps low <= high within its bounds; a radio button with a
                // `value` derives `checked` from `group`
                || matches!(p.key, kinds::P::Low | kinds::P::High)
                || (k.id == "RadioButton" && p.key == kinds::P::Checked);
            let mut b = json!({
                    "compare_before_set": true,
                    "direction": "two_way",
                    "echo": "none",
                    "event": event,
                    "lifetime": {"on_destroy": "idempotent_disconnect", "on_native_death": "end_binding", "on_stale_update": "warn_noop"},
                    "natively_normalizes": normalizes,
                    "property": p.name,
                    "provenance": format!("svelters-egui: the {} widget writes {} into the node during the pass; `{event}` carries it (arg 0), set_prop never announces", k.native, p.name),
                    "read_back": normalizes,
                    "sync_signals_on_set": [],
                    "value": {"arg": arg},
                });
            // CX3: `bind:<prop>|commit` updates its source on the kind's `commit` signal when it has
            // the change event's shape (the coordinator's seam: the committed value where the change
            // event carries the live one). A `commit` of another shape (a table's cell edit) is not
            // a commit of this prop.
            if let (Some(c), Some(s)) = (k.signals.iter().find(|s| s.name == "commit"), sig)
                && c.args == s.args
            {
                b["commit"] = json!({"event": "commit", "arg": arg});
            }
            (p.name.to_string(), b)
        })
        .collect();
    let children = k.slot.map(|slot| {
        json!({
            "default_slot": slot,
            "policy": "ordered",
            "slots": {slot: {"accepts": kinds::slot_accepts(k.id), "attrs": {}, "many": true, "order": "before_sibling", "route_by_kind": false}},
        })
    });
    let mut kj = json!({
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
        "provenance": {"kind": if provider.is_some() { k.native } else { "crates/svelters-egui/src/kinds/builtin.rs" }},
        "signals": signals,
        "text_content": k.text_content,
    });
    // WG2a: `context-menu` and `popup` attach to the element they are declared in
    if let Some(t) = kinds::attaches_to(k.id) {
        kj.as_object_mut().expect("kind object").insert("attaches_to".into(), json!(t));
    }
    kj
}

// ---- DOC1: the docs fields (`kinds/docs.rs`) ----

/// The pseudo-classes that match built-in state `name` (`StateBits::BUILTIN`): `(name, negated)`.
fn pseudo_classes(name: &str) -> Vec<(&'static str, bool)> {
    let Some((_, bit)) = crate::scene::StateBits::BUILTIN.iter().find(|(n, _)| *n == name) else {
        panic!("svelters-egui: kinds/docs.rs names unknown state {name}");
    };
    svelters_style::STATE_PSEUDO_CLASSES.iter().filter(|(_, m, _)| m.0 == bit.0).map(|(n, _, neg)| (*n, *neg)).collect()
}

fn state_json(name: &str, source: kinds::docs::Source, when: &str) -> J {
    let pc = pseudo_classes(name);
    let pick = |neg: bool| pc.iter().filter(|(_, n)| *n == neg).map(|(p, _)| format!(":{p}")).collect::<Vec<_>>();
    json!({"name": name, "pseudo": pick(false), "negated": pick(true), "source": source.name(), "when": when})
}

/// The built-in states a kind gets: its walk's (`style.rs`), then its own.
fn states_json(d: &kinds::docs::KindDoc, has_enabled: bool) -> J {
    use kinds::docs::{Focus, Source, Walk};
    let mut v: Vec<J> = Vec::new();
    match d.walk {
        Walk::Leaf(focus) => {
            v.push(state_json("hovered", Source::Response, "the pointer is over it"));
            v.push(state_json("pressed", Source::Response, "the primary button is down on it"));
            match focus {
                Focus::Yes => v.push(state_json("focused", Source::Response, "it has keyboard focus")),
                Focus::Clickable => v.push(state_json("focused", Source::Response, "it takes clicks (`onclicked` is set) and has keyboard focus")),
                Focus::Never => {}
            }
            v.push(state_json("focus-within", Source::Scene, "it, or an element inside it, has keyboard focus"));
        }
        Walk::Container => {
            v.push(state_json("hovered", Source::Response, "the pointer is over its box"));
            v.push(state_json("pressed", Source::Response, "the primary button is down over its box"));
            v.push(state_json("focus-within", Source::Scene, "an element inside it has keyboard focus"));
        }
        Walk::None => {}
    }
    if d.walk != Walk::None {
        if has_enabled {
            v.push(state_json("disabled", Source::Prop, "`enabled` is false"));
        }
        v.push(state_json("exiting", Source::Scene, "it plays an out transition before it is removed"));
    }
    for s in d.states {
        v.push(state_json(s.name, s.source, s.when));
    }
    J::Array(v)
}

fn a11y_json(a: &kinds::docs::A11yDoc) -> J {
    let variants: Vec<J> = a.variants.iter().map(|(role, when)| json!({"role": role, "when": when})).collect();
    json!({
        "actions": a.actions,
        "author_id": if a.decorated { Some("anchor") } else { None },
        "invalid": a.decorated,
        "name_from": a.name_from,
        "note": a.note,
        "role": a.role,
        "value": a.value,
        "variants": variants,
    })
}

/// Adds the docs fields of kind `k` to its spec object `kj`.
fn add_docs(kj: &mut J, k: &KindDef, provider: Option<&ProviderKind>) {
    let own;
    let d = match provider {
        Some(pk) => {
            own = kinds::docs::provider_doc(pk.entry.caps.contains(crate::provider::Caps::FOCUSABLE));
            &own
        }
        None => kinds::docs::kind_doc(k.id).unwrap_or_else(|| panic!("svelters-egui: kind {} has no row in kinds/docs.rs", k.id)),
    };
    // `:disabled` follows `enabled`, which only kinds under `Widget` have (not `window`)
    let has_enabled = kinds::kind_index(k.id).is_none_or(|i| kinds::find_prop(i, "enabled").is_some());
    let m = kj.as_object_mut().expect("kind object");
    m.insert("a11y".into(), a11y_json(&d.a11y));
    m.insert("category".into(), json!(d.category.name()));
    m.insert("css_hidden".into(), json!(kinds::docs::css_hidden(k.id)));
    m.insert("css_note".into(), json!(d.css_note));
    m.insert("paint".into(), json!(d.paint.name()));
    m.insert("states".into(), states_json(d, has_enabled));
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
                    Ty::Str => "string",
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
    // CS5e: what the egui cascade computes with no kind prop behind it (taffy fields the kind table
    // has no prop for, and paint). `via: css` with a `decl_type`, as the decl dialect's entries: the
    // compiler leaves them to the engine, which computes them because egui has no toolkit CSS.
    for &(name, ty, values) in crate::style::ENGINE_PROPS {
        let mut m = Map::new();
        m.insert("decl_type".into(), json!(ty));
        if !values.is_empty() {
            m.insert("values".into(), json!(values));
        }
        m.insert("via".into(), json!("css"));
        props.insert(name.to_string(), J::Object(m));
    }
    json!({
        "dialect": "none",
        // CS5e: the backend runs the CSS engine; `<style>` reaches it as CSS (D9 on egui)
        "engine": "backend",
        "properties": props,
        // DOC1: the state pseudo-classes and the built-in state each reads (`:enabled` is
        // `:disabled` negated)
        "pseudo_classes": svelters_style::STATE_PSEUDO_CLASSES
            .iter()
            .map(|(n, m, neg)| {
                let state = crate::scene::StateBits::BUILTIN.iter().find(|(_, b)| b.0 == m.0).map(|(s, _)| *s);
                json!({"name": n, "negated": neg, "state": state})
            })
            .collect::<Vec<_>>(),
        "setter_selectors": ["type", "class", "id"],
        "units": {"px": {"provenance": "egui points: 1px in a style sheet is one point", "scale": 1.0}},
    })
}

/// The whole spec.
pub fn semantics() -> J {
    // OW0: a kind with an own widget lists the widget's extra props and signals too
    let reg = KindRegistry::global();
    let kinds: Map<String, J> = KINDS
        .iter()
        .enumerate()
        .map(|(i, k)| {
            let mut j = kind_json(reg.spec_kind(i));
            add_docs(&mut j, k, None);
            (k.id.to_string(), j)
        })
        .collect();
    let tags: Map<String, J> = KINDS.iter().filter_map(|k| k.tag.map(|t| (t.to_string(), json!(k.id)))).collect();
    json!({
        "backend": "egui",
        // CX3: the lowering table. `bind:prop={store.field}` on a Rust store (E3) is a native link in
        // the host (`host.link`): no JS per change. Draft sources come from provider fragments.
        "bind_sources": [{
            "doc": "A writable field of a Rust store (#[svelters::store]) bound to a native prop: linked in the host, both ways, with no JS per change.",
            "export": "*",
            "lowering": "link",
            "module": "svelters:app/*",
        }],
        "event_aliases": {},
        "framework": "egui 0.37 (svelters-egui; taffy 0.15 layout)",
        "kinds": kinds,
        "method_lookup": "annotated",
        "schema": "svelters-semantics/v0",
        "source": {"kinds": "crates/svelters-egui/src/kinds/builtin.rs", "generator": "crates/svelters-egui/src/spec.rs"},
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

// ---- RA1: provider kinds ----

/// The spec type of a provider prop's slot, with its enum values.
fn slot_spec(slot: &SlotDesc) -> (&'static str, Option<Vec<String>>) {
    match slot.tag {
        SVL_BOOL => ("bool", None),
        SVL_F64 => ("f64", None),
        SVL_I64 => ("i64", None),
        SVL_STR => ("string", None),
        SVL_ENUM => {
            let values = svelters_host::exports::schema().type_desc(slot.ty).map(|d| d.members.iter().map(|m| m.name.clone()).collect());
            ("enum", values)
        }
        _ => ("other", None),
    }
}

/// A value as JSON (defaults in the spec): enums by name, records and handles as `null`.
fn value_json(v: &Value) -> J {
    match v {
        Value::Null => J::Null,
        Value::Bool(b) => json!(b),
        Value::I64(i) => json!(i),
        Value::U64(u) => json!(u),
        Value::F64(f) => json!(f),
        Value::Str(s) => json!(s),
        Value::Enum { ty, value } => svelters_host::exports::schema()
            .type_desc(*ty)
            .and_then(|d| d.members.iter().find(|m| m.value == *value).map(|m| json!(m.name)))
            .unwrap_or(json!(value)),
        Value::List(items) => J::Array(items.iter().map(value_json).collect()),
        Value::Map(entries) => J::Object(entries.iter().map(|(k, v)| (k.clone(), value_json(v))).collect()),
        _ => J::Null,
    }
}

/// Fills a provider prop's type, values, range, default and observability from its Rust type and its
/// widget's default.
fn provider_prop(j: &mut J, pk: &ProviderKind, i: usize) {
    let m = j.as_object_mut().expect("prop object");
    let Some(t) = pk.types.get(i) else { return };
    let (ty, values) = slot_spec(&t.slot);
    m.insert("type".into(), json!(ty));
    if let Some(v) = values {
        m.insert("values".into(), json!(v));
    }
    if ty == "i64" && t.slot.min.is_finite() && t.slot.max.is_finite() {
        m.insert("range".into(), json!({"min": t.slot.min, "max": t.slot.max}));
    }
    let w = (pk.entry.new)();
    let default = svelters_host::exports::__private::with_cx(pk.entry.namespace, |cx| w.get_prop(i, cx)).map(|v| value_json(&v)).unwrap_or(J::Null);
    m.insert("default".into(), default);
    m.insert("ts".into(), json!(t.ts));
    let p = &pk.entry.props[i];
    if p.required {
        m.insert("required".into(), json!(true));
    }
    if p.bind {
        // Written only through `WidgetCx::set`, which announces every change: exact by construction.
        m.insert(
            "observe".into(),
            json!({"class": "exact", "fact": format!("egui:{}:{}", pk.entry.id, p.name), "mechanism": "widget", "verified": "delivery"}),
        );
    }
}

/// The provider kinds of this binary as a `svelters-semantics/v0` fragment: `kinds` and `tags` only.
/// `svelters-compile --spec-fragment kinds.json` merges it into the built-in egui spec (a tag or kind
/// that is already there is an error, never a shadow). Behaviours (EV2b) are not elements: they are
/// listed under `behaviors` (for tools; the compiler ignores the key), each with its kind, the
/// `svelters:behaviors/<ns>` module and export, and `attaches_to`.
pub fn fragment() -> J {
    let reg = KindRegistry::global();
    let kinds: Map<String, J> = reg
        .elements()
        .map(|p| {
            let mut j = kind_json(p.def);
            add_docs(&mut j, p.def, Some(p));
            (p.entry.id.to_string(), j)
        })
        .collect();
    let tags: Map<String, J> = reg
        .elements()
        .map(|p| (p.entry.tag.to_string(), json!(p.entry.id)))
        .collect();
    let caps: Map<String, J> = reg
        .elements()
        .map(|p| (p.entry.id.to_string(), json!(p.entry.caps.names())))
        .collect();
    let behaviors: Map<String, J> = reg
        .behaviors()
        .map(|p| {
            let mut j = kind_json(p.def);
            let m = j.as_object_mut().expect("kind object");
            m.insert("attaches_to".into(), json!(p.entry.attaches_to));
            m.insert(
                "module".into(),
                json!(format!("svelters:behaviors/{}", p.entry.namespace)),
            );
            m.insert("export".into(), json!(p.entry.behavior_name()));
            m.insert("tag".into(), json!(p.entry.tag));
            (p.entry.id.to_string(), j)
        })
        .collect();
    let mut f = json!({
        "backend": "egui",
        "behaviors": behaviors,
        "caps": caps,
        "fragment": true,
        "kinds": kinds,
        "schema": "svelters-semantics/v0",
        "source": {"generator": "crates/svelters-egui/src/spec.rs (svelters_egui::dump)"},
        "tags": tags,
    });
    // CX3: draft bind sources the app's crates declare (only when there are any, so a fragment of
    // kinds alone is unchanged)
    let mut sources: Vec<&crate::provider::BindSourceDef> = inventory::iter::<crate::provider::BindSourceDef>.into_iter().collect();
    sources.sort_by_key(|b| (b.module, b.export));
    if !sources.is_empty() {
        f["bind_sources"] = sources
            .iter()
            .map(|b| json!({"doc": b.doc, "export": b.export, "get": b.get, "lowering": "draft", "module": b.module, "set": b.set}))
            .collect();
    }
    f
}

/// [`fragment`] as written to `kinds.json`.
pub fn fragment_text() -> String {
    let mut s = serde_json::to_string_pretty(&fragment()).expect("fragment json");
    s.push('\n');
    s
}

fn ts_doc(out: &mut String, indent: &str, doc: &str) {
    let doc = doc.trim();
    if !doc.is_empty() {
        let _ = writeln!(out, "{indent}/** {} */", doc.replace("*/", "*\\/").replace('\n', " "));
    }
}

fn ts_key(name: &str) -> String {
    if name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') && !name.starts_with(|c: char| c.is_ascii_digit()) {
        name.to_string()
    } else {
        format!("'{name}'")
    }
}

fn ts_of(ty: Ty) -> String {
    match ty {
        Ty::Bool => "boolean".into(),
        Ty::F64 | Ty::I64 => "number".into(),
        Ty::Str => "string".into(),
        Ty::Enum(values) => values.iter().map(|v| format!("'{v}'")).collect::<Vec<_>>().join(" | "),
        Ty::StrList => "string[]".into(),
        Ty::IdList => "number[]".into(),
        Ty::Snippet => "((...args: any[]) => any) | null".into(),
        Ty::Provider => "unknown".into(),
    }
}

/// The members of one kind's own props, handlers and bindings (`indent`-prefixed lines).
fn ts_members(out: &mut String, indent: &str, k: &KindDef, provider: Option<&ProviderKind>) {
    for (i, p) in k.props.iter().enumerate() {
        ts_doc(out, indent, p.doc);
        let (ty, optional) = match provider {
            Some(pk) => (pk.types.get(i).map_or("unknown".to_string(), |t| t.ts.clone()), !pk.entry.props[i].required),
            None => {
                let t = ts_of(p.ty);
                (if p.nullable { format!("{t} | null") } else { t }, true)
            }
        };
        let _ = writeln!(out, "{indent}{}{}: {ty};", ts_key(p.name), if optional { "?" } else { "" });
        if p.native == Native::Widget {
            let _ = writeln!(out, "{indent}{}?: {ty};", ts_key(&format!("bind:{}", p.name)));
        }
    }
    for (i, s) in k.signals.iter().enumerate() {
        ts_doc(out, indent, s.doc);
        let args: Vec<String> = match provider {
            Some(pk) => vec![pk.event_type(i).map_or("unknown".to_string(), |t| t.ts.clone())],
            None => s.args.iter().map(|(_, t)| ts_of(*t)).collect(),
        };
        // A plain signal's handlers get its arguments; a sync one's get a `SvlEvent` first (runtime
        // `dispatch` / `dispatch_sync`).
        let names: Vec<String> = match provider {
            Some(_) => vec!["value".to_string()],
            None => s.args.iter().map(|(n, _)| n.replace('-', "_")).collect(),
        };
        let params: Vec<String> = names.iter().zip(&args).map(|(n, t)| format!("{n}: {t}")).collect();
        // EV1: the DOM-style events get a `SvlUiEvent` (target, phase, point, key) and a capture form
        if kinds::is_dom_event(s.name) {
            let _ = writeln!(out, "{indent}{}?: (e: SvlUiEvent) => void;", ts_key(&format!("on{}", s.name)));
            let _ = writeln!(out, "{indent}{}?: (e: SvlUiEvent) => void;", ts_key(&format!("on{}capture", s.name)));
            continue;
        }
        let params = if s.sync { std::iter::once(format!("e: SvlEvent<[{}]>", args.join(", "))).chain(params).collect::<Vec<_>>().join(", ") } else { params.join(", ") };
        let _ = writeln!(out, "{indent}{}?: ({params}) => void;", ts_key(&format!("on{}", s.name)));
    }
}

const TYPINGS_HEADER: &str = "// @generated by crates/svelters-egui/src/spec.rs from the egui kind registry. Do not edit.\n";

const TYPINGS_PRELUDE: &str = r#"// An egui app's tsconfig: "types": ["@svelters/native-runtime/types/egui"] (instead of
// "@svelters/native-runtime/types", GTK's), plus the app's dumped .svelters/app/{app,kinds}.d.ts.
import type { Component, ComponentProps, SvelteComponent } from 'svelte';

/** What a sync signal's handler gets first; the signal's arguments follow it (and are its `args`). A plain
 * signal's handler gets the arguments only. */
export interface SvlEvent<A extends unknown[] = unknown[]> {
  type: string;
  target: unknown;
  args: A;
  preventDefault(): void;
  stopPropagation(): void;
}

/** EV1: what a DOM-style event's handler gets (`pointerdown`, `click`, `wheel`, `keydown`, `focusin`, ...):
 * dispatched along the scene in capture, target and bubble order. */
export interface SvlUiEvent extends SvlEvent {
  /** The node the event is for; `currentTarget` is the node whose listener runs. */
  target: unknown;
  currentTarget: unknown;
  /** 1 capture, 2 at the target, 3 bubble. */
  eventPhase: 1 | 2 | 3;
  bubbles: true;
  cancelable: boolean;
  defaultPrevented: boolean;
  stopImmediatePropagation(): void;
  /** Pointer events and `wheel`: the point in the window's (or slot's viewport's) points. */
  x?: number;
  y?: number;
  clientX?: number;
  clientY?: number;
  /** Pointer events: 0 primary, 1 middle, 2 secondary, 3 back, 4 forward. */
  button?: number;
  /** `wheel`: the scroll in points. */
  deltaX?: number;
  deltaY?: number;
  /** Key events: egui's key name (`A`, `Enter`, `ArrowDown`), and whether it is an auto-repeat. */
  key?: string;
  repeat?: boolean;
  /** Focus events: the node focus came from (`focusin`) or went to (`focusout`). */
  relatedTarget?: unknown;
  shiftKey: boolean;
  ctrlKey: boolean;
  altKey: boolean;
  metaKey: boolean;
}

type ClassValue = string | null | undefined | false | ClassValue[] | { [name: string]: unknown };
/** Styling attributes every native element takes. */
export type StyleAttrs = {
  class?: ClassValue;
  style?: string | null;
  id?: string | null;
  [directive: `class:${string}`]: unknown;
  [directive: `style:${string}`]: string | number | null | undefined;
};
/** `on:<event>` per `on<event>` handler: a listener, or (bare) FW1's forwarding of the component's `on<event>` prop. */
export type OnDirectives<P> = { [K in keyof P as K extends `on${infer N}` ? `on:${N}` : never]?: P[K] };
/** An egui element's attributes: its props, handlers and `bind:`s, plus styling and `on:` directives. */
export type EguiAttrs<P> = StyleAttrs & OnDirectives<P> & P;

"#;

const TYPINGS_POSTLUDE: &str = r#"
type ElementAttrs<Tag extends string> = Tag extends keyof svelteHTML.IntrinsicElements ? svelteHTML.IntrinsicElements[Tag] : {};
/** FW1: the `on<Event>` callback prop a bare `on:<Event>` on `<Tag>` forwards, typed as the element's own handler. */
export type Forward<Tag extends string, Event extends string> = {
  [K in `on${Event}`]?: K extends keyof ElementAttrs<Tag> ? NonNullable<ElementAttrs<Tag>[K]> : (event: any) => void;
};
/** FW1: the `on<Event>` callback prop a bare `on:<Event>` on a child component forwards, typed as the child's prop. */
export type ForwardFrom<C extends Component<any, any> | SvelteComponent, Event extends string> = {
  [K in `on${Event}`]?: K extends keyof ComponentProps<C> ? NonNullable<ComponentProps<C>[K]> : (...args: any[]) => void;
};

type WithAttrs<E> = { [K in keyof E]: EguiAttrs<E[K]> };
// Tags that are also HTML elements keep svelte-check's own HTML entry, which `extends` cannot
// override; it merges `HTMLAttributes` into each of them, so their egui props go there.
type EguiHtmlNamedProps = EguiAttrs<EguiElements['button']> & EguiAttrs<EguiElements['label']> & EguiAttrs<EguiElements['table']>;

declare global {
  namespace svelteHTML {
    interface IntrinsicElements extends WithAttrs<EguiElements> {}
    interface HTMLAttributes<T extends EventTarget = any> extends EguiHtmlNamedProps {}
  }
}
"#;

/// `packages/native-runtime/types/egui.d.ts`: the built-in egui elements for svelte-check, with
/// `on<event>` handlers, `on:<event>` directives (FW1) and `bind:` keys. An egui app lists
/// `@svelters/native-runtime/types/egui` in its tsconfig `types` (instead of
/// `@svelters/native-runtime/types`, which declares GTK's elements); `kinds.d.ts` adds the provider
/// kinds of the app's binary on top ([`kinds_typings`]).
pub fn builtin_typings() -> String {
    let mut s = String::from(TYPINGS_HEADER);
    s.push_str(TYPINGS_PRELUDE);
    let widget = kinds::kind_index("Widget").expect("Widget kind");
    s.push_str("/** Props every element placed in a window has (the `Widget` base). */\nexport interface EguiWidgetProps {\n");
    ts_members(&mut s, "  ", kinds::kind(widget), None);
    s.push_str("}\n\n/** The built-in egui elements, by tag. */\nexport interface EguiElements {\n");
    for (i, k) in KINDS.iter().enumerate() {
        let Some(tag) = k.tag else { continue };
        ts_doc(&mut s, "  ", k.doc);
        let bases: Vec<&KindDef> = kinds::chain(i).into_iter().skip(1).map(kinds::kind).collect();
        let base = if bases.iter().any(|b| b.id == "Widget") { "EguiWidgetProps & " } else { "" };
        let _ = writeln!(s, "  {}: {base}{{", ts_key(tag));
        for b in bases.iter().rev().filter(|b| b.id != "Widget") {
            ts_members(&mut s, "    ", b, None);
        }
        ts_members(&mut s, "    ", KindRegistry::global().spec_kind(i), None);
        s.push_str("  };\n");
    }
    s.push_str("}\n");
    s.push_str(TYPINGS_POSTLUDE);
    s
}

/// `kinds.d.ts`: the `svelteHTML.IntrinsicElements` entries of this binary's provider kinds (on top of
/// [`builtin_typings`]), their named prop types imported from `svelters:app/<namespace>` (`app.d.ts`).
pub fn kinds_typings() -> String {
    let reg = KindRegistry::global();
    let mut s = String::from(TYPINGS_HEADER);
    s.push_str("import type { EguiAttrs, EguiWidgetProps, SvlEvent } from '@svelters/native-runtime/types/egui';\n");
    let mut namespaces: Vec<&str> = reg.elements().map(|p| p.entry.namespace).collect();
    namespaces.sort_unstable();
    namespaces.dedup();
    let schema = svelters_host::exports::schema();
    for ns in namespaces {
        if let Some(m) = schema.module(ns) {
            let names: Vec<&str> = m.types.iter().map(|t| t.name.as_str()).collect();
            if !names.is_empty() {
                let _ = writeln!(s, "import type {{ {} }} from 'svelters:app/{ns}';", names.join(", "));
            }
        }
    }
    s.push_str("\ndeclare global {\n  namespace svelteHTML {\n    interface IntrinsicElements {\n");
    for p in reg.elements() {
        ts_doc(&mut s, "      ", p.entry.docs);
        let _ = writeln!(s, "      {}: EguiAttrs<EguiWidgetProps & {{", ts_key(p.entry.tag));
        ts_members(&mut s, "        ", p.def, Some(p));
        s.push_str("      }>;\n");
    }
    s.push_str("    }\n  }\n}\n\nexport {};\n");
    s
}

/// The namespaces that have behaviours, sorted.
fn behavior_namespaces(reg: &KindRegistry) -> Vec<&'static str> {
    let mut ns: Vec<&'static str> = reg.behaviors().map(|p| p.entry.namespace).collect();
    ns.sort_unstable();
    ns.dedup();
    ns
}

/// The `svelters:behaviors/<namespace>` modules (EV2b, ARCH-EMBED §8.5): `(namespace, source)`, one
/// `nativeBehavior` Svelte action per behaviour. Vite resolves the import to
/// `<root>/.svelters/app/behaviors/<namespace>.js`.
pub fn behavior_modules() -> Vec<(&'static str, String)> {
    let reg = KindRegistry::global();
    behavior_namespaces(reg)
        .into_iter()
        .map(|ns| {
            let mut s = format!("// svelters:behaviors/{ns} - @generated by crates/svelters-egui/src/spec.rs from the egui kind registry. Do not edit.\n");
            s.push_str("import { nativeBehavior } from '@svelters/native-runtime';\n");
            for p in reg.behaviors().filter(|p| p.entry.namespace == ns) {
                s.push('\n');
                ts_doc(&mut s, "", p.entry.docs);
                let events: Vec<String> = p.entry.events.iter().map(|e| format!("'{}'", e.name)).collect();
                let _ = writeln!(s, "export const {} = nativeBehavior('{}', [{}]);", p.entry.behavior_name().unwrap_or("?"), p.entry.tag, events.join(", "));
            }
            (ns, s)
        })
        .collect()
}

/// `behaviors.d.ts`: an ambient `declare module 'svelters:behaviors/<namespace>'` per namespace with
/// behaviours: each export is an `Action` whose params are the behaviour's props and its events'
/// `on<event>` callbacks; the named types come from `svelters:app/<namespace>`.
pub fn behaviors_typings() -> String {
    let reg = KindRegistry::global();
    let schema = svelters_host::exports::schema();
    let mut s = String::from(TYPINGS_HEADER);
    for ns in behavior_namespaces(reg) {
        let _ = writeln!(s, "\ndeclare module 'svelters:behaviors/{ns}' {{");
        s.push_str("  import type { Action } from 'svelte/action';\n");
        if let Some(m) = schema.module(ns) {
            let names: Vec<&str> = m.types.iter().map(|t| t.name.as_str()).collect();
            if !names.is_empty() {
                let _ = writeln!(
                    s,
                    "  import type {{ {} }} from 'svelters:app/{ns}';",
                    names.join(", ")
                );
            }
        }
        for p in reg.behaviors().filter(|p| p.entry.namespace == ns) {
            let name = p.entry.behavior_name().unwrap_or("?");
            let _ = writeln!(
                s,
                "\n  /** The params of `use:{name}`: its props and its events' callbacks. */"
            );
            let _ = writeln!(s, "  export interface {}Params {{", p.entry.name);
            ts_members(&mut s, "    ", p.def, Some(p));
            s.push_str("  }\n");
            ts_doc(&mut s, "  ", p.entry.docs);
            let optional = if p.entry.props.iter().any(|q| q.required) {
                ""
            } else {
                " | undefined"
            };
            let _ = writeln!(
                s,
                "  export const {name}: Action<any, {}Params{optional}>;",
                p.entry.name
            );
        }
        s.push_str("}\n");
    }
    s
}

// ZU3: build-time dump of typings and specs (`--dump-types`), not run per frame.
#[allow(clippy::disallowed_methods)]
/// Writes this binary's typings and specs into `dir` (an app's `.svelters/app`): the exports
/// (`exports.json`, `app.d.ts`, one stub per module; `svelters_host::exports::dump`), `kinds.json`
/// ([`fragment`]), `kinds.d.ts` ([`kinds_typings`]) and, when the binary has behaviours,
/// `behaviors.d.ts` ([`behaviors_typings`]) and `behaviors/<ns>.js` ([`behavior_modules`]). Only
/// changed files are rewritten.
pub fn dump(dir: &Path) -> std::io::Result<svelters_host::exports::DumpReport> {
    let mut report = svelters_host::exports::dump(dir)?;
    let mut files = vec![
        ("kinds.json".to_string(), fragment_text()),
        ("kinds.d.ts".to_string(), kinds_typings()),
    ];
    let modules = behavior_modules();
    if !modules.is_empty() {
        files.push(("behaviors.d.ts".to_string(), behaviors_typings()));
        std::fs::create_dir_all(dir.join("behaviors"))?;
    }
    files.extend(
        modules
            .into_iter()
            .map(|(ns, text)| (format!("behaviors/{ns}.js"), text)),
    );
    for (name, text) in files {
        let path = dir.join(name);
        if std::fs::read_to_string(&path).ok().as_deref() == Some(text.as_str()) {
            report.unchanged += 1;
        } else {
            std::fs::write(&path, text)?;
            report.written.push(path);
        }
    }
    Ok(report)
}
