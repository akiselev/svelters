//! AU7: named busy sources (cadatomic WAITS §3.3): the settle between acts, built from sources that
//! say who is busy, how much and why.
//!
//! - Each source is registered by the code that owns the work (the job registry, tool sessions,
//!   the HTTP client, the worker pool) with an id, a [`BusyKind`] and a [`ClockDomain`], and a
//!   probe returning a [`BusyReport`] (`count`, `reason`, `since`). Probes must not block; they run
//!   on the UI thread after every frame a session runs.
//! - **Foreground** sources block [`Session::wait_settled`](crate::Session::wait_settled);
//!   **background** sources never do (polling, file watchers, long-lived connections, an
//!   indeterminate spinner). A source must choose: the registry refuses one without a kind or a
//!   clock domain.
//! - A foreground [`ClockDomain::Wall`] source that is busy holds the virtual clock (Compose's
//!   `autoAdvance`): a virtual timeout never cuts off real work.
//! - A settle timeout lists every busy foreground source with its count, reason and how long it has
//!   been busy, longest first (Detox's `--debug-synchronization`: the first repeated one is the bug).

use crate::Error;
use crate::journal::ClockDomain;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

/// Whether a source holds the settle (WAITS §3.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BusyKind {
    /// A user action waits on it: blocks settle.
    Foreground,
    /// Never blocks settle (polling, watchers, long-lived connections, spinners).
    Background,
}

impl BusyKind {
    pub fn as_str(self) -> &'static str {
        match self {
            BusyKind::Foreground => "foreground",
            BusyKind::Background => "background",
        }
    }
}

/// What a probe reports: idle when `count` is 0.
#[non_exhaustive]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BusyReport {
    /// Work in flight (jobs, requests, messages).
    pub count: u32,
    /// Why, for people: `"drc on board 1"`, `"GET route:parts"`.
    pub reason: String,
    /// When it went busy (`None`: the registry counts from the first poll that saw it busy).
    pub since: Option<Instant>,
}

impl BusyReport {
    pub fn idle() -> BusyReport {
        BusyReport::default()
    }

    pub fn busy(count: u32, reason: impl Into<String>) -> BusyReport {
        BusyReport { count, reason: reason.into(), since: None }
    }

    pub fn since(mut self, at: Instant) -> BusyReport {
        self.since = Some(at);
        self
    }

    pub fn is_busy(&self) -> bool {
        self.count > 0
    }
}

type Probe = Arc<dyn Fn() -> BusyReport + Send + Sync>;

/// A source to register: an id, a probe, and (required) a kind and a clock domain.
pub struct BusySource {
    id: String,
    kind: Option<BusyKind>,
    domain: Option<ClockDomain>,
    probe: Probe,
}

impl BusySource {
    /// Choose [`BusySource::foreground`] or [`BusySource::background`] and a clock domain before
    /// registering.
    pub fn new(id: impl Into<String>, probe: impl Fn() -> BusyReport + Send + Sync + 'static) -> BusySource {
        BusySource { id: id.into(), kind: None, domain: None, probe: Arc::new(probe) }
    }

    pub fn kind(mut self, kind: BusyKind) -> BusySource {
        self.kind = Some(kind);
        self
    }

    pub fn foreground(self) -> BusySource {
        self.kind(BusyKind::Foreground)
    }

    pub fn background(self) -> BusySource {
        self.kind(BusyKind::Background)
    }

    pub fn domain(mut self, domain: ClockDomain) -> BusySource {
        self.domain = Some(domain);
        self
    }

    /// External work (workers, network, disk, thread-pool jobs).
    pub fn wall(self) -> BusySource {
        self.domain(ClockDomain::Wall)
    }

    /// UI-internal work (animations, timers, debounces).
    pub fn virtual_clock(self) -> BusySource {
        self.domain(ClockDomain::Virtual)
    }
}

/// A busy source as a poll saw it.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub struct BusyStatus {
    pub id: String,
    pub kind: BusyKind,
    pub domain: ClockDomain,
    pub count: u32,
    pub reason: String,
    /// How long it has been busy, in its domain's ms.
    pub busy_for_ms: f64,
}

impl BusyStatus {
    pub fn to_json(&self) -> Value {
        json!({
            "id": self.id, "kind": self.kind.as_str(), "domain": self.domain.as_str(),
            "count": self.count, "reason": self.reason, "busy_for_ms": self.busy_for_ms.round(),
        })
    }

    /// `jobs ×2 (drc, route) for 1500 ms wall`.
    pub fn describe(&self) -> String {
        let mut s = format!("{} ×{}", self.id, self.count);
        if !self.reason.is_empty() {
            s.push_str(&format!(" ({})", self.reason));
        }
        s.push_str(&format!(" for {} ms {}", self.busy_for_ms.round(), self.domain.as_str()));
        s
    }
}

struct Entry {
    id: String,
    kind: BusyKind,
    domain: ClockDomain,
    probe: Probe,
    /// The first poll that saw it busy: wall instant and virtual ms.
    first_busy: Option<(Instant, f64)>,
}

/// The app's busy sources. A handle: clones share the registry; sessions and emitters on any thread
/// may hold one.
#[derive(Clone, Default)]
pub struct BusyRegistry {
    inner: Arc<Mutex<Vec<Entry>>>,
}

impl std::fmt::Debug for BusyRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.lock().iter().map(|e| (&e.id, e.kind, e.domain))).finish()
    }
}

impl BusyRegistry {
    pub fn new() -> BusyRegistry {
        BusyRegistry::default()
    }

    fn lock(&self) -> MutexGuard<'_, Vec<Entry>> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Whether `other` is a handle to the same registry.
    pub fn same(&self, other: &BusyRegistry) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }

    /// Registers `source`. Fails with `busy.invalid` when it has no kind or clock domain, or its id
    /// is taken.
    pub fn register(&self, source: BusySource) -> Result<(), Error> {
        let BusySource { id, kind, domain, probe } = source;
        let kind = kind.ok_or_else(|| {
            Error::new("busy.invalid", format!("busy source `{id}` has no kind: choose foreground (a user action waits on it) or background"))
        })?;
        let domain = domain.ok_or_else(|| {
            Error::new("busy.invalid", format!("busy source `{id}` has no clock domain: virtual (UI work) or wall (external work)"))
        })?;
        let mut v = self.lock();
        if v.iter().any(|e| e.id == id) {
            return Err(Error::new("busy.invalid", format!("busy source `{id}` is already registered")));
        }
        v.push(Entry { id, kind, domain, probe, first_busy: None });
        Ok(())
    }

    /// Removes the source `id`; `false` when there was none.
    pub fn unregister(&self, id: &str) -> bool {
        let mut v = self.lock();
        let n = v.len();
        v.retain(|e| e.id != id);
        v.len() != n
    }

    /// The registered sources: `(id, kind, domain)`.
    pub fn sources(&self) -> Vec<(String, BusyKind, ClockDomain)> {
        self.lock().iter().map(|e| (e.id.clone(), e.kind, e.domain)).collect()
    }

    /// Polls every source (probes run outside the registry's lock) and returns the busy ones,
    /// longest busy first. `virtual_ms` is the session's clock, for virtual-domain durations.
    pub fn poll(&self, virtual_ms: f64) -> Vec<BusyStatus> {
        let probes: Vec<(String, Probe)> = self.lock().iter().map(|e| (e.id.clone(), e.probe.clone())).collect();
        let reports: Vec<(String, BusyReport)> = probes.into_iter().map(|(id, p)| (id, p())).collect();
        let now = Instant::now();
        let mut out = Vec::new();
        let mut v = self.lock();
        for (id, r) in reports {
            let Some(e) = v.iter_mut().find(|e| e.id == id) else { continue };
            if !r.is_busy() {
                e.first_busy = None;
                continue;
            }
            let (t0, v0) = *e.first_busy.get_or_insert((r.since.unwrap_or(now), virtual_ms));
            let busy_for_ms = match e.domain {
                ClockDomain::Wall => now.duration_since(r.since.unwrap_or(t0)).as_secs_f64() * 1000.0,
                ClockDomain::Virtual => virtual_ms - v0,
            };
            out.push(BusyStatus { id, kind: e.kind, domain: e.domain, count: r.count, reason: r.reason, busy_for_ms });
        }
        out.sort_by(|a, b| b.busy_for_ms.total_cmp(&a.busy_for_ms));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn a_source_must_choose_a_kind_and_a_domain() {
        let r = BusyRegistry::new();
        let e = r.register(BusySource::new("jobs", BusyReport::idle).wall()).unwrap_err();
        assert_eq!(e.code, "busy.invalid");
        assert!(e.message.contains("no kind"), "{e}");
        let e = r.register(BusySource::new("jobs", BusyReport::idle).foreground()).unwrap_err();
        assert!(e.message.contains("no clock domain"), "{e}");
        r.register(BusySource::new("jobs", BusyReport::idle).foreground().wall()).unwrap();
        assert_eq!(r.register(BusySource::new("jobs", BusyReport::idle).background().wall()).unwrap_err().code, "busy.invalid");
        assert!(r.unregister("jobs") && !r.unregister("jobs"));
    }

    #[test]
    fn poll_reports_busy_sources_longest_first() {
        let r = BusyRegistry::new();
        let n = Arc::new(AtomicU32::new(0));
        let m = n.clone();
        r.register(BusySource::new("anim", move || BusyReport::busy(m.load(Ordering::SeqCst), "fade")).foreground().virtual_clock()).unwrap();
        r.register(BusySource::new("idle", BusyReport::idle).foreground().wall()).unwrap();
        assert!(r.poll(0.0).is_empty());
        n.store(1, Ordering::SeqCst);
        r.poll(100.0);
        r.register(BusySource::new("net", || BusyReport::busy(2, "GET route:parts")).background().wall()).unwrap();
        let busy = r.poll(400.0);
        assert_eq!(busy.iter().map(|b| b.id.as_str()).collect::<Vec<_>>(), ["anim", "net"]);
        assert_eq!(busy[0].busy_for_ms, 300.0);
        assert_eq!(busy[0].describe(), "anim ×1 (fade) for 300 ms virtual");
        // idle again resets the duration
        n.store(0, Ordering::SeqCst);
        r.poll(500.0);
        n.store(1, Ordering::SeqCst);
        assert_eq!(r.poll(600.0)[0].busy_for_ms, 0.0);
    }
}
