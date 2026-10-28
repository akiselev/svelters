//! The kind hooks WIDGETS-BASIC asked for: paint-part styles, whether JS listens, prop-derived state,
//! chrome padding, per-child layout and visibility, inert children, the window scale in the contexts,
//! peer groups (exclusivity, one Tab stop, roving), scope activation (`default`/`cancel`, `:default`),
//! `label for`, cancelable defaults through the sync deliverer, and in-app drag and drop.

use super::*;
use crate::cx::{ChildLayoutCx, EventCx, MeasureCx, PropCx, RecordCx};
use crate::kind::{Event, Handled, Kind, KindSpec, PeerGroup, PeerMsg, PeerTarget, PropSpec, Rove, SignalFlags, SignalSpec};
use crate::value::{PropDefault, ValueType};
use crate::{KindRegistry, ListenFlags, NodeFlags, Outcome, PropId, SignalId, StateMask, Value, common};
use skelter_base::change::ChangeClass;
use skelter_base::event::PointerPhase;
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
            Event::Key(k) if k.state == skelter_base::event::KeyState::Pressed && k.key == Some(Key::ArrowDown) => {
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
    fn chrome_padding(&self, _cx: &mut MeasureCx) -> Option<skelter_base::geom::Insets> {
        (self.caption > 0.0).then(|| skelter_base::geom::Insets { top: self.caption, right: 0.0, bottom: 0.0, left: 0.0 })
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
            Event::Key(k) if k.state == skelter_base::event::KeyState::Pressed && k.key == Some(Key::Space) => {
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

const POP: KindId = KindId::of("pop");
const POP_OPEN: PropId = PropId::of("pop", "open", "bool");

static POP_SPEC: KindSpec = KindSpec {
    props: &[PropSpec::new("open", ValueType::Bool, ChangeClass::Paint).default(PropDefault::Bool(false)).native()],
    container: true,
    ..KindSpec::new("pop")
};

/// A popup: while `open`, a press outside closes it.
#[derive(Default)]
struct Pop;

impl Kind for Pop {
    fn spec() -> &'static KindSpec {
        &POP_SPEC
    }
    fn record(&self, cx: &mut RecordCx) {
        cx.draw_box();
    }
    fn light_dismiss(&self, node: &crate::NodeView) -> bool {
        node.prop_bool(POP_OPEN)
    }
    fn outside_press(&mut self, cx: &mut EventCx, _p: &crate::PointerInput) -> Handled {
        let _ = cx.set_native(POP_OPEN, Value::Bool(false));
        Handled::Yes
    }
}

const SEL: KindId = KindId::of("sel");
const SEL_ON: PropId = PropId::of("sel", "selecting", "bool");

static SEL_SPEC: KindSpec = KindSpec {
    props: &[PropSpec::new("selecting", ValueType::Bool, ChangeClass::Paint).default(PropDefault::Bool(false)).native()],
    focusable: true,
    ..KindSpec::new("sel")
};

/// A text stand-in: a click selects its text and claims the one selection; losing it collapses.
#[derive(Default)]
struct Sel;

impl Kind for Sel {
    fn spec() -> &'static KindSpec {
        &SEL_SPEC
    }
    fn measure(&self, _cx: &mut MeasureCx, _k: taffy::Size<Option<f32>>, _a: taffy::Size<taffy::AvailableSpace>) -> taffy::Size<f32> {
        taffy::Size { width: 20.0, height: 10.0 }
    }
    fn record(&self, cx: &mut RecordCx) {
        cx.draw_box();
    }
    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        if let Event::Click(_) = ev {
            let _ = cx.set_native(SEL_ON, Value::Bool(true));
            cx.claim_selection();
            return Handled::Yes;
        }
        Handled::No
    }
    fn selection_lost(&mut self, cx: &mut EventCx) {
        let _ = cx.set_native(SEL_ON, Value::Bool(false));
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
    r.register::<Pop>().unwrap();
    r.register::<Sel>().unwrap();
    let mut d = Document::new(Arc::new(r), Box::new(FixedText));
    d.set_text_shaper(Box::new(FixedText));
    let root = d.create_window(W, BOX, Size::new(200.0, 100.0), Scale(2.0));
    (d, root)
}

/// The centre of `n`'s box in window points (from the last frame).
fn at(d: &Document, n: NodeId) -> (f32, f32) {
    let r = d.world_rect(n).expect("laid out");
    ((r.min.x + r.max.x) / 2.0, (r.min.y + r.max.y) / 2.0)
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

#[test]
fn a_disabled_container_disables_its_content() {
    // POL2-LAYOUT 4: a disabled frame (any container) takes the inert path an unchecked checkable
    // frame takes: `:disabled`, no focus, AccessKit's disabled flag on every descendant
    let (mut d, root) = hdoc();
    let frame_ = child(&mut d, root, BOX, "");
    let inner = child(&mut d, frame_, BOX, "");
    let t = child(&mut d, inner, TOGGLE, "");
    d.focus(t, crate::FocusReason::Keyboard);
    d.set_prop(frame_, common::DISABLED, Value::Bool(true)).unwrap();
    assert!(d.state(frame_).contains(StateMask::DISABLED), "the container itself");
    assert!(d.state(inner).contains(StateMask::DISABLED) && d.state(t).contains(StateMask::DISABLED), "and every descendant");
    assert_eq!(d.focused(W), None, "a disabled node loses focus");
    let c = frame(&mut d).expect("a frame");
    let a11y = c.a11y.expect("accessibility");
    let node = a11y.nodes.iter().find(|(id, _)| id.0 == t.0).map(|(_, n)| n.clone()).expect("the toggle's node");
    assert!(node.is_disabled(), "AccessKit's disabled flag");
    // a late child is disabled too; enabling the container enables the content, its own prop holds
    let late = d.create(TOGGLE);
    d.insert(frame_, late, None, None).unwrap();
    assert!(d.state(late).contains(StateMask::DISABLED));
    d.set_prop(late, common::DISABLED, Value::Bool(true)).unwrap();
    d.set_prop(frame_, common::DISABLED, Value::Bool(false)).unwrap();
    assert!(!d.state(t).contains(StateMask::DISABLED) && !d.state(inner).contains(StateMask::DISABLED));
    assert!(d.state(late).contains(StateMask::DISABLED));
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
    let (x, y) = at(&d, g[1]);
    click(&mut d, x, y);
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
    if let skelter_base::event::InputEvent::Pointer(p) = &mut mv.event {
        p.buttons = 1;
    }
    assert!(d.dispatch(&mv).events.iter().all(|e| e.signal != common::DRAG_BEGIN), "within the threshold: no drag");
    let (tx, ty) = (d.world_rect(dst).unwrap().min.x + 10.0, d.world_rect(dst).unwrap().min.y + 5.0);
    let mut mv = pointer(PointerPhase::Move, tx, ty, 20);
    if let skelter_base::event::InputEvent::Pointer(p) = &mut mv.event {
        p.buttons = 1;
    }
    events.extend(d.dispatch(&mv).events);
    assert!(d.state(src).contains(StateMask::DRAGGING));
    assert!(d.state(dst).contains(StateMask::DROP_TARGET));
    events.extend(d.dispatch(&pointer(PointerPhase::Up, tx, ty, 30)).events);
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
    if let skelter_base::event::InputEvent::Pointer(p) = &mut mv.event {
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

// ---- kinds that scroll themselves, carets, padding ----

const PAGE: KindId = KindId::of("page");

static PAGE_SPEC: KindSpec = KindSpec { focusable: true, ..KindSpec::new("page") };

/// A code-editor stand-in: 1000 points of content it scrolls itself; Down scrolls by 10 fast.
#[derive(Default)]
struct Page;

impl Kind for Page {
    fn spec() -> &'static KindSpec {
        &PAGE_SPEC
    }
    fn record(&self, cx: &mut RecordCx) {
        cx.draw_box();
    }
    fn self_scroll(&self, _node: &crate::NodeView) -> Option<crate::SelfScroll> {
        Some(crate::SelfScroll { content: Size::new(50.0, 1000.0), x: false, y: true })
    }
    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        match ev {
            Event::Key(k) if k.key == Some(Key::ArrowDown) => {
                let off = cx.document().scroll_offset(cx.node());
                cx.scroll_fast(skelter_base::geom::Vec2::new(0.0, off.y + 10.0));
                cx.set_caret(Some(skelter_base::geom::Rect::new(1.0, 2.0, 2.0, 12.0)));
                Handled::Yes
            }
            _ => Handled::No,
        }
    }
}

#[test]
fn a_kind_that_scrolls_itself_scrolls_on_the_compositor() {
    let mut r = KindRegistry::new();
    r.register::<testkinds::BoxKind>().unwrap();
    r.register::<Page>().unwrap();
    let mut d = Document::new(Arc::new(r), Box::new(FixedText));
    let root = d.create_window(W, BOX, Size::new(200.0, 100.0), Scale(1.0));
    let page = child(&mut d, root, PAGE, "width: 50px; height: 80px; margin-left: 10px; background-color: #ffffff");
    let c = frame(&mut d).unwrap();
    let scroll = c.props.ops.iter().any(|op| matches!(op, skelter_base::props::PropOp::Scroll(..)));
    assert!(scroll, "it has a scroll node");
    assert!(c.layers.iter().any(|l| matches!(l, skelter_base::layer::LayerDelta::Add(desc) if matches!(desc.kind, skelter_base::layer::LayerKind::Scroll { .. }))));
    d.focus(page, crate::FocusReason::Keyboard);
    // the focus move re-records the node (its ring): not the scroll's frame
    frame(&mut d);
    d.take_fast_commits();
    d.dispatch(&key(Key::ArrowDown, Modifiers::default()));
    let fast = d.take_fast_commits();
    assert!(fast.iter().any(|f| matches!(f, skelter_base::commit::FastCommit::ScrollOffset { offset, .. } if offset.y == 10.0)));
    let caret = fast.iter().find_map(|f| match f {
        skelter_base::commit::FastCommit::Caret { rect, transform, .. } => Some((*rect, *transform)),
        _ => None,
    });
    let (rect, space) = caret.expect("a caret");
    assert_eq!(rect.map(|r| r.min.x), Some(1.0), "the caret is in the node's own space");
    assert_eq!(Some(space), d.record[d.ix(page)].transform, "the node's transform node, which places it 10 points in");
    let c = frame(&mut d).unwrap();
    assert!(c.lists.is_empty(), "a scroll is no re-record");
    let resent = |c: &skelter_base::commit::Commit| c.a11y.as_ref().is_some_and(|u| u.nodes.iter().any(|(id, _)| id.0 == page.0));
    assert!(resent(&c), "its accessibility node re-sends with the offset (text runs follow it)");
    // a wheel or fling scroll the compositor reports does too
    let sc = d.record[d.ix(page)].scroll.expect("its scroll node");
    d.apply_scroll(W, sc, skelter_base::geom::Vec2::new(0.0, 40.0));
    assert!(resent(&frame(&mut d).unwrap()));
}

const FIXED_PAGE: KindId = KindId::of("fixed-page");
static FIXED_PAGE_SPEC: KindSpec = KindSpec { focusable: true, ..KindSpec::new("fixed-page") };

/// [`Page`] with its box as the fixed part and a line as its scrolled content.
#[derive(Default)]
struct FixedPage;

impl Kind for FixedPage {
    fn spec() -> &'static KindSpec {
        &FIXED_PAGE_SPEC
    }
    fn record(&self, cx: &mut RecordCx) {
        cx.fixed(|cx| cx.draw_box());
        cx.draw_line(Point::new(0.0, 500.0), Point::new(50.0, 500.0), 1.0);
    }
    fn self_scroll(&self, _node: &crate::NodeView) -> Option<crate::SelfScroll> {
        Some(crate::SelfScroll { content: Size::new(50.0, 1000.0), x: false, y: true })
    }
    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        match ev {
            Event::Key(k) if k.key == Some(Key::ArrowDown) => {
                let off = cx.document().scroll_offset(cx.node());
                cx.scroll_fast(skelter_base::geom::Vec2::new(0.0, off.y + 10.0));
                Handled::Yes
            }
            _ => Handled::No,
        }
    }
}

#[test]
fn a_self_scrolling_kinds_fixed_part_stays_out_of_its_scroll_layer() {
    use skelter_base::display::Prim;
    use skelter_base::layer::{LayerDelta, LayerKind};
    let mut r = KindRegistry::new();
    r.register::<testkinds::BoxKind>().unwrap();
    r.register::<FixedPage>().unwrap();
    let mut d = Document::new(Arc::new(r), Box::new(FixedText));
    let root = d.create_window(W, BOX, Size::new(200.0, 100.0), Scale(1.0));
    let page = child(&mut d, root, FIXED_PAGE, "width: 50px; height: 80px; background-color: #ffffff");
    let c = frame(&mut d).unwrap();
    let fixed_key = crate::record::fixed_key(page);
    let list = |n: NodeId| c.lists.iter().find(|(k, _)| *k == n).map(|(_, l)| l.clone()).expect("a list");
    assert!(matches!(list(fixed_key).items[..], [Prim::Quad(_)]), "the box is the fixed part");
    assert!(matches!(list(page).items[..], [Prim::Line(_)]), "the line is the scrolled content");
    let scroll_layer = c.layers.iter().find_map(|l| match l {
        LayerDelta::Add(desc) if matches!(desc.kind, LayerKind::Scroll { .. }) => Some(desc.id),
        _ => None,
    });
    let items_of = |layer| {
        c.layers.iter().find_map(|l| match l {
            LayerDelta::Items { layer: id, items } if *id == layer => Some(items.clone()),
            _ => None,
        })
    };
    let p = d.record[d.ix(page)].clone();
    let root_layer = d.window(W).unwrap().root_layer.unwrap();
    let root_items = items_of(root_layer).expect("root items");
    let fixed = root_items.iter().find(|it| it.node == fixed_key).expect("the fixed part in the parent's layer");
    assert_eq!(Some(fixed.transform), p.transform, "in the node's own space");
    let content = items_of(scroll_layer.expect("a scroll layer")).expect("scroll layer items");
    assert!(content.iter().any(|it| it.node == page && Some(it.transform) == p.content), "the rest scrolls");
    // a scroll moves the content on the compositor: nothing is re-recorded
    d.focus(page, crate::FocusReason::Keyboard);
    frame(&mut d);
    d.dispatch(&key(Key::ArrowDown, Modifiers::default()));
    assert!(d.take_fast_commits().iter().any(|f| matches!(f, skelter_base::commit::FastCommit::ScrollOffset { .. })));
    let c = frame(&mut d).unwrap();
    assert!(c.lists.is_empty(), "a scroll is no re-record");
    // removing the node removes both lists
    d.remove(root, page).unwrap();
    let c = frame(&mut d).unwrap();
    assert!(c.removed.contains(&page) && c.removed.contains(&fixed_key));
}

const PREP_PAGE: KindId = KindId::of("prep-page");
static PREP_PAGE_SPEC: KindSpec = KindSpec { props: &[PropSpec::new("to", ValueType::F64, ChangeClass::Paint).default(PropDefault::F64(0.0))], ..KindSpec::new("prep-page") };
const PREP_TO: PropId = PropId::of("prep-page", "to", "number");

/// [`FixedPage`] that scrolls in its prepare (a layout that scrolls its caret into view): to `to`.
#[derive(Default)]
struct PrepPage;

impl Kind for PrepPage {
    fn spec() -> &'static KindSpec {
        &PREP_PAGE_SPEC
    }
    fn prepare(&mut self, cx: &mut crate::PrepareCx) {
        let to = cx.prop(PREP_TO).and_then(Value::as_f64).unwrap_or(0.0) as f32;
        if cx.scroll_offset().y != to {
            cx.scroll_to(skelter_base::geom::Vec2::new(0.0, to));
            cx.set_caret(Some(skelter_base::geom::Rect::new(1.0, 2.0, 2.0, 12.0)));
        }
    }
    fn record(&self, cx: &mut RecordCx) {
        cx.fixed(|cx| cx.draw_box());
        cx.draw_line(Point::new(0.0, 500.0), Point::new(50.0, 500.0), 1.0);
    }
    fn self_scroll(&self, _node: &crate::NodeView) -> Option<crate::SelfScroll> {
        Some(crate::SelfScroll { content: Size::new(50.0, 1000.0), x: false, y: true })
    }
}

#[test]
fn a_prepare_scrolls_through_the_fast_path_clamped_to_the_content() {
    let mut r = KindRegistry::new();
    r.register::<testkinds::BoxKind>().unwrap();
    r.register::<PrepPage>().unwrap();
    let mut d = Document::new(Arc::new(r), Box::new(FixedText));
    let root = d.create_window(W, BOX, Size::new(200.0, 100.0), Scale(1.0));
    let page = child(&mut d, root, PREP_PAGE, "width: 50px; height: 80px; background-color: #ffffff");
    frame(&mut d);
    d.take_fast_commits();
    d.set_prop(page, PREP_TO, Value::F64(30.0)).unwrap();
    frame(&mut d).unwrap();
    let fast = d.take_fast_commits();
    assert!(fast.iter().any(|f| matches!(f, skelter_base::commit::FastCommit::ScrollOffset { offset, .. } if offset.y == 30.0)), "{fast:?}");
    assert!(fast.iter().any(|f| matches!(f, skelter_base::commit::FastCommit::Caret { rect: Some(_), .. })), "the prepare's caret");
    assert_eq!(d.scroll_offset(page).y, 30.0);
    let c = frame(&mut d);
    assert!(c.is_none_or(|c| c.lists.is_empty()), "the scroll is no re-record");
    // past the end: clamped to the content the kind gave (1000 - 80)
    d.set_prop(page, PREP_TO, Value::F64(5000.0)).unwrap();
    frame(&mut d);
    assert_eq!(d.scroll_offset(page).y, 920.0);
}

const FIXED_FIRST: KindId = KindId::of("fixed-first");
static FIXED_FIRST_SPEC: KindSpec = KindSpec::new("fixed-first");

/// Records a line, then its fixed part (box, and a nested fixed line); it does not scroll itself.
#[derive(Default)]
struct FixedFirst;

impl Kind for FixedFirst {
    fn spec() -> &'static KindSpec {
        &FIXED_FIRST_SPEC
    }
    fn record(&self, cx: &mut RecordCx) {
        cx.draw_line(Point::new(0.0, 0.0), Point::new(1.0, 0.0), 1.0);
        let n = cx.fixed(|cx| {
            cx.draw_box();
            cx.fixed(|cx| cx.draw_line(Point::new(0.0, 2.0), Point::new(1.0, 2.0), 1.0));
            7
        });
        assert_eq!(n, 7);
    }
}

#[test]
fn a_fixed_part_of_a_kind_that_does_not_scroll_itself_goes_first() {
    use skelter_base::display::Prim;
    let mut r = KindRegistry::new();
    r.register::<testkinds::BoxKind>().unwrap();
    r.register::<FixedFirst>().unwrap();
    let mut d = Document::new(Arc::new(r), Box::new(FixedText));
    let root = d.create_window(W, BOX, Size::new(200.0, 100.0), Scale(1.0));
    let a = child(&mut d, root, FIXED_FIRST, "width: 50px; height: 50px; background-color: #ffffff");
    let c = frame(&mut d).unwrap();
    assert!(!c.lists.iter().any(|(k, _)| *k == crate::record::fixed_key(a)), "no separate fixed list");
    let list = c.lists.iter().find(|(k, _)| *k == a).map(|(_, l)| l.clone()).expect("a list");
    assert!(matches!(list.items[..], [Prim::Quad(_), Prim::Line(_), Prim::Line(_)]), "{:?}", list.items);
}

fn carets(d: &mut Document) -> Vec<Option<skelter_base::geom::Rect>> {
    d.take_fast_commits()
        .into_iter()
        .filter_map(|f| match f {
            skelter_base::commit::FastCommit::Caret { rect, .. } => Some(rect),
            _ => None,
        })
        .collect()
}

#[test]
fn no_caret_for_an_editor_that_isnt_rendered() {
    let mut r = KindRegistry::new();
    r.register::<testkinds::BoxKind>().unwrap();
    r.register::<Page>().unwrap();
    let mut d = Document::new(Arc::new(r), Box::new(FixedText));
    let root = d.create_window(W, BOX, Size::new(200.0, 100.0), Scale(1.0));
    let css = "width: 50px; height: 80px; color: #ff0000";
    let page = child(&mut d, root, PAGE, css);
    frame(&mut d);
    d.focus(page, crate::FocusReason::Keyboard);
    // the focus move re-records the node (its ring): not the scroll's frame
    frame(&mut d);
    d.take_fast_commits();
    d.dispatch(&key(Key::ArrowDown, Modifiers::default()));
    let fast = d.take_fast_commits();
    let color = fast.iter().find_map(|f| match f {
        skelter_base::commit::FastCommit::Caret { color, rect: Some(_), .. } => Some(*color),
        _ => None,
    });
    assert_eq!(color, Some(skelter_base::color::Color::rgba(1.0, 0.0, 0.0, 1.0)), "the caret takes the node's colour");
    let restyle = |d: &mut Document, extra: &str| {
        d.set_style(page, crate::StyleInput { inline: Some(format!("{css}{extra}").into()), ..Default::default() });
    };
    // display: none hides the caret once, then the editor sends none while it stays hidden
    restyle(&mut d, "; display: none");
    frame(&mut d);
    assert_eq!(carets(&mut d), vec![None]);
    d.dispatch(&key(Key::ArrowDown, Modifiers::default()));
    frame(&mut d);
    assert_eq!(carets(&mut d), Vec::<Option<skelter_base::geom::Rect>>::new(), "no caret while hidden");
    // rendered again: the last placed caret comes back
    restyle(&mut d, "");
    frame(&mut d);
    assert_eq!(carets(&mut d), vec![Some(skelter_base::geom::Rect::new(1.0, 2.0, 2.0, 12.0))]);
    // visibility: hidden counts too
    restyle(&mut d, "; visibility: hidden");
    frame(&mut d);
    assert_eq!(carets(&mut d), vec![None]);
    // caret-color wins over color; `auto` falls back to color
    let caret_color = |d: &mut Document, extra: &str| {
        restyle(d, extra);
        frame(d);
        // hidden, the page lost focus (the focus fixup rule): shown again, it takes it back
        d.focus(page, crate::FocusReason::Keyboard);
        d.take_fast_commits();
        d.dispatch(&key(Key::ArrowDown, Modifiers::default()));
        d.take_fast_commits().iter().find_map(|f| match f {
            skelter_base::commit::FastCommit::Caret { color, rect: Some(_), .. } => Some(*color),
            _ => None,
        })
    };
    let blue = skelter_base::color::Color::rgba(0.0, 0.0, 1.0, 1.0);
    assert_eq!(caret_color(&mut d, "; caret-color: #0000ff"), Some(blue), "caret-color over color");
    assert_eq!(caret_color(&mut d, "; caret-color: auto"), color, "auto = color");
}

#[test]
fn views_carry_padding_and_the_window_transform() {
    let (mut d, root) = hdoc();
    let a = child(&mut d, root, BOX, "width: 50px; height: 50px; margin-left: 20px; padding: 3px 4px; border-width: 1px; border-style: solid");
    frame(&mut d);
    let v = d.view(a).unwrap();
    assert_eq!(v.padding().left, 4.0);
    assert_eq!(v.content_box().min.x, 5.0, "border, then padding");
    assert_eq!(v.window_transform().e, 20.0);
    assert_eq!(v.from_window(Point::new(25.0, 0.0)).x, 5.0);
}

#[test]
fn nested_sync_events_follow_the_listener() {
    let (mut d, root) = hdoc();
    let b = child(&mut d, root, BTN, "");
    let t = child(&mut d, root, TOGGLE, "");
    d.listen(b, BTN_CHANGED, ListenFlags::EMPTY);
    d.listen(t, common::FOCUS, ListenFlags::EMPTY);
    let seen: Rc<RefCell<Vec<(SignalId, bool)>>> = Rc::default();
    let log = seen.clone();
    d.set_sync_delivery(Box::new(move |doc, ev| {
        log.borrow_mut().push((ev.signal, ev.cancelable));
        if ev.signal == BTN_CHANGED {
            // a listener moves focus: its focus events come after this call, not inside it
            doc.focus(t, crate::FocusReason::Programmatic);
            assert_eq!(log.borrow().len(), 1, "not re-entered");
        }
        Outcome { prevented: ev.signal == common::FOCUS, stopped: false }
    }));
    d.focus(b, crate::FocusReason::Keyboard);
    d.take_events();
    d.dispatch(&key(Key::Space, Modifiers::default()));
    assert_eq!(*seen.borrow(), [(BTN_CHANGED, true), (common::FOCUS, false)], "delivered, not cancelable");
    assert_eq!(d.focused(W), Some(t));
}

#[test]
fn a_press_outside_an_open_surface_dismisses_it() {
    let (mut d, root) = hdoc();
    let pop = child(&mut d, root, POP, "width: 40px; height: 40px");
    let inner = child(&mut d, pop, TOGGLE, "");
    let outside = child(&mut d, root, TOGGLE, "");
    d.set_prop(pop, POP_OPEN, Value::Bool(true)).unwrap();
    frame(&mut d);
    let (x, y) = at(&d, inner);
    click(&mut d, x, y);
    assert!(d.view(pop).unwrap().prop_bool(POP_OPEN), "a press inside keeps it open");
    assert!(d.view(inner).unwrap().prop_bool(testkinds::TOGGLE_CHECKED));
    let (x, y) = at(&d, outside);
    click(&mut d, x, y);
    assert!(!d.view(pop).unwrap().prop_bool(POP_OPEN), "a press outside closes it");
    assert!(d.view(outside).unwrap().prop_bool(testkinds::TOGGLE_CHECKED), "and is dispatched as usual");
}

#[test]
fn one_selection_owner_under_the_gtk_rule() {
    let (mut d, root) = hdoc();
    let a = child(&mut d, root, SEL, "");
    let b = child(&mut d, root, SEL, "");
    frame(&mut d);
    let on = |d: &Document, n| d.view(n).unwrap().prop_bool(SEL_ON);
    let ((ax, ay), (bx, by)) = (at(&d, a), at(&d, b));
    click(&mut d, ax, ay);
    click(&mut d, bx, by);
    assert!(on(&d, a) && on(&d, b), "off by default: both keep their selection");
    d.set_single_selection(true);
    click(&mut d, ax, ay);
    click(&mut d, bx, by);
    assert!(!on(&d, a) && on(&d, b), "claiming collapses the previous owner");
}

#[test]
fn a_drag_started_from_outside_the_tree_drops_like_any_other() {
    let (mut d, root) = hdoc();
    let island = child(&mut d, root, BOX, "width: 40px; height: 40px");
    let dst = child(&mut d, root, BOX, "width: 40px; height: 40px");
    d.set_prop(dst, common::DROP_TYPES, Value::from("cad/*")).unwrap();
    d.listen(island, common::DRAG_BEGIN, ListenFlags::EMPTY);
    d.listen(dst, common::DROP, ListenFlags::EMPTY);
    frame(&mut d);
    // the press went to the island (an egui drag), then skelter takes the drag over
    let (x, y) = at(&d, island);
    d.dispatch(&pointer(PointerPhase::Down, x, y, 0));
    assert!(d.start_drag(W, 0, island, "cad/part", "7"));
    assert!(!d.start_drag(W, 0, island, "cad/part", "7"), "one drag at a time");
    assert!(d.state(island).contains(StateMask::DRAGGING));
    assert!(d.take_events().iter().any(|e| e.signal == common::DRAG_BEGIN));
    let (tx, ty) = at(&d, dst);
    let mut mv = pointer(PointerPhase::Move, tx, ty, 10);
    if let skelter_base::event::InputEvent::Pointer(p) = &mut mv.event {
        p.buttons = 1;
    }
    d.dispatch(&mv);
    assert!(d.state(dst).contains(StateMask::DROP_TARGET));
    let up = d.dispatch(&pointer(PointerPhase::Up, tx, ty, 20));
    let drop = up.events.iter().find(|e| e.signal == common::DROP).expect("drop").arg.clone().unwrap();
    assert_eq!(drop.field("type"), Some(&Value::from("cad/part")));
    assert_eq!(drop.field("data"), Some(&Value::from("7")));
}
