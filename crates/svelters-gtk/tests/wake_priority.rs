//! FU2: `wake` (the thread-safe "run a host checkpoint soon") on GTK.
//!
//! - A busy default-priority 1 ms timer must not starve it: the E1 lane saw an idle-priority wake
//!   never run while such a timer was busy, so worker completions and store updates were never
//!   delivered.
//! - Wakes coalesce: any number of requests before the loop turns cost one checkpoint.
//! - A checkpoint that keeps re-waking itself (a JS-thread future that yields, a completion backlog
//!   over the drain budget) must not starve redraw: a source at `GDK_PRIORITY_REDRAW` that becomes
//!   ready during a checkpoint runs within a few checkpoints.
//!
//! The host is a stub `svl_host` that counts checkpoints. Needs a display (tools/dev/headless).

use gtk::glib;
use std::cell::Cell;
use std::ffi::c_void;
use std::rc::Rc;
use std::sync::atomic::{AtomicPtr, AtomicU32, Ordering};
use std::time::{Duration, Instant};
use svelters_abi::*;
use svelters_gtk::testing::on_gtk;
use svelters_gtk::GtkBackend;

#[derive(Default)]
struct Stub {
    checkpoints: AtomicU32,
    /// Checkpoints that call `wake` again before returning.
    rewakes_left: AtomicU32,
    backend: AtomicPtr<svl_backend>,
    /// Called on the UI thread at the end of every checkpoint, with the checkpoint's number.
    on_checkpoint: Cell<Option<Box<dyn Fn(u32)>>>,
}

unsafe extern "C" fn h_post(_: *mut c_void, _: svl_handle, _: svl_id, _: *const svl_value, _: usize) {}
unsafe extern "C" fn h_checkpoint(p: *mut c_void) {
    let s = unsafe { &*(p as *const Stub) };
    let n = s.checkpoints.fetch_add(1, Ordering::SeqCst) + 1;
    if s.rewakes_left.load(Ordering::SeqCst) > 0 {
        s.rewakes_left.fetch_sub(1, Ordering::SeqCst);
        let b = s.backend.load(Ordering::SeqCst);
        unsafe { ((*b).wake)((*b).self_) };
    }
    if let Some(f) = s.on_checkpoint.take() {
        f(n);
        s.on_checkpoint.set(Some(f));
    }
}
unsafe extern "C" fn h_call_sync(_: *mut c_void, _: svl_handle, _: svl_id, _: *const svl_value, _: usize, _: *mut svl_value) -> i32 {
    SVL_SYNC_FAILED
}
unsafe extern "C" fn h_destroyed(_: *mut c_void, _: svl_handle, _: u32) {}
unsafe extern "C" fn h_invoke(_: *mut c_void, _: u64, _: *const svl_value, _: usize, out: *mut svl_value) -> i32 {
    unsafe { *out = svl_value::null() };
    0
}
unsafe extern "C" fn h_release(_: *mut c_void, _: u64) {}
unsafe extern "C" fn h_frame(_: *mut c_void) {}

struct Fixture {
    backend: GtkBackend,
    stub: Box<Stub>,
    _host: Box<svl_host>,
}

impl Fixture {
    fn new() -> Fixture {
        let backend = GtkBackend::new().expect("backend");
        let stub = Box::new(Stub::default());
        stub.backend.store(backend.as_raw() as *mut svl_backend, Ordering::SeqCst);
        let host = Box::new(svl_host {
            self_: &*stub as *const Stub as *mut c_void,
            post_event: h_post,
            checkpoint: h_checkpoint,
            call_sync: h_call_sync,
            on_destroyed: h_destroyed,
            invoke_callback: h_invoke,
            release_callback: h_release,
            frame: h_frame,
            prop_changed: None,
            dispatch_now: None,
        });
        unsafe { backend.attach_host(&*host) };
        Fixture { backend, stub, _host: host }
    }

    fn checkpoints(&self) -> u32 {
        self.stub.checkpoints.load(Ordering::SeqCst)
    }

    /// The vtable as an address another thread can call `wake` through.
    fn wake_handle(&self) -> WakeHandle {
        WakeHandle(self.backend.as_raw() as usize)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // Sources still queued must not reach the stub once it is gone.
        unsafe { self.backend.attach_host(std::ptr::null()) };
        drain();
    }
}

#[derive(Clone, Copy)]
struct WakeHandle(usize);

impl WakeHandle {
    /// `wake` from a fresh thread (the backend's `wake` is documented thread-safe).
    fn wake_from_thread(self, times: u32) {
        std::thread::spawn(move || {
            let b = self.0 as *const svl_backend;
            for _ in 0..times {
                unsafe { ((*b).wake)((*b).self_) };
            }
        })
        .join()
        .unwrap();
    }
}

fn drain() {
    let ctx = glib::MainContext::default();
    let end = Instant::now() + Duration::from_millis(50);
    while Instant::now() < end {
        while ctx.pending() {
            ctx.iteration(false);
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn spin(d: Duration) {
    let t = Instant::now();
    while t.elapsed() < d {
        std::hint::spin_loop();
    }
}

/// Iterates the default context (blocking) until `done` or `limit` passes; the time it took.
fn iterate_until(limit: Duration, done: impl Fn() -> bool) -> Duration {
    let ctx = glib::MainContext::default();
    let t0 = Instant::now();
    while !done() && t0.elapsed() < limit {
        ctx.iteration(true);
    }
    t0.elapsed()
}

#[test]
fn a_busy_default_priority_timer_does_not_starve_wake() {
    on_gtk(|| {
        let f = Fixture::new();
        drain();
        // A 1 ms timer at the default priority whose handler takes longer than its interval: it is ready
        // again at every iteration, like a busy native timer or I/O watch in a loaded app.
        let fired = Rc::new(Cell::new(0u32));
        let fc = fired.clone();
        let busy = glib::timeout_add_local_full(Duration::from_millis(1), glib::Priority::DEFAULT, move || {
            fc.set(fc.get() + 1);
            spin(Duration::from_micros(1500));
            glib::ControlFlow::Continue
        });
        iterate_until(Duration::from_millis(30), || false);
        let mut worst = Duration::ZERO;
        for round in 0..30 {
            let before = f.checkpoints();
            let t0 = Instant::now();
            f.wake_handle().wake_from_thread(1);
            iterate_until(Duration::from_secs(2), || f.checkpoints() > before);
            let took = t0.elapsed();
            assert!(
                f.checkpoints() > before,
                "round {round}: a wake from another thread was not delivered within {took:?} while a default-priority \
                 1 ms timer was busy ({} timer dispatches)",
                fired.get()
            );
            worst = worst.max(took);
        }
        busy.remove();
        eprintln!("worst wake latency under a busy 1 ms timer: {worst:?} ({} timer dispatches)", fired.get());
        assert!(worst < Duration::from_millis(100), "worst wake latency {worst:?}");
    });
}

#[test]
fn wakes_coalesce_until_the_checkpoint_runs() {
    on_gtk(|| {
        let f = Fixture::new();
        drain();
        let h = f.wake_handle();
        // 8 threads x 100 wakes before the loop turns: one pending wake, one checkpoint.
        let threads: Vec<_> = (0..8).map(|_| std::thread::spawn(move || h.wake_from_thread(100))).collect();
        threads.into_iter().for_each(|t| t.join().unwrap());
        drain();
        assert_eq!(f.checkpoints(), 1, "800 wakes before the loop ran");
        // The next wake after the checkpoint schedules a new one.
        h.wake_from_thread(1);
        drain();
        assert_eq!(f.checkpoints(), 2);
        // A wake from the UI thread itself, too.
        let b = f.backend.as_raw();
        unsafe { ((*b).wake)((*b).self_) };
        unsafe { ((*b).wake)((*b).self_) };
        drain();
        assert_eq!(f.checkpoints(), 3);
    });
}

/// A stand-in for GTK's redraw (the frame clock's layout and paint run at `GDK_PRIORITY_REDRAW`): a
/// checkpoint "changes a label", which makes a redraw-priority source ready, while every checkpoint
/// re-wakes. The redraw must not wait for the whole chain.
#[test]
fn rewaking_checkpoints_do_not_starve_redraw() {
    on_gtk(|| {
        let f = Fixture::new();
        drain();
        const CHAIN: u32 = 400;
        // (checkpoint that queued the redraw, checkpoints run before it was dispatched)
        let waits: Rc<std::cell::RefCell<Vec<(u32, u32)>>> = Rc::default();
        let queued = Rc::new(Cell::new(false));
        let stub: *const Stub = &*f.stub;
        let (w, q) = (waits.clone(), queued.clone());
        f.stub.on_checkpoint.set(Some(Box::new(move |n| {
            if n % 10 != 1 || q.replace(true) {
                return;
            }
            let (w, q) = (w.clone(), q.clone());
            glib::idle_add_local_full(glib::Priority::from(gtk::gdk::PRIORITY_REDRAW as i32), move || {
                // SAFETY: the fixture (and its stub) outlives the loop this test iterates.
                let now = unsafe { &*stub }.checkpoints.load(Ordering::SeqCst);
                w.borrow_mut().push((n, now - n));
                q.set(false);
                glib::ControlFlow::Break
            });
        })));
        f.stub.rewakes_left.store(CHAIN, Ordering::SeqCst);
        f.wake_handle().wake_from_thread(1);
        iterate_until(Duration::from_secs(10), || f.checkpoints() > CHAIN);
        drain();
        f.stub.on_checkpoint.set(None);
        assert_eq!(f.checkpoints(), CHAIN + 1, "the chain ran to its end");
        let waits = waits.borrow().clone();
        assert!(waits.len() >= 10, "redraws during the chain: {waits:?}");
        let worst = waits.iter().map(|w| w.1).max().unwrap();
        eprintln!("redraw waited at most {worst} checkpoints ({} redraws over {CHAIN} re-waking checkpoints)", waits.len());
        assert!(worst <= 8, "a redraw-priority source waited {worst} checkpoints: {waits:?}");
    });
}
