//! The egui fixtures' op logs on presto equal egui's: `fixtures/*-egui/expected.oplog` (counter,
//! if-toggle, keyed-list, motion, timers: Tier 0 kinds only) replayed against the presto backend
//! through the real vtable, as svelters-egui's `oplog_replay.rs` replays it
//! on egui. Every op must succeed and read back; `event` and `changed` lines are produced natively and
//! must reach the recording host exactly once (`changed` exactly when the log observes it).
//!
//! Native output comes from the tree's `emit`/`native_set` (what presto-core's `EventCx` calls when a
//! kind handles input); once presto-test's `Harness` can click and type, these lines are produced by
//! injected input instead.

mod common;

use common::Rig;
use std::collections::{HashMap, HashSet};
use svelters_abi::*;
use svelters_presto::surface::Value as PValue;

fn value_for(rig: &Rig, h: svl_handle, prop: &str, j: &serde_json::Value) -> Value {
    let id = rig.prop_id(h, prop);
    let tag = rig.br().describe(NameKind::Prop, rig.kind(h), id).map(|d| d.params[0].tag).unwrap_or(SVL_NULL);
    match j {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Bool(*b),
        serde_json::Value::Number(n) if tag == SVL_I64 => Value::I64(n.as_i64().unwrap()),
        serde_json::Value::Number(n) => Value::F64(n.as_f64().unwrap()),
        serde_json::Value::String(s) => Value::Str(s.clone()),
        other => panic!("unsupported value {other}"),
    }
}

fn pvalue(v: &Value) -> PValue {
    match v {
        Value::Bool(b) => PValue::Bool(*b),
        Value::F64(f) => PValue::F64(*f),
        Value::I64(i) => PValue::I64(*i),
        Value::Str(s) => PValue::Str(s.clone()),
        other => panic!("no native value for {other:?}"),
    }
}

fn replay(name: &str, log: &str) -> (usize, usize) {
    let rig = Rig::new();
    let mut map: HashMap<String, svl_handle> = HashMap::new();
    let mut observing: HashSet<(svl_handle, String)> = HashSet::new();
    let (mut events, mut changes) = (0, 0);
    for (n, line) in log.lines().enumerate().filter(|(_, l)| !l.trim().is_empty()) {
        let ctx = format!("{name}:{}: {line}", n + 1);
        let parts: Vec<&str> = line.splitn(4, ' ').collect();
        let hd = |t: &str| *map.get(t).unwrap_or_else(|| panic!("{ctx}: unknown handle {t}"));
        match parts[0] {
            "create" => {
                let h = rig.create(parts[1]);
                map.insert(parts[2].to_string(), h);
            }
            "insert" => {
                let w: Vec<&str> = line.split(' ').collect();
                let before = match w[3].strip_prefix("before=").unwrap() {
                    "end" => None,
                    t => Some(hd(t)),
                };
                assert_eq!(w[4], "slot=default", "{ctx}");
                rig.br().insert(hd(w[1]), hd(w[2]), before, 0).unwrap_or_else(|e| panic!("{ctx}: {e}"));
            }
            "remove" => rig.br().remove(hd(parts[1]), hd(parts[2])).unwrap_or_else(|e| panic!("{ctx}: {e}")),
            "destroy" => {
                let h = hd(parts[1]);
                rig.br().destroy(h);
                assert!(rig.br().handle_info(h).is_err(), "{ctx}: still alive");
            }
            "set" => {
                let h = hd(parts[1]);
                let j: serde_json::Value = serde_json::from_str(parts[3]).unwrap();
                let v = value_for(&rig, h, parts[2], &j);
                rig.set(h, parts[2], v.clone());
                let back = rig.get(h, parts[2]);
                assert!(back == v || matches!((&back, &v), (Value::F64(a), Value::I64(b)) if *a == *b as f64), "{ctx}: read back {back:?}");
            }
            "listen" => {
                let on = parts[3].starts_with("on");
                rig.br().listen(hd(parts[1]), rig.signal_id(hd(parts[1]), parts[2]), on).unwrap_or_else(|e| panic!("{ctx}: {e}"));
            }
            "observe" => {
                let (h, on) = (hd(parts[1]), parts[3] == "on");
                rig.observe(h, parts[2], on);
                if on {
                    observing.insert((h, parts[2].to_string()));
                } else {
                    observing.remove(&(h, parts[2].to_string()));
                }
            }
            "call" => {
                assert_eq!(parts[3], "[]", "{ctx}");
                rig.call(hd(parts[1]), parts[2]);
            }
            "style" => {
                let br = rig.br();
                let text = |s: &str| -> Option<String> { serde_json::from_str::<Option<String>>(s).unwrap() };
                let r = match parts[1] {
                    // `style sheet <id> <css>`: the op's text is `id SEP css`
                    "sheet" => {
                        let id: String = serde_json::from_str(parts[2]).unwrap();
                        let payload = text(parts[3]).map(|css| format!("{id}{SVL_STYLE_SEP}{css}"));
                        br.style(SVL_STYLE_SHEET, 0, payload.as_deref())
                    }
                    "scheme" => br.style(SVL_STYLE_SCHEME, 0, text(parts[2]).as_deref()),
                    _ => {
                        let op = match parts[2] {
                            "classes" => SVL_STYLE_CLASSES,
                            "name" => SVL_STYLE_NAME,
                            "inline" => SVL_STYLE_INLINE,
                            "opacity" => SVL_STYLE_OPACITY,
                            "transform" => SVL_STYLE_TRANSFORM,
                            "clip" => SVL_STYLE_CLIP,
                            "inert" => SVL_STYLE_INERT,
                            o => panic!("{ctx}: style op {o}"),
                        };
                        br.style(op, hd(parts[1]), text(parts[3]).as_deref())
                    }
                };
                r.unwrap_or_else(|e| panic!("{ctx}: {e}"));
            }
            // a widget signal, produced natively and delivered at event time
            "event" => {
                let (h, sig) = (hd(parts[1]), parts[2]);
                rig.take();
                let args: Vec<serde_json::Value> = serde_json::from_str(parts[3]).unwrap();
                let want: Vec<Value> = args
                    .iter()
                    .map(|a| match a {
                        serde_json::Value::String(s) => Value::Str(s.clone()),
                        serde_json::Value::Bool(b) => Value::Bool(*b),
                        serde_json::Value::Number(n) => Value::F64(n.as_f64().unwrap()),
                        o => panic!("{ctx}: arg {o}"),
                    })
                    .collect();
                // a kind that emits its new value has written it first (a text field's `changed`)
                let kinds = rig.backend.kinds().clone();
                let kind = rig.backend.with_doc(|d| d.kind_of(rig.backend.node(h).unwrap())).unwrap();
                let km = kinds.by_core(kind).unwrap();
                if let Some(sm) = km.signal_named(sig) {
                    for ((arg, _), v) in sm.spec.args.iter().zip(&want) {
                        if km.prop_named(arg).is_some_and(|p| p.spec.native) {
                            rig.native_set(h, arg, pvalue(v));
                        }
                    }
                }
                rig.emit(h, sig, want.iter().map(pvalue).collect());
                rig.backend.pump();
                let l = rig.take();
                let sid = rig.signal_id(h, sig);
                let got: Vec<_> = l.events.iter().filter(|e| e.0 == h && e.1 == sid).collect();
                assert_eq!(got.len(), 1, "{ctx}: delivered {:?}", l.events);
                assert_eq!(got[0].2, want, "{ctx}: event args");
                assert!(l.checkpoints >= 1, "{ctx}: delivered at event time (a checkpoint ran)");
                events += 1;
            }
            // a native-owned prop the kind changed, with its `changed` signal
            "changed" => {
                let (h, prop) = (hd(parts[1]), parts[2]);
                let j: serde_json::Value = serde_json::from_str(parts[3]).unwrap();
                let v = value_for(&rig, h, prop, &j);
                rig.take();
                rig.native_set(h, prop, pvalue(&v));
                rig.emit(h, "changed", vec![pvalue(&v)]);
                rig.backend.pump();
                assert_eq!(rig.get(h, prop), v, "{ctx}: native value");
                let l = rig.take();
                let pid = rig.prop_id(h, prop);
                let announced = l.changed.iter().filter(|c| **c == (h, pid)).count();
                let want = observing.contains(&(h, prop.to_string())) as usize;
                assert_eq!(announced, want, "{ctx}: announcements {:?}", l.changed);
                changes += 1;
            }
            other => panic!("{ctx}: unknown op {other}"),
        }
    }
    (events, changes)
}

#[test]
fn egui_fixtures_replay_on_presto() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut names: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let pkg = std::fs::read_to_string(e.path().join("package.json")).unwrap_or_default();
            // the menus gallery uses Tier 1 kinds (menus, popups, modals) presto does not have yet
            e.path().join("expected.oplog").exists() && pkg.contains("\"toolkit\": \"egui\"") && e.file_name() != "gallery-egui-menus"
        })
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    names.sort();
    assert_eq!(names, ["counter-egui", "if-toggle-egui", "keyed-list-egui", "motion-egui", "timers-egui"]);
    for name in names {
        let log = std::fs::read_to_string(dir.join(&name).join("expected.oplog")).unwrap();
        let (events, changes) = replay(&name, &log);
        if name == "counter-egui" {
            assert_eq!((events, changes), (1, 3));
        }
    }
}
