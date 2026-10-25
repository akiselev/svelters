//! Monitors from `NSScreen`, in presto's global coordinates: points, origin at the primary screen's
//! top-left (AppKit's origin is its bottom-left, y up).

use crate::shim::Monitor;
use objc2::rc::Retained;
use objc2::{MainThreadMarker, msg_send, sel};
use objc2_app_kit::NSScreen;
use objc2_foundation::{NSObjectProtocol, NSRect};
use presto_base::geom::{Point, Rect, Scale};

/// AppKit screen rect to global top-left rect, given the primary screen's height.
pub(crate) fn to_global(r: NSRect, primary_height: f64) -> Rect {
    let top = primary_height - (r.origin.y + r.size.height);
    Rect {
        min: Point::new(r.origin.x as f32, top as f32),
        max: Point::new(
            (r.origin.x + r.size.width) as f32,
            (top + r.size.height) as f32,
        ),
    }
}

pub(crate) fn primary_height(mtm: MainThreadMarker) -> f64 {
    NSScreen::screens(mtm)
        .firstObject()
        .map(|s| s.frame().size.height)
        .unwrap_or(0.0)
}

pub(crate) fn monitors(mtm: MainThreadMarker) -> Vec<Monitor> {
    let screens = NSScreen::screens(mtm);
    let ph = primary_height(mtm);
    screens
        .iter()
        .enumerate()
        .map(|(i, s)| monitor(&s, ph, i == 0))
        .collect()
}

fn monitor(s: &Retained<NSScreen>, ph: f64, primary: bool) -> Monitor {
    // `localizedName` (10.15) and `maximumFramesPerSecond` (12) are checked before use.
    let name = if s.respondsToSelector(sel!(localizedName)) {
        s.localizedName().to_string()
    } else {
        String::new()
    };
    let refresh_hz = s.respondsToSelector(sel!(maximumFramesPerSecond)).then(|| {
        // SAFETY: the selector exists (checked) and returns NSInteger.
        let fps: isize = unsafe { msg_send![&**s, maximumFramesPerSecond] };
        fps as f32
    });
    Monitor {
        name,
        bounds: to_global(s.frame(), ph),
        work_area: to_global(s.visibleFrame(), ph),
        scale: Scale(s.backingScaleFactor() as f32),
        refresh_hz: refresh_hz.filter(|hz| *hz > 0.0),
        primary,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use objc2_foundation::{NSPoint, NSSize};

    #[test]
    fn flips_to_top_left() {
        // A 1440x900 primary; a second screen above it.
        let r = to_global(
            NSRect::new(NSPoint::new(0.0, 900.0), NSSize::new(1920.0, 1080.0)),
            900.0,
        );
        assert_eq!(r.min, Point::new(0.0, -1080.0));
        assert_eq!(r.max, Point::new(1920.0, 0.0));
        let menu_bar_less = to_global(
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1440.0, 875.0)),
            900.0,
        );
        assert_eq!(menu_bar_less.min.y, 25.0);
    }
}
