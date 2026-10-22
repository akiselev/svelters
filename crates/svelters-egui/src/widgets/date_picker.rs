//! `date-picker`, svelters' own (OW6; ARCH-WIDGETS §6.1, WIDGET-PARITY §2.19 `QDateEdit` with
//! `calendarPopup`): a drop button showing the date (`format`) with R14's chevron
//! (`::part(chevron)`), opening the calendar ([`CalBody`], the `calendar` kind's parts) in a popup
//! (`::part(popup)`, OW3's Anchored and Dismissable). `commit-mode`: `confirm` (the default, as the
//! egui wrapper: a pick is pending until Save; Enter saves, Cancel or Escape drops it) or
//! `immediate` (Qt: a pick commits and closes).
//!
//! States: `:hover`, `:active`, `:focus-visible` (R8's field ring), `:open`, `:read-only`,
//! `:disabled` (R9). Keys, closed: Enter, Space, F4 or Alt+Down open it. Open: the calendar takes
//! the keyboard (arrows, Home/End, PageUp/PageDown, Enter), Escape closes it and focus returns to
//! the button. AccessKit: a `Button` named by the shown date with `expanded` and `has_popup`;
//! the popup's grid is the calendar's. Writer lane: OW6.

use super::calendar::{self, CalBody, CalCfg, CalOut, IsoDate, WEEKDAY_FORMAT};
use super::picker_glyphs::{self, Dir};
use super::picker_popup::{self, BUTTON_UA, CHEVRON_UA, Drop, Footer, POPUP_UA};
use crate::interact::{Focusable, Hoverable, Pressable};
use crate::paint::{self, Align, Ua};
use crate::provider::{A11yCx, AvailableSpace, Event, MeasureCx, Part, StateBits, Widget, WidgetCx};
use egui::{Key, Modifiers, Rect, Vec2, accesskit, pos2, vec2};

/// `commit-mode` values of `date-picker`.
pub const DATE_COMMIT_MODE: &[&str] = &["confirm", "immediate"];

pub const UA: Ua = BUTTON_UA;

/// A date drop button with a calendar popup.
// the `#[part]` fields name the kind's `::part()`s; nothing reads them (paint parts)
#[allow(dead_code)]
#[svelters::widget(namespace = "sv", replaces = "date-picker", caps = FOCUSABLE | PAINTS_BOX)]
pub struct DatePicker {
    #[prop(bind, measure)]
    value: IsoDate,
    #[prop(default = "%Y-%m-%d", measure)]
    format: String,
    #[prop(default = true)]
    calendar_week: bool,
    #[prop]
    min: IsoDate,
    #[prop]
    max: IsoDate,
    #[prop(default = "auto")]
    first_day: String,
    #[prop(default = "short")]
    weekday_format: String,
    #[prop(default = true)]
    navigation: bool,
    #[prop]
    read_only: bool,
    #[prop]
    day_parts: Vec<String>,
    #[prop(default = "confirm")]
    commit_mode: String,
    #[event]
    changed: Event<String>,
    #[event]
    commit: Event<String>,
    #[event]
    page_changed: Event<Vec<i64>>,
    #[behavior]
    hover: Hoverable,
    #[behavior]
    focus: Focusable,
    #[behavior]
    pressable: Pressable,
    #[state]
    body: CalBody,
    #[state]
    drop: Drop,
    #[state]
    open: bool,
    /// The day picked in `confirm` mode, until Save.
    #[state]
    pending: Option<jiff::civil::Date>,
    #[part]
    chevron: Part,
    #[part]
    popup: Part,
    #[part]
    header: Part,
    #[part]
    prev: Part,
    #[part]
    next: Part,
    #[part]
    month: Part,
    #[part]
    year: Part,
    #[part]
    grid: Part,
    #[part]
    day: Part,
    #[part]
    weekday: Part,
    #[part(name = "week-number")]
    week_number: Part,
    #[part]
    save: Part,
    #[part]
    cancel: Part,
}

impl DatePicker {
    /// The button's text: the value (else today) in `format`.
    fn text(&self) -> String {
        let d = self.value.date().unwrap_or_else(calendar::today);
        let mut s = String::new();
        // a bad `format` shows ISO (jiff's strftime reports the error through fmt)
        if std::fmt::write(&mut s, format_args!("{}", d.strftime(&self.format))).is_err() {
            return d.to_string();
        }
        s
    }

    fn confirm(&self) -> bool {
        super::pick(&self.commit_mode, DATE_COMMIT_MODE, "confirm") == "confirm"
    }

    fn set_open(&mut self, cx: &mut WidgetCx<'_>, ctx: &egui::Context, on: bool, keyboard: bool) {
        if on && !self.open {
            self.drop.opening(ctx, keyboard);
        }
        self.open = on;
        cx.set_state(StateBits::OPEN, on);
    }

    /// The user's pick: `changed` (default: the value), then `commit`.
    fn apply(&mut self, cx: &mut WidgetCx<'_>, d: jiff::civil::Date) {
        let iso = d.to_string();
        if iso != self.value.0 && !cx.dispatch(&self.changed, iso.clone()).prevented() {
            cx.set(&mut self.value, IsoDate(iso.clone()));
        }
        cx.dispatch(&self.commit, self.value.0.clone());
    }
}

impl Widget for DatePicker {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, _avail: AvailableSpace) -> Vec2 {
        let s = cx.style(UA);
        let t = paint::measure_text(cx.ctx, &s, &self.text(), AvailableSpace::MaxContent);
        let c = s.part(cx.part_css("chevron"), CHEVRON_UA);
        let lh = s.px("line-height").unwrap_or(t.y);
        vec2(t.x + super::parts::gap(&s, 4.0) + c.px("width").unwrap_or(16.0), t.y.max(lh)) + paint::box_extra(&s)
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        let ctx = ui.ctx().clone();
        self.drop.begin(&ctx);
        cx.set_state(StateBits::READ_ONLY, self.read_only);
        cx.set_state(StateBits::OPEN, self.open);
        let resp = cx.interact(ui, self.units_sense());
        let fired = self.run_units(cx, ui, &resp);
        let enabled = cx.enabled();
        let host_id = resp.id;
        if enabled && fired.pressable.iter().any(|p| p.is_click()) {
            let kb = ctx.input(|i| i.key_pressed(Key::Enter) || i.key_pressed(Key::Space));
            let on = !self.open;
            self.set_open(cx, &ctx, on, kb);
        } else if enabled && !self.open && resp.has_focus() && Drop::open_keys(&ctx) {
            self.set_open(cx, &ctx, true, true);
        }
        if !enabled && self.open {
            self.set_open(cx, &ctx, false, false);
        }
        if self.drop.just_opened(self.open) {
            self.body.reset(self.value.date());
            self.pending = None;
        }

        // ---- the popup ----
        if self.open {
            if self.drop.hold(&ctx, host_id) {
                self.set_open(cx, &ctx, false, false);
            }
        }
        if self.open {
            let confirm = self.confirm();
            let shown = if confirm { self.pending.or(self.value.date()) } else { self.value.date() };
            let keys = self.drop.has_keys(&ctx, host_id);
            let ring = self.drop.keyboard;
            let host = cx.style(UA);
            let surf = cx.part(&host, "popup", POPUP_UA);
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
            let body_size = CalBody::measure(&surf, &cfg);
            let frame = Drop::frame(&ctx, &surf, resp.rect, picker_popup::popup_size(&surf, body_size, confirm));
            if self.drop.dismissed(&ctx, frame, resp.rect) {
                self.open = false;
            } else {
                let geom = paint::box_geom(&surf, frame);
                let pid = Drop::id(host_id);
                let body = &mut self.body;
                let enter = keys && ctx.input(|i| i.key_pressed(Key::Enter));
                let res = self.drop.show(&ctx, host_id, frame, |pui| {
                    paint::paint_box(&mut cx.paint_cx(pui).part("popup"), &surf, frame);
                    let br = Rect::from_min_size(geom.content.min, body_size);
                    let outs = body.ui(pui, cx, &surf, &cfg, shown, br, keys, ring || keys, pid);
                    let foot = confirm.then(|| {
                        let fr = Rect::from_min_max(pos2(geom.content.left(), geom.content.bottom() - picker_popup::footer_height(surf.is_dark())), geom.content.max);
                        picker_popup::footer(pui, cx, &surf, pid, fr)
                    });
                    (outs, foot.flatten())
                });
                if keys {
                    ctx.input_mut(|i| {
                        for k in [Key::ArrowUp, Key::ArrowDown, Key::ArrowLeft, Key::ArrowRight, Key::Home, Key::End, Key::PageUp, Key::PageDown, Key::Enter, Key::Space, Key::Tab] {
                            i.consume_key(Modifiers::NONE, k);
                        }
                    });
                }
                if let Some((outs, foot)) = res {
                    for o in outs {
                        match o {
                            CalOut::Pick(d) if !confirm || enter => {
                                self.set_open(cx, &ctx, false, false);
                                self.apply(cx, d);
                            }
                            CalOut::Pick(d) => self.pending = Some(d),
                            CalOut::Page(y, m) => {
                                cx.dispatch(&self.page_changed, vec![y as i64, m as i64]);
                            }
                        }
                    }
                    match foot {
                        Some(Footer::Save) => {
                            self.set_open(cx, &ctx, false, false);
                            if let Some(d) = self.pending.take() {
                                self.apply(cx, d);
                            }
                        }
                        Some(Footer::Cancel) => self.set_open(cx, &ctx, false, false),
                        None => {}
                    }
                }
            }
        }
        self.drop.end(&ctx, host_id, self.open);
        cx.set_state(StateBits::OPEN, self.open);
        cx.commit_state();

        // ---- the closed button ----
        let host = cx.style(UA);
        if host.state().contains(StateBits::HOVERED)
            && let Some(c) = host.cursor()
        {
            ctx.set_cursor_icon(c);
        }
        let text = self.text();
        if ui.is_rect_visible(cx.rect()) {
            let g = cx.paint_box(ui, UA);
            let c = cx.part(&host, "chevron", CHEVRON_UA);
            let (cr, lr) = picker_popup::split_chevron(&host, &c, g.content);
            let mut pc = cx.paint_cx(ui);
            picker_glyphs::chevron(&mut pc.part("chevron"), &c, Dir::Down, cr);
            let gal = paint::text_galley(&ctx, &host, &text, None);
            paint::paint_text(&mut pc.part("label"), &host, &gal, lr, Align::Start);
            cx.paint_outline(ui, UA);
        }
        let open = self.open;
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, &text));
        ctx.accesskit_node_builder(resp.id, |n| {
            n.set_expanded(open);
            n.set_has_popup(accesskit::HasPopup::Dialog);
        });
        resp
    }

    fn a11y(&self, _cx: &A11yCx<'_>, node: &mut accesskit::Node) {
        node.set_role(accesskit::Role::Button);
        self.units_a11y(node);
        if self.read_only {
            node.set_read_only();
        }
    }

    fn consume_key(&self, key: Key, m: Modifiers) -> bool {
        self.units_claims(key, m) || (!m.any() && key == Key::F4) || (m.alt && key == Key::ArrowDown) || (self.open && (key == Key::Escape || CalBody::claims(key, m)))
    }
}
