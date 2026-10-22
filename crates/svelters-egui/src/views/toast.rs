//! Toasts (WG4a): `toast-host` and the Rust API [`show`].
//!
//! A `toast-host` draws the toasts of its viewport stacked in a corner, in a foreground
//! `egui::Area` over everything; it takes no space in the layout. Toasts come from JS through its
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
use egui::{Align2, Color32, Id, Sense, Ui, vec2};
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

    fn color(self, v: &egui::Visuals) -> Color32 {
        match self {
            Level::Info => v.selection.bg_fill,
            Level::Success => Color32::from_rgb(0x3c, 0xa0, 0x5a),
            Level::Warning => v.warn_fg_color,
            Level::Error => v.error_fg_color,
        }
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
        self.list.push(Toast { id: self.next, text, runs: None, level, duration, shown: None });
        self.next
    }

    /// `show(text, level?, duration?)`; RT2: `text` may be a run list (`crate::rich::from_value`).
    pub(crate) fn show(&mut self, args: &[Value]) -> StrResult<Value> {
        arity("show", args, 1, 3)?;
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
        let id = self.push(text, level, duration);
        if let Some(t) = self.list.last_mut() {
            t.runs = runs;
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

#[allow(clippy::disallowed_types)] // OW: wrapper until OW1a3, OW7
/// Draws toast-host `i`: takes the Rust queue, ages the toasts, draws them in the corner area,
/// then emits `closed`/`clicked`.
pub(crate) fn paint(arena: &mut Arena, i: usize, ui: &mut Ui, out: &mut Vec<Out>) {
    let node = arena.slots.id_at(i).expect("live node");
    let h = node.to_raw();
    let kind = arena.core[i].kind as usize;
    let id = arena.core[i].egui_id;
    let ctx = ui.ctx().clone();
    let now = ui.input(|inp| inp.time);
    let queued: Vec<Queued> = ctx.data_mut(|d| std::mem::take(d.get_temp_mut_or_default::<Vec<Queued>>(queue_id())));
    let W::ToastHost(t) = arena.w_mut(i) else { unreachable!() };
    for q in queued {
        t.push(q.text, q.level, q.duration);
    }
    let mut closed: Vec<(i64, &'static str)> = std::mem::take(&mut t.closed);
    let mut clicked = Vec::new();
    let mut next_wake: Option<f64> = None;
    let (align, dir) = match t.corner {
        "bottom-left" => (Align2::LEFT_BOTTOM, vec2(12.0, -12.0)),
        "top-right" => (Align2::RIGHT_TOP, vec2(-12.0, 12.0)),
        "top-left" => (Align2::LEFT_TOP, vec2(12.0, 12.0)),
        _ => (Align2::RIGHT_BOTTOM, vec2(-12.0, -12.0)),
    };
    let mut area_rect = egui::Rect::NOTHING;
    if !t.list.is_empty() {
        let list = &mut t.list;
        let r = egui::Area::new(id.with("toasts")).order(egui::Order::Foreground).anchor(align, dir).interactable(true).show(&ctx, |ui| {
            ui.set_max_width(360.0);
            let mut gone = Vec::new();
            for toast in list.iter_mut() {
                let shown = *toast.shown.get_or_insert(now);
                let accent = toast.level.color(ui.visuals());
                // the text takes the body's clicks; the close button sits beside it (no overlap)
                let fr = egui::Frame::popup(ui.style()).inner_margin(egui::Margin::symmetric(10, 8)).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let (dot, _) = ui.allocate_exact_size(vec2(4.0, ui.text_style_height(&egui::TextStyle::Body)), Sense::hover());
                        ui.painter().rect_filled(dot, 2.0, accent);
                        let body = match &toast.runs {
                            // RT2: one paragraph of plain runs (`crate::rich`); a click on a
                            // run with an `href` opens it
                            Some(r) => {
                                let colors = crate::rich::Colors { text: ui.visuals().text_color(), link: ui.visuals().hyperlink_color };
                                let job = crate::rich::layout_job(ui.ctx(), ui.style(), r, colors, None, None);
                                let (pos, galley, resp) = egui::Label::new(job).wrap().selectable(false).sense(Sense::click()).layout_in_ui(ui);
                                ui.painter().galley(pos, galley.clone(), colors.text);
                                let href = resp.interact_pointer_pos().filter(|_| resp.clicked()).and_then(|p| r.href_at(&galley, p - pos));
                                if let Some(h) = href {
                                    ui.ctx().open_url(egui::OpenUrl::same_tab(h));
                                }
                                resp
                            }
                            None => ui.add(egui::Label::new(toast.text.as_str()).wrap().selectable(false).sense(Sense::click())),
                        };
                        let close = ui.add(egui::Button::new("×").small().frame(false)).on_hover_text("Dismiss").accessible_name("Dismiss");
                        (body.clicked(), close.clicked())
                    })
                    .inner
                });
                // the toast itself: an Alert named by its level and text (hover only: it takes no
                // clicks from its text or close button)
                let alert = ui.interact(fr.response.rect, id.with(("toast", toast.id)), Sense::hover());
                let label = format!("{}: {}", toast.level.title(), toast.text);
                alert.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Alert, true, &label));
                let (body_clicked, dismissed) = fr.inner;
                if dismissed {
                    gone.push((toast.id, "dismissed"));
                    continue;
                }
                if body_clicked {
                    clicked.push(toast.id);
                }
                let hovered = alert.hovered();
                if toast.duration > 0.0 {
                    let mut left = shown + toast.duration - now;
                    if hovered && left < 1.0 {
                        // hovering holds it open
                        toast.shown = Some(shown + (1.0 - left));
                        left = 1.0;
                    }
                    if left <= 0.0 {
                        gone.push((toast.id, "timeout"));
                    } else {
                        next_wake = Some(next_wake.map_or(left, |w: f64| w.min(left)));
                    }
                }
                ui.add_space(6.0);
            }
            gone
        });
        area_rect = r.response.rect;
        let gone = r.inner;
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
    let (s_closed, s_clicked) = (sig("closed"), sig("clicked"));
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
}
