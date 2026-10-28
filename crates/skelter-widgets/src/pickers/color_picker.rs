//! `color-picker` (svelters-egui `widgets/color_picker.rs`; WIDGET-PARITY §2.20 `QColorDialog`): a
//! drop button showing the colour (a swatch, a checkerboard under alpha) with a chevron, opening
//! the colour editor ([`ColorBody`]) in a popup. `commit-mode`: `live` (the default: each edit
//! changes `value`; closing the popup after edits commits) or `confirm` (edits are pending in the
//! body until Save; Cancel or Escape drops them).

use super::color::{rgba_color, rgba_or_black};
use super::color_editor::{ColorBody, ColorCfg, ColorGeom, ColorOut, MAX_CUSTOM, SQUARE};
use super::popup::{self, ButtonState, CloseReason, DropOut, DropState, Footer, FooterState};
use super::PickerLook;
use crate::table::shim::{self, Emit, Platform, TextPainter, Value};
use skelter_base::display::DisplayList;
use skelter_base::event::{Modifiers, PointerEvent};
use skelter_base::geom::{Point, Rect, Size};
use skelter_base::key::Key;

/// The swatch on the closed button (`::part(swatch)`).
pub const SWATCH: Size = Size::new(28.0, 16.0);

/// The `color-picker` kind.
#[derive(Clone, Debug, PartialEq)]
pub struct ColorPicker {
    /// The colour, normalized hex.
    pub value: String,
    pub cfg: ColorCfg,
    /// `commit-mode="live"`.
    pub live: bool,
    pub body: ColorBody,
    pub drop: DropState,
    /// `live`: an edit happened since the popup opened (closing commits it).
    pub edited: bool,
    pub footer: FooterState,
}

impl Default for ColorPicker {
    fn default() -> Self {
        ColorPicker { value: "#000000".into(), cfg: ColorCfg::default(), live: true, body: ColorBody::default(), drop: DropState::default(), edited: false, footer: FooterState::default() }
    }
}

impl ColorPicker {
    pub fn rgba(&self) -> [u8; 4] {
        rgba_or_black(&self.value)
    }

    /// The closed button's size.
    pub fn size(&self, look: &PickerLook) -> Size {
        popup::button_size(look, Size::new(0.0, SWATCH.h), SWATCH.w - popup::BUTTON_GAP)
    }

    fn body_size(&self, look: &PickerLook) -> Size {
        ColorGeom::new(look, &self.cfg, Point::ZERO, SQUARE.w).size(Point::ZERO)
    }

    /// The popup's border box.
    pub fn popup_size(&self, look: &PickerLook) -> Size {
        popup::popup_size(look, self.body_size(look), !self.live)
    }

    /// Opening resets the body; closing after live edits is their commit.
    fn after_drop(&mut self, out: Option<DropOut>) -> Vec<Emit> {
        match out {
            Some(DropOut::Opened { .. }) => {
                self.body = ColorBody::default();
                self.edited = false;
                Vec::new()
            }
            Some(DropOut::Closed(_)) if self.live && std::mem::take(&mut self.edited) => vec![Emit::new("commit", vec![Value::Str(self.value.clone())])],
            Some(DropOut::Closed(_)) => {
                self.body = ColorBody::default();
                Vec::new()
            }
            None => Vec::new(),
        }
    }

    /// Applies the body's outcomes: `live` writes them to `value` (`changed`, and `commit` at each
    /// commit point); `confirm` keeps them in the body until Save. The add cell appends to
    /// `custom-colors` in both.
    fn body_outs(&mut self, outs: Vec<ColorOut>) -> Vec<Emit> {
        let mut e = Vec::new();
        for o in outs {
            match o {
                ColorOut::Input(h) if self.live => {
                    let hex = ColorBody::hex_of(h, &self.cfg);
                    self.body.edited(h);
                    if hex != self.value {
                        self.value = hex.clone();
                        self.edited = true;
                        e.push(Emit::new("changed", vec![Value::Str(hex)]));
                    }
                }
                ColorOut::Commit if self.live => e.push(Emit::new("commit", vec![Value::Str(self.value.clone())])),
                ColorOut::Input(_) | ColorOut::Commit => {}
                ColorOut::AddCustom => {
                    if self.cfg.custom.len() < MAX_CUSTOM {
                        let h = self.body.shown(self.rgba());
                        self.cfg.custom.push(ColorBody::hex_of(h, &self.cfg));
                        e.push(Emit::new("custom-colors-changed", vec![Value::List(self.cfg.custom.iter().cloned().map(Value::Str).collect())]));
                    }
                }
            }
        }
        e
    }

    /// Save (`confirm`): the body's colour becomes the value.
    fn save(&mut self) -> Vec<Emit> {
        let h = self.body.shown(self.rgba());
        let hex = ColorBody::hex_of(h, &self.cfg);
        let mut e = Vec::new();
        self.drop.close(CloseReason::Done);
        if hex != self.value {
            self.value = hex.clone();
            e.push(Emit::new("changed", vec![Value::Str(hex)]));
        }
        e.push(Emit::new("commit", vec![Value::Str(self.value.clone())]));
        self.body = ColorBody::default();
        e
    }

    /// Opens or closes the editor from the `open` prop (no keyboard focus moves into it); closing
    /// after live edits commits them.
    pub fn set_open(&mut self, open: bool) -> Vec<Emit> {
        let out = if open { self.drop.open(false) } else { self.drop.close(CloseReason::Done) };
        self.after_drop(out)
    }

    pub fn button_pointer(&mut self, ev: &PointerEvent, size: Size, enabled: bool) -> Vec<Emit> {
        let out = self.drop.button_pointer(ev, size, enabled);
        self.after_drop(out)
    }

    pub fn key(&mut self, key: Key, m: Modifiers, enabled: bool) -> Vec<Emit> {
        if !enabled {
            let out = self.drop.close(CloseReason::Disabled);
            return self.after_drop(out);
        }
        if !self.drop.is_open() || key == Key::Escape {
            let out = self.drop.key(key, m, enabled);
            return self.after_drop(out);
        }
        let v = self.rgba();
        let outs = self.body.key(key, m, v, enabled);
        self.body_outs(outs)
    }

    /// The popup was dismissed from outside.
    pub fn dismiss(&mut self) -> Vec<Emit> {
        let out = self.drop.close(CloseReason::Outside);
        self.after_drop(out)
    }

    /// A pointer event in popup-local points.
    pub fn popup_pointer(&mut self, ev: &PointerEvent, text: &mut dyn TextPainter, look: &PickerLook, platform: Platform, enabled: bool) -> Vec<Emit> {
        if !self.drop.is_open() {
            return Vec::new();
        }
        let size = self.popup_size(look);
        let (body_r, foot) = popup::popup_parts(look, size, self.body_size(look), !self.live);
        let g = ColorGeom::new(look, &self.cfg, body_r.min, SQUARE.w);
        let v = self.rgba();
        let outs = self.body.pointer(ev, &g, &self.cfg, v, enabled);
        let mut e = self.body_outs(outs);
        if let Some(row) = foot {
            let buttons = popup::footer_layout(text, look, platform, row);
            match self.footer.pointer(ev, &buttons) {
                Some(Footer::Save) => e.extend(self.save()),
                Some(Footer::Cancel) => {
                    let out = self.drop.close(CloseReason::Done);
                    e.extend(self.after_drop(out));
                }
                None => {}
            }
        }
        e
    }

    /// Records the closed button: the swatch (a checkerboard under alpha) and the chevron.
    pub fn record_button(&self, list: &mut DisplayList, look: &PickerLook, size: Size, focus_visible: bool, enabled: bool) {
        self.record_button_in(list, look, size, ButtonState { open: self.drop.is_open(), focus_visible, enabled, hovered: self.drop.hovered(), pressed: self.drop.pressed() });
    }

    /// [`ColorPicker::record_button`] with the node's hover and press too.
    pub fn record_button_in(&self, list: &mut DisplayList, look: &PickerLook, size: Size, st: ButtonState) {
        popup::record_button(list, look, size, st);
        let (content, _, _) = popup::button_parts(look, size);
        let cy = (content.min.y + content.max.y) / 2.0;
        let sw = Rect::new(content.min.x, cy - SWATCH.h / 2.0, content.min.x + SWATCH.w.min(content.width()), cy + SWATCH.h / 2.0);
        let rgba = self.rgba();
        if rgba[3] < 255 {
            super::color_editor::record_checker(list, sw);
        }
        shim::frame(list, sw, rgba_color(rgba), look.control_border, look.stroke, 2.0);
    }

    /// Records the popup (popup-local points): surface, editor, footer.
    pub fn record_popup(&self, list: &mut DisplayList, text: &mut dyn TextPainter, look: &PickerLook, platform: Platform, focus_visible: bool) {
        let size = self.popup_size(look);
        popup::record_surface(list, look, size);
        let (body_r, foot) = popup::popup_parts(look, size, self.body_size(look), !self.live);
        let g = ColorGeom::new(look, &self.cfg, body_r.min, SQUARE.w);
        self.body.record(list, text, look, &g, &self.cfg, self.rgba(), true, focus_visible);
        if let Some(row) = foot {
            let buttons = popup::footer_layout(text, look, platform, row);
            self.footer.record(list, text, look, &buttons);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pickers::test_util::ptr;
    use crate::table::shim::testing::FixedText;
    use skelter_base::event::PointerPhase;

    fn square_centre(p: &ColorPicker, look: &PickerLook) -> Point {
        let size = p.popup_size(look);
        let (body_r, _) = popup::popup_parts(look, size, p.body_size(look), !p.live);
        let g = ColorGeom::new(look, &p.cfg, body_r.min, SQUARE.w);
        Point::new(g.square.min.x + g.square.width() / 2.0, g.square.min.y + g.square.height() / 2.0)
    }

    #[test]
    fn live_edits_change_the_value_and_closing_commits() {
        let look = PickerLook::default();
        let mut t = FixedText::default();
        let mut p = ColorPicker { value: "#ff0000".into(), ..ColorPicker::default() };
        p.key(Key::F4, Modifiers::default(), true);
        let c = square_centre(&p, &look);
        let e = p.popup_pointer(&ptr(PointerPhase::Down, c.x, c.y), &mut t, &look, Platform::Gnome, true);
        assert_eq!(e, vec![Emit::new("changed", vec![Value::Str("#804040".into())])]);
        p.popup_pointer(&ptr(PointerPhase::Up, c.x, c.y), &mut t, &look, Platform::Gnome, true);
        let e = p.key(Key::Escape, Modifiers::default(), true);
        assert_eq!(e, vec![Emit::new("commit", vec![Value::Str("#804040".into())])]);
    }

    #[test]
    fn confirm_edits_wait_for_save() {
        let look = PickerLook::default();
        let mut t = FixedText::default();
        let mut p = ColorPicker { value: "#ff0000".into(), live: false, ..ColorPicker::default() };
        p.key(Key::Enter, Modifiers::default(), true);
        let c = square_centre(&p, &look);
        assert!(p.popup_pointer(&ptr(PointerPhase::Down, c.x, c.y), &mut t, &look, Platform::Gnome, true).is_empty());
        p.popup_pointer(&ptr(PointerPhase::Up, c.x, c.y), &mut t, &look, Platform::Gnome, true);
        assert_eq!(p.value, "#ff0000");
        let size = p.popup_size(&look);
        let (_, foot) = popup::popup_parts(&look, size, p.body_size(&look), true);
        let buttons = popup::footer_layout(&mut t, &look, Platform::Gnome, foot.unwrap());
        let save = buttons.iter().find(|b| b.0 == Footer::Save).unwrap().1;
        p.popup_pointer(&ptr(PointerPhase::Down, save.min.x + 1.0, save.min.y + 1.0), &mut t, &look, Platform::Gnome, true);
        let e = p.popup_pointer(&ptr(PointerPhase::Up, save.min.x + 1.0, save.min.y + 1.0), &mut t, &look, Platform::Gnome, true);
        assert_eq!(e, vec![Emit::new("changed", vec![Value::Str("#804040".into())]), Emit::new("commit", vec![Value::Str("#804040".into())])]);
        assert!(!p.drop.is_open());
    }
}
