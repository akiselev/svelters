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
use crate::scene::props::{as_enum, as_f64, as_i64, as_str};
use crate::scene::{Arena, W};
use crate::walk::Out;
use crate::paint::{self, Align, StyleView, UaValue::*};
use crate::provider::StateBits;
use crate::widgets::dialog::{TOOL_UA, tool_with};
use egui::{Color32, Id, Pos2, Rect, Sense, Ui, vec2};
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

/// One toast's layout this pass.
struct Laid {
    rect: Rect,
    galley: std::sync::Arc<egui::Galley>,
    actions: Vec<std::sync::Arc<egui::Galley>>,
}

/// Draws toast-host `i` (OW7: svelters' own surfaces): takes the Rust queue, ages the toasts on
/// egui's clock (kittest's virtual one), stacks them in the corner in one foreground Area, then
/// emits `closed`, `clicked` and `action`.
pub(crate) fn paint(arena: &mut Arena, i: usize, ui: &mut Ui, out: &mut Vec<Out>) {
    use crate::widgets::popup::{css, dark, part_css};
    let node = arena.slots.id_at(i).expect("live node");
    let h = node.to_raw();
    let kind = arena.core[i].kind as usize;
    let id = arena.core[i].egui_id;
    let ctx = ui.ctx().clone();
    let style = ui.style().clone();
    let now = ui.input(|inp| inp.time);
    let enabled = ui.is_enabled() && arena.common(i).enabled;
    let queued: Vec<Queued> = ctx.data_mut(|d| std::mem::take(d.get_temp_mut_or_default::<Vec<Queued>>(queue_id())));
    let host_css = css(arena, node);
    let [toast_css, accent_css, text_css, action_css, close_css] = ["toast", "accent", "text", "action", "close"].map(|p| part_css(arena, node, p));
    let dk = dark(&ctx);
    let W::ToastHost(t) = arena.w_mut(i) else { unreachable!() };
    for q in queued {
        t.push(q.text, q.level, q.duration);
    }
    let mut closed: Vec<(i64, &'static str)> = std::mem::take(&mut t.closed);
    let mut clicked = Vec::new();
    let mut acted: Vec<(i64, usize)> = Vec::new();
    let mut next_wake: Option<f64> = None;
    let mut area_rect = Rect::NOTHING;
    if !t.list.is_empty() {
        let bounds = crate::interact::anchor::bounds(&ctx);
        let host = StyleView::new(host_css.as_deref(), paint::Ua::ROOT, StateBits::NONE, dk);
        let ts = host.part(toast_css.as_deref(), TOAST_UA);
        let [pl, pr, pt, pb] = ts.sides("padding");
        let [bl, br, bt, bb] = ts.sides("border-width");
        let margin = ts.sides("margin")[0];
        let gap = ts.px("gap").unwrap_or(6.0);
        let max_w = ts.px("max-width").unwrap_or(360.0).min(bounds.width() - 2.0 * margin).max(120.0);
        let xs = host.part(text_css.as_deref(), TEXT_UA);
        let aus = host.part(action_css.as_deref(), ACTION_UA);
        let [apl, apr, apt, apb] = aus.sides("padding");
        let (close_w, inner_gap) = (22.0, 8.0);
        let chrome_x = pl + pr + bl + br + inner_gap + close_w + 4.0;
        let text_color = xs.color("color").unwrap_or(Color32::PLACEHOLDER);
        let link = StyleView::ua_only(LINK_UA, StateBits::NONE, dk).color("color").unwrap_or(text_color);
        // lay every toast out, then stack them from the corner
        let mut laid: Vec<Laid> = Vec::with_capacity(t.list.len());
        for toast in &t.list {
            let accent_w = host.part(accent_css.as_deref(), toast.level.accent_ua()).px("width").unwrap_or(4.0);
            let wrap = (max_w - chrome_x - accent_w).max(40.0);
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
            let mut hgt = galley.size().y.max(close_w) + pt + pb + bt + bb;
            if !actions.is_empty() {
                hgt += gap + actions.iter().map(|g| g.size().y + apt + apb + 2.0).fold(0.0, f32::max);
            }
            let w = (content_w + chrome_x + accent_w).min(max_w);
            laid.push(Laid { rect: Rect::from_min_size(Pos2::ZERO, vec2(w, hgt).round()), galley, actions });
        }
        let total: f32 = laid.iter().map(|l| l.rect.height()).sum::<f32>() + gap * (laid.len() - 1) as f32;
        let mut y = if t.corner.starts_with("top") { bounds.top() + margin } else { bounds.bottom() - margin - total };
        for l in &mut laid {
            let w = l.rect.width();
            let x = if t.corner.ends_with("left") { bounds.left() + margin } else { bounds.right() - margin - w };
            l.rect = Rect::from_min_size(egui::pos2(x, y), l.rect.size());
            y += l.rect.height() + gap;
            area_rect = area_rect.union(l.rect);
        }
        let list = &mut t.list;
        let mut gone = Vec::new();
        crate::widgets::popup::area(&ctx, id.with("toasts"), egui::Order::Foreground, area_rect, |aui| {
            if !enabled {
                aui.disable();
            }
            let mut pc = paint::PaintCx::for_node(aui.painter(), h);
            pc.tag = "toast-host";
            for (toast, l) in list.iter_mut().zip(&laid) {
                let shown = *toast.shown.get_or_insert(now);
                toast.bits.resize(2 + toast.actions.len(), StateBits::NONE);
                let r = l.rect;
                let tid = toast.id;
                let tidk = id.with(("toast", tid));
                // the toast itself: an Alert named by its level and text, a polite or assertive
                // live region; a click on its body is `clicked`
                let body = aui.interact(r, tidk, Sense::click());
                let hovered = aui.rect_contains_pointer(r);
                toast.bits[0].set(StateBits::HOVERED, hovered);
                let ts = host.part(toast_css.as_deref(), TOAST_UA).in_state(toast.bits[0]);
                paint::paint_box(&mut pc.part("toast"), &ts, r);
                let content = Rect::from_min_max(r.min + vec2(pl + bl, pt + bt), r.max - vec2(pr + br, pb + bb));
                let acc = host.part(accent_css.as_deref(), toast.level.accent_ua());
                let accent_w = acc.px("width").unwrap_or(4.0);
                let text_h = l.galley.size().y.max(close_w);
                paint::paint_box(&mut pc.part("accent"), &acc, Rect::from_min_size(content.min, vec2(accent_w, content.height())));
                let tx = content.min.x + accent_w + inner_gap;
                let text_rect = Rect::from_min_size(egui::pos2(tx, content.min.y + (text_h - l.galley.size().y) / 2.0), l.galley.size());
                paint::paint_text(&mut pc.part("text"), &xs, &l.galley, text_rect, Align::Start);
                // the text is its own node (a click on it, also an AccessKit click, is the body's)
                let tl = aui.interact(text_rect, id.with(("toast-text", tid)), Sense::CLICK);
                tl.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, toast.text.as_str()));
                let label = format!("{}: {}", toast.level.title(), toast.text);
                body.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Alert, true, &label));
                let live = toast.level.live();
                aui.ctx().accesskit_node_builder(body.id, |n| n.set_live(live));
                // the close button, top right
                let close = Rect::from_min_size(egui::pos2(content.max.x - close_w, content.min.y), vec2(close_w, close_w));
                let mut b = std::mem::take(&mut toast.bits[1]);
                let dismissed = tool_with(aui, &mut pc, host.part(close_css.as_deref(), TOOL_UA), "close", &mut b, close, tidk, Some(crate::widgets::glyph::Glyph::Close), None, "Dismiss", None);
                toast.bits[1] = b;
                // the actions, in a row under the text
                let mut ax = tx;
                let ay = content.min.y + text_h + gap;
                let mut action = None;
                for (j, g) in l.actions.iter().enumerate() {
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
                    let href = tl.interact_pointer_pos().or(body.interact_pointer_pos()).filter(|p| text_rect.contains(*p)).and_then(|p| toast.runs.as_ref()?.href_at(&l.galley, p - text_rect.min).map(str::to_string));
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
                        next_wake = Some(next_wake.map_or(left, |w: f64| w.min(left)));
                    }
                }
            }
        });
        list.retain(|t| !gone.iter().any(|(id, _)| *id == t.id));
        closed.extend(gone);
        if !closed.is_empty() {
            ctx.request_repaint();
        }
    }
    if let Some(w) = next_wake {
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(w));
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
