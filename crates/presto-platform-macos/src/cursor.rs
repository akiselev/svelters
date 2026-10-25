//! Cursor icons as `NSCursor`s. Shapes AppKit only has privately (help, diagonal resize, move, zoom,
//! busy) use the private class methods winit and other toolkits use, when the class answers them; the
//! arrow otherwise.

use crate::app;
use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2::{ClassType, MainThreadMarker, msg_send, sel};
use objc2_app_kit::{NSCursor, NSView, NSWindow};
use presto_base::cursor::CursorIcon;

fn private(selector: Sel) -> Option<Retained<NSCursor>> {
    let cls = NSCursor::class();
    if !cls.responds_to(selector) {
        return None;
    }
    // SAFETY: the class answers the selector (checked); these class methods return an NSCursor.
    unsafe { msg_send![cls, performSelector: selector] }
}

/// The `NSCursor` for `icon`; `None` for [`CursorIcon::None`] (hidden). The edge-resize cursors are
/// deprecated in macOS 15 in favour of `frameResizeCursorFromPosition:inDirections:`, which earlier
/// releases lack.
#[allow(deprecated)]
pub(crate) fn ns_cursor(icon: CursorIcon) -> Option<Retained<NSCursor>> {
    use CursorIcon as C;
    let c = match icon {
        C::None => return None,
        C::Default => NSCursor::arrowCursor(),
        C::ContextMenu => NSCursor::contextualMenuCursor(),
        C::Help => private(sel!(_helpCursor)).unwrap_or_else(NSCursor::arrowCursor),
        C::Pointer => NSCursor::pointingHandCursor(),
        C::Progress | C::Wait => {
            private(sel!(busyButClickableCursor)).unwrap_or_else(NSCursor::arrowCursor)
        }
        C::Cell | C::Crosshair => NSCursor::crosshairCursor(),
        C::Text => NSCursor::IBeamCursor(),
        C::VerticalText => NSCursor::IBeamCursorForVerticalLayout(),
        C::Alias => NSCursor::dragLinkCursor(),
        C::Copy => NSCursor::dragCopyCursor(),
        C::Move | C::AllScroll => {
            private(sel!(_moveCursor)).unwrap_or_else(NSCursor::openHandCursor)
        }
        C::NoDrop | C::NotAllowed => NSCursor::operationNotAllowedCursor(),
        C::Grab => NSCursor::openHandCursor(),
        C::Grabbing => NSCursor::closedHandCursor(),
        C::EResize => NSCursor::resizeRightCursor(),
        C::WResize => NSCursor::resizeLeftCursor(),
        C::NResize => NSCursor::resizeUpCursor(),
        C::SResize => NSCursor::resizeDownCursor(),
        C::EwResize | C::ColResize => NSCursor::resizeLeftRightCursor(),
        C::NsResize | C::RowResize => NSCursor::resizeUpDownCursor(),
        C::NeResize | C::SwResize | C::NeswResize => {
            private(sel!(_windowResizeNorthEastSouthWestCursor))
                .unwrap_or_else(NSCursor::arrowCursor)
        }
        C::NwResize | C::SeResize | C::NwseResize => {
            private(sel!(_windowResizeNorthWestSouthEastCursor))
                .unwrap_or_else(NSCursor::arrowCursor)
        }
        C::ZoomIn => private(sel!(_zoomInCursor)).unwrap_or_else(NSCursor::arrowCursor),
        C::ZoomOut => private(sel!(_zoomOutCursor)).unwrap_or_else(NSCursor::arrowCursor),
    };
    Some(c)
}

/// Shows `icon` now if the pointer is over `view` in the key window, and refreshes the view's cursor
/// rect (`resetCursorRects`) so AppKit restores it on re-entry.
pub(crate) fn apply(_mtm: MainThreadMarker, icon: CursorIcon, window: &NSWindow, view: &NSView) {
    window.invalidateCursorRectsForView(view);
    let inside = window.isKeyWindow() && {
        let p = view.convertPoint_fromView(window.mouseLocationOutsideOfEventStream(), None);
        view.mouse_inRect(p, view.bounds())
    };
    if !inside {
        return;
    }
    set_hidden(icon == CursorIcon::None);
    if let Some(c) = ns_cursor(icon) {
        c.set();
    }
}

/// Hides or shows the cursor, keeping AppKit's hide/unhide calls balanced.
pub(crate) fn set_hidden(hidden: bool) {
    let changed = app::with_app(|st| std::mem::replace(&mut st.cursor_hidden, hidden) != hidden)
        .unwrap_or(false);
    if changed {
        if hidden {
            NSCursor::hide()
        } else {
            NSCursor::unhide()
        }
    }
}
