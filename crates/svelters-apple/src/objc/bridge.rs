//! The dynamic ObjC bridge (research note §4.1): classes by name, properties by KVC, methods by
//! `NSInvocation`, all driven by a [`Ty`] from the runtime or the generated tables.
//!
//! Deviation from the research note: no ObjC helper file. `NSInvocation` is driven with typed
//! `msg_send!` calls from Rust (its own API has fixed signatures), so the crate needs no C compiler
//! and `cargo check --target aarch64-apple-darwin` works on Linux without an SDK.

use crate::enc::{self, Members, PropAttrs, Scalar, StructTy, Ty};
use crate::names;
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Sel};
use objc2::{class, msg_send};
use objc2_foundation::{NSNumber, NSString};
use std::ffi::{CStr, CString, c_char, c_uint, c_void};
use svelters_abi::Value;

/// Runs `f`, turning an Objective-C exception into an error (with the `exceptions` feature; without
/// it an exception aborts the process, as any ObjC exception crossing Rust frames does).
pub fn catch<R>(what: &str, f: impl FnOnce() -> R) -> Result<R, String> {
    #[cfg(feature = "exceptions")]
    {
        match objc2::exception::catch(std::panic::AssertUnwindSafe(f)) {
            Ok(r) => Ok(r),
            Err(Some(e)) => {
                let reason: Option<Retained<NSString>> = unsafe { msg_send![&*e, reason] };
                Err(format!("{what}: Objective-C exception: {}", reason.map(|r| r.to_string()).unwrap_or_default()))
            }
            Err(None) => Err(format!("{what}: Objective-C exception (nil)")),
        }
    }
    #[cfg(not(feature = "exceptions"))]
    {
        let _ = what;
        Ok(f())
    }
}

/// Any object as `AnyObject` (upcast).
pub fn erase<T: objc2::Message>(r: Retained<T>) -> Retained<AnyObject> {
    // SAFETY: every Objective-C object is an AnyObject.
    unsafe { Retained::cast_unchecked(r) }
}

pub fn class_named(name: &str) -> Option<&'static AnyClass> {
    let c = CString::new(name).ok()?;
    AnyClass::get(&c)
}

pub fn sel(name: &str) -> Sel {
    let c = CString::new(name).unwrap_or_default();
    Sel::register(&c)
}

pub fn nsstring(s: &str) -> Retained<NSString> {
    NSString::from_str(s)
}

pub fn responds(obj: &AnyObject, s: Sel) -> bool {
    unsafe { msg_send![obj, respondsToSelector: s] }
}

pub fn is_kind(obj: &AnyObject, cls: &AnyClass) -> bool {
    unsafe { msg_send![obj, isKindOfClass: cls] }
}

pub fn class_name_of(obj: &AnyObject) -> String {
    obj.class().name().to_string_lossy().into_owned()
}

/// The class and its superclasses, most derived first.
pub fn class_chain(cls: &AnyClass) -> Vec<&AnyClass> {
    let mut out = vec![cls];
    let mut c = cls;
    while let Some(s) = c.superclass() {
        out.push(s);
        c = s;
    }
    out
}

unsafe extern "C" {
    fn free(p: *mut c_void);
}

/// Finds a declared property whose kebab-case name is `js` (most derived class first).
pub fn find_property(cls: &AnyClass, js: &str) -> Option<(String, PropAttrs)> {
    for c in class_chain(cls) {
        let mut n: c_uint = 0;
        let list = unsafe { objc2::ffi::class_copyPropertyList(c, &mut n) };
        if list.is_null() {
            continue;
        }
        let mut found = None;
        for i in 0..n as usize {
            let p = unsafe { *list.add(i) };
            let name = unsafe { CStr::from_ptr(objc2::ffi::property_getName(p)) }.to_string_lossy();
            if names::kebab(&name) == js {
                let attrs = unsafe { CStr::from_ptr(objc2::ffi::property_getAttributes(p)) }.to_string_lossy();
                found = Some((name.into_owned(), enc::parse_property_attrs(&attrs)));
                break;
            }
        }
        unsafe { free(list as *mut c_void) };
        if found.is_some() {
            return found;
        }
    }
    None
}

/// A property known only through its accessor methods (declared in a protocol the class adopts,
/// such as `identifier` from NSUserInterfaceItemIdentification, which `class_copyPropertyList`
/// does not list): a zero-argument getter named `js` in kebab-case, writable when `set<Key>:` exists.
pub fn find_accessor(cls: &AnyClass, js: &str) -> Option<(String, PropAttrs)> {
    for c in class_chain(cls) {
        for m in c.instance_methods().iter() {
            let name = m.name().name().to_string_lossy().into_owned();
            if name.contains(':') || name.starts_with('_') || names::kebab(&name) != js {
                continue;
            }
            let ret = m.return_type().to_string_lossy().into_owned();
            let ty = enc::parse_type(&ret).0;
            if ty == Ty::Void {
                continue;
            }
            let setter = names::setter_for(&name);
            let writable = cls.instance_method(sel(&setter)).is_some();
            return Some((name, PropAttrs { ty, readonly: !writable, copy: false, weak: false, getter: None, setter: None }));
        }
    }
    None
}

/// Finds an instance method whose JS name (`names::selector_name`) is `js`. Among several
/// (`display` / `display:`) the one with fewer arguments wins.
pub fn find_selector(cls: &AnyClass, js: &str) -> Option<Sel> {
    let mut best: Option<(usize, Sel)> = None;
    for c in class_chain(cls) {
        for m in c.instance_methods().iter() {
            let s = m.name();
            let name = s.name().to_string_lossy();
            if name.starts_with('_') || names::selector_name(&name) != js {
                continue;
            }
            let arity = names::selector_arity(&name);
            if best.is_none_or(|(a, _)| arity < a) {
                best = Some((arity, s));
            }
        }
        if best.is_some() {
            break;
        }
    }
    best.map(|(_, s)| s)
}

/// How to map native objects to handles when converting values.
pub trait Objects {
    /// The object behind a handle (for `HANDLE` values passed to native code).
    fn object(&self, h: u64) -> Option<Retained<AnyObject>>;
    /// The handle of an object read from native code: its existing handle, or a new borrowed one.
    fn handle_of(&self, obj: &AnyObject) -> u64;
}

/// Boxes a JS value as the object KVC (or an object argument) expects for type `ty`.
pub fn box_value(v: &Value, ty: &Ty, members: Members, flags: bool, objs: &dyn Objects) -> Result<Option<Retained<AnyObject>>, String> {
    match ty {
        Ty::Bool | Ty::I(_) | Ty::U(_) | Ty::F32 | Ty::F64 => {
            let s = enc::scalar_in(v, ty, members, flags)?;
            let n = match (s, ty) {
                (Scalar::Bool(b), _) => NSNumber::new_bool(b),
                (Scalar::I64(i), Ty::Bool) => NSNumber::new_bool(i != 0),
                (Scalar::I64(i), _) => NSNumber::new_i64(i),
                (Scalar::U64(u), _) => NSNumber::new_u64(u),
                (Scalar::F64(f), _) => NSNumber::new_f64(f),
            };
            Ok(Some(erase(n)))
        }
        Ty::Struct(st) => {
            let native = enc::struct_in(v, st)?;
            let bytes = enc::struct_bytes(st, &native);
            let enc_c = CString::new(st.encoding()).unwrap_or_default();
            let val: Option<Retained<AnyObject>> =
                unsafe { msg_send![class!(NSValue), valueWithBytes: bytes.as_ptr() as *const c_void, objCType: enc_c.as_ptr()] };
            Ok(val)
        }
        Ty::Object(cls) => object_in(v, cls.as_deref(), objs),
        Ty::Sel => match v {
            Value::Str(_) => Err("selectors are set through `listen`, not as values".into()),
            _ => Err("expected a selector".into()),
        },
        other => Err(format!("values of native type {other:?} are not supported")),
    }
}

/// An object argument: `null` -> nil, a handle -> its object, a string -> `NSString` (or an
/// `NSAttributedString`-typed slot gets a plain `NSString`, which AppKit accepts for `stringValue`),
/// a number/bool -> `NSNumber`.
pub fn object_in(v: &Value, cls: Option<&str>, objs: &dyn Objects) -> Result<Option<Retained<AnyObject>>, String> {
    match v {
        Value::Null => Ok(None),
        Value::Handle(h) => objs.object(*h).map(Some).ok_or_else(|| format!("stale or foreign handle {h}")),
        Value::Str(s) => match cls {
            None | Some("NSString") | Some("NSMutableString") => Ok(Some(erase(nsstring(s)))),
            Some("NSAttributedString") => {
                let a: Option<Retained<AnyObject>> =
                    unsafe { msg_send![msg_send![class!(NSAttributedString), alloc], initWithString: &*nsstring(s)] };
                Ok(a)
            }
            Some("NSImage") => {
                // A named image or an SF Symbol name; styling (ST1) owns everything richer.
                let img: Option<Retained<AnyObject>> = unsafe { msg_send![class!(NSImage), imageNamed: &*nsstring(s)] };
                match img {
                    Some(i) => Ok(Some(i)),
                    None => {
                        let sym: Option<Retained<AnyObject>> = unsafe {
                            msg_send![class!(NSImage), imageWithSystemSymbolName: &*nsstring(s), accessibilityDescription: Option::<&AnyObject>::None]
                        };
                        sym.map(Some).ok_or_else(|| format!("no image named \"{s}\""))
                    }
                }
            }
            Some(c) => Err(format!("a string cannot be converted to {c}")),
        },
        Value::Bool(b) => Ok(Some(erase(NSNumber::new_bool(*b)))),
        Value::I64(i) => Ok(Some(erase(NSNumber::new_i64(*i)))),
        Value::U64(u) => Ok(Some(erase(NSNumber::new_u64(*u)))),
        Value::F64(f) => Ok(Some(erase(NSNumber::new_f64(*f)))),
        Value::Str16(w) => object_in(&Value::Str(String::from_utf16_lossy(w)), cls, objs),
        Value::Boxed { handle, .. } => objs.object(*handle).map(Some).ok_or_else(|| format!("stale or foreign value {handle}")),
        // NON-GUI types (N4): lists, maps, bytes and dates cross as Foundation collections and values.
        Value::List(items) => {
            let arr: Retained<AnyObject> = unsafe { msg_send![class!(NSMutableArray), arrayWithCapacity: items.len()] };
            for it in items {
                let o = match object_in(it, None, objs)? {
                    Some(o) => o,
                    None => unsafe { msg_send![class!(NSNull), null] },
                };
                let _: () = unsafe { msg_send![&*arr, addObject: &*o] };
            }
            Ok(Some(arr))
        }
        Value::Map(kv) => {
            let dict: Retained<AnyObject> = unsafe { msg_send![class!(NSMutableDictionary), dictionaryWithCapacity: kv.len()] };
            for (k, v) in kv {
                let o = match object_in(v, None, objs)? {
                    Some(o) => o,
                    None => unsafe { msg_send![class!(NSNull), null] },
                };
                let _: () = unsafe { msg_send![&*dict, setObject: &*o, forKey: &*nsstring(k)] };
            }
            Ok(Some(dict))
        }
        Value::Bytes(b) => {
            let d: Retained<AnyObject> = unsafe { msg_send![class!(NSData), dataWithBytes: b.as_ptr() as *const c_void, length: b.len()] };
            Ok(Some(d))
        }
        Value::DateTime(ms) => {
            let d: Retained<AnyObject> = unsafe { msg_send![class!(NSDate), dateWithTimeIntervalSince1970: *ms / 1000.0] };
            Ok(Some(d))
        }
        other => Err(format!("cannot convert {} to an object", enc::value_kind(other))),
    }
}

/// An `NSError` as the ABI's native error: its domain, its code and `localizedDescription`.
pub fn native_error(err: &AnyObject) -> svelters_abi::NativeError {
    let domain: Option<Retained<NSString>> = unsafe { msg_send![err, domain] };
    let code: isize = unsafe { msg_send![err, code] };
    let msg: Option<Retained<NSString>> = unsafe { msg_send![err, localizedDescription] };
    svelters_abi::NativeError {
        domain: domain.map(|d| d.to_string()).unwrap_or_default(),
        code: Value::I64(code as i64),
        message: msg.map(|m| m.to_string()).unwrap_or_default(),
    }
}

/// Converts an object read back from native code: strings, numbers, arrays (lists), values
/// (structs) and anything else as a handle.
pub fn object_out(obj: Option<&AnyObject>, objs: &dyn Objects) -> Value {
    let Some(obj) = obj else { return Value::Null };
    if let Some(s) = obj.downcast_ref::<NSString>() {
        return Value::Str(s.to_string());
    }
    if is_kind(obj, class!(NSAttributedString)) {
        let s: Option<Retained<NSString>> = unsafe { msg_send![obj, string] };
        return Value::Str(s.map(|s| s.to_string()).unwrap_or_default());
    }
    if let Some(n) = obj.downcast_ref::<NSNumber>() {
        let t: *const c_char = unsafe { msg_send![n, objCType] };
        let t = if t.is_null() { String::new() } else { unsafe { CStr::from_ptr(t) }.to_string_lossy().into_owned() };
        return match enc::parse_type(&t).0 {
            Ty::F32 | Ty::F64 => Value::F64(n.as_f64()),
            Ty::Bool => Value::Bool(n.as_bool()),
            Ty::U(8) => Value::U64(n.as_u64()),
            _ => Value::I64(n.as_i64()),
        };
    }
    if is_kind(obj, class!(NSArray)) {
        let count: usize = unsafe { msg_send![obj, count] };
        let mut out = Vec::with_capacity(count);
        for i in 0..count {
            let it: Option<Retained<AnyObject>> = unsafe { msg_send![obj, objectAtIndex: i] };
            out.push(object_out(it.as_deref(), objs));
        }
        return Value::List(out);
    }
    if is_kind(obj, class!(NSDictionary)) {
        let keys: Retained<AnyObject> = unsafe { msg_send![obj, allKeys] };
        let count: usize = unsafe { msg_send![&*keys, count] };
        let mut out = Vec::with_capacity(count);
        for i in 0..count {
            let k: Retained<AnyObject> = unsafe { msg_send![&*keys, objectAtIndex: i] };
            let v: Option<Retained<AnyObject>> = unsafe { msg_send![obj, objectForKey: &*k] };
            let ks: Retained<NSString> = unsafe { msg_send![&*k, description] };
            out.push((ks.to_string(), object_out(v.as_deref(), objs)));
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        return Value::Map(out);
    }
    if is_kind(obj, class!(NSSet)) || is_kind(obj, class!(NSOrderedSet)) {
        let arr: Retained<AnyObject> =
            if is_kind(obj, class!(NSSet)) { unsafe { msg_send![obj, allObjects] } } else { unsafe { msg_send![obj, array] } };
        return object_out(Some(&arr), objs);
    }
    if is_kind(obj, class!(NSData)) {
        let len: usize = unsafe { msg_send![obj, length] };
        let p: *const u8 = unsafe { msg_send![obj, bytes] };
        let b = if p.is_null() || len == 0 { Vec::new() } else { unsafe { std::slice::from_raw_parts(p, len) }.to_vec() };
        return Value::Bytes(b);
    }
    if is_kind(obj, class!(NSDate)) {
        let t: f64 = unsafe { msg_send![obj, timeIntervalSince1970] };
        return Value::DateTime(t * 1000.0);
    }
    if is_kind(obj, class!(NSNull)) {
        return Value::Null;
    }
    if is_kind(obj, class!(NSValue)) {
        let t: *const c_char = unsafe { msg_send![obj, objCType] };
        if !t.is_null() {
            let t = unsafe { CStr::from_ptr(t) }.to_string_lossy();
            if let Ty::Struct(st) = enc::parse_type(&t).0 {
                if st.fields() > 0 {
                    return struct_of(obj, &st);
                }
            }
        }
    }
    Value::Handle(objs.handle_of(obj))
}

fn struct_of(val: &AnyObject, st: &StructTy) -> Value {
    let mut buf = vec![0u8; st.fields() * 8];
    let _: () = unsafe { msg_send![val, getValue: buf.as_mut_ptr() as *mut c_void, size: buf.len()] };
    enc::struct_out(st, &enc::struct_from_bytes(st, &buf))
}

/// Converts a KVC result for a property of type `ty`.
pub fn unbox_value(obj: Option<&AnyObject>, ty: &Ty, members: Members, flags: bool, objs: &dyn Objects) -> Value {
    match ty {
        Ty::Bool | Ty::I(_) | Ty::U(_) | Ty::F32 | Ty::F64 => {
            let Some(n) = obj.and_then(|o| o.downcast_ref::<NSNumber>()) else { return Value::Null };
            let s = match ty {
                Ty::Bool => Scalar::Bool(n.as_bool()),
                Ty::I(_) => Scalar::I64(n.as_i64()),
                Ty::U(_) => Scalar::U64(n.as_u64()),
                _ => Scalar::F64(n.as_f64()),
            };
            enc::scalar_out(s, ty, members, flags)
        }
        Ty::Struct(st) if st.fields() > 0 => match obj {
            Some(o) if is_kind(o, class!(NSValue)) => struct_of(o, st),
            _ => Value::Null,
        },
        _ => object_out(obj, objs),
    }
}

/// `-[obj setValue:forKey:]`.
pub fn kvc_set(obj: &AnyObject, key: &str, v: Option<&AnyObject>) -> Result<(), String> {
    let k = nsstring(key);
    catch(key, || {
        let _: () = unsafe { msg_send![obj, setValue: v, forKey: &*k] };
    })
}

/// `-[obj valueForKey:]`.
pub fn kvc_get(obj: &AnyObject, key: &str) -> Result<Option<Retained<AnyObject>>, String> {
    let k = nsstring(key);
    catch(key, || unsafe { msg_send![obj, valueForKey: &*k] })
}

/// Sends a one-object-argument message (`addArrangedSubview:`), the shape of most slot ops.
pub fn send_obj(obj: &AnyObject, s: &str, arg: Option<&AnyObject>) -> Result<(), String> {
    let sl = sel(s);
    if !responds(obj, sl) {
        return Err(format!("{} does not respond to {s}", class_name_of(obj)));
    }
    catch(s, || unsafe {
        use objc2::runtime::MessageReceiver;
        let _: () = obj.send_message(sl, (arg,));
    })
}

/// Sends `obj sel` with no arguments and no result (`removeFromSuperview`).
pub fn send0(obj: &AnyObject, s: &str) -> Result<(), String> {
    let sl = sel(s);
    if !responds(obj, sl) {
        return Err(format!("{} does not respond to {s}", class_name_of(obj)));
    }
    catch(s, || unsafe {
        use objc2::runtime::MessageReceiver;
        let _: () = obj.send_message(sl, ());
    })
}

/// Calls any method through `NSInvocation`. Arguments are converted by the method signature;
/// missing trailing object arguments are `nil` (the `sender` of action methods).
pub fn invoke(obj: &AnyObject, s: Sel, args: &[Value], objs: &dyn Objects) -> Result<Value, String> {
    let sname = s.name().to_string_lossy().into_owned();
    if names::owning_family(&sname) {
        return Err(format!("{sname}: methods of the alloc/new/copy/init families are not callable (use create)"));
    }
    match invoke_ex(obj, s, args, objs, &Invoke::default()) {
        Ok(o) => Ok(o.value),
        Err(Failed::Msg(m)) => Err(m),
        Err(Failed::Native(e)) => Err(format!("{sname}: {}", e.message)),
    }
}

/// How [`invoke_ex`] treats the special arguments and the result.
#[derive(Default)]
pub struct Invoke {
    /// A built block (`blocks::completion`) for the argument at this index (`@?`); not a JS argument.
    pub block: Option<(usize, *mut c_void)>,
    /// The method ends in an `NSError **` out parameter (objc2's `Result`): the caller does not pass it,
    /// and a failure (NO / nil / a set error with a void result) is [`Failed::Native`].
    pub error_out: bool,
    /// The target is a freshly allocated object (`+alloc`) and the selector an initializer: the result
    /// is the initialized object (+1); the allocated one was consumed.
    pub init: bool,
}

/// A failed [`invoke_ex`]: a plain message, or the `NSError` the method reported.
pub enum Failed {
    Msg(String),
    Native(svelters_abi::NativeError),
}

impl From<String> for Failed {
    fn from(m: String) -> Failed {
        Failed::Msg(m)
    }
}

/// The result of [`invoke_ex`]: the converted value and, for an object result, the object itself.
pub struct Invoked {
    pub value: Value,
    pub obj: Option<Retained<AnyObject>>,
}

/// `NSInvocation` with blocks, `NSError **` outs and owning-family results (NON-GUI types, N4). `target`
/// may be a class object (class methods). Results of the new/copy/mutableCopy/init families are +1.
pub fn invoke_ex(target: &AnyObject, s: Sel, args: &[Value], objs: &dyn Objects, how: &Invoke) -> Result<Invoked, Failed> {
    let sname = s.name().to_string_lossy().into_owned();
    if !responds(target, s) {
        return Err(format!("{} does not respond to {sname}", class_name_of(target)).into());
    }
    let sig: Option<Retained<AnyObject>> = unsafe { msg_send![target, methodSignatureForSelector: s] };
    let sig = sig.ok_or_else(|| format!("{sname}: no method signature"))?;
    let nargs: usize = unsafe { msg_send![&*sig, numberOfArguments] };
    let user = nargs.saturating_sub(2);
    let hidden = usize::from(how.error_out) + usize::from(how.block.is_some());
    let visible = user.saturating_sub(hidden);
    if args.len() > visible {
        return Err(format!("{sname} takes {visible} argument(s), got {}", args.len()).into());
    }
    let inv: Retained<AnyObject> = unsafe { msg_send![class!(NSInvocation), invocationWithMethodSignature: &*sig] };
    let _: () = unsafe { msg_send![&*inv, setSelector: s] };
    let _: () = unsafe { msg_send![&*inv, setTarget: target] };
    // Buffers and boxed objects stay alive until the invocation returns.
    let mut keep: Vec<Option<Retained<AnyObject>>> = Vec::new();
    let mut bufs: Vec<Vec<u8>> = Vec::new();
    let mut err: *mut AnyObject = std::ptr::null_mut();
    let err_slot: *mut *mut AnyObject = &mut err;
    let mut next = 0usize;
    for i in 0..user {
        let t: *const c_char = unsafe { msg_send![&*sig, getArgumentTypeAtIndex: i + 2] };
        let t = unsafe { CStr::from_ptr(t) }.to_string_lossy().into_owned();
        let ty = enc::parse_type(&t).0;
        let bytes = if let Some((_, b)) = how.block.filter(|(at, _)| *at == i) {
            (b as usize).to_le_bytes().to_vec()
        } else if how.error_out && i + 1 == user {
            if ty != Ty::Pointer {
                return Err(format!("{sname}: the last argument is not an NSError ** out parameter ({t})").into());
            }
            (err_slot as usize).to_le_bytes().to_vec()
        } else {
            let a = args.get(next);
            next += 1;
            match (a, &ty) {
                (None, Ty::Object(_) | Ty::Block | Ty::Class) => (0usize).to_le_bytes().to_vec(),
                (None, _) => return Err(format!("{sname}: missing argument {} ({t})", i + 1).into()),
                (Some(v), Ty::Object(cls)) => {
                    let o = object_in(v, cls.as_deref(), objs)?;
                    let p = o.as_deref().map_or(std::ptr::null(), |o| o as *const AnyObject) as usize;
                    keep.push(o);
                    p.to_le_bytes().to_vec()
                }
                (Some(Value::Null), Ty::Class) => (0usize).to_le_bytes().to_vec(),
                (Some(Value::Str(name)), Ty::Class) => {
                    let c = class_named(name).ok_or_else(|| format!("{sname}: no class named {name}"))?;
                    (c as *const AnyClass as usize).to_le_bytes().to_vec()
                }
                (Some(v), Ty::Struct(st)) => enc::struct_bytes(st, &enc::struct_in(v, st)?),
                (Some(Value::Str(name)), Ty::Sel) => {
                    let s = sel(name);
                    (unsafe { std::mem::transmute::<Sel, usize>(s) }).to_le_bytes().to_vec()
                }
                (Some(v), Ty::Bool | Ty::I(_) | Ty::U(_) | Ty::F32 | Ty::F64) => enc::scalar_in(v, &ty, &[], false)?.bytes_for(&ty),
                (Some(_), other) => return Err(format!("{sname}: argument {} of type {other:?} is not supported", i + 1).into()),
            }
        };
        let mut b = bytes;
        b.resize(b.len().max(16), 0);
        bufs.push(b);
        let p = bufs.last_mut().unwrap().as_mut_ptr() as *mut c_void;
        let _: () = unsafe { msg_send![&*inv, setArgument: p, atIndex: (i + 2) as isize] };
    }
    catch(&sname, || {
        let _: () = unsafe { msg_send![&*inv, invoke] };
    })?;
    let rt: *const c_char = unsafe { msg_send![&*sig, methodReturnType] };
    let rt = unsafe { CStr::from_ptr(rt) }.to_string_lossy().into_owned();
    let rlen: usize = unsafe { msg_send![&*sig, methodReturnLength] };
    let ty = enc::parse_type(&rt).0;
    let mut ret = vec![0u8; rlen.max(16)];
    if ty != Ty::Void && rlen > 0 {
        let _: () = unsafe { msg_send![&*inv, getReturnValue: ret.as_mut_ptr() as *mut c_void] };
    }
    drop(keep);
    let owned = how.init || names::owning_family(&sname);
    let ptr = usize::from_le_bytes(ret[..8].try_into().unwrap()) as *mut AnyObject;
    let boolish = matches!(ty, Ty::Bool | Ty::I(1));
    if how.error_out {
        let failed = match &ty {
            Ty::Void => !err.is_null(),
            _ if boolish => ret[0] == 0,
            Ty::Object(_) => ptr.is_null(),
            _ => false,
        };
        if failed {
            // The error is autoreleased (+0): retain it while converting.
            return Err(match unsafe { Retained::retain(err) } {
                Some(e) => Failed::Native(native_error(&e)),
                None => Failed::Msg(format!("{sname} failed")),
            });
        }
        if boolish {
            // objc2's `Result<(), NSError>`: YES is success with no value.
            return Ok(Invoked { value: Value::Null, obj: None });
        }
    }
    if ty == Ty::Void || rlen == 0 {
        return Ok(Invoked { value: Value::Null, obj: None });
    }
    Ok(match &ty {
        Ty::Object(_) => {
            // +1 for the owning families, else +0 (retained while converting).
            let o = if owned { unsafe { Retained::from_raw(ptr) } } else { unsafe { Retained::retain(ptr) } };
            Invoked { value: object_out(o.as_deref(), objs), obj: o }
        }
        Ty::Struct(st) if st.fields() > 0 => Invoked { value: enc::struct_out(st, &enc::struct_from_bytes(st, &ret)), obj: None },
        Ty::Bool | Ty::I(_) | Ty::U(_) | Ty::F32 | Ty::F64 => Invoked {
            value: match enc::scalar_from_bytes(&ty, &ret) {
                Some(s) => enc::scalar_out(s, &ty, &[], false),
                None => Value::Null,
            },
            obj: None,
        },
        Ty::Class => {
            let c = ptr as *const AnyClass;
            let v = if c.is_null() { Value::Null } else { Value::Str(unsafe { &*c }.name().to_string_lossy().into_owned()) };
            Invoked { value: v, obj: None }
        }
        Ty::CStr => {
            let p = ptr as *const c_char;
            let v = if p.is_null() { Value::Null } else { Value::Str(unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()) };
            Invoked { value: v, obj: None }
        }
        other => return Err(format!("{sname}: result type {other:?} is not supported").into()),
    })
}
