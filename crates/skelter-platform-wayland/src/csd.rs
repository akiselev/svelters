//! Our client-side decorations. Where the compositor draws none (no `xdg-decoration`, as on Mutter, or it
//! chose client-side), the app draws its own title bar and tells the platform what each part means with
//! hit-test regions; the platform adds a resize border along the window's edges itself (not while maximised,
//! fullscreen or tiled). This module turns pointer presses on them into xdg-shell requests:
//!
//! | Part | Primary press | Primary release (same part) | Secondary press | Double click |
//! |---|---|---|---|---|
//! | caption | `xdg_toplevel.move` | — | `show_window_menu` | toggle maximised |
//! | system menu (window icon) | `show_window_menu` | — | `show_window_menu` | — |
//! | resize border | `xdg_toplevel.resize` | — | — | — |
//! | minimise / maximise / close | — | minimise / toggle maximised / close request | — | — |
//!
//! Presses that start a move, resize or menu are not delivered to the app (the compositor takes the pointer
//! grab); button parts get their events as ordinary input too, so the app can draw pressed states.

use crate::shim::{HitKind, HitRegion, hit_test};
use skelter_base::cursor::CursorIcon;
use skelter_base::geom::Point;
use skelter_base::time::{Span, Time};

pub use wayland_protocols::xdg::shell::client::xdg_toplevel::ResizeEdge;

/// The resize border's width inside the window edge, and the corner extent along each edge (points).
pub const BORDER: f32 = 6.0;
pub const CORNER: f32 = 16.0;

/// The double-click window and slop, as GTK's defaults (`gtk-double-click-time` 400 ms, distance 5 px).
pub const DOUBLE_CLICK: Span = Span::from_millis(400);
pub const DOUBLE_CLICK_SLOP: f32 = 5.0;

/// The part of the decorations under the pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CsdHit {
    Region(HitKind),
    Edge(ResizeEdge),
}

/// What the decorations need to know about the window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CsdFrame {
    /// The window's size in points.
    pub size: (f32, f32),
    /// The resize border is active (CSD, floating, resizable).
    pub border: bool,
}

/// The resize edge at `p`, if it is on the border.
pub fn edge(frame: CsdFrame, p: Point) -> Option<ResizeEdge> {
    if !frame.border {
        return None;
    }
    let (w, h) = frame.size;
    let (l, r, t, b) = (p.x < BORDER, p.x >= w - BORDER, p.y < BORDER, p.y >= h - BORDER);
    let (near_l, near_r, near_t, near_b) = (p.x < CORNER, p.x >= w - CORNER, p.y < CORNER, p.y >= h - CORNER);
    Some(match () {
        _ if (t && near_l) || (l && near_t) => ResizeEdge::TopLeft,
        _ if (t && near_r) || (r && near_t) => ResizeEdge::TopRight,
        _ if (b && near_l) || (l && near_b) => ResizeEdge::BottomLeft,
        _ if (b && near_r) || (r && near_b) => ResizeEdge::BottomRight,
        _ if t => ResizeEdge::Top,
        _ if b => ResizeEdge::Bottom,
        _ if l => ResizeEdge::Left,
        _ if r => ResizeEdge::Right,
        _ => return None,
    })
}

/// The decoration part under `p`: the border first, then the topmost region; `None` over client content,
/// including `Client` regions cut out of a title bar.
pub fn hit(regions: &[HitRegion], frame: CsdFrame, p: Point) -> Option<CsdHit> {
    if let Some(e) = edge(frame, p) {
        return Some(CsdHit::Edge(e));
    }
    Some(hit_test(regions, p)).filter(|k| *k != HitKind::Client).map(CsdHit::Region)
}

/// The cursor a part shows while hovered, overriding the app's.
pub fn cursor_for(hit: CsdHit) -> Option<CursorIcon> {
    let CsdHit::Edge(e) = hit else { return None };
    Some(match e {
        ResizeEdge::Top => CursorIcon::NResize,
        ResizeEdge::Bottom => CursorIcon::SResize,
        ResizeEdge::Left => CursorIcon::WResize,
        ResizeEdge::Right => CursorIcon::EResize,
        ResizeEdge::TopLeft => CursorIcon::NwResize,
        ResizeEdge::TopRight => CursorIcon::NeResize,
        ResizeEdge::BottomLeft => CursorIcon::SwResize,
        ResizeEdge::BottomRight => CursorIcon::SeResize,
        _ => return None,
    })
}

/// What a press or release on the decorations asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CsdAction {
    Move,
    Resize(ResizeEdge),
    WindowMenu,
    ToggleMaximize,
    Minimize,
    Close,
}

/// Per-window decoration pointer state.
#[derive(Default)]
pub struct CsdState {
    last_caption_click: Option<(Time, Point)>,
    pressed: Option<HitKind>,
}

impl CsdState {
    /// A primary (`primary == true`) or secondary press at `p`. Returns the action and whether the press is
    /// consumed (not delivered to the app).
    pub fn press(&mut self, regions: &[HitRegion], frame: CsdFrame, p: Point, primary: bool, now: Time) -> (Option<CsdAction>, bool) {
        let kind = match hit(regions, frame, p) {
            None => {
                self.pressed = None;
                return (None, false);
            }
            Some(CsdHit::Edge(e)) => return if primary { (Some(CsdAction::Resize(e)), true) } else { (None, false) },
            Some(CsdHit::Region(k)) => k,
        };
        if !primary {
            return match kind {
                HitKind::Caption | HitKind::SystemMenu => (Some(CsdAction::WindowMenu), true),
                _ => (None, false),
            };
        }
        self.pressed = Some(kind);
        match kind {
            HitKind::Caption => {
                let double = self.last_caption_click.is_some_and(|(t, q)| {
                    now - t <= DOUBLE_CLICK && (p.x - q.x).abs() <= DOUBLE_CLICK_SLOP && (p.y - q.y).abs() <= DOUBLE_CLICK_SLOP
                });
                if double {
                    self.last_caption_click = None;
                    (Some(CsdAction::ToggleMaximize), true)
                } else {
                    self.last_caption_click = Some((now, p));
                    (Some(CsdAction::Move), true)
                }
            }
            HitKind::SystemMenu => (Some(CsdAction::WindowMenu), true),
            _ => (None, false),
        }
    }

    /// A primary release at `p`: window buttons act when pressed and released on the same region.
    pub fn release(&mut self, regions: &[HitRegion], frame: CsdFrame, p: Point) -> Option<CsdAction> {
        let pressed = self.pressed.take()?;
        if hit(regions, frame, p) != Some(CsdHit::Region(pressed)) {
            return None;
        }
        match pressed {
            HitKind::Minimize => Some(CsdAction::Minimize),
            HitKind::Maximize => Some(CsdAction::ToggleMaximize),
            HitKind::Close => Some(CsdAction::Close),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use skelter_base::geom::Rect;

    const FRAME: CsdFrame = CsdFrame { size: (800.0, 600.0), border: true };

    fn regions() -> Vec<HitRegion> {
        vec![
            HitRegion { rect: Rect::new(0.0, 0.0, 800.0, 32.0), kind: HitKind::Caption },
            HitRegion { rect: Rect::new(700.0, 0.0, 740.0, 32.0), kind: HitKind::Maximize },
            HitRegion { rect: Rect::new(760.0, 0.0, 800.0, 32.0), kind: HitKind::Close },
            HitRegion { rect: Rect::new(300.0, 8.0, 400.0, 28.0), kind: HitKind::Client },
            HitRegion { rect: Rect::new(8.0, 8.0, 28.0, 28.0), kind: HitKind::SystemMenu },
        ]
    }

    fn t(ms: u64) -> Time {
        Time(ms * 1_000_000)
    }

    #[test]
    fn border_edges_and_corners() {
        assert_eq!(edge(FRAME, Point::new(400.0, 2.0)), Some(ResizeEdge::Top));
        assert_eq!(edge(FRAME, Point::new(400.0, 598.0)), Some(ResizeEdge::Bottom));
        assert_eq!(edge(FRAME, Point::new(1.0, 300.0)), Some(ResizeEdge::Left));
        assert_eq!(edge(FRAME, Point::new(799.0, 300.0)), Some(ResizeEdge::Right));
        assert_eq!(edge(FRAME, Point::new(2.0, 2.0)), Some(ResizeEdge::TopLeft));
        assert_eq!(edge(FRAME, Point::new(10.0, 2.0)), Some(ResizeEdge::TopLeft));
        assert_eq!(edge(FRAME, Point::new(798.0, 595.0)), Some(ResizeEdge::BottomRight));
        assert_eq!(edge(FRAME, Point::new(400.0, 300.0)), None);
        assert_eq!(edge(CsdFrame { border: false, ..FRAME }, Point::new(400.0, 2.0)), None);
    }

    #[test]
    fn regions_under_the_border() {
        let r = regions();
        assert_eq!(hit(&r, FRAME, Point::new(400.0, 16.0)), Some(CsdHit::Region(HitKind::Caption)));
        assert_eq!(hit(&r, FRAME, Point::new(780.0, 16.0)), Some(CsdHit::Region(HitKind::Close)));
        assert_eq!(hit(&r, FRAME, Point::new(400.0, 2.0)), Some(CsdHit::Edge(ResizeEdge::Top)));
        assert_eq!(hit(&r, FRAME, Point::new(400.0, 300.0)), None);
        // A client cut-out (a search box in the title bar) blocks the caption.
        assert_eq!(hit(&r, FRAME, Point::new(350.0, 16.0)), None);
        assert_eq!(cursor_for(CsdHit::Edge(ResizeEdge::Top)), Some(CursorIcon::NResize));
        assert_eq!(cursor_for(CsdHit::Region(HitKind::Caption)), None);
    }

    #[test]
    fn caption_press_moves_and_double_click_maximizes() {
        let r = regions();
        let mut s = CsdState::default();
        assert_eq!(s.press(&r, FRAME, Point::new(100.0, 16.0), true, t(0)), (Some(CsdAction::Move), true));
        assert_eq!(s.press(&r, FRAME, Point::new(102.0, 17.0), true, t(300)), (Some(CsdAction::ToggleMaximize), true));
        assert_eq!(s.press(&r, FRAME, Point::new(102.0, 17.0), true, t(500)), (Some(CsdAction::Move), true));
        assert_eq!(s.press(&r, FRAME, Point::new(102.0, 17.0), true, t(1000)), (Some(CsdAction::Move), true));
        assert_eq!(s.press(&r, FRAME, Point::new(150.0, 17.0), true, t(1100)), (Some(CsdAction::Move), true));
    }

    #[test]
    fn menus_and_resizes() {
        let r = regions();
        let mut s = CsdState::default();
        assert_eq!(s.press(&r, FRAME, Point::new(100.0, 16.0), false, t(0)), (Some(CsdAction::WindowMenu), true));
        assert_eq!(s.press(&r, FRAME, Point::new(16.0, 16.0), true, t(0)), (Some(CsdAction::WindowMenu), true));
        assert_eq!(s.press(&r, FRAME, Point::new(100.0, 300.0), false, t(0)), (None, false));
        assert_eq!(s.press(&r, FRAME, Point::new(3.0, 3.0), true, t(0)), (Some(CsdAction::Resize(ResizeEdge::TopLeft)), true));
    }

    #[test]
    fn buttons_act_on_release_inside() {
        let r = regions();
        let mut s = CsdState::default();
        assert_eq!(s.press(&r, FRAME, Point::new(780.0, 16.0), true, t(0)), (None, false));
        assert_eq!(s.release(&r, FRAME, Point::new(781.0, 15.0)), Some(CsdAction::Close));
        s.press(&r, FRAME, Point::new(720.0, 16.0), true, t(0));
        assert_eq!(s.release(&r, FRAME, Point::new(400.0, 16.0)), None);
        s.press(&r, FRAME, Point::new(720.0, 16.0), true, t(0));
        assert_eq!(s.release(&r, FRAME, Point::new(721.0, 16.0)), Some(CsdAction::ToggleMaximize));
    }
}
