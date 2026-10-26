//! The bridge from the built-in kinds to skelter-core: [`Hosted<W>`] implements [`skelter_core::Kind`] for
//! any widget written against this crate's kind interface ([`crate::shim::core::Kind`]: typed props kept
//! by the kind, the egui-semantics [`Tracker`](crate::interact::Tracker) fed from skelter events), and
//! [`register`] adds it to a [`KindRegistry`]. The document's services carry what the kinds share: the
//! text system (`skelter_text::TextSystem`, without which kinds measure and record no text), the platform
//! [`Policy`] (default: the OS's row) and the [`Theme`](crate::theme::Theme) whose scheme UA tokens
//! resolve in (default: dark).
//!
//! What the bridge maps:
//! - the spec: this crate's [`KindSpec`](crate::shim::core::KindSpec) becomes skelter-core's (built once
//!   per type and kept for the process); props skelter-core declares for every kind (`disabled`) are left
//!   to it and read from the node;
//! - prop writes: [`Kind::prop_changed`] hands the stored value (or the default) to the widget; the
//!   widget's native changes (`checked` after a click) go back through [`EventCx::set_native`];
//! - events: pointer down/up/move, hover, wheel, keys, focus and cancel become this crate's events
//!   (skelter-core's own `Click` is not used: the tracker decides clicks with egui's rules); the pointer
//!   is captured on press, as egui's interaction does;
//! - state: skelter-core owns `:hover`, `:active`, `:focus`, `:focus-visible` and `:focus-within`; the
//!   widget's other state bits (`:checked`, `:indeterminate`, `:selected`, `:dragging`, …) are written
//!   to the node;
//! - images: uploaded in [`Kind::prepare`] (skelter-core's image service), referenced when recording;
//! - the hooks skelter-core added for the kinds: paint-part styles (`::part(name)`), listened signals,
//!   signals delivered at event time (a prevented `clicked` skips its default), peer groups (radio
//!   exclusivity, roving focus), container hooks (chrome padding, per-child layout, inert children),
//!   prop-derived state bits, the device scale, `label for` focus, light dismiss (a press outside an
//!   open surface reaches the widget as `Event::OutsidePress`).

use crate::controls::run::{self, RunNode};
use crate::interact::{FocusReason as WFocus, Native, Policy, StateBits};
use crate::shim::core as w;
use crate::shim::text::{PlacedRun, RichText, TextLayout, TextShaper};
use skelter_base::commit::ImageData;
use skelter_base::display::{DisplayList, Prim};
use skelter_base::event::{PointerEvent, PointerPhase, WheelEvent};
use skelter_base::geom::{Insets, Point, Vec2};
use skelter_base::ids::{ImageId, NodeId, RunId};
use skelter_base::text::GlyphRun;
use skelter_core::{
    A11yCx, ChildLayoutCx, Event, EventCx, NodeView, PeerGroup, PeerMsg, PeerTarget, Rove as CoreRove, SignalId, FocusReason, Handled, HitCx, HitResult, Kind, KindRegistry, KindSpec, MeasureCx, PrepareCx, PropCx, PropDefault, PropError, PropId, PropSpec, RecordCx, RegistryError,
    MethodId, SignalFlags, SignalSpec, SlotSpec, StateMask, StyleCx, Value, ValueType,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

// ---- services ----

/// The policy the kinds follow when the document has none as a service: the OS's row.
pub fn os_policy() -> Policy {
    *Native::for_os(os()).policy()
}

fn os() -> crate::interact::Platform {
    if cfg!(target_os = "macos") {
        crate::interact::Platform::Macos
    } else if cfg!(target_os = "windows") {
        crate::interact::Platform::Windows
    } else {
        crate::interact::Platform::Linux
    }
}

/// A text shaper that has nothing to shape with: empty layouts (no system installed).
struct NoText;

impl TextShaper for NoText {
    fn layout(&mut self, _text: &RichText, _max_width: Option<f32>) -> Arc<TextLayout> {
        Arc::new(TextLayout::default())
    }
}

fn shaper(ts: Option<&mut skelter_text::TextSystem>) -> Box<dyn TextShaper + '_> {
    match ts {
        Some(ts) => Box::new(ts),
        None => Box::new(NoText),
    }
}

impl TextShaper for &mut skelter_text::TextSystem {
    fn layout(&mut self, t: &RichText, max_width: Option<f32>) -> Arc<TextLayout> {
        (**self).layout_rich(t, max_width)
    }
}

/// skelter-text's text system as this crate's shaper: styles map one to one (line height in points
/// becomes a multiple of the size), and each span's paint style is its index (0 = base, `i + 1` =
/// `spans[i]`).
impl TextShaper for skelter_text::TextSystem {
    fn layout(&mut self, t: &RichText, max_width: Option<f32>) -> Arc<TextLayout> {
        self.layout_rich(t, max_width)
    }
}

/// [`TextShaper::layout`] for skelter-text's system, as a named function so `&mut TextSystem` shares it.
trait LayoutRich {
    fn layout_rich(&mut self, t: &RichText, max_width: Option<f32>) -> Arc<TextLayout>;
}

impl LayoutRich for skelter_text::TextSystem {
    fn layout_rich(&mut self, t: &RichText, max_width: Option<f32>) -> Arc<TextLayout> {
        let style = |s: &crate::shim::text::TextStyle, paint: u32| skelter_text::TextStyle {
            family: s.family.to_string(),
            size: s.size,
            weight: s.weight,
            italic: s.italic,
            line_height: s.line_height.filter(|_| s.size > 0.0).map(|l| l / s.size),
            letter_spacing: s.letter_spacing,
            paint,
            ..skelter_text::TextStyle::default()
        };
        let rich = skelter_text::RichText {
            text: t.text.clone(),
            spans: t.spans.iter().enumerate().map(|(i, s)| skelter_text::Span { range: s.range.clone(), style: style(&s.style, i as u32 + 1) }).collect(),
            base: style(&t.base, 0),
            align: match t.align {
                crate::shim::text::TextAlign::Start => skelter_text::Align::Start,
                crate::shim::text::TextAlign::Center => skelter_text::Align::Center,
                crate::shim::text::TextAlign::End => skelter_text::Align::End,
            },
            ellipsis: t.ellipsis,
            max_lines: t.max_lines,
            tab_size: None,
        };
        let l = skelter_text::TextSystem::layout(self, &rich, max_width);
        let runs = l.runs().iter().map(|r| PlacedRun { run: r.run.clone(), origin: r.origin, style: r.style }).collect();
        let geometry = Some(crate::shim::text::Geometry(Arc::new(SkelterGeometry(l.clone()))));
        Arc::new(TextLayout { size: l.size(), runs, lines: l.lines().len() as u32, geometry })
    }
}

/// skelter-text's layout as caret and selection geometry.
struct SkelterGeometry(Arc<skelter_text::TextLayout>);

impl crate::shim::text::TextGeometry for SkelterGeometry {
    fn hit_byte(&self, p: Point) -> usize {
        self.0.hit_test(p).byte
    }
    fn selection_rects(&self, range: std::ops::Range<usize>) -> Vec<skelter_base::geom::Rect> {
        self.0.selection_rects(range)
    }
}

// ---- specs ----

/// skelter-core's spec for a widget, and the maps between the two prop index spaces.
struct Spec {
    core: &'static KindSpec,
    /// For each of the widget's props: its index in `core.props`, or `None` for a prop skelter-core
    /// declares for every kind (`disabled`).
    to_core: Vec<Option<usize>>,
    /// The widget's `disabled` prop, fed from the node's state.
    disabled: Option<w::PropId>,
}

fn core_type(t: w::ValueType) -> ValueType {
    match t {
        w::ValueType::Bool => ValueType::Bool,
        w::ValueType::I64 => ValueType::I64,
        w::ValueType::F64 => ValueType::F64,
        w::ValueType::Str => ValueType::Str,
        w::ValueType::Enum(_) => ValueType::Enum,
        w::ValueType::Color => ValueType::Color,
        w::ValueType::List => ValueType::List,
        w::ValueType::Record => ValueType::Record,
    }
}

fn core_default(p: &w::PropSpec) -> PropDefault {
    match p.default {
        w::Default::Bool(b) => PropDefault::Bool(b),
        w::Default::I64(v) => PropDefault::I64(v),
        w::Default::F64(v) => PropDefault::F64(v),
        w::Default::Str(s) => PropDefault::Str(s),
        w::Default::Enum(name) => match p.ty {
            w::ValueType::Enum(names) => names.iter().position(|n| *n == name).map_or(PropDefault::Unset, |i| PropDefault::Enum(i as u32)),
            _ => PropDefault::Str(name),
        },
        w::Default::None => PropDefault::Unset,
    }
}

fn is_common(name: &str) -> bool {
    skelter_core::common::COMMON.props.iter().any(|p| p.name == name)
}

/// What a widget type declares beyond its spec (the shim `Kind`'s associated functions).
#[derive(Clone, Copy)]
struct Traits {
    prop_states: StateBits,
    parts: &'static [&'static str],
    places_children: bool,
    folds_children: bool,
    folds_elements: bool,
    methods: &'static [skelter_core::MethodSpec],
}

impl Traits {
    fn of<W: w::Kind>() -> Traits {
        Traits { prop_states: W::prop_states(), parts: W::parts(), places_children: W::places_children(), folds_children: W::folds_children() || W::folds_elements(), folds_elements: W::folds_elements(), methods: W::methods() }
    }
}

fn spec_of(ws: &'static w::KindSpec, t: Traits) -> &'static Spec {
    static SPECS: OnceLock<Mutex<HashMap<usize, &'static Spec>>> = OnceLock::new();
    let key = ws as *const w::KindSpec as usize;
    let mut map = SPECS.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    map.entry(key).or_insert_with(|| {
        let mut props: Vec<PropSpec> = Vec::new();
        let mut to_core = Vec::new();
        let mut disabled = None;
        for (i, p) in ws.props.iter().enumerate() {
            if is_common(p.name) {
                if p.name == "disabled" {
                    disabled = Some(w::PropId(i as u32));
                }
                to_core.push(None);
                continue;
            }
            let mut c = PropSpec::new(p.name, core_type(p.ty), p.change).default(core_default(p));
            if let w::ValueType::Enum(names) = p.ty {
                c = c.enum_values(names);
            }
            if p.bindable {
                c = c.native();
            }
            to_core.push(Some(props.len()));
            props.push(c);
        }
        let signals: Vec<SignalSpec> = ws
            .signals
            .iter()
            .map(|s| {
                let arg = match s.args {
                    [] => None,
                    [(_, t)] => Some(core_type(*t)),
                    _ => Some(ValueType::Record),
                };
                let mut f = SignalFlags::NONE;
                if s.passive {
                    f = f.union(SignalFlags::PASSIVE);
                }
                if s.cancelable {
                    f = f.union(SignalFlags::CANCELABLE);
                }
                if s.sync {
                    f = f.union(SignalFlags::SYNC);
                }
                SignalSpec::new(s.name, arg, f)
            })
            .collect();
        let slots: Vec<SlotSpec> = ws.slots.iter().map(|n| SlotSpec { name: n, single: false }).collect();
        let core = Box::leak(Box::new(KindSpec {
            name: ws.tag,
            props: Box::leak(props.into_boxed_slice()),
            signals: Box::leak(signals.into_boxed_slice()),
            slots: Box::leak(slots.into_boxed_slice()),
            role: ws.role,
            text_prop: ws.text_prop,
            focusable: ws.focusable,
            container: ws.children,
            folds_children: t.folds_children,
            parts: t.parts,
            places_children: t.places_children,
            prop_states: StateMask(t.prop_states.0 & !CORE_BITS.0),
            methods: t.methods,
            ..KindSpec::new(ws.tag)
        }));
        Box::leak(Box::new(Spec { core, to_core, disabled }))
    })
}

// ---- values ----

fn to_core_value(p: &w::PropSpec, v: &w::Value) -> Option<Value> {
    Some(match v {
        w::Value::Bool(b) => Value::Bool(*b),
        w::Value::I64(i) => Value::I64(*i),
        w::Value::F64(f) => Value::F64(*f),
        w::Value::Str(s) => match p.ty {
            w::ValueType::Enum(names) => Value::Enum(names.iter().position(|n| **n == **s)? as u32),
            _ => Value::Str(s.clone()),
        },
        w::Value::Enum(name) => match p.ty {
            w::ValueType::Enum(names) => Value::Enum(names.iter().position(|n| n == name)? as u32),
            _ => Value::Str(Arc::from(*name)),
        },
        w::Value::Color(c) => Value::Color(*c),
        w::Value::List(l) => Value::List(l.iter().filter_map(plain_to_core).collect::<Vec<_>>().into()),
        w::Value::Record(r) => Value::Record(r.iter().filter_map(|(k, v)| Some((k.clone(), plain_to_core(v)?))).collect::<Vec<_>>().into()),
    })
}

/// A value with no spec to resolve enum names against (list items, record fields, signal args).
fn plain_to_core(v: &w::Value) -> Option<Value> {
    Some(match v {
        w::Value::Bool(b) => Value::Bool(*b),
        w::Value::I64(i) => Value::I64(*i),
        w::Value::F64(f) => Value::F64(*f),
        w::Value::Str(s) => Value::Str(s.clone()),
        w::Value::Enum(s) => Value::Str(Arc::from(*s)),
        w::Value::Color(c) => Value::Color(*c),
        w::Value::List(l) => Value::List(l.iter().filter_map(plain_to_core).collect::<Vec<_>>().into()),
        w::Value::Record(r) => Value::Record(r.iter().filter_map(|(k, v)| Some((k.clone(), plain_to_core(v)?))).collect::<Vec<_>>().into()),
    })
}

fn from_core_value(p: &w::PropSpec, v: &Value) -> Option<w::Value> {
    Some(match v {
        Value::Bool(b) => w::Value::Bool(*b),
        Value::I64(i) => w::Value::I64(*i),
        Value::F64(f) => w::Value::F64(*f),
        Value::Str(s) => w::Value::Str(s.clone()),
        Value::Enum(i) => match p.ty {
            w::ValueType::Enum(names) => w::Value::Enum(names.get(*i as usize)?),
            _ => return None,
        },
        Value::Color(c) => w::Value::Color(*c),
        Value::List(l) => w::Value::List(l.iter().filter_map(plain_from_core).collect()),
        Value::Record(r) => w::Value::Record(r.iter().filter_map(|(k, v)| Some((k.clone(), plain_from_core(v)?))).collect()),
    })
}

fn plain_from_core(v: &Value) -> Option<w::Value> {
    Some(match v {
        Value::Bool(b) => w::Value::Bool(*b),
        Value::I64(i) => w::Value::I64(*i),
        Value::F64(f) => w::Value::F64(*f),
        Value::Str(s) => w::Value::Str(s.clone()),
        Value::Enum(i) => w::Value::I64(*i as i64),
        Value::Color(c) => w::Value::Color(*c),
        Value::List(l) => w::Value::List(l.iter().filter_map(plain_from_core).collect()),
        Value::Record(r) => w::Value::Record(r.iter().filter_map(|(k, v)| Some((k.clone(), plain_from_core(v)?))).collect()),
    })
}

/// A signal's arguments as skelter-core's one argument: none, the one value, or a record by the spec's
/// argument names.
fn signal_arg(s: &w::SignalSpec, args: &[w::Value]) -> Option<Value> {
    match args {
        [] => None,
        [v] => plain_to_core(v),
        many => Some(Value::Record(s.args.iter().zip(many).filter_map(|((n, _), v)| Some((Arc::from(*n), plain_to_core(v)?))).collect::<Vec<_>>().into())),
    }
}

// ---- the hosted kind ----

/// The node's part styles, for [`w::Styles::parts`].
fn part_styles<'a>(node: NodeView<'a>, parts: &[&'static str]) -> Vec<(&'static str, &'a svelters_style::ComputedStyle)> {
    parts.iter().filter_map(|n| Some((*n, node.part(n)?))).collect()
}

/// Installs what the built-in kinds read from `doc`'s services: the text system, the platform policy
/// (and its auto-default rule) and the theme.
pub fn install(doc: &mut skelter_core::Document, text: skelter_text::TextSystem, policy: Policy, theme: crate::theme::Theme) {
    doc.set_service(text);
    doc.set_auto_default(policy.auto_default);
    doc.set_service(policy);
    doc.set_service(theme);
}

/// A folding node's run children as `runs` records; empty for other kinds.
fn folded_of(node: NodeView<'_>, folds: bool, elements: bool) -> Vec<w::Value> {
    if !folds {
        return Vec::new();
    }
    if elements {
        return element_records(&|id| node.view(id), node.children());
    }
    run::fold(&run_nodes(&|id| node.view(id), node.children(), css_color(node.css(), "color")))
}

/// The children as element records (`Kind::folds_elements`): `{tag, anchor, props, children}`, each prop
/// of the child's kind by name with its value or default.
fn element_records<'v>(view_of: &dyn Fn(NodeId) -> Option<NodeView<'v>>, ids: &[NodeId]) -> Vec<w::Value> {
    ids.iter()
        .filter_map(|&id| {
            let v = view_of(id)?;
            let spec = v.kind().spec;
            let props: Vec<(Arc<str>, w::Value)> = (0..spec.props.len())
                .filter_map(|i| {
                    let p = &spec.props[i];
                    let val = v.prop(spec.prop_id(i))?;
                    let wv = match val {
                        Value::Enum(e) => w::Value::Str(Arc::from(*p.enum_values.get(*e as usize)?)),
                        other => plain_from_core(other)?,
                    };
                    Some((Arc::from(p.name), wv))
                })
                .collect();
            Some(w::Value::Record(vec![
                (Arc::from("tag"), w::Value::str(spec.name)),
                (Arc::from("anchor"), w::Value::str(v.anchor())),
                (Arc::from("props"), w::Value::Record(props)),
                (Arc::from("children"), w::Value::List(element_records(view_of, v.children()))),
            ]))
        })
        .collect()
}

/// The state bits skelter-core writes itself; the bridge never writes them back.
const CORE_BITS: StateBits = StateBits(StateBits::HOVERED.0 | StateBits::PRESSED.0 | StateBits::FOCUSED.0 | StateBits::FOCUS_VISIBLE.0 | StateBits::FOCUS_WITHIN.0 | StateBits::DISABLED.0);

/// A widget hosted as a skelter-core kind (see the module docs).
pub struct Hosted<W: w::Kind + Default> {
    pub widget: W,
    /// Images uploaded in `prepare`, by the widget's image key.
    images: HashMap<String, ImageId>,
}

impl<W: w::Kind + Default> Default for Hosted<W> {
    fn default() -> Self {
        Hosted { widget: W::default(), images: HashMap::new() }
    }
}

/// Registers widget `W` in `registry`.
pub fn register<W: w::Kind + Default>(registry: &mut KindRegistry) -> Result<skelter_core::KindId, RegistryError> {
    fn make<W: w::Kind + Default>() -> Box<dyn Kind> {
        Box::new(Hosted::<W>::default())
    }
    registry.register_with(spec_of(W::spec(), Traits::of::<W>()).core, make::<W>)
}

/// Registers every Tier 0 kind of [`crate::controls`] in `registry`.
pub fn register_tier0(registry: &mut KindRegistry) -> Result<(), RegistryError> {
    use crate::controls::*;
    register::<FlexBox>(registry)?;
    register::<Button>(registry)?;
    register::<Checkbox>(registry)?;
    register::<Frame>(registry)?;
    register::<Grid>(registry)?;
    register::<Hyperlink>(registry)?;
    register::<Icon>(registry)?;
    register::<Image>(registry)?;
    register::<Label>(registry)?;
    register::<ProgressBar>(registry)?;
    register::<RadioButton>(registry)?;
    register::<Separator>(registry)?;
    register::<Sides>(registry)?;
    register::<Slider>(registry)?;
    register::<Spinner>(registry)?;
    register::<Stack>(registry)?;
    register::<Switch>(registry)?;
    register::<ToggleButton>(registry)?;
    register::<Span>(registry)?;
    register::<Bold>(registry)?;
    register::<Italic>(registry)?;
    register::<Code>(registry)?;
    register::<Anchor>(registry)?;
    Ok(())
}

fn styles<'a>(css: Option<&'a svelters_style::ComputedStyle>, parts: &'a [(&'static str, &'a svelters_style::ComputedStyle)], children: &'a [w::Value], state: StateMask, dark: bool) -> w::Styles<'a> {
    w::Styles { computed: css, parts, children, state: StateBits(state.0), dark }
}

/// A computed colour of `css`.
fn css_color(css: Option<&svelters_style::ComputedStyle>, name: &str) -> Option<skelter_base::color::Color> {
    match css?.get(name)? {
        svelters_style::ComputedValue::Color(c) => Some(crate::paint::color_of(*c)),
        _ => None,
    }
}

/// The run children under `ids` (`controls::run`), read through `view_of`, with what their computed
/// style sets: a colour other than `parent`'s, a weight, italic.
fn run_nodes<'v>(view_of: &dyn Fn(NodeId) -> Option<NodeView<'v>>, ids: &[NodeId], parent: Option<skelter_base::color::Color>) -> Vec<RunNode> {
    ids.iter()
        .filter_map(|&id| {
            let v = view_of(id)?;
            let spec = v.kind().spec;
            let css = v.css();
            let href = spec.prop_named("href").and_then(|p| v.prop_str(p)).unwrap_or("").to_owned();
            let prop_weight = spec.prop_named("font-weight").and_then(|p| v.prop(p)).and_then(Value::as_i64).unwrap_or(0);
            let css_weight = match css.and_then(|c| c.get("font-weight")) {
                Some(svelters_style::ComputedValue::FontWeight(w)) => *w as i64,
                _ => 0,
            };
            let italic = matches!(css.and_then(|c| c.get("font-style")), Some(svelters_style::ComputedValue::Keyword("italic" | "oblique")));
            let own = css_color(css, "color");
            let color = own.filter(|c| Some(*c) != parent);
            let children = run_nodes(view_of, v.children(), own.or(parent));
            Some(RunNode { tag: spec.name.to_owned(), text: v.text().to_owned(), href, weight: prop_weight.max(css_weight), italic, color, children })
        })
        .collect()
}

fn is_dark(theme: Option<&mut crate::theme::Theme>) -> bool {
    theme.is_none_or(|t| t.is_dark())
}

/// Resources for the prepare pass: new images are rasterised and kept for upload after the pass;
/// runs are not kept.
struct PrepareRes<'a> {
    images: &'a HashMap<String, ImageId>,
    new: Vec<(String, ImageData)>,
    used: Vec<String>,
}

impl w::Resources for PrepareRes<'_> {
    fn run(&mut self, _run: &Arc<GlyphRun>) -> RunId {
        RunId(0)
    }
    fn image(&mut self, key: &str, make: &mut dyn FnMut() -> Option<ImageData>) -> Option<ImageId> {
        self.used.push(key.to_owned());
        if let Some(id) = self.images.get(key) {
            return Some(*id);
        }
        if !self.new.iter().any(|(k, _)| k == key) {
            self.new.push((key.to_owned(), make()?));
        }
        // the dry pass's lists are dropped: any id will do
        Some(ImageId(0))
    }
}

/// Resources for recording: runs are kept (by index) and handed to skelter-core after the pass; images
/// are the ones `prepare` uploaded.
struct RecordRes<'a> {
    runs: Vec<Arc<GlyphRun>>,
    images: &'a HashMap<String, ImageId>,
}

impl w::Resources for RecordRes<'_> {
    fn run(&mut self, run: &Arc<GlyphRun>) -> RunId {
        self.runs.push(run.clone());
        RunId(self.runs.len() as u64)
    }
    fn image(&mut self, key: &str, _make: &mut dyn FnMut() -> Option<ImageData>) -> Option<ImageId> {
        self.images.get(key).copied()
    }
}

impl<W: w::Kind + Default> Hosted<W> {
    fn spec_ref() -> &'static Spec {
        spec_of(W::spec(), Traits::of::<W>())
    }
    fn traits() -> Traits {
        Traits::of::<W>()
    }

    fn pointer_event(phase: PointerPhase, p: &skelter_core::PointerInput) -> w::Event {
        w::Event::Pointer(PointerEvent { phase, pointer: p.pointer, kind: p.kind, button: p.button, buttons: p.buttons, pos: p.local, modifiers: p.modifiers, pressure: p.pressure, tilt: Vec2::ZERO, history: Default::default() })
    }

    fn translate(ev: &Event) -> Option<w::Event> {
        Some(match ev {
            Event::PointerDown(p) => Self::pointer_event(PointerPhase::Down, p),
            Event::PointerUp(p) => Self::pointer_event(PointerPhase::Up, p),
            Event::PointerMove(p) => Self::pointer_event(PointerPhase::Move, p),
            Event::PointerCancel => w::Event::CaptureLost,
            Event::HoverEnter => w::Event::Hover(true),
            Event::HoverLeave => w::Event::Hover(false),
            Event::Wheel(wh) => w::Event::Wheel(WheelEvent { pos: wh.local, delta: wh.delta, phase: None, momentum: None, modifiers: wh.modifiers }),
            Event::Key(k) => w::Event::Key((*k).clone()),
            Event::FocusIn(r) => w::Event::Focus {
                gained: true,
                reason: match r {
                    FocusReason::Pointer => WFocus::Pointer,
                    FocusReason::Keyboard => WFocus::Keyboard,
                    FocusReason::Programmatic => WFocus::Program,
                },
            },
            Event::FocusOut => w::Event::Focus { gained: false, reason: WFocus::Program },
            Event::Timer => w::Event::Timer,
            Event::A11y(r) => w::Event::A11y((*r).clone()),
            // clicks are the tracker's; IME is the text kinds'
            Event::Click(_) | Event::Ime(_) => return None,
        })
    }

    /// Runs `f` (an event or a peer message) on the widget with this crate's event context over `cx`,
    /// then applies what it did: state bits, native prop writes, signals (cancelable ones at event time
    /// through `emit_sync`), peer requests, focus, timers, cursor and repaint.
    fn run(&mut self, cx: &mut EventCx, f: impl FnOnce(&mut W, &mut w::EventCx) -> w::Handled) -> Handled {
        let node = cx.node();
        let Some(view) = cx.view(node) else { return Handled::No };
        let (size, mask) = (view.size(), view.state());
        self.sync_disabled(view.disabled());
        let policy = cx.service::<Policy>().copied().unwrap_or_else(os_policy);
        let s = Self::spec_ref();
        let ws = W::spec();
        let mut state = StateBits(mask.0);
        let before = state;
        let time = cx.time();
        let (handled, out) = {
            let mut sync = |sig: w::SignalId, args: &[w::Value]| -> bool {
                let i = sig.0 as usize;
                let Some(spec) = ws.signals.get(i) else { return false };
                cx.emit_sync(s.core.signal_id(i), signal_arg(spec, args)).prevented
            };
            let mut wcx = w::EventCx::new(time, size, &mut state, &policy);
            wcx.focused = mask.contains(StateMask::FOCUSED);
            wcx.sync = Some(&mut sync);
            let handled = f(&mut self.widget, &mut wcx);
            let w::EventCx { emitted, changed, focus_request, cursor, repaint, wake, peers, focus_anchor, scroll_to, emitted_on, .. } = wcx;
            (handled, (emitted, changed, focus_request, cursor, repaint, wake, peers, focus_anchor, scroll_to, emitted_on))
        };
        let (emitted, changed, focus_request, cursor, repaint, wake, peers, focus_anchor, scroll_to, emitted_on) = out;
        let diff = StateBits((state.0 ^ before.0) & !CORE_BITS.0);
        for (_, bit) in StateBits::BUILTIN {
            if diff.contains(*bit) {
                cx.set_state(StateMask(bit.0), state.contains(*bit));
            }
        }
        for p in changed {
            let i = p.0 as usize;
            let (Some(Some(ci)), Some(v)) = (s.to_core.get(i), self.widget.get_prop(p)) else { continue };
            if let Some(cv) = to_core_value(&ws.props[i], &v) {
                let _ = cx.set_native(s.core.prop_id(*ci), cv);
            }
        }
        if let Some(o) = scroll_to {
            cx.scroll_fast(o);
            let rec: Vec<(Arc<str>, Value)> = vec![("scrollLeft".into(), Value::F64(o.x as f64)), ("scrollTop".into(), Value::F64(o.y as f64))];
            cx.emit(skelter_core::common::SCROLL, Some(Value::Record(rec.into())));
        }
        for e in emitted {
            let i = e.signal.0 as usize;
            let Some(sig) = ws.signals.get(i) else { continue };
            cx.emit(s.core.signal_id(i), signal_arg(sig, &e.args));
        }
        if !emitted_on.is_empty() {
            let children: Vec<NodeId> = cx.view(node).map(|v| v.children().to_vec()).unwrap_or_default();
            for (k, sig, arg) in emitted_on {
                if let Some(&c) = children.get(k) {
                    cx.emit_on(c, sig, arg);
                }
            }
        }
        for r in peers {
            let to = match r.to {
                w::PeerTarget::Others => PeerTarget::Others,
                w::PeerTarget::Rove(rv) => PeerTarget::Rove(match rv {
                    crate::interact::Rove::By(n) => CoreRove::By(n),
                    crate::interact::Rove::First => CoreRove::First,
                    crate::interact::Rove::Last => CoreRove::Last,
                }),
            };
            let msg = match r.msg {
                w::PeerMsg::Roved => PeerMsg::Roved,
                w::PeerMsg::Checked(args) => PeerMsg::Notify(Some(Value::List(args.iter().filter_map(plain_to_core).collect::<Vec<_>>().into()))),
            };
            cx.peer_request(to, msg);
        }
        match focus_request {
            Some(true) => cx.focus(FocusReason::Pointer),
            Some(false) => cx.blur(),
            None => {}
        }
        if let Some(a) = focus_anchor {
            cx.focus_anchor(&a);
        }
        if let Some(t) = wake {
            cx.wake_at(t);
        }
        if let Some(c) = cursor {
            cx.set_cursor(c);
        }
        if repaint || diff != StateBits::NONE {
            cx.request_paint();
        }
        if handled == w::Handled::Yes { Handled::Yes } else { Handled::No }
    }

    /// Feeds the node's `disabled` into the widget's own `disabled` prop.
    fn sync_disabled(&mut self, disabled: bool) {
        if let Some(p) = Self::spec_ref().disabled {
            let _ = self.widget.set_prop(p, &w::Value::Bool(disabled));
        }
    }
}

impl<W: w::Kind + Default> Kind for Hosted<W> {
    fn spec() -> &'static KindSpec {
        spec_of(W::spec(), Traits::of::<W>()).core
    }

    fn prop_changed(&mut self, cx: &mut PropCx, prop: PropId, value: Option<&Value>) -> Result<(), PropError> {
        let s = Self::spec_ref();
        let ws = W::spec();
        let Some(ci) = s.core.props.iter().enumerate().find(|(i, _)| s.core.prop_id(*i) == prop).map(|(i, _)| i) else {
            // a common prop: `disabled` reaches the widget through the node's state
            self.sync_disabled(cx.disabled());
            return Ok(());
        };
        let Some(wi) = s.to_core.iter().position(|c| *c == Some(ci)) else { return Ok(()) };
        let wp = &ws.props[wi];
        let v = match value {
            Some(v) => from_core_value(wp, v).ok_or(PropError::WrongType { prop, expected: core_type(wp.ty), got: v.ty() })?,
            None => match wp.default.value() {
                Some(d) => d,
                None => return Ok(()),
            },
        };
        self.widget.set_prop(w::PropId(wi as u32), &v).map_err(|e| PropError::Rejected(format!("{}: {e:?}", wp.name)))
    }

    fn layout_style(&self, cx: &StyleCx) -> taffy::Style {
        let base = cx.default_layout();
        self.widget.layout_style(&w::StyleCx { computed: cx.css(), state: StateBits(cx.state().0), dark: true, base: Some(base) })
    }

    fn measure(&self, cx: &mut MeasureCx, known: taffy::Size<Option<f32>>, avail: taffy::Size<taffy::AvailableSpace>) -> taffy::Size<f32> {
        let node = cx.node;
        let dark = is_dark(cx.service::<crate::theme::Theme>());
        let mut text = shaper(cx.service::<skelter_text::TextSystem>());
        let folded = folded_of(node, Self::spec_ref().core.folds_children, Self::traits().folds_elements);
        let parts = part_styles(node, Self::spec_ref().core.parts);
        self.widget.measure(&mut w::MeasureCx { style: styles(node.css(), &parts, &folded, node.state(), dark), text: &mut *text, scale: node.scale() }, known, avail)
    }

    fn prepare(&mut self, cx: &mut PrepareCx) {
        // a dry recording finds the images the list will reference; the new ones are uploaded
        let node = cx.node;
        let dark = is_dark(cx.service::<crate::theme::Theme>());
        let folded = folded_of(node, Self::spec_ref().core.folds_children, Self::traits().folds_elements);
        let parts = part_styles(node, Self::spec_ref().core.parts);
        let emitted: Vec<w::Emitted>;
        let (new, used) = {
            let mut text = shaper(cx.service::<skelter_text::TextSystem>());
            // the widget's own prepare (layout of what it places or draws itself) runs before the dry recording
            let mut pcx = w::PrepareCx { size: node.size(), style: styles(node.css(), &parts, &folded, node.state(), dark), text: &mut *text, emitted: Vec::new() };
            self.widget.prepare(&mut pcx);
            emitted = pcx.emitted;
            let mut res = PrepareRes { images: &self.images, new: Vec::new(), used: Vec::new() };
            let mut list = DisplayList::new();
            self.widget.record(&mut w::RecordCx { size: node.size(), style: styles(node.css(), &parts, &folded, node.state(), dark), list: &mut list, text: &mut *text, res: &mut res, scale: node.scale() });
            (res.new, res.used)
        };
        for (key, data) in new {
            let id = cx.upload_image(Arc::new(data));
            self.images.insert(key, id);
        }
        let mut released = Vec::new();
        self.images.retain(|k, id| {
            let keep = used.iter().any(|u| u == k);
            if !keep {
                released.push(*id);
            }
            keep
        });
        for id in released {
            cx.release_image(id);
        }
        let s = Self::spec_ref();
        let ws = W::spec();
        for e in emitted {
            let i = e.signal.0 as usize;
            let Some(sig) = ws.signals.get(i) else { continue };
            cx.emit(s.core.signal_id(i), signal_arg(sig, &e.args));
        }
    }

    fn record(&self, cx: &mut RecordCx) {
        let node = cx.node;
        let dark = is_dark(cx.service::<crate::theme::Theme>());
        let folded = folded_of(node, Self::spec_ref().core.folds_children, Self::traits().folds_elements);
        let parts = part_styles(node, Self::spec_ref().core.parts);
        let mut list = DisplayList::new();
        let runs = {
            let mut text = shaper(cx.service::<skelter_text::TextSystem>());
            let mut res = RecordRes { runs: Vec::new(), images: &self.images };
            self.widget.record(&mut w::RecordCx { size: node.size(), style: styles(node.css(), &parts, &folded, node.state(), dark), list: &mut list, text: &mut *text, res: &mut res, scale: node.scale() });
            res.runs
        };
        // the pass's run indices become skelter-core's run ids
        let ids: Vec<RunId> = runs.into_iter().map(|r| cx.add_run(r)).collect();
        let bounds = list.bounds;
        for mut p in list.items {
            if let Prim::Glyphs(g) = &mut p
                && let Some(id) = (g.run.0 as usize).checked_sub(1).and_then(|i| ids.get(i))
            {
                g.run = *id;
            }
            let ink = match &p {
                Prim::PushClip(_) | Prim::PopClip => skelter_base::geom::Rect::ZERO,
                _ => bounds,
            };
            cx.push(p, ink);
        }
    }

    fn hit_test(&self, cx: &HitCx, p: Point) -> HitResult {
        match self.widget.hit_test(&w::HitCx { size: cx.size() }, p) {
            w::HitResult::Hit => cx.box_hit(p),
            w::HitResult::Miss => HitResult::Miss,
        }
    }

    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        let Some(wev) = Self::translate(ev) else { return Handled::No };
        // egui's interaction keeps the pointer on the pressed widget until release
        if matches!(ev, Event::PointerDown(_)) {
            cx.capture_pointer();
        }
        self.run(cx, |w, wcx| w.event(wcx, &wev))
    }

    fn call(&mut self, cx: &mut EventCx, method: MethodId, args: &[Value]) -> Result<Option<Value>, skelter_core::PropError> {
        let core = Self::spec_ref().core;
        let Some(i) = (0..core.methods.len()).find(|&i| core.method_id(i) == method) else {
            return Err(skelter_core::PropError::Rejected(format!("{method:?} is not a method of {}", core.name)));
        };
        match self.run(cx, |w, wcx| w.call(wcx, i, args)) {
            Handled::Yes => Ok(None),
            Handled::No => Err(skelter_core::PropError::Rejected(format!("{} did not take {}", core.name, core.methods[i].name))),
        }
    }

    fn as_any(&self) -> Option<&dyn std::any::Any> {
        Some(&self.widget)
    }

    fn peer_group(&self, _node: &NodeView) -> Option<PeerGroup> {
        let g = self.widget.peer_group()?;
        let s = Self::spec_ref();
        let ws = W::spec();
        let key = match g.key {
            Some((name, v)) => {
                let i = ws.props.iter().position(|p| p.name == name)?;
                Some((s.core.prop_named(name)?, to_core_value(&ws.props[i], &v)?))
            }
            None => None,
        };
        Some(PeerGroup { key, stop: s.core.prop_named(g.stop) })
    }

    fn peer(&mut self, cx: &mut EventCx, _from: NodeId, msg: &PeerMsg) -> Handled {
        let msg = match msg {
            PeerMsg::Roved => w::PeerMsg::Roved,
            PeerMsg::Notify(Some(Value::List(l))) => w::PeerMsg::Checked(l.iter().filter_map(plain_from_core).collect()),
            PeerMsg::Notify(v) => w::PeerMsg::Checked(v.iter().filter_map(plain_from_core).collect()),
        };
        self.run(cx, |w, wcx| w.peer(wcx, &msg))
    }

    fn light_dismiss(&self, _node: &NodeView) -> bool {
        self.widget.light_dismiss()
    }

    fn outside_press(&mut self, cx: &mut EventCx, p: &skelter_core::PointerInput) -> Handled {
        let ev = w::Event::OutsidePress(p.local);
        self.run(cx, |w, wcx| w.event(wcx, &ev))
    }

    fn listen_changed(&mut self, _cx: &mut PropCx, signal: SignalId, listened: bool) {
        let s = Self::spec_ref();
        if let Some(i) = (0..s.core.signals.len()).find(|&i| s.core.signal_id(i) == signal) {
            self.widget.listen(w::SignalId(i as u32), listened);
        }
    }

    fn chrome_padding(&self, cx: &mut MeasureCx) -> Option<Insets> {
        let node = cx.node;
        let dark = is_dark(cx.service::<crate::theme::Theme>());
        let parts = part_styles(node, Self::spec_ref().core.parts);
        let mut text = shaper(cx.service::<skelter_text::TextSystem>());
        let [left, right, top, bottom] = self.widget.chrome_padding(&mut w::MeasureCx { style: styles(node.css(), &parts, &[], node.state(), dark), text: &mut *text, scale: node.scale() })?;
        Some(Insets { top, right, bottom, left })
    }

    fn child_layout(&self, cx: &ChildLayoutCx, style: &mut taffy::Style) -> bool {
        let anchors: Vec<&str> = (0..cx.count).map(|k| cx.anchor_of(k)).collect();
        self.widget.child_layout(&w::ChildCx { index: cx.index, anchors: &anchors }, style)
    }

    fn children_inert(&self) -> bool {
        self.widget.children_inert()
    }

    fn prop_state(&self, _node: &NodeView) -> StateMask {
        StateMask(self.widget.prop_state().0 & W::prop_states().0 & !CORE_BITS.0)
    }

    fn a11y(&self, cx: &mut A11yCx) {
        cx.default_a11y();
        let (state, size) = (StateBits(cx.state().0), cx.size());
        self.widget.a11y(&mut w::A11yCx { node: cx.out, state, size });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::Button;

    #[test]
    fn button_spec_maps_to_core() {
        let s = <Hosted<Button> as Kind>::spec();
        assert_eq!(s.name, "button");
        assert!(s.focusable);
        assert!(s.props.iter().all(|p| p.name != "disabled"), "`disabled` is skelter-core's common prop");
        assert!(s.props.iter().any(|p| p.name == "text" && p.ty == ValueType::Str));
        assert!(s.signals.iter().any(|g| g.name == "clicked" && g.arg.is_none()));
        let mut reg = KindRegistry::new();
        register::<Button>(&mut reg).unwrap();
    }

    #[test]
    fn every_tier0_kind_registers() {
        // no tag or member id collides, and every text prop exists
        let mut reg = KindRegistry::new();
        register_tier0(&mut reg).unwrap();
    }

    #[test]
    fn a_label_folds_its_children() {
        use crate::controls::{Bold, Label};
        assert!(<Hosted<Label> as Kind>::spec().folds_children);
        assert!(!<Hosted<Bold> as Kind>::spec().folds_children);
        assert!(<Hosted<Label> as Kind>::spec().container);
    }

    #[test]
    fn hooks_reach_the_core_spec() {
        use crate::controls::{Checkbox, Label, Slider, Stack};
        let b = <Hosted<Button> as Kind>::spec();
        assert_eq!(b.parts, ["icon", "label"]);
        assert!(b.prop_states.contains(StateMask::DEFAULT) && b.prop_states.contains(StateMask::SELECTED));
        assert!(!b.prop_states.contains(StateMask::HOVERED), "skelter-core's own bits are never derived");
        assert!(<Hosted<Checkbox> as Kind>::spec().prop_states.contains(StateMask::INDETERMINATE));
        assert!(<Hosted<Stack> as Kind>::spec().places_children);
        assert!(!<Hosted<Label> as Kind>::spec().places_children);
        assert!(<Hosted<Slider> as Kind>::spec().parts.contains(&"thumb"));
        let mut c = Hosted::<Checkbox>::default();
        c.widget.props.checked = true;
        assert!(w::Kind::prop_state(&c.widget).contains(StateBits::CHECKED), "what core sets after a prop write");
    }

    #[test]
    fn values_round_trip() {
        let p = w::PropSpec::new("orientation", w::ValueType::Enum(&["horizontal", "vertical"]), skelter_base::change::ChangeClass::Layout, w::Default::Enum("horizontal"), "");
        let v = to_core_value(&p, &w::Value::Enum("vertical")).unwrap();
        assert_eq!(v, Value::Enum(1));
        assert_eq!(from_core_value(&p, &v), Some(w::Value::Enum("vertical")));
        assert_eq!(core_default(&p), PropDefault::Enum(0));
    }
}
