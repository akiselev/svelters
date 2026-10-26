//! `Harness::render`: a document commits a root layer holding one red quad; the harness draws the
//! committed tree on the environment's adapter. Runs only with `SKELTER_GPU_TESTS=1` (CI sets it:
//! lavapipe on Linux, WARP on Windows, Metal on macOS).

use skelter::seams::{Dispatched, DocumentApi};
use skelter_base::accesskit::ActionRequest;
use skelter_base::anim::AnimationEvent;
use skelter_base::color::{Color, Fill};
use skelter_base::commit::Commit;
use skelter_base::display::{BorderStyle, DisplayList, Prim, Quad};
use skelter_base::event::{InputEnvelope, InputEvent, KeyState, WindowEvent};
use skelter_base::geom::{Affine, Corners, Insets, Rect, Scale, Size, Vec2};
use skelter_base::ids::{LayerId, NodeId, PropNodeId, Revision, WindowId};
use skelter_base::key::Key;
use skelter_base::layer::{ItemRef, LayerDelta, LayerDesc, LayerKind};
use skelter_base::props::{PropOp, TransformNode};
use skelter_base::time::Time;
use skelter_test::Harness;
use std::sync::Arc;

const QUAD: NodeId = NodeId::new(1, 1);
const RED: Color = Color::rgba(1.0, 0.0, 0.0, 1.0);
const BLUE: Color = Color::rgba(0.0, 0.0, 1.0, 1.0);

/// One root layer and one quad; Space turns the quad blue.
struct QuadDoc {
    sent_layers: bool,
    color: Color,
    dirty: bool,
    revision: u64,
}

impl QuadDoc {
    fn list(&self) -> DisplayList {
        let rect = Rect::new(10.0, 10.0, 50.0, 30.0);
        let mut l = DisplayList::new();
        l.push(
            Prim::Quad(Quad {
                rect,
                radii: Corners::ZERO,
                fill: Fill::Solid(self.color),
                border_widths: Insets::ZERO,
                border_color: Color::TRANSPARENT,
                border_style: BorderStyle::Solid,
            }),
            rect,
        );
        l
    }
}

impl DocumentApi for QuadDoc {
    fn dispatch(&mut self, ev: &InputEnvelope) -> Dispatched {
        if let InputEvent::Key(k) = &ev.event
            && k.state == KeyState::Pressed
            && k.key == Some(Key::Space)
        {
            self.color = BLUE;
            self.dirty = true;
        }
        Dispatched {
            dirty: self.dirty,
            ..Dispatched::default()
        }
    }
    fn frame(&mut self, window: WindowId, _target: Time) -> Option<Commit> {
        if !std::mem::take(&mut self.dirty) {
            return None;
        }
        self.revision += 1;
        let mut c = Commit {
            window,
            revision: Revision(self.revision),
            ..Commit::default()
        };
        if !std::mem::replace(&mut self.sent_layers, true) {
            let space = PropNodeId(1);
            c.props.ops.push(PropOp::Transform(
                space,
                TransformNode {
                    parent: None,
                    local: Affine::IDENTITY,
                    scroll: None,
                },
            ));
            c.layers.push(LayerDelta::Add(LayerDesc {
                id: LayerId(1),
                parent: None,
                kind: LayerKind::Root,
                transform: space,
                clip: None,
                effect: None,
                z: 0,
                bounds: Rect::new(0.0, 0.0, 100.0, 60.0),
                opaque: false,
            }));
            c.layers.push(LayerDelta::Items {
                layer: LayerId(1),
                items: vec![ItemRef {
                    node: QUAD,
                    transform: space,
                    clip: None,
                    effect: None,
                }],
            });
        }
        c.lists.push((QUAD, Arc::new(self.list())));
        Some(c)
    }
    fn needs_frame(&self, _window: WindowId) -> bool {
        self.dirty
    }
    fn resize(&mut self, _window: WindowId, _size: Size, _scale: Scale) {
        self.dirty = true;
    }
    fn close_window(&mut self, _window: WindowId) {}
    fn scroll_moved(&mut self, _window: WindowId, _node: PropNodeId, _offset: Vec2) {}
    fn animation_event(&mut self, _window: WindowId, _ev: AnimationEvent) {}
    fn accessibility_action(&mut self, _window: WindowId, _req: &ActionRequest) -> bool {
        false
    }
    fn accessibility_activated(&mut self, _window: WindowId) {}
    fn window_event(&mut self, _window: WindowId, _ev: &WindowEvent) {}
    fn device_reset(&mut self) {}
}

fn gpu_tests() -> bool {
    let on = std::env::var("SKELTER_GPU_TESTS").as_deref() == Ok("1");
    if !on {
        eprintln!("skipped: set SKELTER_GPU_TESTS=1 to run GPU tests");
    }
    on
}

fn near(a: [u8; 4], b: [u8; 4]) -> bool {
    a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= 2)
}

#[test]
fn render_draws_the_committed_tree() {
    if !gpu_tests() {
        return;
    }
    let mut h = Harness::<QuadDoc>::builder()
        .with_size(Size::new(100.0, 60.0))
        .build(QuadDoc {
            sent_layers: false,
            color: RED,
            dirty: true,
            revision: 0,
        });
    let img = h.render().expect("a wgpu adapter (SKELTER_GPU_TESTS=1)");
    assert_eq!((img.width, img.height), (100, 60));
    assert!(
        near(img.pixel(30, 20), [255, 0, 0, 255]),
        "inside the quad: {:?}",
        img.pixel(30, 20)
    );
    assert!(
        near(img.pixel(80, 50), [255, 255, 255, 255]),
        "the clear colour: {:?}",
        img.pixel(80, 50)
    );

    h.key_press(Key::Space);
    h.run();
    let img = h.render().expect("render again");
    assert!(
        near(img.pixel(30, 20), [0, 0, 255, 255]),
        "re-recorded list: {:?}",
        img.pixel(30, 20)
    );
}

#[test]
fn render_is_in_device_pixels() {
    if !gpu_tests() {
        return;
    }
    let mut h = Harness::<QuadDoc>::builder()
        .with_size(Size::new(100.0, 60.0))
        .with_scale(2.0)
        .build(QuadDoc {
            sent_layers: false,
            color: RED,
            dirty: true,
            revision: 0,
        });
    let img = h.render().expect("a wgpu adapter (SKELTER_GPU_TESTS=1)");
    assert_eq!((img.width, img.height), (200, 120));
    // Point (30, 20) is device pixel (60, 40); point (55, 20) is outside the quad.
    assert!(
        near(img.pixel(60, 40), [255, 0, 0, 255]),
        "{:?}",
        img.pixel(60, 40)
    );
    assert!(
        near(img.pixel(110, 40), [255, 255, 255, 255]),
        "{:?}",
        img.pixel(110, 40)
    );
}
