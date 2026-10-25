//! Custom title bars: the app's [`HitRegion`]s answer `WM_NCHITTEST`, so the OS drags, resizes,
//! snaps and shows Windows 11's snap-layout flyout (over a region answering `HTMAXBUTTON`) while the
//! app draws everything.

use crate::shim::{HitKind, HitRegion};
use presto_base::geom::Point;
use windows::Win32::UI::WindowsAndMessaging::{
    HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTCAPTION, HTCLIENT, HTCLOSE, HTLEFT, HTMAXBUTTON, HTMINBUTTON,
    HTRIGHT, HTSYSMENU, HTTOP, HTTOPLEFT, HTTOPRIGHT,
};

pub fn ht_code(kind: HitKind) -> u32 {
    match kind {
        HitKind::Client => HTCLIENT,
        HitKind::Caption => HTCAPTION,
        HitKind::SystemMenu => HTSYSMENU,
        HitKind::Minimize => HTMINBUTTON,
        HitKind::Maximize => HTMAXBUTTON,
        HitKind::Close => HTCLOSE,
        HitKind::ResizeTop => HTTOP,
        HitKind::ResizeBottom => HTBOTTOM,
        HitKind::ResizeLeft => HTLEFT,
        HitKind::ResizeRight => HTRIGHT,
        HitKind::ResizeTopLeft => HTTOPLEFT,
        HitKind::ResizeTopRight => HTTOPRIGHT,
        HitKind::ResizeBottomLeft => HTBOTTOMLEFT,
        HitKind::ResizeBottomRight => HTBOTTOMRIGHT,
    }
}

/// The region kind at `p` (window points): later regions are above earlier ones.
pub fn region_at(regions: &[HitRegion], p: Point) -> Option<HitKind> {
    regions.iter().rev().find(|r| r.rect.contains(p)).map(|r| r.kind)
}

/// The resize border a point inside the client area falls in, for windows whose frame is extended
/// into the client area (`WM_NCCALCSIZE` returns the whole window). `border` is the resize border
/// width in points; the top border only resizes when `top` (a maximised window has none).
pub fn resize_border(size: (f32, f32), p: Point, border: f32, top: bool) -> Option<HitKind> {
    let (w, h) = size;
    let left = p.x < border;
    let right = p.x >= w - border;
    let up = top && p.y < border;
    let down = p.y >= h - border;
    Some(match (up, down, left, right) {
        (true, _, true, _) => HitKind::ResizeTopLeft,
        (true, _, _, true) => HitKind::ResizeTopRight,
        (_, true, true, _) => HitKind::ResizeBottomLeft,
        (_, true, _, true) => HitKind::ResizeBottomRight,
        (true, _, _, _) => HitKind::ResizeTop,
        (_, true, _, _) => HitKind::ResizeBottom,
        (_, _, true, _) => HitKind::ResizeLeft,
        (_, _, _, true) => HitKind::ResizeRight,
        _ => return None,
    })
}

/// `WM_NCHITTEST` for a custom-decorated window: resize borders first (unless maximised), then the
/// app's regions, else client.
pub fn hit_test(regions: &[HitRegion], size: (f32, f32), p: Point, border: f32, maximized: bool, resizable: bool) -> u32 {
    if resizable && !maximized {
        if let Some(k) = resize_border(size, p, border, true) {
            return ht_code(k);
        }
    }
    region_at(regions, p).map(ht_code).unwrap_or(HTCLIENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use presto_base::geom::Rect;

    fn regions() -> Vec<HitRegion> {
        vec![
            HitRegion { rect: Rect::new(0.0, 0.0, 800.0, 32.0), kind: HitKind::Caption },
            HitRegion { rect: Rect::new(100.0, 0.0, 300.0, 32.0), kind: HitKind::Client },
            HitRegion { rect: Rect::new(708.0, 0.0, 754.0, 32.0), kind: HitKind::Maximize },
            HitRegion { rect: Rect::new(754.0, 0.0, 800.0, 32.0), kind: HitKind::Close },
        ]
    }

    #[test]
    fn later_regions_win() {
        let r = regions();
        assert_eq!(region_at(&r, Point::new(50.0, 10.0)), Some(HitKind::Caption));
        assert_eq!(region_at(&r, Point::new(150.0, 10.0)), Some(HitKind::Client), "a tab in the caption");
        assert_eq!(region_at(&r, Point::new(720.0, 10.0)), Some(HitKind::Maximize));
        assert_eq!(region_at(&r, Point::new(50.0, 100.0)), None);
    }

    #[test]
    fn snap_layouts_need_htmaxbutton() {
        let r = regions();
        let ht = hit_test(&r, (800.0, 600.0), Point::new(720.0, 16.0), 6.0, false, true);
        assert_eq!(ht, HTMAXBUTTON);
        assert_eq!(hit_test(&r, (800.0, 600.0), Point::new(400.0, 300.0), 6.0, false, true), HTCLIENT);
    }

    #[test]
    fn borders_resize_unless_maximized() {
        let r = regions();
        assert_eq!(hit_test(&r, (800.0, 600.0), Point::new(2.0, 2.0), 6.0, false, true), HTTOPLEFT);
        assert_eq!(hit_test(&r, (800.0, 600.0), Point::new(400.0, 2.0), 6.0, false, true), HTTOP);
        assert_eq!(hit_test(&r, (800.0, 600.0), Point::new(799.0, 599.0), 6.0, false, true), HTBOTTOMRIGHT);
        assert_eq!(hit_test(&r, (800.0, 600.0), Point::new(400.0, 2.0), 6.0, true, true), HTCAPTION);
        assert_eq!(hit_test(&r, (800.0, 600.0), Point::new(2.0, 300.0), 6.0, false, false), HTCLIENT);
    }
}
