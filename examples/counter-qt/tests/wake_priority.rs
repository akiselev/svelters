//! FU2 on Qt: a busy 1 ms Qt timer (its handler takes longer than its interval, so it is due at every
//! loop turn) must not starve `wake` from another thread. Qt's `wake` posts a queued call to the
//! waker object; posted events are their own event source at the default priority (the GLib event
//! dispatcher's post-event source, or the first step of every `processEvents` on the Unix one), so
//! they run in the same loop turn as a busy timer.
//!
//! Runs inside `QCoreApplication::exec` (the backend's `run`), not a manual pump: the GLib dispatcher
//! treats timers differently while `exec` runs (after a timer fires, the next timers wait for an idle
//! turn, Qt's own guard against timer starvation). Needs a display: `tools/dev/headless`.

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use counter_qt::hooks;
use svelters_abi::*;

static CHECKPOINTS: AtomicU32 = AtomicU32::new(0);
static TICKS: AtomicU32 = AtomicU32::new(0);
static STOP: AtomicBool = AtomicBool::new(false);
static RESULT: Mutex<Option<Result<Duration, String>>> = Mutex::new(None);

fn be() -> &'static svl_backend {
    counter_qt::backend()
}

unsafe extern "C" fn post_event(_: *mut c_void, _: svl_handle, _: svl_id, _: *const svl_value, _: usize) {}
unsafe extern "C" fn on_destroyed(_: *mut c_void, _: svl_handle, _: u32) {}
unsafe extern "C" fn call_sync(_: *mut c_void, _: svl_handle, _: svl_id, _: *const svl_value, _: usize, _: *mut svl_value) -> i32 {
    -1
}
unsafe extern "C" fn checkpoint(_: *mut c_void) {
    CHECKPOINTS.fetch_add(1, Ordering::SeqCst);
}
unsafe extern "C" fn invoke_callback(_: *mut c_void, _: u64, _: *const svl_value, _: usize, _: *mut svl_value) -> i32 {
    -1
}
unsafe extern "C" fn release_callback(_: *mut c_void, _: u64) {}
unsafe extern "C" fn frame(_: *mut c_void) {}

fn spin(d: Duration) {
    let t = Instant::now();
    while t.elapsed() < d {
        std::hint::spin_loop();
    }
}

/// A 1 ms single-shot that re-arms itself and works 1.5 ms per tick; stops (and quits the loop) on STOP.
fn busy_tick() {
    if STOP.load(Ordering::SeqCst) {
        unsafe { (be().quit)(be().self_) };
        return;
    }
    TICKS.fetch_add(1, Ordering::SeqCst);
    spin(Duration::from_micros(1500));
    hooks::run_later(1, Box::new(busy_tick));
}

/// From another thread: 30 wakes, each waited for (up to 2 s); the worst latency, or the round that
/// was never delivered.
fn waker_thread() {
    std::thread::sleep(Duration::from_millis(50));
    let mut worst = Duration::ZERO;
    let mut out = Ok(());
    for round in 0..30 {
        // The timer is running: a tick since the last round.
        let ticks = TICKS.load(Ordering::SeqCst);
        let t = Instant::now();
        while TICKS.load(Ordering::SeqCst) == ticks && t.elapsed() < Duration::from_secs(1) {
            std::thread::sleep(Duration::from_micros(200));
        }
        let before = CHECKPOINTS.load(Ordering::SeqCst);
        let t0 = Instant::now();
        unsafe { (be().wake)(be().self_) };
        while CHECKPOINTS.load(Ordering::SeqCst) == before && t0.elapsed() < Duration::from_secs(2) {
            std::thread::sleep(Duration::from_micros(200));
        }
        let took = t0.elapsed();
        if CHECKPOINTS.load(Ordering::SeqCst) == before {
            out = Err(format!(
                "round {round}: a wake from another thread was not delivered within {took:?} while a 1 ms Qt timer was busy ({} ticks)",
                TICKS.load(Ordering::SeqCst)
            ));
            break;
        }
        worst = worst.max(took);
    }
    *RESULT.lock().unwrap() = Some(out.map(|()| worst));
    STOP.store(true, Ordering::SeqCst);
}

#[test]
fn qt_a_busy_timer_does_not_starve_wake() {
    let host = Box::leak(Box::new(svl_host {
        self_: std::ptr::null_mut(),
        post_event,
        checkpoint,
        call_sync,
        on_destroyed,
        invoke_callback,
        release_callback,
        frame,
        prop_changed: None,
        dispatch_now: None,
    }));
    unsafe { counter_qt::set_host(host) };
    hooks::run_later(0, Box::new(busy_tick));
    let t = std::thread::spawn(waker_thread);
    let rc = unsafe { (be().run)(be().self_, host) };
    assert_eq!(rc, 0);
    t.join().unwrap();
    let ticks = TICKS.load(Ordering::SeqCst);
    match RESULT.lock().unwrap().take().expect("the waker thread finished") {
        Ok(worst) => {
            eprintln!("Qt: worst wake latency under a busy 1 ms timer: {worst:?} ({ticks} ticks)");
            assert!(ticks >= 30, "the busy timer kept running: {ticks} ticks");
            assert!(worst < Duration::from_millis(100), "worst wake latency {worst:?}");
        }
        Err(e) => panic!("{e}"),
    }
}
