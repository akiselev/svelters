//! serdes micro-benchmarks. Part A: the real svelters-host path on dummy backends.
//! Part B: prototype bindings on a bare rquickjs context.
#![allow(non_camel_case_types, clippy::missing_safety_doc)]

use rquickjs::{
    ArrayBuffer, Array, Atom, Context, Ctx, Function, Object, Persistent, Runtime, Value,
    qjs,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{c_char, c_int, c_void};
use std::hint::black_box;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use svelters_abi::*;

static SINK: AtomicU64 = AtomicU64::new(0);
static NEXT: AtomicU64 = AtomicU64::new(1);

fn sink(x: u64) {
    SINK.fetch_add(x, Ordering::Relaxed);
}

unsafe fn consume(v: &svl_value) {
    unsafe {
        match v.tag {
            SVL_BOOL => sink(v.u.b as u64),
            SVL_I64 => sink(v.u.i as u64),
            SVL_F64 => sink(v.u.f as u64),
            SVL_HANDLE => sink(v.u.h),
            SVL_STR => {
                let s = v.u.s;
                sink(s.len as u64 + if s.len > 0 { *s.ptr as u8 as u64 } else { 0 })
            }
            SVL_LIST => {
                for i in 0..v.u.l.len {
                    consume(&*v.u.l.ptr.add(i));
                }
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------- raw C-ABI dummy backend (A2)

unsafe extern "C" fn r_resolve(_: *mut c_void, _what: u32, _k: svl_id, _n: *const c_char, len: usize) -> svl_id {
    len as u32 + 1
}
unsafe extern "C" fn r_create(_: *mut c_void, _k: svl_id) -> svl_handle {
    NEXT.fetch_add(1, Ordering::Relaxed)
}
unsafe extern "C" fn r_insert(_: *mut c_void, _: svl_handle, _: svl_handle, _: svl_handle, _: svl_id) -> c_int {
    0
}
unsafe extern "C" fn r_remove(_: *mut c_void, _: svl_handle, _: svl_handle) -> c_int {
    0
}
unsafe extern "C" fn r_destroy(_: *mut c_void, _: svl_handle) {}
unsafe extern "C" fn r_set_prop(_: *mut c_void, h: svl_handle, p: svl_id, v: *const svl_value) -> c_int {
    unsafe { consume(&*v) };
    sink(h + p as u64);
    0
}
unsafe extern "C" fn r_get_prop(_: *mut c_void, _: svl_handle, p: svl_id, out: *mut svl_value) -> c_int {
    unsafe {
        (*out).tag = SVL_I64;
        (*out).u.i = p as i64;
    }
    0
}
unsafe extern "C" fn r_call(_: *mut c_void, _: svl_handle, _: svl_id, a: *const svl_value, n: usize, out: *mut svl_value) -> c_int {
    unsafe {
        for i in 0..n {
            consume(&*a.add(i));
        }
        (*out).tag = SVL_NULL;
    }
    0
}
unsafe extern "C" fn r_listen(_: *mut c_void, _: svl_handle, _: svl_id, _: bool) -> c_int {
    0
}
unsafe extern "C" fn r_run(_: *mut c_void, _: *const svl_host) -> c_int {
    0
}
unsafe extern "C" fn r_wake(_: *mut c_void) {}
unsafe extern "C" fn r_quit(_: *mut c_void) {}
unsafe extern "C" fn r_last_error(_: *mut c_void) -> *const c_char {
    c"".as_ptr()
}

fn raw_backend() -> svl_backend {
    svl_backend {
        abi_version: SVL_ABI_VERSION,
        self_: std::ptr::null_mut(),
        resolve: r_resolve,
        create: r_create,
        insert: r_insert,
        remove: r_remove,
        destroy: r_destroy,
        set_prop: r_set_prop,
        get_prop: r_get_prop,
        call: r_call,
        listen: r_listen,
        run: r_run,
        wake: r_wake,
        quit: r_quit,
        last_error: r_last_error,
    }
}

// ---------------------------------------------------------------- Rust-trait dummy backend (A1)

struct TraitDummy;
fn consume_value(v: &svelters_abi::Value) {
    match v {
        svelters_abi::Value::I64(i) => sink(*i as u64),
        svelters_abi::Value::F64(f) => sink(*f as u64),
        svelters_abi::Value::Str(s) => sink(s.len() as u64),
        svelters_abi::Value::List(l) => l.iter().for_each(consume_value),
        svelters_abi::Value::Bool(b) => sink(*b as u64),
        _ => {}
    }
}
impl Backend for TraitDummy {
    fn resolve(&self, _w: NameKind, _k: svl_id, name: &str) -> svl_id {
        name.len() as u32 + 1
    }
    fn create(&self, _k: svl_id) -> StrResult<svl_handle> {
        Ok(NEXT.fetch_add(1, Ordering::Relaxed))
    }
    fn insert(&self, _: svl_handle, _: svl_handle, _: Option<svl_handle>, _: svl_id) -> StrResult<()> {
        Ok(())
    }
    fn remove(&self, _: svl_handle, _: svl_handle) -> StrResult<()> {
        Ok(())
    }
    fn destroy(&self, _: svl_handle) {}
    fn set_prop(&self, _h: svl_handle, _p: svl_id, v: &svelters_abi::Value) -> StrResult<()> {
        consume_value(v);
        Ok(())
    }
    fn get_prop(&self, _h: svl_handle, p: svl_id) -> StrResult<svelters_abi::Value> {
        Ok(svelters_abi::Value::I64(p as i64))
    }
    fn call(&self, _h: svl_handle, _m: svl_id, a: &[svelters_abi::Value]) -> StrResult<svelters_abi::Value> {
        a.iter().for_each(consume_value);
        Ok(svelters_abi::Value::Null)
    }
    fn listen(&self, _: svl_handle, _: svl_id, _: bool) -> StrResult<()> {
        Ok(())
    }
    fn run(&self, _host: &HostRef<'_>) -> StrResult<()> {
        Ok(())
    }
    fn waker(&self) -> Waker {
        std::sync::Arc::new(|| {})
    }
    fn quit(&self) {}
}

fn part_a(label: &str, b: BackendRef<'_>, js: &str) {
    let cfg = svelters_host::HostConfig {
        turn_budget: None,
        echo_console: false,
        ..Default::default()
    };
    let host = svelters_host::Host::new(b, cfg).expect("host");
    host.load_bundle(js).expect("bundle");
    for l in host.take_console() {
        println!("{label}\t{}", l.text);
    }
    for e in host.take_errors() {
        println!("ERR {e}");
    }
}

// ---------------------------------------------------------------- Part B: v2 value model prototype

pub const V_NULL: u32 = 0;
pub const V_BOOL: u32 = 1;
pub const V_I64: u32 = 2;
pub const V_F64: u32 = 3;
pub const V_STR: u32 = 4;
pub const V_HANDLE: u32 = 5;
pub const V_LIST: u32 = 6;
pub const V_RECORD: u32 = 7;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Span {
    ptr: *const c_void,
    len: usize,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub union U2 {
    b: bool,
    i: i64,
    f: f64,
    h: u64,
    span: Span,
}
/// Prototype of the v2 value: 24 bytes like v1, plus a 32-bit `type_id` (enum/record/boxed type).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct V2 {
    tag: u16,
    flags: u16,
    type_id: u32,
    u: U2,
}
impl V2 {
    fn i(x: i64) -> V2 {
        V2 { tag: V_I64 as u16, flags: 0, type_id: 0, u: U2 { i: x } }
    }
    fn f(x: f64) -> V2 {
        V2 { tag: V_F64 as u16, flags: 0, type_id: 0, u: U2 { f: x } }
    }
}

type SetFn = unsafe extern "C" fn(*mut c_void, u64, u32, *const V2) -> c_int;

unsafe fn consume2(v: &V2) {
    unsafe {
        match v.tag as u32 {
            V_I64 => sink(v.u.i as u64),
            V_F64 => sink(v.u.f as u64),
            V_BOOL => sink(v.u.b as u64),
            V_STR => sink(v.u.span.len as u64 + if v.u.span.len > 0 { *(v.u.span.ptr as *const u8) as u64 } else { 0 }),
            V_LIST | V_RECORD => {
                let p = v.u.span.ptr as *const V2;
                for i in 0..v.u.span.len {
                    consume2(&*p.add(i));
                }
            }
            _ => {}
        }
    }
}

#[inline(never)]
unsafe extern "C" fn b_set_prop(_: *mut c_void, h: u64, p: u32, v: *const V2) -> c_int {
    unsafe { consume2(&*v) };
    sink(h + p as u64);
    0
}

const RECT_TYPE: u32 = 77;

struct Cache {
    // ids pre-resolved by the JS side; this table says which props are typed records
    rect_props: Vec<u32>,
}

/// Converts with a borrowed C string: ASCII-only 8-bit JS strings come back zero-copy.
unsafe fn with_v2<'js, R>(ctx: &Ctx<'js>, v: &Value<'js>, f: impl FnOnce(&V2) -> R) -> rquickjs::Result<R> {
    if let Some(i) = v.as_int() {
        return Ok(f(&V2::i(i as i64)));
    }
    if let Some(x) = v.as_float() {
        return Ok(f(&V2::f(x)));
    }
    if let Some(b) = v.as_bool() {
        return Ok(f(&V2 { tag: V_BOOL as u16, flags: 0, type_id: 0, u: U2 { b } }));
    }
    if v.is_string() {
        let mut len: u64 = 0;
        let ptr = unsafe { qjs::JS_ToCStringLen2(ctx.as_raw().as_ptr(), &mut len, v.as_raw(), false) };
        let r = f(&V2 { tag: V_STR as u16, flags: 0, type_id: 0, u: U2 { span: Span { ptr: ptr.cast(), len: len as usize } } });
        unsafe { qjs::JS_FreeCString(ctx.as_raw().as_ptr(), ptr) };
        return Ok(r);
    }
    if let Some(arr) = v.as_array() {
        let mut items: [V2; 8] = [V2::i(0); 8];
        let n = arr.len().min(8);
        for i in 0..n {
            let x: Value = arr.get(i)?;
            items[i] = if let Some(k) = x.as_int() { V2::i(k as i64) } else { V2::f(x.as_float().unwrap_or(0.0)) };
        }
        return Ok(f(&V2 { tag: V_LIST as u16, flags: 0, type_id: 0, u: U2 { span: Span { ptr: items.as_ptr().cast(), len: n } } }));
    }
    Ok(f(&V2 { tag: V_NULL as u16, flags: 0, type_id: 0, u: U2 { i: 0 } }))
}

fn num(v: &Value<'_>) -> f64 {
    v.as_int().map(|i| i as f64).or_else(|| v.as_float()).unwrap_or(f64::NAN)
}

type Reg = std::rc::Rc<RefCell<(u32, HashMap<u32, Persistent<Function<'static>>>)>>;

fn install<'js>(ctx: Ctx<'js>, setfn: SetFn, base: Instant, reg: Reg) {
        let b = Object::new(ctx.clone()).unwrap();
        let cache = std::rc::Rc::new(Cache { rect_props: vec![3] });
        // atoms for the record fields, created once (what generated code would do)
        let ax = Atom::from_str(ctx.clone(), "x").unwrap();
        let ay = Atom::from_str(ctx.clone(), "y").unwrap();
        let aw = Atom::from_str(ctx.clone(), "width").unwrap();
        let ah = Atom::from_str(ctx.clone(), "height").unwrap();
        let atoms = std::rc::Rc::new([ax, ay, aw, ah].map(|a| Persistent::save(&ctx, a)));
        b.set("now", Function::new(ctx.clone(), move || base.elapsed().as_secs_f64() * 1000.0).unwrap()).unwrap();
        b.set("log", Function::new(ctx.clone(), |s: String| println!("{s}")).unwrap()).unwrap();
        b.set("noop", Function::new(ctx.clone(), |h: f64, id: u32, _v: Value| { sink(h as u64 + id as u64) }).unwrap()).unwrap();
        {
            let cache = cache.clone();
            let atoms = atoms.clone();
            b.set(
                "setPropId",
                Function::new(ctx.clone(), move |ctx: Ctx<'js>, h: f64, id: u32, v: Value<'js>| -> rquickjs::Result<()> {
                    if cache.rect_props.contains(&id)
                        && let Some(o) = v.as_object()
                        && v.as_array().is_none()
                    {
                        // typed record conversion keyed by the prop's declared type (generated in M3)
                        let mut f = [V2::f(0.0); 4];
                        for (k, a) in atoms.iter().enumerate() {
                            let a = a.clone().restore(&ctx)?;
                            let x: Value = o.get(a)?;
                            f[k] = V2::f(num(&x));
                        }
                        let rec = V2 { tag: V_RECORD as u16, flags: 0, type_id: RECT_TYPE, u: U2 { span: Span { ptr: f.as_ptr().cast(), len: 4 } } };
                        unsafe { setfn(std::ptr::null_mut(), h as u64, id, &rec) };
                        return Ok(());
                    }
                    unsafe { with_v2(&ctx, &v, |x| setfn(std::ptr::null_mut(), h as u64, id, x)) }?;
                    Ok(())
                })
                .unwrap(),
            )
            .unwrap();
        }
        b.set("strUtf8", Function::new(ctx.clone(), move |ctx: Ctx<'js>, v: Value<'js>| {
            let mut len: u64 = 0;
            let p = unsafe { qjs::JS_ToCStringLen2(ctx.as_raw().as_ptr(), &mut len, v.as_raw(), false) };
            sink(len + unsafe { *p } as u64);
            unsafe { qjs::JS_FreeCString(ctx.as_raw().as_ptr(), p) };
        }).unwrap()).unwrap();
        b.set("strUtf16", Function::new(ctx.clone(), move |ctx: Ctx<'js>, v: Value<'js>| {
            let mut len: u64 = 0;
            let p = unsafe { qjs::JS_ToCStringLenUTF16(ctx.as_raw().as_ptr(), &mut len, v.as_raw()) };
            sink(len + unsafe { *p } as u64);
            unsafe { qjs::JS_FreeCStringUTF16(ctx.as_raw().as_ptr(), p) };
        }).unwrap()).unwrap();
        b.set("strRust", Function::new(ctx.clone(), move |s: String| sink(s.len() as u64)).unwrap()).unwrap();
        b.set(
            "setRect4",
            Function::new(ctx.clone(), move |h: f64, id: u32, x: f64, y: f64, w: f64, hh: f64| {
                let f = [V2::f(x), V2::f(y), V2::f(w), V2::f(hh)];
                let rec = V2 { tag: V_RECORD as u16, flags: 0, type_id: RECT_TYPE, u: U2 { span: Span { ptr: f.as_ptr().cast(), len: 4 } } };
                unsafe { setfn(std::ptr::null_mut(), h as u64, id, &rec) };
            })
            .unwrap(),
        )
        .unwrap();
        b.set(
            "flush",
            Function::new(ctx.clone(), move |ctx: Ctx<'js>, ab: ArrayBuffer<'js>, n: usize, strs: Array<'js>| -> rquickjs::Result<()> {
                let raw = ab.as_raw().expect("live buffer");
                let words = unsafe { std::slice::from_raw_parts(raw.as_ptr() as *const f64, n) };
                let mut i = 0;
                while i < n {
                    let op = words[i] as u32;
                    let h = words[i + 1] as u64;
                    let id = words[i + 2] as u32;
                    match op {
                        1 => {
                            unsafe { setfn(std::ptr::null_mut(), h, id, &V2::i(words[i + 3] as i64)) };
                            i += 4;
                        }
                        2 => {
                            let s: Value = strs.get(words[i + 3] as usize)?;
                            unsafe { with_v2(&ctx, &s, |x| setfn(std::ptr::null_mut(), h, id, x)) }?;
                            i += 4;
                        }
                        3 => {
                            let f = [V2::f(words[i + 3]), V2::f(words[i + 4]), V2::f(words[i + 5]), V2::f(words[i + 6])];
                            let rec = V2 { tag: V_RECORD as u16, flags: 0, type_id: RECT_TYPE, u: U2 { span: Span { ptr: f.as_ptr().cast(), len: 4 } } };
                            unsafe { setfn(std::ptr::null_mut(), h, id, &rec) };
                            i += 7;
                        }
                        _ => panic!("bad op {op}"),
                    }
                }
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();
        b.set(
            "flush16",
            Function::new(ctx.clone(), move |ab: ArrayBuffer<'js>, n: usize, sb: ArrayBuffer<'js>, _sn: usize| -> rquickjs::Result<()> {
                let raw = ab.as_raw().expect("live");
                let words = unsafe { std::slice::from_raw_parts(raw.as_ptr() as *const f64, n) };
                let sraw = sb.as_raw().expect("live");
                let units = sraw.as_ptr() as *const u16;
                let mut i = 0;
                while i < n {
                    let (h, id, off, len) = (words[i + 1] as u64, words[i + 2] as u32, words[i + 3] as usize, words[i + 4] as usize);
                    // A UTF-16 consumer (Qt) would take these code units directly; GTK would transcode.
                    let v = V2 { tag: V_STR as u16, flags: 1, type_id: 0, u: U2 { span: Span { ptr: unsafe { units.add(off) }.cast(), len } } };
                    unsafe { setfn(std::ptr::null_mut(), h, id, &v) };
                    i += 5;
                }
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();
        // callback registry: JS function -> persistent root keyed by id (what a native closure would hold)
        {
            let reg = reg.clone();
            b.set(
                "regFn",
                Function::new(ctx.clone(), move |ctx: Ctx<'js>, f: Function<'js>| -> u32 {
                    let mut r = reg.borrow_mut();
                    r.0 += 1;
                    let id = r.0;
                    r.1.insert(id, Persistent::save(&ctx, f));
                    id
                })
                .unwrap(),
            )
            .unwrap();
        }
        {
            let reg2 = reg.clone();
            b.set("unregFn", Function::new(ctx.clone(), move |id: u32| { reg2.borrow_mut().1.remove(&id); }).unwrap()).unwrap();
        }
        ctx.globals().set("B", b).unwrap();
}

fn part_b(js: &str) {
    let rt = Runtime::new().unwrap();
    let ctx = Context::full(&rt).unwrap();
    let setfn: SetFn = black_box(b_set_prop);
    let base = Instant::now();
    let reg: Reg = Default::default();
    ctx.with(|ctx| {
        install(ctx.clone(), setfn, base, reg.clone());
        let src = std::fs::read_to_string(js).unwrap();
        if let Err(e) = ctx.eval::<(), _>(src) {
            let ex = ctx.catch();
            println!("ERR {e}: {ex:?}");
        }
        reg.borrow_mut().1.clear();
    });
}

fn main() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("js");
    if let Some(p) = std::env::args().nth(1) { part_b(&p); return; }
    let a = dir.join("a.js");
    let raw = raw_backend();
    part_a("A2 raw-C-ABI-backend", unsafe { BackendRef::from_raw(&raw).unwrap() }, a.to_str().unwrap());
    let vt = BackendVtable::new(TraitDummy);
    part_a("A1 rust-trait-backend", vt.backend_ref(), a.to_str().unwrap());
    part_b(dir.join("b.js").to_str().unwrap());
    eprintln!("sink {}", SINK.load(Ordering::Relaxed));
}
