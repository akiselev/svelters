// winmd_dump: dump a WinRT runtime class (and its base chain) from .winmd files as
// svelters-flavoured facts: content property, dependency properties, attached properties,
// properties (get_/put_), events (add_/remove_ with delegate + args), methods, factories.
//
// usage: winmd_dump <class-full-name>... -- <winmd files...>
use std::collections::BTreeMap;
use windows_metadata::reader::*;
use windows_metadata::*;

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
        Type::String => "HSTRING".into(),
        Type::Object => "IInspectable".into(),
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
        Type::RefConst(t) => format!("ref {}", tn(t)),
        other => format!("{other:?}"),
    }
}

fn attrs<'a, T: HasAttributes<'a>>(row: &T) -> Vec<String> {
    row.attributes()
        .map(|a| {
            let vals: Vec<String> = a
                .value()
                .into_iter()
                .map(|(k, v)| {
                    let v = match v {
                        Value::Utf8(s) | Value::Utf16(s) => format!("{s:?}"),
                        Value::TypeName(t) => format!("{}.{}", t.namespace, t.name),
                        Value::EnumValue(t, inner) => format!("{}({:?})", t.name, inner),
                        other => format!("{other:?}"),
                    };
                    if k.is_empty() { v } else { format!("{k}={v}") }
                })
                .collect();
            format!("{}({})", a.name().trim_end_matches("Attribute"), vals.join(", "))
        })
        .collect()
}

fn find<'a>(idx: &'a Index, full: &str) -> Option<TypeDef<'a>> {
    let (ns, name) = full.rsplit_once('.')?;
    idx.get(ns, name).next()
}

struct Members {
    props: BTreeMap<String, (String, bool, bool)>, // name -> (type, get, put)
    events: Vec<(String, String)>,
    methods: Vec<String>,
}

fn sig_str(m: &MethodDef, generics: &[Type]) -> (Vec<String>, String) {
    let sig = m.signature(generics);
    let names: Vec<String> = m.params().filter(|p| p.sequence() > 0).map(|p| p.name().to_string()).collect();
    let ps = sig
        .types
        .iter()
        .enumerate()
        .map(|(i, t)| format!("{}: {}", names.get(i).cloned().unwrap_or_default(), tn(t)))
        .collect();
    (ps, tn(&sig.return_type))
}

fn collect(idx: &Index, iface: &Type, out: &mut Members) {
    let Type::ClassName(n) = iface else { return };
    let Some(def) = idx.get(&n.namespace, trim_tick(&n.name)).next() else {
        out.methods.push(format!("<unresolved interface {}>", tn(iface)));
        return;
    };
    for m in def.methods() {
        let name = m.name();
        let (ps, ret) = sig_str(&m, &n.generics);
        let special = m.flags().contains(MethodAttributes::SpecialName);
        if special && name.starts_with("get_") {
            let e = out.props.entry(name[4..].into()).or_insert((ret.clone(), false, false));
            e.1 = true;
        } else if special && name.starts_with("put_") {
            let ty = ps.first().map(|s| s.split(": ").nth(1).unwrap_or("").to_string()).unwrap_or_default();
            let e = out.props.entry(name[4..].into()).or_insert((ty, false, false));
            e.2 = true;
        } else if special && name.starts_with("add_") {
            let ty = ps.first().map(|s| s.split(": ").nth(1).unwrap_or("").to_string()).unwrap_or_default();
            out.events.push((name[4..].into(), ty));
        } else if special && name.starts_with("remove_") {
        } else {
            out.methods.push(format!("{name}({}) -> {ret}", ps.join(", ")));
        }
    }
}

fn dump_class(idx: &Index, def: TypeDef) {
    println!("\n=== class {}.{}", def.namespace(), def.name());
    if let Some(ext) = def.extends() {
        println!("  extends: {}.{}", ext.namespace(), ext.name());
    }
    for a in attrs(&def) {
        if !a.starts_with("ContractVersion") && !a.starts_with("WebHostHidden") {
            println!("  @{a}");
        }
    }
    // class-level attributes naming statics/factory interfaces
    let mut own = Members { props: BTreeMap::new(), events: vec![], methods: vec![] };
    let mut statics = Members { props: BTreeMap::new(), events: vec![], methods: vec![] };
    for a in def.attributes() {
        let an = a.name();
        if an == "StaticAttribute" || an == "ComposableAttribute" || an == "ActivatableAttribute" {
            for (_, v) in a.value() {
                if let Value::TypeName(t) = v {
                    let ty = Type::ClassName(TypeName::named(&t.namespace, &t.name));
                    let tgt = if an == "StaticAttribute" { &mut statics } else { &mut statics };
                    collect(idx, &ty, tgt);
                }
            }
        }
    }
    if def.category() == TypeCategory::Interface {
        collect(idx, &Type::ClassName(TypeName::named(def.namespace(), def.name())), &mut own);
    }
    for ii in def.interface_impls() {
        let ty = ii.interface(&[]);
        let a = attrs(&ii);
        let tags: Vec<_> = a.iter().filter(|s| !s.starts_with("ContractVersion")).cloned().collect();
        println!("  implements {} {}", tn(&ty), tags.join(" "));
        // Overridable (protected virtual) interfaces: list separately
        if a.iter().any(|s| s.starts_with("Overridable")) {
            let mut ov = Members { props: BTreeMap::new(), events: vec![], methods: vec![] };
            collect(idx, &ty, &mut ov);
            for m in ov.methods {
                println!("    virtual {m}");
            }
            continue;
        }
        if a.iter().any(|s| s.starts_with("Protected")) {
            continue;
        }
        collect(idx, &ty, &mut own);
    }
    for (name, (ty, g, p)) in &own.props {
        println!("  prop {name}: {ty} [{}{}]", if *g { "r" } else { "" }, if *p { "w" } else { "" });
    }
    for (name, ty) in &own.events {
        println!("  event {name}: {ty}");
    }
    for m in &own.methods {
        println!("  method {m}");
    }
    let dps: Vec<_> = statics.props.iter().filter(|(_, (t, _, _))| t.ends_with("DependencyProperty")).map(|(n, _)| n.clone()).collect();
    if !dps.is_empty() {
        println!("  dependency-properties ({}): {}", dps.len(), dps.join(", "));
    }
    for (name, (ty, _, _)) in &statics.props {
        if !ty.ends_with("DependencyProperty") {
            println!("  static prop {name}: {ty}");
        }
    }
    for m in &statics.methods {
        let tag = if m.starts_with("Get") || m.starts_with("Set") { "attached-accessor" } else if m.starts_with("CreateInstance") { "factory" } else { "static" };
        println!("  {tag} {m}");
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let sep = args.iter().position(|a| a == "--").expect("usage: classes -- winmds");
    let files: Vec<File> = args[sep + 1..].iter().map(|p| File::read(p).unwrap_or_else(|| panic!("read {p}"))).collect();
    let idx = Index::new(files);
    for full in &args[..sep] {
        if let Some(rest) = full.strip_prefix("--items=") {
            // --items=<namespace>:<prefix>  list Win32 items (fn/const/type) by name prefix
            let (ns, pre) = rest.split_once(':').unwrap_or((rest, ""));
            let mut n = 0;
            for (name, item) in idx.namespace_items(ns) {
                if !name.starts_with(pre) { continue; }
                n += 1;
                let k = match item { Item::Type(_) => "type", Item::Fn(_) => "fn", Item::Const(_) => "const" };
                if n <= 40 { println!("  {k} {name}"); }
            }
            println!("{ns}:{pre}* -> {n} items");
            continue;
        }
        if full == "--stats" {
            stats(&idx);
            continue;
        }
        let chain = full.ends_with('+');
        let full = full.trim_end_matches('+');
        let Some(mut def) = find(&idx, full) else {
            println!("not found: {full}");
            continue;
        };
        loop {
            dump_class(&idx, def);
            if !chain {
                break;
            }
            let Some(ext) = def.extends() else { break };
            match find(&idx, &format!("{}.{}", ext.namespace(), ext.name())) {
                Some(d) if d.namespace() != "System" => def = d,
                _ => break,
            }
        }
    }
}

// whole-namespace statistics for Microsoft.UI.Xaml.Controls
fn stats(idx: &Index) {
    let mut classes = 0;
    let mut content = BTreeMap::<String, String>::new();
    let mut dp = 0usize;
    let mut events = 0usize;
    let mut handler_types = BTreeMap::<String, usize>::new();
    for def in idx.types() {
        if !def.namespace().starts_with("Microsoft.UI.Xaml") || def.category() != TypeCategory::Class {
            continue;
        }
        classes += 1;
        if let Some(a) = def.find_attribute("ContentPropertyAttribute") {
            if let Some((_, Value::Utf16(s) | Value::Utf8(s))) = a.value().into_iter().next() {
                content.insert(format!("{}.{}", def.namespace(), def.name()), s);
            }
        }
        let mut m = Members { props: BTreeMap::new(), events: vec![], methods: vec![] };
        for ii in def.interface_impls() {
            collect(idx, &ii.interface(&[]), &mut m);
        }
        events += m.events.len();
        for (_, t) in &m.events {
            let k = t.split('<').next().unwrap_or("").to_string();
            *handler_types.entry(k).or_default() += 1;
        }
        let mut s = Members { props: BTreeMap::new(), events: vec![], methods: vec![] };
        for a in def.attributes() {
            if a.name() == "StaticAttribute" {
                for (_, v) in a.value() {
                    if let Value::TypeName(t) = v {
                        collect(idx, &Type::ClassName(TypeName::named(&t.namespace, &t.name)), &mut s);
                    }
                }
            }
        }
        dp += s.props.values().filter(|(t, _, _)| t.ends_with("DependencyProperty")).count();
    }
    println!("classes in Microsoft.UI.Xaml.*: {classes}");
    println!("DependencyProperty statics: {dp}");
    println!("events (own interfaces): {events}");
    let mut ht: Vec<_> = handler_types.into_iter().collect();
    ht.sort_by(|a, b| b.1.cmp(&a.1));
    println!("event handler delegate types (top 12): {:?}", &ht[..ht.len().min(12)]);
    println!("ContentPropertyAttribute on {} classes:", content.len());
    for (k, v) in &content {
        println!("  {k} -> {v}");
    }
}
