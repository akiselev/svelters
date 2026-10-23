//! Toasts (WG4a; OW7 own drawing): `toast-host` and the Rust API [`show`].
//!
//! A `toast-host` draws the toasts of its viewport stacked in a corner, in a foreground
//! `egui::Area` over everything; it takes no space in the layout. OW7: each toast is svelters' own
//! surface (DESIGN R7 `elev-3`: `surface-raised`, a 1px stroke, `popup-shadow`), styled through
//! the host's parts: `::part(toast)`, `::part(accent)` (the level bar, `info`/`success`/`warning`/
//! `error` tokens), `::part(text)` (its colour and font; RT2 runs keep their tag defaults: the
//! runs of `show` are not nodes), `::part(action)` and `::part(close)` (R8 keyboard focus ring).
//! No egui `Frame`, `Label` or `Button`. Toasts come from JS through its
//! methods (`bind:this={host}`, then `host.call('show', text, level, duration)` returns the toast's
//! id; `dismiss(id)`, `clear()`) or from Rust through [`show`], which queues on the egui context:
//! the first `toast-host` drawn in that viewport takes the queue. A toast closes after its duration
//! (the host's `duration` when none is given; 0 = until closed; hovering it holds it open), by its
//! close button, or by `dismiss`/`clear`; each close emits `closed(id, reason)`.

use super::{arg, arity};
use crate::kinds::{self, P, TOAST_CORNER, TOAST_LEVEL};
use crate::scene::props::{as_bool, as_enum, as_f64, as_i64, as_str};
use crate::scene::{Arena, W};
use crate::walk::Out;
use crate::paint::{self, Align, StyleView, UaValue::*};
use crate::provider::StateBits;
use crate::widgets::dialog::{TOOL_UA, tool_with};
use egui::{Color32, Id, Pos2, Rect, Sense, Ui, Vec2, vec2};
use std::sync::Arc;
use svelters_abi::{StrResult, Value};

/// A toast's severity (its accent colour).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Level {
    #[default]
    Info,
    Success,
    Warning,
    Error,
}

impl Level {
    pub fn name(self) -> &'static str {
        match self {
            Level::Info => "info",
            Level::Success => "success",
            Level::Warning => "warning",
            Level::Error => "error",
        }
    }

    /// The name capitalized, as the toast's AccessKit label starts (`Warning: Disk full`).
    pub fn title(self) -> &'static str {
        match self {
            Level::Info => "Info",
            Level::Success => "Success",
            Level::Warning => "Warning",
            Level::Error => "Error",
        }
    }

    fn of(name: &str) -> Level {
        match name {
            "success" => Level::Success,
            "warning" => Level::Warning,
            "error" => Level::Error,
            _ => Level::Info,
        }
    }

    /// The accent bar's look (the level's theme token).
    fn accent_ua(self) -> paint::Ua {
        match self {
            Level::Info => ACCENT_INFO,
            Level::Success => ACCENT_SUCCESS,
            Level::Warning => ACCENT_WARNING,
            Level::Error => ACCENT_ERROR,
        }
    }

    /// AccessKit politeness: warnings and errors interrupt (`assertive`), the rest wait (`polite`).
    fn live(self) -> egui::accesskit::Live {
        match self {
            Level::Warning | Level::Error => egui::accesskit::Live::Assertive,
            _ => egui::accesskit::Live::Polite,
        }
    }
}

/// `toast-host::part(toast)` (R7, `elev-3` as VS Code's notifications, `shadow-lg`).
pub const TOAST_UA: paint::Ua = paint::Ua(&[paint::UaRule::base(&[
    ("background-color", Token("menu-bg")),
    ("border-color", Token("menu-border-color")),
    ("border-width", Token("stroke")),
    ("border-radius", Token("menu-radius")),
    ("box-shadow", Token("popup-shadow")),
    ("padding", Px(8.0)),
    ("padding-left", Px(10.0)),
    ("max-width", Px(360.0)),
    ("margin", Px(12.0)),
    ("gap", Px(6.0)),
    ("color", Token("fg")),
])]);
/// `::part(accent)`: a 4px bar in the level's colour.
pub const ACCENT_INFO: paint::Ua = paint::Ua(&[paint::UaRule::base(&[("width", Px(4.0)), ("border-radius", Px(2.0)), ("background-color", Token("info"))])]);
pub const ACCENT_SUCCESS: paint::Ua = paint::Ua(&[paint::UaRule::base(&[("width", Px(4.0)), ("border-radius", Px(2.0)), ("background-color", Token("success"))])]);
pub const ACCENT_WARNING: paint::Ua = paint::Ua(&[paint::UaRule::base(&[("width", Px(4.0)), ("border-radius", Px(2.0)), ("background-color", Token("warning"))])]);
pub const ACCENT_ERROR: paint::Ua = paint::Ua(&[paint::UaRule::base(&[("width", Px(4.0)), ("border-radius", Px(2.0)), ("background-color", Token("error"))])]);
/// `::part(text)`: the message (wrapped to the toast's width).
pub const TEXT_UA: paint::Ua = paint::Ua(&[paint::UaRule::base(&[("color", Token("fg"))])]);
/// `::part(action)`: a raised secondary button (R1, R3, R2, R8 2px ring at offset 1).
pub const ACTION_UA: paint::Ua = paint::Ua(&[
    paint::UaRule::base(&[
        ("background-color", Token("button2-bg")),
        ("color", Token("button2-fg")),
        ("border-color", Token("control-border")),
        ("border-width", Token("stroke")),
        ("border-radius", Token("button-radius")),
        ("box-shadow", Token("elev-1")),
        ("padding", Px(2.0)),
        ("padding-left", Px(8.0)),
        ("padding-right", Px(8.0)),
        ("cursor", Keyword("pointer")),
    ]),
    paint::UaRule::when(StateBits::HOVERED, &[("background-color", Token("button2-hover"))]),
    paint::UaRule::when(StateBits::PRESSED, &[("box-shadow", Shadows(&[])), ("background-color", Token("toolbar-active"))]),
    paint::UaRule::when(StateBits::FOCUS_VISIBLE, &[("outline-style", Keyword("solid")), ("outline-width", Px(2.0)), ("outline-color", Token("focus")), ("outline-offset", Px(1.0))]),
]);

const fn part(name: &'static str, docs: &'static str) -> crate::provider::PartEntry {
    crate::provider::PartEntry { name, docs, node: false }
}

/// OW7b: `toast-host`'s paint parts (the look of its `show` toasts).
pub static HOST_PARTS: &[crate::provider::PartEntry] = &[
    part("toast", "A `show` toast's box (R7 `elev-3`)."),
    part("accent", "A `show` toast's level bar."),
    part("text", "A `show` toast's text (its runs keep their tag defaults)."),
    part("action", "A `show` toast's action buttons."),
    part("close", "A `show` toast's close button."),
];

/// OW7b: a `<toast>`'s paint parts (the box is the node itself).
pub static NODE_PARTS: &[crate::provider::PartEntry] = &[part("accent", "The level bar (4px, the level's colour)."), part("close", "The close button (a 22px tool button).")];

/// OW7b: the paint parts `::part()` rules reach on the toast kinds.
pub(crate) fn parts_of(w: &W) -> Option<&'static [crate::provider::PartEntry]> {
    match w {
        W::ToastHost(_) => Some(HOST_PARTS),
        W::Toast(_) => Some(NODE_PARTS),
        _ => None,
    }
}

/// One toast.
#[derive(Clone, Debug, PartialEq)]
pub struct Toast {
    pub id: i64,
    pub text: String,
    /// RT2: the runs when `show` got a run list (`crate::rich::from_value`); `text` is their text.
    pub runs: Option<crate::rich::Rich>,
    pub level: Level,
    /// Seconds; 0 = until closed.
    pub duration: f64,
    /// egui time it was first drawn (its clock starts there).
    pub shown: Option<f64>,
    /// OW7: its action buttons' labels (`show`'s 4th argument); a click emits `action(id, index)`
    /// and closes it (`closed(id, "action")`).
    pub actions: Vec<String>,
    /// OW7: the parts' state bits between passes: [toast, close, actions…].
    bits: Vec<StateBits>,
}

/// A `toast-host` node's props and toasts.
#[derive(Clone, Debug, PartialEq)]
pub struct Toasts {
    pub corner: &'static str,
    pub duration: f64,
    pub list: Vec<Toast>,
    next: i64,
    /// Closed by a method since the last frame: (id, reason), emitted when the host is drawn.
    closed: Vec<(i64, &'static str)>,
}

impl Default for Toasts {
    fn default() -> Self {
        Toasts { corner: "bottom-right", duration: 4.0, list: Vec::new(), next: 0, closed: Vec::new() }
    }
}

impl Toasts {
    pub(crate) fn set(&mut self, key: P, v: &Value) -> Result<(), String> {
        match key {
            P::Corner => self.corner = as_enum(v, TOAST_CORNER)?,
            P::Duration => self.duration = as_f64(v)?.max(0.0),
            _ => return Err(format!("{key:?} is not a prop of <toast-host>")),
        }
        Ok(())
    }

    pub(crate) fn get(&self, key: P) -> Option<Value> {
        Some(match key {
            P::Corner => Value::Str(self.corner.to_string()),
            P::Duration => Value::F64(self.duration),
            _ => return None,
        })
    }

    /// Adds a toast; returns its id.
    pub fn push(&mut self, text: String, level: Level, duration: Option<f64>) -> i64 {
        self.next += 1;
        let duration = duration.unwrap_or(self.duration).max(0.0);
        self.list.push(Toast { id: self.next, text, runs: None, level, duration, shown: None, actions: Vec::new(), bits: Vec::new() });
        self.next
    }

    /// `show(text, level?, duration?, actions?)`; RT2: `text` may be a run list
    /// (`crate::rich::from_value`); OW7: `actions` a list of button labels.
    pub(crate) fn show(&mut self, args: &[Value]) -> StrResult<Value> {
        arity("show", args, 1, 4)?;
        let runs = match &args[0] {
            Value::List(_) => Some(crate::rich::from_value(&args[0]).map_err(|e| format!("show text: {e}"))?),
            _ => None,
        };
        let text = match &runs {
            Some(r) => r.plain.clone(),
            None => as_str(&args[0])?,
        };
        let level = match arg(args, 1) {
            Value::Null => Level::Info,
            v => Level::of(as_enum(v, TOAST_LEVEL).map_err(|e| format!("show level: {e}"))?),
        };
        let duration = match arg(args, 2) {
            Value::Null => None,
            v => Some(as_f64(v).map_err(|e| format!("show duration: {e}"))?),
        };
        let actions = match arg(args, 3) {
            Value::Null => Vec::new(),
            Value::List(l) => l.iter().map(as_str).collect::<Result<Vec<_>, _>>().map_err(|e| format!("show actions: {e}"))?,
            v => return Err(format!("show actions: expected a list of labels, got {v:?}")),
        };
        let id = self.push(text, level, duration);
        if let Some(t) = self.list.last_mut() {
            t.runs = runs;
            t.actions = actions;
        }
        Ok(Value::I64(id))
    }

    /// `dismiss(id)`: false if no such toast is shown.
    pub(crate) fn dismiss(&mut self, args: &[Value]) -> StrResult<Value> {
        arity("dismiss", args, 1, 1)?;
        let id = as_i64(&args[0])?;
        let before = self.list.len();
        self.list.retain(|t| t.id != id);
        let found = self.list.len() != before;
        if found {
            self.closed.push((id, "method"));
        }
        Ok(Value::Bool(found))
    }

    pub(crate) fn clear(&mut self, args: &[Value]) -> StrResult<Value> {
        arity("clear", args, 0, 0)?;
        for t in self.list.drain(..) {
            self.closed.push((t.id, "method"));
        }
        Ok(Value::Null)
    }
}

/// OW7b: a declarative `<toast>` node: its props and the host's run state for it.
#[derive(Clone, Debug, PartialEq)]
pub struct ToastNode {
    pub level: Level,
    /// Seconds; `None`: the host's `duration`; 0: until closed.
    pub duration: Option<f64>,
    pub open: bool,
    /// egui time it showed (its clock starts there); cleared when it opens again.
    shown: Option<f64>,
    /// Drawn open at the last pass (a JS `open = false` is then a close with reason `""`).
    was_open: bool,
    /// While its exit transition plays (`:exiting`): the egui time it ends.
    exit_until: Option<f64>,
    /// The close button's state bits between passes.
    close_bits: StateBits,
    /// `layout::layout_overlay`'s generation.
    seen: u64,
}

impl Default for ToastNode {
    fn default() -> Self {
        ToastNode { level: Level::Info, duration: None, open: true, shown: None, was_open: false, exit_until: None, close_bits: StateBits::NONE, seen: 0 }
    }
}

impl ToastNode {
    pub(crate) fn set(&mut self, key: P, v: &Value) -> Result<(), String> {
        match key {
            P::Level => self.level = Level::of(as_enum(v, TOAST_LEVEL)?),
            P::Duration => self.duration = if matches!(v, Value::Null) { None } else { Some(as_f64(v)?.max(0.0)) },
            P::Open => {
                let open = as_bool(v)?;
                if open && !self.open {
                    // shown anew: a fresh timeout, no exit left over
                    self.shown = None;
                    self.exit_until = None;
                }
                self.open = open;
            }
            _ => return Err(format!("{key:?} is not a prop of <toast>")),
        }
        Ok(())
    }

    pub(crate) fn get(&self, key: P) -> Option<Value> {
        Some(match key {
            P::Level => Value::Str(self.level.name().to_string()),
            P::Duration => self.duration.map_or(Value::Null, Value::F64),
            P::Open => Value::Bool(self.open),
            _ => return None,
        })
    }
}

/// A toast queued from Rust ([`show`]).
#[derive(Clone, Debug)]
struct Queued {
    text: String,
    level: Level,
    duration: Option<f64>,
}

fn queue_id() -> Id {
    Id::unique("svelters.toast.queue")
}

/// Shows a toast from Rust in `ctx`'s viewport: the first `toast-host` drawn there takes it on its
/// next frame (`duration`: seconds, `None` for the host's, 0 until closed). A repaint is requested.
pub fn show(ctx: &egui::Context, text: impl Into<String>, level: Level, duration: Option<f64>) {
    let q = Queued { text: text.into(), level, duration };
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Vec<Queued>>(queue_id()).push(q));
    ctx.request_repaint();
}

/// Toasts queued with [`show`] that no host took yet (tests, diagnostics).
pub fn queued(ctx: &egui::Context) -> usize {
    ctx.data(|d| d.get_temp::<Vec<Queued>>(queue_id()).map_or(0, |q| q.len()))
}


/// The link colour of RT2 runs with an `href` in a toast.
const LINK_UA: paint::Ua = paint::Ua(&[paint::UaRule::base(&[("color", Token("link"))])]);


/// What a stacked toast is.
enum Of {
    /// OW7b: a `<toast>` child (its arena index); `lead` is its content's offset in the frame.
    Node { c: usize, lead: Vec2 },
    /// A `show` toast (its index in the host's list) and its galleys.
    Shown { k: usize, galley: Arc<egui::Galley>, actions: Vec<Arc<egui::Galley>> },
}

/// One toast's frame this pass.
struct Laid {
    rect: Rect,
    of: Of,
}

/// The toast chrome around a toast's content, from the host's or the toast's look: the box's
/// padding and border, the accent bar and the close button (`lead`: before the content, `extra`:
/// lead plus after).
struct Chrome {
    lead: Vec2,
    extra: Vec2,
    close_w: f32,
    inner_gap: f32,
}

fn chrome(ts: &StyleView<'_>, accent_w: f32) -> Chrome {
    let [pl, pr, pt, pb] = ts.sides("padding");
    let [bl, br, bt, bb] = ts.sides("border-width");
    let (close_w, inner_gap) = (22.0, 8.0);
    let lead = vec2(pl + bl + accent_w + inner_gap, pt + bt);
    Chrome { lead, extra: lead + vec2(pr + br + inner_gap + close_w + 4.0, pb + bb), close_w, inner_gap }
}

/// OW7b: the toast's own look (its computed style over [`TOAST_UA`], running transitions applied).
fn node_view<'a>(arena: &'a Arena, css: Option<&'a svelters_style::ComputedStyle>, c: usize, dk: bool) -> StyleView<'a> {
    let over = arena.slots.id_at(c).map_or(&[][..], |n| arena.style.overlay(n));
    StyleView::new(css, TOAST_UA, arena.core[c].state, dk).with_overlay(over)
}

/// The accessible text of toast `c`: its descendants' texts, buttons left out (they are named
/// themselves).
fn node_text(arena: &Arena, c: usize, acc: &mut String) {
    for k in 0..arena.children[c].len() {
        let d = arena.children[c][k].index() as usize;
        if kinds::kind_index("Button") == Some(arena.core[d].kind as usize) {
            continue;
        }
        let t = match arena.w(d) {
            W::Label { rich: Some(r), .. } => r.plain.as_str(), // RT2: its runs' text
            w => crate::scene::node::text_of(w),
        };
        if !t.is_empty() {
            if !acc.is_empty() {
                acc.push(' ');
            }
            acc.push_str(t);
        } else {
            node_text(arena, d, acc);
        }
    }
}

/// OW7b: closes `<toast>` `c` with `reason`: `open` false (`changed(false)` unless JS set it),
/// `:exiting` on while its `transition` plays out, then `closed(reason)`.
fn close_node(arena: &mut Arena, c: usize, reason: &'static str, now: f64, from_js: bool, out: &mut Vec<Out>) {
    let W::Toast(t) = arena.w_mut(c) else { return };
    t.open = false;
    t.was_open = false;
    let mut state = arena.core[c].state;
    state.set(StateBits::HOVERED, false);
    crate::style::set_own_state(arena, c, state | StateBits::EXITING);
    let node = arena.slots.id_at(c).expect("live node");
    let exit = arena.style.computed(node).and_then(|c| StyleView::new(Some(c), paint::Ua::NONE, state, false).engine().map(|e| e.transitions().iter().map(|t| t.duration + t.delay).fold(0.0, f32::max))).unwrap_or(0.0) as f64;
    if exit > 0.0 {
        if let W::Toast(t) = arena.w_mut(c) {
            t.exit_until = Some(now + exit);
        }
    } else {
        crate::style::set_own_state(arena, c, state);
    }
    if !from_js {
        crate::leaf::widget_changed(arena, c, &[P::Open], vec![Value::Bool(false)], out);
    }
    let s = kinds::signal_id(arena.core[c].kind as usize, "closed");
    if arena.listens(c, s) {
        out.push(Out::Event(node.to_raw(), s, vec![Value::Str(reason.to_string())]));
    }
}

/// Draws toast-host `i` (OW7: svelters' own surfaces; OW7b: with its `<toast>` children): takes
/// the Rust queue, ages the toasts on egui's clock (kittest's virtual one), stacks them in the
/// corner in one foreground Area (the children first, in order, then the `show` toasts), then
/// emits `closed`, `clicked` and `action`.
pub(crate) fn paint(arena: &mut Arena, i: usize, origin: Vec2, ui: &mut Ui, out: &mut Vec<Out>) {
    use crate::widgets::popup::{css, dark, part_css};
    let node = arena.slots.id_at(i).expect("live node");
    let h = node.to_raw();
    let kind = arena.core[i].kind as usize;
    let id = arena.core[i].egui_id;
    let ctx = ui.ctx().clone();
    let style = ui.style().clone();
    let now = ui.input(|inp| inp.time);
    let enabled = ui.is_enabled() && arena.common(i).enabled && arena.common(i).visible;
    let queued: Vec<Queued> = ctx.data_mut(|d| std::mem::take(d.get_temp_mut_or_default::<Vec<Queued>>(queue_id())));
    let host_css = css(arena, node);
    let [toast_css, accent_css, text_css, action_css, close_css] = ["toast", "accent", "text", "action", "close"].map(|p| part_css(arena, node, p));
    let dk = dark(&ctx);
    let host_duration = {
        let W::ToastHost(t) = arena.w_mut(i) else { unreachable!() };
        for q in queued {
            t.push(q.text, q.level, q.duration);
        }
        t.duration
    };
    // OW7b: the `<toast>` children. A JS `open = false` closes one (`closed("")`); an exit
    // transition that played out ends.
    let kids: Vec<usize> = arena.children[i].iter().map(|c| c.index() as usize).filter(|&c| matches!(arena.w(c), W::Toast(_))).collect();
    let mut next_wake: Option<f64> = None;
    let mut wake = |w: f64| next_wake = Some(next_wake.map_or(w, |n: f64| n.min(w)));
    for &c in &kids {
        let W::Toast(t) = arena.w(c) else { continue };
        if t.was_open && !t.open {
            close_node(arena, c, "", now, true, out);
        }
        let W::Toast(t) = arena.w_mut(c) else { continue };
        if let Some(until) = t.exit_until {
            if now >= until || t.open {
                t.exit_until = None;
                let mut state = arena.core[c].state;
                state.set(StateBits::EXITING, false);
                crate::style::set_own_state(arena, c, state);
            } else {
                wake(until - now);
            }
        }
    }
    let shown_kids: Vec<usize> = kids.iter().copied().filter(|&c| matches!(arena.w(c), W::Toast(t) if t.open || t.exit_until.is_some())).collect();
    let mut list = {
        let W::ToastHost(t) = arena.w_mut(i) else { unreachable!() };
        std::mem::take(&mut t.list)
    };
    let mut closed: Vec<(i64, &'static str)> = {
        let W::ToastHost(t) = arena.w_mut(i) else { unreachable!() };
        std::mem::take(&mut t.closed)
    };
    let mut clicked = Vec::new();
    let mut acted: Vec<(i64, usize)> = Vec::new();
    let mut node_closes: Vec<(usize, &'static str)> = Vec::new();
    let mut node_clicks: Vec<usize> = Vec::new();
    let mut area_rect = Rect::NOTHING;
    if !list.is_empty() || !shown_kids.is_empty() {
        let bounds = crate::interact::anchor::bounds(&ctx);
        let host = StyleView::new(host_css.as_deref(), paint::Ua::ROOT, StateBits::NONE, dk);
        let ts = host.part(toast_css.as_deref(), TOAST_UA);
        let margin = ts.sides("margin")[0];
        let gap = ts.px("gap").unwrap_or(6.0);
        let max_w = ts.px("max-width").unwrap_or(360.0).min(bounds.width() - 2.0 * margin).max(120.0);
        let xs = host.part(text_css.as_deref(), TEXT_UA);
        let aus = host.part(action_css.as_deref(), ACTION_UA);
        let [apl, apr, apt, apb] = aus.sides("padding");
        let text_color = xs.color("color").unwrap_or(Color32::PLACEHOLDER);
        let link = StyleView::ua_only(LINK_UA, StateBits::NONE, dk).color("color").unwrap_or(text_color);
        let m = crate::layout::Metrics::new(&ctx);
        // lay every toast out, then stack them from the corner
        let mut laid: Vec<Laid> = Vec::with_capacity(list.len() + shown_kids.len());
        for &c in &shown_kids {
            let Some(cn) = arena.slots.id_at(c) else { continue };
            let (ccss, acss) = (css(arena, cn), part_css(arena, cn, "accent"));
            let level = match arena.w(c) {
                W::Toast(t) => t.level,
                _ => Level::Info,
            };
            let s = node_view(arena, ccss.as_deref(), c, dk);
            let accent_w = s.part(acss.as_deref(), level.accent_ua()).px("width").unwrap_or(4.0);
            let max_w = s.px("max-width").unwrap_or(max_w).min(bounds.width() - 2.0 * margin);
            let ch = chrome(&s, accent_w);
            let seen = match arena.w(c) {
                W::Toast(t) => t.seen,
                _ => 0,
            };
            // max-content first; wrapped to the toast's max width when wider
            let (mut sz, mut generation) = crate::layout::layout_overlay(arena, &m, cn, [None, None], Pos2::ZERO - origin, seen);
            if sz.x + ch.extra.x > max_w {
                (sz, generation) = crate::layout::layout_overlay(arena, &m, cn, [Some((max_w - ch.extra.x).max(40.0)), None], Pos2::ZERO - origin, generation);
            }
            if let W::Toast(t) = arena.w_mut(c) {
                t.seen = generation;
            }
            let size = vec2(sz.x + ch.extra.x, sz.y.max(ch.close_w) + ch.extra.y).round();
            laid.push(Laid { rect: Rect::from_min_size(Pos2::ZERO, size), of: Of::Node { c, lead: ch.lead } });
        }
        for (k, toast) in list.iter().enumerate() {
            let accent_w = host.part(accent_css.as_deref(), toast.level.accent_ua()).px("width").unwrap_or(4.0);
            let ch = chrome(&ts, accent_w);
            let wrap = (max_w - ch.extra.x).max(40.0);
            let galley = match &toast.runs {
                // RT2: one paragraph of plain runs (tag defaults; these runs are not nodes)
                Some(r) => {
                    let job = crate::rich::layout_job(&ctx, &style, r, crate::rich::Colors { text: text_color, link }, Some(wrap), None);
                    ctx.fonts_mut(|f| f.layout_job(job))
                }
                None => paint::text_galley(&ctx, &xs, &toast.text, Some(wrap)),
            };
            let actions: Vec<_> = toast.actions.iter().map(|a| paint::text_galley(&ctx, &aus, a, None)).collect();
            let row_w: f32 = actions.iter().map(|g| g.size().x + apl + apr + 2.0).sum::<f32>() + gap * actions.len().saturating_sub(1) as f32;
            let content_w = galley.size().x.max(row_w);
            let mut hgt = galley.size().y.max(ch.close_w) + ch.extra.y;
            if !actions.is_empty() {
                hgt += gap + actions.iter().map(|g| g.size().y + apt + apb + 2.0).fold(0.0, f32::max);
            }
            let w = (content_w + ch.extra.x).min(max_w);
            laid.push(Laid { rect: Rect::from_min_size(Pos2::ZERO, vec2(w, hgt).round()), of: Of::Shown { k, galley, actions } });
        }
        let total: f32 = laid.iter().map(|l| l.rect.height()).sum::<f32>() + gap * (laid.len() - 1) as f32;
        let corner = match arena.w(i) {
            W::ToastHost(t) => t.corner,
            _ => "bottom-right",
        };
        let mut y = if corner.starts_with("top") { bounds.top() + margin } else { bounds.bottom() - margin - total };
        for l in &mut laid {
            let w = l.rect.width();
            let x = if corner.ends_with("left") { bounds.left() + margin } else { bounds.right() - margin - w };
            l.rect = Rect::from_min_size(egui::pos2(x, y), l.rect.size());
            y += l.rect.height() + gap;
            area_rect = area_rect.union(l.rect);
            // a `<toast>`'s content goes to its place in the frame
            if let Of::Node { c, lead } = l.of
                && let Some(cn) = arena.slots.id_at(c)
            {
                let seen = match arena.w(c) {
                    W::Toast(t) => t.seen,
                    _ => 0,
                };
                let r = arena.core[c].rect;
                let size = [Some(r.width()), Some(r.height())];
                crate::layout::layout_overlay(arena, &m, cn, size, l.rect.min + lead - origin, seen);
            }
        }
        let mut gone = Vec::new();
        crate::widgets::popup::area(&ctx, id.with("toasts"), egui::Order::Foreground, area_rect, |aui| {
            if !enabled {
                aui.disable();
            }
            // the Area covers the stack (its layer is what the pointer is over)
            aui.expand_to_include_rect(area_rect);
            for l in &laid {
                let r = l.rect;
                match &l.of {
                    &Of::Node { c, .. } => {
                        if let Some(reason) = paint_node(arena, c, origin, r, host_duration, now, aui, dk, out, &mut node_clicks, &mut wake) {
                            node_closes.push((c, reason));
                        }
                    }
                    Of::Shown { k, galley, actions } => {
                        let toast = &mut list[*k];
                        let shown = *toast.shown.get_or_insert(now);
                        toast.bits.resize(2 + toast.actions.len(), StateBits::NONE);
                        let tid = toast.id;
                        let tidk = id.with(("toast", tid));
                        let mut pc = paint::PaintCx::for_node(aui.painter(), h);
                        pc.tag = "toast-host";
                        // the toast itself: an Alert named by its level and text, a polite or
                        // assertive live region; a click on its body is `clicked`
                        let body = aui.interact(r, tidk, Sense::click());
                        let hovered = aui.rect_contains_pointer(r);
                        toast.bits[0].set(StateBits::HOVERED, hovered);
                        let ts = host.part(toast_css.as_deref(), TOAST_UA).in_state(toast.bits[0]);
                        paint::paint_box(&mut pc.part("toast"), &ts, r);
                        let acc = host.part(accent_css.as_deref(), toast.level.accent_ua());
                        let accent_w = acc.px("width").unwrap_or(4.0);
                        let ch = chrome(&ts, accent_w);
                        let content = Rect::from_min_max(r.min + ch.lead - vec2(accent_w + ch.inner_gap, 0.0), r.max - (ch.extra - ch.lead) + vec2(ch.close_w + 4.0 + ch.inner_gap, 0.0));
                        let text_h = galley.size().y.max(ch.close_w);
                        paint::paint_box(&mut pc.part("accent"), &acc, Rect::from_min_size(content.min, vec2(accent_w, content.height())));
                        let tx = r.min.x + ch.lead.x;
                        let text_rect = Rect::from_min_size(egui::pos2(tx, content.min.y + (text_h - galley.size().y) / 2.0), galley.size());
                        paint::paint_text(&mut pc.part("text"), &xs, galley, text_rect, Align::Start);
                        // the text is its own node (a click on it, also an AccessKit click, is the body's)
                        let tl = aui.interact(text_rect, id.with(("toast-text", tid)), Sense::CLICK);
                        tl.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, toast.text.as_str()));
                        let label = format!("{}: {}", toast.level.title(), toast.text);
                        body.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Alert, true, &label));
                        let live = toast.level.live();
                        aui.ctx().accesskit_node_builder(body.id, |n| n.set_live(live));
                        // the close button, top right
                        let close = Rect::from_min_size(egui::pos2(content.max.x - ch.close_w, content.min.y), vec2(ch.close_w, ch.close_w));
                        let mut b = std::mem::take(&mut toast.bits[1]);
                        let dismissed = tool_with(aui, &mut pc, host.part(close_css.as_deref(), TOOL_UA), "close", &mut b, close, tidk, Some(crate::widgets::glyph::Glyph::Close), None, "Dismiss", None);
                        toast.bits[1] = b;
                        // the actions, in a row under the text
                        let mut ax = tx;
                        let ay = content.min.y + text_h + gap;
                        let mut action = None;
                        for (j, g) in actions.iter().enumerate() {
                            let ar = Rect::from_min_size(egui::pos2(ax, ay), vec2(g.size().x + apl + apr + 2.0, g.size().y + apt + apb + 2.0));
                            ax = ar.right() + gap;
                            let mut b = std::mem::take(&mut toast.bits[2 + j]);
                            let ps = host.part(action_css.as_deref(), ACTION_UA);
                            if tool_with(aui, &mut pc, ps, "action", &mut b, ar, tidk.with(j), None, Some(g), &toast.actions[j], None) {
                                action = Some(j);
                            }
                            toast.bits[2 + j] = b;
                        }
                        if dismissed {
                            gone.push((tid, "dismissed"));
                            continue;
                        }
                        if let Some(j) = action {
                            acted.push((tid, j));
                            gone.push((tid, "action"));
                            continue;
                        }
                        if body.clicked() || tl.clicked() {
                            // RT2: a click on a run with an `href` opens it
                            let href = tl.interact_pointer_pos().or(body.interact_pointer_pos()).filter(|p| text_rect.contains(*p)).and_then(|p| toast.runs.as_ref()?.href_at(galley, p - text_rect.min).map(str::to_string));
                            if let Some(u) = href {
                                aui.ctx().open_url(egui::OpenUrl::same_tab(u));
                            }
                            clicked.push(tid);
                        }
                        if toast.duration > 0.0 {
                            let mut left = shown + toast.duration - now;
                            if hovered && left < 1.0 {
                                // hovering holds it open
                                toast.shown = Some(shown + (1.0 - left));
                                left = 1.0;
                            }
                            if left <= 0.0 {
                                gone.push((tid, "timeout"));
                            } else {
                                wake(left);
                            }
                        }
                    }
                }
            }
        });
        list.retain(|t| !gone.iter().any(|(id, _)| *id == t.id));
        closed.extend(gone);
        if !closed.is_empty() || !node_closes.is_empty() {
            ctx.request_repaint();
        }
    }
    if let W::ToastHost(t) = arena.w_mut(i) {
        t.list = list;
    }
    for (c, reason) in node_closes {
        close_node(arena, c, reason, now, false, out);
        if let W::Toast(t) = arena.w(c)
            && let Some(until) = t.exit_until
        {
            wake(until - now);
        }
    }
    for c in node_clicks {
        let s = kinds::signal_id(arena.core[c].kind as usize, "clicked");
        if arena.listens(c, s) {
            let ch = arena.slots.id_at(c).expect("live node").to_raw();
            out.push(Out::Event(ch, s, vec![]));
        }
    }
    if let Some(w) = next_wake {
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(w.max(0.0)));
    }
    arena.core[i].drawn = area_rect;
    let sig = |name: &str| kinds::signal_id(kind, name);
    let (s_closed, s_clicked, s_action) = (sig("closed"), sig("clicked"), sig("action"));
    if arena.listens(i, s_closed) {
        for (tid, reason) in closed {
            out.push(Out::Event(h, s_closed, vec![Value::I64(tid), Value::Str(reason.to_string())]));
        }
    }
    if arena.listens(i, s_clicked) {
        for tid in clicked {
            out.push(Out::Event(h, s_clicked, vec![Value::I64(tid)]));
        }
    }
    if arena.listens(i, s_action) {
        for (tid, j) in acted {
            out.push(Out::Event(h, s_action, vec![Value::I64(tid), Value::I64(j as i64)]));
        }
    }
}

/// OW7b: paints `<toast>` `c` in frame `r` (its content already laid out in it): the box, the
/// accent bar and the close button from its own look, its children as ordinary nodes, an
/// `Alert` live region named by its text. Returns why it closes, if it does this pass.
#[allow(clippy::too_many_arguments)]
fn paint_node(arena: &mut Arena, c: usize, origin: Vec2, r: Rect, host_duration: f64, now: f64, aui: &mut Ui, dk: bool, out: &mut Vec<Out>, clicks: &mut Vec<usize>, wake: &mut impl FnMut(f64)) -> Option<&'static str> {
    use crate::widgets::popup::{css, part_css};
    let cn = arena.slots.id_at(c)?;
    let W::Toast(t) = arena.w(c) else { return None };
    let (level, open) = (t.level, t.open);
    let duration = t.duration.unwrap_or(host_duration);
    let cid = arena.core[c].egui_id;
    // its state: `:hover` (the pointer over it), `:exiting` (kept by `close_node`)
    let hovered = open && aui.rect_contains_pointer(r);
    let mut state = arena.core[c].state;
    state.set(StateBits::HOVERED, hovered);
    if state != arena.core[c].state {
        crate::style::set_own_state(arena, c, state);
    }
    let (ccss, acss, xcss) = (css(arena, cn), part_css(arena, cn, "accent"), part_css(arena, cn, "close"));
    let s = node_view(arena, ccss.as_deref(), c, dk);
    let opacity = s.opacity();
    let acc = s.part(acss.as_deref(), level.accent_ua()).with_overlay(arena.style.part_overlay(cn, "accent"));
    let accent_w = acc.px("width").unwrap_or(4.0);
    let ch = chrome(&s, accent_w);
    let mut ui = aui.new_child(egui::UiBuilder::new().max_rect(r).id_salt(("toast-node", cn.to_raw())));
    if opacity < 1.0 {
        ui.multiply_opacity(opacity);
    }
    if !open {
        ui.disable(); // exiting: drawn, not interactive
    }
    let body = ui.interact(r, cid.with("toast"), Sense::click());
    let mut pc = paint::PaintCx::for_node(ui.painter(), cn.to_raw());
    pc.tag = "toast";
    paint::paint_box(&mut pc, &s, r);
    let content_top = r.min.y + ch.lead.y;
    let content_h = r.height() - ch.extra.y;
    let ax = r.min.x + ch.lead.x - ch.inner_gap - accent_w;
    paint::paint_box(&mut pc.part("accent"), &acc, Rect::from_min_size(egui::pos2(ax, content_top), vec2(accent_w, content_h)));
    let close = Rect::from_min_size(egui::pos2(r.max.x - (ch.extra.x - ch.lead.x) + ch.inner_gap, content_top), vec2(ch.close_w, ch.close_w));
    let mut b = match arena.w(c) {
        W::Toast(t) => t.close_bits,
        _ => StateBits::NONE,
    };
    let xs = s.part(xcss.as_deref(), TOOL_UA).with_overlay(arena.style.part_overlay(cn, "close"));
    let dismissed = tool_with(&ui, &mut pc, xs, "close", &mut b, close, cid.with("toast"), Some(crate::widgets::glyph::Glyph::Close), None, "Dismiss", None);
    drop(pc);
    if let W::Toast(t) = arena.w_mut(c) {
        t.close_bits = b;
    }
    // AccessKit: an Alert named `Level: text`, a polite or assertive live region
    let mut text = String::new();
    node_text(arena, c, &mut text);
    let label = format!("{}: {}", level.title(), text);
    body.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Alert, true, &label));
    ui.ctx().accesskit_node_builder(body.id, |n| n.set_live(level.live()));
    // the children: ordinary nodes (labels with their runs, buttons)
    arena.core[c].drawn = r;
    let before = out.len();
    crate::walk::paint_children(arena, c, origin, &mut ui, out);
    if !open {
        return None;
    }
    // a click on a child button (one with a `clicked` handler) is an action: it closes the toast
    let acted = out[before..].iter().any(|o| match o {
        Out::Event(src, sig, _) => is_child_click(arena, c, *src, *sig),
        _ => false,
    });
    let W::Toast(t) = arena.w_mut(c) else { return None };
    t.was_open = true;
    let shown = *t.shown.get_or_insert(now);
    if dismissed {
        return Some("dismissed");
    }
    if acted {
        return Some("action");
    }
    if body.clicked() {
        clicks.push(c);
    }
    if duration > 0.0 {
        let mut left = shown + duration - now;
        if hovered && left < 1.0 {
            // hovering holds it open
            t.shown = Some(shown + (1.0 - left));
            left = 1.0;
        }
        if left <= 0.0 {
            return Some("timeout");
        }
        wake(left);
    }
    None
}

/// Whether event `sig` from node `src` (raw) is a `clicked` of a button under `<toast>` `c`.
fn is_child_click(arena: &Arena, c: usize, src: u64, sig: u32) -> bool {
    let mut stack: Vec<usize> = arena.children[c].iter().map(|k| k.index() as usize).collect();
    while let Some(d) = stack.pop() {
        if arena.slots.id_at(d).is_some_and(|n| n.to_raw() == src) {
            let kind = arena.core[d].kind as usize;
            return kinds::kind_index("Button") == Some(kind) && kinds::signal_id(kind, "clicked") == sig;
        }
        stack.extend(arena.children[d].iter().map(|k| k.index() as usize));
    }
    false
}
