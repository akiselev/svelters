//! A backend that records every operation in the shared op-log format
//! (`plans/architecture/backend-abi.md#op-log-format`) and drives a fake event loop from a script.
//!
//! The fake loop in [`Backend::run`]:
//! 1. processes the script steps in order: `event` logs the event, posts it to the host and
//!    calls `host.checkpoint` so it is delivered at once; `checkpoint` calls `host.checkpoint`
//!    (what that drains is the host's business; see `HostConfig::scripted`);
//! 2. before each step, and after the last, runs a checkpoint if the host called `wake` meanwhile;
//! 3. then idles, running a checkpoint on every `wake`, and returns when `quit` is called or when
//!    no wake arrives for [`LogOptions::settle`].

use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use svelters_abi::{
    Backend, BackendVtable, HostRef, MemberDesc, NameKind, SVL_BOOL, SVL_F64, SVL_GONE_PARENT, SVL_I64, SVL_LIFE_OWNED,
    SVL_LIFE_TOPLEVEL, SlotDesc, Value, Waker, gone_from_name, gone_name, svl_handle, svl_handle_info, svl_id,
};

/// One scripted interaction step.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    Event {
        handle: svl_handle,
        signal: String,
        args: Vec<Value>,
    },
    /// The native object behind `handle` dies for `reason` (an `SVL_GONE_*` code), taking its
    /// descendants with it. Logs `gone #h <reason>` for each and reports them to the host.
    Gone {
        handle: svl_handle,
        reason: u32,
    },
    Checkpoint,
}

/// Parses a fixture's `script.json`: an array of `{"event": [h, signal, args]}`,
/// `{"gone": [h, reason]}` and `{"checkpoint": true}` steps.
pub fn parse_script(json: &str) -> Result<Vec<Step>, String> {
    let root: serde_json::Value = serde_json::from_str(json).map_err(|e| format!("script: {e}"))?;
    let steps = root.as_array().ok_or("script: expected a JSON array")?;
    steps
        .iter()
        .enumerate()
        .map(|(i, s)| parse_step(s).map_err(|e| format!("script step {i}: {e}")))
        .collect()
}

fn parse_step(s: &serde_json::Value) -> Result<Step, String> {
    let obj = s.as_object().ok_or("expected an object")?;
    if let Some(ev) = obj.get("event") {
        let ev = ev
            .as_array()
            .ok_or("event: expected [handle, signal, args]")?;
        let handle = ev
            .first()
            .and_then(|h| h.as_u64())
            .ok_or("event: bad handle")?;
        let signal = ev
            .get(1)
            .and_then(|s| s.as_str())
            .ok_or("event: bad signal")?
            .to_string();
        let args = match ev.get(2) {
            None => Vec::new(),
            Some(a) => a
                .as_array()
                .ok_or("event: args must be an array")?
                .iter()
                .map(json_to_value)
                .collect::<Result<_, _>>()?,
        };
        Ok(Step::Event {
            handle,
            signal,
            args,
        })
    } else if let Some(g) = obj.get("gone") {
        let g = g.as_array().ok_or("gone: expected [handle, reason]")?;
        let handle = g.first().and_then(|h| h.as_u64()).ok_or("gone: bad handle")?;
        let name = g.get(1).and_then(|s| s.as_str()).ok_or("gone: bad reason")?;
        let reason = gone_from_name(name).ok_or_else(|| format!("gone: unknown reason `{name}`"))?;
        Ok(Step::Gone { handle, reason })
    } else if obj.contains_key("checkpoint") {
        Ok(Step::Checkpoint)
    } else {
        Err("unknown step (expected `event`, `gone` or `checkpoint`)".into())
    }
}

fn json_to_value(v: &serde_json::Value) -> Result<Value, String> {
    use serde_json::Value as J;
    Ok(match v {
        J::Null => Value::Null,
        J::Bool(b) => Value::Bool(*b),
        J::Number(n) => match n.as_i64() {
            Some(i) => Value::I64(i),
            None => Value::F64(n.as_f64().ok_or("number out of range")?),
        },
        J::String(s) => Value::Str(s.clone()),
        J::Array(a) => Value::List(a.iter().map(json_to_value).collect::<Result<_, _>>()?),
        J::Object(_) => return Err("objects are not valid event arguments".into()),
    })
}

/// Formats a number the way `JSON.stringify` does.
fn js_number(f: f64, out: &mut String) {
    if !f.is_finite() {
        out.push_str("null");
    } else if f == 0.0 {
        out.push('0');
    } else if f.abs() >= 1e21 {
        let s = format!("{f:e}");
        match s.split_once('e') {
            Some((m, e)) if !e.starts_with('-') => {
                let _ = write!(out, "{m}e+{e}");
            }
            _ => out.push_str(&s),
        }
    } else if f.abs() < 1e-6 {
        let _ = write!(out, "{f:e}");
    } else {
        let _ = write!(out, "{f}");
    }
}

fn write_json(v: &Value, out: &mut String) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::I64(i) => {
            let _ = write!(out, "{i}");
        }
        Value::F64(f) | Value::DateTime(f) => js_number(*f, out),
        Value::Handle(h) => {
            let _ = write!(out, "{h}");
        }
        Value::Str(s) => out.push_str(&serde_json::to_string(s).expect("string serializes")),
        Value::Str16(u) => out.push_str(&serde_json::to_string(&String::from_utf16_lossy(u)).expect("string serializes")),
        Value::List(items) => {
            out.push('[');
            for (i, x) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_json(x, out);
            }
            out.push(']');
        }
        // Typed values carry no names without descriptors; the log prints their raw payloads.
        Value::U64(u) => {
            let _ = write!(out, "{u}");
        }
        Value::Enum { value, .. } => {
            let _ = write!(out, "{value}");
        }
        Value::Flags { mask, .. } => {
            let _ = write!(out, "{mask}");
        }
        Value::Record { fields, .. } => write_json(&Value::List(fields.clone()), out),
        Value::Bytes(b) => write_json(&Value::List(b.iter().map(|x| Value::I64(*x as i64)).collect()), out),
        Value::Boxed { handle, .. } => {
            let _ = write!(out, "{handle}");
        }
        Value::Callback { id, .. } => {
            let _ = write!(out, "\"<callback {id}>\"");
        }
        Value::Map(entries) => {
            out.push('{');
            for (i, (k, x)) in entries.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(k).expect("string serializes"));
                out.push(':');
                write_json(x, out);
            }
            out.push('}');
        }
        Value::Error(e) => {
            out.push_str("{\"error\":");
            out.push_str(&serde_json::to_string(&e.message).expect("string serializes"));
            out.push('}');
        }
    }
}

fn json_string(v: &Value) -> String {
    let mut s = String::new();
    write_json(v, &mut s);
    s
}

#[derive(Default)]
struct Names {
    ids: HashMap<(u32, u32, String), svl_id>,
    rev: Vec<(u32, u32, String)>,
}

impl Names {
    fn intern(&mut self, what: u32, kind: svl_id, name: &str) -> svl_id {
        if let Some(id) = self.ids.get(&(what, kind, name.to_string())) {
            return *id;
        }
        self.rev.push((what, kind, name.to_string()));
        let id = self.rev.len() as svl_id;
        self.ids.insert((what, kind, name.to_string()), id);
        id
    }

    fn name(&self, id: svl_id) -> &str {
        &self.rev[(id - 1) as usize].2
    }
}

/// Element tag -> prop -> coarse type (`integer`, `number`, `boolean`), as in
/// `packages/testhost/src/schema.json`.
pub type Schema = HashMap<String, HashMap<String, String>>;

/// Parses the JSON form of a [`Schema`].
pub fn parse_schema(json: &str) -> Result<Schema, String> {
    serde_json::from_str(json).map_err(|e| format!("schema: {e}"))
}

/// Fake-loop tuning.
#[derive(Clone, Debug)]
pub struct LogOptions {
    /// How long the loop idles without a `wake` after the script before it returns.
    pub settle: Duration,
}

static DEFAULT_SCHEMA: std::sync::OnceLock<Arc<Schema>> = std::sync::OnceLock::new();

/// Makes every `LogBackend` created afterwards publish property descriptors from `schema`: a
/// stand-in for the descriptors of a real backend, so the host coerces attribute strings the way it
/// does for a native toolkit and the log matches the Node TestHost's, which uses the same schema.
/// Process-wide and set once (the fixture harness does it); later calls are ignored.
pub fn set_default_schema(schema: Schema) {
    let _ = DEFAULT_SCHEMA.set(Arc::new(schema));
}

impl Default for LogOptions {
    fn default() -> Self {
        LogOptions {
            settle: Duration::from_millis(150),
        }
    }
}

struct State {
    log: String,
    next_handle: u64,
    live: HashMap<svl_handle, svl_id>,
    props: HashMap<(svl_handle, svl_id), Value>,
    parent: HashMap<svl_handle, svl_handle>,
    children: HashMap<svl_handle, Vec<svl_handle>>,
    names: Names,
    script: Vec<Step>,
    wake_pending: bool,
    quit: bool,
    checkpoints: u64,
    /// Deadline of the pending `schedule_wake`, if any.
    wake_deadline: Option<Instant>,
    timer_fires: u64,
    schedule_calls: u64,
}

struct Shared {
    st: Mutex<State>,
    cv: Condvar,
    wakes: AtomicU64,
    opts: LogOptions,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.st.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// The log backend. Clones share state, so keep one clone to inspect the log after
/// [`BackendVtable::new`] consumes the other.
#[derive(Clone)]
pub struct LogBackend {
    shared: Arc<Shared>,
}

impl LogBackend {
    pub fn new(script: Vec<Step>) -> LogBackend {
        LogBackend::with_options(script, LogOptions::default())
    }

    pub fn with_options(script: Vec<Step>, opts: LogOptions) -> LogBackend {
        LogBackend {
            shared: Arc::new(Shared {
                st: Mutex::new(State {
                    log: String::new(),
                    next_handle: 1,
                    live: HashMap::new(),
                    props: HashMap::new(),
                    parent: HashMap::new(),
                    children: HashMap::new(),
                    names: Names::default(),
                    script,
                    wake_pending: false,
                    quit: false,
                    checkpoints: 0,
                    wake_deadline: None,
                    timer_fires: 0,
                    schedule_calls: 0,
                }),
                cv: Condvar::new(),
                wakes: AtomicU64::new(0),
                opts,
            }),
        }
    }

    /// Builds the vtable, keeping `self` as an inspection handle.
    pub fn vtable(&self) -> BackendVtable {
        BackendVtable::new(self.clone())
    }

    /// The op log so far: one op per line, each line newline-terminated.
    pub fn log(&self) -> String {
        self.shared.lock().log.clone()
    }

    /// Number of `wake` calls received (from any thread).
    pub fn wake_count(&self) -> u64 {
        self.shared.wakes.load(Ordering::SeqCst)
    }

    /// Number of times a `schedule_wake` deadline expired and ran a checkpoint.
    pub fn timer_fire_count(&self) -> u64 {
        self.shared.lock().timer_fires
    }

    /// Number of `schedule_wake` calls received.
    pub fn schedule_call_count(&self) -> u64 {
        self.shared.lock().schedule_calls
    }

    /// Number of checkpoints the fake loop has requested.
    pub fn checkpoint_count(&self) -> u64 {
        self.shared.lock().checkpoints
    }

    fn append(&self, line: std::fmt::Arguments<'_>) {
        let mut st = self.shared.lock();
        let _ = st.log.write_fmt(line);
        st.log.push('\n');
    }

    fn check_live(&self, h: svl_handle) -> Result<svl_id, String> {
        self.shared
            .lock()
            .live
            .get(&h)
            .copied()
            .ok_or_else(|| format!("stale or foreign handle #{h}"))
    }

    fn checkpoint(&self, host: &HostRef<'_>) {
        self.shared.lock().checkpoints += 1;
        host.checkpoint();
    }

    /// Kills `h` and its descendants (pre-order): logs `gone #x <reason>` for each (`reason` for
    /// `h`, `parent` below it) and forgets them. Returns the list in report order.
    fn kill(&self, h: svl_handle, reason: u32) -> Vec<(svl_handle, u32)> {
        let mut st = self.shared.lock();
        let mut dying = Vec::new();
        let mut stack = vec![(h, reason)];
        while let Some((x, why)) = stack.pop() {
            dying.push((x, why));
            if let Some(kids) = st.children.get(&x) {
                stack.extend(kids.iter().rev().map(|c| (*c, SVL_GONE_PARENT)));
            }
        }
        if let Some(p) = st.parent.remove(&h) {
            if let Some(c) = st.children.get_mut(&p) {
                c.retain(|x| *x != h);
            }
        }
        for (x, why) in &dying {
            let _ = writeln!(st.log, "gone #{x} {}", gone_name(*why));
        }
        for (x, _) in &dying {
            st.live.remove(x);
            st.parent.remove(x);
            st.children.remove(x);
            st.props.retain(|(ph, _), _| ph != x);
        }
        dying
    }

    fn take_wake(&self) -> bool {
        std::mem::take(&mut self.shared.lock().wake_pending)
    }

    fn quit_requested(&self) -> bool {
        self.shared.lock().quit
    }
}

impl Backend for LogBackend {
    fn resolve(&self, what: NameKind, kind: svl_id, name: &str) -> svl_id {
        self.shared.lock().names.intern(what as u32, kind, name)
    }

    fn describe(&self, what: NameKind, kind: svl_id, member: svl_id) -> Option<MemberDesc> {
        let schema = DEFAULT_SCHEMA.get()?;
        if what != NameKind::Prop {
            return None;
        }
        let st = self.shared.lock();
        let (_, _, name) = st.names.rev.get((member as usize).checked_sub(1)?)?;
        let ty = schema.get(st.names.name(kind))?.get(name)?;
        let slot = match ty.as_str() {
            "integer" => SlotDesc::new(SVL_I64),
            "number" => SlotDesc::new(SVL_F64),
            "boolean" => SlotDesc::new(SVL_BOOL),
            _ => return None,
        };
        let slot = slot.named(name);
        Some(MemberDesc {
            name: name.clone(),
            params: vec![slot.clone()],
            ret: slot,
            flags: 0,
        })
    }

    fn create(&self, kind: svl_id) -> Result<svl_handle, String> {
        let mut st = self.shared.lock();
        let h = st.next_handle;
        st.next_handle += 1;
        st.live.insert(h, kind);
        let name = st.names.name(kind).to_string();
        let _ = writeln!(st.log, "create {name} #{h}");
        Ok(h)
    }

    fn insert(
        &self,
        parent: svl_handle,
        child: svl_handle,
        before: Option<svl_handle>,
        slot: svl_id,
    ) -> Result<(), String> {
        self.check_live(parent)?;
        self.check_live(child)?;
        if let Some(b) = before {
            self.check_live(b)?;
        }
        let mut st = self.shared.lock();
        if parent == child {
            return Err(format!("cannot insert #{child} into itself"));
        }
        if let Some(p) = st.parent.get(&child) {
            return Err(format!("#{child} already has parent #{p}"));
        }
        let siblings = st.children.entry(parent).or_default();
        let at = match before {
            Some(b) => siblings
                .iter()
                .position(|c| *c == b)
                .ok_or_else(|| format!("before #{b} is not a child of #{parent}"))?,
            None => siblings.len(),
        };
        siblings.insert(at, child);
        st.parent.insert(child, parent);
        let before = before.map_or("end".to_string(), |b| format!("#{b}"));
        let slot = if slot == 0 {
            "default".to_string()
        } else {
            st.names.name(slot).to_string()
        };
        let _ = writeln!(
            st.log,
            "insert #{parent} #{child} before={before} slot={slot}"
        );
        Ok(())
    }

    fn remove(&self, parent: svl_handle, child: svl_handle) -> Result<(), String> {
        self.check_live(parent)?;
        self.check_live(child)?;
        let mut st = self.shared.lock();
        if st.parent.get(&child) != Some(&parent) {
            return Err(format!("#{child} is not a child of #{parent}"));
        }
        st.parent.remove(&child);
        if let Some(c) = st.children.get_mut(&parent) {
            c.retain(|x| *x != child);
        }
        let _ = writeln!(st.log, "remove #{parent} #{child}");
        Ok(())
    }

    fn destroy(&self, h: svl_handle) {
        let mut st = self.shared.lock();
        if st.live.remove(&h).is_some() {
            if let Some(c) = st.parent.remove(&h).and_then(|p| st.children.get_mut(&p)) {
                c.retain(|x| *x != h);
            }
            st.children.remove(&h);
            st.props.retain(|(ph, _), _| *ph != h);
            let _ = writeln!(st.log, "destroy #{h}");
        }
    }

    fn set_prop(&self, h: svl_handle, prop: svl_id, v: &Value) -> Result<(), String> {
        self.check_live(h)?;
        let mut st = self.shared.lock();
        let name = st.names.name(prop).to_string();
        let _ = writeln!(st.log, "set #{h} {name} {}", json_string(v));
        st.props.insert((h, prop), v.clone());
        Ok(())
    }

    fn get_prop(&self, h: svl_handle, prop: svl_id) -> Result<Value, String> {
        self.check_live(h)?;
        Ok(self
            .shared
            .lock()
            .props
            .get(&(h, prop))
            .cloned()
            .unwrap_or(Value::Null))
    }

    fn call(&self, h: svl_handle, method: svl_id, args: &[Value]) -> Result<Value, String> {
        self.check_live(h)?;
        let mut st = self.shared.lock();
        let name = st.names.name(method).to_string();
        let _ = writeln!(
            st.log,
            "call #{h} {name} {}",
            json_string(&Value::List(args.to_vec()))
        );
        Ok(Value::Null)
    }

    fn listen(&self, h: svl_handle, signal: svl_id, on: bool) -> Result<(), String> {
        self.check_live(h)?;
        let mut st = self.shared.lock();
        let name = st.names.name(signal).to_string();
        let _ = writeln!(
            st.log,
            "listen #{h} {name} {}",
            if on { "on" } else { "off" }
        );
        Ok(())
    }

    fn handle_info(&self, h: svl_handle) -> Result<svl_handle_info, String> {
        let kind = self.check_live(h)?;
        let toplevel = self.shared.lock().names.name(kind) == "window";
        Ok(svl_handle_info {
            kind,
            lifetime: if toplevel { SVL_LIFE_TOPLEVEL } else { SVL_LIFE_OWNED },
            flags: 0,
        })
    }

    fn schedule_wake(&self, ms: i64) {
        let mut st = self.shared.lock();
        st.schedule_calls += 1;
        st.wake_deadline = (ms >= 0).then(|| Instant::now() + Duration::from_millis(ms as u64));
        drop(st);
        self.shared.cv.notify_all();
    }

    fn run(&self, host: &HostRef<'_>) -> Result<(), String> {
        let steps = std::mem::take(&mut self.shared.lock().script);
        for step in steps {
            if self.quit_requested() {
                return Ok(());
            }
            if self.take_wake() {
                self.checkpoint(host);
            }
            match step {
                Step::Checkpoint => self.checkpoint(host),
                Step::Gone { handle, reason } => {
                    self.check_live(handle)?;
                    for (h, why) in self.kill(handle, reason) {
                        host.on_destroyed(h, why);
                    }
                    self.checkpoint(host);
                }
                Step::Event {
                    handle,
                    signal,
                    args,
                } => {
                    let kind = self.check_live(handle)?;
                    let sig =
                        self.shared
                            .lock()
                            .names
                            .intern(NameKind::Signal as u32, kind, &signal);
                    self.append(format_args!(
                        "event #{handle} {signal} {}",
                        json_string(&Value::List(args.clone()))
                    ));
                    host.post_event(handle, sig, &args);
                    self.checkpoint(host);
                }
            }
        }
        loop {
            let mut st = self.shared.lock();
            let mut timer_fired = false;
            while !st.wake_pending && !st.quit && !timer_fired {
                // A pending `schedule_wake` keeps the loop alive until its deadline; otherwise
                // the loop returns once nothing has woken it for `settle`.
                let wait = match st.wake_deadline {
                    Some(d) => {
                        let now = Instant::now();
                        if now >= d {
                            st.wake_deadline = None;
                            st.timer_fires += 1;
                            timer_fired = true;
                            continue;
                        }
                        d - now
                    }
                    None => self.shared.opts.settle,
                };
                let (g, res) = self
                    .shared
                    .cv
                    .wait_timeout(st, wait)
                    .unwrap_or_else(|e| e.into_inner());
                st = g;
                if res.timed_out() && !st.wake_pending && st.wake_deadline.is_none() {
                    return Ok(());
                }
            }
            if st.quit {
                return Ok(());
            }
            st.wake_pending = false;
            drop(st);
            self.checkpoint(host);
        }
    }

    fn waker(&self) -> Waker {
        let shared = self.shared.clone();
        Arc::new(move || {
            shared.wakes.fetch_add(1, Ordering::SeqCst);
            shared.lock().wake_pending = true;
            shared.cv.notify_all();
        })
    }

    fn quit(&self) {
        self.shared.lock().quit = true;
        self.shared.cv.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_formatting_matches_js() {
        let f = |x: f64| json_string(&Value::F64(x));
        assert_eq!(f(1.0), "1");
        assert_eq!(f(-0.0), "0");
        assert_eq!(f(1.5), "1.5");
        assert_eq!(f(1e21), "1e+21");
        assert_eq!(f(1.5e-7), "1.5e-7");
        assert_eq!(f(f64::NAN), "null");
        assert_eq!(f(100000.0), "100000");
    }

    #[test]
    fn strings_and_lists_are_compact_json() {
        let v = Value::List(vec![
            Value::Str("a\"b\n".into()),
            Value::Null,
            Value::Bool(true),
            Value::I64(-3),
        ]);
        assert_eq!(json_string(&v), r#"["a\"b\n",null,true,-3]"#);
    }

    #[test]
    fn script_parses() {
        let s = parse_script(r#"[{"event":[1,"click",[]]},{"checkpoint":true}]"#).unwrap();
        assert_eq!(
            s,
            vec![
                Step::Event {
                    handle: 1,
                    signal: "click".into(),
                    args: vec![]
                },
                Step::Checkpoint
            ]
        );
        assert!(parse_script(r#"[{"nope":1}]"#).is_err());
    }

    #[test]
    fn ops_are_logged_through_the_vtable() {
        let log = LogBackend::new(vec![]);
        let vt = log.vtable();
        let b = vt.backend_ref();
        let kind = b.resolve(NameKind::Kind, 0, "box");
        let parent = b.create(kind).unwrap();
        let child = b.create(b.resolve(NameKind::Kind, 0, "label")).unwrap();
        b.insert(parent, child, None, 0).unwrap();
        b.set_prop(
            child,
            b.resolve(NameKind::Prop, kind, "label"),
            &Value::Str("hi".into()),
        )
        .unwrap();
        b.call(
            parent,
            b.resolve(NameKind::Method, kind, "show"),
            &[Value::I64(1)],
        )
        .unwrap();
        b.remove(parent, child).unwrap();
        b.destroy(child);
        assert!(b.insert(parent, child, None, 0).is_err());
        assert_eq!(b.last_error(), "stale or foreign handle #2");
        assert_eq!(
            log.log(),
            "create box #1\ncreate label #2\ninsert #1 #2 before=end slot=default\nset #2 label \"hi\"\ncall #1 show [1]\nremove #1 #2\ndestroy #2\n"
        );
    }
}
