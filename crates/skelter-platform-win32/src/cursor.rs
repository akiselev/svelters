//! Cursor icons: CSS names to system cursors, applied on `WM_SETCURSOR` over the client area.

use skelter_base::cursor::CursorIcon;
use windows::Win32::UI::WindowsAndMessaging::{
    HCURSOR, IDC_APPSTARTING, IDC_ARROW, IDC_CROSS, IDC_HAND, IDC_HELP, IDC_IBEAM, IDC_NO, IDC_SIZEALL,
    IDC_SIZENESW, IDC_SIZENS, IDC_SIZENWSE, IDC_SIZEWE, IDC_WAIT, LoadCursorW,
};
use windows_core::PCWSTR;

/// The system cursor resource for an icon; `None` hides the cursor. CSS cursors Windows has no
/// system cursor for (grab, zoom, cell, alias, copy, context-menu, vertical-text) take the closest
/// one.
pub fn system_cursor(icon: CursorIcon) -> Option<PCWSTR> {
    Some(match icon {
        CursorIcon::None => return None,
        CursorIcon::Default | CursorIcon::ContextMenu | CursorIcon::Alias | CursorIcon::Copy => IDC_ARROW,
        CursorIcon::Help => IDC_HELP,
        CursorIcon::Pointer => IDC_HAND,
        CursorIcon::Progress => IDC_APPSTARTING,
        CursorIcon::Wait => IDC_WAIT,
        CursorIcon::Cell | CursorIcon::Crosshair => IDC_CROSS,
        CursorIcon::Text | CursorIcon::VerticalText => IDC_IBEAM,
        CursorIcon::Move | CursorIcon::AllScroll | CursorIcon::Grab | CursorIcon::Grabbing => IDC_SIZEALL,
        CursorIcon::NoDrop | CursorIcon::NotAllowed => IDC_NO,
        CursorIcon::EResize | CursorIcon::WResize | CursorIcon::EwResize | CursorIcon::ColResize => IDC_SIZEWE,
        CursorIcon::NResize | CursorIcon::SResize | CursorIcon::NsResize | CursorIcon::RowResize => IDC_SIZENS,
        CursorIcon::NeResize | CursorIcon::SwResize | CursorIcon::NeswResize => IDC_SIZENESW,
        CursorIcon::NwResize | CursorIcon::SeResize | CursorIcon::NwseResize => IDC_SIZENWSE,
        CursorIcon::ZoomIn | CursorIcon::ZoomOut => IDC_ARROW,
    })
}

/// Loads (shared, never freed) the system cursor for `icon`; `None` = no cursor.
pub fn load(icon: CursorIcon) -> Option<HCURSOR> {
    let id = system_cursor(icon)?;
    unsafe { LoadCursorW(None, id) }.ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_names_map_to_system_cursors() {
        assert_eq!(system_cursor(CursorIcon::Text), Some(IDC_IBEAM));
        assert_eq!(system_cursor(CursorIcon::ColResize), Some(IDC_SIZEWE));
        assert_eq!(system_cursor(CursorIcon::NwseResize), Some(IDC_SIZENWSE));
        assert_eq!(system_cursor(CursorIcon::None), None);
    }
}
