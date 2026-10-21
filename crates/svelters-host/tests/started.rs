//! G0 (ABI v2.7): the started hook (`run_with`, `start_with`), the non-blocking `start`, and the
//! pre-attach of `load_bundle`, on the log backend.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use svelters_abi::{Backend, BackendVtable, HostRef, NameKind, Value, Waker, svl_handle, svl_handle_info, svl_id};
use svelters_backend_log::{LogBackend, Step, parse_script};
use svelters_host::Host;
use svelters_host::testing::{fixture_config, run_bundle};

fn js(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/js").join(name)
}

fn script(json: &str) -> Vec<Step> {
    parse_script(json).unwrap()
}

/// A bundle in a scratch directory.
fn bundle(name: &str, src: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("svelters-started-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("main.js");
    std::fs::write(&p, src).unwrap();
    p
}

const EVENTS: &str = r#"[{"event":[2,"clicked",[1.5,"x"]]},{"event":[2,"clicked",[]]},{"checkpoint":true}]"#;

#[test]
fn run_with_fires_once_before_the_first_step_and_mounts_the_bundle() {
    let (cfg, opts) = fixture_config();
    let log = LogBackend::with_options(script(EVENTS), opts);
    let vt = log.vtable();
    let host = Host::new(vt.backend_ref(), cfg).unwrap();
    let fired = Rc::new(Cell::new(0));
    let log_at_hook = Rc::new(RefCell::new(None::<String>));
    let (f, l, lg) = (fired.clone(), log_at_hook.clone(), log.clone());
    host.run_with(move |h| {
        f.set(f.get() + 1);
        *l.borrow_mut() = Some(lg.log());
        assert!(h.is_started());
        h.load_bundle(js("basic_ops.js")).unwrap();
    })
    .unwrap();
    assert_eq!(fired.get(), 1);
    assert_eq!(log_at_hook.borrow().as_deref(), Some(""), "the hook runs before any step");
    assert!(!host.is_started(), "run returned");
    let errors = host.take_errors();
    assert!(errors.is_empty(), "{errors:?}");

    // Same op log as the desktop order (load, then run) over the same script.
    let (cfg, opts) = fixture_config();
    let classic = run_bundle(&js("basic_ops.js"), script(EVENTS), cfg, opts).unwrap();
    assert_eq!(log.log(), classic.oplog);
    assert!(log.log().starts_with("create window #1\n"));
    assert!(log.log().contains("set #3 label \"clicked 2 []\"\n"));
}

#[test]
fn start_with_fires_synchronously_and_the_embedder_checkpoints() {
    let (cfg, opts) = fixture_config();
    let log = LogBackend::with_options(Vec::new(), opts);
    let vt = log.vtable();
    let path = bundle(
        "start",
        "const h = globalThis.__svelters_host;\n\
         const w = h.create('window');\n\
         Promise.resolve().then(() => { const l = h.create('label'); h.insert(w, l, null); });\n",
    );
    let host = Host::new(vt.backend_ref(), cfg).unwrap();
    assert!(!host.is_started());
    let fired = Rc::new(Cell::new(false));
    let f = fired.clone();
    host.start_with(move |h| {
        f.set(true);
        h.load_bundle(&path).unwrap();
    })
    .unwrap();
    assert!(fired.get(), "start_with returns after the hook ran");
    assert!(host.is_started());
    assert!(log.is_attached());
    assert_eq!(log.log(), "create window #1\n", "scripted load runs no jobs");

    host.checkpoint();
    assert_eq!(log.log(), "create window #1\ncreate label #2\ninsert #1 #2 before=end slot=default\n");

    // A hook registered on a started host fires at once; start again is a no-op.
    let again = Rc::new(Cell::new(false));
    let a = again.clone();
    host.on_started(move |_| a.set(true));
    assert!(again.get());
    host.start().unwrap();

    host.stop();
    assert!(!host.is_started());
    assert!(!log.is_attached());
    host.stop();
    host.start().unwrap();
    assert!(log.is_attached());
    let errors = host.take_errors();
    assert!(errors.is_empty(), "{errors:?}");
    drop(host);
    assert!(!log.is_attached(), "Drop detaches");
}

#[test]
fn run_without_a_hook_is_unchanged() {
    let (cfg, opts) = fixture_config();
    let r = run_bundle(&js("basic_ops.js"), script(EVENTS), cfg, opts).unwrap();
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    assert_eq!(
        r.oplog,
        "create window #1\n\
         set #1 title \"Hi\"\n\
         create button #2\n\
         set #2 label \"go\"\n\
         listen #2 clicked on\n\
         insert #1 #2 before=end slot=default\n\
         create label #3\n\
         insert #1 #3 before=#2 slot=titlebar\n\
         call #1 present []\n\
         event #2 clicked [1.5,\"x\"]\n\
         event #2 clicked []\n\
         set #3 label \"clicked 2 [1.5,\\\"x\\\"]\"\n\
         set #3 label \"clicked 2 []\"\n"
    );
    // No wake before run without a hook: the loop's first checkpoint comes from the script only.
    assert_eq!(r.wake_count, 0);
}

#[test]
fn load_bundle_pre_attaches_without_starting() {
    let (cfg, opts) = fixture_config();
    let log = LogBackend::with_options(script(EVENTS), opts);
    let vt = log.vtable();
    let host = Host::new(vt.backend_ref(), cfg).unwrap();
    assert!(!log.is_attached());
    host.load_bundle(js("basic_ops.js")).unwrap();
    assert!(log.is_attached(), "desktop order: attached before mount");
    assert!(!host.is_started(), "a pre-attach is not started");
    host.run().unwrap();
    drop(host);
    assert!(!log.is_attached());
}

/// A backend with every v2.7 default: no `start`.
struct NoStart(LogBackend);

impl Backend for NoStart {
    fn resolve(&self, w: NameKind, k: svl_id, n: &str) -> svl_id {
        self.0.resolve(w, k, n)
    }
    fn create(&self, k: svl_id) -> Result<svl_handle, String> {
        self.0.create(k)
    }
    fn insert(&self, p: svl_handle, c: svl_handle, b: Option<svl_handle>, s: svl_id) -> Result<(), String> {
        self.0.insert(p, c, b, s)
    }
    fn remove(&self, p: svl_handle, c: svl_handle) -> Result<(), String> {
        self.0.remove(p, c)
    }
    fn destroy(&self, h: svl_handle) {
        self.0.destroy(h)
    }
    fn set_prop(&self, h: svl_handle, p: svl_id, v: &Value) -> Result<(), String> {
        self.0.set_prop(h, p, v)
    }
    fn get_prop(&self, h: svl_handle, p: svl_id) -> Result<Value, String> {
        self.0.get_prop(h, p)
    }
    fn call(&self, h: svl_handle, m: svl_id, a: &[Value]) -> Result<Value, String> {
        self.0.call(h, m, a)
    }
    fn listen(&self, h: svl_handle, s: svl_id, on: bool) -> Result<(), String> {
        self.0.listen(h, s, on)
    }
    fn handle_info(&self, h: svl_handle) -> Result<svl_handle_info, String> {
        self.0.handle_info(h)
    }
    fn run(&self, host: &HostRef<'_>) -> Result<(), String> {
        self.0.run(host)
    }
    fn waker(&self) -> Waker {
        self.0.waker()
    }
    fn quit(&self) {
        self.0.quit()
    }
    fn schedule_wake(&self, ms: i64) {
        self.0.schedule_wake(ms)
    }
}

#[test]
fn start_without_backend_support_is_an_error() {
    let (cfg, opts) = fixture_config();
    let vt = BackendVtable::new(NoStart(LogBackend::with_options(Vec::new(), opts)));
    assert!(!vt.backend_ref().has_start());
    let host = Host::new(vt.backend_ref(), cfg).unwrap();
    let fired = Rc::new(Cell::new(false));
    let f = fired.clone();
    let err = host.start_with(move |_| f.set(true)).unwrap_err();
    assert!(err.message.contains("start"), "{err:?}");
    assert!(!fired.get());
    assert!(!host.is_started());
    // load_bundle does not need `start`.
    host.load_bundle(js("basic_ops.js")).unwrap();
    host.stop();
}
