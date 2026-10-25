//! The `text-edit` kind: [`TextEdit`] wraps a [`TextField`] in presto-core's [`Kind`] (architecture
//! §4.2), with svelters' `text-edit` props and signals (svelters-egui `kinds/builtin.rs`) plus
//! `read-only`, `wrap`, `cancel` and `select`.
//!
//! It implements presto-core's `Kind` directly rather than through the substrate's `Hosted<W>`
//! bridge (`crate::host`): a text field needs `Event::Ime` and the `text_input` hook (architecture
//! §11.3), which `crate::shim::core` does not carry (coordinator, 2026-10-25: keep it so).
//!
//! The document's services carry what it shares with the other kinds: the `TextSystem` (layouts with
//! geometry), the platform `Policy` ([`TextPolicy::from_policy`]) and the clipboard
//! (`Box<dyn presto_platform::Clipboard>`). The caret goes out through presto-core's fast channel
//! (`EventCx::set_caret`), the IME's window-space queries through the node's window transform.
//! Without a text system the kind records its box only and takes no input (it cannot place a caret);
//! without a clipboard it edits without one.

use super::editor::{EditorConfig, FocusReason as FieldFocus};
use super::field::{FieldColors, FieldPointer, FieldSignal, TextField};
use super::keys::KeyAction;
use super::geometry::SystemLayouter;
use super::policy::{Platform, TextPolicy};
use super::shim::{Clipboard, RecordSink};
use presto_base::accesskit;
use presto_base::change::ChangeClass;
use presto_base::color::Color;
use presto_base::cursor::CursorIcon;
use presto_base::display::Prim;
use presto_base::event::PointerPhase;
use presto_base::geom::{Point, Rect};
use presto_base::input::TextInputClient;
use presto_platform::ClipboardKind;
use presto_base::text::GlyphRun;
use presto_core::{
    A11yCx, ComputedBox, Event, EventCx, FocusReason, Handled, Kind, KindSpec, MeasureCx, PointerInput, PrepareCx, PropCx, PropDefault, PropError,
    PropId, PropSpec, RecordCx, SignalFlags, SignalSpec, Value, ValueType,
};
use presto_core::taffy;
use presto_text::TextStyle;
use std::sync::Arc;

/// Text-field signals reach JS at event time (architecture §11.1).
const SIGNAL: SignalFlags = SignalFlags::SYNC;

static PROPS: [PropSpec; 7] = [
    PropSpec::new("text", ValueType::Str, ChangeClass::Layout).default(PropDefault::Str("")).native(),
    PropSpec::new("hint-text", ValueType::Str, ChangeClass::Layout).default(PropDefault::Str("")),
    PropSpec::new("multiline", ValueType::Bool, ChangeClass::Layout).default(PropDefault::Bool(false)),
    PropSpec::new("password", ValueType::Bool, ChangeClass::Layout).default(PropDefault::Bool(false)),
    PropSpec::new("wrap", ValueType::Bool, ChangeClass::Layout).default(PropDefault::Bool(true)),
    PropSpec::new("char-limit", ValueType::I64, ChangeClass::Paint).default(PropDefault::I64(0)),
    PropSpec::new("read-only", ValueType::Bool, ChangeClass::Paint).default(PropDefault::Bool(false)),
];

static SIGNALS: [SignalSpec; 5] = [
    SignalSpec::new("changed", Some(ValueType::Str), SIGNAL),
    SignalSpec::new("commit", Some(ValueType::Str), SIGNAL),
    SignalSpec::new("submit", Some(ValueType::Str), SIGNAL),
    SignalSpec::new("cancel", None, SIGNAL),
    SignalSpec::new("select", None, SIGNAL),
];

/// The `text-edit` spec.
pub static SPEC: KindSpec = KindSpec {
    name: "text-edit",
    props: &PROPS,
    signals: &SIGNALS,
    slots: &[],
    role: accesskit::Role::TextInput,
    text_prop: None,
    focusable: true,
    doc: "A single- or multi-line text field.",
    ..KindSpec::new("text-edit")
};

/// The selection colour until theme tokens reach kinds (svelters-egui's fallback, VS Code's dark
/// `editor.selectionBackground`).
const SELECTION: Color = Color { r: 38.0 / 255.0, g: 79.0 / 255.0, b: 120.0 / 255.0, a: 1.0 };

/// The node's text style for presto-text.
pub fn text_style(s: &ComputedBox) -> TextStyle {
    TextStyle {
        family: s.font_family.to_string(),
        size: s.font_size,
        weight: s.font_weight,
        italic: s.italic,
        line_height: s.line_height,
        letter_spacing: s.letter_spacing,
        wrap: s.wrap,
        ..TextStyle::default()
    }
}

fn colors(s: &ComputedBox) -> FieldColors {
    FieldColors {
        text: s.color,
        placeholder: s.color.with_alpha(s.color.a * 0.5),
        selection: SELECTION,
        selection_inactive: SELECTION.with_alpha(0.5),
        preedit_underline: s.color,
    }
}

/// The `text-edit` kind. See the module docs.
pub struct TextEdit {
    pub field: TextField,
}

impl Default for TextEdit {
    fn default() -> Self {
        let policy = TextPolicy::from_policy(Platform::current(), &crate::host::os_policy());
        TextEdit { field: TextField::new("", EditorConfig::default(), policy) }
    }
}

impl RecordSink for RecordCx<'_> {
    fn push(&mut self, p: Prim, ink: Rect) {
        RecordCx::push(self, p, ink);
    }
    fn draw_run(&mut self, run: Arc<GlyphRun>, origin: Point, color: Color) {
        RecordCx::draw_run(self, run, origin, color);
    }
}

fn field_focus(r: FocusReason) -> FieldFocus {
    match r {
        FocusReason::Pointer => FieldFocus::Pointer,
        FocusReason::Keyboard => FieldFocus::Keyboard,
        FocusReason::Programmatic => FieldFocus::Programmatic,
    }
}

fn pointer(phase: PointerPhase, p: &PointerInput) -> FieldPointer {
    FieldPointer {
        phase,
        button: p.button,
        buttons: p.buttons,
        modifiers: p.modifiers,
        local: p.local,
        count: (phase == PointerPhase::Down).then_some(p.count.max(1)),
    }
}

/// presto-platform's clipboard as the field's.
struct PlatformClipboard<'a>(&'a mut dyn presto_platform::Clipboard);

impl Clipboard for PlatformClipboard<'_> {
    fn get_text(&mut self) -> Option<String> {
        self.0.get_text(ClipboardKind::Clipboard)
    }
    fn set_text(&mut self, t: &str) {
        self.0.set_text(ClipboardKind::Clipboard, t);
    }
    fn get_primary(&mut self) -> Option<String> {
        self.0.get_text(ClipboardKind::Primary)
    }
    fn set_primary(&mut self, t: &str) {
        self.0.set_text(ClipboardKind::Primary, t);
    }
    // presto-platform has no "give PRIMARY up" yet (WAYLAND's methods are not in base): an empty
    // PRIMARY is the closest
    fn clear_primary(&mut self) {
        self.0.set_text(ClipboardKind::Primary, "");
    }
}

impl TextEdit {
    fn prop_name(cx: &PropCx, prop: PropId) -> Option<&'static str> {
        cx.node.kind().prop_index(prop).map(|i| PROPS[i].name)
    }

    /// Emits the field's signals, and writes `text` back as native-owned state after an edit.
    fn emit(&mut self, cx: &mut EventCx) {
        let text = self.field.text();
        for s in self.field.take_signals() {
            let Some(name) = s.name() else { continue };
            if s == FieldSignal::Changed
                && let Some(p) = SPEC.prop_named("text")
            {
                let _ = cx.set_native(p, Value::Str(text.as_str().into()));
            }
            let Some(sig) = SPEC.signal_named(name) else { continue };
            let arg = matches!(s, FieldSignal::Changed | FieldSignal::Commit | FieldSignal::Submit).then(|| Value::Str(text.as_str().into()));
            cx.emit(sig, arg);
        }
    }

    /// After input: serve the clipboard, send the caret, emit, repaint when the content changed.
    fn finish(&mut self, cx: &mut EventCx) {
        if let Some(cb) = cx.service::<Box<dyn presto_platform::Clipboard>>() {
            self.field.serve(&mut PlatformClipboard(&mut **cb));
        }
        if let Some(rect) = self.field.caret_update() {
            cx.set_caret(rect);
        }
        self.emit(cx);
        if self.field.take_repaint() {
            cx.request_paint();
        }
    }
}

impl Kind for TextEdit {
    fn spec() -> &'static KindSpec {
        &SPEC
    }

    fn prop_changed(&mut self, cx: &mut PropCx, prop: PropId, value: Option<&Value>) -> Result<(), PropError> {
        let Some(name) = Self::prop_name(cx, prop) else { return Ok(()) };
        let s = |v: Option<&Value>| v.and_then(Value::as_str).unwrap_or("").to_string();
        let b = |v: Option<&Value>| v.and_then(Value::as_bool).unwrap_or(false);
        let mut cfg = *self.field.editor.config();
        match name {
            "text" => self.field.set_text(&s(value)),
            "hint-text" => self.field.set_placeholder(&s(value)),
            "wrap" => self.field.set_wrap(value.and_then(Value::as_bool).unwrap_or(true)),
            "multiline" => cfg.multiline = b(value),
            "password" => cfg.password = b(value),
            "read-only" => cfg.read_only = b(value),
            "char-limit" => cfg.max_chars = value.and_then(Value::as_i64).filter(|&n| n > 0).map(|n| n.min(u32::MAX as i64) as u32),
            _ => {}
        }
        if cfg != *self.field.editor.config() {
            self.field.set_config(cfg);
        }
        // a prop write is not user input
        self.field.take_signals();
        Ok(())
    }

    fn measure(&self, cx: &mut MeasureCx, known: taffy::Size<Option<f32>>, avail: taffy::Size<taffy::AvailableSpace>) -> taffy::Size<f32> {
        let cfg = self.field.editor.config();
        let max_w = known.width.or(match avail.width {
            taffy::AvailableSpace::Definite(w) => Some(w),
            _ => None,
        });
        let wrap = (cfg.multiline && self.field.wraps()).then_some(max_w).flatten();
        let text = self.field.shown_text();
        let t = cx.measure_text(text, wrap);
        let p = if self.field.placeholder().is_empty() { t } else { cx.measure_text(self.field.placeholder(), wrap) };
        let row = cx.measure_text(" ", None);
        let b = cx.style().border_widths;
        // one point past the text for the caret at its end
        taffy::Size { width: t.w.max(p.w) + 1.0 + b.left + b.right, height: t.h.max(p.h).max(row.h) + b.top + b.bottom }
    }

    fn prepare(&mut self, cx: &mut PrepareCx) {
        let style = text_style(cx.style());
        let content = cx.content_box();
        let to_window = cx.window_transform();
        if let Some(ts) = cx.service::<presto_text::TextSystem>() {
            self.field.prepare(&mut SystemLayouter { system: ts, style }, content, to_window);
        }
        if self.field.take_repaint() {
            cx.request_paint();
        }
    }

    fn record(&self, cx: &mut RecordCx) {
        cx.draw_box();
        let c = colors(cx.style());
        self.field.record(cx, &c);
    }

    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        let Some(style) = cx.view(cx.node()).map(|v| text_style(v.style())) else { return Handled::No };
        if let (Event::FocusIn(_), Some(p)) = (ev, cx.service::<crate::interact::Policy>().copied()) {
            self.field.editor.set_policy(TextPolicy::from_policy(Platform::current(), &p));
        }
        let now = cx.time();
        // what the input asks of the document, applied once the text system is given back
        let (mut focus, mut capture, mut cursor) = (false, None::<bool>, None);
        let handled = {
            let Some(ts) = cx.service::<presto_text::TextSystem>() else { return Handled::No };
            let mut lay = SystemLayouter { system: ts, style };
            match ev {
                Event::Key(k) => !matches!(self.field.key(k, now, &mut lay), KeyAction::Ignore),
                Event::Ime(i) => {
                    self.field.ime(i, now, &mut lay);
                    true
                }
                Event::PointerDown(p) => {
                    focus = !self.field.editor.focused();
                    let took = self.field.pointer(&pointer(PointerPhase::Down, p), now, &mut lay);
                    capture = took.then_some(true);
                    took
                }
                Event::PointerMove(p) => {
                    cursor = Some(CursorIcon::Text);
                    self.field.pointer(&pointer(PointerPhase::Move, p), now, &mut lay)
                }
                Event::PointerUp(p) => {
                    capture = Some(false);
                    self.field.pointer(&pointer(PointerPhase::Up, p), now, &mut lay)
                }
                Event::PointerCancel => self.field.pointer(&FieldPointer::new(PointerPhase::Cancel, None, 0, Point::ZERO), now, &mut lay),
                Event::HoverEnter => {
                    cursor = Some(CursorIcon::Text);
                    false
                }
                Event::FocusIn(r) => {
                    self.field.focus_in(field_focus(*r));
                    true
                }
                Event::FocusOut => {
                    self.field.focus_out();
                    true
                }
                _ => false,
            }
        };
        if focus {
            cx.focus(FocusReason::Pointer);
        }
        match capture {
            Some(true) => cx.capture_pointer(),
            Some(false) => cx.release_pointer(),
            None => {}
        }
        if let Some(c) = cursor {
            cx.set_cursor(c);
        }
        self.finish(cx);
        if handled { Handled::Yes } else { Handled::No }
    }

    fn text_input(&mut self) -> Option<&mut dyn TextInputClient> {
        Some(&mut self.field)
    }

    fn a11y(&self, cx: &mut A11yCx) {
        cx.default_a11y();
        self.field.a11y(cx.out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_keeps_svelters_names_and_change_classes() {
        let s = TextEdit::spec();
        assert_eq!(s.name, "text-edit");
        assert!(s.focusable && !s.container);
        for (name, class) in super::super::field::TEXT_EDIT_PROPS {
            let p = PROPS.iter().find(|p| p.name == *name).unwrap_or_else(|| panic!("prop {name}"));
            assert_eq!(p.change, *class, "{name}");
        }
        assert!(PROPS[0].native_owned, "the field writes `text` itself");
        for name in super::super::field::TEXT_EDIT_SIGNALS {
            assert!(s.signal_named(name).is_some(), "signal {name}");
        }
    }
}
