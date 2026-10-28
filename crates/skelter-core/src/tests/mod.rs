//! skelter-core's tests: the tree, change classes and transactions, commit deltas, dispatch order,
//! focus traversal and the accessibility projection, on the test kinds of [`crate::testkinds`].

mod a11y;
mod absolute;
mod change;
mod commit;
mod direction;
mod dispatch;
mod emits;
mod focus;
mod folds;
mod hooks;
mod overlay;
mod toplayer;
mod prepare;
mod ime;
mod query;
mod runner;
mod services;
mod style;
mod tree;
mod virtual_rows;

use crate::testkinds::{self, FixedText};
use crate::{Document, KindId, StyleInput};
use skelter_base::event::{InputEnvelope, InputEvent, KeyEvent, KeyLocation, KeyState, Modifiers, MouseButton, PointerEvent, PointerPhase, PointerType};
use skelter_base::geom::{Point, Scale, Size, Vec2};
use skelter_base::ids::{InputSeq, NodeId, WindowId};
use skelter_base::key::Key;
use skelter_base::time::Time;

pub(crate) const BOX: KindId = KindId::of("box");
pub(crate) const TEXT: KindId = KindId::of("text");
pub(crate) const TOGGLE: KindId = KindId::of("toggle");
pub(crate) const W: WindowId = WindowId(1);

/// A document with one 200×100 window whose root is a box.
pub(crate) fn doc() -> (Document, NodeId) {
    let mut d = Document::new(testkinds::registry(), Box::new(FixedText));
    d.set_text_shaper(Box::new(FixedText));
    let root = d.create_window(W, BOX, Size::new(200.0, 100.0), Scale(1.0));
    (d, root)
}

/// A child of `parent` with inline style `css`.
pub(crate) fn child(d: &mut Document, parent: NodeId, kind: KindId, css: &str) -> NodeId {
    let n = d.create(kind);
    if !css.is_empty() {
        d.set_style(n, StyleInput { inline: Some(css.into()), ..Default::default() });
    }
    d.insert(parent, n, None, None).unwrap();
    n
}

pub(crate) fn frame(d: &mut Document) -> Option<skelter_base::commit::Commit> {
    d.frame(W, Time(0))
}

static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn envelope(event: InputEvent, t: u64) -> InputEnvelope {
    InputEnvelope { seq: InputSeq(SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)), window: W, os_time: Time(t), recv_time: Time(t), event }
}

pub(crate) fn pointer(phase: PointerPhase, x: f32, y: f32, t: u64) -> InputEnvelope {
    let p = PointerEvent {
        phase,
        pointer: 0,
        kind: PointerType::Mouse,
        button: matches!(phase, PointerPhase::Down | PointerPhase::Up).then_some(MouseButton::Primary),
        buttons: if phase == PointerPhase::Down { 1 } else { 0 },
        pos: Point::new(x, y),
        modifiers: Modifiers::default(),
        pressure: 0.5,
        tilt: Vec2::ZERO,
        history: Default::default(),
    };
    envelope(InputEvent::Pointer(p), t)
}

pub(crate) fn key(k: Key, mods: Modifiers) -> InputEnvelope {
    let e = KeyEvent { key: Some(k), physical: 0, text: None, state: KeyState::Pressed, repeat: false, location: KeyLocation::Standard, modifiers: mods };
    envelope(InputEvent::Key(e), 0)
}

/// Down then up at (x, y).
pub(crate) fn click(d: &mut Document, x: f32, y: f32) -> Vec<crate::Dispatched> {
    vec![d.dispatch(&pointer(PointerPhase::Down, x, y, 1_000)), d.dispatch(&pointer(PointerPhase::Up, x, y, 2_000))]
}
