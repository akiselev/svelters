//! OW0 (ARCH-WIDGETS §1.2): behaviour units, the "how it behaves" layer of svelters' own widgets.
//!
//! A unit is a plain struct: no scene, no JS. It reads an [`InputCx`] (the response of
//! `ui.interact` at the host's rect, the raw input, the platform [`Policy`](crate::native::Policy),
//! the time), updates its own fields, sets the state bits it owns ([`Unit::OWNS`]), and pushes
//! typed intents that the widget turns into prop writes and events. It contributes to the
//! host's AccessKit node ([`Unit::a11y`]) and claims keys for EV3 ([`Unit::claims`]).
//!
//! egui's `Ui::interact` stays the substrate (picking against last pass's rects, drag capture,
//! click timing, focus memory, AccessKit action routing; the user's Q2); every semantic is ours.
//!
//! Three ways to run units, one implementation:
//! - **in a kind**: `#[behavior] press: Pressable` fields of a `#[svelters::widget]`; the macro
//!   generates `self.run_units(cx, &resp)`, which runs them in declaration order through
//!   [`WidgetCx::run_units`](crate::provider::WidgetCx::run_units) and returns their intents;
//! - **without a scene node** (OW-RUST, a foreign painter such as cadatomic's `ChromePainter`):
//!   [`run`] with an `egui::Ui`, a response and the caller's own [`StateBits`];
//! - **on any element** as a `use:` action: [`UseUnit`] (OW0b exports the three units from
//!   `svelters:behaviors/sv`).
//!
//! Disabled hosts: units other than [`Hoverable`] produce no intents (R9: disabled controls stay
//! hoverable). Writer lane: OW.

mod focus;
mod hover;
mod press;
mod resize;
pub mod scroll;
mod toggle;

pub use focus::{Focus, FocusParams, Focusable};
pub use hover::{Hover, Hoverable};
pub use press::{Press, PressParams, Pressable};
pub use resize::{Resizable, Resize, ResizeParams};
pub use scroll::{BarGeom, Scroll, ScrollBy, ScrollParams, Scrollable};
pub use toggle::{Toggle, ToggleParams, Toggleable};

use crate::native::Policy;
use crate::provider::StateBits;
use egui::accesskit;

/// A behaviour unit (ARCH-WIDGETS §1.2).
pub trait Unit: Default + 'static {
    /// What the kind (or the `use:` params) configures.
    type Params: Default;
    /// What happened this pass, typed.
    type Intent;
    /// The state bits this unit writes; cleared when it is detached, disabled or inert.
    const OWNS: StateBits;
    /// What the host's `ui.interact` must sense for this unit (merged over all units).
    fn sense(&self, _p: &Self::Params) -> egui::Sense {
        egui::Sense::hover()
    }
    /// Reads this pass's input, updates its state and bits, pushes intents.
    fn input(&mut self, p: &Self::Params, cx: &mut InputCx<'_>, out: &mut Intents<Self::Intent>);
    /// Its part of the host's AccessKit node (properties, states, supported actions).
    fn a11y(&self, _p: &Self::Params, _node: &mut accesskit::Node) {}
    /// EV3 R11: while the host is focused, whether this unit takes `key` with `m` itself.
    fn claims(&self, _p: &Self::Params, _key: egui::Key, _m: egui::Modifiers) -> bool {
        false
    }
    /// When it needs another frame without input (key repeat, momentum, caret blink).
    fn next_frame(&self) -> Option<std::time::Duration> {
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
    /// The app (`request-focus`, `autofocus`, a dialog's restore) or assistive technology.
    Program,
}

/// Focus as units see it, and the focus changes they ask for (applied after the units ran, outside
/// egui's input lock).
#[derive(Clone, Copy, Debug, Default)]
pub struct FocusCx {
    /// The host has keyboard focus this pass.
    pub has: bool,
    /// This pass's input held a pointer press (anywhere): a focus gained now came from it.
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
}

/// What units read of an `egui::Response`, taken before they run. `Response` methods lock egui's
/// context, which is held (for the raw input) while units run, so units never see a `Response`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Resp {
    pub id: egui::Id,
    pub rect: egui::Rect,
    /// Hovered and not covered (egui's `hovered`; false on a disabled host).
    pub hovered: bool,
    /// The pointer is over the rect, covered or disabled or not.
    pub contains_pointer: bool,
    /// A pointer button is held down on it (`is_pointer_button_down_on`).
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
    pub drag_delta: egui::Vec2,
    pub interact_pointer_pos: Option<egui::Pos2>,
    pub has_focus: bool,
    pub enabled: bool,
}

impl Resp {
    pub fn of(r: &egui::Response) -> Resp {
        Resp {
            id: r.id,
            rect: r.rect,
            hovered: r.hovered(),
            contains_pointer: r.contains_pointer(),
            down_on: r.is_pointer_button_down_on(),
            clicked: r.clicked(),
            double_clicked: r.double_clicked(),
            triple_clicked: r.triple_clicked(),
            secondary_clicked: r.secondary_clicked(),
            middle_clicked: r.middle_clicked(),
            drag_started: r.drag_started(),
            dragged: r.dragged(),
            drag_stopped: r.drag_stopped(),
            drag_delta: r.drag_delta(),
            interact_pointer_pos: r.interact_pointer_pos(),
            has_focus: r.has_focus(),
            enabled: r.enabled(),
        }
    }
}

/// The responses of a host's node parts (a slider's thumb, spin buttons), by part name. OW0a
/// widgets have none; OW1b and later fill it.
#[derive(Default)]
pub struct PartResponses(pub Vec<(&'static str, Resp)>);

impl PartResponses {
    pub fn get(&self, name: &str) -> Option<&Resp> {
        self.0.iter().find(|(n, _)| *n == name).map(|(_, r)| r)
    }
}

/// What a unit reads in [`Unit::input`].
pub struct InputCx<'a> {
    /// `ui.interact(rect, id, merged sense)` of the host (or the part the unit sits on), as a
    /// snapshot ([`Resp`]).
    pub resp: &'a Resp,
    /// The raw input of this pass (keys only reach the focused node: EV3 filters).
    pub input: &'a egui::InputState,
    /// The platform row (GTK, Qt; Windows and Mac in OW0b).
    pub policy: &'a Policy,
    /// egui's input time in seconds (kittest's virtual clock).
    pub now: f64,
    /// The host's state bits, written in place before paint.
    pub state: &'a mut StateBits,
    /// The host takes input (`enabled` and not inert).
    pub enabled: bool,
    /// AccessKit action requests routed to the host's id this pass.
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

/// Runs units on a host that is not (or need not be) a scene node: `resp` is the host's
/// `ui.interact`, `state` its bits (the caller keeps them between frames), `f` calls
/// [`Unit::input`] of each unit with the [`InputCx`] it gets. Focus requests are applied after
/// `f` (and `escape_keeps_focus` per policy). Returns the focus as the units left it.
///
/// This is all a foreign painter needs (OW-RUST, berth's `ChromePainter`): no scene, no JS.
pub fn run(ui: &egui::Ui, resp: &egui::Response, state: &mut StateBits, parts: &PartResponses, f: impl FnOnce(&mut InputCx<'_>)) -> FocusCx {
    let ctx = ui.ctx();
    let policy = &crate::native::policy_of(ctx);
    let enabled = ui.is_enabled();
    let mut snap = Resp::of(resp);
    if !enabled {
        // egui senses nothing on a disabled widget; R9 keeps it hoverable (tooltip, reason)
        snap.contains_pointer |= ui.rect_contains_pointer(resp.rect);
    }
    let mut focus = FocusCx { has: snap.has_focus, ..FocusCx::default() };
    ctx.input(|inp| {
        focus.pointer_pressed = inp.pointer.any_pressed();
        focus.key_pressed = inp.events.iter().any(|e| matches!(e, egui::Event::Key { pressed: true, .. }));
        let target = resp.id.accesskit_id();
        let a11y: Vec<accesskit::ActionRequest> = inp
            .events
            .iter()
            .filter_map(|e| match e {
                egui::Event::AccessKitActionRequest(r) if r.target_node == target => Some(r.clone()),
                _ => None,
            })
            .collect();
        let mut cx = InputCx { resp: &snap, input: inp, policy, now: inp.time, state, enabled, a11y: &a11y, focus: &mut focus, parts };
        f(&mut cx);
    });
    if focus.request && !focus.has {
        resp.request_focus();
    } else if focus.surrender && focus.has {
        resp.surrender_focus();
    }
    crate::native::keep_focus_on_escape(ui, resp.id);
    focus
}

/// The `use:` adapter (ARCH-WIDGETS §1.2): one unit on any element, its intents delivered as the
/// action's `on<intent>` callbacks. OW0a has the shape; OW0b registers `use:hoverable`,
/// `use:pressable` and `use:focusable` in `svelters:behaviors/sv` through EV2b's attached kinds.
#[derive(Default)]
pub struct UseUnit<U: Unit> {
    pub unit: U,
    pub params: U::Params,
    pub state: StateBits,
}

impl<U: Unit> UseUnit<U> {
    /// Runs the unit on the host's response; returns its intents.
    pub fn input(&mut self, ui: &egui::Ui, resp: &egui::Response) -> Intents<U::Intent> {
        let mut out = Intents::default();
        let (unit, params) = (&mut self.unit, &self.params);
        run(ui, resp, &mut self.state, &PartResponses::default(), |cx| unit.input(params, cx, &mut out));
        out
    }
}
