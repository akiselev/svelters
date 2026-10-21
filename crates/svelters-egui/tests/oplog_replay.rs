//! Replays the egui fixtures' op logs (`fixtures/*/expected.oplog` with `"toolkit": "egui"`: the
//! egui twins of the P1 fixtures counter, if-toggle, keyed-list, timers) against the egui backend.
//! Ops go through the real vtable; `event` and `changed` lines are produced natively, by input to the
//! kittest harness, and must reach the recording host (`changed` exactly when the log observes it).

mod common;

use common::Rig;
use egui_kittest::kittest::Queryable;
use std::collections::{HashMap, HashSet};
use svelters_abi::*;

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

fn replay(name: &str, log: &str) -> (usize, usize) {
    let mut rig = Rig::new();
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
                    "sheet" => br.style(SVL_STYLE_SHEET, 0, text(parts[3]).as_deref()),
                    "scheme" => br.style(SVL_STYLE_SCHEME, 0, text(parts[2]).as_deref()),
                    _ => {
                        let op = match parts[2] {
                            "classes" => SVL_STYLE_CLASSES,
                            "name" => SVL_STYLE_NAME,
                            "inline" => SVL_STYLE_INLINE,
                            "opacity" => SVL_STYLE_OPACITY,
                            o => panic!("{ctx}: style op {o}"),
                        };
                        br.style(op, hd(parts[1]), text(parts[3]).as_deref())
                    }
                };
                r.unwrap_or_else(|e| panic!("{ctx}: {e}"));
            }
            // Native input: produce it on the widget and check it reached the host.
            "event" => {
                let (h, sig) = (hd(parts[1]), parts[2]);
                rig.run();
                rig.take();
                let args: Vec<serde_json::Value> = serde_json::from_str(parts[3]).unwrap();
                match (sig, rig.backend.with_scene(|s| s.n(h).tag())) {
                    ("clicked", _) => rig.click(h),
                    ("changed", "text-edit") => rig.type_into(h, args[0].as_str().unwrap()),
                    other => panic!("{ctx}: cannot produce {other:?}"),
                }
                let l = rig.take();
                let sid = rig.signal_id(h, sig);
                let got: Vec<_> = l.events.iter().filter(|e| e.0 == h && e.1 == sid).collect();
                assert_eq!(got.len(), 1, "{ctx}: delivered {:?}", l.events);
                let want: Vec<Value> = args.iter().map(|a| match a {
                    serde_json::Value::String(s) => Value::Str(s.clone()),
                    serde_json::Value::Bool(b) => Value::Bool(*b),
                    serde_json::Value::Number(n) => Value::F64(n.as_f64().unwrap()),
                    o => panic!("{ctx}: arg {o}"),
                }).collect();
                assert_eq!(got[0].2, want, "{ctx}: event args");
                events += 1;
            }
            "changed" => {
                let (h, prop) = (hd(parts[1]), parts[2]);
                let j: serde_json::Value = serde_json::from_str(parts[3]).unwrap();
                rig.run();
                rig.take();
                match prop {
                    "text" => rig.type_into(h, j.as_str().unwrap()),
                    "checked" => {
                        if rig.get(h, "checked") != Value::Bool(j.as_bool().unwrap()) {
                            rig.click(h);
                        }
                    }
                    "value" => {
                        let name = match rig.get(h, "accessible-name") {
                            Value::Str(s) => s,
                            _ => unreachable!(),
                        };
                        rig.harness.get_by_label(&name).set_value_accesskit(j.as_f64().unwrap());
                        rig.run();
                    }
                    p => panic!("{ctx}: cannot change {p} natively"),
                }
                assert_eq!(rig.get(h, prop), value_for(&rig, h, prop, &j), "{ctx}: native value");
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
    // the final tree is drawn and every leaf fits the rect layout gave it
    rig.run();
    rig.backend.with_scene(|s| {
        for h in s.handles() {
            let n = s.n(h);
            if n.children.is_empty() && !n.is_window() && n.drawn != egui::Rect::NOTHING {
                let laid = n.rect.size();
                let drawn = n.drawn.size();
                assert!(drawn.x <= laid.x + 1.0 && drawn.y <= laid.y + 1.0, "{name}: <{}> drawn {drawn:?} > laid out {laid:?}", n.tag());
            }
        }
    });
    (events, changes)
}

#[test]
fn replay_egui_fixtures() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut names: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let pkg = std::fs::read_to_string(e.path().join("package.json")).unwrap_or_default();
            e.path().join("expected.oplog").exists() && pkg.contains("\"toolkit\": \"egui\"")
        })
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    names.sort();
    assert_eq!(names, ["counter-egui", "if-toggle-egui", "keyed-list-egui", "timers-egui"]);
    for name in names {
        let log = std::fs::read_to_string(dir.join(&name).join("expected.oplog")).unwrap();
        let (events, changes) = replay(&name, &log);
        eprintln!("{name}: replayed, {events} native events and {changes} native changes delivered");
    }
}
