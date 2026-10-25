//! Stand-ins for presto-core and presto-text APIs that do not exist yet (`plans/cloud/presto-b2/BRIEF.md`:
//! the smallest thing the widgets need, replaced at the consolidation). Each item names the API it stands
//! in for; `plans/milestones/PRESTO-widgets-basic.md` lists them.
//!
//! - [`core`]: `presto_core::{Kind, KindSpec, Value, …}` and the kind contexts (architecture §4.2). The
//!   contexts here carry what Tier 0 kinds read and write; presto-core's own contexts will carry more.
//! - [`text`]: `presto_text::{RichText, TextLayout, TextSystem::layout}` (b1 brief), behind
//!   [`text::TextShaper`] so record and measure code takes any shaper.
//! - [`testing`]: a deterministic monospace shaper and a context harness, for this crate's tests (a test
//!   double, never a production path).

pub mod core {
    //! Stand-in for presto-core's kind API (architecture §4.2, b1 brief "presto-core").

    use crate::interact::{FocusReason, Policy, StateBits};
    use crate::shim::text::TextShaper;
    use presto_base::accesskit;
    use presto_base::change::ChangeClass;
    use presto_base::color::Color;
    use presto_base::cursor::CursorIcon;
    use presto_base::display::DisplayList;
    use presto_base::event::{KeyEvent, PointerEvent, WheelEvent};
    use presto_base::geom::{Point, Rect, Size};
    use presto_base::ids::{ImageId, RunId};
    use presto_base::commit::ImageData;
    use presto_base::text::GlyphRun;
    use presto_base::time::Time;
    use std::sync::Arc;
    use svelters_style::ComputedStyle;

    /// Stands in for `presto_core::PropId`: here the index of the prop in its kind's [`KindSpec::props`].
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct PropId(pub u32);
    /// Stands in for `presto_core::SignalId`: the index in [`KindSpec::signals`].
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct SignalId(pub u32);

    /// Stands in for `presto_core::Value`.
    #[derive(Clone, Debug, PartialEq)]
    pub enum Value {
        Bool(bool),
        I64(i64),
        F64(f64),
        Str(Arc<str>),
        /// One of the prop's [`ValueType::Enum`] names.
        Enum(&'static str),
        Color(Color),
        List(Vec<Value>),
        Record(Vec<(Arc<str>, Value)>),
    }

    impl Value {
        pub fn as_bool(&self) -> Option<bool> {
            match self {
                Value::Bool(b) => Some(*b),
                _ => None,
            }
        }
        pub fn as_f64(&self) -> Option<f64> {
            match self {
                Value::F64(v) => Some(*v),
                Value::I64(v) => Some(*v as f64),
                _ => None,
            }
        }
        pub fn as_i64(&self) -> Option<i64> {
            match self {
                Value::I64(v) => Some(*v),
                Value::F64(v) if v.fract() == 0.0 => Some(*v as i64),
                _ => None,
            }
        }
        pub fn as_str(&self) -> Option<&str> {
            match self {
                Value::Str(s) => Some(s),
                Value::Enum(s) => Some(s),
                _ => None,
            }
        }
        pub fn str(s: &str) -> Value {
            Value::Str(Arc::from(s))
        }
    }

    /// A prop's or signal argument's type, as the spec carries it.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum ValueType {
        Bool,
        I64,
        F64,
        Str,
        Enum(&'static [&'static str]),
        Color,
        List,
        Record,
    }

    /// A prop's default in the spec.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum Default {
        Bool(bool),
        I64(i64),
        F64(f64),
        Str(&'static str),
        Enum(&'static str),
        None,
    }

    impl Default {
        pub fn value(self) -> Option<Value> {
            Some(match self {
                Default::Bool(b) => Value::Bool(b),
                Default::I64(v) => Value::I64(v),
                Default::F64(v) => Value::F64(v),
                Default::Str(s) => Value::str(s),
                Default::Enum(s) => Value::Enum(s),
                Default::None => return None,
            })
        }
    }

    /// One prop of a kind: name (kebab-case, as Svelte writes it), type, change class (architecture §4.3).
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct PropSpec {
        pub name: &'static str,
        pub ty: ValueType,
        pub change: ChangeClass,
        pub default: Default,
        /// Native code changes it too (a checkbox's `checked`): JS observes it through `bind:` (§4.5).
        pub bindable: bool,
        pub doc: &'static str,
    }

    impl PropSpec {
        pub const fn new(name: &'static str, ty: ValueType, change: ChangeClass, default: Default, doc: &'static str) -> PropSpec {
            PropSpec { name, ty, change, default, bindable: false, doc }
        }
        pub const fn bindable(mut self) -> PropSpec {
            self.bindable = true;
            self
        }
    }

    /// One signal (a JS-facing event) of a kind (architecture §11.5).
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct SignalSpec {
        pub name: &'static str,
        /// Its listeners never call `preventDefault` (scroll and drag paths default to passive).
        pub passive: bool,
        /// A listener can cancel the native default that follows it.
        pub cancelable: bool,
        /// Delivered synchronously at event time (the default action waits for it).
        pub sync: bool,
        pub args: &'static [(&'static str, ValueType)],
        pub doc: &'static str,
    }

    impl SignalSpec {
        pub const fn new(name: &'static str, args: &'static [(&'static str, ValueType)], doc: &'static str) -> SignalSpec {
            SignalSpec { name, passive: true, cancelable: false, sync: false, args, doc }
        }
        pub const fn cancelable(mut self) -> SignalSpec {
            self.cancelable = true;
            self.passive = false;
            self.sync = true;
            self
        }
    }

    /// Where a node's accessible name comes from.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum NameFrom {
        /// A prop's string value (`text`, `label`).
        Prop(&'static str),
        /// The text of its descendants.
        Contents,
        /// No name of its own (`aria-label` still applies).
        None,
    }

    /// Stands in for `presto_core::KindSpec`: the static description of a kind (architecture §4.2).
    #[derive(Debug)]
    pub struct KindSpec {
        /// The element tag (`button`, `checkbox`).
        pub tag: &'static str,
        pub props: &'static [PropSpec],
        pub signals: &'static [SignalSpec],
        /// Named child slots; empty = children go in the default slot (or none, for leaves).
        pub slots: &'static [&'static str],
        /// The kind takes element children.
        pub children: bool,
        pub role: accesskit::Role,
        pub name_from: NameFrom,
        /// The prop that holds its text content (`<label>text</label>`), if any.
        pub text_prop: Option<&'static str>,
        /// It takes keyboard focus by default (in the Tab order).
        pub focusable: bool,
    }

    impl KindSpec {
        pub fn prop(&self, name: &str) -> Option<PropId> {
            self.props.iter().position(|p| p.name == name).map(|i| PropId(i as u32))
        }
        pub fn signal(&self, name: &str) -> Option<SignalId> {
            self.signals.iter().position(|s| s.name == name).map(|i| SignalId(i as u32))
        }
        pub fn prop_spec(&self, id: PropId) -> Option<&'static PropSpec> {
            self.props.get(id.0 as usize)
        }
    }

    /// Stands in for `presto_core::PropError`.
    #[derive(Clone, Debug, PartialEq)]
    pub enum PropError {
        UnknownProp,
        /// The value has the wrong type (expected type in the spec).
        Type { prop: &'static str, expected: ValueType },
        /// Native code changed this state since the event JS saw (architecture §4.5).
        Stale,
        /// The value has the right type but the kind rejects it (a `grid-template-columns` that is
        /// not a track list); `message` says why.
        Invalid { prop: &'static str, message: String },
    }

    /// Stands in for `presto_core::Handled`.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Handled {
        No,
        Yes,
    }

    /// Stands in for `presto_core::HitResult`.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum HitResult {
        Miss,
        Hit,
    }

    /// Stands in for the event a kind receives from presto-core's dispatch (architecture §11). Pointer
    /// positions are node-local (points); core has done the hit test, capture and hover.
    #[derive(Clone, Debug, PartialEq)]
    pub enum Event {
        /// A pointer event on this node or captured by it (it pressed here and the button is held).
        Pointer(PointerEvent),
        /// The pointer is over this node and nothing covers it (`true`), or no longer (`false`).
        Hover(bool),
        Wheel(WheelEvent),
        /// A key while this node has keyboard focus.
        Key(KeyEvent),
        Focus { gained: bool, reason: FocusReason },
        /// An assistive-technology action addressed to this node.
        A11y(accesskit::ActionRequest),
        /// The time the kind asked for with [`EventCx::wake_at`] came (press repeat, a press held long
        /// enough to become a drag).
        Timer,
        /// The pointer capture ended without a release (another window took it, the node was hidden).
        CaptureLost,
    }

    /// Stands in for `presto_core::StyleCx`: the node's computed style and state, for `layout_style`.
    pub struct StyleCx<'a> {
        pub computed: Option<&'a ComputedStyle>,
        pub state: StateBits,
        pub dark: bool,
    }

    impl StyleCx<'_> {
        /// The layout style the computed style gives (presto-core maps CSS to taffy; this stand-in gives
        /// taffy's default).
        pub fn default_layout(&self) -> taffy::Style {
            taffy::Style::default()
        }
    }

    /// The style a kind reads, shared by the contexts: the node's computed style, its parts' computed
    /// styles (`::part(name)`), state and scheme.
    pub struct Styles<'a> {
        pub computed: Option<&'a ComputedStyle>,
        pub parts: &'a [(&'static str, &'a ComputedStyle)],
        pub state: StateBits,
        pub dark: bool,
    }

    impl<'a> Styles<'a> {
        pub fn part_css(&self, name: &str) -> Option<&'a ComputedStyle> {
            self.parts.iter().find(|(n, _)| *n == name).map(|(_, c)| *c)
        }
    }

    /// What a container that places or hides its children itself reads about one child in
    /// [`Kind::child_layout`]: its index and the `anchor` of every child (empty when it has none).
    pub struct ChildCx<'a> {
        pub index: usize,
        pub anchors: &'a [&'a str],
    }

    impl ChildCx<'_> {
        pub fn count(&self) -> usize {
            self.anchors.len()
        }
    }

    /// Stands in for `presto_core::MeasureCx`.
    pub struct MeasureCx<'a> {
        pub style: Styles<'a>,
        pub text: &'a mut dyn TextShaper,
        pub scale: f32,
    }

    /// Stands in for `presto_core::PrepareCx`: after layout, before recording.
    pub struct PrepareCx<'a> {
        pub size: Size,
        pub style: Styles<'a>,
        pub text: &'a mut dyn TextShaper,
    }

    /// The resources a recording registers (runs and images), the stand-in for presto-core's commit
    /// builder: a run or image registered once keeps its id while the node holds it.
    pub trait Resources {
        /// The id of `run` in the commit (registered on first use).
        fn run(&mut self, run: &Arc<GlyphRun>) -> RunId;
        /// The id of the image under `key` (a content hash or an icon key with its size and colour);
        /// `make` rasterises it when the key is new. `None` when `make` fails.
        fn image(&mut self, key: &str, make: &mut dyn FnMut() -> Option<ImageData>) -> Option<ImageId>;
    }

    /// Stands in for `presto_core::RecordCx`: writes the node's display list in node-local coordinates.
    pub struct RecordCx<'a> {
        /// The node's border box size (its local rect is `(0,0)..size`).
        pub size: Size,
        pub style: Styles<'a>,
        pub list: &'a mut DisplayList,
        pub text: &'a mut dyn TextShaper,
        pub res: &'a mut dyn Resources,
        /// Device pixels per point, for snapping edges to the pixel grid.
        pub scale: f32,
    }

    impl RecordCx<'_> {
        pub fn rect(&self) -> Rect {
            Rect::from_origin_size(Point::ZERO, self.size)
        }
    }

    /// Stands in for `presto_core::HitCx`.
    pub struct HitCx {
        pub size: Size,
    }

    impl HitCx {
        pub fn box_hit(&self, p: Point) -> HitResult {
            if Rect::from_origin_size(Point::ZERO, self.size).contains(p) { HitResult::Hit } else { HitResult::Miss }
        }
    }

    /// A signal a kind emitted, for presto-core to deliver to JS at event time.
    #[derive(Clone, Debug, PartialEq)]
    pub struct Emitted {
        pub signal: SignalId,
        pub args: Vec<Value>,
    }

    /// Stands in for `presto_core::EventCx`: what a kind's native default reads and writes.
    pub struct EventCx<'a> {
        pub now: Time,
        pub size: Size,
        /// The node's state bits; units write the ones they own.
        pub state: &'a mut StateBits,
        /// The node takes input (`enabled` and not inert).
        pub enabled: bool,
        /// The node has keyboard focus.
        pub focused: bool,
        pub policy: &'a Policy,
        /// Signals to deliver, in order.
        pub emitted: Vec<Emitted>,
        /// Props the kind changed natively (`checked` after a click): core journals them with their
        /// change class and tells `bind:` observers.
        pub changed: Vec<PropId>,
        /// The kind asked for focus (`Some(true)`) or gave it up (`Some(false)`).
        pub focus_request: Option<bool>,
        /// The kind wants an [`Event::Timer`] at this time.
        pub wake: Option<Time>,
        /// The kind's content changed (re-record).
        pub repaint: bool,
        pub cursor: Option<CursorIcon>,
        /// The kind asked for pointer capture (`true`) or released it.
        pub capture: Option<bool>,
    }

    impl<'a> EventCx<'a> {
        pub fn new(now: Time, size: Size, state: &'a mut StateBits, policy: &'a Policy) -> EventCx<'a> {
            let enabled = !state.contains(StateBits::DISABLED);
            EventCx { now, size, state, enabled, focused: false, policy, emitted: Vec::new(), changed: Vec::new(), focus_request: None, wake: None, repaint: false, cursor: None, capture: None }
        }
        pub fn emit(&mut self, signal: SignalId, args: Vec<Value>) {
            self.emitted.push(Emitted { signal, args });
        }
        pub fn prop_changed(&mut self, prop: PropId) {
            if !self.changed.contains(&prop) {
                self.changed.push(prop);
            }
            self.repaint = true;
        }
        /// Keeps the earliest of the wake-ups asked for.
        pub fn wake_at(&mut self, t: Time) {
            self.wake = Some(self.wake.map_or(t, |w| w.min(t)));
        }
    }

    /// Stands in for `presto_core::A11yCx`.
    pub struct A11yCx<'a> {
        pub node: &'a mut accesskit::Node,
        pub state: StateBits,
        pub size: Size,
    }

    /// Stands in for `presto_core::Kind` (architecture §4.2). Prop storage is the kind's own: core calls
    /// [`Kind::set_prop`]/[`Kind::get_prop`] by spec index (presto-core's API for this is not fixed yet).
    pub trait Kind: 'static {
        fn spec() -> &'static KindSpec
        where
            Self: Sized;
        fn kind_spec(&self) -> &'static KindSpec;
        fn set_prop(&mut self, prop: PropId, v: &Value) -> Result<(), PropError>;
        fn get_prop(&self, prop: PropId) -> Option<Value>;
        fn layout_style(&self, cx: &StyleCx) -> taffy::Style {
            cx.default_layout()
        }
        fn measure(&self, _cx: &mut MeasureCx, _known: taffy::Size<Option<f32>>, _avail: taffy::Size<taffy::AvailableSpace>) -> taffy::Size<f32> {
            taffy::Size::ZERO
        }
        /// Stands in for presto-core's padded layout (svelters-egui `LayoutCx::padded_layout`): room
        /// `[left, right, top, bottom]` in points for chrome the kind paints itself (a frame's
        /// border and caption row), which core adds to the length parts of the padding of
        /// [`Kind::layout_style`] before laying out the children. `None`: nothing to add. It reads
        /// the measure context because the chrome can hold text.
        fn chrome_padding(&self, _cx: &mut MeasureCx) -> Option<[f32; 4]> {
            None
        }
        /// Stands in for presto-core's per-child layout hook: a container that places or hides its
        /// children itself (`stack`) adjusts child `cx.index`'s taffy style, after core mapped the
        /// child's computed style. Returns whether the child is shown (painted, hit and in the
        /// accessibility tree); a child that is not shown but keeps its layout still sizes the
        /// container.
        fn child_layout(&self, _cx: &ChildCx, _style: &mut taffy::Style) -> bool {
            true
        }
        /// Stands in for presto-core's subtree enablement: `true` makes every descendant inert (no
        /// input, `:disabled`), as an unchecked `checkable` frame does to its content.
        fn children_inert(&self) -> bool {
            false
        }
        fn prepare(&mut self, _cx: &mut PrepareCx) {}
        fn record(&self, cx: &mut RecordCx);
        fn hit_test(&self, cx: &HitCx, p: Point) -> HitResult {
            cx.box_hit(p)
        }
        fn event(&mut self, _cx: &mut EventCx, _ev: &Event) -> Handled {
            Handled::No
        }
        fn a11y(&self, cx: &mut A11yCx);
        /// EV3: while focused, whether the kind takes `key` with `m` itself (keymaps and Tab skip it).
        fn claims_key(&self, _key: presto_base::key::Key, _m: presto_base::event::Modifiers) -> bool {
            false
        }
    }

    /// Checks a value against a prop's spec type (what `set_prop` implementations call first).
    pub fn check(spec: &PropSpec, v: &Value) -> Result<(), PropError> {
        let ok = matches!(
            (spec.ty, v),
            (ValueType::Bool, Value::Bool(_))
                | (ValueType::I64, Value::I64(_))
                | (ValueType::F64, Value::F64(_) | Value::I64(_))
                | (ValueType::Str, Value::Str(_))
                | (ValueType::Color, Value::Color(_))
                | (ValueType::List, Value::List(_))
                | (ValueType::Record, Value::Record(_))
        ) || matches!((spec.ty, v), (ValueType::Enum(names), Value::Enum(s) ) if names.contains(s))
            || matches!((spec.ty, v), (ValueType::Enum(names), Value::Str(s)) if names.contains(&&**s));
        if ok { Ok(()) } else { Err(PropError::Type { prop: spec.name, expected: spec.ty }) }
    }

    /// An enum prop's value as one of its spec names (a `Str` with a listed name counts).
    pub fn enum_name(spec: &PropSpec, v: &Value) -> Option<&'static str> {
        let ValueType::Enum(names) = spec.ty else { return None };
        let s = v.as_str()?;
        names.iter().copied().find(|n| *n == s)
    }
}

pub mod text {
    //! Stand-in for presto-text's layout API (b1 brief "presto-text"): `RichText` in, `TextLayout` with
    //! placed glyph runs out. Colour is not part of shaping: each placed run carries a style index, and
    //! the recorder colours it.

    use presto_base::geom::{Point, Size};
    use presto_base::text::GlyphRun;
    use std::ops::Range;
    use std::sync::Arc;

    /// Stands in for `presto_text::TextStyle`.
    #[derive(Clone, Debug, PartialEq)]
    pub struct TextStyle {
        /// CSS `font-family` list.
        pub family: Arc<str>,
        pub size: f32,
        pub weight: u16,
        pub italic: bool,
        /// Line height in points (`None`: the font's normal).
        pub line_height: Option<f32>,
        pub letter_spacing: f32,
        pub underline: bool,
        pub strikethrough: bool,
    }

    impl std::default::Default for TextStyle {
        fn default() -> Self {
            TextStyle { family: Arc::from("system-ui, sans-serif"), size: 13.0, weight: 400, italic: false, line_height: None, letter_spacing: 0.0, underline: false, strikethrough: false }
        }
    }

    /// Stands in for `presto_text::Span`: a byte range of the text in another style.
    #[derive(Clone, Debug, PartialEq)]
    pub struct Span {
        pub range: Range<usize>,
        pub style: TextStyle,
    }

    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum TextAlign {
        #[default]
        Start,
        Center,
        End,
    }

    /// Stands in for `presto_text::RichText`.
    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct RichText {
        pub text: String,
        /// Styled ranges, in order, not overlapping; style index `i + 1` in [`PlacedRun::style`].
        pub spans: Vec<Span>,
        /// Style index 0.
        pub base: TextStyle,
        pub align: TextAlign,
        /// Cut the last line with `…` when it does not fit.
        pub ellipsis: bool,
        pub max_lines: Option<u32>,
    }

    impl RichText {
        pub fn plain(text: &str, base: TextStyle) -> RichText {
            RichText { text: text.to_owned(), base, ..RichText::default() }
        }
    }

    /// Stands in for `presto_text::PlacedRun`: a shaped run placed in the layout (`origin` = baseline
    /// start), with the index of its style (0 = base, `i + 1` = `spans[i]`).
    #[derive(Clone, Debug, PartialEq)]
    pub struct PlacedRun {
        pub run: Arc<GlyphRun>,
        pub origin: Point,
        pub style: u32,
    }

    /// Stands in for `presto_text::TextLayout`.
    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct TextLayout {
        pub size: Size,
        pub runs: Vec<PlacedRun>,
        /// Number of laid-out lines.
        pub lines: u32,
    }

    impl TextLayout {
        pub fn size(&self) -> Size {
            self.size
        }
        pub fn runs(&self) -> &[PlacedRun] {
            &self.runs
        }
    }

    /// Stands in for `presto_text::TextSystem::layout`.
    pub trait TextShaper {
        fn layout(&mut self, text: &RichText, max_width: Option<f32>) -> Arc<TextLayout>;
    }
}

pub mod testing {
    //! Test doubles: a deterministic monospace shaper (every char advances `0.6 × size`, glyph id = the
    //! char's code point, one run per line) and an in-memory [`Resources`]. For tests only.

    use super::core::Resources;
    use super::text::{PlacedRun, RichText, TextLayout, TextShaper};
    use presto_base::commit::ImageData;
    use presto_base::geom::{Point, Size};
    use presto_base::ids::{FontId, ImageId, RunId};
    use presto_base::text::{AaMode, Glyph, GlyphRun, HintMode};
    use std::collections::HashMap;
    use std::sync::Arc;

    #[derive(Default)]
    pub struct MonoShaper;

    impl MonoShaper {
        pub fn advance(size: f32) -> f32 {
            (size * 0.6 * 4.0).round() / 4.0
        }
    }

    impl TextShaper for MonoShaper {
        fn layout(&mut self, t: &RichText, max_width: Option<f32>) -> Arc<TextLayout> {
            let size = t.base.size;
            let adv = Self::advance(size);
            let line_h = t.base.line_height.unwrap_or((size * 1.25).round());
            let per_line = max_width.map(|w| ((w / adv).floor() as usize).max(1));
            let mut lines: Vec<String> = Vec::new();
            for para in t.text.split('\n') {
                let chars: Vec<char> = para.chars().collect();
                match per_line {
                    Some(n) if chars.len() > n => lines.extend(chars.chunks(n).map(|c| c.iter().collect())),
                    _ => lines.push(para.to_owned()),
                }
            }
            if let Some(m) = t.max_lines {
                lines.truncate(m.max(1) as usize);
            }
            let mut runs = Vec::new();
            let mut width: f32 = 0.0;
            for (i, line) in lines.iter().enumerate() {
                let glyphs: Vec<Glyph> = line.chars().enumerate().map(|(k, c)| Glyph { id: c as u32, x: k as f32 * adv, y: 0.0 }).collect();
                let advance = glyphs.len() as f32 * adv;
                width = width.max(advance);
                let run = GlyphRun { font: FontId(0), size, variations: 0, glyphs, ascent: size * 0.8, descent: size * 0.2, advance, hint: HintMode::Vertical, aa: AaMode::Grayscale, embolden: 0.0, skew_deg: 0.0 };
                let baseline = i as f32 * line_h + (line_h - size) / 2.0 + size * 0.8;
                runs.push(PlacedRun { run: Arc::new(run), origin: Point::new(0.0, baseline), style: 0 });
            }
            Arc::new(TextLayout { size: Size::new(width, line_h * lines.len() as f32), runs, lines: lines.len() as u32 })
        }
    }

    /// Resources kept in memory: run ids count up from 1, image ids from 1 per new key.
    #[derive(Default)]
    pub struct MemResources {
        pub runs: Vec<Arc<GlyphRun>>,
        pub images: HashMap<String, (ImageId, ImageData)>,
    }

    impl Resources for MemResources {
        fn run(&mut self, run: &Arc<GlyphRun>) -> RunId {
            if let Some(i) = self.runs.iter().position(|r| Arc::ptr_eq(r, run)) {
                return RunId(i as u64 + 1);
            }
            self.runs.push(run.clone());
            RunId(self.runs.len() as u64)
        }
        fn image(&mut self, key: &str, make: &mut dyn FnMut() -> Option<ImageData>) -> Option<ImageId> {
            if let Some((id, _)) = self.images.get(key) {
                return Some(*id);
            }
            let data = make()?;
            let id = ImageId(self.images.len() as u64 + 1);
            self.images.insert(key.to_owned(), (id, data));
            Some(id)
        }
    }
}
