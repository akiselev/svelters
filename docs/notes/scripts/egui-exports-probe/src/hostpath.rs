//! Part B: the existing svelters host + ABI v3 path, with a Rust `Backend` that serves a user type.
//!
//! - sync `host.call` cost through the C ABI (descriptor-typed F64 -> F64);
//! - async methods (`SVL_M_ASYNC` + a one-shot CALLBACK) whose work runs on rayon: the worker sends
//!   (callback id, result) to a completion queue and calls the thread-safe `wake`; the loop thread
//!   drains it with `host.invoke_callback` and runs a checkpoint, which settles the Promise;
//! - the JS thread stays free while a worker computes: 1 ms tick events keep being delivered;
//! - `SVL_BYTES` results (copied today) for a 64 MiB buffer.

use std::cell::{Cell, RefCell};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use svelters_abi::*;
use svelters_host::{Host, HostConfig};

const K_SIM: svl_id = 1;
const M_ADD: svl_id = 1;
const M_NOOP: svl_id = 2;
const M_STEP: svl_id = 3;
const M_BYTES: svl_id = 4;
const M_FINISH: svl_id = 5;
const M_TICKS: svl_id = 6;
const S_TICK: svl_id = 1;
const S_START: svl_id = 2;
const CB_T: svl_id = SVL_T_FIRST_BACKEND + 1;

enum Msg {
    Wake,
    Done(u64, Value),
    Tick,
    Quit,
}

struct ProbeBackend {
    tx: Sender<Msg>,
    rx: RefCell<Option<Receiver<Msg>>>,
    acc: Cell<f64>,
    next: Cell<u64>,
    ticking: Arc<AtomicBool>,
    worker_busy_ns: Arc<Mutex<u64>>,
    stats: RefCell<(u64, u64, u64, u64)>,
}

fn cb_of(args: &[Value]) -> StrResult<u64> {
    match args.last() {
        Some(Value::Callback { id, .. }) => Ok(*id),
        other => Err(format!("expected the completion callback, got {other:?}")),
    }
}

impl Backend for ProbeBackend {
    fn resolve(&self, what: NameKind, _kind: svl_id, name: &str) -> svl_id {
        match (what, name) {
            (NameKind::Kind, "sim") => K_SIM,
            (NameKind::Method, "add") => M_ADD,
            (NameKind::Method, "noop") => M_NOOP,
            (NameKind::Method, "step") => M_STEP,
            (NameKind::Method, "bytes") => M_BYTES,
            (NameKind::Method, "finish") => M_FINISH,
            (NameKind::Method, "ticks") => M_TICKS,
            (NameKind::Signal, "tick") => S_TICK,
            (NameKind::Signal, "start") => S_START,
            _ => 0,
        }
    }
    fn create(&self, kind: svl_id) -> StrResult<svl_handle> {
        if kind != K_SIM {
            return Err("unknown kind".into());
        }
        let h = self.next.get();
        self.next.set(h + 1);
        Ok(h)
    }
    fn insert(&self, _: svl_handle, _: svl_handle, _: Option<svl_handle>, _: svl_id) -> StrResult<()> {
        Err("no tree".into())
    }
    fn remove(&self, _: svl_handle, _: svl_handle) -> StrResult<()> {
        Err("no tree".into())
    }
    fn destroy(&self, _: svl_handle) {}
    fn set_prop(&self, _: svl_handle, _: svl_id, _: &Value) -> StrResult<()> {
        Err("no props".into())
    }
    fn get_prop(&self, _: svl_handle, _: svl_id) -> StrResult<Value> {
        Err("no props".into())
    }
    fn call(&self, _h: svl_handle, method: svl_id, args: &[Value]) -> StrResult<Value> {
        match method {
            M_ADD => {
                let Some(Value::F64(x)) = args.first() else { return Err(format!("add: {args:?}")) };
                self.acc.set(self.acc.get() + x);
                Ok(Value::F64(self.acc.get()))
            }
            M_NOOP => {
                let id = cb_of(args)?;
                let tx = self.tx.clone();
                rayon::spawn(move || {
                    let _ = tx.send(Msg::Done(id, Value::Null));
                });
                Ok(Value::Null)
            }
            M_STEP => {
                let Some(Value::I64(ms)) = args.first().cloned() else { return Err(format!("step: {args:?}")) };
                let id = cb_of(args)?;
                let tx = self.tx.clone();
                let busy = self.worker_busy_ns.clone();
                rayon::spawn(move || {
                    // CPU-bound for `ms` milliseconds.
                    let t0 = Instant::now();
                    let mut x = 0.0f64;
                    while t0.elapsed() < Duration::from_millis(ms as u64) {
                        for i in 0..10_000 {
                            x += (i as f64).sqrt();
                        }
                    }
                    *busy.lock().unwrap() += t0.elapsed().as_nanos() as u64;
                    let _ = tx.send(Msg::Done(id, Value::F64(x)));
                });
                Ok(Value::Null)
            }
            M_BYTES => {
                let Some(Value::I64(n)) = args.first() else { return Err(format!("bytes: {args:?}")) };
                Ok(Value::Bytes(vec![1u8; *n as usize]))
            }
            M_TICKS => {
                let on = matches!(args.first(), Some(Value::Bool(true)));
                self.ticking.store(on, Ordering::SeqCst);
                Ok(Value::Null)
            }
            M_FINISH => {
                let _ = self.tx.send(Msg::Quit);
                Ok(Value::Null)
            }
            _ => Err(format!("unknown method {method}")),
        }
    }
    fn listen(&self, _: svl_handle, _: svl_id, _: bool) -> StrResult<()> {
        Ok(())
    }
    fn handle_info(&self, _: svl_handle) -> StrResult<svl_handle_info> {
        Ok(svl_handle_info { kind: K_SIM, lifetime: SVL_LIFE_SHARED, flags: 0 })
    }
    /// The "toolkit loop": what an egui backend's frame callback would do (drain completions, then
    /// run one host checkpoint).
    fn run(&self, host: &HostRef<'_>) -> StrResult<()> {
        let rx = self.rx.borrow_mut().take().ok_or("run twice")?;
        // A ticker thread: 1 ms "frames" while JS asked for them.
        let ticking = self.ticking.clone();
        let ttx = self.tx.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let st = stop.clone();
        let ticker = std::thread::spawn(move || {
            while !st.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(1));
                if ticking.load(Ordering::SeqCst) {
                    let _ = ttx.send(Msg::Tick);
                }
            }
        });
        host.post_event(1, S_START, &[]);
        host.checkpoint();
        loop {
            let first = rx.recv().map_err(|e| e.to_string())?;
            // Coalesce: everything already queued is handled before one checkpoint.
            let mut batch = vec![first];
            while let Ok(m) = rx.try_recv() {
                batch.push(m);
            }
            let mut quit = false;
            for m in batch {
                match m {
                    Msg::Done(id, v) => {
                        let t0 = Instant::now();
                        let _ = host.invoke_callback(id, &[v]);
                        self.stats.borrow_mut().0 += t0.elapsed().as_nanos() as u64;
                        self.stats.borrow_mut().1 += 1;
                    }
                    Msg::Tick => host.post_event(1, S_TICK, &[]),
                    Msg::Wake => {}
                    Msg::Quit => quit = true,
                }
            }
            let t0 = Instant::now();
            host.checkpoint();
            self.stats.borrow_mut().2 += t0.elapsed().as_nanos() as u64;
            self.stats.borrow_mut().3 += 1;
            if quit {
                break;
            }
        }
        stop.store(true, Ordering::SeqCst);
        let _ = ticker.join();
        let (inv, n_inv, cp, n_cp) = *self.stats.borrow();
        eprintln!(
            "  loop: invoke_callback {:.2} us avg over {n_inv}; checkpoint {:.1} us avg over {n_cp}",
            inv as f64 / 1e3 / n_inv.max(1) as f64,
            cp as f64 / 1e3 / n_cp.max(1) as f64
        );
        Ok(())
    }
    fn waker(&self) -> Waker {
        let tx = Mutex::new(self.tx.clone());
        Arc::new(move || {
            let _ = tx.lock().unwrap().send(Msg::Wake);
        })
    }
    fn quit(&self) {
        let _ = self.tx.send(Msg::Quit);
    }
    fn schedule_wake(&self, _ms: i64) {}
    fn describe(&self, what: NameKind, _kind: svl_id, member: svl_id) -> Option<MemberDesc> {
        if what != NameKind::Method {
            return None;
        }
        let done = || SlotDesc::new(SVL_CALLBACK).typed(CB_T).named("done");
        Some(match member {
            M_ADD => MemberDesc { name: "add".into(), params: vec![SlotDesc::new(SVL_F64).named("x")], ret: SlotDesc::new(SVL_F64), flags: 0 },
            M_NOOP => MemberDesc { name: "noop".into(), params: vec![done()], ret: SlotDesc::new(SVL_NULL), flags: SVL_M_ASYNC },
            M_STEP => MemberDesc {
                name: "step".into(),
                params: vec![SlotDesc::new(SVL_I64).named("ms"), done()],
                ret: SlotDesc::new(SVL_NULL),
                flags: SVL_M_ASYNC,
            },
            M_BYTES => MemberDesc { name: "bytes".into(), params: vec![SlotDesc::new(SVL_I64).named("n")], ret: SlotDesc::new(SVL_BYTES), flags: 0 },
            M_TICKS => MemberDesc { name: "ticks".into(), params: vec![SlotDesc::new(SVL_BOOL).named("on")], ret: SlotDesc::new(SVL_NULL), flags: 0 },
            M_FINISH => MemberDesc { name: "finish".into(), params: vec![], ret: SlotDesc::new(SVL_NULL), flags: 0 },
            _ => return None,
        })
    }
    fn describe_type(&self, ty: svl_id) -> Option<TypeDesc> {
        (ty == CB_T).then(|| TypeDesc {
            name: "Completion".into(),
            kind: SVL_CALLBACK,
            scope: SVL_CB_ASYNC,
            members: vec![],
            fields: vec![SlotDesc::new(SVL_NULL).named("result")],
            ret: SlotDesc::new(SVL_NULL),
        })
    }
}

pub fn run() -> Vec<String> {
    let (tx, rx) = mpsc::channel();
    let busy = Arc::new(Mutex::new(0u64));
    let vt = BackendVtable::new(ProbeBackend {
        tx,
        rx: RefCell::new(Some(rx)),
        acc: Cell::new(0.0),
        next: Cell::new(1),
        ticking: Arc::new(AtomicBool::new(false)),
        worker_busy_ns: busy.clone(),
        stats: RefCell::new((0, 0, 0, 0)),
    });
    let cfg = HostConfig { echo_console: false, echo_errors: false, timing: true, ..HostConfig::default() };
    let host = Host::new(vt.backend_ref(), cfg).unwrap();
    host.load_bundle(concat!(env!("CARGO_MANIFEST_DIR"), "/js/host.js")).unwrap();
    host.run().unwrap();
    let mut out: Vec<String> = host.take_console().into_iter().map(|l| l.text).collect();
    for e in host.take_errors() {
        out.push(format!("ERROR {e}"));
    }
    let t = host.take_timing();
    out.push(format!(
        "host timing: {} checkpoints, JS {:.1} ms, ops {:.1} ms ({} ops); worker busy {:.1} ms",
        t.checkpoints,
        t.js_ns as f64 / 1e6,
        t.ops_ns as f64 / 1e6,
        t.ops,
        *busy.lock().unwrap() as f64 / 1e6
    ));
    out
}
