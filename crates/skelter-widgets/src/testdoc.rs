#![allow(dead_code)]
//! A skelter-core document for the kinds' tests: core's test `box` as the root, this crate's
//! direct and hosted kinds, fixed-metric text, and pointer and key input.

use skelter_base::event::{InputEnvelope, InputEvent, KeyEvent, KeyLocation, KeyState, Modifiers, MouseButton, PointerEvent, PointerPhase, PointerType};
use skelter_base::geom::{Point, Scale, Size, Vec2};
use skelter_base::ids::{InputSeq, NodeId, WindowId};
use skelter_base::key::Key;
use skelter_base::time::Time;
use skelter_core::testkinds::{BoxKind, FixedText};
use skelter_core::{Dispatched, Document, KindId, KindRegistry, StyleInput};
use std::sync::Arc;

pub const W: WindowId = WindowId(1);
pub const BOX: KindId = KindId::of("box");

/// A document with one `size` window whose root is a `box`, with the kinds `register` adds.
pub fn doc(size: Size, register: impl FnOnce(&mut KindRegistry)) -> (Document, NodeId) {
    let mut r = KindRegistry::new();
    r.register::<BoxKind>().unwrap();
    register(&mut r);
    let mut d = Document::new(Arc::new(r), Box::new(FixedText));
    let root = d.create_window(W, BOX, size, Scale(1.0));
    (d, root)
}

/// A child of `parent` with inline style `css`.
pub fn child(d: &mut Document, parent: NodeId, kind: KindId, css: &str) -> NodeId {
    let n = d.create(kind);
    if !css.is_empty() {
        d.set_style(n, StyleInput { inline: Some(css.into()), ..Default::default() });
    }
    d.insert(parent, n, None, None).unwrap();
    n
}

pub fn frame(d: &mut Document) -> Option<skelter_base::commit::Commit> {
    d.frame(W, Time(0))
}

static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn envelope(event: InputEvent, t: u64) -> InputEnvelope {
    InputEnvelope { seq: InputSeq(SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)), window: W, os_time: Time(t), recv_time: Time(t), event }
}

/// A primary-button pointer event at (x, y); a move holds the button when `held`.
pub fn pointer(phase: PointerPhase, x: f32, y: f32, t: u64, held: bool) -> InputEnvelope {
    let p = PointerEvent {
        phase,
        pointer: 0,
        kind: PointerType::Mouse,
        button: matches!(phase, PointerPhase::Down | PointerPhase::Up).then_some(MouseButton::Primary),
        buttons: if phase == PointerPhase::Down || held { 1 } else { 0 },
        pos: Point::new(x, y),
        modifiers: Modifiers::default(),
        pressure: 0.5,
        tilt: Vec2::ZERO,
        history: Default::default(),
    };
    envelope(InputEvent::Pointer(p), t)
}

/// A key press with its text.
pub fn key(k: Key, text: Option<&str>) -> InputEnvelope {
    let e = KeyEvent { key: Some(k), physical: 0, text: text.map(str::to_owned), state: KeyState::Pressed, repeat: false, location: KeyLocation::Standard, modifiers: Modifiers::default() };
    envelope(InputEvent::Key(e), 0)
}

/// The line-start (`end: false`) or line-end key of this platform: Home/End, Cmd+Left/Right on macOS
/// (where Home and End scroll and leave the caret).
pub fn line_edge(end: bool) -> InputEnvelope {
    let (k, m) = if cfg!(target_os = "macos") {
        (if end { Key::ArrowRight } else { Key::ArrowLeft }, Modifiers::SUPER)
    } else {
        (if end { Key::End } else { Key::Home }, Modifiers::default())
    };
    let e = KeyEvent { key: Some(k), physical: 0, text: None, state: KeyState::Pressed, repeat: false, location: KeyLocation::Standard, modifiers: m };
    envelope(InputEvent::Key(e), 0)
}

/// Down then up at (x, y).
pub fn click(d: &mut Document, x: f32, y: f32) -> Vec<Dispatched> {
    vec![d.dispatch(&pointer(PointerPhase::Down, x, y, 1_000, false)), d.dispatch(&pointer(PointerPhase::Up, x, y, 2_000, false))]
}
