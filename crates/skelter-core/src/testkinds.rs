//! Minimal kinds and text services for tests (here and in the crates that build on skelter-core): a
//! box, a text leaf and a focusable toggle, a [`TextMeasure`] and a [`TextShaper`] with fixed metrics
//! (every character is `0.5em` wide, a line is `1.2em` tall). Widgets proper are `skelter-widgets`'.

use crate::common;
use crate::cx::{EventCx, MeasureCx, RecordCx, ShapedText, TextShaper};
use crate::kind::{Event, Handled, Kind, KindSpec, NativeState, PropSpec, SignalFlags, SignalSpec};
use crate::registry::KindRegistry;
use crate::value::{PropDefault, Value, ValueType};
use crate::{PropId, SignalId};
use skelter_base::change::ChangeClass;
use skelter_base::color::{Color, Fill};
use skelter_base::display::{Prim, Quad};
use skelter_base::geom::{Corners, Insets, Point, Size};
use skelter_base::ids::{FontId, Revision};
use skelter_base::text::{Glyph, GlyphRun, TextMeasure, TextSpec};
use std::sync::Arc;

/// Fixed metrics: a character is `0.5 * size` wide, a line `1.2 * size` (or the line height) tall.
#[derive(Default, Clone, Copy)]
pub struct FixedText;

impl FixedText {
    fn line_h(spec: &TextSpec<'_>) -> f32 {
        spec.size * spec.line_height.unwrap_or(1.2)
    }
    fn char_w(spec: &TextSpec<'_>) -> f32 {
        spec.size * 0.5 + spec.letter_spacing
    }
    /// Greedy word wrapping at `max`; returns the lines' character counts.
    fn lines(spec: &TextSpec<'_>, max: Option<f32>) -> Vec<usize> {
        let cw = Self::char_w(spec);
        let mut out = Vec::new();
        for para in spec.text.split('\n') {
            let Some(max) = max.filter(|_| spec.wrap) else {
                out.push(para.chars().count());
                continue;
            };
            let per = ((max / cw).floor() as usize).max(1);
            let mut line = 0usize;
            for word in para.split(' ') {
                let w = word.chars().count();
                if line == 0 {
                    line = w;
                } else if line + 1 + w <= per {
                    line += 1 + w;
                } else {
                    out.push(line);
                    line = w;
                }
            }
            out.push(line);
        }
        out
    }
}

impl TextMeasure for FixedText {
    fn measure(&mut self, spec: &TextSpec<'_>, max_width: Option<f32>) -> Size {
        let lines = Self::lines(spec, max_width);
        let w = lines.iter().copied().max().unwrap_or(0) as f32 * Self::char_w(spec);
        Size::new(w, lines.len() as f32 * Self::line_h(spec))
    }
    fn content_widths(&mut self, spec: &TextSpec<'_>) -> (f32, f32) {
        let cw = Self::char_w(spec);
        let min = spec.text.split([' ', '\n']).map(|w| w.chars().count()).max().unwrap_or(0) as f32 * cw;
        let max = spec.text.split('\n').map(|l| l.chars().count()).max().unwrap_or(0) as f32 * cw;
        (min, max)
    }
}

impl TextShaper for FixedText {
    fn shape(&mut self, spec: &TextSpec<'_>, max_width: Option<f32>) -> ShapedText {
        let cw = Self::char_w(spec);
        let lh = Self::line_h(spec);
        let size = self.measure(spec, max_width);
        let mut runs = Vec::new();
        let chars: Vec<char> = spec.text.chars().collect();
        let mut at = 0;
        for (k, n) in Self::lines(spec, max_width).into_iter().enumerate() {
            let glyphs = (0..n).map(|g| Glyph { id: chars.get(at + g).map_or(0, |&c| c as u32), x: g as f32 * cw, y: 0.0 }).collect();
            at += n + 1;
            let run = GlyphRun {
                font: FontId(0),
                size: spec.size,
                variations: 0,
                glyphs,
                ascent: spec.size * 0.8,
                descent: spec.size * 0.2,
                advance: n as f32 * cw,
                hint: Default::default(),
                aa: Default::default(),
                embolden: 0.0,
                skew_deg: 0.0,
            };
            runs.push((Arc::new(run), Point::new(0.0, k as f32 * lh + spec.size * 0.8)));
        }
        ShapedText { size, runs }
    }
}

/// `box`: a container that draws its CSS box, plus a `tint` square when set.
#[derive(Default)]
pub struct BoxKind;

pub const BOX_TINT: PropId = PropId::of("box", "tint", "color");
pub const BOX_EXTENT: PropId = PropId::of("box", "extent", "number");
pub const BOX_RAISE: PropId = PropId::of("box", "raise", "int");

static BOX: KindSpec = KindSpec {
    props: &[
        PropSpec::new("tint", ValueType::Color, ChangeClass::Paint),
        PropSpec::new("extent", ValueType::F64, ChangeClass::Layout),
        PropSpec::new("raise", ValueType::I64, ChangeClass::Property),
    ],
    container: true,
    ..KindSpec::new("box")
};

impl Kind for BoxKind {
    fn spec() -> &'static KindSpec {
        &BOX
    }
    fn measure(&self, cx: &mut MeasureCx, _known: taffy::Size<Option<f32>>, _avail: taffy::Size<taffy::AvailableSpace>) -> taffy::Size<f32> {
        let e = cx.prop_f64(BOX_EXTENT).unwrap_or(0.0) as f32;
        taffy::Size { width: e, height: e }
    }
    fn record(&self, cx: &mut RecordCx) {
        cx.draw_box();
        if let Some(c) = cx.prop(BOX_TINT).and_then(Value::as_color) {
            let r = cx.bounds();
            let q = Quad { rect: r, radii: Corners::ZERO, fill: Fill::Solid(c), border_widths: Insets::ZERO, border_color: Color::TRANSPARENT, border_style: Default::default() };
            cx.push(Prim::Quad(q), r);
        }
    }
}

/// `icon`: a leaf drawing a coverage mask tinted by its computed `color` from palette slot 0, as
/// skelter-widgets' registry icons do (a node that draws only from its palette).
#[derive(Default)]
pub struct IconKind;

static ICON_SPEC: KindSpec = KindSpec::new("icon");

impl Kind for IconKind {
    fn spec() -> &'static KindSpec {
        &ICON_SPEC
    }
    fn record(&self, cx: &mut RecordCx) {
        let r = cx.bounds();
        let color = cx.node.style.color;
        let icon = skelter_base::display::Icon { image: skelter_base::ids::ImageId(1), rect: r, color: Some(color), slot: Some(skelter_base::display::PaletteSlot::new(0, 1.0)) };
        cx.push(Prim::Icon(icon), r);
    }
}

/// `text`: a leaf showing its `text` prop in the computed font.
#[derive(Default)]
pub struct TextKind;

pub const TEXT: PropId = PropId::of("text", "text", "string");

static TEXT_SPEC: KindSpec = KindSpec {
    props: &[PropSpec::new("text", ValueType::Str, ChangeClass::Layout).default(PropDefault::Str(""))],
    role: accesskit::Role::Label,
    text_prop: Some("text"),
    ..KindSpec::new("text")
};

impl Kind for TextKind {
    fn spec() -> &'static KindSpec {
        &TEXT_SPEC
    }
    fn measure(&self, cx: &mut MeasureCx, known: taffy::Size<Option<f32>>, avail: taffy::Size<taffy::AvailableSpace>) -> taffy::Size<f32> {
        let max = known.width.or(match avail.width {
            taffy::AvailableSpace::Definite(w) => Some(w),
            taffy::AvailableSpace::MinContent => Some(0.0),
            taffy::AvailableSpace::MaxContent => None,
        });
        let text = cx.text();
        let s = cx.measure_text(text, max);
        taffy::Size { width: s.w, height: s.h }
    }
    fn record(&self, cx: &mut RecordCx) {
        cx.draw_box();
        let text = cx.text();
        let w = cx.size().w;
        cx.draw_text(text, Point::ZERO, Some(w));
    }
}

/// `toggle`: focusable; a click or Space flips its native-owned `checked` and emits `toggled`.
#[derive(Default)]
pub struct ToggleKind {
    changed: Option<Revision>,
}

pub const TOGGLE_CHECKED: PropId = PropId::of("toggle", "checked", "bool");
pub const TOGGLED: SignalId = SignalId::of("toggle", "toggled", "bool");
/// `flip()`: toggles and returns the new value.
pub const TOGGLE_FLIP: crate::MethodId = crate::MethodId::of("toggle", "flip", "bool");
/// Held this long after a press, the toggle flips (a stand-in for press-held behaviour).
pub const HOLD: skelter_base::time::Span = skelter_base::time::Span::from_millis(500);

static TOGGLE: KindSpec = KindSpec {
    props: &[PropSpec::new("checked", ValueType::Bool, ChangeClass::Paint).default(PropDefault::Bool(false)).native()],
    signals: &[SignalSpec::new("toggled", Some(ValueType::Bool), SignalFlags::SYNC.union(SignalFlags::BUBBLES))
        .args(&[crate::ArgSpec::new("checked", ValueType::Bool)])],
    methods: &[crate::MethodSpec::new("flip", &[], Some(crate::ArgSpec::new("checked", ValueType::Bool)))],
    role: accesskit::Role::CheckBox,
    focusable: true,
    ..KindSpec::new("toggle")
};

impl ToggleKind {
    fn flip(&mut self, cx: &mut EventCx) -> Handled {
        let on = !cx.view(cx.node()).is_some_and(|v| v.prop_bool(TOGGLE_CHECKED));
        if cx.set_native(TOGGLE_CHECKED, Value::Bool(on)).is_ok() {
            self.changed = Some(cx.document().revision());
            cx.set_state(svelters_style::StateMask::CHECKED, on);
            cx.emit(TOGGLED, Some(Value::Bool(on)));
        }
        Handled::Yes
    }
}

impl NativeState for ToggleKind {
    fn changed_at(&self, prop: PropId) -> Option<Revision> {
        (prop == TOGGLE_CHECKED).then_some(self.changed).flatten()
    }
    fn get(&self, _prop: PropId) -> Option<Value> {
        None
    }
}

impl Kind for ToggleKind {
    fn spec() -> &'static KindSpec {
        &TOGGLE
    }
    fn measure(&self, _cx: &mut MeasureCx, _known: taffy::Size<Option<f32>>, _avail: taffy::Size<taffy::AvailableSpace>) -> taffy::Size<f32> {
        taffy::Size { width: 16.0, height: 16.0 }
    }
    fn record(&self, cx: &mut RecordCx) {
        cx.draw_box();
    }
    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        match ev {
            Event::Click(_) => self.flip(cx),
            Event::Key(k) if k.state == skelter_base::event::KeyState::Pressed && k.key == Some(skelter_base::key::Key::Space) => self.flip(cx),
            Event::PointerDown(_) => {
                let at = cx.time() + HOLD;
                cx.wake_at(at);
                Handled::No
            }
            Event::PointerUp(_) | Event::PointerCancel => {
                cx.cancel_wake();
                Handled::No
            }
            Event::Timer => self.flip(cx),
            Event::A11y(r) if r.action == accesskit::Action::Click => self.flip(cx),
            _ => Handled::No,
        }
    }
    fn native_state(&self) -> Option<&dyn NativeState> {
        Some(self)
    }
    fn call(&mut self, cx: &mut EventCx, method: crate::MethodId, _args: &[Value]) -> Result<Option<Value>, crate::PropError> {
        if method != TOGGLE_FLIP {
            return Err(crate::PropError::Rejected(format!("{method:?}")));
        }
        self.flip(cx);
        Ok(cx.view(cx.node()).map(|v| Value::Bool(v.prop_bool(TOGGLE_CHECKED))))
    }
    fn a11y(&self, cx: &mut crate::cx::A11yCx) {
        cx.default_a11y();
        let on = cx.prop_bool(TOGGLE_CHECKED);
        cx.out.set_toggled(if on { accesskit::Toggled::True } else { accesskit::Toggled::False });
        cx.out.add_action(accesskit::Action::Click);
    }
}

/// A registry with the three test kinds.
pub fn registry() -> Arc<KindRegistry> {
    let mut r = KindRegistry::new();
    r.register::<BoxKind>().expect("box");
    r.register::<TextKind>().expect("text");
    r.register::<ToggleKind>().expect("toggle");
    r.register::<IconKind>().expect("icon");
    Arc::new(r)
}

/// The common `tabindex` prop's id, re-exported for tests that set it.
pub const TABINDEX: PropId = common::TABINDEX;
