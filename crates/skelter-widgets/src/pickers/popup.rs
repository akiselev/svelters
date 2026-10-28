//! What `date-picker` and `color-picker` share (svelters-egui `picker_popup.rs`, `picker_glyphs.rs`):
//! the drop button's open/close state ([`DropState`]: a click, Enter/Space, F4 or Alt+Down open it;
//! Escape, a press outside, a pick or the footer close it), the closed button's geometry and look,
//! the popup surface's size for a body, its placement ([`place_popup`], the views' anchored
//! placement: below the button, flipped above at the bottom edge) and its overlay-stack entry
//! ([`overlay_spec`]: the popup is a layer of the window's overlay stack, not painted inline), and
//! the Save/Cancel footer of `commit-mode="confirm"` in the platform's button order.

use super::{PickerLook, Ptr};
use crate::table::shim::{self, HAlign, Platform, TextPainter};
use skelter_base::color::Color;
use skelter_base::display::DisplayList;
use skelter_base::event::{Modifiers, PointerEvent};
use skelter_base::geom::{Point, Rect, Size};
use skelter_base::ids::{LayerId, NodeId, PropNodeId};
use skelter_base::key::Key;
use crate::views::overlay::{OverlayClass, OverlaySpec};
use crate::views::placement::{AnchorParams, Placement, place};

/// Why the popup closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseReason {
    /// The button was clicked again.
    Toggle,
    Escape,
    /// A press outside the popup and the button.
    Outside,
    /// A pick (`immediate`) or the footer.
    Done,
    /// The host turned disabled.
    Disabled,
}

/// A drop button's popup state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DropState {
    open: bool,
    /// Opened from the keyboard: the body shows its focus ring at once.
    keyboard: bool,
    /// The primary button went down on the drop button (a click is its release on it).
    pressed: bool,
    /// The pointer is over the drop button (`:hover`).
    hovered: bool,
}

/// What a [`DropState`] input did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropOut {
    Opened { keyboard: bool },
    Closed(CloseReason),
}

impl DropState {
    pub fn is_open(&self) -> bool {
        self.open
    }
    /// Opened from the keyboard (the body's cursor shows its ring).
    pub fn keyboard(&self) -> bool {
        self.keyboard
    }
    pub fn hovered(&self) -> bool {
        self.hovered
    }
    /// The button is pressed (`:active`).
    pub fn pressed(&self) -> bool {
        self.pressed
    }

    pub fn open(&mut self, keyboard: bool) -> Option<DropOut> {
        if self.open {
            return None;
        }
        self.open = true;
        self.keyboard = keyboard;
        Some(DropOut::Opened { keyboard })
    }

    pub fn close(&mut self, why: CloseReason) -> Option<DropOut> {
        if !self.open {
            return None;
        }
        self.open = false;
        self.keyboard = false;
        Some(DropOut::Closed(why))
    }

    /// A pointer event on the closed button, in its local points (`size`: the button's).
    pub fn button_pointer(&mut self, ev: &PointerEvent, size: Size, enabled: bool) -> Option<DropOut> {
        let inside = |p: Point| Rect::from_origin_size(Point::ZERO, size).contains(p);
        match Ptr::of(ev)? {
            Ptr::Move(p) => self.hovered = inside(p),
            Ptr::Leave => {
                self.hovered = false;
                self.pressed = false;
            }
            Ptr::Down(p) => self.pressed = enabled && inside(p),
            Ptr::Up(p) => {
                let click = std::mem::take(&mut self.pressed) && inside(p);
                if click && enabled {
                    return if self.open { self.close(CloseReason::Toggle) } else { self.open(false) };
                }
            }
        }
        None
    }

    /// A key while the button is focused (closed) or the popup holds the keyboard (open).
    /// Returns what happened; `None` leaves the key to the body.
    pub fn key(&mut self, key: Key, m: Modifiers, enabled: bool) -> Option<DropOut> {
        if !enabled {
            return None;
        }
        if self.open {
            return (key == Key::Escape).then(|| self.close(CloseReason::Escape)).flatten();
        }
        let plain = m == Modifiers::default();
        let opens = (plain && matches!(key, Key::Enter | Key::Space | Key::F4)) || (m.contains(Modifiers::ALT) && key == Key::ArrowDown);
        if opens { self.open(true) } else { None }
    }

    /// The keys the drop kinds claim from the command layer (svelters-egui `consume_key`).
    pub fn claims(&self, key: Key, m: Modifiers) -> bool {
        let plain = m == Modifiers::default();
        (plain && matches!(key, Key::Enter | Key::Space | Key::F4)) || (m.contains(Modifiers::ALT) && key == Key::ArrowDown) || (self.open && key == Key::Escape)
    }
}

/// The closed button's metrics (svelters-egui `BUTTON_UA`, `CHEVRON_UA`).
pub const BUTTON_PAD_X: f32 = 6.0;
pub const BUTTON_PAD_Y: f32 = 3.0;
pub const BUTTON_LINE_H: f32 = 18.0;
pub const BUTTON_GAP: f32 = 4.0;
pub const CHEVRON_W: f32 = 16.0;
/// The popup surface's padding and the gap between body and footer (`POPUP_UA`).
pub const POPUP_PAD: f32 = 8.0;
pub const POPUP_GAP: f32 = 8.0;
/// The footer buttons' side padding and the gap between them.
pub const FOOTER_PAD_X: f32 = 10.0;
pub const FOOTER_GAP: f32 = 6.0;

/// The closed button's size around a label of `label` size and a lead item (the colour swatch)
/// `lead` points wide (0 for none).
pub fn button_size(look: &PickerLook, label: Size, lead: f32) -> Size {
    let lead = if lead > 0.0 { lead + BUTTON_GAP } else { 0.0 };
    let w = lead + label.w + BUTTON_GAP + CHEVRON_W + 2.0 * (BUTTON_PAD_X + look.stroke);
    let h = label.h.max(BUTTON_LINE_H) + 2.0 * (BUTTON_PAD_Y + look.stroke);
    Size::new(w, h)
}

/// The closed button's content box, and in it the chevron's rect (at the end) and the label's.
pub fn button_parts(look: &PickerLook, size: Size) -> (Rect, Rect, Rect) {
    let b = look.stroke;
    let content = Rect::new(BUTTON_PAD_X + b, BUTTON_PAD_Y + b, size.w - BUTTON_PAD_X - b, size.h - BUTTON_PAD_Y - b);
    let chevron = Rect::new(content.max.x - CHEVRON_W, content.min.y, content.max.x, content.max.y);
    let label = Rect::new(content.min.x, content.min.y, chevron.min.x - BUTTON_GAP, content.max.y);
    (content, chevron, label)
}

/// The visual state of the closed button.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ButtonState {
    pub open: bool,
    pub focus_visible: bool,
    pub enabled: bool,
    /// The pointer is over it, or it is pressed (the node's `:hover`/`:active`, or the drop's own).
    pub hovered: bool,
    pub pressed: bool,
}

/// Records the closed button's box (R4 field look: `control-bg`, `control-border`, the `focus`
/// border while open, R8's 1 px ring inside while focus-visible) and its chevron; the caller
/// draws the label in [`button_parts`]' label rect. Disabled: 0.4 opacity.
pub fn record_button(list: &mut DisplayList, look: &PickerLook, size: Size, st: ButtonState) {
    let fade = |c: Color| if st.enabled { c } else { c.with_alpha(c.a * 0.4) };
    let r = Rect::from_origin_size(Point::ZERO, size);
    let border = if st.open { look.focus } else { look.control_border };
    shim::frame(list, r, fade(look.control_bg), fade(border), look.stroke, look.radius);
    // hover and press tint the field (`toolbar-hover`, `list-hover`) inside its border
    if st.enabled && (st.hovered || st.pressed) {
        let tint = if st.pressed { look.toolbar_hover } else { look.list_hover };
        shim::fill(list, r.inflate(-look.stroke), tint, (look.radius - look.stroke).max(0.0));
    }
    if st.focus_visible {
        shim::frame(list, r.inflate(-look.stroke), Color::TRANSPARENT, look.focus, 1.0, (look.radius - look.stroke).max(0.0));
    }
    let (_, chevron, _) = button_parts(look, size);
    record_chevron(list, Chevron::Down, chevron, 16.0, fade(look.icon));
}

/// Which way a chevron points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chevron {
    Left,
    Right,
    Down,
}

/// Records a chevron centred in `r` (svelters-egui `picker_glyphs::chevron`'s path, sized from
/// `font_size`), as two line segments.
pub fn record_chevron(list: &mut DisplayList, d: Chevron, r: Rect, font_size: f32, color: Color) {
    let side = font_size.min(r.width()).min(r.height());
    if side <= 0.0 {
        return;
    }
    let c = Point::new((r.min.x + r.max.x) / 2.0, (r.min.y + r.max.y) / 2.0);
    let half = side / 2.0 - side * 0.2;
    let (top, bottom, left, right) = (c.y - half, c.y + half, c.x - half, c.x + half);
    let w = half * 0.5;
    let width = (side / 12.0).max(1.0);
    let pts = match d {
        Chevron::Left => [Point::new(c.x + w, top), Point::new(c.x - w, c.y), Point::new(c.x + w, bottom)],
        Chevron::Right => [Point::new(c.x - w, top), Point::new(c.x + w, c.y), Point::new(c.x - w, bottom)],
        Chevron::Down => [Point::new(left, c.y - w), Point::new(c.x, c.y + w), Point::new(right, c.y - w)],
    };
    shim::line(list, pts[0], pts[1], width, color);
    shim::line(list, pts[1], pts[2], width, color);
}

/// The gap between the button and its popup.
pub const POPUP_OFFSET: f32 = 2.0;

/// Where a `popup` size popup of a `button` size drop button goes (button-local points): below
/// it, start-aligned, flipped above when `bounds` (the window's room, button-local) lacks the room
/// below, and shifted back inside `bounds`.
pub fn place_popup(button: Size, popup: Size, bounds: Rect) -> Rect {
    place_popup_dir(button, popup, bounds, false)
}

/// [`place_popup`] in a direction: `rtl` aligns the popup to the button's right edge.
pub fn place_popup_dir(button: Size, popup: Size, bounds: Rect, rtl: bool) -> Rect {
    use crate::views::placement::{Align, Side};
    place(&AnchorParams {
        anchor: Rect::from_origin_size(Point::ZERO, button),
        size: popup,
        bounds,
        placement: if rtl { Placement { side: Side::Bottom, align: Align::End } } else { Placement::BOTTOM_START },
        gap: POPUP_OFFSET,
        flip: true,
        shift: true,
        ..AnchorParams::default()
    })
    .rect
}

/// The window overlay stack's entry for an open popup of `node` at `popup` (window points) on the
/// compositor layer `layer` moving with `transform`: a light-dismissable [`OverlayClass::Popup`]
/// whose `button` (window points) counts as inside (a press on it toggles instead) and which hands
/// focus back to the node when it closes.
pub fn overlay_spec(node: NodeId, layer: LayerId, transform: PropNodeId, popup: Rect, button: Rect) -> OverlaySpec {
    let mut s = OverlaySpec::new(OverlayClass::Popup, node, layer, transform, popup);
    s.opener = Some(node);
    s.inside = vec![button];
    s
}

/// The popup surface's border box for a body of `body` size (plus the footer row when `footer`).
pub fn popup_size(look: &PickerLook, body: Size, footer: bool) -> Size {
    let f = if footer { POPUP_GAP + look.control_h } else { 0.0 };
    let extra = 2.0 * (POPUP_PAD + look.stroke);
    Size::new(body.w + extra, body.h + f + extra)
}

/// The body's rect in the popup (popup-local points) and the footer row's, for a popup of `size`.
pub fn popup_parts(look: &PickerLook, size: Size, body: Size, footer: bool) -> (Rect, Option<Rect>) {
    let o = POPUP_PAD + look.stroke;
    let content = Rect::new(o, o, size.w - o, size.h - o);
    let body_r = Rect::from_origin_size(content.min, body);
    let foot = footer.then(|| Rect::new(content.min.x, content.max.y - look.control_h, content.max.x, content.max.y));
    (body_r, foot)
}

/// Records the popup surface (`menu-bg`, `menu-border-color`, `menu-radius`).
pub fn record_surface(list: &mut DisplayList, look: &PickerLook, size: Size) {
    shim::frame(list, Rect::from_origin_size(Point::ZERO, size), look.popup_bg, look.popup_border, look.stroke, look.radius_md);
}

/// A footer button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Footer {
    Save,
    Cancel,
}

impl Footer {
    pub fn label(self) -> &'static str {
        match self {
            Footer::Save => "Save",
            Footer::Cancel => "Cancel",
        }
    }
}

/// The footer's buttons left to right in `platform`'s dialog order (QDialogButtonBox: accept
/// before reject on Windows and KDE, after it on GNOME and macOS), right-aligned in the row.
pub fn footer_order(platform: Platform) -> [Footer; 2] {
    match platform {
        Platform::Windows | Platform::Kde => [Footer::Save, Footer::Cancel],
        Platform::Gnome | Platform::MacOs => [Footer::Cancel, Footer::Save],
    }
}

/// The footer's buttons and their rects in `row`.
pub fn footer_layout(text: &mut dyn TextPainter, look: &PickerLook, platform: Platform, row: Rect) -> Vec<(Footer, Rect)> {
    let order = footer_order(platform);
    let style = look.text(look.fg);
    let widths: Vec<f32> = order.iter().map(|f| text.measure(f.label(), &style).w + 2.0 * FOOTER_PAD_X).collect();
    let total: f32 = widths.iter().sum::<f32>() + FOOTER_GAP;
    let mut x = row.max.x - total;
    order
        .iter()
        .zip(widths)
        .map(|(f, w)| {
            let r = Rect::new(x, row.min.y, x + w, row.max.y);
            x += w + FOOTER_GAP;
            (*f, r)
        })
        .collect()
}

/// The footer's pointer state (hover and press per button).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FooterState {
    hovered: Option<Footer>,
    pressed: Option<Footer>,
}

impl FooterState {
    /// A pointer event in popup-local points over the footer laid out as `buttons`; a click returns
    /// its button.
    pub fn pointer(&mut self, ev: &PointerEvent, buttons: &[(Footer, Rect)]) -> Option<Footer> {
        let at = |p: Point| buttons.iter().find(|(_, r)| r.contains(p)).map(|b| b.0);
        match Ptr::of(ev)? {
            Ptr::Move(p) => self.hovered = at(p),
            Ptr::Leave => *self = FooterState::default(),
            Ptr::Down(p) => self.pressed = at(p),
            Ptr::Up(p) => {
                let hit = self.pressed.take();
                if hit.is_some() && hit == at(p) {
                    return hit;
                }
            }
        }
        None
    }

    /// Records the footer: Save in `accent` (R1's primary), Cancel as a secondary button.
    pub fn record(&self, list: &mut DisplayList, text: &mut dyn TextPainter, look: &PickerLook, buttons: &[(Footer, Rect)]) {
        for (f, r) in buttons {
            let hover = self.hovered == Some(*f);
            let (bg, fg, border) = match f {
                Footer::Save => (if hover { look.accent_hover } else { look.accent }, look.accent_fg, look.button_border),
                Footer::Cancel => (if hover { look.button_bg_hover } else { look.button_bg }, look.button_fg, look.control_border),
            };
            shim::frame(list, *r, bg, border, if border.a > 0.0 { look.stroke } else { 0.0 }, look.radius);
            text.draw(list, f.label(), &look.text(fg), *r, HAlign::Center, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pickers::test_util::ptr;
    use skelter_base::event::PointerPhase;

    #[test]
    fn the_button_opens_on_click_and_keys_and_escape_closes() {
        let mut d = DropState::default();
        let size = Size::new(100.0, 24.0);
        assert_eq!(d.button_pointer(&ptr(PointerPhase::Down, 10.0, 10.0), size, true), None);
        assert_eq!(d.button_pointer(&ptr(PointerPhase::Up, 12.0, 10.0), size, true), Some(DropOut::Opened { keyboard: false }));
        assert!(d.is_open());
        assert_eq!(d.key(Key::Escape, Modifiers::default(), true), Some(DropOut::Closed(CloseReason::Escape)));
        assert_eq!(d.key(Key::F4, Modifiers::default(), true), Some(DropOut::Opened { keyboard: true }));
        assert!(d.keyboard());
        d.close(CloseReason::Outside);
        assert_eq!(d.key(Key::ArrowDown, Modifiers::ALT, true), Some(DropOut::Opened { keyboard: true }));
        d.close(CloseReason::Done);
        assert_eq!(d.key(Key::ArrowDown, Modifiers::default(), true), None);
        // a press that leaves the button is no click
        d.button_pointer(&ptr(PointerPhase::Down, 10.0, 10.0), size, true);
        assert_eq!(d.button_pointer(&ptr(PointerPhase::Up, 200.0, 10.0), size, true), None);
        assert!(!d.is_open());
        // disabled: nothing opens
        d.button_pointer(&ptr(PointerPhase::Down, 10.0, 10.0), size, false);
        assert_eq!(d.button_pointer(&ptr(PointerPhase::Up, 10.0, 10.0), size, false), None);
    }

    #[test]
    fn the_footer_follows_the_platform_order() {
        assert_eq!(footer_order(Platform::Windows), [Footer::Save, Footer::Cancel]);
        assert_eq!(footer_order(Platform::MacOs), [Footer::Cancel, Footer::Save]);
        let mut t = crate::table::shim::testing::FixedText::default();
        let look = PickerLook::default();
        let row = Rect::new(0.0, 0.0, 200.0, 26.0);
        let b = footer_layout(&mut t, &look, Platform::Gnome, row);
        assert_eq!(b[1].0, Footer::Save);
        assert_eq!(b[1].1.max.x, 200.0, "right-aligned");
        assert!(b[0].1.max.x + FOOTER_GAP <= b[1].1.min.x + 0.001);
        let mut f = FooterState::default();
        let c = b[1].1.min;
        f.pointer(&ptr(PointerPhase::Down, c.x + 1.0, 5.0), &b);
        assert_eq!(f.pointer(&ptr(PointerPhase::Up, c.x + 2.0, 5.0), &b), Some(Footer::Save));
    }
}
