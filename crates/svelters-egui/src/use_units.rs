//! OW1a4 (ARCH-WIDGETS §1.2): the behaviour units as `use:` actions on any element, exported by
//! `svelters:behaviors/sv`. Each one is an EV2b behaviour (an attached kind) that runs its unit
//! through [`UseUnit::run_on`] right after the host is drawn, every frame, in Rust:
//!
//! ```svelte
//! <script>import { pressable, toggleable } from 'svelters:behaviors/sv';</script>
//! <box use:pressable={{ onpress: () => n++ }} use:toggleable={{ checked, ontoggle: (c) => checked = c }}>…</box>
//! ```
//!
//! - The params are the unit's `Params` (camelCase names); the intents are `on<event>` callbacks.
//! - The host gets the unit's state bits ([`Unit::OWNS`]: `:hover`, `:active`, `:focus-visible`,
//!   `:checked`, `:expanded`, `:dragging`), so the host's CSS styles it as the own widgets. They
//!   are written after the host drew, so they show from the next frame (a bit change repaints).
//! - User-visible intents (a press, a toggle, an expand, a step, a scrub commit) go through EV1's
//!   [`BehaviorCx::dispatch`]: a Rust filter can prevent them. Value units (`toggleable`,
//!   `expandable`, `steppable`, `dragToValue`, `resizable`, `scrollable`) are uncontrolled: they
//!   apply the change to their own param unless prevented, and the app's param update
//!   (`checked = c`) is then a no-op.
//! - The unit's AccessKit properties go on the host's node (`toggled`, `expanded`, actions).
//! - When the behaviour leaves, the bits it set are cleared.
//!
//! Drag and drop is not here: WG4b's `drag-type`/`drag-data`/`drop-types` props already run the
//! `DragSource` and `DropTarget` units on every element (`kinds/dnd.rs`), with `:dragging` and
//! `:drop-target`. A host that is an own widget already composing the same unit handles input
//! twice; the action is for plain elements (`box`, `label`, `icon`, `image`).
//!
//! Writer lane: OW1a4.

use crate::interact::anchor::{self, Placement, Side};
use crate::interact::roving_list::Orientation;
use crate::interact::{
    ActivateOn, AnchorParams, Anchored, Axis, Dismiss, DismissParams, Dismissable, Drag, DragParams, DragToValue, Draggable, Expand, ExpandParams, Expandable, Focus, FocusParams, FocusReason, Focusable, Hover, Hoverable, Move, Press, PressParams,
    Pressable, Resizable, Resize, ResizeParams, Roving, RovingList, RovingListParams, RovingParams, Scroll, ScrollParams, Scrollable, Scrub, ScrubMode, ScrubParams, Search, StepParams, Steppable, Tip, Toggle, ToggleParams, Toggleable,
    TooltipParams, Tooltipable, Typeahead, TypeaheadParams, Unit, UseUnit, Wheel,
};
use crate::provider::{Behavior, BehaviorCx, Event, StateBits};
use crate::scene::{NodeId, NodeMut, NodeRef};

/// Writes the unit's own bits from its run onto the host.
fn sync<U: Unit>(u: &UseUnit<U>, cx: &mut BehaviorCx<'_>) {
    let own = U::OWNS.0;
    cx.set_host_state(StateBits(own & u.state.0), true);
    cx.set_host_state(StateBits(own & !u.state.0), false);
}

/// Runs `u` on the host's response and writes its bits back.
fn run<U: Unit>(u: &mut UseUnit<U>, resp: &egui::Response, cx: &mut BehaviorCx<'_>) -> Vec<U::Intent> {
    let host = cx.host_bits();
    let fired = u.run_on(resp, host);
    sync(u, cx);
    if let Some(d) = u.unit.next_frame() {
        resp.ctx.request_repaint_after(d);
    }
    fired.0
}

/// The keys of a key-driven unit reach only a focused host: such units also make it focusable.
fn focusable(s: egui::Sense) -> egui::Sense {
    s | egui::Sense::focusable_noninteractive()
}

/// `use:pressable`'s `press` payload.
#[derive(svelters::Record, Clone, Debug, Default, PartialEq)]
pub struct UsePress {
    /// 1 for a click, 2 for a double click.
    pub count: f64,
    /// `primary`, `secondary` or `middle`.
    pub button: String,
}

/// `use:toggleable`'s `toggle` payload.
#[derive(svelters::Record, Clone, Debug, Default, PartialEq)]
pub struct UseToggle {
    pub checked: bool,
    pub indeterminate: bool,
}

/// `use:anchored`'s `placed` payload: the side and the surface's rect, in points.
#[derive(svelters::Record, Clone, Debug, Default, PartialEq)]
pub struct UsePlaced {
    /// `top`, `bottom`, `left` or `right`.
    pub side: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// `use:dismissable`'s `dismiss` payload.
#[derive(svelters::Record, Clone, Debug, Default, PartialEq)]
pub struct UseDismiss {
    /// `escape` or `outside-press`.
    pub reason: String,
    /// The press point (`null` for Escape).
    pub x: Option<f64>,
    pub y: Option<f64>,
}

/// `use:draggable`'s `drag` payload.
#[derive(svelters::Record, Clone, Debug, Default, PartialEq)]
pub struct UseDrag {
    /// `start`, `move`, `end` or `cancel`.
    pub phase: String,
    /// The total movement since the start, in points.
    pub dx: f64,
    pub dy: f64,
}

/// `use:scrollable`'s `scroll` payload: the new offset, in points.
#[derive(svelters::Record, Clone, Debug, Default, PartialEq)]
pub struct UseScroll {
    pub x: f64,
    pub y: f64,
}

fn button_name(b: egui::PointerButton) -> &'static str {
    match b {
        egui::PointerButton::Primary => "primary",
        egui::PointerButton::Secondary => "secondary",
        egui::PointerButton::Middle => "middle",
        egui::PointerButton::Extra1 => "back",
        egui::PointerButton::Extra2 => "forward",
    }
}

// ---- hover, press, focus ----

/// `use:hoverable`: the host gets `:hover` (also while disabled, DESIGN R9) and reports entering
/// and leaving.
#[svelters::behavior(namespace = "sv", name = "hoverable")]
pub struct UseHoverable {
    /// The pointer entered (`true`) or left (`false`) the host.
    #[event]
    hover: Event<bool>,
    #[state]
    u: UseUnit<Hoverable>,
}

impl Behavior for UseHoverable {
    fn after_ui(&mut self, _host: NodeRef<'_>, resp: &egui::Response, cx: &mut BehaviorCx<'_>) {
        for h in run(&mut self.u, resp, cx) {
            cx.emit(&self.hover, matches!(h, Hover::Enter));
        }
    }
    fn detached(&mut self, host: &mut NodeMut<'_>) {
        host.set_state(Hoverable::OWNS, false);
    }
}

/// `use:pressable`: the host gets `:active` while pressed and reports presses: `press({ count,
/// button })` for a click (pointer, Space/Enter on focus, AccessKit `Click`), `down`, `up(inside)`,
/// `cancel`, and `repeat` while held when `autoRepeat` is set.
#[svelters::behavior(namespace = "sv", name = "pressable")]
pub struct UsePressable {
    /// `press` fires on the press instead of the release.
    #[prop(rename = "activateOn", enumeration = "release|press")]
    activate_on: String,
    /// Fires `repeat` while held (0.5 s, then every 0.1 s, accelerating).
    #[prop(rename = "autoRepeat")]
    auto_repeat: bool,
    /// A click: `{ count, button }`.
    #[event]
    press: Event<UsePress>,
    /// A button went down on the host.
    #[event]
    down: Event<()>,
    /// The button went up: `inside` the host or not.
    #[event]
    up: Event<bool>,
    /// The press was cancelled (Escape, focus loss).
    #[event]
    cancel: Event<()>,
    /// Auto-repeat while held (`autoRepeat`).
    #[event]
    repeat: Event<()>,
    #[state]
    u: UseUnit<Pressable>,
}

impl Behavior for UsePressable {
    fn sense(&self) -> egui::Sense {
        self.u.sense()
    }
    fn after_ui(&mut self, _host: NodeRef<'_>, resp: &egui::Response, cx: &mut BehaviorCx<'_>) {
        self.u.params = PressParams {
            activate_on: if self.activate_on == "press" { ActivateOn::Press } else { ActivateOn::Release },
            repeat: self.auto_repeat.then_some((0.5, 0.1)),
            accelerate: self.auto_repeat,
        };
        for p in run(&mut self.u, resp, cx) {
            match p {
                Press::Click { count, button, .. } => {
                    cx.dispatch(&self.press, UsePress { count: count as f64, button: button_name(button).into() });
                }
                Press::Down => cx.emit(&self.down, ()),
                Press::Up { inside } => cx.emit(&self.up, inside),
                Press::Cancel => cx.emit(&self.cancel, ()),
                Press::Repeat => {
                    cx.dispatch(&self.repeat, ());
                }
            }
        }
    }
    fn detached(&mut self, host: &mut NodeMut<'_>) {
        host.set_state(Pressable::OWNS, false);
    }
}

/// `use:focusable`: makes the host a focus stop (Tab, click-to-focus per platform) with `:focus`
/// and `:focus-visible` (keyboard or program focus only, never a pointer press).
#[svelters::behavior(namespace = "sv", name = "focusable")]
pub struct UseFocusable {
    /// In the Tab order.
    #[prop(rename = "tabStop", default = true)]
    tab_stop: bool,
    /// A press focuses it (`null`: the platform's rule).
    #[prop(rename = "focusOnClick")]
    focus_on_click: Option<bool>,
    /// Focus came: `keyboard`, `pointer` or `program`.
    #[event]
    focus: Event<String>,
    /// Focus left.
    #[event]
    blur: Event<()>,
    #[state]
    u: UseUnit<Focusable>,
}

impl UseFocusable {
    fn params(&self) -> FocusParams {
        FocusParams { tab_stop: self.tab_stop, focus_on_click: self.focus_on_click }
    }
}

impl Behavior for UseFocusable {
    fn sense(&self) -> egui::Sense {
        self.u.unit.sense(&self.params())
    }
    fn after_ui(&mut self, _host: NodeRef<'_>, resp: &egui::Response, cx: &mut BehaviorCx<'_>) {
        self.u.params = self.params();
        for f in run(&mut self.u, resp, cx) {
            match f {
                Focus::Gained(r) => cx.emit(
                    &self.focus,
                    match r {
                        FocusReason::Keyboard => "keyboard",
                        FocusReason::Pointer => "pointer",
                        FocusReason::Program => "program",
                    }
                    .to_string(),
                ),
                Focus::Lost => cx.emit(&self.blur, ()),
            }
        }
    }
    fn detached(&mut self, host: &mut NodeMut<'_>) {
        host.set_state(Focusable::OWNS, false);
    }
}

// ---- toggle, expand ----

/// `use:toggleable`: a click (or Space/Enter, AccessKit `Click`) toggles; the host gets `:checked`
/// and `:indeterminate`, and AccessKit `toggled`.
#[svelters::behavior(namespace = "sv", name = "toggleable")]
pub struct UseToggleable {
    #[prop]
    checked: bool,
    #[prop]
    indeterminate: bool,
    /// Cycles unchecked, checked, indeterminate.
    #[prop]
    tristate: bool,
    /// A checked host stays checked on a click (a radio item).
    #[prop]
    radio: bool,
    /// The user toggled it: `{ checked, indeterminate }`.
    #[event]
    toggle: Event<UseToggle>,
    #[state]
    u: UseUnit<Toggleable>,
}

impl Behavior for UseToggleable {
    fn sense(&self) -> egui::Sense {
        self.u.sense()
    }
    fn after_ui(&mut self, _host: NodeRef<'_>, resp: &egui::Response, cx: &mut BehaviorCx<'_>) {
        self.u.params = ToggleParams { checked: self.checked, indeterminate: self.indeterminate, tristate: self.tristate, radio: self.radio };
        for Toggle::Toggled { checked, indeterminate } in run(&mut self.u, resp, cx) {
            if !cx.dispatch(&self.toggle, UseToggle { checked, indeterminate }).prevented() {
                (self.checked, self.indeterminate) = (checked, indeterminate);
            } else {
                // prevented: the bits go back to the params
                cx.set_host_state(StateBits::CHECKED, self.checked && !self.indeterminate);
                cx.set_host_state(StateBits::INDETERMINATE, self.indeterminate);
            }
        }
    }
    fn detached(&mut self, host: &mut NodeMut<'_>) {
        host.set_state(Toggleable::OWNS, false);
    }
}

/// `use:expandable`: Right/Left on the focused host, AccessKit `Expand`/`Collapse` and (with
/// `byClick`) a click open and close it; the host gets `:expanded` and AccessKit `expanded`.
#[svelters::behavior(namespace = "sv", name = "expandable")]
pub struct UseExpandable {
    #[prop]
    expanded: bool,
    /// A click toggles it.
    #[prop(rename = "byClick", default = true)]
    by_click: bool,
    /// The user opened (`true`) or closed it.
    #[event]
    expand: Event<bool>,
    #[state]
    u: UseUnit<Expandable>,
}

impl Behavior for UseExpandable {
    fn sense(&self) -> egui::Sense {
        focusable(self.u.sense())
    }
    fn after_ui(&mut self, _host: NodeRef<'_>, resp: &egui::Response, cx: &mut BehaviorCx<'_>) {
        self.u.params = ExpandParams { expanded: self.expanded, on_click: self.by_click };
        for Expand::Set(v) in run(&mut self.u, resp, cx) {
            if !cx.dispatch(&self.expand, v).prevented() {
                self.expanded = v;
            } else {
                cx.set_host_state(StateBits::EXPANDED, self.expanded);
            }
        }
    }
    fn detached(&mut self, host: &mut NodeMut<'_>) {
        host.set_state(Expandable::OWNS, false);
    }
}

// ---- groups ----

/// `use:roving`: the host's siblings that carry `use:roving` too form one Tab stop; the arrows,
/// Home and End move focus among the enabled ones (wrapping). Reports the move as `rove(to)`, the
/// member index it went to.
#[svelters::behavior(namespace = "sv", name = "roving")]
pub struct UseRoving {
    /// Takes part in the group.
    #[prop(default = true)]
    on: bool,
    /// Focus moved to member `to` (an index among the group's members).
    #[event]
    rove: Event<f64>,
    #[state]
    u: UseUnit<Roving>,
    /// The host had focus at the end of the last frame: a member focused this frame by a move
    /// does not take the same key again.
    #[state]
    had_focus: bool,
}

/// The enabled members of `host`'s roving group, in order.
fn roving_members(host: NodeRef<'_>, me: NodeId) -> Vec<NodeId> {
    let Some(parent) = host.parent() else { return vec![me] };
    parent
        .children()
        .filter(|n| n.id() == me || (!n.state().contains(StateBits::DISABLED) && n.attached().any(|b| b.tag() == "sv.roving" && b.widget(|r: &UseRoving| r.on).unwrap_or(false))))
        .map(|n| n.id())
        .collect()
}

impl Behavior for UseRoving {
    fn sense(&self) -> egui::Sense {
        egui::Sense::focusable_noninteractive()
    }
    fn after_ui(&mut self, host: NodeRef<'_>, resp: &egui::Response, cx: &mut BehaviorCx<'_>) {
        self.u.params = RovingParams { on: self.on };
        let fired = run(&mut self.u, resp, cx);
        let had = std::mem::replace(&mut self.had_focus, resp.has_focus());
        let Some(r) = fired.last().copied().filter(|_| had) else { return };
        let me = host.id();
        let members = roving_members(host, me);
        let at = members.iter().position(|&m| m == me).unwrap_or(0);
        let to = r.target(at, members.len());
        if to != at {
            cx.focus_node(members[to]);
            cx.emit(&self.rove, to as f64);
        }
    }
}

/// `use:rovingList`: a highlighted index inside the host (its own rows): the arrows (by
/// `orientation`), Home and End move it over the enabled items, wrapping with `wrap`.
#[svelters::behavior(namespace = "sv", name = "rovingList")]
pub struct UseRovingList {
    /// One entry per item: whether it can be highlighted.
    #[prop]
    enabled: Vec<bool>,
    #[prop(enumeration = "vertical|horizontal")]
    orientation: String,
    #[prop]
    wrap: bool,
    /// The highlighted item (`-1`: none); the host writes it on hover.
    #[prop(default = -1)]
    current: i64,
    /// The highlight moved to item `to`.
    #[event]
    r#move: Event<f64>,
    #[state]
    u: UseUnit<RovingList>,
}

impl Behavior for UseRovingList {
    fn sense(&self) -> egui::Sense {
        egui::Sense::focusable_noninteractive()
    }
    fn after_ui(&mut self, _host: NodeRef<'_>, resp: &egui::Response, cx: &mut BehaviorCx<'_>) {
        self.u.params = RovingListParams {
            enabled: self.enabled.clone(),
            orientation: if self.orientation == "horizontal" { Orientation::Horizontal } else { Orientation::Vertical },
            wrap: self.wrap,
        };
        self.u.unit.set(usize::try_from(self.current).ok());
        for Move { to } in run(&mut self.u, resp, cx) {
            self.current = to as i64;
            cx.emit(&self.r#move, to as f64);
        }
    }
}

/// `use:typeahead`: printable keys typed on the focused host within `timeout` build a prefix,
/// reported as `search(text)` (typing one letter again cycles: the app picks the next match).
#[svelters::behavior(namespace = "sv", name = "typeahead")]
pub struct UseTypeahead {
    /// Seconds between keys before the prefix restarts.
    #[prop(default = 1.0)]
    timeout: f64,
    /// The prefix typed so far.
    #[event]
    search: Event<String>,
    #[state]
    u: UseUnit<Typeahead>,
}

impl Behavior for UseTypeahead {
    fn sense(&self) -> egui::Sense {
        egui::Sense::focusable_noninteractive()
    }
    fn after_ui(&mut self, _host: NodeRef<'_>, resp: &egui::Response, cx: &mut BehaviorCx<'_>) {
        self.u.params = TypeaheadParams { timeout: self.timeout };
        for Search { text } in run(&mut self.u, resp, cx) {
            cx.emit(&self.search, text);
        }
    }
}

// ---- floating surfaces ----

/// `use:anchored`: where a surface of the host's size goes beside `anchor` (`[x, y, width,
/// height]` in points) inside the window, flipping and shifting to stay visible. Reports
/// `placed({ side, x, y, width, height })` when the placement changes; the app positions the host.
#[svelters::behavior(namespace = "sv", name = "anchored")]
pub struct UseAnchored {
    /// The anchor rect: `[x, y, width, height]`.
    #[prop]
    anchor: Vec<f64>,
    /// `bottom-start`, `top`, `right-end` and so on.
    #[prop(default = "bottom-start".to_string())]
    placement: String,
    /// Points between the anchor and the surface.
    #[prop]
    gap: f64,
    /// Flips to the other side when it does not fit.
    #[prop(default = true)]
    flip: bool,
    /// Shifts along the side to stay inside the window.
    #[prop(default = true)]
    shift: bool,
    /// The surface is at least as wide as the anchor.
    #[prop(rename = "matchWidth")]
    match_width: bool,
    /// Where it goes: `{ side, x, y, width, height }`.
    #[event]
    placed: Event<UsePlaced>,
    #[state]
    u: UseUnit<Anchored>,
    #[state]
    last: Option<(Side, egui::Rect)>,
}

impl Behavior for UseAnchored {
    fn after_ui(&mut self, _host: NodeRef<'_>, resp: &egui::Response, cx: &mut BehaviorCx<'_>) {
        let a = |i: usize| self.anchor.get(i).copied().unwrap_or(0.0) as f32;
        self.u.params = AnchorParams {
            anchor: egui::Rect::from_min_size(egui::pos2(a(0), a(1)), egui::vec2(a(2), a(3))),
            size: resp.rect.size(),
            bounds: anchor::bounds(&resp.ctx),
            placement: Placement::parse(&self.placement).unwrap_or_default(),
            gap: self.gap as f32,
            flip: self.flip,
            shift: self.shift,
            match_anchor_width: self.match_width,
            ..AnchorParams::default()
        };
        if let Some(p) = run(&mut self.u, resp, cx).pop()
            && self.last != Some((p.side, p.rect))
        {
            self.last = Some((p.side, p.rect));
            let side = match p.side {
                Side::Top => "top",
                Side::Bottom => "bottom",
                Side::Left => "left",
                Side::Right => "right",
            };
            let r = p.rect;
            cx.emit(&self.placed, UsePlaced { side: side.into(), x: r.min.x as f64, y: r.min.y as f64, width: r.width() as f64, height: r.height() as f64 });
        }
    }
}

/// `use:dismissable`: asks to close the host (a popup the app owns) on Escape (with `escape`) or
/// on a press outside it (with `outside: 'close'`): `dismiss({ reason, x, y })`, `reason` one of
/// `escape`, `outside-press`.
#[svelters::behavior(namespace = "sv", name = "dismissable")]
pub struct UseDismissable {
    /// Escape dismisses.
    #[prop(default = true)]
    escape: bool,
    /// A press outside the host: `close` asks to dismiss, `ignore` does not.
    #[prop(enumeration = "close|ignore")]
    outside: String,
    /// `{ reason, x, y }`.
    #[event]
    dismiss: Event<UseDismiss>,
    #[state]
    u: UseUnit<Dismissable>,
}

impl Behavior for UseDismissable {
    fn after_ui(&mut self, _host: NodeRef<'_>, resp: &egui::Response, cx: &mut BehaviorCx<'_>) {
        self.u.params = DismissParams {
            inside: vec![resp.rect],
            on_outside: if self.outside == "ignore" { crate::interact::dismiss::Outside::Ignore } else { crate::interact::dismiss::Outside::Close },
            escape: self.escape,
        };
        for Dismiss { reason, at } in run(&mut self.u, resp, cx) {
            let reason = match reason {
                crate::interact::dismiss::Reason::Escape => "escape",
                crate::interact::dismiss::Reason::OutsidePress => "outside-press",
                crate::interact::dismiss::Reason::FocusOut => "focus-out",
                crate::interact::dismiss::Reason::AnchorHidden => "anchor-hidden",
                crate::interact::dismiss::Reason::Programmatic => "programmatic",
            };
            cx.emit(&self.dismiss, UseDismiss { reason: reason.into(), x: at.map(|p| p.x as f64), y: at.map(|p| p.y as f64) });
        }
    }
}

/// `use:tooltipable`: the platform's tooltip timing on the host (the delay, the follow-on window,
/// a press hiding it until the pointer leaves, disabled hosts included, R9), for a tooltip the app
/// draws itself: `tip(true)` to show, `tip(false)` to hide. The common `tooltip` prop needs none of
/// this.
#[svelters::behavior(namespace = "sv", name = "tooltipable")]
pub struct UseTooltipable {
    /// Seconds before it shows (`0`: the platform's).
    #[prop]
    delay: f64,
    /// Seconds it stays (`0`: until the pointer leaves).
    #[prop]
    duration: f64,
    /// Show (`true`) or hide.
    #[event]
    tip: Event<bool>,
    #[state]
    u: UseUnit<Tooltipable>,
}

impl Behavior for UseTooltipable {
    fn after_ui(&mut self, _host: NodeRef<'_>, resp: &egui::Response, cx: &mut BehaviorCx<'_>) {
        let delay = if self.delay > 0.0 { self.delay } else { crate::native::policy_of(&resp.ctx).tooltip_delay_ms as f64 / 1000.0 };
        self.u.params = TooltipParams { has_tip: true, delay, duration: self.duration, ..TooltipParams::default() };
        for t in run(&mut self.u, resp, cx) {
            cx.emit(&self.tip, matches!(t, Tip::Show));
        }
    }
}

// ---- drags and values ----

/// `use:draggable`: a drag past the platform threshold on the host, with `:dragging`:
/// `drag({ phase, dx, dy })`, `phase` one of `start`, `move`, `end`, `cancel` (Escape); `dx`, `dy` are
/// the total movement since the start (along `axis`).
#[svelters::behavior(namespace = "sv", name = "draggable")]
pub struct UseDraggable {
    #[prop(enumeration = "both|x|y")]
    axis: String,
    /// Points before a drag starts (`null`: the platform's).
    #[prop]
    threshold: Option<f64>,
    /// `{ phase, dx, dy }`.
    #[event]
    drag: Event<UseDrag>,
    #[state]
    u: UseUnit<Draggable>,
}

impl Behavior for UseDraggable {
    fn sense(&self) -> egui::Sense {
        self.u.sense()
    }
    fn after_ui(&mut self, _host: NodeRef<'_>, resp: &egui::Response, cx: &mut BehaviorCx<'_>) {
        self.u.params = DragParams {
            threshold: self.threshold.map(|t| t as f32),
            axis: match self.axis.as_str() {
                "x" => Axis::X,
                "y" => Axis::Y,
                _ => Axis::Both,
            },
        };
        let mut total = egui::Vec2::ZERO;
        for d in run(&mut self.u, resp, cx) {
            let phase = match d {
                Drag::Start { .. } => "start",
                Drag::Move { total: t, .. } => {
                    total = t;
                    "move"
                }
                Drag::End => "end",
                Drag::Cancel => "cancel",
            };
            cx.emit(&self.drag, UseDrag { phase: phase.into(), dx: total.x as f64, dy: total.y as f64 });
        }
    }
    fn detached(&mut self, host: &mut NodeMut<'_>) {
        host.set_state(Draggable::OWNS, false);
    }
}

/// `use:dragToValue`: dragging on the host scrubs `value` (relative: `speed` per point, Shift
/// slower, Ctrl/Cmd faster), with `:active` and `:dragging`: `scrub(value)` live, `commit(value)`
/// at the release, Escape restores.
#[svelters::behavior(namespace = "sv", name = "dragToValue")]
pub struct UseDragToValue {
    #[prop]
    value: f64,
    #[prop(default = f64::NEG_INFINITY)]
    min: f64,
    #[prop(default = f64::INFINITY)]
    max: f64,
    /// Snaps to multiples (`0`: none).
    #[prop]
    step: f64,
    /// Value per point dragged.
    #[prop(default = 1.0)]
    speed: f64,
    /// The value while dragging.
    #[event]
    scrub: Event<f64>,
    /// The value at the release (or the restored one after Escape).
    #[event]
    commit: Event<f64>,
    #[state]
    u: UseUnit<DragToValue>,
}

impl Behavior for UseDragToValue {
    fn sense(&self) -> egui::Sense {
        self.u.sense()
    }
    fn after_ui(&mut self, _host: NodeRef<'_>, resp: &egui::Response, cx: &mut BehaviorCx<'_>) {
        self.u.params = ScrubParams { value: self.value, min: self.min, max: self.max, step: self.step, mode: ScrubMode::Relative { speed: self.speed }, ..ScrubParams::default() };
        for s in run(&mut self.u, resp, cx) {
            match s {
                Scrub::Scrub(v) => {
                    self.value = v;
                    cx.emit(&self.scrub, v);
                }
                Scrub::Commit(v) | Scrub::Cancel { restore: v } => {
                    if !cx.dispatch(&self.commit, v).prevented() {
                        self.value = v;
                    }
                }
                Scrub::Start | Scrub::End => {}
            }
        }
    }
    fn detached(&mut self, host: &mut NodeMut<'_>) {
        host.set_state(DragToValue::OWNS, false);
    }
}

/// `use:steppable`: on the focused host, the arrows step `value` by `step` (PageUp/PageDown by
/// `page`, Home/End to `min`/`max`) and the wheel steps it per `wheel`: `step(value)`.
#[svelters::behavior(namespace = "sv", name = "steppable")]
pub struct UseSteppable {
    #[prop]
    value: f64,
    #[prop(default = f64::NEG_INFINITY)]
    min: f64,
    #[prop(default = f64::INFINITY)]
    max: f64,
    #[prop(default = 1.0)]
    step: f64,
    /// PageUp/PageDown step (`0`: ten steps).
    #[prop]
    page: f64,
    /// Past `max` goes to `min` and back.
    #[prop]
    wrap: bool,
    /// When the wheel steps: `focused` (default), `hover`, `off`.
    #[prop(enumeration = "focused|hover|off")]
    wheel: String,
    /// The new value.
    #[event]
    change: Event<f64>,
    #[state]
    u: UseUnit<Steppable>,
}

impl Behavior for UseSteppable {
    fn sense(&self) -> egui::Sense {
        focusable(egui::Sense::click())
    }
    fn after_ui(&mut self, _host: NodeRef<'_>, resp: &egui::Response, cx: &mut BehaviorCx<'_>) {
        self.u.params = StepParams { value: self.value, min: self.min, max: self.max, step: self.step, page: self.page, wrap: self.wrap, wheel: Wheel::parse(&self.wheel), ..StepParams::default() };
        let p = self.u.params;
        for s in run(&mut self.u, resp, cx) {
            let v = p.apply(self.value, s);
            if v != self.value && !cx.dispatch(&self.change, v).prevented() {
                self.value = v;
            }
        }
    }
}

/// `use:resizable`: the host is a sash: dragging it (or the arrows when focused) resizes `size`
/// along `axis` within `min`..`max`, with `:hover` after the platform's delay, `:active`,
/// `:dragging` and `:focus-visible`; a double click resets (`collapsible` collapses). Reports
/// `resize(size)` live and `commit(size)` at the end; Escape restores.
#[svelters::behavior(namespace = "sv", name = "resizable")]
pub struct UseResizable {
    /// The size the sash controls, in points.
    #[prop]
    size: f64,
    #[prop(enumeration = "x|y")]
    axis: String,
    /// A drag towards the start grows it (a sash on the panel's left or top edge).
    #[prop]
    invert: bool,
    #[prop]
    min: f64,
    #[prop(default = f64::INFINITY)]
    max: f64,
    /// Arrow-key step.
    #[prop(default = 10.0)]
    step: f64,
    #[prop]
    collapsible: bool,
    #[event]
    resize: Event<f64>,
    #[event]
    commit: Event<f64>,
    #[state]
    u: UseUnit<Resizable>,
}

impl Behavior for UseResizable {
    fn sense(&self) -> egui::Sense {
        focusable(self.u.sense())
    }
    fn after_ui(&mut self, _host: NodeRef<'_>, resp: &egui::Response, cx: &mut BehaviorCx<'_>) {
        self.u.params = ResizeParams {
            axis: usize::from(self.axis == "y"),
            sign: if self.invert { -1.0 } else { 1.0 },
            min: self.min as f32,
            max: self.max as f32,
            step: self.step as f32,
            collapsible: self.collapsible,
            ..ResizeParams::default()
        };
        if !self.u.unit.dragging() {
            self.u.unit.set_size(self.size as f32);
        }
        for r in run(&mut self.u, resp, cx) {
            match r {
                Resize::Start => {}
                Resize::Resize { size } => cx.emit(&self.resize, size as f64),
                Resize::Commit { size } | Resize::Cancel { size } => {
                    if !cx.dispatch(&self.commit, size as f64).prevented() {
                        self.size = size as f64;
                    }
                }
            }
        }
        // a splitter needs a name (the host's, else what it does)
        resp.ctx.accesskit_node_builder(resp.id, |nd| {
            if nd.label().is_none() {
                nd.set_label("Resize");
            }
        });
        if self.u.unit.hovered() || self.u.unit.dragging() {
            resp.ctx.set_cursor_icon(if self.axis == "y" { egui::CursorIcon::ResizeRow } else { egui::CursorIcon::ResizeColumn });
        }
    }
    fn detached(&mut self, host: &mut NodeMut<'_>) {
        host.set_state(Resizable::OWNS, false);
    }
}

/// `use:scrollable`: the wheel, trackpad momentum, and (with `keys`) the arrows and
/// PageUp/PageDown/Home/End on the focused host scroll a virtual `content` of
/// `[width, height]` points through the host's rect: `scroll({ x, y })`, the new offset. The app
/// draws the content at minus the offset.
#[svelters::behavior(namespace = "sv", name = "scrollable")]
pub struct UseScrollable {
    /// The content's size: `[width, height]`.
    #[prop]
    content: Vec<f64>,
    /// The offset: `[x, y]`.
    #[prop]
    offset: Vec<f64>,
    /// Which axes scroll.
    #[prop(enumeration = "y|x|both", default = "y".to_string())]
    axes: String,
    /// The keys scroll when the host is focused.
    #[prop]
    keys: bool,
    /// The new offset: `{ x, y }`.
    #[event]
    scroll: Event<UseScroll>,
    #[state]
    u: UseUnit<Scrollable>,
}

impl Behavior for UseScrollable {
    fn sense(&self) -> egui::Sense {
        if self.keys { egui::Sense::focusable_noninteractive() } else { egui::Sense::hover() }
    }
    fn after_ui(&mut self, _host: NodeRef<'_>, resp: &egui::Response, cx: &mut BehaviorCx<'_>) {
        let v = |v: &[f64], i: usize| v.get(i).copied().unwrap_or(0.0) as f32;
        self.u.params = ScrollParams {
            axes: match self.axes.as_str() {
                "x" => [true, false],
                "both" => [true, true],
                _ => [false, true],
            },
            chain: true,
            page_keys: self.keys,
            arrow_keys: self.keys,
        };
        let content = egui::vec2(v(&self.content, 0), v(&self.content, 1));
        self.u.unit.set_geometry(content, resp.rect.size(), [None, None]);
        let want = egui::vec2(v(&self.offset, 0), v(&self.offset, 1));
        if self.u.unit.offset() != want {
            self.u.unit.set_offset(want);
        }
        for s in run(&mut self.u, resp, cx) {
            if let Scroll::Scrolled { offset, .. } = s {
                self.offset = vec![offset.x as f64, offset.y as f64];
                cx.emit(&self.scroll, UseScroll { x: offset.x as f64, y: offset.y as f64 });
            }
        }
    }
}
