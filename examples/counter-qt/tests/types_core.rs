//! NT2 core: Qt non-GUI types through the backend's C ABI (plans/research/NON-GUI-TYPES.md, N2).
//! QObjects JS makes are SHARED (a parented one is only forgotten on destroy), value classes are VALUE
//! handles with kinds, methods and `equals`, unique objects and items are JS-owned until a container takes
//! them (PySide ownership facts), overloads dispatch at run time (decision Q4), the QMetaObject fallback
//! reaches members without facts, and a QFuture completes the host's one-shot callback.
//!
//! One nextest test per case (`harness = false`: Qt needs the main thread). Needs a display.

use counter_qt::hooks::{child_count, live_handles, pump, test_future, type_counts};
use std::cell::RefCell;
use std::rc::Rc;
use svelters_abi::*;

fn be() -> BackendRef<'static> {
    // SAFETY: the generated backend is a static vtable.
    unsafe { BackendRef::from_raw(counter_qt::backend()) }.expect("abi version")
}

fn kind(name: &str) -> svl_id {
    let k = be().resolve(NameKind::Kind, 0, name);
    assert_ne!(k, 0, "unknown kind {name}");
    k
}

fn mid(k: svl_id, name: &str) -> svl_id {
    let id = be().resolve(NameKind::Method, k, name);
    assert_ne!(id, 0, "unknown method {name}");
    id
}

fn pid(k: svl_id, name: &str) -> svl_id {
    let id = be().resolve(NameKind::Prop, k, name);
    assert_ne!(id, 0, "unknown prop {name}");
    id
}

fn s(x: &str) -> Value {
    Value::Str(x.into())
}

fn text(v: &Value) -> String {
    match v {
        Value::Str(s) => s.clone(),
        Value::Str16(u) => String::from_utf16_lossy(u),
        other => panic!("expected a string, got {other:?}"),
    }
}

fn handle(v: &Value) -> svl_handle {
    match v {
        Value::Handle(h) | Value::Boxed { handle: h, .. } => *h,
        other => panic!("expected a handle, got {other:?}"),
    }
}

/// `call` on a live handle by method name (the kind comes from handle_info, as the host does it).
fn call(h: svl_handle, m: &str, args: &[Value]) -> Result<Value, String> {
    let k = be().handle_info(h).expect("live handle").kind;
    be().call(h, mid(k, m), args).map_err(|e| e.message)
}

fn stat(k: &str, m: &str, args: &[Value]) -> Result<Value, String> {
    let kid = kind(k);
    be().call_static(kid, mid(kid, m), args).map_err(|e| e.message)
}

fn new(k: &str, args: &[Value]) -> Value {
    stat(k, "new", args).unwrap_or_else(|e| panic!("new {k}: {e}"))
}

fn chain(k: svl_id) -> Vec<String> {
    let mut out = Vec::new();
    let mut k = k;
    while k != 0 {
        let d = be().describe(NameKind::Kind, 0, k).expect("kind descriptor");
        out.push(d.name.clone());
        k = d.ret.ty;
    }
    out
}

#[derive(Default)]
struct Rec {
    events: Vec<(svl_handle, svl_id, Vec<Value>)>,
    gone: Vec<(svl_handle, u32)>,
    callbacks: Vec<(u64, Vec<Value>)>,
}

struct Host(Rc<RefCell<Rec>>);

impl HostCallbacks for Host {
    fn post_event(&self, h: svl_handle, sig: svl_id, args: &[Value]) {
        self.0.borrow_mut().events.push((h, sig, args.to_vec()));
    }
    fn checkpoint(&self) {}
    fn call_sync(&self, _: svl_handle, _: svl_id, _: &[Value]) -> Result<Value, SyncError> {
        Err(SyncError::Failed("no sync handlers".into()))
    }
    fn on_destroyed(&self, h: svl_handle, reason: u32) {
        self.0.borrow_mut().gone.push((h, reason));
    }
    fn frame(&self) {}
    fn invoke_callback(&self, callback: u64, args: &[Value]) -> StrResult<Value> {
        self.0.borrow_mut().callbacks.push((callback, args.to_vec()));
        Ok(Value::Null)
    }
}

fn install_host() -> Rc<RefCell<Rec>> {
    let rec = Rc::new(RefCell::new(Rec::default()));
    let vt = Box::leak(Box::new(HostVtable::new(Host(rec.clone()))));
    // SAFETY: leaked above, so it outlives every call the backend can make.
    unsafe { counter_qt::set_host(vt.as_raw()) };
    rec
}

fn timer_is_a_shared_object_with_props_methods_and_signals() {
    let rec = install_host();
    let k = kind("QTimer");
    assert_eq!(chain(k), ["QTimer", "QObject"]);
    let base = live_handles();
    let v = be().construct(k, &[pid(k, "interval"), pid(k, "singleShot")], &[Value::I64(20), Value::Bool(true)]).unwrap();
    let t = handle(&v);
    let info = be().handle_info(t).unwrap();
    assert_eq!((info.lifetime, info.kind), (SVL_LIFE_SHARED, k));
    assert_eq!(be().get_prop(t, pid(k, "interval")).unwrap(), Value::I64(20));
    assert_eq!(be().get_prop(t, pid(k, "single-shot")).unwrap(), Value::Bool(true), "kebab names resolve too");
    be().set_prop(t, pid(k, "interval"), &Value::I64(10)).unwrap();
    assert_eq!(call(t, "isSingleShot", &[]).unwrap_or_else(|e| panic!("{e}")), Value::Bool(true));
    // A signal through the metaobject relay.
    let timeout = be().resolve(NameKind::Signal, k, "timeout");
    assert_ne!(timeout, 0);
    be().listen(t, timeout, true).unwrap();
    call(t, "start", &[]).unwrap();
    assert_eq!(be().get_prop(t, pid(k, "active")).unwrap(), Value::Bool(true));
    pump(80);
    assert!(rec.borrow().events.iter().any(|e| e.0 == t && e.1 == timeout), "timeout reached the host: {:?}", rec.borrow().events.len());
    // Overloads: start(int msec) next to start().
    call(t, "start", &[Value::I64(1000)]).unwrap();
    assert_eq!(be().get_prop(t, pid(k, "interval")).unwrap(), Value::I64(1000));
    let err = call(t, "start", &[s("soon")]).unwrap_err();
    assert!(err.contains("no overload takes these arguments"), "{err}");
    // A parentless SHARED object dies with its handle.
    be().destroy(t);
    pump(20);
    assert_eq!(live_handles(), base);
}

fn a_parented_object_is_only_forgotten() {
    install_host();
    let parent = handle(&new("QTimer", &[]));
    let child = handle(&new("QTimer", &[Value::Handle(parent)]));
    assert_eq!(be().handle_info(child).unwrap().lifetime, SVL_LIFE_SHARED);
    assert_eq!(child_count(parent), 1);
    be().destroy(child);
    pump(20);
    assert_eq!(child_count(parent), 1, "the QObject parent owns it: destroy only drops our handle");
    be().destroy(parent);
    pump(20);
}

fn url_is_a_value_with_a_kind_methods_statics_and_equality() {
    install_host();
    let (v0, _, _) = type_counts();
    let k = kind("QUrl");
    let u = new("QUrl", &[s("https://example.com/a/b?x=1")]);
    let Value::Boxed { handle: h, .. } = u else { panic!("a VALUE handle: {u:?}") };
    let info = be().handle_info(h).unwrap();
    assert_eq!((info.lifetime, info.kind), (SVL_LIFE_VALUE, k));
    assert_eq!(text(&call(h, "host", &[]).unwrap()), "example.com");
    assert_eq!(text(&call(h, "path", &[]).unwrap()), "/a/b");
    assert_eq!(call(h, "isValid", &[]).unwrap(), Value::Bool(true));
    assert_eq!(text(&call(h, "toString", &[]).unwrap()), "https://example.com/a/b?x=1");
    // Mutators are hidden (decision Q3): no `setHost`.
    assert_eq!(be().resolve(NameKind::Method, k, "setHost"), 0);
    let local = handle(&stat("QUrl", "fromLocalFile", &[s("/tmp/x.txt")]).unwrap());
    assert_eq!(call(local, "isLocalFile", &[]).unwrap(), Value::Bool(true));
    assert_eq!(text(&call(local, "toLocalFile", &[]).unwrap()), "/tmp/x.txt");
    // QMetaType equality: a new handle per crossing, equal values.
    let again = handle(&new("QUrl", &[s("https://example.com/a/b?x=1")]));
    assert_ne!(again, h);
    let ty = match u { Value::Boxed { ty, .. } => ty, _ => 0 };
    assert_eq!(call(h, "equals", &[Value::Boxed { ty, handle: again }]).unwrap(), Value::Bool(true));
    assert_eq!(call(h, "equals", &[Value::Boxed { ty, handle: local }]).unwrap(), Value::Bool(false));
    for x in [h, local, again] {
        be().destroy(x);
    }
    assert_eq!(type_counts().0, v0, "every VALUE handle released");
}

fn overloads_pick_by_arity_and_js_type() {
    install_host();
    let loc = handle(&new("QLocale", &[s("C")]));
    // toString(int) for an integer, toString(double, char, int) for a fraction.
    assert_eq!(text(&call(loc, "toString", &[Value::I64(42)]).unwrap()), "42");
    assert_eq!(text(&call(loc, "toString", &[Value::F64(1.5)]).unwrap()), "1.5");
    // A date record (a plain object through a dynamic slot reaches the backend as a map).
    let d = Value::Map(vec![("year".into(), Value::I64(2026)), ("month".into(), Value::I64(10)), ("day".into(), Value::I64(19))]);
    let fmt = text(&call(loc, "toString", &[d, s("yyyy-MM-dd")]).unwrap());
    assert_eq!(fmt, "2026-10-19");
    // QFileInfo(QString) vs QFileInfo(QDir, QString): the second needs a QDir value first.
    let fi = handle(&new("QFileInfo", &[s("/usr/share")]));
    assert_eq!(text(&call(fi, "fileName", &[]).unwrap()), "share");
    let dir = new("QDir", &[s("/usr")]);
    let fi2 = handle(&new("QFileInfo", &[dir.clone(), s("share")]));
    assert_eq!(text(&call(fi2, "absoluteFilePath", &[]).unwrap()), "/usr/share");
    for x in [loc, fi, fi2, handle(&dir)] {
        be().destroy(x);
    }
}

fn string_list_model_rows_indexes_and_signals() {
    let rec = install_host();
    let k = kind("QStringListModel");
    assert_eq!(chain(k), ["QStringListModel", "QAbstractListModel", "QAbstractItemModel", "QObject"]);
    let m = handle(&be().construct(k, &[], &[]).unwrap());
    call(m, "setStringList", &[Value::List(vec![s("a"), s("b")])]).unwrap();
    assert_eq!(call(m, "rowCount", &[]).unwrap(), Value::I64(2));
    let inserted = be().resolve(NameKind::Signal, k, "rows-inserted");
    be().listen(m, inserted, true).unwrap();
    assert_eq!(call(m, "insertRows", &[Value::I64(2), Value::I64(1)]).unwrap(), Value::Bool(true));
    let ev = rec.borrow().events.iter().find(|e| e.0 == m && e.1 == inserted).cloned().expect("rowsInserted");
    // (const QModelIndex &parent, int first, int last): a QModelIndex value, then integers.
    assert!(matches!(ev.2[0], Value::Boxed { .. }), "{:?}", ev.2);
    assert_eq!((&ev.2[1], &ev.2[2]), (&Value::I64(2), &Value::I64(2)));
    let idx = call(m, "index", &[Value::I64(2), Value::I64(0)]).unwrap();
    assert_eq!(call(m, "setData", &[idx.clone(), s("c")]).unwrap(), Value::Bool(true));
    assert_eq!(call(m, "stringList", &[]).unwrap(), Value::List(vec![s("a"), s("b"), s("c")]));
    let i = handle(&idx);
    assert_eq!(be().handle_info(i).unwrap().kind, kind("QModelIndex"));
    assert_eq!(call(i, "row", &[]).unwrap(), Value::I64(2));
    assert_eq!(text(&call(i, "data", &[]).unwrap()), "c");
    // A persistent index: removing the row invalidates it instead of leaving it dangling.
    call(m, "removeRows", &[Value::I64(2), Value::I64(1)]).unwrap();
    assert_eq!(call(i, "isValid", &[]).unwrap(), Value::Bool(false));
    be().destroy(i);
    be().destroy(m);
    pump(20);
}

fn standard_items_move_into_the_model_and_report_native_death() {
    let rec = install_host();
    let (_, p0, t0) = type_counts();
    let m = handle(&new("QStandardItemModel", &[]));
    let it = handle(&new("QStandardItem", &[s("one")]));
    assert_eq!(be().handle_info(it).unwrap().lifetime, SVL_LIFE_OWNED, "JS owns an item it made");
    call(m, "appendRow", &[Value::Handle(it)]).unwrap();
    assert_eq!(be().handle_info(it).unwrap().lifetime, SVL_LIFE_BORROWED, "appendRow moved it into the model (PySide parent fact)");
    // Identity: the model hands back the same handle.
    assert_eq!(call(m, "item", &[Value::I64(0)]).unwrap(), Value::Handle(it));
    assert_eq!(text(&call(it, "text", &[]).unwrap()), "one");
    // An item the model makes from its (tracked) prototype is watched too.
    let idx = call(m, "index", &[Value::I64(1), Value::I64(0)]).unwrap();
    let _ = idx;
    call(m, "setItem", &[Value::I64(1), Value::I64(0), Value::Handle(handle(&new("QStandardItem", &[s("two")])))]).unwrap();
    assert_eq!(call(m, "rowCount", &[]).unwrap(), Value::I64(2));
    // takeItem gives ownership back (`parent remove` on the result).
    let back = handle(&call(m, "takeItem", &[Value::I64(1)]).unwrap());
    assert_eq!(be().handle_info(back).unwrap().lifetime, SVL_LIFE_OWNED);
    be().destroy(back);
    // Removing the row deletes the item natively: the host hears it, the handle is gone.
    call(m, "removeRow", &[Value::I64(0)]).unwrap();
    assert!(rec.borrow().gone.contains(&(it, SVL_GONE_OWNER)), "{:?}", rec.borrow().gone);
    assert!(be().handle_info(it).is_err());
    be().destroy(m);
    pump(20);
    let (_, p1, t1) = type_counts();
    assert_eq!((p1, t1), (p0, t0), "no item handle or tracked item left behind");
}

fn unique_objects_are_js_owned() {
    install_host();
    let (_, p0, _) = type_counts();
    let db = handle(&new("QMimeDatabase", &[]));
    assert_eq!(be().handle_info(db).unwrap().lifetime, SVL_LIFE_OWNED);
    let mt = handle(&call(db, "mimeTypeForFile", &[s("notes.txt"), s("extension")]).unwrap());
    assert_eq!(text(&call(mt, "name", &[]).unwrap()), "text/plain");
    let dir = handle(&new("QTemporaryDir", &[]));
    assert_eq!(call(dir, "isValid", &[]).unwrap(), Value::Bool(true));
    let path = text(&call(dir, "path", &[]).unwrap());
    assert!(std::path::Path::new(&path).is_dir());
    be().destroy(dir);
    assert!(!std::path::Path::new(&path).exists(), "deleting the QTemporaryDir removed the directory");
    be().destroy(db);
    be().destroy(mt);
    assert_eq!(type_counts().1, p0);
}

fn element_handles_reach_their_type_and_the_meta_fallback() {
    let rec = install_host();
    let te = be().create(kind("text-edit")).unwrap();
    let doc = call(te, "document", &[]).unwrap();
    let d = handle(&doc);
    call(d, "setPlainText", &[s("Hello")]).unwrap();
    assert_eq!(text(&call(d, "toPlainText", &[]).unwrap()), "Hello");
    assert_eq!(call(d, "characterCount", &[]).unwrap(), Value::I64(6));
    // The metaobject fallback: `deleteLater` and `destroyed` have no facts (the core owns lifetimes) but are
    // reachable by name on a QObject kind.
    let t = handle(&new("QTimer", &[]));
    let k = kind("QTimer");
    let dl = be().resolve(NameKind::Method, k, "deleteLater");
    assert!(dl >= 0x1000000, "a dynamic id: {dl:#x}");
    let unknown = be().call(t, be().resolve(NameKind::Method, k, "noSuchThing"), &[]).unwrap_err().message;
    assert!(unknown.contains("unknown method noSuchThing"), "{unknown}");
    be().call(t, dl, &[]).unwrap();
    pump(20);
    assert!(rec.borrow().gone.iter().any(|g| g.0 == t), "native death after deleteLater: {:?}", rec.borrow().gone);
    be().destroy(te);
    pump(20);
}

fn futures_complete_the_hosts_callback() {
    let rec = install_host();
    test_future(41, 10, 7);
    test_future(42, 10, -1);
    pump(80);
    let calls = rec.borrow().callbacks.clone();
    let ok = calls.iter().find(|c| c.0 == 41).expect("resolved");
    assert_eq!(ok.1, [Value::I64(7)]);
    let Value::Error(e) = &calls.iter().find(|c| c.0 == 42).expect("cancelled").1[0] else { panic!("an error") };
    assert_eq!((e.domain.as_str(), e.message.as_str()), ("qt", "operation cancelled"));
}

fn main() {
    // SAFETY: single-threaded at this point.
    unsafe {
        std::env::set_var("QT_ACCESSIBILITY", "0");
        std::env::set_var("DBUS_SESSION_BUS_ADDRESS", "unix:path=/nonexistent");
    }
    counter_qt::run_cases(&[
        ("timer_is_a_shared_object_with_props_methods_and_signals", timer_is_a_shared_object_with_props_methods_and_signals),
        ("a_parented_object_is_only_forgotten", a_parented_object_is_only_forgotten),
        ("url_is_a_value_with_a_kind_methods_statics_and_equality", url_is_a_value_with_a_kind_methods_statics_and_equality),
        ("overloads_pick_by_arity_and_js_type", overloads_pick_by_arity_and_js_type),
        ("string_list_model_rows_indexes_and_signals", string_list_model_rows_indexes_and_signals),
        ("standard_items_move_into_the_model_and_report_native_death", standard_items_move_into_the_model_and_report_native_death),
        ("unique_objects_are_js_owned", unique_objects_are_js_owned),
        ("element_handles_reach_their_type_and_the_meta_fallback", element_handles_reach_their_type_and_the_meta_fallback),
        ("futures_complete_the_hosts_callback", futures_complete_the_hosts_callback),
    ]);
}
