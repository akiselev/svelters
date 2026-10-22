//! E3 services: started with the app's first host (lazy ones on first import), never restarted by a hot
//! reload (the new runtime gets the same Rust store), stopped and joined by `shutdown_services`. A test
//! binary of its own: services are process-wide.

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};
use svelters::prelude::*;
use svelters_backend_log::LogBackend;
use svelters_host::{Host, testing::fixture_config};

static TICKER_STARTS: AtomicU32 = AtomicU32::new(0);
static LAZY_STARTS: AtomicU32 = AtomicU32::new(0);
static TICKER_STOPPED: AtomicU32 = AtomicU32::new(0);

/// What the ticker service publishes.
#[svelters::store(module = "svc")]
#[derive(Default)]
pub struct Ticks {
    pub n: u64,
}

/// Counts up every 2 ms on a thread of its own until shutdown.
#[svelters::service(module = "svc")]
fn ticker(cx: ServiceCx) -> Store<Ticks> {
    TICKER_STARTS.fetch_add(1, Ordering::SeqCst);
    let store = Store::new(Ticks::default());
    let s = store.clone();
    cx.thread(move |stop| {
        while !stop.wait(Duration::from_millis(2)) {
            s.write(TicksField::N, |t| t.n += 1);
        }
        TICKER_STOPPED.fetch_add(1, Ordering::SeqCst);
    });
    store
}

/// Started when JS first imports `svelters:app/lazy`.
#[svelters::service(module = "lazy", lazy)]
fn idle(_cx: ServiceCx) -> Store<Ticks> {
    LAZY_STARTS.fetch_add(1, Ordering::SeqCst);
    Store::new(Ticks { n: 42 })
}

fn wait(what: &str, cond: impl Fn() -> bool) {
    let end = Instant::now() + Duration::from_secs(20);
    while !cond() {
        assert!(Instant::now() < end, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn services_start_with_the_app_survive_reloads_and_stop_at_shutdown() {
    assert_eq!(
        TICKER_STARTS.load(Ordering::SeqCst),
        0,
        "nothing runs before the app starts"
    );
    let dir = std::env::temp_dir().join(format!("svelters-services-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let v1 = r#"
import { ticker } from 'svelters:app/svc';
const H = globalThis.__svelters_host;
globalThis.__svelters_changed = (b) => { for (let i = 0; i < b.length; i += 3) if (b[i + 1] === 'n') globalThis.last = b[i + 2]; };
H.observe(ticker.svlHandle, 'n', true);
globalThis.report = () => console.log('v1 sees', globalThis.last > 0);
"#;
    std::fs::write(dir.join("main.js"), v1).unwrap();
    let (cfg, _) = fixture_config();
    let log = LogBackend::new(Vec::new());
    let vt = log.vtable();
    let host = Host::new(vt.backend_ref(), cfg).unwrap();
    // The first host starts the eager services; the lazy one waits for its import.
    assert_eq!(TICKER_STARTS.load(Ordering::SeqCst), 1);
    assert_eq!(LAZY_STARTS.load(Ordering::SeqCst), 0);
    assert_eq!(
        svelters::exports::service::running_services(),
        vec!["svc/ticker".to_string()]
    );
    host.load_bundle(dir.join("main.js")).unwrap();
    let turn = || {
        host.checkpoint();
        host.drain_jobs();
        host.take_console()
            .into_iter()
            .map(|l| l.text)
            .collect::<Vec<_>>()
    };
    let mut lines = Vec::new();
    let end = Instant::now() + Duration::from_secs(20);
    while host.reactive_stats().store_delivered < 3 {
        assert!(Instant::now() < end, "no ticks delivered");
        lines.extend(turn());
        std::thread::sleep(Duration::from_millis(3));
    }

    // A reload: the service is not restarted; the new runtime reads the same store, still counting.
    std::fs::write(
        dir.join("main.js"),
        r#"
import { ticker } from 'svelters:app/svc';
import { idle } from 'svelters:app/lazy';
const H = globalThis.__svelters_host;
const first = ticker.n;
globalThis.__svelters_changed = (b) => {
  for (let i = 0; i < b.length; i += 3) if (b[i + 1] === 'n' && b[i + 2] > first && !globalThis.saw) { globalThis.saw = true; console.log('v2 counting on from', first > 0); }
};
H.observe(ticker.svlHandle, 'n', true);
console.log('lazy', idle.n);
"#,
    )
    .unwrap();
    let ok = std::rc::Rc::new(std::cell::Cell::new(false));
    let flag = ok.clone();
    host.on_reload(move |r| flag.set(r.ok));
    host.request_reload();
    unsafe { svelters_abi::HostRef::from_raw(host.host_callbacks()) }.checkpoint();
    assert!(ok.get(), "{:#?}", host.take_errors());
    let end = Instant::now() + Duration::from_secs(20);
    while !lines.iter().any(|l| l == "v2 counting on from true") {
        assert!(Instant::now() < end, "{lines:?}");
        lines.extend(turn());
        std::thread::sleep(Duration::from_millis(3));
    }
    assert!(lines.iter().any(|l| l == "lazy 42"), "{lines:?}");
    assert_eq!(
        TICKER_STARTS.load(Ordering::SeqCst),
        1,
        "a reload never restarts a service"
    );
    assert_eq!(
        LAZY_STARTS.load(Ordering::SeqCst),
        1,
        "the lazy service started on its import"
    );
    assert!(host.take_errors().is_empty());

    // Shutdown stops and joins the service thread.
    let report = svelters::exports::shutdown_services(Duration::from_secs(5));
    assert_eq!(report.services, 2);
    assert_eq!(report.joined, 1);
    assert!(report.timed_out.is_empty());
    wait("ticker stopped", || {
        TICKER_STOPPED.load(Ordering::SeqCst) == 1
    });
    drop(host);
}
