//! skelter events → egui events, one translator at a time.

use skelter_base::event::{
    GestureEvent, GesturePhase, ImeEvent, InputEvent, KeyEvent, KeyLocation, KeyState, Modifiers, ScrollDelta,
    WheelEvent,
};
use skelter_base::geom::{Point, Rect, Vec2};
use skelter_base::key::Key;
use skelter_egui_island::input::{InputTranslator, egui_key, modifiers};

fn translator() -> InputTranslator {
    InputTranslator::new(Rect::new(100.0, 50.0, 400.0, 250.0)).with_macos(false)
}

fn key(key: Key, text: Option<&str>, modifiers: Modifiers) -> InputEvent {
    InputEvent::Key(KeyEvent {
        key: Some(key),
        physical: 0,
        text: text.map(str::to_owned),
        state: KeyState::Pressed,
        repeat: false,
        location: KeyLocation::Standard,
        modifiers,
    })
}

fn wheel(pos: Point) -> InputEvent {
    InputEvent::Wheel(WheelEvent {
        pos,
        delta: ScrollDelta::Pixels(Vec2::new(0.0, -12.0)),
        phase: Some(GesturePhase::Moved),
        momentum: None,
        modifiers: Modifiers::default(),
    })
}

#[test]
fn every_skelter_key_has_its_egui_name() {
    for name in ["ArrowDown", "Escape", "Tab", "A", "Z", "0", "F1", "F35", "OpenBracket", "Questionmark"] {
        let k = Key::from_name(name).unwrap_or_else(|| panic!("skelter has {name}"));
        // egui's `name()` gives display names ("Down"); `from_name` takes the variant names skelter keeps.
        assert_eq!(egui_key(k), egui::Key::from_name(name));
        assert!(egui_key(k).is_some());
    }
}

#[test]
fn modifiers_follow_the_platform_command_key() {
    let ctrl = modifiers(Modifiers::CTRL, false);
    assert!(ctrl.ctrl && ctrl.command && !ctrl.mac_cmd);
    let cmd = modifiers(Modifiers::SUPER, true);
    assert!(cmd.mac_cmd && cmd.command && !cmd.ctrl);
    let altgr = modifiers(Modifiers::ALT_GR, false);
    assert!(altgr.alt && !altgr.ctrl);
}

#[test]
fn clipboard_shortcuts_become_egui_commands() {
    let mut t = translator();
    let mut out = Vec::new();
    t.set_focused(true, &mut out);
    out.clear();
    t.translate(&key(Key::C, Some("c"), Modifiers::CTRL), &mut out);
    t.translate(&key(Key::X, Some("x"), Modifiers::CTRL), &mut out);
    assert_eq!(out, vec![egui::Event::Copy, egui::Event::Cut]);
    let routed = t.translate(&key(Key::V, Some("v"), Modifiers::CTRL), &mut out);
    assert!(routed.paste && routed.consumed, "the island reads the clipboard for a paste");
    assert_eq!(out.len(), 2, "a paste queues nothing until the clipboard is read");
}

#[test]
fn keys_need_focus_and_text_skips_control_characters() {
    let mut t = translator();
    let mut out = Vec::new();
    assert!(!t.translate(&key(Key::A, Some("a"), Modifiers::default()), &mut out).consumed);
    assert!(out.is_empty());
    t.set_focused(true, &mut out);
    assert_eq!(out, vec![egui::Event::WindowFocused(true)]);
    out.clear();
    t.translate(&key(Key::A, Some("a"), Modifiers::SHIFT), &mut out);
    assert!(matches!(out[0], egui::Event::Key { key: egui::Key::A, pressed: true, .. }));
    assert_eq!(out[1], egui::Event::Text("a".to_owned()));
    out.clear();
    // macOS sends Delete as a private-use character.
    t.translate(&key(Key::Delete, Some("\u{f728}"), Modifiers::default()), &mut out);
    assert_eq!(out.len(), 1);
}

#[test]
fn ime_preedit_ranges_become_chars() {
    let mut t = translator();
    let mut out = Vec::new();
    t.set_focused(true, &mut out);
    out.clear();
    // "日本" is 6 bytes; the cursor after the first character is byte 3, char 1.
    t.translate(&InputEvent::Ime(ImeEvent::Preedit { text: "日本".to_owned(), cursor: Some((3, 6)) }), &mut out);
    t.translate(&InputEvent::Ime(ImeEvent::Commit("日本".to_owned())), &mut out);
    assert_eq!(
        out,
        vec![
            egui::Event::Ime(egui::ImeEvent::Preedit { text: "日本".to_owned(), active_range_chars: Some(1..2) }),
            egui::Event::Ime(egui::ImeEvent::Commit("日本".to_owned())),
        ]
    );
}

#[test]
fn wheel_inside_moves_the_pointer_first_and_keeps_the_sign() {
    let mut t = translator();
    let mut out = Vec::new();
    assert!(!t.translate(&wheel(Point::new(10.0, 10.0)), &mut out).consumed);
    assert!(out.is_empty());
    assert!(t.translate(&wheel(Point::new(110.0, 60.0)), &mut out).consumed);
    assert_eq!(out[0], egui::Event::PointerMoved(egui::pos2(10.0, 10.0)));
    match &out[1] {
        egui::Event::MouseWheel { unit, delta, phase, source, .. } => {
            assert_eq!(*unit, egui::MouseWheelUnit::Point);
            assert_eq!(*delta, egui::vec2(0.0, -12.0));
            assert_eq!(*phase, egui::TouchPhase::Move);
            assert_eq!(*source, egui::MouseWheelSource::Trackpad);
        }
        other => panic!("expected a wheel, got {other:?}"),
    }
}

#[test]
fn pinch_over_the_island_is_egui_zoom() {
    let mut t = translator();
    let mut out = Vec::new();
    t.translate(&wheel(Point::new(110.0, 60.0)), &mut out);
    out.clear();
    t.translate(&InputEvent::Gesture(GestureEvent::Pinch { delta: 0.0, phase: GesturePhase::Moved }), &mut out);
    assert_eq!(out, vec![egui::Event::Zoom(1.0)]);
}

#[test]
fn raw_motion_over_the_island_is_egui_mouse_moved() {
    let mut t = translator();
    let mut out = Vec::new();
    let raw = InputEvent::RawMotion { dx: 3.0, dy: -2.0 };
    assert!(!t.translate(&raw, &mut out).consumed);
    assert!(out.is_empty());
    t.translate(&wheel(Point::new(110.0, 60.0)), &mut out);
    out.clear();
    assert!(t.translate(&raw, &mut out).consumed);
    assert_eq!(out, vec![egui::Event::MouseMoved(egui::vec2(3.0, -2.0))]);
}

#[test]
fn physical_keys_map_by_us_layout_position() {
    use skelter_egui_island::input::physical_key;
    assert_eq!(physical_key(0x07_0004), Some(egui::Key::A));
    assert_eq!(physical_key(0x07_001D), Some(egui::Key::Z));
    assert_eq!(physical_key(0x07_0027), Some(egui::Key::Num0));
    assert_eq!(physical_key(0x07_0059), Some(egui::Key::Num1), "numpad digits fold as egui-winit folds them");
    assert_eq!(physical_key(0x07_0058), Some(egui::Key::Enter));
    assert_eq!(physical_key(0x07_002F), Some(egui::Key::OpenBracket));
    assert_eq!(physical_key(0x07_0073), Some(egui::Key::F24));
    assert_eq!(physical_key(0x07_0039), None, "Caps Lock has no egui key");
    assert_eq!(physical_key(0x0C_00E9), None, "consumer page");
}

#[test]
fn keys_carry_their_physical_key_and_shortcuts_fall_back_to_it() {
    let mut t = translator();
    let mut out = Vec::new();
    t.set_focused(true, &mut out);
    out.clear();
    let press = |key: Option<Key>, physical: u32, text: Option<&str>, modifiers: Modifiers| {
        InputEvent::Key(KeyEvent {
            key,
            physical,
            text: text.map(str::to_owned),
            state: KeyState::Pressed,
            repeat: false,
            location: KeyLocation::Standard,
            modifiers,
        })
    };
    // A Cyrillic layout: the key at C's position types "с" and has no Latin logical key.
    t.translate(&press(None, 0x07_0006, Some("с"), Modifiers::CTRL), &mut out);
    assert_eq!(out, vec![egui::Event::Copy], "Ctrl+(the key at C) copies");
    out.clear();
    t.translate(&press(Some(Key::Q), 0x07_0004, Some("q"), Modifiers::default()), &mut out);
    assert!(
        matches!(out[0], egui::Event::Key { key: egui::Key::Q, physical_key: Some(egui::Key::A), .. }),
        "AZERTY: logical Q at A's position, got {:?}",
        out[0]
    );
}
