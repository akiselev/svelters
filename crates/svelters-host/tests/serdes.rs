//! Host conversion tests: type-directed validation, attribute-string coercion, id ops, borrowed
//! strings and boxed values, against a Rust backend that publishes descriptors.
//!
//! The backend (`Typed`) stores every written value; the JS bundles in `tests/js/serdes_*.js`
//! catch what the host throws and report through `console.log`, which these tests assert on.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use svelters_abi::*;
use svelters_host::{Host, HostConfig};

const ENUM_T: u32 = SVL_T_FIRST_BACKEND + 1;
const FLAGS_T: u32 = SVL_T_FIRST_BACKEND + 2;
const RECT_T: u32 = SVL_T_FIRST_BACKEND + 3;
const ICON_T: u32 = SVL_T_FIRST_BACKEND + 4;
const LIST_T: u32 = SVL_T_FIRST_BACKEND + 5;
const ERR_T: u32 = SVL_T_FIRST_BACKEND + 6;
const CB_CALL_T: u32 = SVL_T_FIRST_BACKEND + 7;
const CB_NOTIFIED_T: u32 = SVL_T_FIRST_BACKEND + 8;
const CB_ASYNC_T: u32 = SVL_T_FIRST_BACKEND + 9;
const CB_FOREVER_T: u32 = SVL_T_FIRST_BACKEND + 10;

const METHODS: &[&str] = &[
    "setBoth", "bounds", "fail", "raw", "gerror", "visit", "subscribe", "fire", "unsubscribe", "once", "forever", "fireOld", "fireOnce",
    "slow", "complete", "completeError", "slowFail", "slowOf", "measure", "bump", "origin", "badOuts",
];

#[derive(Default)]
struct State {
    props: HashMap<u32, Value>,
    /// `(prop, flags)` of every string seen, to check the encoding the host chose.
    str_flags: Vec<(u32, u16)>,
    destroyed: Vec<(svl_handle, u64)>,
    checkpoints: u64,
    next_boxed: u64,
    next_handle: u64,
    calls: Vec<(String, Vec<Value>)>,
}

struct Typed {
    st: Rc<RefCell<State>>,
    utf16: bool,
    post: Rc<RefCell<Vec<(svl_id, Vec<Value>)>>>,
    quit: Cell<bool>,
    /// The host callbacks, kept from `run` so that `call` can invoke callbacks synchronously.
    host: Cell<*const svl_host>,
    subscribed: Cell<u64>,
    old: Cell<u64>,
    once: Cell<u64>,
    /// Completion callbacks of started async operations.
    pending: RefCell<Vec<u64>>,
}

impl Typed {
    fn host(&self) -> HostRef<'static> {
        // SAFETY: set in `run`, which outlives every call made from the events it delivers.
        unsafe { HostRef::from_raw(self.host.get()) }
    }
    fn invoke(&self, id: u64, args: &[Value]) -> StrResult<Value> {
        self.host().invoke_callback(id, args).map_err(|e| e.message)
    }
}

const PROP_NAMES: &[&str] = &[
    "spacing", "label", "orientation", "align", "geometry", "color", "data", "icon", "tags", "ratio", "enabled", "loose",
    "readonly", "when", "meta",
];

fn prop_slot(id: u32) -> Option<SlotDesc> {
    Some(match id {
        1 => SlotDesc::new(SVL_I64).range(-1.0, 1000.0),
        2 => SlotDesc::new(SVL_STR).flags(SVL_D_NULLABLE),
        3 => SlotDesc::new(SVL_ENUM).typed(ENUM_T),
        4 => SlotDesc::new(SVL_FLAGS).typed(FLAGS_T),
        5 => SlotDesc::new(SVL_RECORD).typed(RECT_T),
        6 => SlotDesc::new(SVL_RECORD).typed(SVL_T_COLOR).flags(SVL_D_STRING_FORM),
        7 => SlotDesc::new(SVL_BYTES).flags(SVL_D_NULLABLE),
        8 => SlotDesc::new(SVL_BOXED).typed(ICON_T).flags(SVL_D_NULLABLE | SVL_D_STRING_FORM),
        9 => SlotDesc::new(SVL_LIST).typed(LIST_T),
        10 => SlotDesc::new(SVL_F64).range(0.0, 1.0),
        11 => SlotDesc::new(SVL_BOOL),
        13 => SlotDesc::new(SVL_I64),
        14 => SlotDesc::new(SVL_F64).typed(SVL_T_DATETIME),
        15 => SlotDesc::new(SVL_NULL).typed(SVL_T_VARIANT).flags(SVL_D_NULLABLE),
        _ => return None, // 12 "loose": no descriptor, converted dynamically
    })
}

impl Backend for Typed {
    fn resolve(&self, what: NameKind, kind: svl_id, name: &str) -> svl_id {
        match what {
            NameKind::Kind => (name == "thing") as u32,
            NameKind::Prop if kind == 1 => PROP_NAMES.iter().position(|n| *n == name).map_or(0, |i| i as u32 + 1),
            NameKind::Method if kind == 1 => METHODS.iter().position(|n| *n == name).map_or(0, |i| i as u32 + 1),
            NameKind::Signal if kind == 1 => (name == "changed") as u32,
            _ => 0,
        }
    }
    fn create(&self, kind: svl_id) -> StrResult<svl_handle> {
        if kind != 1 {
            return Err("bad kind".to_string());
        }
        let mut st = self.st.borrow_mut();
        st.next_handle += 1;
        Ok(st.next_handle)
    }
    fn insert(&self, _: svl_handle, _: svl_handle, _: Option<svl_handle>, _: svl_id) -> StrResult<()> {
        Ok(())
    }
    fn remove(&self, _: svl_handle, _: svl_handle) -> StrResult<()> {
        Ok(())
    }
    fn destroy(&self, h: svl_handle) {
        let mut st = self.st.borrow_mut();
        let at = st.checkpoints;
        st.destroyed.push((h, at));
    }
    fn set_prop(&self, _: svl_handle, prop: svl_id, v: &Value) -> StrResult<()> {
        self.st.borrow_mut().props.insert(prop, v.clone());
        Ok(())
    }
    fn set_prop_ref(&self, h: svl_handle, prop: svl_id, v: ValueRef<'_>) -> StrResult<()> {
        if v.tag() == SVL_STR {
            self.st.borrow_mut().str_flags.push((prop, v.flags()));
        }
        if prop == 13 {
            return Err("readonly property".into());
        }
        self.set_prop(h, prop, &v.to_value())
    }
    fn get_prop(&self, _: svl_handle, prop: svl_id) -> StrResult<Value> {
        let mut st = self.st.borrow_mut();
        if prop == 8 {
            st.next_boxed += 1;
            return Ok(Value::Boxed { ty: ICON_T, handle: 1000 + st.next_boxed });
        }
        let v = st.props.get(&prop).cloned().unwrap_or(Value::Null);
        // A UTF-16 backend answers strings in UTF-16 (`SVL_CAP_UTF16_OUT`), as the Qt backend does.
        Ok(match v {
            Value::Str(s) if self.utf16 => Value::Str16(s.encode_utf16().collect()),
            v => v,
        })
    }
    fn call(&self, _: svl_handle, method: svl_id, args: &[Value]) -> StrResult<Value> {
        let name = METHODS[method as usize - 1];
        self.st.borrow_mut().calls.push((name.to_string(), args.to_vec()));
        let cb_id = |i: usize| match args.get(i) {
            Some(Value::Callback { id, .. }) => Ok(*id),
            other => Err(format!("expected a callback, got {other:?}")),
        };
        match method {
            6 => {
                let id = cb_id(0)?;
                let mut out = Vec::new();
                for n in [1, 2] {
                    out.push(self.invoke(id, &[Value::I64(n)])?);
                }
                Ok(Value::List(out))
            }
            7 => {
                self.old.set(self.subscribed.get());
                self.subscribed.set(cb_id(0)?);
                Ok(Value::Null)
            }
            8 => self.invoke(self.subscribed.get(), &[Value::I64(7)]),
            9 => {
                self.host().release_callback(self.subscribed.get());
                Ok(Value::Null)
            }
            10 => {
                self.once.set(cb_id(0)?);
                Ok(Value::Null)
            }
            11 => Ok(Value::Null),
            12 => self.invoke(self.old.get(), &[Value::I64(1)]),
            13 => self.invoke(self.once.get(), &[Value::I64(1)]),
            // async: remember the completion callback and return at once
            14 => {
                self.pending.borrow_mut().push(cb_id(args.len() - 1)?);
                Ok(Value::Null)
            }
            18 => {
                if !matches!(args.first(), Some(Value::Str(_))) {
                    return Err("slowOf: expected the label".into());
                }
                self.pending.borrow_mut().push(cb_id(args.len() - 1)?);
                Ok(Value::Null)
            }
            15 => {
                let id = self.pending.borrow_mut().pop().ok_or("nothing pending")?;
                self.invoke(id, &[args.first().cloned().unwrap_or(Value::Null)])
            }
            16 => {
                let id = self.pending.borrow_mut().pop().ok_or("nothing pending")?;
                self.invoke(
                    id,
                    &[Value::Error(Box::new(NativeError {
                        domain: "g-io-error-quark".into(),
                        code: Value::Enum { ty: ERR_T, value: 0 },
                        message: "it failed".into(),
                    }))],
                )
            }
            17 => Err("cannot start".into()),
            // out params: the backend sees only the JS-supplied arguments and answers [ret, outs...]
            19 => {
                let Some(Value::F64(scale)) = args.first() else { return Err(format!("measure: bad args {args:?}")) };
                if args.len() != 1 {
                    return Err(format!("measure: out parameters must not be sent, got {args:?}"));
                }
                Ok(Value::List(vec![Value::Bool(true), Value::I64((*scale * 10.0) as i64), Value::I64((*scale * 20.0) as i64)]))
            }
            20 => match args {
                [Value::I64(a), Value::Str(b)] => Ok(Value::List(vec![Value::Null, Value::I64(a + 1), Value::Str(format!("{b}!"))])),
                _ => Err(format!("bump: bad args {args:?}")),
            },
            21 => Ok(Value::List(vec![Value::Null, Value::Record { ty: RECT_T, fields: vec![Value::F64(1.0), Value::F64(2.0), Value::F64(3.0), Value::F64(4.0)] }])),
            22 => Ok(Value::I64(1)),
            1 => Ok(Value::Bool(true)),
            2 => Ok(Value::Record { ty: RECT_T, fields: vec![Value::F64(1.0), Value::F64(2.0), Value::F64(3.0), Value::F64(4.0)] }),
            3 => Err("boom".into()),
            5 => Ok(Value::Error(Box::new(NativeError {
                domain: "g-io-error-quark".into(),
                code: Value::Enum { ty: ERR_T, value: 1 },
                message: "No such file or directory".into(),
            }))),
            _ => Ok(Value::List(args.to_vec())),
        }
    }
    fn listen(&self, _: svl_handle, _: svl_id, _: bool) -> StrResult<()> {
        Ok(())
    }
    fn run(&self, host: &HostRef<'_>) -> StrResult<()> {
        self.host.set(host.as_raw());
        for (sig, args) in self.post.borrow_mut().drain(..) {
            host.post_event(1, sig, &args);
        }
        host.checkpoint();
        Ok(())
    }
    fn waker(&self) -> Waker {
        Arc::new(|| {})
    }
    fn cancel(&self, callback: u64) -> bool {
        let known = self.pending.borrow().contains(&callback);
        if known {
            self.pending.borrow_mut().retain(|c| *c != callback);
            let e = NativeError {
                domain: "g-io-error-quark".into(),
                code: Value::Enum { ty: ERR_T, value: 2 },
                message: "Operation was cancelled".into(),
            };
            let _ = self.invoke(callback, &[Value::Error(Box::new(e))]);
        }
        known
    }
    fn quit(&self) {
        self.quit.set(true);
    }
    fn schedule_wake(&self, _: i64) {}
    fn handle_info(&self, _: svl_handle) -> StrResult<svl_handle_info> {
        Ok(svl_handle_info { kind: 1, lifetime: 1, flags: 0 })
    }
    fn caps(&self) -> u32 {
        if self.utf16 { SVL_CAP_UTF16_IN } else { 0 }
    }
    fn describe(&self, what: NameKind, kind: svl_id, member: svl_id) -> Option<MemberDesc> {
        if kind != 1 {
            return None;
        }
        match what {
            NameKind::Prop => {
                let slot = prop_slot(member)?;
                let name = PROP_NAMES[member as usize - 1].to_string();
                let slot = slot.named(&name);
                Some(MemberDesc {
                    name,
                    params: if member == 13 { vec![] } else { vec![slot.clone()] },
                    ret: slot,
                    flags: 0,
                })
            }
            NameKind::Method => match member {
                1 => Some(MemberDesc {
                    name: "setBoth".into(),
                    params: vec![SlotDesc::new(SVL_I64).named("spacing").range(-1.0, 1000.0), SlotDesc::new(SVL_STR).named("label").flags(SVL_D_OPTIONAL)],
                    ret: SlotDesc::new(SVL_BOOL),
                    flags: 0,
                }),
                2 => Some(MemberDesc { name: "bounds".into(), params: vec![], ret: SlotDesc::new(SVL_RECORD).typed(RECT_T), flags: 0 }),
                14 | 17 | 18 => {
                    let mut params = vec![];
                    if member == 18 {
                        params.push(SlotDesc::new(SVL_STR).named("label"));
                    }
                    params.push(SlotDesc::new(SVL_CALLBACK).typed(CB_ASYNC_T).named("done"));
                    Some(MemberDesc { name: METHODS[member as usize - 1].into(), params, ret: SlotDesc::new(SVL_NULL), flags: SVL_M_ASYNC })
                }
                19 => Some(MemberDesc {
                    name: "measure".into(),
                    params: vec![
                        SlotDesc::new(SVL_F64).named("scale"),
                        SlotDesc::new(SVL_I64).named("width").flags(SVL_D_OUT),
                        SlotDesc::new(SVL_I64).named("height").flags(SVL_D_OUT),
                    ],
                    ret: SlotDesc::new(SVL_BOOL),
                    flags: 0,
                }),
                20 => Some(MemberDesc {
                    name: "bump".into(),
                    params: vec![SlotDesc::new(SVL_I64).named("count").flags(SVL_D_INOUT), SlotDesc::new(SVL_STR).named("text").flags(SVL_D_INOUT)],
                    ret: SlotDesc::new(SVL_NULL),
                    flags: 0,
                }),
                21 => Some(MemberDesc {
                    name: "origin".into(),
                    params: vec![SlotDesc::new(SVL_RECORD).typed(RECT_T).named("area").flags(SVL_D_OUT)],
                    ret: SlotDesc::new(SVL_NULL),
                    flags: 0,
                }),
                22 => Some(MemberDesc {
                    name: "badOuts".into(),
                    params: vec![SlotDesc::new(SVL_I64).named("n").flags(SVL_D_OUT)],
                    ret: SlotDesc::new(SVL_I64),
                    flags: 0,
                }),
                6 | 7 | 10 | 11 => {
                    let ty = match member {
                        6 => CB_CALL_T,
                        7 => CB_NOTIFIED_T,
                        10 => CB_ASYNC_T,
                        _ => CB_FOREVER_T,
                    };
                    Some(MemberDesc {
                        name: METHODS[member as usize - 1].into(),
                        params: vec![SlotDesc::new(SVL_CALLBACK).typed(ty).named("fn")],
                        ret: SlotDesc::new(SVL_NULL),
                        flags: 0,
                    })
                }
                _ => None,
            },
            _ => None,
        }
    }
    fn describe_type(&self, ty: svl_id) -> Option<TypeDesc> {
        let none = SlotDesc::new(SVL_NULL);
        let member = |n: &str, v: i64| EnumMember { name: n.into(), value: v };
        Some(match ty {
            ENUM_T => TypeDesc { name: "Orientation".into(), kind: SVL_ENUM, scope: 0, members: vec![member("horizontal", 0), member("vertical", 1)], fields: vec![], ret: none },
            FLAGS_T => TypeDesc {
                name: "Align".into(),
                kind: SVL_FLAGS, scope: 0,
                members: vec![member("left", 1), member("right", 2), member("top", 4), member("vcenter", 8)],
                fields: vec![],
                ret: none,
            },
            RECT_T => TypeDesc {
                name: "Rect".into(),
                kind: SVL_RECORD, scope: 0,
                members: vec![],
                fields: ["x", "y", "width", "height"].iter().map(|n| SlotDesc::new(SVL_F64).named(n)).collect(),
                ret: none,
            },
            SVL_T_COLOR => TypeDesc {
                name: "Color".into(),
                kind: SVL_RECORD, scope: 0,
                members: vec![],
                fields: ["red", "green", "blue", "alpha"].iter().map(|n| SlotDesc::new(SVL_F64).named(n).range(0.0, 1.0)).collect(),
                ret: none,
            },
            ICON_T => TypeDesc { name: "Icon".into(), kind: SVL_BOXED, scope: 0, members: vec![], fields: vec![], ret: none },
            CB_CALL_T | CB_NOTIFIED_T | CB_ASYNC_T | CB_FOREVER_T => TypeDesc {
                name: "Callback".into(),
                kind: SVL_CALLBACK,
                scope: match ty {
                    CB_CALL_T => SVL_CB_CALL,
                    CB_NOTIFIED_T => SVL_CB_NOTIFIED,
                    CB_ASYNC_T => SVL_CB_ASYNC,
                    _ => SVL_CB_FOREVER,
                },
                members: vec![],
                fields: vec![SlotDesc::new(SVL_I64).named("n")],
                ret: if ty == CB_CALL_T { SlotDesc::new(SVL_I64) } else { SlotDesc::new(SVL_NULL) },
            },
            ERR_T => TypeDesc {
                name: "Gio.IOErrorEnum".into(),
                kind: SVL_ENUM, scope: 0,
                members: vec![member("failed", 0), member("not-found", 1), member("cancelled", 2)],
                fields: vec![],
                ret: none,
            },
            LIST_T => TypeDesc { name: "string[]".into(), kind: SVL_LIST, scope: 0, members: vec![], fields: vec![SlotDesc::new(SVL_STR)], ret: none },
            _ => return None,
        })
    }
}

struct Rig {
    st: Rc<RefCell<State>>,
    post: Rc<RefCell<Vec<(svl_id, Vec<Value>)>>>,
    vt: BackendVtable,
}

fn rig(utf16: bool) -> Rig {
    let st = Rc::new(RefCell::new(State::default()));
    let post = Rc::new(RefCell::new(Vec::new()));
    let vt = BackendVtable::new(Typed { st: st.clone(), utf16, post: post.clone(), quit: Cell::new(false), host: Cell::new(std::ptr::null()), subscribed: Cell::new(0), old: Cell::new(0), once: Cell::new(0), pending: RefCell::default() });
    Rig { st, post, vt }
}

fn bundle(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/js").join(name)
}

fn quiet() -> HostConfig {
    HostConfig { echo_console: false, echo_errors: false, ..HostConfig::default() }
}

fn run(rig: &Rig, js: &str) -> (Vec<String>, Vec<String>) {
    let host = Host::new(rig.vt.backend_ref(), quiet()).unwrap();
    host.load_bundle(bundle(js)).unwrap();
    host.drain_jobs();
    let errs = host.take_errors().into_iter().map(|e| e.to_string()).collect();
    (host.take_console().into_iter().map(|l| l.text).collect(), errs)
}

fn lines(console: &[String]) -> Vec<&str> {
    console.iter().map(|s| s.as_str()).collect()
}

#[test]
fn validation_errors_name_the_element_member_and_value() {
    let r = rig(false);
    let (out, errs) = run(&r, "serdes_errors.js");
    assert!(errs.is_empty(), "{errs:?}");
    let want = [
        r#"TypeError <thing>.spacing: expected integer, got "8px" (string)"#,
        "RangeError <thing>.spacing: 3.5 is not an integer",
        "RangeError <thing>.spacing: 5000 is out of range (-1..1000)",
        "TypeError <thing>.spacing: expected integer, got null",
        "TypeError <thing>.spacing: expected integer, got undefined",
        "TypeError <thing>.label: expected string, got 8",
        "TypeError <thing>.orientation: expected one of 'horizontal' | 'vertical', got \"hidden\" (string)",
        "TypeError <thing>.align: expected an array of 'left' | 'right' | 'top' | 'vcenter', got \"sideways\" (string)",
        "TypeError <thing>.geometry: missing field 'height' in Rect",
        "TypeError <thing>.geometry.width: expected number, got \"wide\" (string)",
        "TypeError <thing>.geometry: expected Rect, got 5",
        "TypeError <thing>.enabled: expected boolean, got \"yes\" (string)",
        "TypeError <thing>.data: expected ArrayBuffer or typed array, got \"bytes\" (string)",
        "TypeError <thing>.tags[1]: expected string, got 2",
        "RangeError <thing>.color.red: 2 is out of range",
        "TypeError <thing>.setBoth: expected 1 to 2 argument(s), got 3",
        "TypeError <thing>.setBoth(spacing): expected integer, got \"x\" (string)",
        "TypeError <thing>.when: expected Date, got \"yesterday\" (string)",
        "TypeError <thing>.readonly: property is read-only",
        "Error unknown Prop 'nosuch'",
        "Error stale or foreign handle 999",
        "TypeError <thing>.loose: undefined is not a valid host value",
        "Error boom",
    ];
    assert_eq!(lines(&out), want);
}

#[test]
fn native_errors_become_native_error_objects() {
    let r = rig(false);
    let (out, errs) = run(&r, "serdes_native_error.js");
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(
        lines(&out),
        [
            "NativeError|g-io-error-quark|not-found|No such file or directory|true|true",
            "Error|boom|false",
        ]
    );
}

#[test]
fn attribute_strings_are_coerced_from_descriptors() {
    let r = rig(false);
    let (out, errs) = run(&r, "serdes_coerce.js");
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(lines(&out), ["done"]);
    let st = r.st.borrow();
    assert_eq!(st.props[&1], Value::I64(8), "\"8\" for an integer slot");
    assert_eq!(st.props[&10], Value::F64(0.5), "\"0.5\" for a number slot");
    assert_eq!(st.props[&11], Value::Bool(true), "\"\" is a bare boolean attribute");
    assert_eq!(st.props[&3], Value::Enum { ty: ENUM_T, value: 1 }, "enum names become native values");
    assert_eq!(st.props[&4], Value::Flags { ty: FLAGS_T, mask: 1 | 8 }, "\"left vcenter\" for flags");
    assert_eq!(st.props[&2], Value::Str("8".into()), "a string slot keeps its text");
}

#[test]
fn typed_variants_cross_as_records() {
    let r = rig(false);
    let (out, errs) = run(&r, "serdes_variant.js");
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(lines(&out), ["empty signature: TypeError", "class: i true"], "{out:?}");
    let typed = |sig: &str, v: Value| Value::Record { ty: SVL_T_TYPED_VARIANT, fields: vec![Value::Str(sig.into()), v] };
    assert_eq!(
        r.st.borrow().props[&15],
        typed(
            "a{sv}",
            Value::Map(vec![
                ("n".into(), typed("(is)", Value::List(vec![Value::I64(1), Value::Str("x".into())]))),
                ("list".into(), Value::List(vec![Value::I64(1), Value::I64(2)])),
            ])
        )
    );
}

#[test]
fn values_round_trip_through_the_host() {
    let r = rig(false);
    let (out, errs) = run(&r, "serdes_values.js");
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(lines(&out), ["ok"], "{out:?}");
    let st = r.st.borrow();
    assert_eq!(st.props[&5], Value::Record { ty: RECT_T, fields: vec![Value::F64(1.0), Value::F64(2.0), Value::F64(30.0), Value::F64(40.0)] });
    assert_eq!(st.props[&7], Value::Bytes(vec![1, 2, 3, 4]));
    assert_eq!(st.props[&9], Value::List(vec![Value::Str("a".into()), Value::Str("b\u{e9}".into())]));
    assert_eq!(st.props[&14], Value::DateTime(1_700_000_000_000.0));
    assert_eq!(
        st.props[&15],
        Value::Map(vec![
            ("a".into(), Value::I64(1)),
            (
                "b".into(),
                Value::List(vec![
                    Value::Bool(true),
                    Value::Str("x".into()),
                    Value::Map(vec![("c".into(), Value::Null), ("d".into(), Value::F64(0.5))]),
                    Value::DateTime(1_700_000_000_000.0),
                    Value::Bytes(vec![1, 2]),
                ]),
            ),
        ]),
        "a variant slot takes JSON-like trees: objects are maps"
    );
    assert_eq!(st.props[&6], Value::Str("#ff8800".into()), "string forms pass through for the backend to parse");
    assert_eq!(st.props[&12], Value::List(vec![Value::I64(1), Value::Str("x".into())]), "no descriptor: dynamic conversion");
}

#[test]
fn ids_and_names_are_interchangeable_and_cached() {
    let r = rig(false);
    let (out, errs) = run(&r, "serdes_ids.js");
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(lines(&out), ["ok"], "{out:?}");
    assert_eq!(r.st.borrow().props[&1], Value::I64(42));
}

#[test]
fn wide_strings_go_to_utf16_backends_as_utf16() {
    let r = rig(true);
    let (out, errs) = run(&r, "serdes_strings.js");
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(lines(&out), ["ok"], "{out:?}");
    let st = r.st.borrow();
    assert!(st.str_flags.iter().all(|(_, fl)| fl & SVL_F_UTF16 != 0), "a UTF-16 backend is always handed UTF-16: {:?}", st.str_flags);
    assert_eq!(st.props[&2], Value::Str("caf\u{e9} \u{2713} \u{1F600}".into()));

    let r = rig(false);
    let (out, errs) = run(&r, "serdes_strings.js");
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(lines(&out), ["ok"]);
    let st = r.st.borrow();
    assert!(st.str_flags.iter().all(|(_, fl)| fl & SVL_F_UTF16 == 0), "{:?}", st.str_flags);
    assert!(st.str_flags.iter().any(|(_, fl)| fl & SVL_F_NUL_TERM != 0), "UTF-8 strings come NUL terminated: {:?}", st.str_flags);
    assert_eq!(st.props[&2], Value::Str("caf\u{e9} \u{2713} \u{1F600}".into()));
}

#[test]
fn boxed_values_dispose_and_finalize_at_checkpoints() {
    let r = rig(false);
    let host = Host::new(r.vt.backend_ref(), quiet()).unwrap();
    host.load_bundle(bundle("serdes_boxed.js")).unwrap();
    host.drain_jobs();
    assert!(host.take_errors().is_empty());
    let console: Vec<String> = host.take_console().into_iter().map(|l| l.text).collect();
    assert_eq!(lines(&console), ["disposed", "double-dispose-ok", "stale-rejected"]);
    // dispose() released handle 1001 immediately, exactly once; handle 1002 is still alive.
    assert_eq!(r.st.borrow().destroyed, vec![(1001, 0)]);
    assert_eq!(host.live_boxed(), 2, "1002 (kept) and 1003 (dropped, not yet collected)");

    // GC: the wrapper for 1003 was dropped without dispose(). Collection only queues the release.
    host.collect_garbage();
    host.drain_jobs();
    assert_eq!(r.st.borrow().destroyed, vec![(1001, 0)], "the finalizer never calls the backend");
    // The release is applied at the next checkpoint.
    r.st.borrow_mut().checkpoints = 7;
    host.checkpoint();
    let destroyed = r.st.borrow().destroyed.clone();
    assert_eq!(destroyed, vec![(1001, 0), (1003, 7)], "released by the checkpoint, once");
    host.checkpoint();
    assert_eq!(r.st.borrow().destroyed.len(), 2);
    assert_eq!(host.live_boxed(), 1);
}

#[test]
fn engine_provides_the_primitives_boxed_values_rely_on() {
    let r = rig(false);
    let host = Host::new(r.vt.backend_ref(), quiet()).unwrap();
    host.load_bundle(bundle("probe_fr.js")).unwrap();
    host.drain_jobs();
    host.collect_garbage();
    host.drain_jobs();
    let out: Vec<String> = host.take_console().into_iter().map(|l| l.text).collect();
    // FinalizationRegistry carries the GC release path.
    assert!(out[0].starts_with("function function"), "{out:?}");
    // An unregister token is held strongly by QuickJS: a target registered with itself as the token is
    // never finalized. The prelude therefore registers boxed wrappers without a token.
    assert_eq!(out.len(), 2, "{out:?}");
    assert_eq!(out[1], "finalized held-2");
}

/// Runs `js` inside an event dispatched from the backend's loop, where the host callbacks exist.
fn run_in_loop(r: &Rig, js: &str) -> (Vec<String>, Vec<String>, usize) {
    r.post.borrow_mut().push((1, vec![]));
    let host = Host::new(r.vt.backend_ref(), quiet()).unwrap();
    host.load_bundle(bundle(js)).unwrap();
    host.run().unwrap();
    host.drain_jobs();
    host.checkpoint();
    let live = host.live_callbacks();
    let errs = host.take_errors().into_iter().map(|e| e.message).collect();
    (host.take_console().into_iter().map(|l| l.text).collect(), errs, live)
}

#[test]
fn callbacks_follow_their_scope_and_owner() {
    let r = rig(false);
    let (out, errs, live) = run_in_loop(&r, "serdes_callbacks.js");
    assert_eq!(
        lines(&out),
        [
            "visit [10,20]",
            "fire ok 7",
            "native-release then fire: callback was released or its owner is gone",
            "re-register, old id: callback was released or its owner is gone",
            "once ok",
            "once twice: callback was released or its owner is gone",
            "forever: TypeError <thing>.forever(fn): callbacks with scope 'forever' are refused: they could never be released",
            "owner gone: callback was released or its owner is gone",
            "throwing: callback threw: boom from js",
            "reentrant: callback threw: tree operations (create, insert, remove, destroy) are not allowed inside a native callback",
            "bad arg: TypeError <thing>.visit(fn): expected function, got 5",
            "done",
        ],
        "{out:?} {errs:?}"
    );
    // The throwing callbacks were reported, not swallowed.
    assert!(errs.iter().any(|e| e.contains("boom from js")), "{errs:?}");
    assert_eq!(live, 3, "the three notified callbacks nobody released stay registered; call, async and owner-bound ones are gone");
}

#[test]
fn async_methods_return_promises_settled_by_the_backend() {
    let r = rig(false);
    let (out, errs, live) = run_in_loop(&r, "serdes_async.js");
    assert_eq!(
        lines(&out),
        [
            "late completion error: callback was released or its owner is gone",
            "arity: TypeError <thing>.slowOf: expected 1 argument(s), got 0",
            "sync part done",
            "done",
            "resolved payload",
            "rejected NativeError g-io-error-quark failed it failed",
            "slowOf resolved 42",
            "start failure rejected: svelters cannot start",
            "aborted NativeError g-io-error-quark cancelled Operation was cancelled",
            "pre-aborted NativeError g-io-error-quark cancelled Operation was cancelled",
            // the completion orphaned by its owner's death is rejected by the host (N1), never left pending
            "orphan rejected svelters disposed",
        ],
        "{out:?} {errs:?}"
    );
    assert!(errs.is_empty(), "rejections that are handled are not reported: {errs:?}");
    assert_eq!(live, 0, "every completion callback is released after its single use");
}

#[test]
fn out_and_inout_params_come_back_as_one_object() {
    let r = rig(false);
    let (out, errs) = run(&r, "serdes_outs.js");
    assert_eq!(
        lines(&out),
        [
            "measure {\"ret\":true,\"width\":25,\"height\":50}",
            "bump {\"count\":8,\"text\":\"hi!\"}",
            "origin {\"area\":{\"x\":1,\"y\":2,\"width\":3,\"height\":4}}",
            "arity TypeError <thing>.measure: expected 1 argument(s), got 3",
            "bad answer Error <thing>.badOuts: backend answered I64(1); expected a list of the return value and 1 out value(s)",
        ],
        "{out:?} {errs:?}"
    );
    assert!(errs.is_empty(), "{errs:?}");
}

#[test]
fn typed_events_decode_to_names_and_objects() {
    let r = rig(false);
    r.post.borrow_mut().push((
        1,
        vec![
            Value::Enum { ty: ENUM_T, value: 1 },
            Value::Flags { ty: FLAGS_T, mask: 1 | 4 | 64 },
            Value::Record { ty: RECT_T, fields: vec![Value::F64(1.0), Value::F64(2.0), Value::F64(3.0), Value::F64(4.0)] },
            Value::DateTime(86_400_000.0),
            Value::Bytes(vec![9, 8]),
        ],
    ));
    let host = Host::new(r.vt.backend_ref(), quiet()).unwrap();
    host.load_bundle(bundle("serdes_events.js")).unwrap();
    host.run().unwrap();
    host.drain_jobs();
    let errs: Vec<_> = host.take_errors().into_iter().map(|e| e.to_string()).collect();
    assert!(errs.is_empty(), "{errs:?}");
    let console: Vec<String> = host.take_console().into_iter().map(|l| l.text).collect();
    assert_eq!(
        lines(&console),
        [r#"["vertical",["left","top",64],{"x":1,"y":2,"width":3,"height":4},"1970-01-02T00:00:00.000Z",[9,8]]"#]
    );
}
