//! Scripted interaction through the real toolkit.
//!
//! The driver runs inside the toolkit's loop (a repeating one-shot timer), performs one script
//! step at a time through [`Toolkit`], waits until the host is idle, and at the end dumps the
//! native widget tree, takes the screenshot and ends the run.
//!
//! Script steps are the `script.json` steps of the fixtures: `{"event": [handle, signal, args]}`
//! becomes a native action (`clicked` -> click the button, `changed` -> type into the entry), and
//! `{"checkpoint": true}` waits until the host is idle. Handle numbers are backend handles; the
//! fixtures' handle numbers are creation order, which is also how both backends number handles.

use serde_json::{Value as Json, json};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::{Duration, Instant};
use svelters_abi::{BackendRef, Value};
use svelters_backend_log::Step;
use svelters_host::Host;

const POLL_MS: u32 = 10;
/// Consecutive idle polls that count as settled.
const SETTLE_POLLS: u32 = 3;
/// Time given to the toolkit to map and paint the window before the screenshot.
const PAINT_MS: u32 = 250;
/// How long `run()` may take to return after the window was closed natively.
const CLOSE_GRACE: Duration = Duration::from_secs(5);
/// The root widget: handle 1 is the first widget a fixture creates, its window.
pub const ROOT: u64 = 1;

/// The toolkit side of the driver. All methods run on the toolkit's main thread.
pub trait Toolkit {
    /// Click the native button behind `h`.
    fn click(&self, h: u64) -> Result<(), String>;
    /// Type `text` into the native entry behind `h`, replacing its content.
    fn type_text(&self, h: u64, text: &str) -> Result<(), String>;
    /// The widget tree below `h` as a JSON array of `{d, class, text}` in layout order.
    fn dump(&self, h: u64) -> Json;
    /// Save a PNG of the widget behind `h`.
    fn screenshot(&self, h: u64, path: &str) -> Result<(), String>;
    /// Force layout and rendering of the widget behind `h` without saving anything.
    fn render(&self, h: u64) -> Result<(), String>;
    /// Close the top-level behind `h` as the user would.
    fn close(&self, h: u64) -> Result<(), String>;
    /// Native toplevel windows that still exist (a leak check after reload).
    fn native_toplevels(&self) -> usize;
    /// Run `f` on the toolkit loop after `ms` milliseconds.
    fn schedule(&self, ms: u32, f: Box<dyn FnOnce()>);
    /// `(nanoseconds, calls)` spent inside backend calls since the last take.
    fn take_toolkit_timing(&self) -> (u64, u64);
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum End {
    /// Ask the backend to quit.
    Quit,
    /// Close the window natively; `run()` must return on its own.
    Close,
}

pub struct Options {
    pub steps: Vec<Step>,
    pub screenshot: Option<String>,
    pub end: End,
    pub timeout: Duration,
    /// Record the time split per step (needs host and toolkit timing enabled).
    pub measure: bool,
}

#[derive(PartialEq, Eq)]
enum Phase {
    Steps,
    Painting,
    Ended,
}

struct Pending {
    index: usize,
    what: String,
    started: Instant,
    first_idle: Option<Instant>,
}

struct St {
    steps: VecDeque<Step>,
    index: usize,
    phase: Phase,
    idle_polls: u32,
    pending: Option<Pending>,
    failures: Vec<String>,
    dump: Option<Json>,
    step_log: Vec<Json>,
    started: Instant,
    closed_at: Option<Instant>,
}

pub struct Driver {
    tk: Rc<dyn Toolkit>,
    host: Rc<Host<'static>>,
    backend: BackendRef<'static>,
    opts: Options,
    st: RefCell<St>,
}

/// What the driver observed; the run's outcome beyond the host's own error list.
pub struct DriverReport {
    pub failures: Vec<String>,
    pub dump: Option<Json>,
    pub steps: Vec<Json>,
}

impl Driver {
    pub fn new(tk: Rc<dyn Toolkit>, host: Rc<Host<'static>>, backend: BackendRef<'static>, opts: Options) -> Rc<Driver> {
        let steps = opts.steps.iter().cloned().collect();
        Rc::new(Driver {
            tk,
            host,
            backend,
            st: RefCell::new(St {
                steps,
                index: 0,
                phase: Phase::Steps,
                idle_polls: 0,
                pending: None,
                failures: Vec::new(),
                dump: None,
                step_log: Vec::new(),
                started: Instant::now(),
                closed_at: None,
            }),
            opts,
        })
    }

    /// Schedule the first tick; the toolkit loop must be started afterwards.
    pub fn start(self: &Rc<Self>) {
        self.st.borrow_mut().started = Instant::now();
        self.schedule(POLL_MS);
    }

    pub fn report(&self) -> DriverReport {
        let mut st = self.st.borrow_mut();
        DriverReport {
            failures: std::mem::take(&mut st.failures),
            dump: st.dump.take(),
            steps: std::mem::take(&mut st.step_log),
        }
    }

    fn schedule(self: &Rc<Self>, ms: u32) {
        let me = self.clone();
        self.tk.schedule(ms, Box::new(move || me.tick()));
    }

    fn fail(&self, msg: String) {
        eprintln!("[counter] {msg}");
        self.st.borrow_mut().failures.push(msg);
    }

    fn tick(self: &Rc<Self>) {
        if self.st.borrow().phase == Phase::Ended {
            return;
        }
        if self.st.borrow().started.elapsed() > self.opts.timeout {
            self.fail(format!("timed out after {:?}", self.opts.timeout));
            self.end(true);
            return;
        }
        let phase_steps = self.st.borrow().phase == Phase::Steps;
        if !phase_steps {
            // Painting phase: one tick after PAINT_MS.
            self.finish();
            return;
        }

        // Settle detection: nothing queued in the host for SETTLE_POLLS consecutive polls, and at
        // least one checkpoint has run (the first one mounts the app).
        let idle = self.host.is_idle() && self.host.checkpoint_count() > 0;
        {
            let mut st = self.st.borrow_mut();
            if idle {
                st.idle_polls += 1;
                if let Some(p) = st.pending.as_mut() {
                    p.first_idle.get_or_insert_with(Instant::now);
                }
            } else {
                st.idle_polls = 0;
                if let Some(p) = st.pending.as_mut() {
                    p.first_idle = None;
                }
            }
        }
        if self.st.borrow().idle_polls < SETTLE_POLLS {
            self.schedule(POLL_MS);
            return;
        }

        self.record_pending();

        let step = self.st.borrow_mut().steps.pop_front();
        match step {
            None => {
                self.st.borrow_mut().phase = Phase::Painting;
                let dump = self.tk.dump(ROOT);
                self.st.borrow_mut().dump = Some(dump);
                self.schedule(PAINT_MS);
            }
            Some(step) => {
                self.perform(step);
                self.schedule(POLL_MS);
            }
        }
    }

    fn perform(&self, step: Step) {
        let index = {
            let mut st = self.st.borrow_mut();
            st.index += 1;
            st.idle_polls = 0;
            st.index - 1
        };
        let what = match &step {
            Step::Checkpoint => "checkpoint".to_string(),
            Step::Event { handle, signal, .. } => format!("{signal}#{handle}"),
            Step::Gone { handle, .. } => format!("gone#{handle}"),
            Step::Sync { handle, signal, .. } => format!("sync {signal}#{handle}"),
        };
        if self.opts.measure {
            let _ = self.host.take_timing();
            let _ = self.tk.take_toolkit_timing();
        }
        self.st.borrow_mut().pending = Some(Pending { index, what, started: Instant::now(), first_idle: None });
        // A native death in the script is produced by closing the window the way a user would.
        if let Step::Gone { handle, .. } = step {
            if let Err(e) = self.tk.close(handle) {
                self.fail(format!("step {index} (close #{handle}): {e}"));
            }
        }
        if let Step::Sync { handle, signal, .. } = &step {
            self.fail(format!("step {index}: sync step ({signal} on #{handle}) has no native action"));
        }
        if let Step::Event { handle, signal, args } = step {
            let res = match (signal.as_str(), args.first()) {
                ("clicked", _) => self.tk.click(handle),
                ("changed", Some(Value::Str(text))) => self.tk.type_text(handle, text),
                (other, _) => Err(format!("script event '{other}' has no native action")),
            };
            if let Err(e) = res {
                self.fail(format!("step {index} ({signal} on #{handle}): {e}"));
            }
        }
    }

    /// Writes the timing record of the step that just settled.
    fn record_pending(&self) {
        let Some(p) = self.st.borrow_mut().pending.take() else { return };
        if !self.opts.measure || p.what == "checkpoint" {
            return;
        }
        let host = self.host.take_timing();
        let (tk_ns, tk_calls) = self.tk.take_toolkit_timing();
        let wall = p.first_idle.unwrap_or_else(Instant::now).duration_since(p.started);
        let t0 = Instant::now();
        let render = self.tk.render(ROOT);
        let render_ms = t0.elapsed().as_secs_f64() * 1e3;
        if let Err(e) = render {
            self.fail(format!("render probe: {e}"));
        }
        let ms = |ns: u64| ns as f64 / 1e6;
        let js_only = host.js_ns.saturating_sub(host.ops_ns);
        let abi = host.ops_ns.saturating_sub(tk_ns);
        // Everything between the action and the first idle poll that is neither JS, host/ABI nor a
        // synchronous backend call: the toolkit's own event handling, layout and show/polish.
        let deferred = (wall.as_secs_f64() * 1e3 - ms(js_only) - ms(abi) - ms(tk_ns)).max(0.0);
        self.st.borrow_mut().step_log.push(json!({
            "step": p.index,
            "what": p.what,
            "wall_ms": wall.as_secs_f64() * 1e3,
            "js_ms": ms(js_only),
            "host_abi_ms": ms(abi),
            "toolkit_calls_ms": ms(tk_ns),
            "toolkit_deferred_ms": deferred,
            "host_ops": host.ops,
            "toolkit_calls": tk_calls,
            "checkpoints": host.checkpoints,
            "render_ms": render_ms,
        }));
    }

    fn finish(self: &Rc<Self>) {
        if let Some(path) = &self.opts.screenshot {
            if let Err(e) = self.tk.screenshot(ROOT, path) {
                self.fail(format!("screenshot: {e}"));
            }
        }
        self.end(false);
    }

    fn end(self: &Rc<Self>, force_quit: bool) {
        self.st.borrow_mut().phase = Phase::Ended;
        if force_quit || self.opts.end == End::Quit {
            self.backend.quit();
            return;
        }
        if let Err(e) = self.tk.close(ROOT) {
            self.fail(format!("close: {e}"));
            self.backend.quit();
            return;
        }
        self.st.borrow_mut().closed_at = Some(Instant::now());
        // If `run()` does not return by itself the backend is stuck; quit it and fail the run.
        let me = self.clone();
        self.tk.schedule(
            CLOSE_GRACE.as_millis() as u32,
            Box::new(move || {
                me.fail("run() did not return after the last window closed".into());
                me.backend.quit();
            }),
        );
    }
}
