//! W-J0 (plans/research/presto/compiler-and-js.md §5.6 step 5): the JS→Rust crossing on stock
//! QuickJS-NG, per call, for the binding shapes the fork question weighs:
//!
//! - an rquickjs `Function::new` closure (today's host ops);
//! - a raw C function with `magic` (W-J1's fast ops: `quickjs/fast.rs`);
//! - an exotic class whose `set_property` hook catches `node.label = v` (option (c1));
//! - each with a number and with a short ASCII string argument;
//! - the runtime's JS-side last-value dedupe (`Map` lookups before the call) against none.
//!
//! Every body is a no-op that reads its arguments as the real op would, so the numbers are the
//! crossing plus argument reading. Run on a quiet machine, release build:
//! `cargo bench -p svelters-host --bench boundary` (prints ns per call; no assertions).

use rquickjs::{Context, Ctx, Function, Object, Runtime, qjs};
use std::ffi::c_int;
use std::time::Instant;

const N: u32 = 2_000_000;

unsafe extern "C" fn raw_op(ctx: *mut qjs::JSContext, _this: qjs::JSValue, _argc: c_int, argv: *mut qjs::JSValue, _magic: c_int) -> qjs::JSValue {
    let mut acc = 0.0;
    for i in 0..3 {
        let v = unsafe { *argv.add(i) };
        unsafe {
            let tag = qjs::JS_VALUE_GET_TAG(v);
            if tag == qjs::JS_TAG_INT {
                acc += qjs::JS_VALUE_GET_INT(v) as f64;
            } else if qjs::JS_TAG_IS_FLOAT64(tag) {
                acc += qjs::JS_VALUE_GET_FLOAT64(v);
            } else if qjs::JS_IsString(v) {
                let mut len: qjs::size_t = 0;
                let s = qjs::JS_ToCStringLen2(ctx, &mut len, v, false);
                acc += len as f64;
                qjs::JS_FreeCString(ctx, s);
            }
        }
    }
    std::hint::black_box(acc);
    qjs::JS_UNDEFINED
}

unsafe extern "C" fn exotic_set(ctx: *mut qjs::JSContext, _obj: qjs::JSValue, atom: qjs::JSAtom, value: qjs::JSValue, _recv: qjs::JSValue, _flags: c_int) -> c_int {
    let _ = ctx;
    std::hint::black_box((atom, unsafe { qjs::JS_VALUE_GET_TAG(value) }));
    1
}

fn install<'js>(ctx: &Ctx<'js>) -> rquickjs::Result<()> {
    let g = ctx.globals();
    g.set("closure", Function::new(ctx.clone(), |a: f64, b: f64, c: rquickjs::Value| {
        std::hint::black_box((a, b, c.as_string().map(|s| s.to_string().map(|s| s.len()))));
    })?)?;
    let raw = ctx.as_raw().as_ptr();
    let ty = qjs::JSCFunctionType { generic_magic: Some(raw_op) };
    let f = unsafe { qjs::JS_NewCFunction2(raw, ty.generic, c"raw".as_ptr(), 3, qjs::JSCFunctionEnum_JS_CFUNC_generic_magic, 7) };
    g.set("raw", unsafe { rquickjs::Value::from_raw(ctx.clone(), f) })?;
    // the exotic class (option (c1)): `node.label = v` lands in `exotic_set`
    let rt = unsafe { qjs::JS_GetRuntime(raw) };
    let mut class: qjs::JSClassID = 0;
    unsafe { qjs::JS_NewClassID(rt, &mut class) };
    let exotic = Box::leak(Box::new(unsafe { std::mem::zeroed::<qjs::JSClassExoticMethods>() }));
    exotic.set_property = Some(exotic_set);
    let def = qjs::JSClassDef { class_name: c"BenchNode".as_ptr(), finalizer: None, gc_mark: None, call: None, exotic };
    unsafe { qjs::JS_NewClass(rt, class, &def) };
    let obj = unsafe { qjs::JS_NewObjectClass(raw, class) };
    g.set("node", unsafe { rquickjs::Value::from_raw(ctx.clone(), obj) })?;
    let _: Object = g.get("node")?;
    Ok(())
}

fn time(ctx: &Ctx<'_>, name: &str, body: &str) {
    let src = format!("(n) => {{ const last = new Map(); let x = 0; for (let i = 0; i < n; i++) {{ {body} }} return x; }}");
    let f: Function = ctx.eval(src).expect("bench source");
    let _: rquickjs::Value = f.call((N / 10,)).expect("warm-up");
    let t = Instant::now();
    let _: rquickjs::Value = f.call((N,)).expect("run");
    println!("{name:<40} {:>8.1} ns/call", t.elapsed().as_nanos() as f64 / N as f64);
}

fn main() {
    let rt = Runtime::new().unwrap();
    let cx = Context::full(&rt).unwrap();
    cx.with(|ctx| {
        install(&ctx).unwrap();
        time(&ctx, "empty loop", "x += i;");
        time(&ctx, "closure (3 numbers)", "closure(1, i, 2);");
        time(&ctx, "raw C function (3 numbers)", "raw(1, i, 2);");
        time(&ctx, "exotic set (number)", "node.label = i;");
        time(&ctx, "closure (string)", "closure(1, 2, 'label');");
        time(&ctx, "raw C function (string)", "raw(1, 2, 'label');");
        time(&ctx, "exotic set (string)", "node.label = 'label';");
        time(&ctx, "Map dedupe, changed, then raw", "const v = i; if (!(last.has('label') && last.get('label') === v)) { last.set('label', v); raw(1, v, 2); }");
        time(&ctx, "Map dedupe, unchanged (no call)", "const v = 5; if (!(last.has('label') && last.get('label') === v)) { last.set('label', v); raw(1, v, 2); }");
        time(&ctx, "inline compare, unchanged (no call)", "const v = 5; if (v !== x) { x = v; raw(1, v, 2); }");
    });
}
