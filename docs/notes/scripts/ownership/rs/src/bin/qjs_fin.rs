//! QuickJS-NG (rquickjs 0.14) finalizer experiments: FinalizationRegistry and Rust class Drop
//! racing with explicit destroy. The "host" is a handle table with idempotent release.
use rquickjs::{class::Trace, Context, Function, Runtime};
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

#[derive(Default)]
struct Host {
    live: HashSet<u64>,
    next: u64,
    log: Vec<String>,
}

thread_local!(static HOST: RefCell<Host> = RefCell::new(Host::default()));

fn create() -> u64 {
    HOST.with(|h| {
        let mut h = h.borrow_mut();
        h.next += 1; // monotonic, never reused
        let id = h.next;
        h.live.insert(id);
        h.log.push(format!("create {id}"));
        id
    })
}
fn release(id: u64, why: &str) {
    HOST.with(|h| {
        let mut h = h.borrow_mut();
        let was = h.live.remove(&id);
        h.log.push(format!("release {id} via {why}: {}", if was { "freed" } else { "stale no-op" }));
    })
}

/// A JS-visible wrapper whose Rust Drop runs inside the QuickJS GC.
#[derive(Trace, rquickjs::JsLifetime)]
#[rquickjs::class]
pub struct Wrapped {
    #[qjs(skip_trace)]
    id: u64,
}
impl Drop for Wrapped {
    fn drop(&mut self) {
        // must not touch JS or the backend re-entrantly here: only record
        release(self.id, "class-drop");
    }
}
#[rquickjs::methods]
impl Wrapped {
    #[qjs(constructor)]
    pub fn new() -> Self {
        Wrapped { id: create() }
    }
    #[qjs(get)]
    pub fn id(&self) -> u64 {
        self.id
    }
}

fn main() {
    let rt = Runtime::new().unwrap();
    let ctx = Context::full(&rt).unwrap();
    let order = Rc::new(RefCell::new(Vec::<String>::new()));
    ctx.with(|ctx| {
        let g = ctx.globals();
        g.set("create", Function::new(ctx.clone(), || create() as f64).unwrap()).unwrap();
        g.set("destroy", Function::new(ctx.clone(), |id: f64| release(id as u64, "explicit-destroy")).unwrap()).unwrap();
        g.set("fr_release", Function::new(ctx.clone(), |id: f64| release(id as u64, "FinalizationRegistry")).unwrap()).unwrap();
        rquickjs::Class::<Wrapped>::define(&g).unwrap();
        let has_fr: bool = ctx.eval("typeof FinalizationRegistry === 'function' && typeof WeakRef === 'function'").unwrap();
        println!("info FinalizationRegistry+WeakRef available: {has_fr}");
        ctx.eval::<(), _>(r#"
            globalThis.fr = new FinalizationRegistry((h) => fr_release(h));
            globalThis.mk = (destroyFirst) => {
              const node = { h: create(), listeners: [] };
              node.listeners.push(() => node.h);            // closure cycle node <-> handler
              fr.register(node, node.h);
              if (destroyFirst) destroy(node.h);             // explicit destroy, then drop the JS ref
            };
            mk(true);   // 1: destroyed explicitly, finalizer later -> must be a stale no-op
            mk(false);  // 2: never destroyed: finalizer is the only release
            globalThis.w = new Wrapped(); // 3: Rust-class drop path
            destroy(w.id);               // explicit destroy first
            globalThis.w = null;
            globalThis.keep = new Wrapped(); // 4: still referenced, must survive gc
        "#).unwrap();
    });
    order.borrow_mut().push("before gc".into());
    rt.run_gc();
    HOST.with(|h| h.borrow_mut().log.push("--- after run_gc, before jobs ---".into()));
    while rt.is_job_pending() {
        let _ = rt.execute_pending_job();
    }
    HOST.with(|h| h.borrow_mut().log.push("--- after jobs ---".into()));
    HOST.with(|h| {
        let h = h.borrow();
        for l in &h.log {
            println!("  {l}");
        }
        let mut live: Vec<_> = h.live.iter().copied().collect();
        live.sort();
        println!("  live after gc: {live:?}");
    });
    drop(ctx);
    drop(rt);
    HOST.with(|h| {
        let h = h.borrow();
        println!("  after runtime drop, last log: {:?}", h.log.last());
        println!("  live after runtime drop: {:?}", h.live);
    });
}
