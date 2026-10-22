//! `drag-value`, svelters' own (OW1b, ARCH-WIDGETS §6.1; Qt QSpinBox/QDoubleSpinBox parity,
//! WIDGET-PARITY §2.6): a number field that scrubs (drag), steps (Up/Down, PageUp/PageDown ×10,
//! the wheel, the optional `dec`/`inc` buttons) and is typed into after a click. One
//! [`ValueBox`]; the node is its field (AccessKit `SpinButton`).
//!
//! Signals (§2.4): `changed` and `input` on every user change (`changed` stays the bound event),
//! `commit` at a commit point (FM1 QW7, CX3's `bind:value|commit`), `submit` on Enter,
//! `drag-started`/`drag-stopped`. Parts: `text` (P), `inc`, `dec` (paint parts with their own
//! states until OW-CSS computes part styles; `:disabled` at a limit without `wrap`).

use super::numfmt::{NumFormat, auto_decimals, decimals_of};
use super::value_box::{BoxCfg, Correction, ValueBox};
use crate::interact::{StepParams, Wheel};
use crate::provider::{A11yCx, AvailableSpace, Event, MeasureCx, Part, StateBits, Widget, WidgetCx};

/// A number edited by dragging, stepping or typing.
#[svelters::widget(namespace = "sv", replaces = "drag-value", caps = FOCUSABLE | EDITABLE | PAINTS_BOX)]
pub struct DragValue {
    /// The value. Kept within `min` and `max`.
    #[prop(bind, measure)]
    value: f64,
    /// Lower bound; null for none.
    #[prop]
    min: Option<f64>,
    /// Upper bound; null for none.
    #[prop]
    max: Option<f64>,
    /// Value change per point dragged (Shift ×0.1, Ctrl/Cmd ×10).
    #[prop(default = 1.0)]
    speed: f64,
    /// Decimals shown; null for automatic (from `speed` and `step`).
    #[prop(measure)]
    decimals: Option<i64>,
    /// Text before the number.
    #[prop(measure)]
    prefix: String,
    /// Text after the number.
    #[prop(measure)]
    suffix: String,
    /// One step (Up/Down, a step button, a wheel notch; Qt `singleStep`).
    #[prop(default = 1.0)]
    step: f64,
    /// PageUp/PageDown in value units; 0 for ten steps.
    #[prop]
    page_step: f64,
    /// Shows the `dec`/`inc` step buttons at the right end (Qt `buttonSymbols`).
    #[prop(measure)]
    buttons: bool,
    /// Commit-only mode (Qt `keyboardTracking=false`): `value` moves only at a commit point.
    #[prop(rename = "commit")]
    commit_mode: bool,
    /// When the wheel steps: `focused` (default: over the field while it has focus), `hover`, `off`.
    #[prop(default = "focused".to_string(), enumeration = "focused|hover|off")]
    wheel: String,
    /// Stepping past one end goes to the other (Qt `wrapping`).
    #[prop]
    wrap: bool,
    /// Shown instead of the number at `min` (Qt `specialValueText`, e.g. "Auto").
    #[prop(measure)]
    special_value_text: String,
    /// Holding a step button speeds it up (Qt `accelerated`).
    #[prop]
    accelerated: bool,
    /// `default`, or `adaptive` (Qt AdaptiveDecimalStepType: one decade below the value).
    #[prop(default = "default".to_string(), enumeration = "default|adaptive")]
    step_type: String,
    /// An out-of-range typed value commits to the `nearest` bound (default) or the `previous` value.
    #[prop(default = "nearest".to_string(), enumeration = "nearest|previous")]
    correction: String,
    /// Shown and focusable, not changed by the user (Qt `readOnly`).
    #[prop]
    read_only: bool,
    /// The locale's group separators (Qt `showGroupSeparator`).
    #[prop(default = true, measure)]
    group_separator: bool,
    /// A registered formatter (`numfmt::register_formatter`; Qt `textFromValue`).
    #[prop(measure)]
    formatter: String,
    /// A registered parser (`numfmt::register_parser`; Qt `valueFromText`).
    #[prop]
    parser: String,
    /// The user changed it; carries the new value.
    #[event]
    changed: Event<f64>,
    /// §2.4's live signal (as `changed`).
    #[event]
    input: Event<f64>,
    /// The user committed a value (drag released, Enter or blur after typing, a step).
    #[event]
    commit: Event<f64>,
    /// Enter in the field; carries the value.
    #[event]
    submit: Event<f64>,
    #[event]
    drag_started: Event<()>,
    #[event]
    drag_stopped: Event<()>,
    /// The number's text (paint part).
    #[part]
    text: Part,
    /// The step buttons (paint parts with their own `:hover`/`:active`/`:disabled`).
    #[part]
    inc: Part,
    #[part]
    dec: Part,
    #[state]
    field: ValueBox,
}

impl DragValue {
    fn bounds(&self) -> (f64, f64) {
        let lo = self.min.filter(|m| !m.is_nan()).unwrap_or(f64::NEG_INFINITY);
        let hi = self.max.filter(|m| !m.is_nan()).unwrap_or(f64::INFINITY);
        (lo, hi)
    }

    fn fmt(&self) -> NumFormat<'_> {
        let (lo, _) = self.bounds();
        let d = match self.decimals {
            Some(n) => {
                let n = n.clamp(0, 15) as usize;
                (n, n)
            }
            None => {
                let speed = if self.speed.is_finite() && self.speed != 0.0 { self.speed.abs() } else { 1.0 };
                let a = auto_decimals(speed).max(decimals_of(self.step));
                (a, a + 2)
            }
        };
        NumFormat {
            prefix: &self.prefix,
            suffix: &self.suffix,
            decimals: d,
            grouping: self.group_separator,
            special: &self.special_value_text,
            min: lo,
            formatter: &self.formatter,
            parser: &self.parser,
        }
    }

    fn step_params(&self) -> StepParams {
        let (lo, hi) = self.bounds();
        StepParams {
            value: self.value,
            min: lo,
            max: hi,
            step: if self.step.is_finite() && self.step > 0.0 { self.step } else { 1.0 },
            page: self.page_step,
            wrap: self.wrap,
            adaptive: self.step_type == "adaptive",
            wheel: Wheel::parse(&self.wheel),
            decimals: self.decimals.map(|d| d.clamp(0, 15) as usize),
            ..StepParams::default()
        }
    }
}

impl Widget for DragValue {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, _avail: AvailableSpace) -> egui::Vec2 {
        let dark = cx.ctx.theme() == egui::Theme::Dark;
        let f = self.fmt();
        let (lo, hi) = self.bounds();
        // wide enough for the value and both finite ends (no relayout while it changes)
        let v = if self.value.is_finite() { self.value } else { 0.0 };
        let w = [v, lo, hi].into_iter().filter(|x| x.is_finite()).map(|x| ValueBox::text_width(cx.ctx, cx.css, &f.display(x), dark)).fold(egui::Vec2::ZERO, |a, b| a.max(b));
        let mut size = egui::vec2(w.x.max(40.0), w.y.max(24.0));
        if self.buttons {
            size.x += 2.0 * size.y;
        }
        size
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        let (lo, hi) = self.bounds();
        // a value written outside the bounds is clamped when drawn (announced, no event)
        if self.value.is_finite() && (self.value < lo || self.value > hi) {
            let v = self.value.clamp(lo.min(hi), hi.max(lo));
            cx.set(&mut self.value, v);
        }
        let mut field = std::mem::take(&mut self.field);
        let out = {
            let cfg = BoxCfg {
                id: cx.id(),
                rect: cx.rect(),
                value: self.value,
                step: self.step_params(),
                speed: self.speed,
                fmt: self.fmt(),
                commit_mode: self.commit_mode,
                read_only: self.read_only,
                enabled: cx.enabled(),
                buttons: self.buttons,
                accelerated: self.accelerated,
                correction: Correction::parse(&self.correction),
                invalid: cx.node().common().invalid,
                css: cx.computed(),
                dark: ui.ctx().theme() == egui::Theme::Dark,
                node: cx.node().id().to_raw(),
            };
            field.ui(ui, &cfg)
        };
        self.field = field;
        for b in [StateBits::HOVERED, StateBits::FOCUSED, StateBits::FOCUS_VISIBLE, StateBits::PRESSED, StateBits::DRAGGING, StateBits::READ_ONLY] {
            cx.set_state(b, out.state.contains(b));
        }
        cx.commit_state();
        if out.drag_started {
            cx.dispatch(&self.drag_started, ());
        }
        if let Some(v) = out.input
            && v != self.value
        {
            // EV1: a Rust filter may prevent the write (the default action of `changed`)
            if !cx.dispatch(&self.changed, v).prevented() {
                cx.set(&mut self.value, v);
                cx.dispatch(&self.input, v);
            }
        }
        if let Some(v) = out.commit {
            cx.dispatch(&self.commit, v);
        }
        if let Some(v) = out.submit {
            cx.dispatch(&self.submit, v);
        }
        if out.drag_stopped {
            cx.dispatch(&self.drag_stopped, ());
        }
        let enabled = cx.enabled();
        let v = self.value;
        out.resp.widget_info(|| egui::WidgetInfo::drag_value(enabled, v));
        out.resp
    }

    fn a11y(&self, cx: &A11yCx<'_>, node: &mut egui::accesskit::Node) {
        // a prefix or suffix names it when nothing else does (egui's rule)
        let affixes: Vec<&str> = [self.prefix.trim(), self.suffix.trim()].into_iter().filter(|s| !s.is_empty()).collect();
        let label = (cx.name.is_none() && !affixes.is_empty()).then(|| affixes.join(" "));
        self.field.a11y(node, label.as_deref());
    }

    fn consume_key(&self, key: egui::Key, m: egui::Modifiers) -> bool {
        self.field.claims(key, m)
    }
}
