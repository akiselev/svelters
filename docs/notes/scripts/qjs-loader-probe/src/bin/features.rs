//! Which ES / QuickJS-NG features does rquickjs 0.14 (QuickJS-NG 0.16.2+26) give the host?
//! Prints one line per probe. Run: `features`.
use rquickjs::{CatchResultExt, Context, Runtime};

const PROBES: &[(&str, &str)] = &[
    ("Error.prepareStackTrace callsites", r#"
        Error.prepareStackTrace = (e, cs) => cs.map(c => `${c.getFileName()}:${c.getLineNumber()}:${c.getColumnNumber()} ${c.getFunctionName()}`).join('|');
        function inner() { return new Error('x').stack } inner()"#),
    ("Error.captureStackTrace", "const o = {}; Error.captureStackTrace(o); typeof o.stack"),
    ("Error.stackTraceLimit", "String(Error.stackTraceLimit)"),
    ("WeakRef + FinalizationRegistry", "typeof WeakRef + ' ' + typeof FinalizationRegistry"),
    ("SharedArrayBuffer / Atomics", "typeof SharedArrayBuffer + ' ' + typeof Atomics"),
    ("Intl", "typeof Intl"),
    ("toLocaleString(de-DE)", "(1234567.891).toLocaleString('de-DE') + ' ' + new Date(0).toLocaleDateString('de-DE')"),
    ("Iterator helpers", "[1,2,3].values().map(x => x*2).toArray().join()"),
    ("Float16Array", "typeof Float16Array"),
    ("Array.fromAsync", "typeof Array.fromAsync"),
    ("Promise.withResolvers", "typeof Promise.withResolvers"),
    ("using / DisposableStack", "typeof DisposableStack + ' ' + typeof Symbol.dispose"),
    ("RegExp v flag", "/[\\p{L}--[a-z]]/v.test('A')"),
    ("Set methods", "new Set([1,2]).union(new Set([3])).size"),
    ("structuredClone", "typeof structuredClone"),
    ("queueMicrotask", "typeof queueMicrotask"),
    ("performance.now", "typeof performance"),
    ("BigInt", "String(2n ** 64n)"),
    ("Temporal", "typeof Temporal"),
    ("private fields + static blocks", "class A { #x = 1; static { this.y = 2 } get x() { return this.#x } } new A().x + A.y"),
    ("Proxy deep", "const p = new Proxy({}, { get: (t,k) => k }); p.hello"),
    ("Math.sumPrecise / Error.isError", "typeof Math.sumPrecise + ' ' + typeof Error.isError"),
];

fn main() {
    let rt = Runtime::new().unwrap();
    let ctx = Context::full(&rt).unwrap();
    ctx.with(|ctx| {
        for (name, src) in PROBES {
            let r: Result<String, _> = ctx.eval::<rquickjs::Value, _>(*src).and_then(|v| {
                v.get::<rquickjs::Coerced<String>>().map(|c| c.0)
            }).catch(&ctx);
            match r {
                Ok(v) => println!("{name:36} {v}"),
                Err(e) => println!("{name:36} ERROR {e}"),
            }
        }
    });
    // FinalizationRegistry callback after a full GC: is the cleanup job queued?
    ctx.with(|ctx| {
        ctx.eval::<(), _>("globalThis.fired = 0; globalThis.fr = new FinalizationRegistry(() => fired++); (() => { fr.register({}, 1) })()").unwrap();
    });
    rt.run_gc();
    while rt.execute_pending_job().unwrap_or(false) {}
    ctx.with(|ctx| println!("{:36} {}", "FinalizationRegistry fires after run_gc", ctx.eval::<i32, _>("fired").unwrap()));
}
