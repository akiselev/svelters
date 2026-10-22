//! `calendar`, a new kind (OW6; WIDGET-PARITY §2.19 `QCalendarWidget`), and the calendar body
//! that `date-picker` shows in its popup ([`CalBody`]).
//!
//! Layout: a header (`::part(header)`) with `::part(prev)`, the title (`::part(month)`,
//! `::part(year)`) and `::part(next)`; a weekday row (`::part(weekday)`, `weekday-format`); a
//! 6×7 day grid (`::part(grid)`, `::part(day)`), with ISO week numbers in front
//! (`::part(week-number)`, `calendar-week`). The week starts on `first-day`.
//!
//! Day states: `:hover`, `:selected` (the value), `:current` (the keyboard cursor; with the
//! host's `:focus-visible` it carries the ring, DESIGN R8 for rows: 1px `focus` at −1),
//! `:disabled` (outside `min`/`max`). Day names (P10): `today`, `weekend`, `other-month` and the
//! names `day-parts` gives a date. Until the style engine computes per-instance part states and
//! multi-name parts on egui (handoff in plans/milestones/OW6-pickers.md), the states and names
//! paint from the UA tables below over the host's `::part(day)` style.
//!
//! Keys (the host is one tab stop; the grid roves): arrows move a day or a week, Home/End the
//! week's start/end, PageUp/PageDown a month (with Shift a year), Enter/Space pick the current
//! day. The pointer: a day click picks it, prev/next page. `read-only` shows and roves but never
//! picks. AccessKit: a `Grid` of `Row`s of `GridCell`s (named by ISO date, `selected`,
//! `disabled`), with `Button`s for prev/next. Writer lane: OW6.

use crate::interact::{Focusable, Hoverable};
use crate::paint::{self, Align, StyleView, Ua, UaRule, UaValue::*};
use crate::provider::{A11yCx, AvailableSpace, Event, MeasureCx, StateBits, Widget, WidgetCx};
use egui::{Key, Modifiers, Rect, Vec2, accesskit, pos2, vec2};
use jiff::civil::{Date, Weekday};

/// The host: no box of its own inline (a panel's content), the text style the parts inherit.
pub const UA: Ua = Ua(&[UaRule::base(&[("color", Token("fg")), ("font-size", Token("font-size")), ("gap", Px(2.0))])]);

/// `::part(header)`: the navigation row.
pub const HEADER_UA: Ua = Ua(&[UaRule::base(&[("height", Token("row-h")), ("font-weight", Number(600.0))])]);

/// `::part(prev)`, `::part(next)`: toolbar-style icon buttons (R3 hover, R10 pointer).
pub const NAV_UA: Ua = Ua(&[
    UaRule::base(&[("width", Token("row-h")), ("height", Token("row-h")), ("border-radius", Token("input-radius")), ("color", Token("icon")), ("font-size", Px(16.0)), ("cursor", Keyword("pointer"))]),
    UaRule::when(StateBits::HOVERED, &[("background-color", Token("toolbar-hover"))]),
    UaRule::when(StateBits::PRESSED, &[("background-color", Token("toolbar-active"))]),
    UaRule::when(StateBits::DISABLED, &[("color", Token("disabled-fg")), ("cursor", Keyword("default"))]),
]);

/// `::part(weekday)` and `::part(week-number)`: secondary text.
pub const WEEKDAY_UA: Ua = Ua(&[UaRule::base(&[("color", Token("muted")), ("width", Px(28.0)), ("height", Px(20.0))])]);

/// `::part(day)`: list-row states (DESIGN R6), radius `input-radius`.
pub const DAY_UA: Ua = Ua(&[
    UaRule::base(&[
        ("width", Px(28.0)),
        ("height", Px(24.0)),
        ("border-radius", Token("input-radius")),
        ("color", Token("fg")),
        ("cursor", Keyword("pointer")),
        ("outline-style", Keyword("none")),
        ("outline-color", Token("focus")),
        ("outline-width", Px(1.0)),
        ("outline-offset", Px(-1.0)),
    ]),
    UaRule::when(StateBits::HOVERED, &[("background-color", Token("list-hover"))]),
    UaRule::when(StateBits::SELECTED, &[("background-color", Token("list-sel")), ("color", Token("list-sel-fg"))]),
    UaRule::when(StateBits::FOCUS_VISIBLE, &[("outline-style", Keyword("solid"))]),
    UaRule::when(StateBits::DISABLED, &[("color", Token("disabled-fg")), ("cursor", Keyword("not-allowed"))]),
]);

/// `::part(day other-month)`: days of the neighbouring months (muted, else as [`DAY_UA`]).
pub const OTHER_MONTH_UA: Ua = Ua(&[
    UaRule::base(&[
        ("width", Px(28.0)),
        ("height", Px(24.0)),
        ("border-radius", Token("input-radius")),
        ("color", Token("muted")),
        ("cursor", Keyword("pointer")),
        ("outline-style", Keyword("none")),
        ("outline-color", Token("focus")),
        ("outline-width", Px(1.0)),
        ("outline-offset", Px(-1.0)),
    ]),
    UaRule::when(StateBits::HOVERED, &[("background-color", Token("list-hover"))]),
    UaRule::when(StateBits::SELECTED, &[("background-color", Token("list-sel")), ("color", Token("list-sel-fg"))]),
    UaRule::when(StateBits::FOCUS_VISIBLE, &[("outline-style", Keyword("solid"))]),
    UaRule::when(StateBits::DISABLED, &[("color", Token("disabled-fg")), ("cursor", Keyword("not-allowed"))]),
]);

/// `::part(day today)`: a 1px `accent` ring inside the cell.
pub const TODAY_UA: Ua = Ua(&[UaRule::base(&[("border-color", Token("accent")), ("border-width", Px(1.0))])]);

/// `first-day` values (`auto`: the locale's, Monday until IN1).
pub const FIRST_DAY: &[&str] = &["auto", "monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"];
/// `weekday-format` values (Qt's `horizontalHeaderFormat`).
pub const WEEKDAY_FORMAT: &[&str] = &["narrow", "short", "long", "none"];

const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
const WEEKDAYS: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];

/// Parses an ISO 8601 date (`""` is none).
pub fn parse_date(s: &str) -> Result<Option<Date>, String> {
    if s.is_empty() {
        return Ok(None);
    }
    s.parse::<Date>().map(Some).map_err(|e| format!("`{s}` is not an ISO 8601 date (YYYY-MM-DD): {e}"))
}

/// A date prop: ISO 8601 or `""`, stored normalized; anything else is refused on write (as the
/// egui wrapper's `date-picker` did).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IsoDate(pub String);

impl IsoDate {
    pub fn date(&self) -> Option<Date> {
        date_or_none(&self.0)
    }
}

impl svelters_host::exports::JsType for IsoDate {
    fn describe(b: &mut svelters_host::exports::__private::TypeBuilder) -> svelters_host::exports::__private::TypeRef {
        <String as svelters_host::exports::JsType>::describe(b)
    }
}

impl svelters_host::exports::FromJs for IsoDate {
    fn from_js(v: svelters_host::exports::Value, cx: &svelters_host::exports::CallCx<'_>) -> Result<Self, svelters_host::exports::NativeError> {
        let s = <Option<String> as svelters_host::exports::FromJs>::from_js(v, cx)?.unwrap_or_default();
        match parse_date(s.trim()) {
            Ok(d) => Ok(IsoDate(d.map(|d| d.to_string()).unwrap_or_default())),
            Err(message) => Err(super::invalid(message)),
        }
    }
}

impl svelters_host::exports::IntoJs for IsoDate {
    fn into_js(self, cx: &svelters_host::exports::CallCx<'_>) -> Result<svelters_host::exports::Value, svelters_host::exports::NativeError> {
        self.0.into_js(cx)
    }
}

/// A date read leniently (a bad string is none).
pub(crate) fn date_or_none(s: &str) -> Option<Date> {
    parse_date(s).ok().flatten()
}

pub(crate) fn today() -> Date {
    jiff::Zoned::now().date()
}

fn add_days(d: Date, n: i64) -> Date {
    d.checked_add(jiff::Span::new().days(n)).unwrap_or(d)
}

fn add_months(d: Date, n: i64) -> Date {
    d.checked_add(jiff::Span::new().months(n)).unwrap_or(d)
}

/// What the calendar is configured with (the props shared by `calendar` and `date-picker`).
#[derive(Clone, Debug)]
pub struct CalCfg<'a> {
    /// Monday = 0.
    pub first_day: i8,
    pub weekday_format: &'a str,
    pub week_numbers: bool,
    pub navigation: bool,
    pub min: Option<Date>,
    pub max: Option<Date>,
    pub read_only: bool,
    /// `"YYYY-MM-DD name…"` entries.
    pub day_parts: &'a [String],
}

impl CalCfg<'_> {
    /// `first-day` as a Monday-zero offset.
    pub fn first_day_of(v: &str) -> i8 {
        match super::pick(v, FIRST_DAY, "auto") {
            "auto" => 0,
            d => FIRST_DAY.iter().position(|x| *x == d).map_or(0, |i| i as i8 - 1),
        }
    }

    pub fn in_range(&self, d: Date) -> bool {
        self.min.is_none_or(|m| d >= m) && self.max.is_none_or(|m| d <= m)
    }

    fn clamp(&self, d: Date) -> Date {
        let d = self.min.map_or(d, |m| d.max(m));
        self.max.map_or(d, |m| d.min(m))
    }

    /// The extra part names `day-parts` gives `d`.
    pub fn names_of(&self, d: Date) -> impl Iterator<Item = &str> {
        let iso = d.to_string();
        self.day_parts.iter().filter_map(move |e| e.strip_prefix(iso.as_str()).filter(|r| r.starts_with(' ')).map(str::trim)).flat_map(str::split_whitespace)
    }

    /// The weekday header's text for Monday-zero `wd`.
    fn weekday_label(&self, wd: usize) -> String {
        let name = WEEKDAYS[wd % 7];
        match super::pick(self.weekday_format, WEEKDAY_FORMAT, "short") {
            "narrow" => name[..1].to_string(),
            "long" => name.to_string(),
            "none" => String::new(),
            _ => name[..2].to_string(),
        }
    }
}

/// The first cell of month `(y, m)`'s 6×7 grid.
pub(crate) fn grid_start(y: i16, m: i8, first_day: i8) -> Date {
    let first = Date::new(y, m, 1).unwrap_or_default();
    let wd = first.weekday().to_monday_zero_offset();
    add_days(first, -(((wd - first_day).rem_euclid(7)) as i64))
}

/// What one pass of the calendar did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CalOut {
    /// The user picked a day (click, Enter/Space, AccessKit Click).
    Pick(Date),
    /// The shown month changed by the user (prev/next, keys).
    Page(i16, i8),
}

/// The calendar's state between frames.
#[derive(Debug, Default)]
pub struct CalBody {
    /// The shown month; none: the value's (else today's).
    page: Option<(i16, i8)>,
    /// The keyboard cursor (`:current`).
    cursor: Option<Date>,
    prev_bits: StateBits,
    next_bits: StateBits,
}

/// The rects of one layout of the body.
struct Geom {
    header: Rect,
    prev: Rect,
    next: Rect,
    title: Rect,
    weekdays: Rect,
    grid: Rect,
    cell: Vec2,
    week_col: f32,
}

impl CalBody {
    /// The month shown.
    pub fn page(&self, value: Option<Date>) -> (i16, i8) {
        self.page.unwrap_or_else(|| {
            let d = value.unwrap_or_else(today);
            (d.year(), d.month())
        })
    }

    /// Shows the month of `d` and puts the cursor on it (a host write, the popup opening).
    pub fn reset(&mut self, d: Option<Date>) {
        self.page = None;
        self.cursor = d;
    }

    pub fn cursor(&self, value: Option<Date>) -> Date {
        self.cursor.or(value).unwrap_or_else(today)
    }

    fn sizes(host: &StyleView<'_>, cfg: &CalCfg<'_>) -> (Vec2, Vec2, f32, f32) {
        let st = host.state();
        let dark = host.is_dark();
        let day = StyleView::ua_only(DAY_UA, st, dark);
        let cell = vec2(day.px("width").unwrap_or(28.0), day.px("height").unwrap_or(24.0));
        let wk = StyleView::ua_only(WEEKDAY_UA, st, dark);
        let week_col = if cfg.week_numbers { wk.px("width").unwrap_or(cell.x) } else { 0.0 };
        let wd_h = if cfg.weekday_format == "none" { 0.0 } else { wk.px("height").unwrap_or(20.0) };
        let hd = StyleView::ua_only(HEADER_UA, st, dark);
        let header_h = if cfg.navigation { hd.px("height").unwrap_or(22.0) } else { 0.0 };
        (cell, vec2(week_col, wd_h), header_h, super::parts::gap(host, 2.0))
    }

    /// The size the body asks for (content box).
    pub fn measure(host: &StyleView<'_>, cfg: &CalCfg<'_>) -> Vec2 {
        let (cell, wk, header_h, gap) = CalBody::sizes(host, cfg);
        let w = wk.x + 7.0 * cell.x;
        let h = header_h + gap + wk.y + 6.0 * cell.y;
        vec2(w, h)
    }

    fn geom(host: &StyleView<'_>, cfg: &CalCfg<'_>, r: Rect) -> Geom {
        let (cell, wk, header_h, gap) = CalBody::sizes(host, cfg);
        let nav = StyleView::ua_only(NAV_UA, host.state(), host.is_dark());
        let nav_w = nav.px("width").unwrap_or(header_h);
        let header = Rect::from_min_size(r.min, vec2(r.width(), header_h));
        let prev = Rect::from_min_size(header.min, vec2(nav_w, header_h));
        let next = Rect::from_min_size(pos2(header.right() - nav_w, header.top()), vec2(nav_w, header_h));
        let title = Rect::from_min_max(pos2(prev.right(), header.top()), pos2(next.left(), header.bottom()));
        let top = header.bottom() + if header_h > 0.0 { gap } else { 0.0 };
        let weekdays = Rect::from_min_size(pos2(r.left(), top), vec2(r.width(), wk.y));
        let grid = Rect::from_min_size(pos2(r.left() + wk.x, weekdays.bottom()), vec2(7.0 * cell.x, 6.0 * cell.y));
        Geom { header, prev, next, title, weekdays, grid, cell, week_col: wk.x }
    }

    fn cell_rect(g: &Geom, k: usize) -> Rect {
        Rect::from_min_size(g.grid.min + vec2((k % 7) as f32 * g.cell.x, (k / 7) as f32 * g.cell.y), g.cell)
    }

    fn set_page(&mut self, d: Date, out: &mut Vec<CalOut>, value: Option<Date>) {
        let p = (d.year(), d.month());
        if p != self.page(value) {
            out.push(CalOut::Page(p.0, p.1));
        }
        self.page = Some(p);
    }

    /// The keys the body takes while its host is focused.
    pub fn claims(key: Key, _m: Modifiers) -> bool {
        matches!(key, Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown | Key::Home | Key::End | Key::PageUp | Key::PageDown | Key::Enter | Key::Space)
    }

    /// Runs and paints the body in `r`. `host` is the host's style; `keys`: the host holds the
    /// keyboard (focused, or its popup is open); `enabled`: picks allowed; `a11y_parent`: the
    /// AccessKit node the grid hangs under; `ring`: the cursor shows the focus ring.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn ui(&mut self, ui: &mut egui::Ui, cx: &WidgetCx<'_>, host: &StyleView<'_>, cfg: &CalCfg<'_>, value: Option<Date>, r: Rect, keys: bool, ring: bool, a11y_parent: egui::Id) -> Vec<CalOut> {
        let mut out = Vec::new();
        let enabled = cx.enabled() && ui.is_enabled();
        let g = CalBody::geom(host, cfg, r);
        let id = a11y_parent.with("calendar");

        // ---- keys ----
        if keys && enabled {
            let mut cur = self.cursor(value);
            let before = cur;
            let mut pick = false;
            ui.input(|i| {
                for e in &i.events {
                    let egui::Event::Key { key, pressed: true, modifiers: m, .. } = e else { continue };
                    let wd = (cur.weekday().to_monday_zero_offset() - cfg.first_day).rem_euclid(7) as i64;
                    cur = match key {
                        Key::ArrowLeft => add_days(cur, -1),
                        Key::ArrowRight => add_days(cur, 1),
                        Key::ArrowUp => add_days(cur, -7),
                        Key::ArrowDown => add_days(cur, 7),
                        Key::Home => add_days(cur, -wd),
                        Key::End => add_days(cur, 6 - wd),
                        Key::PageUp => add_months(cur, if m.shift { -12 } else { -1 }),
                        Key::PageDown => add_months(cur, if m.shift { 12 } else { 1 }),
                        Key::Enter | Key::Space => {
                            pick = true;
                            cur
                        }
                        _ => cur,
                    };
                }
            });
            cur = cfg.clamp(cur);
            if cur != before {
                self.cursor = Some(cur);
                self.set_page(cur, &mut out, value);
                ui.ctx().request_repaint();
            }
            if pick && !cfg.read_only && cfg.in_range(cur) {
                out.push(CalOut::Pick(cur));
            }
        }

        // ---- the header ----
        let (py, pm) = self.page(value);
        let first_shown = Date::new(py, pm, 1).unwrap_or_default();
        let dark = host.is_dark();
        if cfg.navigation {
            let hdr = cx.part(host, "header", HEADER_UA);
            let mut pc = cx.paint_cx(ui);
            paint::paint_box(&mut pc.part("header"), &hdr, g.header);
            let month = MONTHS[(pm - 1) as usize];
            let mv = cx.part(&hdr, "month", Ua::NONE);
            let yv = cx.part(&hdr, "year", Ua::NONE);
            let mg = paint::text_galley(ui.ctx(), &mv, month, None);
            let yg = paint::text_galley(ui.ctx(), &yv, &py.to_string(), None);
            let sp = mv.font_size() * 0.3;
            let total = mg.size().x + sp + yg.size().x;
            let x0 = g.title.center().x - total / 2.0;
            paint::paint_text(&mut pc.part("month"), &mv, &mg, Rect::from_min_max(pos2(x0, g.title.top()), pos2(x0 + mg.size().x, g.title.bottom())), Align::Start);
            let yx = x0 + mg.size().x + sp;
            paint::paint_text(&mut pc.part("year"), &yv, &yg, Rect::from_min_max(pos2(yx, g.title.top()), pos2(yx + yg.size().x, g.title.bottom())), Align::Start);
            drop(pc);
            let prev_month = add_days(first_shown, -1);
            let next_month = add_months(first_shown, 1);
            for (k, (name, rect, target, label)) in [("prev", g.prev, prev_month, "Previous month"), ("next", g.next, next_month, "Next month")].into_iter().enumerate() {
                let bid = id.with(name);
                let on = enabled && cfg.in_range_month(target);
                let resp = ui.interact(rect, bid, if on { egui::Sense::click() } else { egui::Sense::hover() });
                let bits = if k == 0 { &mut self.prev_bits } else { &mut self.next_bits };
                bits.set(StateBits::HOVERED, resp.hovered());
                bits.set(StateBits::PRESSED, resp.is_pointer_button_down_on());
                bits.set(StateBits::DISABLED, !on);
                let v = cx.part(host, name, NAV_UA).in_state(*bits);
                let mut pc = cx.paint_cx(ui);
                paint::paint_box(&mut pc.part(name), &v, rect);
                super::picker_glyphs::chevron(&mut pc.part(name), &v, if k == 0 { super::picker_glyphs::Dir::Left } else { super::picker_glyphs::Dir::Right }, rect);
                if resp.hovered()
                    && let Some(c) = v.cursor()
                {
                    ui.ctx().set_cursor_icon(c);
                }
                let ak = ui.input(|i| i.has_accesskit_action_request(bid, accesskit::Action::Click));
                if on && (resp.clicked() || ak) {
                    let d = cfg.clamp(target);
                    self.cursor = Some(d);
                    self.set_page(target, &mut out, value);
                }
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, on, label));
            }
        }

        // ---- weekdays and week numbers ----
        let wk = cx.part(host, "weekday", WEEKDAY_UA);
        if cfg.weekday_format != "none" {
            let mut pc = cx.paint_cx(ui);
            for c in 0..7 {
                let t = cfg.weekday_label(c + cfg.first_day as usize);
                let gl = paint::text_galley(ui.ctx(), &wk, &t, None);
                let rr = Rect::from_min_size(pos2(g.grid.left() + c as f32 * g.cell.x, g.weekdays.top()), vec2(g.cell.x, g.weekdays.height()));
                paint::paint_text(&mut pc.part("weekday"), &wk, &gl, rr, Align::Center);
            }
        }
        let start = grid_start(py, pm, cfg.first_day);
        if cfg.week_numbers {
            let wn = cx.part(host, "week-number", WEEKDAY_UA);
            let mut pc = cx.paint_cx(ui);
            for row in 0..6 {
                let d = add_days(start, row * 7 + 3);
                let n = d.iso_week_date().week();
                let gl = paint::text_galley(ui.ctx(), &wn, &n.to_string(), None);
                let rr = Rect::from_min_size(pos2(r.left(), g.grid.top() + row as f32 * g.cell.y), vec2(g.week_col, g.cell.y));
                paint::paint_text(&mut pc.part("week-number"), &wn, &gl, rr, Align::Center);
            }
        }

        // ---- the grid ----
        let hover = ui.input(|i| i.pointer.hover_pos()).filter(|p| g.grid.contains(*p) && ui.rect_contains_pointer(g.grid));
        let cursor = self.cursor(value);
        let today = today();
        let today_v = StyleView::ua_only(TODAY_UA, StateBits::NONE, dark);
        let focus_visible = ring && keys;
        ui.ctx().accesskit_node_builder(a11y_parent, |n| {
            n.set_row_count(6);
            n.set_column_count(7);
        });
        for row in 0..6usize {
            let row_ui = ui.new_child(egui::UiBuilder::new().id_salt(("cal-row", row)).max_rect(Rect::from_min_size(pos2(g.grid.left(), g.grid.top() + row as f32 * g.cell.y), vec2(g.grid.width(), g.cell.y))).accessibility_parent(a11y_parent));
            row_ui.ctx().accesskit_node_builder(row_ui.unique_id(), |n| {
                n.set_role(accesskit::Role::Row);
                n.set_row_index(row);
            });
            for col in 0..7usize {
                let k = row * 7 + col;
                let d = add_days(start, k as i64);
                let rect = CalBody::cell_rect(&g, k);
                let cid = id.with(("day", k));
                let resp = row_ui.interact(rect, cid, egui::Sense::click());
                let in_range = cfg.in_range(d);
                let mut bits = StateBits::NONE;
                bits.set(StateBits::HOVERED, hover.is_some_and(|p| rect.contains(p)));
                bits.set(StateBits::PRESSED, resp.is_pointer_button_down_on());
                bits.set(StateBits::SELECTED, value == Some(d));
                bits.set(StateBits::CURRENT, cursor == d);
                bits.set(StateBits::FOCUS_VISIBLE, focus_visible && cursor == d);
                bits.set(StateBits::DISABLED, !in_range || !enabled);
                let other = d.month() != pm;
                let v = cx.part(host, "day", if other { OTHER_MONTH_UA } else { DAY_UA }).in_state(bits);
                let weekend = matches!(d.weekday(), Weekday::Saturday | Weekday::Sunday);
                let is_today = d == today;
                let mut pc = cx.paint_cx(ui);
                let mut dp = pc.part("day");
                paint::paint_box(&mut dp, &v, rect);
                if is_today && !bits.contains(StateBits::SELECTED) {
                    let c = today_v.color("border-color").unwrap_or(egui::Color32::TRANSPARENT);
                    let w = today_v.px("border-width").unwrap_or(1.0);
                    let rr = v.radii()[0];
                    dp.painter.rect_stroke(dp.snap_rect(rect), rr, egui::Stroke::new(w, c), egui::StrokeKind::Inside);
                    dp.record(paint::TraceKind::Box, rect, c, egui::Stroke::new(w, c), 0.0);
                }
                let gl = paint::text_galley(ui.ctx(), &v, &d.day().to_string(), None);
                paint::paint_text(&mut dp, &v, &gl, rect, Align::Center);
                paint::paint_outline(&mut dp, &v, rect);
                drop(pc);
                if bits.contains(StateBits::HOVERED)
                    && let Some(c) = v.cursor()
                {
                    ui.ctx().set_cursor_icon(c);
                }
                let ak = row_ui.input(|i| i.has_accesskit_action_request(cid, accesskit::Action::Click));
                if (resp.clicked() || ak) && enabled && in_range {
                    self.cursor = Some(d);
                    if other {
                        self.set_page(d, &mut out, value);
                    }
                    if !cfg.read_only {
                        out.push(CalOut::Pick(d));
                    }
                }
                let iso = d.to_string();
                let mut names: Vec<&str> = Vec::new();
                if is_today {
                    names.push("today");
                }
                if weekend {
                    names.push("weekend");
                }
                if other {
                    names.push("other-month");
                }
                names.extend(cfg.names_of(d));
                let desc = format!("{} {} {} {}", WEEKDAYS[d.weekday().to_monday_zero_offset() as usize], d.day(), MONTHS[(d.month() - 1) as usize], d.year());
                let sel = value == Some(d);
                row_ui.ctx().accesskit_node_builder(cid, |n| {
                    n.set_role(accesskit::Role::GridCell);
                    n.set_label(iso.clone());
                    n.set_description(if names.is_empty() { desc.clone() } else { format!("{desc} ({})", names.join(", ")) });
                    n.set_column_index(col);
                    n.set_row_index(row);
                    n.set_selected(sel);
                    if !in_range || !enabled {
                        n.set_disabled();
                    } else {
                        n.add_action(accesskit::Action::Click);
                    }
                });
            }
        }
        out
    }
}

impl CalCfg<'_> {
    /// Whether any day of `d`'s month is within `min`/`max`.
    fn in_range_month(&self, d: Date) -> bool {
        let first = Date::new(d.year(), d.month(), 1).unwrap_or(d);
        let last = add_days(add_months(first, 1), -1);
        self.min.is_none_or(|m| last >= m) && self.max.is_none_or(|m| first <= m)
    }
}

/// An inline month calendar.
// the `#[part]` fields name the kind's `::part()`s; nothing reads them (paint parts)
#[allow(dead_code)]
#[svelters::widget(namespace = "sv", replaces = "calendar", caps = FOCUSABLE | PAINTS_BOX)]
pub struct Calendar {
    /// The picked date (`bind:value`), ISO 8601; `""` for none.
    #[prop(bind, measure)]
    value: IsoDate,
    #[prop]
    min: IsoDate,
    #[prop]
    max: IsoDate,
    #[prop(default = "auto", measure)]
    first_day: String,
    #[prop(default = "short", measure)]
    weekday_format: String,
    #[prop(default = true, measure)]
    calendar_week: bool,
    #[prop(default = true, measure)]
    navigation: bool,
    #[prop]
    read_only: bool,
    #[prop]
    day_parts: Vec<String>,
    /// The user picked a day.
    #[event]
    changed: Event<String>,
    #[event]
    commit: Event<String>,
    /// The shown month changed: `(year, month)`.
    #[event]
    page_changed: Event<Vec<i64>>,
    #[behavior]
    hover: Hoverable,
    #[behavior]
    focus: Focusable,
    #[state]
    body: CalBody,
    /// The value the body was last reset to (a host write moves the page to it).
    #[state]
    shown: Option<String>,
    #[part]
    header: crate::provider::Part,
    #[part]
    prev: crate::provider::Part,
    #[part]
    next: crate::provider::Part,
    #[part]
    month: crate::provider::Part,
    #[part]
    year: crate::provider::Part,
    #[part]
    grid: crate::provider::Part,
    #[part]
    day: crate::provider::Part,
    #[part]
    weekday: crate::provider::Part,
    #[part(name = "week-number")]
    week_number: crate::provider::Part,
}

impl Calendar {
    fn cfg(&self) -> CalCfg<'_> {
        CalCfg {
            first_day: CalCfg::first_day_of(&self.first_day),
            weekday_format: super::pick(&self.weekday_format, WEEKDAY_FORMAT, "short"),
            week_numbers: self.calendar_week,
            navigation: self.navigation,
            min: self.min.date(),
            max: self.max.date(),
            read_only: self.read_only,
            day_parts: &self.day_parts,
        }
    }
}

impl Widget for Calendar {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, _avail: AvailableSpace) -> Vec2 {
        let s = cx.style(UA);
        CalBody::measure(&s, &self.cfg()) + paint::box_extra(&s)
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        cx.set_state(StateBits::READ_ONLY, self.read_only);
        if self.shown.as_deref() != Some(self.value.0.as_str()) {
            self.body.reset(self.value.date());
            self.shown = Some(self.value.0.clone());
        }
        let resp = cx.interact(ui, self.units_sense());
        let _ = self.run_units(cx, ui, &resp);
        let host = cx.style(UA);
        if host.opacity() < 1.0 {
            ui.multiply_opacity(host.opacity());
        }
        let geom = cx.paint_box(ui, UA);
        let focused = cx.state(StateBits::FOCUSED);
        let ring = cx.state(StateBits::FOCUS_VISIBLE);
        let value = self.value.date();
        // field by field: the body borrows `self.body` mutably beside the config
        let cfg = CalCfg {
            first_day: CalCfg::first_day_of(&self.first_day),
            weekday_format: super::pick(&self.weekday_format, WEEKDAY_FORMAT, "short"),
            week_numbers: self.calendar_week,
            navigation: self.navigation,
            min: self.min.date(),
            max: self.max.date(),
            read_only: self.read_only,
            day_parts: &self.day_parts,
        };
        let outs = self.body.ui(ui, cx, &host, &cfg, value, geom.content, focused, ring, resp.id);
        for o in outs {
            match o {
                CalOut::Pick(d) => {
                    let iso = d.to_string();
                    if !cx.dispatch(&self.changed, iso.clone()).prevented() {
                        self.shown = Some(iso.clone());
                        cx.set(&mut self.value, IsoDate(iso.clone()));
                        cx.dispatch(&self.commit, iso);
                    }
                }
                CalOut::Page(y, m) => {
                    cx.dispatch(&self.page_changed, vec![y as i64, m as i64]);
                }
            }
        }
        cx.paint_outline(ui, UA);
        let (enabled, text) = (cx.enabled(), self.value.0.clone());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Grid, enabled, &text));
        resp
    }

    fn a11y(&self, _cx: &A11yCx<'_>, node: &mut accesskit::Node) {
        node.set_role(accesskit::Role::Grid);
        self.units_a11y(node);
        if self.read_only {
            node.set_read_only();
        }
    }

    fn consume_key(&self, key: Key, m: Modifiers) -> bool {
        CalBody::claims(key, m) || self.units_claims(key, m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grids_start_on_the_first_day_of_the_week() {
        // 2026-10-01 is a Thursday
        assert_eq!(grid_start(2026, 10, 0).to_string(), "2026-09-28"); // Monday
        assert_eq!(grid_start(2026, 10, 6).to_string(), "2026-09-27"); // Sunday
        assert_eq!(grid_start(2026, 10, 3).to_string(), "2026-10-01"); // Thursday
        assert_eq!(CalCfg::first_day_of("sunday"), 6);
        assert_eq!(CalCfg::first_day_of("auto"), 0);
        assert_eq!(CalCfg::first_day_of("bogus"), 0);
    }

    #[test]
    fn day_parts_name_their_dates() {
        let parts = vec!["2026-10-22 milestone release".to_string(), "2026-10-2 nope".to_string()];
        let cfg = CalCfg { first_day: 0, weekday_format: "short", week_numbers: true, navigation: true, min: None, max: None, read_only: false, day_parts: &parts };
        let d: Date = "2026-10-22".parse().unwrap();
        assert_eq!(cfg.names_of(d).collect::<Vec<_>>(), vec!["milestone", "release"]);
        assert_eq!(cfg.names_of("2026-10-02".parse().unwrap()).count(), 0);
        assert_eq!(cfg.weekday_label(6), "Su");
    }
}
