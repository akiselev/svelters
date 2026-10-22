//! Helpers of the non-GUI type tests (N4, plans/milestones/N4-appkit-types.md): the generated per-type
//! fact groups (`examples/counter-mac/tests/types.rs`, written by `tools/metadata/appkit/gen_types.py` from
//! the type facts) and the hand-written core tests (`types_core.rs`) drive the backend's v2.5 members
//! (`construct`, `call_static`, `describe(SVL_KIND)`) and its reflective calls through [`Harness`].

use super::*;
use objc2::rc::Weak;
use objc2::runtime::AnyClass;

/// A JSON-ish scalar in a generated table (`Value` owns heap data).
#[derive(Clone, Copy, Debug)]
pub enum J {
    Null,
    Bool(bool),
    I(i64),
    F(f64),
    S(&'static str),
}

impl J {
    pub fn v(self) -> Value {
        match self {
            J::Null => Value::Null,
            J::Bool(b) => Value::Bool(b),
            J::I(i) => Value::I64(i),
            J::F(f) => Value::F64(f),
            J::S(s) => Value::Str(s.to_string()),
        }
    }
}

/// The JS slot kind of a prop (`types_appkit_classify` slot `k`), for loose result checks.
pub type Kind = &'static str;

/// One curated type, as the generated tests see it.
pub struct Spec {
    pub class: &'static str,
    pub value: bool,
    /// How to make the subject: a class selector (or `init…` / `new`) and its arguments.
    pub subject: (&'static str, &'static [J]),
    /// A sample for `new X(string)` (the `string_ctor` initializer), if the class has one.
    pub string_ctor: Option<(&'static str, &'static str)>,
    pub constructible: bool,
    /// (KVC key, getter selector, writable, kind) of every bound property.
    pub props: &'static [(&'static str, &'static str, bool, Kind)],
    /// Zero-argument instance selectors that only read (safe to call on the subject).
    pub readers: &'static [&'static str],
    /// Zero-argument class selectors that only read or make.
    pub statics: &'static [&'static str],
    /// Every bound method: (selector, static).
    pub members: &'static [(&'static str, bool)],
    /// Async members: (selector, static, JS arguments).
    pub asyncs: &'static [(&'static str, bool, u8)],
}

/// The bound members of a non-curated type (the `members` fact group of every bound type).
pub struct Members {
    pub class: &'static str,
    pub members: &'static [(&'static str, bool)],
    pub keys: &'static [&'static str],
}

/// An error that means the binding is wrong (not an Objective-C exception the method raised by design).
pub fn binding_error(msg: &str) -> bool {
    ["does not respond", "unknown method", "not supported", "takes ", "missing argument", "stale or foreign", "unknown prop", "no method signature"]
        .iter()
        .any(|p| msg.contains(p))
}

impl Harness {
    pub fn type_kind(&self, class: &str) -> u32 {
        let k = self.b_types().resolve(NameKind::Kind, 0, class);
        assert!(k != 0, "unknown type kind {class}");
        k
    }

    fn b_types(&self) -> &dyn Backend {
        &self.backend
    }

    /// `call_static(kind, sel, args)`; a native error comes back as `Ok(Value::Error)`.
    pub fn call_static(&self, class: &str, sel: &str, args: Vec<Value>) -> Result<Value, String> {
        let k = self.type_kind(class);
        match self.b_types().resolve(NameKind::Method, k, sel) {
            0 => Err(format!("unknown method {class}.{sel}")),
            m => self.b_types().call_static(k, m, &args),
        }
    }

    /// `construct(kind, props)`.
    pub fn construct(&self, class: &str, props: Vec<(&str, Value)>) -> Result<u64, String> {
        let k = self.type_kind(class);
        let mut ids = Vec::new();
        let mut vals = Vec::new();
        for (n, v) in props {
            match self.b_types().resolve(NameKind::Prop, k, n) {
                0 => return Err(format!("unknown prop {class}.{n}")),
                id => {
                    ids.push(id);
                    vals.push(v);
                }
            }
        }
        match self.b_types().construct(k, &ids, &vals)? {
            Value::Handle(h) => Ok(h),
            other => Err(format!("construct {class}: expected a handle, got {other:?}")),
        }
    }

    /// The native type names of a handle's kind chain (`describe(SVL_KIND)`, the host's `typeChain`).
    pub fn type_chain(&self, h: u64) -> Vec<String> {
        let mut k = self.kind_of(h);
        let mut out = Vec::new();
        while k != 0 && out.len() < 64 {
            let Some(d) = self.b_types().describe(NameKind::Kind, 0, k) else { break };
            out.push(d.name.clone());
            k = d.ret.ty;
        }
        out
    }

    /// The descriptor of method `sel` on a handle's kind (or a class's kind with `h == 0`).
    pub fn method_desc(&self, kind: u32, sel: &str) -> Option<MemberDesc> {
        match self.b_types().resolve(NameKind::Method, kind, sel) {
            0 => None,
            m => self.b_types().describe(NameKind::Method, kind, m),
        }
    }

    /// Starts an async member (`SVL_M_ASYNC`): `h == 0` for a class method. Returns the completion id.
    pub fn start_async(&self, h: u64, class: &str, sel: &str, mut args: Vec<Value>) -> Result<u64, String> {
        thread_local! { static NEXT: std::cell::Cell<u64> = const { std::cell::Cell::new(1 << 40) }; }
        let id = NEXT.with(|n| {
            let v = n.get() + 1;
            n.set(v);
            v
        });
        let k = if h == 0 { self.type_kind(class) } else { self.kind_of(h) };
        let desc = self.method_desc(k, sel).ok_or_else(|| format!("{class}.{sel} has no async descriptor"))?;
        let cb = desc.params.last().filter(|p| p.tag == SVL_CALLBACK).ok_or("no completion slot")?;
        args.push(Value::Callback { id, ty: cb.ty });
        let m = self.b_types().resolve(NameKind::Method, k, sel);
        let r = if h == 0 { self.b_types().call_static(k, m, &args) } else { self.b_types().call(h, m, &args) };
        match r? {
            Value::Error(e) => Err(format!("{}: {} ({:?})", e.domain, e.message, e.code)),
            _ => Ok(id),
        }
    }

    pub fn cancel_async(&self, id: u64) -> bool {
        self.b_types().cancel(id)
    }

    /// Pumps the run loop until completion `id` arrives (or `ms` pass); returns its value.
    pub fn wait_completion(&self, id: u64, ms: u64) -> Option<Value> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(ms);
        let mut seen = Vec::new();
        loop {
            self.backend.run_type_hops();
            for (cid, args) in self.take_completions() {
                if cid == id {
                    return Some(args.into_iter().next().unwrap_or(Value::Null));
                }
                seen.push((cid, args));
            }
            if std::time::Instant::now() >= deadline {
                return None;
            }
            self.pump(10);
        }
    }

    /// Destroys every handle minted since `before` (returned objects the test did not keep).
    pub fn release_since(&self, before: &[u64]) {
        let new: Vec<u64> = self.backend.handles().into_iter().filter(|x| !before.contains(x)).collect();
        for x in new.into_iter().rev() {
            self.destroy(x);
        }
    }
}

fn class(name: &str) -> &'static AnyClass {
    svelters_apple::objc::bridge::class_named(name).unwrap_or_else(|| panic!("class {name} is not loaded"))
}

/// Makes the subject of a curated type: a SHARED handle.
pub fn subject(h: &Harness, s: &Spec) -> u64 {
    let args: Vec<Value> = s.subject.1.iter().map(|j| j.v()).collect();
    let v = h.call_static(s.class, s.subject.0, args).unwrap_or_else(|e| panic!("{}: +{}: {e}", s.class, s.subject.0));
    match v {
        Value::Handle(x) => x,
        other => panic!("{}: +{} returned {other:?}, not an object", s.class, s.subject.0),
    }
}

fn check_kind(name: &str, kind: Kind, v: &Value) {
    let ok = match (kind, v) {
        (_, Value::Error(e)) => panic!("{name}: native error {}: {}", e.domain, e.message),
        (_, Value::Null) => true,
        ("bool", Value::Bool(_)) => true,
        ("int" | "enum" | "flags", Value::I64(_) | Value::U64(_) | Value::Bool(_)) => true,
        ("float", Value::F64(_) | Value::I64(_)) => true,
        ("string", Value::Str(_)) => true,
        ("object" | "value" | "any", _) => true,
        ("record", Value::Record { .. } | Value::Map(_) | Value::List(_)) => true,
        ("list", Value::List(_)) => true,
        ("map", Value::Map(_)) => true,
        ("bytes", Value::Bytes(_)) => true,
        ("date", Value::DateTime(_)) => true,
        ("number", Value::I64(_) | Value::F64(_) | Value::U64(_) | Value::Bool(_)) => true,
        ("type", Value::Str(_)) => true,
        _ => false,
    };
    assert!(ok, "{name}: a {kind} slot read {v:?}");
}

/// On a macOS older than the SDK the facts came from, members newer than the OS are absent: those are
/// logged and skipped. On a current macOS (CI) every member must exist.
fn older_than_sdk() -> bool {
    !svelters_apple::objc::bridge::os_at_least(crate::generated::types::SDK_MACOS)
}

/// Whether `sel` is missing from `cls` (instance) or its metaclass (`is_static`) on an older macOS.
fn absent_here(cls: &str, sel: &str, is_static: bool) -> bool {
    if !older_than_sdk() {
        return false;
    }
    let c = class(cls);
    let s = svelters_apple::objc::bridge::sel(sel);
    let found = if is_static {
        let meta = unsafe { &*(c as *const AnyClass as *const objc2::runtime::AnyObject) };
        svelters_apple::objc::bridge::responds(meta, s)
    } else {
        c.instance_method(s).is_some()
    };
    if !found {
        eprintln!("{cls} {}{sel}: absent on this macOS (older than SDK {}), skipped", if is_static { "+" } else { "-" }, crate::generated::types::SDK_MACOS);
    }
    !found
}

fn read_ok(what: &str, r: Result<Value, String>) -> Option<Value> {
    match r {
        Ok(v) => Some(v),
        Err(e) if binding_error(&e) => panic!("{what}: {e}"),
        // An Objective-C exception the method raises by design (e.g. -cyanComponent of an RGB colour).
        Err(e) => {
            eprintln!("{what}: native exception (accepted): {e}");
            None
        }
    }
}

/// `construct`: the subject is a SHARED handle whose kind chain names the class; `new X(string)` and
/// `new X(props)` work where the facts say so.
pub fn construct(s: &Spec) {
    let h = Harness::shared();
    let _g = h.case_guard();
    let before = h.backend.handles();
    let x = subject(h, s);
    assert_eq!(h.lifetime(x), SVL_LIFE_SHARED, "{}: non-GUI objects are SHARED", s.class);
    let chain = h.type_chain(x);
    assert!(chain.iter().any(|c| c == s.class) || svelters_apple::objc::bridge::is_kind(&h.backend.object(x).unwrap(), class(s.class)),
        "{}: kind chain {chain:?}", s.class);
    if let Some((sel, sample)) = s.string_ctor {
        let v = h.call_static(s.class, sel, vec![Value::Str(sample.into())]).unwrap();
        assert!(matches!(v, Value::Handle(_)), "{}: {sel} {sample:?} -> {v:?}", s.class);
    }
    if s.constructible && !s.value {
        let y = h.construct(s.class, vec![]).unwrap_or_else(|e| panic!("construct {}: {e}", s.class));
        assert_eq!(h.lifetime(y), SVL_LIFE_SHARED);
    }
    h.release_since(&before);
    assert_eq!(h.backend.handles(), before, "{}: handles left after the test", s.class);
}

/// `props`: every bound property reads with the declared slot kind; simple writable ones round-trip.
pub fn props(s: &Spec) {
    let h = Harness::shared();
    let _g = h.case_guard();
    let before = h.backend.handles();
    let x = subject(h, s);
    for (key, getter, writable, kind) in s.props {
        if absent_here(s.class, getter, false) {
            continue;
        }
        let what = format!("{}.{key}", s.class);
        let v = if s.value { read_ok(&what, h.call(x, getter, vec![])) } else { read_ok(&what, h.get(x, key)) };
        let Some(v) = v else { continue };
        check_kind(&what, kind, &v);
        if *writable && !s.value && matches!(*kind, "bool" | "int" | "float" | "string" | "enum" | "flags") && v != Value::Null {
            h.set(x, key, v.clone()).unwrap_or_else(|e| panic!("{what} = {v:?}: {e}"));
            let back = h.get(x, key).unwrap();
            assert_eq!(back, v, "{what}: round trip");
        }
    }
    h.release_since(&before);
}

/// `methods`: every zero-argument reader is reached by selector on the subject.
pub fn methods(s: &Spec) {
    let h = Harness::shared();
    let _g = h.case_guard();
    let before = h.backend.handles();
    let x = subject(h, s);
    for sel in s.readers {
        if absent_here(s.class, sel, false) {
            continue;
        }
        read_ok(&format!("-[{} {sel}]", s.class), h.call(x, sel, vec![]));
    }
    h.release_since(&before);
}

/// `statics`: every zero-argument class reader / factory is reached through `call_static`.
pub fn statics(s: &Spec) {
    let h = Harness::shared();
    let _g = h.case_guard();
    let before = h.backend.handles();
    for sel in s.statics {
        if let Some(Value::Handle(r)) = read_ok(&format!("+[{} {sel}]", s.class), h.call_static(s.class, sel, vec![])) {
            assert_eq!(h.lifetime(r), SVL_LIFE_SHARED, "+[{} {sel}] returns a SHARED handle", s.class);
        }
    }
    h.release_since(&before);
}

/// `lifetime`: one handle per object (identity), `destroy` drops our reference and restores the baseline;
/// a fresh subject (made by `new`/`init…`) is deallocated once our handle is gone.
pub fn lifetime(s: &Spec) {
    let h = Harness::shared();
    let _g = h.case_guard();
    let before = h.backend.handles();
    let x = subject(h, s);
    let again = h.call(x, "self", vec![]).unwrap();
    assert_eq!(again, Value::Handle(x), "{}: the same object crosses as the same handle", s.class);
    if s.value {
        assert_eq!(h.call(x, "isEqual:", vec![Value::Handle(x)]).unwrap(), Value::Bool(true), "{}: equals itself", s.class);
    }
    let fresh = s.subject.0 == "new" || s.subject.0.starts_with("init");
    let weak = objc2::rc::autoreleasepool(|_| Weak::from_retained(&h.retain(x)));
    h.destroy(x);
    assert!(!h.alive(x), "{}: destroyed", s.class);
    h.release_since(&before);
    assert_eq!(h.backend.handles(), before, "{}: back to the baseline", s.class);
    h.pump(10);
    if fresh {
        assert!(weak.load().is_none(), "{}: a fresh subject is deallocated after its only handle is destroyed", s.class);
    }
}

/// `members`: every bound method and property of the facts resolves on this macOS (selectors by the
/// runtime, KVC keys through `resolve(SVL_PROP)`).
pub fn members(m: &Members) {
    let h = Harness::shared();
    let c = class(m.class);
    let meta = unsafe { &*(c as *const AnyClass as *const objc2::runtime::AnyObject) };
    let mut missing = Vec::new();
    for (sel, is_static) in m.members {
        let s = svelters_apple::objc::bridge::sel(sel);
        let ok = if sel.starts_with("init") {
            // Class clusters implement initializers on the placeholder `+alloc` returns.
            c.instance_method(s).is_some() || {
                let a: *mut objc2::runtime::AnyObject = unsafe { objc2::runtime::MessageReceiver::send_message(c, objc2::sel!(alloc), ()) };
                let ok = unsafe { a.as_ref() }.is_some_and(|a| svelters_apple::objc::bridge::responds(a, s));
                if let Some(a) = unsafe { a.as_ref() } {
                    let _: () = unsafe { objc2::msg_send![a, release] };
                }
                ok
            }
        } else if *is_static {
            svelters_apple::objc::bridge::responds(meta, s)
        } else {
            c.instance_method(s).is_some()
        };
        if !ok {
            missing.push(format!("{}{sel}", if *is_static { "+" } else { "-" }));
        }
    }
    let k = h.type_kind(m.class);
    for key in m.keys {
        if h.backend_trait().resolve(NameKind::Prop, k, key) == 0 {
            missing.push(format!("prop {key}"));
        }
    }
    if !missing.is_empty() && older_than_sdk() {
        eprintln!("{}: absent on this macOS (older than SDK {}), skipped: {missing:?}", m.class, crate::generated::types::SDK_MACOS);
        return;
    }
    assert!(missing.is_empty(), "{}: not found on this macOS: {missing:?}", m.class);
}

/// `async`: every async member describes itself as `SVL_M_ASYNC` with a completion slot (static flag as
/// the facts say); NSItemProvider also completes, rejects, cancels and dies with its owner for real.
pub fn asyncs(s: &Spec) {
    let h = Harness::shared();
    let _g = h.case_guard();
    let k = h.type_kind(s.class);
    for (sel, is_static, args) in s.asyncs {
        if absent_here(s.class, sel, *is_static) {
            continue;
        }
        let d = h.method_desc(k, sel).unwrap_or_else(|| panic!("{}.{sel}: no descriptor", s.class));
        assert!(d.flags & SVL_M_ASYNC != 0, "{}.{sel}: SVL_M_ASYNC", s.class);
        assert_eq!(d.flags & SVL_M_STATIC != 0, *is_static, "{}.{sel}: static flag", s.class);
        assert_eq!(d.params.len(), *args as usize + 1, "{}.{sel}: arguments + completion", s.class);
        assert_eq!(d.params.last().map(|p| p.tag), Some(SVL_CALLBACK));
    }
    if s.class == "NSItemProvider" {
        item_provider_roundtrip(h);
    }
}

/// NSItemProvider: `loadDataRepresentationForTypeIdentifier:completionHandler:` completes on a background
/// queue; the completion hops to the main thread and resolves with the bytes; an unknown type rejects with
/// the NSError; a cancel rejects at once and the late block is ignored; destroying the owner drops it.
pub fn item_provider_roundtrip(h: &Harness) {
    let before = h.backend.handles();
    let p = match h.call_static("NSItemProvider", "new", vec![]).unwrap() {
        Value::Handle(p) => p,
        v => panic!("+new -> {v:?}"),
    };
    h.call(p, "registerObject:visibility:", vec![Value::Str("hello".into()), Value::I64(0)]).unwrap();
    let id = h.start_async(p, "NSItemProvider", "loadDataRepresentationForTypeIdentifier:completionHandler:", vec![Value::Str("public.utf8-plain-text".into())]).unwrap();
    let v = h.wait_completion(id, 5000).expect("completion within 5 s");
    assert_eq!(v, Value::Bytes(b"hello".to_vec()), "resolved with the registered string's bytes");
    let id = h.start_async(p, "NSItemProvider", "loadDataRepresentationForTypeIdentifier:completionHandler:", vec![Value::Str("public.png".into())]).unwrap();
    match h.wait_completion(id, 5000).expect("rejection within 5 s") {
        Value::Error(e) => assert!(!e.domain.is_empty(), "an NSError: {e:?}"),
        v => panic!("an unknown type rejects, got {v:?}"),
    }
    let id = h.start_async(p, "NSItemProvider", "loadDataRepresentationForTypeIdentifier:completionHandler:", vec![Value::Str("public.utf8-plain-text".into())]).unwrap();
    let cancelled = h.cancel_async(id);
    if !cancelled {
        // The block won the race: its completion is queued; deliver it.
        h.wait_completion(id, 5000).expect("the completion that beat the cancel");
    } else {
        match h.take_completions().into_iter().find(|(c, _)| *c == id).map(|(_, a)| a) {
            Some(a) => assert!(matches!(a.first(), Some(Value::Error(e)) if e.code == Value::I64(3072)), "cancel rejects: {a:?}"),
            None => panic!("cancel completes at once"),
        }
        h.pump(50);
        h.backend.run_type_hops();
        assert!(h.take_completions().iter().all(|(c, _)| *c != id), "the late block is ignored");
    }
    assert_eq!(h.backend.pending_async(), 0, "every operation settled");
    let id = h.start_async(p, "NSItemProvider", "loadDataRepresentationForTypeIdentifier:completionHandler:", vec![Value::Str("public.utf8-plain-text".into())]).unwrap();
    assert_eq!(h.backend.pending_async(), 1);
    h.destroy(p);
    assert_eq!(h.backend.pending_async(), 0, "the owner's operations are dropped with it");
    h.pump(50);
    h.backend.run_type_hops();
    assert!(h.take_completions().iter().all(|(c, _)| *c != id), "no completion after the owner died (the host rejects it)");
    h.release_since(&before);
}
