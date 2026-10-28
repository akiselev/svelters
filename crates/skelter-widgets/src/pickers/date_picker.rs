//! `date-picker` (svelters-egui `widgets/date_picker.rs`; WIDGET-PARITY §2.19 `QDateEdit` with
//! `calendarPopup`): a drop button showing the date in `format` with a chevron, opening the calendar
//! ([`CalBody`]) in a popup. `commit-mode`: `confirm` (the default: a pick is pending until Save;
//! Enter saves at once, Cancel or Escape drops it) or `immediate` (Qt: a pick commits and closes).
//!
//! Keys, closed: Enter, Space, F4 or Alt+Down open it. Open: the calendar takes the keyboard,
//! Escape closes it (focus returns to the button: the focus model's part).

use super::calendar::{CalBody, CalCfg, CalGeom, CalOut};
use super::date::Date;
use super::popup::{self, ButtonState, CloseReason, DropOut, DropState, Footer, FooterState};
use super::PickerLook;
use crate::table::shim::{Emit, HAlign, Platform, TextPainter, Value};
use skelter_base::display::DisplayList;
use skelter_base::event::{Modifiers, PointerEvent};
use skelter_base::geom::{Point, Size};
use skelter_base::key::Key;

/// `commit-mode` values of `date-picker`.
pub const DATE_COMMIT_MODE: &[&str] = &["confirm", "immediate"];

/// The `date-picker` kind.
#[derive(Clone, Debug, PartialEq)]
pub struct DatePicker {
    /// The date; none for `""` (the button shows today).
    pub value: Option<Date>,
    /// `strftime` format of the button's text.
    pub format: String,
    pub cfg: CalCfg,
    /// `commit-mode="confirm"`.
    pub confirm: bool,
    pub body: CalBody,
    pub drop: DropState,
    /// The day picked in `confirm` mode, until Save.
    pub pending: Option<Date>,
    pub footer: FooterState,
}

impl Default for DatePicker {
    fn default() -> Self {
        DatePicker { value: None, format: "%Y-%m-%d".into(), cfg: CalCfg::default(), confirm: true, body: CalBody::default(), drop: DropState::default(), pending: None, footer: FooterState::default() }
    }
}

impl DatePicker {
    /// The button's text: the value in `format` (ISO when `format` is bad); `""` for none, where
    /// the button shows [`DatePicker::placeholder`]. `_today` is unused (an empty picker shows no date).
    pub fn text(&self, _today: Date) -> String {
        self.value.map(|d| d.strftime(&self.format).unwrap_or_else(|_| d.to_string())).unwrap_or_default()
    }

    /// What an empty button shows, dimmed: `format` with its fields spelled out (`%Y-%m-%d` is
    /// `yyyy-mm-dd`).
    pub fn placeholder(&self) -> String {
        let mut out = String::new();
        let mut it = self.format.chars();
        while let Some(c) = it.next() {
            if c != '%' {
                out.push(c);
                continue;
            }
            out.push_str(match it.next() {
                Some('Y') => "yyyy",
                Some('y') => "yy",
                Some('m') => "mm",
                Some('d') | Some('e') => "dd",
                Some('B') | Some('b') => "month",
                Some('A') | Some('a') => "day",
                Some('%') => "%",
                _ => "",
            });
        }
        out
    }

    /// The closed button's size (room for the placeholder too, so picking a date does not resize it).
    pub fn size(&self, text: &mut dyn TextPainter, look: &PickerLook, today: Date) -> Size {
        let style = look.text(look.control_fg);
        let label = text.measure(&self.text(today), &style);
        let hint = text.measure(&self.placeholder(), &style);
        popup::button_size(look, Size::new(label.w.max(hint.w), label.h.max(hint.h)), 0.0)
    }

    /// Opens or closes the calendar from the `open` prop (no keyboard focus moves into it).
    pub fn set_open(&mut self, open: bool) {
        if open {
            let out = self.drop.open(false);
            let _ = self.opened(out);
        } else {
            self.drop.close(CloseReason::Done);
            self.pending = None;
        }
    }

    fn body_size(&self, look: &PickerLook) -> Size {
        CalGeom::new(look, &self.cfg, Point::ZERO).size()
    }

    /// The popup's border box (the views lane places it below the button).
    pub fn popup_size(&self, look: &PickerLook) -> Size {
        popup::popup_size(look, self.body_size(look), self.confirm)
    }

    fn opened(&mut self, out: Option<DropOut>) -> Vec<Emit> {
        if let Some(DropOut::Opened { .. }) = out {
            self.body.reset(self.value);
            self.pending = None;
        }
        Vec::new()
    }

    /// The user's pick: `changed(value)` when it differs, then `commit(value)`.
    fn apply(&mut self, d: Date) -> Vec<Emit> {
        let mut e = Vec::new();
        if self.value != Some(d) {
            self.value = Some(d);
            e.push(Emit::new("changed", vec![Value::Str(d.to_string())]));
        }
        e.push(Emit::new("commit", vec![Value::Str(d.to_string())]));
        e
    }

    fn body_outs(&mut self, outs: Vec<CalOut>, enter: bool) -> Vec<Emit> {
        let mut e = Vec::new();
        for o in outs {
            match o {
                CalOut::Pick(d) if !self.confirm || enter => {
                    self.drop.close(CloseReason::Done);
                    e.extend(self.apply(d));
                }
                CalOut::Pick(d) => self.pending = Some(d),
                CalOut::Page(y, m) => e.push(Emit::new("page-changed", vec![Value::I64(y as i64), Value::I64(m as i64)])),
            }
        }
        e
    }

    /// A pointer event on the closed button (button-local points).
    pub fn button_pointer(&mut self, ev: &PointerEvent, size: Size, enabled: bool) -> Vec<Emit> {
        let out = self.drop.button_pointer(ev, size, enabled);
        self.opened(out)
    }

    /// A key while the button is focused or the popup holds the keyboard.
    pub fn key(&mut self, key: Key, m: Modifiers, today: Date, enabled: bool) -> Vec<Emit> {
        if !enabled {
            self.drop.close(CloseReason::Disabled);
            return Vec::new();
        }
        if !self.drop.is_open() || key == Key::Escape {
            let out = self.drop.key(key, m, enabled);
            return self.opened(out);
        }
        let shown = if self.confirm { self.pending.or(self.value) } else { self.value };
        let outs = self.body.key(key, m, &self.cfg, shown, today, enabled);
        self.body_outs(outs, key == Key::Enter)
    }

    /// The popup was dismissed from outside (a press outside it and the button).
    pub fn dismiss(&mut self) {
        self.drop.close(CloseReason::Outside);
    }

    /// A pointer event in popup-local points (the popup of [`DatePicker::popup_size`]).
    pub fn popup_pointer(&mut self, ev: &PointerEvent, text: &mut dyn TextPainter, look: &PickerLook, platform: Platform, today: Date, enabled: bool) -> Vec<Emit> {
        if !self.drop.is_open() {
            return Vec::new();
        }
        let size = self.popup_size(look);
        let (body_r, foot) = popup::popup_parts(look, size, self.body_size(look), self.confirm);
        let g = CalGeom::new(look, &self.cfg, body_r.min);
        let shown = if self.confirm { self.pending.or(self.value) } else { self.value };
        let outs = self.body.pointer(ev, &g, &self.cfg, shown, today, enabled);
        let mut e = self.body_outs(outs, false);
        if let Some(row) = foot.filter(|_| self.drop.is_open()) {
            let buttons = popup::footer_layout(text, look, platform, row);
            match self.footer.pointer(ev, &buttons) {
                Some(Footer::Save) => {
                    self.drop.close(CloseReason::Done);
                    if let Some(d) = self.pending.take() {
                        e.extend(self.apply(d));
                    }
                }
                Some(Footer::Cancel) => {
                    self.drop.close(CloseReason::Done);
                    self.pending = None;
                }
                None => {}
            }
        }
        e
    }

    /// Records the closed button.
    #[allow(clippy::too_many_arguments)]
    pub fn record_button(&self, list: &mut DisplayList, text: &mut dyn TextPainter, look: &PickerLook, size: Size, today: Date, focus_visible: bool, enabled: bool) {
        self.record_button_in(list, text, look, size, today, ButtonState { open: self.drop.is_open(), focus_visible, enabled, hovered: self.drop.hovered(), pressed: self.drop.pressed() });
    }

    /// [`DatePicker::record_button`] with the node's hover and press too.
    pub fn record_button_in(&self, list: &mut DisplayList, text: &mut dyn TextPainter, look: &PickerLook, size: Size, today: Date, st: ButtonState) {
        let enabled = st.enabled;
        popup::record_button(list, look, size, st);
        let (_, _, label) = popup::button_parts(look, size);
        let fg = if enabled { look.control_fg } else { look.control_fg.with_alpha(look.control_fg.a * 0.4) };
        match self.value {
            Some(_) => text.draw(list, &self.text(today), &look.text(fg), label, HAlign::Start, true),
            // empty: the format's fields, in the muted (placeholder) colour
            None => text.draw(list, &self.placeholder(), &look.text(if enabled { look.muted } else { fg }), label, HAlign::Start, true),
        }
    }

    /// Records the popup (popup-local points): surface, calendar, footer.
    pub fn record_popup(&self, list: &mut DisplayList, text: &mut dyn TextPainter, look: &PickerLook, platform: Platform, today: Date) {
        let size = self.popup_size(look);
        popup::record_surface(list, look, size);
        let (body_r, foot) = popup::popup_parts(look, size, self.body_size(look), self.confirm);
        let g = CalGeom::new(look, &self.cfg, body_r.min);
        let shown = if self.confirm { self.pending.or(self.value) } else { self.value };
        self.body.record(list, text, look, &g, &self.cfg, shown, today, true, true);
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

    fn d(s: &str) -> Date {
        Date::parse_iso(s).unwrap()
    }

    #[test]
    fn confirm_mode_holds_a_pick_until_save() {
        let look = PickerLook::default();
        let today = d("2026-10-22");
        let mut p = DatePicker { value: Some(d("2026-10-01")), ..DatePicker::default() };
        assert!(p.key(Key::F4, Modifiers::default(), today, true).is_empty());
        assert!(p.drop.is_open());
        assert!(p.key(Key::ArrowRight, Modifiers::default(), today, true).is_empty());
        // a click on a day is pending
        let size = p.popup_size(&look);
        let (body_r, foot) = popup::popup_parts(&look, size, p.body_size(&look), true);
        let g = CalGeom::new(&look, &p.cfg, body_r.min);
        let r = g.cell_rect(24); // 2026-10-22
        let mut t = FixedText::default();
        p.popup_pointer(&ptr(PointerPhase::Down, r.min.x + 2.0, r.min.y + 2.0), &mut t, &look, Platform::Windows, today, true);
        assert!(p.popup_pointer(&ptr(PointerPhase::Up, r.min.x + 2.0, r.min.y + 2.0), &mut t, &look, Platform::Windows, today, true).is_empty());
        assert_eq!(p.pending, Some(today));
        assert_eq!(p.value, Some(d("2026-10-01")));
        let buttons = popup::footer_layout(&mut t, &look, Platform::Windows, foot.unwrap());
        let save = buttons.iter().find(|b| b.0 == Footer::Save).unwrap().1;
        p.popup_pointer(&ptr(PointerPhase::Down, save.min.x + 1.0, save.min.y + 1.0), &mut t, &look, Platform::Windows, today, true);
        let e = p.popup_pointer(&ptr(PointerPhase::Up, save.min.x + 1.0, save.min.y + 1.0), &mut t, &look, Platform::Windows, today, true);
        assert_eq!(e, vec![Emit::new("changed", vec![Value::Str("2026-10-22".into())]), Emit::new("commit", vec![Value::Str("2026-10-22".into())])]);
        assert!(!p.drop.is_open());
    }

    #[test]
    fn enter_saves_and_immediate_mode_commits_a_pick() {
        let today = d("2026-10-22");
        let mut p = DatePicker { value: Some(d("2026-10-01")), ..DatePicker::default() };
        p.key(Key::Enter, Modifiers::default(), today, true);
        assert!(p.drop.is_open());
        p.key(Key::ArrowDown, Modifiers::default(), today, true);
        let e = p.key(Key::Enter, Modifiers::default(), today, true);
        assert_eq!(e.last(), Some(&Emit::new("commit", vec![Value::Str("2026-10-08".into())])));
        assert!(!p.drop.is_open());
        let mut q = DatePicker { confirm: false, ..DatePicker::default() };
        q.key(Key::Space, Modifiers::default(), today, true);
        q.key(Key::Escape, Modifiers::default(), today, true);
        assert!(!q.drop.is_open());
        assert_eq!(q.value, None, "Escape drops");
    }

    #[test]
    fn the_button_text_uses_the_format() {
        let p = DatePicker { value: Some(d("2026-10-02")), format: "%d %b %Y".into(), ..DatePicker::default() };
        assert_eq!(p.text(d("2026-10-22")), "02 Oct 2026");
        let bad = DatePicker { value: Some(d("2026-10-22")), format: "%Q".into(), ..DatePicker::default() };
        assert_eq!(bad.text(d("2026-10-22")), "2026-10-22", "a bad format shows ISO");
        // empty: no date (not today), the format's fields as the placeholder
        let empty = DatePicker { format: "%d %B %Y".into(), ..DatePicker::default() };
        assert_eq!(empty.text(d("2026-10-22")), "");
        assert_eq!(empty.placeholder(), "dd month yyyy");
        assert_eq!(DatePicker::default().placeholder(), "yyyy-mm-dd");
    }

    #[test]
    fn open_opens_and_closes_the_calendar() {
        let mut p = DatePicker::default();
        p.set_open(true);
        assert!(p.drop.is_open());
        p.set_open(false);
        assert!(!p.drop.is_open());
    }
}
