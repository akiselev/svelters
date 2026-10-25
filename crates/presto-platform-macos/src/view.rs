//! The content view of every presto window: hosts the window's `CAMetalLayer`, turns `NSEvent`s into
//! presto input, implements `NSTextInputClient` over the window's text-input target, hosts the
//! AccessKit adapter, takes file drops, and reports size, scale, theme and live resize.
//!
//! Keys follow architecture §11.2 (IME first): with a text-input target, presses without Cmd or Ctrl
//! go through the input context; the IME's `insertText:` with the key's own text, or its
//! `doCommandBySelector:`, hands the press back as a key event, while compositions, dead keys,
//! accents, dictation and the Character Viewer edit the target directly ([`text_input`]).

use crate::app::{self, Pending, post, post_input};
use crate::events;
use crate::keymap;
use crate::shim::HitKind;
use crate::text_input::{self, ClientTarget, ImeTarget, InsertOutcome, KeyRoute, NsRange};
use crate::window::size_of;
use crate::{a11y, cursor, layer};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSAppearanceCustomization, NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSApplication,
    NSDragOperation, NSEvent, NSEventType, NSPasteboard, NSPasteboardTypeFileURL,
    NSTextInputClient, NSTrackingArea, NSTrackingAreaOptions, NSView,
};
use objc2_foundation::{
    NSArray, NSAttributedString, NSAttributedStringKey, NSNotFound, NSObjectProtocol, NSPoint,
    NSRange, NSRangePointer, NSRect, NSSize, NSString, NSUInteger, NSURL, NSUserDefaults,
};
use presto_base::event::{
    ImeEvent, InputEvent, KeyEvent, KeyLocation, KeyState, Modifiers, PointerPhase, Theme,
    WindowEvent,
};
use presto_base::geom::{Point, Rect};
use presto_base::ids::WindowId;
use presto_base::input::TextInputClient;
use presto_base::key::Key;
use std::path::PathBuf;

pub(crate) struct ViewIvars {
    window: WindowId,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars hold a plain id and PrestoView does not implement
    // Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "PrestoView"]
    #[ivars = ViewIvars]
    pub(crate) struct PrestoView;

    unsafe impl NSObjectProtocol for PrestoView {}

    impl PrestoView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(acceptsFirstResponder))]
        fn accepts_first_responder(&self) -> bool {
            true
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(wantsUpdateLayer))]
        fn wants_update_layer(&self) -> bool {
            true
        }

        #[unsafe(method(mouseDownCanMoveWindow))]
        fn mouse_down_can_move_window(&self) -> bool {
            false
        }

        // ---- size, scale, appearance, live resize ----

        #[unsafe(method(setFrameSize:))]
        fn set_frame_size(&self, size: NSSize) {
            // SAFETY: NSView's implementation with the same argument.
            let _: () = unsafe { msg_send![super(self), setFrameSize: size] };
            self.report_size();
        }

        #[unsafe(method(viewDidChangeBackingProperties))]
        fn did_change_backing_properties(&self) {
            // SAFETY: as above.
            let _: () = unsafe { msg_send![super(self), viewDidChangeBackingProperties] };
            let id = self.window_id();
            let Some(window) = self.window() else { return };
            let scale = window.backingScaleFactor();
            app::with_window(id, |w| layer::set_scale(std::iter::once(&*w.layer).chain(w.os_layers.values().map(|l| &**l)), scale));
            post(Pending::Window(id, WindowEvent::ScaleChanged { scale: presto_base::geom::Scale(scale as f32) }));
            self.report_size();
        }

        #[unsafe(method(viewWillStartLiveResize))]
        fn will_start_live_resize(&self) {
            // SAFETY: as above.
            let _: () = unsafe { msg_send![super(self), viewWillStartLiveResize] };
            self.set_live_resize(true);
        }

        #[unsafe(method(viewDidEndLiveResize))]
        fn did_end_live_resize(&self) {
            // SAFETY: as above.
            let _: () = unsafe { msg_send![super(self), viewDidEndLiveResize] };
            self.set_live_resize(false);
        }

        #[unsafe(method(viewDidChangeEffectiveAppearance))]
        fn did_change_appearance(&self) {
            post(Pending::Window(self.window_id(), WindowEvent::ThemeChanged(self.theme())));
        }

        #[unsafe(method(resetCursorRects))]
        fn reset_cursor_rects(&self) {
            let icon = app::with_window(self.window_id(), |w| w.cursor).unwrap_or_default();
            if let Some(c) = cursor::ns_cursor(icon) {
                self.addCursorRect_cursor(self.bounds(), &c);
            }
        }

        // ---- pointer ----

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            if self.caption_mouse_down(event) {
                return;
            }
            self.pointer(event, PointerPhase::Down, true);
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, event: &NSEvent) {
            self.pointer(event, PointerPhase::Up, true);
        }

        #[unsafe(method(rightMouseDown:))]
        fn right_mouse_down(&self, event: &NSEvent) {
            self.pointer(event, PointerPhase::Down, true);
        }

        #[unsafe(method(rightMouseUp:))]
        fn right_mouse_up(&self, event: &NSEvent) {
            self.pointer(event, PointerPhase::Up, true);
        }

        #[unsafe(method(otherMouseDown:))]
        fn other_mouse_down(&self, event: &NSEvent) {
            self.pointer(event, PointerPhase::Down, true);
        }

        #[unsafe(method(otherMouseUp:))]
        fn other_mouse_up(&self, event: &NSEvent) {
            self.pointer(event, PointerPhase::Up, true);
        }

        #[unsafe(method(mouseMoved:))]
        fn mouse_moved(&self, event: &NSEvent) {
            self.pointer(event, PointerPhase::Move, false);
        }

        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, event: &NSEvent) {
            self.pointer(event, PointerPhase::Move, false);
        }

        #[unsafe(method(rightMouseDragged:))]
        fn right_mouse_dragged(&self, event: &NSEvent) {
            self.pointer(event, PointerPhase::Move, false);
        }

        #[unsafe(method(otherMouseDragged:))]
        fn other_mouse_dragged(&self, event: &NSEvent) {
            self.pointer(event, PointerPhase::Move, false);
        }

        #[unsafe(method(mouseEntered:))]
        fn mouse_entered(&self, event: &NSEvent) {
            post_input(self.window_id(), events::os_time(event), events::crossing(self, event, PointerPhase::Enter));
        }

        #[unsafe(method(mouseExited:))]
        fn mouse_exited(&self, event: &NSEvent) {
            post_input(self.window_id(), events::os_time(event), events::crossing(self, event, PointerPhase::Leave));
        }

        #[unsafe(method(tabletPoint:))]
        fn tablet_point(&self, event: &NSEvent) {
            self.pointer(event, PointerPhase::Move, false);
        }

        #[unsafe(method(tabletProximity:))]
        fn tablet_proximity(&self, event: &NSEvent) {
            post_input(self.window_id(), events::os_time(event), events::proximity(self, event));
        }

        #[unsafe(method(scrollWheel:))]
        fn scroll_wheel(&self, event: &NSEvent) {
            post_input(self.window_id(), events::os_time(event), events::wheel(self, event));
        }

        #[unsafe(method(magnifyWithEvent:))]
        fn magnify(&self, event: &NSEvent) {
            post_input(self.window_id(), events::os_time(event), events::magnify(event));
        }

        #[unsafe(method(rotateWithEvent:))]
        fn rotate(&self, event: &NSEvent) {
            post_input(self.window_id(), events::os_time(event), events::rotate(event));
        }

        #[unsafe(method(smartMagnifyWithEvent:))]
        fn smart_magnify(&self, event: &NSEvent) {
            post_input(
                self.window_id(),
                events::os_time(event),
                InputEvent::Gesture(presto_base::event::GestureEvent::DoubleTap),
            );
        }

        /// Custom title bars: AppKit must not take title-bar clicks for window moves while the app
        /// marks its own caption regions (private AppKit hook GPUI also overrides).
        #[unsafe(method(_opaqueRectForWindowMoveWhenInTitlebar))]
        fn opaque_rect_for_window_move(&self) -> NSRect {
            let owns = app::with_window(self.window_id(), |w| w.hit_regions.iter().any(|r| r.kind == HitKind::Caption))
                .unwrap_or(false);
            if owns { self.bounds() } else { NSRect::ZERO }
        }

        // ---- keys ----

        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: &NSEvent) {
            self.handle_key_down(event);
        }

        #[unsafe(method(keyUp:))]
        fn key_up(&self, event: &NSEvent) {
            let policy = app::with_app(|st| st.option_as_alt).unwrap_or_default();
            let (ev, _) = events::key(event, KeyState::Released, policy);
            post_input(self.window_id(), events::os_time(event), InputEvent::Key(ev));
        }

        #[unsafe(method(flagsChanged:))]
        fn flags_changed(&self, event: &NSEvent) {
            let id = self.window_id();
            let t = events::os_time(event);
            let raw = events::raw_flags(event);
            let mods = keymap::modifiers_from_flags(raw);
            let code = event.keyCode();
            if let Some((key, transition)) = keymap::modifier_transition(code, raw) {
                for &state in transition.states() {
                    let ev = KeyEvent {
                        key: Some(key),
                        physical: keymap::physical_code(code),
                        text: None,
                        state,
                        repeat: false,
                        location: keymap::key_location(code),
                        modifiers: mods,
                    };
                    post_input(id, t, InputEvent::Key(ev));
                }
            }
            post_input(id, t, InputEvent::ModifiersChanged(mods));
        }

        /// Ctrl chords are taken here (AppKit keeps some, like Ctrl+Tab, from `keyDown:`); Cmd chords
        /// go to the menu bar first and reach `keyDown:` when no menu item takes them.
        #[unsafe(method(performKeyEquivalent:))]
        fn perform_key_equivalent(&self, event: &NSEvent) -> bool {
            self.ctrl_key_equivalent(event)
        }

        // The standard Edit actions (the default menu bar's items), as the shortcuts they stand for.

        #[unsafe(method(copy:))]
        fn copy(&self, _sender: Option<&AnyObject>) {
            self.shortcut(Key::C, 0x08, false);
        }

        #[unsafe(method(cut:))]
        fn cut(&self, _sender: Option<&AnyObject>) {
            self.shortcut(Key::X, 0x07, false);
        }

        #[unsafe(method(paste:))]
        fn paste(&self, _sender: Option<&AnyObject>) {
            self.shortcut(Key::V, 0x09, false);
        }

        #[unsafe(method(selectAll:))]
        fn select_all(&self, _sender: Option<&AnyObject>) {
            self.shortcut(Key::A, 0x00, false);
        }

        #[unsafe(method(undo:))]
        fn undo(&self, _sender: Option<&AnyObject>) {
            self.shortcut(Key::Z, 0x06, false);
        }

        #[unsafe(method(redo:))]
        fn redo(&self, _sender: Option<&AnyObject>) {
            self.shortcut(Key::Z, 0x06, true);
        }

        // ---- accessibility (AccessKit host) ----

        #[unsafe(method(isAccessibilityElement))]
        fn is_accessibility_element(&self) -> bool {
            false
        }

        #[unsafe(method(accessibilityChildren))]
        fn accessibility_children(&self) -> *mut AnyObject {
            a11y::with_adapter(self.window_id(), |a, h| (a.view_children(h).cast::<AnyObject>(), None))
                .unwrap_or(std::ptr::null_mut())
        }

        #[unsafe(method(accessibilityFocusedUIElement))]
        fn accessibility_focused(&self) -> *mut AnyObject {
            a11y::with_adapter(self.window_id(), |a, h| (a.focus(h).cast::<AnyObject>(), None))
                .unwrap_or(std::ptr::null_mut())
        }

        #[unsafe(method(accessibilityHitTest:))]
        fn accessibility_hit_test(&self, point: NSPoint) -> *mut AnyObject {
            let p = accesskit_macos::NSPoint::new(point.x, point.y);
            a11y::with_adapter(self.window_id(), |a, h| (a.hit_test(p, h).cast::<AnyObject>(), None))
                .unwrap_or(std::ptr::null_mut())
        }

        // ---- file drops ----

        #[unsafe(method(draggingEntered:))]
        fn dragging_entered(&self, sender: &AnyObject) -> NSDragOperation {
            let id = self.window_id();
            for path in dragged_paths(sender) {
                post_input(id, crate::time::now(), InputEvent::FileHovered(path));
            }
            NSDragOperation::Copy
        }

        #[unsafe(method(draggingExited:))]
        fn dragging_exited(&self, _sender: Option<&AnyObject>) {
            post_input(self.window_id(), crate::time::now(), InputEvent::FileHoverCancelled);
        }

        #[unsafe(method(performDragOperation:))]
        fn perform_drag_operation(&self, sender: &AnyObject) -> bool {
            let id = self.window_id();
            let paths = dragged_paths(sender);
            let any = !paths.is_empty();
            for path in paths {
                post_input(id, crate::time::now(), InputEvent::FileDropped(path));
            }
            any
        }
    }

    unsafe impl NSTextInputClient for PrestoView {
        #[unsafe(method(insertText:replacementRange:))]
        fn insert_text(&self, string: &AnyObject, replacement_range: NSRange) {
            self.ime_insert(&plain_string(string), ns_range(replacement_range));
        }

        #[unsafe(method(doCommandBySelector:))]
        fn do_command_by_selector(&self, _selector: Sel) {
            // The selector is ignored: key bindings are the app's (a user may have rebound the key).
            let id = self.window_id();
            let key = app::with_window(id, |w| {
                w.key.as_mut().filter(|k| !k.consumed).map(|k| {
                    k.consumed = true;
                    k.event.clone()
                })
            })
            .flatten();
            if let Some(ev) = key {
                post_input(id, crate::time::now(), InputEvent::Key(ev));
            }
        }

        #[unsafe(method(setMarkedText:selectedRange:replacementRange:))]
        fn set_marked_text(&self, string: &AnyObject, selected_range: NSRange, replacement_range: NSRange) {
            let id = self.window_id();
            app::with_window(id, |w| {
                if let Some(k) = w.key.as_mut() {
                    k.consumed = true;
                }
            });
            let text = plain_string(string);
            with_target(id, |t, ev| t.set_marked(&text, ns_range(selected_range), ns_range(replacement_range), ev));
        }

        #[unsafe(method(unmarkText))]
        fn unmark_text(&self) {
            with_target(self.window_id(), |t, ev| t.unmark(ev));
        }

        #[unsafe(method(selectedRange))]
        fn selected_range(&self) -> NSRange {
            objc_range(with_target(self.window_id(), |t, _| t.selected()).unwrap_or(NsRange::NOT_FOUND))
        }

        #[unsafe(method(markedRange))]
        fn marked_range(&self) -> NSRange {
            objc_range(with_target(self.window_id(), |t, _| t.marked()).unwrap_or(NsRange::NOT_FOUND))
        }

        #[unsafe(method(hasMarkedText))]
        fn has_marked_text(&self) -> bool {
            with_target(self.window_id(), |t, _| t.marked().to_range().is_some()).unwrap_or(false)
        }

        #[unsafe(method(attributedSubstringForProposedRange:actualRange:))]
        fn attributed_substring(&self, range: NSRange, actual_range: NSRangePointer) -> *mut NSAttributedString {
            let Some((text, actual)) = with_target(self.window_id(), |t, _| t.substring(ns_range(range))).flatten() else {
                return std::ptr::null_mut();
            };
            if !actual_range.is_null() {
                // SAFETY: AppKit passes a valid pointer or null (checked).
                unsafe { actual_range.write(objc_range(actual)) };
            }
            let s = NSAttributedString::initWithString(NSAttributedString::alloc(), &NSString::from_str(&text));
            Retained::autorelease_return(s)
        }

        #[unsafe(method(validAttributesForMarkedText))]
        fn valid_attributes_for_marked_text(&self) -> *mut NSArray<NSAttributedStringKey> {
            Retained::autorelease_return(NSArray::new())
        }

        #[unsafe(method(firstRectForCharacterRange:actualRange:))]
        fn first_rect_for_character_range(&self, range: NSRange, actual_range: NSRangePointer) -> NSRect {
            let Some((rect, actual)) = with_target(self.window_id(), |t, _| t.first_rect(ns_range(range))).flatten() else {
                return NSRect::ZERO;
            };
            if !actual_range.is_null() {
                // SAFETY: as above.
                unsafe { actual_range.write(objc_range(actual)) };
            }
            self.view_rect_to_screen(&rect)
        }

        #[unsafe(method(characterIndexForPoint:))]
        fn character_index_for_point(&self, point: NSPoint) -> NSUInteger {
            let p = self.screen_point_to_view(point);
            with_target(self.window_id(), |t, _| t.index_for_point(p)).unwrap_or(NSNotFound as NSUInteger)
        }
    }
);

impl PrestoView {
    pub(crate) fn new(mtm: MainThreadMarker, window: WindowId, frame: NSRect) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(ViewIvars { window });
        // SAFETY: NSView's designated initialiser.
        let view: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        let options = NSTrackingAreaOptions::MouseEnteredAndExited
            | NSTrackingAreaOptions::MouseMoved
            | NSTrackingAreaOptions::ActiveAlways
            | NSTrackingAreaOptions::InVisibleRect
            | NSTrackingAreaOptions::EnabledDuringMouseDrag;
        // SAFETY: the view owns the area; with InVisibleRect AppKit keeps its rect up to date.
        let area = unsafe {
            NSTrackingArea::initWithRect_options_owner_userInfo(
                NSTrackingArea::alloc(),
                NSRect::ZERO,
                options,
                Some(&view),
                None,
            )
        };
        view.addTrackingArea(&area);
        // SAFETY: NSPasteboardTypeFileURL is a valid pasteboard type.
        let types = NSArray::from_slice(&[unsafe { NSPasteboardTypeFileURL }]);
        view.registerForDraggedTypes(&types);
        view
    }

    pub(crate) fn window_id(&self) -> WindowId {
        self.ivars().window
    }

    fn pointer(&self, event: &NSEvent, phase: PointerPhase, with_button: bool) {
        post_input(
            self.window_id(),
            events::os_time(event),
            events::pointer(self, event, phase, with_button),
        );
    }

    fn report_size(&self) {
        let id = self.window_id();
        let Some(window) = self.window() else { return };
        let (size, scale) = size_of(self, window.backingScaleFactor());
        let changed = app::with_window(id, |w| {
            let new = Some((size, scale.0));
            std::mem::replace(&mut w.last_size, new) != new
        })
        .unwrap_or(false);
        if changed {
            post(Pending::Window(id, WindowEvent::Resized { size, scale }));
        }
    }

    fn set_live_resize(&self, active: bool) {
        let id = self.window_id();
        let changed = app::with_window(id, |w| {
            w.layer.setPresentsWithTransaction(active);
            std::mem::replace(&mut w.live_resize, active) != active
        })
        .unwrap_or(false);
        if changed {
            post(Pending::LiveResize(id, active));
        }
    }

    fn theme(&self) -> Theme {
        // SAFETY: the appearance names are valid statics.
        let names = NSArray::from_slice(&[unsafe { NSAppearanceNameAqua }, unsafe {
            NSAppearanceNameDarkAqua
        }]);
        let best = self
            .effectiveAppearance()
            .bestMatchFromAppearancesWithNames(&names);
        // SAFETY: as above.
        if best.is_some_and(|b| &*b == unsafe { NSAppearanceNameDarkAqua }) {
            Theme::Dark
        } else {
            Theme::Light
        }
    }

    /// A click in an app-drawn caption moves the window (or zooms or minimises on double click, as
    /// the user's title-bar preference says). Returns whether the click was taken.
    fn caption_mouse_down(&self, event: &NSEvent) -> bool {
        let p = events::location(self, event);
        let kind = app::with_window(self.window_id(), |w| {
            w.hit_regions
                .iter()
                .rev()
                .find(|r| contains(&r.rect, p))
                .map(|r| r.kind)
        })
        .flatten();
        if kind != Some(HitKind::Caption) {
            return false;
        }
        let Some(window) = self.window() else {
            return false;
        };
        if event.clickCount() == 2 {
            let action = NSUserDefaults::standardUserDefaults()
                .stringForKey(&NSString::from_str("AppleActionOnDoubleClick"))
                .map(|s| s.to_string());
            match action.as_deref() {
                Some("Minimize") => window.miniaturize(None),
                Some("None") => {}
                _ => window.zoom(None),
            }
        } else {
            window.performWindowDragWithEvent(event);
        }
        true
    }

    fn handle_key_down(&self, event: &NSEvent) {
        let id = self.window_id();
        let t = events::os_time(event);
        let policy = app::with_app(|st| st.option_as_alt).unwrap_or_default();
        let (ev, option_alt) = events::key(event, KeyState::Pressed, policy);
        let raw = events::raw_flags(event);
        let composing = with_target(id, |t, _| t.marked().to_range().is_some());
        let chord = raw & (keymap::flags::COMMAND | keymap::flags::CONTROL) != 0;
        match text_input::route_key(
            composing.is_some(),
            composing.unwrap_or(false),
            option_alt,
            chord,
        ) {
            KeyRoute::Direct => post_input(id, t, InputEvent::Key(ev)),
            KeyRoute::ImeFirst => {
                app::with_window(id, |w| {
                    w.key = Some(crate::window::PendingKey {
                        event: ev,
                        consumed: false,
                    })
                });
                let handled = self.inputContext().is_some_and(|cx| cx.handleEvent(event));
                let pending = app::with_window(id, |w| w.key.take()).flatten();
                if let Some(p) = pending
                    && !p.consumed
                    && !handled
                {
                    post_input(id, t, InputEvent::Key(p.event));
                }
            }
        }
    }

    fn ctrl_key_equivalent(&self, event: &NSEvent) -> bool {
        let raw = events::raw_flags(event);
        if event.r#type() != NSEventType::KeyDown
            || raw & keymap::flags::CONTROL == 0
            || raw & keymap::flags::COMMAND != 0
        {
            return false;
        }
        let policy = app::with_app(|st| st.option_as_alt).unwrap_or_default();
        let (ev, _) = events::key(event, KeyState::Pressed, policy);
        post_input(
            self.window_id(),
            events::os_time(event),
            InputEvent::Key(ev),
        );
        true
    }

    fn ime_insert(&self, text: &str, replacement: NsRange) {
        let id = self.window_id();
        let key = app::with_window(id, |w| {
            w.key.as_mut().filter(|k| !k.consumed).map(|k| {
                k.consumed = true;
                k.event.clone()
            })
        })
        .flatten();
        if let Some(key) = key {
            let had_marked =
                with_target(id, |t, _| t.marked().to_range().is_some()).unwrap_or(false);
            if text_input::classify_insert(key.text.as_deref(), text, had_marked, replacement)
                == InsertOutcome::Key
            {
                let ev = KeyEvent {
                    text: Some(text.to_owned()),
                    ..key
                };
                post_input(id, crate::time::now(), InputEvent::Key(ev));
                return;
            }
        }
        with_target(id, |t, ev| t.insert(text, replacement, ev));
    }

    /// A menu action as the shortcut it stands for (Cmd plus `key`, with Shift for redo). When the
    /// shortcut itself triggered the menu, only the press is sent: its key-up still arrives.
    fn shortcut(&self, key: Key, code: u16, shift: bool) {
        let mtm = self.mtm();
        let from_key = NSApplication::sharedApplication(mtm)
            .currentEvent()
            .is_some_and(|e| e.r#type() == NSEventType::KeyDown);
        let mut modifiers = Modifiers::SUPER;
        if shift {
            modifiers = modifiers.union(Modifiers::SHIFT);
        }
        let states: &[KeyState] = if from_key {
            &[KeyState::Pressed]
        } else {
            &[KeyState::Pressed, KeyState::Released]
        };
        for &state in states {
            let ev = KeyEvent {
                key: Some(key),
                physical: keymap::physical_code(code),
                text: None,
                state,
                repeat: false,
                location: KeyLocation::Standard,
                modifiers,
            };
            post_input(self.window_id(), crate::time::now(), InputEvent::Key(ev));
        }
    }

    /// View points (top-left origin) to screen coordinates (AppKit's, bottom-left origin).
    fn view_rect_to_screen(&self, r: &Rect) -> NSRect {
        let local = layer::cg_rect(r);
        let in_window = self.convertRect_toView(local, None);
        match self.window() {
            Some(w) => w.convertRectToScreen(in_window),
            None => in_window,
        }
    }

    fn screen_point_to_view(&self, p: NSPoint) -> Point {
        let in_window = match self.window() {
            Some(w) => w.convertPointFromScreen(p),
            None => p,
        };
        let v = self.convertPoint_fromView(in_window, None);
        Point::new(v.x as f32, v.y as f32)
    }
}

fn contains(r: &Rect, p: Point) -> bool {
    p.x >= r.min.x && p.x < r.max.x && p.y >= r.min.y && p.y < r.max.y
}

fn ns_range(r: NSRange) -> NsRange {
    NsRange::new(r.location, r.length)
}

fn objc_range(r: NsRange) -> NSRange {
    NSRange::new(r.location, r.length)
}

/// `insertText:` and `setMarkedText:` get an NSString or an NSAttributedString.
fn plain_string(obj: &AnyObject) -> String {
    if let Some(s) = obj.downcast_ref::<NSAttributedString>() {
        s.string().to_string()
    } else if let Some(s) = obj.downcast_ref::<NSString>() {
        s.to_string()
    } else {
        String::new()
    }
}

fn dragged_paths(sender: &AnyObject) -> Vec<PathBuf> {
    // SAFETY: `sender` conforms to NSDraggingInfo, whose draggingPasteboard returns an NSPasteboard.
    let pb: Option<Retained<NSPasteboard>> = unsafe { msg_send![sender, draggingPasteboard] };
    let Some(items) = pb.and_then(|pb| pb.pasteboardItems()) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            // SAFETY: NSPasteboardTypeFileURL is a valid type.
            let s = item.stringForType(unsafe { NSPasteboardTypeFileURL })?;
            let url = NSURL::URLWithString(&s)?;
            url.path().map(|p| PathBuf::from(p.to_string()))
        })
        .collect()
}

/// Lends the window's text-input target to `f` with nothing borrowed (the client is app code that
/// may call back into the platform), puts it back, then delivers the [`ImeEvent`]s `f` produced.
/// `None` when the window has no target, or it is already lent out (a nested IME call).
pub(crate) fn with_target<R>(
    id: WindowId,
    f: impl FnOnce(&mut dyn ImeTarget, &mut Vec<ImeEvent>) -> R,
) -> Option<R> {
    enum Lent {
        Client(Box<dyn TextInputClient>),
        Simple(text_input::SimpleIme),
    }
    let lent = app::with_window(id, |w| match w.client.take() {
        Some(c) => Some(Lent::Client(c)),
        None if w.has_client => None,
        None => w.ime.take().map(Lent::Simple),
    })
    .flatten()?;
    let mut events = Vec::new();
    let (r, lent) = match lent {
        Lent::Client(mut c) => {
            let r = f(&mut ClientTarget(&mut *c), &mut events);
            (r, Lent::Client(c))
        }
        Lent::Simple(mut s) => {
            let r = f(&mut s, &mut events);
            (r, Lent::Simple(s))
        }
    };
    app::with_window(id, |w| match lent {
        // Unless the app replaced or removed the client meanwhile.
        Lent::Client(c) if w.has_client && w.client.is_none() => w.client = Some(c),
        Lent::Client(_) => {}
        Lent::Simple(s) => {
            if w.ime.is_none() {
                w.ime = Some(s);
            }
        }
    });
    app::post_ime(id, events);
    Some(r)
}
