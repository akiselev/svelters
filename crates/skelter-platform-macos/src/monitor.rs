//! Monitors from `NSScreen`, in skelter's global coordinates: points, origin at the primary screen's
//! top-left (AppKit's origin is its bottom-left, y up).

use objc2::rc::Retained;
use objc2::{MainThreadMarker, msg_send, sel};
use objc2_app_kit::NSScreen;
use objc2_foundation::{NSObject, NSObjectProtocol, NSRect, NSString};
use skelter_base::geom::{DeviceRect, DeviceSize, Point, Rect, Scale};
use skelter_platform::Monitor;

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

/// The `CGDirectDisplayID` of a screen (`deviceDescription["NSScreenNumber"]`).
fn display_id(s: &NSScreen) -> Option<u32> {
    let key = NSString::from_str("NSScreenNumber");
    // SAFETY: `deviceDescription` returns a dictionary (never nil in practice; checked), and
    // NSScreenNumber is an NSNumber.
    unsafe {
        let desc: Option<Retained<NSObject>> = msg_send![s, deviceDescription];
        let num: Option<Retained<NSObject>> = msg_send![&*desc?, objectForKey: &*key];
        let id: u32 = msg_send![&*num?, unsignedIntValue];
        Some(id)
    }
}

/// Pixels per inch of a screen whose framebuffer is `device` pixels and whose panel measures `mm`
/// (`CGDisplayScreenSize`, millimetres; zero when unknown).
pub(crate) fn screen_ppi(device: DeviceSize, mm: (f64, f64)) -> Option<f32> {
    Monitor::ppi_from_mm(device, (mm.0 as f32, mm.1 as f32))
}

/// The monitor for `s`, given the primary screen's height.
pub(crate) fn monitor_of_screen(s: &Retained<NSScreen>, ph: f64) -> Monitor {
    monitor(s, ph, false)
}

fn monitor(s: &Retained<NSScreen>, ph: f64, _primary: bool) -> Monitor {
    // `localizedName` (10.15) and `maximumFramesPerSecond` (12) are checked before use.
    let name = s
        .respondsToSelector(sel!(localizedName))
        .then(|| s.localizedName().to_string());
    let fps = s.respondsToSelector(sel!(maximumFramesPerSecond)).then(|| {
        // SAFETY: the selector exists (checked) and returns NSInteger.
        let fps: isize = unsafe { msg_send![&**s, maximumFramesPerSecond] };
        fps
    });
    let scale = s.backingScaleFactor();
    let bounds = to_device(&to_global(s.frame(), ph), scale);
    let size = DeviceSize {
        w: bounds.w,
        h: bounds.h,
    };
    let mm = display_id(s).map(|id| objc2_core_graphics::CGDisplayScreenSize(id));
    Monitor {
        name,
        position: (bounds.x, bounds.y),
        size: DeviceSize {
            w: bounds.w,
            h: bounds.h,
        },
        work_area: Some(to_device(&to_global(s.visibleFrame(), ph), scale)),
        scale: Scale(scale as f32),
        refresh_millihertz: fps.filter(|f| *f > 0).map(|f| f as u32 * 1000),
        ppi: mm.and_then(|mm| screen_ppi(size, (mm.width, mm.height))),
    }
}

/// Global points to device pixels at one screen's scale (macOS's desktop is in points; each screen's
/// pixels are its points times its own scale).
pub(crate) fn to_device(r: &Rect, scale: f64) -> DeviceRect {
    let px = |v: f32| (v as f64 * scale).round() as i32;
    DeviceRect {
        x: px(r.min.x),
        y: px(r.min.y),
        w: (px(r.max.x) - px(r.min.x)).max(0) as u32,
        h: (px(r.max.y) - px(r.min.y)).max(0) as u32,
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

    #[test]
    fn ppi_from_screen_size() {
        // A 14" MacBook Pro framebuffer (3024x1964) on a 302 x 196 mm panel: about 254 ppi.
        let p = screen_ppi(DeviceSize { w: 3024, h: 1964 }, (302.0, 196.0)).unwrap();
        assert!((p - 254.3).abs() < 1.0, "{p}");
        assert_eq!(
            screen_ppi(DeviceSize { w: 3024, h: 1964 }, (0.0, 0.0)),
            None
        );
    }

    #[test]
    fn device_pixels_scale_points() {
        let r = Rect {
            min: Point::new(-10.0, 25.0),
            max: Point::new(1430.0, 900.0),
        };
        assert_eq!(
            to_device(&r, 2.0),
            DeviceRect {
                x: -20,
                y: 50,
                w: 2880,
                h: 1750
            }
        );
    }
}
