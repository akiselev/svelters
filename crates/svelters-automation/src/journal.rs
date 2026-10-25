//! AU7: the app's event journal (cadatomic WAITS §3.2): every occurrence the app cares about
//! (a command receipt, a job finishing, a network response, a worker message), appended with a
//! sequence number, so a wait can be "the first event matching P after the cursor taken before the
//! act".
//!
//! - **One journal per app run**, shared by every [`Session`](crate::Session) and every emitter:
//!   [`Journal`] is a cheap handle (`Clone`, `Send + Sync`); [`Journal::emit`] works from any thread.
//! - **Kinds are typed and registered** ([`EventKind`]: an id, a field schema, a clock domain and a
//!   display text key). Emitting or waiting on a kind nobody registered fails: not implemented means
//!   not registered.
//! - **A bounded ring.** When it is full the oldest events go, and a wait whose window lost events
//!   fails with `journal.overflow`; nothing is dropped silently.
//! - **Cursors** ([`Journal::cursor`]): the sequence number of the last event. Take one before the act,
//!   then wait for an event after it ([`Session::wait_event`](crate::Session::wait_event)): an event
//!   emitted inside the act is still seen, so the arm-before-act race cannot happen.
//!
//! ```ignore
//! let journal = session.journal().clone();
//! journal.register(EventKind::new("job.finished", ClockDomain::Wall).field("job", FieldType::String))?;
//! // on any thread, from the code that owns the work:
//! journal.emit("job.finished", "drc", json!({"job": "drc"}))?;
//! // in the test:
//! let at = session.cursor();
//! session.click("role=button[name=Run]", ActOptions::default())?;
//! session.wait_event(&Matcher::kind("job.finished").subject("drc"), at, 30_000.0)?;
//! ```

use crate::Error;
use serde_json::{Map, Value, json};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

/// Which clock an event kind or busy source counts in (WAITS §3.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ClockDomain {
    /// UI-internal work: animations, timers, debounces, settle. Runs on the session's virtual clock.
    Virtual,
    /// External work: jobs on the thread pool, workers, network, disk. Counts wall time, and
    /// holds the virtual clock while a foreground source of it is busy.
    Wall,
}

impl ClockDomain {
    pub fn as_str(self) -> &'static str {
        match self {
            ClockDomain::Virtual => "virtual",
            ClockDomain::Wall => "wall",
        }
    }

    pub fn parse(s: &str) -> Result<ClockDomain, Error> {
        match s {
            "virtual" => Ok(ClockDomain::Virtual),
            "wall" => Ok(ClockDomain::Wall),
            d => Err(Error::new("params.invalid", format!("unknown clock domain `{d}` (virtual | wall)"))),
        }
    }
}

/// The type of an event field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldType {
    String,
    Number,
    Bool,
    Object,
    Array,
    /// Any JSON value.
    Any,
}

impl FieldType {
    fn admits(self, v: &Value) -> bool {
        match self {
            FieldType::String => v.is_string(),
            FieldType::Number => v.is_number(),
            FieldType::Bool => v.is_boolean(),
            FieldType::Object => v.is_object(),
            FieldType::Array => v.is_array(),
            FieldType::Any => true,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            FieldType::String => "string",
            FieldType::Number => "number",
            FieldType::Bool => "bool",
            FieldType::Object => "object",
            FieldType::Array => "array",
            FieldType::Any => "any",
        }
    }
}

/// One field of an [`EventKind`]'s schema.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    pub name: String,
    pub ty: FieldType,
    pub required: bool,
}

/// A registered event kind: its id (`job.finished`), field schema, clock domain and the catalog key
/// of its display text.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventKind {
    pub id: String,
    pub fields: Vec<Field>,
    pub domain: ClockDomain,
    /// The display text's catalog key (default `event.<id>`).
    pub text_key: String,
}

impl EventKind {
    pub fn new(id: impl Into<String>, domain: ClockDomain) -> EventKind {
        let id = id.into();
        EventKind {
            text_key: format!("event.{id}"),
            id,
            fields: Vec::new(),
            domain,
        }
    }

    /// A required field.
    pub fn field(mut self, name: impl Into<String>, ty: FieldType) -> EventKind {
        self.fields.push(Field { name: name.into(), ty, required: true });
        self
    }

    /// An optional field.
    pub fn optional(mut self, name: impl Into<String>, ty: FieldType) -> EventKind {
        self.fields.push(Field { name: name.into(), ty, required: false });
        self
    }

    pub fn text_key(mut self, key: impl Into<String>) -> EventKind {
        self.text_key = key.into();
        self
    }

    /// Checks `fields` against the schema: required fields present, types right, no unknown field.
    pub fn validate(&self, fields: &Map<String, Value>) -> Result<(), String> {
        for f in &self.fields {
            match fields.get(&f.name) {
                None | Some(Value::Null) if f.required => return Err(format!("`{}` is missing", f.name)),
                None | Some(Value::Null) => {}
                Some(v) if !f.ty.admits(v) => {
                    return Err(format!("`{}` wants a {}, got {v}", f.name, f.ty.as_str()));
                }
                Some(_) => {}
            }
        }
        if let Some(k) = fields.keys().find(|k| !self.fields.iter().any(|f| &f.name == *k)) {
            return Err(format!("`{k}` is not a field of `{}`", self.id));
        }
        Ok(())
    }

    pub fn to_json(&self) -> Value {
        json!({
            "id": self.id, "domain": self.domain.as_str(), "text_key": self.text_key,
            "fields": self.fields.iter().map(|f| json!({"name": f.name, "type": f.ty.as_str(), "required": f.required})).collect::<Vec<_>>(),
        })
    }
}

/// A position in the journal: the sequence number of the last event before it. A wait since a
/// cursor sees the events with `seq > cursor`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Cursor(pub u64);

/// One journal entry.
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    /// From 1, in emit order.
    pub seq: u64,
    /// The journal's virtual clock when it was emitted (the session's clock).
    pub at_virtual_ms: f64,
    /// Wall ms since the journal was made.
    pub at_wall_ms: f64,
    pub kind: String,
    /// What it is about: a job id, a route id, a worker name (`""`: nothing in particular).
    pub subject: String,
    pub fields: Map<String, Value>,
}

impl Event {
    pub fn to_json(&self) -> Value {
        json!({
            "seq": self.seq, "at_virtual_ms": self.at_virtual_ms, "at_wall_ms": self.at_wall_ms,
            "kind": self.kind, "subject": self.subject, "fields": self.fields,
        })
    }

    /// One line for reports: `#12 job.finished drc {"status":"ok"} @1500ms`.
    pub fn describe(&self) -> String {
        let mut s = format!("#{} {}", self.seq, self.kind);
        if !self.subject.is_empty() {
            s.push(' ');
            s.push_str(&self.subject);
        }
        if !self.fields.is_empty() {
            s.push(' ');
            s.push_str(&Value::Object(self.fields.clone()).to_string());
        }
        s.push_str(&format!(" @{}ms", self.at_virtual_ms.round()));
        s
    }
}

type Pred = Arc<dyn Fn(&Event) -> bool + Send + Sync>;

/// What an event wait looks for: a kind (exact, or a prefix `job.*`), a subject, field values, and
/// optionally a Rust predicate. Every part given must match.
#[derive(Clone, Default)]
pub struct Matcher {
    pub kind: Option<String>,
    pub subject: Option<String>,
    /// Each field must equal the value given.
    pub fields: Map<String, Value>,
    pred: Option<(String, Pred)>,
}

impl std::fmt::Debug for Matcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.describe())
    }
}

impl Matcher {
    /// Events of `kind` (`"job.*"`: every kind under `job.`).
    pub fn kind(kind: impl Into<String>) -> Matcher {
        Matcher { kind: Some(kind.into()), ..Matcher::default() }
    }

    /// Any event.
    pub fn any() -> Matcher {
        Matcher::default()
    }

    pub fn subject(mut self, subject: impl Into<String>) -> Matcher {
        self.subject = Some(subject.into());
        self
    }

    pub fn field(mut self, name: impl Into<String>, value: impl Into<Value>) -> Matcher {
        self.fields.insert(name.into(), value.into());
        self
    }

    /// A Rust predicate on top (`what` names it in reports). Not carried by [`Matcher::to_json`].
    pub fn when(mut self, what: impl Into<String>, pred: impl Fn(&Event) -> bool + Send + Sync + 'static) -> Matcher {
        self.pred = Some((what.into(), Arc::new(pred)));
        self
    }

    fn kind_matches(&self, kind: &str) -> bool {
        match self.kind.as_deref() {
            None => true,
            Some(k) => match k.strip_suffix('*') {
                Some(prefix) => kind.starts_with(prefix),
                None => k == kind,
            },
        }
    }

    pub fn matches(&self, e: &Event) -> bool {
        self.kind_matches(&e.kind)
            && self.subject.as_ref().is_none_or(|s| s == &e.subject)
            && self.fields.iter().all(|(k, v)| e.fields.get(k) == Some(v))
            && self.pred.as_ref().is_none_or(|(_, p)| p(e))
    }

    /// `job.finished drc {"status":"ok"}`.
    pub fn describe(&self) -> String {
        let mut s = self.kind.clone().unwrap_or_else(|| "any event".into());
        if let Some(sub) = &self.subject {
            s.push(' ');
            s.push_str(sub);
        }
        if !self.fields.is_empty() {
            s.push(' ');
            s.push_str(&Value::Object(self.fields.clone()).to_string());
        }
        if let Some((what, _)) = &self.pred {
            s.push_str(&format!(" where {what}"));
        }
        s
    }

    /// `{kind?, subject?, fields?}`.
    pub fn from_json(v: &Value) -> Result<Matcher, Error> {
        let bad = |m: &str| Error::new("params.invalid", format!("matcher: {m}"));
        let o = v.as_object().ok_or_else(|| bad("wants an object {kind, subject, fields}"))?;
        let s = |k: &str| match o.get(k) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(s)) => Ok(Some(s.clone())),
            Some(_) => Err(bad(&format!("`{k}` wants a string"))),
        };
        let fields = match o.get("fields") {
            None | Some(Value::Null) => Map::new(),
            Some(Value::Object(m)) => m.clone(),
            Some(_) => return Err(bad("`fields` wants an object")),
        };
        Ok(Matcher { kind: s("kind")?, subject: s("subject")?, fields, pred: None })
    }

    pub fn to_json(&self) -> Value {
        json!({"kind": self.kind, "subject": self.subject, "fields": self.fields})
    }
}

/// The default ring size: a step's window must fit.
pub const DEFAULT_CAPACITY: usize = 4096;

struct Scheduled {
    due_ms: f64,
    kind: String,
    subject: String,
    fields: Map<String, Value>,
}

struct Inner {
    kinds: HashMap<String, EventKind>,
    ring: VecDeque<Event>,
    capacity: usize,
    next_seq: u64,
    /// The highest sequence number dropped from the ring (0: none).
    dropped_through: u64,
    virtual_ms: f64,
    /// [`Journal::emit_after`]'s events, in the order given.
    scheduled: Vec<Scheduled>,
}

/// The app-wide event journal (see the module docs). A handle: clones share the journal.
#[derive(Clone)]
pub struct Journal {
    inner: Arc<Mutex<Inner>>,
    wall0: Instant,
}

impl Default for Journal {
    fn default() -> Self {
        Journal::new(DEFAULT_CAPACITY)
    }
}

impl std::fmt::Debug for Journal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let i = self.lock();
        f.debug_struct("Journal").field("len", &i.ring.len()).field("capacity", &i.capacity).field("head", &(i.next_seq - 1)).finish()
    }
}

impl Journal {
    /// A journal keeping the last `capacity` events (at least 1).
    pub fn new(capacity: usize) -> Journal {
        Journal {
            inner: Arc::new(Mutex::new(Inner {
                kinds: HashMap::new(),
                ring: VecDeque::new(),
                capacity: capacity.max(1),
                next_seq: 1,
                dropped_through: 0,
                virtual_ms: 0.0,
                scheduled: Vec::new(),
            })),
            wall0: Instant::now(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        // an emitter that panicked mid-push leaves a consistent ring: keep going
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Whether `other` is a handle to the same journal.
    pub fn same(&self, other: &Journal) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }

    /// Registers `kind`. Registering the same definition again is a no-op; a different one under
    /// the same id fails with `event.kind_conflict`.
    pub fn register(&self, kind: EventKind) -> Result<(), Error> {
        let mut i = self.lock();
        match i.kinds.get(&kind.id) {
            Some(k) if *k == kind => Ok(()),
            Some(k) => Err(Error::new(
                "event.kind_conflict",
                format!("`{}` is registered with another definition: {}", kind.id, k.to_json()),
            )),
            None => {
                i.kinds.insert(kind.id.clone(), kind);
                Ok(())
            }
        }
    }

    pub fn kind(&self, id: &str) -> Option<EventKind> {
        self.lock().kinds.get(id).cloned()
    }

    /// Every registered kind, by id.
    pub fn kinds(&self) -> Vec<EventKind> {
        let mut v: Vec<EventKind> = self.lock().kinds.values().cloned().collect();
        v.sort_by(|a, b| a.id.cmp(&b.id));
        v
    }

    /// The clock domains of the kinds `m` can match (both when it names no kind).
    pub fn domains(&self, m: &Matcher) -> Result<Vec<ClockDomain>, Error> {
        let i = self.lock();
        let mut out = Vec::new();
        match m.kind.as_deref() {
            None => return Ok(vec![ClockDomain::Virtual, ClockDomain::Wall]),
            Some(k) if !k.ends_with('*') => match i.kinds.get(k) {
                Some(kind) => out.push(kind.domain),
                None => return Err(unregistered(k)),
            },
            Some(_) => {
                for k in i.kinds.values().filter(|k| m.kind_matches(&k.id)) {
                    if !out.contains(&k.domain) {
                        out.push(k.domain);
                    }
                }
            }
        }
        Ok(out)
    }

    fn check(i: &Inner, kind: &str, fields: &Value) -> Result<Map<String, Value>, Error> {
        let k = i.kinds.get(kind).ok_or_else(|| unregistered(kind))?;
        let fields = match fields {
            Value::Null => Map::new(),
            Value::Object(m) => m.clone(),
            v => return Err(Error::new("event.invalid", format!("`{kind}` fields want an object, got {v}"))),
        };
        k.validate(&fields).map_err(|m| Error::new("event.invalid", format!("`{kind}`: {m}")))?;
        Ok(fields)
    }

    fn push(&self, i: &mut Inner, kind: String, subject: String, fields: Map<String, Value>) -> u64 {
        let seq = i.next_seq;
        i.next_seq += 1;
        let e = Event {
            seq,
            at_virtual_ms: i.virtual_ms,
            at_wall_ms: self.wall0.elapsed().as_secs_f64() * 1000.0,
            kind,
            subject,
            fields,
        };
        i.ring.push_back(e);
        while i.ring.len() > i.capacity {
            let gone = i.ring.pop_front().expect("over capacity");
            i.dropped_through = gone.seq;
        }
        seq
    }

    /// Appends an event of a registered `kind` (fields: an object or `null`), from any thread.
    /// Returns its sequence number. Fails with `event.unregistered` or `event.invalid`.
    pub fn emit(&self, kind: &str, subject: impl Into<String>, fields: Value) -> Result<u64, Error> {
        let mut i = self.lock();
        let fields = Journal::check(&i, kind, &fields)?;
        Ok(self.push(&mut i, kind.to_string(), subject.into(), fields))
    }

    /// Checks the event now and appends it once the journal's virtual clock has moved `delay_ms`
    /// on (a replayed transcript's timing). The pending events count as busy for whoever
    /// registers [`Journal::scheduled`] as a source.
    pub fn emit_after(&self, delay_ms: f64, kind: &str, subject: impl Into<String>, fields: Value) -> Result<(), Error> {
        let mut i = self.lock();
        let fields = Journal::check(&i, kind, &fields)?;
        if delay_ms <= 0.0 {
            self.push(&mut i, kind.to_string(), subject.into(), fields);
            return Ok(());
        }
        let due_ms = i.virtual_ms + delay_ms;
        i.scheduled.push(Scheduled { due_ms, kind: kind.to_string(), subject: subject.into(), fields });
        Ok(())
    }

    /// Events waiting for [`Journal::emit_after`]'s delay.
    pub fn scheduled(&self) -> usize {
        self.lock().scheduled.len()
    }

    /// Moves the journal's virtual clock to `ms` (the session does this after each frame) and
    /// appends the scheduled events that are due, in the order they were scheduled.
    pub fn set_virtual_ms(&self, ms: f64) {
        let mut i = self.lock();
        i.virtual_ms = i.virtual_ms.max(ms);
        if i.scheduled.is_empty() {
            return;
        }
        let now = i.virtual_ms;
        let (due, keep): (Vec<_>, Vec<_>) = std::mem::take(&mut i.scheduled).into_iter().partition(|s| s.due_ms <= now);
        i.scheduled = keep;
        for s in due {
            self.push(&mut i, s.kind, s.subject, s.fields);
        }
    }

    pub fn virtual_ms(&self) -> f64 {
        self.lock().virtual_ms
    }

    /// The position after the last event.
    pub fn cursor(&self) -> Cursor {
        Cursor(self.lock().next_seq - 1)
    }

    pub fn capacity(&self) -> usize {
        self.lock().capacity
    }

    pub fn len(&self) -> usize {
        self.lock().ring.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lock().ring.is_empty()
    }

    /// Every event after `since`. Fails with `journal.overflow` when the ring dropped some of them.
    pub fn since(&self, since: Cursor) -> Result<Vec<Event>, Error> {
        let i = self.lock();
        overflow(&i, since)?;
        Ok(i.ring.iter().filter(|e| e.seq > since.0).cloned().collect())
    }

    /// The last `n` events after `since` (reports; no overflow error).
    pub fn tail(&self, since: Cursor, n: usize) -> Vec<Event> {
        let i = self.lock();
        let after: Vec<&Event> = i.ring.iter().filter(|e| e.seq > since.0).collect();
        after[after.len().saturating_sub(n)..].iter().map(|e| (*e).clone()).collect()
    }

    /// The first event after `since` for which `f` holds (`journal.overflow` as [`Journal::since`]).
    pub fn find(&self, since: Cursor, mut f: impl FnMut(&Event) -> bool) -> Result<Option<Event>, Error> {
        let i = self.lock();
        overflow(&i, since)?;
        Ok(i.ring.iter().filter(|e| e.seq > since.0).find(|e| f(e)).cloned())
    }
}

fn overflow(i: &Inner, since: Cursor) -> Result<(), Error> {
    if i.dropped_through > since.0 {
        return Err(Error::new(
            "journal.overflow",
            format!(
                "the journal dropped events {}..={} after cursor {} (capacity {}): make the journal bigger or take the cursor later",
                since.0 + 1,
                i.dropped_through,
                since.0,
                i.capacity
            ),
        ));
    }
    Ok(())
}

fn unregistered(kind: &str) -> Error {
    Error::new("event.unregistered", format!("no event kind `{kind}` is registered (Journal::register)"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn journal(cap: usize) -> Journal {
        let j = Journal::new(cap);
        j.register(EventKind::new("job.finished", ClockDomain::Wall).field("status", FieldType::String)).unwrap();
        j.register(EventKind::new("job.started", ClockDomain::Wall)).unwrap();
        j
    }

    #[test]
    fn kinds_are_registered_and_checked() {
        let j = journal(8);
        assert_eq!(j.emit("job.nope", "", Value::Null).unwrap_err().code, "event.unregistered");
        assert_eq!(j.emit("job.finished", "drc", Value::Null).unwrap_err().code, "event.invalid");
        assert_eq!(j.emit("job.finished", "drc", json!({"status": 1})).unwrap_err().code, "event.invalid");
        assert_eq!(j.emit("job.finished", "drc", json!({"status": "ok", "x": 1})).unwrap_err().code, "event.invalid");
        assert_eq!(j.emit("job.finished", "drc", json!({"status": "ok"})).unwrap(), 1);
        // the same definition again is fine, another one is not
        j.register(EventKind::new("job.started", ClockDomain::Wall)).unwrap();
        assert_eq!(j.register(EventKind::new("job.started", ClockDomain::Virtual)).unwrap_err().code, "event.kind_conflict");
    }

    #[test]
    fn matchers_and_domains() {
        let j = journal(8);
        j.emit("job.started", "drc", Value::Null).unwrap();
        j.emit("job.finished", "drc", json!({"status": "ok"})).unwrap();
        let all = j.since(Cursor(0)).unwrap();
        assert!(Matcher::kind("job.*").matches(&all[0]));
        assert!(Matcher::kind("job.finished").field("status", "ok").matches(&all[1]));
        assert!(!Matcher::kind("job.finished").subject("route").matches(&all[1]));
        assert!(Matcher::any().when("seq 2", |e| e.seq == 2).matches(&all[1]));
        assert_eq!(j.domains(&Matcher::kind("job.*")).unwrap(), vec![ClockDomain::Wall]);
        assert_eq!(j.domains(&Matcher::kind("net.response")).unwrap_err().code, "event.unregistered");
        let m = Matcher::from_json(&json!({"kind": "job.finished", "fields": {"status": "ok"}})).unwrap();
        assert!(m.matches(&all[1]) && !m.matches(&all[0]));
    }

    #[test]
    fn overflow_is_reported_not_silent() {
        let j = journal(2);
        let at = j.cursor();
        for _ in 0..3 {
            j.emit("job.started", "x", Value::Null).unwrap();
        }
        assert_eq!(j.since(at).unwrap_err().code, "journal.overflow");
        // a cursor taken after the drop is fine
        assert_eq!(j.since(Cursor(1)).unwrap().len(), 2);
    }

    #[test]
    fn scheduled_events_wait_for_the_virtual_clock() {
        let j = journal(8);
        j.emit_after(100.0, "job.started", "a", Value::Null).unwrap();
        j.emit_after(50.0, "job.started", "b", Value::Null).unwrap();
        j.set_virtual_ms(60.0);
        assert_eq!(j.since(Cursor(0)).unwrap().iter().map(|e| e.subject.as_str()).collect::<Vec<_>>(), ["b"]);
        j.set_virtual_ms(100.0);
        assert_eq!(j.scheduled(), 0);
        assert_eq!(j.since(Cursor(1)).unwrap()[0].at_virtual_ms, 100.0);
    }

    #[test]
    fn emit_works_from_any_thread() {
        let j = journal(64);
        let hs: Vec<_> = (0..4)
            .map(|t| {
                let j = j.clone();
                std::thread::spawn(move || {
                    for _ in 0..8 {
                        j.emit("job.started", format!("t{t}"), Value::Null).unwrap();
                    }
                })
            })
            .collect();
        hs.into_iter().for_each(|h| h.join().unwrap());
        let seqs: Vec<u64> = j.since(Cursor(0)).unwrap().iter().map(|e| e.seq).collect();
        assert_eq!(seqs, (1..=32).collect::<Vec<_>>());
    }
}
