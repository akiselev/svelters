//! Puts the views' kinds on presto-core's [`presto_core::Kind`]: [`Bridge<K>`] wraps a [`shim::Kind`]
//! and translates its spec, prop writes, events, signals, display list and AccessKit node to and
//! from presto-core's. The view logic and its tests stay on the shim; [`register_all`] adds every
//! view kind to a [`KindRegistry`].
//!
//! What presto-core's contexts do not offer yet, the bridge keeps per thread in [`Env`] until they
//! do (each is a request in `plans/milestones/PRESTO-widgets-views.md`):
//! - the time an event happens at and the platform policy ([`set_now`], [`set_policy`]);
//! - deadlines a kind asks for ([`take_wakes`]): core has no timer event for kinds yet;
//! - fast commits a kind makes while handling input ([`take_fast`]);
//! - text measurement while recording (`RecordCx` draws text in the node's font but cannot measure
//!   a string or draw it at another size or colour; labels are drawn in the node's font and colour).

use super::shim::{self, Labels, Palette, Policy};
use presto_base::change::ChangeClass;
use presto_base::color::Color;
use presto_base::commit::FastCommit;
use presto_base::display::{DisplayList, Prim};
use presto_base::event::{PointerEvent, PointerPhase, WheelEvent};
use presto_base::geom::{Point, Rect, Size, Vec2};
use presto_base::ids::NodeId;
use presto_base::time::Time;
use presto_core::{
    A11yCx, Event, EventCx, FocusReason, Handled, Kind, KindRegistry, KindSpec, PointerInput, PropCx, PropError, PropId,
    PropSpec, RecordCx, RegistryError, SignalFlags, SignalSpec, SlotSpec, Value, ValueType,
};
use std::any::TypeId;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// What the bridge supplies in place of the missing core services, per UI thread.
#[derive(Default)]
pub struct Env {
    pub now: Time,
    pub policy: Option<Policy>,
    pub palette: Palette,
    /// Deadlines asked for while handling events: (node, when).
    pub wakes: Vec<(NodeId, Time)>,
    pub fast: Vec<FastCommit>,
}

thread_local! {
    static ENV: RefCell<Env> = RefCell::new(Env::default());
}

/// The time of the event being dispatched (the app runner sets it before `Document::dispatch`).
pub fn set_now(t: Time) {
    ENV.with(|e| e.borrow_mut().now = t);
}
pub fn set_policy(p: Policy) {
    ENV.with(|e| e.borrow_mut().policy = Some(p));
}
pub fn set_palette(p: Palette) {
    ENV.with(|e| e.borrow_mut().palette = p);
}
/// The deadlines kinds asked for since the last call.
pub fn take_wakes() -> Vec<(NodeId, Time)> {
    ENV.with(|e| std::mem::take(&mut e.borrow_mut().wakes))
}
/// The fast commits kinds made since the last call, for the compositor.
pub fn take_fast() -> Vec<FastCommit> {
    ENV.with(|e| std::mem::take(&mut e.borrow_mut().fast))
}

fn policy() -> Policy {
    ENV.with(|e| e.borrow().policy).unwrap_or_else(|| Policy::for_os(shim::Os::current()))
}

/// The core spec of each bridged kind, built once from its shim spec and leaked (one per kind).
fn spec_of<K: shim::Kind>() -> &'static KindSpec {
    static SPECS: OnceLock<Mutex<HashMap<TypeId, &'static KindSpec>>> = OnceLock::new();
    let mut map = SPECS.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    map.entry(TypeId::of::<K>()).or_insert_with(|| Box::leak(Box::new(convert_spec(K::spec()))))
}

fn convert_spec(s: &shim::KindSpec) -> KindSpec {
    let props: Vec<PropSpec> = s
        .props
        .iter()
        .map(|p| {
            let (ty, values): (ValueType, &'static [&'static str]) = match p.ty {
                shim::PropType::Bool => (ValueType::Bool, &[]),
                shim::PropType::I64 => (ValueType::I64, &[]),
                shim::PropType::F64 => (ValueType::F64, &[]),
                shim::PropType::Str => (ValueType::Str, &[]),
                shim::PropType::Enum(v) => (ValueType::Enum, v),
                shim::PropType::Color => (ValueType::Color, &[]),
                shim::PropType::List => (ValueType::List, &[]),
                shim::PropType::Record => (ValueType::Record, &[]),
            };
            // compositor-class props are the kind's native state (scroll offsets, splitter sizes)
            let spec = PropSpec::new(p.name, ty, p.change).enum_values(values);
            if p.change == ChangeClass::Property { spec.native() } else { spec }
        })
        .collect();
    let signals: Vec<SignalSpec> = s
        .signals
        .iter()
        .map(|g| {
            let mut f = SignalFlags::SYNC;
            if g.passive {
                f = f.union(SignalFlags::PASSIVE);
            }
            if g.cancelable {
                f = f.union(SignalFlags::CANCELABLE);
            }
            // the views emit their details as records (see `arg`)
            SignalSpec::new(g.name, Some(ValueType::Record), f)
        })
        .collect();
    let slots: Vec<SlotSpec> = s.slots.iter().filter(|n| !n.is_empty()).map(|n| SlotSpec { name: n, single: false }).collect();
    let mut spec = KindSpec::new(s.name);
    spec.props = Box::leak(props.into_boxed_slice());
    spec.signals = Box::leak(signals.into_boxed_slice());
    spec.slots = Box::leak(slots.into_boxed_slice());
    spec.role = s.role;
    spec.container = !s.slots.is_empty();
    spec.focusable = matches!(
        s.role,
        accesskit::Role::ScrollView
            | accesskit::Role::ListBox
            | accesskit::Role::Tree
            | accesskit::Role::MenuBar
            | accesskit::Role::ComboBox
            | accesskit::Role::Splitter
    );
    spec
}

use presto_base::accesskit;

/// A core value as the shim kinds read it (enums by name).
fn to_shim(v: &Value, enum_values: &[&str]) -> shim::Value {
    match v {
        Value::Bool(b) => shim::Value::Bool(*b),
        Value::I64(i) => shim::Value::I64(*i),
        Value::F64(f) => shim::Value::F64(*f),
        Value::Str(s) => shim::Value::Str(s.to_string()),
        Value::Enum(i) => enum_values.get(*i as usize).map_or(shim::Value::I64(*i as i64), |n| shim::Value::Str((*n).to_string())),
        Value::Color(c) => shim::Value::Color(*c),
        Value::List(l) => shim::Value::List(l.iter().map(|v| to_shim(v, &[])).collect()),
        Value::Record(r) => shim::Value::Record(r.iter().map(|(k, v)| (k.to_string(), to_shim(v, &[]))).collect()),
    }
}

/// A shim value as a core value (`Null` has no core value).
fn to_core(v: &shim::Value) -> Option<Value> {
    Some(match v {
        shim::Value::Null => return None,
        shim::Value::Bool(b) => Value::Bool(*b),
        shim::Value::I64(i) => Value::I64(*i),
        shim::Value::F64(f) => Value::F64(*f),
        shim::Value::Str(s) => Value::Str(s.as_str().into()),
        shim::Value::Color(c) => Value::Color(*c),
        shim::Value::List(l) => Value::List(l.iter().filter_map(to_core).collect()),
        shim::Value::Record(r) => Value::Record(r.iter().filter_map(|(k, v)| Some((Arc::from(k.as_str()), to_core(v)?))).collect()),
    })
}

/// A signal detail as the record the spec declares (a bare value becomes `{value}`).
fn arg(v: &shim::Value) -> Option<Value> {
    match v {
        shim::Value::Null => None,
        shim::Value::Record(_) => to_core(v),
        other => Some(Value::Record(Arc::from([(Arc::from("value"), to_core(other)?)]))),
    }
}

/// The ink rect of a primitive, for [`RecordCx::push`].
fn ink(p: &Prim) -> Rect {
    match p {
        Prim::Quad(q) => q.rect,
        Prim::Shadow(s) => s.rect.translate(s.offset).inflate(s.blur * 2.0 + s.spread.max(0.0)),
        Prim::Line(l) => Rect::new(l.from.x.min(l.to.x), l.from.y.min(l.to.y), l.from.x.max(l.to.x), l.from.y.max(l.to.y)).inflate(l.width),
        Prim::Glyphs(_) => Rect::ZERO,
        Prim::Icon(i) => i.rect,
        Prim::Image(i) => i.rect,
        Prim::Path(_) => Rect::ZERO,
        Prim::Surface(s) => s.rect,
        Prim::Canvas(c) => c.rect,
        Prim::PushClip(_) | Prim::PopClip => Rect::ZERO,
    }
}

/// Labels drawn through the node's [`RecordCx`]: what the shim recorded so far is flushed first, so
/// paint order is kept.
struct CoreLabels<'a, 'b> {
    cx: &'a mut RecordCx<'b>,
    flushed: usize,
}

impl CoreLabels<'_, '_> {
    fn flush(&mut self, list: &DisplayList) {
        for p in &list.items[self.flushed..] {
            self.cx.push(p.clone(), ink(p));
        }
        self.flushed = list.items.len();
    }
}

impl Labels for CoreLabels<'_, '_> {
    /// An estimate (0.6 em per char, 1.25 em lines) until `RecordCx` can measure text.
    fn measure(&mut self, text: &str, font_size: f32, max_width: Option<f32>) -> Size {
        let w = text.chars().count() as f32 * font_size * 0.6;
        Size::new(max_width.map_or(w, |m| w.min(m)), font_size * 1.25)
    }
    fn record(&mut self, list: &mut DisplayList, text: &str, origin: Point, _font_size: f32, _color: Color, max_width: Option<f32>) {
        self.flush(list);
        self.cx.draw_text(text, origin, max_width);
    }
}

/// A view kind on presto-core's [`Kind`] trait.
#[derive(Default)]
pub struct Bridge<K> {
    pub inner: K,
}

impl<K: shim::Kind> Bridge<K> {
    fn prop_name(prop: PropId) -> Option<&'static shim::PropSpec> {
        let core = spec_of::<K>();
        (0..core.props.len()).find(|&i| core.prop_id(i) == prop).map(|i| &K::spec().props[i])
    }
}

fn shim_pointer(p: &PointerInput, phase: PointerPhase) -> PointerEvent {
    PointerEvent {
        phase,
        pointer: p.pointer,
        kind: p.kind,
        button: p.button,
        buttons: p.buttons,
        pos: p.local,
        modifiers: p.modifiers,
        pressure: p.pressure,
        tilt: Vec2::ZERO,
        history: Default::default(),
    }
}

impl<K: shim::Kind + Default> Kind for Bridge<K> {
    fn spec() -> &'static KindSpec {
        spec_of::<K>()
    }

    fn prop_changed(&mut self, cx: &mut PropCx, prop: PropId, value: Option<&Value>) -> Result<(), PropError> {
        let Some(p) = Self::prop_name(prop) else { return Ok(()) };
        let values = match p.ty {
            shim::PropType::Enum(v) => v,
            _ => &[],
        };
        let v = value.map_or(shim::Value::Null, |v| to_shim(v, values));
        match self.inner.set_prop(p.name, &v) {
            Ok(c) => {
                match c {
                    ChangeClass::Structure | ChangeClass::Layout => cx.request_layout(),
                    ChangeClass::Paint | ChangeClass::Property => cx.request_paint(),
                }
                cx.request_a11y();
                Ok(())
            }
            // a prop the kind does not handle itself (a common prop routed here) is not an error
            Err(shim::PropError::Unknown) => Ok(()),
            Err(shim::PropError::Invalid(m)) => Err(PropError::Rejected(format!("{}: {m}", p.name))),
            Err(shim::PropError::Stale) => Err(PropError::Rejected(format!("{}: changed natively since this write was made", p.name))),
        }
    }

    fn record(&self, cx: &mut RecordCx) {
        let palette = ENV.with(|e| e.borrow().palette);
        let now = ENV.with(|e| e.borrow().now);
        let size = cx.size();
        let mut list = DisplayList::new();
        let mut labels = CoreLabels { cx, flushed: 0 };
        let mut scx = shim::RecordCx { size, list: &mut list, palette: &palette, labels: &mut labels, now };
        self.inner.record(&mut scx);
        labels.flush(&list);
    }

    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        let sev = match ev {
            Event::PointerDown(p) => shim::Event::Pointer(shim_pointer(p, PointerPhase::Down)),
            Event::PointerUp(p) => shim::Event::Pointer(shim_pointer(p, PointerPhase::Up)),
            Event::PointerMove(p) => shim::Event::Pointer(shim_pointer(p, PointerPhase::Move)),
            Event::Wheel(w) => shim::Event::Wheel(WheelEvent { pos: w.local, delta: w.delta, phase: None, momentum: None, modifiers: w.modifiers }),
            Event::Key(k) => shim::Event::Key((*k).clone()),
            Event::FocusIn(_) => shim::Event::Focus(true),
            Event::FocusOut => shim::Event::Focus(false),
            // the views act on press and release (egui's click semantics live in their units);
            // hover enter/leave carry no position, cancel and IME are not view input
            Event::Click(_) | Event::PointerCancel | Event::HoverEnter | Event::HoverLeave | Event::Ime(_) => return Handled::No,
        };
        let node = cx.node();
        let size = cx.view(node).map_or(Size::ZERO, |v| v.size());
        let pol = policy();
        let now = ENV.with(|e| e.borrow().now);
        let mut scx = shim::EventCx::new(now, size, &pol);
        if let shim::Event::Pointer(p) = &sev {
            scx.modifiers = p.modifiers;
        }
        let handled = self.inner.event(&mut scx, &sev);
        let core = spec_of::<K>();
        for e in &scx.emitted {
            if let Some(id) = core.signal_named(e.signal) {
                cx.emit(id, arg(&e.detail));
            }
        }
        match scx.changed {
            Some(ChangeClass::Structure | ChangeClass::Layout) => cx.request_layout(),
            Some(ChangeClass::Paint | ChangeClass::Property) => cx.request_paint(),
            None => {}
        }
        match scx.capture {
            Some(true) => cx.capture_pointer(),
            Some(false) => cx.release_pointer(),
            None => {}
        }
        if scx.focus {
            cx.focus(FocusReason::Pointer);
        }
        if let Some(c) = scx.cursor {
            cx.set_cursor(c);
        }
        ENV.with(|e| {
            let mut e = e.borrow_mut();
            e.fast.append(&mut scx.fast);
            if let Some(t) = scx.wake {
                e.wakes.push((node, t));
            }
        });
        match handled {
            shim::Handled::Yes => Handled::Yes,
            shim::Handled::No => Handled::No,
        }
    }

    fn a11y(&self, cx: &mut A11yCx) {
        cx.default_a11y();
        self.inner.a11y(cx.out);
    }
}

/// Registers every view kind.
pub fn register_all(reg: &mut KindRegistry) -> Result<(), RegistryError> {
    use super::*;
    reg.register::<Bridge<scroll::ScrollArea>>()?;
    reg.register::<Bridge<scroll::Scrollbar>>()?;
    reg.register::<Bridge<panel::Panel>>()?;
    reg.register::<Bridge<split::SplitView>>()?;
    reg.register::<Bridge<virtual_list::VirtualList>>()?;
    reg.register::<Bridge<tree::TreeView>>()?;
    reg.register::<Bridge<popup::Popup>>()?;
    reg.register::<Bridge<menu::MenuBar>>()?;
    reg.register::<Bridge<menu::ContextMenu>>()?;
    reg.register::<Bridge<menu::MenuKind>>()?;
    reg.register::<Bridge<menu::MenuItemKind>>()?;
    reg.register::<Bridge<menu::MenuSeparatorKind>>()?;
    reg.register::<Bridge<select::Select>>()?;
    reg.register::<Bridge<tooltip::Tooltip>>()?;
    reg.register::<Bridge<toast::ToastHost>>()?;
    reg.register::<Bridge<modal::Modal>>()?;
    reg.register::<Bridge<inner_window::InnerWindow>>()?;
    reg.register::<Bridge<status_bar::StatusBar>>()?;
    reg.register::<Bridge<dialog_buttons::DialogButtons>>()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_view_kind_registers_without_collisions() {
        let mut reg = KindRegistry::new();
        register_all(&mut reg).expect("view kinds register");
        assert_eq!(reg.len(), 19);
        let scroll = reg.by_name("scroll-area").expect("scroll-area");
        assert_eq!(scroll.spec.role, accesskit::Role::ScrollView);
    }

    #[test]
    fn values_round_trip_and_enums_read_by_name() {
        assert_eq!(to_shim(&Value::Enum(1), &["a", "b"]), shim::Value::Str("b".into()));
        let r = shim::Value::Record(vec![("x".into(), shim::Value::F64(2.0))]);
        assert_eq!(to_shim(&to_core(&r).unwrap(), &[]), r);
        assert_eq!(arg(&shim::Value::I64(3)).unwrap().field("value"), Some(&Value::I64(3)));
        assert_eq!(arg(&shim::Value::Null), None);
    }
}
