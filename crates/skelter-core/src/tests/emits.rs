//! Emitting from `prepare` ([`PrepareCx::emit`]), on a descendant ([`EventCx::emit_on`],
//! [`EventCx::set_native_on`]), and the drag image of in-app drags ([`DragPreview`]).

use super::*;
use crate::cx::{EventCx, PrepareCx, RecordCx};
use crate::kind::{DragPreview, Event, Handled, Kind, KindSpec, PropSpec, SignalFlags, SignalSpec};
use crate::value::{PropDefault, ValueType};
use crate::{KindRegistry, ListenFlags, PropId, SignalId, StateMask, StyleInput, Value, common};
use skelter_base::change::ChangeClass;
use std::sync::Arc;

const WIN: KindId = KindId::of("win");
const WIN_RANGE: SignalId = SignalId::of("win", "range", "int");

static WIN_SPEC: KindSpec = KindSpec {
    signals: &[SignalSpec::new("range", Some(ValueType::I64), SignalFlags::PASSIVE)],
    container: true,
    focusable: true,
    ..KindSpec::new("win")
};

/// A virtual list stand-in: each prepare reports its data window (`range`, the prepare count); a
/// click emits `picked` on its first child and writes the child's `label`, and tries the same on
/// its siblings (not descendants: refused).
#[derive(Default)]
struct Win {
    prepared: i64,
}

impl Kind for Win {
    fn spec() -> &'static KindSpec {
        &WIN_SPEC
    }
    fn prepare(&mut self, cx: &mut PrepareCx) {
        self.prepared += 1;
        cx.emit(WIN_RANGE, Some(Value::I64(self.prepared)));
    }
    fn record(&self, cx: &mut RecordCx) {
        cx.draw_box();
    }
    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        let Event::Click(_) = ev else { return Handled::No };
        let n = cx.node();
        let Some(c) = cx.view(n).and_then(|v| v.children().first().copied()) else { return Handled::No };
        assert!(cx.emit_on(c, ITEM_PICKED, Some(Value::I64(7))));
        cx.set_native_on(c, ITEM_LABEL, Value::from("picked")).unwrap();
        let parent = cx.document().parent(n).expect("in the tree");
        let siblings: Vec<NodeId> = cx.view(parent).map(|v| v.children().iter().copied().filter(|&s| s != n).collect()).unwrap_or_default();
        for s in siblings {
            assert!(!cx.emit_on(s, ITEM_PICKED, None), "not a descendant");
            assert!(cx.set_native_on(s, ITEM_LABEL, Value::from("no")).is_err());
        }
        Handled::Yes
    }
}

const ITEM: KindId = KindId::of("item");
const ITEM_PICKED: SignalId = SignalId::of("item", "picked", "int");
const ITEM_LABEL: PropId = PropId::of("item", "label", "string");

static ITEM_SPEC: KindSpec = KindSpec {
    props: &[PropSpec::new("label", ValueType::Str, ChangeClass::Paint).default(PropDefault::Str("")).native()],
    signals: &[SignalSpec::new("picked", Some(ValueType::I64), SignalFlags::PASSIVE)],
    ..KindSpec::new("item")
};

/// A row of [`Win`], and a drag source that supplies its own drag image (itself, at the press).
#[derive(Default)]
struct Item;

impl Kind for Item {
    fn spec() -> &'static KindSpec {
        &ITEM_SPEC
    }
    fn record(&self, cx: &mut RecordCx) {
        cx.draw_box();
    }
    fn drag_preview(&self, node: &crate::NodeView, at: Point) -> Option<DragPreview> {
        Some(DragPreview::Node { node: node.id(), offset: Vec2::new(-at.x, -at.y) })
    }
}

fn edoc() -> (Document, NodeId) {
    let mut r = KindRegistry::new();
    r.register::<testkinds::BoxKind>().unwrap();
    r.register::<Win>().unwrap();
    r.register::<Item>().unwrap();
    let mut d = Document::new(Arc::new(r), Box::new(FixedText));
    d.set_text_shaper(Box::new(FixedText));
    let root = d.create_window(W, BOX, Size::new(200.0, 100.0), Scale(1.0));
    (d, root)
}

#[test]
fn a_kind_emits_from_prepare_after_it() {
    let (mut d, root) = edoc();
    let w = child(&mut d, root, WIN, "width: 50px; height: 50px");
    d.listen(w, WIN_RANGE, ListenFlags::EMPTY);
    frame(&mut d);
    let ev = d.take_events();
    let range: Vec<&crate::JsEvent> = ev.iter().filter(|e| e.signal == WIN_RANGE).collect();
    assert_eq!(range.len(), 1, "queued once the prepare returned");
    assert_eq!((range[0].target, range[0].arg.clone(), range[0].cancelable), (w, Some(Value::I64(1)), false));
    frame(&mut d);
    assert!(d.take_events().is_empty(), "a clean node is not prepared again");
    // nobody listens: nothing is queued
    d.unlisten(w, WIN_RANGE, false);
    d.set_style(w, StyleInput { inline: Some("width: 60px; height: 50px".into()), ..Default::default() });
    frame(&mut d);
    assert!(d.take_events().iter().all(|e| e.signal != WIN_RANGE));
}

#[test]
fn an_event_emits_on_and_writes_a_descendant_only() {
    let (mut d, root) = edoc();
    let w = child(&mut d, root, WIN, "width: 100px; height: 50px");
    let item = child(&mut d, w, ITEM, "width: 20px; height: 20px");
    let outsider = child(&mut d, root, ITEM, "width: 20px; height: 20px");
    d.listen(item, ITEM_PICKED, ListenFlags::EMPTY);
    d.listen(outsider, ITEM_PICKED, ListenFlags::EMPTY);
    frame(&mut d);
    d.take_events();
    // a click on the list, beside its row
    let out = click(&mut d, 80.0, 25.0);
    let mut picked: Vec<(NodeId, Option<Value>)> = out.iter().flat_map(|o| o.events.iter()).filter(|e| e.signal == ITEM_PICKED).map(|e| (e.target, e.arg.clone())).collect();
    picked.extend(d.take_events().into_iter().filter(|e| e.signal == ITEM_PICKED).map(|e| (e.target, e.arg)));
    assert_eq!(picked, [(item, Some(Value::I64(7)))], "at the row, not the list");
    assert_eq!(d.view(item).unwrap().prop_str(ITEM_LABEL), Some("picked"));
    assert_eq!(d.view(outsider).unwrap().prop_str(ITEM_LABEL), Some(""));
}

#[test]
fn a_drag_keeps_the_preview_its_source_or_the_app_supplies() {
    let (mut d, root) = edoc();
    let src = child(&mut d, root, ITEM, "width: 40px; height: 40px");
    let plain = child(&mut d, root, BOX, "width: 40px; height: 40px");
    d.set_prop(src, common::DRAG_TYPE, Value::from("cad/part")).unwrap();
    d.set_prop(plain, common::DRAG_TYPE, Value::from("cad/net")).unwrap();
    frame(&mut d);
    let drag = |d: &mut Document, x: f32, y: f32| {
        d.dispatch(&pointer(PointerPhase::Down, x, y, 0));
        let mut mv = pointer(PointerPhase::Move, x + 20.0, y, 10);
        if let InputEvent::Pointer(p) = &mut mv.event {
            p.buttons = 1;
        }
        d.dispatch(&mv);
    };
    // the source kind's own preview, offset to the press
    drag(&mut d, 10.0, 5.0);
    assert!(d.state(src).contains(StateMask::DRAGGING));
    assert_eq!(d.drag_preview(), Some(&DragPreview::Node { node: src, offset: Vec2::new(-10.0, -5.0) }));
    d.dispatch(&pointer(PointerPhase::Up, 30.0, 5.0, 20));
    assert_eq!(d.drag_preview(), None, "gone with the drag");
    // a kind without one: the app's provider
    let seen: std::rc::Rc<std::cell::Cell<Option<NodeId>>> = Default::default();
    let log = seen.clone();
    d.set_drag_preview_provider(Box::new(move |doc, n, _at| {
        log.set(Some(n));
        let ty = doc.view(n)?.prop_str(common::DRAG_TYPE)?.to_owned();
        let mut list = skelter_base::display::DisplayList::new();
        list.push(skelter_base::display::Prim::PopClip, skelter_base::geom::Rect::ZERO);
        Some(DragPreview::List { list, runs: Vec::new(), offset: Vec2::new(ty.len() as f32, 0.0) })
    }));
    let r = d.world_rect(plain).expect("laid out");
    drag(&mut d, r.min.x + 5.0, r.min.y + 5.0);
    assert_eq!(seen.get(), Some(plain));
    assert!(matches!(d.drag_preview(), Some(DragPreview::List { offset, .. }) if offset.x == 7.0));
    assert!(d.set_drag_preview(None));
    assert_eq!(d.drag_preview(), None);
}

const UPPER: KindId = KindId::of("upper");
const UPPER_TEXT: PropId = PropId::of("upper", "text", "string");

static UPPER_SPEC: KindSpec = KindSpec {
    props: &[PropSpec::new("text", ValueType::Str, ChangeClass::Paint).default(PropDefault::Str("")).native()],
    ..KindSpec::new("upper")
};

/// A kind that normalises its bound `text` while preparing (upper case), as a code editor settles a
/// bound value at mount (FQ-73); a write of the wrong type is refused at the call.
#[derive(Default)]
struct Upper;

impl Kind for Upper {
    fn spec() -> &'static KindSpec {
        &UPPER_SPEC
    }
    fn prepare(&mut self, cx: &mut PrepareCx) {
        let t = cx.node.prop_str(UPPER_TEXT).unwrap_or("").to_owned();
        if t != t.to_uppercase() {
            cx.set_native(UPPER_TEXT, Value::from(t.to_uppercase())).unwrap();
        }
        assert!(cx.set_native(UPPER_TEXT, Value::Bool(true)).is_err(), "wrong type");
        assert!(cx.set_native(ITEM_LABEL, Value::from("x")).is_err(), "not its prop");
    }
    fn record(&self, cx: &mut RecordCx) {
        cx.draw_box();
    }
}

#[test]
fn a_kind_writes_native_state_from_prepare() {
    let mut r = KindRegistry::new();
    r.register::<testkinds::BoxKind>().unwrap();
    r.register::<Upper>().unwrap();
    r.register::<Item>().unwrap();
    let mut d = Document::new(Arc::new(r), Box::new(FixedText));
    let root = d.create_window(W, BOX, Size::new(200.0, 100.0), Scale(1.0));
    let u = child(&mut d, root, UPPER, "width: 50px; height: 20px");
    d.set_prop(u, UPPER_TEXT, Value::from("net1")).unwrap();
    d.drain_changes();
    frame(&mut d);
    assert_eq!(d.view(u).unwrap().prop_str(UPPER_TEXT), Some("NET1"), "applied once the prepare returned");
    let changes = d.drain_changes();
    assert!(
        changes.iter().any(|c| matches!(c, crate::journal::Change::Prop { node, prop, native: true, .. } if *node == u && *prop == UPPER_TEXT)),
        "journalled as the kind's own (for `bind:`): {changes:?}"
    );
}
