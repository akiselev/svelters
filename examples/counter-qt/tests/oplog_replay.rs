//! Replays the fixtures' expected op logs (fixtures/*/expected.oplog) against the Qt backend,
//! the Qt twin of crates/svelters-gtk/tests/oplog_replay.rs. `event` lines are produced natively
//! (`click`, typing) and must reach the stub host; `attr` lines set a child attribute and read it
//! back; `listen` lines carry the listener phases (`listen_ex` for `after` / `capture`); `changed` lines
//! are produced natively (`QObject::setProperty`) and must be announced (`prop_changed`) exactly when
//! the pair is observed, as in the GTK replay.
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
/// ABI v2.6 announcements (`prop_changed`) since the last `changed` line.
static CHANGED: Mutex<Vec<(u64, u32)>> = Mutex::new(Vec::new());

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
unsafe extern "C" fn prop_changed(_: *mut c_void, h: svl_handle, prop: svl_id) {
    CHANGED.lock().unwrap().push((h, prop));
}

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

/// Signals of the non-GUI types the replay produces natively: (signal, method to call, its argument).
const TYPE_TRIGGERS: &[(&str, &str, &str)] = &[("contents-changed", "setPlainText", "typed natively")];

/// A JSON value of a log line as an ABI value; a number in a handle slot is a `#n` of the log.
fn json_value(v: &serde_json::Value, handles: &HashMap<String, Value>, handle_slot: bool) -> Value {
    match v {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Bool(*b),
        serde_json::Value::Number(n) if handle_slot && n.is_i64() => {
            handles.get(&format!("#{}", n.as_i64().unwrap())).cloned().unwrap_or_else(|| panic!("unknown handle #{n}"))
        }
        serde_json::Value::Number(n) => n.as_i64().map(Value::I64).unwrap_or_else(|| Value::F64(n.as_f64().unwrap())),
        serde_json::Value::String(s) => Value::Str(s.clone()),
        serde_json::Value::Array(a) => Value::List(a.iter().map(|x| json_value(x, handles, false)).collect()),
        serde_json::Value::Object(o) => Value::Map(o.iter().map(|(k, x)| (k.clone(), json_value(x, handles, false))).collect()),
    }
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
            // ABI v2.5 (N2): `construct #h Kind {props}` and `static Kind.method [args] [#h]` on the Qt type kinds.
            "construct" => {
                let k = b.resolve(NameKind::Kind, 0, parts[2]);
                if k == 0 {
                    return Some(format!("no Qt type {}", parts[2]));
                }
                kinds.insert(parts[1].to_string(), k);
            }
            "static" => {
                let (call, args) = line["static ".len()..].split_once(' ').unwrap();
                let (kind, method) = call.rsplit_once('.').unwrap();
                let k = b.resolve(NameKind::Kind, 0, kind);
                if k == 0 || b.resolve(NameKind::Method, k, &format!("static:{method}")) == 0 {
                    return Some(format!("no Qt static {call}"));
                }
                if let Some((_, h)) = args.rsplit_once(" #") {
                    kinds.insert(format!("#{h}"), k);
                }
            }
            "event" if TYPE_TRIGGERS.iter().any(|t| t.0 == parts[2]) => {}
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
                // Element methods take no arguments in the logs; type-layer methods take JSON arguments.
                let typed = k >= 0x20000 && k < 0x200000;
                if b.resolve(NameKind::Method, k, parts[2]) == 0 || (!typed && parts.get(3).is_some_and(|a| a.trim() != "[]")) {
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
    // Results of construct / static (N2): handles and VALUE handles, by their `#n` in the log.
    let mut vals: HashMap<String, Value> = HashMap::new();
    let (mut events, mut attrs, mut changes) = (0, 0, 0);
    // (handle, prop) pairs observed at this point of the log (the host refcounts; the log shows only the
    // 0->1 and 1->0 transitions).
    let mut observing: std::collections::HashSet<(u64, svl_id)> = std::collections::HashSet::new();
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
            "construct" => {
                let k = b.resolve(NameKind::Kind, 0, parts[2]);
                let props: serde_json::Value = serde_json::from_str(parts[3]).unwrap_or_else(|e| panic!("{ctx}: {e}"));
                let (mut ids, mut values) = (Vec::new(), Vec::new());
                for (name, v) in props.as_object().unwrap() {
                    let p = b.resolve(NameKind::Prop, k, name);
                    let slot = b.describe(NameKind::Prop, k, p).and_then(|d| d.params.first().map(|s| s.tag));
                    ids.push(p);
                    values.push(json_value(v, &vals, matches!(slot, Some(SVL_HANDLE | SVL_BOXED))));
                }
                let v = b.construct(k, &ids, &values).unwrap_or_else(|e| panic!("{ctx}: {}", e.message));
                let Value::Handle(h) = v else { panic!("{ctx}: construct answered {v:?}") };
                map.insert(parts[1].to_string(), h);
                kinds.insert(h, k);
                vals.insert(parts[1].to_string(), v);
            }
            "static" => {
                let (call, args) = line["static ".len()..].split_once(' ').unwrap();
                let (kname, method) = call.rsplit_once('.').unwrap();
                let (json, result) = match args.rsplit_once(" #") {
                    Some((j, n)) => (j, Some(format!("#{n}"))),
                    None => (args, None),
                };
                let k = b.resolve(NameKind::Kind, 0, kname);
                let m = b.resolve(NameKind::Method, k, &format!("static:{method}"));
                let args: serde_json::Value = serde_json::from_str(json).unwrap_or_else(|e| panic!("{ctx}: {e}"));
                let args: Vec<Value> = args.as_array().unwrap().iter().map(|v| json_value(v, &vals, false)).collect();
                let v = b.call_static(k, m, &args).unwrap_or_else(|e| panic!("{ctx}: {}", e.message));
                if let Some(name) = result {
                    let h = match &v {
                        Value::Handle(h) | Value::Boxed { handle: h, .. } => *h,
                        other => panic!("{ctx}: a constructor answered {other:?}"),
                    };
                    map.insert(name.clone(), h);
                    kinds.insert(h, b.handle_info(h).map(|i| i.kind).unwrap_or(k));
                    vals.insert(name, v);
                }
            }
            "remove" => b.remove(hd(parts[1]), hd(parts[2])).unwrap_or_else(|e| panic!("{ctx}: {}", e.message)),
            "destroy" => b.destroy(hd(parts[1])),
            "set" => {
                let o = hd(parts[1]);
                let prop = b.resolve(NameKind::Prop, kind(o), parts[2]);
                let mut v = parse_val(parts[3]).value();
                // A handle or VALUE handle the app made (`set #7 document 1`): the log's `#n`.
                if let Value::I64(n) = v {
                    let slot = b.describe(NameKind::Prop, kind(o), prop).and_then(|d| d.params.first().map(|p| p.tag));
                    if matches!(slot, Some(SVL_HANDLE | SVL_BOXED)) {
                        v = vals.get(&format!("#{n}")).cloned().unwrap_or_else(|| panic!("{ctx}: unknown handle #{n}"));
                    }
                }
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
                let args: serde_json::Value = serde_json::from_str(parts.get(3).copied().unwrap_or("[]")).unwrap_or_else(|e| panic!("{ctx}: {e}"));
                let args: Vec<Value> = args.as_array().unwrap().iter().map(|v| json_value(v, &vals, false)).collect();
                b.call(o, m, &args).unwrap_or_else(|e| panic!("{ctx}: {}", e.message));
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
            // ABI v2.6 observation (R2 `bind:` lowering): `observe #h prop on|off` must be accepted natively.
            "observe" => {
                let o = hd(parts[1]);
                let prop = b.resolve(NameKind::Prop, kind(o), parts[2]);
                let on = parts[3].trim() == "on";
                b.observe(o, prop, on).unwrap_or_else(|e| panic!("{ctx}: {}", e.message));
                if on {
                    observing.insert((o, prop));
                } else {
                    observing.remove(&(o, prop));
                }
            }
            // A scripted native change (FU3, as the GTK replay): produce it natively (`QObject::setProperty`,
            // not the ABI's `set_prop`) and check the backend announced it exactly when JS observes the
            // property (a `bind:p|nonreactive` target changes natively without being observed).
            "changed" => {
                let o = hd(parts[1]);
                let prop = b.resolve(NameKind::Prop, kind(o), parts[2]);
                let Some(v) = parts.get(3) else { panic!("{ctx}: a changed line without a value cannot be produced") };
                CHANGED.lock().unwrap().clear();
                counter_qt::hooks::native_set(o, parts[2], &parse_val(v).value()).unwrap_or_else(|e| panic!("{ctx}: {e}"));
                // Notifiers that hop through the loop (deferred geometry, another thread) get a turn.
                counter_qt::hooks::pump(20);
                let got = CHANGED.lock().unwrap().clone();
                if observing.contains(&(o, prop)) {
                    assert!(got.contains(&(o, prop)), "{ctx}: change not announced, got {got:?}");
                } else {
                    assert!(!got.iter().any(|c| c.0 == o), "{ctx}: an unobserved change was announced: {got:?}");
                }
                changes += 1;
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
                // A signal of a non-GUI type (N2), produced by changing the object natively through its method.
                if let Some((_, method, arg)) = TYPE_TRIGGERS.iter().find(|t| t.0 == sig) {
                    let m = b.resolve(NameKind::Method, kind(o), method);
                    b.call(o, m, &[Value::Str(arg.to_string())]).unwrap_or_else(|e| panic!("{ctx}: {}", e.message));
                    let got = EVENTS.lock().unwrap().clone();
                    assert!(got.contains(&(o, sid)), "{ctx}: event not delivered, got {got:?}");
                    events += 1;
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
    eprintln!("{name}: replayed, {events} native events delivered, {changes} native changes, {attrs} attributes set and read back");
}

fn main() {
    counter_qt::run_as_test("replay_all_fixtures", body);
}

fn body() {
    let host = svl_host { self_: std::ptr::null_mut(), post_event, checkpoint, call_sync, on_destroyed, invoke_callback, release_callback, frame, prop_changed: Some(prop_changed) };
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
    for must in ["bind-observe-qt", "child-attrs-qt", "counter-qt", "qt-types"] {
        assert!(replayed.iter().any(|n| n == must), "fixtures/{must} must replay on Qt, replayed {replayed:?}");
    }
    println!("oplog_replay ok: {replayed:?}");
}
