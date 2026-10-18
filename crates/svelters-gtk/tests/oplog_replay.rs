//! Replays the P1 fixtures' expected op logs (fixtures/*/expected.oplog) against the GTK backend.
//! `event` lines are produced natively (click / typing) and must reach the stub host.

use gtk::prelude::*;
use std::collections::HashMap;
use svelters_gtk::testing::{on_gtk, Harness, V};

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

/// A JSON value of an op-log line as a harness value. A bare number where a handle is expected is
/// resolved through `handles` (the log prints handles as plain numbers).
fn json_v(v: &serde_json::Value, handle: &dyn Fn(i64) -> u64, as_handle: bool) -> V {
    use serde_json::Value as J;
    match v {
        J::Null => V::Null,
        J::Bool(b) => V::Bool(*b),
        J::Number(n) if as_handle => V::Handle(handle(n.as_i64().expect("handle number"))),
        J::Number(n) => n.as_i64().map(V::I64).unwrap_or_else(|| V::F64(n.as_f64().unwrap())),
        J::String(s) => V::Str(s.clone()),
        J::Array(a) => V::List(a.iter().map(|x| json_v(x, handle, false)).collect()),
        J::Object(_) => panic!("objects are not replayed"),
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
                    let is_handle = h.prop_desc(o, prop).and_then(|d| d.params.first().map(|p| p.tag)) == Some(svelters_abi::SVL_HANDLE);
                    let r = match parse_val(parts[3]) {
                        Val::Int(i) if is_handle => h.set_handle(o, prop, hd(&format!("#{i}"))),
                        Val::Str(s) => h.set_str(o, prop, &s),
                        Val::Int(i) => h.set_i64(o, prop, i),
                        Val::Bool(b) => h.set_bool(o, prop, b),
                        Val::Float(_) | Val::Null => panic!("{ctx}: unsupported value"),
                    };
                    r.unwrap_or_else(|e| panic!("{ctx}: {e}"));
                }
                // ABI v2.5: `construct #h Kind {props}` and `static Kind.method [args] [#h]`.
                "construct" => {
                    let props: serde_json::Value = serde_json::from_str(parts[3]).unwrap_or_else(|e| panic!("{ctx}: {e}"));
                    let by_num = |n: i64| *map.get(&format!("#{n}")).unwrap_or_else(|| panic!("{ctx}: unknown handle #{n}"));
                    let props: Vec<(String, V)> = props
                        .as_object()
                        .unwrap()
                        .iter()
                        .map(|(k, v)| {
                            let is_handle = h.prop_slot_tag(parts[2], k) == Some(svelters_abi::SVL_HANDLE);
                            (k.clone(), json_v(v, &by_num, is_handle))
                        })
                        .collect();
                    let refs: Vec<(&str, V)> = props.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
                    let id = h.construct(parts[2], &refs).unwrap_or_else(|e| panic!("{ctx}: {e}"));
                    map.insert(parts[1].to_string(), id);
                }
                "static" => {
                    let (call, args) = line["static ".len()..].split_once(' ').unwrap();
                    let (kind, method) = call.rsplit_once('.').unwrap();
                    let (json, result) = match args.rsplit_once(" #") {
                        Some((j, n)) => (j, Some(format!("#{n}"))),
                        None => (args, None),
                    };
                    let args: serde_json::Value = serde_json::from_str(json).unwrap_or_else(|e| panic!("{ctx}: {e}"));
                    let by_num = |n: i64| *map.get(&format!("#{n}")).unwrap_or_else(|| panic!("{ctx}: unknown handle #{n}"));
                    let args: Vec<V> = args.as_array().unwrap().iter().map(|v| json_v(v, &by_num, false)).collect();
                    let r = h.call_static(kind, method, &args).unwrap_or_else(|e| panic!("{ctx}: {e}"));
                    if let (Some(name), V::Handle(id)) = (result, r) {
                        map.insert(name, id);
                    }
                }
                "call" if parts[3] != "[]" => {
                    let args: serde_json::Value = serde_json::from_str(parts[3]).unwrap_or_else(|e| panic!("{ctx}: {e}"));
                    let by_num = |n: i64| *map.get(&format!("#{n}")).unwrap_or_else(|| panic!("{ctx}: unknown handle #{n}"));
                    let args: Vec<V> = args.as_array().unwrap().iter().map(|v| json_v(v, &by_num, false)).collect();
                    h.call_args(hd(parts[1]), parts[2], &args).unwrap_or_else(|e| panic!("{ctx}: {e}"));
                }
                "call" => h.call(hd(parts[1]), parts[2]).unwrap_or_else(|e| panic!("{ctx}: {e}")),
                "listen" => {
                    let mut words = parts[3].split(' ');
                    let on = words.next() == Some("on");
                    let mut flags = 0;
                    for w in words.next().unwrap_or("").split('+').filter(|w| !w.is_empty()) {
                        flags |= match w {
                            "after" => svelters_abi::SVL_LISTEN_AFTER,
                            "capture" => svelters_abi::SVL_LISTEN_CAPTURE,
                            other => panic!("{ctx}: unknown listener flag {other}"),
                        };
                    }
                    let r = if flags == 0 { h.listen(hd(parts[1]), parts[2], on) } else { h.listen_ex(hd(parts[1]), parts[2], on, flags) };
                    r.unwrap_or_else(|e| panic!("{ctx}: {e}"));
                }
                // ABI v2.2 child attribute: `attr #parent #child name json`.
                "attr" => {
                    let rest: Vec<&str> = line.splitn(5, ' ').collect();
                    let v = match parse_val(rest[4]) {
                        Val::Null => V::Null,
                        Val::Bool(b) => V::Bool(b),
                        Val::Int(i) => V::I64(i),
                        Val::Float(f) => V::F64(f),
                        Val::Str(s) => V::Str(s),
                    };
                    h.set_child_attr(hd(rest[1]), hd(rest[2]), rest[3], &v).unwrap_or_else(|e| panic!("{ctx}: {e}"));
                    if !matches!(v, V::Null) {
                        assert_eq!(h.get_child_attr(hd(rest[1]), hd(rest[2]), rest[3]), v, "{ctx}: read back");
                    }
                }
                // ABI v2.3 styling: `style #h classes|name|inline|opacity json`, `style sheet "id" json`,
                // `style scheme "dark" n` (the palette pairs only matter to Qt).
                "style" => {
                    use svelters_abi::*;
                    let text = |v: Val| match v {
                        Val::Str(s) => Some(s),
                        Val::Null => None,
                        _ => panic!("{ctx}: style payload must be a string or null"),
                    };
                    let r = if parts[1] == "sheet" {
                        let Val::Str(id) = parse_val(parts[2]) else { panic!("{ctx}: sheet id") };
                        match (id.as_str(), text(parse_val(parts[3]))) {
                            ("*", _) => h.style(SVL_STYLE_SHEET, 0, None),
                            (_, None) => h.style(SVL_STYLE_SHEET, 0, Some(&id)),
                            (_, Some(body)) => h.style(SVL_STYLE_SHEET, 0, Some(&format!("{id}{SVL_STYLE_SEP}{body}"))),
                        }
                    } else if parts[1] == "scheme" {
                        let Val::Str(s) = parse_val(parts[2]) else { panic!("{ctx}: scheme") };
                        h.style(SVL_STYLE_SCHEME, 0, Some(&s))
                    } else {
                        let op = match parts[2] {
                            "classes" => SVL_STYLE_CLASSES,
                            "name" => SVL_STYLE_NAME,
                            "inline" => SVL_STYLE_INLINE,
                            "opacity" => SVL_STYLE_OPACITY,
                            other => panic!("{ctx}: unknown style op {other}"),
                        };
                        let t = text(parse_val(parts[3]));
                        let r = h.style(op, hd(parts[1]), t.as_deref());
                        if op == SVL_STYLE_CLASSES {
                            for c in t.as_deref().unwrap_or("").split_whitespace() {
                                assert!(h.widget(hd(parts[1])).has_css_class(c), "{ctx}: class {c} not applied");
                            }
                        }
                        r
                    };
                    r.unwrap_or_else(|e| panic!("{ctx}: {e}"));
                }
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
                        // Menu actions (X4): activating the action is what a click or shortcut does. A
                        // radio member turning off is the consequence of its sibling turning on.
                        "activate" | "toggled" if h.backend.menu_action_name(o).is_some() => {
                            if sig == "toggled" && parts[3] == "[false]" {
                                continue;
                            }
                            let name = h.backend.menu_action_name(o).unwrap();
                            let (_, short) = name.split_once('.').unwrap();
                            let radio = !short.starts_with('a') || short.starts_with("g");
                            let target = radio.then(|| format!("a{o}").to_variant());
                            h.backend.menu_group(o).unwrap().activate_action(short, target.as_ref());
                        }
                        "clicked" => h.widget(o).downcast::<gtk::Button>().unwrap().emit_clicked(),
                        "changed" => {
                            let Val::Str(s) = parse_val(parts[3].trim_start_matches('[').trim_end_matches(']')) else { panic!() };
                            match h.obj(o).downcast::<gtk::TextBuffer>() {
                                // A text buffer (ABI v2.5 construct): its text changes natively.
                                Ok(buf) => buf.set_text(&s),
                                Err(_) => h.widget(o).dynamic_cast::<gtk::Editable>().unwrap().set_text(&s),
                            }
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
        let log = std::fs::read_to_string(e.path().join("expected.oplog")).unwrap();
        let pkg = std::fs::read_to_string(e.path().join("package.json")).unwrap_or_default();
        let toolkit = serde_json::from_str::<serde_json::Value>(&pkg).ok().and_then(|v| v["svelters"]["toolkit"].as_str().map(str::to_string));
        if let Some(tk) = toolkit.filter(|tk| tk != "gtk") {
            // AppKit / WinUI fixtures use their own tags (stack-view, text-block); examples/counter-mac
            // and examples/counter-win run them.
            eprintln!("{}: skipped ({tk} fixture)", e.file_name().to_string_lossy());
            continue;
        }
        if e.file_name().to_string_lossy().ends_with("-qt") {
            // Qt twins use Qt tags (push-button, box-layout); examples/counter runs them on Qt.
            eprintln!("{}: skipped (Qt fixture)", e.file_name().to_string_lossy());
            continue;
        }
        if log.lines().any(|l| l.starts_with("sync ")) {
            // Sync results (close vetoes, handled keys) change what the toolkit does next, which a
            // flat replay cannot follow; `tests/sync.rs` and `tests/host_sync.rs` cover them natively.
            eprintln!("{}: skipped (sync handlers)", e.file_name().to_string_lossy());
            continue;
        }
        if log.lines().any(|l| l.starts_with("create virtual-list ")) {
            // Model-view fixtures splice list arguments and bind rows from toolkit callbacks; the
            // flat replay has neither. `tests/vlist.rs` and `tests/lists_e2e.rs` run them natively.
            eprintln!("{}: skipped (virtual-list)", e.file_name().to_string_lossy());
            continue;
        }
        replay(&e.file_name().to_string_lossy(), log);
    }
}
