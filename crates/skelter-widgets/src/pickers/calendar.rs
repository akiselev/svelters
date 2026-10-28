//! `calendar` (svelters-egui `widgets/calendar.rs`; WIDGET-PARITY §2.19 `QCalendarWidget`) and the
//! calendar body `date-picker` shows in its popup ([`CalBody`]).
//!
//! Layout: a header with prev/next and the month and year (`navigation`); a weekday row
//! (`weekday-format`); a 6×7 day grid starting on `first-day`, with ISO week numbers in front
//! (`calendar-week`). Day states: hover, selected (the value), current (the keyboard cursor; with
//! the host's focus-visible it carries the ring), disabled (outside `min`/`max`); day names
//! `today`, `weekend`, `other-month` and the names `day-parts` gives a date.
//!
//! Keys (the host is one tab stop; the grid roves): arrows move a day or a week, Home/End the
//! week's start/end, PageUp/PageDown a month (Shift: a year), Enter/Space pick the cursor's day.
//! The pointer: a day click picks it, prev/next page. `read-only` shows and roves but never picks.

use super::date::{Date, MONTHS, WEEKDAYS};
use super::popup::{self, Chevron};
use super::{PickerLook, Ptr};
use crate::table::shim::{self, Emit, HAlign, TextPainter, Value};
use skelter_base::color::Color;
use skelter_base::display::DisplayList;
use skelter_base::event::{Modifiers, PointerEvent};
use skelter_base::geom::{Point, Rect, Size};
use skelter_base::key::Key;

/// `first-day` values (`auto`: the locale's; Monday until locale data lands, as svelters-egui).
pub const FIRST_DAY: &[&str] = &["auto", "monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"];
/// `weekday-format` values (Qt's `horizontalHeaderFormat`).
pub const WEEKDAY_FORMAT: &[&str] = &["narrow", "short", "long", "none"];

/// The day cell (`::part(day)` width and height), the weekday row's height, the gap below the header.
pub const CELL: Size = Size::new(28.0, 24.0);
pub const WEEKDAY_H: f32 = 20.0;
pub const GAP: f32 = 2.0;

/// What the calendar is configured with (the props `calendar` and `date-picker` share).
#[derive(Clone, Debug, PartialEq)]
pub struct CalCfg {
    /// Monday = 0.
    pub first_day: u8,
    /// One of [`WEEKDAY_FORMAT`].
    pub weekday_format: &'static str,
    pub week_numbers: bool,
    pub navigation: bool,
    pub min: Option<Date>,
    pub max: Option<Date>,
    pub read_only: bool,
    /// `"YYYY-MM-DD name…"` entries.
    pub day_parts: Vec<String>,
}

impl Default for CalCfg {
    fn default() -> Self {
        CalCfg { first_day: 0, weekday_format: "short", week_numbers: true, navigation: true, min: None, max: None, read_only: false, day_parts: Vec::new() }
    }
}

impl CalCfg {
    /// `first-day` as a Monday-zero offset (`auto` and unknown values: Monday).
    pub fn first_day_of(v: &str) -> u8 {
        FIRST_DAY.iter().position(|x| *x == v).filter(|&i| i > 0).map_or(0, |i| i as u8 - 1)
    }

    /// `weekday-format` (unknown values: `short`).
    pub fn weekday_format_of(v: &str) -> &'static str {
        WEEKDAY_FORMAT.iter().copied().find(|x| *x == v).unwrap_or("short")
    }

    pub fn in_range(&self, d: Date) -> bool {
        self.min.is_none_or(|m| d >= m) && self.max.is_none_or(|m| d <= m)
    }

    pub fn clamp(&self, d: Date) -> Date {
        let d = self.min.map_or(d, |m| d.max(m));
        self.max.map_or(d, |m| d.min(m))
    }

    /// Whether any day of `d`'s month is within `min`/`max`.
    pub fn in_range_month(&self, d: Date) -> bool {
        self.min.is_none_or(|m| d.last_of_month() >= m) && self.max.is_none_or(|m| d.first_of_month() <= m)
    }

    /// The extra part names `day-parts` gives `d`.
    pub fn names_of(&self, d: Date) -> Vec<&str> {
        let iso = d.to_string();
        self.day_parts.iter().filter_map(|e| e.strip_prefix(iso.as_str()).filter(|r| r.starts_with(' '))).flat_map(str::split_whitespace).collect()
    }

    /// The weekday header's text for Monday-zero `wd`.
    pub fn weekday_label(&self, wd: usize) -> &'static str {
        let name = WEEKDAYS[wd % 7];
        match self.weekday_format {
            "narrow" => &name[..1],
            "long" => name,
            "none" => "",
            _ => &name[..2],
        }
    }
}

/// The first cell of month `(y, m)`'s 6×7 grid.
pub fn grid_start(y: i32, m: u8, first_day: u8) -> Date {
    let first = Date::new(y, m, 1).unwrap_or_else(|| Date::new(1970, 1, 1).expect("valid"));
    let back = (first.weekday() as i64 - first_day as i64).rem_euclid(7);
    first.add_days(-back)
}

/// The 42 days of month `(y, m)`'s grid, row by row.
pub fn grid_days(y: i32, m: u8, first_day: u8) -> [Date; 42] {
    let start = grid_start(y, m, first_day);
    std::array::from_fn(|k| start.add_days(k as i64))
}

/// What the calendar did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalOut {
    /// The user picked a day (click, Enter/Space).
    Pick(Date),
    /// The user moved the calendar to another month.
    Page(i32, u8),
}

/// A part of the body under a point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalHit {
    Prev,
    Next,
    /// A grid cell, 0..42.
    Day(usize),
}

/// The rects of one layout of the body (body-local points).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CalGeom {
    pub header: Rect,
    pub prev: Rect,
    pub next: Rect,
    pub title: Rect,
    pub weekdays: Rect,
    pub grid: Rect,
    pub cell: Size,
    pub week_col: f32,
}

impl CalGeom {
    pub fn new(look: &PickerLook, cfg: &CalCfg, origin: Point) -> CalGeom {
        let week_col = if cfg.week_numbers { CELL.w } else { 0.0 };
        let wd_h = if cfg.weekday_format == "none" { 0.0 } else { WEEKDAY_H };
        let header_h = if cfg.navigation { look.row_h } else { 0.0 };
        let w = week_col + 7.0 * CELL.w;
        let header = Rect::from_origin_size(origin, Size::new(w, header_h));
        let prev = Rect::from_origin_size(origin, Size::new(header_h, header_h));
        let next = Rect::new(header.max.x - header_h, header.min.y, header.max.x, header.max.y);
        let title = Rect::new(prev.max.x, header.min.y, next.min.x, header.max.y);
        let top = header.max.y + if header_h > 0.0 { GAP } else { 0.0 };
        let weekdays = Rect::new(origin.x, top, origin.x + w, top + wd_h);
        let grid = Rect::from_origin_size(Point::new(origin.x + week_col, weekdays.max.y), Size::new(7.0 * CELL.w, 6.0 * CELL.h));
        CalGeom { header, prev, next, title, weekdays, grid, cell: CELL, week_col }
    }

    /// The body's size (content box).
    pub fn size(&self) -> Size {
        Size::new(self.header.width().max(self.week_col + self.grid.width()), self.grid.max.y - self.header.min.y)
    }

    pub fn cell_rect(&self, k: usize) -> Rect {
        let o = Point::new(self.grid.min.x + (k % 7) as f32 * self.cell.w, self.grid.min.y + (k / 7) as f32 * self.cell.h);
        Rect::from_origin_size(o, self.cell)
    }

    pub fn hit(&self, p: Point, cfg: &CalCfg) -> Option<CalHit> {
        if cfg.navigation && self.prev.contains(p) {
            return Some(CalHit::Prev);
        }
        if cfg.navigation && self.next.contains(p) {
            return Some(CalHit::Next);
        }
        if self.grid.contains(p) {
            let c = ((p.x - self.grid.min.x) / self.cell.w) as usize;
            let r = ((p.y - self.grid.min.y) / self.cell.h) as usize;
            return Some(CalHit::Day(r.min(5) * 7 + c.min(6)));
        }
        None
    }
}

/// The calendar's state between events.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CalBody {
    /// The shown month; none: the value's (else today's).
    page: Option<(i32, u8)>,
    /// The keyboard cursor.
    cursor: Option<Date>,
    hovered: Option<CalHit>,
    pressed: Option<CalHit>,
}

impl CalBody {
    /// The month shown.
    pub fn page(&self, value: Option<Date>, today: Date) -> (i32, u8) {
        self.page.unwrap_or_else(|| {
            let d = value.unwrap_or(today);
            (d.year(), d.month())
        })
    }

    /// Shows the month of `d` and puts the cursor on it (a host write, the popup opening).
    pub fn reset(&mut self, d: Option<Date>) {
        self.page = None;
        self.cursor = d;
    }

    pub fn cursor(&self, value: Option<Date>, today: Date) -> Date {
        self.cursor.or(value).unwrap_or(today)
    }

    pub fn hovered(&self) -> Option<CalHit> {
        self.hovered
    }

    fn set_page(&mut self, d: Date, value: Option<Date>, today: Date, out: &mut Vec<CalOut>) {
        let p = (d.year(), d.month());
        if p != self.page(value, today) {
            out.push(CalOut::Page(p.0, p.1));
        }
        self.page = Some(p);
    }

    /// The keys the body takes while its host holds the keyboard.
    pub fn claims(key: Key) -> bool {
        matches!(key, Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown | Key::Home | Key::End | Key::PageUp | Key::PageDown | Key::Enter | Key::Space)
    }

    /// A key pressed while the host holds the keyboard. Moves the cursor (clamped to `min`/`max`),
    /// pages along with it, and picks on Enter/Space.
    pub fn key(&mut self, key: Key, m: Modifiers, cfg: &CalCfg, value: Option<Date>, today: Date, enabled: bool) -> Vec<CalOut> {
        let mut out = Vec::new();
        if !enabled || !CalBody::claims(key) {
            return out;
        }
        let cur = self.cursor(value, today);
        let wd = (cur.weekday() as i64 - cfg.first_day as i64).rem_euclid(7);
        let shift = m.contains(Modifiers::SHIFT);
        let next = match key {
            Key::ArrowLeft => cur.add_days(-1),
            Key::ArrowRight => cur.add_days(1),
            Key::ArrowUp => cur.add_days(-7),
            Key::ArrowDown => cur.add_days(7),
            Key::Home => cur.add_days(-wd),
            Key::End => cur.add_days(6 - wd),
            Key::PageUp => cur.add_months(if shift { -12 } else { -1 }),
            Key::PageDown => cur.add_months(if shift { 12 } else { 1 }),
            _ => cur,
        };
        let next = cfg.clamp(next);
        if next != cur || self.cursor.is_none() {
            self.cursor = Some(next);
            self.set_page(next, value, today, &mut out);
        }
        if matches!(key, Key::Enter | Key::Space) && !cfg.read_only && cfg.in_range(next) {
            out.push(CalOut::Pick(next));
        }
        out
    }

    /// A pointer event in body-local points laid out as `g`.
    pub fn pointer(&mut self, ev: &PointerEvent, g: &CalGeom, cfg: &CalCfg, value: Option<Date>, today: Date, enabled: bool) -> Vec<CalOut> {
        let mut out = Vec::new();
        let Some(p) = Ptr::of(ev) else { return out };
        match p {
            Ptr::Move(p) => self.hovered = g.hit(p, cfg),
            Ptr::Leave => {
                self.hovered = None;
                self.pressed = None;
            }
            Ptr::Down(p) => self.pressed = if enabled { g.hit(p, cfg) } else { None },
            Ptr::Up(p) => {
                let hit = self.pressed.take();
                if hit.is_none() || hit != g.hit(p, cfg) || !enabled {
                    return out;
                }
                let (py, pm) = self.page(value, today);
                let first = Date::new(py, pm, 1).unwrap_or(today);
                match hit {
                    Some(CalHit::Prev) | Some(CalHit::Next) => {
                        let target = if hit == Some(CalHit::Prev) { first.add_days(-1).first_of_month() } else { first.add_months(1) };
                        if cfg.in_range_month(target) {
                            self.cursor = Some(cfg.clamp(target));
                            self.set_page(target, value, today, &mut out);
                        }
                    }
                    Some(CalHit::Day(k)) => {
                        let d = grid_days(py, pm, cfg.first_day)[k];
                        if cfg.in_range(d) {
                            self.cursor = Some(d);
                            if d.month() != pm {
                                self.set_page(d, value, today, &mut out);
                            }
                            if !cfg.read_only {
                                out.push(CalOut::Pick(d));
                            }
                        }
                    }
                    None => {}
                }
            }
        }
        out
    }

    /// Records the body at `g`. `ring`: the cursor day shows the focus ring (the host is
    /// focus-visible, or its popup was opened from the keyboard).
    #[allow(clippy::too_many_arguments)]
    pub fn record(&self, list: &mut DisplayList, text: &mut dyn TextPainter, look: &PickerLook, g: &CalGeom, cfg: &CalCfg, value: Option<Date>, today: Date, enabled: bool, ring: bool) {
        let (py, pm) = self.page(value, today);
        let first = Date::new(py, pm, 1).unwrap_or(today);
        if cfg.navigation {
            let bold = look.text(look.fg).with_weight(600);
            text.draw(list, &format!("{} {}", MONTHS[pm as usize - 1], py), &bold, g.title, HAlign::Center, true);
            for (hit, rect, target, dir) in [(CalHit::Prev, g.prev, first.add_days(-1), Chevron::Left), (CalHit::Next, g.next, first.add_months(1), Chevron::Right)] {
                let on = enabled && cfg.in_range_month(target);
                if on && self.pressed == Some(hit) {
                    shim::fill(list, rect, look.toolbar_hover.with_alpha((look.toolbar_hover.a * 1.6).min(1.0)), look.radius);
                } else if on && self.hovered == Some(hit) {
                    shim::fill(list, rect, look.toolbar_hover, look.radius);
                }
                popup::record_chevron(list, dir, rect, 16.0, if on { look.icon } else { look.disabled_fg });
            }
        }
        if cfg.weekday_format != "none" {
            let st = look.text(look.muted);
            for c in 0..7 {
                let r = Rect::new(g.grid.min.x + c as f32 * g.cell.w, g.weekdays.min.y, g.grid.min.x + (c + 1) as f32 * g.cell.w, g.weekdays.max.y);
                text.draw(list, cfg.weekday_label(c + cfg.first_day as usize), &st, r, HAlign::Center, false);
            }
        }
        let days = grid_days(py, pm, cfg.first_day);
        if cfg.week_numbers {
            let st = look.text(look.muted);
            for row in 0..6 {
                // the row's Thursday-relative day decides the ISO week, as svelters-egui (+3)
                let (_, w) = days[row * 7 + 3].iso_week();
                let r = Rect::new(g.weekdays.min.x, g.grid.min.y + row as f32 * g.cell.h, g.weekdays.min.x + g.week_col, g.grid.min.y + (row + 1) as f32 * g.cell.h);
                text.draw(list, &w.to_string(), &st, r, HAlign::Center, false);
            }
        }
        let cursor = self.cursor(value, today);
        for (k, d) in days.iter().copied().enumerate() {
            let rect = g.cell_rect(k);
            let in_range = cfg.in_range(d) && enabled;
            let selected = value == Some(d);
            let other = d.month() != pm;
            let bg = if selected {
                look.list_sel
            } else if in_range && self.hovered == Some(CalHit::Day(k)) {
                look.list_hover
            } else {
                Color::TRANSPARENT
            };
            shim::fill(list, rect, bg, look.radius);
            if selected && look.high_contrast {
                // the selection fill is the window's own colour in High Contrast: outline it
                shim::frame(list, rect, Color::TRANSPARENT, look.fg, 1.0, look.radius);
            }
            let fg = if !in_range {
                look.disabled_fg
            } else if selected {
                look.list_sel_fg
            } else if other {
                look.muted
            } else {
                look.fg
            };
            text.draw(list, &d.day().to_string(), &look.text(fg), rect, HAlign::Center, false);
            if d == today {
                // today: a short bar under its number (the keyboard cursor is the ring)
                let mark = if selected { look.list_sel_fg } else if look.high_contrast { look.fg } else { look.accent };
                let (cx, y) = ((rect.min.x + rect.max.x) / 2.0, rect.max.y - 4.0);
                shim::line(list, skelter_base::geom::Point::new(cx - 6.0, y), skelter_base::geom::Point::new(cx + 6.0, y), 2.0, mark);
            }
            if ring && d == cursor {
                shim::frame(list, rect.inflate(-1.0), Color::TRANSPARENT, look.focus, 1.0, look.radius);
            }
        }
    }

    /// The accessible description of grid cell `k` (svelters-egui's GridCell description:
    /// "Thursday 22 October 2026 (today, milestone)"); the label is the ISO date.
    pub fn describe(&self, k: usize, cfg: &CalCfg, value: Option<Date>, today: Date) -> (String, String) {
        let (py, pm) = self.page(value, today);
        let d = grid_days(py, pm, cfg.first_day)[k];
        let mut names: Vec<&str> = Vec::new();
        if d == today {
            names.push("today");
        }
        if d.weekday() >= 5 {
            names.push("weekend");
        }
        if d.month() != pm {
            names.push("other-month");
        }
        names.extend(cfg.names_of(d));
        let desc = format!("{} {} {} {}", WEEKDAYS[d.weekday() as usize], d.day(), MONTHS[d.month() as usize - 1], d.year());
        let desc = if names.is_empty() { desc } else { format!("{desc} ({})", names.join(", ")) };
        (d.to_string(), desc)
    }
}

/// The `calendar` kind: an inline month calendar with `bind:value`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Calendar {
    /// The picked date; none for `""`.
    pub value: Option<Date>,
    pub cfg: CalCfg,
    pub body: CalBody,
}

impl Calendar {
    /// A host write of `value`: the page moves to it.
    pub fn set_value(&mut self, value: Option<Date>) {
        self.value = value;
        self.body.reset(value);
    }

    pub fn size(&self, look: &PickerLook) -> Size {
        CalGeom::new(look, &self.cfg, Point::ZERO).size()
    }

    /// Turns the body's outcomes into the kind's value and signals: a pick sets `value` and raises
    /// `changed(value)` then `commit(value)`; a page change raises `page-changed(year, month)`.
    pub fn apply(&mut self, outs: Vec<CalOut>) -> Vec<Emit> {
        let mut emits = Vec::new();
        for o in outs {
            match o {
                CalOut::Pick(d) => {
                    self.value = Some(d);
                    let iso = Value::Str(d.to_string());
                    emits.push(Emit::new("changed", vec![iso.clone()]));
                    emits.push(Emit::new("commit", vec![iso]));
                }
                CalOut::Page(y, m) => emits.push(Emit::new("page-changed", vec![Value::I64(y as i64), Value::I64(m as i64)])),
            }
        }
        emits
    }

    pub fn key(&mut self, key: Key, m: Modifiers, today: Date, enabled: bool) -> Vec<Emit> {
        let outs = self.body.key(key, m, &self.cfg, self.value, today, enabled);
        self.apply(outs)
    }

    pub fn pointer(&mut self, ev: &PointerEvent, look: &PickerLook, today: Date, enabled: bool) -> Vec<Emit> {
        let g = CalGeom::new(look, &self.cfg, Point::ZERO);
        let outs = self.body.pointer(ev, &g, &self.cfg, self.value, today, enabled);
        self.apply(outs)
    }

    pub fn record(&self, list: &mut DisplayList, text: &mut dyn TextPainter, look: &PickerLook, today: Date, enabled: bool, focus_visible: bool) {
        let g = CalGeom::new(look, &self.cfg, Point::ZERO);
        self.body.record(list, text, look, &g, &self.cfg, self.value, today, enabled, focus_visible);
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
    fn grids_start_on_the_first_day_of_the_week() {
        // 2026-10-01 is a Thursday
        assert_eq!(grid_start(2026, 10, 0), d("2026-09-28"));
        assert_eq!(grid_start(2026, 10, 6), d("2026-09-27"));
        assert_eq!(grid_start(2026, 10, 3), d("2026-10-01"));
        // February 2021 starts on a Monday: its grid starts on its first day
        let g = grid_days(2021, 2, 0);
        assert_eq!(g[0], d("2021-02-01"));
        assert_eq!(g[27], d("2021-02-28"));
        assert_eq!(g[41], d("2021-03-14"));
        assert_eq!(CalCfg::first_day_of("sunday"), 6);
        assert_eq!(CalCfg::first_day_of("auto"), 0);
        assert_eq!(CalCfg::first_day_of("bogus"), 0);
    }

    #[test]
    fn day_parts_and_labels() {
        let cfg = CalCfg { day_parts: vec!["2026-10-22 milestone release".into(), "2026-10-2 nope".into()], ..CalCfg::default() };
        assert_eq!(cfg.names_of(d("2026-10-22")), vec!["milestone", "release"]);
        assert!(cfg.names_of(d("2026-10-02")).is_empty());
        assert_eq!(cfg.weekday_label(6), "Su");
        let body = CalBody::default();
        // cell 24 of October 2026 (Monday first) is 2026-10-22
        let (label, desc) = body.describe(24, &cfg, None, d("2026-10-22"));
        assert_eq!(label, "2026-10-22");
        assert_eq!(desc, "Thursday 22 October 2026 (today, milestone, release)");
    }

    #[test]
    fn keys_move_across_months_and_pick() {
        let cfg = CalCfg::default();
        let today = d("2026-10-22");
        let mut b = CalBody::default();
        let v = Some(d("2026-10-31"));
        assert_eq!(b.key(Key::ArrowRight, Modifiers::default(), &cfg, v, today, true), vec![CalOut::Page(2026, 11)]);
        assert_eq!(b.cursor(v, today), d("2026-11-01"));
        assert_eq!(b.key(Key::ArrowUp, Modifiers::default(), &cfg, v, today, true), vec![CalOut::Page(2026, 10)]);
        assert_eq!(b.cursor(v, today), d("2026-10-25"));
        b.key(Key::Home, Modifiers::default(), &cfg, v, today, true);
        assert_eq!(b.cursor(v, today), d("2026-10-19"), "Monday of that week");
        b.key(Key::End, Modifiers::default(), &cfg, v, today, true);
        assert_eq!(b.cursor(v, today), d("2026-10-25"));
        assert_eq!(b.key(Key::PageUp, Modifiers::SHIFT, &cfg, v, today, true), vec![CalOut::Page(2025, 10)]);
        assert_eq!(b.key(Key::Enter, Modifiers::default(), &cfg, v, today, true), vec![CalOut::Pick(d("2025-10-25"))]);
        let ro = CalCfg { read_only: true, ..CalCfg::default() };
        assert!(b.key(Key::Space, Modifiers::default(), &ro, v, today, true).is_empty(), "read-only roves but never picks");
    }

    #[test]
    fn min_and_max_clamp_the_cursor_and_disable_days() {
        let cfg = CalCfg { min: Some(d("2026-10-10")), max: Some(d("2026-10-20")), ..CalCfg::default() };
        let today = d("2026-10-15");
        let mut b = CalBody::default();
        b.reset(Some(d("2026-10-11")));
        b.key(Key::ArrowUp, Modifiers::default(), &cfg, None, today, true);
        assert_eq!(b.cursor(None, today), d("2026-10-10"), "clamped to min");
        b.key(Key::PageDown, Modifiers::default(), &cfg, None, today, true);
        assert_eq!(b.cursor(None, today), d("2026-10-20"), "clamped to max");
        assert!(cfg.in_range_month(d("2026-10-01")));
        assert!(!cfg.in_range_month(d("2026-11-01")));
        // clicking a disabled day picks nothing
        let look = PickerLook::default();
        let g = CalGeom::new(&look, &cfg, Point::ZERO);
        let r = g.cell_rect(0); // 2026-09-28
        let c = Point::new(r.min.x + 2.0, r.min.y + 2.0);
        b.pointer(&ptr(PointerPhase::Down, c.x, c.y), &g, &cfg, None, today, true);
        assert!(b.pointer(&ptr(PointerPhase::Up, c.x, c.y), &g, &cfg, None, today, true).is_empty());
    }

    #[test]
    fn a_click_picks_and_next_pages() {
        let look = PickerLook::default();
        let today = d("2026-10-22");
        let mut cal = Calendar::default();
        let g = CalGeom::new(&look, &cal.cfg, Point::ZERO);
        let r = g.cell_rect(24);
        let (x, y) = (r.min.x + 3.0, r.min.y + 3.0);
        cal.pointer(&ptr(PointerPhase::Down, x, y), &look, today, true);
        let e = cal.pointer(&ptr(PointerPhase::Up, x, y), &look, today, true);
        assert_eq!(e, vec![Emit::new("changed", vec![Value::Str("2026-10-22".into())]), Emit::new("commit", vec![Value::Str("2026-10-22".into())])]);
        assert_eq!(cal.value, Some(today));
        let n = g.next;
        let (x, y) = (n.min.x + 2.0, n.min.y + 2.0);
        cal.pointer(&ptr(PointerPhase::Down, x, y), &look, today, true);
        assert_eq!(cal.pointer(&ptr(PointerPhase::Up, x, y), &look, today, true), vec![Emit::new("page-changed", vec![Value::I64(2026), Value::I64(11)])]);
    }

    #[test]
    fn records_header_weekdays_week_numbers_and_42_days() {
        let look = PickerLook::default();
        let cal = Calendar { value: Some(d("2026-10-22")), ..Calendar::default() };
        let mut list = DisplayList::new();
        let mut t = FixedText::default();
        cal.record(&mut list, &mut t, &look, d("2026-10-22"), true, false);
        let texts = t.texts();
        assert_eq!(texts[0], "October 2026");
        assert_eq!(&texts[1..8], &["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"]);
        assert_eq!(texts[8], "40", "the first row's ISO week");
        assert_eq!(texts.len(), 1 + 7 + 6 + 42);
        assert_eq!(texts[14], "28", "the grid starts on Monday 28 September");
    }
}
