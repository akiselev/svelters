//! winmd-tool: the Rust half of the WinUI metadata pipeline (`tools/metadata/winui/regen.sh`).
//!
//! ```text
//! winmd-tool dump <Namespace.Class>... -- <winmd files or dirs>...
//!     JSON on stdout: each class with its base chain, plus the closure of what their members
//!     refer to (event delegates, event-args classes, enums, structs). Read by ingest.py.
//! winmd-tool bindgen --out <file.rs> --filter-file <filter.txt> -- <winmd files or dirs>...
//!     windows-bindgen over the given metadata (plus the bundled Windows SDK metadata), restricted
//!     to the member-level filter written by gen_backend.py.
//! ```
//!
//! Properties and events are recovered from `SpecialName` accessor methods (`get_`/`put_`/`add_`),
//! as windows-bindgen does; `windows-metadata` does not expose the ECMA Property/Event tables.
use serde_json::{Map, Value as Json, json};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use windows_metadata::reader::*;
use windows_metadata::*;

fn trim_tick(s: &str) -> &str {
    s.split('`').next().unwrap_or(s)
}

/// Type spelling used in the JSON: primitives by Rust-ish name, WinRT types by full name.
fn tn(t: &Type) -> String {
    match t {
        Type::Void => "void".into(),
        Type::Bool => "bool".into(),
        Type::Char => "char16".into(),
        Type::I8 => "i8".into(),
        Type::U8 => "u8".into(),
        Type::I16 => "i16".into(),
        Type::U16 => "u16".into(),
        Type::I32 => "i32".into(),
        Type::U32 => "u32".into(),
        Type::I64 => "i64".into(),
        Type::U64 => "u64".into(),
        Type::F32 => "f32".into(),
        Type::F64 => "f64".into(),
        Type::ISize => "isize".into(),
        Type::USize => "usize".into(),
        Type::String => "string".into(),
        Type::Object => "object".into(),
        Type::ClassName(n) | Type::ValueName(n) => {
            let mut s = format!("{}.{}", n.namespace, trim_tick(&n.name));
            if !n.generics.is_empty() {
                s.push('<');
                s.push_str(&n.generics.iter().map(tn).collect::<Vec<_>>().join(", "));
                s.push('>');
            }
            s
        }
        Type::Array(t) => format!("{}[]", tn(t)),
        Type::Generic(n, _) => n.clone(),
        Type::RefMut(t) => format!("out {}", tn(t)),
        Type::RefConst(t) => tn(t),
        other => format!("{other:?}"),
    }
}

/// Every named WinRT type mentioned in `t` (generic arguments included).
fn named(t: &Type, out: &mut Vec<String>) {
    match t {
        Type::ClassName(n) | Type::ValueName(n) => {
            out.push(format!("{}.{}", n.namespace, trim_tick(&n.name)));
            for g in &n.generics {
                named(g, out);
            }
        }
        Type::Array(t) | Type::RefMut(t) | Type::RefConst(t) => named(t, out),
        _ => {}
    }
}

fn attr_value(v: &Value) -> Json {
    match v {
        Value::Utf8(s) | Value::Utf16(s) => json!(s),
        Value::TypeName(t) => json!(format!("{}.{}", t.namespace, t.name)),
        Value::EnumValue(t, inner) => json!(format!("{}.{}", t.name, match &**inner {
            Value::I32(i) => i.to_string(),
            Value::U32(i) => i.to_string(),
            other => format!("{other:?}"),
        })),
        Value::Bool(b) => json!(b),
        Value::I32(i) => json!(i),
        Value::U32(i) => json!(i),
        other => json!(format!("{other:?}")),
    }
}

/// Class attributes as `{Name: [values]}` (repeated attributes such as `Static` append).
fn attrs<'a, T: HasAttributes<'a>>(row: &T) -> Map<String, Json> {
    let mut m = Map::new();
    for a in row.attributes() {
        let name = a.name().trim_end_matches("Attribute").to_string();
        let vals: Vec<Json> = a.value().iter().map(|(_, v)| attr_value(v)).collect();
        let e = m.entry(name).or_insert_with(|| json!([]));
        e.as_array_mut().unwrap().push(json!(vals));
    }
    m
}

fn find<'a>(idx: &'a Index, full: &str) -> Option<TypeDef<'a>> {
    let base = full.split('<').next().unwrap_or(full);
    let (ns, name) = base.rsplit_once('.')?;
    idx.get(ns, name).next().or_else(|| idx.iter().find(|(n, t, _)| *n == ns && trim_tick(t) == name).map(|(_, _, d)| d))
}

#[derive(Default)]
struct Members {
    props: BTreeMap<String, Json>,
    events: Vec<Json>,
    methods: Vec<Json>,
}

fn params_of(m: &MethodDef, generics: &[Type]) -> (Vec<Json>, Type) {
    let sig = m.signature(generics);
    let names: Vec<String> = m.params().filter(|p| p.sequence() > 0).map(|p| p.name().to_string()).collect();
    let ps = sig
        .types
        .iter()
        .enumerate()
        .map(|(i, t)| json!({"name": names.get(i).cloned().unwrap_or_default(), "type": tn(t)}))
        .collect();
    (ps, sig.return_type)
}

/// Members of one interface (`iface` may be a generic instantiation).
fn collect(idx: &Index, iface: &Type, via: &str, out: &mut Members) {
    let Type::ClassName(n) = iface else { return };
    let Some(def) = find(idx, &format!("{}.{}", n.namespace, trim_tick(&n.name))) else {
        out.methods.push(json!({"unresolved_interface": tn(iface)}));
        return;
    };
    for m in def.methods() {
        let name = m.name();
        let (ps, ret) = params_of(&m, &n.generics);
        let special = m.flags().contains(MethodAttributes::SpecialName);
        if special && let Some(p) = name.strip_prefix("get_") {
            let e = out.props.entry(p.into()).or_insert_with(|| json!({"name": p, "type": tn(&ret), "get": false, "set": false, "interface": via}));
            e["get"] = json!(true);
        } else if special && let Some(p) = name.strip_prefix("put_") {
            let ty = ps.first().map(|j| j["type"].clone()).unwrap_or(json!(""));
            let e = out.props.entry(p.into()).or_insert_with(|| json!({"name": p, "type": ty, "get": false, "set": false, "interface": via}));
            e["set"] = json!(true);
        } else if special && let Some(ev) = name.strip_prefix("add_") {
            let ty = ps.first().map(|j| j["type"].clone()).unwrap_or(json!(""));
            out.events.push(json!({"name": ev, "delegate": ty, "interface": via}));
        } else if special && name.starts_with("remove_") {
        } else {
            out.methods.push(json!({"name": name, "params": ps, "ret": tn(&ret), "interface": via}));
        }
    }
}

fn signature_refs(idx: &Index, iface: &Type, out: &mut Vec<String>) {
    let Type::ClassName(n) = iface else { return };
    let Some(def) = find(idx, &format!("{}.{}", n.namespace, trim_tick(&n.name))) else { return };
    for m in def.methods() {
        let sig = m.signature(&n.generics);
        for t in &sig.types {
            named(t, out);
        }
        named(&sig.return_type, out);
    }
}

fn class_json(idx: &Index, def: TypeDef, refs: &mut Vec<String>) -> Json {
    let full = format!("{}.{}", def.namespace(), trim_tick(def.name()));
    let mut own = Members::default();
    let mut statics = Members::default();
    let mut factories = Vec::new();
    let a = attrs(&def);
    for at in def.attributes() {
        let an = at.name();
        if an == "StaticAttribute" || an == "ComposableAttribute" || an == "ActivatableAttribute" {
            for (_, v) in at.value() {
                if let Value::TypeName(t) = v {
                    let ty = Type::ClassName(TypeName::named(&t.namespace, &t.name));
                    let via = format!("{}.{}", t.namespace, t.name);
                    if an == "StaticAttribute" {
                        collect(idx, &ty, &via, &mut statics);
                    } else {
                        let methods: Vec<String> =
                            find(idx, &via).map(|d| d.methods().map(|m| m.name().to_string()).collect()).unwrap_or_default();
                        factories.push(json!({"kind": an.trim_end_matches("Attribute"), "interface": via, "methods": methods}));
                    }
                }
            }
        }
    }
    let mut overridable = Vec::new();
    let mut interfaces = Vec::new();
    for ii in def.interface_impls() {
        let ty = ii.interface(&[]);
        let ia = attrs(&ii);
        let via = tn(&ty);
        let is_default = ia.contains_key("Default");
        if ia.contains_key("Overridable") {
            let mut ov = Members::default();
            collect(idx, &ty, &via, &mut ov);
            overridable.extend(ov.methods);
            continue;
        }
        if ia.contains_key("Protected") {
            continue;
        }
        interfaces.push(json!({"name": via, "default": is_default}));
        signature_refs(idx, &ty, refs);
        collect(idx, &ty, &via, &mut own);
    }
    if def.category() == TypeCategory::Interface {
        let ty = Type::ClassName(TypeName::named(def.namespace(), def.name()));
        collect(idx, &ty, &full, &mut own);
    }
    // Activatable with no interface value = default constructor.
    let default_ctor = def
        .attributes()
        .filter(|at| at.name() == "ActivatableAttribute")
        .any(|at| !at.value().iter().any(|(_, v)| matches!(v, Value::TypeName(_))));
    let dps: Vec<String> = statics
        .props
        .iter()
        .filter(|(_, p)| p["type"].as_str().is_some_and(|t| t.ends_with(".DependencyProperty")))
        .map(|(n, _)| n.clone())
        .collect();
    let static_props: Vec<Json> = statics
        .props
        .values()
        .filter(|p| !p["type"].as_str().is_some_and(|t| t.ends_with(".DependencyProperty")))
        .cloned()
        .collect();
    json!({
        "category": "class",
        "name": full,
        "extends": def.extends().map(|e| format!("{}.{}", e.namespace(), e.name())),
        "sealed": def.flags().contains(TypeAttributes::Sealed),
        "default_constructor": default_ctor,
        "factories": factories,
        "attributes": a,
        "interfaces": interfaces,
        "props": own.props.into_values().collect::<Vec<_>>(),
        "events": own.events,
        "methods": own.methods,
        "overridable": overridable,
        "dependency_properties": dps,
        "static_props": static_props,
        "static_events": statics.events,
        "static_methods": statics.methods,
    })
}

fn enum_json(def: TypeDef) -> Json {
    let mut members = Vec::new();
    for f in def.fields() {
        if let Some(c) = f.constant() {
            let v = match c.value() {
                Value::I32(i) => i as i64,
                Value::U32(u) => u as i64,
                _ => 0,
            };
            members.push(json!([f.name(), v]));
        }
    }
    json!({
        "category": "enum",
        "name": format!("{}.{}", def.namespace(), def.name()),
        "flags": def.attributes().any(|a| a.name() == "FlagsAttribute"),
        "members": members,
    })
}

fn struct_json(def: TypeDef, refs: &mut Vec<String>) -> Json {
    let fields: Vec<Json> = def
        .fields()
        .map(|f| {
            let t = f.ty();
            named(&t, refs);
            json!({"name": f.name(), "type": tn(&t)})
        })
        .collect();
    json!({"category": "struct", "name": format!("{}.{}", def.namespace(), def.name()), "fields": fields})
}

fn delegate_json(idx: &Index, def: TypeDef, refs: &mut Vec<String>) -> Json {
    let invoke = def.methods().find(|m| m.name() == "Invoke");
    let (params, generics): (Vec<Json>, Vec<String>) = match invoke {
        Some(m) => {
            // Generic delegates (TypedEventHandler<TSender, TResult>) keep their parameters symbolic.
            let gens: Vec<Type> = def.generic_params().enumerate().map(|(i, g)| Type::Generic(g.name().to_string(), i as u16)).collect();
            let sig = m.signature(&gens);
            for t in &sig.types {
                named(t, refs);
            }
            let names: Vec<String> = m.params().filter(|p| p.sequence() > 0).map(|p| p.name().to_string()).collect();
            let ps = sig.types.iter().enumerate().map(|(i, t)| json!({"name": names.get(i).cloned().unwrap_or_default(), "type": tn(t)})).collect();
            (ps, def.generic_params().map(|g| g.name().to_string()).collect())
        }
        None => (vec![], vec![]),
    };
    let _ = idx;
    json!({"category": "delegate", "name": format!("{}.{}", def.namespace(), trim_tick(def.name())), "generics": generics, "params": params})
}

fn dump(idx: &Index, roots: &[String]) -> Json {
    let mut out = BTreeMap::<String, Json>::new();
    let mut queue: VecDeque<(String, bool)> = roots.iter().map(|r| (r.clone(), true)).collect();
    let mut seen = BTreeSet::new();
    while let Some((full, deep)) = queue.pop_front() {
        let base = full.split('<').next().unwrap_or(&full).to_string();
        if !seen.insert(base.clone()) || base.starts_with("System.") {
            continue;
        }
        let Some(def) = find(idx, &base) else {
            out.insert(base.clone(), json!({"category": "unresolved", "name": base}));
            continue;
        };
        let mut refs = Vec::new();
        let j = match def.category() {
            TypeCategory::Class => {
                let j = class_json(idx, def, &mut refs);
                if let Some(ext) = j["extends"].as_str() {
                    queue.push_back((ext.to_string(), deep));
                }
                j
            }
            TypeCategory::Enum => enum_json(def),
            TypeCategory::Struct => struct_json(def, &mut refs),
            TypeCategory::Delegate => delegate_json(idx, def, &mut refs),
            TypeCategory::Interface => json!({"category": "interface", "name": base}),
            TypeCategory::Attribute => continue,
        };
        // Roots and their bases pull in everything their members mention, one level deep:
        // delegates, args classes (with their bases), enums and structs. Classes reached that way
        // are dumped shallowly (their own refs are not followed further) unless they are delegate
        // args, whose settable props matter for sync events.
        let is_delegate = def.category() == TypeCategory::Delegate;
        if deep || is_delegate || def.category() == TypeCategory::Struct || base.ends_with("EventArgs") {
            for r in refs {
                let Some(d) = find(idx, &r) else { continue };
                let follow = match d.category() {
                    TypeCategory::Enum | TypeCategory::Struct | TypeCategory::Delegate => true,
                    TypeCategory::Class => is_delegate || r.ends_with("EventArgs"),
                    _ => false,
                };
                if follow {
                    queue.push_back((r, false));
                }
            }
        }
        out.insert(base, j);
    }
    json!({"schema": "svelters-winmd-dump/v0", "types": out})
}

/// Input paths with `@list.txt` expanded (one path per line, as `fetch.py --winmds` prints) and
/// directories expanded to their `.winmd` files, sorted.
fn inputs_paths(paths: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for p in paths {
        let listed: Vec<String> = match p.strip_prefix('@') {
            Some(list) => std::fs::read_to_string(list)
                .unwrap_or_else(|e| panic!("read {list}: {e}"))
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(String::from)
                .collect(),
            None => vec![p.clone()],
        };
        for p in listed {
            let path = std::path::Path::new(&p);
            if path.is_dir() {
                let mut entries: Vec<_> = std::fs::read_dir(path).expect("read dir").filter_map(|e| e.ok()).map(|e| e.path()).collect();
                entries.sort();
                out.extend(entries.into_iter().filter(|e| e.extension().is_some_and(|x| x == "winmd")).map(|e| e.display().to_string()));
            } else {
                out.push(p);
            }
        }
    }
    out
}

/// The given winmds plus the SDK's Windows.winmd (types such as `Windows.System.VirtualKey`), as
/// windows-bindgen sees them with `--in default`.
fn inputs(paths: &[String]) -> Vec<File> {
    let mut files: Vec<File> = inputs_paths(paths).iter().map(|p| File::read(p).unwrap_or_else(|| panic!("read {p}"))).collect();
    files.push(File::new(windows_default::WINRT.to_vec()).expect("bundled Windows.winmd"));
    files
}

/// windows-bindgen 0.100 emits a factory-cache helper for every `[Static]`/`[Composable]`/
/// `[Activatable]` interface of a selected class but does not select the interface itself when the
/// class is filtered by member. Add a name-only shell (`IFooStatics::{}`) for each one the filter
/// does not mention; writes `<filter>.full` and returns its path.
fn with_factory_shells(idx: &Index, filter: &str) -> String {
    let text = std::fs::read_to_string(filter).unwrap_or_else(|e| panic!("read {filter}: {e}"));
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).collect();
    let mentioned: BTreeSet<String> = lines.iter().map(|l| l.split("::").next().unwrap().to_string()).collect();
    let mut extra = BTreeSet::new();
    // The classes and their base chains: a base class is emitted (as a required type) with its own
    // factory helpers too.
    let mut classes = BTreeSet::new();
    for class in &mentioned {
        let mut cur = find(idx, class);
        while let Some(def) = cur {
            if def.category() != TypeCategory::Class || def.namespace() == "System" {
                break;
            }
            classes.insert(format!("{}.{}", def.namespace(), def.name()));
            cur = def.extends().and_then(|e| find(idx, &format!("{}.{}", e.namespace(), e.name())));
        }
    }
    for class in &classes {
        let Some(def) = find(idx, class) else { continue };
        for at in def.attributes() {
            if matches!(at.name(), "StaticAttribute" | "ComposableAttribute" | "ActivatableAttribute") {
                for (_, v) in at.value() {
                    if let Value::TypeName(t) = v {
                        let iface = format!("{}.{}", t.namespace, t.name);
                        if !mentioned.contains(&iface) {
                            extra.insert(format!("{iface}::{{}}"));
                        }
                    }
                }
            }
        }
    }
    let out = format!("{filter}.full");
    let mut all: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    all.extend(extra);
    std::fs::write(&out, all.join("\n") + "\n").unwrap_or_else(|e| panic!("write {out}: {e}"));
    out
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let usage = "usage: winmd-tool dump <classes>... -- <winmds>... | winmd-tool bindgen --out <rs> --filter-file <txt> -- <winmds>...";
    let Some(cmd) = args.first() else {
        eprintln!("{usage}");
        std::process::exit(2);
    };
    let sep = args.iter().position(|a| a == "--").unwrap_or_else(|| {
        eprintln!("{usage}");
        std::process::exit(2)
    });
    let (head, winmds) = (&args[1..sep], &args[sep + 1..]);
    match cmd.as_str() {
        "dump" => {
            let idx = Index::new(inputs(winmds));
            let j = dump(&idx, head);
            println!("{}", serde_json::to_string_pretty(&j).unwrap());
        }
        "bindgen" => {
            let mut a: Vec<String> = vec!["--in".into(), "default".into()];
            for f in inputs_paths(winmds) {
                a.push(f);
            }
            let mut rest = head.to_vec();
            if let Some(i) = rest.iter().position(|x| x == "--filter-file") {
                let path = rest[i + 1].clone();
                rest[i + 1] = with_factory_shells(&Index::new(inputs(winmds)), &path);
            }
            a.extend(rest);
            windows_bindgen::bindgen(a.iter().map(|s| s.as_str()));
        }
        _ => {
            eprintln!("{usage}");
            std::process::exit(2);
        }
    }
}
