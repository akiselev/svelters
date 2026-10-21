//! Hand-written checks of the non-GUI type core (N4): SHARED handles and identity, activation and composable
//! factories, statics, props, records, boxing, collections, events through dynamic delegates, DependencyProperty
//! observation, async operations (resolve, reject, cancel) with the hop to the UI thread, and the thread check at
//! minting. Windows only (`harness = false`: one STA thread); `cargo test -p svelters-winui --test types_core`.

#[cfg(windows)]
mod tests {
    use svelters_abi::{SVL_LIFE_SHARED, Value};
    use svelters_winui::testing::Harness;

    fn s(x: &str) -> Value {
        Value::Str(x.into())
    }

    fn handle(v: Value) -> u64 {
        match v {
            Value::Handle(h) => h,
            other => panic!("expected a handle, got {other:?}"),
        }
    }

    fn color(a: i64, r: i64, g: i64, b: i64) -> Value {
        Value::Map(vec![("a".into(), Value::I64(a)), ("r".into(), Value::I64(r)), ("g".into(), Value::I64(g)), ("b".into(), Value::I64(b))])
    }

    pub fn uri_construct_props_statics() {
        let mut h = Harness::new();
        let u = h.new_object("Windows.Foundation.Uri", &[s("https://example.com:8080/a/b?x=1#top")]).unwrap();
        assert_eq!(h.info(u).unwrap().lifetime, SVL_LIFE_SHARED);
        assert_eq!(h.get(u, "host").unwrap(), s("example.com"));
        assert_eq!(h.get(u, "port").unwrap(), Value::I64(8080));
        assert_eq!(h.get(u, "fragment").unwrap(), s("#top"));
        assert_eq!(h.call(u, "toString", &[]).unwrap(), s("https://example.com:8080/a/b?x=1#top"));
        let rel = h.new_object("Windows.Foundation.Uri", &[s("https://example.com/a/"), s("c/d")]).unwrap();
        assert_eq!(h.get(rel, "path").unwrap(), s("/a/c/d"));
        assert_eq!(h.call_static("Windows.Foundation.Uri", "escapeComponent", &[s("a b&c")]).unwrap(), s("a%20b%26c"));
        let k = h.kind("Windows.Foundation.Uri");
        assert_eq!(h.describe_kind(k).unwrap().0, "Windows.Foundation.Uri");
        // A bad URI is a WinRT error (NativeError), not a crash.
        match h.call_static("Windows.Foundation.Uri", "new", &[s("not a uri")]) {
            Ok(Value::Error(e)) => assert_eq!(e.domain, "winrt", "{e:?}"),
            other => panic!("expected an error value, got {other:?}"),
        }
    }

    pub fn identity_and_destroy() {
        let mut h = Harness::new();
        let q1 = handle(h.call_static("Microsoft.UI.Dispatching.DispatcherQueue", "getForCurrentThread", &[]).unwrap());
        let q2 = handle(h.call_static("Microsoft.UI.Dispatching.DispatcherQueue", "getForCurrentThread", &[]).unwrap());
        assert_eq!(q1, q2, "one handle per native object");
        assert_eq!(h.get(q1, "hasThreadAccess").unwrap(), Value::Bool(true));
        h.destroy(q1);
        assert!(h.info(q1).is_err(), "destroy drops the handle");
        let q3 = handle(h.call_static("Microsoft.UI.Dispatching.DispatcherQueue", "getForCurrentThread", &[]).unwrap());
        assert_ne!(q3, q1, "handles are never reused");
    }

    pub fn property_set_boxing_and_events() {
        let mut h = Harness::new();
        let ps = h.construct("Windows.Foundation.Collections.PropertySet", &[]).unwrap();
        h.listen(ps, "map-changed", true).unwrap();
        assert_eq!(h.call(ps, "insert", &[s("n"), Value::F64(1.5)]).unwrap(), Value::Bool(false));
        h.call(ps, "insert", &[s("t"), s("text")]).unwrap();
        h.call(ps, "insert", &[s("b"), Value::Bool(true)]).unwrap();
        assert_eq!(h.call(ps, "lookup", &[s("n")]).unwrap(), Value::F64(1.5));
        assert_eq!(h.call(ps, "lookup", &[s("t")]).unwrap(), s("text"));
        assert_eq!(h.call(ps, "lookup", &[s("b")]).unwrap(), Value::Bool(true));
        assert_eq!(h.get(ps, "size").unwrap(), Value::I64(3));
        assert_eq!(h.call(ps, "hasKey", &[s("t")]).unwrap(), Value::Bool(true));
        match h.call(ps, "getView", &[]).unwrap() {
            Value::Map(m) => assert_eq!(m.len(), 3),
            other => panic!("getView: {other:?}"),
        }
        h.pump(50);
        let evs = h.event_args(ps, "map-changed");
        assert_eq!(evs.len(), 3, "one map-changed per insert: {evs:?}");
        assert_eq!(evs[0][0], Value::Handle(ps), "the sender is the same handle (identity)");
        h.listen(ps, "map-changed", false).unwrap();
        h.call(ps, "remove", &[s("t")]).unwrap();
        h.pump(50);
        assert_eq!(h.event_args(ps, "map-changed").len(), 3, "nothing after unlisten");
    }

    pub fn brush_records_dp_observe_and_thread_class() {
        let mut h = Harness::new();
        let b = h.construct("Microsoft.UI.Xaml.Media.SolidColorBrush", &[("opacity", Value::F64(0.5))]).unwrap();
        assert_eq!(h.get(b, "opacity").unwrap(), Value::F64(0.5));
        let red = h.call_static("Microsoft.UI.Colors", "red", &[]).unwrap();
        assert_eq!(red, color(255, 255, 0, 0));
        h.observe(b, "color", true).unwrap();
        h.set(b, "color", red.clone()).unwrap();
        assert_eq!(h.get(b, "color").unwrap(), red);
        assert!(h.take_changed_named(b, "color") >= 1, "a DependencyProperty change is announced");
        h.observe(b, "color", false).unwrap();
        let c = color(255, 1, 2, 3);
        let b2 = h.new_object("Microsoft.UI.Xaml.Media.SolidColorBrush", &[c.clone()]).unwrap();
        assert_eq!(h.get(b2, "color").unwrap(), c);
        // Section 4.1 rule 3: a Compositor made on another dispatcher thread is refused at minting.
        let err = h.mint_foreign_compositor().unwrap_err();
        assert!(err.contains("another UI thread"), "{err}");
        let comp = h.construct("Microsoft.UI.Composition.Compositor", &[]).unwrap();
        h.call(comp, "close", &[]).unwrap();
    }

    pub fn structs_from_statics() {
        let mut h = Harness::new();
        let t = h.call_static("Microsoft.UI.Xaml.ThicknessHelper", "fromLengths", &[Value::F64(1.0), Value::F64(2.0), Value::F64(3.0), Value::F64(4.0)]).unwrap();
        assert_eq!(t, Value::Map(vec![("left".into(), Value::F64(1.0)), ("top".into(), Value::F64(2.0)), ("right".into(), Value::F64(3.0)), ("bottom".into(), Value::F64(4.0))]));
        let c = h.call_static("Microsoft.UI.ColorHelper", "fromArgb", &[Value::I64(255), Value::I64(10), Value::I64(20), Value::I64(30)]).unwrap();
        assert_eq!(c, color(255, 10, 20, 30));
    }

    pub fn timer_events() {
        let mut h = Harness::new();
        let q = handle(h.call_static("Microsoft.UI.Dispatching.DispatcherQueue", "getForCurrentThread", &[]).unwrap());
        let t = handle(h.call_keep(q, "createTimer", &[]).unwrap());
        h.set(t, "interval", Value::F64(15.0)).unwrap();
        assert_eq!(h.get(t, "interval").unwrap(), Value::F64(15.0));
        h.set(t, "isRepeating", Value::Bool(true)).unwrap();
        h.listen(t, "tick", true).unwrap();
        h.call(t, "start", &[]).unwrap();
        h.pump(300);
        h.call(t, "stop", &[]).unwrap();
        let ticks = h.event_args(t, "tick");
        assert!(ticks.len() >= 2, "ticks: {}", ticks.len());
        assert_eq!(ticks[0][0], Value::Handle(t), "sender is the timer");
    }

    pub fn async_storage_round_trip() {
        let mut h = Harness::new();
        let dir = Harness::temp_dir("core");
        let folder = handle(h.await_static("Windows.Storage.StorageFolder", "getFolderFromPathAsync", &[s(&dir)]).unwrap());
        let file = handle(h.await_call(folder, "createFileAsync", &[s("n4.txt"), s("replace-existing")]).unwrap());
        assert_eq!(h.get(file, "name").unwrap(), s("n4.txt"));
        h.await_static("Windows.Storage.FileIO", "writeTextAsync", &[Value::Handle(file), s("hello from svelters")]).unwrap();
        assert_eq!(h.await_static("Windows.Storage.FileIO", "readTextAsync", &[Value::Handle(file)]).unwrap(), s("hello from svelters"));
        assert_eq!(std::fs::read_to_string(std::path::Path::new(&dir).join("n4.txt")).unwrap(), "hello from svelters");
        // A list result (IVectorView<StorageFile>).
        match h.await_call(folder, "getFilesAsync", &[]).unwrap() {
            Value::List(items) => assert_eq!(items.len(), 1),
            other => panic!("getFilesAsync: {other:?}"),
        }
        // Rejection with the WinRT error.
        let err = h.await_call(folder, "getFileAsync", &[s("missing.txt")]).unwrap_err();
        assert!(err.contains("file-not-found"), "{err}");
        // Cancel: one completion, a value or `cancelled`.
        let cb = h.call_async(folder, "getItemsAsync", &[]).unwrap();
        h.cancel(cb);
        match h.wait(cb, 10_000) {
            Some(Value::Error(e)) => assert_eq!(e.code, s("cancelled"), "{e:?}"),
            Some(_) => {}
            None => panic!("no completion after cancel"),
        }
    }

    pub fn json_values() {
        let mut h = Harness::new();
        let o = handle(h.call_static("Windows.Data.Json.JsonObject", "parse", &[s(r#"{"a": 1, "b": "two", "c": [1, 2]}"#)]).unwrap());
        assert_eq!(h.call(o, "getNamedNumber", &[s("a")]).unwrap(), Value::F64(1.0));
        assert_eq!(h.call(o, "getNamedString", &[s("b")]).unwrap(), s("two"));
        assert_eq!(h.get(o, "valueType").unwrap(), s("object"));
        assert!(matches!(h.call(o, "stringify", &[]).unwrap(), Value::Str(t) if t.contains("\"b\":\"two\"")));
    }

    pub static TESTS: &[(&str, fn())] = &[
        ("types_core::uri_construct_props_statics", uri_construct_props_statics),
        ("types_core::identity_and_destroy", identity_and_destroy),
        ("types_core::property_set_boxing_and_events", property_set_boxing_and_events),
        ("types_core::brush_records_dp_observe_and_thread_class", brush_records_dp_observe_and_thread_class),
        ("types_core::structs_from_statics", structs_from_statics),
        ("types_core::timer_events", timer_events),
        ("types_core::async_storage_round_trip", async_storage_round_trip),
        ("types_core::json_values", json_values),
    ];
}

fn main() {
    #[cfg(windows)]
    let tests = tests::TESTS;
    #[cfg(not(windows))]
    let tests: &[(&str, fn())] = &[];
    std::process::exit(svelters_winui::harness::main(tests));
}
