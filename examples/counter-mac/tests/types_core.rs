//! Hand-written tests of the AppKit non-GUI type core (N4, crates/svelters-appkit/src/types.rs): kinds by
//! class name, SHARED identity, init families, `NSError **` outs, Foundation conversions, struct
//! arguments and results, and the element side (non-view objects from element props are SHARED now).
//! An empty suite on other platforms.

fn main() {
    #[cfg(target_os = "macos")]
    svelters_apple::testing::run_cases(
        true,
        &[
            ("kinds_by_class_name", mac::kinds_by_class_name),
            ("init_family_and_nil", mac::init_family_and_nil),
            ("nserror_out_is_a_native_error", mac::nserror_out_is_a_native_error),
            ("foundation_conversions", mac::foundation_conversions),
            ("struct_arguments_and_results", mac::struct_arguments_and_results),
            ("pasteboard_round_trip", mac::pasteboard_round_trip),
            ("element_props_return_shared_objects", mac::element_props_return_shared_objects),
            ("construct_with_props", mac::construct_with_props),
        ],
    );
    #[cfg(not(target_os = "macos"))]
    svelters_apple::testing::run_cases(false, &[]);
}

#[cfg(target_os = "macos")]
mod mac {
    use svelters_abi::*;
    use svelters_appkit::testing::Harness;

    fn handle(v: Value) -> u64 {
        match v {
            Value::Handle(h) => h,
            other => panic!("expected an object handle, got {other:?}"),
        }
    }

    pub fn kinds_by_class_name() {
        let h = Harness::shared();
        let _g = h.case_guard();
        let before = h.backend.handles();
        let b = h.backend_trait();
        assert_eq!(b.resolve(NameKind::Kind, 0, "NSDoesNotExistSvelters"), 0, "an unknown class has no kind");
        assert_ne!(b.resolve(NameKind::Kind, 0, "button"), 0, "element tags still resolve");
        let s = handle(h.call_static("NSAttributedString", "initWithString:", vec![Value::Str("hi".into())]).unwrap());
        let chain = h.type_chain(s);
        assert!(chain.iter().any(|c| c == "NSAttributedString"), "chain {chain:?}");
        assert_eq!(chain.last().map(String::as_str), Some("NSObject"), "chain ends at the root: {chain:?}");
        assert!(h.construct("NSButton", vec![]).is_err(), "widgets are elements, not constructed types");
        h.release_since(&before);
    }

    pub fn init_family_and_nil() {
        let h = Harness::shared();
        let _g = h.case_guard();
        let before = h.backend.handles();
        let u = handle(h.call_static("NSURL", "initWithString:", vec![Value::Str("https://example.com/a?b=1".into())]).unwrap());
        assert_eq!(h.lifetime(u), SVL_LIFE_SHARED);
        assert_eq!(h.call(u, "absoluteString", vec![]).unwrap(), Value::Str("https://example.com/a?b=1".into()));
        assert_eq!(h.call(u, "host", vec![]).unwrap(), Value::Str("example.com".into()));
        let same = handle(h.call(u, "absoluteURL", vec![]).unwrap());
        assert_eq!(h.call(u, "isEqual:", vec![Value::Handle(same)]).unwrap(), Value::Bool(true), "equals through isEqual:");
        // A factory returning nil is null; an initializer returning nil is an error.
        assert_eq!(h.call_static("NSURL", "URLWithString:", vec![Value::Str("http://[".into())]).unwrap(), Value::Null);
        assert!(h.call_static("NSURL", "initWithString:", vec![Value::Str("http://[".into())]).is_err());
        h.release_since(&before);
        assert_eq!(h.backend.handles(), before);
    }

    pub fn nserror_out_is_a_native_error() {
        let h = Harness::shared();
        let _g = h.case_guard();
        let before = h.backend.handles();
        let fm = handle(h.call_static("NSFileManager", "defaultManager", vec![]).unwrap());
        match h.call(fm, "contentsOfDirectoryAtPath:error:", vec![Value::Str("/nonexistent-svelters-n4".into())]).unwrap() {
            Value::Error(e) => {
                assert_eq!(e.domain, "NSCocoaErrorDomain");
                assert_eq!(e.code, Value::I64(260), "NSFileReadNoSuchFileError: {e:?}");
                assert!(!e.message.is_empty());
            }
            v => panic!("expected a native error, got {v:?}"),
        }
        match h.call(fm, "contentsOfDirectoryAtPath:error:", vec![Value::Str("/".into())]).unwrap() {
            Value::List(items) => assert!(items.iter().any(|i| *i == Value::Str("Users".into())), "{items:?}"),
            v => panic!("a list of names, got {v:?}"),
        }
        // BOOL + NSError: success is null.
        let dir = std::env::temp_dir().join(format!("svelters-n4-{}", std::process::id()));
        let r = h.call(fm, "createDirectoryAtPath:withIntermediateDirectories:attributes:error:", vec![Value::Str(dir.display().to_string()), Value::Bool(true), Value::Null]).unwrap();
        assert_eq!(r, Value::Null);
        assert!(dir.is_dir());
        let _ = std::fs::remove_dir(&dir);
        h.release_since(&before);
    }

    pub fn foundation_conversions() {
        let h = Harness::shared();
        let _g = h.case_guard();
        let before = h.backend.handles();
        let d = handle(h.call_static("NSUserDefaults", "initWithSuiteName:", vec![Value::Str("dev.svelters.n4-test".into())]).unwrap());
        let samples = vec![
            ("str", Value::Str("héllo".into())),
            ("int", Value::I64(42)),
            ("float", Value::F64(1.5)),
            ("bool", Value::Bool(true)),
            ("list", Value::List(vec![Value::I64(1), Value::Str("two".into())])),
            ("map", Value::Map(vec![("a".into(), Value::I64(1)), ("b".into(), Value::List(vec![Value::Str("x".into())]))])),
            ("bytes", Value::Bytes(vec![0, 1, 2, 255])),
            ("date", Value::DateTime(1_700_000_000_000.0)),
        ];
        for (k, v) in &samples {
            h.call(d, "setObject:forKey:", vec![v.clone(), Value::Str((*k).into())]).unwrap();
            let back = h.call(d, "objectForKey:", vec![Value::Str((*k).into())]).unwrap();
            assert_eq!(&back, v, "{k} round-trips through NSUserDefaults");
        }
        h.call(d, "removePersistentDomainForName:", vec![Value::Str("dev.svelters.n4-test".into())]).unwrap();
        h.release_since(&before);
    }

    pub fn struct_arguments_and_results() {
        let h = Harness::shared();
        let _g = h.case_guard();
        let before = h.backend.handles();
        let size = Value::Map(vec![("width".into(), Value::F64(10.0)), ("height".into(), Value::F64(20.0))]);
        let img = handle(h.call_static("NSImage", "initWithSize:", vec![size]).unwrap());
        let s = h.get(img, "size").unwrap();
        let fields = match &s {
            Value::Record { fields, .. } => fields.clone(),
            v => panic!("size is a record, got {v:?}"),
        };
        assert_eq!(fields, vec![Value::F64(10.0), Value::F64(20.0)]);
        let r = h.call(img, "alignmentRect", vec![]).unwrap();
        assert!(matches!(r, Value::Record { .. }), "{r:?}");
        let s = handle(h.call_static("NSAttributedString", "initWithString:", vec![Value::Str("hello".into())]).unwrap());
        let sub = handle(h.call(s, "attributedSubstringFromRange:", vec![Value::Map(vec![("location".into(), Value::I64(1)), ("length".into(), Value::I64(3))])]).unwrap());
        assert_eq!(h.call(sub, "string", vec![]).unwrap(), Value::Str("ell".into()));
        h.release_since(&before);
    }

    pub fn pasteboard_round_trip() {
        let h = Harness::shared();
        let _g = h.case_guard();
        let before = h.backend.handles();
        let pb = handle(h.call_static("NSPasteboard", "pasteboardWithUniqueName", vec![]).unwrap());
        h.call(pb, "clearContents", vec![]).unwrap();
        let ok = h.call(pb, "setString:forType:", vec![Value::Str("svelters".into()), Value::Str("public.utf8-plain-text".into())]).unwrap();
        assert_eq!(ok, Value::Bool(true));
        assert_eq!(h.call(pb, "stringForType:", vec![Value::Str("public.utf8-plain-text".into())]).unwrap(), Value::Str("svelters".into()));
        match h.call(pb, "types", vec![]).unwrap() {
            Value::List(t) => assert!(t.contains(&Value::Str("public.utf8-plain-text".into())), "{t:?}"),
            v => panic!("{v:?}"),
        }
        h.call(pb, "releaseGlobally", vec![]).unwrap();
        h.release_since(&before);
    }

    pub fn element_props_return_shared_objects() {
        let h = Harness::shared();
        let _g = h.case_guard();
        let before = h.backend.handles();
        let tf = h.create("text-field");
        let f = handle(h.get(tf, "font").unwrap());
        assert_eq!(h.lifetime(f), SVL_LIFE_SHARED, "a non-view object read from an element is SHARED (decision Q1)");
        assert_eq!(handle(h.get(tf, "font").unwrap()), f, "one handle per object");
        assert!(h.type_chain(f).iter().any(|c| c == "NSFont"), "{:?}", h.type_chain(f));
        // A value made in JS goes into an element prop.
        let c = handle(h.call_static("NSColor", "colorWithSRGBRed:green:blue:alpha:", vec![Value::F64(1.0), Value::F64(0.0), Value::F64(0.0), Value::F64(1.0)]).unwrap());
        h.set(tf, "text-color", Value::Handle(c)).unwrap();
        let back = handle(h.get(tf, "text-color").unwrap());
        assert_eq!(h.call(back, "isEqual:", vec![Value::Handle(c)]).unwrap(), Value::Bool(true));
        h.destroy(tf);
        h.release_since(&before);
    }

    pub fn construct_with_props() {
        let h = Harness::shared();
        let _g = h.case_guard();
        let before = h.backend.handles();
        let u = h.construct("NSUndoManager", vec![("levelsOfUndo", Value::I64(7))]).unwrap();
        assert_eq!(h.lifetime(u), SVL_LIFE_SHARED);
        assert_eq!(h.get(u, "levelsOfUndo").unwrap(), Value::I64(7));
        assert!(h.construct("NSUndoManager", vec![("noSuchProp", Value::I64(1))]).is_err());
        h.release_since(&before);
        assert_eq!(h.backend.handles(), before, "a failed construct leaves no handle");
    }
}
