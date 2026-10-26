//! Monitors: bounds, work areas, per-monitor DPI, refresh rates and physical pixel density.

use crate::clipboard::utf16_until_nul;
use skelter_platform::Monitor;
use skelter_base::geom::{DeviceRect, DeviceSize, Scale};
use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    CreateDCW, DEVMODEW, DeleteDC, ENUM_CURRENT_SETTINGS, EnumDisplayMonitors, EnumDisplaySettingsW, GetDeviceCaps,
    GetMonitorInfoW, HDC, HMONITOR, HORZSIZE, MONITOR_DEFAULTTONEAREST, MONITORINFO, MONITORINFOEXW, MonitorFromWindow,
    VERTSIZE,
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

/// Pixels per inch from `GetDeviceCaps`' `HORZSIZE`/`VERTSIZE` (millimetres, from the EDID; 0 when unknown).
pub fn ppi_from_caps(size: DeviceSize, horz_mm: i32, vert_mm: i32) -> Option<f32> {
    Monitor::ppi_from_mm(size, (horz_mm.max(0) as f32, vert_mm.max(0) as f32))
}

/// The physical size (millimetres) of the display device `device` (`MONITORINFOEXW.szDevice`).
fn physical_mm(device: &[u16]) -> (i32, i32) {
    let name = PCWSTR(device.as_ptr());
    let dc = unsafe { CreateDCW(name, name, PCWSTR::null(), None) };
    if dc.is_invalid() {
        return (0, 0);
    }
    let mm = unsafe { (GetDeviceCaps(Some(dc), HORZSIZE), GetDeviceCaps(Some(dc), VERTSIZE)) };
    let _ = unsafe { DeleteDC(dc) };
    mm
}

/// The monitor showing most of `hwnd` (the nearest one when it is off screen).
pub fn monitor_handle(hwnd: HWND) -> HMONITOR {
    unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) }
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
    let size = DeviceSize { w: rect.w, h: rect.h };
    let (horz, vert) = physical_mm(&info.szDevice);
    Some(Monitor {
        name: Some(utf16_until_nul(&info.szDevice)),
        position: (rect.x, rect.y),
        size,
        work_area: Some(device_rect(&info.monitorInfo.rcWork)),
        scale: dpi_to_scale(dx),
        refresh_millihertz: refresh,
        ppi: ppi_from_caps(size, horz, vert),
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
    fn ppi_from_device_caps() {
        // A 15.6" 1920x1080 laptop panel: 344 x 194 mm, about 141 ppi.
        let p = ppi_from_caps(DeviceSize { w: 1920, h: 1080 }, 344, 194).unwrap();
        assert!((p - 141.2).abs() < 0.5, "{p}");
        assert_eq!(ppi_from_caps(DeviceSize { w: 1920, h: 1080 }, 0, 0), None);
    }

    #[test]
    fn rects_convert() {
        let r = RECT { left: -1920, top: 0, right: 0, bottom: 1080 };
        assert_eq!(device_rect(&r), DeviceRect { x: -1920, y: 0, w: 1920, h: 1080 });
    }
}
