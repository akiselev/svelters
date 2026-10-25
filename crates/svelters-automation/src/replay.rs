//! AU7: record and replay of external sources (cadatomic WAITS §3.6), generic only: the seam a host
//! source (an HTTP client, a worker pool) goes through, so a test can replay what a live run
//! recorded and the journal sees the same events either way. cadatomic supplies the sources.
//!
//! - A source owns a [`SourceSeam`] in one of three modes: **live** (does the work, emits its
//!   events), **record** (live, and every event and result goes into a [`Recorder`]'s
//!   [`Transcript`]), **replay** (does no work: [`Exchange::replay`] emits the recorded events, with
//!   their recorded delays on the virtual clock, and hands back the recorded result).
//! - Each unit of work is an [`Exchange`] keyed by the source (`GET route:parts`); the n-th exchange
//!   with a key replays the n-th recorded one.
//! - **Redaction** ([`Redactor`]): URLs become route ids (`route:/api/parts/:id`, no host, query or
//!   fragment) and payload fields become stable ids (`body#1`), in live, record and replay alike, so
//!   the journal of a live run and of its replay are the same and the transcript holds no secrets.
//!   Results are stored as the source gives them: put in a result only what replay needs.
//! - [`Transcript`]: NDJSON, one header line, then one line per event or result.
//!
//! ```ignore
//! let net = SourceSeam::live("net", journal.clone()).redactor(Redactor::new().url("url").intern("body"));
//! // in the HTTP client:
//! let x = net.exchange(format!("GET {}", route_of(&url)));
//! if net.is_replaying() { return decode(x.replay()?); }
//! x.emit("net.request", route, json!({"url": url}))?;
//! let resp = do_request(&url);
//! x.emit("net.response", route, json!({"status": resp.status}))?;
//! x.finish(json!({"status": resp.status, "body": resp.text}));
//! ```

use crate::Error;
use crate::busy::{BusyKind, BusyReport, BusySource};
use crate::journal::{ClockDomain, Journal};
use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

/// The transcript format's version (the header line).
pub const TRANSCRIPT_VERSION: u32 = 1;

/// One line of a transcript (after the header).
#[derive(Clone, Debug, PartialEq)]
pub enum TranscriptLine {
    /// An event a source emitted during an exchange, `after_ms` wall ms after it started.
    Event {
        source: String,
        key: String,
        /// The exchange's occurrence of `key` (0 for the first).
        n: u32,
        after_ms: f64,
        kind: String,
        subject: String,
        fields: Map<String, Value>,
    },
    /// An exchange's result.
    Result { source: String, key: String, n: u32, value: Value },
}

impl TranscriptLine {
    pub fn to_json(&self) -> Value {
        match self {
            TranscriptLine::Event { source, key, n, after_ms, kind, subject, fields } => json!({
                "type": "event", "source": source, "key": key, "n": n, "after_ms": after_ms.round(),
                "kind": kind, "subject": subject, "fields": fields,
            }),
            TranscriptLine::Result { source, key, n, value } => {
                json!({"type": "result", "source": source, "key": key, "n": n, "value": value})
            }
        }
    }

    fn from_json(v: &Value) -> Result<TranscriptLine, String> {
        let s = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string).ok_or_else(|| format!("`{k}` wants a string"));
        let n = v.get("n").and_then(Value::as_u64).ok_or("`n` wants a number")? as u32;
        match v.get("type").and_then(Value::as_str) {
            Some("event") => Ok(TranscriptLine::Event {
                source: s("source")?,
                key: s("key")?,
                n,
                after_ms: v.get("after_ms").and_then(Value::as_f64).unwrap_or(0.0),
                kind: s("kind")?,
                subject: s("subject")?,
                fields: v.get("fields").and_then(Value::as_object).cloned().unwrap_or_default(),
            }),
            Some("result") => Ok(TranscriptLine::Result {
                source: s("source")?,
                key: s("key")?,
                n,
                value: v.get("value").cloned().unwrap_or(Value::Null),
            }),
            t => Err(format!("unknown line type {t:?}")),
        }
    }

    fn at(&self) -> (&str, &str, u32) {
        match self {
            TranscriptLine::Event { source, key, n, .. } | TranscriptLine::Result { source, key, n, .. } => (source, key, *n),
        }
    }
}

/// A recorded run of external sources (see the module docs).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Transcript {
    pub lines: Vec<TranscriptLine>,
}

impl Transcript {
    /// NDJSON: a header `{"type":"transcript","version":1}`, then one line per entry.
    pub fn to_ndjson(&self) -> String {
        let mut out = json!({"type": "transcript", "version": TRANSCRIPT_VERSION}).to_string();
        out.push('\n');
        for l in &self.lines {
            out.push_str(&l.to_json().to_string());
            out.push('\n');
        }
        out
    }

    /// Parses [`Transcript::to_ndjson`]'s output (`replay.invalid` on a bad line or version).
    pub fn from_ndjson(text: &str) -> Result<Transcript, Error> {
        let bad = |i: usize, m: String| Error::new("replay.invalid", format!("transcript line {}: {m}", i + 1));
        let mut lines = text.lines().enumerate().filter(|(_, l)| !l.trim().is_empty());
        let (i, head) = lines.next().ok_or_else(|| Error::new("replay.invalid", "empty transcript"))?;
        let head: Value = serde_json::from_str(head).map_err(|e| bad(i, e.to_string()))?;
        if head.get("type").and_then(Value::as_str) != Some("transcript") {
            return Err(bad(i, "the first line is not a transcript header".into()));
        }
        if head.get("version").and_then(Value::as_u64) != Some(u64::from(TRANSCRIPT_VERSION)) {
            return Err(bad(i, format!("version {} is not {TRANSCRIPT_VERSION}", head["version"])));
        }
        let mut out = Transcript::default();
        for (i, l) in lines {
            let v: Value = serde_json::from_str(l).map_err(|e| bad(i, e.to_string()))?;
            out.lines.push(TranscriptLine::from_json(&v).map_err(|m| bad(i, m))?);
        }
        Ok(out)
    }
}

/// Where record-mode seams write: one transcript for several sources. A handle.
#[derive(Clone, Default)]
pub struct Recorder {
    lines: Arc<Mutex<Vec<TranscriptLine>>>,
}

impl Recorder {
    pub fn new() -> Recorder {
        Recorder::default()
    }

    fn push(&self, l: TranscriptLine) {
        self.lines.lock().unwrap_or_else(|p| p.into_inner()).push(l);
    }

    /// What was recorded so far.
    pub fn transcript(&self) -> Transcript {
        Transcript { lines: self.lines.lock().unwrap_or_else(|p| p.into_inner()).clone() }
    }
}

/// How a field is redacted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Redact {
    /// A URL becomes its route id ([`redact_url`]).
    Url,
    /// Any value becomes a stable id `<field>#<n>` (the n-th distinct value seen).
    Intern,
    /// The field is removed.
    Drop,
}

/// Field redaction rules for a source's events (see the module docs).
#[derive(Default)]
pub struct Redactor {
    rules: Vec<(String, Redact)>,
    interned: Mutex<HashMap<(String, String), String>>,
}

impl Redactor {
    pub fn new() -> Redactor {
        Redactor::default()
    }

    pub fn rule(mut self, field: impl Into<String>, r: Redact) -> Redactor {
        self.rules.push((field.into(), r));
        self
    }

    pub fn url(self, field: impl Into<String>) -> Redactor {
        self.rule(field, Redact::Url)
    }

    pub fn intern(self, field: impl Into<String>) -> Redactor {
        self.rule(field, Redact::Intern)
    }

    pub fn drop_field(self, field: impl Into<String>) -> Redactor {
        self.rule(field, Redact::Drop)
    }

    /// Applies the rules to `fields`.
    pub fn redact(&self, fields: &mut Map<String, Value>) {
        for (name, r) in &self.rules {
            let Some(v) = fields.get(name) else { continue };
            match r {
                Redact::Drop => {
                    fields.remove(name);
                }
                Redact::Url => {
                    if let Some(s) = v.as_str() {
                        let red = redact_url(s);
                        fields.insert(name.clone(), Value::String(red));
                    }
                }
                Redact::Intern => {
                    let raw = match v {
                        Value::String(s) => s.clone(),
                        v => v.to_string(),
                    };
                    let mut map = self.interned.lock().unwrap_or_else(|p| p.into_inner());
                    let n = map.keys().filter(|(f, _)| f == name).count() + 1;
                    let id = map.entry((name.clone(), raw)).or_insert_with(|| format!("{name}#{n}")).clone();
                    fields.insert(name.clone(), Value::String(id));
                }
            }
        }
    }
}

/// A URL's route id: `route:` and its path, without scheme, host, query or fragment, with number,
/// UUID and long hex segments replaced by `:id`. `https://x.io/api/parts/42?q=s` → `route:/api/parts/:id`.
pub fn redact_url(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let path = if url.contains("://") { rest.find('/').map_or("/", |i| &rest[i..]) } else { rest };
    let path = path.split(['?', '#']).next().unwrap_or("/");
    let segs: Vec<&str> = path
        .split('/')
        .map(|s| {
            let id_like = !s.is_empty()
                && (s.chars().all(|c| c.is_ascii_digit())
                    || (s.len() >= 16 && s.chars().all(|c| c.is_ascii_hexdigit() || c == '-')));
            if id_like { ":id" } else { s }
        })
        .collect();
    format!("route:{}", segs.join("/"))
}

/// A seam's mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeamMode {
    Live,
    Record,
    Replay,
}

enum Mode {
    Live,
    Record(Recorder),
    Replay(Vec<TranscriptLine>),
}

struct SeamState {
    /// Exchanges started per key (the next occurrence index).
    counts: HashMap<String, u32>,
    /// Replay: the virtual ms by which every replayed event is out.
    replay_until_ms: f64,
}

/// One source's seam (see the module docs). A handle: clones share the seam.
#[derive(Clone)]
pub struct SourceSeam {
    source: Arc<str>,
    journal: Journal,
    redactor: Arc<Redactor>,
    mode: Arc<Mode>,
    state: Arc<Mutex<SeamState>>,
    in_flight: Arc<AtomicU32>,
}

impl SourceSeam {
    fn with(source: &str, journal: Journal, mode: Mode) -> SourceSeam {
        SourceSeam {
            source: source.into(),
            journal,
            redactor: Arc::new(Redactor::new()),
            mode: Arc::new(mode),
            state: Arc::new(Mutex::new(SeamState { counts: HashMap::new(), replay_until_ms: 0.0 })),
            in_flight: Arc::new(AtomicU32::new(0)),
        }
    }

    /// Does the work and emits its events (Watch it, Try it, and production).
    pub fn live(source: &str, journal: Journal) -> SourceSeam {
        SourceSeam::with(source, journal, Mode::Live)
    }

    /// Live, and writes every event and result to `recorder`.
    pub fn record(source: &str, journal: Journal, recorder: &Recorder) -> SourceSeam {
        SourceSeam::with(source, journal, Mode::Record(recorder.clone()))
    }

    /// Replays `transcript`'s exchanges of this source (tests).
    pub fn replay(source: &str, journal: Journal, transcript: &Transcript) -> SourceSeam {
        let lines = transcript.lines.iter().filter(|l| l.at().0 == source).cloned().collect();
        SourceSeam::with(source, journal, Mode::Replay(lines))
    }

    /// The redaction rules (set before the first exchange).
    pub fn redactor(mut self, r: Redactor) -> SourceSeam {
        self.redactor = Arc::new(r);
        self
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn mode(&self) -> SeamMode {
        match *self.mode {
            Mode::Live => SeamMode::Live,
            Mode::Record(_) => SeamMode::Record,
            Mode::Replay(_) => SeamMode::Replay,
        }
    }

    pub fn is_replaying(&self) -> bool {
        self.mode() == SeamMode::Replay
    }

    fn state(&self) -> MutexGuard<'_, SeamState> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Starts an exchange with `key` (a route id, a worker task name; never a raw URL).
    pub fn exchange(&self, key: impl Into<String>) -> Exchange {
        let key = key.into();
        let n = {
            let mut st = self.state();
            let c = st.counts.entry(key.clone()).or_insert(0);
            *c += 1;
            *c - 1
        };
        self.in_flight.fetch_add(1, Ordering::SeqCst);
        Exchange { seam: self.clone(), key, n, start: Instant::now(), done: false }
    }

    /// The seam as a busy source: exchanges in flight (live and record, wall clock), or replayed
    /// events not yet out (replay, virtual clock: their delays run on it).
    pub fn busy_source(&self, kind: BusyKind) -> BusySource {
        let me = self.clone();
        let src = BusySource::new(format!("{}", self.source), move || {
            let live = me.in_flight.load(Ordering::SeqCst);
            let pending = me.is_replaying() && me.state().replay_until_ms > me.journal.virtual_ms();
            let count = live + u32::from(pending);
            if count == 0 { BusyReport::idle() } else { BusyReport::busy(count, format!("{} exchanges", me.source)) }
        })
        .kind(kind);
        src.domain(if self.is_replaying() { ClockDomain::Virtual } else { ClockDomain::Wall })
    }
}

/// One unit of a source's work (see [`SourceSeam::exchange`]). Dropping it finishes it without a
/// result.
pub struct Exchange {
    seam: SourceSeam,
    key: String,
    n: u32,
    start: Instant,
    done: bool,
}

impl Exchange {
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Live and record: emits an event (fields redacted) and records it. Replay: refused
    /// (`replay.live_emit`): a replayed source does no work, [`Exchange::replay`] emits.
    pub fn emit(&self, kind: &str, subject: impl Into<String>, fields: Value) -> Result<u64, Error> {
        let mut fields = match fields {
            Value::Null => Map::new(),
            Value::Object(m) => m,
            v => return Err(Error::new("event.invalid", format!("`{kind}` fields want an object, got {v}"))),
        };
        self.seam.redactor.redact(&mut fields);
        let subject = subject.into();
        match &*self.seam.mode {
            Mode::Replay(_) => Err(Error::new(
                "replay.live_emit",
                format!("`{}` is replaying: call Exchange::replay instead of doing the work", self.seam.source),
            )),
            mode => {
                let seq = self.seam.journal.emit(kind, subject.clone(), Value::Object(fields.clone()))?;
                if let Mode::Record(rec) = mode {
                    rec.push(TranscriptLine::Event {
                        source: self.seam.source.to_string(),
                        key: self.key.clone(),
                        n: self.n,
                        after_ms: self.start.elapsed().as_secs_f64() * 1000.0,
                        kind: kind.to_string(),
                        subject,
                        fields,
                    });
                }
                Ok(seq)
            }
        }
    }

    /// Live and record: ends the exchange with `result` (recorded as given).
    pub fn finish(mut self, result: Value) {
        if let Mode::Record(rec) = &*self.seam.mode {
            rec.push(TranscriptLine::Result { source: self.seam.source.to_string(), key: self.key.clone(), n: self.n, value: result });
        }
        self.end();
    }

    /// Replay: emits the recorded events of this exchange (each after its recorded delay, on the
    /// journal's virtual clock) and returns the recorded result (`null` when none was recorded).
    /// Fails with `replay.missing` when the transcript has no such exchange, and outside replay.
    pub fn replay(mut self) -> Result<Value, Error> {
        let Mode::Replay(lines) = &*self.seam.mode else {
            return Err(Error::new("replay.missing", format!("`{}` is not replaying", self.seam.source)));
        };
        let mine: Vec<&TranscriptLine> = lines.iter().filter(|l| l.at().1 == self.key && l.at().2 == self.n).collect();
        if mine.is_empty() {
            let key = self.key.clone();
            self.end();
            return Err(Error::new(
                "replay.missing",
                format!("the transcript has no exchange `{key}` #{} for `{}`: record it again", self.n, self.seam.source),
            ));
        }
        let mut result = Value::Null;
        let mut last = 0.0f64;
        for l in mine {
            match l {
                TranscriptLine::Event { after_ms, kind, subject, fields, .. } => {
                    self.seam.journal.emit_after(*after_ms, kind, subject.clone(), Value::Object(fields.clone()))?;
                    last = last.max(*after_ms);
                }
                TranscriptLine::Result { value, .. } => result = value.clone(),
            }
        }
        {
            let until = self.seam.journal.virtual_ms() + last;
            let mut st = self.seam.state();
            st.replay_until_ms = st.replay_until_ms.max(until);
        }
        self.end();
        Ok(result)
    }

    fn end(&mut self) {
        if !self.done {
            self.done = true;
            self.seam.in_flight.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

impl Drop for Exchange {
    fn drop(&mut self) {
        self.end();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal::{Cursor, EventKind, FieldType};

    fn journal() -> Journal {
        let j = Journal::default();
        j.register(EventKind::new("net.request", ClockDomain::Wall).field("url", FieldType::String).optional("body", FieldType::Any)).unwrap();
        j.register(EventKind::new("net.response", ClockDomain::Wall).field("status", FieldType::Number)).unwrap();
        j
    }

    #[test]
    fn urls_become_route_ids() {
        assert_eq!(redact_url("https://api.x.io/api/parts/42?q=secret#f"), "route:/api/parts/:id");
        assert_eq!(redact_url("/v1/jobs/3f2a9c1e-77aa-4b1e-9d00-1234567890ab/log"), "route:/v1/jobs/:id/log");
        assert_eq!(redact_url("https://x.io"), "route:/");
    }

    fn run(seam: &SourceSeam) -> Value {
        let x = seam.exchange("GET route:/parts");
        if seam.is_replaying() {
            return x.replay().unwrap();
        }
        x.emit("net.request", "route:/parts", json!({"url": "https://x.io/parts?token=abc", "body": {"secret": 1}})).unwrap();
        x.emit("net.response", "route:/parts", json!({"status": 200})).unwrap();
        x.finish(json!({"parts": 3}));
        json!({"parts": 3})
    }

    fn events(j: &Journal) -> Vec<(String, String, Map<String, Value>)> {
        j.since(Cursor(0)).unwrap().into_iter().map(|e| (e.kind, e.subject, e.fields)).collect()
    }

    #[test]
    fn a_replayed_transcript_emits_the_same_events_as_the_live_run() {
        let rec = Recorder::new();
        let live = journal();
        let seam = SourceSeam::record("net", live.clone(), &rec).redactor(Redactor::new().url("url").intern("body"));
        assert_eq!(run(&seam), json!({"parts": 3}));
        let text = rec.transcript().to_ndjson();
        assert!(!text.contains("token") && !text.contains("secret"), "{text}");

        let replayed = journal();
        let t = Transcript::from_ndjson(&text).unwrap();
        let seam = SourceSeam::replay("net", replayed.clone(), &t);
        assert_eq!(run(&seam), json!({"parts": 3}));
        replayed.set_virtual_ms(1_000.0);
        assert_eq!(events(&live), events(&replayed));
        assert_eq!(events(&live)[0].2["url"], "route:/parts");
        assert_eq!(events(&live)[0].2["body"], "body#1");

        // a second exchange with the same key was never recorded
        let e = seam.exchange("GET route:/parts").replay().unwrap_err();
        assert_eq!(e.code, "replay.missing");
        // replay does no work
        assert_eq!(seam.exchange("x").emit("net.response", "", json!({"status": 1})).unwrap_err().code, "replay.live_emit");
    }

    #[test]
    fn transcripts_check_their_header() {
        assert_eq!(Transcript::from_ndjson("{\"type\":\"event\"}").unwrap_err().code, "replay.invalid");
        assert_eq!(Transcript::from_ndjson("{\"type\":\"transcript\",\"version\":9}").unwrap_err().code, "replay.invalid");
        assert_eq!(Transcript::from_ndjson(&Transcript::default().to_ndjson()).unwrap(), Transcript::default());
    }
}
