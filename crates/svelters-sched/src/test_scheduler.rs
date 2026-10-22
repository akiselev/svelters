//! The deterministic scheduler: one thread, one seeded RNG, simulated time.
//!
//! **What waits in the ready set.** Jobs from [`Spawner::spawn`] (a host's `#[js(worker)]` calls, a
//! `worker()` future's closure), wakes ([`Clock::wake_at`] deadlines that came due,
//! [`simulate_random_delay`] yields, [`Timer`]s), the test's own one-shot actions
//! ([`TestScheduler::spawn_local`]: a reload, a click, an abort) and the foregrounds whose waker
//! fired or whose poll says they want a turn (a host's checkpoint, an egui frame). [`TestScheduler::tick`]
//! picks one of them with the seeded RNG and runs it to completion on the calling thread.
//!
//! **What is never reordered.** A foreground's turn runs whole: the JS microtask queue a checkpoint
//! drains, the events it delivers in order, the timers it fires in deadline order all stay inside
//! the host. Only the order *between* independent units changes: which job finishes first, whether a
//! completion lands before or after a timer turn, whether a reload comes before or after a pending
//! import's checkpoint. Those are orders a real thread pool and a real event loop can produce.
//!
//! **Time.** [`TestScheduler::now`] starts at zero and moves only through [`TestScheduler::advance_clock`],
//! [`TestScheduler::run_for`], [`TestScheduler::run_until_idle`] or a parked
//! [`TestScheduler::block_until`], plus the random turn time [`TestScheduler::set_turn_time`] gives
//! foreground turns (a busy UI thread). Spawned jobs are ready at once unless
//! [`TestScheduler::set_job_delay`] gives them a random simulated duration.
//!
//! **Parking.** `block_until` parks when its condition is false and nothing is ready. With parking
//! allowed (the default) the clock jumps to the next deadline; after [`TestScheduler::forbid_parking`]
//! a park is a test failure that lists what is pending (a test that must finish without waiting
//! for time to pass). A turn limit ([`TestScheduler::set_max_steps`]) catches livelocks and a
//! simulated-time limit ([`TestScheduler::set_idle_limit`]) catches live intervals.

use crate::{Clock, Job, JobKind, JobMeta, Rng, Spawner};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::task::{Context, Poll, Wake, Waker};
use std::time::Duration;

const DEFAULT_MAX_STEPS: u64 = 1_000_000;
const DEFAULT_IDLE_LIMIT: Duration = Duration::from_secs(600);
/// [`simulate_random_delay`] yields 0..=this many times.
const MAX_RANDOM_YIELDS: u64 = 8;

enum Work {
    Job(Job),
    Wake(Waker),
    /// A test action kept on the scheduler's own thread (index into `TestScheduler::locals`).
    Local(usize),
}

struct Task {
    label: String,
    work: Work,
}

struct Timed {
    task: Task,
    /// The `wake_at` key this deadline belongs to.
    key: Option<u64>,
}

type Slot = (Duration, u64);

struct State {
    seed: u64,
    rng: Rng,
    now: Duration,
    seq: u64,
    ready: Vec<Task>,
    timed: BTreeMap<Slot, Timed>,
    keyed: HashMap<u64, Slot>,
    /// Per foreground: its waker fired since its last turn.
    woken: Vec<bool>,
    job_delay: Duration,
    turn_time: Duration,
    forbid_parking: bool,
    max_steps: u64,
    idle_limit: Duration,
    steps: u64,
}

impl State {
    fn promote_due(&mut self) {
        while let Some((&slot, _)) = self.timed.first_key_value() {
            if slot.0 > self.now {
                break;
            }
            let t = self.timed.remove(&slot).expect("first key");
            if let Some(k) = t.key
                && self.keyed.get(&k) == Some(&slot)
            {
                self.keyed.remove(&k);
            }
            self.ready.push(t.task);
        }
    }

    fn schedule(&mut self, at: Duration, task: Task, key: Option<u64>) -> Slot {
        self.seq += 1;
        let slot = (at, self.seq);
        self.timed.insert(slot, Timed { task, key });
        slot
    }

    fn next_deadline(&self) -> Option<Duration> {
        self.timed.keys().next().map(|s| s.0)
    }

    fn job_delay(&mut self) -> Duration {
        let max = self.job_delay;
        self.random_duration(max)
    }

    /// Uniform in `0..=max` at microsecond resolution.
    fn random_duration(&mut self, max: Duration) -> Duration {
        if max.is_zero() {
            return Duration::ZERO;
        }
        let max = max.as_micros().min(u64::MAX as u128 - 1) as u64;
        Duration::from_micros(self.rng.below(max + 1))
    }
}

pub(crate) struct Shared {
    st: Mutex<State>,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.st.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// The thread-safe half of a [`TestScheduler`]: what hosts, jobs and futures hold. It implements
/// [`Spawner`] and [`Clock`].
#[derive(Clone)]
pub struct SchedHandle(Arc<Shared>);

impl SchedHandle {
    pub fn now(&self) -> Duration {
        self.0.lock().now
    }

    pub fn seed(&self) -> u64 {
        self.0.lock().seed
    }

    /// A random number in `0..n` from the schedule's RNG.
    pub fn random_below(&self, n: u64) -> u64 {
        self.0.lock().rng.below(n)
    }

    /// True with probability `p`, from the schedule's RNG.
    pub fn random_bool(&self, p: f64) -> bool {
        self.0.lock().rng.bool(p)
    }

    /// A future that yields to the scheduler a random number of times (0..=8), each yield a ready
    /// task the RNG orders against everything else.
    pub fn simulate_random_delay(&self) -> Delay {
        Delay { left: None, handle: Some(self.clone()) }
    }

    /// A future ready once the simulated clock has moved `d` past now.
    pub fn timer(&self, d: Duration) -> Timer {
        Timer { at: self.now() + d, handle: self.clone() }
    }

    fn push_ready(&self, label: &str, work: Work) {
        self.0.lock().ready.push(Task { label: label.to_string(), work });
    }
}

impl Spawner for SchedHandle {
    fn spawn(&self, job: Job) {
        self.spawn_with(JobMeta::default(), job);
    }

    fn spawn_with(&self, meta: JobMeta, job: Job) {
        let label = meta.label.unwrap_or_else(|| {
            match meta.kind {
                JobKind::Worker => "job (worker)",
                JobKind::Blocking => "job (blocking)",
                _ => "job",
            }
            .to_string()
        });
        let mut st = self.0.lock();
        let delay = st.job_delay();
        let task = Task { label, work: Work::Job(job) };
        if delay.is_zero() {
            st.ready.push(task);
        } else {
            let at = st.now + delay;
            st.schedule(at, task, None);
        }
    }
}

impl Clock for SchedHandle {
    fn now(&self) -> Duration {
        SchedHandle::now(self)
    }

    fn wake_at(&self, key: u64, at: Option<Duration>, waker: &Waker) {
        let mut st = self.0.lock();
        if let Some(slot) = st.keyed.remove(&key) {
            st.timed.remove(&slot);
        }
        if let Some(at) = at {
            let slot = st.schedule(at, Task { label: format!("wake_at #{key}"), work: Work::Wake(waker.clone()) }, Some(key));
            st.keyed.insert(key, slot);
        }
    }
}

thread_local! {
    static CURRENT: RefCell<Vec<SchedHandle>> = const { RefCell::new(Vec::new()) };
}

/// The innermost [`TestScheduler`] alive on this thread, if any.
pub fn current() -> Option<SchedHandle> {
    CURRENT.with(|c| c.borrow().last().cloned())
}

/// [`SchedHandle::simulate_random_delay`] on this thread's [`TestScheduler`]; ready at once when
/// none is running, so code under test may call it unconditionally.
pub fn simulate_random_delay() -> Delay {
    Delay { left: None, handle: current() }
}

/// See [`SchedHandle::simulate_random_delay`].
pub struct Delay {
    left: Option<u64>,
    handle: Option<SchedHandle>,
}

impl Future for Delay {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let Some(h) = self.handle.clone() else { return Poll::Ready(()) };
        let left = match self.left {
            Some(n) => n,
            None => h.random_below(MAX_RANDOM_YIELDS + 1),
        };
        if left == 0 {
            self.left = Some(0);
            return Poll::Ready(());
        }
        self.left = Some(left - 1);
        h.push_ready("simulate_random_delay", Work::Wake(cx.waker().clone()));
        Poll::Pending
    }
}

/// See [`SchedHandle::timer`].
pub struct Timer {
    at: Duration,
    handle: SchedHandle,
}

impl Future for Timer {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let mut st = self.handle.0.lock();
        if st.now >= self.at {
            return Poll::Ready(());
        }
        st.schedule(self.at, Task { label: "timer".into(), work: Work::Wake(cx.waker().clone()) }, None);
        Poll::Pending
    }
}

/// An event loop the scheduler gives turns to: a host's checkpoint, an egui frame. See
/// [`TestScheduler::foreground`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Foreground(usize);

struct Fg {
    name: String,
    run: Option<Box<dyn FnMut()>>,
    poll: Option<Box<dyn Fn() -> bool>>,
}

struct FgWake {
    shared: Weak<Shared>,
    id: usize,
}

impl Wake for FgWake {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        if let Some(s) = self.shared.upgrade() {
            s.lock().woken[self.id] = true;
        }
    }
}

enum Pick {
    Task(Task),
    /// A foreground turn and the simulated time it takes.
    Fg(usize, Duration),
}

struct Running<'a>(&'a Cell<bool>);

impl Drop for Running<'_> {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

/// The deterministic scheduler (see the module docs). Lives on the test's thread; hand
/// [`TestScheduler::spawner`], [`TestScheduler::clock`] and a foreground's
/// [`TestScheduler::waker`] to the code under test.
pub struct TestScheduler {
    handle: SchedHandle,
    fgs: RefCell<Vec<Fg>>,
    #[allow(clippy::type_complexity)]
    locals: RefCell<Vec<Option<Box<dyn FnOnce()>>>>,
    running: Cell<bool>,
}

impl TestScheduler {
    pub fn new(seed: u64) -> TestScheduler {
        let st = State {
            seed,
            rng: Rng::new(seed),
            now: Duration::ZERO,
            seq: 0,
            ready: Vec::new(),
            timed: BTreeMap::new(),
            keyed: HashMap::new(),
            woken: Vec::new(),
            job_delay: Duration::ZERO,
            turn_time: Duration::ZERO,
            forbid_parking: false,
            max_steps: DEFAULT_MAX_STEPS,
            idle_limit: DEFAULT_IDLE_LIMIT,
            steps: 0,
        };
        let handle = SchedHandle(Arc::new(Shared { st: Mutex::new(st) }));
        CURRENT.with(|c| c.borrow_mut().push(handle.clone()));
        TestScheduler { handle, fgs: RefCell::new(Vec::new()), locals: RefCell::new(Vec::new()), running: Cell::new(false) }
    }

    pub fn seed(&self) -> u64 {
        self.handle.seed()
    }

    /// Simulated time since the scheduler was created.
    pub fn now(&self) -> Duration {
        self.handle.now()
    }

    /// Turns run so far.
    pub fn steps(&self) -> u64 {
        self.handle.0.lock().steps
    }

    pub fn handle(&self) -> SchedHandle {
        self.handle.clone()
    }

    pub fn spawner(&self) -> Arc<dyn Spawner> {
        Arc::new(self.handle.clone())
    }

    pub fn clock(&self) -> Arc<dyn Clock> {
        Arc::new(self.handle.clone())
    }

    // ---- configuration ----

    /// Spawned jobs take a random simulated duration in `0..=max` (default zero: ready at once), so
    /// their completions interleave with timers.
    pub fn set_job_delay(&self, max: Duration) {
        self.handle.0.lock().job_delay = max;
    }

    /// Each foreground turn takes a random simulated duration in `0..=max` (default zero): a busy
    /// UI thread, during which deadlines pass, so the next turn finds several things due at once (a
    /// completion and a timer in one checkpoint). The only way a tick moves the clock.
    pub fn set_turn_time(&self, max: Duration) {
        self.handle.0.lock().turn_time = max;
    }

    /// A [`TestScheduler::block_until`] that parks fails instead of advancing the clock.
    pub fn forbid_parking(&self) {
        self.handle.0.lock().forbid_parking = true;
    }

    pub fn allow_parking(&self) {
        self.handle.0.lock().forbid_parking = false;
    }

    /// Turns after which the scheduler declares a livelock (default 1,000,000).
    pub fn set_max_steps(&self, n: u64) {
        self.handle.0.lock().max_steps = n;
    }

    /// Simulated time `run_until_idle`/`block_until` may skip before declaring that something never
    /// stops (default 10 minutes).
    pub fn set_idle_limit(&self, d: Duration) {
        self.handle.0.lock().idle_limit = d;
    }

    // ---- randomness for the test itself ----

    /// A random number in `0..n` from the schedule's RNG (so the test's own choices replay too).
    pub fn random_below(&self, n: u64) -> u64 {
        self.handle.random_below(n)
    }

    pub fn random_bool(&self, p: f64) -> bool {
        self.handle.random_bool(p)
    }

    pub fn simulate_random_delay(&self) -> Delay {
        self.handle.simulate_random_delay()
    }

    pub fn timer(&self, d: Duration) -> Timer {
        self.handle.timer(d)
    }

    // ---- foregrounds and test actions ----

    /// Registers an event loop. It gets a turn when its [`TestScheduler::waker`] fired since its
    /// last turn, or when its poll ([`TestScheduler::set_poll`]) says so; a turn calls its runner
    /// ([`TestScheduler::set_runner`]). Register first, hand out the waker, then set the runner (a
    /// host needs the waker before it exists, the runner needs the host).
    pub fn foreground(&self, name: &str) -> Foreground {
        let mut fgs = self.fgs.borrow_mut();
        fgs.push(Fg { name: name.to_string(), run: None, poll: None });
        self.handle.0.lock().woken.push(false);
        Foreground(fgs.len() - 1)
    }

    /// Wakes `fg`: its next turn becomes ready. Thread-safe and cheap; give it to the host as its
    /// wake hook.
    pub fn waker(&self, fg: Foreground) -> Waker {
        Waker::from(Arc::new(FgWake { shared: Arc::downgrade(&self.handle.0), id: fg.0 }))
    }

    pub fn set_runner(&self, fg: Foreground, run: impl FnMut() + 'static) {
        self.fgs.borrow_mut()[fg.0].run = Some(Box::new(run));
    }

    /// An extra readiness check, evaluated before every pick (egui: the last frame asked for an
    /// immediate repaint). Keep it cheap and side-effect free.
    pub fn set_poll(&self, fg: Foreground, poll: impl Fn() -> bool + 'static) {
        self.fgs.borrow_mut()[fg.0].poll = Some(Box::new(poll));
    }

    /// Queues a one-shot action of the test (a reload, a click, an abort) as a ready task, so the
    /// RNG decides where it lands among the other work. Runs on this thread; need not be `Send`.
    pub fn spawn_local(&self, label: &str, f: impl FnOnce() + 'static) {
        let i = self.add_local(f);
        self.handle.push_ready(label, Work::Local(i));
    }

    /// [`TestScheduler::spawn_local`] once the simulated clock reaches now + `d`.
    pub fn spawn_local_after(&self, d: Duration, label: &str, f: impl FnOnce() + 'static) {
        let i = self.add_local(f);
        let mut st = self.handle.0.lock();
        let at = st.now + d;
        st.schedule(at, Task { label: label.to_string(), work: Work::Local(i) }, None);
    }

    fn add_local(&self, f: impl FnOnce() + 'static) -> usize {
        let mut l = self.locals.borrow_mut();
        l.push(Some(Box::new(f)));
        l.len() - 1
    }

    // ---- running ----

    /// Runs one ready unit, chosen by the RNG. False when nothing is ready (parked).
    pub fn tick(&self) -> bool {
        assert!(!self.running.get(), "TestScheduler: tick/run_* called from inside a scheduled task");
        let polled: Vec<bool> = self.fgs.borrow().iter().map(|f| f.run.is_some() && f.poll.as_ref().is_some_and(|p| p())).collect();
        let pick = {
            let mut st = self.handle.0.lock();
            st.promote_due();
            let fgs = self.fgs.borrow();
            let fg_ready: Vec<usize> = (0..fgs.len()).filter(|&i| fgs[i].run.is_some() && (polled[i] || st.woken[i])).collect();
            let n = st.ready.len() + fg_ready.len();
            if n == 0 {
                return false;
            }
            st.steps += 1;
            if st.steps > st.max_steps {
                let what = describe(&st, &fgs);
                drop(st);
                panic!("TestScheduler: no park after {} turns (livelock?)\n{what}", self.steps());
            }
            let k = st.rng.below(n as u64) as usize;
            if k < st.ready.len() {
                Pick::Task(st.ready.swap_remove(k))
            } else {
                let id = fg_ready[k - st.ready.len()];
                st.woken[id] = false;
                let max = st.turn_time;
                Pick::Fg(id, st.random_duration(max))
            }
        };
        self.running.set(true);
        let _running = Running(&self.running);
        match pick {
            Pick::Task(t) => match t.work {
                Work::Job(job) => job(),
                Work::Wake(w) => w.wake(),
                Work::Local(i) => {
                    let f = self.locals.borrow_mut()[i].take().expect("a local task runs once");
                    f();
                }
            },
            Pick::Fg(id, took) => {
                let mut run = self.fgs.borrow_mut()[id].run.take().expect("runner");
                run();
                self.fgs.borrow_mut()[id].run = Some(run);
                self.handle.0.lock().now += took;
            }
        }
        true
    }

    /// Runs ready units until none is left. Moves the clock only by foreground turn time.
    pub fn run_until_parked(&self) {
        while self.tick() {}
    }

    /// Moves the clock forward by `d`. Deadlines that come due are ready for the next tick; nothing
    /// runs here.
    pub fn advance_clock(&self, d: Duration) {
        let mut st = self.handle.0.lock();
        st.now += d;
    }

    /// Lets `d` of simulated time pass the way a real loop would: runs until parked, jumps to the next
    /// deadline within `d`, and again, ending at now + `d` parked.
    pub fn run_for(&self, d: Duration) {
        let target = self.now() + d;
        loop {
            self.run_until_parked();
            let mut st = self.handle.0.lock();
            match st.next_deadline() {
                Some(at) if at <= target => st.now = st.now.max(at),
                _ => {
                    st.now = st.now.max(target);
                    drop(st);
                    self.run_until_parked();
                    return;
                }
            }
        }
    }

    /// Runs until nothing is ready and nothing is scheduled, jumping the clock from deadline to
    /// deadline. Panics past the idle limit (something never stops: a live interval, a frame loop).
    pub fn run_until_idle(&self) {
        let limit = self.now() + self.handle.0.lock().idle_limit;
        loop {
            self.run_until_parked();
            let mut st = self.handle.0.lock();
            let Some(at) = st.next_deadline() else { return };
            if at > limit {
                let what = self.describe_locked(&st);
                drop(st);
                panic!("TestScheduler: still busy past the idle limit (now {:?}, next deadline {at:?})\n{what}", self.now());
            }
            st.now = st.now.max(at);
        }
    }

    /// Runs until `cond` holds. When nothing is ready the scheduler parks: allowed, the clock jumps to
    /// the next deadline; forbidden ([`TestScheduler::forbid_parking`]), or with nothing scheduled, or
    /// past the idle limit, it panics naming `what` and listing what is pending.
    pub fn block_until(&self, what: &str, mut cond: impl FnMut() -> bool) {
        let limit = self.now() + self.handle.0.lock().idle_limit;
        loop {
            if cond() {
                return;
            }
            if self.tick() {
                continue;
            }
            let mut st = self.handle.0.lock();
            let why = match st.next_deadline() {
                Some(at) if !st.forbid_parking && at <= limit => {
                    st.now = st.now.max(at);
                    continue;
                }
                Some(at) if st.forbid_parking => format!("parked with parking forbidden (next deadline {at:?})"),
                Some(at) => format!("past the idle limit (next deadline {at:?})"),
                None => "deadlock: nothing is ready or scheduled".to_string(),
            };
            let pending = self.describe_locked(&st);
            drop(st);
            panic!("TestScheduler: waiting for {what}: {why}\n{pending}");
        }
    }

    /// True when nothing is ready (foreground polls included) and nothing is scheduled.
    pub fn is_idle(&self) -> bool {
        let polled = self.fgs.borrow().iter().any(|f| f.run.is_some() && f.poll.as_ref().is_some_and(|p| p()));
        let st = self.handle.0.lock();
        !polled && st.ready.is_empty() && st.timed.is_empty() && !st.woken.iter().any(|w| *w)
    }

    /// What is pending, for failure messages.
    pub fn pending(&self) -> String {
        let st = self.handle.0.lock();
        self.describe_locked(&st)
    }

    fn describe_locked(&self, st: &State) -> String {
        describe(st, &self.fgs.borrow())
    }
}

fn describe(st: &State, fgs: &[Fg]) -> String {
    let mut s = format!("  seed {} at {:?}, {} turns\n", st.seed, st.now, st.steps);
    let _ = writeln!(s, "  ready: {:?}", st.ready.iter().map(|t| t.label.as_str()).collect::<Vec<_>>());
    let _ = writeln!(s, "  scheduled: {:?}", st.timed.iter().map(|(k, t)| format!("{:?} {}", k.0, t.task.label)).collect::<Vec<_>>());
    let woken: Vec<&str> = fgs.iter().zip(&st.woken).filter(|(_, w)| **w).map(|(f, _)| f.name.as_str()).collect();
    let _ = writeln!(s, "  woken foregrounds: {woken:?}");
    s
}

impl Drop for TestScheduler {
    fn drop(&mut self) {
        CURRENT.with(|c| {
            let mut c = c.borrow_mut();
            if let Some(i) = c.iter().rposition(|h| Arc::ptr_eq(&h.0, &self.handle.0)) {
                c.remove(i);
            }
        });
        // Pending jobs and wakers are dropped outside the lock (a job's drop may spawn or wake).
        let (ready, timed) = {
            let mut st = self.handle.0.lock();
            (std::mem::take(&mut st.ready), std::mem::take(&mut st.timed))
        };
        drop(ready);
        drop(timed);
    }
}
