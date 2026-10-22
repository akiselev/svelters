//! The step interpreter shared by the in-process backends (log, egui).

use crate::case::{Case, Expect, Step};
use crate::view::{self, ViewNode};

/// One backend running one case, driven step by step.
pub trait Target {
    /// Runs until the host is idle (no queued events, timers due, checkpoint or frame pending).
    fn settle(&mut self) -> Result<(), String>;
    /// The settled tree.
    fn view(&mut self) -> Result<Vec<ViewNode>, String>;
    /// Called after `settle` with the expectations checked on that state (the log backend records
    /// them for the GTK and Qt replays).
    fn checked(&mut self, _expects: &[Expect]) {}
    fn click(&mut self, text: &str) -> Result<(), String>;
    fn type_text(&mut self, field: &str, text: &str) -> Result<(), String>;
    fn wait(&mut self, ms: u64) -> Result<(), String>;
    /// `Ok(false)`: snapshots are not taken on this backend (skipped).
    fn snapshot(&mut self, _name: &str) -> Result<bool, String> {
        Ok(false)
    }
    /// Errors JS reported so far (taken).
    fn errors(&mut self) -> Vec<String>;
}

/// What running a case on one backend gave.
#[derive(Clone, Debug, Default)]
pub struct Outcome {
    pub failures: Vec<String>,
    /// Skipped steps and other remarks.
    pub notes: Vec<String>,
    /// Expectations checked.
    pub checks: usize,
}

impl Outcome {
    pub fn passed(&self) -> bool {
        self.failures.is_empty()
    }

    pub fn fail(msg: impl Into<String>) -> Outcome {
        Outcome { failures: vec![msg.into()], ..Default::default() }
    }
}

/// Runs `case` on `t`. Expectation failures are collected and the case goes on; a failed action
/// ends it (the steps after it would test a state the case did not reach).
pub fn run_case(case: &Case, t: &mut dyn Target) -> Outcome {
    let mut out = Outcome::default();
    let mut pending: Vec<(usize, Expect)> = Vec::new();
    let flush = |pending: &mut Vec<(usize, Expect)>, t: &mut dyn Target, out: &mut Outcome| -> Result<(), String> {
        if pending.is_empty() {
            return Ok(());
        }
        t.settle()?;
        let view = t.view()?;
        for (line, e) in pending.iter() {
            out.checks += 1;
            if let Err(msg) = view::check(e, &view) {
                out.failures.push(format!("line {line}: {msg}"));
            }
        }
        let expects: Vec<Expect> = pending.drain(..).map(|(_, e)| e).collect();
        t.checked(&expects);
        Ok(())
    };
    for (step, &line) in case.steps.iter().zip(&case.lines) {
        let r = match step {
            Step::Expect(e) => {
                pending.push((line, e.clone()));
                Ok(())
            }
            other => flush(&mut pending, t, &mut out).and_then(|_| match other {
                Step::Click(text) => t.settle().and_then(|_| t.click(text)),
                Step::Type { field, text } => t.settle().and_then(|_| t.type_text(field, text)),
                Step::Wait(ms) => t.wait(*ms),
                Step::Snapshot(name) => t.settle().and_then(|_| t.snapshot(name)).map(|taken| {
                    if !taken {
                        out.notes.push(format!("line {line}: snapshot {name:?} skipped"));
                    }
                }),
                Step::Expect(_) => unreachable!(),
            }),
        };
        if let Err(msg) = r {
            out.failures.push(format!("line {line}: {msg}"));
            break;
        }
    }
    if out.failures.is_empty() {
        if let Err(msg) = flush(&mut pending, t, &mut out) {
            out.failures.push(msg);
        }
    }
    for e in t.errors() {
        out.failures.push(format!("JS error: {e}"));
    }
    out
}
