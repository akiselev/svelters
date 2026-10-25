//! The harness on a small tree: a document with a counter button, a label, a checkbox and a text field,
//! projected to AccessKit at each frame. It exercises the verbs (`run`, `step`, `get_by_*`, `click`,
//! `key_press`, `type_text`, `event_egui`), the resize handshake, compositor animations, and the latency
//! invariant that every injected input terminates exactly once.

use presto::latency::{InputClass, Segment, Termination};
use presto::seams::{Dispatched, DocumentApi};
use presto_base::accesskit::{
    self, Action, ActionRequest, Node as AkNode, Role, Toggled, TreeInfo, TreeUpdate,
};
use presto_base::anim::{
    AnimProperty, AnimValue, AnimationCmd, AnimationEvent, AnimationSpec, Easing, FillMode,
    Generator, Keyframe, PlaybackDirection, StartTime, Timing,
};
use presto_base::commit::Commit;
use presto_base::event::{
    ImeEvent, InputEnvelope, InputEvent, KeyState, PointerPhase, WindowEvent,
};
use presto_base::geom::{Point, Rect, Scale, Size, Vec2};
use presto_base::ids::{AnimationId, NodeId, PropNodeId, Revision, WindowId};
use presto_base::key::Key;
use presto_base::time::{Span, Time};
use presto_test::{By, Harness};

const ROOT: u64 = 1;
const BUTTON: u64 = 2;
const LABEL: u64 = 3;
const CHECK: u64 = 4;
const FIELD: u64 = 5;
const DUP_A: u64 = 6;
const DUP_B: u64 = 7;

const BUTTON_RECT: Rect = Rect::new(10.0, 10.0, 110.0, 40.0);
const CHECK_RECT: Rect = Rect::new(10.0, 50.0, 110.0, 70.0);
const FIELD_RECT: Rect = Rect::new(10.0, 80.0, 210.0, 100.0);

#[derive(Default)]
struct Doc {
    count: u32,
    checked: bool,
    text: String,
    focus: Option<u64>,
    pressed: bool,
    hovered: Option<Point>,
    dirty: bool,
    revision: u64,
    size: Option<(Size, Scale)>,
    resizes: u32,
    animate_next_frame: Option<Span>,
    animation_events: Vec<AnimationEvent>,
    frames: u32,
    duplicates: bool,
    hovers: Vec<Point>,
    menu: Vec<String>,
}

impl Doc {
    fn node(role: Role, label: &str, rect: Rect, author: &str) -> AkNode {
        let mut n = AkNode::new(role);
        n.set_label(label);
        n.set_author_id(author);
        n.set_bounds(accesskit::Rect {
            x0: rect.min.x as f64,
            y0: rect.min.y as f64,
            x1: rect.max.x as f64,
            y1: rect.max.y as f64,
        });
        n
    }

    fn tree(&self) -> TreeUpdate {
        let mut root = AkNode::new(Role::Window);
        let mut children = vec![
            accesskit::NodeId(BUTTON),
            accesskit::NodeId(LABEL),
            accesskit::NodeId(CHECK),
            accesskit::NodeId(FIELD),
        ];
        let mut nodes = vec![
            (
                accesskit::NodeId(BUTTON),
                Self::node(Role::Button, "Increment", BUTTON_RECT, "inc"),
            ),
            (
                accesskit::NodeId(LABEL),
                Self::node(
                    Role::Label,
                    &format!("Count: {}", self.count),
                    Rect::new(120.0, 10.0, 220.0, 40.0),
                    "count",
                ),
            ),
        ];
        let mut check = Self::node(Role::CheckBox, "Enabled", CHECK_RECT, "enabled");
        check.set_toggled(if self.checked {
            Toggled::True
        } else {
            Toggled::False
        });
        nodes.push((accesskit::NodeId(CHECK), check));
        let mut field = Self::node(Role::TextInput, "Name", FIELD_RECT, "name");
        field.set_value(self.text.as_str());
        nodes.push((accesskit::NodeId(FIELD), field));
        if self.duplicates {
            children.extend([accesskit::NodeId(DUP_A), accesskit::NodeId(DUP_B)]);
            nodes.push((
                accesskit::NodeId(DUP_A),
                Self::node(Role::Label, "Same", Rect::new(0.0, 200.0, 10.0, 210.0), "a"),
            ));
            nodes.push((
                accesskit::NodeId(DUP_B),
                Self::node(Role::Label, "Same", Rect::new(0.0, 220.0, 10.0, 230.0), "b"),
            ));
        }
        root.set_children(children);
        nodes.push((accesskit::NodeId(ROOT), root));
        TreeUpdate {
            nodes,
            tree: Some(TreeInfo::new(accesskit::NodeId(ROOT))),
            tree_id: accesskit::TreeId::ROOT,
            focus: accesskit::NodeId(self.focus.unwrap_or(ROOT)),
        }
    }

    fn activate(&mut self, node: u64) {
        match node {
            BUTTON => self.count += 1,
            CHECK => self.checked = !self.checked,
            _ => {}
        }
        self.dirty = true;
    }
}

impl DocumentApi for Doc {
    fn dispatch(&mut self, ev: &InputEnvelope) -> Dispatched {
        let before = (self.count, self.checked, self.text.clone(), self.focus);
        match &ev.event {
            InputEvent::Pointer(p) => match p.phase {
                PointerPhase::Down if BUTTON_RECT.contains(p.pos) => self.pressed = true,
                PointerPhase::Down if CHECK_RECT.contains(p.pos) => self.focus = Some(CHECK),
                PointerPhase::Down if FIELD_RECT.contains(p.pos) => self.focus = Some(FIELD),
                PointerPhase::Up => {
                    if std::mem::take(&mut self.pressed) && BUTTON_RECT.contains(p.pos) {
                        self.activate(BUTTON);
                    } else if CHECK_RECT.contains(p.pos) {
                        self.activate(CHECK);
                    }
                }
                // Hover is native state with no visual here: no frame.
                PointerPhase::Move => self.hovered = Some(p.pos),
                _ => {}
            },
            InputEvent::Key(k) if k.state == KeyState::Pressed => match k.key {
                Some(Key::Tab) => {
                    let order = [BUTTON, CHECK, FIELD];
                    let next = match self.focus.and_then(|f| order.iter().position(|&o| o == f)) {
                        Some(i) => order[(i + 1) % order.len()],
                        None => order[0],
                    };
                    self.focus = Some(next);
                }
                Some(Key::Space) | Some(Key::Enter) => {
                    if let Some(f) = self.focus {
                        self.activate(f);
                    }
                }
                Some(Key::Backspace) if self.focus == Some(FIELD) => {
                    self.text.pop();
                }
                _ => {}
            },
            InputEvent::Ime(ImeEvent::Commit(t)) if self.focus == Some(FIELD) => {
                self.text.push_str(t)
            }
            _ => {}
        }
        let after = (self.count, self.checked, self.text.clone(), self.focus);
        if before != after {
            self.dirty = true;
        }
        Dispatched {
            dirty: self.dirty,
            js_events: 0,
            fast: Vec::new(),
        }
    }

    fn frame(&mut self, window: WindowId, _target: Time) -> Option<Commit> {
        if !std::mem::take(&mut self.dirty) {
            return None;
        }
        self.frames += 1;
        self.revision += 1;
        let mut c = Commit {
            window,
            revision: Revision(self.revision),
            a11y: Some(self.tree()),
            ..Commit::default()
        };
        if let Some(duration) = self.animate_next_frame.take() {
            c.animations.push(AnimationCmd::Start(AnimationSpec {
                id: AnimationId(1),
                node: NodeId::new(BUTTON as u32, 1),
                target: Some(PropNodeId(1)),
                property: AnimProperty::Opacity,
                generator: Generator::Keyframes {
                    keyframes: vec![
                        Keyframe {
                            offset: 0.0,
                            value: AnimValue::Number(0.0),
                            easing: None,
                        },
                        Keyframe {
                            offset: 1.0,
                            value: AnimValue::Number(1.0),
                            easing: None,
                        },
                    ],
                    timing: Timing {
                        delay: Span::ZERO,
                        end_delay: Span::ZERO,
                        duration,
                        iterations: 1.0,
                        iteration_start: 0.0,
                        direction: PlaybackDirection::Normal,
                        fill: FillMode::None,
                        easing: Easing::Linear,
                    },
                },
                start: StartTime::NextFrame,
            }));
        }
        Some(c)
    }

    fn needs_frame(&self, _window: WindowId) -> bool {
        self.dirty
    }

    fn resize(&mut self, _window: WindowId, size: Size, scale: Scale) {
        self.size = Some((size, scale));
        self.resizes += 1;
        self.dirty = true;
    }

    fn scroll_moved(&mut self, _window: WindowId, _node: PropNodeId, _offset: Vec2) {}

    fn animation_event(&mut self, _window: WindowId, ev: AnimationEvent) {
        self.animation_events.push(ev);
    }

    fn accessibility_action(&mut self, _window: WindowId, req: &ActionRequest) -> bool {
        let node = req.target_node.0;
        match req.action {
            Action::Click => self.activate(node),
            Action::Focus => {
                self.focus = Some(node);
                self.dirty = true;
            }
            _ => return false,
        }
        true
    }

    fn window_event(&mut self, _window: WindowId, _ev: &WindowEvent) {}

    fn menu_activated(&mut self, id: &str) -> bool {
        self.menu.push(id.to_owned());
        if id == "inc" {
            self.activate(BUTTON);
        }
        true
    }

    fn caret_blink(&self) -> Option<Option<presto::compositor_crate::CaretBlink>> {
        Some(Some(presto::compositor_crate::CaretBlink::GTK))
    }

    fn hover(&mut self, _window: WindowId, pos: Point) -> bool {
        self.hovers.push(pos);
        false
    }

    fn ime_state(&mut self, _window: WindowId) -> Option<(Rect, presto_base::input::InputPurpose)> {
        (self.focus == Some(FIELD)).then_some((FIELD_RECT, presto_base::input::InputPurpose::Email))
    }

    fn close_window(&mut self, _window: WindowId) {}

    fn accessibility_activated(&mut self, _window: WindowId) {
        self.dirty = true;
    }

    fn device_reset(&mut self) {
        self.dirty = true;
    }
}

fn harness() -> Harness<Doc> {
    Harness::new(Doc {
        dirty: true,
        ..Doc::default()
    })
}

#[test]
fn builds_and_settles_with_the_first_frame() {
    let h = harness();
    assert!(h.busy().is_none());
    assert!(h.state().frames >= 1);
    // The open-window request's BeginFrame came after the resize's urgent frame and found nothing.
    assert!(h.compositor().skipped_frames() >= 1);
    assert_eq!(h.get_by_label("Count: 0").role(), Role::Label);
    assert_eq!(h.state().size, Some((Size::new(800.0, 600.0), Scale(1.0))));
}

#[test]
fn click_takes_the_dispatch_path_and_updates_the_snapshot() {
    let mut h = harness();
    h.get_by_label("Increment").click();
    h.run();
    assert_eq!(h.state().count, 1);
    assert!(h.query_by_label("Count: 1").is_some());
    assert!(h.query_by_label("Count: 0").is_none());
    let urgent = h.app().latency().counters(h.window()).urgent;
    assert!(urgent >= 1, "a click runs an urgent frame");
}

#[test]
fn a_click_reaches_the_screen_within_one_step() {
    let mut h = harness();
    h.get_by_label("Increment").click();
    // Deliver + urgent frame + commit, then the next vsync applies it.
    h.step();
    assert!(h.query_by_label("Count: 1").is_some());
}

#[test]
fn queries_by_role_label_value_and_author_id() {
    let h = harness();
    assert_eq!(
        h.get_by_role_and_label(Role::Button, "Increment")
            .author_id(),
        Some("inc")
    );
    assert_eq!(h.get_by_author_id("enabled").role(), Role::CheckBox);
    assert_eq!(h.get_by_role(Role::TextInput).label().as_deref(), Some("Name"));
    assert!(h.query_by_role_and_label(Role::Button, "Name").is_none());
    assert!(h.query_by_author_id("missing").is_none());
    assert_eq!(h.get_by_label("Increment").bounds(), Some(BUTTON_RECT));
    assert_eq!(h.query_all(By::new().label_contains("e")).len(), 3);
}

#[test]
#[should_panic(expected = "2 nodes match")]
fn get_panics_when_several_match() {
    let h = Harness::new(Doc {
        dirty: true,
        duplicates: true,
        ..Doc::default()
    });
    h.get_by_label("Same");
}

#[test]
#[should_panic(expected = "no node matches")]
fn get_panics_when_none_match() {
    let h = harness();
    h.get_by_label("Nope");
}

#[test]
fn key_press_moves_focus_and_activates() {
    let mut h = harness();
    h.key_press(Key::Tab);
    h.key_press(Key::Tab);
    h.run();
    assert!(h.get_by_label("Enabled").is_focused());
    assert_eq!(h.focused().map(|n| n.id()), Some(accesskit::NodeId(CHECK)));
    h.key_press(Key::Space);
    h.run();
    assert_eq!(h.get_by_label("Enabled").toggled(), Some(Toggled::True));
}

#[test]
fn type_text_focuses_through_accesskit_then_commits_text() {
    let mut h = harness();
    let field = h.get_by_label("Name");
    field.focus();
    field.type_text("presto");
    h.run();
    assert_eq!(h.get_by_label("Name").value().as_deref(), Some("presto"));
    h.key_press(Key::Backspace);
    h.run();
    assert_eq!(h.get_by_role(Role::TextInput).value().as_deref(), Some("prest"));
}

#[test]
fn accesskit_click_needs_no_bounds() {
    let mut h = harness();
    h.get_by_author_id("inc").click_accesskit();
    h.run();
    assert_eq!(h.state().count, 1);
}

#[test]
fn egui_events_translate_to_the_same_dispatch() {
    let mut h = harness();
    let pos = egui::pos2(50.0, 20.0);
    h.event_egui(egui::Event::PointerMoved(pos));
    for pressed in [true, false] {
        h.event_egui(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.run();
    assert_eq!(h.state().count, 1);
    h.event_egui(egui::Event::Key {
        key: egui::Key::Tab,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    });
    h.run();
    assert!(h.get_by_label("Increment").is_focused());
}

#[test]
fn step_without_input_does_nothing() {
    let mut h = harness();
    let frames = h.state().frames;
    let applied = h.compositor().applied_count();
    for _ in 0..5 {
        h.step();
    }
    assert_eq!(h.state().frames, frames);
    assert_eq!(h.compositor().applied_count(), applied);
    assert!(h.busy().is_none());
}

#[test]
fn resize_goes_through_the_handshake() {
    let mut h = harness();
    let before = h.state().resizes;
    h.set_size(Size::new(400.0, 300.0));
    h.run();
    assert_eq!(h.state().resizes, before + 1);
    assert_eq!(h.state().size, Some((Size::new(400.0, 300.0), Scale(1.0))));
    let resizes = h.compositor().resizes();
    let last = resizes.last().expect("the compositor was told");
    assert_eq!((last.1.w, last.1.h), (400, 300));
    assert_eq!(h.app().latency().counters(h.window()).resize_timeouts, 0);
}

#[test]
fn hidpi_sizes_are_in_points() {
    let h = Harness::<Doc>::builder()
        .with_size(Size::new(300.0, 200.0))
        .with_scale(2.0)
        .build(Doc {
            dirty: true,
            ..Doc::default()
        });
    assert_eq!(h.state().size, Some((Size::new(300.0, 200.0), Scale(2.0))));
    let (size, scale) = h.compositor().window_size(h.window()).unwrap();
    assert_eq!((size.w, size.h, scale), (600, 400, Scale(2.0)));
}

#[test]
fn run_waits_for_compositor_animations() {
    let mut h = harness();
    h.state_mut().animate_next_frame = Some(Span::from_millis(100));
    h.get_by_label("Increment").click();
    let steps = h.run();
    // 100 ms at 60 Hz is six frames; the animation started on the vsync that applied the commit.
    assert!(steps >= 6, "ran {steps} steps");
    let evs = &h.state().animation_events;
    assert!(matches!(
        evs.first(),
        Some(AnimationEvent::Started {
            id: AnimationId(1),
            ..
        })
    ));
    assert!(matches!(
        evs.last(),
        Some(AnimationEvent::Finished {
            id: AnimationId(1),
            ..
        })
    ));
}

#[test]
fn every_injected_input_terminates_exactly_once() {
    let mut h = harness();
    h.hover_at(Point::new(300.0, 300.0));
    h.hover_at(Point::new(301.0, 300.0));
    h.hover_at(Point::new(302.0, 300.0));
    h.get_by_label("Increment").click();
    h.key_press(Key::Tab);
    h.key_press(Key::A);
    h.drag(Point::new(500.0, 500.0), Point::new(550.0, 500.0), 4);
    h.run();
    let lat = h.app().latency();
    assert!(lat.violations().is_empty(), "{:?}", lat.violations());
    assert_eq!(lat.live_count(), 0);
    // 3 hovers, click (3), Tab (2), A (2), drag (1 + 1 + 4 + 1).
    assert_eq!(lat.terminated_count(), 3 + 3 + 2 + 2 + 7);
    for r in lat.finished() {
        assert!(r.end.is_some());
        if let Some(Termination::Presented(_)) = r.end {
            assert!(r.at(presto::latency::Stage::Commit).is_some());
            assert!(r.at(presto::latency::Stage::Presented).is_some());
        }
    }
    // The click's release and the Tab press changed the tree and reached the screen.
    let presented: Vec<_> = lat
        .finished()
        .filter(|r| matches!(r.end, Some(Termination::Presented(_))))
        .collect();
    assert!(presented.iter().any(|r| r.class == InputClass::Click));
    assert!(presented.iter().any(|r| r.class == InputClass::Key));
    // Hovers merged into the newest one.
    assert!(
        lat.finished()
            .any(|r| matches!(r.end, Some(Termination::MergedInto(_))))
    );
    let key = (Segment::InputToPresented, InputClass::Click, h.window());
    assert!(lat.histogram(key).is_some_and(|hist| hist.count() >= 1));
}

#[test]
fn chrome_trace_has_a_flow_per_presented_input() {
    let mut h = harness();
    h.get_by_label("Increment").click();
    h.run();
    let trace = h.app().latency().chrome_trace();
    assert!(trace.starts_with("{\"displayTimeUnit\""));
    assert!(trace.contains("\"ph\":\"s\""));
    assert!(trace.contains("\"ph\":\"f\""));
    assert!(trace.contains("\"name\":\"presented\""));
    assert!(trace.contains("\"name\":\"ui_frame\""));
}

#[test]
fn closing_the_last_window_exits() {
    let mut h = harness();
    h.window_event(WindowEvent::CloseRequested);
    h.step();
    assert!(h.exited());
    assert!(h.compositor().is_shut_down());
}

#[test]
fn hidden_windows_drop_their_inputs() {
    let mut h = harness();
    h.window_event(WindowEvent::Occluded(true));
    h.step();
    h.get_by_label("Increment").click();
    h.step();
    h.window_event(WindowEvent::Occluded(false));
    h.run();
    let lat = h.app().latency();
    assert!(lat.violations().is_empty(), "{:?}", lat.violations());
    assert_eq!(lat.live_count(), 0);
    assert!(
        h.query_by_label("Count: 1").is_some(),
        "the change shows once visible"
    );
}

#[test]
fn coalesced_moves_hover_at_event_time() {
    let mut h = harness();
    h.hover_at(Point::new(300.0, 300.0));
    h.hover_at(Point::new(301.0, 300.0));
    h.run();
    // Each move hovered when it arrived, though the two merged into one dispatched move.
    assert_eq!(
        h.state().hovers,
        vec![Point::new(300.0, 300.0), Point::new(301.0, 300.0)]
    );
}

#[test]
fn the_ime_follows_the_focused_field() {
    use presto::platform::PlatformCall;
    let mut h = harness();
    h.get_by_label("Name").focus();
    h.run();
    let ime: Vec<_> = h
        .calls()
        .iter()
        .filter_map(|c| match c {
            PlatformCall::SetIme(_, s) => Some(s.clone()),
            _ => None,
        })
        .collect();
    let last = ime.last().cloned().flatten().expect("the IME turned on");
    assert_eq!(
        (last.area, last.purpose),
        (FIELD_RECT, presto_base::input::InputPurpose::Email)
    );
    h.key_press(Key::Tab);
    h.run();
    let ime: Vec<_> = h
        .calls()
        .iter()
        .filter_map(|c| match c {
            PlatformCall::SetIme(_, s) => Some(s.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        ime.last(),
        Some(&None),
        "focus left the field: the IME is off"
    );
    assert_eq!(ime.len(), 2, "sent only when it changed");
}

#[test]
fn menu_items_reach_the_document_with_an_urgent_frame() {
    let mut h = harness();
    h.menu_activated("inc");
    assert_eq!(h.state().menu, vec!["inc".to_owned()]);
    h.run();
    assert!(h.query_by_label("Count: 1").is_some());
}

#[test]
fn new_windows_get_the_policy_caret_blink() {
    let h = harness();
    assert_eq!(
        h.compositor().caret_blinks(),
        vec![(h.window(), Some(presto::compositor_crate::CaretBlink::GTK))]
    );
}
