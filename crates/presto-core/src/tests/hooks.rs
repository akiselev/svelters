//! The kind hooks WIDGETS-BASIC asked for: paint-part styles, whether JS listens, prop-derived state,
//! chrome padding, per-child layout and visibility, inert children, the window scale in the contexts,
//! peer groups (exclusivity, one Tab stop, roving), scope activation (`default`/`cancel`, `:default`),
//! `label for`, cancelable defaults through the sync deliverer, and in-app drag and drop.

use super::*;
use crate::cx::{ChildLayoutCx, EventCx, MeasureCx, PropCx, RecordCx};
use crate::kind::{Event, Handled, Kind, KindSpec, PeerGroup, PeerMsg, PeerTarget, PropSpec, Rove, SignalFlags, SignalSpec};
use crate::value::{PropDefault, ValueType};
use crate::{KindRegistry, ListenFlags, NodeFlags, Outcome, PropId, SignalId, StateMask, Value, common};
use presto_base::change::ChangeClass;
use presto_base::event::PointerPhase;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

// ---- kinds ----

const RADIO: KindId = KindId::of("radio");
const RADIO_CHECKED: PropId = PropId::of("radio", "checked", "bool");
const RADIO_GROUP: PropId = PropId::of("radio", "group", "string");

static RADIO_SPEC: KindSpec = KindSpec {
    props: &[
        PropSpec::new("checked", ValueType::Bool, ChangeClass::Paint).default(PropDefault::Bool(false)).native(),
        PropSpec::new("group", ValueType::Str, ChangeClass::Paint).default(PropDefault::Str("")),
    ],
    role: accesskit::Role::RadioButton,
    focusable: true,
    prop_states: StateMask::CHECKED,
    ..KindSpec::new("radio")
};

/// A radio button: a click or a roving move checks it and unchecks its peers; arrows rove.
#[derive(Default)]
struct Radio;

impl Radio {
    fn check(cx: &mut EventCx) {
        let _ = cx.set_native(RADIO_CHECKED, Value::Bool(true));
        cx.peer_request(PeerTarget::Others, PeerMsg::Notify(None));
    }
}

impl Kind for Radio {
    fn spec() -> &'static KindSpec {
        &RADIO_SPEC
    }
    fn measure(&self, _cx: &mut MeasureCx, _k: taffy::Size<Option<f32>>, _a: taffy::Size<taffy::AvailableSpace>) -> taffy::Size<f32> {
        taffy::Size { width: 16.0, height: 16.0 }
    }
    fn record(&self, cx: &mut RecordCx) {
        cx.draw_box();
    }
    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        match ev {
            Event::Click(_) => {
                Self::check(cx);
                Handled::Yes
            }
            Event::Key(k) if k.state == presto_base::event::KeyState::Pressed && k.key == Some(Key::ArrowDown) => {
                cx.peer_request(PeerTarget::Rove(Rove::By(1)), PeerMsg::Roved);
                Handled::Yes
            }
            _ => Handled::No,
        }
    }
    fn prop_state(&self, node: &crate::NodeView) -> StateMask {
        if node.prop_bool(RADIO_CHECKED) { StateMask::CHECKED } else { StateMask::NONE }
    }
    fn peer_group(&self, node: &crate::NodeView) -> Option<PeerGroup> {
        Some(PeerGroup { key: Some((RADIO_GROUP, node.prop(RADIO_GROUP).cloned().unwrap_or(Value::from("")))), stop: Some(RADIO_CHECKED) })
    }
    fn peer(&mut self, cx: &mut EventCx, _from: NodeId, msg: &PeerMsg) -> Handled {
        match msg {
            PeerMsg::Roved => Self::check(cx),
            PeerMsg::Notify(_) => {
                let _ = cx.set_native(RADIO_CHECKED, Value::Bool(false));
            }
        }
        Handled::Yes
    }
}

const DECK: KindId = KindId::of("deck");
const DECK_CURRENT: PropId = PropId::of("deck", "current", "string");
const DECK_CAPTION: PropId = PropId::of("deck", "caption", "number");
const DECK_INERT: PropId = PropId::of("deck", "inert", "bool");

static DECK_SPEC: KindSpec = KindSpec {
    props: &[
        PropSpec::new("current", ValueType::Str, ChangeClass::Layout).default(PropDefault::Str("")),
        PropSpec::new("caption", ValueType::F64, ChangeClass::Layout).default(PropDefault::F64(0.0)),
        PropSpec::new("inert", ValueType::Bool, ChangeClass::Paint).default(PropDefault::Bool(false)),
    ],
    container: true,
    places_children: true,
    parts: &["caption"],
    ..KindSpec::new("deck")
};

/// A stack: shows the child whose anchor is `current`; a `caption` row of chrome on top; `inert`
/// makes its content inert. Records its caption part's `color` as a quad, so tests see the part.
#[derive(Default)]
struct Deck {
    inert: bool,
    caption: f32,
}

impl Kind for Deck {
    fn spec() -> &'static KindSpec {
        &DECK_SPEC
    }
    fn prop_changed(&mut self, _cx: &mut PropCx, prop: PropId, value: Option<&Value>) -> Result<(), crate::PropError> {
        if prop == DECK_INERT {
            self.inert = value.and_then(Value::as_bool).unwrap_or(false);
        } else if prop == DECK_CAPTION {
            self.caption = value.and_then(Value::as_f64).unwrap_or(0.0) as f32;
        }
        Ok(())
    }
    fn record(&self, cx: &mut RecordCx) {
        cx.draw_box();
    }
    fn chrome_padding(&self, _cx: &mut MeasureCx) -> Option<presto_base::geom::Insets> {
        (self.caption > 0.0).then(|| presto_base::geom::Insets { top: self.caption, right: 0.0, bottom: 0.0, left: 0.0 })
    }
    fn child_layout(&self, cx: &ChildLayoutCx, _style: &mut taffy::Style) -> bool {
        let current = cx.parent.prop_str(DECK_CURRENT).unwrap_or("");
        current.is_empty() || cx.anchor_of(cx.index) == current
    }
    fn children_inert(&self) -> bool {
        self.inert
    }
}

const BTN: KindId = KindId::of("btn");
const BTN_DEFAULT: PropId = PropId::of("btn", "default", "bool");
const BTN_CANCEL: PropId = PropId::of("btn", "cancel", "bool");
const BTN_PRESSED: SignalId = SignalId::of("btn", "pressed", "void");
const BTN_CHANGED: SignalId = SignalId::of("btn", "changed", "void");

static BTN_SPEC: KindSpec = KindSpec {
    props: &[
        PropSpec::new("default", ValueType::Bool, ChangeClass::Paint).default(PropDefault::Bool(false)),
        PropSpec::new("cancel", ValueType::Bool, ChangeClass::Paint).default(PropDefault::Bool(false)),
    ],
    signals: &[
        SignalSpec::new("pressed", None, SignalFlags::SYNC),
        SignalSpec::new("changed", None, SignalFlags::SYNC.union(SignalFlags::CANCELABLE)),
    ],
    role: accesskit::Role::Button,
    focusable: true,
    ..KindSpec::new("btn")
};

/// A push button: an AccessKit click emits `pressed`; Space emits the cancelable `changed` and, unless
/// a listener prevented it, sets `:checked`.
#[derive(Default)]
struct Btn;

impl Kind for Btn {
    fn spec() -> &'static KindSpec {
        &BTN_SPEC
    }
    fn measure(&self, _cx: &mut MeasureCx, _k: taffy::Size<Option<f32>>, _a: taffy::Size<taffy::AvailableSpace>) -> taffy::Size<f32> {
        taffy::Size { width: 40.0, height: 20.0 }
    }
    fn record(&self, cx: &mut RecordCx) {
        cx.draw_box();
    }
    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        match ev {
            Event::A11y(r) if r.action == accesskit::Action::Click => {
                cx.emit(BTN_PRESSED, None);
                Handled::Yes
            }
            Event::Key(k) if k.state == presto_base::event::KeyState::Pressed && k.key == Some(Key::Space) => {
                if !cx.emit_sync(BTN_CHANGED, None).prevented {
                    cx.set_state(StateMask::CHECKED, true);
                }
                Handled::Yes
            }
            _ => Handled::No,
        }
    }
}

const LABEL: KindId = KindId::of("lbl");
const LABEL_FOR: PropId = PropId::of("lbl", "for", "string");

static LABEL_SPEC: KindSpec = KindSpec { props: &[PropSpec::new("for", ValueType::Str, ChangeClass::Paint)], role: accesskit::Role::Label, ..KindSpec::new("lbl") };

/// A label: a click focuses the node its `for` names.
#[derive(Default)]
struct Lbl;

impl Kind for Lbl {
    fn spec() -> &'static KindSpec {
        &LABEL_SPEC
    }
    fn measure(&self, _cx: &mut MeasureCx, _k: taffy::Size<Option<f32>>, _a: taffy::Size<taffy::AvailableSpace>) -> taffy::Size<f32> {
        taffy::Size { width: 30.0, height: 10.0 }
    }
    fn record(&self, cx: &mut RecordCx) {
        cx.draw_box();
    }
    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        if let Event::Click(_) = ev {
            let target = cx.view(cx.node()).and_then(|v| v.prop_str(LABEL_FOR)).unwrap_or("").to_owned();
            return if cx.focus_anchor(&target) { Handled::Yes } else { Handled::No };
        }
        Handled::No
    }
}

fn hdoc() -> (Document, NodeId) {
    let mut r = KindRegistry::new();
    r.register::<testkinds::BoxKind>().unwrap();
    r.register::<testkinds::TextKind>().unwrap();
    r.register::<testkinds::ToggleKind>().unwrap();
    r.register::<Radio>().unwrap();
    r.register::<Deck>().unwrap();
    r.register::<Btn>().unwrap();
    r.register::<Lbl>().unwrap();
    let mut d = Document::new(Arc::new(r), Box::new(FixedText));
    d.set_text_shaper(Box::new(FixedText));
    let root = d.create_window(W, BOX, Size::new(200.0, 100.0), Scale(2.0));
    (d, root)
}

fn anchor(d: &mut Document, n: NodeId, a: &str) {
    d.set_prop(n, common::ANCHOR, Value::from(a)).unwrap();
}

// ---- (a) parts, (b) listened, (f) prop state, (g) scale ----

#[test]
fn part_styles_inherit_from_the_node_and_follow_part_rules() {
    let (mut d, root) = hdoc();
    d.add_sheet("deck { color: #00ff00 } deck::part(caption) { color: #ff0000 } deck:hover::part(caption) { color: #0000ff }");
    let deck = child(&mut d, root, DECK, "width: 50px; height: 50px");
    frame(&mut d);
    let red = d.view(deck).unwrap().part("caption").and_then(|c| c.get("color").cloned());
    assert!(red.is_some(), "the part has the rule's colour");
    assert!(d.view(deck).unwrap().part("thumb").is_none(), "only declared parts are computed");
    d.set_state(deck, StateMask::HOVERED, true);
    frame(&mut d);
    let blue = d.view(deck).unwrap().part("caption").and_then(|c| c.get("color").cloned());
    assert_ne!(red, blue, "`:hover::part()` restyles the part with its host");
}

#[test]
fn kinds_see_whether_js_listens() {
    let (mut d, root) = hdoc();
    let deck = child(&mut d, root, DECK, "");
    assert!(!d.view(deck).unwrap().listened(common::CLICK));
    d.listen(deck, common::CLICK, ListenFlags::EMPTY);
    d.listen(deck, common::CLICK, ListenFlags::CAPTURE);
    assert!(d.view(deck).unwrap().listened(common::CLICK));
    d.unlisten(deck, common::CLICK, false);
    assert!(d.view(deck).unwrap().listened(common::CLICK), "the capture listener is still there");
    d.unlisten(deck, common::CLICK, true);
    assert!(!d.view(deck).unwrap().listened(common::CLICK));
}

#[test]
fn prop_state_follows_writes() {
    let (mut d, root) = hdoc();
    let r = child(&mut d, root, RADIO, "");
    assert!(!d.state(r).contains(StateMask::CHECKED));
    d.set_prop(r, RADIO_CHECKED, Value::Bool(true)).unwrap();
    assert!(d.state(r).contains(StateMask::CHECKED), "`:checked` without an event");
    d.reset_prop(r, RADIO_CHECKED).unwrap();
    assert!(!d.state(r).contains(StateMask::CHECKED));
}

#[test]
fn contexts_carry_the_window_scale() {
    let (mut d, root) = hdoc();
    let b = child(&mut d, root, BOX, "");
    assert_eq!(d.view(b).unwrap().scale(), 2.0);
    let detached = d.create(BOX);
    assert_eq!(d.view(detached).unwrap().scale(), 1.0);
    d.resize_window(W, Size::new(200.0, 100.0), Scale(1.5));
    assert_eq!(d.view(b).unwrap().scale(), 1.5);
}

// ---- (e) container hooks ----

#[test]
fn chrome_padding_moves_the_children() {
    let (mut d, root) = hdoc();
    let deck = child(&mut d, root, DECK, "width: 100px; height: 80px; padding: 2px");
    let a = child(&mut d, deck, BOX, "width: 10px; height: 10px");
    frame(&mut d);
    assert_eq!(d.layout_rect(a).unwrap().min.y, 2.0);
    d.set_prop(deck, DECK_CAPTION, Value::F64(20.0)).unwrap();
    frame(&mut d);
    assert_eq!(d.layout_rect(a).unwrap().min.y, 22.0, "the caption row is added to the padding");
}

#[test]
fn a_container_shows_the_child_it_places() {
    let (mut d, root) = hdoc();
    let deck = child(&mut d, root, DECK, "width: 100px; height: 80px");
    let a = child(&mut d, deck, TOGGLE, "");
    let b = child(&mut d, deck, TOGGLE, "");
    anchor(&mut d, a, "a");
    anchor(&mut d, b, "b");
    d.set_prop(deck, DECK_CURRENT, Value::from("b")).unwrap();
    frame(&mut d);
    assert!(d.flags(a).unwrap().contains(NodeFlags::CONCEALED));
    assert!(!d.flags(b).unwrap().contains(NodeFlags::CONCEALED));
    let hits: Vec<NodeId> = d.hit_list(W).unwrap().entries.iter().map(|e| e.node).collect();
    assert!(!hits.contains(&a) && hits.contains(&b), "a concealed child is not hit");
    assert_eq!(next_focus_of(&d), Some(b), "nor focusable");
    d.set_prop(deck, DECK_CURRENT, Value::from("a")).unwrap();
    frame(&mut d);
    assert!(!d.flags(a).unwrap().contains(NodeFlags::CONCEALED));
    assert!(d.flags(b).unwrap().contains(NodeFlags::CONCEALED));
}

fn next_focus_of(d: &Document) -> Option<NodeId> {
    crate::next_focus(d, W, false)
}

#[test]
fn inert_children_are_disabled() {
    let (mut d, root) = hdoc();
    let deck = child(&mut d, root, DECK, "");
    let inner = child(&mut d, deck, BOX, "");
    let t = child(&mut d, inner, TOGGLE, "");
    d.focus(t, crate::FocusReason::Keyboard);
    d.set_prop(deck, DECK_INERT, Value::Bool(true)).unwrap();
    assert!(d.state(t).contains(StateMask::DISABLED), "inert descendants are :disabled");
    assert!(!d.state(deck).contains(StateMask::DISABLED), "not the container itself");
    assert_eq!(d.focused(W), None, "an inert node loses focus");
    // a node inserted under it is inert too, and stops being so when moved out
    let late = d.create(TOGGLE);
    d.insert(inner, late, None, None).unwrap();
    assert!(d.state(late).contains(StateMask::DISABLED));
    d.insert(root, late, None, None).unwrap();
    assert!(!d.state(late).contains(StateMask::DISABLED));
    d.set_prop(deck, DECK_INERT, Value::Bool(false)).unwrap();
    assert!(!d.state(t).contains(StateMask::DISABLED));
    // its own `disabled` still holds
    d.set_prop(t, common::DISABLED, Value::Bool(true)).unwrap();
    d.set_prop(deck, DECK_INERT, Value::Bool(true)).unwrap();
    d.set_prop(deck, DECK_INERT, Value::Bool(false)).unwrap();
    assert!(d.state(t).contains(StateMask::DISABLED));
}

// ---- (d) peer groups ----

fn radios(d: &mut Document, parent: NodeId, group: &str, n: usize) -> Vec<NodeId> {
    (0..n)
        .map(|_| {
            let r = child(d, parent, RADIO, "");
            d.set_prop(r, RADIO_GROUP, Value::from(group)).unwrap();
            r
        })
        .collect()
}

#[test]
fn checking_a_radio_unchecks_its_peers() {
    let (mut d, root) = hdoc();
    let g = radios(&mut d, root, "g", 3);
    let other = radios(&mut d, root, "h", 1)[0];
    d.set_prop(g[0], RADIO_CHECKED, Value::Bool(true)).unwrap();
    d.set_prop(other, RADIO_CHECKED, Value::Bool(true)).unwrap();
    frame(&mut d);
    // the second radio of the row is at x = 16
    click(&mut d, 20.0, 5.0);
    let checked: Vec<bool> = g.iter().map(|&r| d.view(r).unwrap().prop_bool(RADIO_CHECKED)).collect();
    assert_eq!(checked, [false, true, false]);
    assert!(d.view(other).unwrap().prop_bool(RADIO_CHECKED), "another group is untouched");
    assert!(d.state(g[1]).contains(StateMask::CHECKED) && !d.state(g[0]).contains(StateMask::CHECKED));
}

#[test]
fn a_peer_group_is_one_tab_stop_and_arrows_rove() {
    let (mut d, root) = hdoc();
    let before = child(&mut d, root, TOGGLE, "");
    let g = radios(&mut d, root, "g", 3);
    let after = child(&mut d, root, TOGGLE, "");
    d.set_prop(g[1], RADIO_CHECKED, Value::Bool(true)).unwrap();
    d.focus(before, crate::FocusReason::Keyboard);
    assert_eq!(next_focus_of(&d), Some(g[1]), "Tab goes to the checked member");
    d.focus(g[1], crate::FocusReason::Keyboard);
    assert_eq!(next_focus_of(&d), Some(after), "and leaves the group in one step");
    frame(&mut d);
    d.dispatch(&key(Key::ArrowDown, Modifiers::default()));
    assert_eq!(d.focused(W), Some(g[2]), "the arrow moves focus to the next member");
    assert!(d.view(g[2]).unwrap().prop_bool(RADIO_CHECKED) && !d.view(g[1]).unwrap().prop_bool(RADIO_CHECKED), "and checks it");
    // roving skips disabled members and wraps
    d.set_prop(g[0], common::DISABLED, Value::Bool(true)).unwrap();
    d.dispatch(&key(Key::ArrowDown, Modifiers::default()));
    assert_eq!(d.focused(W), Some(g[1]));
    assert_eq!(next_focus_of(&d), Some(after), "the focused member counts as the stop");
}

// ---- (i) scope activation, (j) label for ----

#[test]
fn enter_and_escape_press_the_scopes_default_and_cancel_buttons() {
    let (mut d, root) = hdoc();
    let dialog = child(&mut d, root, BOX, "");
    d.set_prop(dialog, common::FOCUS_SCOPE, Value::Enum(2)).unwrap();
    let field = child(&mut d, dialog, TOGGLE, "");
    let ok = child(&mut d, dialog, BTN, "");
    let cancel = child(&mut d, dialog, BTN, "");
    d.set_prop(ok, BTN_DEFAULT, Value::Bool(true)).unwrap();
    d.set_prop(cancel, BTN_CANCEL, Value::Bool(true)).unwrap();
    d.listen(ok, BTN_PRESSED, ListenFlags::EMPTY);
    d.listen(cancel, BTN_PRESSED, ListenFlags::EMPTY);
    frame(&mut d);
    assert!(d.state(ok).contains(StateMask::DEFAULT));
    d.focus(field, crate::FocusReason::Keyboard);
    let out = d.dispatch(&key(Key::Enter, Modifiers::default()));
    assert!(out.handled);
    assert!(out.events.iter().any(|e| e.signal == BTN_PRESSED && e.target == ok), "Enter pressed the default button");
    let out = d.dispatch(&key(Key::Escape, Modifiers::default()));
    assert!(out.events.iter().any(|e| e.signal == BTN_PRESSED && e.target == cancel), "Escape pressed the cancel button");
}

#[test]
fn auto_default_moves_default_to_the_focused_button() {
    let (mut d, root) = hdoc();
    d.set_auto_default(true);
    let dialog = child(&mut d, root, BOX, "");
    d.set_prop(dialog, common::FOCUS_SCOPE, Value::Enum(2)).unwrap();
    let ok = child(&mut d, dialog, BTN, "");
    let other = child(&mut d, dialog, BTN, "");
    d.set_prop(ok, BTN_DEFAULT, Value::Bool(true)).unwrap();
    d.listen(other, BTN_PRESSED, ListenFlags::EMPTY);
    d.focus(other, crate::FocusReason::Keyboard);
    assert!(d.state(other).contains(StateMask::DEFAULT) && !d.state(ok).contains(StateMask::DEFAULT));
    let out = d.dispatch(&key(Key::Enter, Modifiers::default()));
    assert!(out.events.iter().any(|e| e.signal == BTN_PRESSED && e.target == other));
    d.blur(W);
    assert!(d.state(ok).contains(StateMask::DEFAULT) && !d.state(other).contains(StateMask::DEFAULT), "back to the declared default");
}

#[test]
fn a_label_focuses_and_labels_its_target() {
    let (mut d, root) = hdoc();
    let l = child(&mut d, root, LABEL, "");
    let t = child(&mut d, root, TOGGLE, "");
    anchor(&mut d, t, "name");
    d.set_prop(l, LABEL_FOR, Value::from("name")).unwrap();
    let c = frame(&mut d).unwrap();
    let tree = c.a11y.expect("a11y");
    let node = &tree.nodes.iter().find(|(id, _)| id.0 == t.0).unwrap().1;
    assert_eq!(node.labelled_by(), &[accesskit::NodeId(l.0)]);
    assert_eq!(node.author_id(), Some("name"));
    click(&mut d, 5.0, 5.0);
    assert_eq!(d.focused(W), Some(t), "a click on the label focuses its target");
    assert_eq!(d.find_anchor(W, "name"), Some(t));
}

#[test]
fn invalid_sets_the_state_and_the_accessible_flag() {
    let (mut d, root) = hdoc();
    let t = child(&mut d, root, TOGGLE, "");
    d.set_prop(t, common::INVALID, Value::Bool(true)).unwrap();
    assert!(d.state(t).contains(StateMask::INVALID));
    let c = frame(&mut d).unwrap();
    let node = &c.a11y.unwrap().nodes.into_iter().find(|(id, _)| id.0 == t.0).unwrap().1;
    assert_eq!(node.invalid(), Some(accesskit::Invalid::True));
}

#[test]
fn enum_attributes_match_by_name() {
    let (mut d, root) = hdoc();
    d.add_sheet("[dialog-role=reject] { width: 33px }");
    let b = child(&mut d, root, BOX, "");
    d.set_prop(b, common::DIALOG_ROLE, Value::Enum(2)).unwrap();
    frame(&mut d);
    assert_eq!(d.layout_rect(b).unwrap().width(), 33.0);
}

#[test]
fn the_status_tip_follows_hover_then_focus() {
    let (mut d, root) = hdoc();
    let a = child(&mut d, root, TOGGLE, "");
    let b = child(&mut d, root, TOGGLE, "");
    d.set_prop(a, common::STATUS_TIP, Value::from("first")).unwrap();
    d.set_prop(b, common::STATUS_TIP, Value::from("second")).unwrap();
    frame(&mut d);
    d.focus(b, crate::FocusReason::Keyboard);
    assert_eq!(d.status_tip(W), Some("second"));
    d.dispatch(&pointer(PointerPhase::Move, 5.0, 5.0, 10));
    assert_eq!(d.status_tip(W), Some("first"), "hover wins over focus");
}

// ---- (c) cancelable defaults ----

#[test]
fn a_prevented_signal_skips_the_default() {
    let (mut d, root) = hdoc();
    let b = child(&mut d, root, BTN, "");
    d.listen(b, BTN_CHANGED, ListenFlags::EMPTY);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let log = seen.clone();
    d.set_sync_delivery(Box::new(move |_doc, ev| {
        log.borrow_mut().push(ev.signal);
        Outcome { prevented: true, stopped: false }
    }));
    d.focus(b, crate::FocusReason::Keyboard);
    let out = d.dispatch(&key(Key::Space, Modifiers::default()));
    assert!(seen.borrow().contains(&BTN_CHANGED), "delivered at event time");
    assert!(!out.events.iter().any(|e| e.signal == BTN_CHANGED), "and not listed again");
    assert!(!d.state(b).contains(StateMask::CHECKED), "the default was skipped");
}

#[test]
fn a_prevented_pointerdown_keeps_the_press_from_the_tree() {
    let (mut d, root) = hdoc();
    let t = child(&mut d, root, TOGGLE, "");
    d.listen(t, common::POINTERDOWN, ListenFlags::EMPTY);
    d.set_sync_delivery(Box::new(|_doc, ev| Outcome { prevented: ev.signal == common::POINTERDOWN, stopped: false }));
    frame(&mut d);
    click(&mut d, 5.0, 5.0);
    assert!(!d.view(t).unwrap().prop_bool(testkinds::TOGGLE_CHECKED), "no click");
    assert_eq!(d.focused(W), None, "no focus");
}

// ---- drag and drop ----

#[test]
fn a_drag_moves_from_source_to_target() {
    let (mut d, root) = hdoc();
    let src = child(&mut d, root, BOX, "width: 40px; height: 40px");
    let dst = child(&mut d, root, BOX, "width: 40px; height: 40px");
    d.set_prop(src, common::DRAG_TYPE, Value::from("cad/part")).unwrap();
    d.set_prop(src, common::DRAG_DATA, Value::from("{\"id\":7}")).unwrap();
    d.set_prop(dst, common::DROP_TYPES, Value::from("text/plain, cad/*")).unwrap();
    for (n, s) in [(src, common::DRAG_BEGIN), (src, common::DRAG_END), (dst, common::DRAG_ENTER), (dst, common::DRAG_LEAVE), (dst, common::DROP)] {
        d.listen(n, s, ListenFlags::EMPTY);
    }
    frame(&mut d);
    let mut events = Vec::new();
    d.dispatch(&pointer(PointerPhase::Down, 5.0, 5.0, 0));
    let mut mv = pointer(PointerPhase::Move, 8.0, 5.0, 10);
    if let presto_base::event::InputEvent::Pointer(p) = &mut mv.event {
        p.buttons = 1;
    }
    assert!(d.dispatch(&mv).events.iter().all(|e| e.signal != common::DRAG_BEGIN), "within the threshold: no drag");
    let mut mv = pointer(PointerPhase::Move, 50.0, 5.0, 20);
    if let presto_base::event::InputEvent::Pointer(p) = &mut mv.event {
        p.buttons = 1;
    }
    events.extend(d.dispatch(&mv).events);
    assert!(d.state(src).contains(StateMask::DRAGGING));
    assert!(d.state(dst).contains(StateMask::DROP_TARGET));
    events.extend(d.dispatch(&pointer(PointerPhase::Up, 50.0, 5.0, 30)).events);
    let order: Vec<(NodeId, SignalId)> = events.iter().map(|e| (e.target, e.signal)).collect();
    assert_eq!(order, [(src, common::DRAG_BEGIN), (dst, common::DRAG_ENTER), (dst, common::DRAG_LEAVE), (dst, common::DROP), (src, common::DRAG_END)]);
    let drop = events.iter().find(|e| e.signal == common::DROP).unwrap().arg.clone().unwrap();
    assert_eq!(drop.field("type"), Some(&Value::from("cad/part")));
    assert_eq!(drop.field("data"), Some(&Value::from("{\"id\":7}")));
    assert_eq!(drop.field("x"), Some(&Value::F64(10.0)), "in the target's own points");
    assert_eq!(drop.field("action"), Some(&Value::from("move")));
    let end = events.iter().find(|e| e.signal == common::DRAG_END).unwrap().arg.clone().unwrap();
    assert_eq!(end.field("dropped"), Some(&Value::Bool(true)));
    assert!(!d.state(src).contains(StateMask::DRAGGING) && !d.state(dst).contains(StateMask::DROP_TARGET));
}

#[test]
fn escape_cancels_a_drag() {
    let (mut d, root) = hdoc();
    let src = child(&mut d, root, BOX, "width: 40px; height: 40px");
    d.set_prop(src, common::DRAG_TYPE, Value::from("text/plain")).unwrap();
    d.listen(src, common::DRAG_END, ListenFlags::EMPTY);
    frame(&mut d);
    d.dispatch(&pointer(PointerPhase::Down, 5.0, 5.0, 0));
    let mut mv = pointer(PointerPhase::Move, 30.0, 30.0, 10);
    if let presto_base::event::InputEvent::Pointer(p) = &mut mv.event {
        p.buttons = 1;
    }
    d.dispatch(&mv);
    let out = d.dispatch(&key(Key::Escape, Modifiers::default()));
    let end = out.events.iter().find(|e| e.signal == common::DRAG_END).expect("drag-end").arg.clone().unwrap();
    assert_eq!(end.field("dropped"), Some(&Value::Bool(false)));
    assert_eq!(end.field("action"), Some(&Value::from("none")));
}

#[test]
fn drop_types_match_by_prefix_and_wildcard() {
    use crate::input::dnd::type_matches;
    assert!(type_matches("*", "cad/part"));
    assert!(type_matches("cad/*", "cad/part"));
    assert!(type_matches("cad/part", "cad/part"));
    assert!(!type_matches("cad/*", "text/plain"));
    assert!(!type_matches("ca*", "cad/part"), "a prefix ends with a slash");
}

#[test]
fn the_deliverer_reads_and_writes_the_document() {
    let (mut d, root) = hdoc();
    let b = child(&mut d, root, BTN, "");
    let other = child(&mut d, root, BOX, "");
    d.set_prop(b, common::TITLE, Value::from("ok")).unwrap();
    d.listen(b, BTN_CHANGED, ListenFlags::EMPTY);
    d.set_sync_delivery(Box::new(move |doc, ev| {
        // a listener reads its target and writes elsewhere, in the event's transaction
        let title = doc.view(ev.target).and_then(|v| v.prop_str(common::TITLE)).unwrap_or("").to_owned();
        let rev = doc.revision();
        doc.js_transaction(rev, |doc| doc.set_prop(other, common::TITLE, Value::from(title.as_str()))).unwrap();
        doc.js_transaction(rev, |doc| doc.set_prop(ev.target, common::ARIA_LABEL, Value::from("renamed"))).unwrap();
        Outcome::default()
    }));
    d.focus(b, crate::FocusReason::Keyboard);
    d.dispatch(&key(Key::Space, Modifiers::default()));
    assert_eq!(d.view(other).unwrap().prop_str(common::TITLE), Some("ok"));
    assert_eq!(d.view(b).unwrap().prop_str(common::ARIA_LABEL), Some("renamed"), "a write to the node mid-handler is kept");
    assert!(d.state(b).contains(StateMask::CHECKED), "not prevented: the default ran");
}
