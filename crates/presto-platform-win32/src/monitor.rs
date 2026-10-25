//! Monitors: bounds, work areas, per-monitor DPI and refresh rates.

use crate::clipboard::utf16_until_nul;
use presto_platform::Monitor;
use presto_base::geom::{DeviceRect, DeviceSize, Scale};
use windows::Win32::Foundation::{LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    DEVMODEW, ENUM_CURRENT_SETTINGS, EnumDisplayMonitors, EnumDisplaySettingsW, GetMonitorInfoW, HDC, HMONITOR,
    MONITORINFO, MONITORINFOEXW,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows_core::{BOOL, PCWSTR};

/// Windows' baseline DPI (scale 1.0).
pub const BASE_DPI: u32 = 96;

pub fn dpi_to_scale(dpi: u32) -> Scale {
    Scale(dpi.max(1) as f32 / BASE_DPI as f32)
}

pub fn device_rect(r: &RECT) -> DeviceRect {
    DeviceRect { x: r.left, y: r.top, w: (r.right - r.left).max(0) as u32, h: (r.bottom - r.top).max(0) as u32 }
}

pub fn monitor_info(hmon: HMONITOR) -> Option<Monitor> {
    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    if !unsafe { GetMonitorInfoW(hmon, &mut info as *mut MONITORINFOEXW as *mut MONITORINFO) }.as_bool() {
        return None;
    }
    let (mut dx, mut dy) = (BASE_DPI, BASE_DPI);
    let _ = unsafe { GetDpiForMonitor(hmon, MDT_EFFECTIVE_DPI, &mut dx, &mut dy) };
    let mut mode = DEVMODEW { dmSize: std::mem::size_of::<DEVMODEW>() as u16, ..Default::default() };
    let refresh = unsafe { EnumDisplaySettingsW(PCWSTR(info.szDevice.as_ptr()), ENUM_CURRENT_SETTINGS, &mut mode) }
        .as_bool()
        // 0 and 1 mean "the hardware default".
        .then_some(mode.dmDisplayFrequency)
        .filter(|&hz| hz > 1)
        .map(|hz| hz * 1000);
    let rect = device_rect(&info.monitorInfo.rcMonitor);
    Some(Monitor {
        name: Some(utf16_until_nul(&info.szDevice)),
        position: (rect.x, rect.y),
        size: DeviceSize { w: rect.w, h: rect.h },
        work_area: Some(device_rect(&info.monitorInfo.rcWork)),
        scale: dpi_to_scale(dx),
        refresh_millihertz: refresh,
    })
}

pub fn monitors() -> Vec<Monitor> {
    unsafe extern "system" fn collect(hmon: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> BOOL {
        let out = unsafe { &mut *(data.0 as *mut Vec<HMONITOR>) };
        out.push(hmon);
        BOOL(1)
    }
    let mut handles: Vec<HMONITOR> = Vec::new();
    let _ = unsafe { EnumDisplayMonitors(None, None, Some(collect), LPARAM(&mut handles as *mut _ as isize)) };
    handles.into_iter().filter_map(monitor_info).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dpi_scales() {
        assert_eq!(dpi_to_scale(96), Scale(1.0));
        assert_eq!(dpi_to_scale(144), Scale(1.5));
        assert_eq!(dpi_to_scale(192), Scale(2.0));
    }

    #[test]
    fn rects_convert() {
        let r = RECT { left: -1920, top: 0, right: 0, bottom: 1080 };
        assert_eq!(device_rect(&r), DeviceRect { x: -1920, y: 0, w: 1920, h: 1080 });
    }
}
