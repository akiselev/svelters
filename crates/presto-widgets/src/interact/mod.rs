//! Behaviour units (ARCH-WIDGETS §1.2): the "how it behaves" layer of presto's built-in kinds.
//!
//! A unit is a plain struct: no tree, no JS. It reads an [`InputCx`] (the host's [`Resp`] for this
//! pass, the pass's input, the platform [`Policy`], the time), updates its own fields, sets the state
//! bits it owns ([`Unit::OWNS`]), and pushes typed intents that the kind turns into prop writes and
//! signals. It contributes to the host's AccessKit node ([`Unit::a11y`]) and claims keys for EV3
//! ([`Unit::claims`]).
//!
//! The units are ported from svelters-egui `interact/` and keep egui's click semantics on purpose
//! (architecture §4.2: about 1,000 kittest assertions depend on them). What egui's `Ui::interact` did
//! (click and drag decisions, double clicks, Space/Enter on the focused host, AccessKit `Click`) is
//! [`Tracker`] here: it turns the presto events a node receives into one [`Resp`] and one
//! [`InputFrame`] per event, so a unit's `input` reads exactly what it read under egui.
//!
//! A kind runs its units in [`Kind::event`](crate::shim::core::Kind::event):
//!
//! ```ignore
//! let pass = self.tracker.pass(cx, ev);
//! let mut fired = Intents::default();
//! interact::run(cx, &pass, |icx| self.press.input(&params, icx, &mut fired));
//! ```
//!
//! Disabled hosts: units other than [`Hoverable`] produce no intents (R9: disabled controls stay
//! hoverable).
//!
//! Not here: `Scrollable` (lane WIDGETS-VIEWS, with scroll physics), `DragSource`/`DropTarget` (lane
//! WIDGETS-TIER1).

pub mod policy;
mod press;
mod state;
mod tracker;

mod focus;
mod hover;
mod toggle;

pub use focus::{Focus, FocusParams, Focusable};
pub use hover::{Hover, Hoverable};
pub use policy::{ButtonOrder, Kinetic, Native, Platform, Policy, ScrollPolicy, Scrollbars, TrackClick};
pub use press::{ACCEL_EVERY, ActivateOn, Press, PressParams, Pressable};
pub use state::StateBits;
pub use toggle::{Toggle, ToggleParams, Toggleable};
pub use tracker::{ClickOptions, FrameEvent, InputFrame, Mods, Pass, PointerFrame, Sense, Tracker};

use crate::shim::core::EventCx;
use presto_base::accesskit;
use presto_base::event::MouseButton;
use presto_base::geom::{Point, Rect, Vec2};
use presto_base::key::Key;

/// A behaviour unit (ARCH-WIDGETS §1.2).
pub trait Unit: Default + 'static {
    /// What the kind (or the `use:` params) configures.
    type Params: Default;
    /// What happened this pass, typed.
    type Intent;
    /// The state bits this unit writes; cleared when it is detached, disabled or inert.
    const OWNS: StateBits;
    /// What the host's [`Tracker`] must sense for this unit (merged over all units).
    fn sense(&self, _p: &Self::Params) -> Sense {
        Sense::HOVER
    }
    /// Reads this pass's input, updates its state and bits, pushes intents.
    fn input(&mut self, p: &Self::Params, cx: &mut InputCx<'_>, out: &mut Intents<Self::Intent>);
    /// Its part of the host's AccessKit node (properties, states, supported actions).
    fn a11y(&self, _p: &Self::Params, _node: &mut accesskit::Node) {}
    /// EV3 R11: while the host is focused, whether this unit takes `key` with `m` itself.
    fn claims(&self, _p: &Self::Params, _key: Key, _m: Mods) -> bool {
        false
    }
    /// When it needs another pass without input (key repeat, momentum), in seconds from now.
    fn next_frame(&self) -> Option<f64> {
        None
    }
}

/// The intents of one unit in one pass, in order.
#[derive(Debug)]
pub struct Intents<T>(pub Vec<T>);

impl<T> Default for Intents<T> {
    fn default() -> Self {
        Intents(Vec::new())
    }
}

impl<T> Intents<T> {
    pub fn push(&mut self, t: T) {
        self.0.push(t);
    }
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.0.iter()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl<T> IntoIterator for Intents<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a, T> IntoIterator for &'a Intents<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

/// Why the host gained focus (`focus-visible` and selection-on-focus rules depend on it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusReason {
    /// A pointer press (click-to-focus): never `:focus-visible` (FX1, GTK).
    Pointer,
    /// Tab, Shift+Tab, arrows in a roving group.
    Keyboard,
    /// The app (`focus()`, `autofocus`, a dialog's restore) or assistive technology.
    Program,
}

/// Focus as units see it, and the focus changes they ask for (applied by [`run`] after the units ran).
#[derive(Clone, Copy, Debug, Default)]
pub struct FocusCx {
    /// The host has keyboard focus this pass.
    pub has: bool,
    /// This pass's input held a pointer press (anywhere the host saw): a focus gained now came from it.
    pub pointer_pressed: bool,
    /// This pass's input held a key press.
    pub key_pressed: bool,
    request: bool,
    surrender: bool,
}

impl FocusCx {
    /// Asks for keyboard focus (applied after the units ran).
    pub fn request(&mut self) {
        self.request = true;
        self.surrender = false;
    }
    /// Gives keyboard focus up (applied after the units ran).
    pub fn surrender(&mut self) {
        self.surrender = true;
        self.request = false;
    }
    /// Whether a unit asked for focus this pass.
    pub fn requested(&self) -> bool {
        self.request
    }
    /// Whether a unit gave focus up this pass.
    pub fn surrendered(&self) -> bool {
        self.surrender
    }
}

/// What units read of the host's interaction this pass: the fields of egui's `Response` the units
/// used, computed by [`Tracker`] with egui's rules. Positions are host-local points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Resp {
    /// The host's border box (local: origin at 0,0).
    pub rect: Rect,
    /// Hovered and not covered (false on a disabled host, and while another node holds a press).
    pub hovered: bool,
    /// The pointer is over the rect, covered or disabled or not.
    pub contains_pointer: bool,
    /// A pointer button is held down on it (egui's `is_pointer_button_down_on`).
    pub down_on: bool,
    /// Primary click, including Space/Enter on the focused host and AccessKit `Click` (egui).
    pub clicked: bool,
    pub double_clicked: bool,
    pub triple_clicked: bool,
    pub secondary_clicked: bool,
    pub middle_clicked: bool,
    pub drag_started: bool,
    pub dragged: bool,
    pub drag_stopped: bool,
    pub drag_delta: Vec2,
    pub interact_pointer_pos: Option<Point>,
    pub has_focus: bool,
    pub enabled: bool,
}

impl Default for Resp {
    fn default() -> Self {
        Resp {
            rect: Rect::ZERO,
            hovered: false,
            contains_pointer: false,
            down_on: false,
            clicked: false,
            double_clicked: false,
            triple_clicked: false,
            secondary_clicked: false,
            middle_clicked: false,
            drag_started: false,
            dragged: false,
            drag_stopped: false,
            drag_delta: Vec2::ZERO,
            interact_pointer_pos: None,
            has_focus: false,
            enabled: true,
        }
    }
}

impl Resp {
    /// Whether `button` clicked the host this pass.
    pub fn clicked_by(&self, button: MouseButton) -> bool {
        match button {
            MouseButton::Primary => self.clicked,
            MouseButton::Secondary => self.secondary_clicked,
            MouseButton::Middle => self.middle_clicked,
            _ => false,
        }
    }
}

/// The responses of a host's node parts (a slider's thumb, spin buttons), by part name.
#[derive(Default)]
pub struct PartResponses(pub Vec<(&'static str, Resp)>);

impl PartResponses {
    pub fn get(&self, name: &str) -> Option<&Resp> {
        self.0.iter().find(|(n, _)| *n == name).map(|(_, r)| r)
    }
}

/// What a unit reads in [`Unit::input`].
pub struct InputCx<'a> {
    /// The host's (or the part's) interaction this pass.
    pub resp: &'a Resp,
    /// The input of this pass (keys only reach the focused node).
    pub input: &'a InputFrame,
    pub policy: &'a Policy,
    /// The input time in seconds (the virtual clock in tests).
    pub now: f64,
    /// The host's state bits, written in place before recording.
    pub state: &'a mut StateBits,
    /// The host takes input (`enabled` and not inert).
    pub enabled: bool,
    /// AccessKit action requests routed to the host this pass.
    pub a11y: &'a [accesskit::ActionRequest],
    pub focus: &'a mut FocusCx,
    pub parts: &'a PartResponses,
}

impl InputCx<'_> {
    /// Sets or clears `bits` on the host.
    pub fn set(&mut self, bits: StateBits, on: bool) {
        self.state.set(bits, on);
    }
    /// Whether an AccessKit action `a` was requested for the host this pass.
    pub fn a11y_requested(&self, a: accesskit::Action) -> bool {
        self.a11y.iter().any(|r| r.action == a)
    }
}

/// Runs units on one pass of a host: builds the [`InputCx`] over `cx` (state bits, enabled, policy),
/// calls `f`, then applies the focus the units asked for to `cx` (and the policy's Escape rule).
/// Returns the focus as the units left it.
pub fn run(cx: &mut EventCx<'_>, pass: &Pass, f: impl FnOnce(&mut InputCx<'_>)) -> FocusCx {
    run_parts(cx, pass, &PartResponses::default(), f)
}

/// [`run`] with the responses of the host's parts.
pub fn run_parts(cx: &mut EventCx<'_>, pass: &Pass, parts: &PartResponses, f: impl FnOnce(&mut InputCx<'_>)) -> FocusCx {
    let mut focus = FocusCx { has: pass.resp.has_focus, pointer_pressed: pass.input.pointer.any_pressed(), key_pressed: pass.input.any_key_pressed(), ..FocusCx::default() };
    let enabled = cx.enabled;
    let policy = cx.policy;
    let mut icx = InputCx { resp: &pass.resp, input: &pass.input, policy, now: pass.input.time, state: &mut *cx.state, enabled, a11y: &pass.a11y, focus: &mut focus, parts };
    f(&mut icx);
    if focus.request && !focus.has {
        cx.focus_request = Some(true);
    } else if focus.surrender && focus.has {
        cx.focus_request = Some(false);
    } else if focus.has && !policy.escape_keeps_focus && pass.input.key_pressed(Key::Escape) {
        // egui drops focus on Escape; the GTK, Qt, Windows and macOS rows keep it (FX1)
        cx.focus_request = Some(false);
    }
    focus
}
