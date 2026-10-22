//! SCHED1: the TestScheduler's own contract: same seed, same schedule; seeds differ; time moves only
//! when told; wake_at replaces and cancels; foregrounds get turns when woken; parking rules; the
//! SEED/ITERATIONS parsing.

use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::time::Duration;
use svelters_sched::{JobKind, JobMeta, Seeds, Spawner, TestScheduler, run_seeds, wake_key};

fn order_for(seed: u64) -> Vec<u32> {
    let s = TestScheduler::new(seed);
    let out = Arc::new(Mutex::new(Vec::new()));
    for i in 0..8 {
        let out = out.clone();
        s.spawner().spawn(Box::new(move || out.lock().unwrap().push(i)));
    }
    s.run_until_parked();
    out.lock().unwrap().clone()
}

#[test]
fn the_same_seed_replays_the_same_order_and_seeds_differ() {
    assert_eq!(order_for(7), order_for(7));
    let distinct: std::collections::HashSet<Vec<u32>> = (0..20).map(order_for).collect();
    assert!(distinct.len() > 10, "{distinct:?}");
    let mut sorted = order_for(3);
    sorted.sort();
    assert_eq!(sorted, (0..8).collect::<Vec<_>>());
}

#[test]
fn jobs_spawned_by_jobs_run_too_and_labels_show_in_pending() {
    let s = TestScheduler::new(1);
    let h = s.handle();
    let hits = Arc::new(Mutex::new(0));
    let hits2 = hits.clone();
    s.spawner().spawn_with(
        JobMeta::new(JobKind::Blocking).label("outer"),
        Box::new(move || {
            *hits2.lock().unwrap() += 1;
            let hits3 = hits2.clone();
            h.spawn(Box::new(move || *hits3.lock().unwrap() += 10));
        }),
    );
    assert!(s.pending().contains("outer"), "{}", s.pending());
    s.run_until_parked();
    assert_eq!(*hits.lock().unwrap(), 11);
    assert!(s.is_idle());
}

#[test]
fn time_moves_only_when_told_and_wake_at_replaces_and_cancels() {
    let s = TestScheduler::new(0);
    let fg = s.foreground("loop");
    let turns = Rc::new(RefCell::new(Vec::new()));
    let (t, h) = (turns.clone(), s.handle());
    s.set_runner(fg, move || t.borrow_mut().push(h.now()));
    let clock = s.clock();
    let key = wake_key();
    clock.wake_at(key, Some(Duration::from_millis(10)), &s.waker(fg));
    clock.wake_at(key, Some(Duration::from_millis(30)), &s.waker(fg)); // replaces 10 ms
    s.run_until_parked();
    assert!(turns.borrow().is_empty());
    s.advance_clock(Duration::from_millis(20));
    s.run_until_parked();
    assert!(turns.borrow().is_empty(), "the 10 ms request was replaced");
    s.run_for(Duration::from_millis(20));
    assert_eq!(*turns.borrow(), [Duration::from_millis(30)]);
    assert_eq!(s.now(), Duration::from_millis(40));
    clock.wake_at(key, Some(Duration::from_millis(50)), &s.waker(fg));
    clock.wake_at(key, None, &s.waker(fg)); // cancels
    s.run_until_idle();
    assert_eq!(turns.borrow().len(), 1);
    assert_eq!(s.now(), Duration::from_millis(40));
}

#[test]
fn job_delays_interleave_completions_with_deadlines() {
    // With random job durations, a job can finish before or after a 5 ms deadline; across seeds both happen.
    let mut before = 0;
    let mut after = 0;
    for seed in 0..40 {
        let s = TestScheduler::new(seed);
        s.set_job_delay(Duration::from_millis(10));
        let log = Arc::new(Mutex::new(Vec::new()));
        let l = log.clone();
        s.spawner().spawn(Box::new(move || l.lock().unwrap().push("job")));
        let l = log.clone();
        s.spawn_local_after(Duration::from_millis(5), "deadline", move || l.lock().unwrap().push("deadline"));
        s.run_until_idle();
        match log.lock().unwrap().as_slice() {
            ["job", "deadline"] => before += 1,
            ["deadline", "job"] => after += 1,
            other => panic!("{other:?}"),
        }
    }
    assert!(before > 0 && after > 0, "before {before} after {after}");
}

/// A minimal executor: one future polled by a foreground whenever its waker fires.
fn spawn_future(s: &TestScheduler, fut: impl Future<Output = ()> + 'static) -> Rc<RefCell<bool>> {
    let fg = s.foreground("future");
    let waker = s.waker(fg);
    let done = Rc::new(RefCell::new(false));
    let d = done.clone();
    let mut fut: Option<Pin<Box<dyn Future<Output = ()>>>> = Some(Box::pin(fut));
    s.set_runner(fg, move || {
        if let Some(f) = fut.as_mut()
            && let Poll::Ready(()) = f.as_mut().poll(&mut Context::from_waker(&waker))
        {
            fut = None;
            *d.borrow_mut() = true;
        }
    });
    s.waker(fg).wake();
    done
}

#[test]
fn timers_and_random_delays_are_futures_on_the_simulated_clock() {
    let s = TestScheduler::new(5);
    let h = s.handle();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let seen2 = seen.clone();
    let done = spawn_future(&s, async move {
        h.simulate_random_delay().await;
        seen2.borrow_mut().push(h.now());
        h.timer(Duration::from_millis(25)).await;
        seen2.borrow_mut().push(h.now());
        svelters_sched::simulate_random_delay().await;
    });
    s.run_until_parked();
    assert_eq!(*seen.borrow(), [Duration::ZERO]);
    assert!(!*done.borrow());
    s.block_until("the future", || *done.borrow());
    assert_eq!(*seen.borrow(), [Duration::ZERO, Duration::from_millis(25)]);
}

#[test]
fn forbidden_parking_fails_with_what_is_pending() {
    let s = TestScheduler::new(0);
    s.forbid_parking();
    s.spawn_local_after(Duration::from_secs(1), "later", || {});
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| s.block_until("never", || false)));
    let msg = r.unwrap_err().downcast::<String>().unwrap();
    assert!(msg.contains("parking forbidden") && msg.contains("later"), "{msg}");
    s.allow_parking();
    let hit = Rc::new(RefCell::new(false));
    let h = hit.clone();
    s.spawn_local_after(Duration::from_secs(2), "flag", move || *h.borrow_mut() = true);
    s.block_until("the flag", || *hit.borrow());
    assert_eq!(s.now(), Duration::from_secs(2));
}

#[test]
fn a_livelock_hits_the_turn_limit() {
    let s = TestScheduler::new(0);
    s.set_max_steps(100);
    let fg = s.foreground("spinner");
    s.set_runner(fg, || {});
    s.set_poll(fg, || true);
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| s.run_until_parked()));
    let msg = r.unwrap_err().downcast::<String>().unwrap();
    assert!(msg.contains("livelock"), "{msg}");
}

#[test]
fn seeds_from_the_environment_variables() {
    assert_eq!(Seeds::new(None, None, 12), Seeds { first: 0, count: 12 });
    assert_eq!(Seeds::new(None, Some(50), 12), Seeds { first: 0, count: 50 });
    assert_eq!(Seeds::new(Some(41), None, 12), Seeds { first: 41, count: 1 });
    assert_eq!(Seeds::new(Some(41), Some(3), 12).iter().collect::<Vec<_>>(), [41, 42, 43]);
}

#[test]
fn run_seeds_runs_each_seed_on_a_fresh_scheduler() {
    let mut seen = Vec::new();
    let n = run_seeds("run_seeds_runs_each_seed_on_a_fresh_scheduler", 5, |s| {
        assert_eq!(s.now(), Duration::ZERO);
        assert!(svelters_sched::current().is_some_and(|h| h.seed() == s.seed()));
        seen.push(s.seed());
    });
    assert_eq!(seen.len() as u64, n);
    assert_eq!(seen[0], Seeds::from_env(5).first);
    assert!(svelters_sched::current().is_none());
}

#[test]
fn a_waker_fired_from_another_thread_reaches_the_foreground() {
    let s = TestScheduler::new(0);
    let fg = s.foreground("remote");
    let turns = Rc::new(RefCell::new(0));
    let t = turns.clone();
    s.set_runner(fg, move || *t.borrow_mut() += 1);
    let w: Waker = s.waker(fg);
    std::thread::spawn(move || w.wake()).join().unwrap();
    s.run_until_parked();
    assert_eq!(*turns.borrow(), 1);
}
