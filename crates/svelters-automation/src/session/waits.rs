//! AU7: one wait engine for tests, tours and agents (cadatomic WAITS §3) on the [`Session`]:
//!
//! - **Events** ([`Session::cursor`], [`Session::wait_event`]): the first unconsumed event matching
//!   after a cursor taken before the act; two identical waits consume two events.
//! - **Never** ([`Session::never`]): a matching event after its cursor fails every wait until
//!   [`Session::end_step`].
//! - **Seen** ([`Session::arm_seen`], [`Session::wait_seen`]): a condition latched on any settled
//!   frame since it was armed (a toast that came and went).
//! - **Busy sources** ([`Session::register_busy_source`]): foreground ones hold settle; a busy
//!   foreground wall-clock source holds the virtual clock.
//! - **Budgets** ([`Budget`]): virtual ms count frames run on the virtual clock; wall ms count the
//!   frames that held it, and all wall time for a wait on a wall-clock event kind.
//! - **Reports** ([`WaitReport`]): every wait timeout carries the unmet conditions with their last
//!   values, the events since the wait's cursor, the busy foreground sources, the embed's busy
//!   bits and the failed actionability check, as `Error::data` and in the message.

use super::*;
use crate::busy::{BusyKind, BusySource, BusyStatus};
use crate::journal::{ClockDomain, Event, EventKind, FieldType};

/// How long a wait may take, per clock domain (WAITS §3.5). `From<f64>`: the same number for both.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Budget {
    /// Virtual ms: frames run while the virtual clock moves.
    pub virtual_ms: f64,
    /// Wall ms: frames run while a foreground wall-clock source held the virtual clock, and, for
    /// a wait on a wall-clock event kind, all wall time.
    pub wall_ms: f64,
}

impl Budget {
    pub fn new(virtual_ms: f64, wall_ms: f64) -> Budget {
        Budget { virtual_ms, wall_ms }
    }
}

impl From<f64> for Budget {
    fn from(ms: f64) -> Budget {
        Budget { virtual_ms: ms, wall_ms: ms }
    }
}

/// Where a wait started and what it may spend.
pub(super) struct Deadline {
    v0: f64,
    held0: Duration,
    t0: Instant,
    budget: Budget,
    /// Count all wall time (a wait on a wall-clock event kind), not only the held frames'.
    wall_all: bool,
    since: Cursor,
}

/// A state condition (`wait.for`'s selector, state and expectation).
#[derive(Clone, Debug, PartialEq)]
pub struct Condition {
    pub selector: String,
    pub state: WaitState,
    pub expect: Option<Expect>,
}

impl Condition {
    pub fn new(selector: impl Into<String>, state: WaitState, expect: Option<Expect>) -> Condition {
        Condition { selector: selector.into(), state, expect }
    }

    /// `` `text="Saved"` Visible, text Exact("Saved") ``.
    pub fn describe(&self) -> String {
        let mut s = format!("`{}` {:?}", self.selector, self.state);
        match &self.expect {
            None => {}
            Some(Expect::Count(c)) => s.push_str(&format!(", count {c}")),
            Some(Expect::Text(t)) => s.push_str(&format!(", text {t:?}")),
            Some(Expect::Value(t)) => s.push_str(&format!(", value {t:?}")),
        }
        s
    }

    /// `{selector, state?, expect?}` (`wait.for`'s params).
    pub fn from_json(p: &Value) -> Result<Condition, Error> {
        let state = match p.get("state").and_then(Value::as_str).unwrap_or("visible") {
            "attached" => WaitState::Attached,
            "detached" => WaitState::Detached,
            "visible" => WaitState::Visible,
            "hidden" => WaitState::Hidden,
            "enabled" => WaitState::Enabled,
            s => return Err(Error::new("params.invalid", format!("unknown state `{s}`"))),
        };
        let expect = match p.get("expect") {
            None | Some(Value::Null) => None,
            Some(e) => Some(expect_param(e)?),
        };
        Ok(Condition::new(str_param(p, "selector")?, state, expect))
    }
}

/// An unmet condition of a timed-out wait, with its last value.
#[derive(Clone, Debug, PartialEq)]
pub struct Unmet {
    pub condition: String,
    pub last: String,
}

/// The actionability check that failed (WAITS §3.4): `attached`, `visible`, `stable`, `hit`,
/// `enabled` or `editable`, with what was seen ("still moving: …", "covered by …").
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub struct Actionability<N = DefaultNode> {
    pub check: &'static str,
    pub detail: String,
    /// `hit`: the node that receives the events instead.
    pub cover: Option<N>,
}

impl<N> Actionability<N> {
    pub(super) fn new(check: &'static str, detail: impl Into<String>) -> Actionability<N> {
        Actionability { check, detail: detail.into(), cover: None }
    }
}

/// What a timed-out wait saw (WAITS §4's failure message), as one value: [`WaitReport::to_json`]
/// is the error's `data`, [`WaitReport::text`] its message after the first line.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub struct WaitReport<N = DefaultNode> {
    /// `wait.event job.finished drc`, `wait.for …`, `wait.settled`, `actionable …`.
    pub wait: String,
    /// The budget that ran out.
    pub exhausted: ClockDomain,
    pub budget: Budget,
    pub virtual_ms: f64,
    pub wall_ms: f64,
    pub unmet: Vec<Unmet>,
    /// The cursor the wait counted events from.
    pub since: Cursor,
    /// The last events after `since` (at most 20).
    pub events: Vec<Event>,
    /// Busy foreground sources, longest busy first.
    pub busy: Vec<BusyStatus>,
    /// The embed's busy bits (`Driver::busy`).
    pub embed: Option<String>,
    pub pending_input: bool,
    /// The deprecated host predicate ([`Session::set_idle`]) said busy.
    pub host_busy: bool,
    pub actionability: Option<Actionability<N>>,
}

#[cfg(feature = "egui")]
impl WaitReport {
    pub fn to_json(&self) -> Value {
        self.to_json_with(&|n: NodeId| n.to_raw())
    }
}

impl<N: Copy> WaitReport<N> {
    /// [`WaitReport::to_json`] with `raw` giving a node's JSON id.
    pub fn to_json_with(&self, raw: &dyn Fn(N) -> u64) -> Value {
        json!({
            "wait": self.wait, "exhausted": self.exhausted.as_str(),
            "budget": {"virtual_ms": self.budget.virtual_ms, "wall_ms": self.budget.wall_ms},
            "spent": {"virtual_ms": self.virtual_ms.round(), "wall_ms": self.wall_ms.round()},
            "unmet": self.unmet.iter().map(|u| json!({"condition": u.condition, "last": u.last})).collect::<Vec<_>>(),
            "since": self.since.0,
            "events": self.events.iter().map(Event::to_json).collect::<Vec<_>>(),
            "busy": self.busy.iter().map(BusyStatus::to_json).collect::<Vec<_>>(),
            "embed": self.embed, "pending_input": self.pending_input, "host_busy": self.host_busy,
            "actionability": self.actionability.as_ref().map(|a| json!({"check": a.check, "detail": a.detail, "cover": a.cover.map(raw)})),
        })
    }

    /// The report for people, one item per line.
    pub fn text(&self) -> String {
        let mut out = vec![format!(
            "{} ran out of {} time ({} ms virtual, {} ms wall spent)",
            self.wait,
            self.exhausted.as_str(),
            self.virtual_ms.round(),
            self.wall_ms.round()
        )];
        for u in &self.unmet {
            out.push(format!("  unmet: {} — last: {}", u.condition, u.last));
        }
        if let Some(a) = &self.actionability {
            out.push(format!("  not actionable ({}): {}", a.check, a.detail));
        }
        for b in &self.busy {
            out.push(format!("  busy: {}", b.describe()));
        }
        if let Some(e) = &self.embed {
            out.push(format!("  embed busy: {e}"));
        }
        if self.pending_input {
            out.push("  input pending".into());
        }
        if self.host_busy {
            out.push("  host busy (set_idle)".into());
        }
        if self.events.is_empty() {
            out.push(format!("  no events since cursor {}", self.since.0));
        } else {
            out.push(format!("  events since cursor {}:", self.since.0));
            out.extend(self.events.iter().map(|e| format!("    {}", e.describe())));
        }
        out.join("\n")
    }
}

/// A [`Session::never`] guard.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NeverId(pub u64);

/// A [`Session::arm_seen`] latch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SeenId(pub u64);

pub(super) struct SeenLatch {
    id: SeenId,
    cond: Condition,
    sel: Selector,
    /// `(frame, session ms)` of the first settled frame it held on.
    latched: Option<(u64, f64)>,
    last: String,
}

/// The JSON methods [`Session::call`] hands to [`Session::call_waits`].
pub(super) const METHODS: &[&str] = &[
    "journal.cursor",
    "journal.events",
    "journal.kinds",
    "wait.event",
    "wait.never",
    "wait.seen",
    "wait.step_end",
    "busy.sources",
];

/// Scene changes as journal events ([`Session::bridge_scene_changes`]).
pub const SCENE_CHANGED: &str = "scene.changed";

impl<D: SourceDriver> Session<D> {
    // ---- journal and busy sources ----

    /// The app's event journal: register kinds and emit on it from any thread (a clone).
    pub fn journal(&self) -> &Journal {
        &self.events
    }

    /// Uses the app's journal (one per app run, shared by every session) instead of this
    /// session's own. Consumed events and guards are per session and start over.
    pub fn set_journal(&mut self, journal: Journal) {
        self.events = journal;
        self.consumed.clear();
        self.nevers.clear();
    }

    /// The busy-source registry (a handle: the app may keep a clone and register from anywhere).
    pub fn busy_registry(&self) -> &BusyRegistry {
        &self.busy
    }

    /// Uses the app's registry (shared by every session) instead of this session's own.
    pub fn set_busy_registry(&mut self, registry: BusyRegistry) {
        self.busy = registry;
    }

    /// Registers a named busy source (WAITS §3.3; fails with `busy.invalid` without a kind or a
    /// clock domain). Replaces [`Session::set_idle`].
    pub fn register_busy_source(&self, source: BusySource) -> Result<(), Error> {
        self.busy.register(source)
    }

    /// Every busy source now (foreground and background), longest busy first.
    pub fn busy_now(&self) -> Vec<BusyStatus> {
        self.busy.poll(self.now_ms)
    }

    pub(super) fn foreground_busy(&self) -> Vec<BusyStatus> {
        self.busy_now().into_iter().filter(|b| b.kind == BusyKind::Foreground).collect()
    }

    /// A foreground wall-clock source is busy: the virtual clock holds.
    pub(super) fn wall_work_busy(&self) -> bool {
        self.foreground_busy().iter().any(|b| b.domain == ClockDomain::Wall)
    }

    /// For hosts that run frames themselves (a windowed app mounting `Server::pump`): call after
    /// each frame so the journal's clock moves and [`Session::arm_seen`] latches see the frame.
    pub fn observe_frame(&mut self) {
        self.after_frame();
    }

    pub(super) fn after_frame(&mut self) {
        self.events.set_virtual_ms(self.driver.clock_ms().unwrap_or(self.now_ms));
        if let Some(obs) = &self.scene_bridge {
            let set = self.driver.take_changes(obs);
            for (n, m) in set.by_node {
                let _ = self.events.emit(SCENE_CHANGED, self.driver.node_raw(n).to_string(), json!({"what": m}));
            }
            if set.overflowed {
                let _ = self.events.emit(SCENE_CHANGED, "", json!({"what": "overflowed"}));
            }
        }
        if !self.seen.is_empty() && self.not_settled().is_none() {
            let mut seen = std::mem::take(&mut self.seen);
            let (frame, ms) = (self.frame, self.now_ms);
            for l in seen.iter_mut().filter(|l| l.latched.is_none()) {
                match self.evaluate(&l.sel, &l.cond) {
                    Ok((true, _, _)) => l.latched = Some((frame, ms)),
                    Ok((false, why, _)) => l.last = why,
                    Err(e) => l.last = e.to_string(),
                }
            }
            seen.append(&mut self.seen);
            self.seen = seen;
        }
    }

    /// Bridges SG2's scene journal into the event journal: from now on, each frame's changes are
    /// `scene.changed` events (subject: the node id, `what`: the change mask), virtual clock. The
    /// `journal.subscribe`/`journal.take` pair is unchanged.
    pub fn bridge_scene_changes(&mut self, filter: D::Filter) -> Result<(), Error> {
        self.check(false)?;
        self.events.register(EventKind::new(SCENE_CHANGED, ClockDomain::Virtual).field("what", FieldType::String))?;
        self.scene_bridge = Some(self.driver.observe(filter));
        Ok(())
    }

    // ---- budgets and reports ----

    pub(super) fn deadline(&self, budget: Budget, wall_all: bool) -> Deadline {
        self.deadline_since(budget, wall_all, self.events.cursor())
    }

    fn deadline_since(&self, budget: Budget, wall_all: bool, since: Cursor) -> Deadline {
        Deadline { v0: self.now_ms, held0: self.held_wall, t0: Instant::now(), budget, wall_all, since }
    }

    fn spent(&self, d: &Deadline) -> (f64, f64) {
        let wall = if d.wall_all { d.t0.elapsed() } else { self.held_wall.saturating_sub(d.held0) };
        (self.now_ms - d.v0, wall.as_secs_f64() * 1000.0)
    }

    /// The domain whose budget `d` has spent (`None`: time left).
    pub(super) fn expired(&self, d: &Deadline) -> Option<ClockDomain> {
        let (v, w) = self.spent(d);
        if v >= d.budget.virtual_ms {
            Some(ClockDomain::Virtual)
        } else if w >= d.budget.wall_ms {
            Some(ClockDomain::Wall)
        } else {
            None
        }
    }

    /// The report of a wait that ran out.
    fn report(&self, wait: String, d: &Deadline, unmet: Vec<Unmet>, actionability: Option<Actionability<N<D>>>) -> WaitReport<N<D>> {
        let (virtual_ms, wall_ms) = self.spent(d);
        WaitReport {
            wait,
            exhausted: self.expired(d).unwrap_or(ClockDomain::Virtual),
            budget: d.budget,
            virtual_ms,
            wall_ms,
            unmet,
            since: d.since,
            events: self.events.tail(d.since, 20),
            busy: self.foreground_busy(),
            embed: self.driver.busy(),
            pending_input: self.pending_input,
            host_busy: self.idle.as_ref().is_some_and(|idle| !idle()),
            actionability,
        }
    }

    /// A timeout error: `head` on the first line, the report after it and as `data`.
    pub(super) fn timeout(
        &self,
        code: &'static str,
        head: String,
        wait: String,
        d: &Deadline,
        unmet: Vec<Unmet>,
        actionability: Option<Actionability<N<D>>>,
    ) -> Error {
        let r = self.report(wait, d, unmet, actionability);
        Error::new(code, format!("{head}\n{}", r.text())).with_data(r.to_json_with(&self.raw()))
    }

    // ---- events ----

    /// `journal.cursor`: the journal's position now. Take it before the act.
    pub fn cursor(&self) -> Cursor {
        self.events.cursor()
    }

    /// `wait.event`: steps frames until an event after `since` matches `m` and was not consumed by
    /// an earlier wait of this session; consumes and returns it. An event emitted inside the act
    /// is seen (the cursor was taken before it). Fails with `event.unregistered` for a kind
    /// nobody registered, `journal.overflow` when the window lost events, `wait.never` when a
    /// guard tripped, and `timeout` with a [`WaitReport`].
    pub fn wait_event(&mut self, m: &Matcher, since: Cursor, timeout_ms: f64) -> Result<Event, Error> {
        self.wait_event_within(m, since, Budget::from(timeout_ms))
    }

    /// [`Session::wait_event`] with a budget per clock domain. A wait on a wall-clock kind counts
    /// all wall time against `budget.wall_ms`.
    pub fn wait_event_within(&mut self, m: &Matcher, since: Cursor, budget: Budget) -> Result<Event, Error> {
        self.check(false)?;
        let domains = self.events.domains(m)?;
        let d = self.deadline_since(budget, domains.contains(&ClockDomain::Wall), since);
        loop {
            self.check_nevers()?;
            let consumed = &self.consumed;
            if let Some(e) = self.events.find(since, |e| !consumed.contains(&e.seq) && m.matches(e))? {
                self.consume(e.seq);
                return Ok(e);
            }
            if self.expired(&d).is_some() {
                let n = self.events.tail(since, usize::MAX).len();
                let unmet = vec![Unmet {
                    condition: format!("event {}", m.describe()),
                    last: format!("no unconsumed match among {n} events since cursor {}", since.0),
                }];
                let head = format!("no `{}` event after cursor {}", m.describe(), since.0);
                return Err(self.timeout("timeout", head, format!("wait.event {}", m.describe()), &d, unmet, None));
            }
            self.step();
        }
    }

    fn consume(&mut self, seq: u64) {
        self.consumed.insert(seq);
        // forget what the ring no longer holds
        let cap = self.events.capacity() as u64;
        if self.consumed.len() as u64 > cap {
            let floor = self.events.cursor().0.saturating_sub(cap);
            self.consumed = self.consumed.split_off(&floor);
        }
    }

    /// `wait.never`: from `since` until [`Session::end_step`], a matching event fails every wait
    /// (and `end_step`) with `wait.never`.
    pub fn never(&mut self, m: Matcher, since: Cursor) -> Result<NeverId, Error> {
        self.check(false)?;
        self.events.domains(&m)?;
        let id = NeverId(self.next_id);
        self.next_id += 1;
        self.nevers.push((id, m, since));
        Ok(id)
    }

    /// Drops one guard; `false` when there was none.
    pub fn release_never(&mut self, id: NeverId) -> bool {
        let n = self.nevers.len();
        self.nevers.retain(|(i, _, _)| *i != id);
        n != self.nevers.len()
    }

    /// `Err(wait.never)` when a guard's event happened.
    pub fn check_nevers(&self) -> Result<(), Error> {
        for (_, m, since) in &self.nevers {
            if let Some(e) = self.events.find(*since, |e| m.matches(e))? {
                return Err(Error::new("wait.never", format!("`{}` happened: {}", m.describe(), e.describe()))
                    .with_data(json!({"never": m.to_json(), "event": e.to_json()})));
            }
        }
        Ok(())
    }

    /// `wait.step_end`: the step is complete. Checks the guards one last time, then drops every
    /// guard and latch.
    pub fn end_step(&mut self) -> Result<(), Error> {
        self.check(false)?;
        let r = self.check_nevers();
        self.nevers.clear();
        self.seen.clear();
        r
    }

    // ---- latched conditions ----

    /// `wait.seen` (arm): latches `cond` on the first settled frame it holds on, from now on
    /// (the current frame counts when the app is settled). Transient state (a toast) is caught
    /// without the author knowing it is transient.
    pub fn arm_seen(&mut self, cond: Condition) -> Result<SeenId, Error> {
        self.check(false)?;
        let sel = parse(&cond.selector)?;
        let id = SeenId(self.next_id);
        self.next_id += 1;
        let mut l = SeenLatch { id, cond, sel, latched: None, last: "not evaluated yet".into() };
        if self.not_settled().is_none() {
            match self.evaluate(&l.sel, &l.cond)? {
                (true, _, _) => l.latched = Some((self.frame, self.now_ms)),
                (false, why, _) => l.last = why,
            }
        }
        self.seen.push(l);
        Ok(id)
    }

    fn latch(&self, id: SeenId) -> Result<&SeenLatch, Error> {
        self.seen.iter().find(|l| l.id == id).ok_or_else(|| Error::new("params.invalid", format!("no seen latch {}", id.0)))
    }

    /// Whether the latch held on some settled frame since it was armed.
    pub fn seen(&self, id: SeenId) -> Result<bool, Error> {
        Ok(self.latch(id)?.latched.is_some())
    }

    /// `wait.seen` (wait): steps frames until the latch holds.
    pub fn wait_seen(&mut self, id: SeenId, budget: impl Into<Budget>) -> Result<(), Error> {
        self.check(false)?;
        let d = self.deadline(budget.into(), false);
        loop {
            self.check_nevers()?;
            let l = self.latch(id)?;
            if l.latched.is_some() {
                return Ok(());
            }
            if self.expired(&d).is_some() {
                let unmet = vec![Unmet { condition: format!("seen {}", l.cond.describe()), last: l.last.clone() }];
                let head = format!("{} not seen on any settled frame since it was armed", l.cond.describe());
                let wait = format!("wait.seen {}", l.cond.describe());
                return Err(self.timeout("timeout", head, wait, &d, unmet, None));
            }
            self.step();
        }
    }

    /// Drops a latch; `false` when there was none.
    pub fn release_seen(&mut self, id: SeenId) -> bool {
        let n = self.seen.len();
        self.seen.retain(|l| l.id != id);
        n != self.seen.len()
    }

    // ---- JSON ----

    pub(super) fn call_waits(&mut self, method: &str, p: &Value) -> Result<Value, Error> {
        let since = |p: &Value| p.get("since").and_then(Value::as_u64).map(Cursor);
        let matcher = |p: &Value| match p.get("matcher") {
            Some(m) => Matcher::from_json(m),
            None => Matcher::from_json(p),
        };
        Ok(match method {
            "journal.cursor" => {
                self.check(false)?;
                json!({"cursor": self.cursor().0})
            }
            "journal.events" => {
                self.check(false)?;
                let mut ev = self.events.since(since(p).unwrap_or_default())?;
                if let Some(l) = p.get("limit").and_then(Value::as_u64) {
                    ev.truncate(l as usize);
                }
                json!({"events": ev.iter().map(Event::to_json).collect::<Vec<_>>(), "cursor": self.cursor().0})
            }
            "journal.kinds" => {
                self.check(false)?;
                json!(self.events.kinds().iter().map(EventKind::to_json).collect::<Vec<_>>())
            }
            "wait.event" => {
                let since = since(p).ok_or_else(|| {
                    Error::new("params.invalid", "wait.event wants `since`: take `journal.cursor` before the act and pass it")
                })?;
                self.wait_event_within(&matcher(p)?, since, budget(p))?.to_json()
            }
            "wait.never" => {
                let since = since(p).unwrap_or_else(|| self.cursor());
                json!({"id": self.never(matcher(p)?, since)?.0})
            }
            "wait.step_end" => {
                self.end_step()?;
                json!({})
            }
            "wait.seen" => match p.get("id").and_then(Value::as_u64) {
                Some(id) => {
                    self.wait_seen(SeenId(id), budget(p))?;
                    json!({"id": id, "seen": true})
                }
                None => {
                    let id = self.arm_seen(Condition::from_json(p)?)?;
                    json!({"id": id.0, "seen": self.seen(id)?})
                }
            },
            "busy.sources" => {
                self.check(false)?;
                json!({
                    "sources": self.busy.sources().iter().map(|(id, k, d)| json!({"id": id, "kind": k.as_str(), "domain": d.as_str()})).collect::<Vec<_>>(),
                    "busy": self.busy_now().iter().map(BusyStatus::to_json).collect::<Vec<_>>(),
                })
            }
            m => return Err(Error::new("method.unknown", format!("`{m}`"))),
        })
    }
}

/// `{timeout, wall_timeout?}`: virtual ms, and wall ms (default: the same).
pub(super) fn budget(p: &Value) -> Budget {
    let v = timeout(p);
    Budget { virtual_ms: v, wall_ms: p.get("wall_timeout").and_then(Value::as_f64).unwrap_or(v) }
}
