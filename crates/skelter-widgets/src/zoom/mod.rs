//! UI zoom, VS Code style (Ctrl+= / Ctrl+- / Ctrl+0; Cmd on macOS): svelters-egui `zoom.rs` (ZM1).
//!
//! The zoom multiplies a window's OS scale: device pixels per point = zoom × OS scale. Everything
//! above the renderer is in points and CSS px are points, so a zoom changes no style, layout input
//! or hit test; text re-rasterises at the new scale and measured leaves are remeasured (advances snap
//! to device pixels, so a size in points can move by a fraction). [`Zoom`] holds the factor and says
//! when the effective scale changed; the host applies [`Zoom::scale`] to its windows.
//!
//! Levels are the host's: cadatomic maps VS Code's `window.zoomLevel` n to `1.2^n` ([`level_factor`]).
//! Who handles the keys is [`ZoomKeys`]: the host (it owns the shortcuts) or skelter ([`Zoom::key`]).

use skelter_base::event::{KeyEvent, KeyState, Modifiers};
use skelter_base::geom::Scale;
use skelter_base::key::Key;

/// One zoom level is a factor of 1.2 (VS Code's `window.zoomLevel`).
pub const ZOOM_STEP: f32 = 1.2;
/// The levels the keys step through: `1.2^-8` (≈ 0.23) to `1.2^8` (≈ 4.30).
pub const MIN_LEVEL: i32 = -8;
pub const MAX_LEVEL: i32 = 8;
/// [`Zoom::set`] clamps to this range.
pub const MIN_ZOOM: f32 = 0.2;
pub const MAX_ZOOM: f32 = 5.0;

/// The factor of zoom level `level` (`1.2^level`, the level clamped to [`MIN_LEVEL`]..=[`MAX_LEVEL`]).
pub fn level_factor(level: i32) -> f32 {
    ZOOM_STEP.powi(level.clamp(MIN_LEVEL, MAX_LEVEL))
}

/// The level nearest to `factor`.
pub fn nearest_level(factor: f32) -> i32 {
    if factor.is_nan() || factor <= 0.0 {
        return 0;
    }
    ((factor.ln() / ZOOM_STEP.ln()).round() as i32).clamp(MIN_LEVEL, MAX_LEVEL)
}

/// The level `delta` levels up (`delta > 0`) or down from `factor`. A factor between two levels
/// steps to the neighbouring level in that direction first (1.1 → 1.2 up, 1.0 down).
pub fn step(factor: f32, delta: i32) -> f32 {
    if delta == 0 || factor.is_nan() || factor <= 0.0 {
        return factor;
    }
    let l = factor.ln() / ZOOM_STEP.ln();
    let level = if delta > 0 { (l + 1e-3).floor() as i32 + delta } else { (l - 1e-3).ceil() as i32 + delta };
    level_factor(level)
}

/// Who handles Ctrl+= / Ctrl+- / Ctrl+0 (Cmd on macOS).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum ZoomKeys {
    /// The host owns the shortcuts (cadatomic's keymap) and calls [`Zoom::set`]; the keys pass on.
    #[default]
    Host,
    /// skelter steps by [`ZOOM_STEP`] and consumes the keys ([`Zoom::key`]); the standalone runners.
    Skelter,
}

/// What a zoom shortcut asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZoomCommand {
    In,
    Out,
    Reset,
}

/// The zoom command of key event `ev`, if it is a zoom shortcut: primary modifier (Cmd on macOS,
/// Ctrl elsewhere) plus `=`/`+` (in), `-` (out) or `0` (reset), without Alt; Shift is ignored
/// (`+` is Shift+`=` on many layouts).
pub fn command(ev: &KeyEvent, macos: bool) -> Option<ZoomCommand> {
    if ev.state != KeyState::Pressed || !ev.modifiers.primary(macos) || ev.modifiers.contains(Modifiers::ALT) {
        return None;
    }
    // the other platform's primary modifier must not be held as well (Ctrl+Cmd+= is not zoom)
    let other = if macos { Modifiers::CTRL } else { Modifiers::SUPER };
    if ev.modifiers.contains(other) {
        return None;
    }
    match ev.key? {
        Key::Equals | Key::Plus => Some(ZoomCommand::In),
        Key::Minus => Some(ZoomCommand::Out),
        Key::Num0 => Some(ZoomCommand::Reset),
        _ => None,
    }
}

/// A window group's zoom factor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Zoom {
    factor: f32,
    pub keys: ZoomKeys,
}

impl Default for Zoom {
    fn default() -> Self {
        Zoom { factor: 1.0, keys: ZoomKeys::Host }
    }
}

impl Zoom {
    pub fn new(keys: ZoomKeys) -> Zoom {
        Zoom { factor: 1.0, keys }
    }

    pub fn factor(&self) -> f32 {
        self.factor
    }

    /// Sets the factor, clamped to [`MIN_ZOOM`]..=[`MAX_ZOOM`]; a non-finite or non-positive factor
    /// is ignored. Returns whether it changed (the host then rescales its windows and remeasures).
    pub fn set(&mut self, zoom: f32) -> bool {
        if !zoom.is_finite() || zoom <= 0.0 {
            return false;
        }
        let z = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        let changed = z != self.factor;
        self.factor = z;
        changed
    }

    /// Applies `cmd`: one level in or out, or back to 1. Returns whether the factor changed.
    pub fn apply(&mut self, cmd: ZoomCommand) -> bool {
        let z = match cmd {
            ZoomCommand::In => step(self.factor, 1),
            ZoomCommand::Out => step(self.factor, -1),
            ZoomCommand::Reset => 1.0,
        };
        self.set(z)
    }

    /// [`ZoomKeys::Skelter`]: handles a zoom shortcut. `Some(changed)` when `ev` was one (consumed),
    /// `None` when it was not or the host owns the keys.
    pub fn key(&mut self, ev: &KeyEvent, macos: bool) -> Option<bool> {
        if self.keys != ZoomKeys::Skelter {
            return None;
        }
        command(ev, macos).map(|c| self.apply(c))
    }

    /// The device pixels per point of a window whose OS scale is `os`.
    pub fn scale(&self, os: Scale) -> Scale {
        Scale(os.0 * self.factor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use skelter_base::event::KeyLocation;

    fn key(k: Key, m: Modifiers) -> KeyEvent {
        KeyEvent { key: Some(k), physical: 0, text: None, state: KeyState::Pressed, repeat: false, location: KeyLocation::Standard, modifiers: m }
    }

    #[test]
    fn levels_step_by_1_2() {
        assert_eq!(level_factor(0), 1.0);
        assert!((level_factor(1) - 1.2).abs() < 1e-6);
        assert!((level_factor(-2) - 1.0 / 1.44).abs() < 1e-6);
        assert_eq!(level_factor(100), level_factor(MAX_LEVEL));
        assert!((step(1.0, 1) - 1.2).abs() < 1e-6);
        assert!((step(1.2, 1) - 1.44).abs() < 1e-5);
        assert!((step(1.44, -1) - 1.2).abs() < 1e-5);
        assert!((step(1.2, -1) - 1.0).abs() < 1e-6);
        assert!((step(1.1, 1) - 1.2).abs() < 1e-6);
        assert!((step(1.1, -1) - 1.0).abs() < 1e-6);
        assert_eq!(step(level_factor(MAX_LEVEL), 1), level_factor(MAX_LEVEL));
        assert_eq!(nearest_level(1.44), 2);
        assert_eq!(nearest_level(1.0 / 1.2), -1);
    }

    #[test]
    fn shortcuts_follow_the_platform_modifier() {
        assert_eq!(command(&key(Key::Equals, Modifiers::CTRL), false), Some(ZoomCommand::In));
        assert_eq!(command(&key(Key::Plus, Modifiers::CTRL.union(Modifiers::SHIFT)), false), Some(ZoomCommand::In));
        assert_eq!(command(&key(Key::Minus, Modifiers::SUPER), true), Some(ZoomCommand::Out));
        assert_eq!(command(&key(Key::Num0, Modifiers::SUPER), true), Some(ZoomCommand::Reset));
        assert_eq!(command(&key(Key::Minus, Modifiers::CTRL), true), None, "Ctrl is not Cmd on macOS");
        assert_eq!(command(&key(Key::Minus, Modifiers::CTRL.union(Modifiers::ALT)), false), None);
        assert_eq!(command(&key(Key::A, Modifiers::CTRL), false), None);
    }

    #[test]
    fn zoom_state_clamps_and_scales() {
        let mut z = Zoom::new(ZoomKeys::Skelter);
        assert_eq!(z.key(&key(Key::Equals, Modifiers::CTRL), false), Some(true));
        assert!((z.factor() - 1.2).abs() < 1e-6);
        assert_eq!(z.scale(Scale(2.0)), Scale(2.0 * z.factor()));
        assert_eq!(z.key(&key(Key::Num0, Modifiers::CTRL), false), Some(true));
        assert_eq!(z.key(&key(Key::Num0, Modifiers::CTRL), false), Some(false), "already 1");
        assert!(!z.set(f32::NAN) && !z.set(-1.0));
        assert!(z.set(100.0));
        assert_eq!(z.factor(), MAX_ZOOM);
        let mut host = Zoom::default();
        assert_eq!(host.key(&key(Key::Equals, Modifiers::CTRL), false), None, "the host owns the keys");
    }
}
