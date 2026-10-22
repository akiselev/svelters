//! ZU3 (plans/milestones/ZU3-ui-thread.md, ZED-LESSONS §9 #3): UI-thread frame monitoring.
//!
//! A [`FrameMonitor`] brackets each frame on the UI thread ([`FrameMonitor::begin`] and
//! [`FrameMonitor::end`]) and does two independent things:
//!
//! - **Hang watchdog.** A `svelters-hang` thread checks that a frame that began also ends within
//!   N ms. When one doesn't, it records the UI thread's stack and logs it through `tracing`
//!   (target `svelters_host::frames`, level `warn`); when that frame finally ends, the UI thread
//!   logs how long it took together with DX1's crash context ([`crate::crash::context`]: the last
//!   host ops and JS errors). Between frames the UI thread is idle in the event loop, which is not
//!   a hang, so idle time is never reported.
//!   Stack capture (Unix): the watchdog sends the UI thread a signal (`SIGURG`, which no part of
//!   svelters uses and whose default action is to ignore it); the handler walks the stack with
//!   `backtrace::trace_unsynchronized` into a static buffer of return addresses (no allocation,
//!   no locks of ours) and the watchdog resolves the symbols on its own thread. If the app already
//!   has a `SIGURG` handler, or on Windows, the hang is logged without a stack.
//! - **Frame-time histogram** (`SVELTERS_MEASUREMENTS=1`). The UI-thread time of each frame,
//!   `begin` to `end`, in µs. When the monitor drops (the window closed), a summary is logged and
//!   the samples are written as JSON to `SVELTERS_MEASUREMENTS_OUT` (default
//!   `<temp>/svelters-frames-<pid>.json`); `tools/dev/frame-histogram a.json b.json` compares two
//!   runs.
//!
//! Configuration ([`FrameConfig::from_env`]): `SVELTERS_HANG_MS=<n>` sets the threshold, `0`
//! turns the watchdog off; unset, it is on at [`DEFAULT_HANG_MS`] in debug builds and off in
//! release builds.
//!
//! **Off costs nothing.** With both parts off, [`FrameMonitor::start`] returns `None` before it
//! allocates or spawns anything, and the runner keeps an `Option<FrameMonitor>`: per frame that is
//! one branch on `None`. Release builds are off unless one of the variables is set.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// The hang threshold in debug builds when `SVELTERS_HANG_MS` is unset.
pub const DEFAULT_HANG_MS: u64 = 500;

/// Frames kept by the histogram (4 MB); later frames are counted, not stored.
const MAX_SAMPLES: usize = 1 << 20;

/// What a [`FrameMonitor`] does. Both `None`: it is off.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FrameConfig {
    /// Report a frame that has not ended after this long.
    pub hang: Option<Duration>,
    /// Record frame times and write them here when the monitor drops.
    pub measurements: Option<PathBuf>,
}

impl FrameConfig {
    /// From `SVELTERS_HANG_MS`, `SVELTERS_MEASUREMENTS` and `SVELTERS_MEASUREMENTS_OUT`, with the
    /// watchdog's default on in debug builds only.
    pub fn from_env() -> FrameConfig {
        let var = |k: &str| std::env::var(k).ok();
        FrameConfig::from_vars(
            var("SVELTERS_HANG_MS").as_deref(),
            var("SVELTERS_MEASUREMENTS").as_deref(),
            var("SVELTERS_MEASUREMENTS_OUT").as_deref(),
            cfg!(debug_assertions),
        )
    }

    /// [`FrameConfig::from_env`] on given values; `dev` is whether this is a debug build.
    pub fn from_vars(hang_ms: Option<&str>, measurements: Option<&str>, out: Option<&str>, dev: bool) -> FrameConfig {
        let hang = match hang_ms.map(str::trim) {
            Some(v) => match v.parse::<u64>() {
                Ok(0) => None,
                Ok(ms) => Some(Duration::from_millis(ms)),
                Err(_) => {
                    tracing::warn!(target: "svelters_host::frames", value = v, "SVELTERS_HANG_MS is not a number of ms; watchdog off");
                    None
                }
            },
            None if dev => Some(Duration::from_millis(DEFAULT_HANG_MS)),
            None => None,
        };
        let on = measurements.is_some_and(|v| !matches!(v.trim(), "" | "0" | "false"));
        let measurements = on.then(|| match out {
            Some(p) if !p.is_empty() => PathBuf::from(p),
            _ => std::env::temp_dir().join(format!("svelters-frames-{}.json", std::process::id())),
        });
        FrameConfig { hang, measurements }
    }

    pub fn is_off(&self) -> bool {
        self.hang.is_none() && self.measurements.is_none()
    }
}

/// See the module docs. Create it on the UI thread; it is not `Send`.
pub struct FrameMonitor {
    /// Start of the current frame (`begin` without `end` yet).
    frame: Option<Instant>,
    watchdog: Option<Watchdog>,
    histogram: Option<Histogram>,
    _not_send: std::marker::PhantomData<*const ()>,
}

impl FrameMonitor {
    /// [`FrameMonitor::start`] with [`FrameConfig::from_env`].
    pub fn from_env() -> Option<FrameMonitor> {
        FrameMonitor::start(FrameConfig::from_env())
    }

    /// Starts monitoring the calling thread, which must be the UI thread. `None` when `config` is
    /// off: nothing was allocated and no thread started.
    pub fn start(config: FrameConfig) -> Option<FrameMonitor> {
        if config.is_off() {
            return None;
        }
        let watchdog = config.hang.and_then(Watchdog::start);
        let histogram = config.measurements.map(|out| Histogram { out, us: Vec::new(), dropped: 0 });
        if watchdog.is_none() && histogram.is_none() {
            return None;
        }
        Some(FrameMonitor { frame: None, watchdog, histogram, _not_send: std::marker::PhantomData })
    }

    /// A frame starts. A second `begin` before `end` keeps the first start (a frame split over two
    /// callbacks, e.g. eframe's `logic` then `ui`).
    pub fn begin(&mut self) {
        if self.frame.is_some() {
            return;
        }
        let now = Instant::now();
        self.frame = Some(now);
        if let Some(w) = &self.watchdog {
            w.begin(now);
        }
    }

    /// The frame that began has ended.
    pub fn end(&mut self) {
        let Some(start) = self.frame.take() else { return };
        let took = start.elapsed();
        if let Some(w) = &self.watchdog {
            w.end(took);
        }
        if let Some(h) = &mut self.histogram {
            h.push(took);
        }
    }

    /// The recorded frame times in µs (with `SVELTERS_MEASUREMENTS`).
    pub fn samples(&self) -> Option<&[u32]> {
        self.histogram.as_ref().map(|h| h.us.as_slice())
    }

    /// Whether the hang watchdog runs.
    pub fn watching(&self) -> bool {
        self.watchdog.is_some()
    }
}

impl Drop for FrameMonitor {
    fn drop(&mut self) {
        if let Some(h) = &self.histogram {
            h.finish();
        }
        // the watchdog's Drop stops its thread
    }
}

struct Histogram {
    out: PathBuf,
    us: Vec<u32>,
    dropped: u64,
}

impl Histogram {
    fn push(&mut self, d: Duration) {
        if self.us.len() < MAX_SAMPLES {
            self.us.push(d.as_micros().min(u32::MAX as u128) as u32);
        } else {
            self.dropped += 1;
        }
    }

    // ZU3: writes the measurements once, when the window has closed (after the last frame).
    #[allow(clippy::disallowed_methods)]
    fn finish(&self) {
        let s = summary(&self.us);
        tracing::info!(target: "svelters_host::frames", path = %self.out.display(), dropped = self.dropped, "frame times: {s}");
        let json = serde_json::json!({
            "version": 1,
            "unit": "us",
            "pid": std::process::id(),
            "dropped": self.dropped,
            "frames": self.us,
        });
        if let Err(e) = std::fs::write(&self.out, json.to_string()) {
            tracing::warn!(target: "svelters_host::frames", path = %self.out.display(), error = %e, "frame times not written");
        }
    }
}

/// `n=…, p50=…, p90=…, p99=…, max=…` in ms.
pub fn summary(us: &[u32]) -> String {
    if us.is_empty() {
        return "n=0".into();
    }
    let mut v = us.to_vec();
    v.sort_unstable();
    let p = |q: f64| v[((v.len() - 1) as f64 * q).round() as usize] as f64 / 1000.0;
    format!("n={}, p50={:.2} ms, p90={:.2} ms, p99={:.2} ms, max={:.2} ms", v.len(), p(0.5), p(0.9), p(0.99), p(1.0))
}

/// State shared by the UI thread and the `svelters-hang` thread.
struct Shared {
    epoch: Instant,
    threshold: Duration,
    /// Start of the running frame in ns since `epoch`, plus one; 0 between frames.
    busy_since: AtomicU64,
    /// Frames begun.
    seq: AtomicU64,
    /// The frame (`seq`) last reported as hung, plus one.
    reported: AtomicU64,
    stop: AtomicBool,
}

struct Watchdog {
    shared: Arc<Shared>,
    thread: std::thread::Thread,
}

impl Watchdog {
    fn start(threshold: Duration) -> Option<Watchdog> {
        let shared = Arc::new(Shared {
            epoch: Instant::now(),
            threshold,
            busy_since: AtomicU64::new(0),
            seq: AtomicU64::new(0),
            reported: AtomicU64::new(0),
            stop: AtomicBool::new(false),
        });
        let ui = sampler::current();
        let s = shared.clone();
        match std::thread::Builder::new().name("svelters-hang".into()).spawn(move || watch(&s, ui)) {
            Ok(h) => Some(Watchdog { shared, thread: h.thread().clone() }),
            Err(e) => {
                tracing::warn!(target: "svelters_host::frames", error = %e, "hang watchdog not started");
                None
            }
        }
    }

    fn begin(&self, now: Instant) {
        let s = &self.shared;
        s.seq.fetch_add(1, Ordering::AcqRel);
        let ns = now.saturating_duration_since(s.epoch).as_nanos() as u64 + 1;
        s.busy_since.store(ns, Ordering::Release);
    }

    fn end(&self, took: Duration) {
        let s = &self.shared;
        s.busy_since.store(0, Ordering::Release);
        if s.reported.load(Ordering::Acquire) == s.seq.load(Ordering::Acquire) + 1 {
            let context = crate::crash::context();
            tracing::warn!(
                target: "svelters_host::frames",
                ms = took.as_millis() as u64,
                "UI thread hang ended: the frame took {} ms\n{context}",
                took.as_millis()
            );
        }
    }
}

impl Drop for Watchdog {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        // a frame left open (a panic between begin and end) is not a hang to sample any more
        self.shared.busy_since.store(0, Ordering::Release);
        self.thread.unpark();
    }
}

/// The `svelters-hang` thread: polls four times per threshold (5 ms to 250 ms) until stopped.
fn watch(s: &Shared, ui: sampler::ThreadId) {
    let poll = (s.threshold / 4).clamp(Duration::from_millis(5), Duration::from_millis(250));
    while !s.stop.load(Ordering::Acquire) {
        std::thread::park_timeout(poll);
        let seq = s.seq.load(Ordering::Acquire);
        let since = s.busy_since.load(Ordering::Acquire);
        if since == 0 || s.reported.load(Ordering::Acquire) == seq + 1 {
            continue;
        }
        let busy = s.epoch.elapsed().saturating_sub(Duration::from_nanos(since - 1));
        if busy < s.threshold {
            continue;
        }
        if s.stop.load(Ordering::Acquire) {
            break;
        }
        let stack = sampler::capture(ui);
        // the frame may have ended while the stack was taken: then it is not reported
        if s.seq.load(Ordering::Acquire) != seq || s.busy_since.load(Ordering::Acquire) != since {
            continue;
        }
        // From here on the UI thread logs the hang's end (its duration and the crash context). That
        // line can come before this report, which symbolises the stack first (it takes a while).
        s.reported.store(seq + 1, Ordering::Release);
        let stack = match stack {
            Ok(frames) => sampler::format(&frames),
            Err(why) => format!("  (no stack: {why})\n"),
        };
        tracing::warn!(
            target: "svelters_host::frames",
            ms = busy.as_millis() as u64,
            threshold_ms = s.threshold.as_millis() as u64,
            "UI thread hang: a frame has run for {} ms (threshold {} ms); UI thread stack:\n{stack}",
            busy.as_millis(),
            s.threshold.as_millis()
        );
    }
}

#[cfg(unix)]
mod sampler {
    //! Samples another thread's stack: a signal to that thread, whose handler records the return
    //! addresses into a static buffer.
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    pub(super) type ThreadId = libc::pthread_t;

    const SIGNAL: libc::c_int = libc::SIGURG;
    const MAX_FRAMES: usize = 128;
    /// How long the watchdog waits for the handler to run.
    const WAIT: Duration = Duration::from_millis(200);

    static IPS: [AtomicUsize; MAX_FRAMES] = [const { AtomicUsize::new(0) }; MAX_FRAMES];
    static LEN: AtomicUsize = AtomicUsize::new(0);
    static DONE: AtomicBool = AtomicBool::new(false);
    /// One capture at a time; also guards the handler's installation.
    static CAPTURE: Mutex<Option<Result<(), &'static str>>> = Mutex::new(None);

    pub(super) fn current() -> ThreadId {
        // SAFETY: pthread_self has no preconditions.
        unsafe { libc::pthread_self() }
    }

    extern "C" fn handler(_sig: libc::c_int) {
        // Async-signal context: atomics and the unwinder only, no allocation, no locks of ours.
        let mut n = 0;
        // SAFETY: `trace_unsynchronized` is called by one thread at a time: the UI thread, while the
        // watchdog holds CAPTURE.
        unsafe {
            backtrace::trace_unsynchronized(|f| {
                if n == MAX_FRAMES {
                    return false;
                }
                IPS[n].store(f.ip() as usize, Ordering::Relaxed);
                n += 1;
                true
            });
        }
        LEN.store(n, Ordering::Relaxed);
        DONE.store(true, Ordering::Release);
    }

    /// Installs the handler unless the app has its own `SIGURG` handler.
    fn install() -> Result<(), &'static str> {
        // SAFETY: plain sigaction calls on zeroed/initialised structs; the handler is async-signal-safe
        // as far as the unwinder is (the approach of sampling profilers such as pprof-rs).
        unsafe {
            let mut old: libc::sigaction = std::mem::zeroed();
            if libc::sigaction(SIGNAL, std::ptr::null(), &mut old) != 0 {
                return Err("sigaction failed");
            }
            if old.sa_sigaction != libc::SIG_DFL && old.sa_sigaction != libc::SIG_IGN {
                return Err("the app has its own SIGURG handler");
            }
            let mut sa: libc::sigaction = std::mem::zeroed();
            sa.sa_sigaction = handler as extern "C" fn(libc::c_int) as libc::sighandler_t;
            sa.sa_flags = libc::SA_RESTART;
            libc::sigemptyset(&mut sa.sa_mask);
            if libc::sigaction(SIGNAL, &sa, std::ptr::null_mut()) != 0 {
                return Err("sigaction failed");
            }
        }
        Ok(())
    }

    // ZU3: runs on the svelters-hang thread; the sleep is the bounded wait for the signal handler.
    #[allow(clippy::disallowed_methods)]
    pub(super) fn capture(thread: ThreadId) -> Result<Vec<usize>, &'static str> {
        let mut guard = CAPTURE.lock().unwrap_or_else(|e| e.into_inner());
        (*guard.get_or_insert_with(install))?;
        DONE.store(false, Ordering::Release);
        // SAFETY: `thread` is the UI thread, alive while its FrameMonitor (which owns this watchdog)
        // is; the handler for SIGNAL is installed above.
        if unsafe { libc::pthread_kill(thread, SIGNAL) } != 0 {
            return Err("pthread_kill failed");
        }
        let end = Instant::now() + WAIT;
        while !DONE.load(Ordering::Acquire) {
            if Instant::now() >= end {
                return Err("the UI thread did not run the signal handler (blocked signals?)");
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        let n = LEN.load(Ordering::Relaxed);
        Ok(IPS[..n].iter().map(|a| a.load(Ordering::Relaxed)).collect())
    }

    /// One line per frame, symbolised; the handler's own frames are dropped.
    pub(super) fn format(ips: &[usize]) -> String {
        let mut lines: Vec<String> = Vec::new();
        for &ip in ips {
            let mut name = None;
            let mut place = None;
            backtrace::resolve(ip as *mut std::ffi::c_void, |sym| {
                if name.is_none() {
                    name = sym.name().map(|n| format!("{n:#}"));
                    place = sym.filename().map(|f| format!("{}:{}", f.display(), sym.lineno().unwrap_or(0)));
                }
            });
            let name = name.unwrap_or_else(|| format!("{ip:#x}"));
            lines.push(match place {
                Some(p) => format!("{name}\n        at {p}"),
                None => name,
            });
        }
        // drop everything up to and including the signal trampoline / our handler
        let skip = lines
            .iter()
            .rposition(|l| l.contains("frames::sampler::handler") || l.contains("__restore_rt") || l.contains("_sigtramp"))
            .map_or(0, |i| i + 1);
        let mut out = String::new();
        for (i, l) in lines[skip..].iter().enumerate() {
            out.push_str(&format!("  {i:>3}: {l}\n"));
        }
        out
    }
}

#[cfg(not(unix))]
mod sampler {
    pub(super) type ThreadId = ();

    pub(super) fn current() -> ThreadId {}

    pub(super) fn capture(_thread: ThreadId) -> Result<Vec<usize>, &'static str> {
        Err("stack capture is not implemented on this platform")
    }

    pub(super) fn format(_ips: &[usize]) -> String {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_default_is_off_and_starts_nothing() {
        let cfg = FrameConfig::from_vars(None, None, None, false);
        assert!(cfg.is_off());
        // `start` returns before allocating or spawning: no monitor, no `svelters-hang` thread
        assert!(FrameMonitor::start(cfg).is_none());
        // explicit zeroes are off in debug builds too
        assert!(FrameConfig::from_vars(Some("0"), Some("0"), None, true).is_off());
    }

    #[test]
    fn env_values() {
        let dev = FrameConfig::from_vars(None, None, None, true);
        assert_eq!(dev.hang, Some(Duration::from_millis(DEFAULT_HANG_MS)));
        assert_eq!(dev.measurements, None);
        let rel = FrameConfig::from_vars(Some("80"), Some("1"), Some("/x/f.json"), false);
        assert_eq!(rel.hang, Some(Duration::from_millis(80)));
        assert_eq!(rel.measurements, Some(PathBuf::from("/x/f.json")));
        assert!(FrameConfig::from_vars(Some("soon"), None, None, true).hang.is_none());
    }

    #[test]
    fn histogram_records_frames_and_writes_json() {
        let out = std::env::temp_dir().join(format!("svelters-zu3-hist-{}.json", std::process::id()));
        let mut m = FrameMonitor::start(FrameConfig { hang: None, measurements: Some(out.clone()) }).unwrap();
        assert!(!m.watching());
        for _ in 0..3 {
            m.begin();
            m.begin(); // a split frame keeps its first start
            m.end();
            m.end(); // an unmatched end is ignored
        }
        assert_eq!(m.samples().unwrap().len(), 3);
        drop(m);
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
        assert_eq!(v["unit"], "us");
        assert_eq!(v["frames"].as_array().unwrap().len(), 3);
        let _ = std::fs::remove_file(&out);
        assert!(summary(&[1000, 2000, 3000]).starts_with("n=3, p50=2.00 ms"));
    }

    /// A frame that blocks past the threshold is reported once, with the UI thread's stack.
    #[test]
    fn watchdog_reports_a_hung_frame_with_its_stack() {
        let lines = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let sink = lines.clone();
        let subscriber = Capture(sink);
        let _g = tracing::subscriber::set_default(subscriber.clone());
        // the watchdog thread logs through the global dispatcher; this test owns it in its process
        tracing::subscriber::set_global_default(subscriber).expect("global subscriber");
        let mut m = FrameMonitor::start(FrameConfig { hang: Some(Duration::from_millis(40)), measurements: None }).unwrap();
        assert!(m.watching());
        m.begin();
        hung_frame_body(Duration::from_millis(400));
        m.end();
        drop(m);
        // the watchdog may still be logging (symbolising takes a while)
        let end = Instant::now() + Duration::from_secs(10);
        while Instant::now() < end && !lines.lock().unwrap().iter().any(|l| l.contains("UI thread hang: a frame")) {
            std::thread::sleep(Duration::from_millis(10));
        }
        let lines = lines.lock().unwrap();
        let hangs: Vec<_> = lines.iter().filter(|l| l.contains("UI thread hang: a frame has run")).collect();
        assert_eq!(hangs.len(), 1, "{lines:#?}");
        assert!(lines.iter().any(|l| l.contains("UI thread hang ended")), "{lines:#?}");
        #[cfg(unix)]
        assert!(hangs[0].contains("hung_frame_body"), "the stack names the blocking frame: {}", hangs[0]);
    }

    #[inline(never)]
    fn hung_frame_body(d: Duration) {
        let end = Instant::now() + d;
        while Instant::now() < end {
            std::thread::sleep(Duration::from_millis(5));
        }
        std::hint::black_box(end);
    }

    /// A subscriber that keeps each event's formatted message.
    #[derive(Clone)]
    struct Capture(Arc<std::sync::Mutex<Vec<String>>>);

    impl tracing::Subscriber for Capture {
        fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
            true
        }
        fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
            tracing::span::Id::from_u64(1)
        }
        fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}
        fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
        fn event(&self, e: &tracing::Event<'_>) {
            struct V(String);
            impl tracing::field::Visit for V {
                fn record_debug(&mut self, f: &tracing::field::Field, v: &dyn std::fmt::Debug) {
                    if f.name() == "message" {
                        self.0 = format!("{v:?}");
                    }
                }
            }
            let mut v = V(String::new());
            e.record(&mut v);
            self.0.lock().unwrap().push(v.0);
        }
        fn enter(&self, _: &tracing::span::Id) {}
        fn exit(&self, _: &tracing::span::Id) {}
    }
}
