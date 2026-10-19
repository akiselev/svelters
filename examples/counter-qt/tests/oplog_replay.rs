//! Replays the fixtures' expected op logs (fixtures/*/expected.oplog) against the Qt backend,
//! the Qt twin of crates/svelters-gtk/tests/oplog_replay.rs. `event` lines are produced natively
//! (`click`, typing) and must reach the stub host; `attr` lines set a child attribute and read it
//! back; `listen` lines carry the listener phases (`listen_ex` for `after` / `capture`).
//!
//! Fixtures that need more than the Qt backend offers are skipped with the reason: a `sync` line
//! (a result changes what the toolkit does next), `gone` lines, or a kind / prop / signal / slot /
//! method the Qt element set does not have (most fixtures are written in the GTK vocabulary; the
//! `*-qt` twins are written in the Qt vocabulary).
//! Needs a display (xvfb-run) and the main thread, so no libtest harness.

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::Mutex;
use svelters_abi::*;

static EVENTS: Mutex<Vec<(u64, u32)>> = Mutex::new(Vec::new());

unsafe extern "C" fn post_event(_: *mut c_void, h: svl_handle, sig: svl_id, _: *const svl_value, _: usize) {
    EVENTS.lock().unwrap().push((h, sig));
}
unsafe extern "C" fn checkpoint(_: *mut c_void) {}
unsafe extern "C" fn call_sync(_: *mut c_void, _: svl_handle, _: svl_id, _: *const svl_value, _: usize, _: *mut svl_value) -> i32 {
    SVL_SYNC_FAILED
}
unsafe extern "C" fn on_destroyed(_: *mut c_void, _: svl_handle, _: u32) {}
unsafe extern "C" fn invoke_callback(_: *mut c_void, _: u64, _: *const svl_value, _: usize, _: *mut svl_value) -> i32 {
    -1
}
unsafe extern "C" fn release_callback(_: *mut c_void, _: u64) {}
unsafe extern "C" fn frame(_: *mut c_void) {}

enum Val {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
}

impl Val {
    fn value(self) -> Value {
        match self {
            Val::Null => Value::Null,
            Val::Bool(b) => Value::Bool(b),
            Val::Int(i) => Value::I64(i),
            Val::Float(f) => Value::F64(f),
            Val::Str(s) => Value::Str(s),
        }
    }
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

fn listen_flags(words: &str) -> u32 {
    let mut flags = 0;
    for w in words.split('+').filter(|w| !w.is_empty()) {
        flags |= match w {
            "after" => SVL_LISTEN_AFTER,
            "capture" => SVL_LISTEN_CAPTURE,
            other => panic!("unknown listener flag {other}"),
        };
    }
    flags
}

/// `Some(reason)` when the Qt backend cannot replay `log`. Walks the log once with the same name
/// resolution the replay uses.
fn unsupported(b: &BackendRef, log: &str) -> Option<String> {
    let mut kinds: HashMap<String, svl_id> = HashMap::new();
    for line in log.lines().filter(|l| !l.trim().is_empty()) {
        let parts: Vec<&str> = line.splitn(4, ' ').collect();
        match parts[0] {
            "sync" | "result" => return Some("sync handlers".into()),
            "gone" => return Some("native deaths (gone)".into()),
            "construct" | "static" => return Some("non-GUI types (construct/static, N2)".into()),
            "create" => {
                let k = b.resolve(NameKind::Kind, 0, parts[1]);
                if k == 0 {
                    return Some(format!("no Qt element <{}>", parts[1]));
                }
                kinds.insert(parts[2].to_string(), k);
            }
            "set" => {
                let k = kinds[parts[1]];
                if b.resolve(NameKind::Prop, k, parts[2]) == 0 {
                    return Some(format!("no Qt prop {}", parts[2]));
                }
            }
            "listen" => {
                let k = kinds[parts[1]];
                if b.resolve(NameKind::Signal, k, parts[2]) == 0 {
                    return Some(format!("no Qt signal {}", parts[2]));
                }
            }
            "call" => {
                let k = kinds[parts[1]];
                if b.resolve(NameKind::Method, k, parts[2]) == 0 || parts.get(3).is_some_and(|a| a.trim() != "[]") {
                    return Some(format!("no Qt method {} with these arguments", parts[2]));
                }
            }
            "insert" => {
                let rest: Vec<&str> = line.split(' ').collect();
                if let Some(slot) = rest[4].strip_prefix("slot=").filter(|s| *s != "default") {
                    if b.resolve(NameKind::Slot, kinds[rest[1]], slot) == 0 {
                        return Some(format!("no Qt slot {slot}"));
                    }
                }
            }
            "attr" => {
                let rest: Vec<&str> = line.splitn(5, ' ').collect();
                if b.resolve(NameKind::Attr, kinds[rest[1]], rest[3]) == 0 {
                    return Some(format!("no Qt child attribute {}", rest[3]));
                }
            }
            "event" if !matches!(parts[2], "clicked" | "triggered" | "toggled") => return Some(format!("cannot produce {} natively", parts[2])),
            _ => {}
        }
    }
    None
}

fn replay(b: &BackendRef, name: &str, log: &str) {
    let mut map: HashMap<String, u64> = HashMap::new();
    let mut kinds: HashMap<u64, svl_id> = HashMap::new();
    let (mut events, mut attrs) = (0, 0);
    for (n, line) in log.lines().enumerate().filter(|(_, l)| !l.trim().is_empty()) {
        let ctx = format!("{name}:{}: {line}", n + 1);
        let parts: Vec<&str> = line.splitn(4, ' ').collect();
        let hd = |t: &str| *map.get(t).unwrap_or_else(|| panic!("{ctx}: unknown handle {t}"));
        let kind = |h: u64| kinds[&h];
        match parts[0] {
            "create" => {
                let k = b.resolve(NameKind::Kind, 0, parts[1]);
                let h = b.create(k).unwrap_or_else(|e| panic!("{ctx}: {}", e.message));
                map.insert(parts[2].to_string(), h);
                kinds.insert(h, k);
            }
            "insert" => {
                let rest: Vec<&str> = line.split(' ').collect();
                let before = match rest[3].strip_prefix("before=").unwrap() {
                    "end" => None,
                    t => Some(hd(t)),
                };
                let slot = match rest[4].strip_prefix("slot=").unwrap() {
                    "default" => 0,
                    s => b.resolve(NameKind::Slot, kind(hd(rest[1])), s),
                };
                b.insert(hd(rest[1]), hd(rest[2]), before, slot).unwrap_or_else(|e| panic!("{ctx}: {}", e.message));
            }
            "remove" => b.remove(hd(parts[1]), hd(parts[2])).unwrap_or_else(|e| panic!("{ctx}: {}", e.message)),
            "destroy" => b.destroy(hd(parts[1])),
            "set" => {
                let o = hd(parts[1]);
                let prop = b.resolve(NameKind::Prop, kind(o), parts[2]);
                let mut v = parse_val(parts[3]).value();
                // A bare attribute (`<action checkable>`) is text until the renderer coerces it by the prop's descriptor.
                if let svelters_abi::Value::Str(t) = &v {
                    if matches!(t.as_str(), "" | "true" | "false") && b.describe(NameKind::Prop, kind(o), prop).is_some_and(|d| d.params.first().is_some_and(|p| p.tag == svelters_abi::SVL_BOOL)) {
                        v = svelters_abi::Value::Bool(t != "false");
                    }
                }
                b.set_prop(o, prop, &v).unwrap_or_else(|e| panic!("{ctx}: {}", e.message));
            }
            "call" => {
                let o = hd(parts[1]);
                let m = b.resolve(NameKind::Method, kind(o), parts[2]);
                b.call(o, m, &[]).unwrap_or_else(|e| panic!("{ctx}: {}", e.message));
            }
            "listen" => {
                let o = hd(parts[1]);
                let sig = b.resolve(NameKind::Signal, kind(o), parts[2]);
                let mut words = parts[3].split(' ');
                let on = words.next() == Some("on");
                let flags = listen_flags(words.next().unwrap_or(""));
                let r = if flags == 0 { b.listen(o, sig, on) } else { b.listen_ex(o, sig, on, flags) };
                r.unwrap_or_else(|e| panic!("{ctx}: {}", e.message));
            }
            // ABI v2.2 child attribute: `attr #parent #child name json`.
            "attr" => {
                let rest: Vec<&str> = line.splitn(5, ' ').collect();
                let (p, c) = (hd(rest[1]), hd(rest[2]));
                let attr = b.resolve(NameKind::Attr, kind(p), rest[3]);
                let v = parse_val(rest[4]).value();
                b.set_child_attr(p, c, attr, &v).unwrap_or_else(|e| panic!("{ctx}: {}", e.message));
                if v != Value::Null {
                    assert_eq!(b.get_child_attr(p, c, attr).unwrap_or_else(|e| panic!("{ctx}: {}", e.message)), v, "{ctx}: read back");
                }
                attrs += 1;
            }
            // ABI v2.3 styling. The logs speak the CSS dialect, so sheet and inline text (which Qt
            // would read as QSS) is skipped; classes, names and opacity replay as they are.
            "style" => {
                let text = |s: &str| match parse_val(s).value() {
                    Value::Str(t) => Some(t),
                    _ => None,
                };
                match parts[1] {
                    "sheet" => {
                        b.style(SVL_STYLE_SHEET, 0, None).unwrap_or_else(|e| panic!("{ctx}: {}", e.message));
                    }
                    "scheme" => {
                        let Some(sc) = text(parts[2]) else { panic!("{ctx}: scheme") };
                        b.style(SVL_STYLE_SCHEME, 0, Some(&sc)).unwrap_or_else(|e| panic!("{ctx}: {}", e.message));
                    }
                    _ => {
                        let op = match parts[2] {
                            "classes" => SVL_STYLE_CLASSES,
                            "name" => SVL_STYLE_NAME,
                            "opacity" => SVL_STYLE_OPACITY,
                            _ => continue,
                        };
                        let t = text(parts[3]);
                        b.style(op, hd(parts[1]), t.as_deref()).unwrap_or_else(|e| panic!("{ctx}: {}", e.message));
                        if op == SVL_STYLE_CLASSES {
                            let cls = counter_qt::hooks::native_prop(hd(parts[1]), "class", false);
                            for c in t.as_deref().unwrap_or("").split_whitespace() {
                                assert!(cls.contains(c), "{ctx}: class {c} missing in {cls}");
                            }
                        }
                    }
                }
            }
            "event" => {
                let (o, sig) = (hd(parts[1]), parts[2]);
                let sid = b.resolve(NameKind::Signal, kind(o), sig);
                EVENTS.lock().unwrap().clear();
                // Menu actions (X4): `trigger()` is what a click or shortcut does. A radio member turning
                // off is the consequence of its sibling turning on.
                if sig == "toggled" && parts[3] == "[false]" {
                    continue;
                }
                let click = b.resolve(NameKind::Method, kind(o), if matches!(sig, "triggered" | "toggled") { "trigger" } else { "click" });
                b.call(o, click, &[]).unwrap_or_else(|e| panic!("{ctx}: {}", e.message));
                let got = EVENTS.lock().unwrap().clone();
                assert!(got.contains(&(o, sid)), "{ctx}: event not delivered, got {got:?}");
                events += 1;
            }
            other => panic!("{ctx}: unknown op {other}"),
        }
    }
    eprintln!("{name}: replayed, {events} native events delivered, {attrs} attributes set and read back");
}

fn main() {
    counter_qt::run_as_test("replay_all_fixtures", body);
}

fn body() {
    let host = svl_host { self_: std::ptr::null_mut(), post_event, checkpoint, call_sync, on_destroyed, invoke_callback, release_callback, frame };
    unsafe { counter_qt::set_host(&host) };
    let b = unsafe { BackendRef::from_raw(counter_qt::backend()) }.unwrap();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut names: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).filter(|e| e.path().join("expected.oplog").exists()).collect();
    names.sort_by_key(|e| e.file_name());
    assert!(!names.is_empty(), "no fixtures found");
    let mut replayed = Vec::new();
    for e in names {
        let name = e.file_name().to_string_lossy().into_owned();
        let log = std::fs::read_to_string(e.path().join("expected.oplog")).unwrap();
        if let Some(why) = unsupported(&b, &log) {
            eprintln!("{name}: skipped ({why})");
            continue;
        }
        replay(&b, &name, &log);
        replayed.push(name);
    }
    for must in ["child-attrs-qt", "counter-qt"] {
        assert!(replayed.iter().any(|n| n == must), "fixtures/{must} must replay on Qt, replayed {replayed:?}");
    }
    println!("oplog_replay ok: {replayed:?}");
}
