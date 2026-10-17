//! X2: sync signal handlers (`call_sync`) and the frame clock (`request_frame` / `host.frame`) over
//! the log backend and hand-written bundles in `tests/js/`.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;
use svelters_abi::{
    Backend, BackendVtable, HostRef, NameKind, SVL_SYNC_BUSY, Value, Waker, svl_handle,
    svl_handle_info, svl_id,
};
use svelters_backend_log::{LogBackend, LogOptions, parse_script};
use svelters_host::testing::{fixture_config, run_bundle};
use svelters_host::{Host, HostConfig};

fn js(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/js").join(name)
}

fn quiet() -> HostConfig {
    HostConfig {
        echo_console: false,
        echo_errors: false,
        ..HostConfig::default()
    }
}

const HEAD: &str = "create window #1\nlisten #1 close-request on\ncreate label #2\ninsert #1 #2 before=end slot=default\n";

#[test]
fn sync_results_come_from_the_handler_and_failures_give_the_default() {
    let (cfg, opts) = fixture_config();
    let steps = parse_script(
        r#"[{"checkpoint":true},
            {"sync":[1,"close-request",["veto"]]},
            {"sync":[1,"close-request",["none"]]},
            {"sync":[1,"close-request",["throw"]]},
            {"sync":[1,"close-request",["undef"]]},
            {"sync":[1,"close-request",["object"]]},
            {"sync":[1,"close-request",["nested"]]},
            {"sync":[2,"close-request",[]]}]"#,
    )
    .unwrap();
    let r = run_bundle(&js("sync_basic.js"), steps, cfg, opts).unwrap();
    let want = format!(
        "{HEAD}\
         sync #1 close-request [\"veto\"]\nresult #1 true\n\
         sync #1 close-request [\"none\"]\nresult #1 null\n\
         sync #1 close-request [\"throw\"]\nresult #1 null\n\
         sync #1 close-request [\"undef\"]\nresult #1 null\n\
         sync #1 close-request [\"object\"]\nresult #1 {{\"a\":1}}\n\
         sync #1 close-request [\"nested\"]\nresult #1 null\n\
         sync #2 close-request []\nresult #2 error\n"
    );
    assert_eq!(r.oplog, want);
    // A throwing handler and an unusable result are reported, not swallowed.
    let msgs: Vec<String> = r.errors.iter().map(|e| e.message.clone()).collect();
    assert_eq!(msgs.len(), 2, "{msgs:?}");
    assert!(msgs[0].contains("handler boom"), "{msgs:?}");
    assert!(msgs[1].contains("unsupported value"), "{msgs:?}");
}

#[test]
fn ops_in_a_sync_handler_apply_at_once_and_microtasks_wait_for_the_checkpoint() {
    let (cfg, opts) = fixture_config();
    let steps = parse_script(
        r#"[{"sync":[1,"close-request",["ops"]]},{"checkpoint":true}]"#,
    )
    .unwrap();
    let r = run_bundle(&js("sync_basic.js"), steps, cfg, opts).unwrap();
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    assert_eq!(
        r.oplog,
        format!(
            "{HEAD}sync #1 close-request [\"ops\"]\nset #2 label \"immediate\"\nresult #1 true\nset #2 label \"microtask\"\n"
        )
    );
}

#[test]
fn microtasks_left_by_a_live_sync_call_wake_the_backend() {
    // Not scripted: after a sync call that leaves jobs pending, the host asks for a checkpoint.
    let steps = parse_script(r#"[{"sync":[1,"close-request",["ops"]]}]"#).unwrap();
    let log = LogBackend::with_options(steps, LogOptions { settle: Duration::from_millis(50) });
    let vt = log.vtable();
    let host = Host::new(vt.backend_ref(), quiet()).unwrap();
    host.load_bundle(js("sync_basic.js")).unwrap();
    host.run().unwrap();
    assert!(host.take_errors().is_empty());
    let out = log.log();
    assert!(out.ends_with("result #1 true\nset #2 label \"microtask\"\n"), "{out}");
}

/// Raises sync signals from inside host ops, like a toolkit whose setter emits a vetoable signal
/// (and one that spins a nested loop).
struct Raising {
    inner: LogBackend,
    host: Cell<Option<HostRef<'static>>>,
    codes: Rc<RefCell<Vec<i32>>>,
}

impl Backend for Raising {
    fn resolve(&self, w: NameKind, k: svl_id, n: &str) -> svl_id {
        self.inner.resolve(w, k, n)
    }
    fn create(&self, k: svl_id) -> Result<svl_handle, String> {
        self.inner.create(k)
    }
    fn insert(&self, p: svl_handle, c: svl_handle, b: Option<svl_handle>, s: svl_id) -> Result<(), String> {
        self.inner.insert(p, c, b, s)
    }
    fn remove(&self, p: svl_handle, c: svl_handle) -> Result<(), String> {
        self.inner.remove(p, c)
    }
    fn destroy(&self, h: svl_handle) {
        self.inner.destroy(h)
    }
    fn set_prop(&self, h: svl_handle, p: svl_id, v: &Value) -> Result<(), String> {
        if let Some(host) = self.host.get() {
            // The host cannot re-enter JS: it must answer busy and queue the event.
            let kind = self.inner.resolve(NameKind::Kind, 0, "window");
            let sig = self.inner.resolve(NameKind::Signal, kind, "close-request");
            let code = host.call_sync(1, sig, &[]).err().map_or(0, |e| e.code);
            self.codes.borrow_mut().push(code);
            host.checkpoint(); // a nested loop on top of it
        }
        self.inner.set_prop(h, p, v)
    }
    fn get_prop(&self, h: svl_handle, p: svl_id) -> Result<Value, String> {
        self.inner.get_prop(h, p)
    }
    fn call(&self, h: svl_handle, m: svl_id, a: &[Value]) -> Result<Value, String> {
        self.inner.call(h, m, a)
    }
    fn listen(&self, h: svl_handle, s: svl_id, on: bool) -> Result<(), String> {
        self.inner.listen(h, s, on)
    }
    fn handle_info(&self, h: svl_handle) -> Result<svl_handle_info, String> {
        self.inner.handle_info(h)
    }
    fn schedule_wake(&self, ms: i64) {
        self.inner.schedule_wake(ms)
    }
    fn request_frame(&self) {
        self.inner.request_frame()
    }
    fn run(&self, host: &HostRef<'_>) -> Result<(), String> {
        // SAFETY: the host outlives this call; the stashed copy is cleared before returning.
        self.host.set(Some(unsafe { std::mem::transmute::<HostRef<'_>, HostRef<'static>>(*host) }));
        let r = self.inner.run(host);
        self.host.set(None);
        r
    }
    fn waker(&self) -> Waker {
        self.inner.waker()
    }
    fn quit(&self) {
        self.inner.quit()
    }
}

#[test]
fn a_sync_signal_raised_while_js_runs_is_deferred_not_nested() {
    let (cfg, opts) = fixture_config();
    let steps = parse_script(r#"[{"sync":[1,"close-request",[]]},{"checkpoint":true}]"#).unwrap();
    let log = LogBackend::with_options(steps, opts);
    let codes = Rc::new(RefCell::new(Vec::new()));
    let backend = Raising { inner: log.clone(), host: Cell::new(None), codes: codes.clone() };
    let vt = BackendVtable::new(backend);
    let host = Host::new(vt.backend_ref(), cfg).unwrap();
    host.load_bundle(js("sync_busy.js")).unwrap();
    host.run().unwrap();
    assert!(host.take_errors().is_empty());
    let out = log.log();
    // The handler ran synchronously exactly once and its own setProp raised a second sync signal
    // that could not nest: it came back later as a deferred async event.
    assert!(out.contains("sync #1 close-request []\nset #2 label \"from sync\"\nresult #1 true\n"), "{out}");
    assert!(out.ends_with("result #1 true\ncall #2 seen [true]\n"), "{out}");
    assert_eq!(*codes.borrow(), vec![SVL_SYNC_BUSY]);
    assert!(log.wake_count() >= 1, "the nested checkpoint request became a wake");
    drop(host);
}

#[test]
fn raf_runs_on_the_backend_frame_clock_with_no_host_thread() {
    let threads_before = std::fs::read_dir("/proc/self/task").unwrap().count();
    let log = LogBackend::with_options(vec![], LogOptions { settle: Duration::from_millis(100) });
    let vt = log.vtable();
    let host = Host::new(vt.backend_ref(), quiet()).unwrap();
    host.load_bundle(js("raf.js")).unwrap();
    // The first requestAnimationFrame asked the backend for one frame; later ones joined it.
    assert_eq!(log.frames_requested(), 1);
    assert_eq!(log.frames_delivered(), 0, "nothing runs before the toolkit's frame");
    host.run().unwrap();
    let errs = host.take_errors();
    assert_eq!(errs.len(), 1, "{errs:?}");
    assert!(errs[0].message.contains("frame boom"), "{errs:?}");
    // Frame 1 ran a, b, d (the cancelled and the throwing callback did not stop the others);
    // frame 2, requested from inside frame 1, ran c.
    assert_eq!(log.frames_requested(), 2);
    assert_eq!(log.frames_delivered(), 2);
    let out = log.log();
    assert!(out.ends_with("set #1 label \"a,b,d,c true number\"\n"), "{out}");
    assert_eq!(log.timer_fire_count(), 0);
    assert_eq!(std::fs::read_dir("/proc/self/task").unwrap().count(), threads_before, "no thread was started");
    drop(host);
}

#[test]
fn scripted_hosts_use_a_virtual_timer_for_raf() {
    let (cfg, opts) = fixture_config();
    let steps = parse_script(r#"[{"checkpoint":true}]"#).unwrap();
    let r = run_bundle(&js("raf.js"), steps, cfg, opts).unwrap();
    assert!(r.oplog.ends_with("set #1 label \"a,b,d,c true number\"\n"), "{}", r.oplog);
}
