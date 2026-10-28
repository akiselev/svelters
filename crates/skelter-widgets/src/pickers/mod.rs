//! The pickers (svelters-egui `widgets/{calendar,date_picker,color_editor,color_picker,picker_popup,
//! picker_glyphs}.rs`, `kinds/pickers.rs`; OW6): `calendar` and `color-editor` (inline), and
//! `date-picker` and `color-picker` (a drop button showing the same body in a popup).
//!
//! Values are strings, the same on every backend: a date is ISO 8601 (`2026-10-22`, `""` for none);
//! a colour is CSS hex, `#rrggbb` when opaque and `#rrggbbaa` otherwise (`#rgb`/`#rgba` accepted on
//! write and stored normalized).
//!
//! Each kind is a plain state struct: pointer events arrive in the kind's local points, keys as
//! skelter-base [`Key`](skelter_base::key::Key)s, and the struct returns typed outcomes plus the
//! JS-facing [`Emit`](crate::table::shim::Emit)s; `record` writes its display list. The popup's
//! placement is the views lane's (`popup` placement): the drop kinds only report the popup's size
//! for an anchor rect ([`popup::DropState`]).
//!
//! | Module | What |
//! |---|---|
//! | [`date`] | civil dates: days-from-civil arithmetic, ISO 8601 parse/format, ISO weeks, `strftime` |
//! | [`calendar`] | the month grid model, keys, pointer, record (the `calendar` kind and the date popup's body) |
//! | [`color`] | sRGB ↔ HSV/HSL, CSS hex parse/format |
//! | [`color_editor`] | the SV square, hue and alpha strips, channel boxes, swatches (the `color-editor` kind) |
//! | [`popup`] | the drop button's open/close, the Save/Cancel footer, chevron glyphs |
//! | [`date_picker`], [`color_picker`] | the drop kinds |
//! | [`spec`] | the four kinds' prop and signal names (the svelters spec rows) |
//! | [`kinds`] | the skelter-core `KindSpec`s, and the `Kind` impls of `calendar` and `color-editor` |

pub mod calendar;

/// The date the pickers take as today (the calendar's today mark, the month an empty picker opens
/// on): a document service ([`set_today`]), so the gallery and tests show the same day every run;
/// `None` (or no service): today in UTC ([`date::Date::today_utc`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Today(pub Option<date::Date>);

/// Sets the pickers' today for `doc` (`None`: the platform clock's, in UTC); they read it on their
/// next prepare.
pub fn set_today(doc: &mut skelter_core::Document, today: Option<date::Date>) {
    doc.set_service(Today(today));
    doc.rerecord_all();
}

/// `doc`'s today for the pickers.
pub fn today_of(doc: &skelter_core::Document) -> date::Date {
    doc.service_ref::<Today>().and_then(|t| t.0).unwrap_or_else(date::Date::today_utc)
}

pub mod color;
pub mod color_editor;
pub mod color_picker;
pub mod date;
pub mod date_picker;
pub mod kinds;
pub mod popup;
pub mod spec;

use skelter_base::color::Color;
use skelter_base::event::{PointerEvent, PointerPhase};
use skelter_base::geom::Point;

/// The colours and metrics the pickers paint with: the DESIGN tokens they read in svelters-egui's
/// UA tables (`fg`, `list-sel`, `control-border`, `row-h`, …). The default is VS Code Light Modern
/// at the default density (svelters-egui `theme/data/vscode-light-modern.json`, `density.json`,
/// `shape.json`); the `Kind` impls fill it from the computed style once skelter-core's style lands.
#[derive(Clone, Debug, PartialEq)]
pub struct PickerLook {
    pub fg: Color,
    pub muted: Color,
    pub disabled_fg: Color,
    pub icon: Color,
    pub accent: Color,
    pub accent_hover: Color,
    pub accent_fg: Color,
    pub focus: Color,
    pub control_bg: Color,
    pub control_border: Color,
    pub control_fg: Color,
    pub toolbar_hover: Color,
    pub invalid_border: Color,
    pub list_hover: Color,
    pub list_sel: Color,
    pub list_sel_fg: Color,
    /// The popup surface (`menu-bg` = `surface-raised`) and its border (`menu-border-color`).
    pub popup_bg: Color,
    pub popup_border: Color,
    pub button_bg: Color,
    pub button_bg_hover: Color,
    pub button_fg: Color,
    /// The primary (Save) button's edge (`button-border`, VS Code's `button.border`): transparent in
    /// the Modern themes, the contrast border in the high-contrast ones, where its fill alone is black.
    pub button_border: Color,
    /// A high-contrast theme: selections are outlined (their fills are black or white there).
    pub high_contrast: bool,
    /// `row-h`: list rows, the calendar header.
    pub row_h: f32,
    /// `control-h`: fields and footer buttons.
    pub control_h: f32,
    /// `radius` (`input-radius`, `button-radius`) and `radius-md` (`menu-radius`).
    pub radius: f32,
    pub radius_md: f32,
    /// `stroke`.
    pub stroke: f32,
    /// `font-size`.
    pub font_size: f32,
    pub font_family: String,
}

fn hex(s: &str) -> Color {
    let [r, g, b, a] = color::parse_hex(s).unwrap_or([0, 0, 0, 255]);
    Color::from_rgba8(r, g, b, a)
}

impl Default for PickerLook {
    fn default() -> Self {
        PickerLook {
            fg: hex("#3b3b3b"),
            muted: hex("#3b3b3b"),
            disabled_fg: hex("#61616180"),
            icon: hex("#3b3b3b"),
            accent: hex("#005fb8"),
            accent_hover: hex("#0258a8"),
            accent_fg: hex("#ffffff"),
            focus: hex("#005fb8"),
            control_bg: hex("#ffffff"),
            control_border: hex("#cecece"),
            control_fg: hex("#3b3b3b"),
            toolbar_hover: hex("#b8b8b850"),
            invalid_border: hex("#be1100"),
            list_hover: hex("#f2f2f2"),
            list_sel: hex("#e8e8e8"),
            list_sel_fg: hex("#000000"),
            popup_bg: hex("#f8f8f8"),
            popup_border: hex("#e5e5e5"),
            button_bg: hex("#e5e5e5"),
            button_bg_hover: hex("#cccccc"),
            button_fg: hex("#3b3b3b"),
            button_border: Color::TRANSPARENT,
            high_contrast: false,
            row_h: 22.0,
            control_h: 26.0,
            radius: 4.0,
            radius_md: 6.0,
            stroke: 1.0,
            font_size: 13.0,
            font_family: "system-ui".into(),
        }
    }
}

impl PickerLook {
    /// The body text style in `color`.
    pub fn text(&self, color: Color) -> crate::table::shim::TextStyle {
        crate::table::shim::TextStyle { family: self.font_family.clone(), size: self.font_size, weight: 400, italic: false, color }
    }
}

/// What a pointer event means to a pickers' sub-control: presses, moves and releases of the primary
/// button in the kind's local points (svelters-egui used egui's click semantics: a click is a press
/// and a release on the same sub-control).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Ptr {
    Down(Point),
    Move(Point),
    Up(Point),
    Leave,
}

impl Ptr {
    /// The primary-button (or touch/pen) meaning of `ev`; `None` for other buttons.
    pub(crate) fn of(ev: &PointerEvent) -> Option<Ptr> {
        use skelter_base::event::MouseButton;
        let primary = ev.button.is_none_or(|b| b == MouseButton::Primary);
        match ev.phase {
            PointerPhase::Down if primary => Some(Ptr::Down(ev.pos)),
            PointerPhase::Up if primary => Some(Ptr::Up(ev.pos)),
            PointerPhase::Move | PointerPhase::Enter => Some(Ptr::Move(ev.pos)),
            PointerPhase::Leave | PointerPhase::Cancel => Some(Ptr::Leave),
            _ => None,
        }
    }
}

#[cfg(test)]
pub(crate) mod test_util {
    use skelter_base::event::{MouseButton, PointerEvent, PointerPhase, PointerType};
    use skelter_base::geom::{Point, Vec2};
    use skelter_base::smallvec::SmallVec;

    pub fn ptr(phase: PointerPhase, x: f32, y: f32) -> PointerEvent {
        let button = matches!(phase, PointerPhase::Down | PointerPhase::Up).then_some(MouseButton::Primary);
        PointerEvent {
            phase,
            pointer: 0,
            kind: PointerType::Mouse,
            button,
            buttons: if phase == PointerPhase::Down { 1 } else { 0 },
            pos: Point::new(x, y),
            modifiers: Default::default(),
            pressure: 0.0,
            tilt: Vec2::ZERO,
            history: SmallVec::new(),
        }
    }
}
