//! Our client-side decorations. Where the compositor draws none (no `xdg-decoration`, as on Mutter, or it
//! chose client-side), the app draws its own title bar and frame and tells the platform what each part means
//! with hit-test regions. This module turns pointer presses on those regions into xdg-shell requests:
//!
//! | Region | Primary press | Primary release (same region) | Secondary press | Double click |
//! |---|---|---|---|---|
//! | caption | `xdg_toplevel.move` | — | `show_window_menu` | toggle maximised |
//! | system menu (window icon) | `show_window_menu` | — | `show_window_menu` | — |
//! | resize edges | `xdg_toplevel.resize` | — | — | — |
//! | minimise / maximise / close | — | minimise / toggle maximised / close request | — | — |
//!
//! Presses that start a move or resize are not delivered to the app (the compositor takes the pointer grab);
//! button regions get their events as ordinary input too, so the app can draw pressed states.

use crate::shim::{HitKind, HitRegion};
use presto_base::cursor::CursorIcon;
use presto_base::geom::Point;
use presto_base::time::{Span, Time};

/// The topmost region under `p` (later regions are on top of earlier ones); `None` over client content,
/// including `Client` regions cut out of a title bar.
pub fn hit(regions: &[HitRegion], p: Point) -> Option<HitKind> {
    Some(crate::shim::hit_test(regions, p)).filter(|k| *k != HitKind::Client)
}

/// The cursor a region shows while hovered, overriding the app's.
pub fn cursor_for(kind: HitKind) -> Option<CursorIcon> {
    Some(match kind {
        HitKind::ResizeTop => CursorIcon::NResize,
        HitKind::ResizeBottom => CursorIcon::SResize,
        HitKind::ResizeLeft => CursorIcon::WResize,
        HitKind::ResizeRight => CursorIcon::EResize,
        HitKind::ResizeTopLeft => CursorIcon::NwResize,
        HitKind::ResizeTopRight => CursorIcon::NeResize,
        HitKind::ResizeBottomLeft => CursorIcon::SwResize,
        HitKind::ResizeBottomRight => CursorIcon::SeResize,
        _ => return None,
    })
}

/// The xdg-shell resize edge of a region.
pub fn resize_edge(kind: HitKind) -> Option<ResizeEdge> {
    Some(match kind {
        HitKind::ResizeTop => ResizeEdge::Top,
        HitKind::ResizeBottom => ResizeEdge::Bottom,
        HitKind::ResizeLeft => ResizeEdge::Left,
        HitKind::ResizeRight => ResizeEdge::Right,
        HitKind::ResizeTopLeft => ResizeEdge::TopLeft,
        HitKind::ResizeTopRight => ResizeEdge::TopRight,
        HitKind::ResizeBottomLeft => ResizeEdge::BottomLeft,
        HitKind::ResizeBottomRight => ResizeEdge::BottomRight,
        _ => return None,
    })
}

pub use wayland_protocols::xdg::shell::client::xdg_toplevel::ResizeEdge;

/// What a press or release on the decorations asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CsdAction {
    Move,
    Resize(HitKind),
    WindowMenu,
    ToggleMaximize,
    Minimize,
    Close,
}

/// The double-click window and slop, as GTK's defaults (`gtk-double-click-time` 400 ms, distance 5 px).
pub const DOUBLE_CLICK: Span = Span::from_millis(400);
pub const DOUBLE_CLICK_SLOP: f32 = 5.0;

/// Per-window decoration pointer state.
#[derive(Default)]
pub struct CsdState {
    last_caption_click: Option<(Time, Point)>,
    pressed: Option<HitKind>,
}

impl CsdState {
    /// A primary (`primary == true`) or secondary press at `p`. Returns the action and whether the press is
    /// consumed (not delivered to the app).
    pub fn press(&mut self, regions: &[HitRegion], p: Point, primary: bool, now: Time) -> (Option<CsdAction>, bool) {
        let Some(kind) = hit(regions, p) else {
            self.pressed = None;
            return (None, false);
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
            k if resize_edge(k).is_some() => (Some(CsdAction::Resize(k)), true),
            _ => (None, false),
        }
    }

    /// A primary release at `p`: window buttons act when pressed and released on the same region.
    pub fn release(&mut self, regions: &[HitRegion], p: Point) -> Option<CsdAction> {
        let pressed = self.pressed.take()?;
        if hit(regions, p) != Some(pressed) {
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
    use presto_base::geom::Rect;

    fn regions() -> Vec<HitRegion> {
        vec![
            HitRegion { rect: Rect::new(0.0, 0.0, 800.0, 32.0), kind: HitKind::Caption },
            HitRegion { rect: Rect::new(700.0, 0.0, 740.0, 32.0), kind: HitKind::Maximize },
            HitRegion { rect: Rect::new(760.0, 0.0, 800.0, 32.0), kind: HitKind::Close },
            HitRegion { rect: Rect::new(0.0, 0.0, 800.0, 4.0), kind: HitKind::ResizeTop },
            HitRegion { rect: Rect::new(0.0, 0.0, 8.0, 8.0), kind: HitKind::ResizeTopLeft },
            HitRegion { rect: Rect::new(300.0, 8.0, 400.0, 28.0), kind: HitKind::Client },
            HitRegion { rect: Rect::new(8.0, 8.0, 28.0, 28.0), kind: HitKind::SystemMenu },
        ]
    }

    fn t(ms: u64) -> Time {
        Time(ms * 1_000_000)
    }

    #[test]
    fn later_regions_win() {
        let r = regions();
        assert_eq!(hit(&r, Point::new(400.0, 16.0)), Some(HitKind::Caption));
        assert_eq!(hit(&r, Point::new(780.0, 16.0)), Some(HitKind::Close));
        assert_eq!(hit(&r, Point::new(400.0, 2.0)), Some(HitKind::ResizeTop));
        assert_eq!(hit(&r, Point::new(2.0, 2.0)), Some(HitKind::ResizeTopLeft));
        assert_eq!(hit(&r, Point::new(400.0, 300.0)), None);
        // A client cut-out (a search box in the title bar) blocks the caption.
        assert_eq!(hit(&r, Point::new(350.0, 16.0)), None);
    }

    #[test]
    fn system_menu_opens_the_window_menu() {
        let r = regions();
        let mut s = CsdState::default();
        assert_eq!(s.press(&r, Point::new(16.0, 16.0), true, t(0)), (Some(CsdAction::WindowMenu), true));
    }

    #[test]
    fn caption_press_moves_and_double_click_maximizes() {
        let r = regions();
        let mut s = CsdState::default();
        assert_eq!(s.press(&r, Point::new(100.0, 16.0), true, t(0)), (Some(CsdAction::Move), true));
        assert_eq!(s.press(&r, Point::new(102.0, 17.0), true, t(300)), (Some(CsdAction::ToggleMaximize), true));
        // A third click starts over.
        assert_eq!(s.press(&r, Point::new(102.0, 17.0), true, t(500)), (Some(CsdAction::Move), true));
        // Too slow, or too far.
        assert_eq!(s.press(&r, Point::new(102.0, 17.0), true, t(1000)), (Some(CsdAction::Move), true));
        assert_eq!(s.press(&r, Point::new(150.0, 17.0), true, t(1100)), (Some(CsdAction::Move), true));
    }

    #[test]
    fn secondary_press_on_caption_opens_the_window_menu() {
        let r = regions();
        let mut s = CsdState::default();
        assert_eq!(s.press(&r, Point::new(100.0, 16.0), false, t(0)), (Some(CsdAction::WindowMenu), true));
        assert_eq!(s.press(&r, Point::new(100.0, 300.0), false, t(0)), (None, false));
    }

    #[test]
    fn edges_resize() {
        let r = regions();
        let mut s = CsdState::default();
        assert_eq!(s.press(&r, Point::new(3.0, 3.0), true, t(0)), (Some(CsdAction::Resize(HitKind::ResizeTopLeft)), true));
        assert_eq!(resize_edge(HitKind::ResizeTopLeft), Some(ResizeEdge::TopLeft));
        assert_eq!(cursor_for(HitKind::ResizeTop), Some(CursorIcon::NResize));
        assert_eq!(cursor_for(HitKind::Caption), None);
    }

    #[test]
    fn buttons_act_on_release_inside() {
        let r = regions();
        let mut s = CsdState::default();
        assert_eq!(s.press(&r, Point::new(780.0, 16.0), true, t(0)), (None, false));
        assert_eq!(s.release(&r, Point::new(781.0, 15.0)), Some(CsdAction::Close));
        // Released outside: nothing.
        s.press(&r, Point::new(720.0, 16.0), true, t(0));
        assert_eq!(s.release(&r, Point::new(400.0, 16.0)), None);
        s.press(&r, Point::new(720.0, 16.0), true, t(0));
        assert_eq!(s.release(&r, Point::new(721.0, 16.0)), Some(CsdAction::ToggleMaximize));
    }
}
