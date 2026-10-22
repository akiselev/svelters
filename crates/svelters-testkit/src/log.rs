//! The log backend runner (`log-gtk`, `log-qt`): the case's bundle on the real QuickJS host and the
//! log backend, standing in for GTK or Qt with that toolkit's prop schema (as the op-log fixtures
//! do, `svelters_host::testing`).
//!
//! The log backend plays a script decided up front, so this runner replays: every `settle` runs the
//! bundle again from the start with the steps resolved so far (scripted mode is deterministic, so
//! handles are the same in every run) and rebuilds the tree from the op log ([`Mirror`]). The
//! resolved steps form a [`Plan`]: the handle-based script that `examples/counter` performs through
//! the real GTK and Qt toolkits, whose handles are numbered the same way (creation order).

use crate::case::{Case, Expect};
use crate::oplog::Mirror;
use crate::run::{Outcome, Target, run_case};
use crate::view::{self, ViewNode};
use serde_json::{Value as Json, json};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use svelters_abi::Value;
use svelters_backend_log::{LogBackend, LogOptions, Schema, Step, parse_schema};
use svelters_host::{Host, HostConfig};

/// The handle-based script for the real toolkits, and the expectations to check at each of its
/// checkpoints (`checks[i]` on the state at the i-th `{"checkpoint": true}`).
#[derive(Clone, Debug, Default)]
pub struct Plan {
    pub toolkit: String,
    pub script: Vec<Json>,
    pub checks: Vec<Vec<Expect>>,
}

impl Plan {
    pub fn script_json(&self) -> String {
        serde_json::to_string(&self.script).unwrap()
    }

    /// Checks the plan's expectations against the dumps taken at its checkpoints.
    pub fn verify(&self, dumps: &[Vec<ViewNode>]) -> Vec<String> {
        let mut failures = Vec::new();
        if dumps.len() != self.checks.len() {
            failures.push(format!("{} checkpoint dumps for {} checkpoints", dumps.len(), self.checks.len()));
        }
        for (i, (expects, view)) in self.checks.iter().zip(dumps).enumerate() {
            for e in expects {
                if let Err(msg) = view::check(e, view) {
                    failures.push(format!("checkpoint {i}: {msg}"));
                }
            }
        }
        failures
    }
}

/// The log backend's backend name for `toolkit`.
pub fn backend_name(toolkit: &str) -> String {
    format!("log-{toolkit}")
}

fn schema(toolkit: &str) -> Result<Option<Arc<Schema>>, String> {
    static QT: std::sync::OnceLock<Result<Arc<Schema>, String>> = std::sync::OnceLock::new();
    match toolkit {
        // the default schema, set by `fixture_config`
        "gtk" => Ok(None),
        "qt" => QT.get_or_init(|| parse_schema(include_str!("../../../packages/testhost/src/schema.qt.json")).map(Arc::new)).clone().map(Some),
        other => Err(format!("the log backend models gtk and qt, not {other}")),
    }
}

/// What one run of the bundle on the log backend gave.
pub struct LogRun {
    pub oplog: String,
    pub errors: Vec<String>,
}

/// Runs `bundle` once over `steps` on a log backend standing in for `toolkit`.
pub fn run_once(bundle: &Path, toolkit: &str, steps: Vec<Step>) -> Result<LogRun, String> {
    let (cfg, opts) = svelters_host::testing::fixture_config();
    let mut log = LogBackend::with_options(steps, LogOptions { settle: opts.settle });
    if let Some(s) = schema(toolkit)? {
        log = log.with_schema(s).with_style_target(svelters_abi::SVL_STYLE_TARGET_QSS);
    }
    let vt = log.vtable();
    let host = Host::new(vt.backend_ref(), HostConfig { ..cfg }).map_err(|e| e.to_string())?;
    let sink = log.clone();
    host.set_op_log(move |line| sink.append_line(line));
    host.load_bundle(bundle).map_err(|e| format!("load {}: {e}", bundle.display()))?;
    host.run().map_err(|e| e.to_string())?;
    host.drain_jobs();
    let errors = host.take_errors().iter().map(|e| e.to_string()).collect();
    drop(host);
    Ok(LogRun { oplog: log.log(), errors })
}

/// [`Target`] over the log backend.
pub struct LogTarget {
    pub bundle: PathBuf,
    pub toolkit: String,
    steps: Vec<Step>,
    mirror: Mirror,
    /// The op log of the last run.
    pub oplog: String,
    errors: Vec<String>,
    settled: bool,
    pub plan: Plan,
}

impl LogTarget {
    pub fn new(bundle: &Path, toolkit: &str) -> LogTarget {
        LogTarget {
            bundle: bundle.to_path_buf(),
            toolkit: toolkit.to_string(),
            steps: vec![Step::Checkpoint],
            mirror: Mirror::default(),
            oplog: String::new(),
            errors: Vec::new(),
            settled: false,
            plan: Plan { toolkit: toolkit.to_string(), ..Plan::default() },
        }
    }

    fn act(&mut self, step: Step, script: Json) {
        self.steps.push(step);
        self.steps.push(Step::Checkpoint);
        self.plan.script.push(script);
        self.settled = false;
    }
}

impl Target for LogTarget {
    fn settle(&mut self) -> Result<(), String> {
        if self.settled {
            return Ok(());
        }
        let r = run_once(&self.bundle, &self.toolkit, self.steps.clone())?;
        self.mirror = Mirror::replay(&r.oplog)?;
        self.oplog = r.oplog;
        // every run repeats the earlier runs' errors: keep the last run's
        self.errors = r.errors;
        self.settled = true;
        Ok(())
    }

    fn view(&mut self) -> Result<Vec<ViewNode>, String> {
        Ok(self.mirror.view())
    }

    fn checked(&mut self, expects: &[Expect]) {
        // a real toolkit checks them on the dump it takes at this checkpoint
        self.plan.script.push(json!({"checkpoint": true}));
        self.plan.checks.push(expects.to_vec());
    }

    fn click(&mut self, text: &str) -> Result<(), String> {
        let h = self.mirror.button(text)?;
        self.act(Step::Event { handle: h, signal: "clicked".into(), args: vec![], flags: 0 }, json!({"event": [h, "clicked", []]}));
        Ok(())
    }

    fn type_text(&mut self, field: &str, text: &str) -> Result<(), String> {
        let h = self.mirror.field(field)?;
        let n = &self.mirror.nodes[&h];
        // the signal a real toolkit's typing emits (GtkEditable::changed, QLineEdit::textChanged)
        let signal = if self.toolkit == "qt" { "text-changed" } else { "changed" };
        let real = json!({"event": [h, signal, [text]]});
        if n.observing.contains("text") {
            // R2: `bind:text` observes the prop; the log backend takes the new native value
            self.act(Step::Changed { handle: h, prop: "text".into(), value: Some(Value::Str(text.into())) }, real);
        } else if n.listening.contains(signal) {
            self.act(Step::Event { handle: h, signal: signal.into(), args: vec![Value::Str(text.into())], flags: 0 }, real);
        } else {
            return Err(format!("text input #{h} ({field:?}) neither observes text nor listens to {signal}"));
        }
        Ok(())
    }

    fn wait(&mut self, ms: u64) -> Result<(), String> {
        // the real toolkits' driver has no sleep step: each checkpoint step waits until the host
        // has been idle for 3 polls of 10 ms, so a run of them lets at least `ms` pass (their dumps
        // check nothing)
        for _ in 0..ms.div_ceil(30).max(1) {
            self.plan.script.push(json!({"checkpoint": true}));
            self.plan.checks.push(Vec::new());
        }
        // scripted mode: a checkpoint with no queued events fires the timers on the virtual clock
        self.steps.push(Step::Checkpoint);
        self.settled = false;
        Ok(())
    }

    fn errors(&mut self) -> Vec<String> {
        std::mem::take(&mut self.errors)
    }
}

/// Runs `case` on the log backend for `toolkit`; also returns the plan for the real toolkit.
pub fn run(case: &Case, bundle: &Path, toolkit: &str) -> (Outcome, Plan) {
    if !bundle.is_file() {
        return (Outcome::fail(format!("{} not built (pnpm --filter ./tests/cases build)", bundle.display())), Plan::default());
    }
    let mut t = LogTarget::new(bundle, toolkit);
    let out = run_case(case, &mut t);
    (out, t.plan)
}
