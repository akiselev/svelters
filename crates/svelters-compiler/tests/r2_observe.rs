//! R2 (plans/milestones/R2-compiler-observe.md): `bind:` lowered from the observe facts, `|nonreactive`,
//! the section 8.4 diagnostics, `--strict-native` and the portability mode.
//!
//! Golden cases: `tests/r2/<case>.svelte`, whose first line names the runs
//! (`<!-- backends: gtk,qt; strict: gtk; portability: gtk,qt,appkit,winui -->`), against
//! `tests/r2/<case>.expected`: per run the rendered diagnostics and the emitted `$n.bind*` calls.
//! `UPDATE=1 cargo test -p svelters-compiler --test r2_observe` rewrites the expectations.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use svelters_compiler::compile::{CompileOptions, compile};
use svelters_compiler::semantics::{BindLowering, ElementRef, Observability, Overlay, PlatformSemantics, SpecBackend, plan_bind, spec::ObsClass, spec::ObsVerified};

fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/r2")
}

fn run(src: &str, name: &str, backend: &str, strict: bool, portability: &[&str]) -> String {
    let sem = SpecBackend::builtin(backend).unwrap();
    let others: Vec<SpecBackend> = portability.iter().filter(|b| **b != backend).map(|b| SpecBackend::builtin(b).unwrap()).collect();
    let mut opts = CompileOptions::new(name, &sem);
    opts.strict_native = strict;
    opts.portability = others.iter().map(|o| o as &dyn PlatformSemantics).collect();
    let r = compile(src, &opts);
    let mut out = String::new();
    for d in &r.diagnostics {
        out.push_str(&d.render(name));
        out.push('\n');
    }
    match &r.js {
        Some(js) => {
            for l in js.lines().map(str::trim).filter(|l| l.starts_with("$n.bind") || l.contains("$n.bind")) {
                out.push_str(l);
                out.push('\n');
            }
        }
        None => out.push_str("(no output: errors)\n"),
    }
    out
}

fn case(name: &str) -> String {
    let src = std::fs::read_to_string(dir().join(format!("{name}.svelte"))).unwrap();
    let header = src.lines().next().unwrap().trim().trim_start_matches("<!--").trim_end_matches("-->").trim().to_string();
    let mut out = String::new();
    let file = format!("{name}.svelte");
    for part in header.split(';').map(str::trim) {
        let (mode, list) = part.split_once(':').expect("mode: backends");
        let list: Vec<&str> = list.split(',').map(str::trim).collect();
        match mode {
            "backends" => {
                for b in &list {
                    out.push_str(&format!("== {b}\n{}", run(&src, &file, b, false, &[])));
                }
            }
            "strict" => {
                for b in &list {
                    out.push_str(&format!("== {b} --strict-native\n{}", run(&src, &file, b, true, &[])));
                }
            }
            "portability" => out.push_str(&format!("== --backend {}\n{}", list.join(","), run(&src, &file, list[0], false, &list))),
            other => panic!("unknown mode {other}"),
        }
    }
    out
}

fn golden(name: &str) {
    let got = case(name);
    let path = dir().join(format!("{name}.expected"));
    if std::env::var_os("UPDATE").is_some() {
        std::fs::write(&path, &got).unwrap();
        return;
    }
    let want = std::fs::read_to_string(&path).unwrap_or_default();
    assert_eq!(got, want, "{name}: output differs from {} (UPDATE=1 rewrites it)", path.display());
}

#[test]
fn lowering_gtk() {
    golden("lowering-gtk");
}

#[test]
fn lowering_qt() {
    golden("lowering-qt");
}

#[test]
fn lowering_appkit() {
    golden("lowering-appkit");
}

#[test]
fn lowering_winui() {
    golden("lowering-winui");
}

#[test]
fn diagnostics_gtk() {
    golden("diagnostics-gtk");
}

#[test]
fn unverified_gtk() {
    golden("unverified-gtk");
}

#[test]
fn portability() {
    golden("portability");
}

/// E3: `bind:` with a Rust store field as the bound value lowers like any other `bind:`: onto observation
/// (`bind_prop`) where the element prop is exact, with no diagnostic (strict included).
#[test]
fn store_field_gtk() {
    golden("store-field-gtk");
}

/// The lowering reads the facts: an [`Overlay`] that changes one fact changes the output.
#[test]
fn a_changed_fact_changes_the_lowering() {
    let gtk = SpecBackend::builtin("gtk").unwrap();
    let el = gtk.resolve_element("entry").unwrap();
    let plan = plan_bind(&gtk, &el, "text", false).unwrap();
    assert_eq!(plan.lowering, BindLowering::Prop { fallback: Some(("changed".into(), 0)) });
    let partial = Observability {
        class: ObsClass::Partial,
        missed: vec!["user_edit".into()],
        coarse: false,
        value_cost: Default::default(),
        timing: Default::default(),
        mechanism: "notify".into(),
        verified: ObsVerified::Delivery,
        fact: "Gtk:Entry:text".into(),
    };
    let o = Overlay::new(SpecBackend::builtin("gtk").unwrap()).with_observability("entry", "text", Some(partial));
    let plan = plan_bind(&o, &el, "text", false).unwrap();
    assert_eq!(plan.lowering, BindLowering::Signal { signal: "changed".into(), arg: 0 }, "partial keeps the signal plan");
    assert_eq!(plan.warnings[0].code, "bind_partially_observed");
    let o = Overlay::new(SpecBackend::builtin("gtk").unwrap()).with_observability("entry", "text", None);
    assert_eq!(plan_bind(&o, &el, "text", false).unwrap().lowering, BindLowering::Signal { signal: "changed".into(), arg: 0 }, "unknown keeps the signal plan");
    let o = Overlay::new(SpecBackend::builtin("gtk").unwrap()).with_observability("entry", "cursor-position", None);
    assert_eq!(plan_bind(&o, &el, "cursor-position", false).unwrap_err().code, "bind_no_plan", "unknown without a plan is an error");
}

/// OV1: an `exact` fact lowers onto observation only when a generated row saw a native change
/// delivered. A weakly verified one keeps the declared signal plan, or observes with a warning.
#[test]
fn an_unverified_exact_fact_does_not_replace_the_signal_plan() {
    let gtk = SpecBackend::builtin("gtk").unwrap();
    let el = gtk.resolve_element("entry").unwrap();
    let weak = |fact: &str| Observability {
        class: ObsClass::Exact,
        missed: vec![],
        coarse: false,
        value_cost: Default::default(),
        timing: Default::default(),
        mechanism: "notify".into(),
        verified: ObsVerified::Weak,
        fact: fact.into(),
    };
    let o = Overlay::new(SpecBackend::builtin("gtk").unwrap()).with_observability("entry", "text", Some(weak("Gtk:Editable:text")));
    let plan = plan_bind(&o, &el, "text", false).unwrap();
    assert_eq!(plan.lowering, BindLowering::Signal { signal: "changed".into(), arg: 0 }, "a weak exact fact keeps the signal plan");
    assert!(plan.warnings.is_empty(), "the signal plan needs no warning: {:?}", plan.warnings);
    let o = Overlay::new(SpecBackend::builtin("gtk").unwrap()).with_observability("entry", "cursor-position", Some(weak("Gtk:Editable:cursor-position")));
    let plan = plan_bind(&o, &el, "cursor-position", false).unwrap();
    assert_eq!(plan.lowering, BindLowering::Readonly, "without a signal plan the binding still observes");
    assert_eq!(plan.warnings.len(), 1);
    assert_eq!(plan.warnings[0].code, "bind_observation_unverified");
    assert!(plan.warnings[0].message.contains("Gtk:Editable:cursor-position"), "{}", plan.warnings[0].message);
    let mut none = weak("Gtk:Editable:cursor-position");
    none.verified = ObsVerified::None;
    let o = Overlay::new(SpecBackend::builtin("gtk").unwrap()).with_observability("entry", "cursor-position", Some(none));
    let plan = plan_bind(&o, &el, "cursor-position", false).unwrap();
    assert_eq!(plan.warnings[0].code, "bind_observation_unverified", "no passing row: the same warning");
    // `|nonreactive` reads once: the strength does not matter
    let plan = plan_bind(&o, &el, "cursor-position", true).unwrap();
    assert!(plan.warnings.is_empty());
}

/// observe.d.ts (tools/metadata/semantics/gen_observe_outputs.py) offers exactly the `bind:` keys the
/// compiler accepts, per platform and tag.
#[test]
fn typings_agree_with_the_compiler() {
    let dts = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/native-runtime/types/observe.d.ts")).unwrap();
    // interface name -> keys
    let mut ifaces: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut tags: BTreeMap<String, BTreeMap<String, BTreeSet<String>>> = BTreeMap::new();
    let key = |l: &str| -> Vec<String> {
        let mut out = Vec::new();
        let mut rest = l;
        while let Some(i) = rest.find("*/ \"") {
            let s = &rest[i + 4..];
            let e = s.find('"').unwrap();
            out.push(s[..e].to_string());
            rest = &s[e..];
        }
        out
    };
    let mut cur: Option<String> = None;
    let mut tk: Option<String> = None;
    for l in dts.lines() {
        if let Some(n) = l.strip_prefix("interface ").and_then(|r| r.strip_suffix(" {")) {
            cur = Some(n.to_string());
            ifaces.insert(n.to_string(), BTreeSet::new());
        } else if let Some(n) = l.strip_prefix("export interface ").and_then(|r| r.strip_suffix("ObservedBindings {")) {
            tk = Some(n.to_lowercase());
            cur = None;
        } else if l == "}" {
            cur = None;
        } else if let Some(c) = &cur {
            ifaces.get_mut(c).unwrap().extend(key(l));
        } else if let (Some(t), Some(rest)) = (&tk, l.strip_prefix("  \"")) {
            let (tag, ty) = rest.split_once("\": ").unwrap();
            let mut keys = BTreeSet::new();
            let ty = ty.trim_end_matches(';');
            let (named, extra) = match ty.find("{ ") {
                Some(i) => (&ty[..i], &ty[i..]),
                None => (ty, ""),
            };
            keys.extend(key(extra));
            for part in named.split(" & ").map(str::trim).filter(|p| !p.is_empty() && *p != "{}") {
                if let Some(inner) = part.strip_prefix("Omit<") {
                    let (name, omit) = inner.split_once(", ").unwrap();
                    let omitted: BTreeSet<String> = omit.trim_end_matches('>').split(" | ").map(|s| s.trim_matches('"').to_string()).collect();
                    keys.extend(ifaces[name].iter().filter(|k| !omitted.contains(*k)).cloned());
                } else {
                    keys.extend(ifaces[part].iter().cloned());
                }
            }
            tags.entry(t.clone()).or_default().insert(tag.to_string(), keys);
        }
    }
    for tk in ["gtk", "qt", "appkit", "winui"] {
        let sem = SpecBackend::builtin(tk).unwrap();
        let spec = sem.spec();
        let mut checked = 0;
        for (tag, kind) in &spec.tags {
            let el = ElementRef { tag: tag.clone(), kind: kind.clone() };
            let mut want = BTreeSet::new();
            let mut seen = BTreeSet::new();
            for k in &spec.kinds[kind].chain {
                let Some(ks) = spec.kinds.get(k) else { continue };
                for (js, p) in &ks.props {
                    if !seen.insert(js.clone()) || !p.readable {
                        continue;
                    }
                    if plan_bind(&sem, &el, js, false).is_ok() {
                        want.insert(format!("bind:{js}"));
                    }
                    if plan_bind(&sem, &el, js, true).is_ok() {
                        want.insert(format!("bind:{js}|nonreactive"));
                    }
                }
            }
            let got = tags.get(tk).and_then(|t| t.get(tag)).cloned().unwrap_or_default();
            assert_eq!(got, want, "{tk} <{tag}>: observe.d.ts and plan_bind disagree (rerun tools/metadata/semantics/gen_observe_outputs.py)");
            checked += 1;
        }
        assert!(checked > 0, "{tk}: no tags");
    }
}
