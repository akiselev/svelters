//! Part A: bind straight to rquickjs (no svelters host, no C ABI).
//!
//! - `#[rquickjs::class]` call cost vs a plain JS function;
//! - zero-copy `Float32Array` over a Rust `Arc<[f32]>` (QuickJS-NG immutable ArrayBuffer) vs a copy;
//! - the `Arc` is released when JS collects the view;
//! - `ArrayBuffer.prototype.transfer()` on a JS-allocated buffer: does it copy?
//! - a JS-allocated buffer moved to a rayon worker (transfer, raw pointer, in-place kernel) and handed
//!   back to JS by resolving a Promise from a completion queue drained on the JS thread;
//! - a Rust-allocated buffer that JS filled, reclaimed by Rust without a copy (detach → our drop hook).

use rquickjs::ArrayBufferSource;
use rquickjs::class::Trace;
use rquickjs::function::This;
use rquickjs::{
    ArrayBuffer, CatchResultExt, Class, Context, Ctx, Function, JsLifetime, Persistent, Runtime,
    TypedArray,
};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::mpsc;
use std::sync::{Arc, OnceLock};
use std::time::Instant;

/// An `Arc<[f32]>` as an ArrayBuffer backing store (read-only: used with `from_source_immutable`).
struct ArcF32(Arc<[f32]>);
// SAFETY: the pointer is the Arc's heap slice, valid until the Arc clone is dropped, and moves of
// `ArcF32` do not move the heap allocation. JS only reads it (immutable ArrayBuffer).
unsafe impl ArrayBufferSource for ArcF32 {
    fn as_ptr(&self) -> *mut u8 {
        self.0.as_ptr() as *mut u8
    }
    fn len(&self) -> usize {
        self.0.len() * 4
    }
}

thread_local! {
    static WANT_RECLAIM: Cell<bool> = const { Cell::new(false) };
    static RECLAIMED: RefCell<Option<Vec<f32>>> = const { RefCell::new(None) };
}

/// A Rust `Vec<f32>` that JS may fill; on detach-for-reclaim its Drop hands the Vec back instead of freeing.
struct Reclaimable(Option<Vec<f32>>);
// SAFETY: the Vec's heap buffer does not move while the Vec is not touched; it is only touched in Drop.
unsafe impl ArrayBufferSource for Reclaimable {
    fn as_ptr(&self) -> *mut u8 {
        self.0.as_ref().map_or(std::ptr::null_mut(), |v| v.as_ptr() as *mut u8)
    }
    fn len(&self) -> usize {
        self.0.as_ref().map_or(0, |v| v.len() * 4)
    }
}
impl Drop for Reclaimable {
    fn drop(&mut self) {
        if WANT_RECLAIM.with(|w| w.get()) {
            RECLAIMED.with(|r| *r.borrow_mut() = self.0.take());
        }
    }
}

#[derive(Trace, JsLifetime)]
#[rquickjs::class(rename = "Sim")]
pub struct Sim {
    #[qjs(skip_trace)]
    acc: f64,
    #[qjs(get, set)]
    gain: f64,
    #[qjs(skip_trace)]
    samples: Arc<[f32]>,
}

#[rquickjs::methods]
impl Sim {
    #[qjs(constructor)]
    pub fn new(n: usize) -> Sim {
        Sim { acc: 0.0, gain: 1.0, samples: (0..n).map(|i| i as f32).collect::<Vec<_>>().into() }
    }
    pub fn add(&mut self, x: f64) -> f64 {
        self.acc += x * self.gain;
        self.acc
    }
    #[qjs(get)]
    pub fn len(&self) -> usize {
        self.samples.len()
    }
    /// Zero-copy, read-only view.
    pub fn view<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<TypedArray<'js, f32>> {
        let ab = ArrayBuffer::from_source_immutable(ctx, ArcF32(self.samples.clone()))?;
        TypedArray::from_arraybuffer(ab)
    }
    /// A copy, for comparison.
    pub fn copy<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<TypedArray<'js, f32>> {
        TypedArray::new_copy(ctx, &self.samples[..])
    }
    #[qjs(get)]
    pub fn refs(&self) -> usize {
        Arc::strong_count(&self.samples)
    }
}

struct SendSlice(*mut f32, usize);
// SAFETY: the buffer behind it is reachable from no JS value while the worker runs (it was transferred
// into a buffer only the pending table holds), and it is not freed until the completion is drained.
unsafe impl Send for SendSlice {}

type Pending = Rc<RefCell<HashMap<u64, (Persistent<ArrayBuffer<'static>>, Persistent<Function<'static>>)>>>;

static SHARED: OnceLock<Arc<[f32]>> = OnceLock::new();

thread_local! {
    static PENDING: Pending = Rc::default();
    static TX: RefCell<Option<mpsc::Sender<(u64, u64)>>> = const { RefCell::new(None) };
    static NEXT: Cell<u64> = const { Cell::new(1) };
}

fn shared_view<'js>(ctx: Ctx<'js>) -> rquickjs::Result<TypedArray<'js, f32>> {
    let ab = ArrayBuffer::from_source_immutable(ctx, ArcF32(SHARED.get().unwrap().clone()))?;
    TypedArray::from_arraybuffer(ab)
}

fn make_buffer<'js>(ctx: Ctx<'js>, n: usize) -> rquickjs::Result<TypedArray<'js, f32>> {
    let ab = ArrayBuffer::from_source(ctx, Reclaimable(Some(vec![0.0f32; n])))?;
    TypedArray::from_arraybuffer(ab)
}

/// Takes the Vec back from a buffer made by makeBuffer: detach runs our drop hook.
fn reclaim_sum<'js>(ta: TypedArray<'js, f32>) -> rquickjs::Result<f64> {
    let mut ab = ta.arraybuffer()?;
    WANT_RECLAIM.with(|w| w.set(true));
    ab.detach();
    WANT_RECLAIM.with(|w| w.set(false));
    let v = RECLAIMED.with(|r| r.borrow_mut().take()).expect("not a reclaimable buffer");
    Ok(v.iter().map(|x| *x as f64).sum())
}

/// scaleAsync(f32array, k): transfer the JS buffer (zero-copy), scale it in place on a rayon worker,
/// resolve with the same memory as a new Float32Array on the JS thread.
fn scale_async<'js>(ctx: Ctx<'js>, ta: TypedArray<'js, f32>, k: f64) -> rquickjs::Result<rquickjs::Promise<'js>> {
    let ab = ta.arraybuffer()?;
    let transfer: Function = ab.as_object().get("transfer")?;
    let moved: ArrayBuffer = transfer.call((This(ab.clone()),))?;
    let raw = moved.as_raw().expect("detached");
    let slice = SendSlice(raw.as_ptr() as *mut u8 as *mut f32, raw.len() / 4);
    let (promise, resolve, _reject) = ctx.promise()?;
    let id = NEXT.with(|n| {
        let id = n.get();
        n.set(id + 1);
        id
    });
    PENDING.with(|p| p.borrow_mut().insert(id, (Persistent::save(&ctx, moved), Persistent::save(&ctx, resolve))));
    let tx = TX.with(|t| t.borrow().clone()).unwrap();
    let t0 = Instant::now();
    rayon::spawn(move || {
        let s = slice;
        // SAFETY: see SendSlice.
        let data = unsafe { std::slice::from_raw_parts_mut(s.0, s.1) };
        let k = k as f32;
        for x in data.iter_mut() {
            *x *= k;
        }
        let _ = tx.send((id, t0.elapsed().as_nanos() as u64));
    });
    Ok(promise)
}

pub fn run(n: usize) -> Vec<String> {
    let rt = Runtime::new().unwrap();
    rt.set_memory_limit(1 << 31);
    let ctx = Context::full(&rt).unwrap();
    let out: Rc<RefCell<Vec<String>>> = Rc::default();
    let pending: Pending = PENDING.with(|p| p.clone());
    let (tx, rx) = mpsc::channel::<(u64, u64)>();
    TX.with(|t| *t.borrow_mut() = Some(tx));
    let base = Instant::now();
    SHARED.get_or_init(|| vec![7.0f32; 1024].into());

    ctx.with(|ctx| {
        let g = ctx.globals();
        Class::<Sim>::define(&g).unwrap();
        let o = out.clone();
        g.set("log", Function::new(ctx.clone(), move |s: String| o.borrow_mut().push(s)).unwrap()).unwrap();
        g.set("now", Function::new(ctx.clone(), move || base.elapsed().as_secs_f64() * 1000.0).unwrap()).unwrap();
        g.set("sharedView", Function::new(ctx.clone(), shared_view).unwrap()).unwrap();
        g.set("makeBuffer", Function::new(ctx.clone(), make_buffer).unwrap()).unwrap();
        g.set("reclaimSum", Function::new(ctx.clone(), reclaim_sum).unwrap()).unwrap();
        g.set("scaleAsync", Function::new(ctx.clone(), scale_async).unwrap()).unwrap();
        g.set("N", n).unwrap();
    });

    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/js/direct.js")).unwrap();
    ctx.with(|ctx| {
        if let Err(e) = ctx.eval::<(), _>(src.as_str()).catch(&ctx) {
            out.borrow_mut().push(format!("ERROR {e}"));
        }
    });
    let drain = |rt: &Runtime| while rt.execute_pending_job().unwrap_or(false) {};
    drain(&rt);

    // The completion queue: drained on this (the JS) thread; the worker never touches JS.
    while !pending.borrow().is_empty() {
        let (id, worker_ns) = rx.recv().unwrap();
        let t0 = Instant::now();
        ctx.with(|ctx| {
            let (ab, resolve) = pending.borrow_mut().remove(&id).unwrap();
            let ab = ab.restore(&ctx).unwrap();
            let ta = TypedArray::<f32>::from_arraybuffer(ab).unwrap();
            resolve.restore(&ctx).unwrap().call::<_, ()>((ta,)).unwrap();
        });
        let t_resolve = t0.elapsed().as_secs_f64() * 1e3;
        drain(&rt);
        out.borrow_mut().push(format!(
            "completion on JS thread: resolve {t_resolve:.3} ms, + reactions {:.3} ms (worker kernel {:.1} ms)",
            t0.elapsed().as_secs_f64() * 1e3 - t_resolve,
            worker_ns as f64 / 1e6
        ));
    }

    // Drop the JS references to the shared view, then collect: the Arc clone must be released.
    let before = Arc::strong_count(SHARED.get().unwrap());
    ctx.with(|ctx| {
        let _ = ctx.eval::<(), _>("globalThis.keepShared = null;");
    });
    rt.run_gc();
    let after = Arc::strong_count(SHARED.get().unwrap());
    out.borrow_mut().push(format!("shared Arc strong_count with a JS view alive {before}, after GC {after}"));

    let v = out.borrow().clone();
    v
}
