//! FU3 on Qt: `wake` coalesces, as GTK's does since FU2. One posted checkpoint is pending at a time;
//! wakes while it is pending add nothing. The ticket is cleared before the checkpoint runs, so a wake
//! requested during (or after) the checkpoint gets a checkpoint of its own.
//!
//! Runs inside `QCoreApplication::exec` (the backend's `run`). Needs a display: `tools/dev/headless`.
//! Its own test binary (one Qt application per process), like `wake_priority.rs`.

use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

use counter_qt::hooks;
use svelters_abi::*;

static CHECKPOINTS: AtomicU32 = AtomicU32::new(0);
/// Wakes the next checkpoints request from inside themselves.
static REWAKE: AtomicU32 = AtomicU32::new(0);
static ERRORS: Mutex<Vec<String>> = Mutex::new(Vec::new());

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
    if REWAKE.load(Ordering::SeqCst) > 0 {
        REWAKE.fetch_sub(1, Ordering::SeqCst);
        unsafe { (be().wake)(be().self_) };
    }
}
unsafe extern "C" fn invoke_callback(_: *mut c_void, _: u64, _: *const svl_value, _: usize, _: *mut svl_value) -> i32 {
    -1
}
unsafe extern "C" fn release_callback(_: *mut c_void, _: u64) {}
unsafe extern "C" fn frame(_: *mut c_void) {}

fn wake() {
    unsafe { (be().wake)(be().self_) };
}

fn wake_from_threads(threads: u32, times: u32) {
    let ts: Vec<_> = (0..threads)
        .map(|_| {
            std::thread::spawn(move || {
                for _ in 0..times {
                    wake();
                }
            })
        })
        .collect();
    ts.into_iter().for_each(|t| t.join().unwrap());
}

fn expect(what: &str, base: u32, delta: u32) {
    let got = CHECKPOINTS.load(Ordering::SeqCst) - base;
    if got != delta {
        ERRORS.lock().unwrap().push(format!("{what}: {got} checkpoints, expected {delta}"));
    }
}

/// Each step runs on the UI thread, with the loop turning between steps (50 ms is ample for a posted event).
fn step1() {
    // `run`'s own first checkpoint has run by now.
    let base = CHECKPOINTS.load(Ordering::SeqCst);
    // 8 threads x 100 wakes while the UI thread is blocked here: one posted checkpoint.
    wake_from_threads(8, 100);
    hooks::run_later(50, Box::new(move || step2(base)));
}

fn step2(base: u32) {
    expect("800 wakes from 8 threads before the loop turned", base, 1);
    let base = CHECKPOINTS.load(Ordering::SeqCst);
    // After the checkpoint ran, a new wake (from another thread) schedules a new one.
    wake_from_threads(1, 1);
    hooks::run_later(50, Box::new(move || step3(base)));
}

fn step3(base: u32) {
    expect("one wake from another thread after the checkpoint", base, 1);
    let base = CHECKPOINTS.load(Ordering::SeqCst);
    // From the UI thread itself: two wakes, one checkpoint.
    wake();
    wake();
    hooks::run_later(50, Box::new(move || step4(base)));
}

fn step4(base: u32) {
    expect("two wakes from the UI thread", base, 1);
    let base = CHECKPOINTS.load(Ordering::SeqCst);
    // A wake requested during the checkpoint is not lost: the ticket is cleared before the checkpoint runs.
    REWAKE.store(2, Ordering::SeqCst);
    wake();
    hooks::run_later(50, Box::new(move || step5(base)));
}

fn step5(base: u32) {
    expect("a checkpoint that re-wakes twice in a chain", base, 3);
    unsafe { (be().quit)(be().self_) };
}

#[test]
fn qt_wakes_coalesce_until_the_checkpoint_runs() {
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
    hooks::run_later(50, Box::new(step1));
    let rc = unsafe { (be().run)(be().self_, host) };
    assert_eq!(rc, 0);
    let errors = ERRORS.lock().unwrap();
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}
