//! Replays the P1 fixtures' expected op logs (fixtures/*/expected.oplog) against the GTK backend.
//! `event` lines are produced natively (click / typing) and must reach the stub host.

use gtk::prelude::*;
use std::collections::HashMap;
use svelters_gtk::testing::{on_gtk, Harness};

enum Val {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
}

fn parse_val(s: &str) -> Val {
    let s = s.trim();
    match s {
        "null" => Val::Null,
        "true" => Val::Bool(true),
        "false" => Val::Bool(false),
        _ if s.starts_with('"') => {
            let inner = &s[1..s.len() - 1];
            let mut out = String::new();
            let mut it = inner.chars();
            while let Some(c) = it.next() {
                if c != '\\' {
                    out.push(c);
                    continue;
                }
                match it.next() {
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some('u') => {
                        let hex: String = it.by_ref().take(4).collect();
                        out.push(char::from_u32(u32::from_str_radix(&hex, 16).unwrap()).unwrap());
                    }
                    Some(c) => out.push(c),
                    None => panic!("dangling escape"),
                }
            }
            Val::Str(out)
        }
        _ => s.parse::<i64>().map(Val::Int).unwrap_or_else(|_| Val::Float(s.parse().unwrap())),
    }
}

fn replay(name: &str, log: String) {
    let name = name.to_string();
    on_gtk(move || {
        let h = Harness::new();
        let mut map: HashMap<String, u64> = HashMap::new();
        let mut events = 0;
        let mut gone_queue: Vec<(u64, u32)> = Vec::new();
        for (n, line) in log.lines().enumerate().filter(|(_, l)| !l.trim().is_empty()) {
            let ctx = format!("{name}:{}: {line}", n + 1);
            let parts: Vec<&str> = line.splitn(4, ' ').collect();
            let hd = |t: &str| *map.get(t).unwrap_or_else(|| panic!("{ctx}: unknown handle {t}"));
            match parts[0] {
                "create" => {
                    let id = h.try_create(parts[1]).unwrap_or_else(|e| panic!("{ctx}: {e}"));
                    map.insert(parts[2].to_string(), id);
                }
                "insert" => {
                    let rest: Vec<&str> = line.split(' ').collect();
                    let before = match rest[3].strip_prefix("before=").unwrap() {
                        "end" => None,
                        t => Some(hd(t)),
                    };
                    let slot = match rest[4].strip_prefix("slot=").unwrap() {
                        "default" => None,
                        s => Some(s),
                    };
                    h.insert(hd(rest[1]), hd(rest[2]), before, slot).unwrap_or_else(|e| panic!("{ctx}: {e}"));
                }
                "remove" => h.remove(hd(parts[1]), hd(parts[2])).unwrap_or_else(|e| panic!("{ctx}: {e}")),
                "destroy" => h.destroy(hd(parts[1])),
                "set" => {
                    let (o, prop) = (hd(parts[1]), parts[2]);
                    let r = match parse_val(parts[3]) {
                        Val::Str(s) => h.set_str(o, prop, &s),
                        Val::Int(i) => h.set_i64(o, prop, i),
                        Val::Bool(b) => h.set_bool(o, prop, b),
                        Val::Float(_) | Val::Null => panic!("{ctx}: unsupported value"),
                    };
                    r.unwrap_or_else(|e| panic!("{ctx}: {e}"));
                }
                "call" => h.call(hd(parts[1]), parts[2]).unwrap_or_else(|e| panic!("{ctx}: {e}")),
                "listen" => h.listen(hd(parts[1]), parts[2], parts[3] == "on").unwrap_or_else(|e| panic!("{ctx}: {e}")),
                // A native death in the log: the first `gone` of a run is produced by closing
                // the window natively; the descendants it lists (`parent`) must be reported by the
                // backend's cascade, in order.
                "gone" => {
                    let o = hd(parts[1]);
                    let reason = svelters_abi::gone_from_name(parts[2]).unwrap_or_else(|| panic!("{ctx}: bad reason"));
                    if reason == svelters_abi::SVL_GONE_PARENT {
                        let first = gone_queue.remove(0);
                        assert_eq!(first, (o, reason), "{ctx}: cascade order/reason");
                    } else {
                        assert_eq!(reason, svelters_abi::SVL_GONE_CLOSED, "{ctx}: only closed can be produced natively");
                        svelters_gtk::testing::pump(150); // realize the window; close() ignores unrealized ones
                        h.widget(o).downcast::<gtk::Window>().unwrap().close();
                        gone_queue = h.take_gone();
                        assert_eq!(gone_queue.remove(0), (o, reason), "{ctx}: window death not reported");
                    }
                }
                "event" => {
                    let (o, sig) = (hd(parts[1]), parts[2]);
                    h.take_events();
                    match sig {
                        "clicked" => h.widget(o).downcast::<gtk::Button>().unwrap().emit_clicked(),
                        "changed" => {
                            let Val::Str(s) = parse_val(parts[3].trim_start_matches('[').trim_end_matches(']')) else { panic!() };
                            h.widget(o).dynamic_cast::<gtk::Editable>().unwrap().set_text(&s);
                        }
                        other => panic!("{ctx}: cannot produce {other}"),
                    }
                    let got = h.take_events();
                    assert!(got.contains(&(o, sig.to_string())), "{ctx}: event not delivered, got {got:?}");
                    events += 1;
                }
                other => panic!("{ctx}: unknown op {other}"),
            }
        }
        eprintln!("{name}: replayed, {events} native events delivered");
    });
}

#[test]
fn replay_all_fixtures() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut names: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).filter(|e| e.path().join("expected.oplog").exists()).collect();
    names.sort_by_key(|e| e.file_name());
    assert!(!names.is_empty(), "no fixtures found");
    for e in names {
        replay(&e.file_name().to_string_lossy(), std::fs::read_to_string(e.path().join("expected.oplog")).unwrap());
    }
}
