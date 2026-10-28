//! The `text-edit` kind: [`TextEdit`] wraps a [`TextField`] in skelter-core's [`Kind`] (architecture
//! §4.2), with svelters' `text-edit` props and signals (svelters-egui `kinds/builtin.rs`) plus
//! `read-only`, `wrap`, `cancel` and `select`.
//!
//! It implements skelter-core's `Kind` directly rather than through the substrate's `Hosted<W>`
//! bridge (`crate::host`): a text field needs `Event::Ime` and the `text_input` hook (architecture
//! §11.3), which `crate::shim::core` does not carry (coordinator, 2026-10-25: keep it so).
//!
//! The document's services carry what it shares with the other kinds: the `TextSystem` (layouts with
//! geometry), the platform `Policy` ([`TextPolicy::from_policy`]) and the clipboard
//! (`Box<dyn skelter_platform::Clipboard>`). The caret goes out through skelter-core's fast channel
//! (`EventCx::set_caret`), the IME's window-space queries through the node's window transform.
//! Without a text system the kind records its box only and takes no input (it cannot place a caret);
//! without a clipboard it edits without one. Keys reach the IME first by core's dispatch
//! (`Document::set_ime_source`, set by the platform layer). The field scrolls itself (`self_scroll`):
//! its box is the fixed part (`RecordCx::fixed`), its text is recorded in content space, and a scroll
//! (the caret into view, in an event or in `prepare` through `PrepareCx::scroll_to`) is a compositor
//! offset, never a re-record; the field takes the document's offset back before each event and
//! prepare (a wheel or fling moved it). GTK's one selection is core's
//! (`Document::set_single_selection`): the field claims it when it selects and collapses on
//! `selection_lost`.

use super::editor::{EditorConfig, FocusReason as FieldFocus};
use super::field::{FieldColors, FieldPointer, FieldSignal, TextField};
use super::keys::KeyAction;
use super::geometry::SystemLayouter;
use super::policy::{Platform, TextPolicy};
use super::shim::{Clipboard, RecordSink};
use skelter_base::accesskit;
use skelter_base::change::ChangeClass;
use skelter_base::color::Color;
use skelter_base::cursor::CursorIcon;
use skelter_base::display::Prim;
use skelter_base::event::PointerPhase;
use skelter_base::geom::{Point, Rect};
use skelter_base::input::TextInputClient;
use skelter_platform::ClipboardKind;
use skelter_base::text::GlyphRun;
use skelter_core::{
    A11yCx, ComputedBox, Event, EventCx, FocusReason, Handled, Kind, KindSpec, MeasureCx, PointerInput, PrepareCx, PropCx, PropDefault, PropError,
    PropId, PropSpec, RecordCx, SignalFlags, SignalSpec, Value, ValueType,
};
use skelter_core::taffy;
use skelter_text::TextStyle;
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

/// The selection colour without a token sheet (svelters-egui's fallback, VS Code's dark
/// `editor.selectionBackground`).
const SELECTION: Color = Color { r: 38.0 / 255.0, g: 79.0 / 255.0, b: 120.0 / 255.0, a: 1.0 };

/// The token the selection is filled with (VS Code's `editor.selectionBackground`): read through
/// the cascade, so each theme (and an override on any ancestor) gives its own.
const SELECTION_TOKEN: &str = "--sv-selection";

/// The node's text style for skelter-text.
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

/// The field's colours: its computed `color`, and the theme's selection (`css`: the node's computed
/// style, whose `--sv-selection` wins over [`SELECTION`]).
fn colors(s: &ComputedBox, css: Option<&svelters_style::ComputedStyle>) -> FieldColors {
    let selection = css.and_then(|c| c.custom(SELECTION_TOKEN)).and_then(crate::theme::parse_color).unwrap_or(SELECTION);
    FieldColors {
        text: s.color,
        placeholder: s.color.with_alpha(s.color.a * 0.5),
        selection,
        selection_inactive: selection.with_alpha(selection.a * 0.5),
        preedit_underline: s.color,
    }
}

/// The `text-edit` kind. See the module docs.
pub struct TextEdit {
    pub field: TextField,
    /// A multi-line field's overlay scroll bars, in its foreground (POL2-LAYOUT 1).
    bars: crate::views::scroll::CoreBars,
}

impl Default for TextEdit {
    fn default() -> Self {
        let policy = TextPolicy::from_policy(Platform::current(), &crate::host::os_policy());
        let mut field = TextField::new("", EditorConfig::default(), policy);
        // a scroll is the compositor's offset of the content space
        field.set_scroll_repaints(false);
        TextEdit { field, bars: Default::default() }
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

/// skelter-platform's clipboard as the field's.
struct PlatformClipboard<'a>(&'a mut dyn skelter_platform::Clipboard);

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
    // skelter-platform has no "give PRIMARY up" yet (WAYLAND's methods are not in base): an empty
    // PRIMARY is the closest
    fn clear_primary(&mut self) {
        self.0.set_text(ClipboardKind::Primary, "");
    }
}

impl TextEdit {
    fn multiline(&self) -> bool {
        self.field.editor.config().multiline
    }

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

    /// After input: serve the clipboard, scroll (fast), send the caret, emit, repaint when the content
    /// changed.
    fn finish(&mut self, cx: &mut EventCx) {
        if let Some(cb) = cx.service::<Box<dyn skelter_platform::Clipboard>>() {
            self.field.serve(&mut PlatformClipboard(&mut **cb));
        }
        let node = cx.node();
        if self.field.scroll_offset() != cx.document().scroll_offset(node) {
            cx.scroll_fast(self.field.scroll_offset());
        }
        if let Some(rect) = self.field.caret_update() {
            cx.set_caret(rect);
        }
        // GTK's one selection: core collapses the previous owner (when the document has the rule on)
        if self.field.has_selection() {
            cx.claim_selection();
        } else {
            cx.release_selection();
        }
        self.emit(cx);
        if self.field.take_repaint() {
            cx.request_paint();
        }
        // text, selection, preedit, scroll or layout: the runs and selection go out again (a caret
        // blink is not an event)
        if self.field.take_a11y() {
            cx.request_a11y();
        }
    }

    /// The run index of a virtual child id of `node` (the runs are its only virtual children).
    fn run_of(node: skelter_base::ids::NodeId) -> impl Fn(accesskit::NodeId) -> Option<usize> {
        move |id| skelter_core::a11y::virtual_child(id).filter(|(o, _)| *o == node).map(|(_, k)| k as usize)
    }
}

impl Kind for TextEdit {
    fn spec() -> &'static KindSpec {
        &SPEC
    }

    fn as_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
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
        self.bars.rtl.set(cx.node.is_rtl());
        self.field.rtl = cx.node.is_rtl();
        let style = text_style(cx.style());
        let content = cx.content_box();
        let to_window = cx.window_transform();
        let offset = cx.scroll_offset();
        self.field.set_scroll_offset(offset);
        if let Some(ts) = cx.service::<skelter_text::TextSystem>() {
            self.field.prepare(&mut SystemLayouter { system: ts, style, rtl: self.field.rtl }, content, to_window);
        }
        // the layout scrolled the caret into view: a compositor offset, and the caret follows
        if self.field.scroll_offset() != offset {
            cx.scroll_to(self.field.scroll_offset());
        }
        if let Some(rect) = self.field.caret_update() {
            cx.set_caret(rect);
        }
        if self.field.take_repaint() {
            cx.request_paint();
        }
        // the frame sends the node when its layout or props changed, which is when a prepare
        // relays the text out
        self.field.take_a11y();
    }

    fn record(&self, cx: &mut RecordCx) {
        cx.fixed(|cx| cx.draw_box());
        let c = colors(cx.style(), cx.css());
        self.field.record_content(cx, &c);
    }

    /// A multi-line field's overlay bars at core's offset, over the text (a single-line field shows
    /// none, as VS Code's input box).
    fn record_foreground(&self, cx: &mut RecordCx) {
        if !self.multiline() {
            return;
        }
        let node = cx.node;
        let theme = cx.service::<crate::theme::Theme>().map(|t| t.clone());
        let dark = theme.as_ref().is_none_or(|t| t.is_dark());
        let parent = node.parent().and_then(|p| p.css());
        let styles = crate::shim::core::Styles { computed: node.css(), parts: &[], children: &[], state: crate::interact::StateBits(node.style_state().0), dark, theme: theme.as_ref(), inherited: parent };
        let list = self.bars.record_list(&styles, node.size(), node.scale(), node.scroll_geometry().as_ref());
        let ink = list.bounds;
        for p in list.items {
            cx.push(p, ink);
        }
    }

    /// The text scrolls in the padding box by the field's range ([`TextField::scroll_max`]).
    fn self_scroll(&self, node: &skelter_core::NodeView) -> Option<skelter_core::SelfScroll> {
        let b = node.style().border_widths;
        let view = skelter_base::geom::Size::new((node.size().w - b.left - b.right).max(0.0), (node.size().h - b.top - b.bottom).max(0.0));
        let max = self.field.scroll_max();
        Some(skelter_core::SelfScroll { content: skelter_base::geom::Size::new(view.w + max.x, view.h + max.y), x: true, y: true })
    }

    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        // AT requests need no layout: they edit the text or move the selection
        if let Event::A11y(req) = ev {
            let took = self.field.a11y_action(req, &Self::run_of(cx.node()));
            self.finish(cx);
            return if took { Handled::Yes } else { Handled::No };
        }
        let Some(style) = cx.view(cx.node()).map(|v| text_style(v.style())) else { return Handled::No };
        // read on use: a policy set while the field is focused applies to its next event (SV62)
        // and the host's text keymap (S2, `host::set_text_keymap`)
        let policy = cx.service::<crate::interact::Policy>().copied();
        let keys = cx.service::<Arc<super::keys::TextKeymap>>().cloned();
        if policy.is_some() || keys != self.field.editor.policy().keys {
            let mut tp = match policy {
                Some(p) => TextPolicy::from_policy(Platform::current(), &p),
                None => self.field.editor.policy().clone(),
            };
            tp.keys = keys;
            if tp != *self.field.editor.policy() {
                self.field.editor.set_policy(tp);
            }
        }
        let now = cx.time();
        // the offset a wheel or fling may have moved
        let offset = cx.document().scroll_offset(cx.node());
        let moved = offset != self.field.scroll_offset();
        self.field.set_scroll_offset(offset);
        // a multi-line field's bars: shown by a scroll, faded by their timer
        if self.multiline() {
            self.bars.note(now, policy, cx.document().media_preferences().reduced_motion, moved || matches!(ev, Event::Wheel(_)));
            if matches!(ev, Event::Timer) {
                cx.request_foreground();
            }
            if let Some(t) = self.bars.wake() {
                cx.wake_at(t);
            }
            // POL2-LAYOUT Q3: a press on a bar drags its thumb or pages its track, before the text
            if let Some(g) = cx.view(cx.node()).and_then(|v| v.scroll_geometry()) {
                let bar = match ev {
                    Event::PointerDown(p) | Event::PointerMove(p) | Event::PointerUp(p) => {
                        let phase = match ev {
                            Event::PointerDown(_) => PointerPhase::Down,
                            Event::PointerMove(_) => PointerPhase::Move,
                            _ => PointerPhase::Up,
                        };
                        let primary = p.button == Some(skelter_base::event::MouseButton::Primary) || (phase != PointerPhase::Down && p.buttons & 1 != 0);
                        let shift = p.modifiers.contains(skelter_base::event::Modifiers::SHIFT);
                        Some(self.bars.pointer(&g, phase, primary, p.local, shift))
                    }
                    Event::PointerCancel => Some(self.bars.pointer(&g, PointerPhase::Cancel, false, Point::ZERO, false)),
                    Event::HoverLeave => Some(self.bars.pointer(&g, PointerPhase::Leave, false, Point::ZERO, false)),
                    Event::Timer => {
                        let (scroll, next) = self.bars.repeat(&g);
                        Some(crate::views::scroll::BarPointer { scroll, wake: next, repaint: scroll.is_some(), ..Default::default() })
                    }
                    _ => None,
                };
                if let Some(b) = bar {
                    match b.capture {
                        Some(true) => cx.capture_pointer(),
                        Some(false) => cx.release_pointer(),
                        None => {}
                    }
                    if let Some(o) = b.scroll {
                        cx.scroll_fast(o);
                        self.field.set_scroll_offset(o);
                    }
                    if b.repaint {
                        cx.request_foreground();
                    }
                    if let Some(t) = b.wake {
                        cx.wake_at(t);
                    }
                    if b.ours {
                        cx.set_cursor(CursorIcon::Default);
                        return Handled::Yes;
                    }
                }
            }
        }
        // what the input asks of the document, applied once the text system is given back
        let (mut focus, mut capture, mut cursor) = (false, None::<bool>, None);
        let handled = {
            let Some(ts) = cx.service::<skelter_text::TextSystem>() else { return Handled::No };
            let mut lay = SystemLayouter { system: ts, style, rtl: self.field.rtl };
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

    fn selection_lost(&mut self, cx: &mut EventCx) {
        self.field.lose_selection();
        self.emit(cx);
        if self.field.take_repaint() {
            cx.request_paint();
        }
    }

    fn text_input(&mut self) -> Option<&mut dyn TextInputClient> {
        Some(&mut self.field)
    }

    fn a11y(&self, cx: &mut A11yCx) {
        cx.default_a11y();
        let node = cx.node.id();
        let runs = self.field.a11y(cx.out, &|k| skelter_core::a11y::virtual_id(node, k as u32));
        for (k, run) in runs.into_iter().enumerate() {
            let added = cx.add_virtual_child(run);
            debug_assert_eq!(added, Some(k as u32), "the runs are the field's only virtual children");
        }
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

    #[test]
    fn a_focused_field_follows_a_new_policy_without_refocusing() {
        // SV62: `host::set_policy` while the field has focus reaches its next event
        use crate::testdoc::{child, doc, line_edge};
        let (mut d, root) = doc(skelter_base::geom::Size::new(300.0, 100.0), |r| {
            r.register::<TextEdit>().unwrap();
        });
        d.set_service(skelter_text::TextSystem::new(skelter_text::FontStore::new(skelter_text::FontConfig::testing())));
        let first = crate::host::os_policy();
        d.set_service(first);
        let f = child(&mut d, root, skelter_core::KindId::of("text-edit"), "width: 60px; height: 24px");
        d.focus(f, FocusReason::Keyboard);
        d.dispatch(&line_edge(true));
        let ms = |d: &skelter_core::Document| d.with_kind(f, |k| k.as_any().unwrap().downcast_ref::<TextEdit>().unwrap().field.editor.policy().double_click_ms).unwrap();
        assert_eq!(ms(&d), first.double_click_ms);
        let mut next = first;
        next.double_click_ms = first.double_click_ms + 123;
        crate::host::set_policy(&mut d, next);
        d.dispatch(&line_edge(false));
        assert_eq!(ms(&d), next.double_click_ms);
    }

    #[test]
    fn a_multi_line_field_draws_its_bar_over_the_text_and_moves_it_alone() {
        // POL2-LAYOUT 1: the bar is the foreground (above the scroll layer); a scroll records only it
        use crate::testdoc::{child, doc, frame};
        use skelter_base::display::Prim;
        use skelter_base::ids::NodeId;
        let (mut d, root) = doc(skelter_base::geom::Size::new(300.0, 100.0), |r| {
            r.register::<TextEdit>().unwrap();
        });
        d.set_service(skelter_text::TextSystem::new(skelter_text::FontStore::new(skelter_text::FontConfig::testing())));
        let f = child(&mut d, root, skelter_core::KindId::of("text-edit"), "width: 120px; height: 40px; padding: 2px; border-width: 1px; background-color: #ffffff");
        d.set_prop(f, SPEC.prop_named("multiline").unwrap(), Value::Bool(true)).unwrap();
        let text: Vec<String> = (1..=30).map(|i| format!("line {i}")).collect();
        d.set_prop(f, SPEC.prop_named("text").unwrap(), Value::from(text.join("\n").as_str())).unwrap();
        let fore_key = NodeId(f.0 | 3 << 62);
        let fore = |c: &skelter_base::commit::Commit| c.lists.iter().find(|(k, _)| *k == fore_key).map(|(_, l)| l.clone());
        let c = frame(&mut d).expect("a frame");
        let thumb = fore(&c).expect("a foreground");
        let Prim::Quad(q) = &thumb.items[0] else { panic!("a thumb: {:?}", thumb.items) };
        assert!(q.rect.min.y < 10.0, "at the top: {:?}", q.rect);
        // the first prepare asked for another pass (the field's repaint flag, R1): let it run
        while d.needs_frame(skelter_base::ids::WindowId(1)) {
            frame(&mut d);
        }
        d.set_scroll_offset(f, skelter_base::geom::Vec2::new(0.0, 200.0));
        let c = frame(&mut d).expect("a frame");
        assert_eq!(c.lists.iter().map(|(k, _)| *k).collect::<Vec<_>>(), [fore_key], "only the bar re-records");
        let Prim::Quad(q2) = &fore(&c).unwrap().items[0] else { unreachable!() };
        assert!(q2.rect.min.y > q.rect.min.y, "the thumb moved down: {:?}", q2.rect);
    }

    #[test]
    fn a_multi_line_fields_thumb_drags_and_its_track_pages() {
        // POL2-LAYOUT Q3: a press on the bar is the bar's, not the text's
        use crate::testdoc::{child, doc, frame, pointer};
        use skelter_base::display::Prim;
        use skelter_base::event::PointerPhase;
        use skelter_base::ids::NodeId;
        let (mut d, root) = doc(skelter_base::geom::Size::new(300.0, 100.0), |r| {
            r.register::<TextEdit>().unwrap();
        });
        d.set_service(skelter_text::TextSystem::new(skelter_text::FontStore::new(skelter_text::FontConfig::testing())));
        let f = child(&mut d, root, skelter_core::KindId::of("text-edit"), "width: 120px; height: 40px; padding: 2px; border-width: 1px; background-color: #ffffff");
        d.set_prop(f, SPEC.prop_named("multiline").unwrap(), Value::Bool(true)).unwrap();
        let text: Vec<String> = (1..=30).map(|i| format!("line {i}")).collect();
        d.set_prop(f, SPEC.prop_named("text").unwrap(), Value::from(text.join("\n").as_str())).unwrap();
        let fore_key = NodeId(f.0 | 3 << 62);
        let c = frame(&mut d).expect("a frame");
        while d.needs_frame(skelter_base::ids::WindowId(1)) {
            frame(&mut d);
        }
        let list = c.lists.iter().find(|(k, _)| *k == fore_key).map(|(_, l)| l.clone()).expect("a foreground");
        let Prim::Quad(q) = &list.items[0] else { panic!("a thumb: {:?}", list.items) };
        let origin = d.world_rect(f).unwrap().min;
        let (x, y) = (origin.x + (q.rect.min.x + q.rect.max.x) / 2.0, origin.y + (q.rect.min.y + q.rect.max.y) / 2.0);
        let caret = |d: &skelter_core::Document| d.with_kind(f, |k| k.as_any().unwrap().downcast_ref::<TextEdit>().unwrap().field.editor.caret()).unwrap();
        let before = caret(&d);
        d.dispatch(&pointer(PointerPhase::Down, x, y, 1_000, false));
        d.dispatch(&pointer(PointerPhase::Move, x, y + 10.0, 1_100, true));
        let dragged = d.scroll_offset(f).y;
        assert!(dragged > 0.0, "the thumb drag scrolled the text");
        d.dispatch(&pointer(PointerPhase::Up, x, y + 10.0, 1_200, false));
        assert_eq!(caret(&d), before, "the press left the caret where it was");
        frame(&mut d);
        // a press on the track below the thumb pages on
        let bottom = origin.y + 40.0 - 3.0;
        d.dispatch(&pointer(PointerPhase::Down, x, bottom, 2_000, false));
        d.dispatch(&pointer(PointerPhase::Up, x, bottom, 2_050, false));
        assert!(d.scroll_offset(f).y > dragged, "the track press paged down");
    }

    #[test]
    fn a_long_line_scrolls_on_the_compositor_and_its_box_stays_fixed() {
        use crate::testdoc::{W, child, doc, frame, line_edge};
        use skelter_base::commit::FastCommit;
        use skelter_base::display::Prim;
        use skelter_base::ids::NodeId;
        let (mut d, root) = doc(skelter_base::geom::Size::new(300.0, 100.0), |r| {
            r.register::<TextEdit>().unwrap();
        });
        d.set_service(skelter_text::TextSystem::new(skelter_text::FontStore::new(skelter_text::FontConfig::testing())));
        let f = child(&mut d, root, skelter_core::KindId::of("text-edit"), "width: 60px; height: 24px; padding: 2px; border-width: 1px; background-color: #ffffff");
        d.set_prop(f, SPEC.prop_named("text").unwrap(), Value::from("a long single line that does not fit")).unwrap();
        let c = frame(&mut d).unwrap();
        let fixed_key = NodeId(f.0 | 1 << 63);
        let list = |c: &skelter_base::commit::Commit, n: NodeId| c.lists.iter().find(|(k, _)| *k == n).map(|(_, l)| l.clone());
        assert!(list(&c, fixed_key).is_some_and(|l| matches!(l.items[0], Prim::Quad(_))), "the box is the fixed part");
        assert!(list(&c, f).is_some_and(|l| l.items.iter().any(|p| matches!(p, Prim::Glyphs(_)))), "the text is the scrolled content");
        d.focus(f, FocusReason::Keyboard);
        // a collapsed caret first (focus may have selected all): that re-records
        d.dispatch(&line_edge(false));
        frame(&mut d);
        d.take_fast_commits();
        let offset = |d: &skelter_core::Document| d.scroll_offset(f).x;
        assert_eq!(offset(&d), 0.0);
        // End: the caret goes past the box, the content moves on the compositor
        d.dispatch(&line_edge(true));
        let fast = d.take_fast_commits();
        let scrolled = fast.iter().find_map(|c| match c {
            FastCommit::ScrollOffset { offset, .. } => Some(offset.x),
            _ => None,
        });
        assert!(scrolled.is_some_and(|x| x > 0.0), "a fast scroll: {fast:?}");
        assert_eq!(Some(offset(&d)), scrolled);
        assert!(fast.iter().any(|c| matches!(c, FastCommit::Caret { rect: Some(_), .. })), "the caret follows");
        let c = frame(&mut d);
        assert!(c.as_ref().is_none_or(|c| list(c, f).is_none() && list(c, fixed_key).is_none()), "neither the text nor the box re-records");
        // Home scrolls back the same way
        d.dispatch(&line_edge(false));
        assert!(d.take_fast_commits().iter().any(|c| matches!(c, FastCommit::ScrollOffset { offset, .. } if offset.x == 0.0)));
        let c = frame(&mut d);
        assert!(c.as_ref().is_none_or(|c| list(c, f).is_none()));
        let _ = W;
    }
}
