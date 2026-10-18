//! For every runtime class in the given winmds: its ThreadingModel and MarshalingType attributes,
//! whether it derives from DependencyObject / UIElement, and whether it is activatable.
//!
//!   winrt-threading-probe <namespace prefix>... -- <winmd>...
//! Prints one TSV row per class, then a summary per (group, threading, marshaling).
use std::collections::BTreeMap;
use windows_metadata::reader::*;
use windows_metadata::Type;

/// ThreadingModel: 1 STA, 2 MTA, 3 Both; MarshalingType: 1 None, 2 Agile, 3 Standard.
fn enum_val(v: &windows_metadata::Value) -> String {
    let s = format!("{v:?}");
    let n = s.rsplit("I32(").next().and_then(|t| t.trim_end_matches(')').trim_end_matches(')').parse::<i32>().ok());
    match (s.contains("ThreadingModel"), n) {
        (true, Some(1)) => "STA".into(),
        (true, Some(2)) => "MTA".into(),
        (true, Some(3)) => "Both".into(),
        (false, Some(1)) => "None".into(),
        (false, Some(2)) => "Agile".into(),
        (false, Some(3)) => "Standard".into(),
        _ => s,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let sep = args.iter().position(|a| a == "--").expect("usage: prefixes -- winmds");
    let (prefixes, files) = (&args[..sep], &args[sep + 1..]);
    let idx = Index::new(files.iter().map(|f| File::read(f).unwrap_or_else(|| panic!("cannot read {f}"))).collect());
    let mut summary: BTreeMap<(String, String, String), Vec<String>> = BTreeMap::new();
    for def in idx.types() {
        let ns = def.namespace();
        if !prefixes.iter().any(|p| ns.starts_with(p.as_str())) {
            continue;
        }
        if def.category() != TypeCategory::Class {
            continue;
        }
        let full = format!("{ns}.{}", def.name());
        let (mut threading, mut marshal, mut activatable) = ("-".to_string(), "-".to_string(), false);
        for a in def.attributes() {
            match a.name() {
                "ThreadingAttribute" => threading = a.value().iter().map(|(_, v)| enum_val(v)).collect::<Vec<_>>().join(","),
                "MarshalingBehaviorAttribute" => marshal = a.value().iter().map(|(_, v)| enum_val(v)).collect::<Vec<_>>().join(","),
                "ActivatableAttribute" => activatable = true,
                _ => {}
            }
        }
        // Structural affinity: the class or a base exposes a `DispatcherQueue` property (XAML
        // DependencyObject, Composition CompositionObject, AppWindow, ...), i.e. it is bound to a
        // dispatcher thread whatever its Threading/MarshalingBehavior attributes say.
        let has_dq = |d: &TypeDef| {
            d.interface_impls().any(|ii| {
                let ty = ii.interface(&[]);
                let Type::ClassName(n) = ty else { return false };
                idx.get(&n.namespace, n.name.split('`').next().unwrap_or(&n.name))
                    .next()
                    .map(|i| i.methods().any(|m| m.name() == "get_DispatcherQueue"))
                    .unwrap_or(false)
            })
        };
        let mut affine = has_dq(&def);
        let mut c2 = def.extends();
        while let Some(e) = c2 {
            let d = idx.get(e.namespace(), e.name()).next();
            if let Some(d) = d {
                affine |= has_dq(&d);
                c2 = d.extends();
            } else {
                c2 = None;
            }
        }
        // base chain
        let mut chain = vec![];
        let mut cur = def.extends();
        while let Some(e) = cur {
            let n = format!("{}.{}", e.namespace(), e.name());
            chain.push(n.clone());
            cur = idx.get(e.namespace(), e.name()).next().and_then(|d| d.extends());
        }
        let group = if chain.iter().any(|c| c.ends_with(".UIElement")) || full.ends_with(".UIElement") {
            "UIElement"
        } else if chain.iter().any(|c| c.ends_with("Xaml.DependencyObject")) || full.ends_with("Xaml.DependencyObject") {
            "DependencyObject (non-visual)"
        } else if ns.starts_with("Microsoft.UI.Xaml") {
            "Xaml, not a DependencyObject"
        } else {
            "other"
        };
        let aff = if affine { "dispatcher-affine" } else { "-" };
        println!("{full}\t{group}\t{threading}\t{marshal}\t{aff}\t{activatable}\t{}", chain.first().cloned().unwrap_or_default());
        summary.entry((group.to_string(), format!("{threading}/{marshal}"), aff.to_string())).or_default().push(full);
    }
    eprintln!("== summary: group / threading/marshaling / DispatcherQueue-affine -> count (examples)");
    for ((g, t, m), v) in &summary {
        let ex: Vec<&str> = v.iter().take(6).map(|s| s.rsplit('.').next().unwrap()).collect();
        eprintln!("{g:32} {t:12} {m:12} {:5}  {}", v.len(), ex.join(" "));
    }
}
