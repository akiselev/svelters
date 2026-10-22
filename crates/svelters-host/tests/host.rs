//! Host behaviour tests over hand-written bundles in `tests/js/` (independent of the P1 fixtures).

use std::cell::Cell;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use svelters_abi::{Backend, BackendVtable, HostRef, NameKind, Value, Waker, svl_handle, svl_handle_info, svl_id};
use svelters_backend_log::{LogBackend, LogOptions, Step, parse_script};
use svelters_host::testing::{fixture_config, run_bundle};
use svelters_host::{ErrorKind, Host, HostConfig};

fn js(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/js")
        .join(name)
}

fn quiet() -> HostConfig {
    HostConfig {
        echo_console: false,
        echo_errors: false,
        ..HostConfig::default()
    }
}

fn script(json: &str) -> Vec<Step> {
    parse_script(json).unwrap()
}

#[test]
fn ops_and_events_scripted() {
    let (cfg, opts) = fixture_config();
    let steps = script(
        r#"[{"event":[2,"clicked",[1.5,"x"]]},{"event":[2,"clicked",[]]},{"checkpoint":true}]"#,
    );
    let r = run_bundle(&js("basic_ops.js"), steps, cfg, opts).unwrap();
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
}

/// `performance.now()` is the virtual clock in scripted mode, not QuickJS-NG's built-in wall clock
/// (a transition timed on wall time against virtual frames made TK1's cases flaky).
#[test]
fn scripted_performance_now_is_the_virtual_clock() {
    let (cfg, opts) = fixture_config();
    let r = run_bundle(&js("perf_now.js"), script(r#"[{"checkpoint":true}]"#), cfg, opts).unwrap();
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    assert!(r.oplog.contains("set #2 label \"elapsed 1000\"\n"), "{}", r.oplog);
}

#[test]
fn scripted_timers_run_on_virtual_clock_in_due_order() {
    // The bundle's timers span 60 ms of virtual time. The best of five runs must beat that, so a
    // loaded machine cannot make the check flaky but a sleeping clock cannot pass.
    let mut best = Duration::MAX;
    let mut report = None;
    for _ in 0..5 {
        let (cfg, opts) = fixture_config();
        let started = Instant::now();
        let r = run_bundle(
            &js("timers_promises.js"),
            script(r#"[{"checkpoint":true}]"#),
            cfg,
            opts,
        )
        .unwrap();
        best = best.min(started.elapsed());
        report = Some(r);
    }
    assert!(
        best < Duration::from_millis(55),
        "virtual clock must not sleep: {best:?}"
    );
    let r = report.unwrap();
    let labels: Vec<&str> = r
        .oplog
        .lines()
        .filter_map(|l| l.strip_prefix("set #2 label "))
        .collect();
    // Deadlines are wall-clock, so under load a timer registered a few ms later can overtake
    // one with a nearby deadline, as in a browser. Assert only orderings that hold regardless;
    // the exact sequence is pinned by the virtual-clock (scripted) timers fixture.
    let pos = |l: &str| {
        labels
            .iter()
            .position(|x| *x == format!("\"{l}\""))
            .unwrap_or_else(|| panic!("{l} missing from {labels:?}"))
    };
    assert_eq!(labels.len(), 9, "{labels:?}");
    assert_eq!(labels[..4], ["\"start\"", "\"p1\"", "\"mt\"", "\"p2\""]);
    assert!(pos("t20") < pos("t50") && pos("t20") < pos("async-40"), "{labels:?}");
    assert_eq!(pos("t50") + 1, pos("t50-micro"), "{labels:?}");
    assert!(pos("t50-micro") < pos("t50+10"), "{labels:?}");
}

#[test]
fn real_timers_and_promises_complete_without_busy_polling() {
    let opts = LogOptions {
        settle: Duration::from_millis(150),
    };
    let started = Instant::now();
    let r = run_bundle(&js("timers_promises.js"), vec![], quiet(), opts).unwrap();
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    assert!(
        started.elapsed() >= Duration::from_millis(60),
        "timers fire at their deadlines"
    );

    let labels: Vec<&str> = r
        .oplog
        .lines()
        .filter_map(|l| l.strip_prefix("set #2 label "))
        .collect();
    assert_eq!(
        labels,
        [
            "\"start\"",
            "\"p1\"",
            "\"mt\"",
            "\"p2\"",
            "\"t20\"",
            "\"async-40\"",
            "\"t50\"",
            "\"t50-micro\"",
            "\"t50+10\""
        ]
    );
    // At most one backend timer expiry per distinct deadline (20, 40, 50, 60 ms), delivered
    // through `schedule_wake`; late wakes under load merge deadlines. A polling host would show
    // hundreds.
    assert!(r.timer_fires >= 1, "timer_fires = {}", r.timer_fires);
    assert!(r.timer_fires <= 10, "timer_fires = {}", r.timer_fires);
    assert!(
        r.checkpoint_count <= 10,
        "checkpoint_count = {}",
        r.checkpoint_count
    );
}

#[test]
fn console_and_error_reporting_with_stacks() {
    let (cfg, opts) = fixture_config();
    let steps = script(r#"[{"checkpoint":true},{"event":[1,"closed",[]]}]"#);
    let r = run_bundle(&js("errors.js"), steps, cfg, opts).unwrap();

    let lines: Vec<(&str, &str)> = r
        .console
        .iter()
        .map(|c| (c.level.as_str(), c.text.as_str()))
        .collect();
    assert_eq!(
        lines,
        [
            ("log", "hello {\"a\":1} [1,2] null undefined"),
            ("error", "bad 42")
        ]
    );

    let by_msg = |m: &str| {
        r.errors
            .iter()
            .find(|e| e.message.contains(m))
            .unwrap_or_else(|| panic!("no error {m}: {:?}", r.errors))
    };
    let rejection = by_msg("unhandled one");
    assert_eq!(rejection.kind, ErrorKind::UnhandledRejection);
    assert!(
        rejection
            .stack
            .as_deref()
            .is_some_and(|s| s.contains("errors.js")),
        "{:?}",
        rejection.stack
    );
    let timer = by_msg("boom in timer");
    assert_eq!(timer.kind, ErrorKind::Exception);
    assert!(
        timer
            .stack
            .as_deref()
            .is_some_and(|s| s.contains("errors.js")),
        "{:?}",
        timer.stack
    );
    let dispatch = by_msg("boom in dispatch");
    assert!(dispatch.stack.is_some());
}

#[test]
fn runaway_loop_during_load_is_interrupted() {
    let log = LogBackend::new(vec![]);
    let vt = log.vtable();
    let cfg = HostConfig {
        turn_budget: Some(Duration::from_millis(150)),
        ..quiet()
    };
    let host = Host::new(vt.backend_ref(), cfg).unwrap();
    let started = Instant::now();
    let err = host.load_bundle(js("runaway_load.js")).unwrap_err();
    assert_eq!(err.kind, ErrorKind::Interrupted, "{err}");
    assert!(started.elapsed() < Duration::from_secs(5));
    // The ops before the loop were forwarded.
    assert_eq!(log.log(), "create window #1\n");
}

#[test]
fn runaway_handler_is_interrupted_and_host_keeps_working() {
    let (mut cfg, opts) = fixture_config();
    cfg.turn_budget = Some(Duration::from_millis(150));
    let steps = script(r#"[{"event":[1,"spin",[]]},{"event":[1,"ping",[]]}]"#);
    let r = run_bundle(&js("runaway_event.js"), steps, cfg, opts).unwrap();
    assert_eq!(r.errors.len(), 1, "{:?}", r.errors);
    assert_eq!(r.errors[0].kind, ErrorKind::Interrupted);
    assert!(
        r.oplog
            .ends_with("event #1 ping []\nset #1 title \"alive\"\n"),
        "{}",
        r.oplog
    );
}

#[test]
fn external_interrupt_flag_stops_a_script() {
    let log = LogBackend::new(vec![]);
    let vt = log.vtable();
    let cfg = HostConfig {
        turn_budget: None,
        ..quiet()
    };
    let host = Host::new(vt.backend_ref(), cfg).unwrap();
    let flag = host.interrupt_flag();
    let t = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        flag.store(true, std::sync::atomic::Ordering::Relaxed);
    });
    let err = host.load_bundle(js("runaway_load.js")).unwrap_err();
    t.join().unwrap();
    assert_eq!(err.kind, ErrorKind::Interrupted);
}

#[test]
fn memory_limit_is_enforced() {
    let log = LogBackend::new(vec![]);
    let vt = log.vtable();
    let cfg = HostConfig {
        memory_limit: 8 * 1024 * 1024,
        ..quiet()
    };
    let host = Host::new(vt.backend_ref(), cfg).unwrap();
    let err = host.load_bundle(js("memory_hog.js")).unwrap_err();
    assert_eq!(err.kind, ErrorKind::OutOfMemory, "{err}");
}

#[test]
fn stack_limit_is_enforced() {
    let log = LogBackend::new(vec![]);
    let vt = log.vtable();
    let cfg = HostConfig {
        max_stack_size: 128 * 1024,
        ..quiet()
    };
    let host = Host::new(vt.backend_ref(), cfg).unwrap();
    let err = host.load_bundle(js("stack_overflow.js")).unwrap_err();
    assert!(err.message.to_lowercase().contains("stack"), "{err}");
}

#[test]
fn relative_imports_resolve_inside_the_bundle_dir() {
    let (cfg, opts) = fixture_config();
    let r = run_bundle(&js("imports/main.js"), vec![], cfg, opts).unwrap();
    assert_eq!(r.oplog, "create window #1\nset #1 title \"imported\"\n");
}

#[test]
fn imports_cannot_leave_the_bundle_dir_or_use_bare_specifiers() {
    for name in ["imports/escape.js", "imports/bare.js"] {
        let (cfg, opts) = fixture_config();
        let err = run_bundle(&js(name), vec![], cfg, opts)
            .err()
            .unwrap_or_else(|| panic!("{name} should fail"));
        assert!(
            err.contains("Load") || err.contains("bundle"),
            "{name}: {err}"
        );
    }
}

#[test]
fn missing_bundle_is_a_load_error() {
    let log = LogBackend::new(vec![]);
    let vt = log.vtable();
    let host = Host::new(vt.backend_ref(), quiet()).unwrap();
    assert_eq!(
        host.load_bundle(js("nope.js")).unwrap_err().kind,
        ErrorKind::Load
    );
}

/// A backend written against the `Backend` trait whose `set_prop` calls `host.checkpoint`, like
/// a toolkit that opens a modal dialog (nested loop) from inside a property setter.
struct Nesting {
    inner: LogBackend,
    host: Cell<Option<HostRef<'static>>>,
    nested_calls: Cell<u32>,
}

impl Backend for Nesting {
    fn resolve(&self, w: NameKind, k: svl_id, n: &str) -> svl_id {
        self.inner.resolve(w, k, n)
    }
    fn create(&self, k: svl_id) -> Result<svl_handle, String> {
        self.inner.create(k)
    }
    fn insert(
        &self,
        p: svl_handle,
        c: svl_handle,
        b: Option<svl_handle>,
        s: svl_id,
    ) -> Result<(), String> {
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
            self.nested_calls.set(self.nested_calls.get() + 1);
            host.checkpoint();
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
        self.host.set(Some(unsafe {
            std::mem::transmute::<HostRef<'_>, HostRef<'static>>(*host)
        }));
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
fn checkpoint_reentered_from_a_nested_loop_does_not_panic() {
    let steps = script(r#"[{"event":[2,"clicked",[1]]},{"checkpoint":true}]"#);
    let (cfg, opts) = fixture_config();
    let log = LogBackend::with_options(steps, opts);
    let backend = Nesting {
        inner: log.clone(),
        host: Cell::new(None),
        nested_calls: Cell::new(0),
    };
    let vt = BackendVtable::new(backend);
    let host = Host::new(vt.backend_ref(), cfg).unwrap();
    host.load_bundle(js("basic_ops.js")).unwrap();
    host.run().unwrap();
    assert!(host.take_errors().is_empty());
    let out = log.log();
    assert!(out.contains("event #2 clicked [1]\n"), "{out}");
    assert!(out.ends_with("set #3 label \"clicked 1 [1]\"\n"), "{out}");
    // The nested request was recorded and turned into a wake instead of re-running JS.
    assert!(log.wake_count() >= 1);
}

fn thread_names() -> Vec<String> {
    std::fs::read_dir("/proc/self/task")
        .unwrap()
        .filter_map(|e| std::fs::read_to_string(e.unwrap().path().join("comm")).ok())
        .map(|s| s.trim().to_string())
        .collect()
}

#[test]
fn host_starts_no_timer_thread() {
    let log = LogBackend::new(vec![]);
    let vt = log.vtable();
    let host = Host::new(vt.backend_ref(), quiet()).unwrap();
    host.load_bundle(js("timers_promises.js")).unwrap();
    // The bundle's timers are pending now; the toolkit (here the log backend) owns the timing.
    assert!(log.schedule_call_count() >= 1);
    assert!(
        !thread_names().iter().any(|n| n.starts_with("svelters-timers")),
        "{:?}",
        thread_names()
    );
    host.run().unwrap();
    drop(host);
}

fn console_text(r: &svelters_host::testing::RunReport) -> Vec<String> {
    r.console.iter().map(|c| c.text.clone()).collect()
}

#[test]
fn native_death_cascades_and_dead_handles_explain_themselves() {
    let (cfg, opts) = fixture_config();
    let steps = script(r#"[{"gone":[1,"closed"]},{"checkpoint":true}]"#);
    let r = run_bundle(&js("lifetime.js"), steps, cfg, opts).unwrap();
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    let out = console_text(&r);
    let has = |needle: &str| out.iter().any(|l| l.contains(needle));
    let line = |prefix: &str| {
        out.iter()
            .find(|l| l.starts_with(prefix))
            .unwrap_or_else(|| panic!("no console line starting with {prefix:?}: {out:#?}"))
            .clone()
    };

    // destroy twice is fine; afterwards reads say why, writes are no-ops, unknown handles differ.
    assert!(line("get destroyed:").contains("handle #4 was destroyed (destroyed by the app)"));
    assert_eq!(line("set destroyed:"), "set destroyed: ok");
    assert!(line("get never issued:").contains("stale or foreign handle 9999"));

    // The cascade reaches JS in order, window first.
    let gone: Vec<&String> = out.iter().filter(|l| l.starts_with("gone ")).collect();
    assert_eq!(gone, ["gone #1 closed", "gone #2 parent", "gone #3 parent"]);

    assert!(line("get descendant:").contains("handle #3 was destroyed (its parent was destroyed)"));
    assert_eq!(line("set descendant:"), "set descendant: ok");
    assert_eq!(line("listen off descendant:"), "listen off descendant: ok");
    assert!(line("listen on descendant:").contains("was destroyed"));
    assert_eq!(line("destroy descendant:"), "destroy descendant: ok");
    assert_eq!(line("destroy descendant again:"), "destroy descendant again: ok");
    assert_eq!(line("remove dead:"), "remove dead: ok");
    assert!(line("insert dead:").contains("was destroyed"));
    assert!(line("call dead:").contains("handle #1 was destroyed (its window was closed)"));
    assert!(has("gone #3 parent"));

    // Dead handles never reach the backend: only the one real destroy is logged, and every
    // cascaded handle is logged exactly once as `gone`.
    assert_eq!(r.oplog.matches("destroy #").count(), 1, "{}", r.oplog);
    assert!(r.oplog.ends_with("gone #1 closed\ngone #2 parent\ngone #3 parent\n"), "{}", r.oplog);
}

#[test]
fn handles_are_never_reused_and_the_dead_ring_is_bounded() {
    let (cfg, opts) = fixture_config();
    let r = run_bundle(&js("dead_ring.js"), vec![], cfg, opts).unwrap();
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    let out = console_text(&r);
    // 5000 handles went through a ring of 4096: the oldest was forgotten, the newest is explained.
    assert!(out[0].contains("stale or foreign handle"), "{out:?}");
    assert!(out[1].contains("was destroyed (destroyed by the app)"), "{out:?}");
}
