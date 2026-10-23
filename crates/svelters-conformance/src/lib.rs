//! WG3 native-behaviour references (widget survey NA1, `plans/milestones/WG3-native-references.md`).
//!
//! - [`pins`]: the pin scripts (`conformance/pins/*.json`) and the key-role table.
//! - [`trace`]: the `state #h {…}` trace writer for `tools/oplog-compare.mjs`.
//! - [`scroll`]: WG3s scroll samples and motion metrics (pins 9 and 10).
//! - [`golden`]: `conformance/golden/<platform>/<toolkit>-<version>.json`, capture and compare.
//! - [`xtest`]: the XTEST user-path injector the GTK and Qt captures use.
//! - [`win`]: the Windows user path (WG3w): `SendInput` and UI Automation, one driver for every fixture.
//! - [`macos`]: the CGEvent and accessibility user path of the AppKit captures (WG3m).
//! - [`run_pin`]: drives one pin through a [`Driver`] (one per toolkit, in the tests of this crate,
//!   `examples/counter-qt/tests/na_capture.rs` and this crate's egui measurement).

pub mod golden;
#[cfg(target_os = "macos")]
pub mod macos;
pub mod pins;
pub mod scroll;
pub mod trace;
#[cfg(windows)]
pub mod win;
#[cfg(target_os = "linux")]
pub mod xtest;

use golden::PinResult;
use pins::{Action, Chord, Keys, Momentum, Pin};
use scroll::ScrollRecord;
use trace::Checkpoint;

/// One toolkit's fixture and input. The capture builds a fresh fixture per pin.
pub trait Driver {
    /// Builds the fixture window (fields `a`, `b`, button `c`) and applies the pin's setup.
    fn setup(&mut self, pin: &Pin) -> Result<(), String>;
    fn key(&mut self, chord: &Chord) -> Result<(), String>;
    /// `at`: a character offset in a field (`None` on the button: its centre). `button`: 1 primary,
    /// 2 middle.
    fn click(
        &mut self,
        target: &str,
        at: Option<usize>,
        count: u32,
        button: u32,
    ) -> Result<(), String>;
    /// Types `text` into the focused widget, one key per character.
    fn type_text(&mut self, text: &str) -> Result<(), String>;
    fn drag(&mut self, target: &str, from: usize, to: usize, count: u32) -> Result<(), String>;
    /// Reads every anchor's state (and the pin's `observe` pseudo-anchors) and writes the `state`
    /// lines into the trace.
    fn observe(&mut self) -> Result<Checkpoint, String>;
    /// Destroys the fixture and returns the pin's trace (op log with `state` lines).
    fn teardown(&mut self) -> Result<String, String>;
    /// WG3s: `burst` wheel events of `lines` notches (positive towards the content's end) over
    /// `target`, `interval_ms` apart; returns after the scroll settled, with the samples.
    fn scroll_wheel(
        &mut self,
        _target: &str,
        _lines: [i32; 2],
        _shift: bool,
        _burst: u32,
        _interval_ms: u32,
    ) -> Result<ScrollRecord, String> {
        Err(UNSUPPORTED.into())
    }
    /// WG3s: a trackpad scroll over `target` (began, changed…, ended), then `momentum`.
    fn scroll_gesture(
        &mut self,
        _target: &str,
        _deltas: &[[f64; 2]],
        _interval_ms: u32,
        _momentum: &Momentum,
    ) -> Result<ScrollRecord, String> {
        Err(UNSUPPORTED.into())
    }
}

/// What a driver without a scroll fixture answers to a scroll action: the step is recorded
/// without input (as an intent without a chord is).
pub const UNSUPPORTED: &str = "unsupported";

/// Runs one pin: setup, a checkpoint, then each action followed by a checkpoint.
pub fn run_pin(d: &mut dyn Driver, pin: &Pin, keys: &Keys) -> Result<(PinResult, String), String> {
    run_pin_scroll(d, pin, keys).map(|(r, t, _)| (r, t))
}

/// [`run_pin`], plus each scroll action's samples (`(label, record)`, for the trace directory).
/// A scroll action's checkpoint gets the `motion` pseudo-anchor ([`scroll::metrics`]).
#[allow(clippy::type_complexity)]
pub fn run_pin_scroll(
    d: &mut dyn Driver,
    pin: &Pin,
    keys: &Keys,
) -> Result<(PinResult, String, Vec<(String, ScrollRecord)>), String> {
    let ctx = |m: String| format!("pin {}: {m}", pin.id);
    d.setup(pin).map_err(ctx)?;
    let mut checkpoints = vec![("setup".to_string(), d.observe().map_err(ctx)?)];
    let mut records = Vec::new();
    for a in &pin.actions {
        // WG3s: a scroll action's record, the metrics' axis and its notch count
        let mut scrolled: Option<(ScrollRecord, String, usize, Option<u32>)> = None;
        let scroll_result = |r: Result<ScrollRecord, String>| match r {
            Ok(rec) => Ok(Some(rec)),
            Err(e) if e == UNSUPPORTED => Ok(None),
            Err(e) => Err(e),
        };
        match a {
            Action::Wheel {
                target,
                lines,
                shift,
                burst,
                interval_ms,
            } => {
                scroll_result(d.scroll_wheel(target, *lines, *shift, *burst, *interval_ms)).map(
                    |rec| {
                        // Shift turns a vertical wheel horizontal
                        let axis = if *shift || lines[1] == 0 { 0 } else { 1 };
                        let n = lines[0].unsigned_abs().max(lines[1].unsigned_abs()) * burst;
                        scrolled = rec.map(|r| (r, target.clone(), axis, Some(n)));
                    },
                )
            }
            Action::Gesture {
                target,
                deltas,
                interval_ms,
                momentum,
                ..
            } => {
                scroll_result(d.scroll_gesture(target, deltas, *interval_ms, momentum)).map(|rec| {
                    let (sx, sy) = deltas
                        .iter()
                        .fold((0.0, 0.0), |a, d| (a.0 + d[0].abs(), a.1 + d[1].abs()));
                    let axis = if sx > sy { 0 } else { 1 };
                    scrolled = rec.map(|r| (r, target.clone(), axis, None));
                })
            }
            _ => Ok(()),
        }
        .map_err(|m| ctx(format!("{}: {m}", a.label())))?;
        match a {
            // An intent the platform has no chord for (`unicode-entry` off Linux) is a step without input: its
            // checkpoint records that nothing happened (WG3w).
            Action::Key { intent, .. } if !keys.map.contains_key(intent) => Ok(()),
            Action::Key { intent, shift } => d.key(&keys.chord(intent, *shift).map_err(ctx)?),
            Action::Click {
                target,
                at,
                count,
                button,
            } => d.click(target, *at, *count, *button),
            Action::Type { text } => d.type_text(text),
            Action::Drag {
                target,
                from,
                to,
                count,
            } => d.drag(target, *from, *to, *count),
            Action::Wheel { .. } | Action::Gesture { .. } => Ok(()),
        }
        .map_err(|m| ctx(format!("{}: {m}", a.label())))?;
        let mut cp = d.observe().map_err(ctx)?;
        if let Some((rec, target, axis, notches)) = scrolled {
            cp.push((
                scroll::MOTION.to_string(),
                trace::State::motion(scroll::metrics(&rec, &target, axis, notches)),
            ));
            records.push((a.label(), rec));
        }
        checkpoints.push((a.label(), cp));
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
        records,
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
        // Pin 6 (IME) runs on the toolkits' built-in input methods, not an IM server: GTK's simple
        // context (dead keys, Ctrl+Shift+U), Qt's compose plugin (dead keys). Captures record both.
        .env("GTK_IM_MODULE", "gtk-im-context-simple")
        .env("QT_IM_MODULE", "compose")
        .env_remove("XMODIFIERS")
        .status()
        .expect("run tools/dev/headless");
    Some(status.success())
}

/// The trace handle of a selection pseudo-anchor (`clipboard`, `primary`): not a node, so a fixed
/// number no backend hands out.
pub fn selection_handle(anchor: &str) -> u64 {
    if anchor == pins::CLIPBOARD {
        u64::MAX - 1
    } else {
        u64::MAX - 2
    }
}

/// Start of a pin on an X11 toolkit: empty CLIPBOARD and PRIMARY, then the pin's clipboard seed.
#[cfg(target_os = "linux")]
pub fn prepare_selections(
    x: &mut xtest::XInput,
    pin: &Pin,
    pump: &mut dyn FnMut(u64),
) -> Result<(), String> {
    x.clear_selections(pump)?;
    if let Some(t) = &pin.setup.clipboard {
        x.seed_clipboard(t, pump)?;
    }
    Ok(())
}

/// Appends the pin's selection pseudo-anchors to a checkpoint (read from the X server).
#[cfg(target_os = "linux")]
pub fn observe_selections(
    x: &xtest::XInput,
    observe: &[String],
    pump: &mut dyn FnMut(u64),
    trace: &mut trace::Trace,
    cp: &mut Checkpoint,
) -> Result<(), String> {
    for name in observe {
        let sel = if name == pins::CLIPBOARD {
            "CLIPBOARD"
        } else {
            "PRIMARY"
        };
        let s = trace::State::selection(x.read_selection(sel, pump)?);
        trace.state(selection_handle(name), &s);
        cp.push((name.clone(), s));
    }
    Ok(())
}
