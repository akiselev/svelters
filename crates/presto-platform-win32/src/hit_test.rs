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

/// The top resize band of a custom-framed window. Its `WM_NCCALCSIZE` removes only the caption, so
/// the OS keeps the side and bottom borders as non-client area; the top border lies inside the
/// client area and is answered here (with the top corners), `border` points tall and, at the
/// corners, wide.
pub fn top_band(width: f32, p: Point, border: f32) -> Option<HitKind> {
    if p.y >= border {
        return None;
    }
    Some(if p.x < border {
        HitKind::ResizeTopLeft
    } else if p.x >= width - border {
        HitKind::ResizeTopRight
    } else {
        HitKind::ResizeTop
    })
}

/// `WM_NCHITTEST` inside the client area of a custom-framed window: the top resize band (when
/// `top_border` is given: resizable and not maximised), then the app's regions, else client.
pub fn hit_test(regions: &[HitRegion], width: f32, p: Point, top_border: Option<f32>) -> u32 {
    if let Some(k) = top_border.and_then(|b| top_band(width, p, b)) {
        return ht_code(k);
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
        assert_eq!(hit_test(&r, 800.0, Point::new(720.0, 16.0), Some(6.0)), HTMAXBUTTON);
        assert_eq!(hit_test(&r, 800.0, Point::new(400.0, 300.0), Some(6.0)), HTCLIENT);
    }

    #[test]
    fn top_band_resizes_unless_maximized() {
        let r = regions();
        assert_eq!(hit_test(&r, 800.0, Point::new(2.0, 2.0), Some(6.0)), HTTOPLEFT);
        assert_eq!(hit_test(&r, 800.0, Point::new(400.0, 2.0), Some(6.0)), HTTOP);
        assert_eq!(hit_test(&r, 800.0, Point::new(799.0, 2.0), Some(6.0)), HTTOPRIGHT);
        assert_eq!(hit_test(&r, 800.0, Point::new(400.0, 2.0), None), HTCAPTION, "maximised: no band");
        assert_eq!(hit_test(&r, 800.0, Point::new(780.0, 2.0), Some(6.0)), HTTOP, "the band is above the close button");
    }
}
