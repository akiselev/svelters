//! WG3 native-behaviour references (widget survey NA1, `plans/milestones/WG3-native-references.md`).
//!
//! - [`pins`]: the pin scripts (`conformance/pins/*.json`) and the key-role table.
//! - [`trace`]: the `state #h {…}` trace writer for `tools/oplog-compare.mjs`.
//! - [`golden`]: `conformance/golden/<platform>/<toolkit>-<version>.json`, capture and compare.
//! - [`xtest`]: the XTEST user-path injector the GTK and Qt captures use.
//! - [`win`]: the Windows user path (WG3w): `SendInput` and UI Automation, one driver for every fixture.
//! - [`run_pin`]: drives one pin through a [`Driver`] (one per toolkit, in the tests of this crate,
//!   `examples/counter-qt/tests/na_capture.rs` and this crate's egui measurement).

pub mod golden;
pub mod pins;
pub mod trace;
#[cfg(windows)]
pub mod win;
#[cfg(target_os = "linux")]
pub mod xtest;

use golden::PinResult;
use pins::{Action, Chord, Keys, Pin};
use trace::Checkpoint;

/// One toolkit's fixture and input. The capture builds a fresh fixture per pin.
pub trait Driver {
    /// Builds the fixture window (fields `a`, `b`, button `c`) and applies the pin's setup.
    fn setup(&mut self, pin: &Pin) -> Result<(), String>;
    fn key(&mut self, chord: &Chord) -> Result<(), String>;
    /// `at`: a character offset in a field (`None` on the button: its centre).
    fn click(&mut self, target: &str, at: Option<usize>, count: u32) -> Result<(), String>;
    fn drag(&mut self, target: &str, from: usize, to: usize, count: u32) -> Result<(), String>;
    /// Reads every anchor's state and writes the `state` lines into the trace.
    fn observe(&mut self) -> Result<Checkpoint, String>;
    /// Destroys the fixture and returns the pin's trace (op log with `state` lines).
    fn teardown(&mut self) -> Result<String, String>;
}

/// Runs one pin: setup, a checkpoint, then each action followed by a checkpoint.
pub fn run_pin(d: &mut dyn Driver, pin: &Pin, keys: &Keys) -> Result<(PinResult, String), String> {
    let ctx = |m: String| format!("pin {}: {m}", pin.id);
    d.setup(pin).map_err(ctx)?;
    let mut checkpoints = vec![("setup".to_string(), d.observe().map_err(ctx)?)];
    for a in &pin.actions {
        match a {
            Action::Key { intent, shift } => d.key(&keys.chord(intent, *shift).map_err(ctx)?),
            Action::Click { target, at, count } => d.click(target, *at, *count),
            Action::Drag {
                target,
                from,
                to,
                count,
            } => d.drag(target, *from, *to, *count),
        }
        .map_err(|m| ctx(format!("{}: {m}", a.label())))?;
        checkpoints.push((a.label(), d.observe().map_err(ctx)?));
    }
    let trace = d.teardown().map_err(ctx)?;
    Ok((
        PinResult {
            id: pin.id.clone(),
            pin: pin.pin,
            fingerprint: pins::fingerprint(pin, keys),
            checkpoints,
        },
        trace,
    ))
}

/// Re-runs this test process on a private Xvfb (`tools/dev/headless` with
/// `SVELTERS_HEADLESS_FORCE=1`): XTEST input goes to the whole display, so a capture must not share
/// one with other tests. Returns `Some(passed)` in the parent, `None` in the private child.
pub fn rerun_on_private_display(args: &[&str]) -> Option<bool> {
    if std::env::var_os("SVELTERS_NA_PRIVATE").is_some() {
        return None;
    }
    let exe = std::env::current_exe().expect("test executable");
    let headless = pins::repo_root().join("tools/dev/headless");
    let status = std::process::Command::new(headless)
        .arg(exe)
        .args(args)
        .env("SVELTERS_HEADLESS_FORCE", "1")
        .env("SVELTERS_NA_PRIVATE", "1")
        .status()
        .expect("run tools/dev/headless");
    Some(status.success())
}
